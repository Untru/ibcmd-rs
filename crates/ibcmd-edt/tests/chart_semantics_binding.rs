use formats_xml::form::{
    FormDialect, apply_chart_semantics_resource, chart_semantics_resource_count,
    project_chart_semantics, read_chart_sidecar, read_form, same_chart_semantics_resource,
    write_form,
};
use morph1c_core::ir::{FormBody, Uuid, form::ChartValue};

fn assert_resource_shape(bytes: &[u8], expected: [usize; 5]) -> usize {
    let payload: serde_json::Value = serde_json::from_slice(bytes).unwrap();
    let keys = ["records", "values", "trends", "designs", "numbers"];
    let actual = keys.map(|key| {
        payload
            .get(key)
            .map_or(0, |value| value.as_array().unwrap().len())
    });
    assert_eq!(actual, expected, "exact CURRENT transport categories");
    if expected[3] == 1 {
        assert_eq!(
            payload["designs"][0]["current"],
            serde_json::json!([["realExSeriesData", "Absent"]])
        );
    }
    actual.into_iter().sum()
}

const UUID: Uuid = Uuid([1; 16]);
fn original() -> FormBody {
    let xml = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<form:Form xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:form=\"http://g5.1c.ru/v8/dt/form\"><attributes><name>Diagram</name><id>1</id><valueType><types>Chart</types></valueType><view><common>true</common></view><edit><common>true</common></edit><extInfo xsi:type=\"form:ChartExtInfo\"/></attributes></form:Form>\r\n";
    let mut body = read_form(FormDialect::Edt, xml.as_bytes()).unwrap();
    let xml = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<chart:Chart xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:chart=\"http://g5.1c.ru/v8/dt/chart/model\" xmlns:core=\"http://g5.1c.ru/v8/dt/mcore\"><realDataItems><tooltip>first current</tooltip><isToolTipFormatted>true</isToolTipFormatted></realDataItems><realDataItems><tooltip>second current</tooltip><isToolTipFormatted>true</isToolTipFormatted></realDataItems></chart:Chart>\r\n";
    body.data_attributes[0].chart_settings = Some(read_chart_sidecar(xml.as_bytes()).unwrap());
    body
}
fn rows(body: &mut FormBody) -> &mut Vec<Vec<(String, ChartValue)>> {
    let chart = body.data_attributes[0].chart_settings.as_mut().unwrap();
    let ChartValue::Items(items) = &mut chart
        .fields
        .iter_mut()
        .find(|(n, _)| n == "realDataItems")
        .unwrap()
        .1
    else {
        panic!("typed items")
    };
    items
}

fn native_counterpart(projected: &FormBody) -> FormBody {
    let xml = write_form(FormDialect::Designer, projected).unwrap();
    read_form(FormDialect::Designer, &xml).unwrap()
}

#[test]
fn closed_current_flags_restore_after_native_projection_without_previous_values() {
    let original = original();
    assert_eq!(chart_semantics_resource_count(&original).unwrap(), Some(3));
    let (projected, resource) = project_chart_semantics(&original, UUID).unwrap().unwrap();
    assert_eq!(assert_resource_shape(&resource, [2, 0, 0, 1, 0]), 3);
    assert_eq!(chart_semantics_resource_count(&projected).unwrap(), None);
    assert!(project_chart_semantics(&projected, UUID).unwrap().is_none());
    let payload = std::str::from_utf8(&resource).unwrap();
    assert!(!payload.contains("first current"));
    assert!(!payload.contains("second current"));
    assert!(same_chart_semantics_resource(&resource, &resource));
    let xml = write_form(FormDialect::Designer, &projected).unwrap();
    let mut returned = read_form(FormDialect::Designer, &xml).unwrap();
    apply_chart_semantics_resource(&mut returned, UUID, &resource).unwrap();
    assert_eq!(
        returned.data_attributes[0].chart_settings,
        original.data_attributes[0].chart_settings
    );
    assert_eq!(chart_semantics_resource_count(&original).unwrap(), Some(3));
}

#[test]
fn invalid_second_record_never_partially_applies_first_record() {
    let (projected, bytes) = project_chart_semantics(&original(), UUID).unwrap().unwrap();
    let mut projected = native_counterpart(&projected);
    let before = serde_json::to_vec(&projected).unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    for mutation in ["digest", "orphan", "unknown", "duplicate", "default"] {
        let mut changed = json.clone();
        match mutation {
            "digest" => changed["records"][1]["current_item_sha256"] = "0".repeat(64).into(),
            "orphan" => changed["records"][1]["path"][1]["Item"] = 999.into(),
            "unknown" => changed["records"][1]["extra"] = true.into(),
            "duplicate" => changed["records"][1] = changed["records"][0].clone(),
            "default" => changed["records"][1]["flags"][0][1] = false.into(),
            _ => unreachable!(),
        }
        let invalid = serde_json::to_vec(&changed).unwrap();
        assert!(
            apply_chart_semantics_resource(&mut projected, UUID, &invalid).is_err(),
            "{mutation}"
        );
        assert_eq!(
            serde_json::to_vec(&projected).unwrap(),
            before,
            "{mutation}"
        );
        assert!(!same_chart_semantics_resource(&bytes, &invalid));
    }
    assert!(apply_chart_semantics_resource(&mut projected, Uuid([2; 16]), &bytes).is_err());
    assert_eq!(serde_json::to_vec(&projected).unwrap(), before);
}

#[test]
fn current_edits_and_reordering_reject_stale_bindings_and_new_projection_restores_current_flags() {
    let (projected, old) = project_chart_semantics(&original(), UUID).unwrap().unwrap();
    let mut projected = native_counterpart(&projected);
    rows(&mut projected).reverse();
    let before = serde_json::to_vec(&projected).unwrap();
    assert!(apply_chart_semantics_resource(&mut projected, UUID, &old).is_err());
    assert_eq!(serde_json::to_vec(&projected).unwrap(), before);
    let mut current = original();
    rows(&mut current).reverse();
    rows(&mut current)[0]
        .iter_mut()
        .find(|(n, _)| n == "tooltip")
        .unwrap()
        .1 = ChartValue::Str("edited current".into());
    let (projected, fresh) = project_chart_semantics(&current, UUID).unwrap().unwrap();
    let mut projected = native_counterpart(&projected);
    assert!(!same_chart_semantics_resource(&old, &fresh));
    apply_chart_semantics_resource(&mut projected, UUID, &fresh).unwrap();
    assert_eq!(
        serde_json::to_vec(&projected).unwrap(),
        serde_json::to_vec(&current).unwrap()
    );
}
