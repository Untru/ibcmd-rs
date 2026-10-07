//! CURRENT metadata members and table-owned rows; native markers are projections,
//! never replacements for authored paths or a reason to normalize typed form data.
use formats_xml::form::{
    FormDialect, FormProjectionContext, read_form, write_form, write_form_with_context,
};
use morph1c_core::{
    ir::{
        Configuration, FormBody, FormControlKind, FormItem, MetadataObject, ObjectKind,
        PropertyValue, TypeRef, TypeSpec, Uuid,
    },
    spec::forms::controls::{form_field as ff, table as tb},
    version::{FormatVersion, with_roundtrip_target, with_source_version},
};
use sha2::{Digest, Sha256};

fn read(bytes: &[u8], dialect: FormDialect, version: FormatVersion) -> FormBody {
    with_source_version(Some(version), || read_form(dialect, bytes)).unwrap()
}
fn form(version: FormatVersion, tables: bool) -> FormBody {
    let table = if tables {
        concat!(
            "<autoCommandBar><name>FormCommandBar</name><id>-1</id></autoCommandBar>",
            "<items xsi:type=\"form:Table\"><name>Rows</name><id>1</id><dataPath xsi:type=\"form:DataPath\"><segments>List</segments></dataPath></items>",
            "<items xsi:type=\"form:Table\"><name>OtherRows</name><id>2</id><dataPath xsi:type=\"form:DataPath\"><segments>OtherList</segments></dataPath></items>"
        )
    } else {
        ""
    };
    let xml = format!(
        concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
            "<form:Form xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:form=\"http://g5.1c.ru/v8/dt/form\">{}",
            "<attributes><name>List</name><valueType><types>DynamicList</types></valueType><view><common>true</common></view><edit><common>true</common></edit><extInfo xsi:type=\"form:DynamicListExtInfo\"><mainTable>Catalog.Source</mainTable><autoFillAvailableFields>true</autoFillAvailableFields></extInfo></attributes>",
            "<attributes><name>OtherList</name><valueType><types>DynamicList</types></valueType><view><common>true</common></view><edit><common>true</common></edit><extInfo xsi:type=\"form:DynamicListExtInfo\"><mainTable>Catalog.Source</mainTable><customQuery>true</customQuery><autoFillAvailableFields>true</autoFillAvailableFields><queryText>SELECT S.Ref AS Link FROM Catalog.Source AS S</queryText></extInfo></attributes>",
            "</form:Form>\r\n"
        ),
        table
    );
    read(xml.as_bytes(), FormDialect::Edt, version)
}
fn add_paths(body: &mut FormBody, paths: &[&str], version: FormatVersion) {
    for (index, path) in paths.iter().enumerate() {
        let mut item = FormItem::new(
            FormControlKind::new("LabelField"),
            format!("Field{index}"),
            10 + index as i64,
        );
        item.properties
            .push((ff::F_DATA_PATH, PropertyValue::Ref((*path).into())));
        body.items.push(item);
    }
    // Materialize the actual EDT-authored fixture before defining its complete
    // CURRENT semantic identity, as in the existing control-path regression.
    let bytes = with_roundtrip_target(version, || write_form(FormDialect::Edt, body)).unwrap();
    *body = read(&bytes, FormDialect::Edt, version);
}
fn child(owner: &mut MetadataObject, name: &str, ty: &str) {
    let kind = format!("{}.Attribute", owner.kind.as_str());
    let mut field = MetadataObject::new(ObjectKind::new(&kind), name, Uuid([2; 16]));
    let id = morph1c_core::spec::registry::spec_for(&kind)
        .unwrap()
        .fields
        .iter()
        .find(|field| field.name == "type")
        .unwrap()
        .id;
    field.properties.push((
        id,
        PropertyValue::Type(TypeSpec {
            parts: vec![TypeRef {
                id: ty.into(),
                qualifier: None,
            }],
        }),
    ));
    owner.children.push(field);
}
fn metadata() -> Configuration {
    let mut config = Configuration::new();
    let mut source = MetadataObject::new(ObjectKind::new("Catalog"), "Source", Uuid([1; 16]));
    child(&mut source, "Reference", "CatalogRef.Target");
    let mut target = MetadataObject::new(ObjectKind::new("Catalog"), "Target", Uuid([3; 16]));
    child(&mut target, "Known", "Boolean");
    child(&mut target, "Next", "CatalogRef.Target");
    config.objects = vec![source, target];
    config
}
fn native(body: &FormBody, config: &Configuration, version: FormatVersion) -> Vec<u8> {
    let context = FormProjectionContext::new(config).unwrap();
    with_roundtrip_target(version, || {
        write_form_with_context(FormDialect::Designer, body, &context)
    })
    .unwrap()
}
fn emitted_paths(bytes: &[u8]) -> Vec<String> {
    let document = formats_xml::parse(bytes).unwrap();
    let mut pending = vec![&document.root];
    let mut paths = Vec::new();
    while let Some(element) = pending.pop() {
        if element.local == "DataPath" {
            paths.push(element.text.clone());
        }
        pending.extend(element.children.iter().rev());
    }
    paths
}
fn assert_cycle(body: &FormBody, config: &Configuration, version: FormatVersion) -> Vec<u8> {
    let original = serde_json::to_vec(body).unwrap();
    let bytes = native(body, config, version);
    let returned = read(&bytes, FormDialect::Designer, version);
    assert_eq!(
        serde_json::to_vec(&returned).unwrap(),
        original,
        "native reread changed complete typed IR"
    );
    let edt = with_roundtrip_target(version, || write_form(FormDialect::Edt, &returned)).unwrap();
    let second = read(&edt, FormDialect::Edt, version);
    assert_eq!(
        serde_json::to_vec(&second).unwrap(),
        original,
        "EDT reread changed complete typed IR"
    );
    assert_eq!(native(&second, config, version), bytes);
    assert_eq!(
        serde_json::to_vec(body).unwrap(),
        original,
        "projection mutated authored IR"
    );
    bytes
}

