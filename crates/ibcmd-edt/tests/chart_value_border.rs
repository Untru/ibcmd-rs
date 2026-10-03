use formats_xml::form::{
    FormDialect, apply_chart_semantics_resource, project_chart_semantics, read_chart_sidecar,
    read_form, write_chart_sidecar, write_form,
};
use ibcmd_edt::{
    ConversionOptions, Project, ReaderLimits, edt_to_xml, read_xml_source, xml_to_edt,
};
use ibcmd_xml::source_tree::{SourceEntry, SourceTree};
use morph1c_core::ir::{
    FormBody, MetadataObject, NamedFormBody, ObjectKind, PropertyValue, Token, Uuid,
    form::{ChartSettings, ChartTypedValue, ChartValue},
};
use morph1c_core::version::{FormatVersion, with_roundtrip_target, with_source_version};
use morph1c_pipeline::{ConvertOptions, Format, read_config, write_config};
use sha2::{Digest, Sha256};

fn source(content: &str) -> Vec<u8> {
    format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<chart:Chart xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:chart=\"http://g5.1c.ru/v8/dt/chart/model\" xmlns:core=\"http://g5.1c.ru/v8/dt/mcore\">{content}</chart:Chart>\r\n").into_bytes()
}
fn chart(content: &str) -> ChartSettings {
    read_chart_sidecar(&source(content)).unwrap()
}
fn body(settings: ChartSettings) -> FormBody {
    let source = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<form:Form xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:form=\"http://g5.1c.ru/v8/dt/form\"><attributes><name>Diagram</name><id>1</id><valueType><types>Chart</types></valueType><view><common>true</common></view><edit><common>true</common></edit><extInfo xsi:type=\"form:ChartExtInfo\"/></attributes></form:Form>\r\n";
    let mut body = read_form(FormDialect::Edt, source.as_bytes()).unwrap();
    body.data_attributes[0].chart_settings = Some(settings);
    body
}
fn value(settings: &mut ChartSettings) -> &mut ChartTypedValue {
    let ChartValue::Items(items) = &mut settings
        .fields
        .iter_mut()
        .find(|(n, _)| n == "realDataItems")
        .unwrap()
        .1
    else {
        panic!()
    };
    let ChartValue::Value(value) = &mut items[0]
        .iter_mut()
        .find(|(n, _)| n == "dataValue")
        .unwrap()
        .1
    else {
        panic!()
    };
    value
}
const CHART_FORM_UUID: Uuid = Uuid([84; 16]);

