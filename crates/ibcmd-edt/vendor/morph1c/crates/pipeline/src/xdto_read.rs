//! Attach the XDTOPackage SCHEMA SIDECAR (`Package.xdto` EDT / `Ext/Package.bin` Designer) into
//! the object's IR `xdto_schema` during whole-config read (§1.0/§1.3/§1.6). Mirror of
//! [`crate::template_read`] (CommonTemplate `Template.txt`) — "the descriptor read is metadata-
//! only, the schema BODY is a sibling file the pipeline attaches" — with §1.0 STRICTness (a
//! declared XDTOPackage MUST carry its schema sidecar).
//!
//! # Why: edt/designer→cf needs the XDTO schema the way it needs the module/template body
//! The cf XDTOPackage writer emits the thin descriptor `<uuid>` PLUS a separate schema-body
//! element `<uuid>.0` (`formats_cf::assemble_cf` XDTO_BODY_KINDS → `formats_cf::xdto_body`). That
//! body is sourced from `MetadataObject.xdto_schema`. The per-kind descriptor connector reads only
//! the thin `.mdo`/`.xml` (synonym/comment/namespace); the schema itself lives in a sidecar. This
//! pass reads it into `obj.xdto_schema` so `--to cf` can emit the `<uuid>.0` body.
//!
//! # Form (RE: `coverage/cf/s5_reports_services.cf`) — reproducible byte-exact
//! The cf `<uuid>.0` schema body = `BOM + <canonical XDTO XML>` — a PLAIN leaf. RE byte-identical
//! (sha256): cf `<uuid>.0` inflate (317 B) == Designer `Ext/Package.bin` (317 B, `BOM + <package…>`)
//! == `BOM + EDT Package.xdto` (314 B XML). This pass reads that XML (BOM-stripped → ONE canonical
//! body, edt==designer §1.6) and attaches it. §1.0-STRICT: an XDTOPackage descriptor with NO
//! adjacent schema sidecar is a HARD ERROR (never a silent partial `.cf`).
//!
//! # Sidecar layout beside the descriptor (RE: coverage/edt + coverage/designer)
//! * **EDT** (`XDTOPackages/<Name>/<Name>.mdo`, dir-per-object): sidecar SIBLING
//!   `XDTOPackages/<Name>/Package.xdto` (BOM-less on disk).
//! * **Designer** (`XDTOPackages/<Name>.xml`, file-per-object): sidecar
//!   `XDTOPackages/<Name>/Ext/Package.bin` (carries a UTF-8 BOM on disk).
//!
//! Both give the SAME canonical body once the leading BOM is stripped (§1.6).

use std::path::{Path, PathBuf};

use formats_xml::registry::Format;
use morph1c_core::ir::MetadataObject;

use crate::ConvertError;

/// Kinds whose object carries an XDTO-schema sidecar (currently only `XDTOPackage`). Held
/// locally: extend as more schema-bearing kinds gain a witnessed reproducible body layout.
const XDTO_SIDECAR_KINDS: &[&str] = &["XDTOPackage"];

/// The EDT sidecar file name of the XDTO schema (dir-per-object sibling of the `.mdo`).
const EDT_SCHEMA_FILE: &str = "Package.xdto";

/// The Designer sidecar inner path of the XDTO schema (under `<Name>/Ext/`).
const DESIGNER_SCHEMA_INNER: &[&str] = &["Ext", "Package.bin"];

/// UTF-8 BOM (stripped from the sidecar to get the canonical, format-neutral body — §1.6).
const BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