#[test]
fn nested_default_list_paths_resolve_current_declared_reference_members_without_depth_caps() {
    for minor in [20, 21] {
        let version = FormatVersion::new(2, minor);
        let mut body = form(version, false);
        let deep = format!("List.Reference.{}Known", "Next.".repeat(128));
        add_paths(
            &mut body,
            &[
                "List.Reference.Known",
                "List.Reference.Missing",
                "List.Unknown.Known",
                "Unknown.Reference.Missing",
                "OtherList.Link.Missing",
                &deep,
                "List.Reference.Presentation",
                "List.Reference.Представление",
                "List.Reference.Rows.Member",
            ],
            version,
        );
        let mut config = metadata();
        let mut section = MetadataObject::new(
            ObjectKind::new("Catalog.TabularSection"),
            "Rows",
            Uuid([4; 16]),
        );
        section.children.push(MetadataObject::new(
            ObjectKind::new("Catalog.TabularSection.Attribute"),
            "Member",
            Uuid([5; 16]),
        ));
        config.objects[1].children.push(section);
        assert_eq!(
            emitted_paths(&assert_cycle(&body, &config, version)),
            [
                "List.Reference.Known",
                "~List.Reference.Missing",
                "~List.Unknown.Known",
                "Unknown.Reference.Missing",
                "OtherList.Link.Missing",
                &deep,
                "~List.Reference.Presentation",
                "~List.Reference.Представление",
                "List.Reference.Rows.Member",
            ]
        );
        config.objects[1].children[0].name = "Missing".into();
        assert_eq!(
            emitted_paths(&assert_cycle(&body, &config, version))[..2],
            ["~List.Reference.Known", "List.Reference.Missing"]
        );
        // The edited path wins; metadata has no source-owned cached path values.
        body.items[0]
            .properties
            .iter_mut()
            .find(|(id, _)| *id == ff::F_DATA_PATH)
            .unwrap()
            .1 = PropertyValue::Ref("List.Reference.Missing".into());
        assert_eq!(
            emitted_paths(&assert_cycle(&body, &config, version))[0],
            "List.Reference.Missing"
        );
        // Missing referenced metadata and an unclaimed type provider prove
        // neither presence nor absence of deeper members.
        config.objects.pop();
        assert_eq!(
            emitted_paths(&assert_cycle(&body, &config, version))[1],
            "List.Reference.Missing"
        );
    }
}

