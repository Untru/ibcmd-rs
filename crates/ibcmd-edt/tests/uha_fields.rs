use formats_xml::form::{FormDialect, read_form, write_form};
use morph1c_core::ir::{FieldId, FormBody, FormControlKind, FormItem, PropertyValue, Token};

fn item(kind: &str, id: i64, fields: Vec<(FieldId, PropertyValue)>) -> FormItem {
    let mut item = FormItem::new(FormControlKind::new(kind), format!("Control{id}"), id);
    item.ext_info = fields;
    item
}
#[test]
fn witnessed_uha_ext_fields_survive_both_typed_codecs() {
    let mut form = FormBody::new();
    form.items = vec![
        item(
            "InputField",
            1,
            vec![(
                FieldId(962),
                PropertyValue::Ref("Полномочие.Наименование".into()),
            )],
        ),
        item(
            "UsualGroup",
            2,
            vec![(
                FieldId(973),
                PropertyValue::Value(morph1c_core::ir::value::ValueSpec {
                    kind: morph1c_core::ir::value::ValueScalarKind::Str,
                    scalar: Some(Box::new(PropertyValue::Str("Таблица".into()))),
                }),
            )],
        ),
        item(
            "Pages",
            3,
            vec![(
                FieldId(973),
                PropertyValue::Value(morph1c_core::ir::value::ValueSpec {
                    kind: morph1c_core::ir::value::ValueScalarKind::Str,
                    scalar: Some(Box::new(PropertyValue::Str("Таблица".into()))),
                }),
            )],
        ),
        item(
            "FormattedDocumentField",
            4,
            vec![(FieldId(129), PropertyValue::Ref("Style.Background".into()))],
        ),
        item(
            "TextDocumentField",
            5,
            vec![(FieldId(129), PropertyValue::Ref("Web.NavajoWhite".into()))],
        ),
        item(
            "CheckBoxField",
            6,
            vec![(
                FieldId(129),
                PropertyValue::Ref("Style.ToolTipBackColor".into()),
            )],
        ),
        item(
            "GraphicalSchemaField",
            7,
            vec![(FieldId(160), PropertyValue::Enum(Token::new("Enable")))],
        ),
        item(
            "ColumnGroup",
            8,
            vec![(FieldId(974), PropertyValue::Ref("Style.Title".into()))],
        ),
        item(
            "LabelDecoration",
            9,
            vec![(
                FieldId(109),
                PropertyValue::Ref("Style.ControlBorder".into()),
            )],
        ),
    ];
    for dialect in [FormDialect::Designer, FormDialect::Edt] {
        let encoded = write_form(dialect, &form).unwrap();
        let decoded = read_form(dialect, &encoded).unwrap();
        for (before, after) in form.items.iter().zip(&decoded.items) {
            for (id, value) in &before.ext_info {
                assert_eq!(
                    &after.ext_info.iter().find(|(f, _)| f == id).unwrap().1,
                    value
                );
            }
        }
        let text = std::str::from_utf8(&encoded).unwrap();
        let leaf = if dialect == FormDialect::Designer {
            "MultipleValuePresentDataPath"
        } else {
            "segments"
        };
        assert!(
            read_form(
                dialect,
                text.replace(&format!("<{leaf}>"), &format!("<{leaf} unknown='keep'>"))
                    .as_bytes()
            )
            .is_err()
        );
        assert!(
            read_form(
                dialect,
                text.replace("Enable", "UnwitnessedOutput").as_bytes()
            )
            .is_err()
        );
        if dialect == FormDialect::Edt {
            for bad in [
                text.replace("core:BorderRef", "core:UnknownBorder"),
                text.replace("Style.ControlBorder", "Palette.ControlBorder"),
                text.replace("xmlns:core=\"http://g5.1c.ru/v8/dt/mcore\"", ""),
                text.replace("http://www.w3.org/2001/XMLSchema-instance", "urn:wrong:xsi"),
            ] {
                assert!(read_form(dialect, bad.as_bytes()).is_err());
            }
        }
    }
}
#[test]
fn system_comparison_enums_are_typed_and_closed() {
    use formats_xml::{
        parse,
        value_codec::{self, ValueDialect},
    };
    for member in [
        "Equal",
        "NotEqual",
        "InList",
        "NotInList",
        "InHierarchy",
        "InListByHierarchy",
    ] {
        let edt = format!(
            "<value xsi:type='core:SysEnumValue'><value>DataCompositionComparisonType.{member}</value></value>"
        );
        let source =
            value_codec::decode(ValueDialect::Edt, &parse(edt.as_bytes()).unwrap().root).unwrap();
        let PropertyValue::Value(ref spec) = source else {
            panic!("typed value required")
        };
        let designer = value_codec::encode(ValueDialect::Designer, "", "Value", spec).unwrap();
        assert_eq!(
            designer.attrs,
            vec![(
                "xsi:type".into(),
                "dcsset:DataCompositionComparisonType".into()
            )]
        );
        assert_eq!(designer.text.as_deref(), Some(member));
        let native =
            format!("<Value xsi:type='dcsset:DataCompositionComparisonType'>{member}</Value>");
        assert_eq!(
            value_codec::decode(
                ValueDialect::Designer,
                &parse(native.as_bytes()).unwrap().root
            )
            .unwrap(),
            source
        );
        for bad in [
            edt.replace(member, "Invented"),
            edt.replace("DataCompositionComparisonType", "OtherEnum"),
            edt.replace("<value>Data", "<value extra='yes'>Data"),
            edt.replace("</value></value>", "</value><unknown/></value>"),
        ] {
            assert!(
                value_codec::decode(ValueDialect::Edt, &parse(bad.as_bytes()).unwrap().root)
                    .is_err()
            );
        }
    }
}

