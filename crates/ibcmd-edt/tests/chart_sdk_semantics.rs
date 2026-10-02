use formats_xml::form::{
    FormDialect, read_chart_sidecar, read_form, write_chart_sidecar, write_form,
};
use morph1c_core::ir::form::ChartValue;

fn form_with_chart(chart: morph1c_core::ir::form::ChartSettings) -> morph1c_core::ir::FormBody {
    let source = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<form:Form xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:form=\"http://g5.1c.ru/v8/dt/form\"><attributes><name>Diagram</name><id>1</id><valueType><types>Chart</types></valueType><view><common>true</common></view><edit><common>true</common></edit><extInfo xsi:type=\"form:ChartExtInfo\"/></attributes></form:Form>\r\n";
    let mut body = read_form(FormDialect::Edt, source.as_bytes()).unwrap();
    body.data_attributes[0].chart_settings = Some(chart);
    body
}
fn source(content: &str) -> Vec<u8> {
    format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<chart:Chart xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:chart=\"http://g5.1c.ru/v8/dt/chart/model\" xmlns:core=\"http://g5.1c.ru/v8/dt/mcore\">{content}</chart:Chart>\r\n").into_bytes()
}

#[test]
fn translucence_current_values_have_native_projection_and_semantic_identity() {
    for mode in ["Auto", "DontUse", "Use"] {
        let chart = read_chart_sidecar(&source(&format!("<translucenceMode>{mode}</translucenceMode><translucencePercent>37</translucencePercent>"))).unwrap();
        let native = write_form(FormDialect::Designer, &form_with_chart(chart.clone())).unwrap();
        let decoded = read_form(FormDialect::Designer, &native)
            .unwrap()
            .data_attributes[0]
            .chart_settings
            .clone()
            .unwrap();
        let a = serde_json::to_value(&chart).unwrap();
        let b = serde_json::to_value(&decoded).unwrap();
        let mut differences = Vec::new();
        collect_paths(&a, &b, "", &mut differences);
        assert!(
            differences.is_empty(),
            "current chart differences: {differences:?}"
        );
        let settings = std::str::from_utf8(&native).unwrap();
        assert!(settings.contains("<d4p1:translucencePercent>37</d4p1:translucencePercent>"));
        assert_eq!(settings.contains("<d4p1:translucenceMode>"), mode != "Auto");
    }
}

#[test]
fn chart_layout_never_restores_previous_current_values() {
    let mut chart = read_chart_sidecar(&source(
        "<translucenceMode>Auto</translucenceMode><translucencePercent>37</translucencePercent>",
    ))
    .unwrap();
    let original = serde_json::to_vec(&chart).unwrap();
    chart
        .fields
        .iter_mut()
        .find(|(n, _)| n == "translucencePercent")
        .unwrap()
        .1 = ChartValue::Int("52".into());
    assert_ne!(serde_json::to_vec(&chart).unwrap(), original);
    let output = write_chart_sidecar(&chart).unwrap();
    assert!(std::str::from_utf8(&output).unwrap().contains(">52<"));
    let decoded = read_chart_sidecar(&output).unwrap();
    assert_eq!(decoded, chart);
    assert!(
        read_chart_sidecar(&source(
            "<translucenceMode>FutureUnknown</translucenceMode>"
        ))
        .is_err()
    );
}

fn collect_paths(a: &serde_json::Value, b: &serde_json::Value, path: &str, out: &mut Vec<String>) {
    if a == b {
        return;
    }
    match (a, b) {
        (serde_json::Value::Object(x), serde_json::Value::Object(y)) => {
            let keys: std::collections::BTreeSet<_> = x.keys().chain(y.keys()).collect();
            for k in keys {
                let p = format!("{path}/{k}");
                match (x.get(k), y.get(k)) {
                    (Some(a), Some(b)) => collect_paths(a, b, &p, out),
                    _ => out.push(p),
                }
            }
        }
        (serde_json::Value::Array(x), serde_json::Value::Array(y)) => {
            if x.len() != y.len() {
                out.push(format!(
                    "{path}/length {}->{}; fields {:?} -> {:?}",
                    x.len(),
                    y.len(),
                    x.iter()
                        .filter_map(|v| v
                            .as_array()
                            .and_then(|p| p.first())
                            .and_then(|n| n.as_str()))
                        .collect::<Vec<_>>(),
                    y.iter()
                        .filter_map(|v| v
                            .as_array()
                            .and_then(|p| p.first())
                            .and_then(|n| n.as_str()))
                        .collect::<Vec<_>>()
                ))
            }
            for (i, (a, b)) in x.iter().zip(y).enumerate() {
                collect_paths(a, b, &format!("{path}/{i}"), out)
            }
        }
        _ => out.push(path.into()),
    }
}

