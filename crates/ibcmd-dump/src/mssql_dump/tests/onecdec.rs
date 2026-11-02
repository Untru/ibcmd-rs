//! Tests added by the onecdec fork (offline .cf/.cfe/.epf/.erf export).
//! A child of the upstream `tests` module: its helpers stay in reach
//! while its file stays untouched.

use super::*;

#[test]
fn owner_form_ref_falls_back_to_a_foreign_common_form() {
    // An external report names its configuration's common form by uuid;
    // the storage owns no such form, the foreign references know it.
    let form_refs = BTreeMap::new();
    let mut object_refs = BTreeMap::new();
    object_refs.insert(
        "9d6d77a9-1f55-4162-93a5-14bb3f3febaf".to_string(),
        "CommonForm.ФормаОтчета".to_string(),
    );
    object_refs.insert(
        "14512818-58b0-44cc-b00d-d37913c57aad".to_string(),
        "SettingsStorage.ХранилищеВариантовОтчетов".to_string(),
    );
    assert_eq!(
        parse_owner_form_ref(
            Some("9d6d77a9-1f55-4162-93a5-14bb3f3febaf"),
            &form_refs,
            &object_refs
        )
        .as_deref(),
        Some("CommonForm.ФормаОтчета")
    );
    // only common forms qualify
    assert_eq!(
        parse_owner_form_ref(
            Some("14512818-58b0-44cc-b00d-d37913c57aad"),
            &form_refs,
            &object_refs
        ),
        None
    );
}

#[test]
fn foreign_object_references_stay_ids_and_a_type_set_may_respell() {
    const ANY_IB_REF: &str = "280f5f0e-9c8a-49cc-bf6d-4d296cc17a63";
    let mut indexes = MetadataTypeIndexes::default();
    indexes.references.insert(
        "aaaaaaaa-0000-0000-0000-000000000001".into(),
        "cfg:CatalogRef.Own".into(),
    );
    indexes.dcs.insert(
        ANY_IB_REF.into(),
        DcsTypeResolution::TypeSet {
            qname: "cfg:AnyIBRef".into(),
        },
    );
    let foreign = ForeignReferences {
        objects: BTreeMap::new(),
        types: vec![
            (
                "AAAAAAAA-0000-0000-0000-000000000001".into(),
                "cfg:CatalogRef.Foreign".into(),
                "Ref".into(),
            ),
            (
                "bbbbbbbb-0000-0000-0000-000000000002".into(),
                "cfg:DocumentRef.Д".into(),
                "Ref".into(),
            ),
            (ANY_IB_REF.into(), "cfg:AnyRef".into(), "TypeSet".into()),
        ],
    };
    merge_foreign_type_references(&mut indexes, &foreign);
    assert_eq!(
        indexes.references["aaaaaaaa-0000-0000-0000-000000000001"],
        "cfg:CatalogRef.Own"
    );
    // A configuration object the configuration being written does not define
    // keeps its id: the platform writes it as `<v8:TypeId>`, never by name.
    assert!(
        !indexes
            .references
            .contains_key("bbbbbbbb-0000-0000-0000-000000000002")
    );
    assert_eq!(
        indexes.dcs["bbbbbbbb-0000-0000-0000-000000000002"],
        DcsTypeResolution::KeepId
    );
    assert!(
        indexes
            .foreign
            .contains("bbbbbbbb-0000-0000-0000-000000000002")
    );
    assert!(indexes.foreign.contains("cfg:DocumentRef.Д"));
    assert_eq!(
        indexes.dcs[ANY_IB_REF],
        DcsTypeResolution::TypeSet {
            qname: "cfg:AnyRef".into()
        }
    );
}

