use formats_xml::form::{FormDialect, read_form, write_form};
use morph1c_core::{
    ir::{FieldId, FormBody, FormControlKind, FormItem, Lang, PropertyValue, Token},
    spec::forms::controls::form_field as ff,
    version::{FormatVersion, with_roundtrip_target, with_source_version},
};

fn write(dialect: FormDialect, body: &FormBody) -> Vec<u8> {
    with_roundtrip_target(FormatVersion::new(2, 20), || write_form(dialect, body)).unwrap()
}
fn read(dialect: FormDialect, bytes: &[u8]) -> FormBody {
    with_source_version(Some(FormatVersion::new(2, 20)), || {
        read_form(dialect, bytes)
    })
    .unwrap()
}

#[test]
fn image_drag_and_spreadsheet_tooltip_are_emitted_and_remain_editable() {
    let mut body = FormBody::new();
    let mut image = FormItem::new(FormControlKind::new("PictureField"), "Image", 1);
    image
        .ext_info
        .push((FieldId(958), PropertyValue::Bool(true)));
    let mut sheet = FormItem::new(FormControlKind::new("SpreadsheetDocumentField"), "Sheet", 2);
    sheet.properties.push((
        ff::F_TOOL_TIP,
        PropertyValue::Localized(vec![
            (Lang::new("ru"), "Повторить импорт".into()),
            (Lang::new("en"), "Import again".into()),
        ]),
    ));
    sheet.properties.push((
        ff::F_TOOL_TIP_REPRESENTATION,
        PropertyValue::Enum(Token::new("ShowBottom")),
    ));
    sheet.ext_info.push((
        ff::F_EXT_DRAWING_SELECTION_SHOW_MODE,
        PropertyValue::Enum(Token::new("Show")),
    ));
    body.items = vec![image, sheet];
    for dialect in [FormDialect::Designer, FormDialect::Edt] {
        let output = write(dialect, &body);
        let readback = read(dialect, &output);
        assert_eq!(
            readback.items[0].get_ext(FieldId(958)),
            Some(&PropertyValue::Bool(true))
        );
        assert_eq!(
            readback.items[1].get(ff::F_TOOL_TIP),
            body.items[1].get(ff::F_TOOL_TIP)
        );
        assert_eq!(
            readback.items[1].get(ff::F_TOOL_TIP_REPRESENTATION),
            body.items[1].get(ff::F_TOOL_TIP_REPRESENTATION)
        );
        assert_eq!(
            readback.items[1].get_ext(ff::F_EXT_DRAWING_SELECTION_SHOW_MODE),
            Some(&PropertyValue::Enum(Token::new("Show")))
        );
    }
    body.items[0].ext_info[0].1 = PropertyValue::Bool(false);
    body.items[1].properties[0].1 =
        PropertyValue::Localized(vec![(Lang::new("ru"), "Изменено".into())]);
    let output = write(FormDialect::Designer, &body);
    let edited = read(FormDialect::Designer, &output);
    assert_eq!(
        edited.items[0].get_ext(FieldId(958)),
        Some(&PropertyValue::Bool(false))
    );
    assert_eq!(
        edited.items[1].get(ff::F_TOOL_TIP),
        body.items[1].get(ff::F_TOOL_TIP)
    );
}

#[test]
fn spreadsheet_drawing_selection_is_closed_and_nondefault_is_preserved() {
    let mut body = FormBody::new();
    let mut sheet = FormItem::new(FormControlKind::new("SpreadsheetDocumentField"), "Sheet", 1);
    sheet.ext_info.push((
        ff::F_EXT_DRAWING_SELECTION_SHOW_MODE,
        PropertyValue::Enum(Token::new("DontShow")),
    ));
    body.items.push(sheet);
    for dialect in [FormDialect::Designer, FormDialect::Edt] {
        let output = write(dialect, &body);
        assert_eq!(
            read(dialect, &output).items[0].get_ext(ff::F_EXT_DRAWING_SELECTION_SHOW_MODE),
            body.items[0].get_ext(ff::F_EXT_DRAWING_SELECTION_SHOW_MODE)
        );
        let unknown = String::from_utf8(output)
            .unwrap()
            .replace("DontShow", "UnknownDrawingMode");
        assert!(read_form(dialect, unknown.as_bytes()).is_err());
    }
    body.items[0].ext_info[0].1 = PropertyValue::Enum(Token::new("UnknownDrawingMode"));
    assert!(write_form(FormDialect::Designer, &body).is_err());
    assert!(write_form(FormDialect::Edt, &body).is_err());
}

