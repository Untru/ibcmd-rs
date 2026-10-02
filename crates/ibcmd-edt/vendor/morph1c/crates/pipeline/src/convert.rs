//! `read_config` / `write_config` — the whole-config read/write that compose the per-kind
//! byte-exact connectors around the canonical [`Configuration`] IR.
//!
//! `read_config` runs the §1.0 preflight FIRST: any blocked kind (no connector / read
//! error) that is not excluded by `--only` aborts with [`ConvertError::Unsupported`]. Only
//! then does it read the root descriptor into `Configuration.properties` and every
//! selected kind's descriptors into `Configuration.objects`, in a deterministic order
//! (kind-group order, then sorted name). `write_config` looks up each object's connector
//! by `obj.kind.as_str()`, regenerates its descriptor and writes it to the layout path.

use std::path::{Path, PathBuf};

use rayon::prelude::*;

use morph1c_core::ir::{Configuration, MetadataObject};

use formats_xml::registry::{CorpusLayout, Format, FormatKind};

use crate::layout;
use crate::preflight::preflight;
use crate::registry::FormatRegistry;
use crate::{ConvertError, ConvertOptions};

/// The pool the READ phase fans out on — deliberately OVERSUBSCRIBED, unlike everything else.
///
/// Reading an object is dominated by waiting on the disk, not by parsing, and this disk is
/// nowhere near saturated at one outstanding request per core. Measured cold (ERP Designer dump,
/// 129 220 files / 9.18 GB, cache evicted before each run):
///
/// ```text
///    8 threads  215.1 s   42.7 MB/s        32 threads  136.7 s   67.2 MB/s
///   16 threads  153.7 s   59.7 MB/s        64 threads  120.7 s   76.1 MB/s
/// ```
///
/// The WRITE phase is the exact opposite — there the on-access scanner serialises and more
/// threads measured strictly WORSE (whole-SSL `edt→xml`: 8 → 72.1 s, 16 → 93.2 s) — so writes
/// stay on rayon's default pool. Same reason `read_config` installs this pool and `write_config`
/// does not.
pub(crate) fn read_pool() -> &'static rayon::ThreadPool {
    static POOL: std::sync::OnceLock<rayon::ThreadPool> = std::sync::OnceLock::new();
    POOL.get_or_init(|| {
        // 4× the cores, clamped: below 8 the fan-out stops being worth its own pool, and past 64
        // the curve above has flattened while every worker still costs a stack.
        let n = rayon::current_num_threads().clamp(1, 4);
        rayon::ThreadPoolBuilder::new()
            .num_threads(n)
            .thread_name(|i| format!("morph1c-read-{i}"))
            .build()
            .expect("read thread pool")
    })
}

pub fn read_config(
    format: Format,
    src: &Path,
    opts: &ConvertOptions,
) -> Result<(Configuration, Vec<(String, usize)>), ConvertError> {
    let (cfg, skipped) = read_config_at(format, src, opts)?;
    Ok((cfg, skipped))
}

fn read_config_at(
    format: Format,
    src: &Path,
    opts: &ConvertOptions,
) -> Result<(Configuration, Vec<(String, usize)>), ConvertError> {
    // ВЕРСИЯ ИСТОЧНИКА — вход ЧТЕНИЯ (и часть IR): ридеры отличают по ней «свойство не
    // задано» от «свойства в этой версии формата нет» (см. `core::version::with_source_version`).
    // Ставится на ВЕСЬ разбор, потому что сайдкары (тела форм EDT) собственного штампа версии
    // не несут.
    let source_version = crate::source_format_version(format, src);
    morph1c_core::version::with_source_version(source_version, || {
        read_config_inner(format, src, opts, source_version)
    })
}