#[test]
#[ignore = "requires genuine immutable UH source corpus"]
fn all54_genuine_chart_sidecars_preserve_current_semantics() {
    let cases:serde_json::Value=serde_json::from_slice(&std::fs::read("F:/ibcmd/lab/07/whole-form-preflight-491ccee5-readonly-diagnosis-r2/cases-complete.json").unwrap()).unwrap();
    let mut count = 0;
    let mut sdk_differences = Vec::new();
    for case in cases
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["phase"] == "native-write")
    {
        let dir = std::path::Path::new(case["form"].as_str().unwrap())
            .parent()
            .unwrap();
        let mut stack = vec![dir.to_owned()];
        while let Some(dir) = stack.pop() {
            for e in std::fs::read_dir(dir).unwrap() {
                let p = e.unwrap().path();
                if p.is_dir() {
                    stack.push(p);
                    continue;
                }
                if p.extension().is_none_or(|e| e != "chart") {
                    continue;
                }
                let relative = dir_placeholder(case);
                let sdk_form = relative.join("Ext/Form.xml");
                let sdk_bytes = std::fs::read(&sdk_form).unwrap();
                let target_name = p
                    .parent()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap();
                let isolated = isolated_sdk_chart_attribute(&sdk_bytes, target_name);
                let sdk_body = read_form(FormDialect::Designer, &isolated).unwrap();
                let target_name = p
                    .parent()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap();
                let bytes = std::fs::read(&p).unwrap();
                let native_own = write_form(FormDialect::Designer, &sdk_body).unwrap();
                let native_own_parsed = formats_xml::parse(&native_own).unwrap();
                let source_parsed = formats_xml::parse(&isolated).unwrap();
                let original_settings = source_parsed.root.child("Attributes").unwrap().children[0]
                    .child("Settings")
                    .unwrap();
                let returned_settings =
                    native_own_parsed.root.child("Attributes").unwrap().children[0]
                        .child("Settings")
                        .unwrap();
                let mut own_native_paths = Vec::new();
                collect_xml_paths(
                    original_settings,
                    returned_settings,
                    "Settings",
                    &mut own_native_paths,
                );
                assert!(
                    own_native_paths.is_empty(),
                    "same-native Settings lexical paths {count}: {own_native_paths:?}"
                );
                let chart = read_chart_sidecar(&bytes).unwrap();
                let own = write_chart_sidecar(&chart).unwrap();
                assert!(own == bytes, "same-source chart bytes {count}");
                let own_decoded = read_chart_sidecar(&own).unwrap();
                assert_eq!(chart, own_decoded, "same-source chart {count}");
                let native =
                    write_form(FormDialect::Designer, &form_with_chart(chart.clone())).unwrap();
                let returned = read_form(FormDialect::Designer, &native)
                    .unwrap()
                    .data_attributes[0]
                    .chart_settings
                    .clone()
                    .unwrap();
                let mut paths = Vec::new();
                collect_paths(
                    &serde_json::to_value(&chart).unwrap(),
                    &serde_json::to_value(&returned).unwrap(),
                    "",
                    &mut paths,
                );
                assert!(paths.is_empty(), "genuine chart {count}: {paths:?}");
                let a = formats_xml::parse(&isolated).unwrap();
                let b = formats_xml::parse(&native).unwrap();
                let sa = a.root.child("Attributes").unwrap().children[0]
                    .child("Settings")
                    .unwrap();
                let sb = b.root.child("Attributes").unwrap().children[0]
                    .child("Settings")
                    .unwrap();
                let mut sdk_tree_paths = Vec::new();
                collect_xml_paths(sa, sb, "Settings", &mut sdk_tree_paths);
                if !sdk_tree_paths.is_empty() {
                    sdk_differences.push((count, sdk_tree_paths));
                }
                let sdk_chart = sdk_body
                    .data_attributes
                    .iter()
                    .find(|a| a.name == target_name)
                    .and_then(|a| a.chart_settings.as_ref())
                    .expect("paired SDK chart attribute");
                let mut paths = Vec::new();
                collect_paths(
                    &serde_json::to_value(&chart).unwrap(),
                    &serde_json::to_value(sdk_chart).unwrap(),
                    "",
                    &mut paths,
                );
                assert!(paths.is_empty(), "actual SDK chart {count}: {paths:?}");
                count += 1;
            }
        }
    }
    assert_eq!(count, 54);
    if let Ok(path) = std::env::var("IBCMD_CHART_RAW_DIFF_REPORT") {
        assert!(path.starts_with("F:/ibcmd/lab/07/"));
        let mut output = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap();
        use std::io::Write;
        output
            .write_all(&serde_json::to_vec_pretty(&sdk_differences).unwrap())
            .unwrap();
    }
    assert!(
        sdk_differences.is_empty(),
        "actual SDK Settings differences in {} of {count} charts; inspect optional system-path report",
        sdk_differences.len()
    );
}

#[test]
fn current_axis_bounds_duplicates_and_enum_edits_are_checked() {
    let chart=read_chart_sidecar(&source("<valuesAxis><interval><leftIsNum>true</leftIsNum><leftNum>5.0</leftNum><rightIsNum>true</rightIsNum><rightNum>8.0</rightNum></interval><minValueDetectionMethod>AutoDetect</minValueDetectionMethod><maxValueDetectionMethod>AutoDetect</maxValueDetectionMethod></valuesAxis>")).unwrap();
    let native = write_form(FormDialect::Designer, &form_with_chart(chart.clone())).unwrap();
    assert!(
        std::str::from_utf8(&native)
            .unwrap()
            .contains(">5</d4p1:minValue>")
    );
    let decoded = read_form(FormDialect::Designer, &native)
        .unwrap()
        .data_attributes[0]
        .chart_settings
        .clone()
        .unwrap();
    assert_eq!(decoded, chart);
    let mut duplicate = chart.clone();
    duplicate.fields.push(duplicate.fields[0].clone());
    assert!(write_chart_sidecar(&duplicate).is_err());
    let mut invalid = chart;
    invalid
        .fields
        .iter_mut()
        .find(|(n, _)| n == "translucenceMode")
        .unwrap()
        .1 = ChartValue::Enum("FutureUnknown".into());
    assert!(write_chart_sidecar(&invalid).is_err());
}

#[test]
fn ordered_nonempty_gauge_and_reference_current_values_roundtrip() {
    let content = "<gaugeQualityBands><items><begin>2</begin><end>9</end><textString>A</textString></items><items><begin>15</begin><end>23</end><tooltipString>B</tooltipString></items><useTextStr>true</useTextStr></gaugeQualityBands><valuesReferenceLines><chartReferenceLine><value xsi:type=\"core:NumberValue\"><value>3.5</value></value><position>Auto</position></chartReferenceLine></valuesReferenceLines>";
    let mut chart = read_chart_sidecar(&source(content)).unwrap();
    let output = write_form(FormDialect::Designer, &form_with_chart(chart.clone())).unwrap();
    let decoded = read_form(FormDialect::Designer, &output)
        .unwrap()
        .data_attributes[0]
        .chart_settings
        .clone()
        .unwrap();
    assert_eq!(decoded, chart);
    let source_digest = serde_json::to_vec(&chart).unwrap();
    let ChartValue::Nested(gauge) = &mut chart
        .fields
        .iter_mut()
        .find(|(n, _)| n == "gaugeQualityBands")
        .unwrap()
        .1
    else {
        panic!()
    };
    let ChartValue::Items(items) = &mut gauge.iter_mut().find(|(n, _)| n == "items").unwrap().1
    else {
        panic!()
    };
    items.reverse();
    assert_ne!(serde_json::to_vec(&chart).unwrap(), source_digest);
    let output = write_form(FormDialect::Designer, &form_with_chart(chart.clone())).unwrap();
    let decoded = read_form(FormDialect::Designer, &output)
        .unwrap()
        .data_attributes[0]
        .chart_settings
        .clone()
        .unwrap();
    assert_eq!(decoded, chart);
}

fn dir_placeholder(case: &serde_json::Value) -> std::path::PathBuf {
    let p = std::path::Path::new(case["form"].as_str().unwrap());
    let root = std::path::Path::new(case["root"].as_str().unwrap());
    std::path::Path::new("F:/ibcmd/lab/07")
        .join(if case["corpus"] == "83" {
            "oracle-uha83-r1"
        } else {
            "oracle-uha85-r2"
        })
        .join("edt-native-xml")
        .join(p.strip_prefix(root).unwrap().parent().unwrap())
}

