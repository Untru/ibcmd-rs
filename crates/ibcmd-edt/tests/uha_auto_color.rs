use formats_xml::form::{FormDialect, read_form, write_form};
use morph1c_core::{
    ir::{FormBody, FormControlKind, FormItem, PropertyValue},
    spec::forms::controls::button as bt,
    version::{FormatVersion, with_roundtrip_target, with_source_version},
};
fn form() -> FormBody {
    let mut form = FormBody::new();
    let item = FormItem::new(FormControlKind::new("Button"), "Button", 5);
    form.items.push(item);
    let original = write(FormDialect::Designer, &form);
    let mut form = read(FormDialect::Designer, &original);
    form.items[0].designer_button_back_color_auto = true;
    form
}
fn write(dialect: FormDialect, form: &FormBody) -> Vec<u8> {
    with_roundtrip_target(FormatVersion::new(2, 20), || write_form(dialect, form)).unwrap()
}
fn read(dialect: FormDialect, bytes: &[u8]) -> FormBody {
    with_source_version(Some(FormatVersion::new(2, 20)), || {
        read_form(dialect, bytes)
    })
    .unwrap()
}
#[test]
fn exact_button_auto_is_nullable_and_edits_win() {
    let original = write(FormDialect::Designer, &form());
    assert!(String::from_utf8_lossy(&original).contains("<BackColor>auto</BackColor>"));
    let mut typed = read(FormDialect::Designer, &original);
    assert!(typed.items[0].get(bt::F_BACK_COLOR).is_none());
    assert_eq!(write(FormDialect::Designer, &typed), original);
    let edt = write(FormDialect::Edt, &typed);
    assert!(!String::from_utf8_lossy(&edt).contains("backColor"));
    let reread = read(FormDialect::Edt, &edt);
    assert_eq!(
        serde_json::to_vec(&typed).unwrap(),
        serde_json::to_vec(&reread).unwrap()
    );
    typed.items[0].properties.push((
        bt::F_BACK_COLOR,
        PropertyValue::Ref("Style.ToolTipBackColor".into()),
    ));
    let native = write(FormDialect::Designer, &typed);
    assert!(
        String::from_utf8_lossy(&native).contains("<BackColor>style:ToolTipBackColor</BackColor>")
    );
    let edt = write(FormDialect::Edt, &typed);
    let returned = read(FormDialect::Edt, &edt);
    assert_eq!(
        typed.items[0].get(bt::F_BACK_COLOR),
        returned.items[0].get(bt::F_BACK_COLOR)
    );
}
#[test]
fn auto_exception_rejects_other_slots_attributes_duplicates_and_nested_content() {
    let native = String::from_utf8(write(FormDialect::Designer, &form())).unwrap();
    for replacement in [
        "<BackColor other=\"x\">auto</BackColor>",
        "<BackColor>auto<Extra/></BackColor>",
        "<BackColor>auto</BackColor><BackColor>auto</BackColor>",
        "<BackColor xml:space=\"preserve\">auto</BackColor>",
        "<BackColor xmlns=\"urn:unknown\">auto</BackColor>",
        "<BackColor>AUTO</BackColor>",
        "<BorderColor>auto</BorderColor>",
    ] {
        let source = native.replace("<BackColor>auto</BackColor>", replacement);
        assert!(
            with_source_version(Some(FormatVersion::new(2, 20)), || read_form(
                FormDialect::Designer,
                source.as_bytes()
            ))
            .is_err(),
            "accepted {replacement}"
        );
    }
}
#[test]
#[ignore = "Read-only genuine UH original/EDT/nativeSDK nullable Button.BackColor witness on F"]
fn genuine_button_nullable_color_matches_sdk_and_preserves_original_bytes() {
    use std::path::Path;
    let rel = Path::new("Reports/ДвиженияНастраиваемойОтчетности/Forms/ФормаОтчета");
    let original = std::fs::read(
        Path::new(r"F:\ibcmd\lab\04\release-20261001\rc\out\uha8327_db_r1\tree")
            .join(rel)
            .join("Ext/Form.xml"),
    )
    .unwrap();
    let form = read(FormDialect::Designer, &original);
    assert_eq!(write(FormDialect::Designer, &form), original);
    let sdk = std::fs::read(
        Path::new(r"F:\ibcmd\lab\07\native-reference-uha83-r1\native-xml")
            .join(rel)
            .join("Ext/Form.xml"),
    )
    .unwrap();
    let sdk_form = read(FormDialect::Designer, &sdk);
    fn buttons(items: &[FormItem], out: &mut Vec<(String, Option<PropertyValue>)>) {
        for item in items {
            if item.kind.as_str() == "Button" {
                out.push((item.name.clone(), item.get(bt::F_BACK_COLOR).cloned()))
            }
            buttons(&item.children, out)
        }
    }
    let mut a = vec![];
    let mut b = vec![];
    buttons(&form.items, &mut a);
    buttons(&sdk_form.items, &mut b);
    assert_eq!(a, b);
    let edt = std::fs::read(
        Path::new(r"F:\ibcmd\lab\07\oracle-uha83-r1\authentic-workspace\OracleConfiguration\src")
            .join(rel)
            .join("Form.form"),
    )
    .unwrap();
    let edt_form = read(FormDialect::Edt, &edt);
    let mut c = vec![];
    buttons(&edt_form.items, &mut c);
    assert_eq!(a, c);
}

