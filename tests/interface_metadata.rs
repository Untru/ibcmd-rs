//! Synthetic descriptors: no supplier UUIDs or native source trees.
use ibcmd_rs::metadata_model::{
    DescriptorContext,
    brace::{Brace, parse_row},
    compile_descriptor,
    export::{object_names, rewrite_file},
};
use std::{process::Command, sync::Arc};

const ID: &str = "10000000-0000-4000-8000-000000000558";
fn row(switchable: bool) -> Brace {
    parse_row(format!(r#"{{1,{{7,{},{{3,{{1,0,{ID}}},"OwnInterface",{{1,"en","Own title"}},"Own comment",0,0,00000000-0000-0000-0000-000000000000,0}},1}},0}}"#, u8::from(!switchable)).as_bytes()).unwrap()
}
#[test]
fn interface_descriptor_preserves_names_and_switchable_both_directions() {
    for version in ["2.20", "2.21"] {
        for (switchable, multiline) in [(true, false), (false, false), (true, true), (false, true)]
        {
            let mut stored = row(switchable);
            if multiline {
                stored.as_list_mut().unwrap()[1].as_list_mut().unwrap()[2]
                    .as_list_mut()
                    .unwrap()[4] = Brace::str("Own comment\r\r\nSecond line");
            }
            let names = object_names("Interface", &stored)
                .expect("Interface UUID must have a canonical name");
            assert_eq!(names.uuid, ID);
            assert_eq!(names.full_name, "Interface.OwnInterface");
            let xml = format!(
                r#"<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" xmlns:v8="http://v8.1c.ru/8.1/data/core" version="{version}"><Interface uuid="{ID}"><Properties><Name>OwnInterface</Name><Synonym><v8:item><v8:lang>en</v8:lang><v8:content>Own title</v8:content></v8:item></Synonym><Comment>Own comment</Comment><Switchable>{switchable}</Switchable></Properties></Interface></MetaDataObject>"#
            );
            let xml = if multiline {
                xml.replace("Own comment", "Own comment\r\nSecond line")
            } else {
                xml
            };
            let expected = if multiline {
                rewrite_file(
                    xml.replace("Own comment\r\nSecond line", "Own comment")
                        .as_bytes(),
                )
                .unwrap()
                .replace("Own comment", "Own comment\r\nSecond line")
            } else {
                rewrite_file(xml.as_bytes()).unwrap()
            };
            let root = std::env::temp_dir().join(format!(
                "ibcmd-interface-{}-{}",
                std::process::id(),
                uuid::Uuid::new_v4()
            ));
            let _scratch = crate::common::Scratch::adopt(&root);
            std::fs::create_dir(&root).unwrap();
            let path = root.join("Interfaces/OwnInterface.xml");
            let bytes = Arc::new(xml.as_bytes().to_vec());
            let ctx =
                DescriptorContext::with_files(&root, version, &[(path.clone(), bytes.clone())])
                    .unwrap();
            let compiled = compile_descriptor("Interface", &path, &bytes, &ctx).unwrap();
            assert_eq!(parse_row(&compiled).unwrap(), stored);
            let packed = ibcmd_cf::payload::encode_payload(
                ibcmd_cf::payload::PayloadEncoding::RawDeflate,
                &ibcmd_rs::metadata_model::brace::serialize_row(&stored),
                ibcmd_core::limits::ResourceLimits::default(),
            )
            .unwrap();
            let archive =
                ibcmd_v8::writer::write_format15_to_vec(&ibcmd_v8::writer::Format15Document::new(
                    7,
                    vec![ibcmd_v8::writer::Format15Element::named(ID, Some(packed))],
                ))
                .unwrap();
            let cf = root.join("own.cf");
            std::fs::write(&cf, archive).unwrap();
            let output = root.join("out");
            let result = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
                .args(["cf", "export"])
                .arg(&cf)
                .arg(&output)
                .args(["--source-version", version, "--fail-on-opaque"])
                .env("PATH", "")
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(
                std::fs::read(output.join("Interfaces/OwnInterface.xml")).unwrap(),
                expected.into_bytes()
            );
        }
    }
}
#[test]
fn interface_order_follows_common_pictures() {
    let order = ibcmd_schema::configuration_root::child_xml_order();
    let (rank, _) = ibcmd_schema::configuration_root::child_xml_kind("Interface")
        .expect("Interface must be in root inventory");
    assert_eq!(order[rank - 1], "CommonPicture");
    assert_eq!(order[rank + 1], "SessionParameter");
    assert_eq!(
        ibcmd_rs::metadata_model::index::collection_of_kind("Interface"),
        Some("Interfaces")
    );
}

#[test]
fn malformed_interface_records_are_not_admitted_as_names() {
    let original = ibcmd_rs::metadata_model::brace::serialize_row(&row(true));
    let text = String::from_utf8(original).unwrap();
    for invalid in [
        "{1}".to_owned(),
        text.replacen("{7,0,", "{8,0,", 1),
        text.replacen("{7,0,", "{7,2,", 1),
        text.replacen("{7,0,", "{7,\"0\",", 1),
        text.replacen(ID, "00000000-0000-0000-0000-000000000000", 1),
        text.replacen("\"OwnInterface\"", "\"\"", 1),
        text.replacen("\"OwnInterface\"", "\"Nested.Name\"", 1),
        text.replacen("{1,0,", "{1,1,", 1),
        text.replacen("},1},0}", "},9},0}", 1),
    ] {
        assert_ne!(invalid, text, "mutation did not change the record");
        let parsed = parse_row(invalid.as_bytes()).unwrap();
        assert!(
            object_names("Interface", &parsed).is_err(),
            "admitted {invalid}"
        );
    }
}

#[test]
fn interface_source_rejects_lossy_properties_and_foreign_namespaces() {
    let xml = format!(
        r#"<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20"><Interface uuid="{ID}"><Properties><Name>OwnInterface</Name><Synonym/><Comment/><Switchable>true</Switchable></Properties></Interface></MetaDataObject>"#
    );
    let root = std::path::Path::new("own-interface");
    let ctx = DescriptorContext::with_files(root, "2.20", &[]).unwrap();
    let path = root.join("Interfaces/OwnInterface.xml");
    assert!(compile_descriptor("Interface", &path, xml.as_bytes(), &ctx).is_ok());
    for invalid in [
        xml.replace("</Properties>", "<Unknown>1</Unknown></Properties>"),
        xml.replace("<Comment/>", "<Comment/><Comment/>"),
        xml.replace("<Switchable>true", "<Switchable>2"),
        xml.replace("<Switchable>", "<Switchable xmlns=\"urn:foreign\">"),
        xml.replace("<Properties>", "<Properties xmlns=\"urn:foreign\">"),
        xml.replace("uuid=", "xmlns:q=\"urn:foreign\" q:uuid="),
        xml.replace("version=\"2.20\"", "version=\"2.21\""),
        xml.replace(
            "</Interface>",
            "<ChildObjects><Unknown/></ChildObjects></Interface>",
        ),
    ] {
        assert!(
            compile_descriptor("Interface", &path, invalid.as_bytes(), &ctx).is_err(),
            "admitted {invalid}"
        );
    }
}

#[test]
fn invalid_interface_cf_is_refused_without_fail_on_opaque() {
    let mut stored = row(true);
    stored.as_list_mut().unwrap()[1].as_list_mut().unwrap()[1] = Brace::num(2);
    let packed = ibcmd_cf::payload::encode_payload(
        ibcmd_cf::payload::PayloadEncoding::RawDeflate,
        &ibcmd_rs::metadata_model::brace::serialize_row(&stored),
        ibcmd_core::limits::ResourceLimits::default(),
    )
    .unwrap();
    let archive =
        ibcmd_v8::writer::write_format15_to_vec(&ibcmd_v8::writer::Format15Document::new(
            7,
            vec![ibcmd_v8::writer::Format15Element::named(ID, Some(packed))],
        ))
        .unwrap();
    let root =
        std::env::temp_dir().join(format!("ibcmd-invalid-interface-{}", uuid::Uuid::new_v4()));
    let _scratch = crate::common::Scratch::adopt(&root);
    std::fs::create_dir(&root).unwrap();
    let cf = root.join("invalid.cf");
    std::fs::write(&cf, archive).unwrap();
    let out = root.join("out");
    let result = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .args(["cf", "export"])
        .arg(cf)
        .arg(&out)
        .args(["--source-version", "2.20"])
        .env("PATH", "")
        .output()
        .unwrap();
    assert!(
        !result.status.success(),
        "invalid Interface was reported as success: {}",
        String::from_utf8_lossy(&result.stdout)
    );
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        report.contains("unsupported Interface descriptor"),
        "{report}"
    );
    assert!(!out.join("Interfaces/OwnInterface.xml").exists());
    assert!(!out.join("Configuration.xml").exists());
}
