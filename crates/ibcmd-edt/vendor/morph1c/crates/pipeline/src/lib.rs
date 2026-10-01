//! `morph1c-pipeline` — whole-configuration orchestration (ARCHITECTURE.md §2.1,
//! hub-and-spokes). The per-`(format, kind)` byte-exact descriptor readers/writers
//! already exist (registered per format crate as `metadata::FORMAT_KINDS`); this crate
//! is the assembly that COMPOSES them into a config-level converter:
//! `read_config → Configuration` and `Configuration → write_config`, with
//! `convert = compose(read_config, write_config)` across formats through the canonical IR.
//!
//! # Scope
//! The per-kind `read`/`write` operate on a SINGLE descriptor file's bytes; the module `.bsl`
//! bodies, form bodies, templates, rights, help and the rest live in SEPARATE sidecar files /
//! container elements. Those are carried by the `*_read`/`*_write` attach passes composed around
//! the per-kind codecs here, so a whole-config convert is descriptor- AND body-complete for every
//! TARGET (edt/xml/cf). The direction still being closed is **cf as a SOURCE**: the
//! configuration ROOT, the object descriptors and the Language entities are read (`cf → IR`),
//! the object BODIES are not yet. See [`SIDECAR_NOTE`].
//!
//! # §1.0 honesty (the key constraint)
//! A whole-config read enumerates EVERY kind-subdir on disk. Kinds with NO connector, or
//! whose per-file read errors, are NOT silently skipped: [`preflight`] produces a
//! structured [`PreflightReport`] of everything unconvertible, and [`convert`]/
//! [`read_config`] ERROR by default ([`ConvertError::Unsupported`]) listing them. The
//! `--only` selector ([`ConvertOptions::only`]) restricts work to named kinds; kinds
//! present on disk but excluded are then reported LOUDLY as intentionally skipped, never
//! dropped.

pub mod fsio;
pub mod layout;
pub mod registry;
mod source_extensions;

use std::path::Path;

use morph1c_core::version::FormatVersion;


pub use formats_xml::registry::Format;
pub use registry::FormatRegistry;
/// Re-export: `--v8version` parsing (platform → format version, FORMATS.md §2).
pub use morph1c_core::version::parse_v8version;

mod blob_template_read;
mod cmi_read;
mod config_interface_read;
mod command_module_read;
mod convert;
mod dcs_read;
mod exchange_plan_content_read;
mod ext_read;
mod flowchart_read;
mod form_read;
mod form_ref_read;
mod form_write;
mod geos_read;
mod graph_template_read;
mod help_read;
mod language_read;
mod language_write;
mod module_read;
mod mxl_read;
mod picture_read;
mod predefined_read;
mod preflight;
mod rights_read;
mod schedule_read;
mod sdk_body_projection;
mod sidecar_version;
mod standalone_content_read;
mod style_read;
mod subsystem_read;
mod table_ref_read;
mod template_read;
mod template_ref_read;
mod ws_definition_read;
mod xdto_read;

pub use convert::{read_config, write_config};
// Private adapter lab access to the same typed form/sidecar pipeline as whole
// configuration conversion. This adds no ibcmd public canonical model surface.
#[doc(hidden)]
pub use form_read::attach_form_body;
#[doc(hidden)]
pub use form_write::write_form_bodies;
#[doc(hidden)]
pub use sdk_body_projection::mxl_newlines as project_mxl_content_newlines;
pub use preflight::{check, preflight, KindStatus, KindSupport, PreflightEntry, PreflightReport};

