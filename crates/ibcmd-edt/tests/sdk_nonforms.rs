use morph1c_core::ir::{MetadataObject, ObjectKind, PropertyValue, Template, Token, Uuid};
use morph1c_pipeline::{ConvertOptions, Format, read_config, write_config};
fn config(kind: &str, body: &[u8]) -> morph1c_core::ir::Configuration {
    let mut cfg = read_config(
        Format::Designer,
        std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/subsystem-ci/src"
        )),
        &ConvertOptions::default(),
    )
    .unwrap()
    .0;
    let mut object =
        MetadataObject::new(ObjectKind::new("CommonTemplate"), "Witness", Uuid([9; 16]));
    object.properties.push((
        morph1c_core::spec::metadata::report_template_ref::F_TEMPLATE_TYPE,
        PropertyValue::Enum(Token::new(kind)),
    ));
    object.templates.push(Template {
        name: "Witness".into(),
        properties: vec![],
        pages: vec![],
        resources: vec![],
        body: Some(body.to_vec()),
    });
    cfg.objects.push(object);
    cfg
}
fn body(cfg: &morph1c_core::ir::Configuration) -> &[u8] {
    cfg.objects
        .iter()
        .find(|o| o.name == "Witness")
        .unwrap()
        .templates[0]
        .body
        .as_deref()
        .unwrap()
}
const MXL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<document xmlns=\"http://v8.1c.ru/8.2/data/spreadsheet\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\">\r\n<other title=\"a > b\"/><v8:content>first\nsecond &amp; third</v8:content><picture>base64\r\npayload</picture>\r\n</document>";
fn dcs(inner: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<DataCompositionSchema xmlns=\"http://v8.1c.ru/8.1/data-composition-system/schema\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\">\r\n{inner}\r\n</DataCompositionSchema>"
    )
}
#[test]
fn physical_child_order_requires_a_complete_unique_collection_inventory() {
    struct Invalid(&'static [&'static str]);
    impl formats_xml::LocusMap for Invalid {
        fn lookup(&self, _: morph1c_core::ir::FieldId) -> Option<formats_xml::FieldProjection> {
            None
        }
        fn child_emit_order(&self) -> Option<&'static [&'static str]> {
            Some(self.0)
        }
    }
    for order in [
        &["Form"][..],
        &[
            "Attribute",
            "TabularSection",
            "Form",
            "AddressingAttribute",
            "Template",
            "Form",
        ][..],
    ] {
        let error = formats_xml::children::write_children(
            &[],
            morph1c_core::spec::metadata::task::task(),
            &Invalid(order),
            &[],
            morph1c_core::version::FormatVersion::new(2, 20),
            true,
        )
        .unwrap_err();
        assert!(error.to_string().contains("physical child order"));
    }
}
#[test]
fn mxl_source_asset_above_inline_core_limit_uses_bounded_projection() {
    let mut source = b"<document xmlns=\"http://v8.1c.ru/8.2/data/spreadsheet\"><picture>".to_vec();
    source.resize(source.len() + 32 * 1024 * 1024 + 1, b'A');
    source.extend_from_slice(b"</picture></document>");
    let dir = tempfile::tempdir().unwrap();
    write_config(
        Format::Edt,
        &config("SpreadsheetDocument", &source),
        dir.path(),
    )
    .unwrap();
    let raw = std::fs::read(dir.path().join("CommonTemplates/Witness/Template.mxlx")).unwrap();
    assert!(raw == source);
}
#[test]
fn mxl_projects_only_text_newlines_and_preserves_native_source() {
    let cfg = config("SpreadsheetDocument", MXL.as_bytes());
    let edt = tempfile::tempdir().unwrap();
    write_config(Format::Edt, &cfg, edt.path()).unwrap();
    let raw = std::fs::read(edt.path().join("CommonTemplates/Witness/Template.mxlx")).unwrap();
    assert_eq!(
        raw,
        MXL.replace("first\nsecond", "first\r\nsecond").as_bytes()
    );
    let loaded = read_config(Format::Edt, edt.path(), &ConvertOptions::default())
        .unwrap()
        .0;
    assert_eq!(body(&loaded), MXL.as_bytes());
    let native = tempfile::tempdir().unwrap();
    write_config(Format::Designer, &loaded, native.path()).unwrap();
    let expected = [b"\xef\xbb\xbf".as_slice(), MXL.as_bytes()].concat();
    assert_eq!(
        std::fs::read(
            native
                .path()
                .join("CommonTemplates/Witness/Ext/Template.xml")
        )
        .unwrap(),
        expected
    );
    let loaded = read_config(Format::Designer, native.path(), &ConvertOptions::default())
        .unwrap()
        .0;
    let returned = tempfile::tempdir().unwrap();
    write_config(Format::Designer, &loaded, returned.path()).unwrap();
    assert_eq!(
        std::fs::read(
            returned
                .path()
                .join("CommonTemplates/Witness/Ext/Template.xml")
        )
        .unwrap(),
        expected
    );
    for invalid in [
        MXL.replace("<v8:content>", "<v8:content><![CDATA[keep]]>"),
        MXL.replace("data/spreadsheet", "data/other"),
        MXL.replace("<v8:content>", "<v8:content xml:space=\"preserve\">"),
        MXL.replace("<v8:content>", "<v8:content><!--keep-->"),
        MXL.replace("<v8:content>", "<v8:content><unknown/>"),
    ] {
        let cfg = config("SpreadsheetDocument", invalid.as_bytes());
        assert!(write_config(Format::Edt, &cfg, tempfile::tempdir().unwrap().path()).is_err());
    }
    let empty = MXL.replace(
        "<v8:content>first\nsecond &amp; third</v8:content>",
        "<v8:content/>",
    );
    let dir = tempfile::tempdir().unwrap();
    write_config(
        Format::Edt,
        &config("SpreadsheetDocument", empty.as_bytes()),
        dir.path(),
    )
    .unwrap();
    assert_eq!(
        std::fs::read(dir.path().join("CommonTemplates/Witness/Template.mxlx")).unwrap(),
        empty.as_bytes()
    );
}
#[test]
fn dcs_projects_resolved_inline_typeset_alias_and_never_query_text() {
    let inner = "<valueType><v8:TypeSet xmlns:p=\"http://v8.1c.ru/8.1/data/enterprise/current-config\">p:AnyIBRef</v8:TypeSet></valueType><query marker=\"a > b\r\nc\">p:AnyIBRef\nSELECT AnyRef</query>";
    let original = dcs(inner);
    let cfg = config("DataCompositionSchema", original.as_bytes());
    let edt = tempfile::tempdir().unwrap();
    write_config(Format::Edt, &cfg, edt.path()).unwrap();
    let projected = std::fs::read(edt.path().join("CommonTemplates/Witness/Template.dcs")).unwrap();
    let expected = original
        .replace(">p:AnyIBRef</v8:TypeSet>", ">p:AnyRef</v8:TypeSet>")
        .replace("AnyIBRef\nSELECT", "AnyIBRef\r\nSELECT");
    assert_eq!(projected, expected.as_bytes());
    let loaded = read_config(Format::Edt, edt.path(), &ConvertOptions::default())
        .unwrap()
        .0;
    assert_eq!(body(&loaded), original.as_bytes());
    let native = tempfile::tempdir().unwrap();
    write_config(Format::Designer, &loaded, native.path()).unwrap();
    assert_eq!(
        std::fs::read(
            native
                .path()
                .join("CommonTemplates/Witness/Ext/Template.xml")
        )
        .unwrap(),
        [b"\xef\xbb\xbf".as_slice(), original.as_bytes()].concat()
    );
    for (old, new) in [
        ("enterprise/current-config", "enterprise/other"),
        ("<v8:TypeSet", "<v8:Type"),
        ("data/core", "data/custom-core"),
    ] {
        let text = if old == "<v8:TypeSet" {
            original
                .replace(old, new)
                .replace("</v8:TypeSet>", "</v8:Type>")
        } else {
            original.replace(old, new)
        };
        let cfg = config("DataCompositionSchema", text.as_bytes());
        let out = tempfile::tempdir().unwrap();
        write_config(Format::Edt, &cfg, out.path()).unwrap();
        let raw = String::from_utf8(
            std::fs::read(out.path().join("CommonTemplates/Witness/Template.dcs")).unwrap(),
        )
        .unwrap();
        assert!(raw.contains("p:AnyIBRef</"));
    }
    let invalid = original.replace("xmlns:p=", "extra=\"keep\" xmlns:p=");
    assert!(
        write_config(
            Format::Edt,
            &config("DataCompositionSchema", invalid.as_bytes()),
            tempfile::tempdir().unwrap().path()
        )
        .is_err()
    );
    let invalid = original.replace(
        ">p:AnyIBRef</v8:TypeSet>",
        "><!--keep-->p:AnyIBRef</v8:TypeSet>",
    );
    assert!(
        write_config(
            Format::Edt,
            &config("DataCompositionSchema", invalid.as_bytes()),
            tempfile::tempdir().unwrap().path()
        )
        .is_err()
    );
}

