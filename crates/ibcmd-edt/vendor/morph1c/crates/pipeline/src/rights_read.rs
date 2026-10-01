//! Attach the Role RIGHTS SIDECAR (`Rights.rights` EDT / `Ext/Rights.xml` Designer) into the
//! object's IR `rights` during whole-config read (§1.0/§1.6). Mirror of [`crate::module_read`]
//! (CommonModule `Module.bsl`) — "the descriptor read is metadata-only, the rights TABLE is a
//! sibling file the pipeline attaches" — with the §1.0 STRICTness of [`crate::language_read`].
//!
//! # Why: edt/designer→cf needs the rights table the way it needs the module body
//! The cf Role writer emits the descriptor `<uuid>` PLUS a separate rights-table BODY element
//! `<uuid>.0` (`formats_cf::assemble_cf` RIGHTS_BODY_KINDS → `metadata::role::write_rights_body`).
//! That body is sourced from `MetadataObject.rights`. The per-kind descriptor connectors read
//! only the thin `.mdo`/`.xml` (synonym/comment); the rights TABLE lives in a sidecar. This pass
//! reads it into `obj.rights` so `--to cf` can emit the `<uuid>.0` rights body (the whole point
//! of a loadable Role `.cf` — a Role descriptor with no rights body is NON-loadable).
//!
//! # Sidecar layout beside the descriptor (RE: SSL/edt + SSL/designer corpus, 107/107)
//! * **EDT** (`Roles/<Name>/<Name>.mdo`, dir-per-object): sidecar SIBLING
//!   `Roles/<Name>/Rights.rights` (`SidecarFormat::EdtRights`).
//! * **Designer** (`Roles/<Name>.xml`, file-per-object): sidecar
//!   `Roles/<Name>/Ext/Rights.xml` (`SidecarFormat::DesignerRights`).
//!
//! # §1.0 — a Role MUST carry a rights sidecar
//! Unlike a module (optional), EVERY Role carries a rights table (witnessed 107/107 in SSL). So a
//! Role descriptor with NO adjacent rights sidecar is a HARD ERROR (never a silent skip) — the
//! same shape as [`crate::language_read`]'s "declared language without sidecar". The sidecar path
//! is DERIVED from the descriptor, so a stray sidecar with no descriptor cannot arise here
//! (membership is driven by the enumerated Role descriptors, not the directory).

use std::path::{Path, PathBuf};

use formats_xml::registry::Format;
use formats_xml::SidecarFormat;
use morph1c_core::ir::MetadataObject;

use crate::ConvertError;

/// Kinds whose object carries a rights-table sidecar (currently only `Role`). Held locally:
/// extend as more rights-bearing kinds gain a witnessed sidecar layout.
const RIGHTS_SIDECAR_KINDS: &[&str] = &["Role"];

/// Load the Role rights table (from its sidecar) into `obj.rights`.
///
/// `descriptor_path` — path of the descriptor just read (`.mdo`/`.xml`). The sidecar is resolved
/// beside it per the format layout. §1.0: a Role descriptor whose sidecar is MISSING/unreadable
/// is a HARD ERROR ([`ConvertError::Read`]) — a Role always carries rights. Non-Role kinds and
/// cf (container, no sidecar) are no-ops.
pub fn attach_rights_body(
    format: Format,
    kind: &str,
    descriptor_path: &Path,
    obj: &mut MetadataObject,
) -> Result<(), ConvertError> {
    if !RIGHTS_SIDECAR_KINDS.contains(&kind) {
        return Ok(());
    }
    let (path, sidecar_fmt) = match rights_sidecar(format, descriptor_path) {
        Some(x) => x,
        None => return Ok(()), // cf: container — no file-per-object sidecar to attach.
    };
    // §1.0: a Role MUST have its rights sidecar (witnessed 107/107 SSL). A missing one is a hard
    // error (never a silent skip) — a Role `.cf` descriptor with no rights body is NON-loadable.
    if !path.is_file() {
        return Err(ConvertError::Read {
            kind: kind.to_string(),
            object: obj.name.clone(),
            reason: format!(
                "declared {kind} {:?} has no rights sidecar at {} (§1.0 — a Role MUST carry its \
                 Rights.rights / Ext/Rights.xml)",
                obj.name,
                path.display()
            ),
        });
    }
    let bytes = std::fs::read(&path).map_err(|e| ConvertError::Io {
        path: path.display().to_string(),
        reason: e.to_string(),
    })?;
    let table = formats_xml::rights::read(&bytes, sidecar_fmt).map_err(|e| ConvertError::Read {
        kind: kind.to_string(),
        object: obj.name.clone(),
        reason: e.to_string(),
    })?;
    // §1.0: never silently overwrite an already-attached table (the descriptor read must not
    // populate `rights` — only this pass does).
    if obj.rights.is_some() {
        return Err(ConvertError::Read {
            kind: kind.to_string(),
            object: obj.name.clone(),
            reason: "object already carries a rights table before the sidecar attach \
                     (unexpected — the descriptor projection must not populate it)"
                .into(),
        });
    }
    obj.rights = Some(table);
    Ok(())
}

