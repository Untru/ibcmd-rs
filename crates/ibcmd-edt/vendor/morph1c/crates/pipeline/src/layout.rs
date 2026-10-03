//! On-disk path derivation for whole-config traversal — DRY from the registry.
//!
//! The per-kind subdir name (e.g. `"CommonModules"`) is the SAME for EDT and Designer and
//! equals the LAST path segment of that kind's `corpus_subpath` for `DirPerObject` /
//! `FilePerObject` kinds. We do NOT hand-maintain a kind→dir table — the registry already
//! carries it. The root (SingletonFile) is special-cased.
//!
//! The pipeline's `src`/`dst` is the FORMAT ROOT directory (the dir holding all per-kind
//! subdirs plus the root descriptor). To map a registry `corpus_subpath` (which is rooted
//! at `.fixtures`) to a relative path UNDER the format root, we strip the common path
//! prefix shared by the format's `corpus_subpath`s IN THE SAME CORPUS FAMILY:
//!   * EDT  SSL rows: `SSL/edt/src/{CommonModules, …, Configuration}` → root = `SSL/edt/src`
//!   * EDT  ERP rows: `ERP/edt/cfg/src/{DocumentNumerators, …}`       → root = `ERP/edt/cfg/src`
//!   * Designer SSL : `SSL/designer_8.5.1/cf/{CommonModules, …}` + root `SSL/designer_8.5.1/cf`
//!     → root = `SSL/designer_8.5.1/cf`
//!   * Designer ERP : `ERP/designer_8.3.27/{DocumentNumerators, …}` → root = `ERP/designer_8.3.27`
//!
//! MIXED corpora (§ ERP + SSL in one registry): a SINGLE format-wide LCP over ALL rows
//! collapses to empty (`SSL/…` vs `ERP/…` share no prefix). So the format root is computed
//! PER CORPUS FAMILY — the LCP over rows whose `corpus_subpath` shares the given kind's
//! TOP-LEVEL corpus segment (`SSL` / `ERP`). Each kind's on-disk path under its format root
//! is then `corpus_subpath` with that family root removed (root → `""` for Designer,
//! `"Configuration"` for EDT). Holds for both formats and both corpora without a table.

use std::path::{Path, PathBuf};

use formats_xml::registry::{CorpusLayout, FormatKind};
use morph1c_core::ir::{MetadataObject, PropertyValue};
use morph1c_core::spec::metadata::subsystem::F_PARENT_SUBSYSTEM;

use crate::registry::FormatRegistry;
use crate::ConvertError;

/// Split a forward-slash `corpus_subpath` into segments (registry paths use `/`).
fn segments(subpath: &str) -> Vec<&str> {
    subpath.split('/').filter(|s| !s.is_empty()).collect()
}

/// Top-level corpus-family segment of a `corpus_subpath` (`"SSL"` / `"ERP"` / …) — the
/// key that partitions a mixed registry into per-corpus format roots. Empty subpath → `""`.
fn corpus_family(subpath: &str) -> &str {
    segments(subpath).first().copied().unwrap_or("")
}

/// Longest common leading-segment prefix shared by the format's `corpus_subpath`s IN A
/// GIVEN CORPUS FAMILY — i.e. the `.fixtures`-relative path of that family's FORMAT ROOT.
/// `family` is the top-level segment (`"SSL"` / `"ERP"`). Rows of OTHER families are
/// ignored (a single format-wide LCP would collapse to empty across `SSL/…` vs `ERP/…`).
fn format_root_subpath_for_family(reg: &FormatRegistry, family: &str) -> Vec<String> {
    let mut common: Option<Vec<String>> = None;
    for fk in reg.iter() {
        if corpus_family(fk.corpus_subpath) != family {
            continue;
        }
        let segs: Vec<String> = segments(fk.corpus_subpath)
            .into_iter()
            .map(str::to_string)
            .collect();
        common = Some(match common {
            None => segs,
            Some(mut c) => {
                let keep = c
                    .iter()
                    .zip(segs.iter())
                    .take_while(|(a, b)| a.as_str() == **b)
                    .count();
                c.truncate(keep);
                c
            }
        });
    }
    common.unwrap_or_default()
}

