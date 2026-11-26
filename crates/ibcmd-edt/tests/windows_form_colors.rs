use formats_xml::form::{FormDialect, read_form, write_form};
use morph1c_core::{
    ir::{FormBody, FormControlKind, FormItem, PropertyValue},
    spec::forms::controls::button as bt,
    version::{FormatVersion, with_roundtrip_target, with_source_version},
};
// Original SDK platform resources, both 8.3.27 and 8.5.1. This is test evidence,
// not an implementation allowlist. Primary member hashes are retained on F.
const REGISTERED_WINDOWS_COLORS: &[&str] = &[
    "Windows.ScrollBar",
    "Windows.Desktop",
    "Windows.ActiveTitleBar",
    "Windows.InactiveTitleBar",
    "Windows.MenuBar",
    "Windows.WindowBackground",
    "Windows.WindowFrame",
    "Windows.MenuItemText",
    "Windows.WindowText",
    "Windows.ActiveTitleBarText",
    "Windows.ActiveBorder",
    "Windows.InactiveBorder",
    "Windows.ApplicationWorkspace",
    "Windows.Highlight",
    "Windows.HighlightText",
    "Windows.ButtonFace",
    "Windows.ButtonShadow",
    "Windows.DisabledText",
    "Windows.ButtonText",
    "Windows.InactiveTitleBarText",
    "Windows.ButtonHighlight",
    "Windows.ButtonDarkShadow",
    "Windows.ButtonLightShadow",
    "Windows.ToolTipText",
    "Windows.ToolTip",
    "Windows.HotLight",
    "Windows.GradientActiveCaption",
    "Windows.GradientInactiveCaption",
    "Windows.MiddleGradientActiveCaption",
    "Windows.MiddleGradientInactiveCaption",
];
fn write(dialect: FormDialect, form: &FormBody, version: FormatVersion) -> Vec<u8> {
    with_roundtrip_target(version, || write_form(dialect, form)).unwrap()
}
fn read(dialect: FormDialect, bytes: &[u8], version: FormatVersion) -> FormBody {
    with_source_version(Some(version), || read_form(dialect, bytes)).unwrap()
}
fn form(version: FormatVersion, color: &str) -> FormBody {
    let mut form = FormBody::new();
    form.items
        .push(FormItem::new(FormControlKind::new("Button"), "Button", 5));
    let mut form = read(
        FormDialect::Designer,
        &write(FormDialect::Designer, &form, version),
        version,
    );
    form.items[0]
        .properties
        .push((bt::F_BACK_COLOR, PropertyValue::Ref(color.into())));
    form
}
#[test]
fn all_registered_windows_colors_are_form_references_in_both_profiles() {
    for version in [FormatVersion::new(2, 20), FormatVersion::new(2, 21)] {
        for color in REGISTERED_WINDOWS_COLORS {
            let form = form(version, color);
            let native = write(FormDialect::Designer, &form, version);
            assert!(String::from_utf8_lossy(&native).contains(&format!(
                "<BackColor>win:{}</BackColor>",
                color.strip_prefix("Windows.").unwrap()
            )));
            assert_eq!(
                write(
                    FormDialect::Designer,
                    &read(FormDialect::Designer, &native, version),
                    version
                ),
                native
            );
            let edt = write(FormDialect::Edt, &form, version);
            assert!(String::from_utf8_lossy(&edt).contains(&format!("<color>{color}</color>")));
            let returned = read(FormDialect::Edt, &edt, version);
            assert_eq!(
                returned.items[0].get(bt::F_BACK_COLOR),
                form.items[0].get(bt::F_BACK_COLOR)
            );
            let projected = write(FormDialect::Designer, &returned, version);
            assert!(String::from_utf8_lossy(&projected).contains(&format!(
                "<BackColor>win:{}</BackColor>",
                color.strip_prefix("Windows.").unwrap()
            )));
            assert_eq!(
                serde_json::to_vec(&read(FormDialect::Designer, &projected, version)).unwrap(),
                serde_json::to_vec(&returned).unwrap()
            );
        }
    }
}
#[test]
fn symbolic_target_is_current_and_not_a_frozen_registry_membership_gate() {
    let version = FormatVersion::new(2, 21);
    let mut typed = form(version, "Windows.Highlight");
    let original = write(FormDialect::Designer, &typed, version);
    let color = typed.items[0]
        .properties
        .iter_mut()
        .find(|(id, _)| *id == bt::F_BACK_COLOR)
        .unwrap();
    color.1 = PropertyValue::Ref("Windows.HotLight".into());
    let edited = write(FormDialect::Designer, &typed, version);
    assert_ne!(edited, original);
    assert!(String::from_utf8_lossy(&edited).contains("<BackColor>win:HotLight</BackColor>"));
    // SDK ColorRef stores a symbolic target. Unresolved well-formed references
    // are retained, without asserting that the installed runtime can render them.
    let unresolved = form(version, "Windows.FutureSystemColor");
    let edt = write(FormDialect::Edt, &unresolved, version);
    assert_eq!(
        read(FormDialect::Edt, &edt, version).items[0].get(bt::F_BACK_COLOR),
        unresolved.items[0].get(bt::F_BACK_COLOR)
    );
}
#[test]
fn color_namespace_empty_target_and_malformed_xml_are_rejected() {
    let version = FormatVersion::new(2, 21);
    let native = String::from_utf8(write(
        FormDialect::Designer,
        &form(version, "Windows.Highlight"),
        version,
    ))
    .unwrap();
    for value in ["win:", "unknown:Highlight", "#ABC", "#GGGGGG"] {
        let source = native.replace("win:Highlight", value);
        assert!(
            with_source_version(Some(version), || read_form(
                FormDialect::Designer,
                source.as_bytes()
            ))
            .is_err(),
            "accepted {value}"
        );
    }
    for replacement in [
        r#"<BackColor xmlns="urn:unknown">win:Highlight</BackColor>"#,
        r#"<BackColor other="x">win:Highlight</BackColor>"#,
        "<BackColor>win:Highlight<Unknown/></BackColor>",
    ] {
        let source = native.replace("<BackColor>win:Highlight</BackColor>", replacement);
        assert!(
            with_source_version(Some(version), || read_form(
                FormDialect::Designer,
                source.as_bytes()
            ))
            .is_err()
        );
    }
    let edt = String::from_utf8(write(
        FormDialect::Edt,
        &form(version, "Windows.Highlight"),
        version,
    ))
    .unwrap();
    for (from, to) in [
        ("http://g5.1c.ru/v8/dt/mcore", "urn:unknown"),
        ("core:ColorRef", "unknown:ColorRef"),
        ("Windows.Highlight", "Windows."),
        ("Windows.Highlight", "Unknown.Highlight"),
    ] {
        let source = edt.replace(from, to);
        assert!(
            with_source_version(Some(version), || read_form(
                FormDialect::Edt,
                source.as_bytes()
            ))
            .is_err(),
            "accepted {to}"
        );
    }
}

