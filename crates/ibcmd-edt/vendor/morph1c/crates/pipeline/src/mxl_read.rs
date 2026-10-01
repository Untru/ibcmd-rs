//! Attach the CommonTemplate MXL BODY SIDECAR (`Template.mxlx` EDT / `Ext/Template.xml`
//! Designer) into the object's IR `templates` during whole-config read (§1.0/§1.3/§1.6). Mirror
//! of [`crate::template_read`] (TextDocument `Template.txt`) but for `templateType ==
//! SpreadsheetDocument` (MOXEL) templates, whose cf `<uuid>.0` body is the binary MXL image
//! ([`formats_cf::mxl_body`]).
//!
//! # Why: edt/designer→cf needs the MXL body the way it needs the text/schema body
//! A SpreadsheetDocument CommonTemplate is emitted in cf as the descriptor `<uuid>` PLUS a
//! separate body element `<uuid>.0` = the MOXCEL header + brace image ([`formats_cf::mxl_body`]).
//! The per-kind descriptor connector reads only the thin `.mdo`/`.xml` (synonym/comment/
//! templateType); the MXL BODY lives in the sibling `Template.mxlx`. This pass reads the raw
//! mxlx bytes into `obj.templates[0].body` so `--to cf` can tokenize + emit the `<uuid>.0` body.
//! The encoder tokenizes the stored bytes itself (like [`crate::dcs_read`] → `dcs_body`), so this
//! pass stores the canonical (BOM-stripped) mxlx verbatim — no structural parse here.
//!
//! # Sidecar layout beside the descriptor (RE: coverage/s12_templates edt + designer — VERIFIED
//! byte-identical after BOM strip)
//! * **EDT** (`CommonTemplates/<Name>/<Name>.mdo`, dir-per-object): sibling
//!   `CommonTemplates/<Name>/Template.mxlx` (BOM-less on disk).
//! * **Designer** (`CommonTemplates/<Name>.xml`, file-per-object): sidecar
//!   `CommonTemplates/<Name>/Ext/Template.xml` (carries a UTF-8 BOM on disk).
//!
//! Both give the SAME canonical body once the leading BOM is stripped (§1.6).
//!
//! # §1.0: the MXL body is OPTIONAL (a bodyless SpreadsheetDocument stub attaches nothing)
//! Unlike a Role's rights sidecar, a SpreadsheetDocument CommonTemplate has NO independent
//! "declared body" marker — the sidecar's presence IS the body. s4_common ships a property-stub
//! `ОбщМакет_ТипМакета_ТабличныйДокумент` (default SpreadsheetDocument, testing `templateType` in
//! isolation) WITHOUT a `Template.mxlx`; making the sidecar mandatory would break whole-config
//! READ of s4. So: attach the body IFF the sidecar exists (reflects the on-disk state). Downstream
//! honesty stays at ASSEMBLE — a SpreadsheetDocument template WITH a body emits its `<uuid>.0` (the
//! [`formats_cf::mxl_body`] encoder, which §1.0-refuses any construct outside the witnessed subset);
//! a bodyless one that reaches the body-requiring assemble path surfaces `BodyNotEmitted` loudly,
//! never a silent wrong body.

use std::path::{Path, PathBuf};

use formats_xml::registry::Format;
use morph1c_core::ir::value::PropertyValue;
use morph1c_core::ir::{MetadataObject, Template};
use morph1c_core::spec::metadata::common_template::{
    F_TEMPLATE_TYPE, TEMPLATE_TYPE_SPREADSHEET_DOCUMENT,
};

use crate::ConvertError;

/// Kinds whose object carries an MXL template-body sidecar (currently only `CommonTemplate`).
const MXL_SIDECAR_KINDS: &[&str] = &["CommonTemplate"];

/// The EDT sidecar file name of a SpreadsheetDocument (MOXEL) template body.
const MXL_SIDECAR_EDT: &str = "Template.mxlx";
/// The Designer sidecar file name of a SpreadsheetDocument template body (under `<Name>/Ext/`).
const MXL_SIDECAR_DESIGNER: &str = "Template.xml";

/// UTF-8 BOM (stripped to get the canonical, format-neutral body — §1.6).
const BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

