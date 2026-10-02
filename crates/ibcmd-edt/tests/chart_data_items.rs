use formats_xml::form::{
    FormDialect, read_chart_sidecar, read_form, write_chart_sidecar, write_form,
};
use morph1c_core::ir::{
    FormBody,
    form::{ChartSettings, ChartValue},
};

fn source(content: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<chart:Chart xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:chart=\"http://g5.1c.ru/v8/dt/chart/model\" xmlns:core=\"http://g5.1c.ru/v8/dt/mcore\">{content}</chart:Chart>\r\n"
    )
}
fn chart(content: &str) -> ChartSettings {
    read_chart_sidecar(source(content).as_bytes()).unwrap()
}
fn body(settings: ChartSettings) -> FormBody {
    let source = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<form:Form xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:form=\"http://g5.1c.ru/v8/dt/form\"><attributes><name>Diagram</name><id>1</id><valueType><types>Chart</types></valueType><view><common>true</common></view><edit><common>true</common></edit><extInfo xsi:type=\"form:ChartExtInfo\"/></attributes></form:Form>\r\n";
    let mut body = read_form(FormDialect::Edt, source.as_bytes()).unwrap();
    body.data_attributes[0].chart_settings = Some(settings);
    body
}
fn through_native(settings: &ChartSettings) -> ChartSettings {
    let output = write_form(FormDialect::Designer, &body(settings.clone())).unwrap();
    read_form(FormDialect::Designer, &output)
        .unwrap()
        .data_attributes[0]
        .chart_settings
        .clone()
        .unwrap()
}
fn items(settings: &mut ChartSettings) -> &mut Vec<Vec<(String, ChartValue)>> {
    let ChartValue::Items(items) = &mut settings
        .fields
        .iter_mut()
        .find(|(n, _)| n == "realDataItems")
        .unwrap()
        .1
    else {
        panic!("typed item list required")
    };
    items
}

#[test]
fn data_items_preserve_types_information_tooltip_and_current_order() {
    let mut original = chart(
        "<realDataItems><dataValue xsi:type=\"core:NumberValue\"><value>37.5</value></dataValue><infoValue xsi:type=\"core:StringValue\"><value>current information</value></infoValue><tooltip>current tooltip</tooltip><editMode>Use</editMode></realDataItems><realDataItems><dataValue xsi:type=\"core:BooleanValue\"><value>false</value></dataValue><infoValue xsi:type=\"core:NullValue\"/><tooltip>second</tooltip><editMode>DontUse</editMode></realDataItems>",
    );
    assert_eq!(through_native(&original), original);
    let previous = serde_json::to_vec(&original).unwrap();
    let rows = items(&mut original);
    rows.reverse();
    rows[0].iter_mut().find(|(n, _)| n == "tooltip").unwrap().1 =
        ChartValue::Str("edited current tooltip".into());
    assert_ne!(serde_json::to_vec(&original).unwrap(), previous);
    assert_eq!(through_native(&original), original);
    assert_eq!(
        read_chart_sidecar(&write_chart_sidecar(&original).unwrap()).unwrap(),
        original
    );
}

#[test]
fn nested_value_collections_and_full_type_descriptions_remain_typed() {
    let original = chart(
        "<realDataItems><dataValue xsi:type=\"core:ValueList\"><values xsi:type=\"core:StringValue\"><value>first</value></values><values xsi:type=\"core:FixedArrayValue\"><values xsi:type=\"core:NumberValue\"><value>52</value></values><values xsi:type=\"core:UndefinedValue\"/></values></dataValue><infoValue xsi:type=\"core:TypeDescriptionValue\"><value><types>String</types><stringQualifiers><length>32</length></stringQualifiers></value></infoValue></realDataItems>",
    );
    assert_eq!(through_native(&original), original);
    assert_eq!(
        read_chart_sidecar(&write_chart_sidecar(&original).unwrap()).unwrap(),
        original
    );
    let native = write_form(FormDialect::Designer, &body(original)).unwrap();
    let text = String::from_utf8(native.clone()).unwrap();
    assert_eq!(text.matches("<values xmlns=\"\"").count(), 4);
    for invalid in [
        text.replace("xmlns=\"\"", "xmlns=\"http://wrong.example/values\""),
        text.replace(" xmlns=\"\"", ""),
    ] {
        let error = read_form(FormDialect::Designer, invalid.as_bytes()).unwrap_err();
        assert!(error.to_string().contains("empty namespace URI"), "{error}");
    }
}