#[test]
fn public_auto_color_roundtrip_is_exact_and_color_edit_rejects_forged_hash() {
    use ibcmd_edt::{
        ConversionOptions, Project, ReaderLimits, edt_to_xml, read_xml_source, xml_to_edt,
    };
    use ibcmd_xml::source_tree::{SourceEntry, SourcePath, SourceTree};
    use morph1c_core::ir::{MetadataObject, NamedFormBody, ObjectKind, Token, Uuid};
    use morph1c_pipeline::{ConvertOptions, Format, read_config, write_config};
    use sha2::{Digest, Sha256};
    let fixture = std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/subsystem-ci/src"
    ));
    let mut cfg = read_config(Format::Designer, fixture, &ConvertOptions::default())
        .unwrap()
        .0;
    let form = form();
    let mut obj = MetadataObject::new(ObjectKind::new("CommonForm"), "AutoColor", Uuid([42; 16]));
    let form_type = morph1c_core::spec::registry::spec_for("CommonForm")
        .unwrap()
        .fields()
        .iter()
        .find(|f| f.name == "formType")
        .unwrap()
        .id;
    obj.properties
        .push((form_type, PropertyValue::Enum(Token::new("Managed"))));
    obj.form_bodies.push(NamedFormBody {
        name: "AutoColor".into(),
        body: form,
        ordinary_body: None,
        module: None,
        help: vec![],
        help_resources: vec![],
    });
    cfg.objects.push(obj);
    let dir = tempfile::tempdir().unwrap();
    write_config(Format::Designer, &cfg, dir.path()).unwrap();
    let path = dir.path().join("Configuration.xml");
    let root = String::from_utf8(std::fs::read(&path).unwrap())
        .unwrap()
        .replace(
            "</Language>",
            "</Language>\r\n\t\t\t<CommonForm>AutoColor</CommonForm>",
        );
    std::fs::write(path, root).unwrap();
    let options = ConversionOptions {
        edt_version: "2025.2.3".into(),
        xml_dialect: "2.21".into(),
        runtime_version: Some("8.5.1".into()),
    };
    let original = read_xml_source(dir.path(), ReaderLimits::default()).unwrap();
    let generated = xml_to_edt(&original, &options).unwrap().tree;
    let returned = edt_to_xml(&Project::from_tree(generated.clone()).unwrap(), &options)
        .unwrap()
        .tree;
    assert_eq!(returned, original);
    let path = "src/CommonForms/AutoColor/Form.form";
    let bytes = generated
        .entries()
        .iter()
        .find(|e| e.path().as_str() == path)
        .unwrap()
        .bytes();
    let mut edited_form = with_source_version(Some(FormatVersion::new(2, 21)), || {
        read_form(FormDialect::Edt, bytes)
    })
    .unwrap();
    edited_form.items[0].properties.push((
        bt::F_BACK_COLOR,
        PropertyValue::Ref("Style.ToolTipBackColor".into()),
    ));
    let edited = with_roundtrip_target(FormatVersion::new(2, 21), || {
        write_form(FormDialect::Edt, &edited_form)
    })
    .unwrap();
    assert!(edited != bytes);
    let hash = format!("{:x}", Sha256::digest(&edited));
    let mut manifest: serde_json::Value = serde_json::from_slice(
        generated
            .entries()
            .iter()
            .find(|e| e.path().as_str() == ".ibcmd-provenance/manifest.json")
            .unwrap()
            .bytes(),
    )
    .unwrap();
    manifest["generated"][path] = serde_json::Value::String(hash);
    let altered = SourceTree::new(
        generated
            .entries()
            .iter()
            .map(|e| {
                if e.path().as_str() == path {
                    SourceEntry::from_bytes(SourcePath::new(path).unwrap(), edited.clone()).unwrap()
                } else if e.path().as_str() == ".ibcmd-provenance/manifest.json" {
                    SourceEntry::from_bytes(
                        e.path().clone(),
                        serde_json::to_vec(&manifest).unwrap(),
                    )
                    .unwrap()
                } else {
                    e.clone()
                }
            })
            .collect(),
    )
    .unwrap();
    assert!(
        edt_to_xml(&Project::from_tree(altered).unwrap(), &options)
            .unwrap_err()
            .to_string()
            .contains("changed")
    );
}
