//! Attach the CommonTemplate GEOSCHEME BODY SIDECAR (`Template.geos` EDT / `Ext/Template.xml`
//! Designer) into the object's IR `templates` during whole-config read, and re-emit it on write
//! (§1.0/§1.3/§1.6). Sibling of [`crate::mxl_read`] (SpreadsheetDocument `Template.mxlx`) but for
//! `templateType == GeographicalSchema`.
//!
//! # Why a DEDICATED pass (not the mxl_read blob copy)
//! Every other template body (Text/MXL/DCS/XDTO) carries ONE dialect shared by EDT and Designer
//! (byte-identical after BOM strip) → the pipeline blob-copies it. The geoscheme body does NOT:
//! EDT `Template.geos` is the g5 dialect (`<geographicalSchema:GeographicalSchema>`, expanded,
//! materialized default colors/fonts/borders), Designer `Ext/Template.xml` is the extrnprops
//! dialect (`<GeographicalScheme>`, compact attributes). The two are a STRUCTURAL transform
//! ([`formats_xml::geoschema`]), not a blob.
//!
//! # Canonical IR body = extrnprops (Designer/cf dialect) bytes, BOM-stripped (§1.6)
//! The cf `<uuid>.0` body == the Designer `Ext/Template.xml` bytes (extrnprops). So the canonical
//! body carried in `Template::body` is the extrnprops form:
//! * **Designer read** — read `Ext/Template.xml`, strip BOM, store verbatim (blob).
//! * **EDT read** — read `Template.geos` (g5), TRANSFORM g5→extrnprops, store.
//! * **Designer write** — `BOM + body` → `Ext/Template.xml` (blob).
//! * **cf** — `formats_cf::assemble_cf` emits `<uuid>.0` = `BOM + body` (blob).
//! * **EDT write** — TRANSFORM body (extrnprops) → g5 → `Template.geos`.
//!
//! # §1.0
//! The body is OPTIONAL (a bodyless GeographicalSchema stub attaches nothing — matches the platform
//! reference, which stored the coverage geoscheme bodyless before this milestone). A body outside
//! the witnessed subset (non-default projection/bounds) makes the g5 writer refuse loudly
//! ([`formats_xml::geoschema::GeoError`]) rather than fabricate a wrong `Template.geos`.

use std::path::{Path, PathBuf};

use formats_xml::geoschema::{read_geoschema, write_geoschema, GeoDialect};
use formats_xml::registry::Format;
use morph1c_core::ir::value::PropertyValue;
use morph1c_core::ir::{MetadataObject, Template};
use morph1c_core::spec::metadata::common_template::{
    F_TEMPLATE_TYPE, TEMPLATE_TYPE_GEOGRAPHICAL_SCHEMA,
};

use crate::ConvertError;

/// Kinds whose object carries a geoscheme template-body sidecar (currently only `CommonTemplate`).
const GEOS_SIDECAR_KINDS: &[&str] = &["CommonTemplate"];

/// EDT sidecar file name of a GeographicalSchema template body (g5 dialect).
const GEOS_SIDECAR_EDT: &str = "Template.geos";
/// Designer sidecar file name of a GeographicalSchema template body (extrnprops dialect, under
/// `<Name>/Ext/`) — same file name as MXL, disambiguated by `templateType`.
const GEOS_SIDECAR_DESIGNER: &str = "Template.xml";

/// UTF-8 BOM (stripped to get the canonical, format-neutral body — §1.6).
const BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