/// The `.fixtures`-relative FORMAT ROOT for the registry's PRIMARY corpus family. The
/// primary family is the one with the MOST kinds — SSL dominates a mixed registry (dozens
/// of SSL kinds vs a handful of ERP-only ones), so this is robust no matter which kind
/// sorts first alphabetically (an ERP kind like AccumulationRegister must NOT hijack the
/// root). Ties broken deterministically by family name (lexicographic) for reproducibility.
/// In a single-corpus registry this is the whole-registry root; in a mixed registry it is
/// the primary (SSL) corpus root. Per-kind roots (mixed-aware) come from [`kind_rel_dir`].
pub fn format_root_subpath(reg: &FormatRegistry) -> Vec<String> {
    // Tally kinds per corpus family; pick the family with the most kinds (deterministic
    // tie-break on family name). Majority — NOT the first alphabetical kind's family.
    let mut counts: Vec<(String, usize)> = Vec::new();
    for fk in reg.iter() {
        let fam = corpus_family(fk.corpus_subpath).to_string();
        match counts.iter_mut().find(|(f, _)| *f == fam) {
            Some((_, n)) => *n += 1,
            None => counts.push((fam, 1)),
        }
    }
    // Pick the family with the most kinds; on an exact tie, the lexicographically SMALLEST
    // family name wins (deterministic). `max_by` returns the "greatest" per the comparator,
    // so on a count tie we invert the name order (`fb.cmp(fa)`) → smaller name ranks greater.
    match counts
        .into_iter()
        .max_by(|(fa, ca), (fb, cb)| ca.cmp(cb).then_with(|| fb.cmp(fa)))
    {
        Some((family, _)) => format_root_subpath_for_family(reg, &family),
        None => Vec::new(),
    }
}

/// The path of a kind RELATIVE to ITS OWN corpus-family format root (the `corpus_subpath`
/// with that family root prefix stripped). For a per-kind dir this is the single subdir
/// segment (`"CommonModules"` / `"DocumentNumerators"`); for the EDT root it is
/// `"Configuration"`; for the Designer root it is empty (the root descriptor sits directly
/// under the format root).
///
/// STRUCTURAL for the object layouts (`DirPerObject`/`FilePerObject`/`Nested`): the kind
/// ALWAYS lives in its own last-segment subdir under the format root (Subsystem: the
/// `Subsystems` root that then nests recursively), so the rel dir is exactly that last
/// segment — no cross-row LCP needed. This is robust when a corpus family has a SINGLE kind
/// (e.g. ERP's DocumentNumerator): a family-wide LCP would swallow the kind's own subdir (LCP
/// of one path = the whole path) and wrongly yield an EMPTY rel dir.
///
/// For the SINGLETON root (and any non-object layout) the rel dir is `corpus_subpath` minus
/// the SINGLE CONFIG's format root ([`root_format_root_subpath`]) — designer root sits
/// directly under the root (rel empty), EDT root sits in a `Configuration` subdir (rel one
/// segment). That format root is the longest prefix the root shares with an OBJECT-kind row
/// in the same corpus family (the root and its object siblings live under the same config
/// dir), which — unlike a family-wide LCP — does NOT collapse to `coverage/<fmt>` in a
/// multi-stage registry (a same-stage object row pins the full `…/<stage>/src` prefix).
pub fn kind_rel_dir(reg: &FormatRegistry, fk: &FormatKind) -> Vec<String> {
    let segs: Vec<String> = segments(fk.corpus_subpath)
        .into_iter()
        .map(str::to_string)
        .collect();
    if is_object_layout(fk.layout) {
        // Object-per-descriptor kinds (incl. the sidecar layouts, whose descriptor still lives
        // in its own last-segment subdir): the LAST segment is the kind's own subdir under the
        // format root (structural, single-row-safe). `Nested` (Subsystem) is such a kind — its
        // `Subsystems` subdir is the recursion ROOT, so it resolves the same way.
        segs.into_iter().last().into_iter().collect()
    } else {
        // Root / other non-object layouts: strip the SINGLE config's format root (the prefix the
        // root shares with an object-kind sibling), NOT the family-wide LCP (which a multi-stage
        // registry collapses to `coverage/<fmt>`, leaving a wrong stage-qualified rel dir).
        let root = root_format_root_subpath(reg, fk);
        segs.into_iter().skip(root.len()).collect()
    }
}

/// Is `layout` a per-OBJECT (per-descriptor) layout — one whose corpus lives in its own subdir
/// directly under the config format root? These rows share the config's format-root dir with
/// the SingletonFile root, so the root's on-disk subdir = its `corpus_subpath` minus the
/// longest prefix it shares with such a row. Excludes the SingletonFile root itself and the
/// non-directory `Container`/`FormBody` layouts (which are not directory siblings of the root).
fn is_object_layout(layout: CorpusLayout) -> bool {
    matches!(
        layout,
        CorpusLayout::DirPerObject { .. }
            | CorpusLayout::FilePerObject { .. }
            | CorpusLayout::Nested { .. }
            | CorpusLayout::BinarySidecar { .. }
            | CorpusLayout::TextSidecar { .. }
    )
}