#[test]
fn table_current_data_uses_control_identity_current_binding_and_query_output_namespace() {
    for minor in [20, 21] {
        let version = FormatVersion::new(2, minor);
        let mut body = form(version, true);
        add_paths(
            &mut body,
            &[
                "Items.Rows.CurrentData.Ref",
                "Items.OtherRows.CurrentData.Ref",
                "Items.OtherRows.CurrentData.Link",
                "Элементы.Rows.ТекущиеДанные.Reference.Missing",
                "Items.Missing.CurrentData.Ref",
                "Items.Rows.OtherProperty.Missing",
                "Items.OTHERROWS[0].currentdata[0].ref",
            ],
            version,
        );
        let config = metadata();
        let expected = [
            "List",
            "OtherList",
            "Items.Rows.CurrentData.Ref",
            "~Items.OtherRows.CurrentData.Ref",
            "Items.OtherRows.CurrentData.Link",
            "~Элементы.Rows.ТекущиеДанные.Reference.Missing",
            "Items.Missing.CurrentData.Ref",
            "Items.Rows.OtherProperty.Missing",
            "~Items.OTHERROWS[0].currentdata[0].ref",
        ];
        assert_eq!(
            emitted_paths(&assert_cycle(&body, &config, version)),
            expected
        );
        body.items[0]
            .properties
            .iter_mut()
            .find(|(id, _)| *id == tb::F_DATA_PATH)
            .unwrap()
            .1 = PropertyValue::Ref("OtherList".into());
        assert_eq!(
            emitted_paths(&assert_cycle(&body, &config, version))[2],
            "~Items.Rows.CurrentData.Ref"
        );
        body.items[0].name = "Renamed".into();
        assert_eq!(
            emitted_paths(&assert_cycle(&body, &config, version))[2],
            "Items.Rows.CurrentData.Ref",
            "missing owner must not acquire another table's namespace"
        );
        body.items[1]
            .properties
            .retain(|(id, _)| *id != tb::F_DATA_PATH);
        assert!(
            emitted_paths(&assert_cycle(&body, &config, version))
                .contains(&"Items.OtherRows.CurrentData.Ref".to_owned()),
            "removed binding must not reuse its old list namespace"
        );
    }
}

#[test]
fn unsupported_reference_type_and_explicit_column_provider_keep_unknown_fallback() {
    for minor in [20, 21] {
        let version = FormatVersion::new(2, minor);
        let mut body = form(version, false);
        add_paths(&mut body, &["List.Reference.Missing"], version);
        let mut config = metadata();
        let PropertyValue::Type(ty) = &mut config.objects[0].children[0].properties[0].1 else {
            panic!()
        };
        ty.parts.push(TypeRef {
            id: "AnyRef".into(),
            qualifier: None,
        });
        assert_eq!(
            emitted_paths(&assert_cycle(&body, &config, version)),
            ["List.Reference.Missing"]
        );
        config = metadata();
        let mut column = body.data_attributes[1].clone();
        column.dynamic_list = None;
        column.name = "Reference".into();
        column.id = 3;
        column.value_type = Some(TypeSpec {
            parts: vec![TypeRef {
                id: "Boolean".into(),
                qualifier: None,
            }],
        });
        body.data_attributes[0].columns.push(column);
        assert_eq!(
            emitted_paths(&assert_cycle(&body, &config, version)),
            ["List.Reference.Missing"]
        );
    }
}

