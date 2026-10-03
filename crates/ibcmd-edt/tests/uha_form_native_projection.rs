use formats_xml::form::{FormDialect, read_form, write_form};
use morph1c_core::{
    ir::FormBody,
    version::{FormatVersion, with_roundtrip_target},
};
use sha2::Digest;

fn source(version: FormatVersion, control: &str) -> Vec<u8> {
    with_roundtrip_target(version, || {
        let bytes = write_form(FormDialect::Designer, &FormBody::new()).unwrap();
        let xml = String::from_utf8(bytes).unwrap();
        xml.replace(
            "\t<Attributes/>",
            &format!("\t<ChildItems>\r\n{control}\r\n\t</ChildItems>\r\n\t<Attributes/>"),
        )
        .into_bytes()
    })
}
fn cross(version: FormatVersion, bytes: &[u8]) -> Vec<u8> {
    with_roundtrip_target(version, || {
        let native = read_form(FormDialect::Designer, bytes).unwrap();
        let current = write_form(FormDialect::Designer, &native).unwrap();
        assert!(
            current == bytes,
            "same-native mismatch: first={:?}, lengths={}/{}, sha256={:x}/{:x}",
            current.iter().zip(bytes).position(|(a, b)| a != b),
            current.len(),
            bytes.len(),
            sha2::Sha256::digest(&current),
            sha2::Sha256::digest(bytes)
        );
        let edt = write_form(FormDialect::Edt, &native).unwrap();
        let current = read_form(FormDialect::Edt, &edt).unwrap();
        write_form(FormDialect::Designer, &current).unwrap()
    })
}
fn order(bytes: &[u8], tag: &str) -> Vec<String> {
    formats_xml::parse(bytes)
        .unwrap()
        .root
        .child("ChildItems")
        .unwrap()
        .child(tag)
        .unwrap()
        .children
        .iter()
        .map(|e| e.local.clone())
        .collect()
}
fn before(order: &[String], first: &str, second: &str) {
    assert!(
        order.iter().position(|x| x == first).unwrap()
            < order.iter().position(|x| x == second).unwrap(),
        "{first} must precede {second}: {order:?}"
    );
}
#[test]
fn native_field_body_and_actual_extension_follow_sdk_feature_order() {
    for minor in [20, 21] {
        let version = FormatVersion::new(2, minor);
        let bytes = source(
            version,
            "\t\t<InputField name=\"Input\" id=\"1\">\r\n\t\t\t<ShowInHeader>false</ShowInHeader>\r\n\t\t\t<FooterDataPath>Footer</FooterDataPath>\r\n\t\t\t<MarkNegatives>true</MarkNegatives>\r\n\t\t\t<ChoiceButton>true</ChoiceButton>\r\n\t\t</InputField>",
        );
        let native = cross(version, &bytes);
        let tags = order(&native, "InputField");
        before(&tags, "ShowInHeader", "FooterDataPath");
        before(&tags, "MarkNegatives", "ChoiceButton");
        let mut edited = read_form(FormDialect::Designer, &native).unwrap();
        let field = &mut edited.items[0];
        for (_, value) in &mut field.ext_info {
            if matches!(value, morph1c_core::ir::PropertyValue::Bool(true)) {
                *value = morph1c_core::ir::PropertyValue::Bool(false);
            }
        }
        let current =
            with_roundtrip_target(version, || write_form(FormDialect::Designer, &edited)).unwrap();
        assert!(
            String::from_utf8(current)
                .unwrap()
                .contains("<MarkNegatives>false</MarkNegatives>")
        );
    }
}
#[test]
fn native_spreadsheet_scaling_follows_full_current_extension_not_sparse_witness() {
    for minor in [20, 21] {
        let version = FormatVersion::new(2, minor);
        let bytes = source(
            version,
            "\t\t<SpreadSheetDocumentField name=\"Sheet\" id=\"1\">\r\n\t\t\t<Protection>true</Protection>\r\n\t\t\t<Output>Use</Output>\r\n\t\t\t<Edit>true</Edit>\r\n\t\t\t<ViewScalingMode>Normal</ViewScalingMode>\r\n\t\t</SpreadSheetDocumentField>",
        );
        let current = cross(version, &bytes);
        let tags = order(&current, "SpreadSheetDocumentField");
        for tag in ["Protection", "Output", "Edit"] {
            before(&tags, tag, "ViewScalingMode");
        }
    }
}
#[test]
fn native_button_geometry_is_current_and_native_source_order_remains_exact() {
    for minor in [20, 21] {
        let version = FormatVersion::new(2, minor);
        let bytes = source(
            version,
            "\t\t<Button name=\"Button\" id=\"1\">\r\n\t\t\t<TitleHeight>2</TitleHeight>\r\n\t\t\t<Representation>PictureAndText</Representation>\r\n\t\t\t<Height>2</Height>\r\n\t\t\t<AutoMaxHeight>false</AutoMaxHeight>\r\n\t\t\t<Type>CommandBarButton</Type>\r\n\t\t</Button>",
        );
        let tags = order(&cross(version, &bytes), "Button");
        before(&tags, "TitleHeight", "Representation");
        before(&tags, "Height", "AutoMaxHeight");
        // A legal authored same-native order is lexical data; it wins over the
        // target's canonical cross-source scalar order, without cached values.
        let reordered = String::from_utf8(bytes)
            .unwrap()
            .replace(
                "\t\t\t<Height>2</Height>\r\n\t\t\t<AutoMaxHeight>false</AutoMaxHeight>",
                "\t\t\t<AutoMaxHeight>false</AutoMaxHeight>\r\n\t\t\t<Height>2</Height>",
            )
            .into_bytes();
        with_roundtrip_target(version, || {
            let mut current = read_form(FormDialect::Designer, &reordered).unwrap();
            assert_eq!(
                write_form(FormDialect::Designer, &current).unwrap(),
                reordered
            );
            let id = morph1c_core::spec::forms::controls::button::F_HEIGHT;
            current.items[0]
                .properties
                .iter_mut()
                .find(|(field, _)| *field == id)
                .unwrap()
                .1 = morph1c_core::ir::PropertyValue::Int(3);
            let output =
                String::from_utf8(write_form(FormDialect::Designer, &current).unwrap()).unwrap();
            assert!(output.contains("<Height>3</Height>"));
            assert!(!output.contains("<Height>2</Height>"));
        });
    }
}
#[test]
fn scalar_order_does_not_sort_events_children_or_accept_unknown_cells() {
    let version = FormatVersion::new(2, 20);
    let bytes = source(
        version,
        "\t\t<InputField name=\"Input\" id=\"1\">\r\n\t\t\t<Events>\r\n\t\t\t\t<Event name=\"ZSymbol\">First</Event>\r\n\t\t\t\t<Event name=\"ASymbol\">Second</Event>\r\n\t\t\t</Events>\r\n\t\t</InputField>",
    );
    let native = cross(version, &bytes);
    let doc = formats_xml::parse(&native).unwrap();
    let events = doc
        .root
        .child("ChildItems")
        .unwrap()
        .child("InputField")
        .unwrap()
        .child("Events")
        .unwrap();
    assert_eq!(
        events
            .children
            .iter()
            .map(|e| e.attr("name").unwrap().value.as_str())
            .collect::<Vec<_>>(),
        ["ZSymbol", "ASymbol"]
    );
    let unknown = String::from_utf8(bytes)
        .unwrap()
        .replace("<Events>", "<UnknownScalar>true</UnknownScalar><Events>")
        .into_bytes();
    assert!(read_form(FormDialect::Designer, &unknown).is_err());
}

