use formats_xml::form::{
    FormDialect, read_form, read_list_settings_dcss, write_form, write_list_settings_dcss,
};
use morph1c_core::ir::{DcsItem, DcsRightValue, FormBody};

fn fixture() -> FormBody {
    let mut body = FormBody::new();
    body.conditional_appearance
        .push(DcsItem::ConditionalAppearance {
            used: None,
            selection: None,
            filter: vec![DcsItem::FilterComparison {
                used: None,
                left_field: "Object.Kind".into(),
                left_type: "dcscor:Field".into(),
                comparison_type: "Equal".into(),
                right: vec![DcsRightValue::TypeQName("d10p1:Undefined".into())],
                presentation: None,
                view_mode: None,
                user_setting_id: None,
                user_setting_presentation: None,
            }],
            appearance: vec![],
            source_empty_appearance: true,
            presentation: None,
            view_mode: None,
            user_setting_id: None,
        });
    body
}

#[test]
fn scoped_qname_and_explicit_empty_appearance_regenerate_without_changing_semantics() {
    let body = fixture();
    let bytes = write_form(FormDialect::Designer, &body).unwrap();
    let text = String::from_utf8(bytes.clone()).unwrap();
    assert!(text.contains("xmlns:d10p1=\"http://v8.1c.ru/8.2/data/types\""));
    assert!(text.contains(">d10p1:Undefined</dcsset:right>"));
    assert!(text.contains("<dcsset:appearance/>"));
    let decoded = read_form(FormDialect::Designer, &bytes).unwrap();
    assert_eq!(write_form(FormDialect::Designer, &decoded).unwrap(), bytes);
    let mut absent = body.clone();
    let DcsItem::ConditionalAppearance {
        source_empty_appearance,
        filter,
        ..
    } = &mut absent.conditional_appearance[0]
    else {
        unreachable!()
    };
    *source_empty_appearance = false;
    let DcsItem::FilterComparison { right, .. } = &mut filter[0] else {
        unreachable!()
    };
    right[0] = DcsRightValue::TypeQName("d8p1:Undefined".into());
    assert_eq!(
        serde_json::to_value(&body).unwrap(),
        serde_json::to_value(&absent).unwrap()
    );
    let changed = write_form(FormDialect::Designer, &absent).unwrap();
    assert!(
        !String::from_utf8(changed)
            .unwrap()
            .contains("<dcsset:appearance/>")
    );
    let DcsItem::ConditionalAppearance { filter, .. } = &mut absent.conditional_appearance[0]
    else {
        unreachable!()
    };
    let DcsItem::FilterComparison { right, .. } = &mut filter[0] else {
        unreachable!()
    };
    right[0] = DcsRightValue::TypeQName("d8p1:String".into());
    assert_ne!(
        serde_json::to_value(&body).unwrap(),
        serde_json::to_value(&absent).unwrap()
    );
    assert!(
        String::from_utf8(write_form(FormDialect::Designer, &absent).unwrap())
            .unwrap()
            .contains(">d8p1:String</dcsset:right>")
    );
    for bad in [
        text.replace(
            "d10p1:Undefined</dcsset:right>",
            "other:Undefined</dcsset:right>",
        ),
        text.replace("http://v8.1c.ru/8.2/data/types", "urn:unknown:type"),
        text.replace("d10p1:Undefined</dcsset:right>", "d10p1:</dcsset:right>"),
        text.replace("d10p1:Undefined</dcsset:right>", "d10p1:a:b</dcsset:right>"),
        text.replace("d10p1:Undefined</dcsset:right>", "d10p1:1bad</dcsset:right>"),
        text.replace("d10p1:Undefined</dcsset:right>", "d10p1:a$</dcsset:right>"),
        text.replace(
            "<dcsset:appearance/>",
            "<dcsset:appearance unknown=\"true\"/>",
        ),
        text.replace(
            "<dcsset:appearance/>",
            "<dcsset:appearance><Unknown/></dcsset:appearance>",
        ),
    ] {
        assert!(read_form(FormDialect::Designer, bad.as_bytes()).is_err());
    }
    for name in ["Тип", "_Тип1", "Τύπος", "A\u{301}", "\u{10000}Type"] {
        let unicode=text.replace("d10p1:Undefined</dcsset:right>",&format!("d10p1:{name}</dcsset:right>"));
        let decoded=read_form(FormDialect::Designer,unicode.as_bytes()).unwrap();
        assert!(String::from_utf8(write_form(FormDialect::Designer,&decoded).unwrap()).unwrap().contains(&format!(">d10p1:{name}</dcsset:right>")));
    }
}