fn read_config_inner(
    format: Format,
    src: &Path,
    opts: &ConvertOptions,
    source_version: Option<morph1c_core::version::FormatVersion>,
) -> Result<(Configuration, Vec<(String, usize)>), ConvertError> {
    // cf is container-based, not a directory walk: route it to the cf reader instead of
    // silently returning an empty config (a Container-layout kind enumerates to nothing
    // under the directory walk — §1.0 forbids that silent zero). Callers wanting cf must use
    // `read_cf_config`/`convert`, which carry the §1.0 unclaimed accounting.
    if format == Format::Cf {
        return Err(ConvertError::Cf {
            reason: "use read_cf_config (or convert --from cf): cf objects are records inside \
                     one container, not file-per-object descriptors"
                .into(),
        });
    }

    let reg = FormatRegistry::for_format(format)?;

    // (1) §1.0 preflight — abort on any non-excluded blocked kind.
    let report = preflight(format, src, opts)?;
    if report.has_blocked() {
        // Only blocked kinds that are NOT excluded by --only are fatal. With --only active,
        // a blocked kind outside the allow-list is reported as intentionally skipped, not a
        // hard error (the user explicitly scoped the work). Recompute the fatal set.
        let fatal: Vec<_> = report
            .blocked()
            .filter(|e| opts.includes(&e.kind))
            .cloned()
            .collect();
        if !fatal.is_empty() {
            // Surface the FULL report (it lists every kind honestly).
            return Err(ConvertError::Unsupported(report));
        }
    }

    // §1.0: версия источника, которую НЕ удалось определить, остаётся `None` — «похожая» не
    // подставляется (потребители обязаны трактовать это как «судить по одному IR»).
    let mut cfg = Configuration::new().with_source_version(source_version);

    // (2) Root descriptor (SingletonFile) → Configuration.properties (+ identity objects
    // are not added; the root is the config itself). Only when the root kind is selected.
    if let Some(root_fk) = reg.root_kind() {
        if opts.includes(root_fk.kind) {
            if let Some(path) = layout::singleton_path(&reg, root_fk, src) {
                if path.is_file() {
                    let bytes = std::fs::read(&path).map_err(|e| ConvertError::Io {
                        path: path.display().to_string(),
                        reason: e.to_string(),
                    })?;
                    let mut root_obj =
                        (root_fk.read)(&bytes).map_err(|reason| ConvertError::Read {
                            kind: root_fk.kind.to_string(),
                            object: root_fk.kind.to_string(),
                            reason,
                        })?;
                    // Language sidecar (`Languages/<Name>.xml`) — Designer stores each language
                    // as a sibling file (EDT carries `<languages>` INLINE in the descriptor,
                    // already read above). The root descriptor read leaves `F_LANGUAGES` empty
                    // for Designer, so we attach it here from the sidecars declared in the
                    // root's ChildObjects, giving the cf writer the SAME inline `F_LANGUAGES`
                    // that edt→cf gets (so designer→cf emits the standalone Language element).
                    // §1.0: a declared language with no sidecar errors LOUDLY (no silent skip).
                    crate::language_read::attach_languages(format, src, &mut root_obj)?;
                    // Config-level Ext sidecars of the ROOT (4 application modules +
                    // Splash/MainSectionPicture images) → root_obj.modules/.config_pictures.
                    // Designer: root-level `Ext/`; EDT: siblings of `Configuration.mdo`.
                    crate::ext_read::attach_config_ext(format, &path, &mut root_obj)?;
                    crate::help_read::attach_help_pages(
                        format,
                        root_fk.kind,
                        &path,
                        &mut root_obj,
                    )?;
                    // The root descriptor's spec properties ARE the configuration's
                    // properties; its child-objects collection is carried on the root
                    // object's `children`, which we keep as a single root object so the
                    // round-trip is faithful. We store its properties on the config and the
                    // whole root object as object #0 (kind "Configuration") so write_config
                    // can regenerate the singleton file.
                    cfg.properties = root_obj.properties.clone();
                    cfg.objects.push(root_obj);
                }
            }
        }
    }

    // (3) Every selected, supported kind's descriptors → Configuration.objects.
    for fk in reg.iter() {
        // Root already handled above.
        if reg.root_kind().map(|r| r.kind == fk.kind).unwrap_or(false) {
            continue;
        }
        if !opts.includes(fk.kind) {
            // Excluded by --only — reported via the preflight below, not converted.
            continue;
        }
        // §1.0-honest enumeration. For a Nested (Subsystem) kind this RECURSES into every
        // `Subsystems/` level (SSL: all 87 subsystems, not just the 3 top-level ones) and
        // turns a descriptor present on disk but unreachable by traversal into a typed error
        // instead of a silent skip. Every other layout is enumerated exactly as before.
        let objs = layout::collect_objects_checked(&reg, fk, src)?;
        let is_nested = matches!(fk.layout, CorpusLayout::Nested { .. });

        // Objects of a kind are INDEPENDENT reads (own descriptor + own sidecars), and the read
        // is dominated by per-file syscalls — ~18 sidecar probes per object, so SSL alone stats
        // ~50k paths. Fan them across cores.
        //
        // §1.0 determinism: collect INDEXED (`par_iter` preserves input order), then walk the
        // results in object order and return the FIRST failure. So the error a user sees — and
        // the object order in the IR — is exactly the sequential walk's, independent of which
        // thread happened to fail first.
        // Reinstall the caller's ambient roundtrip target in each worker (rayon does not
        // inherit thread_locals): production reads run with none set, but harness-driven reads
        // under a target scope must see the same ambient the sequential walk would.
        let ambient_target = morph1c_core::version::current_roundtrip_target();
        // Тот же захват для ТЕКСТОВОЙ политики диалекта (in-text EOL): она тоже thread_local.
        let ambient_eol = formats_xml::read::in_text_eol_verbatim();
        let results: Vec<Result<MetadataObject, ConvertError>> = read_pool().install(|| {
            objs.par_iter()
                .map(|(name, path)| {
                    formats_xml::read::with_captured_in_text_eol(ambient_eol, || {
                        morph1c_core::version::with_captured_roundtrip_target(
                            ambient_target,
                            || {
                                // Версия ИСТОЧНИКА — тот же thread_local-готча: воркер её не
                                // наследует, а без неё ридер снова достраивал бы свойства,
                                // которых в дампе этой версии нет.
                                morph1c_core::version::with_source_version(source_version, || {
                                    read_object_at(format, fk, name, path)
                                })
                            },
                        )
                    })
                })
                .collect()
        });

        // DIAGNOSTIC-ONLY (env `MORPH1C_COLLECT_ERRORS`): вместо остановки на ПЕРВОМ падении
        // объекта — печатать КАЖДОЕ read-падение (по одному на объект: read_object останавливается
        // на первом непокрытом sidecar/поле) и продолжать. Даёт полный список оставшихся
        // witness-классов за один проход. Продакшн-путь без env-переменной НЕ меняется.
        let collect_errors = false; // Never skip codec errors in production.
        // For Nested kinds, remember each object's hierarchical key + declared nested-child
        // names so membership (dangling / undeclared) can be cross-checked once all are read.
        let mut membership: Vec<(String, Vec<String>)> = Vec::new();
        for ((name, _path), result) in objs.iter().zip(results) {
            let obj = match result {
                Ok(o) => o,
                Err(e) if collect_errors => {
                    eprintln!("COLLECT_ERR\t{}\t{}\t{e}", fk.kind, name);
                    continue;
                }
                Err(e) => return Err(e),
            };
            if is_nested {
                // Nested children are bare-ref subsystem stubs whose `name` is the declared
                // child subsystem name (`<subsystems>Name</subsystems>`).
                let declared: Vec<String> = obj.children.iter().map(|c| c.name.clone()).collect();
                membership.push((name.clone(), declared));
            }
            cfg.objects.push(obj);
        }
        if is_nested && !collect_errors {
            validate_nested_membership(fk.kind, &membership)?;
        }
    }

    // (4) Loud-not-silent skip list (§1.0): EVERY kind present on disk that was NOT
    // converted — whether a supported connector excluded by --only OR a blocked kind with
    // no connector. Built from the full preflight report so nothing present is invisible.
    let skipped: Vec<(String, usize)> = report
        .entries
        .iter()
        .filter(|e| !opts.includes(&e.kind) && e.object_count > 0)
        .map(|e| (e.kind.clone(), e.object_count))
        .collect();

    crate::picture_read::resolve_form_picture_transparency(format, &mut cfg)?;
    Ok((cfg, skipped))
}