fn dynamic_body(content: &str) -> FormBody {
    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<form:Form xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:core=\"http://g5.1c.ru/v8/dt/mcore\" xmlns:form=\"http://g5.1c.ru/v8/dt/form\" xmlns:schema=\"http://g5.1c.ru/v8/dt/data-composition-system/schema\" xmlns:settings=\"http://g5.1c.ru/v8/dt/data-composition-system/settings\">\r\n<attributes><name>List</name><valueType><types>DynamicList</types></valueType><view><common>true</common></view><edit><common>true</common></edit><extInfo xsi:type=\"form:DynamicListExtInfo\">{content}</extInfo></attributes>\r\n</form:Form>\r\n"
    );
    read_form(FormDialect::Edt, xml.as_bytes()).unwrap()
}
fn native_use_always(body: &FormBody, minor: u16) -> Vec<String> {
    let bytes = with_roundtrip_target(FormatVersion::new(2, minor), || {
        write_form(FormDialect::Designer, body)
    })
    .unwrap();
    formats_xml::parse(&bytes)
        .unwrap()
        .root
        .child("Attributes")
        .unwrap()
        .child("Attribute")
        .unwrap()
        .child("UseAlways")
        .unwrap()
        .children
        .iter()
        .map(|field| field.text.clone())
        .collect()
}
#[test]
fn explicit_query_alias_does_not_invent_bilingual_platform_field() {
    let mut body = dynamic_body(
        "<queryText>ВЫБРАТЬ R.Регистратор КАК Регистратор ИЗ РегистрБухгалтерии.Register КАК R</queryText><mainTable>AccountingRegister.Register</mainTable><autoFillAvailableFields>true</autoFillAvailableFields><customQuery>true</customQuery>",
    );
    body.data_attributes[0].not_default_use_always =
        vec!["List.Регистратор".into(), "List.Recorder".into()];
    for minor in [20, 21] {
        assert_eq!(
            native_use_always(&body, minor),
            ["List.Регистратор", "~List.Recorder"]
        );
    }
    body.data_attributes[0]
        .dynamic_list
        .as_mut()
        .unwrap()
        .query_text =
        Some("SELECT R.Recorder AS Recorder FROM AccountingRegister.Register AS R".into());
    for minor in [20, 21] {
        assert_eq!(
            native_use_always(&body, minor),
            ["~List.Регистратор", "List.Recorder"]
        );
    }
    // The semantic reference values never acquire a native unresolved marker.
    assert_eq!(
        body.data_attributes[0].not_default_use_always,
        ["List.Регистратор", "List.Recorder"]
    );
}
#[test]
fn manual_available_fields_are_current_and_empty_schema_is_not_universal_availability() {
    let mut body = dynamic_body(
        "<queryText>SELECT T.A AS A FROM Catalog.Source AS T</queryText><customQuery>true</customQuery><fields xsi:type=\"schema:DataCompositionSchemaDataSetField\"><dataPath>A</dataPath><field>A</field></fields>",
    );
    body.data_attributes[0].not_default_use_always =
        vec!["List.A".into(), "List.B".into(), "Unrelated.A".into()];
    for minor in [20, 21] {
        assert_eq!(
            native_use_always(&body, minor),
            ["List.A", "~List.B", "Unrelated.A"]
        );
    }
    body.data_attributes[0]
        .dynamic_list
        .as_mut()
        .unwrap()
        .fields[0]
        .data_path = "B".into();
    assert_eq!(
        native_use_always(&body, 20),
        ["~List.A", "List.B", "Unrelated.A"]
    );
    body.data_attributes[0]
        .dynamic_list
        .as_mut()
        .unwrap()
        .fields
        .clear();
    assert_eq!(
        native_use_always(&body, 21),
        ["~List.A", "~List.B", "Unrelated.A"]
    );
}
#[test]
fn valid_query_projection_has_no_physical_byte_or_delimiter_depth_quota() {
    let mut body = dynamic_body(
        "<queryText>SELECT T.A AS A FROM Catalog.Source AS T</queryText><customQuery>true</customQuery><autoFillAvailableFields>true</autoFillAvailableFields>",
    );
    body.data_attributes[0].not_default_use_always = vec!["List.A".into(), "List.Missing".into()];
    let query = format!(
        "//{}\nSELECT F({}T.A{}) AS A FROM Catalog.Source AS T",
        "x".repeat(4 * 1024 * 1024 + 1),
        "(".repeat(129),
        ")".repeat(129)
    );
    body.data_attributes[0]
        .dynamic_list
        .as_mut()
        .unwrap()
        .query_text = Some(query);
    assert_eq!(native_use_always(&body, 20), ["List.A", "~List.Missing"]);
    body.data_attributes[0]
        .dynamic_list
        .as_mut()
        .unwrap()
        .query_text = Some("SELECT F(T.A AS A FROM Catalog.Source AS T".into());
    assert!(
        with_roundtrip_target(FormatVersion::new(2, 20), || write_form(
            FormDialect::Designer,
            &body
        ))
        .is_err()
    );
}

