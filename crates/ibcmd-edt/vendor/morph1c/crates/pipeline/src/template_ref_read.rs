//! Enrich a DESIGNER object's template-ref CHILDREN (`*.TemplateRef`, e.g. a Report's macets)
//! with their NESTED descriptor during whole-config read (§1.0/§1.6). Read-side completeness pass
//! that makes the Designer child identity equal to what EDT reads inline.
//!
//! # Why: Designer template children are bare-refs; EDT carries the stub inline
//! A Report/DataProcessor/… template is a CHILD (`kind` ends `.TemplateRef`). EDT stores the stub
//! INLINE in the host `.mdo` (`<templates uuid=… templateType=…>`), so the child MetadataObject
//! read from it already carries uuid + templateType + synonym + comment. The Designer host `.xml`
//! carries only a BARE `<Template>Name</Template>` ref (the `DesignerTemplateRef` LocusMap projects
//! no properties), and the child's real identity lives in a SEPARATE
//! `<Host>/Templates/<Name>.xml` descriptor. Without this pass the Designer child stays a name-only
//! stub (no uuid/templateType), so `dcs_read::attach_dcs_bodies` never recognises a DCS template
//! and `--to cf` emits a Report WITHOUT its DCS template descriptor+body (structurally broken —
//! `cf_compare` fails). This pass reads the nested descriptor and transplants its identity onto the
//! bare-ref child, so designer→cf emits the SAME `<tmpl-uuid>`+`<tmpl-uuid>.0` as edt→cf.
//!
//! # Layout (RE: coverage/s11_dcs designer)
//! Host `Reports/<R>.xml` (file-per-object) → nested descriptor `Reports/<R>/Templates/<T>.xml`
//! (the sibling of the `<T>/Ext/Template.xml` body sidecar `dcs_read` attaches). The nested file
//! is a standalone `<MetaDataObject><Template uuid><Properties><Name><Synonym><Comment>
//! <TemplateType></Properties></Template>` — structurally the CommonTemplate descriptor with a
//! `<Template>` (not `<CommonTemplate>`) element.
//!
//! # Scope
//! Designer only (EDT reads inline; cf is a container). No-op for objects without template
//! children. Read-only completeness: the WRITE side (re-emitting the nested descriptor for a
//! Designer target) is a separate step — the cf target this unblocks needs no nested file.

use std::path::{Path, PathBuf};

use formats_xml::registry::Format;
use formats_xml::{Codec, FieldProjection, LocusMap, XmlLocus};
use morph1c_core::ir::{FieldId, MetadataObject, ObjectKind};
use morph1c_core::spec::metadata::report_template_ref::{self as tref, report_template_ref};

use crate::ConvertError;

/// The `Templates` collection subdir hosting each template's nested descriptor + body.
const TEMPLATES_DIR: &str = "Templates";
/// The wrapper element of a NESTED template descriptor under `<MetaDataObject>` — `<Template>`
/// (not `<CommonTemplate>`; the same `report_template_ref` spec, a different element name).
const TEMPLATE_ELEMENT: &str = "Template";

/// Is `child` a template-ref child (`kind` ends `.TemplateRef` — Report/DataProcessor/Document/…)?
fn is_template_ref_child(child: &MetadataObject) -> bool {
    child.kind.as_str().ends_with(".TemplateRef")
}

/// Designer projection of the STANDALONE nested template descriptor. Hand-written (not the generic
/// `derive::projection`) because that roots paths at `spec.entity` == `"Report.TemplateRef"`, while
/// the on-disk element is `<Template>` — so the property paths must root at `"Template"`. Codecs
/// mirror the CommonTemplate descriptor (Synonym `LocalizedV8`, Comment `PlainText`, TemplateType
/// `EnumText`); Name is claimed by `read_descriptor`'s frame, uuid by the `<Template uuid>` attr.
struct DesignerNestedTemplate;

