use formats_xml::form::{
    FormDialect, apply_chart_semantics_resource, project_chart_semantics, read_chart_sidecar,
    read_form, write_chart_sidecar, write_form,
};
use ibcmd_edt::{
    ConversionOptions, Project, ReaderLimits, edt_to_xml, read_xml_source, xml_to_edt,
};
use ibcmd_xml::source_tree::{SourceEntry, SourceTree};
use morph1c_core::ir::{
    FormBody,
    form::{ChartSettings, ChartTypedValue, ChartValue},
};
use morph1c_core::ir::{MetadataObject, NamedFormBody, ObjectKind, PropertyValue, Token, Uuid};
use morph1c_core::version::{FormatVersion, with_roundtrip_target, with_source_version};
use morph1c_pipeline::{ConvertOptions, Format, read_config, write_config};
use sha2::{Digest, Sha256};

fn source(content: &str) -> Vec<u8> {
    let content = content.replace("xsi:type=\"common:ChartLineTypeValue\"", "xmlns:common=\"http://g5.1c.ru/v8/dt/metadata/common\" xsi:type=\"common:ChartLineTypeValue\"");
    format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<chart:Chart xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:chart=\"http://g5.1c.ru/v8/dt/chart/model\" xmlns:core=\"http://g5.1c.ru/v8/dt/mcore\">{content}</chart:Chart>\r\n").into_bytes()
}
fn chart(value: &str) -> ChartSettings {
    read_chart_sidecar(&source(&format!(
        "<realDataItems><dataValue {value}</dataValue></realDataItems>"
    )))
    .unwrap()
}
fn body(settings: ChartSettings) -> FormBody {
    let mut body = read_form(FormDialect::Edt,b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<form:Form xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:form=\"http://g5.1c.ru/v8/dt/form\"><attributes><name>Diagram</name><id>1</id><valueType><types>Chart</types></valueType><view><common>true</common></view><edit><common>true</common></edit><extInfo xsi:type=\"form:ChartExtInfo\"/></attributes></form:Form>\r\n").unwrap();
    body.data_attributes[0].chart_settings = Some(settings);
    body
}
fn value(settings: &mut ChartSettings) -> &mut ChartTypedValue {
    let ChartValue::Items(items) = &mut settings
        .fields
        .iter_mut()
        .find(|(n, _)| n == "realDataItems")
        .unwrap()
        .1
    else {
        panic!()
    };
    let ChartValue::Value(value) = &mut items[0]
        .iter_mut()
        .find(|(n, _)| n == "dataValue")
        .unwrap()
        .1
    else {
        panic!()
    };
    value
}
const CHART_FORM_UUID: Uuid = Uuid([81; 16]);

fn native(settings: &ChartSettings, minor: u16) -> Vec<u8> {
    with_roundtrip_target(FormatVersion::new(2, minor), || {
        let current = body(settings.clone());
        let projected = project_chart_semantics(&current, CHART_FORM_UUID).unwrap();
        write_form(
            FormDialect::Designer,
            projected.as_ref().map_or(&current, |(body, _)| body),
        )
    })
    .unwrap()
}
fn returned(bytes: &[u8], minor: u16) -> ChartSettings {
    with_source_version(Some(FormatVersion::new(2, minor)), || {
        read_form(FormDialect::Designer, bytes)
    })
    .unwrap()
    .data_attributes
    .remove(0)
    .chart_settings
    .unwrap()
}
// Whole-model comparisons include the typed current resource. Bare returned()
// remains the native schema projection used by the wire enum assertions below.
fn complete_returned(bytes: &[u8], current: &ChartSettings, minor: u16) -> ChartSettings {
    with_roundtrip_target(FormatVersion::new(2, minor), || {
        let original = body(current.clone());
        let projection = project_chart_semantics(&original, CHART_FORM_UUID).unwrap();
        let mut returned = with_source_version(Some(FormatVersion::new(2, minor)), || {
            read_form(FormDialect::Designer, bytes)
        })
        .unwrap();
        if let Some((_, resource)) = projection {
            apply_chart_semantics_resource(&mut returned, CHART_FORM_UUID, &resource).unwrap();
        }
        returned.data_attributes.remove(0).chart_settings.unwrap()
    })
}