// The full native source is parsed first. This scopes the comparison to the complete
// genuine chart Attribute; it does not claim unrelated native form controls are supported.
fn isolated_sdk_chart_attribute(bytes: &[u8], name: &str) -> Vec<u8> {
    let descriptor = formats_xml::parse(bytes).unwrap();
    let mut root = descriptor.root;
    root.children.retain(|e| e.local == "Attributes");
    let attributes = root.children.first_mut().expect("native attributes");
    attributes
        .children
        .retain(|e| e.attr("name").is_some_and(|a| a.value == name));
    assert_eq!(attributes.children.len(), 1, "unique genuine chart owner");
    fn output(el: &formats_xml::descriptor::Element) -> formats_xml::OutElement {
        let mut result = if el.children.is_empty() && el.text.is_empty() {
            formats_xml::OutElement::self_closing(&el.prefix, &el.local)
        } else if el.children.is_empty() {
            formats_xml::OutElement::leaf(&el.prefix, &el.local, &el.text)
        } else {
            formats_xml::OutElement::branch(&el.prefix, &el.local)
        };
        for a in &el.attrs {
            result = result.attr(&a.name, &a.value);
        }
        for child in &el.children {
            result.push(output(child));
        }
        result
    }
    formats_xml::emit::render(
        &formats_xml::Envelope {
            bom: true,
            eol: "\r\n",
            indent_unit: "\t",
            decl: "<?xml version=\"1.0\" encoding=\"UTF-8\"?>",
            trailing_eol: true,
            escape_gt: true,
            escape_quot: false,
            text_eol: "\n",
        },
        &output(&root),
    )
}

fn collect_xml_paths(
    a: &formats_xml::descriptor::Element,
    b: &formats_xml::descriptor::Element,
    path: &str,
    out: &mut Vec<String>,
) {
    if a.prefix != b.prefix || a.local != b.local {
        out.push(format!("{path}/QName"));
    }
    if a.text != b.text {
        out.push(format!("{path}/text"));
    }
    let aa: Vec<_> = a.attrs.iter().map(|x| (&x.name, &x.value)).collect();
    let bb: Vec<_> = b.attrs.iter().map(|x| (&x.name, &x.value)).collect();
    if aa != bb {
        out.push(format!("{path}/attributes"));
    }
    if a.children.len() != b.children.len() {
        out.push(format!(
            "{path}/child-count {}->{}; missing {:?}; extra {:?}",
            a.children.len(),
            b.children.len(),
            a.children
                .iter()
                .filter(|x| !b.children.iter().any(|y| y.local == x.local))
                .map(|x| &x.local)
                .collect::<Vec<_>>(),
            b.children
                .iter()
                .filter(|x| !a.children.iter().any(|y| y.local == x.local))
                .map(|x| &x.local)
                .collect::<Vec<_>>()
        ));
    }
    let an: Vec<_> = a.children.iter().map(|x| (&x.prefix, &x.local)).collect();
    let bn: Vec<_> = b.children.iter().map(|x| (&x.prefix, &x.local)).collect();
    if an != bn {
        out.push(format!("{path}/ordered-QNames"));
    }
    let mut seen = std::collections::HashMap::new();
    for child in &a.children {
        let nth = seen.entry((&child.prefix, &child.local)).or_insert(0usize);
        if let Some(other) = b
            .children
            .iter()
            .filter(|x| x.prefix == child.prefix && x.local == child.local)
            .nth(*nth)
        {
            collect_xml_paths(
                child,
                other,
                &format!("{path}/{}[{}]", child.local, *nth),
                out,
            );
        }
        *nth += 1;
    }
}

#[test]
fn known_gantt_collections_and_item_current_values_are_not_constants() {
    let chart=read_chart_sidecar(&source("<realExSeriesData><properties><id>4</id><valInfo xsi:type=\"core:StringValue\"><value>Info</value></valInfo><key xsi:type=\"core:NumberValue\"><value>7.5</value></key></properties></realExSeriesData><trendLinesArray><seriesId>4</seriesId><line><approximationType>Polynomial</approximationType><approximationDegree>4</approximationDegree><showEquation>true</showEquation></line></trendLinesArray>")).unwrap();
    let native = write_form(FormDialect::Designer, &form_with_chart(chart.clone())).unwrap();
    let reread = read_form(FormDialect::Designer, &native)
        .unwrap()
        .data_attributes[0]
        .chart_settings
        .clone()
        .unwrap();
    assert_eq!(reread, chart);
    let bytes=b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<ganttchart:GanttChart xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:core=\"http://g5.1c.ru/v8/dt/mcore\" xmlns:ganttchart=\"http://g5.1c.ru/v8/dt/ganttchart/model\"><interval><itemKey>4</itemKey><begin>2026-10-02T00:00:00</begin><end>2026-10-03T00:00:00</end></interval><value><itemKey>4</itemKey><editFlag>true</editFlag></value><link><beginKey>4</beginKey><endKey>7</endKey><linkType>EndBegin</linkType></link></ganttchart:GanttChart>\r\n";
    let mut chart = read_chart_sidecar(bytes).unwrap();
    let native = write_form(FormDialect::Designer, &form_with_chart(chart.clone())).unwrap();
    let reread = read_form(FormDialect::Designer, &native)
        .unwrap()
        .data_attributes[0]
        .chart_settings
        .clone()
        .unwrap();
    assert_eq!(reread, chart);
    let before = serde_json::to_vec(&chart).unwrap();
    let ChartValue::Items(items) = &mut chart
        .fields
        .iter_mut()
        .find(|(n, _)| n == "interval")
        .unwrap()
        .1
    else {
        panic!()
    };
    items[0].iter_mut().find(|(n, _)| n == "itemKey").unwrap().1 = ChartValue::Int("18".into());
    assert_ne!(serde_json::to_vec(&chart).unwrap(), before);
    let written = write_chart_sidecar(&chart).unwrap();
    assert_eq!(read_chart_sidecar(&written).unwrap(), chart);
}

