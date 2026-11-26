use formats_xml::form::{FormDialect, read_form, write_form};
use ibcmd_edt::{
    ConversionOptions, Project, ReaderLimits, edt_to_xml, read_xml_source, xml_to_edt,
};
use ibcmd_xml::source_tree::{SourceEntry, SourceTree};
use morph1c_core::ir::{
    FormBody, MetadataObject, NamedFormBody, ObjectKind, PropertyValue, Token, Uuid,
};
use morph1c_core::version::{FormatVersion, with_roundtrip_target, with_source_version};
use morph1c_pipeline::{ConvertOptions, Format, read_config, write_config};
use sha2::{Digest, Sha256};

fn exact_bytes(actual: &[u8], expected: &[u8]) {
    assert!(
        actual == expected,
        "native byte mismatch: first={:?}, lengths={}/{}, sha256={:x}/{:x}",
        actual.iter().zip(expected).position(|(a, b)| a != b),
        actual.len(),
        expected.len(),
        Sha256::digest(actual),
        Sha256::digest(expected)
    );
}

fn read(dialect: FormDialect, bytes: &[u8], minor: u16) -> FormBody {
    let bytes = if bytes.starts_with(b"<form:Form") {
        [
            b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n".as_slice(),
            bytes,
            b"\r\n",
        ]
        .concat()
    } else {
        bytes.to_vec()
    };
    with_source_version(Some(FormatVersion::new(2, minor)), || {
        read_form(dialect, &bytes)
    })
    .unwrap()
}
fn write(dialect: FormDialect, body: &FormBody, minor: u16) -> Vec<u8> {
    with_roundtrip_target(FormatVersion::new(2, minor), || write_form(dialect, body)).unwrap()
}
fn fixture(minor: u16) -> FormBody {
    read(FormDialect::Edt, br#"<form:Form xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:form="http://g5.1c.ru/v8/dt/form" xmlns:core="http://g5.1c.ru/v8/dt/mcore"><items xsi:type="form:FormField"><name>Input</name><id>1</id><type>InputField</type><extInfo xsi:type="form:InputFieldExtInfo"><choiceParameters><name>Filter.Test</name><value xsi:type="form:FormChoiceListDesTimeValue"><value xsi:type="core:NumberValue"><value>13</value></value></value></choiceParameters></extInfo></items></form:Form>"#, minor)
}
#[test]
fn native_null_picture_is_versioned_and_current_value_is_retained() {
    for minor in [20, 21] {
        let body = fixture(minor);
        let native = write(FormDialect::Designer, &body, minor);
        let text = String::from_utf8(native.clone()).unwrap();
        assert_eq!(text.matches("<Picture/>").count(), 0);
        let authored = text.replace(
            "\t\t\t\t\t</app:value>",
            "\t\t\t\t\t\t<Picture/>\r\n\t\t\t\t\t</app:value>",
        );
        assert_ne!(authored, text);
        let authored_body = read(FormDialect::Designer, authored.as_bytes(), minor);
        exact_bytes(
            &write(FormDialect::Designer, &authored_body, minor),
            authored.as_bytes(),
        );
        let current = read(FormDialect::Designer, &native, minor);
        assert_eq!(
            serde_json::to_vec(&current).unwrap(),
            serde_json::to_vec(&body).unwrap()
        );
        assert_eq!(write(FormDialect::Designer, &current, minor), native);
        let changed = text.replace(">13<", ">17<");
        assert_ne!(changed, text);
        let current = read(FormDialect::Designer, changed.as_bytes(), minor);
        assert!(
            String::from_utf8(write(FormDialect::Edt, &current, minor))
                .unwrap()
                .contains(">17<")
        );
        let target_other = write(
            FormDialect::Designer,
            &current,
            if minor == 20 { 21 } else { 20 },
        );
        assert_eq!(
            String::from_utf8(target_other)
                .unwrap()
                .matches("<Picture/>")
                .count(),
            0
        );
    }
}
#[test]
fn fixed_array_members_keep_order_values_and_their_nullable_slot() {
    let mut body = fixture(21);
    let id = morph1c_core::spec::forms::controls::form_field::F_EXT_CHOICE_PARAMETERS;
    let PropertyValue::List(params) = body.items[0]
        .ext_info
        .iter_mut()
        .find(|(field, _)| *field == id)
        .map(|(_, v)| v)
        .unwrap()
    else {
        panic!()
    };
    let PropertyValue::List(pair) = &mut params[0] else {
        panic!()
    };
    pair[1] = PropertyValue::List(vec![pair[1].clone(), pair[1].clone()]);
    for minor in [20, 21] {
        let native = write(FormDialect::Designer, &body, minor);
        assert_eq!(
            String::from_utf8_lossy(&native)
                .matches("<Picture/>")
                .count(),
            0
        );
        let current = read(FormDialect::Designer, &native, minor);
        assert_eq!(
            serde_json::to_vec(&current).unwrap(),
            serde_json::to_vec(&body).unwrap()
        );
        let edt = write(FormDialect::Edt, &current, minor);
        assert_eq!(
            serde_json::to_vec(&read(FormDialect::Edt, &edt, minor)).unwrap(),
            serde_json::to_vec(&current).unwrap()
        );
        let invalid = String::from_utf8(native).unwrap().replacen(
            "</app:value>",
            "<Picture/><Picture/></app:value>",
            1,
        );
        if minor == 21 {
            assert!(
                with_source_version(Some(FormatVersion::new(2, minor)), || read_form(
                    FormDialect::Designer,
                    invalid.as_bytes()
                ))
                .is_err()
            );
        }
    }
}

#[test]
fn nullable_picture_is_closed_and_required_value_is_still_required() {
    let native = String::from_utf8(write(FormDialect::Designer, &fixture(21), 21))
        .unwrap()
        .replace("</app:value>", "<Picture/></app:value>");
    for altered in [
        native.replace("<Picture/>", "<Picture unknown=\"true\"/>"),
        native.replace("<Picture/>", "<Picture/><Picture/>"),
        native.replace("<Picture/>", "<Picture>unknown</Picture>"),
        native.replace("<Picture/>", "<Picture><Unknown/></Picture>"),
        native.replace("<Picture/>", "<xr:Picture/>"),
        native.replace("<Picture/>", "<Picture xmlns=\"urn:unknown\"/>"),
        native.replace("<Value xsi:type=\"xs:decimal\">13</Value>", ""),
    ] {
        assert_ne!(altered, native);
        assert!(
            with_source_version(Some(FormatVersion::new(2, 21)), || read_form(
                FormDialect::Designer,
                altered.as_bytes()
            ))
            .is_err()
        );
    }
}
#[test]
fn public_same_source_strip_current_edit_and_forged_hash_are_safe() {
    for minor in [20, 21] {
        let options = ConversionOptions {
            edt_version: "2025.2.3".into(),
            xml_dialect: format!("2.{minor}"),
            runtime_version: Some(if minor == 20 { "8.3.27" } else { "8.5.1" }.into()),
        };
        let fixture_path = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/subsystem-ci/src"
        ));
        let mut cfg = read_config(Format::Designer, fixture_path, &ConvertOptions::default())
            .unwrap()
            .0;
        let mut obj =
            MetadataObject::new(ObjectKind::new("CommonForm"), "ChoiceNull", Uuid([76; 16]));
        let id = morph1c_core::spec::registry::spec_for("CommonForm")
            .unwrap()
            .fields()
            .iter()
            .find(|f| f.name == "formType")
            .unwrap()
            .id;
        obj.properties
            .push((id, PropertyValue::Enum(Token::new("Managed"))));
        obj.form_bodies.push(NamedFormBody {
            name: "ChoiceNull".into(),
            body: fixture(minor),
            ordinary_body: None,
            module: None,
            help: vec![],
            help_resources: vec![],
        });
        cfg.objects.push(obj);
        let dir = tempfile::tempdir().unwrap();
        with_roundtrip_target(FormatVersion::new(2, minor), || {
            write_config(Format::Designer, &cfg, dir.path())
        })
        .unwrap();
        let path = dir.path().join("Configuration.xml");
        let root = String::from_utf8(std::fs::read(&path).unwrap())
            .unwrap()
            .replace(
                "</Language>",
                "</Language>\r\n\t\t\t<CommonForm>ChoiceNull</CommonForm>",
            );
        std::fs::write(path, root).unwrap();
        {
            let path = dir.path().join("CommonForms/ChoiceNull/Ext/Form.xml");
            let bytes = String::from_utf8(std::fs::read(&path).unwrap()).unwrap();
            let omitted = bytes.replace("<Presentation/>", "");
            let omitted = if minor == 21 {
                omitted.replace("<Picture/>", "")
            } else {
                omitted
            };
            assert_ne!(omitted, bytes);
            std::fs::write(path, omitted).unwrap();
        }
        let original = read_xml_source(dir.path(), ReaderLimits::default()).unwrap();
        let generated = xml_to_edt(&original, &options).unwrap().tree;
        assert_eq!(
            edt_to_xml(&Project::from_tree(generated.clone()).unwrap(), &options)
                .unwrap()
                .tree,
            original
        );
        let form = "src/CommonForms/ChoiceNull/Form.form";
        let raw = generated
            .entries()
            .iter()
            .find(|e| e.path().as_str() == form)
            .unwrap()
            .bytes();
        let changed = String::from_utf8(raw.to_vec())
            .unwrap()
            .replace(">13<", ">17<")
            .into_bytes();
        assert_ne!(changed, raw);
        let mut manifest: serde_json::Value = serde_json::from_slice(
            generated
                .entries()
                .iter()
                .find(|e| e.path().as_str() == ".ibcmd-provenance/manifest.json")
                .unwrap()
                .bytes(),
        )
        .unwrap();
        manifest["generated"][form] = format!("{:x}", Sha256::digest(&changed)).into();
        let alter = |strip: bool| {
            SourceTree::new(
                generated
                    .entries()
                    .iter()
                    .filter(|e| !strip || !e.path().as_str().starts_with(".ibcmd-provenance/"))
                    .map(|e| {
                        SourceEntry::from_bytes(
                            e.path().clone(),
                            if e.path().as_str() == form {
                                changed.clone()
                            } else if e.path().as_str() == ".ibcmd-provenance/manifest.json" {
                                serde_json::to_vec(&manifest).unwrap()
                            } else {
                                e.bytes().to_vec()
                            },
                        )
                        .unwrap()
                    })
                    .collect(),
            )
            .unwrap()
        };
        assert!(edt_to_xml(&Project::from_tree(alter(false)).unwrap(), &options).is_err());
        let current = edt_to_xml(&Project::from_tree(alter(true)).unwrap(), &options)
            .unwrap()
            .tree;
        let bytes = current
            .entries()
            .iter()
            .find(|e| e.path().as_str() == "CommonForms/ChoiceNull/Ext/Form.xml")
            .unwrap()
            .bytes();
        assert!(String::from_utf8_lossy(bytes).contains(">17<"));
        assert_eq!(
            String::from_utf8_lossy(bytes).matches("<Picture/>").count(),
            0
        );
    }
}
#[test]
#[ignore = "Requires immutable local genuine SDK captures"]
fn genuine_nine_nullable_picture_wrappers_are_fully_claimed() {
    let lab = std::path::PathBuf::from(std::env::var_os("IBCMD_EDT_LAB").unwrap());
    let proof: serde_json::Value = serde_json::from_slice(
        &std::fs::read(lab.join("choice-wrapper-research-r1/picture-details-r1.json")).unwrap(),
    )
    .unwrap();
    let template = String::from_utf8(write(FormDialect::Designer, &fixture(21), 21)).unwrap();
    let begin = template.find("<ChoiceParameters>").unwrap();
    let end = template.find("</ChoiceParameters>").unwrap() + "</ChoiceParameters>".len();
    let mut wrappers = 0;
    for row in proof["rows"].as_array().unwrap() {
        let bytes = std::fs::read(row["path"].as_str().unwrap()).unwrap();
        assert_eq!(
            format!("{:x}", Sha256::digest(&bytes)),
            row["sha256"].as_str().unwrap()
        );
        let text = String::from_utf8(bytes).unwrap();
        let mut tail = text.as_str();
        while let Some(start) = tail.find("<ChoiceParameters>") {
            tail = &tail[start..];
            let stop = tail.find("</ChoiceParameters>").unwrap() + "</ChoiceParameters>".len();
            let block = &tail[..stop];
            if block.contains("<Picture/>") {
                wrappers += block.matches("<Picture/>").count();
                let mut tiny = template[..begin].to_string();
                tiny.push_str(block);
                tiny.push_str(&template[end..]);
                let body = with_source_version(Some(FormatVersion::new(2, 21)), || {
                    read_form(FormDialect::Designer, tiny.as_bytes())
                });
                assert!(
                    body.is_ok(),
                    "genuine nullable wrapper must be fully claimed"
                );
            }
            tail = &tail[stop..];
        }
    }
    assert_eq!(wrappers, 9);
}
