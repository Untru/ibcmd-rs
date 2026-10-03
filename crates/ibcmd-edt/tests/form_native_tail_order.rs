use formats_xml::form::{FormDialect, read_form, write_form};
use morph1c_core::{
    ir::FormBody,
    version::{FormatVersion, with_roundtrip_target},
};

#[test]
fn table_autofill_precedes_enabled_in_both_native_profiles() {
    // Original SDK FormChildItemsXmlPartReader uses the QName "Autofill";
    // both genuine native-reference profiles emit it before Enabled. An
    // incorrectly capitalized ranking slot leaves it in an arbitrary position.
    for minor in [20, 21] {
        with_roundtrip_target(FormatVersion::new(2, minor), || {
            let empty =
                String::from_utf8(write_form(FormDialect::Designer, &FormBody::new()).unwrap())
                    .unwrap();
            let source = empty.replace(
                "\t<Attributes/>",
                "\t<ChildItems>\r\n\t\t<Table name=\"Current\" id=\"1\">\r\n\t\t\t<Autofill>false</Autofill>\r\n\t\t\t<Enabled>false</Enabled>\r\n\t\t</Table>\r\n\t</ChildItems>\r\n\t<Attributes/>",
            );
            assert_ne!(source, empty);
            let native = read_form(FormDialect::Designer, source.as_bytes()).unwrap();
            assert_eq!(
                write_form(FormDialect::Designer, &native).unwrap(),
                source.as_bytes(),
            );
            let edt = write_form(FormDialect::Edt, &native).unwrap();
            let current = read_form(FormDialect::Edt, &edt).unwrap();
            let output = write_form(FormDialect::Designer, &current).unwrap();
            let document = formats_xml::parse(&output).unwrap();
            let table = document
                .root
                .child("ChildItems")
                .unwrap()
                .child("Table")
                .unwrap();
            let autofill = table
                .children
                .iter()
                .position(|el| el.local == "Autofill")
                .unwrap();
            let enabled = table
                .children
                .iter()
                .position(|el| el.local == "Enabled")
                .unwrap();
            assert!(autofill < enabled);
            assert_eq!(table.children[autofill].text, "false");
            assert_eq!(table.children[enabled].text, "false");
        });
    }
}

fn before(tags: &[String], first: &str, second: &str) {
    assert!(
        tags.iter().position(|tag| tag == first).unwrap()
            < tags.iter().position(|tag| tag == second).unwrap(),
        "{first} must precede {second}: {tags:?}",
    );
}

#[test]
fn current_root_title_close_and_collapse_precede_command_bar() {
    use morph1c_core::ir::{PropertyValue, Token};
    use morph1c_core::spec::forms::form_root as fr;
    for minor in [20, 21] {
        with_roundtrip_target(FormatVersion::new(2, minor), || {
            let mut body = FormBody::new();
            body.attributes.extend([
                (fr::F_SHOW_TITLE, PropertyValue::Enum(Token::new("false"))),
                (fr::F_SHOW_CLOSE_BUTTON, PropertyValue::Bool(false)),
                (
                    fr::F_COLLAPSE_ITEMS_BY_IMPORTANCE_VARIANT,
                    PropertyValue::Enum(Token::new("DontUse")),
                ),
                (
                    fr::F_SHOW_COMMAND_BAR,
                    PropertyValue::Enum(Token::new("true")),
                ),
            ]);
            // Start with the current typed model, so a same-native lexical
            // ordering facet cannot hide an incorrect canonical writer order.
            let edt = write_form(FormDialect::Edt, &body).unwrap();
            let mut current = read_form(FormDialect::Edt, &edt).unwrap();
            let native = write_form(FormDialect::Designer, &current).unwrap();
            let document = formats_xml::parse(&native).unwrap();
            let tags: Vec<_> = document
                .root
                .children
                .iter()
                .map(|el| el.local.clone())
                .collect();
            for name in [
                "ShowTitle",
                "ShowCloseButton",
                "CollapseItemsByImportanceVariant",
            ] {
                before(&tags, name, "ShowCommandBar");
            }
            let saved = read_form(FormDialect::Designer, &native).unwrap();
            assert_eq!(write_form(FormDialect::Designer, &saved).unwrap(), native);
            current
                .attributes
                .iter_mut()
                .find(|(id, _)| *id == fr::F_COLLAPSE_ITEMS_BY_IMPORTANCE_VARIANT)
                .unwrap()
                .1 = PropertyValue::Enum(Token::new("Use"));
            let edited = write_form(FormDialect::Designer, &current).unwrap();
            let edited = formats_xml::parse(&edited).unwrap();
            assert_eq!(
                edited
                    .root
                    .child("CollapseItemsByImportanceVariant")
                    .unwrap()
                    .text,
                "Use"
            );
        });
    }
}

#[test]
fn gantt_native_tail_moves_current_table_without_sorting_events() {
    use morph1c_core::ir::form::{
        ContextMenuBody, DecoratorBody, DecoratorRef, FormControlKind, FormEvent, FormItem,
        TooltipBody,
    };
    for minor in [20, 21] {
        with_roundtrip_target(FormatVersion::new(2, minor), || {
            for include_table in [false, true] {
                let mut body = FormBody::new();
                let mut gantt =
                    FormItem::new(FormControlKind::new("GanttChartField"), "Current", 1);
                gantt.context_menu = Some(DecoratorRef::new(
                    "Menu",
                    2,
                    DecoratorBody::ContextMenu(ContextMenuBody {
                        auto_fill: Some(true),
                        ..Default::default()
                    }),
                ));
                gantt.ext_tooltip = Some(DecoratorRef::new(
                    "Tip",
                    3,
                    DecoratorBody::Tooltip(TooltipBody::default()),
                ));
                if include_table {
                    gantt.auto_table = Some(Box::new(FormItem::new(
                        FormControlKind::new("Table"),
                        "Auto",
                        4,
                    )));
                }
                gantt.events = vec![
                    FormEvent {
                        name: "ZSymbol".into(),
                        handler: "First".into(),
                    },
                    FormEvent {
                        name: "ASymbol".into(),
                        handler: "Second".into(),
                    },
                ];
                body.items.push(gantt);
                let edt = write_form(FormDialect::Edt, &body).unwrap();
                let current = read_form(FormDialect::Edt, &edt).unwrap();
                let native = write_form(FormDialect::Designer, &current).unwrap();
                let document = formats_xml::parse(&native).unwrap();
                let gantt = document
                    .root
                    .child("ChildItems")
                    .unwrap()
                    .child("GanttChartField")
                    .unwrap();
                let tags: Vec<_> = gantt.children.iter().map(|el| el.local.clone()).collect();
                before(&tags, "ContextMenu", "ExtendedTooltip");
                before(&tags, "ExtendedTooltip", "Events");
                if include_table {
                    before(&tags, "ExtendedTooltip", "Table");
                    before(&tags, "Table", "Events");
                    assert_eq!(
                        gantt.child("Table").unwrap().attr("name").unwrap().value,
                        "Auto"
                    );
                }
                let events = &gantt.child("Events").unwrap().children;
                assert_eq!(
                    events.iter().map(|e| e.text.as_str()).collect::<Vec<_>>(),
                    ["First", "Second"]
                );
                let own = read_form(FormDialect::Designer, &native).unwrap();
                assert_eq!(write_form(FormDialect::Designer, &own).unwrap(), native);
            }
        });
    }
}