#[test]
fn line_style_and_eint_width_are_validated_on_read_and_current_write() {
    for line in [
        "<style>Unknown</style><width>1</width>",
        "<style>Solid</style><width>abc</width>",
        "<style>Solid</style><width>2147483648</width>",
    ] {
        assert!(read_chart_sidecar(&source(&format!("<scaleLine>{line}</scaleLine>"))).is_err());
    }
    let mut chart = read_chart_sidecar(&source(
        "<scaleLine><style>DashDotted</style><width>4</width><gap>true</gap></scaleLine>",
    ))
    .unwrap();
    let item = &mut chart
        .fields
        .iter_mut()
        .find(|(n, _)| n == "scaleLine")
        .unwrap()
        .1;
    let ChartValue::Line { style, .. } = item else {
        panic!()
    };
    *style = "Unknown".into();
    assert!(write_chart_sidecar(&chart).is_err());
    let ChartValue::Line { style, width, .. } = &mut chart
        .fields
        .iter_mut()
        .find(|(n, _)| n == "scaleLine")
        .unwrap()
        .1
    else {
        panic!()
    };
    *style = "Solid".into();
    *width = "-2147483648".into();
    assert!(write_chart_sidecar(&chart).is_ok());
    let ChartValue::Line { width, .. } = &mut chart
        .fields
        .iter_mut()
        .find(|(n, _)| n == "scaleLine")
        .unwrap()
        .1
    else {
        panic!()
    };
    *width = "2147483648".into();
    assert!(write_form(FormDialect::Designer, &form_with_chart(chart)).is_err());
}

#[test]
fn current_palette_colors_preserve_order_and_edits() {
    let mut chart=read_chart_sidecar(&source("<customPalette xsi:type=\"core:ColorRef\"><color>Windows.Highlight</color></customPalette><customPalette xsi:type=\"core:ColorDef\"><red>17</red><green>34</green><blue>51</blue></customPalette>")).unwrap();
    let native = write_form(FormDialect::Designer, &form_with_chart(chart.clone())).unwrap();
    let text = std::str::from_utf8(&native).unwrap();
    assert!(text.contains("xsi:type=\"v8ui:Color\""));
    assert!(text.contains("win:Highlight"));
    let read = read_form(FormDialect::Designer, &native).unwrap();
    assert_eq!(
        read.data_attributes[0].chart_settings.as_ref(),
        Some(&chart)
    );
    let ChartValue::Items(items) = &mut chart
        .fields
        .iter_mut()
        .find(|(n, _)| n == "customPalette")
        .unwrap()
        .1
    else {
        panic!()
    };
    items.reverse();
    items[0][0].1 = ChartValue::Color("#445566".into());
    let native = write_form(FormDialect::Designer, &form_with_chart(chart.clone())).unwrap();
    let read = read_form(FormDialect::Designer, &native).unwrap();
    assert_eq!(
        read.data_attributes[0].chart_settings.as_ref(),
        Some(&chart)
    );
    let bytes = write_chart_sidecar(&chart).unwrap();
    assert_eq!(read_chart_sidecar(&bytes).unwrap(), chart);
    for child in [
        "<customPalette xsi:type=\"core:UnknownColor\"/>",
        "<customPalette xsi:type=\"core:ColorRef\"><color>Windows.</color></customPalette>",
    ] {
        assert!(read_chart_sidecar(&source(child)).is_err());
    }
}

#[test]
fn current_nonnull_gantt_picture_reference_is_typed_and_editable() {
    let bytes=b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<ganttchart:GanttChart xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:core=\"http://g5.1c.ru/v8/dt/mcore\" xmlns:ganttchart=\"http://g5.1c.ru/v8/dt/ganttchart/model\"><points><value><picture xsi:type=\"core:PictureRef\"><picture>StdPicture.Save</picture></picture></value></points></ganttchart:GanttChart>\r\n";
    let mut chart = read_chart_sidecar(bytes).unwrap();
    let native = write_form(FormDialect::Designer, &form_with_chart(chart.clone())).unwrap();
    assert!(
        std::str::from_utf8(&native)
            .unwrap()
            .contains("<xr:Ref>StdPicture.Save</xr:Ref>")
    );
    assert_eq!(
        read_form(FormDialect::Designer, &native)
            .unwrap()
            .data_attributes[0]
            .chart_settings
            .as_ref(),
        Some(&chart)
    );
    let ChartValue::Nested(points) = &mut chart
        .fields
        .iter_mut()
        .find(|(n, _)| n == "points")
        .unwrap()
        .1
    else {
        panic!()
    };
    let ChartValue::Nested(value) = &mut points.iter_mut().find(|(n, _)| n == "value").unwrap().1
    else {
        panic!()
    };
    let ChartValue::Picture(picture) =
        &mut value.iter_mut().find(|(n, _)| n == "picture").unwrap().1
    else {
        panic!()
    };
    let morph1c_core::ir::form::ChartPicture::Reference { reference, .. } = picture.as_mut() else {
        panic!()
    };
    *reference = "StdPicture.Open".into();
    let bytes = write_chart_sidecar(&chart).unwrap();
    assert!(
        std::str::from_utf8(&bytes)
            .unwrap()
            .contains("StdPicture.Open")
    );
    assert_eq!(read_chart_sidecar(&bytes).unwrap(), chart);
}

#[test]
fn native_trend_projection_keeps_current_lines_and_rejects_untransported_topology() {
    let source = source(
        "<realSeriesData><properties><id>2</id></properties></realSeriesData><realSeriesData><properties><id>1</id></properties></realSeriesData><trendLinesArray><seriesId>1</seriesId><line><approximationDegree>3</approximationDegree></line></trendLinesArray><trendLinesArray><seriesId>2</seriesId><line><approximationDegree>4</approximationDegree></line></trendLinesArray><trendLinesArray><seriesId>2</seriesId><line><approximationDegree>5</approximationDegree></line></trendLinesArray><trendLinesArray><seriesId>99</seriesId><line><approximationDegree>6</approximationDegree></line></trendLinesArray>",
    );
    let mut chart = read_chart_sidecar(&source).unwrap();
    let original = chart.clone();
    let projected = formats_xml::form::project_native_trends(&chart).unwrap();
    assert_ne!(projected, chart);
    assert_eq!(chart, original);
    assert!(write_form(FormDialect::Designer, &form_with_chart(chart.clone())).is_err());
    let native = write_form(FormDialect::Designer, &form_with_chart(projected.clone())).unwrap();
    let returned = read_form(FormDialect::Designer, &native)
        .unwrap()
        .data_attributes[0]
        .chart_settings
        .clone()
        .unwrap();
    assert_eq!(returned, projected);
    let ChartValue::Items(arrays) = &mut chart
        .fields
        .iter_mut()
        .find(|(n, _)| n == "trendLinesArray")
        .unwrap()
        .1
    else {
        panic!()
    };
    let ChartValue::Items(lines) = &mut arrays[1].iter_mut().find(|(n, _)| n == "line").unwrap().1
    else {
        panic!()
    };
    lines[0]
        .iter_mut()
        .find(|(n, _)| n == "approximationDegree")
        .unwrap()
        .1 = ChartValue::Int("7".into());
    assert_ne!(
        formats_xml::form::project_native_trends(&chart).unwrap(),
        projected
    );
    arrays_no_old_values(&native);
}
fn arrays_no_old_values(native: &[u8]) {
    let xml = std::str::from_utf8(native).unwrap();
    assert!(xml.contains("<d4p1:approximationDegree>4</d4p1:approximationDegree>"));
    assert!(!xml.contains("<d4p1:approximationDegree>5</d4p1:approximationDegree>"));
    assert!(!xml.contains("<d4p1:approximationDegree>6</d4p1:approximationDegree>"));
}