#[test]
fn enum_package_identity_and_current_values_do_not_follow_a_package_first_match() {
    for minor in [20, 21] {
        let mut left = chart(
            "xsi:type=\"core:EnumValue\"><value>http://g5.1c.ru/v8/dt/mcore#HorizontalAlign/Left</value>",
        );
        let mut right = chart(
            "xsi:type=\"core:EnumValue\"><value>http://g5.1c.ru/v8/dt/metadata/common#HorizontalAlign/Left</value>",
        );
        assert_ne!(
            serde_json::to_vec(&left).unwrap(),
            serde_json::to_vec(&right).unwrap()
        );
        assert_eq!(native(&left, minor), native(&right, minor));
        assert_eq!(
            read_chart_sidecar(&write_chart_sidecar(&left).unwrap()).unwrap(),
            left
        );
        let back = returned(&native(&right, minor), minor);
        assert!(
            matches!(value(&mut back.clone()),ChartTypedValue::SysEnum(Some(s)) if s=="HorizontalAlign.Left")
        );
        *value(&mut left) = ChartTypedValue::Enum {
            package_uri: "http://future.example/model".into(),
            enum_type: "FutureEnum".into(),
            literal: "CurrentLiteral".into(),
        };
        let bytes = native(&left, minor);
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("xmlns:cfg=\"http://v8.1c.ru/8.1/data/enterprise/current-config\""));
        assert!(text.contains("xsi:type=\"cfg:FutureEnum\""));
        assert!(
            matches!(value(&mut returned(&bytes,minor)),ChartTypedValue::SysEnum(Some(s)) if s=="FutureEnum.CurrentLiteral")
        );
        *value(&mut left) = ChartTypedValue::Enum {
            package_uri: "http://future.example/model".into(),
            enum_type: "FutureEnum".into(),
            literal: "Literal.With.Dot".into(),
        };
        let bytes = native(&left, minor);
        assert!(String::from_utf8_lossy(&bytes).contains(">Literal.With.Dot</"));
        assert!(
            matches!(value(&mut returned(&bytes,minor)),ChartTypedValue::SysEnum(Some(s)) if s=="FutureEnum.Literal.With.Dot")
        );
        for literal in ["Active", "Passive", "ActivePassive"] {
            let account = chart(&format!(
                "xsi:type=\"core:EnumValue\"><value>http://g5.1c.ru/v8/dt/metadata/common#AccountType/{literal}</value>"
            ));
            let mut account_back = returned(&native(&account, minor), minor);
            let ChartTypedValue::Scalar(PropertyValue::Value(spec)) = value(&mut account_back)
            else {
                panic!("expected native AccountType scalar")
            };
            assert_eq!(
                spec.kind,
                morph1c_core::ir::value::ValueScalarKind::AccountType
            );
            assert!(matches!(spec.scalar.as_deref(), Some(PropertyValue::Str(s)) if s == literal));
            assert_eq!(
                read_chart_sidecar(&write_chart_sidecar(&account).unwrap()).unwrap(),
                account
            );
        }
        let old = serde_json::to_vec(&right).unwrap();
        let generic_line = chart(
            "xsi:type=\"core:EnumValue\"><value>http://g5.1c.ru/v8/dt/metadata/common#ChartLineType/Solid</value>",
        );
        let generic_native = native(&generic_line, minor);
        assert!(
            String::from_utf8_lossy(&generic_native).contains("xsi:type=\"cfg:ChartLineType\"")
        );
        assert!(
            matches!(value(&mut returned(&generic_native,minor)),ChartTypedValue::SysEnum(Some(s)) if s=="ChartLineType.Solid")
        );
        let special_line = chart("xsi:type=\"common:ChartLineTypeValue\"><value>Solid</value>");
        assert!(
            String::from_utf8_lossy(&native(&special_line, minor))
                .contains("xsi:type=\"v8ui:ChartLineType\"")
        );
        *value(&mut right) = ChartTypedValue::SysEnum(Some("HorizontalAlign.Right".into()));
        assert_ne!(serde_json::to_vec(&right).unwrap(), old);
        assert!(String::from_utf8_lossy(&native(&right, minor)).contains(">Right</"));
    }
}
#[test]
fn expanded_native_aliases_and_unique_protocol_inverse_preserve_current_type() {
    for minor in [20, 21] {
        let original = chart(
            "xsi:type=\"core:SysEnumValue\"><value>ChartTrendLineApproximationType.Linear</value>",
        );
        let bytes = native(&original, minor);
        assert!(String::from_utf8_lossy(&bytes).contains("ChartTrendlineApproximationType"));
        assert_eq!(complete_returned(&bytes, &original, minor), original);
        // A future protocol type may equal a known reverse-map local spelling.
        // Its unknown forward type still belongs to current-config, not the known UI URI.
        let future_local = chart(
            "xsi:type=\"core:SysEnumValue\"><value>ChartTrendlineApproximationType.Current</value>",
        );
        let future_native = native(&future_local, minor);
        assert!(
            String::from_utf8_lossy(&future_native)
                .contains("xsi:type=\"cfg:ChartTrendlineApproximationType\"")
        );
        assert_eq!(
            complete_returned(&future_native, &future_local, minor),
            future_local
        );
        let original = chart("xsi:type=\"core:SysEnumValue\"><value>HorizontalAlign.Left</value>");
        let bytes = String::from_utf8(native(&original, minor)).unwrap();
        let alias = bytes.replace(
            "xsi:type=\"v8ui:HorizontalAlign\"",
            "xmlns:alias=\"http://v8.1c.ru/8.1/data/ui\" xsi:type=\"alias:HorizontalAlign\"",
        );
        assert_ne!(alias, bytes);
        assert_eq!(
            complete_returned(alias.as_bytes(), &original, minor),
            original
        );
        let unprefixed = bytes.replace(
            "xsi:type=\"v8ui:HorizontalAlign\"",
            "xmlns=\"http://v8.1c.ru/8.1/data/ui\" xsi:type=\"HorizontalAlign\"",
        );
        assert_ne!(unprefixed, bytes);
        assert_eq!(
            complete_returned(unprefixed.as_bytes(), &original, minor),
            original
        );
        assert!(
            read_form(
                FormDialect::Designer,
                unprefixed
                    .replace(
                        "xmlns=\"http://v8.1c.ru/8.1/data/ui\"",
                        "xmlns=\"http://wrong.example/ui\""
                    )
                    .as_bytes()
            )
            .is_err()
        );
        assert!(
            read_form(
                FormDialect::Designer,
                alias
                    .replace(
                        "xmlns:alias=\"http://v8.1c.ru/8.1/data/ui\"",
                        "xmlns:alias=\"http://wrong.example/ui\""
                    )
                    .as_bytes()
            )
            .is_err()
        );
        assert!(
            read_form(
                FormDialect::Designer,
                alias
                    .replace("xmlns:alias=\"http://v8.1c.ru/8.1/data/ui\" ", "")
                    .as_bytes()
            )
            .is_err()
        );
        let unicode = chart("xsi:type=\"core:SysEnumValue\"><value>НовыйТип.Значение</value>");
        assert_eq!(
            complete_returned(&native(&unicode, minor), &unicode, minor),
            unicode
        );
    }
}
#[test]
fn nullable_symbolic_values_and_sdk_truncation_remain_distinct_current_data() {
    for text in [
        None,
        Some(""),
        Some("NoDot"),
        Some("HorizontalAlign.Left"),
        Some("HorizontalAlign.Left.Extra"),
    ] {
        let mut current = chart("xsi:type=\"core:SysEnumValue\">");
        *value(&mut current) = ChartTypedValue::SysEnum(text.map(str::to_owned));
        assert_eq!(
            read_chart_sidecar(&write_chart_sidecar(&current).unwrap()).unwrap(),
            current
        );
        if text == Some("HorizontalAlign.Left.Extra") {
            for minor in [20, 21] {
                let mut back = returned(&native(&current, minor), minor);
                assert!(
                    matches!(value(&mut back),ChartTypedValue::SysEnum(Some(s)) if s=="HorizontalAlign.Left")
                );
            }
        }
    }
    for line in [
        "None",
        "Solid",
        "Dotted",
        "Dashed",
        "DashDotted",
        "DashDottedDotted",
    ] {
        let current = chart(&format!(
            "xsi:type=\"common:ChartLineTypeValue\"><value>{line}</value>"
        ));
        for minor in [20, 21] {
            assert_eq!(
                complete_returned(&native(&current, minor), &current, minor),
                current
            );
        }
    }
}
#[test]
fn period_layout_dates_and_current_order_are_typed_and_cross_dialect() {
    // Complete original Ecore StandardPeriodVariant domain, not corpus membership.
    const VARIANTS: &[&str] = &[
        "Custom",
        "Today",
        "ThisWeek",
        "ThisTenDays",
        "ThisMonth",
        "ThisQuarter",
        "ThisHalfYear",
        "ThisYear",
        "FromBeginningOfThisWeek",
        "FromBeginningOfThisTenDays",
        "FromBeginningOfThisMonth",
        "FromBeginningOfThisQuarter",
        "FromBeginningOfThisHalfYear",
        "FromBeginningOfThisYear",
        "Yesterday",
        "LastWeek",
        "LastTenDays",
        "LastMonth",
        "LastQuarter",
        "LastHalfYear",
        "LastYear",
        "LastWeekTillSameWeekDay",
        "LastTenDaysTillSameDayNumber",
        "LastMonthTillSameDate",
        "LastQuarterTillSameDate",
        "LastHalfYearTillSameDate",
        "LastYearTillSameDate",
        "Tomorrow",
        "NextWeek",
        "NextTenDays",
        "NextMonth",
        "NextQuarter",
        "NextHalfYear",
        "NextYear",
        "NextWeekTillSameWeekDay",
        "NextTenDaysTillSameDayNumber",
        "NextMonthTillSameDate",
        "NextQuarterTillSameDate",
        "NextHalfYearTillSameDate",
        "NextYearTillSameDate",
        "TillEndOfThisWeek",
        "TillEndOfThisTenDays",
        "TillEndOfThisMonth",
        "TillEndOfThisQuarter",
        "TillEndOfThisHalfYear",
        "TillEndOfThisYear",
        "Last7Days",
        "Next7Days",
        "Month",
    ];
    // Filled from the exact bound SDK domain below (49 literals).
    for variant in VARIANTS {
        let mut current = chart(&format!(
            "xsi:type=\"core:StandardPeriodValue\"><value><variant>{variant}</variant><startDate>2026-01-02T03:04:05</startDate><endDate>2026-10-02T06:07:08</endDate></value>"
        ));
        for minor in [20, 21] {
            let bytes = native(&current, minor);
            let text = String::from_utf8_lossy(&bytes);
            assert!(text.contains("<v8:startDate>2026-01-02T03:04:05</v8:startDate>"));
            assert!(!text.contains("startDate xsi:type"));
            assert_eq!(complete_returned(&bytes, &current, minor), current);
        }
        assert_eq!(
            read_chart_sidecar(&write_chart_sidecar(&current).unwrap()).unwrap(),
            current
        );
        let old = serde_json::to_vec(&current).unwrap();
        let ChartTypedValue::StandardPeriod { start, end, .. } = value(&mut current) else {
            panic!()
        };
        std::mem::swap(start, end);
        assert_ne!(serde_json::to_vec(&current).unwrap(), old);
    }
}
#[test]
fn malformed_type_uri_and_unknown_duplicate_period_fields_are_rejected() {
    for value in [
        "xsi:type=\"core:EnumValue\">",
        "xsi:type=\"core:EnumValue\"><value>missing-package</value>",
        "xsi:type=\"core:EnumValue\"><value>urn:package#Type/</value>",
        "xsi:type=\"core:SysEnumValue\"><value>A.B</value><value>A.B</value>",
        "xsi:type=\"common:ChartLineTypeValue\"><value>Invalid</value>",
        "xsi:type=\"core:StandardPeriodValue\">",
        "xsi:type=\"core:StandardPeriodValue\"><value><variant>Invalid</variant></value>",
        "xsi:type=\"core:StandardPeriodValue\"><value><variant>Custom</variant><variant>Today</variant></value>",
        "xsi:type=\"core:StandardPeriodValue\"><value><startDate/></value>",
        "xsi:type=\"core:StandardPeriodValue\"><value><startDate xsi:type=\"xs:dateTime\">2026-01-01T00:00:00</startDate></value>",
        "xsi:type=\"core:StandardPeriodValue\"><value><unknown/></value>",
    ] {
        assert!(
            read_chart_sidecar(&source(&format!(
                "<realDataItems><dataValue {value}</dataValue></realDataItems>"
            )))
            .is_err()
        );
    }
    let current = chart("xsi:type=\"core:SysEnumValue\"><value>HorizontalAlign.Left</value>");
    let bytes = native(&current, 21);
    let text = String::from_utf8(bytes).unwrap();
    let invalid = text.replace("http://v8.1c.ru/8.1/data/ui", "http://wrong.example/ui");
    assert!(read_form(FormDialect::Designer, invalid.as_bytes()).is_err());
    let account = chart(
        "xsi:type=\"core:EnumValue\"><value>http://g5.1c.ru/v8/dt/metadata/common#AccountType/Active</value>",
    );
    let invalid_account = String::from_utf8(native(&account, 21))
        .unwrap()
        .replace(">Active</", ">InvalidAccountType</");
    let error = read_form(FormDialect::Designer, invalid_account.as_bytes()).unwrap_err();
    assert!(
        matches!(error, formats_xml::form::FormError::Frame(message) if message.contains("AccountType literal"))
    );
    let period =
        chart("xsi:type=\"core:StandardPeriodValue\"><value><variant>Today</variant></value>");
    let native = String::from_utf8(native(&period, 21)).unwrap();
    assert!(
        read_form(
            FormDialect::Designer,
            native
                .replace("v8:StandardPeriodVariant", "v8:UnknownPeriodVariant")
                .as_bytes()
        )
        .is_err()
    );
}

