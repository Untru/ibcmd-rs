use formats_xml::form::{FormDialect, read_form, write_form};
use morph1c_core::{
    ir::{FormBody, FormControlKind, FormItem, Lang, PropertyValue},
    spec::forms::form_root as fr,
    version::{FormatVersion, with_roundtrip_target, with_source_version},
};
fn write(d: FormDialect, b: &FormBody) -> Vec<u8> {
    with_roundtrip_target(FormatVersion::new(2, 20), || write_form(d, b)).unwrap()
}
fn read(d: FormDialect, b: &[u8]) -> FormBody {
    with_source_version(Some(FormatVersion::new(2, 20)), || read_form(d, b)).unwrap()
}
fn swap(bytes: Vec<u8>, a: &str, b: &str) -> Vec<u8> {
    let s = String::from_utf8(bytes).unwrap();
    assert!(s.contains(a));
    assert!(s.contains(b));
    s.replacen(a, "__slot_placeholder__", 1)
        .replacen(b, a, 1)
        .replacen("__slot_placeholder__", b, 1)
        .into_bytes()
}
fn form() -> FormBody {
    let mut b = FormBody::new();
    b.attributes = vec![
        (fr::F_WIDTH, PropertyValue::Int(100)),
        (fr::F_HEIGHT, PropertyValue::Int(200)),
    ];
    b
}
#[test]
fn singleton_order_is_lexical_values_and_child_order_remain_editable() {
    let mut b = form();
    b.items = vec![
        FormItem::new(FormControlKind::new("Button"), "First", 1),
        FormItem::new(FormControlKind::new("Button"), "Second", 2),
    ];
    let baseline = write(
        FormDialect::Designer,
        &read(FormDialect::Designer, &write(FormDialect::Designer, &b)),
    );
    let source = swap(
        baseline.clone(),
        "<Width>100</Width>",
        "<Height>200</Height>",
    );
    let mut decoded = read(FormDialect::Designer, &source);
    assert_eq!(write(FormDialect::Designer, &decoded), source);
    assert_eq!(
        serde_json::to_value(&decoded).unwrap(),
        serde_json::to_value(read(FormDialect::Designer, &baseline)).unwrap()
    );
    decoded
        .attributes
        .iter_mut()
        .find(|(id, _)| *id == fr::F_WIDTH)
        .unwrap()
        .1 = PropertyValue::Int(777);
    decoded.items.reverse();
    let edited = read(
        FormDialect::Designer,
        &write(FormDialect::Designer, &decoded),
    );
    assert_eq!(
        edited
            .attributes
            .iter()
            .find(|(id, _)| *id == fr::F_WIDTH)
            .unwrap()
            .1,
        PropertyValue::Int(777)
    );
    assert_eq!(edited.items[0].name, "Second");
    assert_eq!(edited.items[1].name, "First");
    assert!(
        decoded.source_wire_order.as_ref().unwrap().scopes["root"]
            .iter()
            .all(|s| s != "100" && s != "200")
    );
}
#[test]
fn interleaved_known_repeats_keep_typed_array_order_and_edits() {
    let mut b = form();
    b.title = Some(PropertyValue::Localized(vec![
        (Lang::new("ru"), "Первый".into()),
        (Lang::new("en"), "Second".into()),
    ]));
    b.excluded_commands = vec!["One".into(), "Two".into()];
    let bytes = write(FormDialect::Edt, &b);
    // Fully typed repeated command region legally interleaved by another known property.
    let source = swap(
        bytes,
        "<excludedCommands>Two</excludedCommands>",
        "<width>100</width>",
    );
    let mut decoded = read(FormDialect::Edt, &source);
    assert_eq!(write(FormDialect::Edt, &decoded), source);
    assert_eq!(decoded.excluded_commands, vec!["One", "Two"]);
    decoded.excluded_commands = vec!["Edited".into(), "New".into(), "Added".into()];
    let edited = read(FormDialect::Edt, &write(FormDialect::Edt, &decoded));
    assert_eq!(edited.excluded_commands, decoded.excluded_commands);
}
#[test]
fn malformed_unknown_duplicate_and_wrong_kind_slots_fail_closed() {
    let source = write(FormDialect::Designer, &form());
    for replacement in [
        "<Width>100</Width><Width>100</Width>",
        "<UnknownSlot>100</UnknownSlot>",
    ] {
        let bad = String::from_utf8(source.clone())
            .unwrap()
            .replace("<Width>100</Width>", replacement);
        assert!(read_form(FormDialect::Designer, bad.as_bytes()).is_err());
    }
    let source = swap(source, "<Width>100</Width>", "<Height>200</Height>");
    let decoded = read(FormDialect::Designer, &source);
    for tags in [
        vec!["Width".into(), "Width".into()],
        vec!["UnknownSlot".into()],
        vec!["enableDrag".into()],
    ] {
        let mut bad = decoded.clone();
        bad.source_wire_order
            .as_mut()
            .unwrap()
            .scopes
            .insert("root".into(), tags);
        assert!(
            with_roundtrip_target(FormatVersion::new(2, 20), || write_form(
                FormDialect::Designer,
                &bad
            ))
            .is_err()
        );
    }
}
#[test]
#[ignore = "read-only bounded genuine UH regeneration captures in F laboratory"]
fn genuine_regeneration_capture_census() {
    let r = std::path::Path::new(
        "F:/ibcmd/lab/07/uha-38a-full-regeneration-r1/xml/form-regeneration-xml-witnesses",
    );
    let mut failed = Vec::new();
    let lab = std::path::Path::new("F:/ibcmd/lab/07/uha-wire-order-captures-r1");
    std::fs::create_dir_all(lab).unwrap();
    let ids: Vec<usize> = if std::env::var("IBCMD_WIRE_REMAINING_ONLY").is_ok() {
        vec![8, 41, 42, 59, 79, 135, 136, 145, 243, 244, 259]
    } else {
        (1..=1024).collect()
    };
    let processed = ids.len();
    for i in ids {
        let source = std::fs::read(r.join(format!("{i}.source.xml"))).unwrap();
        let body = read(FormDialect::Designer, &source);
        let generated = write(FormDialect::Designer, &body);
        let src_tree = formats_xml::parse(&source).unwrap();
        let dst_tree = formats_xml::parse(&generated).unwrap();
        assert_eq!(
            format!("{:?}", src_tree.root),
            format!("{:?}", dst_tree.root),
            "ordered attributes/text/children witness {i}"
        );
        if generated != source {
            failed.push(i);
            std::fs::write(lab.join(format!("{i}.generated.xml")), &generated).unwrap();
        }
        assert_eq!(
            serde_json::to_value(&body).unwrap(),
            serde_json::to_value(read(FormDialect::Designer, &generated)).unwrap(),
            "semantic source witness {i}"
        );
    }
    eprintln!(
        "genuine source captures processed={} rawdiff={} first32={:?}",
        processed,
        failed.len(),
        failed.iter().take(32).collect::<Vec<_>>()
    );
    assert_eq!(
        failed,
        vec![8, 79],
        "only the two independently typed empty-element spellings differ"
    );
}