#[test]
fn root_current_scalar_order_is_profile_correct_and_source_order_is_lexical() {
    use morph1c_core::ir::{PropertyValue, Token};
    use morph1c_core::spec::forms::form_root as fr;
    for minor in [20, 21] {
        let version = FormatVersion::new(2, minor);
        let mut body = FormBody::new();
        body.attributes.extend([
            (
                fr::F_HORIZONTAL_SPACING,
                PropertyValue::Enum(Token::new("Half")),
            ),
            (
                fr::F_VERTICAL_SPACING,
                PropertyValue::Enum(Token::new("Double")),
            ),
            (
                fr::F_AUTO_SAVE_DATA_IN_SETTINGS,
                PropertyValue::Enum(Token::new("Use")),
            ),
            (
                fr::F_ENTER_KEY_BEHAVIOR,
                PropertyValue::Enum(Token::new("DefaultButton")),
            ),
        ]);
        let bytes =
            with_roundtrip_target(version, || write_form(FormDialect::Designer, &body)).unwrap();
        let tags: Vec<_> = formats_xml::parse(&bytes)
            .unwrap()
            .root
            .children
            .iter()
            .map(|e| e.local.clone())
            .collect();
        before(&tags, "HorizontalSpacing", "VerticalSpacing");
        before(&tags, "EnterKeyBehavior", "AutoSaveDataInSettings");
        let source = String::from_utf8(bytes).unwrap().replace("<HorizontalSpacing>Half</HorizontalSpacing>\r\n\t<VerticalSpacing>Double</VerticalSpacing>", "<VerticalSpacing>Double</VerticalSpacing>\r\n\t<HorizontalSpacing>Half</HorizontalSpacing>").into_bytes();
        with_roundtrip_target(version, || {
            let mut current = read_form(FormDialect::Designer, &source).unwrap();
            assert_eq!(write_form(FormDialect::Designer, &current).unwrap(), source);
            current
                .attributes
                .iter_mut()
                .find(|(id, _)| *id == fr::F_HORIZONTAL_SPACING)
                .unwrap()
                .1 = PropertyValue::Enum(Token::new("Double"));
            assert!(
                String::from_utf8(write_form(FormDialect::Designer, &current).unwrap())
                    .unwrap()
                    .contains("<HorizontalSpacing>Double</HorizontalSpacing>")
            );
        });
    }
}
#[test]
fn graphical_schema_current_zero_and_native_factory_defaults_remain_distinct() {
    for minor in [20, 21] {
        let version = FormatVersion::new(2, minor);
        let bytes = source(
            version,
            "\t\t<GraphicalSchemaField name=\"Scheme\" id=\"1\">\r\n\t\t\t<Width>0</Width>\r\n\t\t\t<Height>0</Height>\r\n\t\t</GraphicalSchemaField>",
        );
        let native = cross(version, &bytes);
        let tags = order(&native, "GraphicalSchemaField");
        assert!(tags.iter().any(|tag| tag == "Width"));
        assert!(tags.iter().any(|tag| tag == "Height"));
        let mut current =
            with_roundtrip_target(version, || read_form(FormDialect::Designer, &bytes)).unwrap();
        let id = morph1c_core::spec::forms::controls::form_field::F_EXT_WIDTH;
        current.items[0]
            .ext_info
            .iter_mut()
            .find(|(field, _)| *field == id)
            .unwrap()
            .1 = morph1c_core::ir::PropertyValue::Int(17);
        let output =
            with_roundtrip_target(version, || write_form(FormDialect::Designer, &current)).unwrap();
        assert!(
            String::from_utf8(output)
                .unwrap()
                .contains("<Width>17</Width>")
        );
        let defaults = source(
            version,
            "\t\t<GraphicalSchemaField name=\"DefaultScheme\" id=\"2\">\r\n\t\t\t<Width>50</Width>\r\n\t\t\t<Height>10</Height>\r\n\t\t</GraphicalSchemaField>",
        );
        let canonical = cross(version, &defaults);
        let tags = order(&canonical, "GraphicalSchemaField");
        assert!(!tags.iter().any(|tag| tag == "Width" || tag == "Height"));
        let absent = source(
            version,
            "\t\t<GraphicalSchemaField name=\"DefaultScheme\" id=\"2\"/>",
        );
        let body =
            with_roundtrip_target(version, || read_form(FormDialect::Designer, &absent)).unwrap();
        let edt = with_roundtrip_target(version, || write_form(FormDialect::Edt, &body)).unwrap();
        assert!(String::from_utf8_lossy(&edt).contains("<width>50</width>"));
        assert!(String::from_utf8_lossy(&edt).contains("<height>10</height>"));
        let zero_edt = String::from_utf8(edt)
            .unwrap()
            .replace("<width>50</width>", "")
            .replace("<height>10</height>", "");
        let zero = read_form(FormDialect::Edt, zero_edt.as_bytes()).unwrap();
        let native =
            with_roundtrip_target(version, || write_form(FormDialect::Designer, &zero)).unwrap();
        assert!(String::from_utf8_lossy(&native).contains("<Width>0</Width>"));
        assert!(String::from_utf8_lossy(&native).contains("<Height>0</Height>"));
    }
}
#[test]
fn addition_enabled_context_defaults_preserve_current_false_and_true() {
    for minor in [20, 21] {
        let version = FormatVersion::new(2, minor);
        let bytes = source(
            version,
            "\t\t<Table name=\"Table\" id=\"1\">\r\n\t\t\t<SearchStringAddition name=\"Search\" id=\"2\">\r\n\t\t\t\t<Enabled>false</Enabled>\r\n\t\t\t</SearchStringAddition>\r\n\t\t</Table>",
        );
        let current = cross(version, &bytes);
        assert!(
            String::from_utf8(current)
                .unwrap()
                .contains("<Enabled>false</Enabled>")
        );
        let mut body =
            with_roundtrip_target(version, || read_form(FormDialect::Designer, &bytes)).unwrap();
        let id = morph1c_core::spec::forms::controls::table::F_ADDITION_ENABLED;
        let addition = &mut body.items[0].additions[0];
        addition.properties.retain(|(field, _)| *field != id);
        let edt = with_roundtrip_target(version, || write_form(FormDialect::Edt, &body)).unwrap();
        let emitted = formats_xml::parse(&edt).unwrap();
        let named_addition = emitted
            .root
            .children
            .iter()
            .find(|item| {
                item.local == "items" && item.child("name").is_some_and(|n| n.text == "Table")
            })
            .unwrap()
            .child("searchStringAddition")
            .unwrap();
        assert!(
            named_addition.child("enabled").is_none(),
            "ordinary named Addition current=true must use its sparse default: {:?}",
            named_addition
                .children
                .iter()
                .map(|c| (c.local.as_str(), c.text.as_str()))
                .collect::<Vec<_>>()
        );
        let current = read_form(FormDialect::Edt, &edt).unwrap();
        let native =
            with_roundtrip_target(version, || write_form(FormDialect::Designer, &current)).unwrap();
        assert!(!String::from_utf8_lossy(&native).contains("<Enabled>false</Enabled>"));

        // The same CURRENT true tuple requires an explicit EDT value in a
        // Gantt auto-table: its factory/context default is false, not the
        // ordinary Table/PDF named-addition default true.
        let mut auto = body.clone();
        let mut gantt = morph1c_core::ir::form::FormItem::new(
            morph1c_core::ir::form::FormControlKind::new("GanttChartField"),
            "Gantt",
            3,
        );
        gantt.auto_table = Some(Box::new(auto.items.remove(0)));
        auto.items = vec![gantt];
        let edt_auto =
            with_roundtrip_target(version, || write_form(FormDialect::Edt, &auto)).unwrap();
        assert!(String::from_utf8_lossy(&edt_auto).contains("<enabled>true</enabled>"));
        let parsed_true = read_form(FormDialect::Edt, &edt_auto).unwrap();
        assert!(
            parsed_true.items[0].auto_table.as_ref().unwrap().additions[0]
                .get(id)
                .is_none()
        );
        let omitted = String::from_utf8(edt_auto)
            .unwrap()
            .replace("<enabled>true</enabled>", "");
        let mut parsed_false = read_form(FormDialect::Edt, omitted.as_bytes()).unwrap();
        assert_eq!(
            parsed_false.items[0].auto_table.as_ref().unwrap().additions[0].get(id),
            Some(&morph1c_core::ir::PropertyValue::Bool(false))
        );
        let native_false =
            with_roundtrip_target(version, || write_form(FormDialect::Designer, &parsed_false))
                .unwrap();
        assert!(String::from_utf8_lossy(&native_false).contains("<Enabled>false</Enabled>"));
        parsed_false.items[0].auto_table.as_mut().unwrap().additions[0]
            .properties
            .retain(|(field, _)| *field != id);
        let edited =
            with_roundtrip_target(version, || write_form(FormDialect::Edt, &parsed_false)).unwrap();
        assert!(String::from_utf8_lossy(&edited).contains("<enabled>true</enabled>"));

        // Explicit false in an ordinary named addition remains false. An
        // authored explicit true is retained lexically; a CURRENT edit wins.
        let named_explicit = String::from_utf8(edt).unwrap().replace(
            "<searchStringAddition>",
            "<searchStringAddition><enabled>true</enabled>",
        );
        let mut named = read_form(FormDialect::Edt, named_explicit.as_bytes()).unwrap();
        let returned =
            with_roundtrip_target(version, || write_form(FormDialect::Edt, &named)).unwrap();
        assert!(String::from_utf8_lossy(&returned).contains("<enabled>true</enabled>"));
        named.items[0].additions[0]
            .properties
            .push((id, morph1c_core::ir::PropertyValue::Bool(false)));
        let edited =
            with_roundtrip_target(version, || write_form(FormDialect::Edt, &named)).unwrap();
        assert!(String::from_utf8_lossy(&edited).contains("<enabled>false</enabled>"));
        let doc = formats_xml::parse(&edited).unwrap();
        let named_table = doc
            .root
            .children
            .iter()
            .find(|item| {
                item.local == "items" && item.child("name").is_some_and(|name| name.text == "Table")
            })
            .unwrap();
        let enabled: Vec<_> = named_table
            .child("searchStringAddition")
            .unwrap()
            .children
            .iter()
            .filter(|child| child.prefix.is_empty() && child.local == "enabled")
            .map(|child| child.text.as_str())
            .collect();
        assert_eq!(
            enabled,
            ["false"],
            "CURRENT edit must replace the authored true in this addition"
        );
    }
}

