use morph1c_core::ir::{
    FormBody, HelpPage, HelpResource, MetadataObject, NamedFormBody, ObjectKind, PropertyValue,
    Token, Uuid,
};
use morph1c_core::version::FormatVersion;
use morph1c_pipeline::{ConvertOptions, Format, read_config, write_config};

fn opts() -> ConvertOptions {
    ConvertOptions::default().with_target_version(FormatVersion::new(2, 21))
}
fn config() -> morph1c_core::ir::Configuration {
    read_config(
        Format::Designer,
        std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/subsystem-ci/src"
        )),
        &opts(),
    )
    .unwrap()
    .0
}
fn ordinary() -> morph1c_core::ir::Configuration {
    let mut cfg = config();
    let mut obj = MetadataObject::new(
        ObjectKind::new("CommonForm"),
        "OpaqueOrdinary",
        Uuid([1; 16]),
    );
    obj.properties.push((
        morph1c_core::spec::registry::spec_for("CommonForm")
            .unwrap()
            .fields()
            .iter()
            .find(|f| f.name == "formType")
            .unwrap()
            .id,
        PropertyValue::Enum(Token::new("Ordinary")),
    ));
    obj.form_bodies.push(NamedFormBody {
        name: obj.name.clone(),
        body: FormBody::new(),
        ordinary_body: Some(
            b"\xff\xff\xff\x7f\x00binary\x00Procedure EmbeddedModule()\r\nEndProcedure\r\n"
                .to_vec(),
        ),
        module: None,
        help: vec![],
        help_resources: vec![],
    });
    cfg.objects.push(obj);
    cfg
}
#[test]
fn ordinary_entire_body_survives_both_dialects_without_external_module() {
    let mut cfg = ordinary();
    let expected = cfg.objects.last().unwrap().form_bodies[0]
        .ordinary_body
        .clone()
        .unwrap();
    for format in [Format::Edt, Format::Designer, Format::Edt] {
        let dir = tempfile::tempdir().unwrap();
        write_config(format, &cfg, dir.path()).unwrap();
        let body = match format {
            Format::Edt => "CommonForms/OpaqueOrdinary/Form.oform",
            _ => "CommonForms/OpaqueOrdinary/Ext/Form.bin",
        };
        assert_eq!(std::fs::read(dir.path().join(body)).unwrap(), expected);
        assert!(
            !dir.path()
                .join("CommonForms/OpaqueOrdinary/Module.bsl")
                .exists()
        );
        assert!(
            !dir.path()
                .join("CommonForms/OpaqueOrdinary/Ext/Form/Module.bsl")
                .exists()
        );
        cfg = read_config(format, dir.path(), &opts()).unwrap().0;
        let form = &cfg
            .objects
            .iter()
            .find(|o| o.name == "OpaqueOrdinary")
            .unwrap()
            .form_bodies[0];
        assert_eq!(form.ordinary_body.as_ref().unwrap(), &expected);
        assert!(form.module.is_none());
        assert_eq!(form.body, FormBody::new());
    }
}
#[test]
fn ordinary_reader_rejects_ambiguous_and_oversized_bodies() {
    for extra in ["Form.form", "Module.bsl"] {
        let dir = tempfile::tempdir().unwrap();
        write_config(Format::Edt, &ordinary(), dir.path()).unwrap();
        std::fs::write(
            dir.path().join("CommonForms/OpaqueOrdinary").join(extra),
            b"unexpected",
        )
        .unwrap();
        assert!(read_config(Format::Edt, dir.path(), &opts()).is_err());
    }
    let dir = tempfile::tempdir().unwrap();
    write_config(Format::Edt, &ordinary(), dir.path()).unwrap();
    let body = dir.path().join("CommonForms/OpaqueOrdinary/Form.oform");
    std::fs::OpenOptions::new()
        .write(true)
        .open(body)
        .unwrap()
        .set_len(32 * 1024 * 1024 + 1)
        .unwrap();
    assert!(read_config(Format::Edt, dir.path(), &opts()).is_err());
    let mut cfg = ordinary();
    cfg.objects.last_mut().unwrap().form_bodies[0].body.title =
        Some(PropertyValue::Str("managed".into()));
    let dir = tempfile::tempdir().unwrap();
    assert!(write_config(Format::Edt, &cfg, dir.path()).is_err());
}
#[test]
fn configuration_help_uses_root_ext_and_preserves_pages_resources() {
    let mut cfg = config();
    let root = cfg
        .objects
        .iter_mut()
        .find(|o| o.kind.as_str() == "Configuration")
        .unwrap();
    root.help.push(HelpPage {
        lang: "ru".into(),
        body: "<html>root\nhelp</html>".into(),
    });
    root.help_resources.push(HelpResource {
        rel_path: "image.bin".into(),
        bytes: vec![0, 255, 13, 10],
    });
    for format in [Format::Designer, Format::Edt, Format::Designer] {
        let dir = tempfile::tempdir().unwrap();
        write_config(format, &cfg, dir.path()).unwrap();
        let path = if format == Format::Edt {
            "Configuration/Help/ru.html"
        } else {
            "Ext/Help/ru.html"
        };
        assert!(dir.path().join(path).is_file());
        assert!(!dir.path().join("Configuration/Ext/Help/ru.html").exists());
        cfg = read_config(format, dir.path(), &opts()).unwrap().0;
        let root = cfg
            .objects
            .iter()
            .find(|o| o.kind.as_str() == "Configuration")
            .unwrap();
        assert_eq!(root.help[0].body, "<html>root\nhelp</html>");
        assert_eq!(root.help_resources[0].bytes, [0, 255, 13, 10]);
    }
}
#[test]
#[ignore = "requires genuine read-only F lab oracle witnesses"]
fn genuine_root_help_and_all_ordinary_binary_pairs_match_sdk() {
    let lab = std::path::PathBuf::from(std::env::var("IBCMD_BODY_ORACLE_LAB").expect("lab path"));
    let edt = lab.join("oracle-bsp83-r1/authentic-workspace/OracleConfiguration/src");
    let native = lab.join("native-reference-bsp83-r2/native-xml");
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("Configuration/Help")).unwrap();
    for rel in [
        "Configuration/Configuration.mdo",
        "Configuration/Help/ru.html",
    ] {
        std::fs::copy(edt.join(rel), dir.path().join(rel)).unwrap();
    }
    let cfg = read_config(
        Format::Edt,
        dir.path(),
        &ConvertOptions::only(["Configuration".into()]),
    )
    .unwrap()
    .0;
    let out = tempfile::tempdir().unwrap();
    morph1c_core::version::with_roundtrip_target(FormatVersion::new(2, 20), || {
        write_config(Format::Designer, &cfg, out.path())
    })
    .unwrap();
    for rel in ["Ext/Help.xml", "Ext/Help/ru.html"] {
        assert_eq!(
            std::fs::read(out.path().join(rel)).unwrap(),
            std::fs::read(native.join(rel)).unwrap(),
            "{rel}"
        );
    }
    let evidence: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            lab.join("oracle-uha83-r1/ordinary-form-original-edt-native-sdk-binary-proof.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let rows = evidence["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 9);
    for row in rows {
        let source = std::path::PathBuf::from(row["authentic_edt"]["path"].as_str().unwrap());
        let rel = row["relative_path"].as_str().unwrap();
        let parts = rel.split('/').collect::<Vec<_>>();
        let kind_dir = parts[0];
        let name = parts[1];
        let kind = match kind_dir {
            "CommonForms" => "CommonForm",
            "Catalogs" => "Catalog",
            "Reports" => "Report",
            other => panic!("unmodeled witness kind {other}"),
        };
        let src_root = lab.join("oracle-uha83-r1/authentic-workspace/OracleConfiguration/src");
        let small = tempfile::tempdir().unwrap();
        let body_rel = source.strip_prefix(&src_root).unwrap();
        let descriptor_rel = format!("{kind_dir}/{name}/{name}.mdo");
        for (from, dest) in [
            (
                src_root.join(&descriptor_rel),
                small.path().join(&descriptor_rel),
            ),
            (source.clone(), small.path().join(body_rel)),
        ] {
            std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
            std::fs::copy(from, dest).unwrap();
        }
        fn copy_tree(src: &std::path::Path, dst: &std::path::Path) {
            if !src.exists() {
                return;
            }
            std::fs::create_dir_all(dst).unwrap();
            for entry in std::fs::read_dir(src).unwrap() {
                let entry = entry.unwrap();
                if entry.file_type().unwrap().is_dir() {
                    copy_tree(&entry.path(), &dst.join(entry.file_name()));
                } else {
                    std::fs::copy(entry.path(), dst.join(entry.file_name())).unwrap();
                }
            }
        }
        let templates = format!("{kind_dir}/{name}/Templates");
        copy_tree(&src_root.join(&templates), &small.path().join(&templates));
        let cfg = read_config(
            Format::Edt,
            small.path(),
            &ConvertOptions::only([kind.into()]),
        )
        .unwrap()
        .0;
        let expected = std::fs::read(&source).unwrap();
        let out = tempfile::tempdir().unwrap();
        morph1c_core::version::with_roundtrip_target(FormatVersion::new(2, 20), || {
            write_config(Format::Designer, &cfg, out.path())
        })
        .unwrap();
        assert_eq!(
            std::fs::read(out.path().join(rel)).unwrap(),
            expected,
            "{rel}"
        );
        assert_eq!(
            std::fs::read(row["native_original"]["path"].as_str().unwrap()).unwrap(),
            expected
        );
        assert_eq!(
            std::fs::read(row["post_edt_native_sdk"]["path"].as_str().unwrap()).unwrap(),
            expected
        );
        let returned = read_config(
            Format::Designer,
            out.path(),
            &ConvertOptions::only([kind.into()]),
        )
        .unwrap()
        .0;
        let out2 = tempfile::tempdir().unwrap();
        write_config(Format::Edt, &returned, out2.path()).unwrap();
        assert_eq!(std::fs::read(out2.path().join(body_rel)).unwrap(), expected);
    }
}
#[test]
fn help_unknown_manifest_and_unsafe_language_resource_are_rejected() {
    let mut cfg = config();
    let root = cfg
        .objects
        .iter_mut()
        .find(|o| o.kind.as_str() == "Configuration")
        .unwrap();
    root.help.push(HelpPage {
        lang: "../escape".into(),
        body: "html".into(),
    });
    let dir = tempfile::tempdir().unwrap();
    assert!(write_config(Format::Edt, &cfg, dir.path()).is_err());
    let root = cfg
        .objects
        .iter_mut()
        .find(|o| o.kind.as_str() == "Configuration")
        .unwrap();
    root.help[0].lang = "ru".into();
    root.help_resources.push(HelpResource {
        rel_path: "../outside.bin".into(),
        bytes: vec![1],
    });
    let dir = tempfile::tempdir().unwrap();
    assert!(write_config(Format::Edt, &cfg, dir.path()).is_err());
    cfg.objects
        .iter_mut()
        .find(|o| o.kind.as_str() == "Configuration")
        .unwrap()
        .help_resources
        .clear();
    let dir = tempfile::tempdir().unwrap();
    write_config(Format::Designer, &cfg, dir.path()).unwrap();
    let path = dir.path().join("Ext/Help.xml");
    let bytes = std::fs::read(&path).unwrap();
    let text = String::from_utf8(bytes)
        .unwrap()
        .replace("<Page>ru</Page>", "<Page>ru</Page><Unknown/>");
    std::fs::write(path, text).unwrap();
    assert!(read_config(Format::Designer, dir.path(), &opts()).is_err());
}
#[test]
fn public_conversion_accounts_root_help_and_whole_ordinary_body() {
    use ibcmd_edt::{
        ConversionOptions, Project, ReaderLimits, edt_to_xml, read_xml_source, xml_to_edt,
    };
    use ibcmd_xml::source_tree::SourceTree;
    let mut cfg = ordinary();
    let root = cfg
        .objects
        .iter_mut()
        .find(|o| o.kind.as_str() == "Configuration")
        .unwrap();
    root.help.push(HelpPage {
        lang: "ru".into(),
        body: "<html>root</html>".into(),
    });
    let dir = tempfile::tempdir().unwrap();
    write_config(Format::Designer, &cfg, dir.path()).unwrap();
    let path = dir.path().join("Configuration.xml");
    let text = String::from_utf8(std::fs::read(&path).unwrap())
        .unwrap()
        .replace(
            "</Language>",
            "</Language>\r\n\t\t\t<CommonForm>OpaqueOrdinary</CommonForm>",
        );
    std::fs::write(path, text).unwrap();
    let xml = read_xml_source(dir.path(), ReaderLimits::default()).unwrap();
    let options = ConversionOptions {
        edt_version: "2025.2.3".into(),
        xml_dialect: "2.21".into(),
        runtime_version: Some("8.5.1".into()),
    };
    let converted = xml_to_edt(&xml, &options).unwrap();
    assert_eq!(converted.accounting.len(), xml.entries().len());
    let clean = SourceTree::new(
        converted
            .tree
            .entries()
            .iter()
            .filter(|e| !e.path().as_str().starts_with(".ibcmd-provenance/"))
            .cloned()
            .collect(),
    )
    .unwrap();
    let returned = edt_to_xml(&Project::from_tree(clean).unwrap(), &options).unwrap();
    for rel in [
        "Ext/Help.xml",
        "Ext/Help/ru.html",
        "CommonForms/OpaqueOrdinary/Ext/Form.bin",
    ] {
        assert_eq!(
            xml.entries()
                .iter()
                .find(|e| e.path().as_str() == rel)
                .unwrap()
                .bytes(),
            returned
                .tree
                .entries()
                .iter()
                .find(|e| e.path().as_str() == rel)
                .unwrap()
                .bytes(),
            "{rel}"
        );
    }
}

