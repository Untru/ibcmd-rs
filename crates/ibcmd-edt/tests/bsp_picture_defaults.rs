use std::collections::BTreeMap;

use formats_xml::form::{FormDialect, read_form, resolve_common_picture_transparency, write_form};
use morph1c_core::ir::{FormBody, FormCommand, PropertyValue};
use morph1c_core::spec::forms::command as fc;
use morph1c_core::version::{FormatVersion, with_roundtrip_target, with_source_version};

fn picture(reference: &str, flag: bool) -> PropertyValue {
    PropertyValue::List(vec![
        PropertyValue::Ref(reference.into()),
        PropertyValue::Bool(flag),
    ])
}
fn body(reference: &str, flag: bool) -> FormBody {
    let mut body = FormBody::new();
    let mut command = FormCommand::new("Command", 1);
    command
        .properties
        .push((fc::F_PICTURE, picture(reference, flag)));
    body.commands.push(command);
    body
}
fn render(body: &FormBody, dialect: FormDialect) -> Result<Vec<u8>, formats_xml::form::FormError> {
    with_roundtrip_target(FormatVersion::new(2, 20), || write_form(dialect, body))
}

#[test]
fn edt_bool_uses_typed_metadata_without_deriving_per_use_pixel() {
    for flag in [false, true] {
        let mut body = body("CommonPicture.Image", false);
        let metadata = BTreeMap::from([("CommonPicture.Image".into(), flag)]);
        resolve_common_picture_transparency(&mut body, &metadata, true).unwrap();
        assert_eq!(
            body.commands[0].get(fc::F_PICTURE),
            Some(&picture("CommonPicture.Image", flag))
        );
        let native = String::from_utf8(render(&body, FormDialect::Designer).unwrap()).unwrap();
        assert!(native.contains(&format!("<xr:LoadTransparent>{flag}</xr:LoadTransparent>")));
        assert!(!native.contains("TransparentPixel"));
        let edt = render(&body, FormDialect::Edt).unwrap();
        let mut read = with_source_version(Some(FormatVersion::new(2, 20)), || {
            read_form(FormDialect::Edt, &edt)
        })
        .unwrap();
        resolve_common_picture_transparency(&mut read, &metadata, true).unwrap();
        assert_eq!(
            read.commands[0].get(fc::F_PICTURE),
            body.commands[0].get(fc::F_PICTURE)
        );
        assert!(
            !serde_json::to_string(&body)
                .unwrap()
                .contains("common_picture_transparency")
        );
    }
}

#[test]
fn native_per_use_flags_and_pixels_stay_independent_and_refuse_lossy_edt() {
    let metadata = BTreeMap::from([("CommonPicture.Image".into(), false)]);
    let mut original = body("CommonPicture.Image", true);
    let typed_before = serde_json::to_vec(&original).unwrap();
    resolve_common_picture_transparency(&mut original, &metadata, false).unwrap();
    assert_eq!(serde_json::to_vec(&original).unwrap(), typed_before);
    assert!(
        render(&original, FormDialect::Edt)
            .unwrap_err()
            .to_string()
            .contains("per-use")
    );
    original.commands[0].properties[0].1 = PropertyValue::List(vec![
        PropertyValue::Ref("CommonPicture.Image".into()),
        PropertyValue::Bool(true),
        PropertyValue::List(vec![PropertyValue::Int(12), PropertyValue::Int(12)]),
    ]);
    resolve_common_picture_transparency(
        &mut original,
        &BTreeMap::from([("CommonPicture.Image".into(), true)]),
        false,
    )
    .unwrap();
    let native = String::from_utf8(render(&original, FormDialect::Designer).unwrap()).unwrap();
    assert!(native.contains("<xr:TransparentPixel x=\"12\" y=\"12\"/>"));
    assert!(render(&original, FormDialect::Edt).is_err());
}