#[test]
fn gantt_auto_table_path_is_derived_from_current_parent_without_changing_other_tables() {
    use morph1c_core::{
        ir::PropertyValue,
        spec::forms::controls::{form_field as ff, table as tb},
    };
    for minor in [20, 21] {
        let version = FormatVersion::new(2, minor);
        let xml = br#"<form:Form xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:form="http://g5.1c.ru/v8/dt/form" xmlns:core="http://g5.1c.ru/v8/dt/mcore"><items xsi:type="form:FormField"><name>Gantt</name><id>1</id><dataPath xsi:type="form:DataPath"><segments>CurrentParent</segments></dataPath><type>GanttChartField</type><extInfo xsi:type="form:GanttChartFieldExtInfo"><autoTable><name>Auto</name><id>2</id><readOnly>true</readOnly></autoTable></extInfo></items><items xsi:type="form:Table"><name>Other</name><id>3</id><dataPath xsi:type="form:DataPath"><segments>Independent</segments></dataPath></items></form:Form>"#;
        let xml = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n{}\r\n",
            std::str::from_utf8(xml).unwrap().replace("><", ">\r\n<")
        );
        let mut current = morph1c_core::version::with_source_version(Some(version), || {
            read_form(FormDialect::Edt, xml.as_bytes())
        })
        .unwrap();
        assert!(
            !current.items[0]
                .auto_table
                .as_ref()
                .unwrap()
                .properties
                .iter()
                .any(|(id, _)| *id == tb::F_DATA_PATH)
        );
        for value in ["CurrentParent", "EditedParent"] {
            current.items[0]
                .properties
                .iter_mut()
                .find(|(id, _)| *id == ff::F_DATA_PATH)
                .unwrap()
                .1 = PropertyValue::Ref(value.into());
            let native =
                with_roundtrip_target(version, || write_form(FormDialect::Designer, &current))
                    .unwrap();
            let parsed = formats_xml::parse(&native).unwrap();
            let children = parsed.root.child("ChildItems").unwrap();
            assert_eq!(
                children
                    .child("GanttChartField")
                    .unwrap()
                    .child("Table")
                    .unwrap()
                    .child("DataPath")
                    .unwrap()
                    .text,
                value
            );
            assert_eq!(
                children
                    .child("Table")
                    .unwrap()
                    .child("DataPath")
                    .unwrap()
                    .text,
                "Independent"
            );
            let returned = morph1c_core::version::with_source_version(Some(version), || {
                read_form(FormDialect::Designer, &native)
            })
            .unwrap();
            assert_eq!(
                serde_json::to_value(&returned).unwrap(),
                serde_json::to_value(&current).unwrap()
            );
            let rewritten =
                with_roundtrip_target(version, || write_form(FormDialect::Designer, &returned))
                    .unwrap();
            assert!(
                native == rewritten,
                "current inherited path must rewrite exactly"
            );
        }
        current.items[0]
            .auto_table
            .as_mut()
            .unwrap()
            .properties
            .push((tb::F_DATA_PATH, PropertyValue::Ref("ExplicitChild".into())));
        let native =
            with_roundtrip_target(version, || write_form(FormDialect::Designer, &current)).unwrap();
        let parsed = formats_xml::parse(&native).unwrap();
        assert_eq!(
            parsed
                .root
                .child("ChildItems")
                .unwrap()
                .child("GanttChartField")
                .unwrap()
                .child("Table")
                .unwrap()
                .child("DataPath")
                .unwrap()
                .text,
            "ExplicitChild"
        );
        let returned = read_form(FormDialect::Designer, &native).unwrap();
        assert!(
            returned.items[0]
                .auto_table
                .as_ref()
                .unwrap()
                .properties
                .contains(&(tb::F_DATA_PATH, PropertyValue::Ref("ExplicitChild".into())))
        );
        let unknown = String::from_utf8(native).unwrap().replace(
            "<DataPath>ExplicitChild</DataPath>",
            "<DataPath>ExplicitChild</DataPath><UnknownAutoTableValue>1</UnknownAutoTableValue>",
        );
        assert!(read_form(FormDialect::Designer, unknown.as_bytes()).is_err());
    }
}

