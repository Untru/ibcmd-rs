//! Enrich a DESIGNER object's form-ref CHILDREN (`*.FormRef`) with their NESTED descriptor
//! during whole-config read, and re-emit that descriptor for a Designer target (§1.0/§1.6).
//! Mirror of [`crate::template_ref_read`] for the `Forms/` collection.
//!
//! # Why: Designer form children are bare-refs; EDT carries the stub inline
//! A declared form is a CHILD (`kind` ends `.FormRef`). EDT stores the stub INLINE in the host
//! `.mdo` (`<forms uuid=…><name><synonym><usePurposes>…`), so the child read from it already
//! carries uuid + synonym + usePurposes. The Designer host `.xml` carries only a BARE
//! `<Form>Имя</Form>` ref (the `DesignerFormRef` LocusMap projects no properties), and the
//! child's real identity lives in a SEPARATE `<Host>/Forms/<Имя>.xml` descriptor. Without this
//! pass a designer→edt convert writes a ZEROED-uuid, synonym-less form stub (witnessed: 18/18
//! s15 owner `.mdo` diffs); without the write mirror an edt→designer convert emits a dangling
//! `<Form>` ref (18/18 missing `Forms/<Имя>.xml` — the platform xml-compile fails).
//!
//! # Layout (RE: coverage/s15_subordinate designer)
//! Host `Catalogs/<C>.xml` (file-per-object) → nested descriptor `Catalogs/<C>/Forms/<Имя>.xml`
//! (the sibling of the `<Имя>/Ext/Form.xml` body sidecar `form_read` attaches). The nested file
//! is a standalone `<MetaDataObject><Form uuid><Properties><Name><Synonym><Comment><FormType>
//! <IncludeHelpInContents><UsePurposes><UseInInterfaceCompatibilityMode>[<ExtendedPresentation>]
//! </Properties></Form>` — the same field set for all 18 owner kinds (ExtendedPresentation only
//! for the three kinds whose FormRef spec carries `F_EXTENDED_PRESENTATION`: DataProcessor /
//! FilterCriterion / Report — witnessed 297/297 SSL + 3/3 s15).
//!
//! # FieldId layout is UNIFORM across all `<Kind>.FormRef` specs
//! synonym=1, comment=2, includeHelpInContents=3, help=4, usePurposes=5,
//! [extendedPresentation=6,] formType=7, useInInterfaceCompatibilityMode=8 — so ONE LocusMap
//! serves every owner kind; the spec itself comes from the CHILD's own kind
//! (`spec::registry::spec_for`), which also keys the `usePurposes` VALUE SHAPE: a presence-Bool
//! constant (`Codec::FormUsePurposesV8Const` ↔ EDT `UsePurposesConst`) vs a VARIABLE canonical
//! list (`Codec::UsePurposesV8` ↔ EDT `RefList`) for the kinds whose spec models the field as
//! `List` — SettingsStorage + Catalog/DataProcessor/InformationRegister (ERP witnesses
//! single-purpose `[PersonalComputer]` stubs on all three).
//!
//! # Scope
//! Designer only (EDT reads/writes the stub inline; cf is a container). No-op for objects
//! without form children. The form BODY (`Ext/Form.xml` + `Form/Module.bsl`) is owned by
//! [`crate::form_read`]/[`crate::form_write`] — this pass covers ONLY the nested descriptor.

use std::path::{Path, PathBuf};

use formats_xml::registry::Format;
use formats_xml::{Codec, FieldProjection, LocusMap, XmlLocus};
use morph1c_core::ir::value::ValueKind;
use morph1c_core::ir::{FieldId, MetadataObject, ObjectKind};
use morph1c_core::spec::metadata::catalog_form_ref as fref;
use morph1c_core::spec::metadata::data_processor_form_ref::F_EXTENDED_PRESENTATION;
use morph1c_core::spec::registry::spec_for;

use crate::ConvertError;

/// The `Forms` collection subdir hosting each form's nested descriptor + body dir.
const FORMS_DIR: &str = "Forms";
/// The wrapper element of a NESTED form descriptor under `<MetaDataObject>`.
const FORM_ELEMENT: &str = "Form";

/// Is `child` a form-ref child (`kind` ends `.FormRef`)?
fn is_form_ref_child(child: &MetadataObject) -> bool {
    child.kind.as_str().ends_with(".FormRef")
}