#[test]
fn comparison_and_conditional_appearance_types_use_exact_aliases() {
    let empty = write_form(FormDialect::Edt, &FormBody::new()).unwrap();
    let xml = std::str::from_utf8(&empty).unwrap();
    let with_attributes = xml.replace("</form:Form>", "  <attributes><name>Comparison</name><id>1</id><valueType><types>ComparisonType</types><types>ConditionalAppearance</types></valueType><view><common>true</common></view><edit><common>true</common></edit></attributes>\r\n</form:Form>");
    let form = read_form(FormDialect::Edt, with_attributes.as_bytes()).unwrap();
    let native = write_form(FormDialect::Designer, &form).unwrap();
    let text = std::str::from_utf8(&native).unwrap();
    assert!(text.contains("ent:ComparisonType"));
    assert!(text.contains("http://v8.1c.ru/8.3/data/entext"));
    assert_eq!(
        read_form(FormDialect::Designer, &native)
            .unwrap()
            .data_attributes[0]
            .value_type,
        form.data_attributes[0].value_type
    );
    for bad in [
        text.replace("http://v8.1c.ru/8.3/data/entext", "urn:wrong:entext"),
        text.replace("ConditionalAppearance", "UnknownAppearance"),
    ] {
        assert!(read_form(FormDialect::Designer, bad.as_bytes()).is_err());
    }
}

#[test]
fn reference_pixels_are_independent_and_edt_loss_is_rejected() {
    use morph1c_core::{
        ir::FormCommand,
        spec::forms::{command as fc, controls::button as bt},
    };
    let picture = PropertyValue::List(vec![
        PropertyValue::Ref("CommonPicture.Process".into()),
        PropertyValue::Bool(true),
        PropertyValue::List(vec![PropertyValue::Int(12), PropertyValue::Int(12)]),
    ]);
    let mut form = FormBody::new();
    let mut button = item("Button", 1, vec![]);
    button.properties.push((bt::F_PICTURE, picture.clone()));
    form.items.push(button);
    let mut command = FormCommand::new("Command", 2);
    command.properties.push((fc::F_PICTURE, picture.clone()));
    form.commands.push(command);
    let native = write_form(FormDialect::Designer, &form).unwrap();
    let read = read_form(FormDialect::Designer, &native).unwrap();
    assert_eq!(
        read.items[0]
            .properties
            .iter()
            .find(|(id, _)| *id == bt::F_PICTURE)
            .unwrap()
            .1,
        picture
    );
    assert_eq!(read.commands[0].get(fc::F_PICTURE), Some(&picture));
    assert!(
        write_form(FormDialect::Edt, &read)
            .unwrap_err()
            .to_string()
            .contains("per-use transparency")
    );
    let text = std::str::from_utf8(&native).unwrap();
    for bad in [
        text.replace("<xr:LoadTransparent>true", "<xr:LoadTransparent>false"),
        text.replace("x=\"12\"", "unexpected=\"12\""),
        text.replace(
            "<xr:TransparentPixel ",
            "<xr:TransparentPixel extra='keep' ",
        ),
        text.replace(
            "<xr:TransparentPixel ",
            "<xr:TransparentPixel xmlns:xr='urn:wrong' ",
        ),
    ] {
        assert!(read_form(FormDialect::Designer, bad.as_bytes()).is_err());
    }
}
#[test]
fn logical_name_exception_never_writes_an_oversized_filename() {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("а".repeat(150));
    assert!(morph1c_pipeline::fsio::write(&path, b"keep").is_err());
    assert!(!path.exists());
    assert!(morph1c_pipeline::fsio::create_dir_all(&path).is_err());
    assert!(!path.exists());
}