/// Honest note surfaced by `convert`/`check`: what a whole-config conversion actually carries,
/// and the ONE direction that is still incomplete.
///
/// Descriptors AND bodies are transcoded for every TARGET (edt / xml / cf): module `.bsl`
/// sidecars, object command modules, form bodies, templates (TextDocument / MXL /
/// GeographicalSchema / DCS / blob), rights tables, help pages, predefined data, command
/// interfaces, pictures, XDTO schemas, BusinessProcess flowcharts, ScheduledJob schedules and the
/// config-level `Ext` of the root. The whole SSL configuration converts to a platform-loadable
/// `.cf` from BOTH source dialects (`vrunner cf_compare == 0`).
///
/// The remaining gap is **cf as a SOURCE**: a `.cf` yields the configuration ROOT (`cf → IR`,
/// self-checked by re-encoding) plus every object DESCRIPTOR — but not yet the object BODIES
/// (module/form/template/… `<uuid>.N` elements), so `cf → edt/xml` reproduces the descriptor
/// tree byte-exactly and still misses the body sidecars.
pub const SIDECAR_NOTE: &str = "source-only EDT/XML conversion requires typed coverage of every descriptor and body; unsupported kinds or source cells fail closed. Installed EDT/native acceptance belongs to the host oracle.";

/// Parse a TARGET-format selector from a CLI token (`edt` / `xml` / `cf`).
///
/// `xml` is the Designer (Конфигуратор) XML dump — the user-facing name of [`Format::Designer`];
/// `cf` is the binary container (a valid SOURCE since M3 AND a valid TARGET since M6 phase-2 —
/// the from-scratch assembler [`formats_cf::assemble_cf`]). An unknown token is an explicit
/// error, never a silent fallback (§1.0). The SOURCE format is never spelled out: it is detected
/// from the path ([`detect_source`]).
pub fn parse_format(s: &str) -> Result<Format, ConvertError> {
    match s {
        "edt" => Ok(Format::Edt),
        "xml" => Ok(Format::Designer),
        "cf" => Ok(Format::Cf),
        other => Err(ConvertError::FormatNotSupported(format!(
            "unknown format {other:?}; expected one of: edt, xml, cf"
        ))),
    }
}

/// The user-facing token of a format (the inverse of [`parse_format`]): `edt` / `xml` / `cf`.
///
/// Distinct from [`Format::code`] (the internal diagnostic code, which still says `designer`).
pub fn format_token(f: Format) -> &'static str {
    match f {
        Format::Edt => "edt",
        Format::Designer => "xml",
        Format::Cf => "cf",
    }
}

/// §1.0 honesty note for `--to cf`: cf WRITE assembles a whole `.cf` from IR — the service trio
/// (`root`/`version`/`versions`) + the config-root descriptor + every object's descriptor and its
/// bodies (`<uuid>.N`). An EXTENSION root is packaged as a `.cfe` instead.
///
/// The §1.0 guards remain and still REFUSE rather than emit a partial file: a kind with no cf
/// writer, an object whose required body cannot be emitted, or a config-root child with no
/// descriptor (a dangling reference) each abort with a typed error naming the offender. NB:
/// `--only <Kinds>` on a real config does NOT yield a partial file — it errors LOUDLY too (the
/// config-root still references the excluded children → dangling-reference / roster-gap error).
pub const CF_WRITE_SCOPE_NOTE: &str =
    "cf WRITE assembles a whole .cf from IR: service trio (root/version/versions) + config-root \
     descriptor + every object's descriptor and bodies (<uuid>.N — modules, forms, templates, \
     rights, help, pictures, XDTO, flowcharts, schedules, command interfaces). An EXTENSION root \
     (objectBelonging=Adopted) is packaged as a .cfe. Verified on the whole SSL configuration \
     from BOTH source dialects: the produced .cf loads and vrunner cf_compare reports 0 critical \
     differences. §1.0 guards still REFUSE (never a silent partial .cf): a kind with no cf \
     writer, an object whose required body cannot be emitted, or a config-root child with no \
     emitted descriptor (dangling reference) each abort with a typed error naming the offender. \
     --only on a real config errors too (the root still references the excluded children).";

/// cf is now a valid WRITE target (M6 phase-2). Kept for API compatibility / call sites that
/// want an explicit writability gate; every format is writable, so this is always `Ok`.
pub fn ensure_writable(_to: Format) -> Result<(), ConvertError> {
    Ok(())
}