/// Load the GeographicalSchema CommonTemplate body (from its sidecar) into `obj.templates`, stored
/// CANONICALLY as extrnprops (Designer/cf dialect) bytes — EDT bodies are transformed g5→extrnprops.
///
/// `descriptor_path` — path of the descriptor just read (`.mdo`/`.xml`). Only GeographicalSchema
/// templates attach here. §1.0: the body is OPTIONAL (a bodyless stub attaches nothing). Non-
/// GeographicalSchema template types, non-template kinds, and cf (container) are no-ops.
pub fn attach_geoschema_bodies(
    format: Format,
    kind: &str,
    descriptor_path: &Path,
    obj: &mut MetadataObject,
) -> Result<(), ConvertError> {
    attach_child_geoschema_bodies(format, descriptor_path, obj)?;
    if !GEOS_SIDECAR_KINDS.contains(&kind)
        || template_type(obj) != TEMPLATE_TYPE_GEOGRAPHICAL_SCHEMA
    {
        return Ok(());
    }
    let path = match geos_sidecar_path(format, descriptor_path) {
        Some(p) => p,
        None => return Ok(()), // cf: container — no file-per-object sidecar.
    };
    if !path.is_file() {
        return Ok(()); // OPTIONAL body (bodyless stub) — no-op.
    }
    let bytes = std::fs::read(&path).map_err(|e| ConvertError::Io {
        path: path.display().to_string(),
        reason: e.to_string(),
    })?;
    // Canonicalize to extrnprops (Designer/cf) bytes, BOM-stripped (§1.6: ONE canonical body,
    // edt==designer). `write_geoschema(Designer, …)` emits WITH a BOM (Designer envelope), so the
    // EDT path strips it too — both formats converge on the identical BOM-less canonical.
    let canonical = match format {
        // Designer sidecar IS extrnprops (with BOM) — strip BOM, store verbatim (blob).
        Format::Designer => bytes.strip_prefix(BOM).unwrap_or(&bytes).to_vec(),
        // EDT sidecar is g5 — transform g5 → extrnprops, then strip the emitted BOM.
        Format::Edt => {
            let extrnprops = transform(GeoDialect::Edt, GeoDialect::Designer, &bytes, obj, kind)?;
            extrnprops.strip_prefix(BOM).unwrap_or(&extrnprops).to_vec()
        }
        Format::Cf => return Ok(()),
    };
    if !obj.templates.is_empty() {
        return Err(ConvertError::Read {
            kind: kind.to_string(),
            object: obj.name.clone(),
            reason: "object already carries templates before the geoscheme sidecar attach \
                     (unexpected — only this pass populates a GeographicalSchema body)"
                .into(),
        });
    }
    obj.templates.push(Template {
        pages: Vec::new(),
        resources: Vec::new(),
        name: obj.name.clone(),
        properties: Vec::new(),
        body: Some(canonical),
    });
    Ok(())
}

/// Child-level half of [`attach_geoschema_bodies`]: attach each GeographicalSchema
/// `*.TemplateRef` child's body (object-subordinate template, sidecar under the HOST — RE:
/// coverage/s15_subordinate: EDT `Templates/<T>/Template.geos` g5, Designer
/// `Templates/<T>/Ext/Template.xml` extrnprops). Same canonicalization as the object level:
/// stored body = extrnprops bytes, BOM-stripped; EDT reads TRANSFORM g5→extrnprops. OPTIONAL
/// body (a descriptor-only stub attaches nothing, §1.0). NB (call-order): Designer sources need
/// the [`crate::template_ref_read`] enrichment FIRST (bare-ref children carry no `templateType`).
fn attach_child_geoschema_bodies(
    format: Format,
    descriptor_path: &Path,
    obj: &mut MetadataObject,
) -> Result<(), ConvertError> {
    if format == Format::Cf {
        return Ok(()); // container — no file-per-object sidecars.
    }
    for child in obj.children.iter_mut() {
        if !crate::template_read::is_template_ref_child(child)
            || crate::template_read::template_type_of(child) != TEMPLATE_TYPE_GEOGRAPHICAL_SCHEMA
        {
            continue;
        }
        let path = match crate::template_read::child_sidecar_path(
            format,
            descriptor_path,
            &child.name,
            GEOS_SIDECAR_EDT,
            GEOS_SIDECAR_DESIGNER,
        ) {
            Some(p) => p,
            None => continue, // no parent/stem (defensive).
        };
        if !path.is_file() {
            continue; // OPTIONAL body — descriptor-only stub.
        }
        let bytes = std::fs::read(&path).map_err(|e| ConvertError::Io {
            path: path.display().to_string(),
            reason: e.to_string(),
        })?;
        let kind = child.kind.as_str().to_string();
        let canonical = match format {
            Format::Designer => bytes.strip_prefix(BOM).unwrap_or(&bytes).to_vec(),
            Format::Edt => {
                let extrnprops =
                    transform(GeoDialect::Edt, GeoDialect::Designer, &bytes, child, &kind)?;
                extrnprops.strip_prefix(BOM).unwrap_or(&extrnprops).to_vec()
            }
            Format::Cf => continue,
        };
        // §1.0: only this pass populates a GeographicalSchema child's templates.
        if !child.templates.is_empty() {
            return Err(ConvertError::Read {
                kind,
                object: child.name.clone(),
                reason: "child already carries templates before the geoscheme sidecar attach \
                         (unexpected — only this pass populates a GeographicalSchema body)"
                    .into(),
            });
        }
        child.templates.push(Template {
            pages: Vec::new(),
            resources: Vec::new(),
            name: child.name.clone(),
            properties: Vec::new(),
            body: Some(canonical),
        });
    }
    Ok(())
}

