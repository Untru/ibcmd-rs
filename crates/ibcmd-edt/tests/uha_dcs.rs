//! Synthetic reduced witnesses for UH DCS schema; genuine full pairs stay in F lab.
use formats_xml::form::{
    FormDialect, read_form, read_list_settings_dcss, write_form, write_list_settings_dcss,
};
use morph1c_core::ir::{DcsCorValue, DcsItem, DcsListSettings, DcsParamValue, DcsSettingsGroup};

fn form(content: &str, extra_ns: &str) -> Vec<u8> {
    format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<form:Form xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:core=\"http://g5.1c.ru/v8/dt/mcore\" {extra_ns}xmlns:form=\"http://g5.1c.ru/v8/dt/form\" xmlns:schema=\"http://g5.1c.ru/v8/dt/data-composition-system/schema\" xmlns:settings=\"http://g5.1c.ru/v8/dt/data-composition-system/settings\">\r\n<attributes><name>List</name><valueType><types>DynamicList</types></valueType><view><common>true</common></view><edit><common>true</common></edit><extInfo xsi:type=\"form:DynamicListExtInfo\">{content}</extInfo></attributes>\r\n</form:Form>\r\n").into_bytes()
}
fn dl(body: &morph1c_core::ir::FormBody) -> &morph1c_core::ir::DynamicListAttrExt {
    body.data_attributes[0].dynamic_list.as_ref().unwrap()
}
const SCHEMA: &str = r#"<calculatedFields><dataPath>OnlyInStock</dataPath><expression>False</expression><useRestriction><group>true</group><order>true</order></useRestriction><availableValues><value xsi:type="core:UndefinedValue"/></availableValues><availableValues><value xsi:type="core:BooleanValue"><value>true</value></value></availableValues><valueType><types>Boolean</types></valueType></calculatedFields><fields xsi:type="schema:DataCompositionSchemaDataSetField"><dataPath>ShipmentNumber</dataPath><field>ShipmentNumber</field><orderExpressions><expression>STRINGLENGTH(ShipmentNumber)</expression></orderExpressions><orderExpressions><expression>ShipmentNumber</expression></orderExpressions></fields><parameters><name>Rules</name><values xsi:type="core:BooleanValue"/><useRestriction>true</useRestriction><expression>GetOption(&quot;Rules&quot;)</expression><availableAsField>false</availableAsField></parameters><parameters><name>Contract</name><values xsi:type="core:StringValue"><value/></values><useRestriction>true</useRestriction><use>Always</use></parameters>"#;
#[test]
fn ordered_schema_values_and_parameter_expression_survive_both_dialects() {
    let body = read_form(FormDialect::Edt, &form(SCHEMA, "")).unwrap();
    let list = dl(&body);
    assert_eq!(
        list.calculated_fields[0].available_values[0].value,
        DcsParamValue::Undefined
    );
    assert_eq!(
        list.calculated_fields[0].available_values[1].value,
        DcsParamValue::Boolean("true".into())
    );
    assert_eq!(
        list.fields[0]
            .order_expressions
            .iter()
            .map(|o| o.expression.as_str())
            .collect::<Vec<_>>(),
        ["STRINGLENGTH(ShipmentNumber)", "ShipmentNumber"]
    );
    assert_eq!(
        list.parameters[0].expression.as_deref(),
        Some("GetOption(\"Rules\")")
    );
    assert_eq!(
        list.parameters[1].usage,
        Some(morph1c_core::ir::form::DcsParameterUse::Always)
    );
    for dialect in [FormDialect::Edt, FormDialect::Designer] {
        let bytes = write_form(dialect, &body).unwrap();
        let decoded = read_form(dialect, &bytes).unwrap();
        assert_eq!(dl(&decoded).fields, list.fields);
        assert_eq!(dl(&decoded).calculated_fields, list.calculated_fields);
        assert_parameter_semantics(&dl(&decoded).parameters, &list.parameters);
        let text = String::from_utf8(bytes).unwrap();
        if dialect == FormDialect::Edt {
            let cf = text
                .split("<calculatedFields>")
                .nth(1)
                .unwrap()
                .split("</calculatedFields>")
                .next()
                .unwrap();
            assert!(cf.find("<availableValues>").unwrap() < cf.find("<valueType>").unwrap());
        }
    }
}
#[test]
fn design_time_symbolic_reference_and_rgb_color_have_typed_cross_encoding() {
    let content = r#"<parameters><name>Organization</name><values xsi:type="core_1:DesignTimeValueValue"><value><value>Справочник.Организации.ПустаяСсылка</value></value></values><use>Always</use></parameters><fields xsi:type="schema:DataCompositionSchemaDataSetField"><dataPath>Status</dataPath><field>Status</field><appearance><items xsi:type="settings:SettingsParameterValue"><parameter><value>TextColor</value></parameter><values xsi:type="core:ColorValue"><value xsi:type="core:ColorDef"><red>28</red><green>85</green><blue>174</blue></value></values></items></appearance></fields>"#;
    let ns = "xmlns:core_1=\"http://g5.1c.ru/v8/dt/data-composition-system/core\" ";
    let body = read_form(FormDialect::Edt, &form(content, ns)).unwrap();
    assert_eq!(
        dl(&body).parameters[0].value,
        Some(DcsParamValue::DesignTimeValue(
            "Справочник.Организации.ПустаяСсылка".into()
        ))
    );
    assert_eq!(
        dl(&body).fields[0].appearance[0].value,
        Some(DcsCorValue::Color("#1C55AE".into()))
    );
    for dialect in [FormDialect::Edt, FormDialect::Designer] {
        let encoded = write_form(dialect, &body).unwrap();
        let decoded = read_form(dialect, &encoded).unwrap();
        assert_parameter_semantics(&dl(&decoded).parameters, &dl(&body).parameters);
        assert_eq!(dl(&decoded).fields, dl(&body).fields);
    }
    assert!(read_form(FormDialect::Edt, &form(content, "")).is_err());
    assert!(read_form(FormDialect::Edt, &form(SCHEMA, ns)).is_err());
    assert!(
        read_form(
            FormDialect::Edt,
            &form(&content.replace("<red>28</red>", "<red>256</red>"), ns)
        )
        .is_err()
    );
    assert!(
        read_form(
            FormDialect::Edt,
            &form(
                &content.replace("<blue>174</blue>", "<blue>174</blue><alpha>50</alpha>"),
                ns
            )
        )
        .is_err()
    );
    assert!(
        read_form(
            FormDialect::Edt,
            &form(content, &ns.replace("system/core", "system/unknown"))
        )
        .is_err()
    );
}
#[test]
fn schema_unknown_nodes_values_usage_and_payloads_fail_closed() {
    for source in [
        SCHEMA.replace("<use>Always</use>", "<use>Sometimes</use>"),
        SCHEMA.replace(
            "<expression>ShipmentNumber</expression>",
            "<expression>ShipmentNumber</expression><unknown>true</unknown>",
        ),
        SCHEMA.replace("<value>true</value>", "<value>1</value>"),
        SCHEMA.replace("core:UndefinedValue", "core:UnknownValue"),
        SCHEMA.replace(
            "<expression>ShipmentNumber</expression>",
            "<expression>ShipmentNumber</expression><orderType>Sideways</orderType>",
        ),
        SCHEMA.replace(
            "<expression>ShipmentNumber</expression>",
            "<expression>ShipmentNumber</expression><autoOrder>1</autoOrder>",
        ),
        SCHEMA.replace(
            "<availableAsField>false</availableAsField>",
            "<availableAsField>false</availableAsField><invented/>",
        ),
    ] {
        assert!(
            read_form(FormDialect::Edt, &form(&source, "")).is_err(),
            "{source}"
        );
    }
}
#[test]
fn filter_group_user_identity_survives_native_sidecar_and_section() {
    let item = DcsItem::FilterGroup {
        used: None,
        group_type: "AndGroup".into(),
        items: vec![],
        presentation: None,
        view_mode: None,
        user_setting_id: Some("d44f3ae9-0052-4911-8d59-1c14f5d60cf8".into()),
    };
    let settings = DcsListSettings {
        filter: Some(DcsSettingsGroup {
            items: vec![item],
            view_mode: None,
            user_setting_id: None,
            user_setting_presentation: None,
        }),
        data_parameters: vec![morph1c_core::ir::DcsSettingsParameterValue {
            used: Some(false),
            parameter: "Organization".into(),
            value: Some(DcsCorValue::DesignTimeValue(
                "Справочник.Организации.ПустаяСсылка".into(),
            )),
            user_setting_id: Some("128a1bf8-cffa-4062-b3a2-c3a4379a03eb".into()),
        }],
        ..DcsListSettings::default()
    };
    let bytes = write_list_settings_dcss(&settings);
    assert_eq!(read_list_settings_dcss(&bytes).unwrap(), settings);
    let section = formats_xml::form::write_list_settings_section(
        formats_xml::form::DcsSettingsSection::Filter,
        Some(&settings),
        &|_| None,
    )
    .unwrap();
    assert!(
        String::from_utf8(section)
            .unwrap()
            .contains("d44f3ae9-0052-4911-8d59-1c14f5d60cf8")
    );
    let bad = String::from_utf8(bytes)
        .unwrap()
        .replace("</userSettingID>", "</userSettingID><unknown/>");
    assert!(read_list_settings_dcss(bad.as_bytes()).is_err());
}