#[test]
fn native_8_5_field_scalar_order_preserves_current_values_and_source_order() {
    use morph1c_core::ir::{PropertyValue, Token};
    use morph1c_core::spec::forms::controls::form_field as ff;
    for minor in [20, 21] {
        let version = FormatVersion::new(2, minor);
        let original = source(
            version,
            "\t\t<InputField name=\"Input\" id=\"1\">\r\n\t\t\t<EditMode>EnterOnInput</EditMode>\r\n\t\t\t<AutoEditMode>true</AutoEditMode>\r\n\t\t\t<ShowInHeader>false</ShowInHeader>\r\n\t\t\t<FooterDataPath>Footer</FooterDataPath>\r\n\t\t\t<MarkRequiredComplete>true</MarkRequiredComplete>\r\n\t\t</InputField>\r\n\t\t<LabelField name=\"Label\" id=\"2\">\r\n\t\t\t<CellHyperlinkDisplayVariant>Always</CellHyperlinkDisplayVariant>\r\n\t\t\t<CellHyperlinkRepresentation>Show</CellHyperlinkRepresentation>\r\n\t\t</LabelField>",
        );
        let projected = cross(version, &original);
        let input = order(&projected, "InputField");
        let label = order(&projected, "LabelField");
        if minor == 21 {
            before(&input, "ShowInHeader", "AutoEditMode");
            before(&input, "FooterDataPath", "AutoEditMode");
            before(&input, "MarkRequiredComplete", "AutoEditMode");
            before(
                &label,
                "CellHyperlinkRepresentation",
                "CellHyperlinkDisplayVariant",
            );
        } else {
            before(&input, "AutoEditMode", "ShowInHeader");
            before(
                &label,
                "CellHyperlinkDisplayVariant",
                "CellHyperlinkRepresentation",
            );
        }
        let mut current =
            with_roundtrip_target(version, || read_form(FormDialect::Designer, &original)).unwrap();
        current.items[0]
            .properties
            .iter_mut()
            .find(|(id, _)| *id == ff::F_EDIT_MODE)
            .unwrap()
            .1 = PropertyValue::Enum(Token::new("ReadOnly"));
        current.items[1]
            .properties
            .iter_mut()
            .find(|(id, _)| *id == ff::F_CELL_HYPERLINK_DISPLAY_VARIANT)
            .unwrap()
            .1 = PropertyValue::Enum(Token::new("Auto"));
        let edited =
            with_roundtrip_target(version, || write_form(FormDialect::Designer, &current)).unwrap();
        assert!(!String::from_utf8_lossy(&edited).contains("<AutoEditMode>true</AutoEditMode>"));
        assert!(String::from_utf8_lossy(&edited).contains("<EditMode>ReadOnly</EditMode>"));
        assert!(
            String::from_utf8_lossy(&edited)
                .contains("<CellHyperlinkDisplayVariant>Auto</CellHyperlinkDisplayVariant>")
        );
    }
}