/// Load the SpreadsheetDocument MXL body (from its sidecar) into `obj.templates` — for the
/// CommonTemplate OBJECT and for each SpreadsheetDocument `*.TemplateRef` CHILD (object-
/// subordinate template, sidecar under the HOST: EDT `<host-dir>/Templates/<T>/Template.mxlx`,
/// Designer `<host-stem>/Templates/<T>/Ext/Template.xml` — RE: coverage/s15_subordinate,
/// VERIFIED byte-identical after BOM strip like the CommonTemplate pair).
///
/// `descriptor_path` — path of the descriptor just read (`.mdo`/`.xml`). Only `SpreadsheetDocument`
/// templates attach an MXL body. §1.0: the body is OPTIONAL — attach IFF the sidecar exists (a
/// bodyless SpreadsheetDocument stub attaches nothing, mirroring the TextDocument case). Non-
/// SpreadsheetDocument template types, non-template kinds, and cf (container) are no-ops.
/// NB (call-order): Designer sources need the [`crate::template_ref_read`] enrichment FIRST
/// (a bare-ref child carries no `templateType`).
pub fn attach_mxl_bodies(
    format: Format,
    kind: &str,
    descriptor_path: &Path,
    obj: &mut MetadataObject,
) -> Result<(), ConvertError> {
    attach_child_mxl_bodies(format, descriptor_path, obj)?;
    if !MXL_SIDECAR_KINDS.contains(&kind) {
        return Ok(());
    }
    if template_type(obj) != TEMPLATE_TYPE_SPREADSHEET_DOCUMENT {
        return Ok(());
    }
    let path = match mxl_sidecar_path(format, descriptor_path) {
        Some(p) => p,
        None => return Ok(()), // cf: container — no file-per-object sidecar to attach.
    };
    // OPTIONAL body: absent ⇒ no-op (a property-stub SpreadsheetDocument template without a
    // Template.mxlx, e.g. s4_common's ОбщМакет_ТипМакета_ТабличныйДокумент). Never a hard error
    // here; assemble surfaces BodyNotEmitted for a bodyless body-requiring path.
    if !path.is_file() {
        return Ok(());
    }
    let bytes = std::fs::read(&path).map_err(|e| ConvertError::Io {
        path: path.display().to_string(),
        reason: e.to_string(),
    })?;
    // Canonical body = BOM-stripped bytes (EDT has none, Designer has one → equal after strip).
    let body = bytes.strip_prefix(BOM).unwrap_or(&bytes).to_vec();
    // §1.0: never silently overwrite an already-attached body (the descriptor read + the
    // TextDocument pass must not populate `templates` for a SpreadsheetDocument template).
    if !obj.templates.is_empty() {
        return Err(ConvertError::Read {
            kind: kind.to_string(),
            object: obj.name.clone(),
            reason: "object already carries templates before the MXL sidecar attach (unexpected — \
                     only this pass populates a SpreadsheetDocument body)"
                .into(),
        });
    }
    obj.templates.push(Template {
        pages: Vec::new(),
        resources: Vec::new(),
        name: obj.name.clone(),
        properties: Vec::new(),
        body: Some(body),
    });
    Ok(())
}