fn native(settings: &ChartSettings, minor: u16) -> Vec<u8> {
    with_roundtrip_target(FormatVersion::new(2, minor), || {
        let current = body(settings.clone());
        let projection = project_chart_semantics(&current, CHART_FORM_UUID).unwrap();
        write_form(
            FormDialect::Designer,
            projection
                .as_ref()
                .map_or(&current, |(projected, _)| projected),
        )
    })
    .unwrap()
}
fn returned(bytes: &[u8], current: &ChartSettings, minor: u16) -> ChartSettings {
    with_roundtrip_target(FormatVersion::new(2, minor), || {
        let projection = project_chart_semantics(&body(current.clone()), CHART_FORM_UUID).unwrap();
        let mut returned = with_source_version(Some(FormatVersion::new(2, minor)), || {
            read_form(FormDialect::Designer, bytes)
        })
        .unwrap();
        if let Some((_, resource)) = projection {
            apply_chart_semantics_resource(&mut returned, CHART_FORM_UUID, &resource).unwrap();
        }
        returned.data_attributes.remove(0).chart_settings.unwrap()
    })
}
#[test]
fn full_border_enum_and_signed_eint_current_values_do_not_collapse() {
    for minor in [20, 21] {
        for style in [
            "WithoutBorder",
            "Single",
            "Double",
            "Embossed",
            "Indented",
            "Underline",
            "DoubleUnderline",
            "Rounded",
            "Overline",
        ] {
            for width in [i32::MIN, 0, 1, 3, i32::MAX] {
                let mut current = chart(&format!(
                    "<realDataItems><dataValue xsi:type=\"core:BorderValue\"><value xsi:type=\"core:BorderDef\"><style>{style}</style><width>{width}</width></value></dataValue></realDataItems>"
                ));
                assert_eq!(
                    value(&mut current),
                    &ChartTypedValue::Border {
                        style: style.into(),
                        width: width.to_string()
                    }
                );
                let bytes = native(&current, minor);
                let text = String::from_utf8_lossy(&bytes);
                assert!(text.contains("xsi:type=\"v8ui:Border\""));
                assert!(text.contains(&format!("width=\"{width}\"")));
                assert_eq!(returned(&bytes, &current, minor), current);
                assert_eq!(
                    read_chart_sidecar(&write_chart_sidecar(&current).unwrap()).unwrap(),
                    current
                );
            }
        }
        let absent = chart("<realDataItems/>");
        let current = chart(
            "<realDataItems><dataValue xsi:type=\"core:BorderValue\"><value xsi:type=\"core:BorderDef\"/></dataValue></realDataItems>",
        );
        assert_ne!(current, absent);
        assert_eq!(returned(&native(&current, minor), &current, minor), current);
    }
}
#[test]
fn symbolic_reference_and_nested_order_edits_are_current_typed_data() {
    for minor in [20, 21] {
        let mut current = chart(
            "<realDataItems><dataValue xsi:type=\"core:BorderValue\"><value xsi:type=\"core:BorderRef\"><border>Style.FutureBorder</border></value></dataValue></realDataItems>",
        );
        assert_eq!(
            value(&mut current),
            &ChartTypedValue::BorderRef("Style.FutureBorder".into())
        );
        assert!(
            String::from_utf8_lossy(&native(&current, minor))
                .contains("ref=\"style:FutureBorder\"")
        );
        *value(&mut current) = ChartTypedValue::BorderRef("Style.CurrentBorder".into());
        let bytes = native(&current, minor);
        assert!(String::from_utf8_lossy(&bytes).contains("ref=\"style:CurrentBorder\""));
        assert_eq!(returned(&bytes, &current, minor), current);
        let mut nested = chart(
            "<realDataItems><dataValue xsi:type=\"core:ValueList\"><values xsi:type=\"core:BorderValue\"><value xsi:type=\"core:BorderRef\"><border>Style.CurrentBorder</border></value></values><values xsi:type=\"core:FixedArrayValue\"><values xsi:type=\"core:BorderValue\"><value xsi:type=\"core:BorderDef\"><style>Rounded</style><width>3</width></value></values></values></dataValue></realDataItems>",
        );
        assert_eq!(returned(&native(&nested, minor), &nested, minor), nested);
        let old = serde_json::to_vec(&nested).unwrap();
        let ChartTypedValue::ValueList(values) = value(&mut nested) else {
            panic!()
        };
        values.reverse();
        assert_ne!(serde_json::to_vec(&nested).unwrap(), old);
        assert_eq!(returned(&native(&nested, minor), &nested, minor), nested);
    }
}
#[test]
fn unknown_empty_mixed_duplicate_and_wrong_namespace_borders_fail_closed() {
    for inner in [
        "<value xsi:type=\"core:BorderRef\"><border/></value>",
        "<value xsi:type=\"core:BorderRef\"><border>Windows.X</border></value>",
        "<value xsi:type=\"core:BorderRef\"><border>Style.X</border><border>Style.X</border></value>",
        "<value xsi:type=\"core:BorderRef\"><border>Style.X</border><width>3</width></value>",
        "<value xsi:type=\"core:BorderDef\"><style>Unknown</style></value>",
        "<value xsi:type=\"core:BorderDef\"><width>2147483648</width></value>",
        "<value xsi:type=\"core:BorderDef\"><width>-2147483649</width></value>",
        "<value xsi:type=\"core:BorderDef\"><style>Rounded</style><style>Rounded</style></value>",
        "<value xsi:type=\"core:BorderDef\"><unknown>true</unknown></value>",
        "<value xsi:type=\"core:BorderDef\" unknown=\"true\"/>",
        "<value xmlns:core=\"http://wrong.example/core\" xsi:type=\"core:BorderDef\"/>",
    ] {
        assert!(read_chart_sidecar(&source(&format!("<realDataItems><dataValue xsi:type=\"core:BorderValue\">{inner}</dataValue></realDataItems>"))).is_err());
    }
    let settings = chart(
        "<realDataItems><dataValue xsi:type=\"core:BorderValue\"><value xsi:type=\"core:BorderDef\"><style>Rounded</style><width>3</width></value></dataValue></realDataItems>",
    );
    let native = String::from_utf8(native(&settings, 21)).unwrap();
    for invalid in [
        native.replace("http://v8.1c.ru/8.1/data/ui", "http://wrong.example/ui"),
        native.replace("width=\"3\"", "ref=\"style:X\" width=\"3\""),
        native.replace(
            "<v8ui:style xsi:type=\"v8ui:ControlBorderType\">Rounded</v8ui:style>",
            "<v8ui:style xsi:type=\"v8ui:ControlBorderType\">Unknown</v8ui:style>",
        ),
    ] {
        assert!(read_form(FormDialect::Designer, invalid.as_bytes()).is_err());
    }
}