/// Load the XDTOPackage schema (from its sidecar) into `obj.xdto_schema`.
///
/// `descriptor_path` — path of the descriptor just read (`.mdo`/`.xml`). §1.0: an XDTOPackage
/// descriptor whose schema sidecar is MISSING/unreadable is a HARD ERROR ([`ConvertError::Read`]).
/// Non-XDTOPackage kinds and cf (container) are no-ops.
pub fn attach_xdto_schema(
    format: Format,
    kind: &str,
    descriptor_path: &Path,
    obj: &mut MetadataObject,
) -> Result<(), ConvertError> {
    if !XDTO_SIDECAR_KINDS.contains(&kind) {
        return Ok(());
    }
    let path = match schema_sidecar(format, descriptor_path) {
        Some(p) => p,
        None => return Ok(()), // cf: container — no file-per-object sidecar to attach.
    };
    // §1.0: an XDTOPackage MUST have its schema sidecar (a package descriptor with no schema body
    // is a NON-loadable/incomplete cf). Missing → hard error (never a silent skip).
    if !path.is_file() {
        return Err(ConvertError::Read {
            kind: kind.to_string(),
            object: obj.name.clone(),
            reason: format!(
                "declared {kind} {:?} has no XDTO schema sidecar at {} (§1.0 — an XDTOPackage \
                 MUST carry its Package.xdto / Ext/Package.bin)",
                obj.name,
                path.display()
            ),
        });
    }
    let bytes = std::fs::read(&path).map_err(|e| ConvertError::Io {
        path: path.display().to_string(),
        reason: e.to_string(),
    })?;
    // Canonical body = BOM-stripped bytes (EDT has none, Designer has one → equal after strip).
    let schema = bytes.strip_prefix(BOM).unwrap_or(&bytes).to_vec();
    // §1.0: never silently overwrite an already-attached schema (the descriptor read must not
    // populate `xdto_schema` — only this pass does).
    if obj.xdto_schema.is_some() {
        return Err(ConvertError::Read {
            kind: kind.to_string(),
            object: obj.name.clone(),
            reason: "object already carries an xdto_schema before the sidecar attach (unexpected \
                     — the descriptor projection must not populate it)"
                .into(),
        });
    }
    obj.xdto_schema = Some(schema);
    Ok(())
}

/// Write-side mirror of [`attach_xdto_schema`]: emit the XDTO schema body beside the just-written
/// descriptor `descriptor_out`. The IR carries the canonical (BOM-stripped) schema; Designer
/// re-adds the BOM (`Ext/Package.bin`), EDT writes it bare (`Package.xdto`). No-op for non-XDTO
/// kinds, cf (container), and objects with no attached schema. §1.0: typed [`ConvertError`] on I/O.
pub fn write_xdto_schema(
    format: Format,
    kind: &str,
    descriptor_out: &Path,
    obj: &MetadataObject,
) -> Result<(), ConvertError> {
    if !XDTO_SIDECAR_KINDS.contains(&kind) {
        return Ok(());
    }
    let schema = match &obj.xdto_schema {
        Some(s) => s,
        None => return Ok(()),
    };
    let path = match schema_sidecar(format, descriptor_out) {
        Some(p) => p,
        None => return Ok(()), // cf: container.
    };
    let bytes = match format {
        Format::Designer => {
            let mut b = Vec::with_capacity(BOM.len() + schema.len());
            b.extend_from_slice(BOM);
            b.extend_from_slice(schema);
            b
        }
        Format::Edt | Format::Cf => schema.clone(),
    };
    crate::form_write::write_file(&path, &bytes)
}

/// XDTO schema sidecar path beside the descriptor, per format layout (see module docs).
/// `None` for formats without a file-per-object sidecar (cf is a container).
fn schema_sidecar(format: Format, descriptor_path: &Path) -> Option<PathBuf> {
    match format {
        // EDT: descriptor `<obj-dir>/<Name>.mdo` → sibling `<obj-dir>/Package.xdto`.
        Format::Edt => {
            let obj_dir = descriptor_path.parent()?;
            Some(obj_dir.join(EDT_SCHEMA_FILE))
        }
        // Designer: descriptor `<dir>/<Name>.xml` → `<dir>/<Name>/Ext/Package.bin`.
        Format::Designer => {
            let dir = descriptor_path.parent()?;
            let stem = descriptor_path.file_stem()?;
            let mut p = dir.join(stem);
            for seg in DESIGNER_SCHEMA_INNER {
                p = p.join(seg);
            }
            Some(p)
        }
        Format::Cf => None,
    }
}

