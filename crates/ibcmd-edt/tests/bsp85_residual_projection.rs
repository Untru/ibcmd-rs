use formats_xml::form::{FormDialect, read_form, write_form};
use morph1c_core::{
    ir::{
        DecoratorBody, DecoratorRef, FormBody, FormControlKind, FormItem, MetadataObject,
        ObjectKind, PropertyValue, Token, TooltipBody, Uuid,
    },
    spec::metadata::configuration as cfg,
    version::{FormatVersion, with_roundtrip_target, with_source_version},
};
use morph1c_pipeline::{Format, registry::FormatRegistry};

fn write_configuration(object: &MetadataObject, format: Format) -> Vec<u8> {
    with_roundtrip_target(FormatVersion::new(2, 21), || {
        (FormatRegistry::for_format(format)
            .unwrap()
            .get("Configuration")
            .unwrap()
            .write)(object)
    })
    .unwrap()
}

#[test]
fn configuration_window_default_is_current_typed_value_and_edits_win() {
    let mut object =
        MetadataObject::new(ObjectKind::new("Configuration"), "Windows", Uuid([1; 16]));
    let native = write_configuration(&object, Format::Designer);
    assert!(std::str::from_utf8(&native).unwrap().contains(
        "<ClientApplicationWindowsOpenVariant>OpenDataInTabs</ClientApplicationWindowsOpenVariant>"
    ));
    let edt = write_configuration(&object, Format::Edt);
    assert!(
        !std::str::from_utf8(&edt)
            .unwrap()
            .contains("clientApplicationWindowsOpenVariant")
    );
    object.properties.push((
        cfg::F_WINDOWS_OPEN_VARIANT,
        PropertyValue::Enum(Token::new("OpenDataInDialogs")),
    ));
    for format in [Format::Designer, Format::Edt] {
        let bytes = write_configuration(&object, format);
        assert!(
            std::str::from_utf8(&bytes)
                .unwrap()
                .contains("OpenDataInDialogs")
        );
        let read = (FormatRegistry::for_format(format)
            .unwrap()
            .get("Configuration")
            .unwrap()
            .read)(&bytes)
        .unwrap();
        assert!(read.properties.iter().any(|(id, value)| {
            *id == cfg::F_WINDOWS_OPEN_VARIANT
                && value == &PropertyValue::Enum(Token::new("OpenDataInDialogs"))
        }));
    }
}