#[test]
fn forms_declare_the_dcs_schema_namespace_from_compatibility_8_3_19() {
    for mode in ["Version8_3_19", "Version8_3_27", "Version8_5_1"] {
        assert!(forms_declare_dcs_schema_namespace(Some(mode)), "{mode}");
    }
    for mode in [
        "Version8_3_18",
        "Version8_3_10",
        "Version8_3_8",
        "Version8_2_16",
        "Version8_1",
    ] {
        assert!(!forms_declare_dcs_schema_namespace(Some(mode)), "{mode}");
    }
    // Nothing read (an external object, an unknown spelling): the platform's
    // own edition, which declares it.
    assert!(forms_declare_dcs_schema_namespace(None));
    assert!(forms_declare_dcs_schema_namespace(Some("DontUse")));
}

#[test]
fn forms_write_explicit_usual_group_behavior_from_compatibility_8_3_20() {
    for mode in ["Version8_3_20", "Version8_3_21", "Version8_5_1"] {
        assert!(forms_write_usual_group_behavior(Some(mode)), "{mode}");
    }
    for mode in [
        "Version8_3_19",
        "Version8_3_17",
        "Version8_3_14",
        "Version8_2_16",
    ] {
        assert!(!forms_write_usual_group_behavior(Some(mode)), "{mode}");
    }
    assert!(forms_write_usual_group_behavior(None));
}

/// A `Button` record under the leading member `30` at the canonical `31`'s
/// own length (52 members, name at slot 5): a real configuration's 379 such
/// buttons match
/// `31`/52 member for member -- `ToolTipRepresentation` at slot 30 (`0` none
/// written, `2` Balloon, `3` Button) and the `CommandUniqueness` flag `1` two
/// slots from the end -- so the record is read as it stands, not padded as the
/// shorter `30`/51 is. The record is 8.3.27.2214's own `31` button
/// (`ToolTipRepresentation` None, slot 30 `1`) under the older leading member.
#[test]
fn a_button_of_revision_30_at_the_canonical_length_is_read_unpadded() {
    let record = r#"{30,{6,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,1,"КнопкаБезПодсказки",{1,0},1,{1,409b9a53-7f7e-4178-86c1-33176c7c7a7a},{0},3,0,0,0,2,2,0,0,0,{3,4,{0}},{3,4,{0}},{3,4,{0}},{7,3,0,1,100},{0,0,0},0,{4,0,{0},"",-1,-1,1,0,""},1,{"Pattern"},"",2,1,1,{12,{7,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,0,"КнопкаБезПодсказкиРасширеннаяПодсказка",{1,0},{1,0},1,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{5,0,0,3,0,{0,1,0},{3,4,{0}},{3,4,{0}},{3,0,{0},0,1,0,48312c09-257f-4b29-b280-284dd89efc1e}},0,1,2,{1,{1,0},0},0,0,1,0,0,1,0,3,3,0,0},{"U"},1,0,0,1,0,0,0,3,3,3,0,0,1,0,0,0,1,0}"#;
    let item = parse_form_child_item(
        record,
        None,
        None,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &[],
        &BTreeMap::new(),
    )
    .expect("button");
    assert_eq!(item.tag, "Button");
    assert_eq!(item.command_uniqueness, None);
    let xml = format_form_child_items_xml(&[item], 1);
    assert!(
        xml.contains("<ToolTipRepresentation>None</ToolTipRepresentation>"),
        "{xml}"
    );
    assert!(!xml.contains("<CommandUniqueness>"), "{xml}");
}

#[test]
fn unidentified_items_are_numbered_but_the_navigator_is_not() {
    // A real configuration stores its forms' hidden navigator group (`22`,
    // kind `7`) and its tooltip without ids and never writes them; an
    // unidentified table addition is numbered past the greatest id.
    let layout = "{1,{22,{-1,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,9,\"Панель\"},\
                  {22,{0},0,0,0,7,\"Navigator\",{12,{0},0,0,0,0,\"NavigatorExtendedTooltip\"}},\
                  {55,{9,02023637-7868-4a5f-8576-835a76e0c9ba},0,1,0,\"Список\",\
                  {5,\r\n{0},0,0,0,0,\"СписокСтрокаПоиска\",{22,{0},0,0,0,8,\"Меню\"}}}}";
    let numbered = with_unidentified_form_items_numbered(layout).expect("numbered");
    assert!(
        numbered.contains("{22,{0},0,0,0,7,\"Navigator\",{12,{0},"),
        "{numbered}"
    );
    assert!(
        numbered.contains("{5,\r\n{10,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,0,\"СписокСтрокаПоиска\",{22,{11,02023637-7868-4a5f-8576-835a76e0c9ba},"),
        "{numbered}"
    );
    assert_eq!(
        with_unidentified_form_items_numbered("{1,{0},0,0,0,0,x}"),
        None
    );
}