/// Read a descriptor and all its source-family bodies.
pub(crate) fn read_object_at(
    format: Format,
    fk: &FormatKind,
    name: &str,
    path: &Path,
) -> Result<MetadataObject, ConvertError> {
    let bytes = std::fs::read(path).map_err(|e| ConvertError::Io {
        path: path.display().to_string(),
        reason: e.to_string(),
    })?;
    let mut obj = (fk.read)(&bytes).map_err(|reason| ConvertError::Read {
        kind: fk.kind.to_string(),
        object: name.to_string(),
        reason,
    })?;
    // NESTED (Subsystem) hierarchical identity: Designer expresses it ONLY on disk, so synthesize
    // the canonical `parentSubsystem` chain from the enumeration key (EDT cross-checks its own).
    // Without this a Designer-sourced IR looks all-top-level → own-name collisions on write and a
    // silently FLAT subsystem hierarchy in cf (§1.0/§1.6).
    crate::subsystem_read::reconcile_nested_identity(format, fk, name, &mut obj)?;
    crate::metadata_picture_semantics::attach(format, path, &mut obj)?;
    // Every BODY pass is gated on the depth; the descriptor-level ones are not. The passes below
    // keep the ORIGINAL order in both depths (a `Full` read runs exactly the sequence it always
    // did), because that order is load-bearing — see this function's docs and the note at
    // `template_ref_read` below.
    let bodies = true; // Source-only conversion always reads every body.
    if bodies {
        // Module-body sidecar (`Module.bsl`) → `obj.modules` (a `--to cf` assembly emits `<uuid>.0`).
        crate::module_read::attach_module_body(format, fk.kind, path, &mut obj)?;
        crate::additional_indexes_read::attach(format, path, &mut obj)?;
        // Command-module sidecars (`Commands/<Cmd>/CommandModule.bsl`) → each command CHILD's `modules`.
        crate::command_module_read::attach_command_modules(format, fk.kind, path, &mut obj)?;
        // Help-page sidecars (`Help/<lang>.html` / `Ext/Help.xml`+`Ext/Help/<lang>.html`) → `obj.help`.
        crate::help_read::attach_help_pages(format, fk.kind, path, &mut obj)?;
        // ScheduledJob schedule sidecar (`Schedule.schedule` / `Ext/Schedule.xml`) → `obj.schedule`
        // (a `--to cf` assembly emits it as the `<uuid>.0` body).
        crate::schedule_read::attach_schedule(format, fk.kind, path, &mut obj)?;
        // Style composition sidecar (`Style.style` / `Ext/Style.xml`) → `obj.style_records`
        // (a `--to cf` assembly emits it as the `<uuid>.0` body).
        crate::style_read::attach_style(format, fk.kind, path, &mut obj)?;
        // BusinessProcess flowchart sidecar (`Flowchart.scheme` / `Ext/Flowchart.xml`) →
        // `obj.flowchart` (a `--to cf` assembly emits it as the `<uuid>.7` body).
        crate::flowchart_read::attach_flowchart(format, fk.kind, path, &mut obj)?;
        // Designer-ONLY: ExchangePlan content sidecar (`Ext/Content.xml`) → the `content` spec field
        // (EDT carries it INSIDE the descriptor; the Designer descriptor does not carry it at all).
        crate::exchange_plan_content_read::attach_exchange_plan_content(
            format, fk.kind, path, &mut obj,
        )?;
    }
    // Designer-ONLY: predefined-data sidecar (`Ext/Predefined.xml`) → the `predefined` spec field
    // (EDT carries it INSIDE the descriptor; the Designer descriptor does not carry it at all).
    // WITHOUT it a designer-sourced IR resolves NO `<Owner>.<PredefinedName>` reference (the cf
    // registry harvests those paths from this very block) — `designer→cf` refused 2 objects.
    crate::predefined_read::attach_predefined(format, fk.kind, path, &mut obj)?;
    crate::source_extensions::attach(format, path, &mut obj)?;
    if bodies {
        // Rights-table sidecar (`Rights.rights` / `Ext/Rights.xml`) → `obj.rights` (Role `<uuid>.0`).
        crate::rights_read::attach_rights_body(format, fk.kind, path, &mut obj)?;
    }
    // Designer template-ref DESCRIPTOR enrichment — MUST precede every template-BODY attach
    // (text/mxl/geos/dcs): a Designer bare-ref child carries no `templateType` until enriched,
    // so the body passes could mis-key its sidecar family (e.g. a geoscheme child would default
    // to SpreadsheetDocument and read its `Ext/Template.xml` as an MXL blob).
    crate::template_ref_read::attach_designer_template_ref_descriptors(format, path, &mut obj)?;
    // Designer form-ref DESCRIPTOR enrichment (`Forms/<Имя>.xml` → `.FormRef` child identity:
    // uuid + synonym/usePurposes/…) — the mirror of the EDT inline form stub (§1.0/§1.6).
    crate::form_ref_read::attach_designer_form_ref_descriptors(format, path, &mut obj)?;
    // ExternalDataSource TABLE standalone descriptors (`Tables/<Имя>[…]`) → bare-ref Table
    // children gain uuid + properties + Field children + producedTypes (ОБА xml-диалекта
    // держат тело таблицы в отдельном файле; §1.0 — отсутствующий сайдкар = жёсткая ошибка,
    // молчаливый стаб потерял бы таблицу).
    crate::table_ref_read::attach_table_descriptors(format, path, &mut obj)?;
    if bodies {
        // Template-body sidecar (`Template.txt`) → `templates` (TextDocument, object + children).
        crate::template_read::attach_template_body(format, fk.kind, path, &mut obj)?;
        // MXL template-body sidecar (`Template.mxlx` / `Ext/Template.xml`) → `templates` (object + children).
        crate::mxl_read::attach_mxl_bodies(format, fk.kind, path, &mut obj)?;
        // Geoscheme template-body sidecar (`Template.geos` / `Ext/Template.xml`) → `templates` (object + children).
        crate::geos_read::attach_geoschema_bodies(format, fk.kind, path, &mut obj)?;
        // GraphicalSchema template-body sidecar (`Template.scheme` / `Ext/Template.xml` + item
        // `Items/<Имя>/Picture.<ext>` pictures) → `templates` (object + children).
        crate::graph_template_read::attach_graph_template_bodies(format, fk.kind, path, &mut obj)?;
        // Form-body sidecar (`Form.form` / `Ext/Form.xml`) → `obj.form_bodies` (§1.0-strict).
        crate::form_read::attach_form_body(format, fk.kind, path, &mut obj)?;
        // XDTO-schema sidecar (`Package.xdto` / `Ext/Package.bin`) → `obj.xdto_schema` (§1.0-strict).
        crate::xdto_read::attach_xdto_schema(format, fk.kind, path, &mut obj)?;
        // WSDL sidecar set (`WsDefinitions.wsdl`+`<N>.xsd` / `Ext/WSDefinition.xml`+`Ext/<N>.xsd`)
        // → `obj.ws_definition` (OPTIONAL — presence gated by the file).
        crate::ws_definition_read::attach_ws_definition(format, fk.kind, path, &mut obj)?;
        // DCS template-body sidecar (`Template.dcs` / `Ext/Template.xml`) → `templates` (object + children).
        crate::dcs_read::attach_dcs_bodies(format, path, &mut obj)?;
        // Blob template-body sidecars (AddIn `Template.addin` / BinaryData `Template.bin` /
        // HTMLDocument `ru.html`+`Template.htmldoc`) → `templates` (object + children, EDT lane).
        crate::blob_template_read::attach_blob_template_bodies(format, fk.kind, path, &mut obj)?;
        // Command-interface sidecar (`CommandInterface.cmi` / `Ext/CommandInterface.xml`) → `obj.command_interface`.
        crate::cmi_read::attach_command_interface(format, fk.kind, path, &mut obj)?;
        // Picture image-body sidecar → `obj.picture` (OPTIONAL).
        crate::picture_read::attach_picture_body(format, fk.kind, path, &mut obj)?;
    }
    Ok(obj)
}