#[test]
fn native_aliases_use_expanded_types_instead_of_becoming_system_enumerations() {
    for content in [
        "<dataValue xsi:type=\"core:NumberValue\"><value>12.5</value></dataValue>",
        "<dataValue xsi:type=\"core:ColorValue\"><value xsi:type=\"core:ColorRef\"><color>Windows.HotLight</color></value></dataValue>",
        "<dataValue xsi:type=\"core:TypeDescriptionValue\"><value><types>String</types><stringQualifiers><length>32</length></stringQualifiers></value></dataValue>",
        "<dataValue xsi:type=\"core:StandardPeriodValue\"><value><variant>Today</variant><startDate>2026-10-02T00:00:00</startDate></value></dataValue>",
        "<dataValue xsi:type=\"core:FixedArrayValue\"><values xsi:type=\"core:NumberValue\"><value>2</value></values><values xsi:type=\"core:StringValue\"><value>ordered</value></values></dataValue>",
    ] {
        let settings = chart(&format!("<realDataItems>{content}</realDataItems>"));
        let native =
            String::from_utf8(write_form(FormDialect::Designer, &body(settings.clone())).unwrap())
                .unwrap();
        let start = native.find("<d4p1:dataValue").unwrap();
        let end =
            start + native[start..].find("</d4p1:dataValue>").unwrap() + "</d4p1:dataValue>".len();
        let mut aliased = native[start..end].to_owned();
        for (prefix, alias, uri) in [
            ("xs", "qxs", "http://www.w3.org/2001/XMLSchema"),
            ("v8", "qcore", "http://v8.1c.ru/8.1/data/core"),
            ("v8ui", "qui", "http://v8.1c.ru/8.1/data/ui"),
        ] {
            aliased = aliased.replacen(
                "<d4p1:dataValue",
                &format!("<d4p1:dataValue xmlns:{alias}=\"{uri}\""),
                1,
            );
            aliased = aliased.replace(
                &format!("xsi:type=\"{prefix}:"),
                &format!("xsi:type=\"{alias}:"),
            );
            aliased = aliased
                .replace(&format!("<{prefix}:"), &format!("<{alias}:"))
                .replace(&format!("</{prefix}:"), &format!("</{alias}:"));
            aliased = aliased.replace(&format!(">{prefix}:"), &format!(">{alias}:"));
        }
        let rewritten = format!("{}{}{}", &native[..start], aliased, &native[end..]);
        let returned = read_form(FormDialect::Designer, rewritten.as_bytes()).unwrap();
        assert_eq!(
            returned.data_attributes[0].chart_settings.as_ref().unwrap(),
            &settings
        );
        let wrong = rewritten
            .replace(
                "http://www.w3.org/2001/XMLSchema",
                "http://wrong.example/schema",
            )
            .replace("http://v8.1c.ru/8.1/data/core", "http://wrong.example/core")
            .replace("http://v8.1c.ru/8.1/data/ui", "http://wrong.example/ui");
        assert!(read_form(FormDialect::Designer, wrong.as_bytes()).is_err());
    }
}

#[test]
fn absent_nullable_fields_are_distinct_from_explicit_undefined_null_and_empty_string() {
    let mut empty = chart("<realDataItems/>");
    let row = &items(&mut empty)[0];
    for name in ["dataValue", "infoValue", "tooltip"] {
        assert_eq!(
            row.iter().find(|(n, _)| n == name).unwrap().1,
            ChartValue::Absent
        );
    }
    let output =
        String::from_utf8(write_form(FormDialect::Designer, &body(empty.clone())).unwrap())
            .unwrap();
    assert!(output.contains("<d4p1:item/>"));
    assert!(!output.contains("<d4p1:dataValue"));
    assert!(!output.contains("<d4p1:infoValue"));
    assert!(!output.contains("<d4p1:tooltip"));
    assert_eq!(through_native(&empty), empty);
    for content in [
        "<realDataItems><dataValue xsi:type=\"core:UndefinedValue\"/></realDataItems>",
        "<realDataItems><dataValue xsi:type=\"core:NullValue\"/></realDataItems>",
        "<realDataItems><tooltip></tooltip></realDataItems>",
    ] {
        let explicit = chart(content);
        assert_ne!(explicit, empty);
        assert_eq!(through_native(&explicit), explicit);
        assert_eq!(
            read_chart_sidecar(&write_chart_sidecar(&explicit).unwrap()).unwrap(),
            explicit
        );
    }
}

#[test]
fn duplicate_unknown_data_item_fields_and_value_types_fail() {
    for (content, reason) in [
        (
            "<realDataItems><tooltip>one</tooltip><tooltip>two</tooltip></realDataItems>",
            "duplicate data item field",
        ),
        (
            "<realDataItems><unknownField>true</unknownField></realDataItems>",
            "unrecognized data item field",
        ),
        (
            "<realDataItems><dataValue xsi:type=\"core:UnknownValue\"/></realDataItems>",
            "unknown EDT xsi:type",
        ),
        (
            "<realDataItems><dataValue xsi:type=\"core:NullValue\"><value>wrong</value></dataValue></realDataItems>",
            "Null value has children",
        ),
        (
            "<realDataItems><editMode>FutureUnknown</editMode></realDataItems>",
            "invalid data item edit mode",
        ),
        (
            "<realDataItems><dataValue xmlns:core=\"http://wrong.example/value\" xsi:type=\"core:NumberValue\"><value>1</value></dataValue></realDataItems>",
            "wrong namespace URI",
        ),
        (
            "<realDataItems><dataValue xsi:type=\"core:ValueList\"><values extra=\"unknown\" xsi:type=\"core:NullValue\"/></dataValue></realDataItems>",
            "незнакомый атрибут",
        ),
    ] {
        let error = read_chart_sidecar(source(content).as_bytes()).unwrap_err();
        assert!(
            matches!(error, formats_xml::form::FormError::Frame(_)),
            "{error}"
        );
        assert!(
            error.to_string().contains(reason),
            "expected {reason}: {error}"
        );
    }
}

#[test]
fn nested_binary_font_and_color_values_reuse_complete_current_primitives() {
    let original = chart(
        "<realDataItems><dataValue xsi:type=\"core:ValueList\"><values xsi:type=\"core:BinaryValue\"><value>AAECAw==</value></values><values xsi:type=\"core:FontValue\"><value xsi:type=\"core:FontDef\"><faceName>Arial</faceName><height>14.0</height><bold>true</bold></value></values><values xsi:type=\"core:ColorValue\"><value xsi:type=\"core:ColorRef\"><color>Windows.HotLight</color></value></values><values xsi:type=\"core:BorderValue\"><value xsi:type=\"core:BorderDef\"><style>Single</style><width>2</width></value></values></dataValue></realDataItems>",
    );
    assert_eq!(through_native(&original), original);
    assert_eq!(
        read_chart_sidecar(&write_chart_sidecar(&original).unwrap()).unwrap(),
        original
    );
}
