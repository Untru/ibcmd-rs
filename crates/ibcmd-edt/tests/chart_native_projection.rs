use formats_xml::form::{
    FormDialect, apply_chart_semantics_resource, project_chart_semantics, read_chart_sidecar,
    read_form, write_form,
};
use morph1c_core::ir::form::ChartValue;
use morph1c_core::ir::{FormBody, Uuid};
fn chart(content: &str) -> FormBody {
    let fixture = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<form:Form xmlns:form=\"http://g5.1c.ru/v8/dt/form\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"><attributes><name>Diagram</name><id>1</id><valueType><types>Chart</types></valueType><view><common>true</common></view><edit><common>true</common></edit><extInfo xsi:type=\"form:ChartExtInfo\"/></attributes></form:Form>\r\n";
    let mut body = read_form(FormDialect::Edt, fixture.as_bytes()).unwrap();
    let source = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<chart:Chart xmlns:chart=\"http://g5.1c.ru/v8/dt/chart/model\" xmlns:core=\"http://g5.1c.ru/v8/dt/mcore\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">{content}</chart:Chart>\r\n"
    );
    body.data_attributes[0].chart_settings = Some(read_chart_sidecar(source.as_bytes()).unwrap());
    body
}
fn roundtrip(body: &FormBody) -> (FormBody, Vec<u8>, Vec<u8>) {
    let uuid = Uuid([1; 16]);
    let projection = project_chart_semantics(body, uuid).unwrap();
    let (projected, resource) = projection.unwrap_or_else(|| (body.clone(), Vec::new()));
    let native = write_form(FormDialect::Designer, &projected).unwrap();
    let mut decoded = read_form(FormDialect::Designer, &native).unwrap();
    if !resource.is_empty() {
        apply_chart_semantics_resource(&mut decoded, uuid, &resource).unwrap();
    }
    assert_eq!(
        decoded.data_attributes[0].chart_settings,
        body.data_attributes[0].chart_settings
    );
    (decoded, native, resource)
}

#[test]
fn series_empty_text_follows_current_map_and_actual_inherited_force() {
    use morph1c_core::version::{FormatVersion, with_roundtrip_target};
    for minor in [20, 21] {
        with_roundtrip_target(FormatVersion::new(2, minor), || {
            let mut body = chart(
                "<isSeriesDesign>true</isSeriesDesign><realExSeriesData><properties><id>20</id></properties></realExSeriesData>",
            );
            let (_, native, _) = roundtrip(&body);
            let text = std::str::from_utf8(&native).unwrap();
            let series = text
                .split("<d4p1:realExSeriesData>")
                .nth(1)
                .unwrap()
                .split("</d4p1:realExSeriesData>")
                .next()
                .unwrap();
            assert!(!series.contains("<d4p1:text"));

            let settings = body.data_attributes[0].chart_settings.as_mut().unwrap();
            let ChartValue::Nested(series) = &mut settings
                .fields
                .iter_mut()
                .find(|(name, _)| name == "realExSeriesData")
                .unwrap()
                .1
            else {
                panic!()
            };
            series
                .iter_mut()
                .find(|(name, _)| name == "text")
                .unwrap()
                .1 = ChartValue::Localized(vec![(
                morph1c_core::ir::Lang::new("en"),
                "Current title".into(),
            )]);
            let (decoded, current, _) = roundtrip(&body);
            assert!(
                std::str::from_utf8(&current)
                    .unwrap()
                    .contains("Current title")
            );
            assert_eq!(
                decoded.data_attributes[0].chart_settings,
                body.data_attributes[0].chart_settings
            );

            let settings = body.data_attributes[0].chart_settings.as_mut().unwrap();
            let ChartValue::Nested(series) = &mut settings
                .fields
                .iter_mut()
                .find(|(name, _)| name == "realExSeriesData")
                .unwrap()
                .1
            else {
                panic!()
            };
            series
                .iter_mut()
                .find(|(name, _)| name == "text")
                .unwrap()
                .1 = ChartValue::Localized(Vec::new());
            let (_, cleared, _) = roundtrip(&body);
            assert_eq!(cleared, native);
            // A native source wrapper remains a source spelling; its current map is empty.
            let close = text
                .lines()
                .find(|line| line.trim() == "</d4p1:realExSeriesData>")
                .unwrap();
            let indent = &close[..close.len() - close.trim_start().len()];
            let with_empty = text.replace(close, &format!("{indent}\t<d4p1:text/>\r\n{close}"));
            let source = read_form(FormDialect::Designer, with_empty.as_bytes()).unwrap();
            assert_eq!(
                write_form(FormDialect::Designer, &source).unwrap(),
                with_empty.as_bytes()
            );
        });
    }
}

