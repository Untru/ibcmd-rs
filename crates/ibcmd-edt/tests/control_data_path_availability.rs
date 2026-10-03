use formats_xml::form::{
    FormDialect, FormProjectionContext, bind_native_availability_sources, read_form, write_form,
    write_form_with_context,
};
use morph1c_core::{
    ir::{
        Configuration, FormBody, FormControlKind, FormItem, MetadataObject, NamedFormBody,
        ObjectKind, PropertyValue, Uuid,
    },
    spec::forms::controls::{button as bt, form_field as ff},
    version::{FormatVersion, with_roundtrip_target},
};

fn body() -> FormBody {
    let xml = concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
        "<form:Form xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:core=\"http://g5.1c.ru/v8/dt/mcore\" xmlns:form=\"http://g5.1c.ru/v8/dt/form\" xmlns:schema=\"http://g5.1c.ru/v8/dt/data-composition-system/schema\" xmlns:settings=\"http://g5.1c.ru/v8/dt/data-composition-system/settings\">\r\n",
        "<attributes><name>List</name><valueType><types>DynamicList</types></valueType>",
        "<view><common>true</common></view><edit><common>true</common></edit>",
        "<extInfo xsi:type=\"form:DynamicListExtInfo\"><mainTable>InformationRegister.Register</mainTable>",
        "<customQuery>true</customQuery><autoFillAvailableFields>true</autoFillAvailableFields>",
        "<queryText>SELECT R.A AS A FROM InformationRegister.Register AS R</queryText>",
        "</extInfo></attributes>\r\n</form:Form>\r\n"
    );
    let mut body = read_form(FormDialect::Edt, xml.as_bytes()).unwrap();
    for (kind, name, id, path) in [
        ("LabelField", "Known", 1, "List.A"),
        ("LabelField", "Missing", 2, "List.B"),
        ("Button", "MissingButton", 3, "List.B"),
        ("LabelField", "Unrelated", 4, "Other.B"),
        ("LabelField", "Literal", 5, "~Other.B"),
    ] {
        let mut item = FormItem::new(FormControlKind::new(kind), name, id);
        item.properties.push((
            if kind == "Button" {
                bt::F_DATA_PATH
            } else {
                ff::F_DATA_PATH
            },
            PropertyValue::Ref(path.into()),
        ));
        if kind == "Button" {
            // Native Button.Type is always emitted; make the synthetic bag
            // equivalent to a codec-read current button before raw roundtrip.
            item.properties.push((
                bt::F_BUTTON_TYPE,
                PropertyValue::Enum(morph1c_core::ir::Token::new(bt::BUTTON_TYPE_EDT_DEFAULT)),
            ));
        }
        body.items.push(item);
    }
    body
}
fn paths(bytes: &[u8]) -> Vec<String> {
    formats_xml::parse(bytes)
        .unwrap()
        .root
        .child("ChildItems")
        .unwrap()
        .children
        .iter()
        .map(|item| item.child("DataPath").unwrap().text.clone())
        .collect()
}
fn write(body: &FormBody, minor: u16) -> Vec<u8> {
    with_roundtrip_target(FormatVersion::new(2, minor), || {
        write_form(FormDialect::Designer, body)
    })
    .unwrap()
}
fn assert_same_native(actual: &[u8], expected: &[u8]) {
    let first = actual
        .iter()
        .zip(expected)
        .position(|(a, b)| a != b)
        .unwrap_or(actual.len().min(expected.len()));
    assert!(
        actual == expected,
        "native bytes differ at {first}; lengths actual={} expected={}",
        actual.len(),
        expected.len()
    );
}
fn query(body: &mut FormBody, sql: &str) {
    body.data_attributes[0]
        .dynamic_list
        .as_mut()
        .unwrap()
        .query_text = Some(sql.into());
}
#[test]
fn all_current_control_data_path_codecs_use_sdk_dynamic_list_availability() {
    let body = body();
    for minor in [20, 21] {
        assert_eq!(
            paths(&write(&body, minor)),
            ["List.A", "~List.B", "~List.B", "Other.B", "~Other.B"]
        );
    }
    assert_eq!(
        body.items[1].get(ff::F_DATA_PATH),
        Some(&PropertyValue::Ref("List.B".into()))
    );
    let mut current = body.clone();
    query(
        &mut current,
        "SELECT R.B AS B FROM InformationRegister.Register AS R",
    );
    assert_eq!(
        paths(&write(&current, 20)),
        ["~List.A", "List.B", "List.B", "Other.B", "~Other.B"]
    );
    // A separate subsequent write must not inherit another form's native-origin
    // or CURRENT availability TLS state.
    assert_eq!(
        paths(&write(&body, 21)),
        ["List.A", "~List.B", "~List.B", "Other.B", "~Other.B"]
    );
}
#[test]
fn native_markers_require_current_query_path_and_target_dependencies() {
    for minor in [20, 21] {
        let original = write(&body(), minor);
        let native = read_form(FormDialect::Designer, &original).unwrap();
        assert_same_native(&write(&native, minor), &original);
        let mut edited = native.clone();
        query(
            &mut edited,
            "SELECT R.B AS B FROM InformationRegister.Register AS R",
        );
        assert_eq!(
            &paths(&write(&edited, minor))[..3],
            ["~List.A", "List.B", "List.B"]
        );
        let mut edited = native.clone();
        edited.items[1]
            .properties
            .iter_mut()
            .find(|(id, _)| *id == ff::F_DATA_PATH)
            .unwrap()
            .1 = PropertyValue::Ref("List.A".into());
        assert_eq!(paths(&write(&edited, minor))[1], "List.A");
    }
    let authored = String::from_utf8(write(&body(), 20))
        .unwrap()
        .replace(
            "<DataPath>List.A</DataPath>",
            "<DataPath>~List.A</DataPath>",
        )
        .into_bytes();
    let native = read_form(FormDialect::Designer, &authored).unwrap();
    assert_same_native(&write(&native, 20), &authored);
    assert_eq!(
        paths(&write(&native, 21))[0],
        "List.A",
        "explicit target change invalidates authored marker"
    );
}
#[test]
fn complete_current_metadata_edits_recompute_control_markers_without_source_values() {
    for minor in [20, 21] {
        let version = FormatVersion::new(2, minor);
        let mut source = body();
        query(
            &mut source,
            "SELECT R.* FROM InformationRegister.Register AS R",
        );
        let mut config = Configuration::new().with_source_version(Some(version));
        let mut register = MetadataObject::new(
            ObjectKind::new("InformationRegister"),
            "Register",
            Uuid([1; 16]),
        );
        register.children.push(MetadataObject::new(
            ObjectKind::new("InformationRegister.Resource"),
            "A",
            Uuid([2; 16]),
        ));
        config.objects.push(register);
        let context = FormProjectionContext::new(&config).unwrap();
        let native = with_roundtrip_target(version, || {
            write_form_with_context(FormDialect::Designer, &source, &context)
        })
        .unwrap();
        assert_eq!(&paths(&native)[..3], ["List.A", "~List.B", "~List.B"]);
        let mut owner = MetadataObject::new(ObjectKind::new("CommonForm"), "Form", Uuid([3; 16]));
        owner.form_bodies.push(NamedFormBody {
            name: "Form".into(),
            body: read_form(FormDialect::Designer, &native).unwrap(),
            ordinary_body: None,
            module: None,
            help: vec![],
            help_resources: vec![],
        });
        config.objects.push(owner);
        bind_native_availability_sources(&mut config).unwrap();
        let emit = |config: &Configuration| {
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
        assert_same_native(&emit(&config), &native);
        config.objects[1].form_bodies[0].module = Some("current module edit".into());
        // Module bytes are not availability dependencies.
        assert_same_native(&emit(&config), &native);
        config.objects[0].children[0].name = "B".into();
        assert_eq!(&paths(&emit(&config))[..3], ["~List.A", "List.B", "List.B"]);
        config.objects[1].form_bodies[0].body.items[1]
            .properties
            .iter_mut()
            .find(|(id, _)| *id == ff::F_DATA_PATH)
            .unwrap()
            .1 = PropertyValue::Ref("List.A".into());
        assert_eq!(
            paths(&emit(&config))[1],
            "~List.A",
            "new CURRENT path is resolved against CURRENT metadata"
        );
    }
}

#[test]
fn canonical_control_paths_and_semantic_digest_survive_native_and_edt_readback() {
    use sha2::{Digest, Sha256};
    for minor in [20, 21] {
        let version = FormatVersion::new(2, minor);
        let mut current = body();
        // This test covers ordinary SDK segment identities. The deliberately
        // authored literal-prefix case above is a separate EDT-origin check;
        // native '~' is a protocol delimiter, not an escaped segment character.
        current.items.pop();
        let edt =
            with_roundtrip_target(version, || write_form(FormDialect::Edt, &current)).unwrap();
        let current = read_form(FormDialect::Edt, &edt).unwrap();
        let semantic = serde_json::to_vec(&current).unwrap();
        let native = write(&current, minor);
        let returned = read_form(FormDialect::Designer, &native).unwrap();
        let returned_semantic = serde_json::to_vec(&returned).unwrap();
        assert!(
            semantic == returned_semantic,
            "CURRENT typed form semantics changed: {:x}/{:x}",
            Sha256::digest(&semantic),
            Sha256::digest(&returned_semantic)
        );
        assert_eq!(
            returned.items[1].get(ff::F_DATA_PATH),
            Some(&PropertyValue::Ref("List.B".into()))
        );
        let edt_return =
            with_roundtrip_target(version, || write_form(FormDialect::Edt, &returned)).unwrap();
        let doc = formats_xml::parse(&edt_return).unwrap();
        let segments: Vec<_> = doc
            .root
            .children
            .iter()
            .filter(|child| child.local == "items")
            .map(|child| {
                child
                    .child("dataPath")
                    .unwrap()
                    .child("segments")
                    .unwrap()
                    .text
                    .clone()
            })
            .collect();
        assert_eq!(segments, ["List.A", "List.B", "List.B", "Other.B"]);
        let second = read_form(FormDialect::Edt, &edt_return).unwrap();
        assert!(
            semantic == serde_json::to_vec(&second).unwrap(),
            "second EDT read changed CURRENT semantic digest"
        );
    }
}

#[test]
fn current_backing_field_identity_controls_required_injection_independently_of_alias() {
    let mut cfg = Configuration::new();
    for (index, name) in ["Items", "Other"].into_iter().enumerate() {
        cfg.objects.push(MetadataObject::new(
            ObjectKind::new("Catalog"),
            name,
            Uuid([index as u8 + 1; 16]),
        ));
    }
    let mut body = body();
    body.data_attributes[0]
        .dynamic_list
        .as_mut()
        .unwrap()
        .main_table = Some("Catalog.Items".into());
    body.items.clear();
    let requested = [
        "List.Ref",
        "List.Ссылка",
        "List.Custom",
        "List.Order",
        "List.Порядок",
        "List.DeletionMark",
    ];
    for (index, path) in requested.into_iter().enumerate() {
        let mut item = FormItem::new(
            FormControlKind::new("LabelField"),
            format!("Field{index}"),
            index as i64 + 1,
        );
        item.properties
            .push((ff::F_DATA_PATH, PropertyValue::Ref(path.into())));
        body.items.push(item);
    }
    // DynamicListAttributeService removes selected DbViewFieldDef identities
    // before standard-field injection; AS names never stand in for that set.
    for minor in [20, 21] {
        for (sql, expected) in [
            (
                "SELECT I.Ref AS Custom FROM Catalog.Items AS I",
                [
                    "~List.Ref",
                    "~List.Ссылка",
                    "List.Custom",
                    "List.Order",
                    "List.Порядок",
                    "List.DeletionMark",
                ],
            ),
            (
                "SELECT I.Ссылка AS Custom FROM Catalog.Items AS I",
                [
                    "~List.Ref",
                    "~List.Ссылка",
                    "List.Custom",
                    "List.Order",
                    "List.Порядок",
                    "List.DeletionMark",
                ],
            ),
            (
                "SELECT 0 AS Ссылка FROM Catalog.Items AS I",
                [
                    "List.Ref",
                    "List.Ссылка",
                    "~List.Custom",
                    "List.Order",
                    "List.Порядок",
                    "List.DeletionMark",
                ],
            ),
            (
                "SELECT O.Ref AS Custom FROM Catalog.Items AS I INNER JOIN Catalog.Other AS O ON TRUE",
                [
                    "List.Ref",
                    "List.Ссылка",
                    "List.Custom",
                    "List.Order",
                    "List.Порядок",
                    "List.DeletionMark",
                ],
            ),
            (
                "SELECT I.Ref AS Custom FROM Catalog.Items AS I UNION ALL SELECT I.Ref AS Custom FROM Catalog.Other AS I",
                [
                    "~List.Ref",
                    "~List.Ссылка",
                    "List.Custom",
                    "List.Order",
                    "List.Порядок",
                    "List.DeletionMark",
                ],
            ),
            (
                "SELECT I.Ref AS Custom FROM Catalog.Other AS I UNION ALL SELECT I.Ref AS Custom FROM Catalog.Items AS I",
                [
                    "~List.Ref",
                    "~List.Ссылка",
                    "List.Custom",
                    "List.Order",
                    "List.Порядок",
                    "List.DeletionMark",
                ],
            ),
            // Only the final result SELECT owns this availability namespace.
            (
                "SELECT I.Ref AS Custom INTO Tmp FROM Catalog.Items AS I; SELECT 0 AS Ссылка FROM Catalog.Items AS I",
                [
                    "List.Ref",
                    "List.Ссылка",
                    "~List.Custom",
                    "List.Order",
                    "List.Порядок",
                    "List.DeletionMark",
                ],
            ),
        ] {
            query(&mut body, sql);
            let before = serde_json::to_value(&body).unwrap();
            let context = FormProjectionContext::new(&cfg).unwrap();
            let bytes = with_roundtrip_target(FormatVersion::new(2, minor), || {
                write_form_with_context(FormDialect::Designer, &body, &context)
            })
            .unwrap();
            assert_eq!(paths(&bytes), expected, "profile 2.{minor}: {sql}");
            assert_eq!(
                serde_json::to_value(&body).unwrap(),
                before,
                "projection changed CURRENT data"
            );
        }
    }
}

#[test]
fn custom_attributes_do_not_acquire_language_twins_from_other_metadata_families() {
    let mut cfg = Configuration::new();
    let mut catalog = MetadataObject::new(ObjectKind::new("Catalog"), "Items", Uuid([1; 16]));
    catalog.children.push(MetadataObject::new(
        ObjectKind::new("Catalog.Attribute"),
        "Date",
        Uuid([2; 16]),
    ));
    cfg.objects.push(catalog);
    cfg.objects.push(MetadataObject::new(
        ObjectKind::new("Document"),
        "Other",
        Uuid([3; 16]),
    ));
    let mut body = body();
    body.data_attributes[0]
        .dynamic_list
        .as_mut()
        .unwrap()
        .main_table = Some("Catalog.Items".into());
    body.items.clear();
    for (index, path) in ["List.Date", "List.Дата"].into_iter().enumerate() {
        let mut item = FormItem::new(
            FormControlKind::new("LabelField"),
            format!("Field{index}"),
            index as i64 + 1,
        );
        item.properties
            .push((ff::F_DATA_PATH, PropertyValue::Ref(path.into())));
        body.items.push(item);
    }
    for minor in [20, 21] {
        for (name, sql, expected) in [
            (
                "Date",
                "SELECT I.Date FROM Catalog.Items AS I",
                ["List.Date", "~List.Дата"],
            ),
            (
                "Date",
                "SELECT I.Date FROM Catalog.Items AS I LEFT JOIN Document.Other AS D ON TRUE",
                ["List.Date", "~List.Дата"],
            ),
            (
                "Дата",
                "SELECT I.Дата FROM Catalog.Items AS I",
                ["~List.Date", "List.Дата"],
            ),
        ] {
            cfg.objects[0].children[0].name = name.into();
            query(&mut body, sql);
            let context = FormProjectionContext::new(&cfg).unwrap();
            let native = with_roundtrip_target(FormatVersion::new(2, minor), || {
                write_form_with_context(FormDialect::Designer, &body, &context)
            })
            .unwrap();
            assert_eq!(paths(&native), expected, "profile 2.{minor}: {sql}");
        }
    }
}

#[test]
fn default_dynamic_list_calculated_fields_use_current_names_without_a_query() {
    let mut cfg = Configuration::new();
    cfg.objects.push(MetadataObject::new(
        ObjectKind::new("Catalog"),
        "Items",
        Uuid([1; 16]),
    ));
    let mut body = body();
    let list = body.data_attributes[0].dynamic_list.as_mut().unwrap();
    list.main_table = Some("Catalog.Items".into());
    list.custom_query = false;
    list.query_text = None;
    list.calculated_fields.push(
        serde_json::from_value::<morph1c_core::ir::form::DcsCalculatedField>(serde_json::json!({
            "data_path": "Computed", "expression": ""
        }))
        .unwrap(),
    );
    body.items.clear();
    for (index, path) in ["List.Computed", "List.Renamed", "List.Missing"]
        .into_iter()
        .enumerate()
    {
        let mut item = FormItem::new(
            FormControlKind::new("LabelField"),
            format!("Field{index}"),
            index as i64 + 1,
        );
        item.properties
            .push((ff::F_DATA_PATH, PropertyValue::Ref(path.into())));
        body.items.push(item);
    }
    for minor in [20, 21] {
        for (name, expected) in [
            (
                "Computed",
                ["List.Computed", "~List.Renamed", "~List.Missing"],
            ),
            (
                "Renamed",
                ["~List.Computed", "List.Renamed", "~List.Missing"],
            ),
        ] {
            body.data_attributes[0]
                .dynamic_list
                .as_mut()
                .unwrap()
                .calculated_fields[0]
                .data_path = name.into();
            let before = serde_json::to_value(&body).unwrap();
            let context = FormProjectionContext::new(&cfg).unwrap();
            let bytes = with_roundtrip_target(FormatVersion::new(2, minor), || {
                write_form_with_context(FormDialect::Designer, &body, &context)
            })
            .unwrap();
            assert_eq!(paths(&bytes), expected, "profile 2.{minor}: {name}");
            assert_eq!(serde_json::to_value(&body).unwrap(), before);
        }
    }
}

#[test]
fn calculated_field_availability_uses_current_compatibility_independently_of_xml_profile() {
    use morph1c_core::spec::metadata::configuration::F_COMPATIBILITY_MODE;
    let mut cfg = Configuration::new();
    cfg.objects.push(MetadataObject::new(
        ObjectKind::new("Catalog"),
        "Items",
        Uuid([1; 16]),
    ));
    let mut body = body();
    let list = body.data_attributes[0].dynamic_list.as_mut().unwrap();
    list.main_table = Some("Catalog.Items".into());
    list.calculated_fields.push(
        serde_json::from_value(serde_json::json!({ "data_path": "Computed", "expression": "" }))
            .unwrap(),
    );
    body.items.clear();
    let mut item = FormItem::new(FormControlKind::new("LabelField"), "Computed", 1);
    item.properties
        .push((ff::F_DATA_PATH, PropertyValue::Ref("List.Computed".into())));
    body.items.push(item);
    for minor in [20, 21] {
        for mode in [
            None,
            Some(""),
            Some("8.3.18"),
            Some("8.3.19"),
            Some("8.5.1"),
            Some("DontUse"),
        ] {
            cfg.properties.clear();
            if let Some(mode) = mode {
                cfg.properties.push((
                    F_COMPATIBILITY_MODE,
                    PropertyValue::Enum(morph1c_core::ir::Token::new(mode)),
                ));
            }
            let visible = !matches!(mode, Some("8.3.18" | "DontUse"));
            for (custom_query, auto_fill) in [(false, true), (true, true), (true, false)] {
                let list = body.data_attributes[0].dynamic_list.as_mut().unwrap();
                list.custom_query = custom_query;
                list.auto_fill_available_fields = auto_fill;
                query(&mut body, "SELECT I.Ref FROM Catalog.Items AS I");
                let context = FormProjectionContext::new(&cfg).unwrap();
                let bytes = with_roundtrip_target(FormatVersion::new(2, minor), || {
                    write_form_with_context(FormDialect::Designer, &body, &context)
                })
                .unwrap();
                assert_eq!(
                    paths(&bytes),
                    [if visible {
                        "List.Computed"
                    } else {
                        "~List.Computed"
                    }],
                    "profile2.{minor} mode{mode:?} custom{custom_query} auto{auto_fill}"
                );
            }
        }
        cfg.properties.clear();
        let first = FormProjectionContext::new(&cfg)
            .unwrap()
            .dependency_sha256(&body, FormatVersion::new(2, minor))
            .unwrap();
        cfg.properties.push((
            F_COMPATIBILITY_MODE,
            PropertyValue::Enum(morph1c_core::ir::Token::new("8.3.18")),
        ));
        let second = FormProjectionContext::new(&cfg)
            .unwrap()
            .dependency_sha256(&body, FormatVersion::new(2, minor))
            .unwrap();
        assert_ne!(
            first, second,
            "compatibility edit must invalidate saved availability spelling"
        );
    }
}

#[test]
fn common_attributes_follow_current_content_separator_policy_and_metadata_family() {
    use morph1c_core::ir::Token;
    use morph1c_core::spec::metadata::common_attribute as ca;
    let mut body = body();
    body.items.clear();
    let mut item = FormItem::new(FormControlKind::new("LabelField"), "Shared", 1);
    item.properties
        .push((ff::F_DATA_PATH, PropertyValue::Ref("List.Shared".into())));
    body.items.push(item);
    body.data_attributes[0]
        .dynamic_list
        .as_mut()
        .unwrap()
        .custom_query = false;
    for minor in [20, 21] {
        for (family, auto_use, usage, separation, separated_use, journal_has_documents, expected) in [
            (
                "Catalog",
                "DontUse",
                Some("Use"),
                "DontUse",
                "Independently",
                false,
                "List.Shared",
            ),
            (
                "Catalog",
                "Use",
                Some("DontUse"),
                "DontUse",
                "Independently",
                false,
                "~List.Shared",
            ),
            (
                "Catalog",
                "Use",
                Some("Auto"),
                "DontUse",
                "Independently",
                false,
                "List.Shared",
            ),
            (
                "Catalog",
                "DontUse",
                Some("Auto"),
                "DontUse",
                "Independently",
                false,
                "~List.Shared",
            ),
            (
                "Catalog",
                "Use",
                None,
                "DontUse",
                "Independently",
                false,
                "List.Shared",
            ),
            (
                "Catalog",
                "DontUse",
                Some("Use"),
                "Separate",
                "Independently",
                false,
                "~List.Shared",
            ),
            (
                "Catalog",
                "DontUse",
                Some("Use"),
                "Separate",
                "IndependentlyAndSimultaneously",
                false,
                "List.Shared",
            ),
            (
                "ExchangePlan",
                "DontUse",
                Some("Use"),
                "Separate",
                "Independently",
                false,
                "List.Shared",
            ),
            (
                "Enum",
                "Use",
                None,
                "DontUse",
                "Independently",
                false,
                "~List.Shared",
            ),
            (
                "DocumentJournal",
                "Use",
                None,
                "Separate",
                "IndependentlyAndSimultaneously",
                false,
                "~List.Shared",
            ),
            (
                "DocumentJournal",
                "Use",
                None,
                "Separate",
                "IndependentlyAndSimultaneously",
                true,
                "List.Shared",
            ),
        ] {
            let mut cfg = Configuration::new();
            let mut target = MetadataObject::new(ObjectKind::new(family), "Items", Uuid([1; 16]));
            if journal_has_documents {
                let spec = morph1c_core::spec::registry::spec_for(family).unwrap();
                let field = spec
                    .fields
                    .iter()
                    .find(|f| f.name == "registeredDocuments")
                    .unwrap();
                target.properties.push((
                    field.id,
                    PropertyValue::List(vec![PropertyValue::Ref("Document.Document".into())]),
                ));
            }
            cfg.objects.push(target);
            let mut common =
                MetadataObject::new(ObjectKind::new("CommonAttribute"), "Shared", Uuid([2; 16]));
            common.properties = vec![
                (ca::F_AUTO_USE, PropertyValue::Enum(Token::new(auto_use))),
                (
                    ca::F_DATA_SEPARATION,
                    PropertyValue::Enum(Token::new(separation)),
                ),
                (
                    ca::F_SEPARATED_DATA_USE,
                    PropertyValue::Enum(Token::new(separated_use)),
                ),
                (
                    ca::F_CONTENT,
                    PropertyValue::List(
                        usage
                            .map(|usage| {
                                vec![PropertyValue::List(vec![
                                    PropertyValue::Str(format!("{family}.Items")),
                                    PropertyValue::Enum(Token::new(usage)),
                                ])]
                            })
                            .unwrap_or_default(),
                    ),
                ),
            ];
            cfg.objects.push(common);
            cfg.objects.push(MetadataObject::new(
                ObjectKind::new("Document"),
                "Document",
                Uuid([3; 16]),
            ));
            body.data_attributes[0]
                .dynamic_list
                .as_mut()
                .unwrap()
                .main_table = Some(format!("{family}.Items"));
            let before = serde_json::to_value(&body).unwrap();
            let context = FormProjectionContext::new(&cfg).unwrap();
            let digest = context
                .dependency_sha256(&body, FormatVersion::new(2, minor))
                .unwrap();
            let bytes = with_roundtrip_target(FormatVersion::new(2, minor), || {
                write_form_with_context(FormDialect::Designer, &body, &context)
            })
            .unwrap();
            assert_eq!(
                paths(&bytes),
                [expected],
                "profile2.{minor} family{family} usage{usage:?} auto{auto_use} separator{separation}/{separated_use}"
            );
            assert_eq!(serde_json::to_value(&body).unwrap(), before);
            cfg.objects[1].name = "Renamed".into();
            let changed = FormProjectionContext::new(&cfg).unwrap();
            assert_ne!(
                digest,
                changed
                    .dependency_sha256(&body, FormatVersion::new(2, minor))
                    .unwrap()
            );
            let bytes = with_roundtrip_target(FormatVersion::new(2, minor), || {
                write_form_with_context(FormDialect::Designer, &body, &changed)
            })
            .unwrap();
            assert_eq!(
                paths(&bytes),
                ["~List.Shared"],
                "old common name must not remain available"
            );
        }
    }
}
