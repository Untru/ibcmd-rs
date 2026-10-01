//! Write the Designer language SIDECARS (`Languages/<Name>.xml`) from the root's IR `F_LANGUAGES`
//! during whole-config write (§1.0/§1.6). Write-side mirror of [`crate::language_read`]: the root
//! `Configuration.xml` writer emits only the bare `<Language>Name</Language>` reference in
//! `<ChildObjects>` (+ `<DefaultLanguage>`), NOT the language body — that body lives in a sibling
//! `Languages/<Name>.xml`. Without this pass an edt→designer (or designer→designer) write produces
//! a Designer tree with a DANGLING `<Language>` reference (no body file) → the platform loses the
//! language. This pass emits each declared language's sidecar so the compiled tree is complete.
//!
//! Only [`Format::Designer`] emits sidecars: EDT carries `<languages>` INLINE in
//! `Configuration.mdo` (its descriptor writer emits them), and cf assembles the Language element
//! from `F_LANGUAGES` in `formats_cf::assemble_cf`. So this is Designer-only, symmetric with
//! [`crate::language_read`] which is also Designer-only.

use std::path::Path;

use formats_xml::registry::Format;
use morph1c_core::ir::value::PropertyValue;
use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::configuration as cfg;

use crate::ConvertError;

/// The `Languages/` subdir holding per-language sidecars (same segment as the read side).
const LANGUAGES_DIR: &str = "Languages";

/// Emit each language of the root's `F_LANGUAGES` to `dst/Languages/<Name>.xml` (Designer only).
///
/// * `format` — only [`Format::Designer`] writes sidecars (EDT inline; cf assembles). Others no-op.
/// * `dst` — the Designer format ROOT dir (gets a `Languages/` subdir).
/// * `root` — the root object (kind `Configuration`) carrying `F_LANGUAGES`.
///
/// §1.0: a malformed language row (not `[uuid, name, langCode, synonym]`) → typed
/// [`ConvertError::Write`] (never a silent drop / partial tree).
pub fn write_languages(
    format: Format,
    dst: &Path,
    root: &MetadataObject,
) -> Result<(), ConvertError> {
    if format != Format::Designer {
        return Ok(()); // EDT: inline <languages>; cf: assembled element — no sidecar.
    }
    if root.kind.as_str() != "Configuration" {
        return Ok(());
    }
    let rows = match root.get(cfg::F_LANGUAGES) {
        Some(PropertyValue::List(rows)) => rows,
        _ => return Ok(()), // config without languages — nothing to emit.
    };

    for row in rows {
        let name = language_name(row).ok_or_else(|| ConvertError::Write {
            kind: "Language".to_string(),
            object: root.name.clone(),
            reason: format!("F_LANGUAGES row is not [uuid, name, langCode, synonym]: {row:?}"),
        })?;
        let bytes = formats_designer::language::write_language_sidecar(row).map_err(|e| {
            ConvertError::Write {
                kind: "Language".to_string(),
                object: name.clone(),
                reason: e.to_string(),
            }
        })?;
        let dir = dst.join(LANGUAGES_DIR);
        crate::fsio::create_dir_all(&dir).map_err(|e| ConvertError::Io {
            path: dir.display().to_string(),
            reason: e.to_string(),
        })?;
        let path = dir.join(format!("{name}.xml"));
        crate::fsio::write(&path, &bytes).map_err(|e| ConvertError::Io {
            path: path.display().to_string(),
            reason: e.to_string(),
        })?;
    }
    Ok(())
}

/// The language NAME (`row[1]`) of an `F_LANGUAGES` row, if the row is well-shaped.
fn language_name(row: &PropertyValue) -> Option<String> {
    match row {
        PropertyValue::List(cells) if cells.len() == 4 => match &cells[1] {
            PropertyValue::Str(s) => Some(s.clone()),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(any())]
mod tests {
    use super::*;
    use morph1c_core::ir::{ObjectKind, Uuid};

    fn root_with_language() -> MetadataObject {
        let mut root = MetadataObject::new(ObjectKind::new("Configuration"), "C", Uuid([0; 16]));
        root.properties.push((
            cfg::F_LANGUAGES,
            PropertyValue::List(vec![PropertyValue::List(vec![
                PropertyValue::Str("b68c5232-ee5f-4a01-86f5-df27e276f205".into()),
                PropertyValue::Str("Русский".into()),
                PropertyValue::Str("ru".into()),
                PropertyValue::List(vec![
                    PropertyValue::Str("ru".into()),
                    PropertyValue::Str("Русский".into()),
                ]),
            ])]),
        ));
        root
    }

    #[test]
    fn edt_is_noop() {
        let root = root_with_language();
        let base =
            std::env::temp_dir().join(format!("morph1c-lang-write-edt-{}", std::process::id()));
        write_languages(Format::Edt, &base, &root).unwrap();
        assert!(
            !base.join("Languages").exists(),
            "EDT must not write a Languages sidecar"
        );
    }

    #[test]
    fn designer_writes_loadable_sidecar() {
        let root = root_with_language();
        let base = std::env::temp_dir().join(format!(
            "morph1c-lang-write-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        write_languages(Format::Designer, &base, &root).unwrap();
        let path = base.join("Languages").join("Русский.xml");
        assert!(path.is_file(), "sidecar written at {}", path.display());
        let bytes = std::fs::read(&path).unwrap();
        // Re-read via the designer reader → the same row (offline round-trip proof).
        let (name, _row) = formats_designer::language::read_language_sidecar(&bytes).unwrap();
        assert_eq!(name, "Русский");
        let _ = std::fs::remove_dir_all(&base);
    }
}
