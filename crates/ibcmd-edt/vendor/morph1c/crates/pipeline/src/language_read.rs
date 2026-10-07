//! Attach the Designer language SIDECAR (`Languages/<Name>.xml`) into the root's IR
//! `F_LANGUAGES` during whole-config read (§1.0/§1.6). Mirror of [`crate::module_read`] for
//! CommonModule bodies — same "descriptor read is metadata-only, the sidecar is a sibling
//! file the pipeline attaches" pattern, applied to the Language inline-entity.
//!
//! # Why: designer→cf needs F_LANGUAGES the same way edt→cf gets it
//! The cf Language writer ([`formats_cf::language`]) and the assembler both source Language
//! descriptors from the ROOT object's inline `F_LANGUAGES` list. EDT carries `<languages>`
//! INLINE in `Configuration.mdo` → its Configuration read populates F_LANGUAGES directly, so
//! edt→cf already emits the standalone cf Language element. **Designer** stores each language
//! as a sidecar `Languages/<Name>.xml` and the root `Configuration.xml` only lists a bare
//! `<Language>Name</Language>` reference in `<ChildObjects>` — so the Designer Configuration
//! read leaves F_LANGUAGES empty (`cfg::F_LANGUAGES → None` in its projection). This pass reads
//! those sidecars and fills F_LANGUAGES with the SAME shape the EDT reader produces
//! (`List([uuid, name, langCode, synonym])`), so the untouched cf writer path emits the
//! Language element for designer→cf identically to edt→cf.
//!
//! # §1.0 — declared language MUST have a sidecar
//! The set of languages is authoritative from the root's `ChildObjects` (kind `Language`).
//! Each declared language's sidecar `Languages/<Name>.xml` is REQUIRED: a missing/unreadable
//! sidecar for a declared language is a HARD ERROR (never a silent skip). A `Languages/` file
//! that is NOT referenced by ChildObjects is left alone (membership is driven by the root, not
//! the directory) — but any *declared* language is fully parsed into typed IR.

use std::path::{Path, PathBuf};

use formats_xml::registry::Format;
use morph1c_core::ir::value::PropertyValue;
use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::configuration as cfg;

use crate::ConvertError;

/// The `Languages/` subdir (same segment for Designer trees) holding per-language sidecars.
const LANGUAGES_DIR: &str = "Languages";

/// Fill the root Configuration object's `F_LANGUAGES` from the Designer language sidecars.
///
/// * `format` — only [`Format::Designer`] attaches (EDT reads `<languages>` inline already;
///   cf is a container, no sidecar). Other formats → no-op.
/// * `src` — the format ROOT dir (holds `Languages/` and `Configuration.xml`).
/// * `root` — the root object just read (kind `Configuration`); its `F_CHILD_OBJECTS` lists
///   the declared languages by name.
///
/// On success the root gains an `F_LANGUAGES` property (`List` of per-language rows, EDT-
/// shaped). §1.0: a declared language whose sidecar is missing/malformed → [`ConvertError`].
pub fn attach_languages(
    format: Format,
    src: &Path,
    root: &mut MetadataObject,
) -> Result<(), ConvertError> {
    if format != Format::Designer {
        return Ok(()); // EDT: inline `<languages>`; cf: container — no sidecar attach.
    }
    if root.kind.as_str() != "Configuration" {
        return Ok(());
    }

    let names = declared_language_names(root);

    // §1.0: the `Languages/` directory MUST NOT carry sidecars the root does not declare —
    // membership is authoritative from ChildObjects. An undeclared `Languages/<X>.xml` would
    // otherwise be lost silently (dropped from the config). Cross-check both ways: every disk
    // sidecar is declared (here) and every declared language has a sidecar (below).
    let on_disk = language_sidecars_on_disk(src);
    for stem in &on_disk {
        if !names.iter().any(|n| n == stem) {
            return Err(ConvertError::Read {
                kind: "Language".to_string(),
                object: stem.clone(),
                reason: format!(
                    "Languages/{stem}.xml is present on disk but NOT declared in the root's \
                     ChildObjects (§1.0 — would be silently dropped)"
                ),
            });
        }
    }

    if names.is_empty() {
        return Ok(()); // No Language in ChildObjects → nothing to attach (config without languages).
    }

    let mut rows: Vec<PropertyValue> = Vec::with_capacity(names.len());
    for name in &names {
        let path = language_sidecar_path(src, name);
        if !path.is_file() {
            return Err(ConvertError::Read {
                kind: "Language".to_string(),
                object: name.clone(),
                reason: format!(
                    "declared Language {name:?} has no sidecar at {} (§1.0 — a declared language \
                     MUST have its Languages/<Name>.xml)",
                    path.display()
                ),
            });
        }
        let bytes = std::fs::read(&path).map_err(|e| ConvertError::Io {
            path: path.display().to_string(),
            reason: e.to_string(),
        })?;
        let (parsed_name, row) = formats_designer::language::read_language_sidecar(&bytes)
            .map_err(|e| ConvertError::Read {
                kind: "Language".to_string(),
                object: name.clone(),
                reason: e.to_string(),
            })?;
        // §1.0: the sidecar's own <Name> must match the ChildObjects reference (the file stem).
        if &parsed_name != name {
            return Err(ConvertError::Read {
                kind: "Language".to_string(),
                object: name.clone(),
                reason: format!(
                    "Language sidecar <Name> {parsed_name:?} disagrees with ChildObjects ref {name:?}"
                ),
            });
        }
        rows.push(row);
    }

    // The Designer Configuration read leaves F_LANGUAGES unset (projection → None), so there is
    // nothing to replace. Guard anyway (§1.0 — never a silent double-write).
    if root.get(cfg::F_LANGUAGES).is_some() {
        return Err(ConvertError::Read {
            kind: "Language".to_string(),
            object: root.name.clone(),
            reason: "root already carries F_LANGUAGES before the Designer sidecar attach \
                     (unexpected — the Designer projection must not populate it)"
                .into(),
        });
    }
    root.properties
        .push((cfg::F_LANGUAGES, PropertyValue::List(rows)));
    Ok(())
}

