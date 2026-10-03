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
fn addition_properties_follow_native_order_and_keep_current_values() {
    use morph1c_core::ir::{Lang, PropertyValue};
    use morph1c_core::spec::forms::controls::table as tb;
    for minor in [20, 21] {
        let profile = FormatVersion::new(2, minor);
        morph1c_core::version::with_source_version(Some(profile), || {
            with_roundtrip_target(profile, || {
                for (kind, source_type) in [
                    ("SearchStringAddition", "SearchStringRepresentation"),
                    ("ViewStatusAddition", "ViewStatusRepresentation"),
                    ("SearchControlAddition", "SearchControl"),
                ] {
                    let empty = String::from_utf8(
                        write_form(FormDialect::Designer, &FormBody::new()).unwrap(),
                    )
                    .unwrap();
                    let source = empty.replace("\t<Attributes/>", &format!(concat!(
                    "\t<ChildItems>\r\n\t\t<Table name=\"Current\" id=\"1\">\r\n",
                    "\t\t\t<{kind} name=\"Addition\" id=\"2\">\r\n",
                    "\t\t\t\t<ToolTipRepresentation>Balloon</ToolTipRepresentation>\r\n",
                    "\t\t\t\t<AdditionSource><Item>Current</Item><Type>{source_type}</Type></AdditionSource>\r\n",
                    "\t\t\t\t<Title><v8:item><v8:lang>en</v8:lang><v8:content>Title</v8:content></v8:item></Title>\r\n",
                    "\t\t\t\t<ToolTip><v8:item><v8:lang>en</v8:lang><v8:content>Tip</v8:content></v8:item></ToolTip>\r\n",
                    "\t\t\t\t<AutoMaxWidth>false</AutoMaxWidth>\r\n",
                    "\t\t\t</{kind}>\r\n\t\t</Table>\r\n\t</ChildItems>\r\n\t<Attributes/>"
                ), kind=kind, source_type=source_type));
                    let original = read_form(FormDialect::Designer, source.as_bytes()).unwrap();
                    let canonical = write_form(FormDialect::Designer, &original).unwrap();
                    let reread = read_form(FormDialect::Designer, &canonical).unwrap();
                    assert_eq!(
                        write_form(FormDialect::Designer, &reread).unwrap(),
                        canonical
                    );
                    let edt = write_form(FormDialect::Edt, &original).unwrap();
                    let mut current = read_form(FormDialect::Edt, &edt).unwrap();
                    let addition = &mut current.items[0].additions[0];
                    addition.name = "Edited".into();
                    for (id, value) in &mut addition.properties {
                        if *id == tb::F_ADDITION_SOURCE {
                            *value = PropertyValue::Ref("EditedTable".into());
                        } else if let PropertyValue::Localized(values) = value {
                            *values = vec![(Lang::new("en"), format!("Edited {}", id.0))];
                        }
                    }
                    let output = write_form(FormDialect::Designer, &current).unwrap();
                    let parsed = formats_xml::parse(&output).unwrap();
                    let node = parsed
                        .root
                        .child("ChildItems")
                        .unwrap()
                        .child("Table")
                        .unwrap()
                        .child(kind)
                        .unwrap();
                    let tags: Vec<_> = node.children.iter().map(|c| c.local.clone()).collect();
                    before(&tags, "ToolTipRepresentation", "AdditionSource");
                    before(&tags, "AdditionSource", "Title");
                    before(&tags, "Title", "ToolTip");
                    assert_eq!(
                        node.child("AdditionSource")
                            .unwrap()
                            .child("Item")
                            .unwrap()
                            .text,
                        "EditedTable"
                    );
                    assert_eq!(
                        node.child("AdditionSource")
                            .unwrap()
                            .child("Type")
                            .unwrap()
                            .text,
                        source_type
                    );
                    assert_eq!(node.attr("name").unwrap().value, "Edited");
                    let returned = read_form(FormDialect::Designer, &output).unwrap();
                    assert_eq!(
                        serde_json::to_value(&current).unwrap(),
                        serde_json::to_value(&returned).unwrap()
                    );
                }
            })
        });
    }
}

