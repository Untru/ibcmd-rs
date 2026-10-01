//! Attach the WSReference WSDL SIDECAR set (`WsDefinitions.wsdl` + `<N>.xsd` EDT /
//! `Ext/WSDefinition.xml` + `Ext/<N>.xsd` Designer) into the object's IR `ws_definition`
//! during whole-config read, and re-emit it on write (§1.0/§1.3/§1.6). Mirror of
//! [`crate::xdto_read`] (XDTOPackage `Package.xdto`) — "the descriptor read is metadata-only,
//! the WSDL BODY is a sidecar file set the pipeline attaches" — but the body is OPTIONAL
//! (presence is gated by the FILE, mirroring `template_read`).
//!
//! # Why: edt/designer→cf needs the WSDL set the way it needs the XDTO schema
//! The cf WSReference writer emits the thin descriptor `<uuid>` PLUS (when the source carries
//! a WSDL) a separate body element `<uuid>.0` — a NESTED 32-bit v8 container whose RAW
//! (uncompressed) elements are `0.wsdl` (the WSDL text) and `<N>.xsd` (the imported schemas)
//! (`formats_cf::ws_reference_body`, RE `.fixtures/ERP/cf/erp.cf` 2/2). That body is sourced
//! from `MetadataObject.ws_definition`; this pass reads the sidecars into it.
//!
//! # Form (RE: erp.cf `aaccf07e…​.0` / `b409116f…​.0`) — inner bytes reproducible byte-exact
//! Inner element `0.wsdl` == BOM-stripped `Ext/WSDefinition.xml` == EDT `WsDefinitions.wsdl`
//! (byte-exact, 2/2); inner `<N>.xsd` == the on-disk `<N>.xsd` VERBATIM (BOM-less on disk,
//! byte-identical across dialects, 4/4). The canonical IR therefore carries the BOM-stripped
//! WSDL + verbatim xsds (§1.6: one canon, per-format BOM re-added on write — Designer WSDL
//! carries a BOM, EDT does not; xsds carry none in either dialect).
//!
//! # Sidecar layout beside the descriptor (RE: `.fixtures/ERP` designer_8.3.27 + edt)
//! * **EDT** (`WSReferences/<Name>/<Name>.mdo`, dir-per-object): siblings
//!   `WSReferences/<Name>/WsDefinitions.wsdl` + `WSReferences/<Name>/<N>.xsd`;
//! * **Designer** (`WSReferences/<Name>.xml`, file-per-object):
//!   `WSReferences/<Name>/Ext/WSDefinition.xml` + `WSReferences/<Name>/Ext/<N>.xsd`.
//!
//! The `<N>.xsd` names are NUMERIC (`1.xsd`, `2.xsd`, …) — they ARE the inner cf element
//! names, attached in ascending numeric order (== the witnessed FAT order). §1.0: a non-numeric
//! `*.xsd` sidecar next to a WSDL is not our witnessed shape → typed error (no silent drop,
//! no guessed ordering).

use std::path::{Path, PathBuf};

use formats_xml::registry::Format;
use morph1c_core::ir::{MetadataObject, WsDefinition};

use crate::ConvertError;

/// Kinds whose object carries a WSDL sidecar set (currently only `WSReference`).
const WS_SIDECAR_KINDS: &[&str] = &["WSReference"];

/// The EDT sidecar file name of the WSDL (dir-per-object sibling of the `.mdo`).
const EDT_WSDL_FILE: &str = "WsDefinitions.wsdl";

/// The Designer sidecar file name of the WSDL (under `<Name>/Ext/`).
const DESIGNER_WSDL_FILE: &str = "WSDefinition.xml";

/// UTF-8 BOM (stripped from the WSDL to get the canonical, format-neutral body — §1.6).
const BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

/// `<N>.xsd` numeric stem, or `None` when the file name is not the witnessed numeric shape.
fn xsd_number(file_name: &str) -> Option<u32> {
    file_name.strip_suffix(".xsd")?.parse::<u32>().ok()
}