/// Options controlling a whole-config conversion.
#[derive(Debug, Clone, Default)]
pub struct ConvertOptions {
    /// Restrict conversion to these canonical kind codes (`["CommonModule", …]`). Empty
    /// ⇒ convert EVERYTHING present (and then ANY unsupported kind on disk is a hard
    /// error, §1.0). Non-empty ⇒ only the named kinds are converted; OTHER kinds present
    /// on disk are reported LOUDLY as intentionally skipped, not silently dropped.
    pub only: Vec<String>,
    /// TARGET format version (`--v8version`, resolved platform→format via `VERSION_TABLE`).
    ///
    /// `None` ⇒ the target version is the SOURCE's own (round-trip preserves the dump's
    /// version — a 2.17 source writes a 2.17 dump). It is NOT defaulted to the newest
    /// format: silently upgrading a dump would rewrite its envelope and property set
    /// behind the user's back (§1.0).
    pub target_version: Option<FormatVersion>,
    /// Собирать `.cf` ПАКЕТНО: прочитать ВЕСЬ IR в память, потом кодировать (вместо потокового
    /// прохода [`crate::cf_stream`], который читает тело каждого объекта и сразу его кодирует).
    ///
    /// Обе руки обязаны давать ОДИН И ТОТ ЖЕ байт-образ — потоковый проход строит реестры по
    /// дескрипторам, но кодирует объекты, прочитанные ПОЛНОСТЬЮ, тем же `object_leaves`. Флаг
    /// существует, чтобы это можно было ПРОВЕРИТЬ (`tests/cf_lane_parity.rs` гоняет обе руки на
    /// каждой coverage-стадии и сверяет байты) и чтобы замер по стенке переключал руки ВНУТРИ
    /// одного образа процесса — на этой машине разброс между сборками ±40 %, кросс-сборочное
    /// A/B меньший эффект просто не видит.
    pub cf_batch_assembly: bool,
}

impl ConvertOptions {
    /// Build options restricting conversion to the given kinds.
    pub fn only(kinds: impl IntoIterator<Item = String>) -> Self {
        ConvertOptions {
            only: kinds.into_iter().collect(),
            ..Default::default()
        }
    }

    /// Собирать `.cf` пакетным лейном (см. [`ConvertOptions::cf_batch_assembly`]).
    pub fn with_cf_batch_assembly(mut self, batch: bool) -> Self {
        self.cf_batch_assembly = batch;
        self
    }

    /// Set the TARGET format version (`--v8version`).
    pub fn with_target_version(mut self, target: FormatVersion) -> Self {
        self.target_version = Some(target);
        self
    }

    /// Is the `--only` allow-list active (non-empty)?
    pub fn is_restricted(&self) -> bool {
        !self.only.is_empty()
    }

    /// Is `kind` selected for conversion under these options?
    pub fn includes(&self, kind: &str) -> bool {
        self.only.is_empty() || self.only.iter().any(|k| k == kind)
    }
}

