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
        assert_eq!(dl(&decoded).parameters, list.parameters);
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
        assert_eq!(dl(&decoded).parameters, dl(&body).parameters);
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
                                    assert_eq!(
                                        before.parameters,
                                        after.parameters,
                                        "{} {name}",
                                        path.display()
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
        assert_eq!(dl(&decoded).parameters, dl(&body).parameters);
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