fn form_write(body: &FormBody, dialect: FormDialect) -> Vec<u8> {
    with_roundtrip_target(FormatVersion::new(2, 21), || write_form(dialect, body)).unwrap()
}
fn form_read(bytes: &[u8], dialect: FormDialect) -> FormBody {
    with_source_version(Some(FormatVersion::new(2, 21)), || {
        read_form(dialect, bytes)
    })
    .unwrap()
}
#[test]
fn explicit_edt_panel_name_and_current_edits_win_without_native_spelling_loss() {
    let source = br#"<form:Form xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:form="http://g5.1c.ru/v8/dt/form"><autoCommandBar><name>FormCommandBar</name><id>1</id><horizontalAlign>Left</horizontalAlign><autoFill>true</autoFill></autoCommandBar></form:Form>"#;
    let mut body = form_read(
        &[
            b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n".as_slice(),
            source.as_slice(),
            b"\r\n",
        ]
        .concat(),
        FormDialect::Edt,
    );
    assert!(
        std::str::from_utf8(&form_write(&body, FormDialect::Designer))
            .unwrap()
            .contains("name=\"FormCommandBar\"")
    );
    body.auto_command_bar.as_mut().unwrap().name = "CurrentPanel".into();
    assert!(
        std::str::from_utf8(&form_write(&body, FormDialect::Designer))
            .unwrap()
            .contains("name=\"CurrentPanel\"")
    );
    let native = form_write(&body, FormDialect::Designer);
    let native = String::from_utf8(native)
        .unwrap()
        .replace("name=\"CurrentPanel\"", "name=\"\"");
    let read = form_read(native.as_bytes(), FormDialect::Designer);
    assert!(form_write(&read, FormDialect::Designer) == native.as_bytes());
}
#[test]
fn tooltip_auto_absence_explicit_presence_and_current_alignment_are_independent() {
    use morph1c_core::spec::forms::controls::label_decoration as ld;
    let mut body = FormBody::new();
    let mut item = FormItem::new(FormControlKind::new("LabelDecoration"), "Control", 1);
    item.ext_tooltip = Some(DecoratorRef::new(
        "Tooltip",
        2,
        DecoratorBody::Tooltip(TooltipBody {
            ext_info: vec![(
                ld::F_EXT_HORIZONTAL_ALIGN,
                PropertyValue::Enum(Token::new("Auto")),
            )],
            ..Default::default()
        }),
    ));
    body.items.push(item);
    let edt = form_write(&body, FormDialect::Edt);
    assert!(
        !std::str::from_utf8(&edt)
            .unwrap()
            .contains("<horizontalAlign>Auto</horizontalAlign>")
    );
    let mut read = form_read(&edt, FormDialect::Edt);
    assert!(
        !std::str::from_utf8(&form_write(&read, FormDialect::Edt))
            .unwrap()
            .contains("<horizontalAlign>Auto</horizontalAlign>")
    );
    assert!(
        std::str::from_utf8(&form_write(&read, FormDialect::Designer))
            .unwrap()
            .contains("<HorizontalAlign>Auto</HorizontalAlign>")
    );
    let DecoratorBody::Tooltip(tooltip) = &mut read.items[0].ext_tooltip.as_mut().unwrap().body
    else {
        panic!()
    };
    tooltip.edt_horizontal_align_auto_explicit = true;
    let explicit = form_write(&read, FormDialect::Edt);
    let mut read = form_read(&explicit, FormDialect::Edt);
    assert!(form_write(&read, FormDialect::Edt) == explicit);
    let DecoratorBody::Tooltip(tooltip) = &mut read.items[0].ext_tooltip.as_mut().unwrap().body
    else {
        panic!()
    };
    tooltip.ext_info[0].1 = PropertyValue::Enum(Token::new("Left"));
    assert!(
        std::str::from_utf8(&form_write(&read, FormDialect::Edt))
            .unwrap()
            .contains("<horizontalAlign>Left</horizontalAlign>")
    );
    let native = form_write(&read, FormDialect::Designer);
    assert!(
        !std::str::from_utf8(&native)
            .unwrap()
            .contains("<HorizontalAlign>Left</HorizontalAlign>")
    );
    assert!(
        form_write(
            &form_read(&native, FormDialect::Designer),
            FormDialect::Designer
        ) == native
    );
}

#[test]
fn table_command_bar_order_replays_only_slots_and_current_values() {
    use morph1c_core::spec::forms::controls::table;
    let mut body = FormBody::new();
    let mut item = FormItem::new(FormControlKind::new("Table"), "List", 1);
    item.show_command_bar = Some("true".into());
    item.properties
        .push((table::F_WIDTH, PropertyValue::Int(100)));
    body.items.push(item);
    let baseline = form_write(
        &form_read(
            &form_write(&body, FormDialect::Designer),
            FormDialect::Designer,
        ),
        FormDialect::Designer,
    );
    let text = String::from_utf8(baseline).unwrap();
    let marker = "<ShowCommandBar>true</ShowCommandBar>";
    assert!(text.contains(marker));
    let source = text
        .replace("<Width>100</Width>", "__known_slot__")
        .replace(marker, "<Width>100</Width>")
        .replace("__known_slot__", marker);
    let mut read = form_read(source.as_bytes(), FormDialect::Designer);
    assert!(form_write(&read, FormDialect::Designer) == source.as_bytes());
    read.items[0].show_command_bar = Some("false".into());
    let edited = String::from_utf8(form_write(&read, FormDialect::Designer)).unwrap();
    assert!(edited.contains("<ShowCommandBar>false</ShowCommandBar>"));
    assert!(edited.find("<ShowCommandBar>").unwrap() < edited.find("<Width>").unwrap());
    read.items[0].show_command_bar = None;
    assert!(
        !String::from_utf8(form_write(&read, FormDialect::Designer))
            .unwrap()
            .contains("<ShowCommandBar>")
    );
    for extra in ["<UnknownSlot>value</UnknownSlot>", marker] {
        let bad = source.replace(marker, &format!("{marker}{extra}"));
        assert!(
            with_source_version(Some(FormatVersion::new(2, 21)), || read_form(
                FormDialect::Designer,
                bad.as_bytes()
            ))
            .is_err()
        );
    }
}

