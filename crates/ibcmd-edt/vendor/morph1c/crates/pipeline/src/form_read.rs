//! Attach FORM-BODY SIDECARS (`Form.form` EDT / `Ext/Form.xml` Designer) plus their optional
//! form MODULE (`Module.bsl`) into the object's IR `form_bodies` during whole-config read
//! (§1.0/§1.6). Mirror of [`crate::module_read`] (CommonModule `Module.bsl`) — "the descriptor
//! read is metadata-only, the form BODY is a sibling file the pipeline attaches".
//!
//! # Why: edt↔designer needs the whole form body wired through the pipeline
//! The per-kind descriptor connectors read only the thin `.mdo`/`.xml` metadata (synonym/
//! comment/usePurposes/FormRef stubs); the FORM ITSELF (control tree + attributes/events/
//! commands/…) lives in a separate per-form file read/written byte-exact by
//! `formats_xml::form::{read_form, write_form}`. This pass reads that body (and the optional
//! form module) into `obj.form_bodies` so a whole-config `convert` can re-emit the target
//! format's form body (`pipeline::form_write`).
//!
//! # Two ownership modes (RE: coverage/s10_forms 10/10 + coverage/s15_subordinate 18/18)
//! * **OWN body** (`CommonForm`) — the form IS the object; the body sidecar is named for the
//!   descriptor itself:
//!   - EDT (`CommonForms/<Name>/<Name>.mdo`, dir-per-object): body SIBLING
//!     `CommonForms/<Name>/Form.form`; form module SIBLING `CommonForms/<Name>/Module.bsl`.
//!   - Designer (`CommonForms/<Name>.xml`, file-per-object): body
//!     `CommonForms/<Name>/Ext/Form.xml`; form module `CommonForms/<Name>/Ext/Form/Module.bsl`.
//! * **OBJECT-SUBORDINATE forms** ([`SUBORDINATE_FORM_OWNERS`]: Catalog/Document/…/Report) —
//!   each declared form is a `.FormRef` CHILD of the owner (name + order come from
//!   `obj.children`, NEVER a filesystem scan), and its sidecars live under the owner's
//!   `Forms/<FormName>/` subtree — which is EXACTLY the CommonForm layout re-rooted at a
//!   per-form VIRTUAL descriptor ([`form_anchor_path`]):
//!   - EDT: anchor `<obj-dir>/Forms/<FormName>/<FormName>.mdo` (virtual — EDT subordinate forms
//!     carry no `.mdo`) → body `<obj-dir>/Forms/<FormName>/Form.form`, module
//!     `<obj-dir>/Forms/<FormName>/Module.bsl`.
//!   - Designer: anchor `<dir>/<Owner>/Forms/<FormName>.xml` (the REAL nested form descriptor,
//!     read elsewhere) → body `<dir>/<Owner>/Forms/<FormName>/Ext/Form.xml`, module
//!     `<dir>/<Owner>/Forms/<FormName>/Ext/Form/Module.bsl`.
//!
//! # §1.0 — the form body is OPTIONAL (attach if present)
//! A form descriptor MAY carry a form body sidecar, but need not: real forms (s10_forms 10/10,
//! s15_subordinate 18/18) carry a `Form.form`/`Ext/Form.xml`; lightweight **property-stub**
//! CommonForms that only exercise descriptor properties (`FormType`/`IncludeHelpInContents`/
//! `UseStandardCommands` — e.g. `s4_common`'s 7 `ОбщФорма_*`) have NO body file and STILL
//! compile. So we attach the body IFF the sidecar exists on disk (honest — reflects the actual
//! on-disk state), never a hard error on absence (an earlier STRICT "must carry a body" broke
//! whole-config reads of s4_common). Downstream honesty is enforced at ASSEMBLE: a form that DOES
//! carry a body errors `BodyNotEmitted` until the cf form-body encoder lands; a bodyless
//! CommonForm assembles descriptor-only (valid — matches the platform). The form MODULE is
//! likewise OPTIONAL — absent ⇒ no module in IR.

use std::path::{Path, PathBuf};

use formats_xml::form::{
    FormDialect, read_form, read_list_settings_dcss, read_spreadsheet_mxlx, set_sidecar_ext,
    sidecar_slots,
};
use formats_xml::registry::Format;
use morph1c_core::ir::form::{DecoratorBody, FormItem, FormPicture};
use morph1c_core::ir::{FormBody, MetadataObject, NamedFormBody};

use crate::ConvertError;
#[path = "source_read.rs"]
mod source_read;
pub(crate) use source_read::read_regular_source;

/// The kind whose object carries its OWN whole form body (the form IS the object).
const OWN_FORM_KIND: &str = "CommonForm";

/// Kinds owning OBJECT-SUBORDINATE forms (`Forms/<FormName>/` per declared `.FormRef` child) —
/// the 18 form-owning metadata kinds (RE: coverage/s15_subordinate, one form per kind + the
/// multi-form unit witnesses). Form names and their ORDER come from the owner's `.FormRef`
/// children, never a filesystem scan.
const SUBORDINATE_FORM_OWNERS: &[&str] = &[
    "AccountingRegister",
    "AccumulationRegister",
    "BusinessProcess",
    "CalculationRegister",
    "Catalog",
    "ChartOfAccounts",
    "ChartOfCalculationTypes",
    "ChartOfCharacteristicTypes",
    "DataProcessor",
    "Document",
    "DocumentJournal",
    "Enum",
    "ExchangePlan",
    "FilterCriterion",
    "InformationRegister",
    "Report",
    "SettingsStorage",
    "Task",
];

/// Is `child` a declared form of its owner (`kind` ends `.FormRef` — carries name + order)?
fn is_form_ref_child(child: &MetadataObject) -> bool {
    child.kind.as_str().ends_with(".FormRef")
}

pub(crate) fn declared_form_uuid(
    obj: &MetadataObject,
    name: &str,
) -> Result<morph1c_core::ir::Uuid, ConvertError> {
    if obj.kind.as_str() == OWN_FORM_KIND && obj.name == name {
        return Ok(obj.uuid);
    }
    let mut matches = obj
        .children
        .iter()
        .filter(|child| is_form_ref_child(child) && child.name == name);
    let child = matches.next().ok_or_else(|| ConvertError::Read {
        kind: obj.kind.as_str().into(),
        object: name.into(),
        reason: "picture resource requires an exact declared form UUID".into(),
    })?;
    if matches.next().is_some() {
        return Err(ConvertError::Read {
            kind: obj.kind.as_str().into(),
            object: name.into(),
            reason: "duplicate declared form identity".into(),
        });
    }
    Ok(child.uuid)
}