/// Child-level half of [`attach_mxl_bodies`]: attach each SpreadsheetDocument `*.TemplateRef`
/// child's MXL body (verbatim blob, BOM-stripped — the two dialects are byte-identical after the
/// strip, s15 witness). OPTIONAL body (a descriptor-only stub attaches nothing, §1.0).
fn attach_child_mxl_bodies(
    format: Format,
    descriptor_path: &Path,
    obj: &mut MetadataObject,
) -> Result<(), ConvertError> {
    if format == Format::Cf {
        return Ok(()); // container — no file-per-object sidecars.
    }
    for child in obj.children.iter_mut() {
        if !crate::template_read::is_template_ref_child(child)
            || crate::template_read::template_type_of(child) != TEMPLATE_TYPE_SPREADSHEET_DOCUMENT
        {
            continue;
        }
        let path = match crate::template_read::child_sidecar_path(
            format,
            descriptor_path,
            &child.name,
            MXL_SIDECAR_EDT,
            MXL_SIDECAR_DESIGNER,
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
        let body = bytes.strip_prefix(BOM).unwrap_or(&bytes).to_vec();
        // §1.0: only this pass populates a SpreadsheetDocument child's templates.
        if !child.templates.is_empty() {
            return Err(ConvertError::Read {
                kind: child.kind.as_str().to_string(),
                object: child.name.clone(),
                reason: "child already carries templates before the MXL sidecar attach \
                         (unexpected — only this pass populates a SpreadsheetDocument body)"
                    .into(),
            });
        }
        child.templates.push(Template {
            pages: Vec::new(),
            resources: Vec::new(),
            name: child.name.clone(),
            properties: Vec::new(),
            body: Some(body),
        });
    }
    Ok(())
}

/// Write-side mirror of [`attach_mxl_bodies`]: emit the SpreadsheetDocument MXL body beside the
/// just-written descriptor `descriptor_out` — the CommonTemplate OBJECT body AND each
/// SpreadsheetDocument `*.TemplateRef` CHILD body. Canonical (BOM-stripped) body from
/// `templates`; Designer re-adds the BOM (`Ext/Template.xml`), EDT writes it bare
/// (`Template.mxlx`). No-op for non-SpreadsheetDocument / bodyless objects and cf. §1.0: typed
/// [`ConvertError`] on I/O.
pub fn write_mxl_bodies(
    format: Format,
    kind: &str,
    descriptor_out: &Path,
    obj: &MetadataObject,
) -> Result<(), ConvertError> {
    // (a) SpreadsheetDocument `*.TemplateRef` children.
    for child in &obj.children {
        if !crate::template_read::is_template_ref_child(child)
            || crate::template_read::template_type_of(child) != TEMPLATE_TYPE_SPREADSHEET_DOCUMENT
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
            MXL_SIDECAR_EDT,
            MXL_SIDECAR_DESIGNER,
        ) {
            write_mxl_body(format, &path, body)?;
        }
    }
    // (b) CommonTemplate object-level body.
    if !MXL_SIDECAR_KINDS.contains(&kind)
        || template_type(obj) != TEMPLATE_TYPE_SPREADSHEET_DOCUMENT
    {
        return Ok(());
    }
    // The MXL read attaches exactly one Template carrying the body (or nothing for a bodyless
    // stub). Find that body; absent ⇒ no-op (matches the read's optional attach).
    let body = match obj.templates.iter().find_map(|t| t.body.as_ref()) {
        Some(b) => b,
        None => return Ok(()),
    };
    let path = match mxl_sidecar_path(format, descriptor_out) {
        Some(p) => p,
        None => return Ok(()), // cf: container.
    };
    write_mxl_body(format, &path, body)
}

/// Emit ONE MXL body at `path` in the target dialect: Designer = BOM + body; EDT = bare body.
fn write_mxl_body(format: Format, path: &Path, body: &[u8]) -> Result<(), ConvertError> {
    let bytes = match format {
        Format::Designer => {
            let mut b = Vec::with_capacity(BOM.len() + body.len());
            b.extend_from_slice(BOM);
            b.extend_from_slice(body);
            b
        }
        Format::Edt | Format::Cf => body.to_vec(),
    };
    crate::form_write::write_file(path, &bytes)
}

/// `templateType` of a CommonTemplate descriptor as `&str`, or the canonical default
/// `SpreadsheetDocument` when the (sparse) field is absent — mirrors the spec default.
fn template_type(obj: &MetadataObject) -> &str {
    match obj.get(F_TEMPLATE_TYPE) {
        Some(PropertyValue::Enum(t)) => t.as_str(),
        _ => TEMPLATE_TYPE_SPREADSHEET_DOCUMENT,
    }
}

/// MXL sidecar path beside the descriptor, per format layout (see module docs). `None` for
/// formats without a file-per-object sidecar (cf is a container).
fn mxl_sidecar_path(format: Format, descriptor_path: &Path) -> Option<PathBuf> {
    match format {
        // EDT: descriptor `<obj-dir>/<Name>.mdo` → sibling `<obj-dir>/Template.mxlx`.
        Format::Edt => {
            let obj_dir = descriptor_path.parent()?;
            Some(obj_dir.join(MXL_SIDECAR_EDT))
        }
        // Designer: descriptor `<dir>/<Name>.xml` → `<dir>/<Name>/Ext/Template.xml`.
        Format::Designer => {
            let dir = descriptor_path.parent()?;
            let stem = descriptor_path.file_stem()?;
            Some(dir.join(stem).join("Ext").join(MXL_SIDECAR_DESIGNER))
        }
        Format::Cf => None,
    }
}