/// Names of languages declared in the root's `ChildObjects` (kind == `Language`), in list
/// order. Empty if the config has no languages.
fn declared_language_names(root: &MetadataObject) -> Vec<String> {
    let child_objects = match root.get(cfg::F_CHILD_OBJECTS) {
        Some(PropertyValue::List(rows)) => rows,
        _ => return Vec::new(),
    };
    let mut out = Vec::new();
    for row in child_objects {
        if let PropertyValue::List(cells) = row {
            if cells.len() == 2 {
                if let (PropertyValue::Str(kind), PropertyValue::Str(name)) = (&cells[0], &cells[1])
                {
                    if kind == "Language" {
                        out.push(name.clone());
                    }
                }
            }
        }
    }
    out
}

/// Path of a language sidecar under the Designer format root: `src/Languages/<Name>.xml`.
fn language_sidecar_path(src: &Path, name: &str) -> PathBuf {
    src.join(LANGUAGES_DIR).join(format!("{name}.xml"))
}

/// File stems of every `Languages/*.xml` sidecar on disk (unsorted). Empty if the directory
/// is absent. Used for the §1.0 "no undeclared sidecar" cross-check.
fn language_sidecars_on_disk(src: &Path) -> Vec<String> {
    let dir = src.join(LANGUAGES_DIR);
    let rd = match std::fs::read_dir(&dir) {
        Ok(rd) => rd,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for entry in rd.flatten() {
        let p = entry.path();
        if p.extension().and_then(|e| e.to_str()) != Some("xml") {
            continue;
        }
        if let Some(stem) = p.file_stem().and_then(|n| n.to_str()) {
            out.push(stem.to_string());
        }
    }
    out
}

#[cfg(any())]
mod tests {
    use super::*;
    use morph1c_core::ir::{ObjectKind, Uuid};

    fn root_with_child_objects(rows: Vec<(&str, &str)>) -> MetadataObject {
        let mut root = MetadataObject::new(ObjectKind::new("Configuration"), "C", Uuid([0; 16]));
        root.properties.push((
            cfg::F_CHILD_OBJECTS,
            PropertyValue::List(
                rows.into_iter()
                    .map(|(k, n)| {
                        PropertyValue::List(vec![
                            PropertyValue::Str(k.to_string()),
                            PropertyValue::Str(n.to_string()),
                        ])
                    })
                    .collect(),
            ),
        ));
        root
    }

    #[test]
    fn declared_names_picks_only_languages() {
        let root = root_with_child_objects(vec![
            ("Language", "Русский"),
            ("CommonModule", "ТестМодуль"),
            ("Language", "English"),
        ]);
        assert_eq!(declared_language_names(&root), vec!["Русский", "English"]);
    }

    #[test]
    fn non_designer_is_noop() {
        let mut root = root_with_child_objects(vec![("Language", "Русский")]);
        attach_languages(Format::Edt, Path::new("/nope"), &mut root).unwrap();
        assert!(
            root.get(cfg::F_LANGUAGES).is_none(),
            "EDT must not attach (inline path)"
        );
    }

    #[test]
    fn declared_language_without_sidecar_errors() {
        let mut root = root_with_child_objects(vec![("Language", "Русский")]);
        let err = attach_languages(Format::Designer, Path::new("/nonexistent-root"), &mut root)
            .expect_err("declared language with no sidecar must error (§1.0)");
        match err {
            ConvertError::Read { kind, object, .. } => {
                assert_eq!(kind, "Language");
                assert_eq!(object, "Русский");
            }
            other => panic!("expected ConvertError::Read, got {other:?}"),
        }
    }

    #[test]
    fn no_languages_is_noop() {
        let mut root = root_with_child_objects(vec![("CommonModule", "M")]);
        attach_languages(Format::Designer, Path::new("/nope"), &mut root).unwrap();
        assert!(root.get(cfg::F_LANGUAGES).is_none());
    }

    #[test]
    fn sidecar_path_shape() {
        let p = language_sidecar_path(Path::new("/root"), "Русский");
        assert!(p.ends_with(Path::new("Languages/Русский.xml")), "got {p:?}");
    }
}