/// Load every form body (+ optional form module) `obj` declares from its sidecars into
/// `obj.form_bodies`, in declaration order.
///
/// `descriptor_path` — path of the descriptor just read (`.mdo`/`.xml`). Dual-mode: a
/// `CommonForm` resolves ONE body named for itself beside the descriptor; a subordinate-form
/// OWNER resolves one body per `.FormRef` child under `Forms/<FormName>/` (see module docs).
/// Every body is OPTIONAL: present ⇒ attach it; ABSENT ⇒ skip (a bodyless property-stub form is
/// valid, e.g. s4_common — reflecting the on-disk state, not a hard error). The form MODULE is
/// likewise optional. Non-form kinds and cf (container, no sidecar) are no-ops.
pub fn attach_form_body(
    format: Format,
    kind: &str,
    descriptor_path: &Path,
    obj: &mut MetadataObject,
) -> Result<(), ConvertError> {
    if format == Format::Cf {
        return Ok(()); // cf: container — no file-per-object sidecar to attach.
    }
    // Form names to resolve, in declaration order (own name / `.FormRef` children).
    let names: Vec<String> = if kind == OWN_FORM_KIND {
        vec![obj.name.clone()]
    } else if SUBORDINATE_FORM_OWNERS.contains(&kind) {
        obj.children
            .iter()
            .filter(|c| is_form_ref_child(c))
            .map(|c| c.name.clone())
            .collect()
    } else {
        return Ok(());
    };
    if names.is_empty() {
        return Ok(());
    }
    // §1.0: never silently overwrite an already-attached body (the descriptor read must not
    // populate `form_bodies` — only this pass does).
    if !obj.form_bodies.is_empty() {
        return Err(ConvertError::Read {
            kind: kind.to_string(),
            object: obj.name.clone(),
            reason: "object already carries a form body before the sidecar attach (unexpected — \
                     the descriptor projection must not populate it)"
                .into(),
        });
    }

    for name in names {
        let anchor = match form_anchor_path(format, kind, descriptor_path, &name) {
            Some(a) => a,
            None => return Ok(()), // defensive: no parent/stem to anchor at.
        };
        let (body_path, dialect) = match form_body_path(format, &anchor) {
            Some(x) => x,
            None => return Ok(()), // unreachable (cf handled above) — kept defensive.
        };
        let ordinary_path =
            ordinary_form_body_path(format, &anchor).ok_or_else(|| ConvertError::Read {
                kind: kind.to_string(),
                object: name.clone(),
                reason: "ordinary form layout is unavailable".into(),
            })?;
        let ordinary = declared_form_is_ordinary(obj, &name)?;
        if ordinary_path.exists() {
            if !ordinary
                || body_path.exists()
                || form_module_path(format, &anchor).is_some_and(|p| p.exists())
            {
                return Err(ConvertError::Read { kind: kind.to_string(), object: name.clone(), reason: "ordinary body conflicts with declared form type, managed body, or external Module.bsl".into() });
            }
            let bytes = read_ordinary_body(&ordinary_path)?;
            let (help, help_resources) = if kind == OWN_FORM_KIND {
                (Vec::new(), Vec::new())
            } else {
                crate::help_read::read_help_sidecar(format, kind, &anchor, &name)?
            };
            if format == Format::Designer && !help.is_empty() {
                if let Some(child) = obj
                    .children
                    .iter_mut()
                    .find(|c| is_form_ref_child(c) && c.name == name)
                {
                    crate::help_read::sync_form_ref_help_property(child)?;
                }
            }
            obj.form_bodies.push(NamedFormBody {
                name,
                body: FormBody::new(),
                ordinary_body: Some(bytes),
                module: None,
                help,
                help_resources,
            });
            continue;
        }
        if ordinary && body_path.exists() {
            return Err(ConvertError::Read {
                kind: kind.to_string(),
                object: name.clone(),
                reason: "ordinary declaration carries a managed form body".into(),
            });
        }
        // The form body is OPTIONAL: absent ⇒ skip (a bodyless property-stub form — e.g.
        // s4_common's `ОбщФорма_*` — is a valid config that compiles; reflect the on-disk state
        // rather than erroring). Downstream honesty is at assemble (a form WITH a body errors
        // BodyNotEmitted until the cf form-body encoder lands).
        if !body_path.is_file() {
            continue;
        }
        let bytes = std::fs::read(&body_path).map_err(|e| ConvertError::Io {
            path: body_path.display().to_string(),
            reason: e.to_string(),
        })?;
        let mut body = read_form(dialect, &bytes).map_err(|e| ConvertError::Read {
            kind: kind.to_string(),
            object: format!("{}.{name}", obj.name),
            reason: e.to_string(),
        })?;
        if format == Format::Edt {
            let resource = body_path
                .parent()
                .expect("form body has a parent")
                .join(formats_xml::form::PICTURE_SEMANTICS_RESOURCE);
            if resource.exists() {
                let bytes = std::fs::read(&resource).map_err(|error| ConvertError::Io {
                    path: resource.display().to_string(),
                    reason: error.to_string(),
                })?;
                let model = formats_xml::form::read_picture_semantics_resource(&bytes).map_err(
                    |error| ConvertError::Read {
                        kind: kind.into(),
                        object: name.clone(),
                        reason: error.to_string(),
                    },
                )?;
                if model.form_uuid != declared_form_uuid(obj, &name)? {
                    return Err(ConvertError::Read {
                        kind: kind.into(),
                        object: name.clone(),
                        reason: "picture resource form UUID differs from declared metadata".into(),
                    });
                }
                body.picture_resource_selection = Some(
                    model
                        .records
                        .iter()
                        .map(|row| row.binding.clone())
                        .collect(),
                );
                body.picture_semantics = Some(model);
            }
        }

        // EDT: the spreadsheet-document BODY of a form attribute is a SIDECAR
        // (`Attributes/<attr>/ExtInfo/SpreadsheetData.mxlx` beside `Form.form`), while `Form.form`
        // itself carries only the empty `form:SpreadsheetDocumentExtInfo` marker. Attach it into
        // the attribute's `spreadsheet_settings` — the SAME sub-IR the Designer codec fills from
        // its inline `<Settings mxl:SpreadsheetDocument>` block — so a cross-format convert
        // re-emits the other format's serialization instead of silently dropping the body.
        //
        // EDT: the DCS user-SETTINGS of a dynamic-list attribute are likewise a SIDECAR
        // (`Attributes/<attr>/ExtInfo/ListSettings.dcss`), and `Form.form` carries NO trace of
        // them at all — while Designer inlines them as `<ListSettings>` inside the attribute's
        // `<Settings xsi:type="DynamicList">`. Attach them into the SAME sub-IR the Designer
        // codec fills (`DynamicListAttrExt::list_settings`), so the settings are neither dropped
        // on convert nor invisible to the cf form-body encoder (they FEED the encoded bag).
        //
        // EDT: the FORM-LEVEL conditional appearance is a SIDECAR too
        // (`ConditionalAppearance.dcssca` beside `Form.form`), and `Form.form` carries NO trace of
        // it — while Designer inlines it as `<ConditionalAppearance>` inside `<Attributes>`. Waves
        // 1-22 never read it, so it was silently DROPPED on edt→designer, and the cf encoder could
        // not see the one region it drives (F-wave 23: the counted `(str,str,path,path)`
        // field-reference sub-region of root[3] — see
        // `formats_cf::form_body::ensure_no_derived_path_region`, whose wave-7 "hyperlink over an
        // indexed dataPath" reading the ablation falsified).
        if matches!(dialect, FormDialect::Edt) {
            if let Some(form_dir) = body_path.parent() {
                attach_edt_spreadsheet_sidecars(form_dir, kind, &obj.name, &name, &mut body)?;
                attach_edt_list_settings_sidecars(form_dir, kind, &obj.name, &name, &mut body)?;
                attach_edt_conditional_appearance(form_dir, kind, &obj.name, &name, &mut body)?;
            }
        }

        // The INLINE picture of a control (a `Picture`/`RowsPicture`/`ValuesPicture`/
        // `HeaderPicture` that is NOT a metadata ref) is a binary SIDECAR in BOTH dialects —
        // `Items/<ctl>/<Tag>.<ext>` — and the descriptor carries only a MARKER for it. Neither
        // XML reader can see the file (it gets bytes, not a directory), so the bytes would be
        // silently DROPPED without this pass — and the cf encoder, which INLINES them into the
        // form body, could never reproduce the cell. Attach them here (same division of labour as
        // `SpreadsheetData.mxlx`), and normalise the EDT canon `Ref("")` up to the Designer canon
        // `Ref("abs:<ext>")` now that the extension is known from the file — so BOTH dialects
        // yield the SAME IR (§1.6).
        if let Some(items_dir) = form_items_dir(format, &anchor) {
            attach_form_picture_sidecars(&items_dir, kind, &obj.name, &name, &mut body)?;
        }

        // The form MODULE is optional (present ⇒ read its source; absent ⇒ no module in IR).
        let module = match form_module_path(format, &anchor) {
            Some(p) if p.is_file() => {
                let src = crate::module_read::read_module_text(&p)?;
                Some(src)
            }
            _ => None,
        };

        // The form HELP is a sidecar too — EDT `Forms/<F>/Help/<lang>.html`, Designer
        // `Forms/<F>/Ext/Help.xml` + `Ext/Help/<lang>.html` — the SAME layout/encodings as the
        // OBJECT help, only anchored at the form (`crate::help_read::read_help_sidecar` is reused
        // verbatim; the EDT anchor is virtual and serves only as "parent = the form's dir").
        // In cf it is a SEPARATE element `<form-uuid>.1` (RE ssl.cf). It was never in the IR at
        // all, so 354 help bodies were silently missing from every assembled container.
        //
        // A CommonForm is EXEMPT: the object IS the form, its help sidecar sits at the OBJECT
        // anchor and `help_read::attach_help_pages` has already read it into `obj.help` — reading
        // it here too would DOUBLE it (and `attach_help_pages` runs first, see `convert.rs`).
        let (help, help_resources) = if kind == OWN_FORM_KIND {
            (Vec::new(), Vec::new())
        } else {
            crate::help_read::read_help_sidecar(format, kind, &anchor, &name)?
        };
        // Designer: the thin `Forms/<F>.xml` descriptor carries NO `<Help>` marker while the EDT
        // inline stub does (`<help><pages><lang>ru</lang></pages></help>`) — synthesize the
        // presence-Bool on the `.FormRef` child so BOTH dialects yield the SAME IR (§1.6) and
        // designer→edt does not drop the block (mirror of the object-level sync).
        if format == Format::Designer && !help.is_empty() {
            if let Some(child) = obj
                .children
                .iter_mut()
                .find(|c| is_form_ref_child(c) && c.name == name)
            {
                crate::help_read::sync_form_ref_help_property(child)?;
            }
        }

        obj.form_bodies.push(NamedFormBody {
            ordinary_body: None,
            name,
            body,
            module,
            help,
            help_resources,
        });
    }
    Ok(())
}