/// The `.fixtures`-relative FORMAT ROOT of the SINGLE config that `fk`'s (SingletonFile) root
/// belongs to: the LONGEST leading-segment prefix `fk`'s `corpus_subpath` shares with any
/// OBJECT-kind row ([`is_object_layout`]) in the SAME corpus family. The root descriptor and
/// its object siblings always sit under the same config format-root dir (edt
/// `…/s1_core/src/{Configuration, Catalogs, …}`; designer `…/s1_core/{Configuration.xml,
/// Catalogs, …}`), so this shared prefix IS that format root — WITHOUT collapsing to the
/// family-wide `coverage/<fmt>` LCP that a multi-stage registry would otherwise force: the
/// SAME-stage object row pins the full `…/<stage>/src` prefix, while other-stage rows share
/// only the shorter `coverage/<fmt>` and lose under the longest-match rule. Stripping this
/// from the root's `corpus_subpath` yields its on-disk subdir — `["Configuration"]` (edt) /
/// `[]` (designer).
fn root_format_root_subpath(reg: &FormatRegistry, fk: &FormatKind) -> Vec<String> {
    let segs = segments(fk.corpus_subpath);
    let family = corpus_family(fk.corpus_subpath);
    let mut best = 0usize;
    for other in reg.iter() {
        if !is_object_layout(other.layout) || corpus_family(other.corpus_subpath) != family {
            continue;
        }
        let osegs = segments(other.corpus_subpath);
        let keep = segs
            .iter()
            .zip(osegs.iter())
            .take_while(|(a, b)| a == b)
            .count();
        best = best.max(keep);
    }
    segs.into_iter().take(best).map(str::to_string).collect()
}

/// Join a base dir with forward-slash-relative segments, cross-platform.
fn join_segments(base: &Path, segs: &[String]) -> PathBuf {
    let mut p = base.to_path_buf();
    for s in segs {
        p = p.join(s);
    }
    p
}

/// Absolute on-disk directory of a kind under the format root `src` (the dir that holds
/// this kind's object descriptors, or — for SingletonFile — the dir holding the root file).
pub fn kind_dir(reg: &FormatRegistry, fk: &FormatKind, src: &Path) -> PathBuf {
    join_segments(src, &kind_rel_dir(reg, fk))
}

/// Enumerate object descriptors of a kind under the format root `src`: `(name, path)`,
/// sorted by name. Empty if the kind's dir is absent (caller decides). Mirrors
/// `testkit::collect_objects` but re-rooted at the FORMAT ROOT (not `.fixtures`), so the
/// pipeline does not depend on testkit at library level.
pub fn collect_objects(
    reg: &FormatRegistry,
    fk: &FormatKind,
    src: &Path,
) -> Vec<(String, PathBuf)> {
    let dir = kind_dir(reg, fk, src);
    enumerate(&dir, fk.layout)
}

/// Layout-driven file enumeration under `dir` (re-implemented from testkit to avoid a
/// lib-level dev-dep coupling; same semantics for the layouts the pipeline handles).
fn enumerate(dir: &Path, layout: CorpusLayout) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    match layout {
        CorpusLayout::DirPerObject { ext } => {
            let entries = match std::fs::read_dir(dir) {
                Ok(e) => e,
                Err(_) => return out,
            };
            for entry in entries.flatten() {
                let p = entry.path();
                if !p.is_dir() {
                    continue;
                }
                let name = match p.file_name().and_then(|n| n.to_str()) {
                    Some(n) => n.to_string(),
                    None => continue,
                };
                let file = p.join(format!("{name}.{ext}"));
                if file.is_file() {
                    out.push((name, file));
                }
            }
        }
        CorpusLayout::FilePerObject { ext } => {
            let entries = match std::fs::read_dir(dir) {
                Ok(e) => e,
                Err(_) => return out,
            };
            for entry in entries.flatten() {
                let p = entry.path();
                if p.extension().and_then(|e| e.to_str()) != Some(ext) {
                    continue;
                }
                let name = match p.file_stem().and_then(|n| n.to_str()) {
                    Some(n) => n.to_string(),
                    None => continue,
                };
                out.push((name, p));
            }
        }
        CorpusLayout::SingletonFile { file, name } => {
            let mut path = dir.to_path_buf();
            for seg in file.split('/') {
                path = path.join(seg);
            }
            if path.is_file() {
                out.push((name.to_string(), path));
            }
        }
        // BinarySidecar (picture/Blob) / TextSidecar (rights-table): descriptor enumeration
        // is the underlying DirPerObject (EDT) / FilePerObject (Designer) mode; the sidecar
        // travels with the object (resolved from the descriptor path elsewhere). Delegate so
        // whole-config traversal finds CommonPicture/Role descriptors correctly.
        CorpusLayout::BinarySidecar { .. } | CorpusLayout::TextSidecar { .. } => {
            if let Some(mode) = layout.descriptor_enum_mode() {
                return enumerate(dir, mode);
            }
        }
        // Nested (Subsystem, self-referential nesting): RECURSE into the `nesting_dir`
        // (`Subsystems`) at EVERY level (mirrors `testkit::collect_nested`), so the
        // whole-config traversal enumerates ALL nested subsystems (SSL: 87), not just the 3
        // top-level ones. Keys are hierarchical `parent/child` name paths (unique across the
        // tree, identical between EDT/Designer). This `enumerate` path is NON-strict (its Vec
        // contract feeds preflight/counting); the §1.0 STRICT gate — a descriptor present on
        // disk but unreachable becomes a typed error, not a silent skip — is
        // [`collect_objects_checked`], used by the whole-config read.
        CorpusLayout::Nested {
            ext,
            nesting_dir,
            dir_per_object,
        } => {
            let w = NestedWalk {
                ext,
                nesting_dir,
                dir_per_object,
                kind: "",
                strict: false,
            };
            // Non-strict never errors; the `_` is intentional.
            let _ = collect_nested(w, dir, "", &mut out);
        }
        // cf Container / form FormBody are not whole-config-traversed by this milestone
        // (cf is a later milestone; form bodies are sidecars). Empty — the kind is not a
        // file-per-object descriptor here.
        CorpusLayout::Container { .. } | CorpusLayout::FormBody { .. } => {}
    }
    out.sort();
    out
}