#[test]
fn reference_union_retarget_and_current_password_compatibility_preserve_complete_form() {
    use morph1c_core::{ir::Token, spec::metadata::configuration::F_COMPATIBILITY_MODE};
    for minor in [20, 21] {
        let version = FormatVersion::new(2, minor);
        let mut body = form(version, false);
        add_paths(
            &mut body,
            &[
                "List.Reference.Known",
                "List.Reference.Foreign",
                "List.Reference.Secret",
                "List.Reference.Missing",
            ],
            version,
        );
        let mut config = metadata();
        child(&mut config.objects[1], "Secret", "String");
        let password = morph1c_core::spec::registry::spec_for("Catalog.Attribute")
            .unwrap()
            .fields
            .iter()
            .find(|field| field.name == "passwordMode")
            .unwrap()
            .id;
        config.objects[1]
            .children
            .last_mut()
            .unwrap()
            .properties
            .push((password, PropertyValue::Bool(true)));
        let mut other = MetadataObject::new(ObjectKind::new("Catalog"), "Other", Uuid([6; 16]));
        child(&mut other, "Foreign", "Boolean");
        config.objects.push(other);
        let PropertyValue::Type(ty) = &mut config.objects[0].children[0].properties[0].1 else {
            panic!()
        };
        ty.parts.push(TypeRef {
            id: "CatalogRef.Other".into(),
            qualifier: None,
        });
        config.properties.push((
            F_COMPATIBILITY_MODE,
            PropertyValue::Enum(Token::new("8.3.11")),
        ));
        assert_eq!(
            emitted_paths(&assert_cycle(&body, &config, version)),
            [
                "List.Reference.Known",
                "List.Reference.Foreign",
                "~List.Reference.Secret",
                "~List.Reference.Missing"
            ]
        );
        config.properties[0].1 = PropertyValue::Enum(Token::new("8.3.12"));
        assert_eq!(
            emitted_paths(&assert_cycle(&body, &config, version))[2],
            "List.Reference.Secret",
            "CURRENT project compatibility, independently of XML target"
        );
        let PropertyValue::Type(ty) = &mut config.objects[0].children[0].properties[0].1 else {
            panic!()
        };
        ty.parts.remove(0);
        assert_eq!(
            emitted_paths(&assert_cycle(&body, &config, version)),
            [
                "~List.Reference.Known",
                "List.Reference.Foreign",
                "~List.Reference.Secret",
                "~List.Reference.Missing"
            ]
        );
        child(&mut config.objects[2], "Known", "Boolean");
        assert_eq!(
            emitted_paths(&assert_cycle(&body, &config, version))[0],
            "List.Reference.Known",
            "CURRENT added child must not replay an obsolete negative marker"
        );
    }
}