/// EDT sidecar path of the FORM-LEVEL conditional appearance, relative to the form dir:
/// `ConditionalAppearance.dcssca` (the ONE SSL carrier —
/// `DataProcessors/РаботаСФайлами/Forms/ВерсияПрисоединенногоФайла`). Shared by the read attach and
/// [`crate::form_write`].
pub(crate) fn conditional_appearance_sidecar_path(form_dir: &Path) -> PathBuf {
    form_dir.join("ConditionalAppearance.dcssca")
}

/// Attach the `ConditionalAppearance.dcssca` sidecar into `FormBody::conditional_appearance` — the
/// SAME sub-IR the Designer codec fills from its inline `<ConditionalAppearance>` (see the call
/// site). Absent ⇒ no appearance (the platform writes no file for an empty one).
fn attach_edt_conditional_appearance(
    form_dir: &Path,
    kind: &str,
    owner: &str,
    form_name: &str,
    body: &mut FormBody,
) -> Result<(), ConvertError> {
    let path = conditional_appearance_sidecar_path(form_dir);
    if !path.is_file() {
        return Ok(());
    }
    let bytes = std::fs::read(&path).map_err(|e| ConvertError::Io {
        path: path.display().to_string(),
        reason: e.to_string(),
    })?;
    let (ca_items, ca_without_lf_pal) =
        formats_xml::form::read_conditional_appearance_dcssca(&bytes).map_err(|e| {
            ConvertError::Read {
                kind: kind.to_string(),
                object: format!("{owner}.{form_name}"),
                reason: format!("ConditionalAppearance.dcssca: {e}"),
            }
        })?;
    body.conditional_appearance = ca_items;
    body.ca_envelope_without_lf_pal = ca_without_lf_pal;
    Ok(())
}

/// EDT sidecar path of a form-attribute spreadsheet-document body, relative to the form dir
/// (the dir holding `Form.form`): `Attributes/<attr>/ExtInfo/SpreadsheetData.mxlx`
/// (RE: coverage/s10_forms Форма_Группы + SSL 10/10 sidecars — CommonForm and subordinate
/// forms share the layout). Shared by the read attach and [`crate::form_write`] (write side).
pub(crate) fn spreadsheet_sidecar_path(form_dir: &Path, attr_name: &str) -> PathBuf {
    form_dir
        .join("Attributes")
        .join(attr_name)
        .join("ExtInfo")
        .join("SpreadsheetData.mxlx")
}