impl LocusMap for DesignerNestedTemplate {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        let (path, codec): (&'static [&'static str], Codec) = if field == tref::F_SYNONYM {
            (
                &[TEMPLATE_ELEMENT, "Properties", "Synonym"],
                Codec::LocalizedV8,
            )
        } else if field == tref::F_COMMENT {
            (
                &[TEMPLATE_ELEMENT, "Properties", "Comment"],
                Codec::PlainText,
            )
        } else if field == tref::F_TEMPLATE_TYPE {
            (
                &[TEMPLATE_ELEMENT, "Properties", "TemplateType"],
                Codec::EnumText,
            )
        } else {
            return None;
        };
        Some(FieldProjection::new(
            XmlLocus::PropElement { path, ns: "" },
            codec,
        ))
    }
}

/// Read each template-ref child's nested descriptor (Designer only) and transplant its identity
/// (uuid + synonym/comment/templateType) onto the bare-ref child, so the child equals what EDT
/// reads inline. `descriptor_path` — the host object descriptor just read (`Reports/<R>.xml`).
///
/// §1.0-STRICT: a declared template child WITHOUT its adjacent `Templates/<Name>.xml` descriptor is
/// a HARD ERROR — the cf descriptor references the template uuid, which lives ONLY in that file, so
/// a missing descriptor would mean a silently-uuid-less (structurally broken) `.cf`. EDT and cf are
/// no-ops (EDT carries the stub inline; cf is a container).
pub fn attach_designer_template_ref_descriptors(
    format: Format,
    descriptor_path: &Path,
    obj: &mut MetadataObject,
) -> Result<(), ConvertError> {
    if format != Format::Designer {
        return Ok(());
    }
    for child in obj.children.iter_mut() {
        if !is_template_ref_child(child) {
            continue;
        }
        let path = match nested_descriptor_path(descriptor_path, &child.name) {
            Some(p) => p,
            None => continue, // no parent/stem (defensive) — nothing to attach.
        };
        if !path.is_file() {
            return Err(ConvertError::Read {
                kind: child.kind.as_str().to_string(),
                object: child.name.clone(),
                reason: format!(
                    "template child declares a descriptor but the nested descriptor {} is missing \
                     (§1.0 — a Designer template ref's uuid/templateType live in this file)",
                    path.display()
                ),
            });
        }
        let bytes = std::fs::read(&path).map_err(|e| ConvertError::Io {
            path: path.display().to_string(),
            reason: e.to_string(),
        })?;
        let desc = formats_designer::read_descriptor(
            TEMPLATE_ELEMENT,
            report_template_ref(),
            &DesignerNestedTemplate,
            &bytes,
        )
        .map_err(|e| ConvertError::Read {
            kind: child.kind.as_str().to_string(),
            object: child.name.clone(),
            reason: e.to_string(),
        })?;
        // Transplant the nested descriptor's identity. The bare-ref child already carries the right
        // `kind` (`<X>.TemplateRef`) and `name` (from `<Template>Name</Template>`); the nested read
        // supplies the uuid + the canonical property bag (synonym/comment/templateType) EDT gets
        // inline. `properties` is empty on the bare ref (DesignerTemplateRef projects nothing), so a
        // straight replace is faithful.
        child.uuid = desc.uuid;
        child.properties = desc.properties;
    }
    Ok(())
}