/// Immutable parameters of a nested (Subsystem) self-referential traversal — bundled so the
/// recursive walker stays within clippy's argument budget and the per-level call is
/// `(walk, dir, prefix, out)`.
#[derive(Clone, Copy)]
struct NestedWalk<'a> {
    /// Descriptor extension (`"mdo"` EDT / `"xml"` Designer).
    ext: &'a str,
    /// Self-referential nesting subdir descended at each level (`"Subsystems"`).
    nesting_dir: &'a str,
    /// `true` → EDT dir-per-object (`<Name>/<Name>.ext`); `false` → Designer file-per-object
    /// (`<Name>.ext`). Determines how each level's objects are identified.
    dir_per_object: bool,
    /// Canonical kind code (`"Subsystem"`) — for typed-error context. Unused when `!strict`.
    kind: &'a str,
    /// §1.0 STRICT mode: a directory that hides descriptors under its `nesting_dir` but has
    /// no own descriptor (present on disk yet unreachable) becomes a typed error instead of a
    /// silent skip. Non-strict mode never errors (used by the Vec `enumerate`/count path).
    strict: bool,
}

/// Recursive enumerator for [`CorpusLayout::Nested`] — mirrors `testkit::collect_nested`, but
/// §1.0-honest in STRICT mode. At each level `dir` it identifies objects by the base mode
/// (EDT `<Name>/<Name>.ext` / Designer `<Name>.ext`), pushes `(hierarchical_key, descriptor)`
/// and recurses into `<obj_dir>/<nesting_dir>`. The key is the `/`-joined chain of names
/// (`prefix/<Name>`), unique across the tree and identical between formats (own name may
/// repeat in nested branches). An absent level (`read_dir` Err) simply has no children (not
/// an error — infra-skip-safe). In STRICT mode, a directory that carries nested descriptors
/// but yields no own object raises [`ConvertError::NestedOrphan`] (no silent skip).
fn collect_nested(
    w: NestedWalk,
    dir: &Path,
    prefix: &str,
    out: &mut Vec<(String, PathBuf)>,
) -> Result<(), ConvertError> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(()), // absent level → no nested (infra-skip-safe)
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if w.dir_per_object {
            // EDT: object = subdir `<Name>` holding `<Name>/<Name>.ext`.
            if !p.is_dir() {
                continue;
            }
            let name = match p.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            let file = p.join(format!("{name}.{}", w.ext));
            if file.is_file() {
                let key = join_key(prefix, &name);
                out.push((key.clone(), file));
                collect_nested(w, &p.join(w.nesting_dir), &key, out)?;
            } else if w.strict {
                // Dir with no own descriptor: guard against hiding nested descriptors.
                guard_hidden(w, &p)?;
            }
        } else if p.is_file() && p.extension().and_then(|e| e.to_str()) == Some(w.ext) {
            // Designer: object = file `<Name>.ext`; its nested tree lives in the sibling
            // directory `<Name>/<nesting_dir>`.
            let name = match p.file_stem().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            let key = join_key(prefix, &name);
            out.push((key.clone(), p.clone()));
            collect_nested(w, &dir.join(&name).join(w.nesting_dir), &key, out)?;
        } else if p.is_dir() && w.strict {
            // Designer object dir `<Name>/` is reached via its sibling `<Name>.ext`; if that
            // descriptor is absent, this dir's nested subtree is unreachable — guard it.
            let name = match p.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            if !dir.join(format!("{name}.{}", w.ext)).is_file() {
                guard_hidden(w, &p)?;
            }
        }
    }
    Ok(())
}