#[test]
#[ignore = "requires two immutable SHA-bound UH source/SDK pairs on F; no SDK execution"]
fn genuine_nested_and_control_owned_paths_match_sdk_without_expected_normalization() {
    use morph1c_pipeline::{
        Format, FormatRegistry, attach_form_body, layout, write_form_bodies_with_context,
    };
    let lab = std::path::PathBuf::from(std::env::var_os("IBCMD_EDT_LAB").unwrap());
    let source_root = lab.join("oracle-uha83-r1/authentic-workspace/OracleConfiguration/src");
    let sdk_root = lab.join("native-reference-uha83-r1/native-xml");
    let registry = FormatRegistry::for_format(Format::Edt).unwrap();
    let sha = |bytes: &[u8]| format!("{:x}", Sha256::digest(bytes));
    for (path, form_sha, sdk_sha, kind, member) in [
        (
            "AccumulationRegisters/ПланированиеПотребностей/Forms/ФормаСписка",
            "6556f7620d163e4ded8787b5e4c75bb39831f0680499efaa7fa42907437bc90b",
            "708f9a95cfcd8e7fce610eeb4b2becb70274c949f266d6024392947cfa879473",
            "AccumulationRegister",
            "Список.АналитикаИсточника.Этап",
        ),
        (
            "Catalogs/Лоты/Forms/ЛотыДоступныеПоставщику",
            "77f9e13a0239675f7564e9a177ba37f1b6ace937ea8085db02f0c5c423727e88",
            "ebb4ace6dc1d58c40796f6f4619fc50d1ab5bdaaadd6889d2ad27b244207f187",
            "Catalog",
            "Items.Список.CurrentData.Ref",
        ),
    ] {
        let form_path = source_root.join(path).join("Form.form");
        let expected_path = sdk_root.join(path).join("Ext/Form.xml");
        let source = std::fs::read(&form_path).unwrap();
        let expected = std::fs::read(&expected_path).unwrap();
        assert_eq!(sha(&source), form_sha);
        assert_eq!(sha(&expected), sdk_sha);
        let settings_path = source_root
            .join(path)
            .join("Attributes/Список/ExtInfo/ListSettings.dcss");
        assert_eq!(
            sha(&std::fs::read(&settings_path).unwrap()),
            "543637ca543d7c131f81c73ad3e2d7c72c8d9c340913a5ec87fd687d59fb3af2"
        );
        if kind == "Catalog" {
            assert_eq!(
                sha(&std::fs::read(source_root.join(path).join("Module.bsl")).unwrap()),
                "072da667404fff66d4dc1a5a09bb7710abe921862ae815a8cd328f90451cd043"
            );
        }
        for minor in [20, 21] {
            let version = FormatVersion::new(2, minor);
            let mut config = Configuration::new();
            // Mirror the production CURRENT root compatibility/common attributes;
            // target dialect must never substitute for project compatibility.
            let root_kind = registry.root_kind().unwrap();
            let root_path = layout::singleton_path(&registry, root_kind, &source_root).unwrap();
            let root_bytes = std::fs::read(&root_path).unwrap();
            let root =
                with_source_version(Some(version), || (root_kind.read)(&root_bytes)).unwrap();
            config.properties = root.properties;
            let mut context_bindings = vec![(root_path, sha(&root_bytes))];
            let mut pictures = std::collections::BTreeMap::new();
            for (directory, object_kind) in [
                ("CommonAttributes", "CommonAttribute"),
                ("CommonPictures", "CommonPicture"),
            ] {
                for entry in std::fs::read_dir(source_root.join(directory)).unwrap() {
                    let entry = entry.unwrap();
                    if !entry.file_type().unwrap().is_dir() {
                        continue;
                    }
                    let descriptor = entry
                        .path()
                        .join(format!("{}.mdo", entry.file_name().to_str().unwrap()));
                    let bytes = std::fs::read(&descriptor).unwrap();
                    let object = with_source_version(Some(version), || {
                        (registry.get(object_kind).unwrap().read)(&bytes)
                    })
                    .unwrap();
                    context_bindings.push((descriptor, sha(&bytes)));
                    if object_kind == "CommonAttribute" {
                        config.objects.push(object);
                    } else {
                        use morph1c_core::spec::metadata::common_picture::F_TRANSPARENT_PIXEL;
                        let transparent = match object.get(F_TRANSPARENT_PIXEL) {
                            None => false,
                            Some(PropertyValue::List(values)) => !values.is_empty(),
                            _ => panic!("invalid CURRENT CommonPicture transparency"),
                        };
                        assert!(
                            pictures
                                .insert(format!("CommonPicture.{}", object.name), transparent)
                                .is_none()
                        );
                    }
                }
            }
            for (descriptor, descriptor_kind, digest) in [
                (
                    "AccumulationRegisters/ПланированиеПотребностей/ПланированиеПотребностей.mdo",
                    "AccumulationRegister",
                    "fe459005c82ce47553c69f31ced23106e1b8d83db156917ab69da8235702df04",
                ),
                (
                    "Catalogs/КлючиАналитикиПланирования/КлючиАналитикиПланирования.mdo",
                    "Catalog",
                    "ca9b4ddfcd3fdcae812f3759a0f1ce7980a5a0a34c5eb6896d6112fd4d66ff62",
                ),
                (
                    "Catalogs/Лоты/Лоты.mdo",
                    "Catalog",
                    "d86b31f2295a3c2075cc5b255e01ca4fa0cc71f951c4407884a280bc3ab81928",
                ),
            ] {
                let bytes = std::fs::read(source_root.join(descriptor)).unwrap();
                assert_eq!(sha(&bytes), digest);
                context_bindings.push((source_root.join(descriptor), digest.to_owned()));
                let object = with_source_version(Some(version), || {
                    (registry.get(descriptor_kind).unwrap().read)(&bytes)
                })
                .unwrap();
                config.objects.push(object);
            }
            let parts: Vec<_> = path.split('/').collect();
            let anchor = source_root
                .join(parts[0])
                .join(parts[1])
                .join(format!("{}.mdo", parts[1]));
            let bytes = std::fs::read(&anchor).unwrap();
            let mut owner =
                with_source_version(Some(version), || (registry.get(kind).unwrap().read)(&bytes))
                    .unwrap();
            owner.children.retain(|child| {
                child.kind.as_str().ends_with(".FormRef") && child.name == parts[3]
            });
            assert_eq!(owner.children.len(), 1);
            with_source_version(Some(version), || {
                attach_form_body(Format::Edt, kind, &anchor, &mut owner)
            })
            .unwrap();
            assert_eq!(owner.form_bodies.len(), 1);
            let form_uuid = owner.children[0].uuid;
            formats_xml::form::resolve_common_picture_transparency(
                &mut owner.form_bodies[0].body,
                &pictures,
                true,
            )
            .unwrap();
            formats_xml::form::bind_picture_semantics(
                &mut owner.form_bodies[0].body,
                form_uuid,
                true,
            )
            .unwrap();
            let before = serde_json::to_vec(&owner.form_bodies).unwrap();
            let context = FormProjectionContext::new(&config).unwrap();
            let scratch = tempfile::tempdir().unwrap();
            let native_anchor = scratch.path().join(format!("{}.xml", owner.name));
            with_roundtrip_target(version, || {
                write_form_bodies_with_context(Format::Designer, &native_anchor, &owner, &context)
            })
            .unwrap();
            let generated_path = scratch
                .path()
                .join(&owner.name)
                .join("Forms")
                .join(parts[3])
                .join("Ext/Form.xml");
            let generated = std::fs::read(generated_path).unwrap();
            let mut returned = owner.clone();
            returned.form_bodies.clear();
            with_source_version(Some(version), || {
                attach_form_body(Format::Designer, kind, &native_anchor, &mut returned)
            })
            .unwrap();
            formats_xml::form::resolve_common_picture_transparency(
                &mut returned.form_bodies[0].body,
                &pictures,
                false,
            )
            .unwrap();
            formats_xml::form::bind_picture_semantics(
                &mut returned.form_bodies[0].body,
                form_uuid,
                false,
            )
            .unwrap();
            assert_eq!(
                serde_json::to_vec(&returned.form_bodies).unwrap(),
                before,
                "complete native form transport: {path}"
            );
            let edt_anchor = scratch
                .path()
                .join("edt")
                .join(format!("{}.mdo", owner.name));
            with_roundtrip_target(version, || {
                write_form_bodies_with_context(Format::Edt, &edt_anchor, &returned, &context)
            })
            .unwrap();
            returned.form_bodies.clear();
            with_source_version(Some(version), || {
                attach_form_body(Format::Edt, kind, &edt_anchor, &mut returned)
            })
            .unwrap();
            formats_xml::form::resolve_common_picture_transparency(
                &mut returned.form_bodies[0].body,
                &pictures,
                true,
            )
            .unwrap();
            formats_xml::form::bind_picture_semantics(
                &mut returned.form_bodies[0].body,
                form_uuid,
                true,
            )
            .unwrap();
            assert_eq!(
                serde_json::to_vec(&returned.form_bodies).unwrap(),
                before,
                "complete EDT form transport: {path}"
            );
            assert_eq!(
                serde_json::to_vec(&owner.form_bodies).unwrap(),
                before,
                "projection mutated original form"
            );
            // A scoped projection assertion; unrelated residual families are
            // still adjudicated by ROOT's complete production corpus replay.
            let selected = |bytes: &[u8]| {
                emitted_paths(bytes)
                    .into_iter()
                    .filter(|path| path.trim_start_matches('~') == member)
                    .collect::<Vec<_>>()
            };
            let sdk_paths = selected(&expected);
            assert!(!sdk_paths.is_empty(), "empty genuine witness: {path}");
            assert!(sdk_paths.iter().all(|path| path == &format!("~{member}")));
            assert_eq!(selected(&generated), sdk_paths, "{path}: 2.{minor}");
            for (descriptor, digest) in context_bindings {
                assert_eq!(
                    sha(&std::fs::read(descriptor).unwrap()),
                    digest,
                    "CURRENT context changed during projection"
                );
            }
        }
        assert_eq!(sha(&std::fs::read(form_path).unwrap()), form_sha);
        assert_eq!(sha(&std::fs::read(expected_path).unwrap()), sdk_sha);
        assert_eq!(
            sha(&std::fs::read(settings_path).unwrap()),
            "543637ca543d7c131f81c73ad3e2d7c72c8d9c340913a5ec87fd687d59fb3af2"
        );
        if kind == "Catalog" {
            assert_eq!(
                sha(&std::fs::read(source_root.join(path).join("Module.bsl")).unwrap()),
                "072da667404fff66d4dc1a5a09bb7710abe921862ae815a8cd328f90451cd043"
            );
        }
    }
}
