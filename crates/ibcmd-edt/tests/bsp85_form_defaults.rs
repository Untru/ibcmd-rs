use formats_xml::form::{FormDialect, read_form, write_form};
use morph1c_core::{
    ir::{FormBody, FormControlKind, FormItem, PropertyValue, Token},
    spec::forms::controls::{button as bt, table as tb},
    version::{FormatVersion, with_roundtrip_target, with_source_version},
};

fn version() -> FormatVersion {
    FormatVersion::new(2, 21)
}
fn write(d: FormDialect, body: &FormBody) -> Vec<u8> {
    with_roundtrip_target(version(), || write_form(d, body)).unwrap()
}
fn read(d: FormDialect, bytes: &[u8]) -> FormBody {
    with_source_version(Some(version()), || read_form(d, bytes)).unwrap()
}
fn body(lines: bool, importance: &str) -> FormBody {
    let mut b = FormBody::new();
    let mut table = FormItem::new(FormControlKind::new("Table"), "T", 1);
    table.properties = vec![
        (tb::F_HORIZONTAL_LINES, PropertyValue::Bool(lines)),
        (tb::F_VERTICAL_LINES, PropertyValue::Bool(lines)),
    ];
    let mut button = FormItem::new(FormControlKind::new("Button"), "B", 2);
    button.properties.push((
        bt::F_BUTTON_IMPORTANCE,
        PropertyValue::Enum(Token::new(importance)),
    ));
    b.items = vec![table, button];
    read(FormDialect::Designer, &write(FormDialect::Designer, &b))
}
fn contains(bytes: &[u8], text: &str) -> bool {
    std::str::from_utf8(bytes).unwrap().contains(text)
}

#[test]
fn xml221_defaults_preserve_typed_main_false_and_nondefaults() {
    let seed = body(false, "Main");
    let edt = write(FormDialect::Edt, &seed);
    assert!(!contains(&edt, "<horizontalLines>"));
    assert!(!contains(&edt, "<buttonImportance>"));
    let current = read(FormDialect::Edt, &edt);
    assert_eq!(
        current.items[0]
            .properties
            .iter()
            .find(|(id, _)| *id == tb::F_HORIZONTAL_LINES)
            .unwrap()
            .1,
        PropertyValue::Bool(false)
    );
    assert_eq!(
        current.items[1]
            .properties
            .iter()
            .find(|(id, _)| *id == bt::F_BUTTON_IMPORTANCE)
            .unwrap()
            .1,
        PropertyValue::Enum(Token::new("Main"))
    );
    let native = write(FormDialect::Designer, &current);
    assert!(contains(
        &native,
        "<HorizontalLines>false</HorizontalLines>"
    ));
    assert!(contains(
        &native,
        "<ButtonImportance>Main</ButtonImportance>"
    ));
    assert_eq!(write(FormDialect::Edt, &current), edt);
    let seed = body(true, "Normal");
    let native = write(FormDialect::Designer, &seed);
    assert!(!contains(&native, "<HorizontalLines>"));
    assert!(!contains(&native, "<ButtonImportance>"));
    let current = read(FormDialect::Designer, &native);
    let edt = write(FormDialect::Edt, &current);
    assert!(contains(&edt, "<horizontalLines>true</horizontalLines>"));
    assert!(contains(
        &edt,
        "<buttonImportance>Normal</buttonImportance>"
    ));
    assert_eq!(write(FormDialect::Designer, &current), native);
    assert!(contains(
        &write(FormDialect::Designer, &body(false, "Supplementary")),
        "<ButtonImportance>Supplementary</ButtonImportance>"
    ));
}

