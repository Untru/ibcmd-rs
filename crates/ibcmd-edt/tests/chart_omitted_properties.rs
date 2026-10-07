use formats_xml::form::{
    FormDialect, apply_chart_semantics_resource, project_chart_semantics, read_chart_sidecar,
    read_form, write_chart_sidecar, write_form,
};
use morph1c_core::ir::{FormBody, Uuid, form::ChartValue};

const UUID: Uuid = Uuid([7; 16]);
fn source(kind: &str, content: &str) -> FormBody {
    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<form:Form xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:form=\"http://g5.1c.ru/v8/dt/form\"><attributes><name>Diagram</name><id>1</id><valueType><types>{kind}</types></valueType><view><common>true</common></view><edit><common>true</common></edit><extInfo xsi:type=\"form:{kind}ExtInfo\"/></attributes></form:Form>\r\n"
    );
    let mut body = read_form(FormDialect::Edt, xml.as_bytes()).unwrap();
    // These controls exercise visible Series/Point omission transport, rather
    // than the separately tested disabled-design whole-collection transport.
    let content = if kind == "Chart" {
        format!(
            "<isSeriesDesign>true</isSeriesDesign><isPointsDesign>true</isPointsDesign>{content}"
        )
    } else {
        content.to_owned()
    };
    let content = content
        .replace("<realSeriesData>", "<realSeriesData><properties>")
        .replace("</realSeriesData>", "</properties></realSeriesData>")
        .replace("<realExSeriesData>", "<realExSeriesData><properties>")
        .replace("</realExSeriesData>", "</properties></realExSeriesData>");
    let prefix = if kind == "Chart" {
        "chart"
    } else {
        "ganttchart"
    };
    let uri = if kind == "Chart" {
        "http://g5.1c.ru/v8/dt/chart/model"
    } else {
        "http://g5.1c.ru/v8/dt/ganttchart/model"
    };
    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<{prefix}:{kind} xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:{prefix}=\"{uri}\" xmlns:core=\"http://g5.1c.ru/v8/dt/mcore\">{content}</{prefix}:{kind}>\r\n"
    );
    body.data_attributes[0].chart_settings = Some(read_chart_sidecar(xml.as_bytes()).unwrap());
    body
}
fn rows<'a>(body: &'a mut FormBody, name: &str) -> &'a mut Vec<Vec<(String, ChartValue)>> {
    let chart = body.data_attributes[0].chart_settings.as_mut().unwrap();
    let ChartValue::Items(items) = &mut chart.fields.iter_mut().find(|(n, _)| n == name).unwrap().1
    else {
        panic!("typed chart collection")
    };
    items
}
fn native_counterpart(projected: &FormBody) -> FormBody {
    let xml = write_form(FormDialect::Designer, projected).unwrap();
    read_form(FormDialect::Designer, &xml).unwrap()
}
fn returned(original: &FormBody) -> FormBody {
    let mut final_return = None;
    for minor in [20, 21] {
        let current = morph1c_core::version::with_roundtrip_target(
            morph1c_core::version::FormatVersion::new(2, minor),
            || {
                let (projected, payload) =
                    project_chart_semantics(original, UUID).unwrap().unwrap();
                assert!(project_chart_semantics(&projected, UUID).unwrap().is_none());
                let mut returned = morph1c_core::version::with_source_version(
                    Some(morph1c_core::version::FormatVersion::new(2, minor)),
                    || native_counterpart(&projected),
                );
                apply_chart_semantics_resource(&mut returned, UUID, &payload).unwrap();
                assert_eq!(
                    returned.data_attributes[0].chart_settings,
                    original.data_attributes[0].chart_settings
                );
                let settings = returned.data_attributes[0].chart_settings.as_ref().unwrap();
                assert_eq!(
                    read_chart_sidecar(&write_chart_sidecar(settings).unwrap()).unwrap(),
                    *settings
                );
                returned
            },
        );
        final_return = Some(current);
    }
    final_return.unwrap()
}
#[test]
fn persisted_series_info_preserves_nullable_values_and_complete_numeric_domain() {
    for content in [
        "<realSeriesData><id>1</id><info><enabled>true</enabled><min>-7.5</min><max>Infinity</max><absMin>-Infinity</absMin><absMax>NaN</absMax><startAngle>3.5</startAngle><stopAngle>360.0</stopAngle><percent>12.5</percent><m_isExpand>true</m_isExpand><endX>-2147483648</endX></info></realSeriesData>",
        "<realSeriesData><id>2</id><info><str></str><m_centerPoint><x>-2147483648</x><y>2147483647</y></m_centerPoint><endX>2147483647</endX></info></realSeriesData>",
        "<realExSeriesData><id>3</id><info><str>current text</str><m_centerPoint/></info></realExSeriesData>",
    ] {
        returned(&source("Chart", content));
    }
    let absent = source("Chart", "<realSeriesData><id>1</id></realSeriesData>");
    let (_, absent_resource) = project_chart_semantics(&absent, UUID).unwrap().unwrap();
    let absent_payload: serde_json::Value = serde_json::from_slice(&absent_resource).unwrap();
    assert_eq!(absent_payload["records"], serde_json::json!([]));
    assert_eq!(
        absent_payload["designs"][0]["current"],
        serde_json::json!([["realExSeriesData", "Absent"]])
    );
    returned(&absent);
    let present = source(
        "Chart",
        "<realSeriesData><id>1</id><info/></realSeriesData>",
    );
    assert_ne!(
        absent.data_attributes[0].chart_settings,
        present.data_attributes[0].chart_settings
    );
    returned(&present);
}
#[test]
fn point_auxiliary_values_and_gantt_interval_color_survive_sdk_omission() {
    returned(&source(
        "Chart",
        "<realPointData><id>1</id><intAdd>-2147483648</intAdd><doubleAdd>-123.25</doubleAdd><endX>2147483647</endX></realPointData><realPointData><id>2</id><doubleAdd>NaN</doubleAdd></realPointData>",
    ));
    returned(&source(
        "GanttChart",
        "<interval><itemKey>1</itemKey><key>2</key><textColor xsi:type=\"core:ColorRef\"><color>Windows.HotLight</color></textColor></interval>",
    ));
}
#[test]
fn inactive_axis_boundaries_and_empty_date_mode_remain_distinct() {
    for interval in [
        "<leftIsNum>true</leftIsNum><leftNum>12.5</leftNum><leftDate>2026-10-02T00:00:00</leftDate><rightIsNum>false</rightIsNum><rightNum>27.5</rightNum><rightDate>2026-10-03T00:00:00</rightDate>",
        "<leftIsNum>false</leftIsNum><leftNum>-7.5</leftNum><rightIsNum>false</rightIsNum>",
    ] {
        returned(&source(
            "Chart",
            &format!("<valuesAxis><interval>{interval}</interval></valuesAxis>"),
        ));
    }
}
#[test]
fn trend_topology_uses_current_native_lines_and_preserves_only_unprojected_lines() {
    let original = source(
        "Chart",
        "<realSeriesData><id>1</id></realSeriesData><realSeriesData><id>2</id></realSeriesData><trendLinesArray><seriesId>2</seriesId><line><text><key>ru</key><value>projected two</value></text></line></trendLinesArray><trendLinesArray><seriesId>1</seriesId><line><text><key>ru</key><value>projected one</value></text></line></trendLinesArray><trendLinesArray><seriesId>2</seriesId><line><text><key>ru</key><value>unprojected duplicate</value></text></line></trendLinesArray><trendLinesArray><seriesId>99</seriesId><line><text><key>ru</key><value>unprojected orphan</value></text></line></trendLinesArray><trendLinesArray><seriesId>77</seriesId></trendLinesArray>",
    );
    let (_, payload) = project_chart_semantics(&original, UUID).unwrap().unwrap();
    let text = std::str::from_utf8(&payload).unwrap();
    assert!(!text.contains("projected two"));
    assert!(!text.contains("projected one"));
    assert!(text.contains("unprojected duplicate"));
    assert!(text.contains("unprojected orphan"));
    returned(&original);
    let (projected, bytes) = project_chart_semantics(&original, UUID).unwrap().unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    for mode in [
        "unknown",
        "wrong_native_index",
        "orphan",
        "duplicate",
        "stale",
    ] {
        let mut changed = json.clone();
        match mode {
            "unknown" => changed["trends"][0]["current"]["arrays"][2]["extra"] = true.into(),
            "wrong_native_index" => {
                changed["trends"][0]["current"]["arrays"][0]["lines"]["Native"] = 999.into()
            }
            "orphan" => changed["trends"][0]["attribute"][0]["name"] = "Missing".into(),
            "duplicate" => {
                let row = changed["trends"][0].clone();
                changed["trends"].as_array_mut().unwrap().push(row);
            }
            "stale" => changed["trends"][0]["current_native_sha256"] = "0".repeat(64).into(),
            _ => unreachable!(),
        }
        let mut body = native_counterpart(&projected);
        let before = serde_json::to_vec(&body).unwrap();
        assert!(
            apply_chart_semantics_resource(&mut body, UUID, &serde_json::to_vec(&changed).unwrap())
                .is_err(),
            "{mode}"
        );
        assert_eq!(serde_json::to_vec(&body).unwrap(), before, "{mode}");
    }
    let mut edited = original;
    rows(&mut edited, "trendLinesArray").reverse();
    returned(&edited);
}
#[test]
fn every_binding_is_validated_before_any_omitted_property_is_restored() {
    let original = source(
        "Chart",
        "<realSeriesData><id>1</id><info><str>first</str></info></realSeriesData><realSeriesData><id>2</id><info><str>second</str></info></realSeriesData>",
    );
    let (projected, payload) = project_chart_semantics(&original, UUID).unwrap().unwrap();
    let json: serde_json::Value = serde_json::from_slice(&payload).unwrap();
    for mutation in [
        "unknown",
        "orphan",
        "duplicate",
        "bad_double",
        "extra_flag",
        "stale",
    ] {
        let mut changed = json.clone();
        match mutation {
            "unknown" => changed["records"][1]["series_info"]["unknown"] = true.into(),
            "orphan" => changed["records"][1]["path"][1]["Item"] = 999.into(),
            "duplicate" => changed["records"][1] = changed["records"][0].clone(),
            "bad_double" => changed["records"][1]["series_info"]["min"] = "invalid".into(),
            "extra_flag" => changed["records"][1]["flags"] = serde_json::json!([["enabled", true]]),
            "stale" => changed["records"][1]["current_item_sha256"] = "0".repeat(64).into(),
            _ => unreachable!(),
        }
        let mut current = native_counterpart(&projected);
        let before = serde_json::to_vec(&current).unwrap();
        assert!(
            apply_chart_semantics_resource(
                &mut current,
                UUID,
                &serde_json::to_vec(&changed).unwrap()
            )
            .is_err(),
            "{mutation}"
        );
        assert_eq!(serde_json::to_vec(&current).unwrap(), before, "{mutation}");
    }
    let mut current = native_counterpart(&projected);
    rows(&mut current, "realSeriesData")[1].retain(|(n, _)| n != "info");
    let before = serde_json::to_vec(&current).unwrap();
    assert!(apply_chart_semantics_resource(&mut current, UUID, &payload).is_err());
    assert_eq!(serde_json::to_vec(&current).unwrap(), before);
    let mut current = native_counterpart(&projected);
    rows(&mut current, "realSeriesData").reverse();
    let before = serde_json::to_vec(&current).unwrap();
    assert!(apply_chart_semantics_resource(&mut current, UUID, &payload).is_err());
    assert_eq!(serde_json::to_vec(&current).unwrap(), before);
    let mut changed = original;
    rows(&mut changed, "realSeriesData").reverse();
    returned(&changed);
}
