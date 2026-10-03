//! §1.0-honest preflight: enumerate EVERY kind-subdir present on disk and classify it.
//!
//! A whole-config read must not silently skip anything. The preflight scans the format
//! root for kind-subdirs and reports, per kind: object count and support status
//! (supported / blocked-no-connector / blocked-read-error / intentionally-skipped). The
//! `check` command surfaces this honestly without converting; `read_config`/`convert`
//! turn any BLOCKED kind into a hard error unless `--only` excludes it.
//!
//! Discovery of kinds present on disk is itself registry-driven for SUPPORTED kinds (we
//! know their subdirs), plus a directory scan of the format root for ANY other subdir
//! (so a kind with NO connector is still SEEN and reported, not invisible).

use std::collections::BTreeMap;
use std::path::Path;

use formats_xml::registry::Format;

use crate::layout;
use crate::registry::FormatRegistry;
use crate::{ConvertError, ConvertOptions, SIDECAR_NOTE};

/// Whether a kind present on disk can be converted, and why not if it can't.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KindSupport {
    /// A connector exists and a probe read of EVERY descriptor of the kind succeeded.
    Supported,
    /// No `FormatKind` connector is registered for this kind in this format.
    NoConnector,
    /// A connector exists but reading a descriptor errored (the kind is not fully done).
    ReadError(String),
    /// The root's `ChildObjects` declares a different object count than found on disk —
    /// the dump is INCOMPLETE/BROKEN (§1.0: a silently-truncated dump must fail loudly,
    /// not convert partially; witnessed: a broken ERP dump carried 217 of 1208 catalogs).
    CountMismatch {
        /// Objects of the kind declared by the root descriptor.
        declared: usize,
        /// Object descriptors actually found on disk.
        found: usize,
    },
    /// Excluded by `--only` (present on disk, intentionally not converted). Loud, not silent.
    SkippedByOnly,
}

impl KindSupport {
    /// Is this kind convertible (a connector exists and read probe passed)?
    pub fn is_supported(&self) -> bool {
        matches!(self, KindSupport::Supported)
    }
    /// Is this kind BLOCKED (cannot convert: no connector, a read error, or a
    /// declared-vs-on-disk count mismatch)? Excludes the intentional `--only` skip,
    /// which is a deliberate choice, not a blocker.
    pub fn is_blocked(&self) -> bool {
        matches!(
            self,
            KindSupport::NoConnector | KindSupport::ReadError(_) | KindSupport::CountMismatch { .. }
        )
    }
}

/// One kind found on disk in the format root, with its object count and support status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreflightEntry {
    /// Canonical kind code if known (connector kind), else the on-disk subdir name.
    pub kind: String,
    /// Object descriptors counted on disk for this kind.
    pub object_count: usize,
    /// Support status.
    pub support: KindSupport,
}

/// Backward-friendly alias for a per-kind status (kind + support).
pub type KindStatus = PreflightEntry;

/// Structured §1.0 report: every kind-subdir present on disk, classified. Ordered
/// deterministically by kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreflightReport {
    /// Format scanned.
    pub format: Format,
    /// Per-kind entries (sorted by kind).
    pub entries: Vec<PreflightEntry>,
}

impl PreflightReport {
    /// Kinds that are blocked (no connector or read error) — the §1.0 hard-error set.
    pub fn blocked(&self) -> impl Iterator<Item = &PreflightEntry> {
        self.entries.iter().filter(|e| e.support.is_blocked())
    }
    /// Kinds that are supported (convertible).
    pub fn supported(&self) -> impl Iterator<Item = &PreflightEntry> {
        self.entries.iter().filter(|e| e.support.is_supported())
    }
    /// Are there any blocked kinds?
    pub fn has_blocked(&self) -> bool {
        self.blocked().next().is_some()
    }
}