#[test]
fn enum_values_under_the_older_header_spelling_are_read() {
    // A real configuration keeps enum values whose header tuple opens with
    // `0` (`{0,0,<uuid>}`), the older spelling the object header reader
    // already accepts; the values were dropped and the enum written empty.
    let text = "\u{feff}{1,\r\n{20,dee62b97-84de-42d8-a934-d6f60c35d806,\r\n{0,\r\n{1,\r\n{0,0,aaaaaaaa-0000-4000-8000-000000000001},\"Перечисление\",\r\n{1,\"ru\",\"Перечисление\"},\"\",0,0}\r\n},0},4,\r\n{bee0a08c-07eb-40c0-8544-5c364c171465,2,\r\n{\r\n{0,\r\n{1,\r\n{0,0,aaaaaaaa-0000-4000-8000-000000000002},\"Первое\",\r\n{1,\"ru\",\"Первое\"},\"\",0,0}\r\n},0},\r\n{\r\n{0,\r\n{1,\r\n{0,0,aaaaaaaa-0000-4000-8000-000000000003},\"Второе\",\r\n{1,\"ru\",\"Второе значение\"},\"\",0,0}\r\n},0}\r\n}\r\n}";
    let values = parse_enum_values_from_text(text);
    assert_eq!(
        values
            .iter()
            .map(|value| value.name.as_str())
            .collect::<Vec<_>>(),
        ["Первое", "Второе"]
    );
    assert_eq!(values[1].uuid, "aaaaaaaa-0000-4000-8000-000000000003");
}

#[test]
fn an_orphan_form_descriptor_owns_no_integration_service_body() {
    // A form descriptor whose owner is not read is classified like an
    // integration service (same code and header slot); it must not become the
    // owner of `IntegrationServices/<form>/Ext/Module.bsl`.
    let form_uuid = "aaaaaaaa-0000-4000-8000-000000000010";
    let service_uuid = "aaaaaaaa-0000-4000-8000-000000000011";
    let form = format!(
        "{{1,\r\n{{0,\r\n{{13,\r\n{{3,\r\n{{1,0,{form_uuid}}},\"ПодробнаяИнформация\",\r\n{{1,\"ru\",\"Подробная информация\"}},\"\",0,0,00000000-0000-0000-0000-000000000000,0}},0,1,\r\n{{2,\r\n{{\"#\",1708fdaa-cbce-4289-b373-07a5a74bee91,1}},\r\n{{\"#\",1708fdaa-cbce-4289-b373-07a5a74bee91,2}}\r\n}}\r\n}}\r\n}},0}}"
    );
    let service = format!(
        "{{1,\r\n{{0,\r\n{{3,\r\n{{1,0,{service_uuid}}},\"Обмен\",{{1,\"ru\",\"Обмен\"}},\"\",0,0,00000000-0000-0000-0000-000000000000,0}},5362f1d1-1f56-4a61-a52e-6519a060293e,ad884943-3c3a-4073-ab34-ed12a0d67556,\"\"}},0}}"
    );
    let rows = [
        metadata_text_row_audit_from_text(form_uuid, form),
        metadata_text_row_audit_from_text(service_uuid, service),
    ]
    .into_iter()
    .map(|audit| match audit {
        MetadataTextRowAudit::Extracted(row)
        | MetadataTextRowAudit::ExtractedWithWarning(row, _) => row,
        MetadataTextRowAudit::Miss(_) => panic!("row not extracted"),
    })
    .collect::<Vec<_>>();
    assert_eq!(rows[0].kind.as_deref(), Some("Form"));
    assert_eq!(rows[0].folder, None);
    let owners = build_body_owner_source_index_from_texts(&rows, &BTreeMap::new());
    assert!(!owners.contains_key(form_uuid));
    assert_eq!(owners[service_uuid].kind, "IntegrationService");
}