#[test]
#[ignore = "Read-only genuine UH body emit-drop witnesses in F laboratory"]
fn genuine_typed_tooltips_and_drag_values_survive_same_source_emission() {
    let root = std::path::Path::new(
        "F:/ibcmd/lab/07/uha-38a-full-regeneration-r1/xml/form-regeneration-xml-witnesses",
    );
    for witness in [33, 71, 82, 83, 143, 162, 251, 271] {
        let original = std::fs::read(root.join(format!("{witness}.source.xml"))).unwrap();
        let body = read(FormDialect::Designer, &original);
        let generated = write(FormDialect::Designer, &body);
        let reloaded = read(FormDialect::Designer, &generated);
        assert_eq!(
            serde_json::to_value(&body).unwrap(),
            serde_json::to_value(&reloaded).unwrap(),
            "whole typed semantics witness {witness}"
        );
    }
}

#[test]
fn edt_enum_map_uses_canonical_tokens_when_native_spelling_differs() {
    use morph1c_core::spec::forms::controls::table as tb;
    for (canonical, native) in [
        ("ByContent", "UseContentHeight"),
        ("InTableRows", "UseHeightInTableRows"),
    ] {
        let mut body = FormBody::new();
        let mut table = FormItem::new(FormControlKind::new("Table"), "List", 1);
        table.properties.push((
            tb::F_HEIGHT_CONTROL_VARIANT,
            PropertyValue::Enum(Token::new(canonical)),
        ));
        body.items.push(table);
        for dialect in [FormDialect::Designer, FormDialect::Edt] {
            let generated = write(dialect, &body);
            assert!(String::from_utf8(generated.clone()).unwrap().contains(
                if dialect == FormDialect::Edt {
                    canonical
                } else {
                    native
                }
            ));
            assert_eq!(
                read(dialect, &generated).items[0].get(tb::F_HEIGHT_CONTROL_VARIANT),
                Some(&PropertyValue::Enum(Token::new(canonical)))
            );
        }
    }
}

#[test]
fn drawing_selection_dialect_defaults_and_edits_are_distinct() {
    fn sheet(mode: &str) -> FormBody {
        let mut body = FormBody::new();
        let mut item = FormItem::new(FormControlKind::new("SpreadsheetDocumentField"), "Sheet", 1);
        item.ext_info.push((
            ff::F_EXT_DRAWING_SELECTION_SHOW_MODE,
            PropertyValue::Enum(Token::new(mode)),
        ));
        body.items.push(item);
        body
    }
    for mode in ["Auto", "Show", "DontShow"] {
        let native = write(FormDialect::Designer, &sheet(mode));
        let from_native = read(FormDialect::Designer, &native);
        let edt = write(FormDialect::Edt, &from_native);
        assert_eq!(
            String::from_utf8(edt.clone())
                .unwrap()
                .contains("<drawingSelectionShowMode>"),
            mode != "Show"
        );
        let from_edt = read(FormDialect::Edt, &edt);
        assert_eq!(
            from_edt.items[0].get_ext(ff::F_EXT_DRAWING_SELECTION_SHOW_MODE),
            Some(&PropertyValue::Enum(Token::new(mode)))
        );
        assert_eq!(write(FormDialect::Designer, &from_edt), native);
        let mut changed = from_edt.clone();
        changed.items[0]
            .ext_info
            .iter_mut()
            .find(|(id, _)| *id == ff::F_EXT_DRAWING_SELECTION_SHOW_MODE)
            .unwrap()
            .1 = PropertyValue::Enum(Token::new(if mode == "DontShow" {
            "Show"
        } else {
            "DontShow"
        }));
        assert_ne!(
            serde_json::to_value(&changed).unwrap(),
            serde_json::to_value(&from_edt).unwrap()
        );
        assert_eq!(
            read(FormDialect::Edt, &write(FormDialect::Edt, &changed)).items[0]
                .get_ext(ff::F_EXT_DRAWING_SELECTION_SHOW_MODE),
            changed.items[0].get_ext(ff::F_EXT_DRAWING_SELECTION_SHOW_MODE)
        );
    }
}
