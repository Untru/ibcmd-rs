//! Current installed EDT 2026.2.1.3rc0 writes LF-framed Form.form sources.
//! These minimized typed controls preserve legacy CRLF defaults; no JVM or
//! scratch filesystem is used. This is framing evidence, not whole-model support.
use formats_xml::form::{FormDialect, read_form, write_form};
use morph1c_core::ir::{FormBody, Lang, PropertyValue};
use morph1c_core::version::{FormatVersion, with_roundtrip_target, with_source_version};

fn read(bytes: &[u8], minor: u16) -> FormBody {
    with_source_version(Some(FormatVersion::new(2, minor)), || {
        read_form(FormDialect::Edt, bytes)
    })
    .unwrap()
}
fn write(body: &FormBody, minor: u16) -> Vec<u8> {
    with_roundtrip_target(FormatVersion::new(2, minor), || {
        write_form(FormDialect::Edt, body)
    })
    .unwrap()
}

#[test]
fn witnessed_lf_framing_keeps_current_typed_title_and_legacy_defaults() {
    for minor in [20, 21] {
        let mut body = FormBody::new();
        body.title = Some(PropertyValue::Localized(vec![(
            Lang("ru".into()),
            "First\nSecond".into(),
        )]));
        let legacy = write(&body, minor);
        assert!(legacy.windows(2).any(|b| b == b"\r\n"));
        // The fixture witnesses the current serializer's structural and text LF
        // together. Production readers/writers never rewrite raw input strings.
        let current = String::from_utf8(legacy.clone())
            .unwrap()
            .replace("\r\n", "\n")
            .into_bytes();
        let mut parsed = read(&current, minor);
        assert!(parsed.source_edt_lf);
        assert_eq!(parsed.title, body.title);
        assert_eq!(write(&parsed, minor), current);
        let semantic = serde_json::to_vec(&parsed).unwrap();
        let restored: FormBody = serde_json::from_slice(&semantic).unwrap();
        assert!(!restored.source_edt_lf);
        assert_eq!(restored.title, parsed.title);
        assert_eq!(write(&restored, minor), legacy);
        parsed.title = Some(PropertyValue::Localized(vec![(
            Lang("ru".into()),
            "Edited\r\nCurrent".into(),
        )]));
        let changed = write(&parsed, minor);
        assert_ne!(changed, current);
        assert_eq!(read(&changed, minor).title, parsed.title);
        assert!(changed.starts_with(b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"));
        assert!(changed.windows(2).any(|bytes| bytes == b"\r\n"));
        assert!(read(&changed, minor).source_edt_lf);
        assert!(!read(&legacy, minor).source_edt_lf);
        assert_eq!(write(&read(&legacy, minor), minor), legacy);
    }
}

#[test]
fn native_lf_unknown_nodes_and_invalid_envelopes_still_fail() {
    let v = FormatVersion::new(2, 21);
    let mut body = FormBody::new();
    body.title = Some(PropertyValue::Localized(vec![(
        Lang("ru".into()),
        "Before".into(),
    )]));
    let native = with_roundtrip_target(v, || write_form(FormDialect::Designer, &body)).unwrap();
    let native_lf = String::from_utf8(native).unwrap().replace("\r\n", "\n");
    assert!(read_form(FormDialect::Designer, native_lf.as_bytes()).is_err());
    let current = String::from_utf8(write(&body, 21))
        .unwrap()
        .replace("\r\n", "\n");
    for bad in [
        format!("\u{feff}{current}"),
        current.replace("</form:Form>", "<Unknown/></form:Form>"),
        current.replace("UTF-8", "UTF-16"),
    ] {
        assert!(read_form(FormDialect::Edt, bad.as_bytes()).is_err());
    }
}
