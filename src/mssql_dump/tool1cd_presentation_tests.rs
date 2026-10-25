use super::*;

#[test]
fn tool1cd_presentation_declared_font_retains_typed_colour() {
    let id = "10000000-0000-4000-8000-000000000570";
    let text = format!(
        r##"{{1,{{3,1,{{"#",9cd510c7-abfc-11d4-9434-004095e12fc7,2,{{3,0,{{0}}}}}},{{3,{{1,0,{id}}},"OwnStyle",{{0}},"",0,0,00000000-0000-0000-0000-000000000000,0}}}},0}}"##
    );
    let modern = text.replace("{3,0,{0}}", "{4,0,{0},0}");
    for row in [&text, &modern] {
        let parsed = parse_style_item_properties_from_text(row, id).unwrap();
        assert_eq!(parsed.item_type, "Font");
        assert_eq!(
            parsed.value_xml,
            r##"<Value xsi:type="v8ui:Color">#000000</Value>"##
        );
    }
    let parsed = parse_style_item_properties_from_text(&text, id).unwrap();
    assert_eq!(parsed.item_type, "Font");
    assert_eq!(
        parsed.value_xml,
        r##"<Value xsi:type="v8ui:Color">#000000</Value>"##
    );
    assert!(
        parse_style_item_properties_from_text(
            &text.replace(
                "9cd510c7-abfc-11d4-9434-004095e12fc7",
                "10000000-0000-4000-8000-000000000571"
            ),
            id
        )
        .is_none()
    );
}

#[test]
fn tool1cd_presentation_ws_location_empty_or_escaped_is_exact() {
    let head = MetadataHeader {
        uuid: "10000000-0000-4000-8000-000000000570".into(),
        name: "OwnReference".into(),
        synonyms: Vec::new(),
        comment: String::new(),
        template_type_code: None,
    };
    for (value, expected) in [
        ("", "<LocationURL/>"),
        (
            "https://example.invalid/?a=1&b=2",
            "<LocationURL>https://example.invalid/?a=1&amp;b=2</LocationURL>",
        ),
    ] {
        let p = WSReferenceProperties {
            location_url: value.into(),
            manager_type_id: "10000000-0000-4000-8000-000000000571".into(),
            manager_value_id: "10000000-0000-4000-8000-000000000572".into(),
        };
        for dialect in [
            InfobaseConfigSourceVersion::V2_20,
            InfobaseConfigSourceVersion::V2_21,
        ] {
            let xml = format_ws_reference_source_xml(&head, &p, dialect);
            assert_eq!(xml.matches(expected).count(), 1, "{xml}");
            if value.is_empty() {
                assert!(!xml.contains("<LocationURL></LocationURL>"));
            }
        }
    }
}

#[test]
fn tool1cd_presentation_grayed_picture_uses_a_single_known_reference() {
    let refs = BTreeMap::new();
    assert_eq!(
        parse_common_command_picture_value("{4,1,{-12},0,0,0,1,0,0}", &refs),
        Some((Some("StdPicture.GrayedAll".into()), true))
    );
    assert_ne!(
        parse_common_command_picture_value("{4,1,{-12,999},0,0,0,1,0,0}", &refs),
        Some((Some("StdPicture.GrayedAll".into()), true))
    );
    assert_ne!(
        parse_common_command_picture_value("{4,1,{-123456},0,0,0,1,0,0}", &refs),
        Some((Some("StdPicture.GrayedAll".into()), true))
    );
}
