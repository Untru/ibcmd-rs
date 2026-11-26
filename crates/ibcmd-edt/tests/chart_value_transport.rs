use formats_xml::form::{
    FormDialect, apply_chart_semantics_resource, project_chart_semantics, read_chart_sidecar,
    read_form, write_form,
};
use morph1c_core::ir::{
    FormBody, Uuid,
    form::{ChartTypedValue, ChartValue},
};
const UUID: Uuid = Uuid([9; 16]);
fn source(content: &str) -> FormBody {
    let xml = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<form:Form xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:form=\"http://g5.1c.ru/v8/dt/form\"><attributes><name>Diagram</name><id>1</id><valueType><types>Chart</types></valueType><view><common>true</common></view><edit><common>true</common></edit><extInfo xsi:type=\"form:ChartExtInfo\"/></attributes></form:Form>\r\n";
    let mut body = read_form(FormDialect::Edt, xml.as_bytes()).unwrap();
    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<chart:Chart xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:chart=\"http://g5.1c.ru/v8/dt/chart/model\" xmlns:core=\"http://g5.1c.ru/v8/dt/mcore\">{content}</chart:Chart>\r\n"
    );
    body.data_attributes[0].chart_settings = Some(read_chart_sidecar(xml.as_bytes()).unwrap());
    body
}
fn roundtrip(original: &FormBody) -> (FormBody, Vec<u8>) {
    let (projected, payload) = project_chart_semantics(original, UUID).unwrap().unwrap();
    assert!(project_chart_semantics(&projected, UUID).unwrap().is_none());
    let native = write_form(FormDialect::Designer, &projected).unwrap();
    let mut current = read_form(FormDialect::Designer, &native).unwrap();
    apply_chart_semantics_resource(&mut current, UUID, &payload).unwrap();
    assert_eq!(
        current.data_attributes[0].chart_settings,
        original.data_attributes[0].chart_settings
    );
    (current, payload)
}
fn item(body: &mut FormBody) -> &mut Vec<(String, ChartValue)> {
    let chart = body.data_attributes[0].chart_settings.as_mut().unwrap();
    let ChartValue::Items(rows) = &mut chart
        .fields
        .iter_mut()
        .find(|(n, _)| n == "realDataItems")
        .unwrap()
        .1
    else {
        panic!("typed rows")
    };
    &mut rows[0]
}
#[test]
fn null_references_preserve_direct_state_and_ordered_nested_members() {
    roundtrip(&source(
        "<realDataItems><dataValue xsi:type=\"core:ReferenceValue\"/><infoValue xsi:type=\"core:NullValue\"/></realDataItems>",
    ));
    roundtrip(&source(
        "<realDataItems><dataValue xsi:type=\"core:ValueList\"><values xsi:type=\"core:ReferenceValue\"/><values xsi:type=\"core:ReferenceValue\"/><values xsi:type=\"core:StringValue\"><value>middle</value></values><values xsi:type=\"core:FixedArrayValue\"><values xsi:type=\"core:ReferenceValue\"/><values xsi:type=\"core:ReferenceValue\"><value>Catalog.Current</value></values><values xsi:type=\"core:NullValue\"/></values><values xsi:type=\"core:ReferenceValue\"/></dataValue><isToolTipFormatted>true</isToolTipFormatted></realDataItems>",
    ));
    roundtrip(&source(
        "<realSeriesData><properties><valInfo xsi:type=\"core:ReferenceValue\"/><key xsi:type=\"core:ReferenceValue\"/></properties></realSeriesData>",
    ));
}
#[test]
fn enum_package_identity_and_nullable_system_strings_are_current_values() {
    for value in [
        "<dataValue xsi:type=\"core:EnumValue\"><value>urn:current:first#HorizontalAlign/Left</value></dataValue>",
        "<dataValue xsi:type=\"core:EnumValue\"><value>urn:current:second#HorizontalAlign/Left</value></dataValue>",
        "<dataValue xsi:type=\"core:SysEnumValue\"/>",
        "<dataValue xsi:type=\"core:SysEnumValue\"><value>NoDot</value></dataValue>",
        "<dataValue xsi:type=\"core:SysEnumValue\"><value>FutureEnum.Member.Third</value></dataValue>",
    ] {
        roundtrip(&source(&format!("<realDataItems>{value}</realDataItems>")));
    }
    let first = source(
        "<realDataItems><dataValue xsi:type=\"core:EnumValue\"><value>urn:current:first#HorizontalAlign/Left</value></dataValue></realDataItems>",
    );
    let second = source(
        "<realDataItems><dataValue xsi:type=\"core:EnumValue\"><value>urn:current:second#HorizontalAlign/Left</value></dataValue></realDataItems>",
    );
    let (_, a) = roundtrip(&first);
    let (_, b) = roundtrip(&second);
    assert_ne!(a, b);
}