/// Designer projection of the STANDALONE nested form descriptor. Hand-written (paths root at
/// the on-disk `<Form>` element, not the spec entity). `use_purposes_is_list` keys the
/// `usePurposes` codec by the owning spec's value shape; `extended_presentation` — whether
/// the owner kind's dense set includes `<ExtendedPresentation>` (see module docs).
struct DesignerNestedForm {
    use_purposes_is_list: bool,
    extended_presentation: bool,
}

impl LocusMap for DesignerNestedForm {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        let (path, codec): (&'static [&'static str], Codec) = if field == fref::F_SYNONYM {
            (&[FORM_ELEMENT, "Properties", "Synonym"], Codec::LocalizedV8)
        } else if field == fref::F_COMMENT {
            (&[FORM_ELEMENT, "Properties", "Comment"], Codec::PlainText)
        } else if field == fref::F_INCLUDE_HELP_IN_CONTENTS {
            (
                &[FORM_ELEMENT, "Properties", "IncludeHelpInContents"],
                Codec::BoolText,
            )
        } else if field == fref::F_USE_PURPOSES {
            let codec = if self.use_purposes_is_list {
                Codec::UsePurposesV8
            } else {
                Codec::FormUsePurposesV8Const
            };
            (&[FORM_ELEMENT, "Properties", "UsePurposes"], codec)
        } else if field == F_EXTENDED_PRESENTATION && self.extended_presentation {
            (
                &[FORM_ELEMENT, "Properties", "ExtendedPresentation"],
                Codec::LocalizedV8,
            )
        } else if field == fref::F_FORM_TYPE {
            (&[FORM_ELEMENT, "Properties", "FormType"], Codec::EnumText)
        } else if field == fref::F_USE_IN_INTERFACE_COMPATIBILITY_MODE {
            (
                &[
                    FORM_ELEMENT,
                    "Properties",
                    "UseInInterfaceCompatibilityMode",
                ],
                Codec::EnumText,
            )
        } else {
            // `help` (form-level EDT `<help>` const-block) has NO nested-descriptor analogue
            // (Designer form help is an `Ext/Help.xml` sidecar of the form body dir).
            return None;
        };
        Some(FieldProjection::new(
            XmlLocus::PropElement { path, ns: "" },
            codec,
        ))
    }
    fn field_emit_order(&self) -> Option<&'static [FieldId]> {
        Some(NESTED_FORM_ORDER)
    }
}

/// Witnessed Designer nested-descriptor property order (after `<Name>`).
static NESTED_FORM_ORDER: &[FieldId] = &[
    fref::F_SYNONYM,
    fref::F_COMMENT,
    fref::F_FORM_TYPE,
    fref::F_INCLUDE_HELP_IN_CONTENTS,
    fref::F_USE_PURPOSES,
    fref::F_USE_IN_INTERFACE_COMPATIBILITY_MODE,
    F_EXTENDED_PRESENTATION,
];

/// Owner kinds whose Designer nested form descriptor carries `<ExtendedPresentation>` in its
/// dense property set (WITNESSED: 2/2 s15 + 297/297 SSL for these two kinds; s15's
/// FilterCriterion form does NOT carry it despite the FilterCriterion.FormRef spec having the
/// field — the Designer dense set is kind-specific, not spec-driven).
const EXTENDED_PRESENTATION_KINDS: &[&str] = &["DataProcessor.FormRef", "Report.FormRef"];

/// The child's own FormRef spec + its dialect-map flags (see [`DesignerNestedForm`]).
fn form_ref_spec(
    child: &MetadataObject,
) -> Result<
    (
        &'static morph1c_core::spec::common::EntitySpec,
        DesignerNestedForm,
    ),
    ConvertError,
> {
    let kind = child.kind.as_str();
    let spec = spec_for(kind).ok_or_else(|| ConvertError::Read {
        kind: kind.to_string(),
        object: child.name.clone(),
        reason: "no registered FormRef spec for this kind (§1.0)".into(),
    })?;
    let is_list = spec
        .fields()
        .iter()
        .find(|f| f.id == fref::F_USE_PURPOSES)
        .map(|f| f.value_kind == ValueKind::List)
        .unwrap_or(false);
    Ok((
        spec,
        DesignerNestedForm {
            use_purposes_is_list: is_list,
            extended_presentation: EXTENDED_PRESENTATION_KINDS.contains(&kind),
        },
    ))
}