#[test]
fn table_tooltip_precedes_singleton_additions_and_preserves_current_values() {
    for minor in [20, 21] {
        let profile = FormatVersion::new(2, minor);
        morph1c_core::version::with_source_version(Some(profile), || {
            with_roundtrip_target(profile, || {
                let empty =
                    String::from_utf8(write_form(FormDialect::Designer, &FormBody::new()).unwrap())
                        .unwrap();
                let source = empty.replace(
                "\t<Attributes/>",
                concat!(
                    "\t<ChildItems>\r\n\t\t<Table name=\"Current\" id=\"1\">\r\n",
                    "\t\t\t<ExtendedTooltip name=\"Tip\" id=\"2\"/>\r\n",
                    "\t\t\t<SearchStringAddition name=\"Second\" id=\"4\">\r\n\t\t\t\t<AutoMaxWidth>false</AutoMaxWidth>\r\n\t\t\t</SearchStringAddition>\r\n",
                    "\t\t\t<ViewStatusAddition name=\"Fourth\" id=\"6\">\r\n\t\t\t\t<AutoMaxWidth>false</AutoMaxWidth>\r\n\t\t\t</ViewStatusAddition>\r\n",
                    "\t\t\t<SearchControlAddition name=\"First\" id=\"3\">\r\n\t\t\t\t<AutoMaxWidth>false</AutoMaxWidth>\r\n\t\t\t</SearchControlAddition>\r\n",
                    "\t\t</Table>\r\n\t</ChildItems>\r\n\t<Attributes/>"
                ),
            );
                assert_ne!(source, empty);
                let native = read_form(FormDialect::Designer, source.as_bytes()).unwrap();
                assert_eq!(
                    write_form(FormDialect::Designer, &native).unwrap(),
                    source.as_bytes(),
                );
                let edt = write_form(FormDialect::Edt, &native).unwrap();
                let mut current = read_form(FormDialect::Edt, &edt).unwrap();
                let additions = &mut current.items[0].additions;
                assert_eq!(additions.len(), 3);
                additions.swap(0, 2);
                additions[1].name = "Edited".into();
                let expected_names: std::collections::BTreeMap<_, _> = additions
                    .iter()
                    .map(|a| (a.kind.as_str().to_string(), a.name.clone()))
                    .collect();
                let output = write_form(FormDialect::Designer, &current).unwrap();
                let document = formats_xml::parse(&output).unwrap();
                let table = document
                    .root
                    .child("ChildItems")
                    .unwrap()
                    .child("Table")
                    .unwrap();
                let addition_tags = [
                    "SearchControlAddition",
                    "SearchStringAddition",
                    "ViewStatusAddition",
                ];
                let tooltip_position = table
                    .children
                    .iter()
                    .position(|el| el.local == "ExtendedTooltip")
                    .unwrap();
                let additions: Vec<_> = table
                    .children
                    .iter()
                    .enumerate()
                    .filter(|(_, el)| addition_tags.contains(&el.local.as_str()))
                    .collect();
                assert!(
                    additions
                        .iter()
                        .all(|(position, _)| *position > tooltip_position)
                );
                let actual_names: std::collections::BTreeMap<_, _> = additions
                    .into_iter()
                    .map(|(_, el)| el)
                    .map(|el| (el.local.clone(), el.attr("name").unwrap().value.clone()))
                    .collect();
                assert_eq!(actual_names, expected_names);
                let reread = read_form(FormDialect::Designer, &output).unwrap();
                let reread_names: std::collections::BTreeMap<_, _> = reread.items[0]
                    .additions
                    .iter()
                    .map(|a| (a.kind.as_str().to_string(), a.name.clone()))
                    .collect();
                assert_eq!(reread_names, expected_names);
                assert_eq!(
                    serde_json::to_value(&current).unwrap(),
                    serde_json::to_value(&reread).unwrap()
                );
            })
        });
    }
}