/// §1.0 guard: a directory `obj_dir` that has no own descriptor but carries nested subsystem
/// descriptors under its `nesting_dir` would hide those from traversal — a silent skip.
/// Probe the subtree NON-strictly; if any descriptor is reachable there, raise a typed
/// [`ConvertError::NestedOrphan`]. An empty/irrelevant directory (nothing hidden) is not an
/// error.
fn guard_hidden(w: NestedWalk, obj_dir: &Path) -> Result<(), ConvertError> {
    let probe_walk = NestedWalk { strict: false, ..w };
    let mut probe = Vec::new();
    // Non-strict → never errors; the `_` is intentional.
    let _ = collect_nested(probe_walk, &obj_dir.join(w.nesting_dir), "", &mut probe);
    if probe.is_empty() {
        Ok(())
    } else {
        Err(ConvertError::NestedOrphan {
            kind: w.kind.to_string(),
            dir: obj_dir.display().to_string(),
        })
    }
}

/// Join a hierarchical key: `prefix/name`, or just `name` when `prefix` is empty (top level).
fn join_key(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{prefix}/{name}")
    }
}

/// §1.0-honest enumeration for the whole-config read. Identical to [`collect_objects`] for
/// every layout EXCEPT [`CorpusLayout::Nested`] (Subsystem): there it RECURSES into every
/// `nesting_dir` level (SSL: all 87 subsystems, not just the 3 top-level) and turns a
/// descriptor present on disk but unreachable by traversal (a directory with no own descriptor
/// hiding nested ones) into a typed [`ConvertError::NestedOrphan`] instead of a silent skip.
/// Keys for a Nested kind are the hierarchical `parent/child` name paths.
pub fn collect_objects_checked(
    reg: &FormatRegistry,
    fk: &FormatKind,
    src: &Path,
) -> Result<Vec<(String, PathBuf)>, ConvertError> {
    match fk.layout {
        CorpusLayout::Nested {
            ext,
            nesting_dir,
            dir_per_object,
        } => {
            let dir = kind_dir(reg, fk, src);
            let w = NestedWalk {
                ext,
                nesting_dir,
                dir_per_object,
                kind: fk.kind,
                strict: true,
            };
            let mut out = Vec::new();
            collect_nested(w, &dir, "", &mut out)?;
            out.sort();
            Ok(out)
        }
        // Every other layout: unchanged flat/base enumeration (0 regression).
        _ => Ok(collect_objects(reg, fk, src)),
    }
}

/// Path of a kind's root descriptor file (SingletonFile only): `src/<rel>/<file>`.
pub fn singleton_path(reg: &FormatRegistry, fk: &FormatKind, src: &Path) -> Option<PathBuf> {
    if let CorpusLayout::SingletonFile { file, .. } = fk.layout {
        let mut p = kind_dir(reg, fk, src);
        for seg in file.split('/') {
            p = p.join(seg);
        }
        Some(p)
    } else {
        None
    }
}

/// Output descriptor path for one object when WRITING, under the format root `dst`.
///
/// * `DirPerObject` (EDT): `dst/<dir…>/<Name>/<Name>.<ext>`
/// * `FilePerObject` (Designer): `dst/<dir…>/<Name>.<ext>`
/// * `SingletonFile` (root): `dst/<rel…>/<file>` (the object name is fixed by the layout)
pub fn output_path(
    reg: &FormatRegistry,
    fk: &FormatKind,
    dst: &Path,
    object_name: &str,
) -> Option<PathBuf> {
    let dir = kind_dir(reg, fk, dst);
    match fk.layout {
        CorpusLayout::DirPerObject { ext } => {
            Some(dir.join(object_name).join(format!("{object_name}.{ext}")))
        }
        CorpusLayout::FilePerObject { ext } => Some(dir.join(format!("{object_name}.{ext}"))),
        CorpusLayout::SingletonFile { file, .. } => {
            let mut p = dir;
            for seg in file.split('/') {
                p = p.join(seg);
            }
            Some(p)
        }
        // BinarySidecar/TextSidecar: descriptor output path is the underlying DirPerObject/
        // FilePerObject path (sidecar — binary `Picture.*` / text `Rights.rights` — written
        // alongside via a separate step, like Module.bsl).
        CorpusLayout::BinarySidecar {
            descriptor_ext,
            descriptor_dir_per_object: true,
            ..
        }
        | CorpusLayout::TextSidecar {
            descriptor_ext,
            descriptor_dir_per_object: true,
            ..
        } => Some(
            dir.join(object_name)
                .join(format!("{object_name}.{descriptor_ext}")),
        ),
        CorpusLayout::BinarySidecar {
            descriptor_ext,
            descriptor_dir_per_object: false,
            ..
        }
        | CorpusLayout::TextSidecar {
            descriptor_ext,
            descriptor_dir_per_object: false,
            ..
        } => Some(dir.join(format!("{object_name}.{descriptor_ext}"))),
        // Nested (Subsystem): the flat `object_name` is INSUFFICIENT — nested subsystems share
        // own-names across branches (SSL: 3 collisions) and must be placed by their FULL
        // HIERARCHICAL parent chain, not the bare name. The whole-config writer routes Nested
        // kinds through [`nested_output_path`] (which reads the object's parent chain); the
        // name-only `output_path` declines rather than emit a wrong flat top-level path (§1.0 —
        // no silent fallback that would collide/overwrite).
        CorpusLayout::Nested { .. } => None,
        CorpusLayout::Container { .. } | CorpusLayout::FormBody { .. } => None,
    }
}