/// Read each form-ref child's nested descriptor (Designer only) and transplant its identity
/// (uuid + synonym/comment/includeHelpInContents/usePurposes/formType/…) onto the bare-ref
/// child, so the child equals what EDT reads inline. `descriptor_path` — the host object
/// descriptor just read (`Catalogs/<C>.xml`).
///
/// §1.0-STRICT: a declared form child WITHOUT its adjacent `Forms/<Имя>.xml` descriptor is a
/// HARD ERROR — the form's uuid lives ONLY in that file, so a missing descriptor would mean a
/// silently-uuid-less stub in every other dialect. EDT and cf are no-ops.
pub fn attach_designer_form_ref_descriptors(
    format: Format,
    descriptor_path: &Path,
    obj: &mut MetadataObject,
) -> Result<(), ConvertError> {
    if format != Format::Designer {
        return Ok(());
    }
    for child in obj.children.iter_mut() {
        if !is_form_ref_child(child) {
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
                    "form child declares a descriptor but the nested descriptor {} is missing \
                     (§1.0 — a Designer form ref's uuid/synonym live in this file)",
                    path.display()
                ),
            });
        }
        let bytes = std::fs::read(&path).map_err(|e| ConvertError::Io {
            path: path.display().to_string(),
            reason: e.to_string(),
        })?;
        let (spec, map) = form_ref_spec(child)?;
        let desc =
            formats_designer::read_descriptor(FORM_ELEMENT, spec, &map, &bytes).map_err(|e| {
                ConvertError::Read {
                    kind: child.kind.as_str().to_string(),
                    object: child.name.clone(),
                    reason: format!("{} : {e}", path.display()),
                }
            })?;
        // §1.0 membership: the nested descriptor's own <Name> must equal the declared ref.
        if desc.name != child.name {
            return Err(ConvertError::Read {
                kind: child.kind.as_str().to_string(),
                object: child.name.clone(),
                reason: format!(
                    "nested descriptor {} carries <Name>{}</Name> != declared ref {:?} (§1.0)",
                    path.display(),
                    desc.name,
                    child.name
                ),
            });
        }
        // Transplant the nested descriptor's identity. The bare-ref child already carries the
        // right `kind`/`name`; `properties` is empty on the bare ref (DesignerFormRef projects
        // nothing), so a straight replace is faithful.
        child.uuid = desc.uuid;
        child.properties = desc.properties;
    }
    Ok(())
}

/// Write-side mirror of [`attach_designer_form_ref_descriptors`]: for a DESIGNER target, emit
/// each form-ref child's NESTED descriptor `<Host>/Forms/<Имя>.xml` beside the host descriptor
/// just written. Without it a cross-format convert writes only the bare `<Form>` ref in the
/// host `ChildObjects` and the platform xml-compile fails («Файл объекта не существует»).
/// EDT/cf are no-ops.
pub fn write_designer_form_ref_descriptors(
    format: Format,
    descriptor_out: &Path,
    obj: &MetadataObject,
) -> Result<(), ConvertError> {
    if format != Format::Designer {
        return Ok(());
    }
    for child in &obj.children {
        if !is_form_ref_child(child) {
            continue;
        }
        let path = match nested_descriptor_path(descriptor_out, &child.name) {
            Some(p) => p,
            None => continue,
        };
        let (spec, map) = form_ref_spec(child)?;
        // §1.0 write honesty: a NON-DEFAULT extendedPresentation on a kind whose Designer
        // dense set does not carry the element would be dropped silently by the unmapped
        // projection — refuse loudly instead.
        if !map.extended_presentation {
            if let Some(morph1c_core::ir::value::PropertyValue::Localized(v)) =
                child.get(F_EXTENDED_PRESENTATION)
            {
                if !v.is_empty() {
                    return Err(ConvertError::Write {
                        kind: child.kind.as_str().to_string(),
                        object: child.name.clone(),
                        reason: "extendedPresentation has no witnessed Designer nested-form \
                                 encoding for this kind (§1.0 — refusing a silent drop)"
                            .into(),
                    });
                }
            }
        }
        // Same element + spec + LocusMap the read side uses → the nested descriptor
        // round-trips byte-identically. `write_descriptor` validates `obj.kind == element`,
        // so clone with the standalone `Form` kind.
        let mut form = child.clone();
        form.kind = ObjectKind::new(FORM_ELEMENT);
        let bytes =
            formats_designer::write_descriptor(FORM_ELEMENT, spec, &map, &form).map_err(|e| {
                ConvertError::Write {
                    kind: child.kind.as_str().to_string(),
                    object: child.name.clone(),
                    reason: e.to_string(),
                }
            })?;
        crate::form_write::write_file(&path, &bytes)?;
    }
    Ok(())
}