#[test]
#[ignore = "requires immutable real UH83 source/regeneration witnesses in F laboratory"]
fn genuine_uha83_empty_appearance_and_scoped_type_alias_are_preserved() {
    let lab = std::path::PathBuf::from(std::env::var_os("IBCMD_EDT_LAB").unwrap());
    let witnesses = lab.join("uha-38a-full-regeneration-r1/xml/form-regeneration-xml-witnesses");
    for (id, rel) in [
        (171, "DocumentJournals/ЧекиККМ/Forms/ФормаСписка"),
        (
            217,
            "Documents/ПодтверждениеНулевойСтавкиНДС/Forms/ФормаРабочееМесто",
        ),
        (
            257,
            "InformationRegisters/НДССостояниеРеализации0/Forms/ФормаРабочееМесто",
        ),
    ] {
        let source = std::fs::read(witnesses.join(format!("{id}.source.xml"))).unwrap();
        assert_eq!(
            source,
            std::fs::read(
                lab.join("../04/release-20261001/rc/out/uha8327_db_r1/tree")
                    .join(rel)
                    .join("Ext/Form.xml")
            )
            .unwrap()
        );
        let body = read_form(FormDialect::Designer, &source).unwrap();
        let generated = write_form(FormDialect::Designer, &body).unwrap();
        let original = String::from_utf8(source.clone()).unwrap();
        let output = String::from_utf8(generated.clone()).unwrap();
        if id == 171 {
            assert!(original.contains(">d10p1:Undefined</dcsset:right>"));
            assert!(output.contains("xmlns:d10p1=\"http://v8.1c.ru/8.2/data/types\""));
            assert_eq!(
                original.matches(">d10p1:Undefined</dcsset:right>").count(),
                output.matches(">d10p1:Undefined</dcsset:right>").count()
            );
        } else {
            let expected = original.matches("<dcsset:appearance/>").count();
            assert!(expected > 0);
            assert_eq!(output.matches("<dcsset:appearance/>").count(), expected);
        }
        let returned = read_form(FormDialect::Designer, &generated).unwrap();
        assert_eq!(body.data_attributes.len(), returned.data_attributes.len());
        for (before, after) in body.data_attributes.iter().zip(&returned.data_attributes) {
            assert_eq!(before.dynamic_list.is_some(), after.dynamic_list.is_some());
            if let (Some(before), Some(after)) = (&before.dynamic_list, &after.dynamic_list) {
                assert_eq!(
                    serde_json::to_value(&before.list_settings).unwrap(),
                    serde_json::to_value(&after.list_settings).unwrap()
                );
            }
        }
        let mut paired = 0;
        for attribute in &body.data_attributes {
            if let Some(settings) = attribute
                .dynamic_list
                .as_ref()
                .and_then(|l| l.list_settings.as_ref())
            {
                let edt_path = lab
                    .join("oracle-uha83-r1/authentic-workspace/OracleConfiguration/src")
                    .join(rel)
                    .join("Attributes")
                    .join(attribute.name.as_str())
                    .join("ExtInfo/ListSettings.dcss");
                let edt = std::fs::read(&edt_path).unwrap();
                let decoded = read_list_settings_dcss(&edt).unwrap();
                let mut native_semantics = serde_json::to_value(settings).unwrap();
                let mut edt_semantics = serde_json::to_value(&decoded).unwrap();
                // This existing flag describes the sidecar namespace envelope only.
                native_semantics
                    .as_object_mut()
                    .unwrap()
                    .remove("envelope_without_pal");
                edt_semantics
                    .as_object_mut()
                    .unwrap()
                    .remove("envelope_without_pal");
                assert!(
                    native_semantics == edt_semantics,
                    "paired typed DCS settings differ: {id}"
                );
                assert_eq!(write_list_settings_dcss(&decoded), edt);
                assert_eq!(std::fs::read(edt_path).unwrap(), edt);
                paired += 1;
            }
        }
        assert!(paired > 0);
        assert_eq!(
            std::fs::read(witnesses.join(format!("{id}.source.xml"))).unwrap(),
            source
        );
    }
}