/// Error of a whole-config pipeline operation (typed; no best-effort, §1.0).
#[derive(Debug)]
pub enum ConvertError {
    /// The requested format is not a pipeline format (e.g. `cf` — a later milestone).
    FormatNotSupported(String),
    /// The configuration carries properties introduced LATER than the requested
    /// `--v8version`: writing that target would silently drop them (§1.0). Carries EVERY
    /// offending property, not just the first (see `core::version::check_downgrade`).
    Downgrade(Box<morph1c_core::version::DowngradeError>),
    /// Цель НОВЕЕ источника и требует свойств, значения которых не засвидетельствованы
    /// (правило 2, см. `core::version::upgrade`). Несёт ВЕСЬ список, а не первое поле.
    Upgrade(Box<morph1c_core::version::UpgradeError>),
    /// The SOURCE format could not be determined from the given path ([`detect_source`]) —
    /// §1.0: no silent fallback to a default format. Carries the path and what was expected.
    Detect {
        /// The path handed to the CLI.
        path: String,
        /// Why it could not be classified (and what layouts are recognised).
        reason: String,
    },
    /// One or more kinds present on disk cannot be converted (no connector or a read
    /// error) and were not excluded via `--only`. Carries the structured report so the
    /// CLI can print every blocked kind with counts and reasons (§1.0 — loud, not silent).
    Unsupported(PreflightReport),
    /// A per-kind descriptor read failed (the connector returned an error for a file).
    Read {
        /// Canonical kind code.
        kind: String,
        /// Object name (descriptor file stem / dir name).
        object: String,
        /// Connector error string.
        reason: String,
    },
    /// A per-kind descriptor write failed.
    Write {
        /// Canonical kind code.
        kind: String,
        /// Object name.
        object: String,
        /// Connector error string.
        reason: String,
    },
    /// An object's kind has no writer in the target format (cannot emit it).
    NoWriter {
        /// Canonical kind code.
        kind: String,
    },
    /// A nested (Subsystem) directory hides descriptors under its `nesting_dir` but has no own
    /// descriptor — its nested subsystems are present on disk yet unreachable by traversal
    /// (§1.0 forbids a silent skip). Carries the kind and the hiding directory path.
    NestedOrphan {
        /// Canonical kind code (`"Subsystem"`).
        kind: String,
        /// The directory present on disk whose nested descriptors would be skipped.
        dir: String,
    },
    /// A parent subsystem DECLARES a nested child (`<subsystems>Name</subsystems>`) that has
    /// no descriptor on disk under its `nesting_dir` — a dangling reference (§1.0 — not
    /// silently dropped). Carries the parent's hierarchical key and the missing child name.
    NestedDangling {
        /// Canonical kind code (`"Subsystem"`).
        kind: String,
        /// Parent subsystem hierarchical key (`Parent/…`).
        parent: String,
        /// Declared child subsystem name with no descriptor on disk.
        child: String,
    },
    /// A nested subsystem present on disk is NOT declared by its parent descriptor — physical
    /// nesting and declared membership disagree (§1.0 honesty / correct hierarchical
    /// membership). Carries the enumerated subsystem's hierarchical key.
    NestedUndeclared {
        /// Canonical kind code (`"Subsystem"`).
        kind: String,
        /// The enumerated subsystem's hierarchical key absent from its parent's declarations.
        path: String,
    },
    /// A nested (Subsystem) object's hierarchical PARENT CHAIN could not be reconstructed to an
    /// on-disk path when WRITING. Either its `parentSubsystem` back-reference is malformed, or
    /// two nested objects collapse to the same path (own-name collision with no distinguishing
    /// parent chain — e.g. a source that carries no hierarchical identity). §1.0 forbids silently
    /// writing such an object to the flat top-level path (which would place it wrongly or
    /// OVERWRITE a sibling). Carries the offending object and the reason.
    NestedUnresolved {
        /// Canonical kind code (`"Subsystem"`).
        kind: String,
        /// The object's own name.
        object: String,
        /// Why the hierarchical write path could not be resolved.
        reason: String,
    },
    /// A cf physical/container failure (parse/inflate/collect) — typed, not best-effort.
    Cf {
        /// Human-readable reason (carries the cf connector / container error text).
        reason: String,
    },
    /// A `.cf` source has descriptor records claimed by NO connector (no cf reader for their
    /// kind) and `--only` was not used to scope the work. ALWAYS triggered by default because
    /// the Configuration root has no cf connector (§1.0 — loud, not silent). Carries the
    /// accounting so the CLI prints the unclaimed count + record-code histogram.

    /// A `.cf` source carries object BODIES (`<uuid>.N` elements) that no reader can decode
    /// yet, and `--only` was not used to scope the work. Refusing is the §1.0 requirement: the
    /// produced tree would LOOK like a configuration while silently missing modules / forms /
    /// templates / rights. Boxed to keep `ConvertError` small.

    /// Filesystem error (read/create/write a path).
    Io {
        /// Path involved.
        path: String,
        /// Underlying error.
        reason: String,
    },
}