/// Load the WSReference WSDL sidecar set into `obj.ws_definition`.
///
/// `descriptor_path` — path of the descriptor just read (`.mdo`/`.xml`). The body is OPTIONAL:
/// attach IFF the WSDL sidecar exists (a WSReference without one round-trips descriptor-only —
/// RE erp.cf: `.0` exists ⟺ `Ext/WSDefinition.xml` exists; coverage s5 stub carries neither).
/// Non-WSReference kinds and cf (container) are no-ops. §1.0: a non-numeric `*.xsd` beside the
/// WSDL → typed error (it would be silently dropped otherwise).
pub fn attach_ws_definition(
    format: Format,
    kind: &str,
    descriptor_path: &Path,
    obj: &mut MetadataObject,
) -> Result<(), ConvertError> {
    if !WS_SIDECAR_KINDS.contains(&kind) {
        return Ok(());
    }
    let dir = match sidecar_dir(format, descriptor_path) {
        Some(d) => d,
        None => return Ok(()), // cf: container — no file-per-object sidecars to attach.
    };
    let wsdl_path = dir.join(wsdl_file(format));
    if !wsdl_path.is_file() {
        return Ok(()); // OPTIONAL body — a wsdl-less WSReference is descriptor-only (honest).
    }
    let bytes = std::fs::read(&wsdl_path).map_err(|e| ConvertError::Io {
        path: wsdl_path.display().to_string(),
        reason: e.to_string(),
    })?;
    // Canonical WSDL = BOM-stripped bytes (Designer carries one, EDT none → equal after strip).
    let wsdl = bytes.strip_prefix(BOM).unwrap_or(&bytes).to_vec();

    // Collect the numeric `<N>.xsd` siblings in ascending numeric order (== witnessed cf FAT
    // order). §1.0: a NON-numeric `*.xsd` in the sidecar dir would be silently dropped → error.
    let mut xsds: Vec<(u32, String, Vec<u8>)> = Vec::new();
    let entries = std::fs::read_dir(&dir).map_err(|e| ConvertError::Io {
        path: dir.display().to_string(),
        reason: e.to_string(),
    })?;
    for entry in entries {
        let entry = entry.map_err(|e| ConvertError::Io {
            path: dir.display().to_string(),
            reason: e.to_string(),
        })?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.ends_with(".xsd") {
            continue;
        }
        let n = xsd_number(&name).ok_or_else(|| ConvertError::Read {
            kind: kind.to_string(),
            object: obj.name.clone(),
            reason: format!(
                "WSDL sidecar dir carries a non-numeric xsd {name:?} (witnessed shape is \
                 `<N>.xsd` — §1.0, no guessed ordering)"
            ),
        })?;
        let p = entry.path();
        let bytes = std::fs::read(&p).map_err(|e| ConvertError::Io {
            path: p.display().to_string(),
            reason: e.to_string(),
        })?;
        // xsds are BOM-less on disk in both dialects (witnessed 4/4); strip defensively so a
        // hand-edited BOM does not poison the canon.
        let body = bytes.strip_prefix(BOM).unwrap_or(&bytes).to_vec();
        xsds.push((n, name, body));
    }
    xsds.sort_by_key(|(n, _, _)| *n);

    // §1.0: never silently overwrite an already-attached definition (only this pass populates it).
    if obj.ws_definition.is_some() {
        return Err(ConvertError::Read {
            kind: kind.to_string(),
            object: obj.name.clone(),
            reason: "object already carries a ws_definition before the sidecar attach \
                     (unexpected — the descriptor projection must not populate it)"
                .into(),
        });
    }
    obj.ws_definition = Some(WsDefinition {
        wsdl,
        xsds: xsds.into_iter().map(|(_, name, bytes)| (name, bytes)).collect(),
    });
    Ok(())
}