#[test]
fn sparse_role_false_and_empty_command_use_remain_typed_false() {
    use morph1c_core::{ir::FormCommand, spec::forms::controls::button as bt};
    let role = PropertyValue::List(vec![
        PropertyValue::Str("Role.Restricted".into()),
        PropertyValue::Bool(false),
    ]);
    let mut form = FormBody::new();
    let mut button = item("Button", 1, vec![]);
    button.properties.push((
        bt::F_USER_VISIBLE,
        PropertyValue::List(vec![PropertyValue::Bool(true), role]),
    ));
    form.items.push(button);
    let mut command = FormCommand::new("Denied", 2);
    command.properties.push((
        FieldId(925),
        PropertyValue::List(vec![PropertyValue::Bool(false)]),
    ));
    form.commands.push(command);
    let edt = write_form(FormDialect::Edt, &form).unwrap();
    assert!(
        !std::str::from_utf8(&edt)
            .unwrap()
            .contains("<value>false</value>")
    );
    let source = std::str::from_utf8(&edt)
        .unwrap()
        .replace("<value>false</value>", "");
    let parsed = read_form(FormDialect::Edt, source.as_bytes()).unwrap();
    assert_eq!(
        parsed.items[0]
            .properties
            .iter()
            .find(|(id, _)| *id == bt::F_USER_VISIBLE)
            .unwrap()
            .1,
        form.items[0]
            .properties
            .iter()
            .find(|(id, _)| *id == bt::F_USER_VISIBLE)
            .unwrap()
            .1
    );
    assert_eq!(
        parsed.commands[0].get(FieldId(925)),
        Some(&PropertyValue::List(vec![PropertyValue::Bool(false)]))
    );
    let native = write_form(FormDialect::Designer, &parsed).unwrap();
    let parsed_native = read_form(FormDialect::Designer, &native).unwrap();
    assert_eq!(
        parsed_native.commands[0].get(FieldId(925)),
        parsed.commands[0].get(FieldId(925))
    );
    for bad in [
        source.replace("<role>", "<role unknown='keep'>"),
        source.replace("<for>", "<for><value>true</value><value>false</value>"),
        source.replace("<for>", "<for><value>notBool</value>"),
        source.replace("<role>", "<value true='keep'/><role>"),
        source.replace("<use>", "<use extra='keep'>"),
    ] {
        assert_ne!(bad, source);
        assert!(read_form(FormDialect::Edt, bad.as_bytes()).is_err());
    }
}

#[test]
fn chart_style_font_preserves_all_four_boolean_overrides() {
    use formats_xml::form::{read_chart_sidecar, write_chart_sidecar};
    let source = r#"<?xml version="1.0" encoding="UTF-8"?><chart:Chart xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:chart="http://g5.1c.ru/v8/dt/chart/model" xmlns:core="http://g5.1c.ru/v8/dt/mcore"><translucenceMode>Auto</translucenceMode><pointsScale><titleArea><font xsi:type="core:FontRef"><font>Style.TextFont</font><height>11.0</height><bold>true</bold><italic>false</italic><underline>false</underline><strikeout>false</strikeout></font></titleArea></pointsScale></chart:Chart>"#;
    let source = source.replace("?>", "?>\r\n") + "\r\n";
    let chart = read_chart_sidecar(source.as_bytes()).unwrap();
    let encoded = write_chart_sidecar(&chart).unwrap();
    assert_eq!(read_chart_sidecar(&encoded).unwrap(), chart);
    for bad in [
        source.replace("<bold>true", "<bold>invalid"),
        source.replace("<bold>true</bold>", "<bold>true</bold><bold>false</bold>"),
        source.replace("<italic>", "<italic unknown='keep'>"),
        source.replace(
            "<underline>false</underline>",
            "<underline><child/></underline>",
        ),
    ] {
        assert!(read_chart_sidecar(bad.as_bytes()).is_err());
    }
}

