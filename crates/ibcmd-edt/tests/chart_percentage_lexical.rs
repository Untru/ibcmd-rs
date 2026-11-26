//! Scoped funnel percentage spelling; the EFloat value and transport remain
//! current typed data. Presence changes are deliberately outside this patch.
use formats_xml::form::{
    FormDialect, apply_chart_semantics_resource, project_chart_semantics, read_chart_sidecar,
    read_form, write_chart_sidecar, write_form,
};
use morph1c_core::{
    ir::{
        FormBody, Uuid,
        form::{ChartSettings, ChartValue},
    },
    version::{FormatVersion, with_roundtrip_target, with_source_version},
};
use sha2::{Digest, Sha256};

const UUID: Uuid = Uuid([37; 16]);
const PERCENT_FIELDS: [&str; 3] = [
    "funnelNeckHeightPercent",
    "funnelNeckWidthPercent",
    "funnelGapSumPercent",
];

fn source(content: &str) -> Vec<u8> {
    format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<chart:Chart xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:chart=\"http://g5.1c.ru/v8/dt/chart/model\" xmlns:core=\"http://g5.1c.ru/v8/dt/mcore\">{content}</chart:Chart>\r\n").into_bytes()
}
fn form(chart: ChartSettings, version: FormatVersion) -> FormBody {
    let kind = &chart.kind;
    let source = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<form:Form xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:form=\"http://g5.1c.ru/v8/dt/form\"><attributes><name>Diagram</name><id>1</id><valueType><types>{kind}</types></valueType><view><common>true</common></view><edit><common>true</common></edit><extInfo xsi:type=\"form:{kind}ExtInfo\"/></attributes></form:Form>\r\n"
    );
    let mut body = read(source.as_bytes(), FormDialect::Edt, version);
    body.data_attributes[0].chart_settings = Some(chart);
    body
}
fn write(body: &FormBody, dialect: FormDialect, version: FormatVersion) -> Vec<u8> {
    with_roundtrip_target(version, || write_form(dialect, body)).unwrap()
}
fn read(bytes: &[u8], dialect: FormDialect, version: FormatVersion) -> FormBody {
    with_source_version(Some(version), || read_form(dialect, bytes)).unwrap()
}
fn semantics(body: &FormBody) -> Vec<u8> {
    serde_json::to_vec(body).unwrap()
}
fn through_native(body: &FormBody, version: FormatVersion) -> (Vec<u8>, FormBody) {
    let before = semantics(body);
    let (native, resource) = with_roundtrip_target(version, || {
        let projected = project_chart_semantics(body, UUID).unwrap();
        let native = write_form(
            FormDialect::Designer,
            projected.as_ref().map_or(body, |(current, _)| current),
        )
        .unwrap();
        (native, projected.map(|(_, resource)| resource))
    });
    assert_eq!(
        semantics(body),
        before,
        "projection cannot edit authored IR"
    );
    let mut returned = read(&native, FormDialect::Designer, version);
    if let Some(resource) = resource {
        apply_chart_semantics_resource(&mut returned, UUID, &resource).unwrap();
    }
    assert_eq!(
        semantics(&returned),
        before,
        "complete typed roundtrip without normalization"
    );
    (native, returned)
}
fn percent_payload(bytes: &[u8]) -> Vec<(String, String)> {
    fn walk(node: &formats_xml::Element, result: &mut Vec<(String, String)>) {
        if PERCENT_FIELDS.contains(&node.local.as_str()) {
            result.push((node.local.clone(), node.text.clone()));
        }
        for child in &node.children {
            walk(child, result);
        }
    }
    let doc = formats_xml::parse(bytes).unwrap();
    let mut result = Vec::new();
    walk(&doc.root, &mut result);
    result
}
fn set_percent(chart: &mut ChartSettings, field: &str, value: &str) {
    chart
        .fields
        .iter_mut()
        .find(|(name, _)| name == field)
        .unwrap()
        .1 = ChartValue::Int(value.into());
}

#[test]
fn integral_funnel_percentages_use_scoped_native_spelling_and_keep_edited_values() {
    for minor in [20, 21] {
        let version = FormatVersion::new(2, minor);
        let chart = read_chart_sidecar(&source("<funnelNeckHeightPercent>10.0</funnelNeckHeightPercent><funnelNeckWidthPercent>10.0</funnelNeckWidthPercent><funnelGapSumPercent>3.0</funnelGapSumPercent>")).unwrap();
        let mut body = form(chart, version);
        let (native, returned) = through_native(&body, version);
        assert_eq!(
            percent_payload(&native),
            PERCENT_FIELDS
                .into_iter()
                .zip(["10", "10", "3"])
                .map(|(name, value)| (name.into(), value.into()))
                .collect::<Vec<_>>()
        );
        let chart = body.data_attributes[0].chart_settings.as_mut().unwrap();
        set_percent(chart, PERCENT_FIELDS[0], "25");
        set_percent(chart, PERCENT_FIELDS[1], "12.5");
        set_percent(chart, PERCENT_FIELDS[2], "-3");
        let (edited, current) = through_native(&body, version);
        assert_eq!(
            percent_payload(&edited),
            PERCENT_FIELDS
                .into_iter()
                .zip(["25", "12.5", "-3"])
                .map(|(name, value)| (name.into(), value.into()))
                .collect::<Vec<_>>()
        );
        assert_ne!(semantics(&current), semantics(&returned));
        let chart = current.data_attributes[0].chart_settings.as_ref().unwrap();
        assert_eq!(
            read_chart_sidecar(&write_chart_sidecar(chart).unwrap()).unwrap(),
            *chart
        );
        // EDT retains its independently defined EFloat lexical convention.
        assert!(
            String::from_utf8(write_chart_sidecar(chart).unwrap())
                .unwrap()
                .contains("<funnelNeckHeightPercent>25.0</funnelNeckHeightPercent>")
        );
    }
}