#[test]
#[ignore = "fresh F-only native platform materialization fixtures; never configuration acceptance"]
fn capture_native_chart_materialization_matrix() {
    use morph1c_core::version::{FormatVersion, with_roundtrip_target};
    let root = std::path::PathBuf::from(
        std::env::var("IBCMD_CHART_NATIVE_MATRIX_CAPTURE").expect("fresh lab path"),
    );
    assert!(root.is_absolute() && root.starts_with("F:/ibcmd/lab/07"));
    std::fs::create_dir(&root).unwrap();
    for minor in [20, 21] {
        with_roundtrip_target(FormatVersion::new(2, minor), || {
            let profile = root.join(format!("2.{minor}"));
            std::fs::create_dir(&profile).unwrap();
            for (name, content) in [
                ("font-null", "<pointsScale><titleArea/></pointsScale>"),
                (
                    "font-auto",
                    "<pointsScale><titleArea><font xsi:type=\"core:AutoFont\"/></titleArea></pointsScale>",
                ),
                (
                    "font-absolute",
                    "<pointsScale><titleArea><font xsi:type=\"core:FontDef\"><faceName>Arial</faceName><height>12</height></font></titleArea></pointsScale>",
                ),
                (
                    "border-default",
                    "<pointsScale><titleArea><border xsi:type=\"core:BorderDef\"><width>1</width></border></titleArea></pointsScale>",
                ),
                ("label-area-null", "<pointsScale><titleArea/></pointsScale>"),
                (
                    "label-area-native-empty",
                    "<pointsScale><titleArea/></pointsScale>",
                ),
            ] {
                let mut body = chart(content);
                if name == "label-area-null" {
                    let settings = body.data_attributes[0].chart_settings.as_mut().unwrap();
                    let ChartValue::Nested(scale) = &mut settings
                        .fields
                        .iter_mut()
                        .find(|(n, _)| n == "pointsScale")
                        .unwrap()
                        .1
                    else {
                        panic!()
                    };
                    scale.iter_mut().find(|(n, _)| n == "titleArea").unwrap().1 =
                        ChartValue::Absent;
                }
                let mut native = write_form(FormDialect::Designer, &body).unwrap();
                if name == "label-area-native-empty" {
                    let mut text = String::from_utf8(native).unwrap();
                    let scale_start = text.find("<d4p1:pointsScale>").unwrap();
                    let start = scale_start + text[scale_start..].find("<d4p1:titleArea>").unwrap();
                    let end = start
                        + text[start..].find("</d4p1:titleArea>").unwrap()
                        + "</d4p1:titleArea>".len();
                    text.replace_range(start..end, "<d4p1:titleArea/>");
                    native = text.into_bytes();
                }
                let case = profile.join(name);
                std::fs::create_dir(&case).unwrap();
                std::fs::write(case.join("Form.xml"), &native).unwrap();
                std::fs::write(
                    case.join("current-chart.json"),
                    serde_json::to_vec_pretty(&body.data_attributes[0].chart_settings).unwrap(),
                )
                .unwrap();
                assert!(read_form(FormDialect::Designer, &native).is_ok());
            }
        });
    }
}
#[test]
fn suppressed_design_collections_keep_current_order_edits_and_binding() {
    let mut body = chart(
        "<isSeriesDesign>false</isSeriesDesign><realSeriesData><properties><id>11</id><text><key>en</key><value>First</value></text></properties></realSeriesData><realSeriesData><properties><id>12</id><text><key>en</key><value>Second</value></text></properties></realSeriesData><realExSeriesData><properties><id>20</id></properties></realExSeriesData><isPointsDesign>false</isPointsDesign><realPointData><id>7</id><expand>true</expand></realPointData>",
    );
    let (_, native, resource) = roundtrip(&body);
    let text = std::str::from_utf8(&native).unwrap();
    assert!(!text.contains("<d4p1:realSeriesData>"));
    assert!(!text.contains("<d4p1:realExSeriesData>"));
    assert!(!text.contains("<d4p1:realPointData>"));
    let chart = body.data_attributes[0].chart_settings.as_mut().unwrap();
    let ChartValue::Items(items) = &mut chart
        .fields
        .iter_mut()
        .find(|(n, _)| n == "realSeriesData")
        .unwrap()
        .1
    else {
        panic!()
    };
    items.reverse();
    items.remove(0);
    let (_, _, changed) = roundtrip(&body);
    assert_ne!(changed, resource);
    let mut stale = read_form(FormDialect::Designer, &native).unwrap();
    stale.data_attributes[0]
        .chart_settings
        .as_mut()
        .unwrap()
        .fields
        .iter_mut()
        .find(|(n, _)| n == "isSeriesDesign")
        .unwrap()
        .1 = ChartValue::Bool(true);
    let before = stale.clone();
    assert!(apply_chart_semantics_resource(&mut stale, Uuid([1; 16]), &resource).is_err());
    assert_eq!(stale, before);
    let mut unknown: serde_json::Value = serde_json::from_slice(&resource).unwrap();
    unknown["designs"][0]["unexpected"] = serde_json::json!(true);
    let mut clean = read_form(FormDialect::Designer, &native).unwrap();
    let before = clean.clone();
    assert!(
        apply_chart_semantics_resource(
            &mut clean,
            Uuid([1; 16]),
            &serde_json::to_vec(&unknown).unwrap()
        )
        .is_err()
    );
    assert_eq!(clean, before);
}
#[test]
fn hidden_design_and_visible_item_flags_share_complete_native_binding() {
    let body = chart(
        "<isSeriesDesign>false</isSeriesDesign><realSeriesData><properties><id>11</id></properties></realSeriesData><realDataItems><dataValue xsi:type=\"core:NumberValue\"><value>37.5</value></dataValue><tooltip>current tooltip</tooltip><isToolTipFormatted>true</isToolTipFormatted></realDataItems>",
    );
    let (_, native, resource) = roundtrip(&body);
    let records: serde_json::Value = serde_json::from_slice(&resource).unwrap();
    assert_eq!(records["designs"].as_array().unwrap().len(), 1);
    assert_eq!(records["records"].as_array().unwrap().len(), 1);
    let mut changed = read_form(FormDialect::Designer, &native).unwrap();
    let chart = changed.data_attributes[0].chart_settings.as_mut().unwrap();
    let ChartValue::Items(items) = &mut chart
        .fields
        .iter_mut()
        .find(|(name, _)| name == "realDataItems")
        .unwrap()
        .1
    else {
        panic!()
    };
    items[0]
        .iter_mut()
        .find(|(name, _)| name == "tooltip")
        .unwrap()
        .1 = ChartValue::Str("edited current tooltip".into());
    let before = changed.clone();
    assert!(apply_chart_semantics_resource(&mut changed, Uuid([1; 16]), &resource).is_err());
    assert_eq!(changed, before);
}
#[test]
fn nullable_series_absence_and_present_empty_current_objects_remain_distinct() {
    use morph1c_core::version::{FormatVersion, with_roundtrip_target};
    for minor in [20, 21] {
        with_roundtrip_target(FormatVersion::new(2, minor), || {
            for enabled in [false, true] {
                let prefix = format!("<isSeriesDesign>{enabled}</isSeriesDesign>");
                let absent = chart(&prefix);
                let present = chart(&format!(
                    "{prefix}<realExSeriesData><properties/></realExSeriesData>"
                ));
                assert_ne!(
                    absent.data_attributes[0].chart_settings,
                    present.data_attributes[0].chart_settings
                );
                let (_, raw, absent_resource) = roundtrip(&absent);
                let (_, _, present_resource) = roundtrip(&present);
                assert_ne!(absent_resource, present_resource);
                let native = read_form(FormDialect::Designer, &raw).unwrap();
                let settings = native.data_attributes[0].chart_settings.as_ref().unwrap();
                let ChartValue::Nested(fields) = &settings
                    .fields
                    .iter()
                    .find(|(name, _)| name == "realExSeriesData")
                    .unwrap()
                    .1
                else {
                    panic!()
                };
                assert_eq!(
                    fields.iter().find(|(name, _)| name == "id").unwrap().1,
                    ChartValue::Int("1".into())
                );
                assert_eq!(
                    fields.iter().find(|(name, _)| name == "line").unwrap().1,
                    ChartValue::Line {
                        style: "Solid".into(),
                        width: "2".into(),
                        gap: false
                    }
                );
            }
        });
    }
}
#[test]
fn native_default_provider_emits_unlisted_current_defaults_and_nondefaults() {
    let body = chart(
        "<isSeriesDesign>true</isSeriesDesign><legendPlacement>Auto</legendPlacement><innerRadiusDonutChart>60</innerRadiusDonutChart><realSeriesData><properties><id>5</id><visualType>Auto</visualType><stackGroup/></properties></realSeriesData>",
    );
    let (_, native, _) = roundtrip(&body);
    let text = std::str::from_utf8(&native).unwrap();
    assert!(text.contains("<d4p1:legendPlacement>Auto</"));
    assert!(text.contains("<d4p1:innerRadiusDonutChart>60</"));
    assert!(text.contains("<d4p1:visualType>Auto</"));
    assert!(text.contains("<d4p1:stackGroup/>"));
    let mut edited = body;
    let c = edited.data_attributes[0].chart_settings.as_mut().unwrap();
    c.fields
        .iter_mut()
        .find(|(n, _)| n == "legendPlacement")
        .unwrap()
        .1 = ChartValue::Enum("Right".into());
    c.fields
        .iter_mut()
        .find(|(n, _)| n == "innerRadiusDonutChart")
        .unwrap()
        .1 = ChartValue::Int("47".into());
    let (decoded, output, _) = roundtrip(&edited);
    let text = std::str::from_utf8(&output).unwrap();
    assert!(text.contains(">Right<"));
    assert!(text.contains(">47<"));
    assert_eq!(
        decoded.data_attributes[0].chart_settings,
        edited.data_attributes[0].chart_settings
    );
}
#[test]
fn native_factory_scales_omit_only_complete_defaults_and_axes_remain_forced() {
    let body = chart("<isSeriesDesign>true</isSeriesDesign>");
    let mut wire = String::from_utf8(write_form(FormDialect::Designer, &body).unwrap()).unwrap();
    for name in [
        "valuesScale",
        "pointsScale",
        "seriesScale",
        "additionalValuesScale",
    ] {
        let opening = format!("<d4p1:{name}");
        if let Some(start) = wire.find(&opening) {
            let open_end = start + wire[start..].find('>').unwrap() + 1;
            let end = if wire[..open_end].ends_with("/>") {
                open_end
            } else {
                let closing = format!("</d4p1:{name}>");
                open_end + wire[open_end..].find(&closing).unwrap() + closing.len()
            };
            wire.replace_range(start..end, "");
        }
    }
    let mut current = read_form(FormDialect::Designer, wire.as_bytes()).unwrap();
    let settings = current.data_attributes[0].chart_settings.as_mut().unwrap();
    settings.source_layout = None;
    let native = write_form(FormDialect::Designer, &current).unwrap();
    let text = std::str::from_utf8(&native).unwrap();
    assert!(!text.contains("<d4p1:valuesScale"));
    assert!(!text.contains("<d4p1:additionalValuesScale"));
    assert!(text.contains("<d4p1:valuesAxis/>"));
    assert!(text.contains("<d4p1:pointsAxis/>"));
    assert!(!text.contains("<d4p1:text/>"));
    assert_eq!(
        read_form(FormDialect::Designer, &native)
            .unwrap()
            .data_attributes[0]
            .chart_settings,
        current.data_attributes[0].chart_settings
    );
    // The SDK emptiness predicate tests NULL references, not equality with the fields of
    // an explicitly present object. Each CURRENT non-null default-valued ref must emit.
    let font_body = chart("<labelsFont xsi:type=\"core:AutoFont\"/>");
    let font = font_body.data_attributes[0]
        .chart_settings
        .as_ref()
        .unwrap()
        .fields
        .iter()
        .find(|(name, _)| name == "labelsFont")
        .unwrap()
        .1
        .clone();
    for (name, value) in [
        (
            "border",
            ChartValue::Border {
                style: "WithoutBorder".into(),
                width: "1".into(),
            },
        ),
        ("font", font),
        ("textColor", ChartValue::Color("#000000".into())),
    ] {
        let mut explicit = current.clone();
        let settings = explicit.data_attributes[0].chart_settings.as_mut().unwrap();
        let ChartValue::Nested(scale) = &mut settings
            .fields
            .iter_mut()
            .find(|(n, _)| n == "valuesScale")
            .unwrap()
            .1
        else {
            panic!()
        };
        let ChartValue::Nested(area) =
            &mut scale.iter_mut().find(|(n, _)| n == "titleArea").unwrap().1
        else {
            panic!()
        };
        let slot = &mut area.iter_mut().find(|(n, _)| n == name).unwrap().1;
        assert_eq!(*slot, ChartValue::Absent);
        *slot = value;
        let native = write_form(FormDialect::Designer, &explicit).unwrap();
        assert!(
            std::str::from_utf8(&native)
                .unwrap()
                .contains("<d4p1:valuesScale>")
        );
        let returned = read_form(FormDialect::Designer, &native).unwrap();
        assert_eq!(
            returned.data_attributes[0].chart_settings,
            explicit.data_attributes[0].chart_settings
        );
        let settings = explicit.data_attributes[0].chart_settings.as_mut().unwrap();
        let ChartValue::Nested(scale) = &mut settings
            .fields
            .iter_mut()
            .find(|(n, _)| n == "valuesScale")
            .unwrap()
            .1
        else {
            panic!()
        };
        let ChartValue::Nested(area) =
            &mut scale.iter_mut().find(|(n, _)| n == "titleArea").unwrap().1
        else {
            panic!()
        };
        area.iter_mut().find(|(n, _)| n == name).unwrap().1 = ChartValue::Absent;
        let native = write_form(FormDialect::Designer, &explicit).unwrap();
        assert!(
            !std::str::from_utf8(&native)
                .unwrap()
                .contains("<d4p1:valuesScale")
        );
        assert_eq!(
            read_form(FormDialect::Designer, &native)
                .unwrap()
                .data_attributes[0]
                .chart_settings,
            current.data_attributes[0].chart_settings
        );
    }
    let settings = current.data_attributes[0].chart_settings.as_mut().unwrap();
    let ChartValue::Nested(scale) = &mut settings
        .fields
        .iter_mut()
        .find(|(n, _)| n == "valuesScale")
        .unwrap()
        .1
    else {
        panic!()
    };
    scale.iter_mut().find(|(n, _)| n == "showTitle").unwrap().1 = ChartValue::Enum("Show".into());
    let native = write_form(FormDialect::Designer, &current).unwrap();
    assert!(
        std::str::from_utf8(&native)
            .unwrap()
            .contains("<d4p1:showTitle>Show</d4p1:showTitle>")
    );
    assert_eq!(
        read_form(FormDialect::Designer, &native)
            .unwrap()
            .data_attributes[0]
            .chart_settings,
        current.data_attributes[0].chart_settings
    );
}
#[test]
fn native_null_colors_are_forced_without_conflating_present_objects() {
    use morph1c_core::version::{FormatVersion, with_roundtrip_target, with_source_version};
    for minor in [20, 21] {
        let version = FormatVersion::new(2, minor);
        with_source_version(Some(version), || {
            with_roundtrip_target(version, || {
                let body = chart(
                    "<valuesScale><titleArea><border xsi:type=\"core:BorderDef\"><width>1</width></border></titleArea></valuesScale>",
                );
                let (_, native, _) = roundtrip(&body);
                let text = std::str::from_utf8(&native).unwrap();
                assert!(text.contains("<d4p1:labelsColor>auto</d4p1:labelsColor>"));
                let scale = text
                    .split("<d4p1:valuesScale>")
                    .nth(1)
                    .unwrap()
                    .split("</d4p1:valuesScale>")
                    .next()
                    .unwrap();
                let area = scale
                    .split("<d4p1:titleArea>")
                    .nth(1)
                    .unwrap()
                    .split("</d4p1:titleArea>")
                    .next()
                    .unwrap();
                for name in ["textColor", "backColor", "borderColor"] {
                    assert!(area.contains(&format!("<d4p1:{name}>auto</d4p1:{name}>")));
                }
                assert!(!area.contains("<d4p1:font"));
                assert!(area.contains("<d4p1:border"));
                let decoded = read_form(FormDialect::Designer, &native).unwrap();
                let settings = decoded.data_attributes[0].chart_settings.as_ref().unwrap();
                assert_eq!(
                    settings
                        .fields
                        .iter()
                        .find(|(n, _)| n == "labelsColor")
                        .unwrap()
                        .1,
                    ChartValue::Absent
                );
                let explicit = chart(
                    "<labelsColor xsi:type=\"core:ColorDef\"/><valuesScale><titleArea><font xsi:type=\"core:AutoFont\"/><textColor xsi:type=\"core:ColorRef\"><color>Style.WarningText</color></textColor><border xsi:type=\"core:BorderDef\"><width>1</width></border></titleArea></valuesScale>",
                );
                let (_, native, _) = roundtrip(&explicit);
                let text = std::str::from_utf8(&native).unwrap();
                assert!(text.contains("<d4p1:labelsColor>#000000</d4p1:labelsColor>"));
                assert!(text.contains("<d4p1:textColor>style:WarningText</d4p1:textColor>"));
                assert!(text.contains("<d4p1:font"));
                assert!(text.contains("<d4p1:border"));
                let items = chart(
                    "<isSeriesDesign>true</isSeriesDesign><isPointsDesign>true</isPointsDesign><realPointData><id>7</id></realPointData><realSeriesData><properties><id>9</id></properties></realSeriesData>",
                );
                let (_, native, _) = roundtrip(&items);
                let text = std::str::from_utf8(&native).unwrap();
                for name in ["realPointData", "realSeriesData"] {
                    let item = text
                        .split(&format!("<d4p1:{name}>"))
                        .nth(1)
                        .unwrap()
                        .split(&format!("</d4p1:{name}>"))
                        .next()
                        .unwrap();
                    assert!(item.contains("<d4p1:color>auto</d4p1:color>"));
                }
            })
        });
    }
}