#[test]
fn optional_form_root_properties_and_table_fonts_are_written() {
    use morph1c_core::{ir::FontRef, spec::forms::form_root as fr};
    let mut form = FormBody::new();
    form.attributes = vec![
        (
            fr::F_SCALING_MODE,
            PropertyValue::Enum(Token::new("Normal")),
        ),
        (
            fr::F_CHILDREN_ALIGN,
            PropertyValue::Enum(Token::new("None")),
        ),
        (
            fr::F_SETTINGS_STORAGE,
            PropertyValue::Str("SettingsStorage.UserSettings".into()),
        ),
    ];
    let font = FontRef {
        auto: false,
        font_ref: Some("System.DefaultGUIFont".into()),
        face_name: None,
        height: Some("10.0".into()),
        bold: Some(false),
        italic: Some(false),
        underline: Some(false),
        strikeout: Some(false),
        scale: None,
    };
    let mut table = item("Table", 1, vec![]);
    table.font = Some(font.clone());
    table.title_font = Some(font.clone());
    form.items.push(table);
    for dialect in [FormDialect::Edt, FormDialect::Designer] {
        let encoded = write_form(dialect, &form).unwrap();
        let decoded = read_form(dialect, &encoded).unwrap();
        assert_eq!(decoded.items[0].font.as_ref(), Some(&font));
        assert_eq!(decoded.items[0].title_font.as_ref(), Some(&font));
        for (id, value) in &form.attributes {
            assert_eq!(
                decoded
                    .attributes
                    .iter()
                    .find(|(other, _)| id == other)
                    .unwrap()
                    .1,
                *value
            );
        }
    }
}

#[test]
fn spreadsheet_show_groups_false_uses_exact_paired_default_spelling() {
    use morph1c_core::spec::forms::controls::form_field as ff;
    let mut form = FormBody::new();
    form.items.push(item(
        "SpreadsheetDocumentField",
        1,
        vec![(ff::F_EXT_SHOW_GROUPS, PropertyValue::Bool(false))],
    ));
    let native = write_form(FormDialect::Designer, &form).unwrap();
    assert!(
        std::str::from_utf8(&native)
            .unwrap()
            .contains("<ShowGroups>false</ShowGroups>")
    );
    let parsed = read_form(FormDialect::Designer, &native).unwrap();
    let edt = write_form(FormDialect::Edt, &parsed).unwrap();
    assert!(!std::str::from_utf8(&edt).unwrap().contains("<showGroups>"));
    let parsed_edt = read_form(FormDialect::Edt, &edt).unwrap();
    assert_eq!(parsed.items[0].ext_info, parsed_edt.items[0].ext_info);
    let native_again = write_form(FormDialect::Designer, &parsed_edt).unwrap();
    assert!(
        std::str::from_utf8(&native_again)
            .unwrap()
            .contains("<ShowGroups>false</ShowGroups>")
    );
    form.items[0].ext_info = vec![(ff::F_EXT_SHOW_GROUPS, PropertyValue::Bool(true))];
    let edt = write_form(FormDialect::Edt, &form).unwrap();
    assert!(
        std::str::from_utf8(&edt)
            .unwrap()
            .contains("<showGroups>true</showGroups>")
    );
}

#[test]
#[ignore = "requires immutable genuine UH native/EDT corpus on F"]
fn genuine_uha_chart_font_pair_preserves_boolean_overrides() {
    use formats_xml::form::{read_chart_sidecar, write_chart_sidecar};
    let root = std::path::Path::new(
        "F:/ibcmd/lab/07/oracle-uha83-r1/authentic-workspace/OracleConfiguration/src/Reports/МатрицаРисков/Forms/ФормаОтчета",
    );
    let native = std::fs::read("F:/ibcmd/lab/04/release-20261001/rc/out/uha8327_db_r1/tree/Reports/МатрицаРисков/Forms/ФормаОтчета/Ext/Form.xml").unwrap();
    let edt = std::fs::read(root.join("Attributes/Диаграмма/ExtInfo/Chart.chart")).unwrap();
    assert!(native.len() <= 32 * 1024 * 1024 && edt.len() <= 32 * 1024 * 1024);
    let form = read_form(FormDialect::Designer, &native).unwrap();
    let chart = form
        .data_attributes
        .iter()
        .find_map(|a| a.chart_settings.as_ref())
        .unwrap();
    let cross = write_chart_sidecar(chart).unwrap();
    let decoded = read_chart_sidecar(&cross).unwrap();
    let genuine = read_chart_sidecar(&edt).unwrap();
    for name in ["pointsScale", "valuesScale"] {
        let child = |settings: &morph1c_core::ir::form::ChartSettings| {
            settings
                .fields
                .iter()
                .find(|(n, _)| n == name)
                .unwrap()
                .1
                .clone()
        };
        assert_eq!(child(&decoded), child(&genuine));
    }
    let rewritten = write_form(FormDialect::Designer, &form).unwrap();
    assert_eq!(
        read_form(FormDialect::Designer, &rewritten)
            .unwrap()
            .data_attributes
            .iter()
            .find_map(|a| a.chart_settings.as_ref()),
        Some(chart)
    );
}