/// Output descriptor path for a NESTED (Subsystem) object, reconstructing the HIERARCHICAL
/// on-disk layout from the object's parent chain (its EDT `parentSubsystem` back-reference) —
/// NOT its flat own-name. For parent chain `["A", "B"]` and own-name `C` under `dst`:
///   * EDT      → `dst/Subsystems/A/Subsystems/B/Subsystems/C/C.mdo`
///   * Designer → `dst/Subsystems/A/Subsystems/B/Subsystems/C.xml`
///
/// A top-level `C` → `dst/Subsystems/C/C.mdo` / `dst/Subsystems/C.xml`.
///
/// Own-name alone is insufficient: SSL has 3 own-names shared across branches
/// (`БазоваяФункциональность`, `Печать`, `КонтрольРаботыПользователей`); this lands each at its
/// DISTINCT parent path. An unresolvable parent chain (malformed `parentSubsystem`) is a typed
/// [`ConvertError::NestedUnresolved`] (§1.0 — never a silent flat fallback).
pub fn nested_output_path(
    reg: &FormatRegistry,
    fk: &FormatKind,
    dst: &Path,
    obj: &MetadataObject,
) -> Result<PathBuf, ConvertError> {
    let (ext, nesting_dir, dir_per_object) = match fk.layout {
        CorpusLayout::Nested {
            ext,
            nesting_dir,
            dir_per_object,
        } => (ext, nesting_dir, dir_per_object),
        _ => {
            return Err(ConvertError::NestedUnresolved {
                kind: fk.kind.to_string(),
                object: obj.name.clone(),
                reason: "nested_output_path called on a non-Nested layout".to_string(),
            })
        }
    };
    let parents = subsystem_parent_segments(fk.kind, obj)?;
    // kind_dir(dst) is `dst/<nesting_dir>` (e.g. `dst/Subsystems`). Descend one `<parent>/
    // <nesting_dir>` level per ancestor, then place the object's own descriptor by the base
    // mode (EDT `<Own>/<Own>.ext` dir-per-object; Designer `<Own>.ext` file-per-object).
    let mut p = kind_dir(reg, fk, dst);
    for parent in &parents {
        p = p.join(parent).join(nesting_dir);
    }
    let own = obj.name.as_str();
    let p = if dir_per_object {
        p.join(own).join(format!("{own}.{ext}"))
    } else {
        p.join(format!("{own}.{ext}"))
    };
    Ok(p)
}

/// Reconstruct a nested Subsystem's hierarchical PARENT chain from its EDT `parentSubsystem`
/// back-reference (`<parentSubsystem>Subsystem.A.Subsystem.B</parentSubsystem>` → `["A", "B"]`).
/// This is the canonical hierarchical identity carried in the IR for an EDT-sourced config: the
/// FULL ancestor chain (not just the direct parent), so it reconstructs the exact nesting even
/// when own-names collide across branches. A top-level subsystem carries no `parentSubsystem`
/// (default `""`) → empty chain. A present-but-malformed value is a typed error (§1.0).
pub fn subsystem_parent_segments(
    kind: &str,
    obj: &MetadataObject,
) -> Result<Vec<String>, ConvertError> {
    let raw = match obj.get(F_PARENT_SUBSYSTEM) {
        Some(PropertyValue::Str(s)) => s.as_str(),
        // Absent → top-level (no parents). A non-Str value is a type violation, not a silent skip.
        None => "",
        Some(other) => {
            return Err(ConvertError::NestedUnresolved {
                kind: kind.to_string(),
                object: obj.name.clone(),
                reason: format!("parentSubsystem is not a string value: {other:?}"),
            })
        }
    };
    parse_parent_chain(kind, &obj.name, raw)
}