#[test]
fn gantt_inherited_force_preserves_nested_null_and_current_nonnull_colors() {
    use morph1c_core::version::{FormatVersion, with_roundtrip_target, with_source_version};
    for minor in [20, 21] {
        let version = FormatVersion::new(2, minor);
        with_source_version(Some(version), || {
            with_roundtrip_target(version, || {
                let source = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<ganttchart:GanttChart xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:core=\"http://g5.1c.ru/v8/dt/mcore\" xmlns:ganttchart=\"http://g5.1c.ru/v8/dt/ganttchart/model\"><points><contentCacheItem/></points><series><contentCacheItem/></series><timeScale/></ganttchart:GanttChart>\r\n";
                let mut body = chart("");
                body.data_attributes[0].chart_settings = Some(read_chart_sidecar(source).unwrap());
                let (_, native, _) = roundtrip(&body);
                let text = std::str::from_utf8(&native).unwrap();
                let points = text
                    .split("<d4p1:points>")
                    .nth(1)
                    .unwrap()
                    .split("</d4p1:points>")
                    .next()
                    .unwrap();
                for name in ["mainColor", "secondColor", "backColor", "textColor"] {
                    assert!(points.contains(&format!("<d4p1:{name}>auto</d4p1:{name}>")));
                }
                let time = text
                    .split("<d4p1:timeScale>")
                    .nth(1)
                    .unwrap()
                    .split("</d4p1:timeScale>")
                    .next()
                    .unwrap();
                assert!(time.contains("<d4p1:backColor>auto</d4p1:backColor>"));
                for value in [
                    ChartValue::Color("#000000".into()),
                    ChartValue::Color("style:WarningText".into()),
                    ChartValue::Absent,
                ] {
                    let settings = body.data_attributes[0].chart_settings.as_mut().unwrap();
                    let ChartValue::Nested(points) = &mut settings
                        .fields
                        .iter_mut()
                        .find(|(n, _)| n == "points")
                        .unwrap()
                        .1
                    else {
                        panic!()
                    };
                    let ChartValue::Nested(content) = &mut points
                        .iter_mut()
                        .find(|(n, _)| n == "contentCacheItem")
                        .unwrap()
                        .1
                    else {
                        panic!()
                    };
                    content
                        .iter_mut()
                        .find(|(n, _)| n == "mainColor")
                        .unwrap()
                        .1 = value.clone();
                    let (_, native, _) = roundtrip(&body);
                    let expected = match &value {
                        ChartValue::Color(c) => c.as_str(),
                        ChartValue::Absent => "auto",
                        _ => unreachable!(),
                    };
                    assert!(
                        std::str::from_utf8(&native)
                            .unwrap()
                            .contains(&format!("<d4p1:mainColor>{expected}</d4p1:mainColor>"))
                    );
                }
            })
        });
    }
}