#[test]
#[ignore = "read-only genuine nested ConditionalAppearance type in F laboratory"]
fn nested_column_type_depth_is_bound_and_edited_type_wins() {
    let source=std::fs::read("F:/ibcmd/lab/07/uha-38a-full-regeneration-r1/xml/form-regeneration-xml-witnesses/59.source.xml").unwrap();
    let mut body = read(FormDialect::Designer, &source);
    assert_eq!(write(FormDialect::Designer, &body), source);
    for bad in [
        "xmlns:d7p1=\"http://example.invalid/entext\"",
        "not-a-declaration=\"http://v8.1c.ru/8.3/data/entext\"",
    ] {
        let bad = String::from_utf8(source.clone())
            .unwrap()
            .replace("xmlns:d7p1=\"http://v8.1c.ru/8.3/data/entext\"", bad);
        assert!(read_form(FormDialect::Designer, bad.as_bytes()).is_err());
    }
    fn change(a: &mut morph1c_core::ir::FormDataAttribute) -> bool {
        if let Some(t) = &mut a.value_type
            && t.parts.iter().any(|p| p.id == "ConditionalAppearance")
        {
            t.parts.iter_mut().for_each(|p| {
                if p.id == "ConditionalAppearance" {
                    p.id = "Boolean".into();
                }
            });
            return true;
        }
        a.columns.iter_mut().any(change)
            || a.additional_columns
                .iter_mut()
                .any(|group| group.columns.iter_mut().any(change))
    }
    assert!(body.data_attributes.iter_mut().any(change));
    let edited = write(FormDialect::Designer, &body);
    assert!(
        String::from_utf8(edited.clone())
            .unwrap()
            .contains("<v8:Type>xs:boolean</v8:Type>")
    );
    assert!(
        !String::from_utf8(edited.clone())
            .unwrap()
            .contains("d7p1:ConditionalAppearance")
    );
    assert_eq!(
        serde_json::to_value(&body).unwrap(),
        serde_json::to_value(read(FormDialect::Designer, &edited)).unwrap()
    );
}

#[test]
fn public_property_order_never_masks_edits_with_forged_generated_hash() {
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
    let mut obj = MetadataObject::new(ObjectKind::new("CommonForm"), "WireOrder", Uuid([42; 16]));
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
        name: "WireOrder".into(),
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
            "</Language>\r\n\t\t\t<CommonForm>WireOrder</CommonForm>",
        );
    std::fs::write(path, root).unwrap();
    let options = ConversionOptions {
        edt_version: "2025.2.3".into(),
        xml_dialect: "2.21".into(),
        runtime_version: Some("8.5.1".into()),
    };
    let native_form = dir.path().join("CommonForms/WireOrder/Ext/Form.xml");
    let native = std::fs::read(&native_form).unwrap();
    std::fs::write(
        &native_form,
        swap(native, "<Width>100</Width>", "<Height>200</Height>"),
    )
    .unwrap();
    let original = read_xml_source(dir.path(), ReaderLimits::default()).unwrap();
    let generated = xml_to_edt(&original, &options).unwrap().tree;
    let returned = edt_to_xml(&Project::from_tree(generated.clone()).unwrap(), &options)
        .unwrap()
        .tree;
    assert_eq!(returned, original);
    let path = "src/CommonForms/WireOrder/Form.form";
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
    edited_form
        .attributes
        .iter_mut()
        .find(|(id, _)| *id == fr::F_WIDTH)
        .unwrap()
        .1 = PropertyValue::Int(999);
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

#[test]
fn source_property_order_is_reused_only_for_its_explicit_profile() {
    for dialect in [FormDialect::Designer, FormDialect::Edt] {
        let (a, b) = if dialect == FormDialect::Designer {
            ("<Width>100</Width>", "<Height>200</Height>")
        } else {
            ("<width>100</width>", "<height>200</height>")
        };
        let source = swap(write(dialect, &form()), a, b);
        let decoded = read(dialect, &source);
        let target =
            with_roundtrip_target(FormatVersion::new(2, 21), || write_form(dialect, &decoded))
                .unwrap();
        let text = String::from_utf8(target).unwrap();
        assert!(text.find(a).unwrap() < text.find(b).unwrap());
    }
}