#[test]
fn fractional_scientific_signed_zero_and_nonfinite_values_preserve_typed_domain() {
    for minor in [20, 21] {
        let version = FormatVersion::new(2, minor);
        for (input, native_text) in [
            ("0.0", "0"),
            ("-0.0", "-0"),
            ("1.25", "1.25"),
            ("1e23", "1.0E23"),
            ("1.4e-45", "1.4E-45"),
            ("Infinity", "Infinity"),
            ("-Infinity", "-Infinity"),
            ("NaN", "NaN"),
        ] {
            let content = PERCENT_FIELDS
                .iter()
                .map(|name| format!("<{name}>{input}</{name}>"))
                .collect::<String>();
            let body = form(read_chart_sidecar(&source(&content)).unwrap(), version);
            let (native, _) = through_native(&body, version);
            assert_eq!(
                percent_payload(&native),
                PERCENT_FIELDS
                    .into_iter()
                    .map(|name| (name.to_owned(), native_text.to_owned()))
                    .collect::<Vec<_>>(),
                "all three percentage fields must survive for {input}"
            );
        }
    }
}

#[test]
fn native_fraction_convention_replays_current_values_without_replacing_their_identity() {
    for minor in [20, 21] {
        let version = FormatVersion::new(2, minor);
        let body = form(
            read_chart_sidecar(&source(
                "<funnelNeckHeightPercent>10.0</funnelNeckHeightPercent>",
            ))
            .unwrap(),
            version,
        );
        let (native, _) = through_native(&body, version);
        let text = String::from_utf8(native).unwrap();
        let source = text.replace(
            "<d4p1:funnelNeckHeightPercent>10</d4p1:funnelNeckHeightPercent>",
            "<d4p1:funnelNeckHeightPercent>10.00</d4p1:funnelNeckHeightPercent>",
        );
        assert_ne!(source, text);
        let mut parsed = read(source.as_bytes(), FormDialect::Designer, version);
        assert_eq!(
            write(&parsed, FormDialect::Designer, version),
            source.as_bytes()
        );
        set_percent(
            parsed.data_attributes[0].chart_settings.as_mut().unwrap(),
            PERCENT_FIELDS[0],
            "25",
        );
        let before = semantics(&parsed);
        let edited = write(&parsed, FormDialect::Designer, version);
        assert_eq!(percent_payload(&edited)[0].1, "25.00");
        assert_eq!(
            semantics(&read(&edited, FormDialect::Designer, version)),
            before
        );
        assert_eq!(semantics(&parsed), before);
    }
}

#[test]
#[ignore = "requires immutable SHA-bound UH Chart and nested Gantt Chart paired sources on F"]
fn genuine_sdk_percentages_match_two_independent_authored_chart_pairs() {
    let lab = std::path::PathBuf::from(std::env::var_os("IBCMD_EDT_LAB").unwrap());
    let witnesses = [
        (
            "Catalogs/ВариантыАнализаЦелевыхПоказателей/Forms/НастройкаДемоДанных",
            "Attributes/Диаграмма/ExtInfo/Chart.chart",
            "c7f04d03001749ef172142940d1163008ab2f5518a980d9fb997e9cea6ad32ae",
            "fe0c12110c7eca209b44d37a779c53d3865cabcf18e6fc75582cbf012190410b",
        ),
        (
            "DataProcessors/ДиаграммаГантаОперации/Forms/Форма",
            "Attributes/ДиаграммаГанта/ExtInfo/GanttChart.chart",
            "3f344c496dbdcf90c8818de2afd95371169b938dfd3132f269b09b85d863f1bf",
            "1d802094b226cc04d50bc47b4376a4b7a1f0fd9060a93f25e81ae515853e6116",
        ),
    ];
    let sha = |bytes: &[u8]| format!("{:x}", Sha256::digest(bytes));
    for (form_path, sidecar, source_sha, sdk_sha) in witnesses {
        let input = lab
            .join("oracle-uha83-r1/authentic-workspace/OracleConfiguration/src")
            .join(form_path)
            .join(sidecar);
        let expected = lab
            .join("native-reference-uha83-r1/native-xml")
            .join(form_path)
            .join("Ext/Form.xml");
        let source = std::fs::read(&input).unwrap();
        let sdk = std::fs::read(&expected).unwrap();
        assert_eq!(sha(&source), source_sha);
        assert_eq!(sha(&sdk), sdk_sha);
        let chart = read_chart_sidecar(&source).unwrap();
        for minor in [20, 21] {
            let version = FormatVersion::new(2, minor);
            let body = form(chart.clone(), version);
            let (native, _) = through_native(&body, version);
            // Only the repaired scalar contract is accepted here. Other chart
            // field-presence differences remain in the full census report.
            assert_eq!(
                percent_payload(&native),
                percent_payload(&sdk),
                "{form_path}"
            );
        }
        assert_eq!(sha(&std::fs::read(input).unwrap()), source_sha);
        assert_eq!(sha(&std::fs::read(expected).unwrap()), sdk_sha);
    }
}