#[cfg(any())]
mod tests {
    use super::*;
    use morph1c_core::ir::value::Token;
    use morph1c_core::ir::{ObjectKind, Uuid};

    fn spreadsheet_template(name: &str) -> MetadataObject {
        // default templateType (field absent) == SpreadsheetDocument.
        MetadataObject::new(ObjectKind::new("CommonTemplate"), name, Uuid([1; 16]))
    }

    #[test]
    fn edt_sidecar_path_is_sibling() {
        let p = Path::new("/root/CommonTemplates/Макет/Макет.mdo");
        let s = mxl_sidecar_path(Format::Edt, p).unwrap();
        assert!(
            s.ends_with(Path::new("CommonTemplates/Макет/Template.mxlx")),
            "got {s:?}"
        );
    }

    #[test]
    fn designer_sidecar_path_is_ext_subdir() {
        let p = Path::new("/root/CommonTemplates/Макет.xml");
        let s = mxl_sidecar_path(Format::Designer, p).unwrap();
        assert!(
            s.ends_with(Path::new("CommonTemplates/Макет/Ext/Template.xml")),
            "got {s:?}"
        );
    }

    #[test]
    fn non_template_kind_is_noop() {
        let mut obj = MetadataObject::new(ObjectKind::new("Constant"), "C", Uuid([0; 16]));
        attach_mxl_bodies(
            Format::Edt,
            "Constant",
            Path::new("/nope/C/C.mdo"),
            &mut obj,
        )
        .unwrap();
        assert!(obj.templates.is_empty());
    }

    #[test]
    fn non_spreadsheet_template_type_is_noop() {
        let mut obj =
            MetadataObject::new(ObjectKind::new("CommonTemplate"), "Макет", Uuid([2; 16]));
        obj.properties.push((
            F_TEMPLATE_TYPE,
            PropertyValue::Enum(Token::new("TextDocument")),
        ));
        attach_mxl_bodies(
            Format::Edt,
            "CommonTemplate",
            Path::new("/nope/CommonTemplates/Макет/Макет.mdo"),
            &mut obj,
        )
        .unwrap();
        assert!(
            obj.templates.is_empty(),
            "TextDocument attaches no MXL body"
        );
    }

    #[test]
    fn bodyless_spreadsheet_template_is_noop() {
        // A property-stub SpreadsheetDocument template without a Template.mxlx (s4 case) attaches
        // NOTHING and does NOT error.
        let mut obj = spreadsheet_template("Макет");
        attach_mxl_bodies(
            Format::Edt,
            "CommonTemplate",
            Path::new("/nonexistent-root/CommonTemplates/Макет/Макет.mdo"),
            &mut obj,
        )
        .expect("bodyless SpreadsheetDocument template must be a no-op, not an error");
        assert!(obj.templates.is_empty());
    }

