use formats_xml::form::{FormDialect, read_form, write_form};
use morph1c_core::ir::{
    DcsCorValue, DcsItem, DcsListSettings, DcsSettingsGroup, DcsSettingsParameterValue,
};
use morph1c_core::version::{FormatVersion, with_roundtrip_target, with_source_version};

#[test]
fn native_typed_empty_dcs_nodes_self_close_and_current_edits_remain_visible() {
    let source = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<form:Form xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:core=\"http://g5.1c.ru/v8/dt/mcore\" xmlns:form=\"http://g5.1c.ru/v8/dt/form\"><attributes><name>List</name><valueType><types>DynamicList</types></valueType><view><common>true</common></view><edit><common>true</common></edit><extInfo xsi:type=\"form:DynamicListExtInfo\"/></attributes></form:Form>\r\n";
    for minor in [20, 21] {
        let version = FormatVersion::new(2, minor);
        with_source_version(Some(version), || {
            with_roundtrip_target(version, || {
                let mut body = read_form(FormDialect::Edt, source).unwrap();
                let list = body.data_attributes[0].dynamic_list.as_mut().unwrap();
                list.calculated_fields.push(
                    serde_json::from_value(
                        serde_json::json!({"data_path":"Computed","expression":""}),
                    )
                    .unwrap(),
                );
                list.list_settings = Some(DcsListSettings {
                    filter: Some(DcsSettingsGroup {
                        items: vec![DcsItem::FilterComparison {
                            used: None,
                            left_field: String::new(),
                            left_type: "dcscor:Field".into(),
                            comparison_type: "Equal".into(),
                            right: vec![],
                            presentation: None,
                            view_mode: None,
                            user_setting_id: None,
                            user_setting_presentation: None,
                        }],
                        view_mode: None,
                        user_setting_id: None,
                        user_setting_presentation: None,
                    }),
                    data_parameters: vec![DcsSettingsParameterValue {
                        used: None,
                        parameter: "Caption".into(),
                        value: Some(DcsCorValue::LocalString(vec![])),
                        user_setting_id: None,
                    }],
                    ..DcsListSettings::default()
                });
                let native = write_form(FormDialect::Designer, &body).unwrap();
                let text = std::str::from_utf8(&native).unwrap();
                assert!(text.contains("<dcssch:expression/>"));
                assert!(text.contains("<dcscor:value xsi:type=\"v8:LocalStringType\"/>"));
                assert!(text.contains("<dcsset:left xsi:type=\"dcscor:Field\"/>"));
                let decoded = read_form(FormDialect::Designer, &native).unwrap();
                assert_eq!(
                    decoded.data_attributes[0].dynamic_list,
                    body.data_attributes[0].dynamic_list
                );
                assert_eq!(write_form(FormDialect::Designer, &decoded).unwrap(), native);
                let list = body.data_attributes[0].dynamic_list.as_mut().unwrap();
                list.calculated_fields[0].expression = "1 + 2".into();
                list.list_settings.as_mut().unwrap().data_parameters[0].value =
                    Some(DcsCorValue::Str("Current".into()));
                let DcsItem::FilterComparison { left_field, .. } = &mut list
                    .list_settings
                    .as_mut()
                    .unwrap()
                    .filter
                    .as_mut()
                    .unwrap()
                    .items[0]
                else {
                    panic!()
                };
                *left_field = "CurrentField".into();
                let native = write_form(FormDialect::Designer, &body).unwrap();
                let text = std::str::from_utf8(&native).unwrap();
                assert!(text.contains("<dcssch:expression>1 + 2</dcssch:expression>"));
                assert!(
                    text.contains(
                        "<dcsset:left xsi:type=\"dcscor:Field\">CurrentField</dcsset:left>"
                    )
                );
                assert!(
                    text.contains("<dcscor:value xsi:type=\"xs:string\">Current</dcscor:value>")
                );
                assert_eq!(
                    read_form(FormDialect::Designer, &native)
                        .unwrap()
                        .data_attributes[0]
                        .dynamic_list,
                    body.data_attributes[0].dynamic_list
                );
            })
        });
    }
}