#[test]
fn a_help_link_to_an_html_template_names_the_template_document() {
    let template = "aaaaaaaa-0000-4000-8000-000000000020";
    let catalog = "aaaaaaaa-0000-4000-8000-000000000021";
    let refs = BTreeMap::from([
        (
            template.to_string(),
            "Catalog.База.Template.Раздел".to_string(),
        ),
        (catalog.to_string(), "Catalog.База".to_string()),
    ]);
    let stored = format!(
        "<p><a href=\"../id{template}/8eb4fad1-1fa6-403e-970f-2c12dbb43e23\">t</a>\
         <a href=\"../id{catalog}/038b5c85-fb1c-4082-9c4c-e69f8928bf3a#x\">h</a></p>"
    );
    let written = String::from_utf8(rewrite_help_links(stored.as_bytes(), &refs)).unwrap();
    assert_eq!(
        written,
        "<p><a href=\"Catalog.База.Template.Раздел/Template\">t</a>\
         <a href=\"Catalog.База/Help#x\">h</a></p>"
    );
}

#[test]
fn a_button_group_writes_its_title_font_behind_the_title() {
    // A real configuration's button group (children dropped): native writes
    // `Title`, `TitleFont`, `ToolTip`.
    let record = r#"{22,{235,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,6,"Группа",{1,1,{"ru","Группа"}},{1,1,{"ru","Подсказка"}},0,1,0,0,0,2,2,{3,4,{0}},{7,2,0,{-31},1,100},{0,0,0},1,{2,{0},2,0},0,1,0,1,{12,{318,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,0,"ГруппаExtendedTooltip",{1,0},{1,0},1,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{5,0,0,3,0,{0,1,0},{3,4,{0}},{3,4,{0}},{3,0,{0},0,1,0,48312c09-257f-4b29-b280-284dd89efc1e}},0,1,2,{1,{1,0},0},0,0,1,0,0,1,0,3,3,0,0},0,3,3,0}"#;
    let item = parse_form_child_item(
        record,
        None,
        None,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &[],
        &BTreeMap::new(),
    )
    .expect("button group");
    assert_eq!(item.tag, "ButtonGroup");
    let xml = format_form_child_items_xml(&[item], 1);
    let title = xml.find("<Title>").expect(&xml);
    let font = xml.find("<TitleFont ").expect(&xml);
    let tooltip = xml.find("<ToolTip>").expect(&xml);
    assert!(title < font && font < tooltip, "{xml}");
}

#[test]
fn a_drawing_writes_no_empty_detail_parameter() {
    let mut drawing = parse_moxel_drawings(
        &["{{0,31},5,1,20,24,6,1,20,88,70,1,1,1,0}"],
        &BTreeMap::new(),
    )
    .into_iter()
    .next()
    .expect("drawing");
    drawing.members.detail_parameter = Some(String::new());
    let mut xml = String::new();
    push_moxel_drawing_xml(&mut xml, &drawing, &BTreeMap::new());
    assert!(!xml.contains("detailParameter"), "{xml}");
    drawing.members.detail_parameter = Some("Расшифровка".to_string());
    let mut xml = String::new();
    push_moxel_drawing_xml(&mut xml, &drawing, &BTreeMap::new());
    assert!(
        xml.contains("<detailParameter>Расшифровка</detailParameter>"),
        "{xml}"
    );
}