/// Write-side mirror of [`attach_ws_definition`]: emit the WSDL sidecar set beside the
/// just-written descriptor `descriptor_out`, in the TARGET dialect. The IR carries the canonical
/// (BOM-stripped) WSDL; Designer re-adds the BOM (`Ext/WSDefinition.xml`), EDT writes it bare
/// (`WsDefinitions.wsdl`); xsds are emitted verbatim under their stored `<N>.xsd` names. No-op
/// for other kinds, cf, and objects with no attached definition. §1.0: typed [`ConvertError`]
/// on I/O.
pub fn write_ws_definition(
    format: Format,
    kind: &str,
    descriptor_out: &Path,
    obj: &MetadataObject,
) -> Result<(), ConvertError> {
    if !WS_SIDECAR_KINDS.contains(&kind) {
        return Ok(());
    }
    let def = match &obj.ws_definition {
        Some(d) => d,
        None => return Ok(()),
    };
    let dir = match sidecar_dir(format, descriptor_out) {
        Some(d) => d,
        None => return Ok(()), // cf: container — bodies are assembled, not sidecar files.
    };
    let wsdl_bytes = match format {
        Format::Designer => {
            let mut b = Vec::with_capacity(BOM.len() + def.wsdl.len());
            b.extend_from_slice(BOM);
            b.extend_from_slice(&def.wsdl);
            b
        }
        Format::Edt | Format::Cf => def.wsdl.clone(),
    };
    crate::form_write::write_file(&dir.join(wsdl_file(format)), &wsdl_bytes)?;
    for (name, bytes) in &def.xsds {
        crate::form_write::write_file(&dir.join(name), bytes)?;
    }
    Ok(())
}

/// The WSDL sidecar file name in the target dialect.
fn wsdl_file(format: Format) -> &'static str {
    match format {
        Format::Edt => EDT_WSDL_FILE,
        _ => DESIGNER_WSDL_FILE,
    }
}

/// The sidecar DIRECTORY beside the descriptor, per format layout (module docs). `None` for
/// formats without a file-per-object sidecar (cf is a container).
fn sidecar_dir(format: Format, descriptor_path: &Path) -> Option<PathBuf> {
    match format {
        // EDT: descriptor `<obj-dir>/<Name>.mdo` → sidecars live in `<obj-dir>/`.
        Format::Edt => descriptor_path.parent().map(Path::to_path_buf),
        // Designer: descriptor `<dir>/<Name>.xml` → sidecars live in `<dir>/<Name>/Ext/`.
        Format::Designer => {
            let dir = descriptor_path.parent()?;
            let stem = descriptor_path.file_stem()?;
            Some(dir.join(stem).join("Ext"))
        }
        Format::Cf => None,
    }
}

#[cfg(any())]
mod tests {
    use super::*;
    use morph1c_core::ir::{ObjectKind, Uuid};

    fn ws(name: &str) -> MetadataObject {
        MetadataObject::new(ObjectKind::new("WSReference"), name, Uuid([9; 16]))
    }

    fn temp_base(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "morph1c-wsdef-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ))
    }

    #[test]
    fn non_ws_kind_and_missing_sidecar_are_noops() {
        let mut obj = MetadataObject::new(ObjectKind::new("Catalog"), "Спр", Uuid([1; 16]));
        attach_ws_definition(Format::Edt, "Catalog", Path::new("/nope/С/С.mdo"), &mut obj)
            .unwrap();
        assert!(obj.ws_definition.is_none());
        let mut obj = ws("WSСсылка");
        // WSDL-less WSReference (coverage s5 stub) → descriptor-only, no error.
        attach_ws_definition(
            Format::Edt,
            "WSReference",
            Path::new("/nonexistent-root/WSReferences/WSСсылка/WSСсылка.mdo"),
            &mut obj,
        )
        .unwrap();
        assert!(obj.ws_definition.is_none());
    }