/// Attach every `SpreadsheetData.mxlx` attribute sidecar under `form_dir` into the matching
/// top-level data attribute's `spreadsheet_settings` (the format-neutral sub-IR the Designer
/// codec fills from its inline `<Settings mxl:SpreadsheetDocument>`).
///
/// §1.0 (loud, never a silent drop):
/// * a sidecar paired with an attribute that does NOT carry the
///   `form:SpreadsheetDocumentExtInfo` marker is an unknown variant — hard error;
/// * an ORPHAN sidecar on disk (its `Attributes/<dir>` name matches no top-level attribute)
///   is a hard error;
/// * the sidecar itself is OPTIONAL — a marker attribute without one is valid (26/36 SSL
///   markers carry no body; ⟺ Designer emits no inline `<Settings>` block for them).
fn attach_edt_spreadsheet_sidecars(
    form_dir: &Path,
    kind: &str,
    owner: &str,
    form_name: &str,
    body: &mut FormBody,
) -> Result<(), ConvertError> {
    let object = format!("{owner}.{form_name}");
    for attr in &mut body.data_attributes {
        // Chart.chart / GanttChart.chart — ТРЕТИЙ формат тела Settings (диаграмма; ERP W17:
        // 9 блоков в 5 формах). EDT держит тело в сайдкаре `Attributes/<attr>/ExtInfo/*.chart`;
        // Designer инлайнит его в `<Settings d4p1:Chart>`. Читаем в `attr.chart_settings`
        // (вид определяет корень сайдкара). §1.0: несоответствие структуры — громкий отказ
        // ридера-кодека, никогда не silent-drop.
        let ext_dir = form_dir.join("Attributes").join(&attr.name).join("ExtInfo");
        let chart_sidecar = ["Chart.chart", "GanttChart.chart"]
            .iter()
            .map(|f| ext_dir.join(f))
            .find(|p| p.is_file());
        if let Some(path) = chart_sidecar {
            if attr.chart_settings.is_some() {
                return Err(ConvertError::Read {
                    kind: kind.to_string(),
                    object,
                    reason: format!(
                        "attribute {:?} already carries chart settings before the sidecar \
                         attach (unexpected — the EDT form codec must not populate them)",
                        attr.name
                    ),
                });
            }
            let bytes = std::fs::read(&path).map_err(|e| ConvertError::Io {
                path: path.display().to_string(),
                reason: e.to_string(),
            })?;
            attr.chart_settings =
                Some(formats_xml::form::read_chart_sidecar(&bytes).map_err(|e| {
                    ConvertError::Read {
                        kind: kind.to_string(),
                        object: object.clone(),
                        reason: format!("chart sidecar {}: {e}", path.display()),
                    }
                })?);
        }
        let path = spreadsheet_sidecar_path(form_dir, &attr.name);
        if !path.is_file() {
            continue;
        }
        if !attr.spreadsheet_ext {
            return Err(ConvertError::Read {
                kind: kind.to_string(),
                object,
                reason: format!(
                    "attribute {:?} has a SpreadsheetData.mxlx sidecar but no \
                     form:SpreadsheetDocumentExtInfo marker — unknown variant (§1.0)",
                    attr.name
                ),
            });
        }
        if attr.spreadsheet_settings.is_some() {
            return Err(ConvertError::Read {
                kind: kind.to_string(),
                object,
                reason: format!(
                    "attribute {:?} already carries spreadsheet settings before the sidecar \
                     attach (unexpected — the EDT form codec must not populate it)",
                    attr.name
                ),
            });
        }
        let bytes = std::fs::read(&path).map_err(|e| ConvertError::Io {
            path: path.display().to_string(),
            reason: e.to_string(),
        })?;
        attr.spreadsheet_settings =
            Some(
                read_spreadsheet_mxlx(&bytes).map_err(|e| ConvertError::Read {
                    kind: kind.to_string(),
                    object: object.clone(),
                    reason: format!("{}: {e}", path.display()),
                })?,
            );
    }
    // Orphan detection: a sidecar on disk whose attribute is unknown would otherwise be silently
    // dropped on convert (§1.0). Nested COLUMNS never carry sidecars (0 in corpus) — only
    // top-level attribute names are legal dirs.
    let attrs_dir = form_dir.join("Attributes");
    if attrs_dir.is_dir() {
        let entries = std::fs::read_dir(&attrs_dir).map_err(|e| ConvertError::Io {
            path: attrs_dir.display().to_string(),
            reason: e.to_string(),
        })?;
        for entry in entries {
            let entry = entry.map_err(|e| ConvertError::Io {
                path: attrs_dir.display().to_string(),
                reason: e.to_string(),
            })?;
            if !entry
                .path()
                .join("ExtInfo")
                .join("SpreadsheetData.mxlx")
                .is_file()
            {
                continue;
            }
            let dir_name = entry.file_name().to_string_lossy().into_owned();
            if !body.data_attributes.iter().any(|a| a.name == dir_name) {
                return Err(ConvertError::Read {
                    kind: kind.to_string(),
                    object,
                    reason: format!(
                        "orphan SpreadsheetData.mxlx sidecar for unknown attribute {dir_name:?} \
                         (silent drop forbidden — §1.0)"
                    ),
                });
            }
        }
    }
    Ok(())
}

/// EDT sidecar path of a dynamic-list attribute's DCS user-settings, relative to the form dir:
/// `Attributes/<attr>/ExtInfo/ListSettings.dcss` (RE: SSL 227 sidecars / 229 dynamic lists — the
/// same `Attributes/<attr>/ExtInfo/` locus as `SpreadsheetData.mxlx`). Shared by the read attach
/// and [`crate::form_write`].
pub(crate) fn list_settings_sidecar_path(form_dir: &Path, attr_name: &str) -> PathBuf {
    form_dir
        .join("Attributes")
        .join(attr_name)
        .join("ExtInfo")
        .join("ListSettings.dcss")
}