#[test]
fn whole_metadata_projection_resolves_current_wildcards_names_and_register_roles() {
    use formats_xml::form::{FormProjectionContext, write_form_with_context};
    use morph1c_core::ir::{Configuration, MetadataObject, ObjectKind, PropertyValue};
    use morph1c_core::spec::metadata::information_register as ir;
    let mut cfg = Configuration::new();
    let mut register = MetadataObject::new(
        ObjectKind::new("InformationRegister"),
        "Register",
        morph1c_core::ir::Uuid([1; 16]),
    );
    register.children.push(MetadataObject::new(
        ObjectKind::new("InformationRegister.Dimension"),
        "CurrentKey",
        morph1c_core::ir::Uuid([2; 16]),
    ));
    register.children.push(MetadataObject::new(
        ObjectKind::new("InformationRegister.Resource"),
        "CurrentValue",
        morph1c_core::ir::Uuid([3; 16]),
    ));
    cfg.objects.push(register);
    let mut body = dynamic_body(
        "<mainTable>InformationRegister.Register</mainTable><customQuery>true</customQuery><autoFillAvailableFields>true</autoFillAvailableFields><queryText>SELECT R.* FROM InformationRegister.Register AS R</queryText>",
    );
    body.data_attributes[0].not_default_use_always = [
        "List.CurrentKey",
        "List.CurrentValue",
        "List.OldValue",
        "List.Recorder",
        "List.Period",
    ]
    .map(morph1c_core::ir::form::DataPathSpec::from_form_text)
    .to_vec();
    let paths = |cfg: &Configuration, body: &FormBody, minor| {
        let context = FormProjectionContext::new(cfg).unwrap();
        let bytes = with_roundtrip_target(FormatVersion::new(2, minor), || {
            write_form_with_context(FormDialect::Designer, body, &context)
        })
        .unwrap();
        let document = formats_xml::parse(&bytes).unwrap();
        document
            .root
            .child("Attributes")
            .unwrap()
            .child("Attribute")
            .unwrap()
            .child("UseAlways")
            .unwrap()
            .children
            .iter()
            .map(|n| n.text.clone())
            .collect::<Vec<_>>()
    };
    for minor in [20, 21] {
        assert_eq!(
            paths(&cfg, &body, minor),
            [
                "List.CurrentKey",
                "List.CurrentValue",
                "~List.OldValue",
                "~List.Recorder",
                "~List.Period"
            ]
        );
        cfg.objects[0].children[1].name = "OldValue".into();
        assert_eq!(
            paths(&cfg, &body, minor),
            [
                "List.CurrentKey",
                "~List.CurrentValue",
                "List.OldValue",
                "~List.Recorder",
                "~List.Period"
            ]
        );
        cfg.objects[0].children[1].name = "CurrentValue".into();
        cfg.objects[0].properties.push((
            ir::F_WRITE_MODE,
            PropertyValue::Enum(morph1c_core::ir::Token::new("RecorderSubordinate")),
        ));
        cfg.objects[0].properties.push((
            ir::F_PERIODICITY,
            PropertyValue::Enum(morph1c_core::ir::Token::new("WithinDay")),
        ));
        assert_eq!(
            paths(&cfg, &body, minor),
            [
                "List.CurrentKey",
                "List.CurrentValue",
                "~List.OldValue",
                "List.Recorder",
                "List.Period"
            ]
        );
        cfg.objects[0].properties.clear();
        body.data_attributes[0]
            .dynamic_list
            .as_mut()
            .unwrap()
            .query_text =
            Some("SELECT R.CurrentValue AS OldValue FROM InformationRegister.Register AS R".into());
        assert_eq!(
            paths(&cfg, &body, minor),
            [
                "List.CurrentKey",
                "~List.CurrentValue",
                "List.OldValue",
                "~List.Recorder",
                "~List.Period"
            ]
        );
        body.data_attributes[0]
            .dynamic_list
            .as_mut()
            .unwrap()
            .query_text = Some("SELECT R.* FROM InformationRegister.Register AS R".into());
    }
    let mut duplicate = cfg.clone();
    duplicate.objects.push(cfg.objects[0].clone());
    assert!(FormProjectionContext::new(&duplicate).is_err());
}