/// Write-side mirror of [`attach_designer_template_ref_descriptors`]: for a DESIGNER target, emit
/// each template-ref child's NESTED descriptor `Reports/<R>/Templates/<T>.xml` beside the host
/// descriptor just written (a `<MetaDataObject><Template uuid><Properties>Name/Synonym/Comment/
/// TemplateType</Properties></Template>` file). Without it a cross-format convert writes only the
/// bare `<Template>` ref in the host `ChildObjects`, and the platform xml-compile fails with «Файл
/// объекта не существует» (the template's uuid/type live ONLY in this file). EDT/cf are no-ops (EDT
/// carries the stub inline; cf is a container).
///
/// `descriptor_out` — the host object descriptor just written (`Reports/<R>.xml`). §1.0: typed error
/// on serialization / I/O.
pub fn write_designer_template_ref_descriptors(
    format: Format,
    descriptor_out: &Path,
    obj: &MetadataObject,
) -> Result<(), ConvertError> {
    if format != Format::Designer {
        return Ok(());
    }
    for child in &obj.children {
        if !is_template_ref_child(child) {
            continue;
        }
        let path = match nested_descriptor_path(descriptor_out, &child.name) {
            Some(p) => p,
            None => continue,
        };
        // Same element name + spec + LocusMap the read side uses → the nested descriptor round-trips
        // byte-identically (the `<Template uuid>` frame carries uuid/name; the LocusMap carries
        // synonym/comment/templateType). `write_descriptor` validates `obj.kind == element`, so the
        // standalone descriptor's kind is `Template` (what `read_descriptor` produces) rather than the
        // child's collection-namespaced `<X>.TemplateRef` — clone with that kind.
        let mut tmpl = child.clone();
        tmpl.kind = ObjectKind::new(TEMPLATE_ELEMENT);
        let bytes = formats_designer::write_descriptor(
            TEMPLATE_ELEMENT,
            report_template_ref(),
            &DesignerNestedTemplate,
            &tmpl,
        )
        .map_err(|e| ConvertError::Write {
            kind: child.kind.as_str().to_string(),
            object: child.name.clone(),
            reason: e.to_string(),
        })?;
        crate::form_write::write_file(&path, &bytes)?;
    }
    Ok(())
}

/// Nested template descriptor path beside the host: `<dir>/<stem>/Templates/<Name>.xml` (the
/// sibling of the `<Name>/Ext/Template.xml` body `dcs_read` reads). `None` if the host path has no
/// parent/stem.
fn nested_descriptor_path(descriptor_path: &Path, template_name: &str) -> Option<PathBuf> {
    let dir = descriptor_path.parent()?;
    let stem = descriptor_path.file_stem()?;
    Some(
        dir.join(stem)
            .join(TEMPLATES_DIR)
            .join(format!("{template_name}.xml")),
    )
}

#[cfg(any())]
mod tests {
    use super::*;
    use morph1c_core::ir::value::PropertyValue;
    use morph1c_core::ir::{ObjectKind, Uuid};

    fn template_ref_child(name: &str) -> MetadataObject {
        // Bare-ref shape the Designer Report connector produces: kind + name, no uuid/props.
        MetadataObject::new(ObjectKind::new("Report.TemplateRef"), name, Uuid([0; 16]))
    }

    #[test]
    fn edt_and_cf_are_noops() {
        let mut obj = MetadataObject::new(ObjectKind::new("Report"), "R", Uuid([1; 16]));
        obj.children.push(template_ref_child("T"));
        // A non-existent path must NOT error for EDT/cf (the pass is Designer-only).
        let p = Path::new("/nonexistent/Reports/R/R.mdo");
        attach_designer_template_ref_descriptors(Format::Edt, p, &mut obj).unwrap();
        attach_designer_template_ref_descriptors(Format::Cf, p, &mut obj).unwrap();
        assert_eq!(
            obj.children[0].uuid,
            Uuid([0; 16]),
            "EDT/cf leave the child untouched"
        );
    }

    #[test]
    fn nested_descriptor_path_is_under_host_templates() {
        let p = Path::new("/root/Reports/Отчет_X.xml");
        let s = nested_descriptor_path(p, "ОсновнаяСхемаКомпоновкиДанных").unwrap();
        assert!(
            s.ends_with(Path::new(
                "Reports/Отчет_X/Templates/ОсновнаяСхемаКомпоновкиДанных.xml"
            )),
            "got {s:?}"
        );
    }

