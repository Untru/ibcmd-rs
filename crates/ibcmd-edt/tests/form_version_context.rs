use formats_xml::form::{FormDialect, read_form, write_form};
use morph1c_core::ir::{FormBody, FormControlKind, FormItem, PropertyValue, Token};
use morph1c_core::spec::forms::controls::{button as bt, table as tb};
use morph1c_core::version::{
    FormatVersion, current_roundtrip_target, current_source_version, with_roundtrip_target,
    with_source_version,
};

fn seed() -> FormBody {
    let mut body = FormBody::new();
    let mut table = FormItem::new(FormControlKind::new("Table"), "T", 1);
    table.properties = vec![
        (tb::F_HORIZONTAL_LINES, PropertyValue::Bool(false)),
        (tb::F_VERTICAL_LINES, PropertyValue::Bool(false)),
    ];
    let mut button = FormItem::new(FormControlKind::new("Button"), "B", 2);
    button.properties.push((
        bt::F_BUTTON_IMPORTANCE,
        PropertyValue::Enum(Token::new("Main")),
    ));
    body.items = vec![table, button];
    body
}
fn text(bytes: &[u8]) -> &str {
    std::str::from_utf8(bytes).unwrap()
}
fn value(body: &FormBody, i: usize, id: morph1c_core::ir::FieldId) -> &PropertyValue {
    &body.items[i]
        .properties
        .iter()
        .find(|(field, _)| *field == id)
        .unwrap()
        .1
}

#[test]
fn standalone_default_envelope_defaults_and_presence_use_one_profile() {
    assert_eq!(current_source_version(), None);
    assert_eq!(current_roundtrip_target(), None);
    let edt = write_form(FormDialect::Edt, &seed()).unwrap();
    assert!(!text(&edt).contains("<horizontalLines>"));
    assert!(!text(&edt).contains("<buttonImportance>"));
    let current = read_form(FormDialect::Edt, &edt).unwrap();
    assert_eq!(
        value(&current, 0, tb::F_HORIZONTAL_LINES),
        &PropertyValue::Bool(false)
    );
    assert_eq!(
        value(&current, 1, bt::F_BUTTON_IMPORTANCE),
        &PropertyValue::Enum(Token::new("Main"))
    );
    let native = write_form(FormDialect::Designer, &current).unwrap();
    assert!(text(&native).contains("version=\"2.21\""));
    assert!(text(&native).contains("<HorizontalLines>false</HorizontalLines>"));
    assert!(text(&native).contains("<ButtonImportance>Main</ButtonImportance>"));
    let reread = read_form(FormDialect::Designer, &native).unwrap();
    assert_eq!(write_form(FormDialect::Designer, &reread).unwrap(), native);
    assert_eq!(
        serde_json::to_vec(&reread).unwrap(),
        serde_json::to_vec(&current).unwrap()
    );
    assert_eq!(current_source_version(), None);
    assert_eq!(current_roundtrip_target(), None);
}

#[test]
fn native_root_version_is_authoritative_and_explicit_targets_remain_scoped() {
    let v20 = FormatVersion::new(2, 20);
    let initial =
        with_roundtrip_target(v20, || write_form(FormDialect::Designer, &seed())).unwrap();
    // Materialize the constructed sparse bag before checking source emission.
    let native = with_roundtrip_target(v20, || {
        write_form(
            FormDialect::Designer,
            &read_form(FormDialect::Designer, &initial).unwrap(),
        )
    })
    .unwrap();
    assert!(text(&native).contains("version=\"2.20\""));
    let read_default = read_form(FormDialect::Designer, &native).unwrap();
    let read_other_ambient = with_source_version(Some(FormatVersion::new(2, 21)), || {
        read_form(FormDialect::Designer, &native)
    })
    .unwrap();
    assert_eq!(
        serde_json::to_vec(&read_default).unwrap(),
        serde_json::to_vec(&read_other_ambient).unwrap()
    );
    assert_eq!(
        with_roundtrip_target(v20, || write_form(FormDialect::Designer, &read_default)).unwrap(),
        native
    );
    assert_eq!(current_source_version(), None);
    assert_eq!(current_roundtrip_target(), None);
    assert!(
        with_roundtrip_target(FormatVersion::new(2, 99), || write_form(
            FormDialect::Designer,
            &seed()
        ))
        .is_err()
    );
    assert_eq!(current_roundtrip_target(), None);
}

#[test]
fn standalone_explicit_default_presence_keeps_current_edits_and_rejects_duplicates() {
    let native = write_form(
        FormDialect::Designer,
        &read_form(
            FormDialect::Edt,
            &write_form(FormDialect::Edt, &seed()).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let mut current = read_form(FormDialect::Designer, &native).unwrap();
    current.items[0]
        .properties
        .iter_mut()
        .find(|(id, _)| *id == tb::F_HORIZONTAL_LINES)
        .unwrap()
        .1 = PropertyValue::Bool(true);
    let edited = write_form(FormDialect::Designer, &current).unwrap();
    assert!(text(&edited).contains("<HorizontalLines>true</HorizontalLines>"));
    assert!(!text(&edited).contains("<HorizontalLines>false</HorizontalLines>"));
    assert_eq!(
        value(
            &read_form(FormDialect::Designer, &edited).unwrap(),
            0,
            tb::F_HORIZONTAL_LINES
        ),
        &PropertyValue::Bool(true)
    );
    let duplicate = text(&native).replace(
        "<HorizontalLines>false</HorizontalLines>",
        "<HorizontalLines>false</HorizontalLines>\r\n\t\t\t<HorizontalLines>true</HorizontalLines>",
    );
    assert!(read_form(FormDialect::Designer, duplicate.as_bytes()).is_err());
    assert_eq!(current_source_version(), None);
    assert_eq!(current_roundtrip_target(), None);
}