#[test]
fn native_availability_source_markers_require_unchanged_current_dependencies() {
    use formats_xml::form::{
        FormProjectionContext, bind_native_availability_sources, write_form_with_context,
    };
    use morph1c_core::ir::form::FormPicture;
    use morph1c_core::ir::{
        Configuration, FormControlKind, FormItem, MetadataObject, NamedFormBody, ObjectKind,
        PropertyValue, Uuid,
    };
    use morph1c_core::spec::forms::controls::table as tb;
    for minor in [20, 21] {
        let version = FormatVersion::new(2, minor);
        let mut body = dynamic_body(
            "<mainTable>InformationRegister.Register</mainTable><customQuery>true</customQuery><autoFillAvailableFields>true</autoFillAvailableFields><queryText>SELECT R.* FROM InformationRegister.Register AS R</queryText>",
        );
        body.data_attributes[0].not_default_use_always =
            vec!["List.CurrentValue".into(), "List.Missing".into()];
        body.data_attributes[0].settings_saved_data = vec!["List.CurrentValue".into()];
        let mut rows = FormItem::new(FormControlKind::new("Table"), "Rows", 1);
        rows.properties.push((
            tb::F_ROW_PICTURE_DATA_PATH,
            PropertyValue::Ref("List.CurrentValue".into()),
        ));
        body.items.push(rows);
        let native =
            with_roundtrip_target(version, || write_form(FormDialect::Designer, &body)).unwrap();
        let native = String::from_utf8(native)
            .unwrap()
            .replace(
                "<Field>List.CurrentValue</Field>",
                "<Field>~List.CurrentValue</Field>",
            )
            .replace(
                "<RowPictureDataPath>List.CurrentValue</RowPictureDataPath>",
                "<RowPictureDataPath>~List.CurrentValue</RowPictureDataPath>",
            )
            .into_bytes();
        let body = morph1c_core::version::with_source_version(Some(version), || {
            read_form(FormDialect::Designer, &native)
        })
        .unwrap();
        assert!(
            with_roundtrip_target(version, || write_form(FormDialect::Designer, &body)).unwrap()
                == native
        );
        let mut standalone_edit = body.clone();
        standalone_edit.data_attributes[0]
            .dynamic_list
            .as_mut()
            .unwrap()
            .query_text = Some(
            "SELECT R.CurrentValue AS CurrentValue FROM InformationRegister.Register AS R".into(),
        );
        assert!(
            !String::from_utf8(
                with_roundtrip_target(version, || write_form(
                    FormDialect::Designer,
                    &standalone_edit
                ))
                .unwrap()
            )
            .unwrap()
            .contains("<Field>~List.CurrentValue</Field>"),
            "standalone query edits must invalidate lexical markers too"
        );
        let mut standalone_edit = body.clone();
        standalone_edit.data_attributes[0].not_default_use_always[0] = "List.CURRENTVALUE".into();
        assert!(
            !String::from_utf8(
                with_roundtrip_target(version, || write_form(
                    FormDialect::Designer,
                    &standalone_edit
                ))
                .unwrap()
            )
            .unwrap()
            .contains("<Field>~List.CURRENTVALUE</Field>"),
            "standalone path edits must invalidate lexical markers too"
        );
        let mut config = Configuration::new().with_source_version(Some(version));
        let mut register = MetadataObject::new(
            ObjectKind::new("InformationRegister"),
            "Register",
            Uuid([1; 16]),
        );
        register.children.push(MetadataObject::new(
            ObjectKind::new("InformationRegister.Resource"),
            "CurrentValue",
            Uuid([2; 16]),
        ));
        config.objects.push(register);
        let mut owner = MetadataObject::new(ObjectKind::new("CommonForm"), "Form", Uuid([3; 16]));
        owner.form_bodies.push(NamedFormBody {
            name: "Form".into(),
            body,
            ordinary_body: None,
            module: None,
            help: vec![],
            help_resources: vec![],
        });
        config.objects.push(owner);
        bind_native_availability_sources(&mut config).unwrap();
        let render = |config: &Configuration| {
            let context = FormProjectionContext::new(config).unwrap();
            with_roundtrip_target(version, || {
                write_form_with_context(
                    FormDialect::Designer,
                    &config.objects[1].form_bodies[0].body,
                    &context,
                )
            })
            .unwrap()
        };
        assert!(
            render(&config) == native,
            "unchanged native binding must preserve exact source spelling"
        );
        let original = config.clone();
        config.objects[1].form_bodies[0].module = Some("// CURRENT module change\r\n".into());
        config.objects[1].form_bodies[0].body.items[0]
            .properties
            .push((tb::F_ROWS_PICTURE, PropertyValue::Ref("abs:png".into())));
        config.objects[1].form_bodies[0].body.items[0]
            .pictures
            .push(FormPicture {
                file_name: "RowsPicture.png".into(),
                bytes: vec![1, 2, 3],
            });
        let context = FormProjectionContext::new(&config).unwrap();
        assert_eq!(
            context
                .dependency_sha256(&config.objects[1].form_bodies[0].body, version)
                .unwrap(),
            original.objects[1].form_bodies[0]
                .body
                .availability_source_dependency
                .clone()
                .unwrap(),
            "module/asset bytes do not invalidate availability binding"
        );
        config.objects[1].form_bodies[0].body.items[0].pictures[0].bytes = vec![4, 5];
        assert_eq!(
            FormProjectionContext::new(&config)
                .unwrap()
                .dependency_sha256(&config.objects[1].form_bodies[0].body, version)
                .unwrap(),
            original.objects[1].form_bodies[0]
                .body
                .availability_source_dependency
                .clone()
                .unwrap()
        );
        config = original.clone();
        config.objects[0].children[0].name = "Missing".into();
        let text = String::from_utf8(render(&config)).unwrap();
        assert!(text.contains("<Field>~List.CurrentValue</Field>"));
        assert!(text.contains("<Field>List.Missing</Field>"));
        config = original.clone();
        config.objects[1].form_bodies[0].body.data_attributes[0]
            .dynamic_list
            .as_mut()
            .unwrap()
            .query_text =
            Some("SELECT r.currentvalue AS missing FROM InformationRegister.register AS r".into());
        let text = String::from_utf8(render(&config)).unwrap();
        assert!(text.contains("<Field>~List.CurrentValue</Field>"));
        assert!(text.contains("<Field>List.Missing</Field>"));
        config = original.clone();
        config.objects[1].form_bodies[0].body.data_attributes[0].not_default_use_always[0] =
            "List.CURRENTVALUE".into();
        let text = String::from_utf8(render(&config)).unwrap();
        assert!(
            text.contains("<Field>List.CURRENTVALUE</Field>"),
            "CURRENT case-insensitive path must not inherit stale source sigil"
        );
        assert!(text.contains("<Field>~List.Missing</Field>"));
        assert!(!text.contains("<RowPictureDataPath>~List.CurrentValue</RowPictureDataPath>"));
        config = original.clone();
        config.objects[1].form_bodies[0].body.items[0]
            .properties
            .iter_mut()
            .find(|(id, _)| *id == tb::F_ROW_PICTURE_DATA_PATH)
            .unwrap()
            .1 = PropertyValue::Ref("List.CURRENTVALUE".into());
        config.objects[1].form_bodies[0].body.data_attributes[0].settings_saved_data[0] =
            "List.CURRENTVALUE".into();
        let text = String::from_utf8(render(&config)).unwrap();
        assert!(text.contains("<RowPictureDataPath>List.CURRENTVALUE</RowPictureDataPath>"));
        assert!(text.contains("<Field>List.CURRENTVALUE</Field>"));
        assert!(!text.contains("<Field>~List.CurrentValue</Field>"));
        let serialized = serde_json::to_value(&original.objects[1].form_bodies[0].body).unwrap();
        assert!(serialized.get("availability_source_dependency").is_none());
        config = original;
        config.objects[1].form_bodies[0].body.data_attributes[0]
            .dynamic_list
            .as_mut()
            .unwrap()
            .query_text = Some("SELECT Unsupported.* FROM Missing.Source AS Unsupported".into());
        assert!(
            !String::from_utf8(render(&config))
                .unwrap()
                .contains("<Field>~List.CurrentValue</Field>"),
            "changed unresolved dependencies cannot replay a source marker"
        );
    }
}