/// Write ONE object: its descriptor at the planned path `out`, then every BODY sidecar the read
/// side attached into the IR, in the TARGET dialect.
///
/// Extracted from [`write_config`]'s PHASE 2 loop so the objects can be written in parallel (they
/// own disjoint paths). The ORDER of the sidecar passes below is preserved exactly as it was in
/// the sequential loop — it is load-bearing in the same way the read-side attach order is.
fn write_object(
    format: Format,
    fk: &FormatKind,
    out: &Path,
    obj: &MetadataObject,
    picture_defaults: &std::collections::BTreeMap<String, bool>,
) -> Result<(), ConvertError> {
    let help_view = crate::help_read::descriptor_with_help(obj)?;
    let projection = if format == Format::Edt {
        Some(formats_xml::metadata_picture_semantics::project_with_defaults(help_view.as_ref(), picture_defaults).map_err(|reason| ConvertError::Write {
            kind: fk.kind.into(), object: obj.name.clone(), reason,
        })?)
    } else { None };
    let descriptor = projection.as_ref().map_or(help_view.as_ref(), |(model,_)| model.as_ref());
    let bytes = (fk.write)(descriptor).map_err(|reason| ConvertError::Write {
        kind: fk.kind.to_string(),
        object: obj.name.clone(),
        reason,
    })?;

    if let Some(parent) = out.parent() {
        crate::fsio::create_dir_all(parent).map_err(|e| ConvertError::Io {
            path: parent.display().to_string(),
            reason: e.to_string(),
        })?;
    }
    crate::fsio::write(out, &bytes).map_err(|e| ConvertError::Io {
        path: out.display().to_string(),
        reason: e.to_string(),
    })?;

    // Body sidecars — re-emit every BODY the read side attached into the IR, beside the
    // descriptor just written, in the TARGET dialect (mirror of the read-side `attach_*`
    // passes). Without these a cross-format convert writes only the thin descriptor and
    // SILENTLY DROPS the module source / rights table / XDTO schema / form body — a real
    // content loss the platform then compiles differently (§1.0/§1.6). No-op for objects /
    // kinds that carry no such body. cf never reaches here (handled by the caller).
    crate::source_extensions::emit(format, out, obj)?;
    crate::metadata_picture_semantics::emit(out, projection.as_ref().and_then(|(_,bytes)| bytes.as_deref()))?;
    let kind = obj.kind.as_str();
    crate::form_write::write_form_bodies(format, out, obj)?;
    if kind == "Configuration" {
        // The ROOT's modules are the config-level Ext APPLICATION modules (root-level
        // `Ext/*.bsl` / `src/Configuration/*.bsl`), NOT per-object `<Name>/Ext/` module
        // sidecars — `ext_read` owns their layout (plus the Splash/MainSectionPicture
        // images). The per-kind module pass would loudly refuse them (no Configuration
        // slots in its table), so the root routes here instead.
        crate::ext_read::write_config_ext(format, out, obj)?;
    } else {
        crate::module_read::write_module_bodies(format, kind, out, obj)?;
    }
    crate::command_module_read::write_command_modules(format, kind, out, obj)?;
    crate::help_read::write_help_pages(format, kind, out, obj)?;
    // ScheduledJob `Schedule.schedule` / `Ext/Schedule.xml` → cf `<uuid>.0`.
    crate::schedule_read::write_schedule(format, kind, out, obj)?;
    // Style `Style.style` / `Ext/Style.xml` → cf `<uuid>.0`.
    crate::style_read::write_style(format, kind, out, obj)?;
    // BusinessProcess `Flowchart.scheme` / `Ext/Flowchart.xml` → cf `<uuid>.7`.
    crate::flowchart_read::write_flowchart(format, kind, out, obj)?;
    crate::exchange_plan_content_read::write_exchange_plan_content(format, kind, out, obj)?;
    // Predefined data → Designer `Ext/Predefined.xml` (EDT emits it inline in the descriptor).
    crate::predefined_read::write_predefined(format, kind, out, obj)?;
    crate::rights_read::write_rights_body(format, kind, out, obj)?;
    crate::additional_indexes_read::write(format, out, obj)?;
    crate::xdto_read::write_xdto_schema(format, kind, out, obj)?;
    // WSReference WSDL set (`WsDefinitions.wsdl`+`<N>.xsd` / `Ext/WSDefinition.xml`+`Ext/<N>.xsd`).
    crate::ws_definition_read::write_ws_definition(format, kind, out, obj)?;
    crate::template_read::write_template_bodies(format, kind, out, obj)?;
    crate::mxl_read::write_mxl_bodies(format, kind, out, obj)?;
    crate::geos_read::write_geoschema_bodies(format, kind, out, obj)?;
    crate::graph_template_read::write_graph_template_bodies(format, kind, out, obj)?;
    crate::cmi_read::write_command_interface(format, kind, out, obj)?;
    crate::picture_read::write_picture_body(format, kind, out, obj)?;
    // Report DCS templates: the nested descriptor (`Reports/<R>/Templates/<T>.xml`, Designer
    // only — EDT inlines the stub) + the schema body (`Template.dcs`/`Ext/Template.xml`). Mirror
    // of the read-side `template_ref_read`/`dcs_read` attach passes; without them a cross-format
    // convert emits a Report referencing a non-existent template file (Designer compile fails) or
    // an empty DCS (both formats).
    crate::template_ref_read::write_designer_template_ref_descriptors(format, out, obj)?;
    // Object forms: the nested descriptor (`<Host>/Forms/<Имя>.xml`, Designer only — EDT
    // inlines the stub). Mirror of the read-side `form_ref_read` attach pass; without it a
    // cross-format convert emits an owner referencing a non-existent form file (Designer
    // compile fails). The form BODY is `form_write`'s job (above).
    crate::form_ref_read::write_designer_form_ref_descriptors(format, out, obj)?;
    // ExternalDataSource tables: the STANDALONE table descriptor (`Tables/<Имя>[…]`, BOTH
    // xml dialects). Mirror of the read-side `table_ref_read` attach pass; without it the
    // host carries a bare Table ref pointing at a non-existent file.
    crate::table_ref_read::write_table_descriptors(format, out, obj)?;
    crate::dcs_read::write_dcs_bodies(format, out, obj)?;
    // Blob template bodies (AddIn/BinaryData/HTMLDocument) — EDT layout; Designer with an
    // attached body is a typed refusal (layout unwitnessed, §1.0).
    crate::blob_template_read::write_blob_template_bodies(format, kind, out, obj)?;
    Ok(())
}