/// Nested form descriptor path beside the host: `<dir>/<stem>/Forms/<Имя>.xml` (the sibling of
/// the `<Имя>/Ext/Form.xml` body `form_read` reads). `None` if the host path has no
/// parent/stem.
fn nested_descriptor_path(descriptor_path: &Path, form_name: &str) -> Option<PathBuf> {
    let dir = descriptor_path.parent()?;
    let stem = descriptor_path.file_stem()?;
    Some(
        dir.join(stem)
            .join(FORMS_DIR)
            .join(format!("{form_name}.xml")),
    )
}

#[cfg(any())]
mod tests {
    use super::*;
    use morph1c_core::ir::value::{Lang, PropertyValue};
    use morph1c_core::ir::{ObjectKind, Uuid};

    fn form_ref_child(kind: &str, name: &str) -> MetadataObject {
        // Bare-ref shape the Designer owner connector produces: kind + name, no uuid/props.
        MetadataObject::new(ObjectKind::new(kind), name, Uuid([0; 16]))
    }

    /// Fresh temp dir per test.
    fn temp_base(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "morph1c-formref-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ))
    }

    /// Witnessed nested form descriptor (coverage/s15 Catalog, byte-conventions intact).
    fn witnessed_nested_form() -> String {
        let mut s = String::new();
        s.push('\u{feff}');
        s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n");
        s.push_str("<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:app=\"http://v8.1c.ru/8.2/managed-application/core\" xmlns:cfg=\"http://v8.1c.ru/8.1/data/enterprise/current-config\" xmlns:cmi=\"http://v8.1c.ru/8.2/managed-application/cmi\" xmlns:ent=\"http://v8.1c.ru/8.1/data/enterprise\" xmlns:lf=\"http://v8.1c.ru/8.2/managed-application/logform\" xmlns:pal=\"http://v8.1c.ru/8.1/data/ui/colors/palette\" xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\" xmlns:sys=\"http://v8.1c.ru/8.1/data/ui/fonts/system\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" xmlns:web=\"http://v8.1c.ru/8.1/data/ui/colors/web\" xmlns:win=\"http://v8.1c.ru/8.1/data/ui/colors/windows\" xmlns:xen=\"http://v8.1c.ru/8.3/xcf/enums\" xmlns:xpr=\"http://v8.1c.ru/8.3/xcf/predef\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"2.21\">\r\n");
        s.push_str("\t<Form uuid=\"851aa60e-6ed5-4ce8-8e43-8d9b9c37c0c8\">\r\n");
        s.push_str("\t\t<Properties>\r\n");
        s.push_str("\t\t\t<Name>Форма</Name>\r\n");
        s.push_str("\t\t\t<Synonym>\r\n");
        s.push_str("\t\t\t\t<v8:item>\r\n");
        s.push_str("\t\t\t\t\t<v8:lang>ru</v8:lang>\r\n");
        s.push_str("\t\t\t\t\t<v8:content>Форма объекта</v8:content>\r\n");
        s.push_str("\t\t\t\t</v8:item>\r\n");
        s.push_str("\t\t\t</Synonym>\r\n");
        s.push_str("\t\t\t<Comment/>\r\n");
        s.push_str("\t\t\t<FormType>Managed</FormType>\r\n");
        s.push_str("\t\t\t<IncludeHelpInContents>false</IncludeHelpInContents>\r\n");
        s.push_str("\t\t\t<UsePurposes>\r\n");
        s.push_str("\t\t\t\t<v8:Value xsi:type=\"app:ApplicationUsePurpose\">PlatformApplication</v8:Value>\r\n");
        s.push_str("\t\t\t\t<v8:Value xsi:type=\"app:ApplicationUsePurpose\">MobilePlatformApplication</v8:Value>\r\n");
        s.push_str("\t\t\t</UsePurposes>\r\n");
        s.push_str(
            "\t\t\t<UseInInterfaceCompatibilityMode>Any</UseInInterfaceCompatibilityMode>\r\n",
        );
        s.push_str("\t\t</Properties>\r\n");
        s.push_str("\t</Form>\r\n");
        s.push_str("</MetaDataObject>");
        s
    }

    #[test]
    fn edt_and_cf_are_noops() {
        let mut obj = MetadataObject::new(ObjectKind::new("Catalog"), "Спр", Uuid([1; 16]));
        obj.children
            .push(form_ref_child("Catalog.FormRef", "Форма"));
        let p = Path::new("/nonexistent/Catalogs/Спр.mdo");
        attach_designer_form_ref_descriptors(Format::Edt, p, &mut obj).unwrap();
        attach_designer_form_ref_descriptors(Format::Cf, p, &mut obj).unwrap();
        assert_eq!(
            obj.children[0].uuid,
            Uuid([0; 16]),
            "EDT/cf leave the child untouched"
        );
    }