/// Attach every `ListSettings.dcss` attribute sidecar under `form_dir` into the matching top-level
/// DYNAMIC-LIST attribute's `list_settings` (the format-neutral sub-IR the Designer codec fills
/// from its inline `<ListSettings>` block).
///
/// §1.0 (loud, never a silent drop):
/// * a sidecar paired with an attribute that is NOT a dynamic list is an unknown variant — hard
///   error;
/// * an ORPHAN sidecar on disk (its `Attributes/<dir>` name matches no top-level attribute) is a
///   hard error;
/// * the sidecar itself is OPTIONAL — a dynamic list without one has EMPTY settings (⟺ Designer
///   `<ListSettings/>`; 2/229 in SSL), which stays `None` in the IR.
fn attach_edt_list_settings_sidecars(
    form_dir: &Path,
    kind: &str,
    owner: &str,
    form_name: &str,
    body: &mut FormBody,
) -> Result<(), ConvertError> {
    let object = format!("{owner}.{form_name}");
    for attr in &mut body.data_attributes {
        let path = list_settings_sidecar_path(form_dir, &attr.name);
        if !path.is_file() {
            continue;
        }
        let Some(dl) = attr.dynamic_list.as_mut() else {
            return Err(ConvertError::Read {
                kind: kind.to_string(),
                object,
                reason: format!(
                    "attribute {:?} has a ListSettings.dcss sidecar but is not a dynamic list \
                     (no form:DynamicListExtInfo) — unknown variant (§1.0)",
                    attr.name
                ),
            });
        };
        if dl.list_settings.is_some() {
            return Err(ConvertError::Read {
                kind: kind.to_string(),
                object,
                reason: format!(
                    "attribute {:?} already carries list settings before the sidecar attach \
                     (unexpected — the EDT form codec must not populate it)",
                    attr.name
                ),
            });
        }
        let bytes = std::fs::read(&path).map_err(|e| ConvertError::Io {
            path: path.display().to_string(),
            reason: e.to_string(),
        })?;
        dl.list_settings =
            Some(
                read_list_settings_dcss(&bytes).map_err(|e| ConvertError::Read {
                    kind: kind.to_string(),
                    object: object.clone(),
                    reason: format!("{}: {e}", path.display()),
                })?,
            );
    }
    // Orphan detection (§1.0): a sidecar whose attribute is unknown would be silently dropped.
    let attrs_dir = form_dir.join("Attributes");
    if attrs_dir.is_dir() {
        let entries = std::fs::read_dir(&attrs_dir).map_err(|e| ConvertError::Io {
            path: attrs_dir.display().to_string(),
            reason: e.to_string(),
        })?;
        for entry in entries {
            let entry = entry.map_err(|e| ConvertError::Io {
                path: attrs_dir.display().to_string(),
                reason: e.to_string(),
            })?;
            if !entry
                .path()
                .join("ExtInfo")
                .join("ListSettings.dcss")
                .is_file()
            {
                continue;
            }
            let dir_name = entry.file_name().to_string_lossy().into_owned();
            if !body.data_attributes.iter().any(|a| a.name == dir_name) {
                return Err(ConvertError::Read {
                    kind: kind.to_string(),
                    object,
                    reason: format!(
                        "orphan ListSettings.dcss sidecar for unknown attribute {dir_name:?} \
                         (silent drop forbidden — §1.0)"
                    ),
                });
            }
        }
    }
    Ok(())
}

// --- form-item INLINE pictures (`Items/<ctl>/<Tag>.<ext>`) ---------------------------------------

/// Directory holding a form's per-control picture sidecars, derived from the form's descriptor
/// ANCHOR. It is the `Items/` dir BESIDE the form's `Module.bsl` — i.e. the form BODY dir — in
/// both dialects (RE: SSL 19/19 sidecars per dialect, identical control names and file names):
/// * EDT `Forms/<F>/Form.form` → `Forms/<F>/Items/<ctl>/<Tag>.<ext>`;
/// * Designer `Forms/<F>.xml` → `Forms/<F>/Ext/Form/Items/<ctl>/<Tag>.<ext>`.
///
/// `None` for cf (a container — the picture is INLINED in the form body, no sidecar).
pub(crate) fn form_items_dir(format: Format, descriptor_path: &Path) -> Option<PathBuf> {
    Some(
        form_module_path(format, descriptor_path)?
            .parent()?
            .join("Items"),
    )
}

/// Every control of `body`, in tree order: the root items (recursing through `children`, table
/// `additions`, each control's own `auto_command_bar`, AND its `contextMenu` items) plus the
/// form-level command bar's items. A `contextMenu`'s buttons ARE full `FormItem`s and CAN carry a
/// picture sidecar (witness ERP W21 ВидыБюджетов.ФормаЭлемента `ЭлементыОтчетаРазгруппировать1`
/// `Picture.png`) — omitting them orphaned the file (`ensure_no_orphan_pictures`, §1.0). The
/// extended tooltip (`Tooltip`) body carries no nested items.
fn walk_items_mut(
    body: &mut FormBody,
    f: &mut impl FnMut(&mut FormItem) -> Result<(), ConvertError>,
) -> Result<(), ConvertError> {
    fn go(
        item: &mut FormItem,
        f: &mut impl FnMut(&mut FormItem) -> Result<(), ConvertError>,
    ) -> Result<(), ConvertError> {
        f(item)?;
        for c in &mut item.children {
            go(c, f)?;
        }
        for a in &mut item.additions {
            go(a, f)?;
        }
        if let Some(acb) = &mut item.auto_command_bar {
            for c in &mut acb.items {
                go(c, f)?;
            }
        }
        // Context-menu buttons are full controls and CAN carry a picture sidecar (§1.0 W21).
        if let Some(cm) = &mut item.context_menu {
            if let DecoratorBody::ContextMenu(cb) = &mut cm.body {
                for c in &mut cb.items {
                    go(c, f)?;
                }
            }
        }
        Ok(())
    }
    for item in &mut body.items {
        go(item, f)?;
    }
    if let Some(acb) = &mut body.auto_command_bar {
        for c in &mut acb.items {
            go(c, f)?;
        }
    }
    Ok(())
}

/// Read-only twin of [`walk_items_mut`] (shared with [`crate::form_write`]).
pub(crate) fn walk_items<'a>(body: &'a FormBody, f: &mut impl FnMut(&'a FormItem)) {
    fn go<'a>(item: &'a FormItem, f: &mut impl FnMut(&'a FormItem)) {
        f(item);
        item.children.iter().for_each(|c| go(c, f));
        item.additions.iter().for_each(|a| go(a, f));
        if let Some(acb) = &item.auto_command_bar {
            acb.items.iter().for_each(|c| go(c, f));
        }
        // Context-menu buttons are full controls and CAN carry a picture sidecar (§1.0 W21).
        if let Some(cm) = &item.context_menu {
            if let DecoratorBody::ContextMenu(cb) = &cm.body {
                cb.items.iter().for_each(|c| go(c, f));
            }
        }
    }
    body.items.iter().for_each(|i| go(i, f));
    if let Some(acb) = &body.auto_command_bar {
        acb.items.iter().for_each(|c| go(c, f));
    }
}