impl std::fmt::Display for PreflightReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "preflight ({}): {} kind(s) on disk",
            self.format.code(),
            self.entries.len()
        )?;
        for e in &self.entries {
            let tag = match &e.support {
                KindSupport::Supported => "supported".to_string(),
                KindSupport::NoConnector => "BLOCKED: no connector".to_string(),
                KindSupport::ReadError(r) => format!("BLOCKED: read error: {r}"),
                KindSupport::CountMismatch { declared, found } => format!(
                    "BLOCKED: root declares {declared} object(s), found {found} on disk — \
                     the dump is incomplete/broken (§1.0)"
                ),
                KindSupport::SkippedByOnly => "skipped (--only)".to_string(),
            };
            writeln!(
                f,
                "  {:<28} {:>5} object(s)  [{}]",
                e.kind, e.object_count, tag
            )?;
        }
        write!(f, "note: {SIDECAR_NOTE}")
    }
}

/// Designer SIDECAR directories that hold per-entity companion files consumed by a
/// whole-config ATTACH pass, NOT per-object metadata descriptors of a convertible kind.
/// They must not be reported as blocked "no connector" kinds (§1.0): the pipeline reads them.
///
/// * `Languages` — `Languages/<Name>.xml` language bodies. Designer stores the Language
///   entity as a sidecar; the root `Configuration.xml` lists it in `<ChildObjects>` and the
///   pipeline attaches its `F_LANGUAGES` from these files ([`crate::language_read`]). The
///   Language MEMBERSHIP is still checked via the root's ChildObjects, so an undeclared or
///   missing sidecar is caught there — this only stops the raw directory from being mistaken
///   for an unconvertible metadata kind.
/// * `Ext` — the CONFIG-LEVEL Ext of the root `Configuration` object: the four application
///   modules (`ManagedApplicationModule`/`OrdinaryApplicationModule`/`SessionModule`/
///   `ExternalConnectionModule` `.bsl`) and the root pictures (`Splash`/`MainSectionPicture`
///   ExtPicture wrapper + `Picture.<ext>`). The whole-config read attaches them to the root
///   object ([`crate::ext_read`]); they are per-ROOT companion files, not per-object
///   descriptors of a metadata kind.
const DESIGNER_SIDECAR_DIRS: &[&str] = &["Ext", "Languages"];

/// Is `name` a recognized sidecar directory for `format` (consumed by an attach pass, not a
/// convertible metadata kind)? Only Designer carries file-per-entity sidecar dirs here.
fn is_sidecar_dir(format: Format, name: &str) -> bool {
    format == Format::Designer && DESIGNER_SIDECAR_DIRS.contains(&name)
}

/// On-disk discovery: the canonical kind code for a subdir name, if a connector claims it.
///
/// We map each connector's on-disk subdir (last rel-dir segment) to its kind, then look up
/// the scanned subdir name. Subdirs with no match are unknown kinds (no connector).
fn subdir_to_kind(reg: &FormatRegistry) -> BTreeMap<String, &'static str> {
    let mut m = BTreeMap::new();
    for fk in reg.iter() {
        let rel = layout::kind_rel_dir(reg, fk);
        // Only single-segment per-kind subdirs participate in subdir discovery; the root
        // (Configuration) is handled separately (it is not a kind-subdir of its own name
        // for Designer, and for EDT lives in a `Configuration/` dir handled via registry).
        if rel.len() == 1 {
            m.insert(rel[0].clone(), fk.kind);
        }
    }
    m
}