#[test]
fn explicitly_set_zero_fields_keep_sdk_presence_without_old_values() {
    let mut chart = read_chart_sidecar(&source("<startAnglePieChart>0</startAnglePieChart><finishAnglePieChart>0</finishAnglePieChart><isometricDepth>0</isometricDepth>")).unwrap();
    let native = write_form(FormDialect::Designer, &form_with_chart(chart.clone())).unwrap();
    let text = std::str::from_utf8(&native).unwrap();
    for name in [
        "startAnglePieChart",
        "finishAnglePieChart",
        "isometricDepth",
    ] {
        assert!(text.contains(&format!("<d4p1:{name}>0</d4p1:{name}>")));
    }
    chart
        .fields
        .iter_mut()
        .find(|(n, _)| n == "isometricDepth")
        .unwrap()
        .1 = ChartValue::Int("8".into());
    let edited = write_form(FormDialect::Designer, &form_with_chart(chart)).unwrap();
    assert!(
        std::str::from_utf8(&edited)
            .unwrap()
            .contains("<d4p1:isometricDepth>8</d4p1:isometricDepth>")
    );
    let absent = read_chart_sidecar(&source("")).unwrap();
    let output = write_form(FormDialect::Designer, &form_with_chart(absent)).unwrap();
    assert!(
        !std::str::from_utf8(&output)
            .unwrap()
            .contains("<d4p1:isometricDepth>")
    );
}