#[test]
fn explicit_default_presence_is_lexical_and_current_edits_win() {
    let native = write(FormDialect::Designer, &body(true, "Normal"));
    let source = String::from_utf8(native)
        .unwrap()
        .replace(
            "<Table name=\"T\" id=\"1\">",
            "<Table name=\"T\" id=\"1\">\r\n\t\t\t<HorizontalLines>true</HorizontalLines>",
        )
        .replace(
            "<Button name=\"B\" id=\"2\">",
            "<Button name=\"B\" id=\"2\">\r\n\t\t\t<ButtonImportance>Normal</ButtonImportance>",
        );
    let mut current = read(FormDialect::Designer, source.as_bytes());
    assert_eq!(write(FormDialect::Designer, &current), source.as_bytes());
    assert!(
        !serde_json::to_string(&current)
            .unwrap()
            .contains("source_xml221_default_presence")
    );
    current.items[0]
        .properties
        .iter_mut()
        .find(|(id, _)| *id == tb::F_HORIZONTAL_LINES)
        .unwrap()
        .1 = PropertyValue::Bool(false);
    current.items[1]
        .properties
        .iter_mut()
        .find(|(id, _)| *id == bt::F_BUTTON_IMPORTANCE)
        .unwrap()
        .1 = PropertyValue::Enum(Token::new("Supplementary"));
    let edited = write(FormDialect::Designer, &current);
    assert!(contains(
        &edited,
        "<HorizontalLines>false</HorizontalLines>"
    ));
    assert!(contains(
        &edited,
        "<ButtonImportance>Supplementary</ButtonImportance>"
    ));
    assert!(!contains(
        &edited,
        "<ButtonImportance>Normal</ButtonImportance>"
    ));
    let edt = write(FormDialect::Edt, &body(false, "Main"));
    let source = String::from_utf8(edt).unwrap().replace(
        "<id>2</id>",
        "<id>2</id>\r\n    <buttonImportance>Main</buttonImportance>",
    );
    assert_eq!(
        write(FormDialect::Edt, &read(FormDialect::Edt, source.as_bytes())),
        source.as_bytes()
    );
}

#[test]
fn malformed_presence_or_duplicate_default_slot_never_masks_input() {
    let native = write(FormDialect::Designer, &body(true, "Normal"));
    let mut current = read(FormDialect::Designer, &native);
    current
        .source_xml221_default_presence
        .as_mut()
        .unwrap()
        .scopes
        .insert("root/Table:1".into(), vec!["UnknownField".into()]);
    assert!(
        with_roundtrip_target(version(), || write_form(FormDialect::Designer, &current)).is_err()
    );
    let edt = write(FormDialect::Edt, &body(false, "Main"));
    let duplicate = String::from_utf8(edt).unwrap().replace("<id>2</id>",
        "<id>2</id>\r\n    <buttonImportance>Main</buttonImportance>\r\n    <buttonImportance>Main</buttonImportance>");
    assert!(
        with_source_version(Some(version()), || read_form(
            FormDialect::Edt,
            duplicate.as_bytes()
        ))
        .is_err()
    );
}

#[test]
fn xml221_picture_color_order_keeps_current_values_and_native_source_order() {
    use morph1c_core::spec::forms::controls::label_decoration as ld;
    let mut body = FormBody::new();
    let mut picture = FormItem::new(FormControlKind::new("PictureDecoration"), "P", 3);
    picture.ext_info = vec![
        (
            ld::F_EXT_PICTURE_COLOR,
            PropertyValue::Ref("Web.Red".into()),
        ),
        (
            ld::F_EXT_FILE_DRAG_MODE,
            PropertyValue::Enum(Token::new("AsFile")),
        ),
    ];
    body.items.push(picture);
    fn before(bytes: &[u8], first: &str, second: &str) {
        let text = std::str::from_utf8(bytes).unwrap();
        assert!(text.find(first).unwrap() < text.find(second).unwrap());
    }
    let modern = write(FormDialect::Designer, &body);
    before(&modern, "<FileDragMode>", "<PictureColor>");
    let old = with_roundtrip_target(FormatVersion::new(2, 20), || {
        write_form(FormDialect::Designer, &body)
    })
    .unwrap();
    before(&old, "<PictureColor>", "<FileDragMode>");
    let native_source = String::from_utf8(modern).unwrap().replace(
        "<FileDragMode>AsFile</FileDragMode>\r\n\t\t\t<PictureColor>web:Red</PictureColor>",
        "<PictureColor>web:Red</PictureColor>\r\n\t\t\t<FileDragMode>AsFile</FileDragMode>",
    );
    before(native_source.as_bytes(), "<PictureColor>", "<FileDragMode>");
    let mut current = read(FormDialect::Designer, native_source.as_bytes());
    assert_eq!(
        write(FormDialect::Designer, &current),
        native_source.as_bytes()
    );
    current.items[0]
        .ext_info
        .iter_mut()
        .find(|(id, _)| *id == ld::F_EXT_PICTURE_COLOR)
        .unwrap()
        .1 = PropertyValue::Ref("Web.Blue".into());
    let edited = write(FormDialect::Designer, &current);
    before(&edited, "<PictureColor>", "<FileDragMode>");
    assert!(contains(&edited, "web:Blue"));
    assert!(!contains(&edited, "web:Red"));
    let canonical = read(FormDialect::Edt, &write(FormDialect::Edt, &current));
    let projected = write(FormDialect::Designer, &canonical);
    before(&projected, "<FileDragMode>", "<PictureColor>");
    assert!(contains(&projected, "web:Blue"));
}