#[test]
fn lossy_sdk_percentages_retain_the_current_decimal_and_project_exactly_once() {
    for (decimal, expected) in [
        ("2147483648", "-2147483648"),
        ("4294967296", "0"),
        ("100000000000000000000000.1", "1.0000000000000001E23"),
        ("0.12345678901234567890123456789", "0.12345678901234568"),
        ("1.5e-999", "0.0"),
    ] {
        let original = source(&format!(
            "<valuesReferenceLines><chartReferenceLine><semitransparencyPercent xsi:type=\"core:NumberValue\"><value>{decimal}</value></semitransparencyPercent></chartReferenceLine></valuesReferenceLines>"
        ));
        let (projected, payload) = project_chart_semantics(&original, UUID).unwrap().unwrap();
        assert!(project_chart_semantics(&projected, UUID).unwrap().is_none());
        let native = write_form(FormDialect::Designer, &projected).unwrap();
        let text = std::str::from_utf8(&native).unwrap();
        assert!(
            text.contains(&format!(
                "<d4p1:semitransparencyPercent>{expected}</d4p1:semitransparencyPercent>"
            )),
            "{decimal}: {text}"
        );
        let mut current = read_form(FormDialect::Designer, &native).unwrap();
        apply_chart_semantics_resource(&mut current, UUID, &payload).unwrap();
        assert_eq!(
            current.data_attributes[0].chart_settings,
            original.data_attributes[0].chart_settings
        );
        let changed = text.replace(
            &format!(">{expected}</d4p1:semitransparencyPercent>"),
            ">7</d4p1:semitransparencyPercent>",
        );
        let mut current = read_form(FormDialect::Designer, changed.as_bytes()).unwrap();
        let before = serde_json::to_vec(&current).unwrap();
        assert!(apply_chart_semantics_resource(&mut current, UUID, &payload).is_err());
        assert_eq!(serde_json::to_vec(&current).unwrap(), before);
    }
    let overflowing = format!("1{}.1", "0".repeat(400));
    let original = source(&format!(
        "<valuesReferenceLines><chartReferenceLine><semitransparencyPercent xsi:type=\"core:NumberValue\"><value>{overflowing}</value></semitransparencyPercent></chartReferenceLine></valuesReferenceLines>"
    ));
    let (projected, _) = project_chart_semantics(&original, UUID).unwrap().unwrap();
    let native = write_form(FormDialect::Designer, &projected).unwrap();
    assert!(
        std::str::from_utf8(&native)
            .unwrap()
            .contains(">Infinity</d4p1:semitransparencyPercent>")
    );
    roundtrip(&original);
}
#[test]
fn stale_values_and_unknown_nested_payloads_fail_before_flags_are_changed() {
    let original = source(
        "<realDataItems><dataValue xsi:type=\"core:ValueList\"><values xsi:type=\"core:ReferenceValue\"/><values xsi:type=\"core:StringValue\"><value>current</value></values></dataValue><isToolTipFormatted>true</isToolTipFormatted></realDataItems>",
    );
    let (projected, payload) = project_chart_semantics(&original, UUID).unwrap().unwrap();
    let json: serde_json::Value = serde_json::from_slice(&payload).unwrap();
    for mode in ["unknown", "orphan", "duplicate", "rank", "current_conflict"] {
        let mut changed = json.clone();
        match mode {
            "unknown" => changed["values"][0]["current"]["extra"] = true.into(),
            "orphan" => changed["values"][0]["path"][1]["Item"] = 99.into(),
            "duplicate" => {
                let row = changed["values"][0].clone();
                changed["values"].as_array_mut().unwrap().push(row);
            }
            "rank" => changed["values"][0]["field_rank"] = 999.into(),
            "current_conflict" => {
                changed["values"][0]["current"]["ValueList"][1]["Scalar"]["Value"]["scalar"]["Str"] =
                    "forged".into()
            }
            _ => unreachable!(),
        }
        let mut current = projected.clone();
        let before = serde_json::to_vec(&current).unwrap();
        assert!(
            apply_chart_semantics_resource(
                &mut current,
                UUID,
                &serde_json::to_vec(&changed).unwrap()
            )
            .is_err(),
            "{mode}"
        );
        assert_eq!(serde_json::to_vec(&current).unwrap(), before, "{mode}");
    }
    let mut current = projected;
    let field = &mut item(&mut current)
        .iter_mut()
        .find(|(n, _)| n == "dataValue")
        .unwrap()
        .1;
    let ChartValue::Value(value) = field else {
        panic!("typed value")
    };
    let ChartTypedValue::ValueList(values) = value.as_mut() else {
        panic!("typed list")
    };
    values.clear();
    let before = serde_json::to_vec(&current).unwrap();
    assert!(apply_chart_semantics_resource(&mut current, UUID, &payload).is_err());
    assert_eq!(serde_json::to_vec(&current).unwrap(), before);
    let mut edited = original;
    let ChartValue::Value(value) = &mut item(&mut edited)
        .iter_mut()
        .find(|(n, _)| n == "dataValue")
        .unwrap()
        .1
    else {
        panic!("typed value")
    };
    let ChartTypedValue::ValueList(values) = value.as_mut() else {
        panic!("typed list")
    };
    values.reverse();
    values.push(ChartTypedValue::Reference(None));
    roundtrip(&edited);
}