#[test]
#[ignore = "requires authentic UH DCS paired-witnesses.json from installed EDT and SDK"]
fn authentic_uha_dcs_witnesses_decode_and_reencode() {
    let evidence =
        std::path::PathBuf::from(std::env::var_os("IBCMD_DCS_PAIRED_WITNESSES").unwrap());
    let report: serde_json::Value =
        serde_json::from_slice(&std::fs::read(evidence).unwrap()).unwrap();
    let mut complete = 0;
    let mut failures = Vec::new();
    for case in report["cases"].as_array().unwrap() {
        for (name, dialect) in [
            ("edt", FormDialect::Edt),
            ("native-original", FormDialect::Designer),
            ("native-post-edt", FormDialect::Designer),
        ] {
            let path =
                std::path::PathBuf::from(case["paired"][name]["copy_path"].as_str().unwrap());
            let raw = std::fs::read(&path).unwrap();
            assert!(raw.len() <= 32 * 1024 * 1024);
            use sha2::{Digest, Sha256};
            assert_eq!(
                format!("{:x}", Sha256::digest(&raw)),
                case["paired"][name]["sha256"].as_str().unwrap()
            );
            match read_form(dialect, &raw) {
                Ok(body) => {
                    match write_form(dialect, &body).and_then(|bytes| read_form(dialect, &bytes)) {
                        Ok(decoded) => {
                            assert_eq!(
                                body.data_attributes.len(),
                                decoded.data_attributes.len(),
                                "{} {name}: attribute count",
                                path.display()
                            );
                            for (before, after) in
                                body.data_attributes.iter().zip(&decoded.data_attributes)
                            {
                                assert_eq!(
                                    before.name,
                                    after.name,
                                    "{} {name}: attribute identity/order",
                                    path.display()
                                );
                                assert_eq!(
                                    before.dynamic_list.is_some(),
                                    after.dynamic_list.is_some(),
                                    "{} {name}: dynamic-list presence",
                                    path.display()
                                );
                                if let (Some(before), Some(after)) =
                                    (&before.dynamic_list, &after.dynamic_list)
                                {
                                    assert_eq!(
                                        before.fields,
                                        after.fields,
                                        "{} {name}",
                                        path.display()
                                    );
                                    assert_eq!(
                                        before.calculated_fields,
                                        after.calculated_fields,
                                        "{} {name}",
                                        path.display()
                                    );
                                    assert_parameter_semantics(
                                        &before.parameters,
                                        &after.parameters,
                                    );
                                }
                            }
                            complete += 1;
                        }
                        Err(error) => {
                            failures.push(format!("{} {name} write/read: {error}", path.display()))
                        }
                    }
                }
                Err(error) => failures.push(format!("{} {name} read: {error}", path.display())),
            }
        }
    }
    eprintln!("genuine DCS pairs completed={complete}, failures={failures:#?}");
    assert!(failures.is_empty());
    assert_eq!(complete, report["cases"].as_array().unwrap().len() * 3);
}
#[test]
fn repeated_parameter_values_remain_ordered_and_unknown_values_fail() {
    let ns = "xmlns:core_1=\"http://g5.1c.ru/v8/dt/data-composition-system/core\" ";
    let content = r#"<parameters><name>TaxModes</name><values xsi:type="core_1:DesignTimeValueValue"><value><value>Enum.Tax.Export</value></value></values><values xsi:type="core_1:DesignTimeValueValue"><value><value>Enum.Tax.Raw</value></value></values><values xsi:type="core_1:DesignTimeValueValue"><value><value>Enum.Tax.Other</value></value></values><valueListAllowed>true</valueListAllowed></parameters>"#;
    let body = read_form(FormDialect::Edt, &form(content, ns)).unwrap();
    let param = &dl(&body).parameters[0];
    assert_eq!(
        param.value,
        Some(DcsParamValue::DesignTimeValue("Enum.Tax.Export".into()))
    );
    assert_eq!(
        param.additional_values,
        vec![
            DcsParamValue::DesignTimeValue("Enum.Tax.Raw".into()),
            DcsParamValue::DesignTimeValue("Enum.Tax.Other".into())
        ]
    );
    for dialect in [FormDialect::Edt, FormDialect::Designer] {
        let bytes = write_form(dialect, &body).unwrap();
        let decoded = read_form(dialect, &bytes).unwrap();
        assert_parameter_semantics(&dl(&decoded).parameters, &dl(&body).parameters);
    }
    assert!(
        read_form(
            FormDialect::Edt,
            &form(
                &content.replacen("Enum.Tax.Raw</value>", "Enum.Tax.Raw</value><unknown/>", 1),
                ns
            )
        )
        .is_err()
    );
    assert!(
        read_form(
            FormDialect::Edt,
            &form(
                &content.replacen("core_1:DesignTimeValueValue", "core_1:UnsupportedValue", 1),
                ns
            )
        )
        .is_err()
    );
}
#[test]
#[ignore = "requires genuine read-only UH case 8 witness"]
fn genuine_repeated_parameter_values_survive_both_formats() {
    let source = std::env::var("IBCMD_DCS_REPEATED_WITNESS").unwrap();
    let body = read_form(FormDialect::Edt, &std::fs::read(source).unwrap()).unwrap();
    let original = body
        .data_attributes
        .iter()
        .filter_map(|a| a.dynamic_list.as_ref())
        .flat_map(|d| d.parameters.iter())
        .find(|p| p.name == "НалогообложенияНДСЭкспорт")
        .unwrap();
    assert_eq!(original.additional_values.len(), 2);
    for dialect in [FormDialect::Edt, FormDialect::Designer] {
        let bytes = write_form(dialect, &body).unwrap();
        let decoded = read_form(dialect, &bytes).unwrap();
        let result = decoded
            .data_attributes
            .iter()
            .filter_map(|a| a.dynamic_list.as_ref())
            .flat_map(|d| d.parameters.iter())
            .find(|p| p.name == original.name)
            .unwrap();
        assert_eq!(result, original);
    }
}
fn dcs_orders(bytes: &[u8]) -> Vec<Vec<String>> {
    fn visit(el: &formats_xml::Element, out: &mut Vec<Vec<String>>) {
        if matches!(
            el.attr("xsi:type").map(|a| a.value.as_str()),
            Some("form:DynamicListExtInfo" | "DynamicList")
        ) {
            let tags = el
                .children
                .iter()
                .filter(|c| {
                    matches!(
                        c.local.as_str(),
                        "fields"
                            | "calculatedFields"
                            | "parameters"
                            | "Field"
                            | "CalculatedField"
                            | "Parameter"
                            | "keyType"
                            | "keyField"
                            | "KeyType"
                            | "KeyField"
                            | "MainTable"
                            | "AutoSaveUserSettings"
                    )
                })
                .map(|c| c.local.clone())
                .collect();
            out.push(tags);
            for c in &el.children {
                if matches!(c.local.as_str(), "parameters" | "Parameter") {
                    out.push(c.children.iter().map(|v| v.local.clone()).collect());
                }
            }
        }
        for c in &el.children {
            visit(c, out);
        }
    }
    let root = formats_xml::read::parse(bytes).unwrap();
    let mut result = vec![];
    visit(&root.root, &mut result);
    result
}
#[test]
fn schema_writers_follow_witnessed_field_calculated_parameter_and_usage_order() {
    let content = SCHEMA.replace(
        "<use>Always</use>",
        "<availableAsField>false</availableAsField><use>Always</use>",
    );
    let body = read_form(FormDialect::Edt, &form(&content, "")).unwrap();
    for (dialect, field, calculated, parameters) in [
        (FormDialect::Edt, "fields", "calculatedFields", "parameters"),
        (
            FormDialect::Designer,
            "Field",
            "CalculatedField",
            "Parameter",
        ),
    ] {
        let bytes = write_form(dialect, &body).unwrap();
        let orders = dcs_orders(&bytes);
        let groups = &orders[0];
        assert!(
            groups.iter().position(|s| s == field).unwrap()
                < groups.iter().position(|s| s == calculated).unwrap()
        );
        assert!(
            groups.iter().position(|s| s == calculated).unwrap()
                < groups.iter().position(|s| s == parameters).unwrap()
        );
        let parameter = orders
            .iter()
            .find(|s| s.iter().any(|v| v == "use"))
            .unwrap();
        assert!(
            parameter
                .iter()
                .position(|s| s == "availableAsField")
                .unwrap()
                < parameter.iter().position(|s| s == "use").unwrap()
        );
    }
}
#[test]
#[ignore = "requires genuine F lab DCS order witnesses"]
fn genuine_dcs_writers_preserve_schema_and_parameter_element_order() {
    let lab = std::path::PathBuf::from(std::env::var("IBCMD_DCS_ORDER_LAB").unwrap());
    let cases = [
        (
            FormDialect::Edt,
            "uha83-failing-forms-edt-r1",
            "Form.form",
            vec!["10", "28", "38"],
        ),
        (
            FormDialect::Designer,
            "uha83-failing-forms-xml-r1",
            "Form.xml",
            vec!["101", "118", "40"],
        ),
    ];
    let mut count = 0;
    for (dialect, folder, name, ids) in cases {
        for id in ids {
            let source = std::fs::read(lab.join(folder).join(id).join(name)).unwrap();
            let body = read_form(dialect, &source).unwrap();
            let written = write_form(dialect, &body).unwrap();
            let expected = dcs_orders(&source);
            assert!(!expected.is_empty());
            assert_eq!(dcs_orders(&written), expected, "{folder}/{id}/{name}");
            count += 1;
        }
    }
    assert_eq!(count, 6);
}