#[test]
fn an_older_spreadsheet_picture_record_is_read() {
    // Root revisions 8/9 store an empty picture under the leading `3`, one
    // member short of the `4` record; the platform publishes it as
    // `<picture><index>0</index><picture/></picture>`.
    let pictures = parse_moxel_pictures(&["1", r#"{3,0,{0},"",-1,-1,1,0}"#, "0"], &BTreeMap::new());
    assert_eq!(pictures.len(), 1);
    // any other `{3,…}` shape stays unread
    assert!(parse_moxel_picture(r#"{3,0,{0},"",-1,-1,1}"#, &BTreeMap::new()).is_none());
    assert!(parse_moxel_picture("{3,3,{-1}}", &BTreeMap::new()).is_none());
}

#[test]
fn root_dcs_schema_namespace_is_dropped_unless_the_form_uses_the_prefix() {
    let root = "<Form xmlns=\"http://v8.1c.ru/8.3/xcf/logform\" xmlns:dcscor=\"http://v8.1c.ru/8.1/data-composition-system/core\" xmlns:dcssch=\"http://v8.1c.ru/8.1/data-composition-system/schema\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" version=\"2.20\">\r\n";
    let unused = format!("{root}\t<AutoCommandBar name=\"П\" id=\"-1\"/>\r\n</Form>");
    assert_eq!(
        without_root_dcs_schema_namespace(unused.clone()),
        unused.replace(
            " xmlns:dcssch=\"http://v8.1c.ru/8.1/data-composition-system/schema\"",
            ""
        )
    );
    let used = format!("{root}\t<Field xsi:type=\"dcssch:DataSetFieldField\"/>\r\n</Form>");
    assert_eq!(without_root_dcs_schema_namespace(used.clone()), used);
}

#[test]
fn owner_header_of_an_adopted_object_reads_like_an_own_one() {
    // An extension's adopted object records its adoption after the comment:
    // `1,N,(property uuid,state)xN,<extended object uuid>,0` (ИТК
    // `Catalogs/ДополнительныеОтчетыИОбработки`).
    let adopted = "{0,\r\n{3,\r\n{1,0,07c84bbc-e7d0-4cdf-b0d9-9c3ff914701a},\"Каталог\",\r\n{0},\"\",\
                   1,1,9595ddd6-e72c-47ad-a156-672db811628c,2,\
                   92707138-5004-4378-8477-f909166d319d,0}\r\n}";
    let header = parse_wrapped_register_owner_header(adopted).expect("adopted header");
    assert_eq!(header.uuid, "07c84bbc-e7d0-4cdf-b0d9-9c3ff914701a");
    assert_eq!(header.name, "Каталог");
    let own = "{0,{3,{1,0,07c84bbc-e7d0-4cdf-b0d9-9c3ff914701a},\"Каталог\",{0},\"\",\
               0,0,00000000-0000-0000-0000-000000000000,0}}";
    assert!(parse_wrapped_register_owner_header(own).is_some());
    // an own object never names an extended object
    let own_with_target = "{0,{3,{1,0,07c84bbc-e7d0-4cdf-b0d9-9c3ff914701a},\"Каталог\",{0},\"\",\
                           0,0,92707138-5004-4378-8477-f909166d319d,0}}";
    assert!(parse_wrapped_register_owner_header(own_with_target).is_none());
}

#[test]
fn names_the_standard_pictures_of_the_query_wizard_forms() {
    // ИТК (8.3.27.2214 dumps of three extensions): every occurrence of these
    // identities is spelled with one name.
    for (uuid, name) in [
        (
            "a119150f-6c0c-4a94-97b0-5f08d7ebd6f5",
            "StdPicture.HierarchicalView",
        ),
        (
            "18bca3d7-a7a5-41df-a180-4dff9c217f43",
            "StdPicture.QueryWizardCreateNestedQuery",
        ),
        (
            "7604cff7-5cc6-4f88-8d16-504f01b92a3c",
            "StdPicture.QueryWizardCreateTempTableDescription",
        ),
        (
            "270de5f0-f2df-4845-9fde-30b1ec486217",
            "StdPicture.QueryWizardShowChangesTables",
        ),
        ("c8a269ff-5b6d-4f42-9fa6-369d7b492aa7", "StdPicture.Rename"),
        (
            "fafe4c1f-c265-4220-a0e1-8f82af26b72e",
            "StdPicture.SortList",
        ),
    ] {
        assert_eq!(standard_picture_name(uuid), Some(name), "{uuid}");
    }
}

#[test]
fn an_empty_language_is_written_as_the_platform_writes_it() {
    // 8.3.27.2214 never writes `<v8:lang></v8:lang>`: an empty language is
    // `<v8:lang/>` (ИТК common module and common picture synonyms).
    let dir = std::env::temp_dir().join(format!("ibcmd-empty-lang-{}", std::process::id()));
    let _scratch = crate::test_scratch::TestScratch::adopt(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("CommonModule.xml");
    let xml = "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<Synonym>\r\n\t<v8:item>\r\n\t\t<v8:lang></v8:lang>\r\n\t\t<v8:content>Инфостарт</v8:content>\r\n\t</v8:item>\r\n</Synonym>";
    write_source_xml_file(&path, xml, InfobaseConfigSourceVersion::V2_20).unwrap();
    let written = fs::read_to_string(&path).unwrap();
    let _ = fs::remove_dir_all(&dir);
    assert!(written.contains("\t\t<v8:lang/>\r\n"), "{written}");
    assert!(!written.contains("<v8:lang></v8:lang>"), "{written}");
}

#[test]
fn a_single_row_table_has_no_select_all() {
    // Native 8.3.27.2214 / 8.5.1.1529 trees: every table button naming
    // `SelectAll` sits on a table of the default selection mode (132); on a
    // `SingleRow` table the platform keeps `1:51c99108-...` (ИТК, 6).
    let single = FormTableCommandOwnership {
        excluded_commands: Vec::new(),
        row_set_unchangeable: false,
        list_without_main_table: false,
        single_row: true,
        main_table_family: None,
    };
    let multi = FormTableCommandOwnership {
        single_row: false,
        ..single.clone()
    };
    let select_all = "51c99108-107c-43e1-8918-e48835bf2495";
    assert!(!form_table_owns_button_standard_command(
        "SelectAll",
        select_all,
        &single
    ));
    assert!(form_table_owns_button_standard_command(
        "SelectAll",
        select_all,
        &multi
    ));
    assert!(form_table_owns_button_standard_command(
        "CopyToClipboard",
        "00000000-0000-0000-0000-000000000001",
        &single
    ));
}

#[test]
fn settings_composer_selection_order_and_group_field_members() {
    // ИТК `CommonForms/ИТК_КонструкторНастроекКомпоновкиДанных`: each chain the
    // body stores against the name the 8.3.27.2214 dump writes for the item.
    use FormSettingsComposerType::*;
    for (owner, code, name) in [
        (Selection, "10001", "TitlePicture"),
        (Selection, "10002", "Title"),
        (Selection, "10003", "Placement"),
        (Selection, "10004", "FieldPicture"),
        (Selection, "10005", "Field"),
        (Order, "10001", "FieldPicture"),
        (GroupFields, "10001", "FieldPicture"),
        (GroupFields, "10002", "Field"),
        (GroupFields, "10003", "GroupType"),
        (GroupFields, "10004", "AdditionType"),
        (GroupFields, "10005", "BeginOfPeriodPicture"),
        (GroupFields, "10006", "BeginOfPeriod"),
        (GroupFields, "10007", "EndOfPeriodPicture"),
        (GroupFields, "10008", "EndOfPeriod"),
    ] {
        assert_eq!(
            form_settings_composer_member(owner, code).map(|(member, _)| member),
            Some(name),
            "{owner:?} {code}"
        );
    }
}

#[test]
fn configuration_root_rights_of_an_itk_storage_structure_role() {
    // ИТК `Roles/ИТК_СтруктураХранения` and `ИТК_ПоискПоСтруктуреХранения`
    // (setForNewObjects false): `4df6d046-...` diverges from the flag and
    // 8.3.27.2214 prints it as `ExclusiveModeTerminationAtSessionStart`;
    // `AnalyticsSystemClient` (-1, false) matches the flag and is not printed.
    let rights_text = "{10,{1,{{1,4fa25267-a0df-4ed0-8f71-204ffdf18e1e,0,3},\
        {0,900e3c92-6e18-4874-846a-b28780b5b54c,1,10b8ce49-ae3d-4a2e-afe7-1e3648bd59f7,1,\
        1c799cf9-342d-4bf7-9b6f-951a009228ce,1,3c00c6ee-844e-4620-85e4-671e72f114d9,1,\
        29da0973-3b85-40e5-89da-bce02dbab08e,1,02119c69-f08a-4142-9426-3725d74b7719,1,\
        07ef4641-f7da-417a-bd75-35c40a17c2f7,1,d066966a-ff6a-4a41-bd68-6191cab083bc,1,\
        f6168734-8b8d-4a88-ab39-ef6b51758e83,1,b9b44b51-3ac9-47cd-8b5a-df51afdcceb0,1,\
        818fc6c3-4691-44e3-a80c-e8d424730ead,1,155a0b35-4343-4047-989b-d385373b063e,1,\
        f7c6a0bb-bca6-4cd3-9146-832971cd7073,-1,4df6d046-3bf8-4dda-991c-53ba664296a5,1,\
        399d7390-8d83-4a57-b4d7-c902c15b701f,1}}},{0},4294967295,1,0,4294967295}";
    let rights_blob = deflate_for_test(rights_text.as_bytes());
    let object_refs = BTreeMap::from([(
        "4fa25267-a0df-4ed0-8f71-204ffdf18e1e".to_string(),
        "Configuration.InfostartToolkitPROF".to_string(),
    )]);
    let rights = parse_role_rights_blob(&rights_blob, &object_refs, &BTreeMap::new())
        .expect("the role is exported");
    let xml = format_role_rights_xml(&rights);
    let printed = xml
        .split("<name>")
        .skip(2)
        .filter_map(|tail| tail.split_once("</name>").map(|(name, _)| name))
        .collect::<Vec<_>>();
    assert_eq!(
        printed,
        vec![
            "Administration",
            "DataAdministration",
            "EventLog",
            "ThinClient",
            "ThickClient",
            "ExternalConnection",
            "Automation",
            "MainWindowModeNormal",
            "MainWindowModeWorkplace",
            "MainWindowModeEmbeddedWorkplace",
            "MainWindowModeFullscreenWorkplace",
            "MainWindowModeKiosk",
            "ExclusiveModeTerminationAtSessionStart",
            "ConfigurationExtensionsAdministration",
        ],
        "{xml}"
    );
}

/// A compact (`28`) option bag keeps a group's `Behavior` code at slot 24 --
/// `0` Usual, `1` Collapsible, `2` PopUp, the wide bag's own codes -- and at
/// slot 10 only whether it is anything but `Usual`. A real configuration
/// writes seven PopUp groups under this bag (slots 10/24 `1`/`2`, native
/// `<Behavior>PopUp</Behavior>`); requiring the two slots to be equal lost
/// them. The record is the checked-in compact-bag group with its bag's
/// behavior slots set as those seven carry them.
#[test]
fn a_compact_bag_group_reads_popup_behavior() {
    let field = r#"{22,{27,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,1,{0,{0,{"B",1},0}},5,"Группа1",{1,0},{1,0},0,1,0,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{28,1,0,0,1,{0},{1,0},{"Pattern"},"",{3,4,{0}},1,0,0,1,{1,0},0,0,3,3,2,0,1,1,{3,4,{0}},2,2,0,1},2,a9f3b1ac-f51b-431e-b102-55a69acdecad,{30,{1,02023637-7868-4a5f-8576-835a76e0c9ba},0,1,{0,{0,{"B",1},0}},1,"СоздатьНовыйБланк",{1,0},1,{1,409b9a53-7f7e-4178-86c1-33176c7c7a7a},{0},3,0,0,0,2,2,22,2,0,{3,4,{0}},{3,4,{0}},{3,4,{0}},{7,3,0,1,100},{0,0,0},0,{4,0,{0},"",-1,-1,1,0,""},1,{"Pattern"},"",2,0,1,{11,{33,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,0,"СоздатьНовыйБланкExtendedTooltip",{1,0},{1,0},1,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{5,0,0,3,0,{0,1,0},{3,4,{0}},{3,4,{0}},{3,0,{0},0,1,0,48312c09-257f-4b29-b280-284dd89efc1e}},0,1,2,{1,{1,0},0},0,0,1,0,0,1,0,3,3,0},{"U"},1,0,0,1,0,0,0,3,3,3,0,0,1,0,0,0,1},3d3cb80c-508b-41fa-8a18-680cdf5f1712,{11,{9,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,1,{0,{0,{"B",1},0}},0,"Декорация1",{1,1,{"ru","Будет открыт диалог для выбора параметров создания нового бланка.Вы сможете заменить текущий или создать новый бланк."}},{1,0},1,50,0,2,2,{3,3,{0,ad87bd29-0ad1-4da4-ac62-38e714e0cb9f}},{7,3,0,1,100},{0,0,0},1,{5,0,0,3,0,{0,1,0},{3,4,{0}},{3,4,{0}},{3,0,{0},0,1,0,48312c09-257f-4b29-b280-284dd89efc1e}},1,{22,{10,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,1,{0,{0,{"B",1},0}},8,"Декорация1КонтекстноеМеню",{1,0},{1,0},0,1,0,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{1,1},0,1,0,0,0,3,3,0},1,2,{1,{1,1,{"ru","Будет открыт диалог для выбора параметров создания нового бланка.Вы сможете заменить текущий или создать новый бланк."}},0},0,1,{11,{34,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,0,"Декорация1ExtendedTooltip",{1,0},{1,0},1,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{5,0,0,3,0,{0,1,0},{3,4,{0}},{3,4,{0}},{3,0,{0},0,1,0,48312c09-257f-4b29-b280-284dd89efc1e}},0,1,2,{1,{1,0},0},0,0,1,0,0,1,0,3,3,0},1,0,0,1,0,3,3,0},1,0,1,{11,{32,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,0,"Группа1ExtendedTooltip",{1,0},{1,0},1,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{5,0,0,3,0,{0,1,0},{3,4,{0}},{3,4,{0}},{3,0,{0},0,1,0,48312c09-257f-4b29-b280-284dd89efc1e}},0,1,2,{1,{1,0},0},0,0,1,0,0,1,0,3,3,0},0,3,3,0}"#;
    let item = parse_form_child_item(
        field,
        None,
        None,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &[],
        &BTreeMap::new(),
    )
    .unwrap();
    assert_eq!(item.tag, "UsualGroup");
    assert_eq!(item.behavior, Some("PopUp"));
    let xml = format_form_child_items_xml(&[item], 1);
    assert!(xml.contains("<Behavior>PopUp</Behavior>"), "{xml}");
}

#[test]
fn eds_guard_stored_reference_is_not_marked_foreign() {
    let id = "aaaaaaaa-0000-4000-8000-000000000001";
    let mut indexes = MetadataTypeIndexes::default();
    indexes
        .references
        .insert(id.to_owned(), "cfg:CatalogRef.Own".to_owned());
    merge_foreign_type_references(
        &mut indexes,
        &ForeignReferences {
            objects: BTreeMap::new(),
            types: vec![(
                id.to_owned(),
                "cfg:CatalogRef.Foreign".to_owned(),
                "Ref".to_owned(),
            )],
        },
    );
    assert!(indexes.foreign.is_empty());
    assert_eq!(indexes.references[id], "cfg:CatalogRef.Own");
}