/// Split a `Subsystem.A.Subsystem.B` parent chain into `["A", "B"]`. Empty → `[]`. Tokens must
/// come in `("Subsystem", <name>)` pairs (1C identifiers never contain `.`); a broken structure
/// (odd token count, missing `Subsystem` marker, empty name) is a typed
/// [`ConvertError::NestedUnresolved`].
fn parse_parent_chain(kind: &str, object: &str, raw: &str) -> Result<Vec<String>, ConvertError> {
    if raw.is_empty() {
        return Ok(Vec::new());
    }
    let bad = |reason: String| ConvertError::NestedUnresolved {
        kind: kind.to_string(),
        object: object.to_string(),
        reason,
    };
    let toks: Vec<&str> = raw.split('.').collect();
    if toks.len() % 2 != 0 {
        return Err(bad(format!(
            "parentSubsystem {raw:?} is not a Subsystem.<Name>… chain"
        )));
    }
    let mut segs = Vec::with_capacity(toks.len() / 2);
    let mut i = 0;
    while i < toks.len() {
        if toks[i] != "Subsystem" {
            return Err(bad(format!(
                "parentSubsystem {raw:?}: expected 'Subsystem' marker, found {:?}",
                toks[i]
            )));
        }
        if toks[i + 1].is_empty() {
            return Err(bad(format!(
                "parentSubsystem {raw:?}: empty parent-name segment"
            )));
        }
        segs.push(toks[i + 1].to_string());
        i += 2;
    }
    Ok(segs)
}

#[cfg(any())]
mod tests {
    use super::*;
    use formats_xml::registry::Format;

    #[test]
    fn edt_format_root_and_kind_dirs() {
        let reg = FormatRegistry::for_format(Format::Edt).unwrap();
        // Coverage corpus family root: LCP of all kinds' `coverage/edt/<stage>/src/…` subpaths
        // (stage differs per kind, so the common prefix is `coverage/edt`).
        assert_eq!(format_root_subpath(&reg), vec!["coverage", "edt"]);

        let cm = reg.get("CommonModule").expect("CommonModule connector");
        assert_eq!(kind_rel_dir(&reg, cm), vec!["CommonModules".to_string()]);

        let root = reg.root_kind().expect("EDT has a SingletonFile root");
        assert_eq!(root.kind, "Configuration");
        // The root's rel dir strips the SINGLE config's format root (the prefix shared with a
        // same-stage object row `coverage/edt/s1_core/src/Catalogs` → `coverage/edt/s1_core/src`),
        // NOT the family-wide `coverage/edt` LCP — leaving just the `Configuration` subdir.
        assert_eq!(kind_rel_dir(&reg, root), vec!["Configuration".to_string()]);

        // DirPerObject kind resolves to its own subdir (last segment), independent of the root LCP.
        let dn = reg
            .get("DocumentNumerator")
            .expect("DocumentNumerator connector");
        assert_eq!(
            kind_rel_dir(&reg, dn),
            vec!["DocumentNumerators".to_string()]
        );
    }

    #[test]
    fn designer_format_root_and_kind_dirs() {
        let reg = FormatRegistry::for_format(Format::Designer).unwrap();
        // Coverage corpus family root: LCP of all kinds' `coverage/designer/<stage>/…` subpaths
        // (stage differs per kind, so the common prefix is `coverage/designer`).
        assert_eq!(format_root_subpath(&reg), vec!["coverage", "designer"]);

        let cm = reg.get("CommonModule").expect("CommonModule connector");
        assert_eq!(kind_rel_dir(&reg, cm), vec!["CommonModules".to_string()]);

        let root = reg.root_kind().expect("Designer has a SingletonFile root");
        assert_eq!(root.kind, "Configuration");
        // Designer root descriptor sits directly in the stage dir (`coverage/designer/s1_core/
        // Configuration.xml`), which IS the single config's format root (prefix shared with the
        // object row `coverage/designer/s1_core/Catalogs`), so the root's rel dir is EMPTY.
        assert_eq!(kind_rel_dir(&reg, root), Vec::<String>::new());

        // DirPerObject kind resolves to its own subdir (last segment), independent of the root LCP.
        let dn = reg
            .get("DocumentNumerator")
            .expect("DocumentNumerator connector");
        assert_eq!(
            kind_rel_dir(&reg, dn),
            vec!["DocumentNumerators".to_string()]
        );
    }

    #[test]
    fn output_paths_match_layouts() {
        use std::path::Path;
        let edt = FormatRegistry::for_format(Format::Edt).unwrap();
        let cm = edt.get("CommonModule").unwrap();
        let dst = Path::new("/out");
        let p = output_path(&edt, cm, dst, "Foo").unwrap();
        assert!(
            p.ends_with(Path::new("CommonModules/Foo/Foo.mdo")),
            "got {p:?}"
        );

        let des = FormatRegistry::for_format(Format::Designer).unwrap();
        let cm = des.get("CommonModule").unwrap();
        let p = output_path(&des, cm, dst, "Foo").unwrap();
        assert!(p.ends_with(Path::new("CommonModules/Foo.xml")), "got {p:?}");

        // Designer root → directly under dst.
        let root = des.root_kind().unwrap();
        let p = output_path(&des, root, dst, "Configuration").unwrap();
        assert!(p.ends_with(Path::new("Configuration.xml")), "got {p:?}");

        // Nested (Subsystem) declines the name-only path (own-name is insufficient — see
        // nested_output_path): the whole-config writer must route it through the hierarchical
        // reconstruction, never a flat top-level fallback.
        let sub = edt.get("Subsystem").unwrap();
        assert!(
            output_path(&edt, sub, dst, "Валюты").is_none(),
            "Nested must decline name-only"
        );
    }