fn assert_parameter_semantics(
    actual: &[morph1c_core::ir::DcsParameter],
    expected: &[morph1c_core::ir::DcsParameter],
) {
    // The sole witnessed semantic default is useRestriction=false. Raw native
    // absence/false/true output is asserted independently below; all other
    // ordered values and optional presences remain in semantic serialization.
    assert_eq!(
        serde_json::to_value(actual).unwrap(),
        serde_json::to_value(expected).unwrap()
    );
}
#[test]
fn schema_parameter_default_false_materializes_only_for_edt_origin() {
    let body = read_form(
        FormDialect::Edt,
        &form("<parameters><name>Restriction</name></parameters>", ""),
    )
    .unwrap();
    let encoded = write_form(FormDialect::Designer, &body).unwrap();
    let text = String::from_utf8(encoded).unwrap();
    assert!(text.contains("<dcssch:useRestriction>false</dcssch:useRestriction>"));
    for value in [None, Some(false), Some(true)] {
        let text = match value {
            None => text
                .lines()
                .filter(|line| !line.contains("<dcssch:useRestriction>"))
                .collect::<Vec<_>>()
                .join("\r\n"),
            Some(true) => text.replace(
                "<dcssch:useRestriction>false</dcssch:useRestriction>",
                "<dcssch:useRestriction>true</dcssch:useRestriction>",
            ),
            Some(false) => text.clone(),
        };
        let native = read_form(FormDialect::Designer, text.as_bytes()).unwrap();
        assert_eq!(dl(&native).parameters[0].use_restriction, value);
        let regenerated =
            String::from_utf8(write_form(FormDialect::Designer, &native).unwrap()).unwrap();
        match value {
            None => assert!(!regenerated.contains("<dcssch:useRestriction>")),
            Some(b) => assert!(regenerated.contains(&format!(
                "<dcssch:useRestriction>{b}</dcssch:useRestriction>"
            ))),
        }
    }
    assert!(
        read_form(
            FormDialect::Designer,
            text.replace(
                "<dcssch:useRestriction>false</dcssch:useRestriction>",
                "<dcssch:useRestriction>unknown</dcssch:useRestriction>"
            )
            .as_bytes()
        )
        .is_err()
    );
    let none = dl(&body).parameters[0].clone();
    let mut explicit_false = none.clone();
    explicit_false.use_restriction = Some(false);
    let mut explicit_true = none.clone();
    explicit_true.use_restriction = Some(true);
    assert_eq!(
        serde_json::to_value(&none).unwrap(),
        serde_json::to_value(&explicit_false).unwrap()
    );
    assert_ne!(
        serde_json::to_value(&none).unwrap(),
        serde_json::to_value(&explicit_true).unwrap()
    );
}
#[test]
fn co_present_schema_field_title_precedes_presentation_expression() {
    let content = r#"<fields xsi:type="schema:DataCompositionSchemaDataSetField"><dataPath>Name</dataPath><field>Name</field><title><value>Title</value></title><presentationExpression>STRING(Name)</presentationExpression></fields>"#;
    let body = read_form(FormDialect::Edt, &form(content, "")).unwrap();
    for dialect in [FormDialect::Edt, FormDialect::Designer] {
        let bytes = write_form(dialect, &body).unwrap();
        let text = String::from_utf8(bytes.clone()).unwrap();
        assert!(
            text.find("<title")
                .or_else(|| text.find("<dcssch:title"))
                .unwrap()
                < text.find("presentationExpression>").unwrap()
        );
        assert_eq!(
            dl(&read_form(dialect, &bytes).unwrap()).fields,
            dl(&body).fields
        );
    }
    assert!(
        read_form(
            FormDialect::Edt,
            &form(
                &content.replace(
                    "</presentationExpression>",
                    "</presentationExpression><unmodeled/>"
                ),
                ""
            )
        )
        .is_err()
    );
}
#[test]
#[ignore = "requires genuine F BSP SDK and UH co-present field witnesses"]
fn genuine_sdk_parameter_defaults_and_field_order_are_independently_witnessed() {
    let lab = std::path::PathBuf::from(std::env::var("IBCMD_DCS_ORDER_LAB").unwrap());
    let report: serde_json::Value = serde_json::from_slice(
        &std::fs::read(lab.join("bsp83-direct-dynamiclist-differences-r1.json")).unwrap(),
    )
    .unwrap();
    let mut explicit_false = 0;
    let mut params = 0;
    for row in report["rows"].as_array().unwrap() {
        let rel = row["path"].as_str().unwrap();
        let edt = std::fs::read(
            lab.join("oracle-bsp83-r1/authentic-workspace/OracleConfiguration/src")
                .join(rel.replace("/Ext/Form.xml", "/Form.form")),
        )
        .unwrap();
        let sdk =
            std::fs::read(lab.join("native-reference-bsp83-r2/native-xml").join(rel)).unwrap();
        let source = read_form(FormDialect::Edt, &edt).unwrap();
        let expected = read_form(FormDialect::Designer, &sdk).unwrap();
        let generated = read_form(
            FormDialect::Designer,
            &write_form(FormDialect::Designer, &source).unwrap(),
        )
        .unwrap();
        for attr in &expected.data_attributes {
            if let Some(list) = &attr.dynamic_list {
                let result = generated
                    .data_attributes
                    .iter()
                    .find(|a| a.name == attr.name)
                    .unwrap()
                    .dynamic_list
                    .as_ref()
                    .unwrap();
                assert_parameter_semantics(&result.parameters, &list.parameters);
                assert_eq!(
                    result
                        .parameters
                        .iter()
                        .map(|p| p.use_restriction)
                        .collect::<Vec<_>>(),
                    list.parameters
                        .iter()
                        .map(|p| p.use_restriction)
                        .collect::<Vec<_>>()
                );
                params += list.parameters.len();
                explicit_false += list
                    .parameters
                    .iter()
                    .filter(|p| p.use_restriction == Some(false))
                    .count();
            }
        }
    }
    assert_eq!(params, 28);
    assert_eq!(explicit_false, 24);
    let rel = "DataProcessors/МониторСверкиВзаиморасчетов/Forms/ДокументыНеПолученныеПоЭДО/Form.form";
    let original = std::fs::read(
        lab.join("oracle-uha83-r1/authentic-workspace/OracleConfiguration/src")
            .join(rel),
    )
    .unwrap();
    let source = read_form(FormDialect::Edt, &original).unwrap();
    let generated = write_form(FormDialect::Edt, &source).unwrap();
    fn field_orders(bytes: &[u8]) -> Vec<Vec<String>> {
        fn visit(el: &formats_xml::Element, out: &mut Vec<Vec<String>>) {
            if matches!(el.local.as_str(), "fields" | "Field")
                && el.children.iter().any(|c| c.local == "title")
                && el
                    .children
                    .iter()
                    .any(|c| c.local == "presentationExpression")
            {
                out.push(el.children.iter().map(|c| c.local.clone()).collect());
            }
            for c in &el.children {
                visit(c, out);
            }
        }
        let mut result = vec![];
        visit(&formats_xml::read::parse(bytes).unwrap().root, &mut result);
        result
    }
    assert!(!field_orders(&original).is_empty());
    assert_eq!(field_orders(&generated), field_orders(&original));
    let native_rel = rel.replace("/Form.form", "/Ext/Form.xml");
    let native = std::fs::read(
        lab.join("native-reference-uha83-r1/native-xml")
            .join(native_rel),
    )
    .unwrap();
    let generated_native = write_form(FormDialect::Designer, &source).unwrap();
    assert!(!field_orders(&native).is_empty());
    assert_eq!(field_orders(&generated_native), field_orders(&native));
}