#[test]
fn missing_reference_rejects_and_edited_metadata_changes_actual_bool() {
    let mut body = body("CommonPicture.Image", false);
    assert!(
        resolve_common_picture_transparency(&mut body, &BTreeMap::new(), true)
            .unwrap_err()
            .to_string()
            .contains("unresolved")
    );
    resolve_common_picture_transparency(
        &mut body,
        &BTreeMap::from([("CommonPicture.Image".into(), true)]),
        true,
    )
    .unwrap();
    let before = serde_json::to_vec(&body).unwrap();
    resolve_common_picture_transparency(
        &mut body,
        &BTreeMap::from([("CommonPicture.Image".into(), false)]),
        true,
    )
    .unwrap();
    assert_ne!(serde_json::to_vec(&body).unwrap(), before);
    // A context from a previous form may not leak into a later unrelated write.
    assert!(render(&body, FormDialect::Edt).is_ok());
    let unrelated = self::body("CommonPicture.Image", true);
    assert!(render(&unrelated, FormDialect::Edt).is_err());
    let standard = self::body("StdPicture.Refresh", true);
    assert!(render(&standard, FormDialect::Edt).is_ok());
}

#[test]
#[ignore = "Read-only genuine BSP83 metadata/original/native SDK paired witnesses on F"]
fn genuine_original_and_sdk_true_references_match_metadata_carrier() {
    use std::path::Path;
    let edt =
        Path::new(r"F:\ibcmd\lab\07\oracle-bsp83-r1\authentic-workspace\OracleConfiguration\src");
    let source = Path::new(
        r"F:\ibcmd\lab\parity\ibcmd_rs_bsp_8327_native_20260919_20260921_export_recheck_2\native",
    );
    let sdk = Path::new(r"F:\ibcmd\lab\07\native-reference-bsp83-r2\native-xml");
    let reference = "CommonPicture.ВыполнитьРегламентноеЗаданиеВручную";
    let metadata = BTreeMap::from([(reference.into(), true)]);
    let descriptor = std::fs::read_to_string(edt.join("CommonPictures/ВыполнитьРегламентноеЗаданиеВручную/ВыполнитьРегламентноеЗаданиеВручную.mdo")).unwrap();
    assert!(descriptor.contains("<transparentPixel>"));
    assert!(descriptor.contains("<x>14</x>"));
    assert!(descriptor.contains("<y>8</y>"));
    for relative in [
        "CommonForms/Расширения",
        "DataProcessors/РегламентныеИФоновыеЗадания/Forms/РегламентныеИФоновыеЗадания",
    ] {
        for tree in [source, sdk] {
            let native = std::fs::read(tree.join(relative).join("Ext/Form.xml")).unwrap();
            let body = with_source_version(Some(FormatVersion::new(2, 20)), || {
                read_form(FormDialect::Designer, &native)
            })
            .unwrap();
            assert!(
                body.commands
                    .iter()
                    .any(|command| command.get(fc::F_PICTURE) == Some(&picture(reference, true)))
            );
        }
        let bytes = std::fs::read(edt.join(relative).join("Form.form")).unwrap();
        let mut body = with_source_version(Some(FormatVersion::new(2, 20)), || {
            read_form(FormDialect::Edt, &bytes)
        })
        .unwrap();
        let projection_before_binding = render(&body, FormDialect::Edt).unwrap();
        // These forms also refer to other declared CommonPictures; collect their
        // actual typed descriptor presence without guessing from the ref spelling.
        let mut all = metadata.clone();
        for entry in std::fs::read_dir(edt.join("CommonPictures")).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name().into_string().unwrap();
            let text = std::fs::read_to_string(entry.path().join(format!("{name}.mdo"))).unwrap();
            all.insert(
                format!("CommonPicture.{name}"),
                text.contains("<transparentPixel>"),
            );
        }
        resolve_common_picture_transparency(&mut body, &all, true).unwrap();
        assert!(
            body.commands
                .iter()
                .any(|command| command.get(fc::F_PICTURE) == Some(&picture(reference, true)))
        );
        assert!(
            render(&body, FormDialect::Edt).unwrap() == projection_before_binding,
            "metadata BOOL binding must not alter EDT reference serialization"
        );
    }
}