#[test]
fn root_common_namespace_is_used_bound_lexical_state_not_model_values() {
    let compact = source(
        r#"<realExSeriesData><properties><valInfo xsi:type="common:ChartLineTypeValue" xmlns:common="http://g5.1c.ru/v8/dt/metadata/common"><value>Solid</value></valInfo></properties></realExSeriesData>"#,
    );
    let inline = write_chart_sidecar(&read_chart_sidecar(&compact).unwrap()).unwrap();
    let input=String::from_utf8(inline.clone()).unwrap().replace(r#" xmlns:common="http://g5.1c.ru/v8/dt/metadata/common""#, "").replace(r#"xmlns:core="http://g5.1c.ru/v8/dt/mcore""#, r#"xmlns:core="http://g5.1c.ru/v8/dt/mcore" xmlns:common="http://g5.1c.ru/v8/dt/metadata/common""#).into_bytes();
    let chart = read_chart_sidecar(&input).unwrap();
    assert!(write_chart_sidecar(&chart).unwrap() == input);
    let wrong = String::from_utf8(input.clone())
        .unwrap()
        .replace("http://g5.1c.ru/v8/dt/metadata/common", "urn:wrong");
    assert!(read_chart_sidecar(wrong.as_bytes()).is_err());
    let unknown = String::from_utf8(input)
        .unwrap()
        .replace("<chart:Chart ", r#"<chart:Chart unexpected="true" "#);
    assert!(read_chart_sidecar(unknown.as_bytes()).is_err());
    let decoded = read_chart_sidecar(&inline).unwrap();
    assert_eq!(
        serde_json::to_value(&decoded).unwrap(),
        serde_json::to_value(&chart).unwrap()
    );
    assert!(write_chart_sidecar(&decoded).unwrap() == inline);
    let mut edited = chart;
    let ChartValue::Nested(series) = &mut edited
        .fields
        .iter_mut()
        .find(|(n, _)| n == "realExSeriesData")
        .unwrap()
        .1
    else {
        panic!()
    };
    series.iter_mut().find(|(n, _)| n == "valInfo").unwrap().1 =
        ChartValue::Value(Box::new(morph1c_core::ir::form::ChartTypedValue::Null));
    let output = write_chart_sidecar(&edited).unwrap();
    assert!(
        !std::str::from_utf8(&output)
            .unwrap()
            .contains("xmlns:common")
    );
    assert!(!std::str::from_utf8(&output).unwrap().contains("Solid"));
    assert_eq!(read_chart_sidecar(&output).unwrap(), edited);
}

#[test]
fn current_background_colors_are_ordered_native_repeated_colors() {
    let gantt = source(
        r#"<backIntervals><contentCacheItem xsi:type="core:ColorRef"><color>Windows.Highlight</color></contentCacheItem><contentCacheItem xsi:type="core:ColorDef"><red>10</red><green>20</green><blue>30</blue></contentCacheItem></backIntervals>"#,
    );
    let gantt = String::from_utf8(gantt)
        .unwrap()
        .replace("chart:Chart", "ganttchart:GanttChart")
        .replace(
            "xmlns:chart=\"http://g5.1c.ru/v8/dt/chart/model\"",
            "xmlns:ganttchart=\"http://g5.1c.ru/v8/dt/ganttchart/model\"",
        );
    let chart = read_chart_sidecar(gantt.as_bytes()).unwrap();
    let native = write_form(FormDialect::Designer, &form_with_chart(chart.clone())).unwrap();
    let text = std::str::from_utf8(&native).unwrap();
    assert!(text.contains("<d4p1:contentCacheItem>win:Highlight</d4p1:contentCacheItem>"));
    assert!(text.contains("<d4p1:contentCacheItem>#0A141E</d4p1:contentCacheItem>"));
    let returned = read_form(FormDialect::Designer, &native)
        .unwrap()
        .data_attributes[0]
        .chart_settings
        .clone()
        .unwrap();
    assert_eq!(returned, chart);
    let mut edited = chart;
    let ChartValue::Nested(back) = &mut edited
        .fields
        .iter_mut()
        .find(|(n, _)| n == "backIntervals")
        .unwrap()
        .1
    else {
        panic!()
    };
    let ChartValue::Items(colors) = &mut back
        .iter_mut()
        .find(|(n, _)| n == "contentCacheItem")
        .unwrap()
        .1
    else {
        panic!()
    };
    colors.reverse();
    colors[0][0].1 = ChartValue::Color("#010203".into());
    let current = write_form(FormDialect::Designer, &form_with_chart(edited.clone())).unwrap();
    let returned = read_form(FormDialect::Designer, &current)
        .unwrap()
        .data_attributes[0]
        .chart_settings
        .clone()
        .unwrap();
    assert_eq!(returned, edited);
}

fn current_definition(
    chart: &mut morph1c_core::ir::form::ChartSettings,
) -> &mut morph1c_core::ir::form::ChartPicture {
    let ChartValue::Nested(points) = &mut chart
        .fields
        .iter_mut()
        .find(|(n, _)| n == "points")
        .unwrap()
        .1
    else {
        panic!()
    };
    let ChartValue::Nested(value) = &mut points.iter_mut().find(|(n, _)| n == "value").unwrap().1
    else {
        panic!()
    };
    let ChartValue::Picture(picture) =
        &mut value.iter_mut().find(|(n, _)| n == "picture").unwrap().1
    else {
        panic!()
    };
    picture
}
#[test]
fn current_definition_declared_path_pixel_and_native_presence_are_distinct() {
    let bytes=b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<ganttchart:GanttChart xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:core=\"http://g5.1c.ru/v8/dt/mcore\" xmlns:ganttchart=\"http://g5.1c.ru/v8/dt/ganttchart/model\"><points><value><picture xsi:type=\"core:PictureDef\"><transparentPixel x=\"-1\" y=\"-1\"/></picture></value></points></ganttchart:GanttChart>\r\n";
    let mut chart = read_chart_sidecar(bytes).unwrap();
    let morph1c_core::ir::form::ChartPicture::Definition { file_name, .. } =
        current_definition(&mut chart)
    else {
        panic!()
    };
    *file_name = "ChartBitmap.png".into();
    let native = write_form(FormDialect::Designer, &form_with_chart(chart)).unwrap();
    let text = String::from_utf8(native.clone()).unwrap();
    assert!(!text.contains("xr:TransparentPixel"));
    assert!(text.contains("<xr:Abs>ChartBitmap.png</xr:Abs>"));
    let original = read_form(FormDialect::Designer, &native).unwrap();
    assert!(write_form(FormDialect::Designer, &original).unwrap() == native);
    let mut explicit = text.clone();
    let line = text
        .lines()
        .find(|line| line.contains("<xr:LoadTransparent>true"))
        .unwrap();
    let indent = line.split('<').next().unwrap();
    explicit = explicit.replace(
        line,
        &format!("{line}\r\n{indent}<xr:TransparentPixel x=\"-1\" y=\"-1\"/>"),
    );
    let explicit = explicit.into_bytes();
    let source = read_form(FormDialect::Designer, &explicit).unwrap();
    assert_eq!(
        serde_json::to_value(&source).unwrap(),
        serde_json::to_value(&original).unwrap()
    );
    assert!(write_form(FormDialect::Designer, &source).unwrap() == explicit);
    let mut edited = source;
    let chart = edited.data_attributes[0].chart_settings.as_mut().unwrap();
    let morph1c_core::ir::form::ChartPicture::Definition {
        file_name,
        transparent_pixel,
        ..
    } = current_definition(chart)
    else {
        panic!()
    };
    *file_name = "CurrentBitmap.png".into();
    *transparent_pixel = None;
    let current = write_form(FormDialect::Designer, &edited).unwrap();
    assert!(
        !std::str::from_utf8(&current)
            .unwrap()
            .contains("TransparentPixel")
    );
    assert!(
        std::str::from_utf8(&current)
            .unwrap()
            .contains("CurrentBitmap.png")
    );
    let chart = edited.data_attributes[0].chart_settings.as_mut().unwrap();
    let morph1c_core::ir::form::ChartPicture::Definition { glyph, .. } = current_definition(chart)
    else {
        panic!()
    };
    *glyph = Some((1, 2));
    assert!(write_form(FormDialect::Designer, &edited).is_err());
}

#[test]
fn percentage_integer_projection_is_low32_and_fractional_branch_is_decimal_exact() {
    for (input, expected, equal) in [
        ("0", "0", true),
        ("2147483648", "-2147483648", false),
        ("-2147483649", "2147483647", false),
        ("4294967296", "0", false),
        ("9007199254740993", "1", false),
        ("1E+32", "0", false),
        ("1000.00", "1000", true),
    ] {
        assert_eq!(
            formats_xml::form::native_percentage_integer(input).unwrap(),
            Some((expected.into(), equal))
        );
    }
    for input in ["0.1", "100000000000000000000000.1", "1E-9999"] {
        assert!(
            formats_xml::form::native_percentage_integer(input)
                .unwrap()
                .is_none()
        );
    }
    assert!(formats_xml::form::percentage_numeric_equal("1.000E+3", "1000").unwrap());
    assert!(
        !formats_xml::form::percentage_numeric_equal(
            "0.1234567890123456789012345",
            "0.12345678901234568"
        )
        .unwrap()
    );
    for input in ["abc", "1E", "--1", "1..2", "1E2147483648"] {
        assert!(formats_xml::form::native_percentage_integer(input).is_err());
    }
}

#[test]
fn percentage_fractional_java17_digits_and_exact_loss_contract() {
    for (input, expected, equal) in [
        ("3.5", "3.5", true),
        ("0.1", "0.1", true),
        ("0.1234567890123456789012345", "0.12345678901234568", false),
        ("100000000000000000000000.1", "1.0000000000000001E23", false),
        ("0.0001", "1.0E-4", true),
        ("10000000.1", "1.00000001E7", true),
        ("1E-9999", "0.0", false),
        ("-1E-9999", "-0.0", false),
        ("-0.00000001", "-1.0E-8", true),
        (
            "0.00000000000000000000000000000000000000000000000000000000000000001",
            "1.0E-65",
            true,
        ),
        (
            "1.00000000000000011102230246251565404236316680908203125",
            "1.0",
            false,
        ),
        (
            "1.00000000000000011102230246251565404236316680908203126",
            "1.0000000000000002",
            false,
        ),
    ] {
        assert_eq!(
            formats_xml::form::native_percentage_projection(input).unwrap(),
            (expected.into(), equal),
            "{input}"
        );
    }
    let first = formats_xml::form::native_percentage_projection("100000000000000000000000.1")
        .unwrap()
        .0;
    assert_ne!(
        formats_xml::form::native_percentage_projection(&first)
            .unwrap()
            .0,
        first,
        "projection must not be applied twice"
    );
}

#[test]
#[ignore = "bound public Java17 BigDecimal projection corpus, not configuration acceptance"]
fn percentage_original_java17_public_corpus() {
    let path = std::env::var("IBCMD_CHART_PERCENT_CORPUS").expect("bound public TSV corpus");
    let data = std::fs::read_to_string(path).unwrap();
    let mut count = 0;
    for (index, line) in data.lines().enumerate() {
        let cells: Vec<_> = line.split('\t').collect();
        assert_eq!(cells.len(), 3);
        let expected = (cells[1].to_owned(), cells[2] == "true");
        assert_eq!(
            formats_xml::form::native_percentage_projection(cells[0]).unwrap(),
            expected,
            "public row {index}"
        );
        count += 1;
    }
    assert!(count > 1000);
}

#[test]
fn native_percentage_nonfinite_default_spelling_is_source_only_and_never_replays_a_number() {
    use morph1c_core::ir::form::ChartTypedValue;
    use morph1c_core::ir::value::{PropertyValue, ValueScalarKind, ValueSpec};
    let chart=read_chart_sidecar(&source("<valuesReferenceLines><chartReferenceLine><semitransparencyPercent xsi:type=\"core:NumberValue\"><value>5</value></semitransparencyPercent></chartReferenceLine></valuesReferenceLines>")).unwrap();
    let native = write_form(FormDialect::Designer, &form_with_chart(chart)).unwrap();
    let text = std::str::from_utf8(&native).unwrap();
    for spelling in ["Infinity", "-Infinity"] {
        let current = text.replace(
            ">5</d4p1:semitransparencyPercent>",
            &format!(">{spelling}</d4p1:semitransparencyPercent>"),
        );
        let mut body = read_form(FormDialect::Designer, current.as_bytes()).unwrap();
        assert_eq!(
            write_form(FormDialect::Designer, &body).unwrap(),
            current.as_bytes()
        );
        let chart = body.data_attributes[0].chart_settings.as_mut().unwrap();
        let ChartValue::Nested(lines) = &mut chart
            .fields
            .iter_mut()
            .find(|(n, _)| n == "valuesReferenceLines")
            .unwrap()
            .1
        else {
            panic!()
        };
        let ChartValue::Items(items) = &mut lines
            .iter_mut()
            .find(|(n, _)| n == "chartReferenceLine")
            .unwrap()
            .1
        else {
            panic!()
        };
        items[0]
            .iter_mut()
            .find(|(n, _)| n == "semitransparencyPercent")
            .unwrap()
            .1 = ChartValue::Value(Box::new(ChartTypedValue::Scalar(PropertyValue::Value(
            ValueSpec {
                kind: ValueScalarKind::Number,
                scalar: Some(Box::new(PropertyValue::Str("13".into()))),
            },
        ))));
        let output = write_form(FormDialect::Designer, &body).unwrap();
        assert!(!std::str::from_utf8(&output).unwrap().contains("Infinity"));
        assert!(
            std::str::from_utf8(&output)
                .unwrap()
                .contains(">13</d4p1:semitransparencyPercent>")
        );
        let chart = body.data_attributes[0].chart_settings.as_mut().unwrap();
        let ChartValue::Nested(lines) = &mut chart
            .fields
            .iter_mut()
            .find(|(n, _)| n == "valuesReferenceLines")
            .unwrap()
            .1
        else {
            panic!()
        };
        let ChartValue::Items(items) = &mut lines
            .iter_mut()
            .find(|(n, _)| n == "chartReferenceLine")
            .unwrap()
            .1
        else {
            panic!()
        };
        items[0].retain(|(n, _)| n != "semitransparencyPercent");
        assert!(
            !std::str::from_utf8(&write_form(FormDialect::Designer, &body).unwrap())
                .unwrap()
                .contains("Infinity")
        );
        assert!(
            read_form(
                FormDialect::Designer,
                current.replace(spelling, "not-a-number").as_bytes()
            )
            .is_err()
        );
    }
}

#[test]
fn big_decimal_unicode_digits_follow_original_java_char_domain_and_scale() {
    use formats_xml::form::{native_percentage_projection, normalize_big_decimal};
    for (input, ascii) in [
        ("١.٥", "1.5"),
        ("１.２E+０２", "1.2E+02"),
        ("1.٥", "1.5"),
        ("-.٥", "-.5"),
        ("+١.", "+1."),
    ] {
        assert_eq!(normalize_big_decimal(input).as_deref(), Some(ascii));
        assert_eq!(
            native_percentage_projection(input).unwrap(),
            native_percentage_projection(ascii).unwrap()
        );
    }
    for input in [
        "²",
        "−1",
        "𝟙",
        "١٫٥",
        "1E",
        "1E++2",
        "1E2147483648",
        "1E-2147483649",
        "1E-2147483648",
        ".1E-2147483647",
        "1.2E-2147483647",
    ] {
        assert!(
            normalize_big_decimal(input).is_none(),
            "invalid model number: {input}"
        );
        assert!(native_percentage_projection(input).is_err());
    }
    for input in [
        "1E2147483647",
        "1E-2147483647",
        ".1E2147483647",
        "1.2E2147483647",
    ] {
        assert_eq!(normalize_big_decimal(input).as_deref(), Some(input));
    }
}

#[test]
fn typed_unicode_number_reads_ascii_and_current_number_edits_remain_authoritative() {
    use morph1c_core::ir::{
        form::ChartTypedValue,
        value::{PropertyValue, ValueScalarKind, ValueSpec},
    };
    let content = |number: &str| {
        format!(
            "<realSeriesData><properties><key xsi:type=\"core:NumberValue\"><value>{number}</value></key></properties></realSeriesData>"
        )
    };
    for number in ["١.٥", "１.２E+０２", "1.٥"] {
        let ascii = formats_xml::form::normalize_big_decimal(number).unwrap();
        let mut chart = read_chart_sidecar(&source(&content(number))).unwrap();
        let expected = read_chart_sidecar(&source(&content(&ascii))).unwrap();
        assert_eq!(chart, expected);
        let native = write_form(FormDialect::Designer, &form_with_chart(chart.clone())).unwrap();
        assert_eq!(
            read_form(FormDialect::Designer, &native)
                .unwrap()
                .data_attributes[0]
                .chart_settings
                .as_ref(),
            Some(&expected)
        );
        let ChartValue::Items(items) = &mut chart
            .fields
            .iter_mut()
            .find(|(name, _)| name == "realSeriesData")
            .unwrap()
            .1
        else {
            panic!("current series");
        };
        let properties = &mut items[0];
        properties
            .iter_mut()
            .find(|(name, _)| name == "key")
            .unwrap()
            .1 = ChartValue::Value(Box::new(ChartTypedValue::Scalar(PropertyValue::Value(
            ValueSpec {
                kind: ValueScalarKind::Number,
                scalar: Some(Box::new(PropertyValue::Str("٢.٧٥".into()))),
            },
        ))));
        let output = write_chart_sidecar(&chart).unwrap();
        assert!(std::str::from_utf8(&output).unwrap().contains(">2.75<"));
        assert_eq!(
            read_chart_sidecar(&output).unwrap(),
            read_chart_sidecar(&source(&content("2.75"))).unwrap()
        );
    }
    for number in ["²", "−1", "𝟙", "1.2E-2147483647"] {
        assert!(read_chart_sidecar(&source(&content(number))).is_err());
    }
}

#[test]
#[ignore = "bound complete original Java17 Character.digit(char,10) BMP enumeration"]
fn big_decimal_original_java17_complete_bmp_digit_domain() {
    use sha2::{Digest, Sha256};
    let bytes =
        std::fs::read("F:/ibcmd/lab/07/chart-unicode-bigdecimal-primary-r2/ranges.json").unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        "24976154cc2f0c4c3b5f65325c736c56f3d4b3ff92229b1491d46b8f31577c09"
    );
    let ranges: Vec<[u32; 2]> = serde_json::from_slice(&bytes).unwrap();
    let mut accepted = 0;
    for code in 0..=0xffff {
        let Some(character) = char::from_u32(code) else {
            continue;
        };
        let expected = ranges.iter().find_map(|[first, last]| {
            (code >= *first && code <= *last).then(|| (code - first).to_string())
        });
        let actual = formats_xml::form::normalize_big_decimal(&character.to_string());
        assert_eq!(actual, expected, "Java UTF16 digit U+{code:04X}");
        accepted += usize::from(actual.is_some());
    }
    assert_eq!(accepted, 370);
}

#[test]
fn public_unicode_number_source_consumption_is_typed_in_both_lanes() {
    use ibcmd_edt::{
        ConversionOptions, Project, ReaderLimits, edt_to_xml, read_directory_project,
        read_directory_source, read_xml_source, xml_to_edt,
    };
    use ibcmd_xml::source_tree::{SourceEntry, SourceTree};
    use morph1c_core::{
        ir::{MetadataObject, NamedFormBody, ObjectKind, PropertyValue, Token, Uuid},
        version::{FormatVersion, with_roundtrip_target},
    };
    use morph1c_pipeline::{ConvertOptions, Format, read_config, write_config};
    for minor in [20, 21] {
        let fixture = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/subsystem-ci/src"
        ));
        let mut config = read_config(Format::Designer, fixture, &ConvertOptions::default())
            .unwrap()
            .0;
        let chart=read_chart_sidecar(&source("<realSeriesData><properties><key xsi:type=\"core:NumberValue\"><value>1.5</value></key></properties></realSeriesData>")).unwrap();
        let mut object = MetadataObject::new(
            ObjectKind::new("CommonForm"),
            "UnicodeNumber",
            Uuid([57; 16]),
        );
        let id = morph1c_core::spec::registry::spec_for("CommonForm")
            .unwrap()
            .fields()
            .iter()
            .find(|f| f.name == "formType")
            .unwrap()
            .id;
        object
            .properties
            .push((id, PropertyValue::Enum(Token::new("Managed"))));
        object.form_bodies.push(NamedFormBody {
            name: "UnicodeNumber".into(),
            body: form_with_chart(chart),
            ordinary_body: None,
            module: None,
            help: vec![],
            help_resources: vec![],
        });
        config.objects.push(object);
        let original_dir = tempfile::tempdir().unwrap();
        with_roundtrip_target(FormatVersion::new(2, minor), || {
            write_config(Format::Designer, &config, original_dir.path())
        })
        .unwrap();
        let config_path = original_dir.path().join("Configuration.xml");
        let descriptor = std::fs::read_to_string(&config_path).unwrap().replace(
            "</Language>",
            "</Language>\r\n\t\t\t<CommonForm>UnicodeNumber</CommonForm>",
        );
        std::fs::write(config_path, descriptor).unwrap();
        let options = ConversionOptions {
            edt_version: "2025.2.3".into(),
            xml_dialect: format!("2.{minor}"),
            runtime_version: Some(if minor == 20 { "8.3.27" } else { "8.5.1" }.into()),
        };
        let canonical_native =
            read_xml_source(original_dir.path(), ReaderLimits::default()).unwrap();
        let form_path = original_dir
            .path()
            .join("CommonForms/UnicodeNumber/Ext/Form.xml");
        let native = std::fs::read_to_string(&form_path).unwrap();
        assert!(native.contains(">1.5<"));
        std::fs::write(&form_path, native.replace(">1.5<", ">١.٥<")).unwrap();
        let original = read_xml_source(original_dir.path(), ReaderLimits::default()).unwrap();
        let generated = xml_to_edt(&original, &options).unwrap().tree;
        assert_eq!(
            edt_to_xml(&Project::from_tree(generated.clone()).unwrap(), &options)
                .unwrap()
                .tree,
            original
        );
        let native_project = tempfile::tempdir().unwrap();
        let project_path = native_project.path().join("project");
        read_directory_source(original_dir.path())
            .unwrap()
            .xml_to_edt(&options)
            .unwrap()
            .publish_new(&project_path)
            .unwrap();
        let native_return = native_project.path().join("returned");
        read_directory_project(&project_path)
            .unwrap()
            .edt_to_xml(&options)
            .unwrap()
            .publish_new(&native_return)
            .unwrap();
        assert_eq!(
            read_xml_source(&native_return, ReaderLimits::default()).unwrap(),
            original
        );

        let sidecar = "src/CommonForms/UnicodeNumber/Attributes/Diagram/ExtInfo/Chart.chart";
        let mut changed = false;
        let current = SourceTree::new(
            generated
                .entries()
                .iter()
                .filter(|e| !e.path().as_str().starts_with(".ibcmd-provenance/"))
                .map(|e| {
                    let bytes = if e.path().as_str() == sidecar {
                        let text = String::from_utf8(e.bytes().to_vec()).unwrap();
                        assert!(text.contains(">1.5<"));
                        changed = true;
                        text.replace(">1.5<", ">١.٥<").into_bytes()
                    } else {
                        e.bytes().to_vec()
                    };
                    SourceEntry::from_bytes(e.path().clone(), bytes).unwrap()
                })
                .collect(),
        )
        .unwrap();
        assert!(changed);
        let returned = edt_to_xml(&Project::from_tree(current.clone()).unwrap(), &options).unwrap();
        assert_eq!(returned.tree, canonical_native);
        let directory = tempfile::tempdir().unwrap();
        for e in current.entries() {
            let path = directory.path().join(e.path().as_str());
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, e.bytes()).unwrap();
        }
        let converted = read_directory_project(directory.path())
            .unwrap()
            .edt_to_xml(&options)
            .unwrap();
        let output = directory.path().join("returned");
        converted.publish_new(&output).unwrap();
        assert_eq!(
            read_xml_source(&output, ReaderLimits::default()).unwrap(),
            canonical_native
        );
    }
}