#[test]
#[ignore = "read-only genuine BSP85 855-form census; requires F lab corpora"]
fn genuine_bsp85_all_forms_match_known_defaults_and_keep_native_source() {
    use std::{collections::BTreeMap, fs, path::Path};
    fn selected(bytes: &[u8]) -> BTreeMap<String, String> {
        let text = std::str::from_utf8(bytes).unwrap();
        // Exact slot comparisons use descriptor grammar, not a final payload normalization.
        let doc = formats_xml::parse(text.as_bytes()).unwrap();
        fn visit(
            el: &formats_xml::descriptor::Element,
            path: &str,
            out: &mut BTreeMap<String, String>,
        ) {
            let path = if el.local == "Button" || el.local == "Table" {
                format!("{path}/{}:{}", el.local, el.attr("id").unwrap().value)
            } else {
                path.into()
            };
            for n in &el.children {
                if matches!(
                    n.local.as_str(),
                    "ButtonImportance" | "HorizontalLines" | "VerticalLines"
                ) {
                    out.insert(format!("{path}/{}", n.local), n.text.clone());
                } else {
                    visit(n, &path, out);
                }
            }
        }
        let mut out = BTreeMap::new();
        visit(&doc.root, "root", &mut out);
        out
    }
    let lab = std::env::var("IBCMD_EDT_LAB").unwrap();
    let lab = Path::new(&lab);
    let taxonomy: serde_json::Value =
        serde_json::from_slice(&fs::read(lab.join("bsp85-form-field-taxonomy-r1.json")).unwrap())
            .unwrap();
    let native_root = lab.join("native-reference-bsp85-r4/native-xml");
    let edt_root = lab.join("oracle-bsp85-r3/authentic-workspace/OracleConfiguration/src");
    let mut picture_context = BTreeMap::new();
    for entry in fs::read_dir(edt_root.join("CommonPictures")).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().to_str().unwrap().to_owned();
        let descriptor = entry.path().join(format!("{name}.mdo"));
        let root = formats_xml::parse(&fs::read(descriptor).unwrap())
            .unwrap()
            .root;
        picture_context.insert(
            format!("CommonPicture.{name}"),
            root.child("transparentPixel").is_some(),
        );
    }
    fn attached(anchor: &Path, native: bool) -> FormBody {
        use morph1c_core::ir::{MetadataObject, ObjectKind, Uuid};
        use morph1c_pipeline::{Format, attach_form_body};
        let name = anchor.file_stem().unwrap().to_str().unwrap();
        let mut obj = MetadataObject::new(ObjectKind::new("CommonForm"), name, Uuid([0; 16]));
        with_source_version(Some(version()), || {
            attach_form_body(
                if native {
                    Format::Designer
                } else {
                    Format::Edt
                },
                "CommonForm",
                anchor,
                &mut obj,
            )
        })
        .unwrap();
        assert_eq!(obj.form_bodies.len(), 1);
        obj.form_bodies.pop().unwrap().body
    }
    let mut count = 0;
    let mut raw_equal = 0;
    let mut residual = Vec::new();
    let mut source_failures = Vec::new();
    let mut edt_lexical_differences = Vec::new();
    let witnesses = lab.join("bsp85-default-source-guard-witnesses-r3");
    fs::create_dir_all(&witnesses).unwrap();
    for row in taxonomy["rows"].as_array().unwrap() {
        let path = row["path"].as_str().unwrap();
        if !path.ends_with("/Ext/Form.xml") {
            continue;
        }
        let owner = Path::new(path.strip_suffix("/Ext/Form.xml").unwrap());
        let name = owner.file_name().unwrap().to_str().unwrap();
        let edt_anchor = edt_root.join(owner).join(format!("{name}.mdo"));
        let original_edt = fs::read(edt_root.join(owner).join("Form.form")).unwrap();
        let original_native = fs::read(native_root.join(path)).unwrap();
        let mut actual_body = attached(&edt_anchor, false);
        formats_xml::form::resolve_common_picture_transparency(
            &mut actual_body,
            &picture_context,
            false,
        )
        .unwrap();
        let generated = write(FormDialect::Designer, &actual_body);
        assert_eq!(
            selected(&generated),
            selected(&original_native),
            "known typed defaults mismatch: {path}"
        );
        for (dialect, original, label) in [
            (FormDialect::Edt, &original_edt, "edt"),
            (FormDialect::Designer, &original_native, "native"),
        ] {
            let regenerated = write(dialect, &read(dialect, original));
            if regenerated != *original {
                let index = source_failures.len() + edt_lexical_differences.len();
                if index < 128 {
                    fs::write(
                        witnesses.join(format!("{index}-{label}-source.xml")),
                        original,
                    )
                    .unwrap();
                    fs::write(
                        witnesses.join(format!("{index}-{label}-generated.xml")),
                        &regenerated,
                    )
                    .unwrap();
                }
                let row = serde_json::json!({"path":path, "dialect":label, "index":index});
                if dialect == FormDialect::Edt {
                    assert!(
                        serde_json::to_vec(&read(dialect, original)).unwrap()
                            == serde_json::to_vec(&read(dialect, &regenerated)).unwrap(),
                        "EDT typed same-source semantics changed: {path}"
                    );
                    edt_lexical_differences.push(row);
                } else {
                    source_failures.push(row);
                }
            }
        }
        count += 1;
        if generated == original_native {
            raw_equal += 1;
        } else {
            let index = residual.len();
            if index < 128 {
                fs::write(
                    witnesses.join(format!("cross-{index}-sdk.xml")),
                    &original_native,
                )
                .unwrap();
                fs::write(
                    witnesses.join(format!("cross-{index}-generated.xml")),
                    &generated,
                )
                .unwrap();
            }
            residual.push(path.to_owned());
        }
    }
    fs::write(lab.join("bsp85-defaults-all-genuine-r3.json"), serde_json::to_vec_pretty(&serde_json::json!({
        "status": if source_failures.is_empty() {"KNOWN_DEFAULTS_AND_SAME_SOURCE_PASS_NOT_FULL_ACCEPTANCE"} else {"KNOWN_DEFAULTS_PASS_SAME_SOURCE_FAILED_NOT_ACCEPTANCE"}, "forms":count,
        "raw_sdk_equal":raw_equal,"raw_sdk_residual_count":residual.len(),"raw_sdk_residual_paths":residual, "source_guard_failures":source_failures,"raw_edt_lexical_differences_with_equal_current_typed_semantics":edt_lexical_differences
    })).unwrap()).unwrap();
    assert_eq!(count, 855);
    assert_eq!(
        raw_equal, 855,
        "raw independent SDK payload differences, see F lab report"
    );
    assert!(
        source_failures.is_empty(),
        "same-source regeneration failures, see bounded F lab report"
    );
}