#[test]
fn public_return_keeps_native_lexical_presence_but_type_edits_reject_stale_hashes() {
    use ibcmd_edt::{
        ConversionOptions, Project, ReaderLimits, edt_to_xml, read_xml_source, xml_to_edt,
    };
    use ibcmd_xml::source_tree::{SourceEntry, SourcePath, SourceTree};
    use morph1c_core::ir::{MetadataObject, NamedFormBody, ObjectKind, PropertyValue, Token, Uuid};
    use morph1c_pipeline::{ConvertOptions, Format, read_config, write_config};
    use sha2::{Digest, Sha256};
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
    let mut object = MetadataObject::new(ObjectKind::new("CommonForm"), "ScopedDcs", Uuid([9; 16]));
    object.properties.push((
        morph1c_core::spec::registry::spec_for("CommonForm")
            .unwrap()
            .fields()
            .iter()
            .find(|f| f.name == "formType")
            .unwrap()
            .id,
        PropertyValue::Enum(Token::new("Managed")),
    ));
    object.form_bodies.push(NamedFormBody {
        name: object.name.clone(),
        body: fixture(),
        ordinary_body: None,
        module: None,
        help: vec![],
        help_resources: vec![],
    });
    cfg.objects.push(object);
    let dir = tempfile::tempdir().unwrap();
    write_config(Format::Designer, &cfg, dir.path()).unwrap();
    let config_path = dir.path().join("Configuration.xml");
    let config = String::from_utf8(std::fs::read(&config_path).unwrap())
        .unwrap()
        .replace(
            "</Language>",
            "</Language>\r\n\t\t\t<CommonForm>ScopedDcs</CommonForm>",
        );
    std::fs::write(config_path, config).unwrap();
    let original = read_xml_source(dir.path(), ReaderLimits::default()).unwrap();
    let options = ConversionOptions {
        edt_version: "2025.2.3".into(),
        xml_dialect: "2.21".into(),
        runtime_version: Some("8.5.1".into()),
    };
    let generated = xml_to_edt(&original, &options).unwrap().tree;
    assert_eq!(
        edt_to_xml(&Project::from_tree(generated.clone()).unwrap(), &options)
            .unwrap()
            .tree,
        original
    );
    let path = "src/CommonForms/ScopedDcs/ConditionalAppearance.dcssca";
    let bytes = generated
        .entries()
        .iter()
        .find(|e| e.path().as_str() == path)
        .unwrap()
        .bytes();
    let edited = String::from_utf8(bytes.to_vec())
        .unwrap()
        .replace(":Undefined</right>", ":String</right>")
        .into_bytes();
    assert_ne!(edited.as_slice(), bytes);
    let mut manifest: serde_json::Value = serde_json::from_slice(
        generated
            .entries()
            .iter()
            .find(|e| e.path().as_str() == ".ibcmd-provenance/manifest.json")
            .unwrap()
            .bytes(),
    )
    .unwrap();
    manifest["generated"][path] = format!("{:x}", Sha256::digest(&edited)).into();
    let tree = SourceTree::new(
        generated
            .entries()
            .iter()
            .map(|e| {
                let replacement = match e.path().as_str() {
                    p if p == path => Some(edited.clone()),
                    ".ibcmd-provenance/manifest.json" => {
                        Some(serde_json::to_vec(&manifest).unwrap())
                    }
                    _ => None,
                };
                replacement.map_or_else(
                    || e.clone(),
                    |b| {
                        SourceEntry::from_bytes(SourcePath::new(e.path().as_str()).unwrap(), b)
                            .unwrap()
                    },
                )
            })
            .collect(),
    )
    .unwrap();
    let error = edt_to_xml(&Project::from_tree(tree).unwrap(), &options).unwrap_err();
    assert!(error.to_string().contains("changed"), "{error}");
}