    /// Build a Subsystem IR object carrying a `parentSubsystem` back-reference (EDT identity).
    fn subsystem_obj(name: &str, parent_ref: Option<&str>) -> MetadataObject {
        use morph1c_core::ir::{ObjectKind, Uuid};
        let mut o = MetadataObject::new(ObjectKind::new("Subsystem"), name, Uuid([0u8; 16]));
        if let Some(p) = parent_ref {
            o.properties
                .push((F_PARENT_SUBSYSTEM, PropertyValue::Str(p.to_string())));
        }
        o
    }

    #[test]
    fn parent_chain_parses_full_ancestor_reference() {
        // Top-level (no parentSubsystem) → empty chain.
        assert!(subsystem_parent_segments(
            "Subsystem",
            &subsystem_obj("СтандартныеПодсистемы", None)
        )
        .unwrap()
        .is_empty());
        // One ancestor.
        assert_eq!(
            subsystem_parent_segments(
                "Subsystem",
                &subsystem_obj("Валюты", Some("Subsystem.СтандартныеПодсистемы"))
            )
            .unwrap(),
            vec!["СтандартныеПодсистемы".to_string()]
        );
        // FULL chain (two ancestors) — the collision-resolving identity.
        assert_eq!(
            subsystem_parent_segments(
                "Subsystem",
                &subsystem_obj(
                    "Печать",
                    Some("Subsystem.СтандартныеПодсистемы.Subsystem.Мультиязычность")
                )
            )
            .unwrap(),
            vec![
                "СтандартныеПодсистемы".to_string(),
                "Мультиязычность".to_string()
            ]
        );
    }

    #[test]
    fn malformed_parent_chain_is_typed_error() {
        // Missing the `Subsystem.` marker structure → NestedUnresolved, not a silent flat path.
        let err = subsystem_parent_segments(
            "Subsystem",
            &subsystem_obj("X", Some("СтандартныеПодсистемы")),
        )
        .expect_err("malformed parentSubsystem must error");
        assert!(
            matches!(err, ConvertError::NestedUnresolved { .. }),
            "got {err:?}"
        );
    }

    #[test]
    fn nested_output_path_reconstructs_hierarchy_both_formats() {
        use std::path::Path;
        let edt = FormatRegistry::for_format(Format::Edt).unwrap();
        let des = FormatRegistry::for_format(Format::Designer).unwrap();
        let edt_sub = edt.get("Subsystem").unwrap();
        let des_sub = des.get("Subsystem").unwrap();
        let dst = Path::new("/out");

        // Top-level.
        let top = subsystem_obj("СтандартныеПодсистемы", None);
        let p = nested_output_path(&edt, edt_sub, dst, &top).unwrap();
        assert!(
            p.ends_with(Path::new(
                "Subsystems/СтандартныеПодсистемы/СтандартныеПодсистемы.mdo"
            )),
            "got {p:?}"
        );
        let p = nested_output_path(&des, des_sub, dst, &top).unwrap();
        assert!(
            p.ends_with(Path::new("Subsystems/СтандартныеПодсистемы.xml")),
            "got {p:?}"
        );

        // Two-level nested collision instance: Печать under СтандартныеПодсистемы/Мультиязычность.
        let deep = subsystem_obj(
            "Печать",
            Some("Subsystem.СтандартныеПодсистемы.Subsystem.Мультиязычность"),
        );
        let p = nested_output_path(&edt, edt_sub, dst, &deep).unwrap();
        assert!(
            p.ends_with(Path::new(
                "Subsystems/СтандартныеПодсистемы/Subsystems/Мультиязычность/Subsystems/Печать/Печать.mdo"
            )),
            "got {p:?}"
        );
        let p = nested_output_path(&des, des_sub, dst, &deep).unwrap();
        assert!(
            p.ends_with(Path::new(
                "Subsystems/СтандартныеПодсистемы/Subsystems/Мультиязычность/Subsystems/Печать.xml"
            )),
            "got {p:?}"
        );

        // The OTHER Печать (top-level under СтандартныеПодсистемы) lands at a DISTINCT path —
        // proving own-name collisions no longer overwrite.
        let shallow = subsystem_obj("Печать", Some("Subsystem.СтандартныеПодсистемы"));
        let p2 = nested_output_path(&edt, edt_sub, dst, &shallow).unwrap();
        assert!(
            p2.ends_with(Path::new(
                "Subsystems/СтандартныеПодсистемы/Subsystems/Печать/Печать.mdo"
            )),
            "got {p2:?}"
        );
        assert_ne!(
            nested_output_path(&edt, edt_sub, dst, &deep).unwrap(),
            p2,
            "colliding own-name Печать must resolve to distinct parent paths"
        );
    }
}