/// Attach the INLINE picture sidecar of every control that declares one, and normalise the canon
/// to `Ref("abs:<ext>")` in both dialects.
///
/// §1.0 — three ways this refuses rather than dropping bytes:
/// * a descriptor that CLAIMS a sidecar picture but has NO file on disk (the bytes would be
///   fabricated as empty on write);
/// * a control whose sidecar stem matches MORE than one file (`Picture.zip` + `Picture.png` —
///   the extension would be picked arbitrarily);
/// * an ORPHAN file on disk under `Items/` that no control claims (the bytes would be dropped).
fn attach_form_picture_sidecars(
    items_dir: &Path,
    kind: &str,
    owner: &str,
    form_name: &str,
    body: &mut FormBody,
) -> Result<(), ConvertError> {
    let err = |reason: String| ConvertError::Read {
        kind: kind.to_string(),
        object: format!("{owner}.{form_name}"),
        reason,
    };
    // Files consumed by a control — anything left over under `Items/` is an orphan.
    let mut consumed: Vec<PathBuf> = Vec::new();
    walk_items_mut(body, &mut |item: &mut FormItem| {
        let slots = sidecar_slots(item);
        if slots.is_empty() {
            return Ok(());
        }
        let ctl_dir = items_dir.join(&item.name);
        for (slot, des_ext) in slots {
            // Designer states the extension in `<xr:Abs>`; EDT states nothing — the file does.
            let file = match &des_ext {
                Some(ext) => {
                    let p = ctl_dir.join(format!("{}.{ext}", slot.stem));
                    if !p.is_file() {
                        return Err(err(format!(
                            "control {:?}: descriptor declares the sidecar picture {:?}, but the \
                             file is missing (silent drop forbidden — §1.0)",
                            item.name,
                            p.display()
                        )));
                    }
                    p
                }
                None => {
                    let mut found = matching_sidecars(&ctl_dir, slot.stem)?;
                    match found.len() {
                        1 => found.remove(0),
                        0 => {
                            return Err(err(format!(
                                "control {:?}: descriptor declares an inline (sidecar) picture \
                                 {:?}, but no {}.<ext> file exists under {:?} (silent drop \
                                 forbidden — §1.0)",
                                item.name,
                                slot.stem,
                                slot.stem,
                                ctl_dir.display()
                            )));
                        }
                        _ => {
                            return Err(err(format!(
                                "control {:?}: {} sidecar files match {}.<ext> under {:?} — the \
                                 extension is ambiguous (§1.0)",
                                item.name,
                                found.len(),
                                slot.stem,
                                ctl_dir.display()
                            )));
                        }
                    }
                }
            };
            let bytes = std::fs::read(&file).map_err(|e| ConvertError::Io {
                path: file.display().to_string(),
                reason: e.to_string(),
            })?;
            let file_name = file
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let ext = file_name.rsplit('.').next().unwrap_or_default().to_string();
            set_sidecar_ext(item, &slot, &ext);
            item.pictures.push(FormPicture { file_name, bytes });
            consumed.push(file);
        }
        Ok(())
    })?;
    ensure_no_orphan_pictures(items_dir, &consumed, &err)
}

/// Files under `ctl_dir` whose stem is exactly `stem` (`Picture.zip`, `Picture.png`, …). The
/// dir may not exist — then there are none.
fn matching_sidecars(ctl_dir: &Path, stem: &str) -> Result<Vec<PathBuf>, ConvertError> {
    if !ctl_dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in std::fs::read_dir(ctl_dir).map_err(|e| ConvertError::Io {
        path: ctl_dir.display().to_string(),
        reason: e.to_string(),
    })? {
        let entry = entry.map_err(|e| ConvertError::Io {
            path: ctl_dir.display().to_string(),
            reason: e.to_string(),
        })?;
        let p = entry.path();
        if p.is_file() && p.file_stem().is_some_and(|s| s == stem) {
            out.push(p);
        }
    }
    out.sort();
    Ok(out)
}

/// §1.0: every file under `Items/` must have been claimed by a control — an unclaimed one would
/// be silently dropped on convert.
fn ensure_no_orphan_pictures(
    items_dir: &Path,
    consumed: &[PathBuf],
    err: &impl Fn(String) -> ConvertError,
) -> Result<(), ConvertError> {
    if !items_dir.is_dir() {
        return Ok(());
    }
    for ctl in std::fs::read_dir(items_dir).map_err(|e| ConvertError::Io {
        path: items_dir.display().to_string(),
        reason: e.to_string(),
    })? {
        let ctl = ctl.map_err(|e| ConvertError::Io {
            path: items_dir.display().to_string(),
            reason: e.to_string(),
        })?;
        let ctl_dir = ctl.path();
        if !ctl_dir.is_dir() {
            continue;
        }
        for f in std::fs::read_dir(&ctl_dir).map_err(|e| ConvertError::Io {
            path: ctl_dir.display().to_string(),
            reason: e.to_string(),
        })? {
            let f = f.map_err(|e| ConvertError::Io {
                path: ctl_dir.display().to_string(),
                reason: e.to_string(),
            })?;
            let p = f.path();
            if p.is_file() && !consumed.contains(&p) {
                return Err(err(format!(
                    "orphan form-item sidecar {:?} — no control claims it (silent drop \
                     forbidden — §1.0)",
                    p.display()
                )));
            }
        }
    }
    Ok(())
}

/// The per-form DESCRIPTOR ANCHOR from which [`form_body_path`]/[`form_module_path`] derive the
/// body/module locations for the form named `form_name` of a `kind` object at `descriptor_path`.
///
/// * `CommonForm` (own body) — the descriptor itself (`form_name` is the object name).
/// * Subordinate owner — the VIRTUAL per-form descriptor under the owner's `Forms/` subtree:
///   EDT `<obj-dir>/Forms/<FormName>/<FormName>.mdo` (no such file exists — anchor only);
///   Designer `<dir>/<Owner>/Forms/<FormName>.xml` (the real nested form descriptor).
///
/// `None` for kinds without a registered form-sidecar layout, or when the descriptor path has no
/// parent/stem. Shared by [`crate::form_write`] (target paths) and [`crate::survey`] (per-form
/// roundtrip) so read, write and survey resolve identically.
pub(crate) fn form_anchor_path(
    format: Format,
    kind: &str,
    descriptor_path: &Path,
    form_name: &str,
) -> Option<PathBuf> {
    if kind == OWN_FORM_KIND {
        return Some(descriptor_path.to_path_buf());
    }
    if !SUBORDINATE_FORM_OWNERS.contains(&kind) {
        return None;
    }
    match format {
        // EDT: owner `<obj-dir>/<Owner>.mdo` → anchor `<obj-dir>/Forms/<FormName>/<FormName>.mdo`.
        Format::Edt => {
            let obj_dir = descriptor_path.parent()?;
            Some(
                obj_dir
                    .join("Forms")
                    .join(form_name)
                    .join(format!("{form_name}.mdo")),
            )
        }
        // Designer: owner `<dir>/<Owner>.xml` → anchor `<dir>/<Owner>/Forms/<FormName>.xml`.
        Format::Designer => {
            let dir = descriptor_path.parent()?;
            let stem = descriptor_path.file_stem()?;
            Some(
                dir.join(stem)
                    .join("Forms")
                    .join(format!("{form_name}.xml")),
            )
        }
        Format::Cf => None,
    }
}