#[test]
fn current_metadata_field_roles_follow_sdk_keys_and_hierarchy_kind() {
    use formats_xml::form::{FormProjectionContext, write_form_with_context};
    use morph1c_core::ir::{Configuration, MetadataObject, ObjectKind, PropertyValue, Token, Uuid};
    use morph1c_core::spec::metadata::{catalog as cat, information_register as ir};
    for minor in [20, 21] {
        let version = FormatVersion::new(2, minor);
        let mut config = Configuration::new();
        let mut catalog = MetadataObject::new(ObjectKind::new("Catalog"), "Object", Uuid([1; 16]));
        catalog
            .properties
            .push((cat::F_HIERARCHICAL, PropertyValue::Bool(true)));
        catalog.properties.push((
            cat::F_HIERARCHY_TYPE,
            PropertyValue::Enum(Token::new("HierarchyItems")),
        ));
        config.objects.push(catalog);
        let mut register = MetadataObject::new(
            ObjectKind::new("InformationRegister"),
            "Object",
            Uuid([2; 16]),
        );
        register.properties.push((
            ir::F_WRITE_MODE,
            PropertyValue::Enum(Token::new("RecorderSubordinate")),
        ));
        register.children.push(MetadataObject::new(
            ObjectKind::new("InformationRegister.Dimension"),
            "CurrentKey",
            Uuid([3; 16]),
        ));
        config.objects.push(register);
        config.objects.push(MetadataObject::new(
            ObjectKind::new("AccumulationRegister"),
            "Object",
            Uuid([4; 16]),
        ));
        for (kind, fields, expected) in [
            ("Catalog", vec!["Parent", "IsFolder"], vec![false, true]),
            (
                "InformationRegister",
                vec!["CurrentKey", "Recorder", "LineNumber"],
                vec![false, false, true],
            ),
            (
                "AccumulationRegister",
                vec!["Period", "Recorder", "LineNumber"],
                vec![false, false, false],
            ),
        ] {
            let mut body = dynamic_body(&format!(
                "<mainTable>{kind}.Object</mainTable><customQuery>true</customQuery><autoFillAvailableFields>true</autoFillAvailableFields><queryText>SELECT 1 AS Selected FROM {kind}.Object AS R</queryText>"
            ));
            body.data_attributes[0].not_default_use_always = fields
                .iter()
                .map(|f| morph1c_core::ir::form::DataPathSpec::from_form_text(&format!("List.{f}")))
                .collect();
            let context = FormProjectionContext::new(&config).unwrap();
            let native = with_roundtrip_target(version, || {
                write_form_with_context(FormDialect::Designer, &body, &context)
            })
            .unwrap();
            let doc = formats_xml::parse(&native).unwrap();
            let got: Vec<_> = doc
                .root
                .child("Attributes")
                .unwrap()
                .child("Attribute")
                .unwrap()
                .child("UseAlways")
                .unwrap()
                .children
                .iter()
                .map(|field| field.text.starts_with('~'))
                .collect();
            assert_eq!(got, expected, "CURRENT SDK roles {kind} profile {minor}");
        }
    }
}

#[test]
fn current_identifier_lookup_uses_java_simple_case_without_rewriting_paths() {
    use formats_xml::form::{FormProjectionContext, write_form_with_context};
    use morph1c_core::ir::{Configuration, MetadataObject, ObjectKind, Uuid};
    let mut config = Configuration::new();
    let mut register = MetadataObject::new(
        ObjectKind::new("InformationRegister"),
        "RegisterΣ",
        Uuid([1; 16]),
    );
    for (index, name) in ["Σ", "ſ", "İ", "ı", "ß"].iter().enumerate() {
        register.children.push(MetadataObject::new(
            ObjectKind::new("InformationRegister.Resource"),
            *name,
            Uuid([index as u8 + 2; 16]),
        ));
    }
    register.children.push(MetadataObject::new(
        ObjectKind::new("InformationRegister.Dimension"),
        "ΟΣ",
        Uuid([9; 16]),
    ));
    config.objects.push(register);
    for minor in [20, 21] {
        let mut body = dynamic_body(
            "<mainTable>InformationRegister.registerς</mainTable><customQuery>true</customQuery><autoFillAvailableFields>true</autoFillAvailableFields><queryText>SELECT r.* FROM InformationRegister.registerσ AS r</queryText>",
        );
        body.data_attributes[0].not_default_use_always =
            ["List.σ", "List.ς", "List.s", "List.i", "List.I", "List.ss"]
                .map(morph1c_core::ir::form::DataPathSpec::from_form_text)
                .to_vec();
        let context = FormProjectionContext::new(&config).unwrap();
        let native = with_roundtrip_target(FormatVersion::new(2, minor), || {
            write_form_with_context(FormDialect::Designer, &body, &context)
        })
        .unwrap();
        let doc = formats_xml::parse(&native).unwrap();
        let got: Vec<_> = doc
            .root
            .child("Attributes")
            .unwrap()
            .child("Attribute")
            .unwrap()
            .child("UseAlways")
            .unwrap()
            .children
            .iter()
            .map(|n| n.text.as_str())
            .collect();
        assert_eq!(
            got,
            ["List.σ", "List.ς", "List.s", "List.i", "List.I", "~List.ss"]
        );
        body.data_attributes[0]
            .dynamic_list
            .as_mut()
            .unwrap()
            .query_text = Some("SELECT 1 AS Ος FROM InformationRegister.registerσ AS r".into());
        body.data_attributes[0].not_default_use_always = vec!["List.ΟΣ".into()];
        let native = with_roundtrip_target(FormatVersion::new(2, minor), || {
            write_form_with_context(FormDialect::Designer, &body, &context)
        })
        .unwrap();
        assert!(
            String::from_utf8(native)
                .unwrap()
                .contains("<Field>List.ΟΣ</Field>")
        );
        // Contextual lowercase collision suppresses duplicate role injection.
        assert_eq!("ΟΣ".to_lowercase(), "Ος".to_lowercase());
    }
}