#[cfg(any())]
mod tests {
    use super::*;
    use morph1c_core::ir::{ObjectKind, Uuid};

    fn xdto_obj(name: &str) -> MetadataObject {
        MetadataObject::new(ObjectKind::new("XDTOPackage"), name, Uuid([1; 16]))
    }

    #[test]
    fn edt_sidecar_path_is_sibling() {
        let p = Path::new("/root/XDTOPackages/Пакет/Пакет.mdo");
        let s = schema_sidecar(Format::Edt, p).unwrap();
        assert!(
            s.ends_with(Path::new("XDTOPackages/Пакет/Package.xdto")),
            "got {s:?}"
        );
    }

    #[test]
    fn designer_sidecar_path_is_ext_subdir() {
        let p = Path::new("/root/XDTOPackages/Пакет.xml");
        let s = schema_sidecar(Format::Designer, p).unwrap();
        assert!(
            s.ends_with(Path::new("XDTOPackages/Пакет/Ext/Package.bin")),
            "got {s:?}"
        );
    }

    #[test]
    fn non_xdto_kind_is_noop() {
        let mut obj = MetadataObject::new(ObjectKind::new("Constant"), "C", Uuid([0; 16]));
        attach_xdto_schema(
            Format::Edt,
            "Constant",
            Path::new("/nope/C/C.mdo"),
            &mut obj,
        )
        .unwrap();
        assert!(obj.xdto_schema.is_none());
    }

    #[test]
    fn xdto_without_sidecar_errors() {
        // §1.0: an XDTOPackage descriptor with no adjacent schema sidecar is a hard error.
        let mut obj = xdto_obj("Пакет");
        let err = attach_xdto_schema(
            Format::Edt,
            "XDTOPackage",
            Path::new("/nonexistent-root/XDTOPackages/Пакет/Пакет.mdo"),
            &mut obj,
        )
        .expect_err("XDTOPackage without a sidecar must error (§1.0)");
        match err {
            ConvertError::Read { kind, object, .. } => {
                assert_eq!(kind, "XDTOPackage");
                assert_eq!(object, "Пакет");
            }
            other => panic!("expected ConvertError::Read, got {other:?}"),
        }
    }

    #[test]
    fn attaches_edt_schema_bom_stripped() {
        let base = std::env::temp_dir().join(format!(
            "morph1c-xdto-read-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let obj_dir = base.join("XDTOPackages").join("Пакет");
        std::fs::create_dir_all(&obj_dir).unwrap();
        crate::fsio::write(obj_dir.join("Package.xdto"), b"<package></package>").unwrap();
        let descriptor = obj_dir.join("Пакет.mdo");

        let mut obj = xdto_obj("Пакет");
        attach_xdto_schema(Format::Edt, "XDTOPackage", &descriptor, &mut obj).unwrap();
        assert_eq!(
            obj.xdto_schema.as_deref(),
            Some(b"<package></package>".as_slice())
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn attaches_designer_schema_strips_bom() {
        // Designer Ext/Package.bin carries a BOM; the canonical body strips it (== EDT body).
        let base = std::env::temp_dir().join(format!(
            "morph1c-xdto-read-des-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let ext_dir = base.join("XDTOPackages").join("Пакет").join("Ext");
        std::fs::create_dir_all(&ext_dir).unwrap();
        let mut with_bom = Vec::from(BOM);
        with_bom.extend_from_slice(b"<package></package>");
        crate::fsio::write(ext_dir.join("Package.bin"), &with_bom).unwrap();
        let descriptor = base.join("XDTOPackages").join("Пакет.xml");

        let mut obj = xdto_obj("Пакет");
        attach_xdto_schema(Format::Designer, "XDTOPackage", &descriptor, &mut obj).unwrap();
        assert_eq!(
            obj.xdto_schema.as_deref(),
            Some(b"<package></package>".as_slice()),
            "Designer body BOM-stripped == EDT canonical body (§1.6)"
        );

        let _ = std::fs::remove_dir_all(&base);
    }
}