/// `(form body path, form dialect)` beside the descriptor ANCHOR, per format layout (see module
/// docs). `None` for formats without a file-per-object sidecar (cf is a container). Shared with
/// [`crate::form_write`] (the WRITE side derives the target body path the same way).
pub(crate) fn form_body_path(
    format: Format,
    descriptor_path: &Path,
) -> Option<(PathBuf, FormDialect)> {
    match format {
        // EDT: descriptor `<obj-dir>/<Name>.mdo` → sibling `<obj-dir>/Form.form`.
        Format::Edt => {
            let obj_dir = descriptor_path.parent()?;
            Some((obj_dir.join("Form.form"), FormDialect::Edt))
        }
        // Designer: descriptor `<dir>/<Name>.xml` → `<dir>/<Name>/Ext/Form.xml`.
        Format::Designer => {
            let dir = descriptor_path.parent()?;
            let stem = descriptor_path.file_stem()?;
            Some((
                dir.join(stem).join("Ext").join("Form.xml"),
                FormDialect::Designer,
            ))
        }
        Format::Cf => None,
    }
}

pub(crate) fn ordinary_form_body_path(format: Format, anchor: &Path) -> Option<PathBuf> {
    match format {
        Format::Edt => Some(anchor.parent()?.join("Form.oform")),
        Format::Designer => Some(
            anchor
                .parent()?
                .join(anchor.file_stem()?)
                .join("Ext/Form.bin"),
        ),
        Format::Cf => None,
    }
}
pub(crate) fn declared_form_is_ordinary(
    obj: &MetadataObject,
    name: &str,
) -> Result<bool, ConvertError> {
    let owner = if obj.kind.as_str() == OWN_FORM_KIND && obj.name == name {
        obj
    } else {
        obj.children
            .iter()
            .find(|c| is_form_ref_child(c) && c.name == name)
            .ok_or_else(|| ConvertError::Read {
                kind: obj.kind.as_str().to_string(),
                object: name.to_string(),
                reason: "form body has no exact declared owner".into(),
            })?
    };
    let spec = morph1c_core::spec::registry::spec_for(owner.kind.as_str()).ok_or_else(|| {
        ConvertError::Read {
            kind: owner.kind.as_str().to_string(),
            object: name.to_string(),
            reason: "form kind has no metadata specification".into(),
        }
    })?;
    let field = spec
        .fields()
        .iter()
        .find(|f| f.name == "formType")
        .ok_or_else(|| ConvertError::Read {
            kind: owner.kind.as_str().to_string(),
            object: name.to_string(),
            reason: "form type field is not modeled".into(),
        })?;
    match owner
        .properties
        .iter()
        .find(|(id, _)| *id == field.id)
        .map(|(_, v)| v)
    {
        None => Ok(false),
        Some(morph1c_core::ir::PropertyValue::Enum(token)) if token.as_str() == "Managed" => {
            Ok(false)
        }
        Some(morph1c_core::ir::PropertyValue::Enum(token)) if token.as_str() == "Ordinary" => {
            Ok(true)
        }
        _ => Err(ConvertError::Read {
            kind: owner.kind.as_str().to_string(),
            object: name.to_string(),
            reason: "unknown declared form type".into(),
        }),
    }
}
fn read_ordinary_body(path: &Path) -> Result<Vec<u8>, ConvertError> {
    let bytes = read_regular_source(path).map_err(|error| ConvertError::Io {
        path: path.display().to_string(), reason: error.to_string(),
    })?;
    if bytes.is_empty() {
        return Err(ConvertError::Io { path: path.display().to_string(), reason: "ordinary body is empty".into() });
    }
    Ok(bytes)
}

/// Form MODULE (`Module.bsl`) path beside the descriptor ANCHOR, per format layout (see module
/// docs). EDT `<obj-dir>/Module.bsl`; Designer `<dir>/<Name>/Ext/Form/Module.bsl`. `None` for cf.
/// Shared with [`crate::form_write`].
pub(crate) fn form_module_path(format: Format, descriptor_path: &Path) -> Option<PathBuf> {
    match format {
        Format::Edt => {
            let obj_dir = descriptor_path.parent()?;
            Some(obj_dir.join("Module.bsl"))
        }
        Format::Designer => {
            let dir = descriptor_path.parent()?;
            let stem = descriptor_path.file_stem()?;
            Some(dir.join(stem).join("Ext").join("Form").join("Module.bsl"))
        }
        Format::Cf => None,
    }
}

#[cfg(any())]
mod tests {
    use super::*;
    use morph1c_core::ir::{ObjectKind, Uuid};

    #[test]
    fn edt_form_body_path_is_sibling() {
        let p = Path::new("/root/CommonForms/Форма_Группы/Форма_Группы.mdo");
        let (body, dialect) = form_body_path(Format::Edt, p).unwrap();
        assert!(
            body.ends_with(Path::new("CommonForms/Форма_Группы/Form.form")),
            "got {body:?}"
        );
        assert!(matches!(dialect, FormDialect::Edt));
        let m = form_module_path(Format::Edt, p).unwrap();
        assert!(
            m.ends_with(Path::new("CommonForms/Форма_Группы/Module.bsl")),
            "got {m:?}"
        );
    }

    #[test]
    fn designer_form_body_path_is_ext_subdir() {
        let p = Path::new("/root/CommonForms/Форма_Группы.xml");
        let (body, dialect) = form_body_path(Format::Designer, p).unwrap();
        assert!(
            body.ends_with(Path::new("CommonForms/Форма_Группы/Ext/Form.xml")),
            "got {body:?}"
        );
        assert!(matches!(dialect, FormDialect::Designer));
        let m = form_module_path(Format::Designer, p).unwrap();
        assert!(
            m.ends_with(Path::new("CommonForms/Форма_Группы/Ext/Form/Module.bsl")),
            "got {m:?}"
        );
    }