#[test]
fn native_auto_case_facet_cannot_replay_a_previous_current_color() {
    use morph1c_core::version::{FormatVersion, with_roundtrip_target, with_source_version};
    for minor in [20, 21] {
        let version = FormatVersion::new(2, minor);
        with_source_version(Some(version), || {
            with_roundtrip_target(version, || {
                let (_, native, _) = roundtrip(&chart(""));
                let native = String::from_utf8(native).unwrap();
                for spelling in ["auto", "AUTO", "AuTo", "aUtO"] {
                    let source = native.replace(
                        "<d4p1:labelsColor>auto</d4p1:labelsColor>",
                        &format!("<d4p1:labelsColor>{spelling}</d4p1:labelsColor>"),
                    );
                    assert_ne!(source.matches("<d4p1:labelsColor>").count(), 0);
                    let mut current = read_form(FormDialect::Designer, source.as_bytes()).unwrap();
                    assert_eq!(
                        write_form(FormDialect::Designer, &current).unwrap(),
                        source.as_bytes()
                    );
                    for color in ["#000000", "#12ABEF", "style:WarningText"] {
                        current.data_attributes[0]
                            .chart_settings
                            .as_mut()
                            .unwrap()
                            .fields
                            .iter_mut()
                            .find(|(n, _)| n == "labelsColor")
                            .unwrap()
                            .1 = ChartValue::Color(color.into());
                        let output = write_form(FormDialect::Designer, &current).unwrap();
                        assert!(
                            std::str::from_utf8(&output)
                                .unwrap()
                                .contains(&format!("<d4p1:labelsColor>{color}</d4p1:labelsColor>"))
                        );
                        let decoded = read_form(FormDialect::Designer, &output).unwrap();
                        assert_eq!(
                            decoded.data_attributes[0].chart_settings,
                            current.data_attributes[0].chart_settings
                        );
                    }
                    current.data_attributes[0]
                        .chart_settings
                        .as_mut()
                        .unwrap()
                        .fields
                        .iter_mut()
                        .find(|(n, _)| n == "labelsColor")
                        .unwrap()
                        .1 = ChartValue::Absent;
                    assert_eq!(
                        write_form(FormDialect::Designer, &current).unwrap(),
                        source.as_bytes()
                    );
                    for bad in [
                        "<d4p1:labelsColor extra=\"x\">auto</d4p1:labelsColor>",
                        "<d4p1:labelsColor><d4p1:unknown/></d4p1:labelsColor>",
                    ] {
                        let broken = source.replace(
                            &format!("<d4p1:labelsColor>{spelling}</d4p1:labelsColor>"),
                            bad,
                        );
                        assert!(read_form(FormDialect::Designer, broken.as_bytes()).is_err());
                    }
                }
            })
        });
    }
}