#[test]
fn public_dcs_parameter_presence_is_exact_and_edits_cannot_restore_stale_xml() {
    use ibcmd_edt::{
        ConversionOptions, Project, ReaderLimits, edt_to_xml, read_xml_source, xml_to_edt,
    };
    use ibcmd_xml::source_tree::{SourceEntry, SourcePath, SourceTree};
    use sha2::{Digest, Sha256};
    let bytes = br#"<form:Form xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:core="http://g5.1c.ru/v8/dt/mcore" xmlns:form="http://g5.1c.ru/v8/dt/form" xmlns:schema="http://g5.1c.ru/v8/dt/data-composition-system/schema" xmlns:settings="http://g5.1c.ru/v8/dt/data-composition-system/settings"><attributes><name>List</name><valueType><types>DynamicList</types></valueType><view><common>true</common></view><edit><common>true</common></edit><extInfo xsi:type="form:DynamicListExtInfo"><parameters><name>Restriction</name></parameters></extInfo></attributes></form:Form>"#;
    let mut cfg = ordinary();
    let obj = cfg.objects.last_mut().unwrap();
    obj.properties[0].1 = PropertyValue::Enum(Token::new("Managed"));
    obj.form_bodies[0].ordinary_body = None;
    obj.form_bodies[0].body = formats_xml::form::read_form(
        formats_xml::form::FormDialect::Edt,
        &[
            b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n".as_slice(),
            bytes.as_slice(),
            b"\r\n",
        ]
        .concat(),
    )
    .unwrap();
    obj.form_bodies[0].body.data_attributes[0]
        .dynamic_list
        .as_mut()
        .unwrap()
        .list_settings = Some(morph1c_core::ir::DcsListSettings {
        items_view_mode: Some("Normal".into()),
        ..Default::default()
    });
    let options = ConversionOptions {
        edt_version: "2025.2.3".into(),
        xml_dialect: "2.21".into(),
        runtime_version: Some("8.5.1".into()),
    };
    fn mutate(tree: &SourceTree, path: &str, bytes: Vec<u8>) -> SourceTree {
        SourceTree::new(
            tree.entries()
                .iter()
                .map(|entry| {
                    if entry.path().as_str() == path {
                        SourceEntry::from_bytes(SourcePath::new(path).unwrap(), bytes.clone())
                            .unwrap()
                    } else {
                        entry.clone()
                    }
                })
                .collect(),
        )
        .unwrap()
    }
    for value in [None, Some(false), Some(true)] {
        let dir = tempfile::tempdir().unwrap();
        write_config(Format::Designer, &cfg, dir.path()).unwrap();
        let path = dir.path().join("Configuration.xml");
        let text = String::from_utf8(std::fs::read(&path).unwrap())
            .unwrap()
            .replace(
                "</Language>",
                "</Language>\r\n\t\t\t<CommonForm>OpaqueOrdinary</CommonForm>",
            );
        std::fs::write(path, text).unwrap();
        let form_path = dir.path().join("CommonForms/OpaqueOrdinary/Ext/Form.xml");
        let text = String::from_utf8(std::fs::read(&form_path).unwrap()).unwrap();
        let text = match value {
            None => text
                .lines()
                .filter(|line| !line.contains("<dcssch:useRestriction>"))
                .collect::<Vec<_>>()
                .join("\r\n"),
            Some(false) => text,
            Some(true) => text.replace(
                "<dcssch:useRestriction>false</dcssch:useRestriction>",
                "<dcssch:useRestriction>true</dcssch:useRestriction>",
            ),
        };
        std::fs::write(form_path, text).unwrap();
        let xml = read_xml_source(dir.path(), ReaderLimits::default()).unwrap();
        let generated = xml_to_edt(&xml, &options).unwrap().tree;
        let returned = edt_to_xml(&Project::from_tree(generated.clone()).unwrap(), &options)
            .unwrap()
            .tree;
        assert_eq!(
            returned, xml,
            "native restriction {value:?} must return every original byte"
        );
        if value == Some(false) {
            let path = "src/CommonForms/OpaqueOrdinary/Form.form";
            let original = generated
                .entries()
                .iter()
                .find(|e| e.path().as_str() == path)
                .unwrap();
            let edited = String::from_utf8(original.bytes().to_vec())
                .unwrap()
                .replace(
                    "</parameters>",
                    "<useRestriction>true</useRestriction></parameters>",
                )
                .into_bytes();
            assert_ne!(edited.as_slice(), original.bytes());
            let hash = format!("{:x}", Sha256::digest(&edited));
            let tree = mutate(&generated, path, edited);
            let manifest = tree
                .entries()
                .iter()
                .find(|e| e.path().as_str() == ".ibcmd-provenance/manifest.json")
                .unwrap();
            let mut manifest: serde_json::Value = serde_json::from_slice(manifest.bytes()).unwrap();
            manifest["generated"][path] = serde_json::Value::String(hash);
            let tree = mutate(
                &tree,
                ".ibcmd-provenance/manifest.json",
                serde_json::to_vec(&manifest).unwrap(),
            );
            let error = edt_to_xml(&Project::from_tree(tree).unwrap(), &options).unwrap_err();
            assert!(
                error.to_string().contains("changed"),
                "edited true restriction must fail stale provenance: {error}"
            );
        }
    }
}