    #[test]
    fn edt_subordinate_anchor_resolves_forms_subtree() {
        // Owner `Catalogs/Спр/Спр.mdo`, form "Форма" → body `Catalogs/Спр/Forms/Форма/Form.form`,
        // module `Catalogs/Спр/Forms/Форма/Module.bsl` (RE: coverage/s15_subordinate edt).
        let p = Path::new("/root/Catalogs/Спр/Спр.mdo");
        let anchor = form_anchor_path(Format::Edt, "Catalog", p, "Форма").unwrap();
        let (body, dialect) = form_body_path(Format::Edt, &anchor).unwrap();
        assert!(
            body.ends_with(Path::new("Catalogs/Спр/Forms/Форма/Form.form")),
            "got {body:?}"
        );
        assert!(matches!(dialect, FormDialect::Edt));
        let m = form_module_path(Format::Edt, &anchor).unwrap();
        assert!(
            m.ends_with(Path::new("Catalogs/Спр/Forms/Форма/Module.bsl")),
            "got {m:?}"
        );
    }

    #[test]
    fn designer_subordinate_anchor_resolves_forms_subtree() {
        // Owner `Catalogs/Спр.xml`, form "Форма" → body `Catalogs/Спр/Forms/Форма/Ext/Form.xml`,
        // module `Catalogs/Спр/Forms/Форма/Ext/Form/Module.bsl` (RE: coverage/s15_subordinate).
        let p = Path::new("/root/Catalogs/Спр.xml");
        let anchor = form_anchor_path(Format::Designer, "Catalog", p, "Форма").unwrap();
        let (body, dialect) = form_body_path(Format::Designer, &anchor).unwrap();
        assert!(
            body.ends_with(Path::new("Catalogs/Спр/Forms/Форма/Ext/Form.xml")),
            "got {body:?}"
        );
        assert!(matches!(dialect, FormDialect::Designer));
        let m = form_module_path(Format::Designer, &anchor).unwrap();
        assert!(
            m.ends_with(Path::new("Catalogs/Спр/Forms/Форма/Ext/Form/Module.bsl")),
            "got {m:?}"
        );
    }

    #[test]
    fn non_owner_kind_has_no_anchor() {
        // Constant owns no forms — no anchor, and attach is a no-op.
        let p = Path::new("/root/Constants/К.xml");
        assert!(form_anchor_path(Format::Designer, "Constant", p, "Форма").is_none());
    }

    #[test]
    fn non_form_kind_is_noop() {
        let mut obj = MetadataObject::new(ObjectKind::new("Constant"), "C", Uuid([0; 16]));
        attach_form_body(
            Format::Edt,
            "Constant",
            Path::new("/nope/C/C.mdo"),
            &mut obj,
        )
        .unwrap();
        assert!(obj.form_bodies.is_empty());
    }

    #[test]
    fn common_form_without_body_is_noop() {
        // The form body is OPTIONAL: a bodyless property-stub CommonForm (e.g. s4_common's
        // `ОбщФорма_*`) reads with NO body attached and NO error (reflects the on-disk state).
        let mut obj =
            MetadataObject::new(ObjectKind::new("CommonForm"), "Форма_Нет", Uuid([0; 16]));
        attach_form_body(
            Format::Edt,
            "CommonForm",
            Path::new("/nonexistent-root/CommonForms/Форма_Нет/Форма_Нет.mdo"),
            &mut obj,
        )
        .expect("bodyless CommonForm must be a no-op, not an error");
        assert!(
            obj.form_bodies.is_empty(),
            "no body attached when the sidecar is absent"
        );
    }

    #[test]
    fn owner_without_form_children_is_noop() {
        // A Catalog with NO `.FormRef` children declares no forms — nothing to resolve.
        let mut obj = MetadataObject::new(ObjectKind::new("Catalog"), "Спр", Uuid([0; 16]));
        obj.children.push(MetadataObject::new(
            ObjectKind::new("Catalog.Attribute"),
            "Реквизит",
            Uuid([1; 16]),
        ));
        attach_form_body(
            Format::Edt,
            "Catalog",
            Path::new("/nope/Catalogs/Спр/Спр.mdo"),
            &mut obj,
        )
        .unwrap();
        assert!(obj.form_bodies.is_empty());
    }

    /// A synthetic Catalog owner declaring the s15 fixture's form as a `.FormRef` child (name +
    /// order come from children, exactly what the descriptor connectors produce).
    fn s15_catalog_owner() -> MetadataObject {
        let mut obj =
            MetadataObject::new(ObjectKind::new("Catalog"), "Спр_Подчиненные", Uuid([0; 16]));
        obj.children.push(MetadataObject::new(
            ObjectKind::new("Catalog.FormRef"),
            "Форма",
            Uuid([1; 16]),
        ));
        obj
    }

    #[test]
    fn attach_subordinate_form_edt_fixture() {
        // Real committed witness: coverage/edt/s15_subordinate Catalog `Спр_Подчиненные` with one
        // declared form `Форма` (body `Forms/Форма/Form.form` + module `Forms/Форма/Module.bsl`).
        let desc = morph1c_testkit::fixtures_root()
            .join("coverage/edt/s15_subordinate/src/Catalogs/Спр_Подчиненные/Спр_Подчиненные.mdo");
        if !desc.is_file() {
            eprintln!("attach_subordinate_form_edt_fixture INFRA-SKIP (corpus absent)");
            return;
        }
        let mut obj = s15_catalog_owner();
        attach_form_body(Format::Edt, "Catalog", &desc, &mut obj).expect("attach edt");
        assert_eq!(obj.form_bodies.len(), 1, "one declared form attached");
        assert_eq!(obj.form_bodies[0].name, "Форма");
        assert!(
            obj.form_bodies[0].module.is_some(),
            "s15 form carries Module.bsl"
        );
    }

    #[test]
    fn attach_subordinate_form_designer_fixture() {
        // Same witness, Designer dialect: body `Forms/Форма/Ext/Form.xml` + module
        // `Forms/Форма/Ext/Form/Module.bsl` under the owner dir beside `Спр_Подчиненные.xml`.
        let desc = morph1c_testkit::fixtures_root()
            .join("coverage/designer/s15_subordinate/Catalogs/Спр_Подчиненные.xml");
        if !desc.is_file() {
            eprintln!("attach_subordinate_form_designer_fixture INFRA-SKIP (corpus absent)");
            return;
        }
        let mut obj = s15_catalog_owner();
        attach_form_body(Format::Designer, "Catalog", &desc, &mut obj).expect("attach designer");
        assert_eq!(obj.form_bodies.len(), 1, "one declared form attached");
        assert_eq!(obj.form_bodies[0].name, "Форма");
        assert!(
            obj.form_bodies[0].module.is_some(),
            "s15 form carries Ext/Form/Module.bsl"
        );
    }
}