#[test]
fn public_xml221_current_importance_and_lines_reject_forged_generated_hash() {
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
    let form = body(false, "Main");
    let mut obj = MetadataObject::new(
        ObjectKind::new("CommonForm"),
        "ProfileDefaults",
        Uuid([42; 16]),
    );
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
        name: "ProfileDefaults".into(),
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
            "</Language>\r\n\t\t\t<CommonForm>ProfileDefaults</CommonForm>",
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
    // Explicit native defaults remain exact through the unchanged public route.
    let native_path = "CommonForms/ProfileDefaults/Ext/Form.xml";
    let explicit = SourceTree::new(
        original
            .entries()
            .iter()
            .map(|entry| {
                if entry.path().as_str() == native_path {
                    let current = std::str::from_utf8(entry.bytes())
                        .unwrap()
                        .replace(
                            "<HorizontalLines>false</HorizontalLines>",
                            "<HorizontalLines>true</HorizontalLines>",
                        )
                        .replace(
                            "<VerticalLines>false</VerticalLines>",
                            "<VerticalLines>true</VerticalLines>",
                        )
                        .replace(
                            "<ButtonImportance>Main</ButtonImportance>",
                            "<ButtonImportance>Normal</ButtonImportance>",
                        );
                    SourceEntry::from_bytes(entry.path().clone(), current.into_bytes()).unwrap()
                } else {
                    entry.clone()
                }
            })
            .collect(),
    )
    .unwrap();
    let explicit_generated = xml_to_edt(&explicit, &options).unwrap().tree;
    let explicit_returned = edt_to_xml(&Project::from_tree(explicit_generated).unwrap(), &options)
        .unwrap()
        .tree;
    assert_eq!(explicit_returned, explicit);
    let path = "src/CommonForms/ProfileDefaults/Form.form";
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
    edited_form.items[0]
        .properties
        .iter_mut()
        .find(|(id, _)| *id == tb::F_HORIZONTAL_LINES)
        .unwrap()
        .1 = PropertyValue::Bool(true);
    edited_form.items[1]
        .properties
        .iter_mut()
        .find(|(id, _)| *id == bt::F_BUTTON_IMPORTANCE)
        .unwrap()
        .1 = PropertyValue::Enum(Token::new("Supplementary"));
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
    let stripped = SourceTree::new(
        altered
            .entries()
            .iter()
            .filter(|e| !e.path().as_str().starts_with(".ibcmd-provenance/"))
            .cloned()
            .collect(),
    )
    .unwrap();
    let returned = edt_to_xml(&Project::from_tree(stripped.clone()).unwrap(), &options)
        .unwrap()
        .tree;
    let native = returned
        .entries()
        .iter()
        .find(|e| e.path().as_str() == "CommonForms/ProfileDefaults/Ext/Form.xml")
        .unwrap()
        .bytes();
    assert!(contains(
        native,
        "<ButtonImportance>Supplementary</ButtonImportance>"
    ));
    assert!(!contains(native, "<HorizontalLines>"));
    let model_dir = tempfile::tempdir().unwrap();
    let disk = model_dir.path().join("current");
    ibcmd_xml::source_tree::publish_new(&stripped, &disk).unwrap();
    let model = read_config(
        Format::Edt,
        &disk.join("src"),
        &ConvertOptions::default().with_target_version(version()),
    )
    .unwrap()
    .0;
    let view = morph1c_core::ir::semantic_view::ConfigurationSemanticView {
        configuration: &model,
        template_body: morph1c_pipeline::dcs_template_semantic_body,
    };
    manifest["semantics"] = serde_json::json!(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&view).unwrap())
    ));
    let fully_forged = SourceTree::new(
        altered
            .entries()
            .iter()
            .map(|e| {
                if e.path().as_str() == ".ibcmd-provenance/manifest.json" {
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
    assert!(edt_to_xml(&Project::from_tree(fully_forged).unwrap(), &options).is_err());
    assert!(
        edt_to_xml(&Project::from_tree(altered).unwrap(), &options)
            .unwrap_err()
            .to_string()
            .contains("changed")
    );
}