#[test]
fn edt_null_color_shapes_preserve_source_presence_and_current_edits() {
    use formats_xml::form::write_chart_sidecar;
    use morph1c_core::version::{FormatVersion, with_roundtrip_target, with_source_version};
    for minor in [20, 21] {
        let version = FormatVersion::new(2, minor);
        with_source_version(Some(version), || {
            with_roundtrip_target(version, || {
                let body = chart("<labelsColor xsi:nil=\"true\"/>");
                let canonical = String::from_utf8(
                    write_chart_sidecar(body.data_attributes[0].chart_settings.as_ref().unwrap())
                        .unwrap(),
                )
                .unwrap();
                assert!(canonical.contains("<labelsColor xsi:nil=\"true\"/>"));
                for shape in ["<labelsColor/>", "<labelsColor xsi:nil=\"true\"/>"] {
                    let source = canonical.replace("<labelsColor xsi:nil=\"true\"/>", shape);
                    let mut settings = read_chart_sidecar(source.as_bytes()).unwrap();
                    assert_eq!(
                        settings
                            .fields
                            .iter()
                            .find(|(n, _)| n == "labelsColor")
                            .unwrap()
                            .1,
                        ChartValue::Absent
                    );
                    assert_eq!(write_chart_sidecar(&settings).unwrap(), source.as_bytes());
                    settings
                        .fields
                        .iter_mut()
                        .find(|(n, _)| n == "labelsColor")
                        .unwrap()
                        .1 = ChartValue::Color("#12ABEF".into());
                    let output = write_chart_sidecar(&settings).unwrap();
                    let text = std::str::from_utf8(&output).unwrap();
                    assert!(text.contains("<labelsColor xsi:type=\"core:ColorDef\">"));
                    assert!(!text.contains("<labelsColor xsi:nil="));
                    assert_eq!(read_chart_sidecar(&output).unwrap(), settings);
                }
                for shape in [
                    "<labelsColor unexpected=\"x\"/>",
                    "<labelsColor xsi:nil=\"true\">nonempty</labelsColor>",
                    "<labelsColor xsi:nil=\"true\"><unknown/></labelsColor>",
                    "<labelsColor xmlns:xsi=\"urn:wrong\" xsi:nil=\"true\"/>",
                ] {
                    let source = canonical.replace("<labelsColor xsi:nil=\"true\"/>", shape);
                    assert!(read_chart_sidecar(source.as_bytes()).is_err());
                }
            })
        });
    }
}

#[test]
fn platform_funnel_spelling_and_native_source_fraction_presence_use_current_number() {
    let body = chart(
        "<isSeriesDesign>true</isSeriesDesign><funnelNeckHeightPercent>10.0</funnelNeckHeightPercent><funnelGapSumPercent>3.0</funnelGapSumPercent>",
    );
    let native = write_form(FormDialect::Designer, &body).unwrap();
    let text = std::str::from_utf8(&native).unwrap();
    assert!(text.contains("<d4p1:funnelNeckHeightPercent>10.0</d4p1:funnelNeckHeightPercent>"));
    for (spelling, current) in [("10", "27"), ("10.0", "27.0"), ("10.00", "27.00")] {
        let explicit = text.replace(
            "<d4p1:funnelNeckHeightPercent>10.0</",
            &format!("<d4p1:funnelNeckHeightPercent>{spelling}</"),
        );
        let mut decoded = read_form(FormDialect::Designer, explicit.as_bytes()).unwrap();
        assert_eq!(
            write_form(FormDialect::Designer, &decoded).unwrap(),
            explicit.as_bytes()
        );
        decoded.data_attributes[0]
            .chart_settings
            .as_mut()
            .unwrap()
            .fields
            .iter_mut()
            .find(|(n, _)| n == "funnelNeckHeightPercent")
            .unwrap()
            .1 = ChartValue::Int("27".into());
        let output = write_form(FormDialect::Designer, &decoded).unwrap();
        assert!(
            std::str::from_utf8(&output)
                .unwrap()
                .contains(&format!("<d4p1:funnelNeckHeightPercent>{current}</"))
        );
        decoded.data_attributes[0]
            .chart_settings
            .as_mut()
            .unwrap()
            .fields
            .iter_mut()
            .find(|(n, _)| n == "funnelNeckHeightPercent")
            .unwrap()
            .1 = ChartValue::Int("27.25".into());
        assert!(
            String::from_utf8(write_form(FormDialect::Designer, &decoded).unwrap())
                .unwrap()
                .contains("<d4p1:funnelNeckHeightPercent>27.25</")
        );
    }
}