#[test]
fn public_current_enum_identity_and_null_members_survive_both_cycles_and_forgery_is_rejected() {
    for minor in [20, 21] {
        let options = ConversionOptions {
            edt_version: "2025.2.3".into(),
            xml_dialect: format!("2.{minor}"),
            runtime_version: Some(if minor == 20 { "8.3.27" } else { "8.5.1" }.into()),
        };
        let fixture = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/subsystem-ci/src"
        ));
        let mut cfg = read_config(Format::Designer, fixture, &ConvertOptions::default())
            .unwrap()
            .0;
        let mut obj = MetadataObject::new(
            ObjectKind::new("CommonForm"),
            "ChartEnumPeriod",
            Uuid([64; 16]),
        );
        let field = morph1c_core::spec::registry::spec_for("CommonForm")
            .unwrap()
            .fields()
            .iter()
            .find(|f| f.name == "formType")
            .unwrap()
            .id;
        obj.properties
            .push((field, PropertyValue::Enum(Token::new("Managed"))));
        obj.form_bodies.push(NamedFormBody{name:"ChartEnumPeriod".into(),body:body(chart("xsi:type=\"core:ValueList\"><values xsi:type=\"core:EnumValue\"><value>http://g5.1c.ru/v8/dt/mcore#HorizontalAlign/Left</value></values><values xsi:type=\"core:SysEnumValue\"/><values xsi:type=\"core:ReferenceValue\"/><values xsi:type=\"core:SysEnumValue\"><value>HorizontalAlign.Right.Extra</value></values><values xsi:type=\"core:EnumValue\"><value>http://future.example/model#FutureEnum/Literal.With.Dot</value></values><values xsi:type=\"core:SysEnumValue\"><value>HorizontalAlign.</value></values><values xsi:type=\"core:SysEnumValue\"><value>A:B.Literal</value></values><values xsi:type=\"core:EnumValue\"><value>http://g5.1c.ru/v8/dt/metadata/common#ChartLineType/Solid</value></values><values xsi:type=\"core:EnumValue\"><value>http://g5.1c.ru/v8/dt/metadata/common#AccountType/Active</value></values><values xsi:type=\"core:EnumValue\"><value>http://g5.1c.ru/v8/dt/metadata/common#AccountType/Passive</value></values><values xsi:type=\"core:EnumValue\"><value>http://g5.1c.ru/v8/dt/metadata/common#AccountType/ActivePassive</value></values>")),ordinary_body:None,module:None,help:vec![],help_resources:vec![]});
        cfg.objects.push(obj);
        let dir = tempfile::tempdir().unwrap();
        with_roundtrip_target(FormatVersion::new(2, minor), || {
            write_config(Format::Designer, &cfg, dir.path())
        })
        .unwrap();
        let root = dir.path().join("Configuration.xml");
        let content = String::from_utf8(std::fs::read(&root).unwrap())
            .unwrap()
            .replace(
                "</Language>",
                "</Language>\r\n\t\t\t<CommonForm>ChartEnumPeriod</CommonForm>",
            );
        std::fs::write(root, content).unwrap();
        let original = read_xml_source(dir.path(), ReaderLimits::default()).unwrap();
        let generated = xml_to_edt(&original, &options).unwrap().tree;
        assert_eq!(
            edt_to_xml(&Project::from_tree(generated.clone()).unwrap(), &options)
                .unwrap()
                .tree,
            original
        );
        let path = "src/CommonForms/ChartEnumPeriod/Attributes/Diagram/ExtInfo/Chart.chart";
        let bytes = generated
            .entries()
            .iter()
            .find(|e| e.path().as_str() == path)
            .unwrap()
            .bytes();
        let mut current = read_chart_sidecar(bytes).unwrap();
        let original_chart = cfg
            .objects
            .iter()
            .find(|o| o.name == "ChartEnumPeriod")
            .unwrap()
            .form_bodies[0]
            .body
            .data_attributes[0]
            .chart_settings
            .as_ref()
            .unwrap();
        assert_eq!(
            serde_json::to_vec(&current).unwrap(),
            serde_json::to_vec(original_chart).unwrap()
        );
        let stripped = SourceTree::new(
            generated
                .entries()
                .iter()
                .filter(|e| !e.path().as_str().starts_with(".ibcmd-provenance/"))
                .cloned()
                .collect(),
        )
        .unwrap();
        let plain = edt_to_xml(&Project::from_tree(stripped).unwrap(), &options)
            .unwrap()
            .tree;
        let twice = xml_to_edt(&plain, &options).unwrap().tree;
        let twice = read_chart_sidecar(
            twice
                .entries()
                .iter()
                .find(|e| e.path().as_str() == path)
                .unwrap()
                .bytes(),
        )
        .unwrap();
        assert_eq!(
            serde_json::to_vec(&twice).unwrap(),
            serde_json::to_vec(original_chart).unwrap()
        );
        *value(&mut current) = ChartTypedValue::Enum {
            package_uri: "http://g5.1c.ru/v8/dt/metadata/common".into(),
            enum_type: "HorizontalAlign".into(),
            literal: "Right".into(),
        };
        let edited = write_chart_sidecar(&current).unwrap();
        let mut manifest: serde_json::Value = serde_json::from_slice(
            generated
                .entries()
                .iter()
                .find(|e| e.path().as_str() == ".ibcmd-provenance/manifest.json")
                .unwrap()
                .bytes(),
        )
        .unwrap();
        manifest["generated"][path] = format!("{:x}", Sha256::digest(&edited)).into();
        let alter = |strip: bool| {
            SourceTree::new(
                generated
                    .entries()
                    .iter()
                    .filter(|e| !strip || !e.path().as_str().starts_with(".ibcmd-provenance/"))
                    .map(|e| {
                        SourceEntry::from_bytes(
                            e.path().clone(),
                            if e.path().as_str() == path {
                                edited.clone()
                            } else if e.path().as_str() == ".ibcmd-provenance/manifest.json" {
                                serde_json::to_vec(&manifest).unwrap()
                            } else {
                                e.bytes().to_vec()
                            },
                        )
                        .unwrap()
                    })
                    .collect(),
            )
            .unwrap()
        };
        assert!(edt_to_xml(&Project::from_tree(alter(false)).unwrap(), &options).is_err());
        let current_disk = tempfile::tempdir().unwrap();
        let project_path = current_disk.path().join("project");
        ibcmd_xml::source_tree::publish_new(&alter(true), &project_path).unwrap();
        let model = read_config(
            Format::Edt,
            &project_path.join("src"),
            &ConvertOptions::default().with_target_version(FormatVersion::new(2, minor)),
        )
        .unwrap()
        .0;
        let view = morph1c_core::ir::semantic_view::ConfigurationSemanticView {
            configuration: &model,
            template_body: morph1c_pipeline::dcs_template_semantic_body,
        };
        let mut fully_forged = manifest.clone();
        fully_forged["semantics"] =
            format!("{:x}", Sha256::digest(serde_json::to_vec(&view).unwrap())).into();
        let fully_forged = SourceTree::new(
            alter(false)
                .entries()
                .iter()
                .map(|e| {
                    if e.path().as_str() == ".ibcmd-provenance/manifest.json" {
                        SourceEntry::from_bytes(
                            e.path().clone(),
                            serde_json::to_vec(&fully_forged).unwrap(),
                        )
                        .unwrap()
                    } else {
                        e.clone()
                    }
                })
                .collect(),
        )
        .unwrap();
        assert!(edt_to_xml(&Project::from_tree(fully_forged).unwrap(), &options).is_err());
        let changed = edt_to_xml(&Project::from_tree(alter(true)).unwrap(), &options).unwrap();
        let changed_directory = tempfile::tempdir().unwrap();
        for item in changed.tree.entries() {
            let path = changed_directory.path().join(item.path().as_str());
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, item.bytes()).unwrap();
        }
        let native = changed
            .tree
            .entries()
            .iter()
            .find(|e| e.path().as_str() == "CommonForms/ChartEnumPeriod/Ext/Form.xml")
            .unwrap();
        let mut restored = with_source_version(Some(FormatVersion::new(2, minor)), || {
            read_config(
                Format::Designer,
                changed_directory.path(),
                &ConvertOptions::default(),
            )
        })
        .unwrap()
        .0;
        assert_eq!(
            restored
                .objects
                .iter_mut()
                .find(|o| o.name == "ChartEnumPeriod")
                .unwrap()
                .form_bodies[0]
                .body
                .data_attributes[0]
                .chart_settings
                .as_ref()
                .unwrap(),
            &current
        );
        assert!(String::from_utf8_lossy(native.bytes()).contains(">Right</"));
        assert_ne!(changed.tree, original);
    }
}