#[test]
fn singleton_addition_semantics_validate_cardinality_and_keep_child_order() {
    use morph1c_core::ir::form::{FormControlKind, FormItem};
    let mut body = FormBody::new();
    let mut table = FormItem::new(FormControlKind::new("Table"), "Table", 1);
    for (index, kind) in [
        "SearchStringAddition",
        "ViewStatusAddition",
        "SearchControlAddition",
    ]
    .into_iter()
    .enumerate()
    {
        table.additions.push(FormItem::new(
            FormControlKind::new(kind),
            kind,
            index as i64 + 2,
        ));
    }
    body.items.push(table);
    let original = serde_json::to_vec(&body).unwrap();
    body.items[0].additions.swap(0, 2);
    assert_eq!(serde_json::to_vec(&body).unwrap(), original);
    let mut renamed = body.clone();
    renamed.items[0].additions[0].name = "Edited".into();
    assert_ne!(serde_json::to_vec(&renamed).unwrap(), original);
    let mut deleted = body.clone();
    deleted.items[0].additions.pop();
    assert_ne!(serde_json::to_vec(&deleted).unwrap(), original);
    let search = &mut body.items[0].additions[0];
    search.children.push(FormItem::new(
        FormControlKind::new("LabelDecoration"),
        "First",
        5,
    ));
    search.children.push(FormItem::new(
        FormControlKind::new("LabelDecoration"),
        "Second",
        6,
    ));
    let with_children = serde_json::to_vec(&body).unwrap();
    body.items[0].additions[0].children.swap(0, 1);
    assert_ne!(serde_json::to_vec(&body).unwrap(), with_children);
    let duplicate = body.items[0].additions[0].clone();
    body.items[0].additions.push(duplicate);
    assert!(serde_json::to_vec(&body).is_err());
    for dialect in [FormDialect::Designer, FormDialect::Edt] {
        assert!(write_form(dialect, &body).is_err());
    }
}

#[test]
fn nested_gantt_auto_table_rejects_duplicate_singleton_additions() {
    use morph1c_core::ir::form::{FormControlKind, FormItem};
    let mut body = FormBody::new();
    let mut gantt = FormItem::new(FormControlKind::new("GanttChartField"), "Gantt", 1);
    let mut table = FormItem::new(FormControlKind::new("Table"), "Auto", 2);
    for (name, id) in [("First", 3), ("Second", 4)] {
        table.additions.push(FormItem::new(
            FormControlKind::new("SearchStringAddition"),
            name,
            id,
        ));
    }
    gantt.auto_table = Some(Box::new(table));
    body.items.push(gantt);
    for dialect in [FormDialect::Designer, FormDialect::Edt] {
        let error = write_form(dialect, &body).unwrap_err().to_string();
        assert!(
            error.contains("duplicate singleton addition kind"),
            "{dialect:?}: {error}"
        );
    }
}

fn current_control_projection(minor: u16, kind: &str, fields: &str) -> Vec<u8> {
    with_roundtrip_target(FormatVersion::new(2, minor), || {
        let empty = String::from_utf8(write_form(FormDialect::Designer, &FormBody::new()).unwrap())
            .unwrap();
        let source = empty.replace(
            "\t<Attributes/>",
            &format!(
                "\t<ChildItems>\r\n\t\t<{kind} name=\"Current\" id=\"1\">\r\n{fields}\t\t</{kind}>\r\n\t</ChildItems>\r\n\t<Attributes/>"
            ),
        );
        assert_ne!(source, empty);
        let native = read_form(FormDialect::Designer, source.as_bytes()).unwrap();
        let own_native = write_form(FormDialect::Designer, &native).unwrap();
        let reread = read_form(FormDialect::Designer, &own_native).unwrap();
        assert_eq!(
            write_form(FormDialect::Designer, &reread).unwrap(),
            own_native
        );
        let edt = write_form(FormDialect::Edt, &native).unwrap();
        let mut current = read_form(FormDialect::Edt, &edt).unwrap();
        current.items[0].name = "Edited".into();
        let output = write_form(FormDialect::Designer, &current).unwrap();
        let returned = read_form(FormDialect::Designer, &output).unwrap();
        assert_eq!(returned.items[0].name, "Edited");
        assert_eq!(
            serde_json::to_vec(&current).unwrap(),
            serde_json::to_vec(&returned).unwrap()
        );
        output
    })
}

#[test]
fn native_picture_border_color_precedes_border_on_both_profiles() {
    for minor in [20, 21] {
        let output = current_control_projection(
            minor,
            "PictureField",
            concat!(
                "\t\t\t<BorderColor>style:FormBackColor</BorderColor>\r\n",
                "\t\t\t<Border width=\"1\">\r\n\t\t\t\t<v8ui:style xsi:type=\"v8ui:ControlBorderType\">WithoutBorder</v8ui:style>\r\n\t\t\t</Border>\r\n"
            ),
        );
        let document = formats_xml::parse(&output).unwrap();
        let node = document
            .root
            .child("ChildItems")
            .unwrap()
            .child("PictureField")
            .unwrap();
        let tags: Vec<_> = node.children.iter().map(|c| c.local.clone()).collect();
        before(&tags, "BorderColor", "Border");
        assert_eq!(
            node.child("BorderColor").unwrap().text,
            "style:FormBackColor"
        );
    }
}