/// Write-side mirror of [`attach_geoschema_bodies`]: emit the GeographicalSchema body beside the
/// just-written descriptor, in the TARGET dialect — the CommonTemplate OBJECT body AND each
/// GeographicalSchema `*.TemplateRef` CHILD body. Designer re-adds the BOM (`Ext/Template.xml`,
/// extrnprops blob); EDT TRANSFORMS the canonical (extrnprops) body → g5 (`Template.geos`). No-op
/// for non-GeographicalSchema / bodyless objects and cf.
pub fn write_geoschema_bodies(
    format: Format,
    kind: &str,
    descriptor_out: &Path,
    obj: &MetadataObject,
) -> Result<(), ConvertError> {
    // (a) GeographicalSchema `*.TemplateRef` children.
    for child in &obj.children {
        if !crate::template_read::is_template_ref_child(child)
            || crate::template_read::template_type_of(child) != TEMPLATE_TYPE_GEOGRAPHICAL_SCHEMA
        {
            continue;
        }
        let body = match child.templates.first().and_then(|t| t.body.as_ref()) {
            Some(b) => b,
            None => continue, // bodyless stub — mirrors the read's optional attach.
        };
        if let Some(path) = crate::template_read::child_sidecar_path(
            format,
            descriptor_out,
            &child.name,
            GEOS_SIDECAR_EDT,
            GEOS_SIDECAR_DESIGNER,
        ) {
            write_geos_body(format, &path, body, child, child.kind.as_str())?;
        }
    }
    // (b) CommonTemplate object-level body.
    if !GEOS_SIDECAR_KINDS.contains(&kind)
        || template_type(obj) != TEMPLATE_TYPE_GEOGRAPHICAL_SCHEMA
    {
        return Ok(());
    }
    let body = match obj.templates.iter().find_map(|t| t.body.as_ref()) {
        Some(b) => b,
        None => return Ok(()), // bodyless stub — matches the read's optional attach.
    };
    let path = match geos_sidecar_path(format, descriptor_out) {
        Some(p) => p,
        None => return Ok(()), // cf: container.
    };
    write_geos_body(format, &path, body, obj, kind)
}

/// Emit ONE geoscheme body at `path` in the target dialect: Designer = BOM + extrnprops
/// canonical (blob); EDT = TRANSFORM canonical (extrnprops) → g5.
fn write_geos_body(
    format: Format,
    path: &Path,
    body: &[u8],
    obj: &MetadataObject,
    kind: &str,
) -> Result<(), ConvertError> {
    let bytes = match format {
        Format::Designer => {
            let mut b = Vec::with_capacity(BOM.len() + body.len());
            b.extend_from_slice(BOM);
            b.extend_from_slice(body);
            b
        }
        Format::Edt => transform(GeoDialect::Designer, GeoDialect::Edt, body, obj, kind)?,
        Format::Cf => return Ok(()),
    };
    crate::form_write::write_file(path, &bytes)
}

/// Transform a geoscheme body `from`→`to` dialect via [`formats_xml::geoschema`], mapping any
/// [`GeoError`](formats_xml::geoschema::GeoError) to a typed [`ConvertError`] (§1.0 — never a
/// silent/wrong body).
fn transform(
    from: GeoDialect,
    to: GeoDialect,
    bytes: &[u8],
    obj: &MetadataObject,
    kind: &str,
) -> Result<Vec<u8>, ConvertError> {
    let geo = read_geoschema(from, bytes).map_err(|e| ConvertError::Read {
        kind: kind.to_string(),
        object: obj.name.clone(),
        reason: format!("geoscheme body: {e}"),
    })?;
    write_geoschema(to, &geo).map_err(|e| ConvertError::Write {
        kind: kind.to_string(),
        object: obj.name.clone(),
        reason: format!("geoscheme body: {e}"),
    })
}

/// `templateType` of a CommonTemplate descriptor as `&str`, or the canonical default
/// `SpreadsheetDocument` when the field is absent.
fn template_type(obj: &MetadataObject) -> &str {
    match obj.get(F_TEMPLATE_TYPE) {
        Some(PropertyValue::Enum(t)) => t.as_str(),
        _ => morph1c_core::spec::metadata::common_template::TEMPLATE_TYPE_SPREADSHEET_DOCUMENT,
    }
}

/// Geoscheme sidecar path beside the descriptor, per format layout. `None` for cf (container).
fn geos_sidecar_path(format: Format, descriptor_path: &Path) -> Option<PathBuf> {
    match format {
        // EDT: descriptor `<obj-dir>/<Name>.mdo` → sibling `<obj-dir>/Template.geos`.
        Format::Edt => Some(descriptor_path.parent()?.join(GEOS_SIDECAR_EDT)),
        // Designer: descriptor `<dir>/<Name>.xml` → `<dir>/<Name>/Ext/Template.xml`.
        Format::Designer => {
            let dir = descriptor_path.parent()?;
            let stem = descriptor_path.file_stem()?;
            Some(dir.join(stem).join("Ext").join(GEOS_SIDECAR_DESIGNER))
        }
        Format::Cf => None,
    }
}