#[test]
fn design_and_value_records_preserve_dotted_native_literals_and_reject_stale_counterparts() {
    for minor in [20, 21] {
        with_roundtrip_target(FormatVersion::new(2, minor), || {
            let current = chart(
                "xsi:type=\"core:EnumValue\"><value>http://future.example/model#FutureEnum/Literal.With.Dot</value>",
            );
            let original = body(current.clone());
            let (projected, resource) = project_chart_semantics(&original, CHART_FORM_UUID)
                .unwrap()
                .unwrap();
            let rows: serde_json::Value = serde_json::from_slice(&resource).unwrap();
            assert_eq!(rows["designs"].as_array().unwrap().len(), 1);
            assert_eq!(rows["values"].as_array().unwrap().len(), 1);
            let bytes = write_form(FormDialect::Designer, &projected).unwrap();
            let mut counterpart = with_source_version(Some(FormatVersion::new(2, minor)), || {
                read_form(FormDialect::Designer, &bytes)
            })
            .unwrap();
            assert!(matches!(
                value(counterpart.data_attributes[0].chart_settings.as_mut().unwrap()),
                ChartTypedValue::SysEnum(Some(s)) if s == "FutureEnum.Literal.With.Dot"
            ));
            let mut changed = counterpart.clone();
            *value(changed.data_attributes[0].chart_settings.as_mut().unwrap()) =
                ChartTypedValue::SysEnum(Some("FutureEnum.CurrentLiteral".into()));
            let before = serde_json::to_vec(&changed).unwrap();
            let error = apply_chart_semantics_resource(&mut changed, CHART_FORM_UUID, &resource)
                .unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("stale/conflicting current native design binding")
            );
            assert_eq!(serde_json::to_vec(&changed).unwrap(), before);
            apply_chart_semantics_resource(&mut counterpart, CHART_FORM_UUID, &resource).unwrap();
            assert_eq!(
                counterpart.data_attributes[0]
                    .chart_settings
                    .as_ref()
                    .unwrap(),
                &current
            );
        });
    }
}