#[test]
fn table_native_profile_order_uses_current_values_and_keeps_legacy_order() {
    use morph1c_core::spec::forms::controls::table;
    let mut body = FormBody::new();
    let mut item = FormItem::new(FormControlKind::new("Table"), "List", 1);
    item.show_command_bar = Some("true".into());
    item.properties
        .push((table::F_VIEW_MODE, PropertyValue::Enum(Token::new("List"))));
    body.items.push(item);
    let write = |body: &FormBody, minor| {
        with_roundtrip_target(FormatVersion::new(2, minor), || {
            write_form(FormDialect::Designer, body)
        })
        .unwrap()
    };
    for minor in [20, 21] {
        let output = String::from_utf8(write(&body, minor)).unwrap();
        let before = output.find("<ShowCommandBar>").unwrap() < output.find("<ViewMode>").unwrap();
        assert_eq!(before, minor == 21);
    }
    body.items[0].show_command_bar = Some("false".into());
    let edited = String::from_utf8(write(&body, 21)).unwrap();
    assert!(edited.contains("<ShowCommandBar>false</ShowCommandBar>"));
    assert!(edited.find("<ShowCommandBar>").unwrap() < edited.find("<ViewMode>").unwrap());
    body.items[0].show_command_bar = None;
    assert!(
        !String::from_utf8(write(&body, 21))
            .unwrap()
            .contains("<ShowCommandBar>")
    );
}

#[test]
#[ignore = "Actual BSP85 Table typed ShowCommandBar/ViewMode interleaving witness"]
fn genuine_bsp85_table_order_is_exact_and_current_command_bar_wins() {
    let lab = std::path::PathBuf::from(std::env::var_os("IBCMD_EDT_LAB").unwrap());
    let source =
        std::fs::read(lab.join("bsp85-default-source-guard-witnesses-r1/1-native-source.xml"))
            .unwrap();
    let mut read = form_read(&source, FormDialect::Designer);
    assert!(form_write(&read, FormDialect::Designer) == source);
    fn table(items: &mut [FormItem]) -> Option<&mut FormItem> {
        for item in items {
            if item.kind.as_str() == "Table" && item.id == 6 {
                return Some(item);
            }
            if let Some(found) = table(&mut item.children) {
                return Some(found);
            }
        }
        None
    }
    let target = table(&mut read.items).unwrap();
    target.show_command_bar = Some(
        if target.show_command_bar.as_deref() == Some("false") {
            "true"
        } else {
            "false"
        }
        .into(),
    );
    let current = target.show_command_bar.clone();
    let output = form_write(&read, FormDialect::Designer);
    assert!(output != source);
    let mut returned = form_read(&output, FormDialect::Designer);
    assert_eq!(
        table(&mut returned.items).unwrap().show_command_bar,
        current
    );
    assert!(form_write(&returned, FormDialect::Designer) == output);
}