#[test]
fn native_tooltip_title_precedes_group_alignment_on_both_profiles() {
    for minor in [20, 21] {
        let output = current_control_projection(
            minor,
            "LabelDecoration",
            concat!(
                "\t\t\t<ExtendedTooltip name=\"Tip\" id=\"2\">\r\n",
                "\t\t\t\t<Title formatted=\"false\"><v8:item><v8:lang>en</v8:lang><v8:content>Current title</v8:content></v8:item></Title>\r\n",
                "\t\t\t\t<GroupHorizontalAlign>Right</GroupHorizontalAlign>\r\n",
                "\t\t\t\t<GroupVerticalAlign>Center</GroupVerticalAlign>\r\n",
                "\t\t\t</ExtendedTooltip>\r\n"
            ),
        );
        let document = formats_xml::parse(&output).unwrap();
        let node = document
            .root
            .child("ChildItems")
            .unwrap()
            .child("LabelDecoration")
            .unwrap()
            .child("ExtendedTooltip")
            .unwrap();
        let tags: Vec<_> = node.children.iter().map(|c| c.local.clone()).collect();
        before(&tags, "Title", "GroupHorizontalAlign");
        before(&tags, "Title", "GroupVerticalAlign");
        assert_eq!(
            node.child("Title")
                .unwrap()
                .child("item")
                .unwrap()
                .child("content")
                .unwrap()
                .text,
            "Current title"
        );
    }
}

#[test]
fn native_spreadsheet_names_follow_current_edit_and_group_fields() {
    for minor in [20, 21] {
        let output = current_control_projection(
            minor,
            "SpreadSheetDocumentField",
            concat!(
                "\t\t\t<Protection>true</Protection>\r\n",
                "\t\t\t<Edit>true</Edit>\r\n",
                "\t\t\t<ShowGroups>false</ShowGroups>\r\n",
                "\t\t\t<EnableStartDrag>false</EnableStartDrag>\r\n",
                "\t\t\t<ShowCellNames>true</ShowCellNames>\r\n",
                "\t\t\t<ShowRowAndColumnNames>true</ShowRowAndColumnNames>\r\n"
            ),
        );
        let document = formats_xml::parse(&output).unwrap();
        let node = document
            .root
            .child("ChildItems")
            .unwrap()
            .child("SpreadSheetDocumentField")
            .unwrap();
        let tags: Vec<_> = node.children.iter().map(|c| c.local.clone()).collect();
        for name in ["ShowCellNames", "ShowRowAndColumnNames"] {
            for prior in ["Protection", "Edit", "ShowGroups", "EnableStartDrag"] {
                before(&tags, prior, name);
            }
            assert_eq!(node.child(name).unwrap().text, "true");
        }
    }
}

#[test]
fn native_popup_command_source_precedes_representation_on_85() {
    let output = current_control_projection(
        21,
        "Popup",
        concat!(
            "\t\t\t<CommandSource>Item.List</CommandSource>\r\n",
            "\t\t\t<Representation>Picture</Representation>\r\n"
        ),
    );
    let document = formats_xml::parse(&output).unwrap();
    let node = document
        .root
        .child("ChildItems")
        .unwrap()
        .child("Popup")
        .unwrap();
    let tags: Vec<_> = node.children.iter().map(|c| c.local.clone()).collect();
    before(&tags, "CommandSource", "Representation");
    assert_eq!(node.child("CommandSource").unwrap().text, "Item.List");
    assert_eq!(node.child("Representation").unwrap().text, "Picture");
}

#[test]
fn root_alignment_precedes_current_auto_fill_check_on_both_profiles() {
    for minor in [20, 21] {
        with_roundtrip_target(FormatVersion::new(2, minor), || {
            let empty =
                String::from_utf8(write_form(FormDialect::Designer, &FormBody::new()).unwrap())
                    .unwrap();
            let source = empty.replace(
                "\t<Attributes/>",
                concat!(
                    "\t<HorizontalAlign>Center</HorizontalAlign>\r\n",
                    "\t<VerticalAlign>Center</VerticalAlign>\r\n",
                    "\t<Attributes/>"
                ),
            );
            let native = read_form(FormDialect::Designer, source.as_bytes()).unwrap();
            let edt = write_form(FormDialect::Edt, &native).unwrap();
            let current = read_form(FormDialect::Edt, &edt).unwrap();
            let output = write_form(FormDialect::Designer, &current).unwrap();
            let document = formats_xml::parse(&output).unwrap();
            let tags: Vec<_> = document
                .root
                .children
                .iter()
                .map(|c| c.local.clone())
                .collect();
            before(&tags, "HorizontalAlign", "AutoFillCheck");
            before(&tags, "VerticalAlign", "AutoFillCheck");
            assert_eq!(document.root.child("AutoFillCheck").unwrap().text, "false");
        });
    }
}