/// Run the §1.0 preflight scan over the format root `src`.
///
/// Enumerates: (a) the root descriptor (SingletonFile) and (b) every immediate subdir of
/// the format root that holds object descriptors. Each is classified. A SUPPORTED kind is
/// one with a connector that reads ALL of its descriptors without error; the first failing
/// descriptor makes it `ReadError` (blocked); a subdir with no connector is `NoConnector`.
/// With `opts.only` active, supported kinds NOT in the allow-list become `SkippedByOnly`.
pub fn preflight(
    format: Format,
    src: &Path,
    opts: &ConvertOptions,
) -> Result<PreflightReport, ConvertError> {
    let reg = FormatRegistry::for_format(format)?;
    let known = subdir_to_kind(&reg);

    let mut entries: Vec<PreflightEntry> = Vec::new();

    // (1) The root descriptor (SingletonFile), if present.
    if let Some(root_fk) = reg.root_kind() {
        if let Some(path) = layout::singleton_path(&reg, root_fk, src) {
            if path.is_file() {
                let support =
                    probe_kind(root_fk, &[(root_fk.kind.to_string(), path.clone())], opts);
                entries.push(PreflightEntry {
                    kind: root_fk.kind.to_string(),
                    object_count: 1,
                    support,
                });
            }
        }
    }

    // (2) Every immediate subdir of the format root.
    if let Ok(rd) = std::fs::read_dir(src) {
        let mut subdirs: Vec<String> = Vec::new();
        for entry in rd.flatten() {
            let p = entry.path();
            if !p.is_dir() {
                continue;
            }
            if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                subdirs.push(name.to_string());
            }
        }
        subdirs.sort();
        subdirs.dedup();

        for name in subdirs {
            // The EDT root lives in a `Configuration/` subdir already reported via (1) —
            // skip re-reporting it as an unknown subdir.
            if reg
                .root_kind()
                .map(|r| layout::kind_rel_dir(&reg, r) == vec![name.clone()])
                .unwrap_or(false)
            {
                continue;
            }

            // Sidecar directories (`Languages/`) are companion files attached by the
            // whole-config read, NOT convertible metadata kinds. Skip them so they are not
            // reported as blocked "no connector" (§1.0: they ARE consumed — the language
            // attach reads them, and the root's ChildObjects governs membership).
            if is_sidecar_dir(format, &name) {
                continue;
            }

            match known.get(&name) {
                Some(kind) => {
                    let fk = reg.get(kind).expect("known kind has connector");
                    let objs = layout::collect_objects(&reg, fk, src);
                    if objs.is_empty() {
                        // Subdir exists but holds no matching descriptors — report 0 so it
                        // is visible (still counts as a present, supported-if-probe-ok kind).
                        entries.push(PreflightEntry {
                            kind: (*kind).to_string(),
                            object_count: 0,
                            support: support_for_only(kind, opts),
                        });
                    } else {
                        let support = probe_kind(fk, &objs, opts);
                        entries.push(PreflightEntry {
                            kind: (*kind).to_string(),
                            object_count: objs.len(),
                            support,
                        });
                    }
                }
                None => {
                    // No connector for this on-disk subdir → blocked (§1.0: SEEN, not hidden).
                    let count = count_descriptors(&p_join(src, &name));
                    entries.push(PreflightEntry {
                        kind: name,
                        object_count: count,
                        support: KindSupport::NoConnector,
                    });
                }
            }
        }
    }

    // §1.0 declared-vs-on-disk: `ChildObjects` корня — НЕЗАВИСИМАЯ декларация состава
    // конфигурации. Дамп, где на диске МЕНЬШЕ объектов, чем объявлено, — битый/неполный
    // (witnessed: старая ERP-выгрузка несла 217 справочников из 1208 и молча конвертировалась
    // бы частично). Такой вид блокируется громко, а объявленный вид ВООБЩЕ без каталога
    // появляется в отчёте как CountMismatch{…, found: 0}.
    if let Some(root_fk) = reg.root_kind() {
        if let Some(path) = layout::singleton_path(&reg, root_fk, src) {
            if let Ok(bytes) = std::fs::read(&path) {
                if let Ok(root) = (root_fk.read)(&bytes) {
                    apply_declared_counts(&root, &mut entries);
                }
            }
        }
    }

    entries.sort_by(|a, b| a.kind.cmp(&b.kind));
    Ok(PreflightReport { format, entries })
}