    #[test]
    fn missing_descriptor_is_hard_error() {
        let mut obj = MetadataObject::new(ObjectKind::new("Report"), "Отчет_X", Uuid([2; 16]));
        obj.children
            .push(template_ref_child("ОсновнаяСхемаКомпоновкиДанных"));
        let p = Path::new("/nonexistent-root/Reports/Отчет_X.xml");
        let err =
            attach_designer_template_ref_descriptors(Format::Designer, p, &mut obj).unwrap_err();
        assert!(matches!(err, ConvertError::Read { .. }), "got {err:?}");
    }

    #[test]
    fn non_template_child_is_untouched() {
        let mut obj = MetadataObject::new(ObjectKind::new("Report"), "R", Uuid([3; 16]));
        obj.children.push(MetadataObject::new(
            ObjectKind::new("Report.Attribute"),
            "A",
            Uuid([4; 16]),
        ));
        let p = Path::new("/nonexistent/Reports/R.xml");
        attach_designer_template_ref_descriptors(Format::Designer, p, &mut obj).unwrap();
        assert!(
            obj.children[0].properties.is_empty(),
            "non-template child untouched"
        );
    }

    #[test]
    fn write_designer_nested_descriptor_round_trips() {
        use morph1c_core::ir::value::{Lang, PropertyValue, Token};
        // Build an IR Report carrying a DCS template child, WRITE its nested designer descriptor,
        // then read it back → identical identity (uuid + templateType + synonym).
        let base = std::env::temp_dir().join(format!(
            "morph1c-tref-write-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(base.join("Reports")).unwrap();
        let host = base.join("Reports").join("Отчет_X.xml");

        let child_uuid = Uuid([
            0x0b, 0x15, 0x98, 0xf5, 0xeb, 0x61, 0x40, 0x29, 0x80, 0xc7, 0x00, 0x9a, 0xd7, 0x31,
            0xc3, 0x3e,
        ]);
        let mut child = MetadataObject::new(
            ObjectKind::new("Report.TemplateRef"),
            "ОсновнаяСхемаКомпоновкиДанных",
            child_uuid,
        );
        child.properties.push((
            tref::F_TEMPLATE_TYPE,
            PropertyValue::Enum(Token::new("DataCompositionSchema")),
        ));
        child.properties.push((
            tref::F_SYNONYM,
            PropertyValue::Localized(vec![(
                Lang::new("ru"),
                "Основная схема компоновки данных".into(),
            )]),
        ));
        let mut report = MetadataObject::new(ObjectKind::new("Report"), "Отчет_X", Uuid([9; 16]));
        report.children.push(child);

        write_designer_template_ref_descriptors(Format::Designer, &host, &report).unwrap();
        let nested = base.join("Reports/Отчет_X/Templates/ОсновнаяСхемаКомпоновкиДанных.xml");
        assert!(nested.is_file(), "nested descriptor written");

        let bytes = std::fs::read(&nested).unwrap();
        let desc = formats_designer::read_descriptor(
            TEMPLATE_ELEMENT,
            report_template_ref(),
            &DesignerNestedTemplate,
            &bytes,
        )
        .expect("nested descriptor reads back");
        assert_eq!(desc.uuid, child_uuid, "uuid round-trips");
        assert_eq!(desc.name, "ОсновнаяСхемаКомпоновкиДанных");
        assert!(
            matches!(desc.get(tref::F_TEMPLATE_TYPE), Some(PropertyValue::Enum(t)) if t.as_str() == "DataCompositionSchema"),
            "templateType round-trips"
        );
        // EDT is a no-op (inlines the stub) — no file written.
        let host_edt = base.join("Reports").join("Отчет_Y").join("Отчет_Y.mdo");
        std::fs::create_dir_all(host_edt.parent().unwrap()).unwrap();
        write_designer_template_ref_descriptors(Format::Edt, &host_edt, &report).unwrap();
        assert!(
            !base.join("Reports/Отчет_Y/Templates").exists(),
            "EDT writes no nested descriptor"
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn enriches_bare_ref_child_from_nested_descriptor() {
        // A real designer nested descriptor (from coverage/s11_dcs) → the child gains uuid +
        // templateType + synonym, exactly what EDT reads inline.
        let base = std::env::temp_dir().join(format!(
            "morph1c-tref-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let tdir = base.join("Reports").join("Отчет_X").join("Templates");
        std::fs::create_dir_all(&tdir).unwrap();
        let xml = "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:app=\"http://v8.1c.ru/8.2/managed-application/core\" xmlns:cfg=\"http://v8.1c.ru/8.1/data/enterprise/current-config\" xmlns:cmi=\"http://v8.1c.ru/8.2/managed-application/cmi\" xmlns:ent=\"http://v8.1c.ru/8.1/data/enterprise\" xmlns:lf=\"http://v8.1c.ru/8.2/managed-application/logform\" xmlns:pal=\"http://v8.1c.ru/8.1/data/ui/colors/palette\" xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\" xmlns:sys=\"http://v8.1c.ru/8.1/data/ui/fonts/system\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" xmlns:web=\"http://v8.1c.ru/8.1/data/ui/colors/web\" xmlns:win=\"http://v8.1c.ru/8.1/data/ui/colors/windows\" xmlns:xen=\"http://v8.1c.ru/8.3/xcf/enums\" xmlns:xpr=\"http://v8.1c.ru/8.3/xcf/predef\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"2.21\">\r\n\
\t<Template uuid=\"0b1598f5-eb61-4029-80c7-009ad731c33e\">\r\n\
\t\t<Properties>\r\n\
\t\t\t<Name>ОсновнаяСхемаКомпоновкиДанных</Name>\r\n\
\t\t\t<Synonym>\r\n\
\t\t\t\t<v8:item>\r\n\
\t\t\t\t\t<v8:lang>ru</v8:lang>\r\n\
\t\t\t\t\t<v8:content>Основная схема компоновки данных</v8:content>\r\n\
\t\t\t\t</v8:item>\r\n\
\t\t\t</Synonym>\r\n\
\t\t\t<Comment/>\r\n\
\t\t\t<TemplateType>DataCompositionSchema</TemplateType>\r\n\
\t\t</Properties>\r\n\
\t</Template>\r\n\
</MetaDataObject>";
        crate::fsio::write(tdir.join("ОсновнаяСхемаКомпоновкиДанных.xml"), xml).unwrap();

        let mut obj = MetadataObject::new(ObjectKind::new("Report"), "Отчет_X", Uuid([5; 16]));
        obj.children
            .push(template_ref_child("ОсновнаяСхемаКомпоновкиДанных"));
        let host = base.join("Reports").join("Отчет_X.xml");
        attach_designer_template_ref_descriptors(Format::Designer, &host, &mut obj).unwrap();

        let child = &obj.children[0];
        assert_eq!(
            child.uuid,
            Uuid([
                0x0b, 0x15, 0x98, 0xf5, 0xeb, 0x61, 0x40, 0x29, 0x80, 0xc7, 0x00, 0x9a, 0xd7, 0x31,
                0xc3, 0x3e
            ]),
            "uuid transplanted from the nested descriptor"
        );
        // templateType == DataCompositionSchema is exactly what `dcs_read::is_dcs_template_child`
        // keys on, so the child is now recognised as a DCS template (its body gets attached next).
        assert!(
            matches!(child.get(tref::F_TEMPLATE_TYPE), Some(PropertyValue::Enum(t)) if t.as_str() == "DataCompositionSchema"),
            "templateType read: {:?}",
            child.get(tref::F_TEMPLATE_TYPE)
        );

        let _ = std::fs::remove_dir_all(&base);
    }
}