#[test]
#[ignore = "requires immutable installed EDT/native BSP corpus on F"]
fn genuine_bsp_nonforms_equal_sdk_and_keep_same_source_native_bytes() {
    let lab = std::path::PathBuf::from(std::env::var_os("IBCMD_NONFORM_LAB").unwrap());
    let report: serde_json::Value = serde_json::from_slice(
        &std::fs::read(lab.join("bsp83-direct-nonform-differences-r1.json")).unwrap(),
    )
    .unwrap();
    let authentic = lab.join("oracle-bsp83-r1/authentic-workspace/OracleConfiguration/src");
    let sdk = lab.join("native-reference-bsp83-r2/native-xml");
    let native = std::path::Path::new(
        "F:/ibcmd/lab/parity/ibcmd_rs_bsp_8327_native_20260919_20260921_export_recheck_2/native",
    );
    let mut templates = 0;
    for record in report["records"].as_array().unwrap() {
        let rel = record["path"].as_str().unwrap();
        if rel.starts_with("Tasks/") {
            continue;
        }
        let expected = std::fs::read(sdk.join(rel)).unwrap();
        let root = formats_xml::parse(&expected).unwrap().root;
        let (kind, suffix) = if root.local == "document" {
            ("SpreadsheetDocument", "Template.mxlx")
        } else {
            assert_eq!(root.local, "DataCompositionSchema");
            ("DataCompositionSchema", "Template.dcs")
        };
        let source_rel = rel.strip_suffix("Ext/Template.xml").unwrap().to_string() + suffix;
        let source = std::fs::read(authentic.join(source_rel)).unwrap();
        let dir = tempfile::tempdir().unwrap();
        write_config(
            Format::Edt,
            &config(kind, expected.strip_prefix(b"\xef\xbb\xbf").unwrap()),
            dir.path(),
        )
        .unwrap();
        std::fs::write(
            dir.path().join("CommonTemplates/Witness").join(suffix),
            &source,
        )
        .unwrap();
        let loaded = read_config(Format::Edt, dir.path(), &ConvertOptions::default())
            .unwrap()
            .0;
        let out = tempfile::tempdir().unwrap();
        write_config(Format::Designer, &loaded, out.path()).unwrap();
        assert!(
            std::fs::read(out.path().join("CommonTemplates/Witness/Ext/Template.xml")).unwrap()
                == expected,
            "SDK bytes: {rel}"
        );
        // Same-source native input is transported verbatim, including its own text EOL.
        let native_body = std::fs::read(native.join(rel)).unwrap();
        let cfg = config(kind, native_body.strip_prefix(b"\xef\xbb\xbf").unwrap());
        let original = tempfile::tempdir().unwrap();
        write_config(Format::Designer, &cfg, original.path()).unwrap();
        let loaded = read_config(
            Format::Designer,
            original.path(),
            &ConvertOptions::default(),
        )
        .unwrap()
        .0;
        let returned = tempfile::tempdir().unwrap();
        write_config(Format::Designer, &loaded, returned.path()).unwrap();
        assert_eq!(
            std::fs::read(
                returned
                    .path()
                    .join("CommonTemplates/Witness/Ext/Template.xml")
            )
            .unwrap(),
            native_body,
            "native {rel}"
        );
        assert_public_unchanged_return(&cfg);
        templates += 1;
    }
    assert_eq!(templates, 26);
    let source = tempfile::tempdir().unwrap();
    std::fs::create_dir(source.path().join("Tasks")).unwrap();
    let task = std::fs::read_dir(authentic.join("Tasks"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let name = task.file_name().unwrap().to_str().unwrap();
    std::fs::create_dir(source.path().join("Tasks").join(name)).unwrap();
    std::fs::copy(
        task.join(format!("{name}.mdo")),
        source
            .path()
            .join("Tasks")
            .join(name)
            .join(format!("{name}.mdo")),
    )
    .unwrap();
    let cfg = read_config(
        Format::Edt,
        source.path(),
        &ConvertOptions::only(["Task".into()]),
    )
    .unwrap()
    .0;
    let out = tempfile::tempdir().unwrap();
    morph1c_core::version::with_roundtrip_target(
        morph1c_core::version::FormatVersion::new(2, 20),
        || write_config(Format::Designer, &cfg, out.path()),
    )
    .unwrap();
    assert_eq!(
        std::fs::read(out.path().join("Tasks").join(format!("{name}.xml"))).unwrap(),
        std::fs::read(sdk.join("Tasks").join(format!("{name}.xml"))).unwrap()
    );
}

fn assert_public_unchanged_return(cfg: &morph1c_core::ir::Configuration) {
    let dir = tempfile::tempdir().unwrap();
    write_config(Format::Designer, cfg, dir.path()).unwrap();
    let path = dir.path().join("Configuration.xml");
    let root = String::from_utf8(std::fs::read(&path).unwrap())
        .unwrap()
        .replace(
            "</Language>",
            "</Language>\r\n\t\t\t<CommonTemplate>Witness</CommonTemplate>",
        );
    std::fs::write(path, root).unwrap();
    let xml = ibcmd_edt::read_xml_source(dir.path(), ibcmd_edt::ReaderLimits::default()).unwrap();
    let options = ibcmd_edt::ConversionOptions {
        edt_version: "2025.2.3".into(),
        xml_dialect: "2.21".into(),
        runtime_version: Some("8.5.1".into()),
    };
    let converted = ibcmd_edt::xml_to_edt(&xml, &options).unwrap();
    let project = ibcmd_edt::Project::from_tree(converted.tree).unwrap();
    let returned = ibcmd_edt::edt_to_xml(&project, &options).unwrap();
    assert_eq!(returned.tree, xml);
}