#[test]
#[ignore = "Bound genuine BSP85 typed path projections and configuration default on F"]
fn genuine_bsp85_residual_paths_and_configuration_match_sdk() {
    let lab = std::path::PathBuf::from(std::env::var_os("IBCMD_EDT_LAB").unwrap());
    let taxonomy: serde_json::Value = serde_json::from_slice(
        &std::fs::read(lab.join("bsp85-form-field-taxonomy-r1.json")).unwrap(),
    )
    .unwrap();
    let edt = lab.join("oracle-bsp85-r3/authentic-workspace/OracleConfiguration/src");
    let sdk = lab.join("native-reference-bsp85-r4/native-xml");
    let mut checked = 0;
    for row in taxonomy["rows"].as_array().unwrap() {
        let relative = row["path"].as_str().unwrap();
        if relative == "Configuration.xml" {
            let input = std::fs::read(edt.join("Configuration/Configuration.mdo")).unwrap();
            let object = with_source_version(Some(FormatVersion::new(2, 21)), || {
                (FormatRegistry::for_format(Format::Edt)
                    .unwrap()
                    .get("Configuration")
                    .unwrap()
                    .read)(&input)
            })
            .unwrap();
            // Standalone descriptors do not attach root picture/help sidecars;
            // compare the independently observed current window field only.
            let output = write_configuration(&object, Format::Designer);
            let expected = std::fs::read(sdk.join(relative)).unwrap();
            let field = "<ClientApplicationWindowsOpenVariant>OpenDataInTabs</ClientApplicationWindowsOpenVariant>";
            assert!(std::str::from_utf8(&output).unwrap().contains(field));
            assert!(std::str::from_utf8(&expected).unwrap().contains(field));
            let native_object = with_source_version(Some(FormatVersion::new(2, 21)), || {
                (FormatRegistry::for_format(Format::Designer)
                    .unwrap()
                    .get("Configuration")
                    .unwrap()
                    .read)(&expected)
            })
            .unwrap();
            let regenerated = write_configuration(&native_object, Format::Designer);
            assert!(std::str::from_utf8(&regenerated).unwrap().contains(field));
            continue;
        }
        let differences: Vec<_> = row["differences"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|diff| {
                matches!(
                    diff["scope"].as_str().unwrap().rsplit('/').next().unwrap(),
                    "Field" | "RowPictureDataPath" | "AutoCommandBar" | "HorizontalAlign"
                )
            })
            .collect();
        if differences.is_empty() {
            continue;
        }
        let source =
            std::fs::read(edt.join(relative.replace("/Ext/Form.xml", "/Form.form"))).unwrap();
        let body = with_source_version(Some(FormatVersion::new(2, 21)), || {
            read_form(FormDialect::Edt, &source)
        })
        .unwrap();
        let output = with_roundtrip_target(FormatVersion::new(2, 21), || {
            write_form(FormDialect::Designer, &body)
        })
        .unwrap();
        let expected = std::fs::read(sdk.join(relative)).unwrap();
        let generated = ibcmd_xml::XmlReader::from_slice(&output).unwrap();
        let reference = ibcmd_xml::XmlReader::from_slice(&expected).unwrap();
        let returned = form_read(&form_write(&body, FormDialect::Edt), FormDialect::Edt);
        assert!(
            serde_json::to_vec(&body).unwrap() == serde_json::to_vec(&returned).unwrap(),
            "same-source current form semantics changed: {relative}"
        );
        for diff in differences {
            let tag = diff["scope"].as_str().unwrap().rsplit('/').next().unwrap();
            use sha2::{Digest, Sha256};
            if tag == "AutoCommandBar" {
                let panel = generated
                    .root()
                    .children()
                    .iter()
                    .find_map(|child| {
                        if let ibcmd_xml::XmlNode::Element(element) = child {
                            (element.name().local() == "AutoCommandBar").then_some(element)
                        } else {
                            None
                        }
                    })
                    .unwrap();
                for name in ["name", "id"] {
                    let value = panel.attributes().iter().find(|attribute| matches!(attribute.kind(), ibcmd_xml::AttributeKind::Ordinary(qname) if qname.local() == name)).unwrap().value();
                    assert_eq!(
                        format!("{:x}", Sha256::digest(value.as_bytes())),
                        diff["sdk"]["attributes"][name].as_str().unwrap()
                    );
                }
                checked += 1;
                continue;
            }
            let wanted = diff["sdk"]["text_sha256"].as_str().unwrap();
            fn texts(node: &ibcmd_xml::XmlElement, name: &str, values: &mut Vec<String>) {
                if node.name().local() == name {
                    values.push(
                        node.children()
                            .iter()
                            .filter_map(|child| {
                                if let ibcmd_xml::XmlNode::Text(text) = child {
                                    Some(text.value())
                                } else {
                                    None
                                }
                            })
                            .collect::<String>(),
                    );
                }
                for child in node.children() {
                    if let ibcmd_xml::XmlNode::Element(child) = child {
                        texts(child, name, values);
                    }
                }
            }
            let mut a = vec![];
            let mut b = vec![];
            fn tooltip(node: &ibcmd_xml::XmlElement) -> Option<&ibcmd_xml::XmlElement> {
                if node.name().local() == "ExtendedTooltip" && node.attributes().iter().any(|attribute| matches!(attribute.kind(), ibcmd_xml::AttributeKind::Ordinary(qname) if qname.local() == "id") && attribute.value() == "85") {
                    return Some(node);
                }
                node.children().iter().find_map(|child| {
                    if let ibcmd_xml::XmlNode::Element(child) = child {
                        tooltip(child)
                    } else {
                        None
                    }
                })
            }
            if tag == "HorizontalAlign" {
                // Bind the witnessed owner; another control's alignment must
                // never satisfy this tooltip's projection assertion.
                texts(tooltip(generated.root()).unwrap(), tag, &mut a);
                texts(tooltip(reference.root()).unwrap(), tag, &mut b);
            } else {
                texts(generated.root(), tag, &mut a);
                texts(reference.root(), tag, &mut b);
            }
            let expected = b
                .iter()
                .find(|text| format!("{:x}", Sha256::digest(text.as_bytes())) == wanted)
                .unwrap();
            assert!(
                a.contains(expected),
                "current typed path does not match SDK: {relative} {tag}"
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 20);
}