#[cfg(any())]
mod tests {
    use super::*;
    use morph1c_core::ir::value::Token;
    use morph1c_core::ir::{ObjectKind, Uuid};

    fn geoscheme_obj(name: &str) -> MetadataObject {
        let mut o = MetadataObject::new(ObjectKind::new("CommonTemplate"), name, Uuid([1; 16]));
        o.properties.push((
            F_TEMPLATE_TYPE,
            PropertyValue::Enum(Token::new(TEMPLATE_TYPE_GEOGRAPHICAL_SCHEMA)),
        ));
        o
    }

    #[test]
    fn edt_sidecar_path_is_sibling_geos() {
        let p = Path::new("/root/CommonTemplates/Макет/Макет.mdo");
        let s = geos_sidecar_path(Format::Edt, p).unwrap();
        assert!(
            s.ends_with(Path::new("CommonTemplates/Макет/Template.geos")),
            "got {s:?}"
        );
    }

    #[test]
    fn designer_sidecar_path_is_ext_xml() {
        let p = Path::new("/root/CommonTemplates/Макет.xml");
        let s = geos_sidecar_path(Format::Designer, p).unwrap();
        assert!(
            s.ends_with(Path::new("CommonTemplates/Макет/Ext/Template.xml")),
            "got {s:?}"
        );
    }

    #[test]
    fn non_geoscheme_template_type_is_noop() {
        // A SpreadsheetDocument (default) template attaches nothing here (mxl_read handles it).
        let mut obj =
            MetadataObject::new(ObjectKind::new("CommonTemplate"), "Макет", Uuid([2; 16]));
        attach_geoschema_bodies(
            Format::Edt,
            "CommonTemplate",
            Path::new("/nope/CommonTemplates/Макет/Макет.mdo"),
            &mut obj,
        )
        .unwrap();
        assert!(obj.templates.is_empty());
    }

    #[test]
    fn child_geoscheme_designer_attach_verbatim_and_write_readds_bom() {
        // A GeographicalSchema `*.TemplateRef` child: Designer read = extrnprops verbatim
        // (BOM-stripped), Designer write = BOM + canonical. (The EDT g5 transform legs are the
        // object-level `transform` — covered by the geoschema module/its object-level tests and
        // the s15 conversion diff.)
        let base = std::env::temp_dir().join(format!(
            "morph1c-geos-child-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let ext = base
            .join("DataProcessors")
            .join("Обр")
            .join("Templates")
            .join("МакетГео")
            .join("Ext");
        std::fs::create_dir_all(&ext).unwrap();
        let mut with_bom = Vec::from(BOM);
        with_bom.extend_from_slice(b"<GeographicalScheme/>");
        crate::fsio::write(ext.join("Template.xml"), &with_bom).unwrap();
        let host = base.join("DataProcessors").join("Обр.xml");

        let mut child = MetadataObject::new(
            ObjectKind::new("DataProcessor.TemplateRef"),
            "МакетГео",
            Uuid([3; 16]),
        );
        child.properties.push((
            F_TEMPLATE_TYPE,
            PropertyValue::Enum(Token::new(TEMPLATE_TYPE_GEOGRAPHICAL_SCHEMA)),
        ));
        let mut obj = MetadataObject::new(ObjectKind::new("DataProcessor"), "Обр", Uuid([1; 16]));
        obj.children.push(child);
        attach_geoschema_bodies(Format::Designer, "DataProcessor", &host, &mut obj).unwrap();
        assert_eq!(
            obj.children[0].templates[0].body.as_deref(),
            Some(b"<GeographicalScheme/>".as_slice()),
            "Designer child geoscheme attaches the extrnprops blob verbatim (BOM-stripped)"
        );

        std::fs::create_dir_all(base.join("out").join("DataProcessors")).unwrap();
        let out_host = base.join("out").join("DataProcessors").join("Обр.xml");
        write_geoschema_bodies(Format::Designer, "DataProcessor", &out_host, &obj).unwrap();
        let written =
            std::fs::read(base.join("out/DataProcessors/Обр/Templates/МакетГео/Ext/Template.xml"))
                .unwrap();
        assert_eq!(
            written, with_bom,
            "Designer child write = BOM + canonical verbatim"
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn bodyless_geoscheme_is_noop() {
        let mut obj = geoscheme_obj("Макет");
        attach_geoschema_bodies(
            Format::Edt,
            "CommonTemplate",
            Path::new("/nonexistent/CommonTemplates/Макет/Макет.mdo"),
            &mut obj,
        )
        .expect("bodyless geoscheme must be a no-op, not an error");
        assert!(obj.templates.is_empty());
    }
}