    #[test]
    fn attaches_edt_mxlx_sidecar_bom_stripped() {
        let base = std::env::temp_dir().join(format!(
            "morph1c-mxl-read-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let obj_dir = base.join("CommonTemplates").join("Макет");
        std::fs::create_dir_all(&obj_dir).unwrap();
        crate::fsio::write(obj_dir.join("Template.mxlx"), b"<document/>").unwrap();
        let descriptor = obj_dir.join("Макет.mdo");

        let mut obj = spreadsheet_template("Макет");
        attach_mxl_bodies(Format::Edt, "CommonTemplate", &descriptor, &mut obj).unwrap();
        assert_eq!(obj.templates.len(), 1, "one MXL body attached");
        assert_eq!(
            obj.templates[0].body.as_deref(),
            Some(b"<document/>".as_slice())
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn child_mxl_attach_and_write_round_trip_both_dialects() {
        // A SpreadsheetDocument `*.TemplateRef` child (default templateType — field absent, the
        // s15 МакетТабДок shape): the MXL body is a verbatim blob (BOM add/strip only).
        let base = std::env::temp_dir().join(format!(
            "morph1c-mxl-child-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        // EDT source: Templates/<T>/Template.mxlx (BOM-less).
        let edt_tdir = base
            .join("edt")
            .join("DataProcessors")
            .join("Обр")
            .join("Templates")
            .join("МакетТабДок");
        std::fs::create_dir_all(&edt_tdir).unwrap();
        crate::fsio::write(edt_tdir.join("Template.mxlx"), b"<document/>").unwrap();
        let edt_host = base
            .join("edt")
            .join("DataProcessors")
            .join("Обр")
            .join("Обр.mdo");

        let mut obj = MetadataObject::new(ObjectKind::new("DataProcessor"), "Обр", Uuid([1; 16]));
        obj.children.push(MetadataObject::new(
            ObjectKind::new("DataProcessor.TemplateRef"),
            "МакетТабДок",
            Uuid([2; 16]),
        ));
        attach_mxl_bodies(Format::Edt, "DataProcessor", &edt_host, &mut obj).unwrap();
        assert_eq!(
            obj.children[0].templates[0].body.as_deref(),
            Some(b"<document/>".as_slice())
        );

        // Designer target: Templates/<T>/Ext/Template.xml = BOM + blob.
        std::fs::create_dir_all(base.join("dz").join("DataProcessors")).unwrap();
        let dz_host = base.join("dz").join("DataProcessors").join("Обр.xml");
        write_mxl_bodies(Format::Designer, "DataProcessor", &dz_host, &obj).unwrap();
        let dz = std::fs::read(
            base.join("dz/DataProcessors/Обр/Templates/МакетТабДок/Ext/Template.xml"),
        )
        .unwrap();
        let mut expected = Vec::from(BOM);
        expected.extend_from_slice(b"<document/>");
        assert_eq!(dz, expected, "Designer child MXL = BOM + blob");

        // Designer read attaches the same canonical body back; EDT write restores the mxlx.
        let mut obj2 = MetadataObject::new(ObjectKind::new("DataProcessor"), "Обр", Uuid([1; 16]));
        obj2.children.push(MetadataObject::new(
            ObjectKind::new("DataProcessor.TemplateRef"),
            "МакетТабДок",
            Uuid([2; 16]),
        ));
        attach_mxl_bodies(Format::Designer, "DataProcessor", &dz_host, &mut obj2).unwrap();
        assert_eq!(
            obj2.children[0].templates[0].body.as_deref(),
            Some(b"<document/>".as_slice())
        );
        std::fs::create_dir_all(base.join("edt2").join("DataProcessors").join("Обр")).unwrap();
        let edt2_host = base
            .join("edt2")
            .join("DataProcessors")
            .join("Обр")
            .join("Обр.mdo");
        write_mxl_bodies(Format::Edt, "DataProcessor", &edt2_host, &obj2).unwrap();
        let edt2 =
            std::fs::read(base.join("edt2/DataProcessors/Обр/Templates/МакетТабДок/Template.mxlx"))
                .unwrap();
        assert_eq!(edt2, b"<document/>".to_vec(), "EDT child MXL = bare blob");

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn attaches_designer_mxlx_sidecar_strips_bom() {
        let base = std::env::temp_dir().join(format!(
            "morph1c-mxl-read-des-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let ext_dir = base.join("CommonTemplates").join("Макет").join("Ext");
        std::fs::create_dir_all(&ext_dir).unwrap();
        let mut with_bom = Vec::from(BOM);
        with_bom.extend_from_slice(b"<document/>");
        crate::fsio::write(ext_dir.join("Template.xml"), &with_bom).unwrap();
        let descriptor = base.join("CommonTemplates").join("Макет.xml");

        let mut obj = spreadsheet_template("Макет");
        attach_mxl_bodies(Format::Designer, "CommonTemplate", &descriptor, &mut obj).unwrap();
        assert_eq!(
            obj.templates[0].body.as_deref(),
            Some(b"<document/>".as_slice()),
            "Designer body BOM-stripped == EDT canonical body (§1.6)"
        );

        let _ = std::fs::remove_dir_all(&base);
    }
}
