use formats_xml::form::{
    FormDialect, apply_chart_semantics_resource, project_chart_semantics, read_chart_sidecar,
    read_form, write_chart_sidecar, write_form,
};
use morph1c_core::ir::form::{ChartSettings, ChartValue};
use morph1c_core::version::{FormatVersion, with_roundtrip_target, with_source_version};

fn source(content: &str) -> Vec<u8> {
    format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<chart:Chart xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:chart=\"http://g5.1c.ru/v8/dt/chart/model\" xmlns:core=\"http://g5.1c.ru/v8/dt/mcore\">{content}</chart:Chart>\r\n").into_bytes()
}
fn form_with_chart(chart: ChartSettings) -> morph1c_core::ir::FormBody {
    let source = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<form:Form xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:form=\"http://g5.1c.ru/v8/dt/form\"><attributes><name>Diagram</name><id>1</id><valueType><types>Chart</types></valueType><view><common>true</common></view><edit><common>true</common></edit><extInfo xsi:type=\"form:ChartExtInfo\"/></attributes></form:Form>\r\n";
    let mut body = read_form(FormDialect::Edt, source.as_bytes()).unwrap();
    body.data_attributes[0].chart_settings = Some(chart);
    body
}
const CHART_FORM_UUID: morph1c_core::ir::Uuid = morph1c_core::ir::Uuid([83; 16]);