/// Write-side mirror of [`attach_rights_body`]: emit the Role rights table `obj` carries beside
/// its just-written descriptor `descriptor_out`, serialised in the target dialect via
/// [`formats_xml::rights::write`] (EDT `Rights.rights` / Designer `Ext/Rights.xml`). Non-Role
/// kinds, cf (container), and objects with no attached rights (e.g. a cf source) are no-ops.
/// §1.0: a write failure is a typed [`ConvertError`], never a silent drop.
pub fn write_rights_body(
    format: Format,
    kind: &str,
    descriptor_out: &Path,
    obj: &MetadataObject,
) -> Result<(), ConvertError> {
    if !RIGHTS_SIDECAR_KINDS.contains(&kind) {
        return Ok(());
    }
    let table = match &obj.rights {
        Some(t) => t,
        None => return Ok(()), // no rights attached (cf source has none) — nothing to emit.
    };
    let (path, sidecar_fmt) = match rights_sidecar(format, descriptor_out) {
        Some(x) => x,
        None => return Ok(()), // cf: container.
    };
    // Версия Designer-конверта (`version="2.20"/"2.21"`) берётся кодеком из амбьентного
    // round-trip таргета (`convert` оборачивает ВСЮ запись в `with_roundtrip_target`);
    // не-witnessed таргет → типизированная ошибка кодека (§1.0), не молчаливый дефолт.
    let bytes =
        formats_xml::rights::write(table, sidecar_fmt).map_err(|e| ConvertError::Write {
            kind: kind.to_string(),
            object: obj.name.clone(),
            reason: e.to_string(),
        })?;
    crate::form_write::write_file(&path, &bytes)
}

/// `(sidecar path, envelope format)` beside the descriptor, per format layout (see module docs).
/// `None` for formats without a file-per-object sidecar (cf is a container).
fn rights_sidecar(format: Format, descriptor_path: &Path) -> Option<(PathBuf, SidecarFormat)> {
    match format {
        // EDT: descriptor `<obj-dir>/<Name>.mdo` → sibling `<obj-dir>/Rights.rights`.
        Format::Edt => {
            let obj_dir = descriptor_path.parent()?;
            Some((obj_dir.join("Rights.rights"), SidecarFormat::EdtRights))
        }
        // Designer: descriptor `<dir>/<Name>.xml` → `<dir>/<Name>/Ext/Rights.xml`.
        Format::Designer => {
            let dir = descriptor_path.parent()?;
            let stem = descriptor_path.file_stem()?;
            Some((
                dir.join(stem).join("Ext").join("Rights.xml"),
                SidecarFormat::DesignerRights,
            ))
        }
        Format::Cf => None,
    }
}

#[cfg(any())]
mod tests {
    use super::*;
    use morph1c_core::ir::{ObjectKind, Uuid};
    use morph1c_core::spec::metadata::role::{Right, RightsObject, RightsTable};

    fn sample_table() -> RightsTable {
        RightsTable {
            set_for_new_objects: false,
            set_for_attributes_by_default: true,
            independent_rights_of_child_objects: false,
            objects: vec![RightsObject {
                name: "Configuration.X".to_string(),
                rights: vec![Right {
                    name: "Read".into(),
                    value: true,
                    restriction: None,
                }],
            }],
            restriction_templates: Vec::new(),
        }
    }

    #[test]
    fn edt_sidecar_path_is_sibling() {
        let p = Path::new("/root/Roles/МояРоль/МояРоль.mdo");
        let (side, fmt) = rights_sidecar(Format::Edt, p).unwrap();
        assert!(
            side.ends_with(Path::new("Roles/МояРоль/Rights.rights")),
            "got {side:?}"
        );
        assert!(matches!(fmt, SidecarFormat::EdtRights));
    }

    #[test]
    fn designer_sidecar_path_is_ext_subdir() {
        let p = Path::new("/root/Roles/МояРоль.xml");
        let (side, fmt) = rights_sidecar(Format::Designer, p).unwrap();
        assert!(
            side.ends_with(Path::new("Roles/МояРоль/Ext/Rights.xml")),
            "got {side:?}"
        );
        assert!(matches!(fmt, SidecarFormat::DesignerRights));
    }

    #[test]
    fn non_role_kind_is_noop() {
        let mut obj = MetadataObject::new(ObjectKind::new("Constant"), "C", Uuid([0; 16]));
        attach_rights_body(
            Format::Edt,
            "Constant",
            Path::new("/nope/C/C.mdo"),
            &mut obj,
        )
        .unwrap();
        assert!(obj.rights.is_none());
    }

    #[test]
    fn role_without_sidecar_errors() {
        // §1.0: a Role descriptor with no adjacent sidecar is a hard error (not a silent skip).
        let mut obj = MetadataObject::new(ObjectKind::new("Role"), "МояРоль", Uuid([0; 16]));
        let err = attach_rights_body(
            Format::Edt,
            "Role",
            Path::new("/nonexistent-root/Roles/МояРоль/МояРоль.mdo"),
            &mut obj,
        )
        .expect_err("Role without a rights sidecar must error (§1.0)");
        match err {
            ConvertError::Read { kind, object, .. } => {
                assert_eq!(kind, "Role");
                assert_eq!(object, "МояРоль");
            }
            other => panic!("expected ConvertError::Read, got {other:?}"),
        }
    }

    #[test]
    fn attaches_edt_sidecar_into_ir() {
        // Self-contained: write a real EDT sidecar (via the rights codec) to a temp EDT layout,
        // attach it, and assert the canonical table lands in `obj.rights`.
        let table = sample_table();
        let bytes = formats_xml::rights::write(&table, SidecarFormat::EdtRights).unwrap();
        let base = std::env::temp_dir().join(format!(
            "morph1c-rights-read-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let obj_dir = base.join("Roles").join("МояРоль");
        std::fs::create_dir_all(&obj_dir).unwrap();
        crate::fsio::write(obj_dir.join("Rights.rights"), &bytes).unwrap();
        let descriptor = obj_dir.join("МояРоль.mdo");

        let mut obj = MetadataObject::new(ObjectKind::new("Role"), "МояРоль", Uuid([1; 16]));
        attach_rights_body(Format::Edt, "Role", &descriptor, &mut obj).unwrap();
        assert_eq!(obj.rights.as_ref().expect("rights attached"), &table);

        let _ = std::fs::remove_dir_all(&base);
    }
}