#[test]
fn native_edouble_uses_sdk_long_projection_and_preserves_current_binary64() {
    for (number, native) in [
        ("1e23", "9223372036854775807"),
        ("-1e23", "-9223372036854775808"),
        ("5e-324", "4.9E-324"),
        ("-0", "0"),
        ("17", "17"),
        ("17.25", "17.25"),
        ("Infinity", "Infinity"),
    ] {
        let body = chart(&format!("<userMaxValue>{number}</userMaxValue>"));
        let (_, output, _) = roundtrip(&body);
        assert!(
            String::from_utf8(output)
                .unwrap()
                .contains(&format!("<d4p1:userMaxValue>{native}</"))
        );
    }
    let mut body = chart("<userMaxValue>10.0</userMaxValue>");
    body.data_attributes[0]
        .chart_settings
        .as_mut()
        .unwrap()
        .fields
        .iter_mut()
        .find(|(n, _)| n == "userMaxValue")
        .unwrap()
        .1 = ChartValue::Int("not-a-double".into());
    assert!(write_form(FormDialect::Designer, &body).is_err());
}

#[test]
fn lossy_edouble_resource_is_current_closed_and_bound_before_mutation() {
    use morph1c_core::version::{FormatVersion, with_roundtrip_target};
    for minor in [20, 21] {
        with_roundtrip_target(FormatVersion::new(2, minor), || {
            let body = chart("<userMinValue>-0.0</userMinValue><userMaxValue>1e23</userMaxValue>");
            let (_, native, resource) = roundtrip(&body);
            let payload: serde_json::Value = serde_json::from_slice(&resource).unwrap();
            assert_eq!(payload["numbers"].as_array().unwrap().len(), 1);
            assert_eq!(
                payload["numbers"][0]["current"].as_array().unwrap().len(),
                2
            );
            let mut stale = read_form(FormDialect::Designer, &native).unwrap();
            stale.data_attributes[0]
                .chart_settings
                .as_mut()
                .unwrap()
                .fields
                .iter_mut()
                .find(|(name, _)| name == "userMaxValue")
                .unwrap()
                .1 = ChartValue::Int("41".into());
            let before = stale.clone();
            assert!(apply_chart_semantics_resource(&mut stale, Uuid([1; 16]), &resource).is_err());
            assert_eq!(stale, before);
            for bad in [
                {
                    let mut p = payload.clone();
                    let row = p["numbers"][0].clone();
                    p["numbers"].as_array_mut().unwrap().push(row);
                    p
                },
                {
                    let mut p = payload.clone();
                    p["numbers"][0]["unknown"] = serde_json::json!(true);
                    p
                },
                {
                    let mut p = payload.clone();
                    p["numbers"][0]["current"][0][0] =
                        serde_json::json!([{"Field":"unknownCurrentNumber"}]);
                    p
                },
                {
                    let mut p = payload.clone();
                    p["numbers"][0]["current"][0][1] = serde_json::json!("17");
                    p
                },
            ] {
                let mut native_body = read_form(FormDialect::Designer, &native).unwrap();
                let before = native_body.clone();
                assert!(
                    apply_chart_semantics_resource(
                        &mut native_body,
                        Uuid([1; 16]),
                        &serde_json::to_vec(&bad).unwrap()
                    )
                    .is_err()
                );
                assert_eq!(native_body, before);
            }
            let ordinary = chart("<userMaxValue>17.25</userMaxValue>");
            if let Some((_, ordinary_resource)) =
                project_chart_semantics(&ordinary, Uuid([1; 16])).unwrap()
            {
                let ordinary_payload: serde_json::Value =
                    serde_json::from_slice(&ordinary_resource).unwrap();
                assert!(ordinary_payload.get("numbers").is_none());
            }
            let (_, output, _) = roundtrip(&ordinary);
            assert!(
                std::str::from_utf8(&output)
                    .unwrap()
                    .contains("<d4p1:userMaxValue>17.25</")
            );
        });
    }
}

#[test]
fn lossy_edouble_and_visible_flags_bind_hidden_design_in_one_atomic_resource() {
    let body = chart(
        "<userMinValue>-0</userMinValue><userMaxValue>1e23</userMaxValue><isSeriesDesign>false</isSeriesDesign><realSeriesData><properties><id>11</id><text><key>en</key><value>Current hidden title</value></text></properties></realSeriesData><realDataItems><dataValue xsi:type=\"core:NumberValue\"><value>37.5</value></dataValue><isToolTipFormatted>true</isToolTipFormatted></realDataItems>",
    );
    let (_, native, resource) = roundtrip(&body);
    let payload: serde_json::Value = serde_json::from_slice(&resource).unwrap();
    for name in ["designs", "numbers", "records"] {
        assert_eq!(payload[name].as_array().unwrap().len(), 1, "{name}");
    }
    let mut changed = read_form(FormDialect::Designer, &native).unwrap();
    changed.data_attributes[0]
        .chart_settings
        .as_mut()
        .unwrap()
        .fields
        .iter_mut()
        .find(|(name, _)| name == "userMinValue")
        .unwrap()
        .1 = ChartValue::Int("7".into());
    let before = changed.clone();
    assert!(apply_chart_semantics_resource(&mut changed, Uuid([1; 16]), &resource).is_err());
    assert_eq!(changed, before);
}

#[test]
fn funnel_current_efloat_rounding_is_distinct_from_edouble_and_big_decimal() {
    use morph1c_core::version::{FormatVersion, with_roundtrip_target};
    for minor in [20, 21] {
        with_roundtrip_target(FormatVersion::new(2, minor), || {
            for (number, native) in [
                ("16777217", "1.6777216E7"),
                ("16777219", "1.677722E7"),
                ("1e23", "1.0E23"),
                ("1.4e-45", "1.4E-45"),
                ("3.4028235e38", "3.4028235E38"),
                ("0.1", "0.1"),
                ("-0", "-0.0"),
                ("NaN", "NaN"),
                ("Infinity", "Infinity"),
            ] {
                let body = chart(&format!(
                    "<funnelNeckHeightPercent>{number}</funnelNeckHeightPercent><userMaxValue>16777217</userMaxValue>"
                ));
                let (_, output, _) = roundtrip(&body);
                let output = String::from_utf8(output).unwrap();
                assert!(output.contains(&format!("<d4p1:funnelNeckHeightPercent>{native}</")));
                assert!(output.contains("<d4p1:userMaxValue>16777217</"));
            }
            assert_eq!(
                chart("<userMaxValue>0.10000000000000001</userMaxValue>").data_attributes[0]
                    .chart_settings,
                chart("<userMaxValue>0.1</userMaxValue>").data_attributes[0].chart_settings
            );
            assert_ne!(
                chart("<userMaxValue>-0</userMaxValue>").data_attributes[0].chart_settings,
                chart("<userMaxValue>0</userMaxValue>").data_attributes[0].chart_settings
            );
        });
    }
}