    #[test]
    fn missing_descriptor_is_hard_error() {
        let mut obj = MetadataObject::new(ObjectKind::new("Catalog"), "Спр", Uuid([2; 16]));
        obj.children
            .push(form_ref_child("Catalog.FormRef", "Форма"));
        let p = Path::new("/nonexistent-root/Catalogs/Спр.xml");
        let err = attach_designer_form_ref_descriptors(Format::Designer, p, &mut obj).unwrap_err();
        assert!(matches!(err, ConvertError::Read { .. }), "got {err:?}");
    }

    #[test]
    fn enriches_bare_ref_and_write_round_trips_byte_exact() {
        // Witnessed designer nested descriptor → the child gains uuid + synonym + the
        // presence-Bool usePurposes (exactly what EDT reads inline); re-emitting it from the
        // enriched child reproduces the SOURCE BYTES (fixture byte-conventions).
        let base = temp_base("roundtrip");
        let fdir = base.join("Catalogs").join("Спр").join("Forms");
        std::fs::create_dir_all(&fdir).unwrap();
        let src = witnessed_nested_form();
        crate::fsio::write(fdir.join("Форма.xml"), src.as_bytes()).unwrap();

        let mut obj = MetadataObject::new(ObjectKind::new("Catalog"), "Спр", Uuid([5; 16]));
        obj.children
            .push(form_ref_child("Catalog.FormRef", "Форма"));
        let host = base.join("Catalogs").join("Спр.xml");
        attach_designer_form_ref_descriptors(Format::Designer, &host, &mut obj).unwrap();

        let child = &obj.children[0];
        assert_eq!(
            child.uuid,
            Uuid([
                0x85, 0x1a, 0xa6, 0x0e, 0x6e, 0xd5, 0x4c, 0xe8, 0x8e, 0x43, 0x8d, 0x9b, 0x9c, 0x37,
                0xc0, 0xc8
            ]),
            "uuid transplanted from the nested descriptor"
        );
        // Catalog.FormRef моделирует usePurposes СПИСКОМ (ERP несёт single-purpose стабы),
        // Designer-спеллинг канонизируется в EDT-литералы.
        assert!(
            matches!(child.get(fref::F_USE_PURPOSES), Some(PropertyValue::List(v))
                if v == &vec![
                    PropertyValue::Str("PersonalComputer".to_string()),
                    PropertyValue::Str("MobileDevice".to_string()),
                ]),
            "usePurposes canonical list: {:?}",
            child.get(fref::F_USE_PURPOSES)
        );
        assert!(
            matches!(child.get(fref::F_SYNONYM), Some(PropertyValue::Localized(v))
                if v == &vec![(Lang::new("ru"), "Форма объекта".to_string())]),
            "synonym transplanted: {:?}",
            child.get(fref::F_SYNONYM)
        );

        // Write mirror → byte-exact against the witnessed source.
        let out_host = base.join("out").join("Catalogs").join("Спр.xml");
        std::fs::create_dir_all(out_host.parent().unwrap()).unwrap();
        write_designer_form_ref_descriptors(Format::Designer, &out_host, &obj).unwrap();
        let written = std::fs::read(base.join("out/Catalogs/Спр/Forms/Форма.xml")).unwrap();
        assert_eq!(
            written,
            src.as_bytes(),
            "nested form descriptor round-trips byte-exactly"
        );

        // EDT is a no-op (inlines the stub) — no file written.
        let host_edt = base.join("Catalogs").join("Спр2").join("Спр2.mdo");
        std::fs::create_dir_all(host_edt.parent().unwrap()).unwrap();
        write_designer_form_ref_descriptors(Format::Edt, &host_edt, &obj).unwrap();
        assert!(
            !base.join("Catalogs/Спр2/Forms").exists(),
            "EDT writes no nested descriptor"
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn non_form_child_is_untouched() {
        let mut obj = MetadataObject::new(ObjectKind::new("Catalog"), "Спр", Uuid([3; 16]));
        obj.children.push(MetadataObject::new(
            ObjectKind::new("Catalog.Attribute"),
            "А",
            Uuid([4; 16]),
        ));
        let p = Path::new("/nonexistent/Catalogs/Спр.xml");
        attach_designer_form_ref_descriptors(Format::Designer, p, &mut obj).unwrap();
        assert!(
            obj.children[0].properties.is_empty(),
            "non-form child untouched"
        );
    }
}