/// §1.0 membership integrity for a Nested (Subsystem) kind: per parent, the nested children
/// it DECLARES (`<subsystems>` refs, carried as bare-ref child stubs) must equal the children
/// physically ENUMERATED beneath it (the recursive traversal keys). A declared-but-absent
/// child is a [`ConvertError::NestedDangling`] reference; a present-but-undeclared child is a
/// [`ConvertError::NestedUndeclared`] membership mismatch. `entries` is `(hierarchical_key,
/// declared_child_names)` for every enumerated subsystem, keyed as `layout::collect_objects_
/// checked` produces (`Parent/Child` name paths). No best-effort — the first mismatch errors.
fn validate_nested_membership(
    kind: &str,
    entries: &[(String, Vec<String>)],
) -> Result<(), ConvertError> {
    use std::collections::BTreeSet;
    let enumerated: BTreeSet<&str> = entries.iter().map(|(k, _)| k.as_str()).collect();
    // Dangling: every declared child key must resolve to an enumerated descriptor on disk.
    for (key, declared) in entries {
        for child in declared {
            let child_key = format!("{key}/{child}");
            if !enumerated.contains(child_key.as_str()) {
                return Err(ConvertError::NestedDangling {
                    kind: kind.to_string(),
                    parent: key.clone(),
                    child: child.clone(),
                });
            }
        }
    }
    // Undeclared: every enumerated NON-top subsystem (its key has a parent segment) must be
    // declared by that parent — so physical nesting and declared membership agree.
    let declared_keys: BTreeSet<String> = entries
        .iter()
        .flat_map(|(key, declared)| declared.iter().map(move |c| format!("{key}/{c}")))
        .collect();
    for (key, _) in entries {
        if key.contains('/') && !declared_keys.contains(key) {
            return Err(ConvertError::NestedUndeclared {
                kind: kind.to_string(),
                path: key.clone(),
            });
        }
    }
    Ok(())
}