#[test]
fn native_number_layout_never_overrides_current_binary_rounding() {
    let source = chart(
        "<funnelNeckHeightPercent>10.0</funnelNeckHeightPercent><userMaxValue>10.0</userMaxValue>",
    );
    let native = write_form(FormDialect::Designer, &source).unwrap();
    let mut current = read_form(FormDialect::Designer, &native).unwrap();
    let settings = current.data_attributes[0].chart_settings.as_mut().unwrap();
    settings
        .fields
        .iter_mut()
        .find(|(n, _)| n == "funnelNeckHeightPercent")
        .unwrap()
        .1 = ChartValue::Int("16777217".into());
    settings
        .fields
        .iter_mut()
        .find(|(n, _)| n == "userMaxValue")
        .unwrap()
        .1 = ChartValue::Int("1e23".into());
    let (projected, resource) = project_chart_semantics(&current, Uuid([1; 16]))
        .unwrap()
        .unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&resource).unwrap();
    let numbers = payload["numbers"].as_array().unwrap();
    assert_eq!(numbers.len(), 1);
    assert_eq!(numbers[0]["current"].as_array().unwrap().len(), 1);
    assert!(numbers[0]["current"].to_string().contains("userMaxValue"));
    assert!(
        !numbers[0]["current"]
            .to_string()
            .contains("funnelNeckHeightPercent")
    );
    let output = write_form(FormDialect::Designer, &projected).unwrap();
    let text = std::str::from_utf8(&output).unwrap();
    assert!(text.contains("<d4p1:funnelNeckHeightPercent>1.6777216E7</"));
    assert!(text.contains("<d4p1:userMaxValue>9223372036854775807</"));
    let mut decoded = read_form(FormDialect::Designer, &output).unwrap();
    apply_chart_semantics_resource(&mut decoded, Uuid([1; 16]), &resource).unwrap();
    let expected = chart(
        "<funnelNeckHeightPercent>16777217</funnelNeckHeightPercent><userMaxValue>1e23</userMaxValue>",
    );
    for name in ["funnelNeckHeightPercent", "userMaxValue"] {
        let decoded_fields = &decoded.data_attributes[0]
            .chart_settings
            .as_ref()
            .unwrap()
            .fields;
        let expected_fields = &expected.data_attributes[0]
            .chart_settings
            .as_ref()
            .unwrap()
            .fields;
        assert_eq!(
            decoded_fields.iter().find(|(n, _)| n == name).unwrap().1,
            expected_fields.iter().find(|(n, _)| n == name).unwrap().1
        );
    }
}

#[test]
fn chart_float_inputs_follow_original_emf_grammar_for_current_values_only() {
    use morph1c_core::version::{FormatVersion, with_roundtrip_target};
    for minor in [20, 21] {
        with_roundtrip_target(FormatVersion::new(2, minor), || {
            for (lexical, expected) in [
                ("  1.25\t", "1.25"),
                ("0x1.8p1", "3.0"),
                ("1.25f", "1.25"),
                ("1.25D", "1.25"),
                ("+NaN", "NaN"),
                ("-NaN", "NaN"),
                ("1e500", "Infinity"),
            ] {
                let body = chart(&format!(
                    "<funnelNeckHeightPercent>{lexical}</funnelNeckHeightPercent><userMaxValue>{lexical}</userMaxValue>"
                ));
                let (_, native, _) = roundtrip(&body);
                let text = String::from_utf8(native).unwrap();
                assert!(text.contains(&format!("<d4p1:funnelNeckHeightPercent>{expected}</")));
                let double_expected = if expected == "3.0" { "3" } else { expected };
                assert!(text.contains(&format!("<d4p1:userMaxValue>{double_expected}</")));
            }
            for lexical in ["inf", "NaNf", "InfinityD", "１２.５", "0x1", "1.2.3"] {
                let source = format!(
                    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<chart:Chart xmlns:chart=\"http://g5.1c.ru/v8/dt/chart/model\"><userMaxValue>{lexical}</userMaxValue></chart:Chart>\r\n"
                );
                assert!(read_chart_sidecar(source.as_bytes()).is_err(), "{lexical}");
            }
        });
    }
}

#[test]
fn sdk_unlisted_defaults_are_emitted_for_all_closed_chart_and_series_rows() {
    use morph1c_core::version::{FormatVersion, with_roundtrip_target};
    let chart_rows = [
        "legendPlacement",
        "plotAreaPlacement",
        "titleAreaPlacement",
        "nonnumericValuesUse",
        "pointConnectionAcrossSkippedValues",
        "valuesToolTipShowMode",
        "valuesToolTipFillType",
        "selectionMode",
        "pointsDropLinesShowMode",
        "valuesDropLinesShowMode",
        "valuesEditMode",
    ];
    let series_rows = [
        "showGraphicalRepresentationOfDataInChartLegend",
        "showGraphicalRepresentationOfDataOnChart",
        "visualType",
        "addType",
        "valuesAxisUsage",
        "valuesEditMode",
    ];
    let mut content = String::from(
        "<isSeriesDesign>true</isSeriesDesign><distributedKey/><innerRadiusDonutChart>60</innerRadiusDonutChart>",
    );
    for name in chart_rows {
        content.push_str(&format!("<{name}>Auto</{name}>"));
    }
    content.push_str("<realSeriesData><properties><id>5</id><stackGroup/>");
    for name in series_rows {
        content.push_str(&format!("<{name}>Auto</{name}>"));
    }
    content.push_str("</properties></realSeriesData>");
    for minor in [20, 21] {
        with_roundtrip_target(FormatVersion::new(2, minor), || {
            let body = chart(&content);
            let (_, native, _) = roundtrip(&body);
            let output = String::from_utf8(native).unwrap();
            for name in chart_rows.into_iter().chain(series_rows) {
                let native_name = match name {
                    "showGraphicalRepresentationOfDataInChartLegend" => {
                        "showGraphicalDataRepresentationInChartLegend"
                    }
                    "showGraphicalRepresentationOfDataOnChart" => {
                        "showGraphicalDataRepresentationInChart"
                    }
                    _ => name,
                };
                assert!(
                    output.contains(&format!("<d4p1:{native_name}>Auto</")),
                    "{name}"
                );
            }
            assert!(output.contains("<d4p1:distributedKey/>"));
            assert!(output.contains("<d4p1:stackGroup/>"));
            assert!(output.contains("<d4p1:innerRadiusDonutChart>60</"));
        });
    }
}

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

