//! Minimized current EDT ListSettings.dcss framing control. No JVM or scratch IO.
use formats_xml::form::{read_list_settings_dcss, write_list_settings_dcss};
use morph1c_core::ir::{DcsListSettings, DcsPresentation};

#[test]
fn current_lf_sidecar_is_exact_and_current_text_edits_survive() {
    let settings = DcsListSettings {
        items_view_mode: Some("Normal".into()),
        items_user_setting_presentation: Some(DcsPresentation::Str("Before\nCurrent".into())),
        ..Default::default()
    };
    let legacy = write_list_settings_dcss(&settings);
    assert!(legacy.starts_with(b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n"));
    let current = String::from_utf8(legacy.clone())
        .unwrap()
        .replace("\r\n", "\n")
        .into_bytes();
    let mut parsed = read_list_settings_dcss(&current).unwrap();
    assert!(parsed.source_edt_lf);
    assert_eq!(
        parsed.items_user_setting_presentation,
        settings.items_user_setting_presentation
    );
    assert_eq!(write_list_settings_dcss(&parsed), current);
    parsed.items_user_setting_presentation = Some(DcsPresentation::Str("Edited\r\nCurrent".into()));
    let changed = write_list_settings_dcss(&parsed);
    let returned = read_list_settings_dcss(&changed).unwrap();
    assert!(returned.source_edt_lf);
    assert_eq!(
        returned.items_user_setting_presentation,
        parsed.items_user_setting_presentation
    );
    assert_ne!(changed, current);
    assert!(changed.starts_with(b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"));
    assert!(changed.windows(2).any(|bytes| bytes == b"\r\n"));
    assert_eq!(
        write_list_settings_dcss(&read_list_settings_dcss(&legacy).unwrap()),
        legacy
    );
    let semantic = serde_json::to_vec(&parsed).unwrap();
    let restored: DcsListSettings = serde_json::from_slice(&semantic).unwrap();
    assert!(!restored.source_edt_lf);
    assert_eq!(
        restored.items_user_setting_presentation,
        parsed.items_user_setting_presentation
    );
}

#[test]
fn lexical_lf_does_not_create_settings_or_admit_unknown_input() {
    let empty = DcsListSettings {
        source_edt_lf: true,
        ..Default::default()
    };
    assert!(empty.is_empty());
    assert_eq!(
        serde_json::to_vec(&empty).unwrap(),
        serde_json::to_vec(&DcsListSettings::default()).unwrap()
    );
    let settings = DcsListSettings {
        items_view_mode: Some("Normal".into()),
        ..Default::default()
    };
    let current = String::from_utf8(write_list_settings_dcss(&settings))
        .unwrap()
        .replace("\r\n", "\n");
    for bad in [
        format!("\u{feff}{current}"),
        current.replace("</Settings>", "<Unknown/></Settings>"),
        current.replace("UTF-8", "UTF-16"),
        current.replace("<itemsViewMode>", "<itemsViewMode unknown=\"loss\">"),
    ] {
        assert!(read_list_settings_dcss(bad.as_bytes()).is_err());
    }
}

#[test]
fn cleared_lf_settings_keep_native_empty_self_closing_spelling() {
    use formats_xml::form::{FormDialect, read_form, write_form};
    use morph1c_core::version::{FormatVersion, with_roundtrip_target, with_source_version};
    let source = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<form:Form xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:core=\"http://g5.1c.ru/v8/dt/mcore\" xmlns:form=\"http://g5.1c.ru/v8/dt/form\"><attributes><name>List</name><valueType><types>DynamicList</types></valueType><view><common>true</common></view><edit><common>true</common></edit><extInfo xsi:type=\"form:DynamicListExtInfo\"/></attributes></form:Form>\r\n";
    for minor in [20, 21] {
        let version = FormatVersion::new(2, minor);
        let mut body =
            with_source_version(Some(version), || read_form(FormDialect::Edt, source)).unwrap();
        body.data_attributes[0]
            .dynamic_list
            .as_mut()
            .unwrap()
            .list_settings = Some(DcsListSettings::default());
        let expected =
            with_roundtrip_target(version, || write_form(FormDialect::Designer, &body)).unwrap();
        body.data_attributes[0]
            .dynamic_list
            .as_mut()
            .unwrap()
            .list_settings
            .as_mut()
            .unwrap()
            .source_edt_lf = true;
        let actual =
            with_roundtrip_target(version, || write_form(FormDialect::Designer, &body)).unwrap();
        assert_eq!(actual, expected);
        assert!(
            std::str::from_utf8(&actual)
                .unwrap()
                .contains("<ListSettings/>")
        );
    }
}