    #[test]
    fn attach_and_write_roundtrip_both_dialects() {
        let base = temp_base("rt");
        // EDT: WsDefinitions.wsdl (no BOM) + 2.xsd + 1.xsd (order on disk irrelevant).
        let edt_dir = base.join("edt/WSReferences/Севр");
        std::fs::create_dir_all(&edt_dir).unwrap();
        let edt_mdo = edt_dir.join("Севр.mdo");
        crate::fsio::write(&edt_mdo, b"<mdo/>").unwrap();
        crate::fsio::write(edt_dir.join(EDT_WSDL_FILE), b"<definitions/>\r\n").unwrap();
        crate::fsio::write(edt_dir.join("2.xsd"), b"<xs:schema n=\"2\"/>").unwrap();
        crate::fsio::write(edt_dir.join("1.xsd"), b"<xs:schema n=\"1\"/>").unwrap();

        let mut obj = ws("Севр");
        attach_ws_definition(Format::Edt, "WSReference", &edt_mdo, &mut obj).unwrap();
        let def = obj.ws_definition.as_ref().expect("attached");
        assert_eq!(def.wsdl, b"<definitions/>\r\n");
        assert_eq!(
            def.xsds.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(),
            ["1.xsd", "2.xsd"],
            "xsds sorted by numeric stem"
        );

        // Designer: Ext/WSDefinition.xml carries a BOM → same canon after strip.
        let dsn_dir = base.join("dsn/WSReferences");
        let dsn_ext = dsn_dir.join("Севр/Ext");
        std::fs::create_dir_all(&dsn_ext).unwrap();
        let dsn_xml = dsn_dir.join("Севр.xml");
        crate::fsio::write(&dsn_xml, b"<xml/>").unwrap();
        let mut with_bom = Vec::from(&BOM[..]);
        with_bom.extend_from_slice(b"<definitions/>\r\n");
        crate::fsio::write(dsn_ext.join(DESIGNER_WSDL_FILE), &with_bom).unwrap();
        crate::fsio::write(dsn_ext.join("1.xsd"), b"<xs:schema n=\"1\"/>").unwrap();
        crate::fsio::write(dsn_ext.join("2.xsd"), b"<xs:schema n=\"2\"/>").unwrap();
        let mut dsn_obj = ws("Севр");
        attach_ws_definition(Format::Designer, "WSReference", &dsn_xml, &mut dsn_obj).unwrap();
        assert_eq!(
            dsn_obj.ws_definition, obj.ws_definition,
            "one canonical IR across dialects (§1.6)"
        );

        // Write back to BOTH dialects byte-exact.
        let out_edt = base.join("out-edt/WSReferences/Севр");
        std::fs::create_dir_all(&out_edt).unwrap();
        let out_edt_mdo = out_edt.join("Севр.mdo");
        crate::fsio::write(&out_edt_mdo, b"<mdo/>").unwrap();
        write_ws_definition(Format::Edt, "WSReference", &out_edt_mdo, &dsn_obj).unwrap();
        assert_eq!(
            std::fs::read(out_edt.join(EDT_WSDL_FILE)).unwrap(),
            b"<definitions/>\r\n".to_vec(),
            "EDT wsdl bare (no BOM)"
        );
        assert_eq!(
            std::fs::read(out_edt.join("1.xsd")).unwrap(),
            b"<xs:schema n=\"1\"/>".to_vec()
        );

        let out_dsn = base.join("out-dsn/WSReferences");
        std::fs::create_dir_all(&out_dsn).unwrap();
        let out_dsn_xml = out_dsn.join("Севр.xml");
        crate::fsio::write(&out_dsn_xml, b"<xml/>").unwrap();
        write_ws_definition(Format::Designer, "WSReference", &out_dsn_xml, &obj).unwrap();
        assert_eq!(
            std::fs::read(out_dsn.join("Севр/Ext").join(DESIGNER_WSDL_FILE)).unwrap(),
            with_bom,
            "Designer wsdl = BOM + canon"
        );
        assert_eq!(
            std::fs::read(out_dsn.join("Севр/Ext/2.xsd")).unwrap(),
            b"<xs:schema n=\"2\"/>".to_vec()
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn non_numeric_xsd_is_loud_error() {
        let base = temp_base("badxsd");
        let edt_dir = base.join("WSReferences/Севр");
        std::fs::create_dir_all(&edt_dir).unwrap();
        let mdo = edt_dir.join("Севр.mdo");
        crate::fsio::write(&mdo, b"<mdo/>").unwrap();
        crate::fsio::write(edt_dir.join(EDT_WSDL_FILE), b"<definitions/>").unwrap();
        crate::fsio::write(edt_dir.join("extra.xsd"), b"<xs:schema/>").unwrap();
        let mut obj = ws("Севр");
        let err = attach_ws_definition(Format::Edt, "WSReference", &mdo, &mut obj).unwrap_err();
        assert!(err.to_string().contains("extra.xsd"), "got: {err}");
        let _ = std::fs::remove_dir_all(&base);
    }
}