fn through_native(chart: &ChartSettings, version: FormatVersion) -> (Vec<u8>, ChartSettings) {
    let (native, resource) = with_roundtrip_target(version, || {
        let current = form_with_chart(chart.clone());
        let projection = project_chart_semantics(&current, CHART_FORM_UUID).unwrap();
        let native = write_form(
            FormDialect::Designer,
            projection
                .as_ref()
                .map_or(&current, |(projected, _)| projected),
        )
        .unwrap();
        (native, projection.map(|(_, resource)| resource))
    });
    let mut returned =
        with_source_version(Some(version), || read_form(FormDialect::Designer, &native)).unwrap();
    if let Some(resource) = resource {
        apply_chart_semantics_resource(&mut returned, CHART_FORM_UUID, &resource).unwrap();
    }
    let chart = returned.data_attributes[0].chart_settings.clone().unwrap();
    (native, chart)
}
fn value<'a>(chart: &'a ChartSettings, name: &str) -> &'a ChartValue {
    &chart.fields.iter().find(|(n, _)| n == name).unwrap().1
}
#[test]
fn full_fonts_keep_current_absolute_system_and_auto_overrides() {
    for version in [FormatVersion::new(2, 20), FormatVersion::new(2, 21)] {
        for (xsi, children, expected) in [
            (
                "FontDef",
                "<faceName>Arial</faceName><height>12.0</height><bold>true</bold><italic>true</italic><underline>true</underline><strikeout>true</strikeout><scale>150</scale>",
                "kind=\"Absolute\"",
            ),
            (
                "FontRef",
                "<font>System.DefaultGUIFont</font><faceName>Arial</faceName><height>11.0</height><bold>false</bold><italic>true</italic><underline>false</underline><strikeout>true</strikeout><scale>125</scale>",
                "kind=\"WindowsFont\"",
            ),
            (
                "AutoFont",
                "<faceName>Arial</faceName><height>13.0</height><bold>false</bold><italic>true</italic><underline>false</underline><strikeout>true</strikeout><scale>130</scale>",
                "kind=\"AutoFont\"",
            ),
            ("FontDef", "", "height=\"0\""),
        ] {
            let chart = read_chart_sidecar(&source(&format!(
                "<labelsFont xsi:type=\"core:{xsi}\">{children}</labelsFont>"
            )))
            .unwrap();
            let (native, returned) = through_native(&chart, version);
            assert!(String::from_utf8_lossy(&native).contains(expected));
            assert_eq!(value(&chart, "labelsFont"), value(&returned, "labelsFont"));
            let own = write_chart_sidecar(&chart).unwrap();
            assert_eq!(
                value(&read_chart_sidecar(&own).unwrap(), "labelsFont"),
                value(&chart, "labelsFont")
            );
            let mut edited = chart.clone();
            let ChartValue::Font(font) = &mut edited
                .fields
                .iter_mut()
                .find(|(n, _)| n == "labelsFont")
                .unwrap()
                .1
            else {
                panic!("font")
            };
            font.height = Some("17.0".into());
            font.scale = Some("175".into());
            let (current, returned) = through_native(&edited, version);
            assert!(String::from_utf8_lossy(&current).contains("height=\"17\""));
            assert!(String::from_utf8_lossy(&current).contains("scale=\"175\""));
            assert_eq!(value(&returned, "labelsFont"), value(&edited, "labelsFont"));
        }
    }
}
#[test]
fn current_color_namespaces_and_edits_use_shared_codec() {
    for color in [
        "Windows.Highlight",
        "Windows.HotLight",
        "Web.Snow",
        "Palette.Example",
        "Style.FormBackColor",
    ] {
        let mut chart = read_chart_sidecar(&source(&format!(
            "<labelsColor xsi:type=\"core:ColorRef\"><color>{color}</color></labelsColor>"
        )))
        .unwrap();
        let (native, returned) = through_native(&chart, FormatVersion::new(2, 21));
        let expected = color
            .replace("Windows.", "win:")
            .replace("Web.", "web:")
            .replace("Palette.", "pal:")
            .replace("Style.", "style:");
        assert!(
            String::from_utf8_lossy(&native)
                .contains(&format!("<d4p1:labelsColor>{expected}</d4p1:labelsColor>"))
        );
        assert_eq!(
            value(&chart, "labelsColor"),
            value(&returned, "labelsColor")
        );
        chart
            .fields
            .iter_mut()
            .find(|(n, _)| n == "labelsColor")
            .unwrap()
            .1 = ChartValue::Color("#A1B2C3".into());
        let (current, returned) = through_native(&chart, FormatVersion::new(2, 21));
        assert!(
            String::from_utf8_lossy(&current)
                .contains("<d4p1:labelsColor>#A1B2C3</d4p1:labelsColor>")
        );
        assert_eq!(
            value(&chart, "labelsColor"),
            value(&returned, "labelsColor")
        );
    }
}
#[test]
fn line_gap_is_current_and_repeated_series_order_is_semantic() {
    let chart = read_chart_sidecar(&source("<isSeriesDesign>true</isSeriesDesign><realSeriesData><properties><id>2</id><line><width>3</width><gap>true</gap><style>Dashed</style></line></properties></realSeriesData><realSeriesData><properties><id>1</id><line><width>4</width><style>Solid</style></line></properties></realSeriesData>")).unwrap();
    let (native, returned) = through_native(&chart, FormatVersion::new(2, 21));
    assert!(String::from_utf8_lossy(&native).contains("width=\"3\" gap=\"true\""));
    assert_eq!(
        value(&chart, "realSeriesData"),
        value(&returned, "realSeriesData")
    );
    let mut disabled = chart.clone();
    disabled
        .fields
        .iter_mut()
        .find(|(name, _)| name == "isSeriesDesign")
        .unwrap()
        .1 = ChartValue::Bool(false);
    let (disabled_native, disabled_returned) = through_native(&disabled, FormatVersion::new(2, 21));
    assert!(!String::from_utf8_lossy(&disabled_native).contains("width=\"3\" gap=\"true\""));
    assert_eq!(disabled_returned, disabled);
    let own = write_chart_sidecar(&chart).unwrap();
    assert!(String::from_utf8_lossy(&own).contains("<gap>true</gap>"));
    let sparse = read_chart_sidecar(&source(
        "<isSeriesDesign>true</isSeriesDesign><realSeriesData><properties><line><gap>true</gap></line></properties></realSeriesData>",
    ))
    .unwrap();
    let (sparse_native, sparse_returned) = through_native(&sparse, FormatVersion::new(2, 21));
    assert!(String::from_utf8_lossy(&sparse_native).contains("width=\"0\" gap=\"true\""));
    assert_eq!(
        value(&sparse, "realSeriesData"),
        value(&sparse_returned, "realSeriesData")
    );
    let mut edited = chart.clone();
    let ChartValue::Items(items) = &mut edited
        .fields
        .iter_mut()
        .find(|(n, _)| n == "realSeriesData")
        .unwrap()
        .1
    else {
        panic!("items")
    };
    items.reverse();
    let ChartValue::Line { gap, width, .. } =
        &mut items[1].iter_mut().find(|(n, _)| n == "line").unwrap().1
    else {
        panic!("line")
    };
    *gap = false;
    *width = "7".into();
    let (current, returned) = through_native(&edited, FormatVersion::new(2, 21));
    assert!(String::from_utf8_lossy(&current).contains("width=\"7\" gap=\"false\""));
    assert_ne!(
        value(&chart, "realSeriesData"),
        value(&edited, "realSeriesData")
    );
    assert_eq!(
        value(&returned, "realSeriesData"),
        value(&edited, "realSeriesData")
    );
}
#[test]
fn primitive_unknown_namespace_duplicate_and_malformed_values_fail_closed() {
    for content in [
        "<labelsColor xsi:type=\"core:ColorRef\"><color>Unknown.Highlight</color></labelsColor>",
        "<labelsColor xsi:type=\"core:ColorRef\"><color>Windows.</color></labelsColor>",
        "<labelsColor xsi:type=\"core:ColorDef\"><red>256</red></labelsColor>",
        "<labelsFont xsi:type=\"core:FontRef\"><font>Unknown.Font</font></labelsFont>",
        "<labelsFont xsi:type=\"core:FontDef\"><height>12.0</height><height>13.0</height></labelsFont>",
        "<labelsFont xsi:type=\"core:AutoFont\"><bold>invalid</bold></labelsFont>",
        "<realSeriesData><properties><line><width>2</width><gap>true</gap><gap>false</gap><style>Solid</style></line></properties></realSeriesData>",
        "<realSeriesData><properties><line><style>Dash</style></line></properties></realSeriesData>",
    ] {
        assert!(read_chart_sidecar(&source(content)).is_err());
    }
    let bytes = source("<labelsFont xsi:type=\"core:FontDef\"><height>12.0</height></labelsFont>");
    let bad = String::from_utf8(bytes)
        .unwrap()
        .replace("http://g5.1c.ru/v8/dt/mcore", "urn:unknown");
    assert!(read_chart_sidecar(bad.as_bytes()).is_err());
    let mut chart = read_chart_sidecar(&source("")).unwrap();
    chart
        .fields
        .iter_mut()
        .find(|(n, _)| n == "labelsColor")
        .unwrap()
        .1 = ChartValue::Color("#AГ©AAA".into());
    assert!(write_chart_sidecar(&chart).is_err());
}