impl std::fmt::Display for ConvertError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConvertError::FormatNotSupported(s) => write!(f, "format not supported: {s}"),
            ConvertError::Downgrade(e) => write!(f, "{e}"),
            ConvertError::Upgrade(e) => write!(f, "{e}"),
            ConvertError::Detect { path, reason } => {
                write!(f, "не удалось определить формат источника {path}: {reason}")
            }
            ConvertError::Unsupported(report) => {
                write!(
                    f,
                    "unsupported kinds present on disk (§1.0 — not silently skipped):\n{report}"
                )
            }
            ConvertError::Read {
                kind,
                object,
                reason,
            } => {
                write!(f, "read [{kind}] {object}: {reason}")
            }
            ConvertError::Write {
                kind,
                object,
                reason,
            } => {
                write!(f, "write [{kind}] {object}: {reason}")
            }
            ConvertError::NoWriter { kind } => {
                write!(f, "no writer for kind {kind:?} in target format")
            }
            ConvertError::NestedOrphan { kind, dir } => write!(
                f,
                "[{kind}] directory {dir:?} hides nested descriptors under its nesting subdir \
                 but has no own descriptor — present on disk yet not enumerated (§1.0 — no \
                 silent skip)"
            ),
            ConvertError::NestedDangling {
                kind,
                parent,
                child,
            } => write!(
                f,
                "[{kind}] parent {parent:?} declares nested child {child:?} but no descriptor \
                 exists on disk for it (§1.0 — dangling nested reference)"
            ),
            ConvertError::NestedUndeclared { kind, path } => write!(
                f,
                "[{kind}] nested subsystem {path:?} present on disk is not declared by its \
                 parent (§1.0 — membership mismatch)"
            ),
            ConvertError::NestedUnresolved {
                kind,
                object,
                reason,
            } => write!(
                f,
                "[{kind}] {object}: cannot reconstruct hierarchical write path — {reason} \
                 (§1.0 — no silent flat top-level fallback)"
            ),
            ConvertError::Cf { reason } => write!(f, "cf: {reason}"),
            ConvertError::Io { path, reason } => write!(f, "io {path}: {reason}"),
        }
    }
}

impl std::error::Error for ConvertError {}


pub(crate) fn source_format_version(from: Format, src: &Path) -> Option<FormatVersion> {
    /// Корневой тег Designer-дескриптора — версия формата живёт в ЕГО атрибуте.
    const ROOT_TAG: &str = "<MetaDataObject";

    match from {
        Format::Designer => {
            let root = src.join("Configuration.xml");
            let text = std::fs::read_to_string(&root).ok()?;
            // ВАЖНО: искать `version=` НЕ с начала файла — там XML-пролог
            // `<?xml version="1.0"?>`, и наивный поиск нашёл бы «1.0» вместо версии формата.
            // Версия дампа живёт в атрибуте КОРНЕВОГО тега `<MetaDataObject … version="2.17">`.
            let tag = text.find(ROOT_TAG)? + ROOT_TAG.len();
            let tag_end = tag + text[tag..].find('>')?;
            let head = &text[tag..tag_end];
            let at = head.find("version=\"")? + "version=\"".len();
            let end = at + head[at..].find('"')?;
            formats_designer::common::detect_profile(&head[at..end]).map(|p| p.format)
        }
        Format::Edt => {
            // `src` — это `…/src`; PROJECT.PMF живёт в сиблинге `DT-INF` корня проекта.
            let pmf = src.parent()?.join("DT-INF").join("PROJECT.PMF");
            let text = std::fs::read_to_string(&pmf).ok()?;
            let line = text
                .lines()
                .find_map(|l| l.strip_prefix("Runtime-Version:"))?;
            let mut it = line.trim().split('.');
            let (maj, min, patch) = (
                it.next()?.parse().ok()?,
                it.next()?.parse().ok()?,
                it.next().unwrap_or("0").parse().ok()?,
            );
            morph1c_core::version::format_for_platform(
                morph1c_core::version::PlatformVersion::new(maj, min, patch),
            )
        }
        Format::Cf => None,
    }
}