#[test]
fn public_windows_color_roundtrip_stripped_edits_and_rehashed_provenance() {
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
    let form = form(FormatVersion::new(2, 21), "Windows.Highlight");
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
    edited_form.items[0]
        .properties
        .iter_mut()
        .find(|(id, _)| *id == bt::F_BACK_COLOR)
        .unwrap()
        .1 = PropertyValue::Ref("Windows.HotLight".into());
    let edited = with_roundtrip_target(FormatVersion::new(2, 21), || {
        write_form(FormDialect::Edt, &edited_form)
    })
    .unwrap();
    assert!(edited != bytes);
    let stripped = SourceTree::new(
        generated
            .entries()
            .iter()
            .filter(|e| !e.path().as_str().starts_with(".ibcmd-provenance/"))
            .map(|e| {
                if e.path().as_str() == path {
                    SourceEntry::from_bytes(e.path().clone(), edited.clone()).unwrap()
                } else {
                    e.clone()
                }
            })
            .collect(),
    )
    .unwrap();
    let current = edt_to_xml(&Project::from_tree(stripped).unwrap(), &options)
        .unwrap()
        .tree;
    let current_form = current
        .entries()
        .iter()
        .find(|e| e.path().as_str() == "CommonForms/AutoColor/Ext/Form.xml")
        .unwrap();
    assert!(
        String::from_utf8_lossy(current_form.bytes())
            .contains("<BackColor>win:HotLight</BackColor>")
    );
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
