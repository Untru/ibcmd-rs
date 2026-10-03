use morph1c_core::ir::{MetadataObject, ObjectKind, PropertyValue, Template, Token, Uuid};
use morph1c_pipeline::{ConvertOptions, Format, read_config, write_config};

fn config(body: &[u8]) -> morph1c_core::ir::Configuration {
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
        PropertyValue::Enum(Token::new("DataCompositionSchema")),
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
fn dcs(inner: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><DataCompositionSchema xmlns=\"http://v8.1c.ru/8.1/data-composition-system/schema\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\">{inner}</DataCompositionSchema>"
    )
}
fn project(body: &str) -> Result<Vec<u8>, morph1c_pipeline::ConvertError> {
    let out = tempfile::tempdir().unwrap();
    write_config(Format::Edt, &config(body.as_bytes()), out.path())?;
    Ok(std::fs::read(out.path().join("CommonTemplates/Witness/Template.dcs")).unwrap())
}
const ALIAS: &str = "<v8:TypeSet xmlns:p=\"http://v8.1c.ru/8.1/data/enterprise/current-config\">p:AnyIBRef</v8:TypeSet>";

#[test]
fn dcs_traversal_has_no_former_node_markup_or_depth_ceiling() {
    let mut inner = "<r></r>".repeat(1_048_577);
    inner.push_str(&"<r>".repeat(2_048));
    inner.push_str(ALIAS);
    inner.push_str(&"</r>".repeat(2_048));
    let source = dcs(&inner);
    assert_eq!(
        project(&source).unwrap(),
        source.replace("p:AnyIBRef</", "p:AnyRef</").as_bytes()
    );
}

#[test]
fn arbitrary_cdata_comments_pi_and_only_qualified_qname_characters_survive() {
    let inner = format!(
        "<!--before--><?audit value?><query><![CDATA[SELECT p:AnyIBRef\nWHERE a < b && c > d]]></query>{ALIAS}<foreign xmlns:v8=\"urn:other\">{ALIAS}</foreign>"
    );
    let source = dcs(&inner);
    let expected = source.replacen("p:AnyIBRef</v8:TypeSet>", "p:AnyRef</v8:TypeSet>", 1);
    assert_eq!(project(&source).unwrap(), expected.as_bytes());
    let cdata = dcs(&ALIAS.replace("p:AnyIBRef</", "<![CDATA[p:AnyIBRef]]></"));
    let out = tempfile::tempdir().unwrap();
    write_config(Format::Edt, &config(cdata.as_bytes()), out.path()).unwrap();
    assert_eq!(
        std::fs::read(out.path().join("CommonTemplates/Witness/Template.dcs")).unwrap(),
        cdata.replace("p:AnyIBRef]]>", "p:AnyRef]]>").as_bytes()
    );
    let restored = read_config(Format::Edt, out.path(), &ConvertOptions::default())
        .unwrap()
        .0;
    let template = &restored
        .objects
        .iter()
        .find(|o| o.name == "Witness")
        .unwrap()
        .templates[0];
    assert_eq!(template.body.as_deref().unwrap(), cdata.as_bytes());
    let split = dcs(&ALIAS.replace("p:AnyIBRef</", "<![CDATA[p:AnyI]]><!--keep-->B&#x52;ef</"));
    assert_eq!(
        project(&split).unwrap(),
        split
            .replace("AnyI]]>", "Any]]>")
            .replace("-->B&#x52;", "-->&#x52;")
            .as_bytes()
    );
    // Native-origin emission preserves the exact original lexical payload.
    let out = tempfile::tempdir().unwrap();
    write_config(Format::Designer, &config(split.as_bytes()), out.path()).unwrap();
    assert_eq!(
        std::fs::read(out.path().join("CommonTemplates/Witness/Ext/Template.xml")).unwrap(),
        [b"\xef\xbb\xbf".as_slice(), split.as_bytes()].concat()
    );
}

#[test]
fn namespace_scopes_whitespace_and_qname_qualification_are_not_generalized() {
    let source = dcs(&format!(
        "{ALIAS}<group xmlns:p=\"urn:other\"><v8:TypeSet>p:AnyIBRef</v8:TypeSet></group><v8:TypeSet xmlns:p=\"http://v8.1c.ru/8.1/data/enterprise/current-config\"> p:AnyIBRef </v8:TypeSet>"
    ));
    assert_eq!(
        project(&source).unwrap(),
        source.replacen("p:AnyIBRef</", "p:AnyRef</", 1).as_bytes()
    );
    for malformed in [
        dcs(&ALIAS.replace("xmlns:p=", "extra=\"keep\" xmlns:p=")),
        dcs(&ALIAS.replace("p:AnyIBRef</", "p:AnyIBRef<child/></")),
        dcs("<p:x/>"),
        dcs("<x p:a=\"1\"/>"),
        dcs("<x xmlns:a=\"urn:a\" xmlns:b=\"urn:a\" a:v=\"1\" b:v=\"2\"/>"),
        dcs("<x xmlns:xml=\"urn:wrong\"/>"),
        dcs("<x xmlns:xmlns=\"urn:wrong\"/>"),
        dcs("<x><y></x></y>"),
        dcs("<query><![CDATA[unterminated</query>"),
        dcs("<query>&unknown;</query>"),
        dcs("<query>&#0;</query>"),
        dcs("<query>]]></query>"),
        dcs("<query><!--bad--comment--></query>"),
        dcs("<x a=\"<\"/>"),
        format!(
            "<!DOCTYPE DataCompositionSchema [<!ENTITY x 'value'>]>{}",
            dcs("<query>&x;</query>")
        ),
        format!("{}<second/>", dcs("")),
        format!("{}after", dcs("")),
    ] {
        assert!(project(&malformed).is_err(), "{malformed}");
    }
}