#[test]
fn current_root_extension_fields_follow_command_bar_only_on_85() {
    use morph1c_core::ir::{DocumentFormInfo, FormRootExtInfo, PropertyValue, Token};
    use morph1c_core::spec::forms::form_root as fr;
    for minor in [20, 21] {
        with_roundtrip_target(FormatVersion::new(2, minor), || {
            for document in [false, true] {
                let mut body = FormBody::new();
                body.root_ext_info = Some(FormRootExtInfo {
                    kind: if document {
                        "form:DocumentFormExtInfo"
                    } else {
                        "form:DynamicListFormExtInfo"
                    }
                    .into(),
                    events: Vec::new(),
                    user_settings_group: None,
                });
                body.attributes.push((
                    fr::F_SHOW_COMMAND_BAR,
                    PropertyValue::Enum(Token::new("true")),
                ));
                if document {
                    body.document_form = Some(DocumentFormInfo {
                        auto_time: "CurrentOrLast".into(),
                        use_posting_mode: "Auto".into(),
                        repost_on_write: true,
                    });
                } else {
                    body.attributes
                        .push((fr::F_GROUP_LIST, PropertyValue::Str("CurrentList".into())));
                }
                let edt = write_form(FormDialect::Edt, &body).unwrap();
                let current = read_form(FormDialect::Edt, &edt).unwrap();
                let output = write_form(FormDialect::Designer, &current).unwrap();
                let parsed = formats_xml::parse(&output).unwrap();
                let tags: Vec<_> = parsed
                    .root
                    .children
                    .iter()
                    .map(|c| c.local.clone())
                    .collect();
                let extension_tags: &[&str] = if document {
                    &["AutoTime", "UsePostingMode", "RepostOnWrite"]
                } else {
                    &["GroupList"]
                };
                for tag in extension_tags {
                    if minor == 21 {
                        before(&tags, "ShowCommandBar", tag);
                    } else {
                        before(&tags, tag, "ShowCommandBar");
                    }
                }
                if document {
                    before(&tags, "AutoTime", "UsePostingMode");
                    before(&tags, "UsePostingMode", "RepostOnWrite");
                    assert_eq!(parsed.root.child("AutoTime").unwrap().text, "CurrentOrLast");
                    assert_eq!(parsed.root.child("UsePostingMode").unwrap().text, "Auto");
                    assert_eq!(parsed.root.child("RepostOnWrite").unwrap().text, "true");
                } else {
                    assert_eq!(parsed.root.child("GroupList").unwrap().text, "CurrentList");
                }
                let own_native = read_form(FormDialect::Designer, &output).unwrap();
                assert_eq!(
                    write_form(FormDialect::Designer, &own_native).unwrap(),
                    output
                );
            }
        });
    }
}

#[test]
fn default_root_extension_order_matches_explicit_default_profile() {
    use morph1c_core::ir::{DocumentFormInfo, FormRootExtInfo, PropertyValue, Token};
    use morph1c_core::spec::forms::form_root as fr;
    assert_eq!(morph1c_core::version::current_roundtrip_target(), None);
    for document in [false, true] {
        let mut body = FormBody::new();
        body.root_ext_info = Some(FormRootExtInfo {
            kind: if document {
                "form:DocumentFormExtInfo"
            } else {
                "form:DynamicListFormExtInfo"
            }
            .into(),
            events: Vec::new(),
            user_settings_group: None,
        });
        body.attributes.push((
            fr::F_SHOW_COMMAND_BAR,
            PropertyValue::Enum(Token::new("true")),
        ));
        if document {
            body.document_form = Some(DocumentFormInfo {
                auto_time: "CurrentOrLast".into(),
                use_posting_mode: "Auto".into(),
                repost_on_write: true,
            });
        } else {
            body.attributes
                .push((fr::F_GROUP_LIST, PropertyValue::Str("CurrentList".into())));
        }
        let default = write_form(FormDialect::Designer, &body).unwrap();
        let explicit = with_roundtrip_target(morph1c_core::version::SSL, || {
            write_form(FormDialect::Designer, &body).unwrap()
        });
        assert_eq!(default, explicit);
    }
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
