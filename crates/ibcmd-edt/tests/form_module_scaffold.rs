//! A bounded current-EDT empty-editor-scaffold witness, not general BSL EOL policy.
//! Every scratch directory is retained, including assertion failures.
use morph1c_core::ir::{
    FormBody, MetadataObject, NamedFormBody, ObjectKind, PropertyValue, Token, Uuid,
};
use morph1c_pipeline::{Format, attach_form_body, write_form_bodies};
use std::fs;

const EDT: &str = include_str!("fixtures/form-module-scaffold/edt-lf.bsl");
const NATIVE: &[u8] = include_bytes!("fixtures/form-module-scaffold/native-crlf.bsl");

/// A scratch directory, removed with the value (also when the test fails)
/// unless `IBCMD_TEST_KEEP_FIXTURES=1`.
fn scratch() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("ibcmd-module-scaffold-")
        .disable_cleanup(keep_fixtures())
        .tempdir()
        .unwrap()
}

/// Whether the fixtures are kept for debugging: `IBCMD_TEST_KEEP_FIXTURES=1`.
fn keep_fixtures() -> bool {
    std::env::var_os("IBCMD_TEST_KEEP_FIXTURES").is_some_and(|value| value == "1")
}
fn object() -> MetadataObject {
    let mut object =
        MetadataObject::new(ObjectKind::new("CommonForm"), "Witness", Uuid([0x37; 16]));
    let field = morph1c_core::spec::registry::spec_for("CommonForm")
        .unwrap()
        .fields()
        .iter()
        .find(|field| field.name == "formType")
        .unwrap()
        .id;
    object
        .properties
        .push((field, PropertyValue::Enum(Token::new("Managed"))));
    object
}
fn authored(text: &str) -> MetadataObject {
    let mut object = object();
    object.form_bodies.push(NamedFormBody {
        name: "Witness".into(),
        body: FormBody::new(),
        ordinary_body: None,
        module: Some(text.into()),
        help: vec![],
        help_resources: vec![],
    });
    object
}
fn read_edt(text: &str, root: &std::path::Path) -> MetadataObject {
    let anchor = root.join("source/Witness.mdo");
    write_form_bodies(Format::Edt, &anchor, &authored(text)).unwrap();
    let mut parsed = object();
    attach_form_body(Format::Edt, "CommonForm", &anchor, &mut parsed).unwrap();
    parsed
}
fn emit(object: &MetadataObject, root: &std::path::Path, label: &str, format: Format) -> Vec<u8> {
    let anchor = root.join(label).join(if format == Format::Designer {
        "Witness.xml"
    } else {
        "Witness.mdo"
    });
    write_form_bodies(format, &anchor, object).unwrap();
    fs::read(if format == Format::Designer {
        anchor.parent().unwrap().join("Witness/Ext/Form/Module.bsl")
    } else {
        anchor.parent().unwrap().join("Module.bsl")
    })
    .unwrap()
}
fn native_bytes(text: &str) -> Vec<u8> {
    let mut bytes = vec![0xef, 0xbb, 0xbf];
    bytes.extend_from_slice(text.as_bytes());
    bytes
}

#[test]
fn module_scaffold_current_witness_and_current_region_edits_are_framed_exactly() {
    let root = scratch();
    let root = root.path().to_path_buf();
    let mut parsed = read_edt(EDT, &root);
    assert!(parsed.form_bodies[0].body.source_module_scaffold_edt_lf);
    let semantic = serde_json::to_vec(&parsed).unwrap();
    assert_eq!(parsed.form_bodies[0].module.as_deref(), Some(EDT));
    assert_eq!(
        emit(&parsed, &root, "same-edt", Format::Edt),
        EDT.as_bytes()
    );
    assert_eq!(emit(&parsed, &root, "native", Format::Designer), NATIVE);
    assert_eq!(
        serde_json::to_vec(&parsed).unwrap(),
        semantic,
        "emission must not mutate current text"
    );
    let restored: MetadataObject = serde_json::from_slice(&semantic).unwrap();
    assert!(!restored.form_bodies[0].body.source_module_scaffold_edt_lf);
    assert_eq!(restored.form_bodies[0].module, parsed.form_bodies[0].module);
    let edited = EDT.replace("СлужебныеПроцедурыИФункции", "ТекущаяОбласть_2") + "\n\n";
    parsed.form_bodies[0].module = Some(edited.clone());
    assert_ne!(
        serde_json::to_vec(&parsed).unwrap(),
        semantic,
        "current title/EOL edits stay visible"
    );
    assert_eq!(
        emit(&parsed, &root, "edited", Format::Designer),
        native_bytes(&edited.replace('\n', "\r\n"))
    );
    assert_eq!(
        emit(&parsed, &root, "edited-edt", Format::Edt),
        edited.as_bytes()
    );
}

#[test]
fn module_scaffold_current_non_scaffold_edits_revoke_projection_without_losing_text() {
    for (index, edited) in [
        "#Область НеЗакрыта\n".into(),
        "#КонецОбласти\n#Область A\n#КонецОбласти".into(),
        "#Область A\n#Область B\n#КонецОбласти".into(),
        "#Область\n#КонецОбласти".into(),
        "#Область A B\n#КонецОбласти".into(),
        format!("{EDT}\n#Если Клиент Тогда\n#КонецЕсли"),
        format!("{EDT}\n// authored comment"),
        "Процедура Текущая()\nКонецПроцедуры".into(),
        "Значение = \"первая\n|вторая\";".into(),
        EDT.replacen('\n', "\r\n", 1),
        format!("{EDT}\r"),
        format!("\u{feff}{EDT}"),
        EDT.replacen('\n', "\u{2028}", 1),
        EDT.replacen('\n', "\u{a0}\n", 1),
    ]
    .into_iter()
    .enumerate()
    {
        let root = scratch();
        let root = root.path().to_path_buf();
        let mut parsed = read_edt(EDT, &root);
        assert!(parsed.form_bodies[0].body.source_module_scaffold_edt_lf);
        parsed.form_bodies[0].module = Some(edited.clone());
        assert_eq!(
            emit(&parsed, &root, "edited", Format::Designer),
            native_bytes(&edited),
            "current edit {index}"
        );
        let unattached = root.join("unattached");
        let read = read_edt(&edited, &unattached);
        assert!(
            !read.form_bodies[0].body.source_module_scaffold_edt_lf,
            "source {index}"
        );
        assert_eq!(read.form_bodies[0].module.as_deref(), Some(edited.as_str()));
    }
}

#[test]
fn module_scaffold_native_origin_and_default_writer_keep_all_module_newlines() {
    let root = scratch();
    let root = root.path().to_path_buf();
    let original = authored(EDT);
    assert_eq!(
        emit(&original, &root, "native-source", Format::Designer),
        native_bytes(EDT)
    );
    let anchor = root.join("native-source/Witness.xml");
    let mut parsed = object();
    attach_form_body(Format::Designer, "CommonForm", &anchor, &mut parsed).unwrap();
    assert!(!parsed.form_bodies[0].body.source_module_scaffold_edt_lf);
    assert_eq!(
        emit(&parsed, &root, "native-return", Format::Designer),
        native_bytes(EDT)
    );
    assert_eq!(parsed.form_bodies[0].module.as_deref(), Some(EDT));
    for (index, text) in [
        EDT.replace('\n', "\r\n"),
        "\u{feff} // body\r\n\u{feff}tail".into(),
    ]
    .iter()
    .enumerate()
    {
        assert_eq!(
            emit(
                &authored(text),
                &root,
                &format!("default{index}"),
                Format::Designer
            ),
            native_bytes(text)
        );
    }
}