#[test]
fn public_stripped_current_border_edits_and_forged_hashes_cannot_replay_source() {
    for minor in [20, 21] {
        let options = ConversionOptions {
            edt_version: "2025.2.3".into(),
            xml_dialect: format!("2.{minor}"),
            runtime_version: Some(if minor == 20 { "8.3.27" } else { "8.5.1" }.into()),
        };
        let fixture = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/subsystem-ci/src"
        ));
        let mut cfg = read_config(Format::Designer, fixture, &ConvertOptions::default())
            .unwrap()
            .0;
        let mut obj =
            MetadataObject::new(ObjectKind::new("CommonForm"), "ChartBorder", Uuid([64; 16]));
        let field = morph1c_core::spec::registry::spec_for("CommonForm")
            .unwrap()
            .fields()
            .iter()
            .find(|f| f.name == "formType")
            .unwrap()
            .id;
        obj.properties
            .push((field, PropertyValue::Enum(Token::new("Managed"))));
        obj.form_bodies.push(NamedFormBody{name:"ChartBorder".into(),body:body(chart("<realDataItems><dataValue xsi:type=\"core:BorderValue\"><value xsi:type=\"core:BorderRef\"><border>Style.FutureBorder</border></value></dataValue></realDataItems>")),ordinary_body:None,module:None,help:vec![],help_resources:vec![]});
        cfg.objects.push(obj);
        let dir = tempfile::tempdir().unwrap();
        with_roundtrip_target(FormatVersion::new(2, minor), || {
            write_config(Format::Designer, &cfg, dir.path())
        })
        .unwrap();
        let root = dir.path().join("Configuration.xml");
        let content = String::from_utf8(std::fs::read(&root).unwrap())
            .unwrap()
            .replace(
                "</Language>",
                "</Language>\r\n\t\t\t<CommonForm>ChartBorder</CommonForm>",
            );
        std::fs::write(root, content).unwrap();
        let original = read_xml_source(dir.path(), ReaderLimits::default()).unwrap();
        let generated = xml_to_edt(&original, &options).unwrap().tree;
        assert_eq!(
            edt_to_xml(&Project::from_tree(generated.clone()).unwrap(), &options)
                .unwrap()
                .tree,
            original
        );
        let path = "src/CommonForms/ChartBorder/Attributes/Diagram/ExtInfo/Chart.chart";
        let bytes = generated
            .entries()
            .iter()
            .find(|e| e.path().as_str() == path)
            .unwrap()
            .bytes();
        let mut current = read_chart_sidecar(bytes).unwrap();
        *value(&mut current) = ChartTypedValue::Border {
            style: "Overline".into(),
            width: "7".into(),
        };
        let edited = write_chart_sidecar(&current).unwrap();
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
        let alter = |strip: bool| {
            SourceTree::new(
                generated
                    .entries()
                    .iter()
                    .filter(|e| !strip || !e.path().as_str().starts_with(".ibcmd-provenance/"))
                    .map(|e| {
                        SourceEntry::from_bytes(
                            e.path().clone(),
                            if e.path().as_str() == path {
                                edited.clone()
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
        let changed = edt_to_xml(&Project::from_tree(alter(true)).unwrap(), &options).unwrap();
        let native = changed
            .tree
            .entries()
            .iter()
            .find(|e| e.path().as_str() == "CommonForms/ChartBorder/Ext/Form.xml")
            .unwrap();
        // Read the complete public source, including its actual resource and owner
        // binding, instead of comparing an isolated native projection to CURRENT.
        let complete = xml_to_edt(&changed.tree, &options).unwrap().tree;
        let restored = read_chart_sidecar(
            complete
                .entries()
                .iter()
                .find(|e| e.path().as_str() == path)
                .unwrap()
                .bytes(),
        )
        .unwrap();
        assert_eq!(restored, current);
        assert!(!String::from_utf8_lossy(native.bytes()).contains("style:FutureBorder"));
        assert_ne!(changed.tree, original);
    }
}