/// Сверить объявленные корнем (`ChildObjects`) счётчики видов с найденными на диске.
///
/// * `found < declared` ⇒ [`KindSupport::CountMismatch`] (битый дамп; НЕ перекрывает более
///   точный `ReadError`).
/// * `found > declared` — ЛЕГАЛЬНО: вложенные объекты лежат на диске, но корень объявляет
///   только top-level (Subsystem: 910 на диске при десятках top-level в ChildObjects).
/// * `Language` — inline-вид (сайдкар, не каталог-вид) — исключён.
fn apply_declared_counts(
    root: &morph1c_core::ir::MetadataObject,
    entries: &mut Vec<PreflightEntry>,
) {
    use morph1c_core::ir::value::PropertyValue;
    let rows = match root.get(morph1c_core::spec::metadata::configuration::F_CHILD_OBJECTS) {
        Some(PropertyValue::List(r)) => r,
        _ => return,
    };
    let mut declared: BTreeMap<String, usize> = BTreeMap::new();
    for row in rows {
        if let PropertyValue::List(cells) = row {
            if let Some(PropertyValue::Str(kind)) = cells.first() {
                *declared.entry(kind.clone()).or_default() += 1;
            }
        }
    }
    declared.remove("Language");
    for (kind, dec) in &declared {
        match entries.iter_mut().find(|e| &e.kind == kind) {
            Some(e) => {
                if e.object_count < *dec && !matches!(e.support, KindSupport::ReadError(_)) {
                    e.support = KindSupport::CountMismatch {
                        declared: *dec,
                        found: e.object_count,
                    };
                }
            }
            None => entries.push(PreflightEntry {
                kind: kind.clone(),
                object_count: 0,
                support: KindSupport::CountMismatch {
                    declared: *dec,
                    found: 0,
                },
            }),
        }
    }
}

/// Public `check`: a preflight scan with NO conversion (honest status listing). `--only`
/// is irrelevant to `check` (it reports ALL present kinds' real support), so it scans with
/// empty options.
pub fn check(format: Format, src: &Path) -> Result<PreflightReport, ConvertError> {
    preflight(format, src, &ConvertOptions::default())
}

/// Status for a supported kind taking `--only` into account.
fn support_for_only(kind: &str, opts: &ConvertOptions) -> KindSupport {
    if opts.includes(kind) {
        KindSupport::Supported
    } else {
        KindSupport::SkippedByOnly
    }
}

/// Probe a kind: the connector read of EVERY descriptor of the kind must succeed for
/// `Supported`; the FIRST failing descriptor yields `ReadError` (carrying the object name so
/// the report is precise). `--only` exclusion overrides to `SkippedByOnly`.
///
/// Probing ALL descriptors (not just the first) is what makes `check` HONEST: a half-done
/// connector that reads object #1 but errors on object #57 must be reported BLOCKED, not
/// "supported". `convert` already reads all descriptors, so this aligns `check` with what a
/// real conversion would hit (M1/M2 review nit). It is the same per-file work `convert`
/// does; for the largest SSL kinds (~hundreds of objects) it is well within budget.
fn probe_kind(
    fk: &formats_xml::registry::FormatKind,
    objs: &[(String, std::path::PathBuf)],
    opts: &ConvertOptions,
) -> KindSupport {
    if !opts.includes(fk.kind) {
        return KindSupport::SkippedByOnly;
    }
    for (name, path) in objs {
        match std::fs::read(path) {
            Ok(bytes) => {
                if let Err(e) = (fk.read)(&bytes) {
                    return KindSupport::ReadError(format!("{name}: {e}"));
                }
            }
            Err(e) => return KindSupport::ReadError(format!("io {}: {e}", path.display())),
        }
    }
    KindSupport::Supported
}

/// Count descriptor-bearing entries under a no-connector subdir (best-effort, for the
/// report only): a directory entry per immediate child (file or dir). Purely informational.
fn count_descriptors(dir: &Path) -> usize {
    match std::fs::read_dir(dir) {
        Ok(rd) => rd.flatten().count(),
        Err(_) => 0,
    }
}

/// Join a base dir with a single subdir segment.
fn p_join(base: &Path, name: &str) -> std::path::PathBuf {
    base.join(name)
}