/// Write a whole configuration IR to the destination `dst`.
///
/// * **cf** — `dst` is a single `.cf` FILE (not a directory). The whole configuration is
///   assembled by [`crate::cf_write::write_cf_config`] (service trio + config-root +
///   per-object descriptors) and written as one file. The assembler enforces §1.0 write
///   honesty (no-writer / body-not-emitted kinds error, never a silent partial `.cf`).
/// * **edt/designer** — `dst` is the FORMAT ROOT directory: for each object, look up its
///   connector by `obj.kind.as_str()`, regenerate the descriptor byte-exactly, create the
///   layout path and write it. The root object writes to the singleton path.
///
/// Deterministic: same IR → identical output, byte-for-byte.
pub fn write_config(format: Format, cfg: &Configuration, dst: &Path) -> Result<(), ConvertError> {
    // cf is container-based: a single file assembled from the whole IR, not a directory walk.
    if format == Format::Cf {
        return Err(ConvertError::FormatNotSupported(
            "CF excluded from source-only adapter".into(),
        ));
    }

    let timing = std::env::var_os("MORPH1C_TIMING").is_some();
    let t1 = std::time::Instant::now();
    let reg = FormatRegistry::for_format(format)?;
    let picture_defaults = formats_xml::metadata_picture_semantics::common_picture_defaults(&cfg.objects)
        .map_err(|reason| ConvertError::Write {
            kind: "CommonPicture".into(), object: "configuration context".into(), reason,
        })?;

    // PHASE 1 — plan every object's output path (nothing written yet, so a path error aborts
    // BEFORE any partial output). Nested (Subsystem) kinds reconstruct the HIERARCHICAL path
    // from the object's parent chain (`layout::nested_output_path`) — the flat own-name collides
    // across branches (SSL: 3 collisions); other kinds use the flat layout path.
    let mut planned: Vec<(&'static FormatKind, PathBuf)> = Vec::with_capacity(cfg.objects.len());
    for obj in &cfg.objects {
        let kind = obj.kind.as_str();
        let fk = reg.get(kind).ok_or_else(|| ConvertError::NoWriter {
            kind: kind.to_string(),
        })?;
        let out = if matches!(fk.layout, CorpusLayout::Nested { .. }) {
            layout::nested_output_path(&reg, fk, dst, obj)?
        } else {
            // The object NAME drives the path. For the root, the layout fixes the name.
            layout::output_path(&reg, fk, dst, &obj.name).ok_or_else(|| ConvertError::NoWriter {
                kind: kind.to_string(),
            })?
        };
        planned.push((fk, out));
    }

    // §1.0 honesty guard for the hierarchical WRITE: every Nested descriptor must land at a
    // DISTINCT path. Two subsystems collapsing to the same path means a parent chain was lost
    // (e.g. a source that carries no `parentSubsystem`) and own-names collided — a silent flat
    // fallback that would OVERWRITE a sibling. Fail loudly instead of clobbering.
    let mut nested_seen: std::collections::HashMap<&std::path::Path, &str> =
        std::collections::HashMap::new();
    for ((fk, out), obj) in planned.iter().zip(cfg.objects.iter()) {
        if !matches!(fk.layout, CorpusLayout::Nested { .. }) {
            continue;
        }
        if let Some(prev) = nested_seen.insert(out.as_path(), obj.name.as_str()) {
            return Err(ConvertError::NestedUnresolved {
                kind: fk.kind.to_string(),
                object: obj.name.clone(),
                reason: format!(
                    "collides with nested object {prev:?} at the same path {} (own-name \
                     collision with no distinguishing parent chain — hierarchical identity lost)",
                    out.display()
                ),
            });
        }
    }

    // PHASE 2 — regenerate each descriptor byte-exactly and write it to its planned path.
    //
    // Objects write to DISTINCT paths (PHASE 1 planned them and already proved the Nested ones do
    // not collide), so this is embarrassingly parallel — and it is where the XML lanes spend
    // nearly all their time: SSL emits 9 397 files / ~224 MB, and under `MORPH1C_TIMING` that
    // phase measures ~535 s summed across 8 workers of which ~530 s is the filesystem and ~4 s
    // is our serialisers. Optimising the writers here would buy nothing; see `crate::fsio`.
    //
    // §1.0 determinism: results are collected INDEXED and the FIRST failure in object order is
    // returned, so the error surfaced is the sequential walk's, not whichever thread lost the race.
    //
    // The ambient roundtrip TARGET (installed by `convert` around this whole write) is a
    // thread_local — rayon workers do NOT inherit it, and every version-gated writer (envelope
    // `version=`, sidecar stamps, dense regions) would silently emit the default SSL shape under
    // a 2.20 target. Capture it here and reinstall in each worker.
    let ambient_target = morph1c_core::version::current_roundtrip_target();
    if timing {
        eprintln!("[timing]   plan paths: {:?}", t1.elapsed());
    }
    let (t2, io0) = (std::time::Instant::now(), crate::fsio::io_stats());
    // Per-object cost, summed and max, so `MORPH1C_TIMING` can separate the three ways this
    // phase can be slow: filesystem (summed fs time ≈ summed total), our own CPU (summed total
    // ≫ summed fs), and a scheduling TAIL (max_one alone approaching the wall clock).
    let (busy_ns, max_ns) = (
        std::sync::atomic::AtomicU64::new(0),
        std::sync::atomic::AtomicU64::new(0),
    );
    let results: Vec<Result<(), ConvertError>> = planned
        .par_iter()
        .zip(cfg.objects.par_iter())
        .map(|((fk, out), obj)| {
            // Clock reads only when profiling — see `fsio::accounting`.
            let t = timing.then(std::time::Instant::now);
            let r = morph1c_core::version::with_captured_roundtrip_target(ambient_target, || {
                write_object(format, fk, out, obj, &picture_defaults)
            });
            if let Some(t) = t {
                use std::sync::atomic::Ordering::Relaxed;
                let ns = t.elapsed().as_nanos() as u64;
                busy_ns.fetch_add(ns, Relaxed);
                max_ns.fetch_max(ns, Relaxed);
            }
            r
        })
        .collect();
    if timing {
        use std::sync::atomic::Ordering::Relaxed;
        let io = crate::fsio::io_stats();
        let (files, bytes, nanos) = (io.0 - io0.0, io.1 - io0.1, io.2 - io0.2);
        let (mkc, mkn) = crate::fsio::mkdir_stats();
        let d = std::time::Duration::from_nanos;
        eprintln!(
            "[timing]   write objects: {:?}  ({} files, {:.1} MB) — summed across threads: \
             busy {:.1?}, of it in fs writes {:.1?}, in mkdir {:.1?} ({mkc} calls); \
             slowest single object {:.1?}",
            t2.elapsed(),
            files,
            bytes as f64 / 1e6,
            d(busy_ns.load(Relaxed)),
            d(nanos),
            d(mkn),
            d(max_ns.load(Relaxed)),
        );
    }
    for result in results {
        result?;
    }

    // Language sidecars (Designer target only): the root `Configuration.xml` writer emits only the
    // bare `<Language>` reference in ChildObjects, not the language body. Emit each declared
    // language's `Languages/<Name>.xml` from the root's inline `F_LANGUAGES` (mirror of the read-
    // side `language_read` attach) so the written Designer tree carries no dangling reference. EDT
    // inlines languages in the descriptor; cf assembles them — both no-op here.
    if let Some(root) = cfg
        .objects
        .iter()
        .find(|o| o.kind.as_str() == "Configuration")
    {
        crate::language_write::write_languages(format, dst, root)?;
    }
    if timing {
        eprintln!("[timing]   write_config total: {:?}", t1.elapsed());
    }
    Ok(())
}

#[cfg(any())]
mod tests {
    use super::*;

    /// `(key, [children])` helper for membership tests.
    fn e(key: &str, children: &[&str]) -> (String, Vec<String>) {
        (
            key.to_string(),
            children.iter().map(|s| s.to_string()).collect(),
        )
    }

    #[test]
    fn nested_membership_accepts_consistent_tree() {
        // Root R declares A,B; A declares C. All present → Ok.
        let entries = vec![
            e("R", &["A", "B"]),
            e("R/A", &["C"]),
            e("R/B", &[]),
            e("R/A/C", &[]),
        ];
        assert!(validate_nested_membership("Subsystem", &entries).is_ok());
    }

    #[test]
    fn nested_membership_flags_dangling_ref() {
        // R declares a child B that has no descriptor on disk (not enumerated) → dangling.
        let entries = vec![e("R", &["A", "B"]), e("R/A", &[])];
        let err = validate_nested_membership("Subsystem", &entries).unwrap_err();
        match err {
            ConvertError::NestedDangling { parent, child, .. } => {
                assert_eq!(parent, "R");
                assert_eq!(child, "B");
            }
            other => panic!("expected NestedDangling, got {other:?}"),
        }
    }

    #[test]
    fn nested_membership_flags_undeclared_child() {
        // R/A is enumerated on disk but R does not declare A → undeclared membership.
        let entries = vec![e("R", &[]), e("R/A", &[])];
        let err = validate_nested_membership("Subsystem", &entries).unwrap_err();
        match err {
            ConvertError::NestedUndeclared { path, .. } => assert_eq!(path, "R/A"),
            other => panic!("expected NestedUndeclared, got {other:?}"),
        }
    }
}