fn chart_settings_bytes(bytes: &[u8]) -> Vec<Vec<u8>> {
    use quick_xml::events::Event;
    // quick-xml buffer positions exclude a leading UTF-8 BOM.
    let offset = usize::from(bytes.starts_with(&[0xef, 0xbb, 0xbf])) * 3;
    let mut reader = quick_xml::Reader::from_reader(&bytes[offset..]);
    let mut captured = None;
    let mut result = Vec::new();
    loop {
        let start = offset + reader.buffer_position() as usize;
        match reader.read_event().unwrap() {
            Event::Start(el) => {
                if let Some((_, depth)) = captured.as_mut() {
                    *depth += 1;
                } else if el.local_name().as_ref() == b"Settings"
                    && bytes[start..offset + reader.buffer_position() as usize]
                        .windows(b"http://v8.1c.ru/8.2/data/chart".len())
                        .any(|w| w == b"http://v8.1c.ru/8.2/data/chart")
                {
                    captured = Some((start, 1usize));
                }
            }
            Event::End(_) => {
                if let Some((begin, depth)) = captured.as_mut() {
                    *depth -= 1;
                    if *depth == 0 {
                        result.push(
                            bytes[*begin..offset + reader.buffer_position() as usize].to_vec(),
                        );
                        captured = None;
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    assert!(captured.is_none());
    result
}
#[test]
#[ignore = "requires hash-bound authentic EDT/native SDK chart corpus manifest"]
fn genuine_all_changed_chart_settings_match_native_sdk_bytes() {
    use sha2::{Digest, Sha256};
    let sha = |b: &[u8]| format!("{:x}", Sha256::digest(b));
    let manifest = std::fs::read(std::env::var_os("IBCMD_CHART_MANIFEST").unwrap()).unwrap();
    let report = std::path::PathBuf::from(std::env::var_os("IBCMD_CHART_REPORT").unwrap());
    let cases: serde_json::Value = serde_json::from_slice(&manifest).unwrap();
    assert_eq!(cases["selected_forms"], 23);
    assert_eq!(cases["sidecars"], 27);
    let artifact_dir = report.with_extension("artifacts");
    std::fs::create_dir(&artifact_dir).unwrap();
    use morph1c_core::version::{FormatVersion, with_roundtrip_target, with_source_version};
    let version = match cases["profile"].as_str().unwrap() {
        "83" => FormatVersion::new(2, 20),
        "85" => FormatVersion::new(2, 21),
        _ => panic!("unbound corpus profile"),
    };
    let mut rows = Vec::new();
    let mut failures = 0;
    with_source_version(Some(version), || {
        with_roundtrip_target(version, || {
            for (index, row) in cases["rows"].as_array().unwrap().iter().enumerate() {
                let source = std::fs::read(row["source"].as_str().unwrap()).unwrap();
                let reference = std::fs::read(row["reference"].as_str().unwrap()).unwrap();
                let historical = std::fs::read(row["historical"].as_str().unwrap()).unwrap();
                assert_eq!(sha(&source), row["source_sha256"]);
                assert_eq!(sha(&reference), row["reference_sha256"]);
                assert_eq!(sha(&historical), row["historical_sha256"]);
                let current = read_chart_sidecar(&source).unwrap();
                assert_eq!(
                    formats_xml::form::write_chart_sidecar(&current).unwrap(),
                    source
                );
                // Keep the positively bound real Form/Attribute context (namespace declarations,
                // source ordering and XML depth); replace only the CURRENT typed chart state.
                let isolated = isolated_sdk_chart_attribute(
                    &reference,
                    row["attribute_name"].as_str().unwrap(),
                );
                let reference_body = read_form(FormDialect::Designer, &isolated).unwrap();
                assert_eq!(reference_body.data_attributes.len(), 1);
                assert!(reference_body.data_attributes[0].chart_settings.is_some());
                let mut body = reference_body.clone();
                body.data_attributes[0].chart_settings = Some(current.clone());
                let projected = project_chart_semantics(&body, Uuid([1; 16])).unwrap();
                let (bare, resource) =
                    projected.map_or((body.clone(), None), |(body, res)| (body, Some(res)));
                let native = write_form(FormDialect::Designer, &bare).unwrap();
                let expected = chart_settings_bytes(&reference)
                    .remove(row["chart_index"].as_u64().unwrap() as usize);
                let actual = chart_settings_bytes(&native).remove(0);
                let exact = actual == expected;
                let mut decoded = read_form(FormDialect::Designer, &native).unwrap();
                if let Some(resource) = resource.as_ref() {
                    apply_chart_semantics_resource(&mut decoded, Uuid([1; 16]), resource).unwrap();
                }
                let semantic = decoded.data_attributes[0].chart_settings.as_ref() == Some(&current);
                let own = write_form(FormDialect::Designer, &reference_body).unwrap();
                let own_settings = chart_settings_bytes(&own).remove(0);
                let same_native = own_settings == expected;
                for (suffix, bytes) in [
                    ("expected", expected.as_slice()),
                    ("actual", actual.as_slice()),
                    ("own-native", own_settings.as_slice()),
                ] {
                    let path = artifact_dir.join(format!("{index:03}-{suffix}-Settings.xml"));
                    let mut file = std::fs::OpenOptions::new()
                        .create_new(true)
                        .write(true)
                        .open(path)
                        .unwrap();
                    std::io::Write::write_all(&mut file, bytes).unwrap();
                }
                let mut file = std::fs::OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .open(artifact_dir.join(format!("{index:03}-current.json")))
                    .unwrap();
                serde_json::to_writer_pretty(&mut file, &current).unwrap();
                let unchanged = sha(&std::fs::read(row["source"].as_str().unwrap()).unwrap())
                    == row["source_sha256"]
                    && sha(&std::fs::read(row["reference"].as_str().unwrap()).unwrap())
                        == row["reference_sha256"]
                    && sha(&std::fs::read(row["historical"].as_str().unwrap()).unwrap())
                        == row["historical_sha256"];
                if !(exact && semantic && same_native && unchanged) {
                    failures += 1;
                }
                rows.push(serde_json::json!({"index":index,"path_sha256":row["form_path_sha256"],"exact":exact,"semantic":semantic,"same_native":same_native,"source_unchanged":unchanged,"expected_sha256":sha(&expected),"actual_sha256":sha(&actual),"actual_bytes":actual.len(),"expected_bytes":expected.len(),"resource_sha256":resource.as_ref().map(|b|sha(b))}));
            }
        })
    });
    use std::io::Write;
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(report)
        .unwrap();
    serde_json::to_writer_pretty(&mut output,&serde_json::json!({"complete":true,"manifest_sha256":sha(&manifest),"profile":cases["profile"],"processed":27,"failures":failures,"rows":rows})).unwrap();
    output.write_all(b"\n").unwrap();
    assert_eq!(failures, 0);
}
