use formats_xml::form::{FormDialect, read_form, write_form};
use morph1c_core::ir::{DynamicListExt, FormBody, FormControlKind, FormEvent, FormItem};
use morph1c_core::version::{FormatVersion, with_roundtrip_target, with_source_version};
use sha2::{Digest, Sha256};

const SYMBOL: &str = "urn:future/../Событие";
fn event(name: &str) -> FormEvent {
    FormEvent {
        name: name.into(),
        handler: "CurrentHandler".into(),
    }
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn digest(body: &FormBody) -> String {
    hash(&serde_json::to_vec(body).unwrap())
}
fn write(body: &FormBody, dialect: FormDialect, minor: u16) -> Vec<u8> {
    with_roundtrip_target(FormatVersion::new(2, minor), || write_form(dialect, body)).unwrap()
}
fn read(bytes: &[u8], dialect: FormDialect, minor: u16) -> FormBody {
    with_source_version(Some(FormatVersion::new(2, minor)), || {
        read_form(dialect, bytes)
    })
    .unwrap()
}
fn fixture(minor: u16) -> FormBody {
    let mut body = FormBody::new();
    body.events.push(event("AfterWrite")); // Open on Form without matching ExtInfo.
    let mut table = FormItem::new(FormControlKind::new("Table"), "List", 1);
    table.events = vec![event(SYMBOL), event("BeforeRowChange")];
    table.dynamic_list_ext = Some(DynamicListExt {
        fields: vec![],
        events: vec![event("OnGetDataAtServer")],
    });
    body.items.push(table);
    let mut field = FormItem::new(FormControlKind::new("InputField"), "Input", 2);
    field.events = vec![event(SYMBOL), event("OnChange"), event("StartChoice")];
    body.items.push(field);
    read(
        &write(&body, FormDialect::Edt, minor),
        FormDialect::Edt,
        minor,
    )
}

#[test]
fn actual_container_kind_controls_exclusion_and_open_current_symbols() {
    for minor in [20, 21] {
        let body = fixture(minor);
        assert!(body.items[1].field_event_owners.is_none());
        let bytes = write(&body, FormDialect::Designer, minor);
        let native = read(&bytes, FormDialect::Designer, minor);
        assert_eq!(digest(&native), digest(&body));
        assert_eq!(
            hash(&write(&native, FormDialect::Designer, minor)),
            hash(&bytes)
        );
        assert_eq!(
            digest(&read(
                &write(&native, FormDialect::Edt, minor),
                FormDialect::Edt,
                minor
            )),
            digest(&native)
        );
        let mut no_ext = body.clone();
        no_ext.items[0].dynamic_list_ext = None;
        no_ext.items[0].events.push(event("OnGetDataAtServer"));
        assert_eq!(
            digest(&read(
                &write(&no_ext, FormDialect::Edt, minor),
                FormDialect::Edt,
                minor
            )),
            digest(&no_ext)
        );
        no_ext.items[0].dynamic_list_ext = body.items[0].dynamic_list_ext.clone();
        assert!(
            with_roundtrip_target(FormatVersion::new(2, minor), || write_form(
                FormDialect::Edt,
                &no_ext
            ))
            .is_err()
        );
        let mut changed_kind = body.clone();
        changed_kind.items[1].kind = FormControlKind::new("CheckBoxField");
        // StartChoice is open in a different actual extension with no such name.
        let changed = read(
            &write(&changed_kind, FormDialect::Edt, minor),
            FormDialect::Edt,
            minor,
        );
        assert_eq!(changed.items[1].events, changed_kind.items[1].events);
        assert_ne!(digest(&changed), digest(&body));
        let mut edited = body.clone();
        edited.items[1].events[0].handler = "CurrentEdited".into();
        edited.items[1].events[0].name = "Текущая/Ссылка".into();
        edited.items[1].events[..2].reverse();
        assert_ne!(digest(&edited), digest(&body));
        assert_eq!(
            digest(&read(
                &write(&edited, FormDialect::Edt, minor),
                FormDialect::Edt,
                minor
            )),
            digest(&edited)
        );
    }
}

#[test]
fn field_native_interleaving_is_lexical_and_handlers_are_current() {
    for minor in [20, 21] {
        let body = fixture(minor);
        let bytes = String::from_utf8(write(&body, FormDialect::Designer, minor)).unwrap();
        let start = bytes.find("<InputField ").unwrap();
        let suffix = &bytes[start..];
        let event = suffix.find("<Event name=\"StartChoice\"").unwrap() + start;
        let a = bytes[..event].rfind('\n').unwrap() + 1;
        let b = bytes[event..].find('\n').unwrap() + event + 1;
        let row = bytes[a..b].to_owned();
        let reordered = bytes[..a].to_owned() + &bytes[b..];
        let event = reordered[start..].find("<Event name=").unwrap() + start;
        let at = reordered[..event].rfind('\n').unwrap() + 1;
        let reordered = reordered[..at].to_owned() + &row + &reordered[at..];
        let mut native = read(reordered.as_bytes(), FormDialect::Designer, minor);
        let generated = write(&native, FormDialect::Designer, minor);
        if generated != reordered.as_bytes() {
            std::fs::write(
                std::env::temp_dir().join("event-order-source.xml"),
                reordered.as_bytes(),
            )
            .unwrap();
            std::fs::write(
                std::env::temp_dir().join("event-order-generated.xml"),
                &generated,
            )
            .unwrap();
        }
        assert_eq!(hash(&generated), hash(reordered.as_bytes()));
        assert_eq!(digest(&native), digest(&body));
        native.items[1]
            .events
            .iter_mut()
            .find(|e| e.name == "StartChoice")
            .unwrap()
            .handler = "EditedCurrent".into();
        let current = String::from_utf8(write(&native, FormDialect::Designer, minor)).unwrap();
        assert!(current.contains(">EditedCurrent</Event>"));
        assert_ne!(digest(&native), digest(&body));
        native.items[1].events.retain(|e| e.name != "StartChoice");
        assert!(
            !String::from_utf8(write(&native, FormDialect::Designer, minor))
                .unwrap()
                .contains("name=\"StartChoice\"")
        );
    }
}

#[test]
fn empty_duplicate_unknown_extension_wrong_owner_and_stale_binding_reject() {
    use morph1c_core::ir::form::{FieldEventOwner, FieldEventOwners};
    for minor in [20, 21] {
        let body = fixture(minor);
        let bytes = String::from_utf8(write(&body, FormDialect::Edt, minor)).unwrap();
        for (from, to) in [
            (
                "<event>StartChoice</event>",
                "<event>UnknownExtension</event>",
            ),
            ("<event>StartChoice</event>", "<event>OnChange</event>"),
            ("<event>OnChange</event>", "<event></event>"),
            ("<name>CurrentHandler</name>", "<name></name>"),
        ] {
            assert!(
                with_source_version(Some(FormatVersion::new(2, minor)), || read_form(
                    FormDialect::Edt,
                    bytes.replace(from, to).as_bytes()
                ))
                .is_err()
            );
        }
        let mut empty_handler = body.clone();
        empty_handler.items[0].events[0].handler.clear();
        for dialect in [FormDialect::Edt, FormDialect::Designer] {
            assert!(write_form(dialect, &empty_handler).is_err());
        }
        let mut duplicate = body.clone();
        duplicate.items[1].events.push(event(SYMBOL));
        assert!(write_form(FormDialect::Edt, &duplicate).is_err());
        let mut stale = body.clone();
        stale.items[1].field_event_owners = Some(FieldEventOwners {
            extension_kind: "form:InputFieldExtInfo".into(),
            extension_attached: true,
            owners: std::collections::BTreeMap::from([
                (SYMBOL.into(), FieldEventOwner::Body),
                ("OnChange".into(), FieldEventOwner::Body),
                ("StartChoice".into(), FieldEventOwner::Extension),
            ]),
        });
        stale.items[1].events[0].name = "EditedName".into();
        assert!(write_form(FormDialect::Edt, &stale).is_err());
        stale.items[1].events[0].name = SYMBOL.into();
        stale.items[1].kind = FormControlKind::new("LabelField");
        assert!(write_form(FormDialect::Edt, &stale).is_err());
        stale.items[1].kind = FormControlKind::new("InputField");
        stale.items[1]
            .field_event_owners
            .as_mut()
            .unwrap()
            .owners
            .insert("StartChoice".into(), FieldEventOwner::Body);
        assert!(write_form(FormDialect::Edt, &stale).is_err());
    }
}

#[test]
fn absent_field_extension_retains_open_known_spelling_and_current_topology() {
    use morph1c_core::ir::form::{FieldEventOwner, FieldEventOwners};
    for minor in [20, 21] {
        let body = fixture(minor);
        let bytes = String::from_utf8(write(&body, FormDialect::Edt, minor)).unwrap();
        let field = bytes.find("<type>InputField</type>").unwrap();
        let ext = bytes[field..].find("<extInfo ").unwrap() + field;
        let end = bytes[ext..].find("</extInfo>").unwrap() + ext + "</extInfo>".len();
        let missing = bytes[..ext].to_owned() + &bytes[end..];
        let mut nil = read(missing.as_bytes(), FormDialect::Edt, minor);
        nil.items[1].events.push(event("StartChoice"));
        nil.items[1].field_event_owners = Some(FieldEventOwners {
            extension_kind: "form:InputFieldExtInfo".into(),
            extension_attached: false,
            owners: nil.items[1]
                .events
                .iter()
                .map(|e| (e.name.clone(), FieldEventOwner::Body))
                .collect(),
        });
        let encoded = write(&nil, FormDialect::Edt, minor);
        assert_eq!(
            digest(&read(&encoded, FormDialect::Edt, minor)),
            digest(&nil)
        );
        assert_ne!(digest(&nil), digest(&body));
        nil.items[1]
            .field_event_owners
            .as_mut()
            .unwrap()
            .extension_attached = true;
        assert!(write_form(FormDialect::Edt, &nil).is_err());
    }
}

#[test]
#[ignore = "requires hash-bound genuine original/EDT/SDK Table sources on F"]
fn genuine_table_proxy_has_identical_current_order_and_handler_hashes_all_three() {
    let lab = std::path::PathBuf::from(std::env::var_os("IBCMD_EDT_LAB").unwrap());
    let proof: serde_json::Value = serde_json::from_slice(
        &std::fs::read(lab.join("uha205-table-event-primary-r1/triple-witness-r2.json")).unwrap(),
    )
    .unwrap();
    let mut rows = Vec::new();
    for input in proof["inputs"].as_object().unwrap().values() {
        let bytes = std::fs::read(input["path"].as_str().unwrap()).unwrap();
        assert_eq!(hash(&bytes), input["sha256"].as_str().unwrap());
        let dialect = if input["path"].as_str().unwrap().ends_with(".form") {
            FormDialect::Edt
        } else {
            FormDialect::Designer
        };
        let value = read(&bytes, dialect, 20);
        fn find(items: &[FormItem]) -> Option<&FormItem> {
            for item in items {
                if item.kind.as_str() == "Table" && item.id == 241 {
                    return Some(item);
                }
                if let Some(v) = find(&item.children) {
                    return Some(v);
                }
            }
            None
        }
        let table = find(&value.items).unwrap();
        let events: Vec<_> = table
            .events
            .iter()
            .map(|e| (e.name.clone(), hash(e.handler.as_bytes())))
            .collect();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].0, "b3c10170-c5ff-4cba-b537-679e1c872b45");
        rows.push(events);
        let mut scope = FormBody::new();
        scope.items.push(table.clone());
        for target in [FormDialect::Designer, FormDialect::Edt] {
            let returned = read(&write(&scope, target, 20), target, 20);
            assert_eq!(returned.items[0].events, scope.items[0].events);
            assert_eq!(
                returned.items[0]
                    .dynamic_list_ext
                    .as_ref()
                    .map(|e| &e.events),
                scope.items[0].dynamic_list_ext.as_ref().map(|e| &e.events)
            );
        }
    }
    assert_eq!(rows[0], rows[1]);
    assert_eq!(rows[1], rows[2]);
}

fn nil_fixture(minor: u16, none_type: bool) -> FormBody {
    use morph1c_core::ir::form::{FieldEventOwner, FieldEventOwners};
    let mut body = fixture(minor);
    body.items[0]
        .events
        .push(event("b3c10170-c5ff-4cba-b537-679e1c872b45"));
    let original = String::from_utf8(write(&body, FormDialect::Edt, minor)).unwrap();
    let field = original.find("<type>InputField</type>").unwrap();
    let start = original[field..].find("<extInfo ").unwrap() + field;
    let end = original[start..].find("</extInfo>").unwrap() + start + "</extInfo>".len();
    let missing = original[..start].to_owned() + &original[end..];
    let mut body = read(missing.as_bytes(), FormDialect::Edt, minor);
    let field = &mut body.items[1];
    field.events = vec![event("StartChoice"), event("OnChange"), event(SYMBOL)];
    field.field_event_owners = Some(FieldEventOwners {
        extension_kind: "form:InputFieldExtInfo".into(),
        extension_attached: false,
        owners: field
            .events
            .iter()
            .map(|e| (e.name.clone(), FieldEventOwner::Body))
            .collect(),
    });
    field.field_type_none = none_type;
    body
}
#[test]
fn nil_resource_binds_complete_current_owners_order_type_and_handlers() {
    use formats_xml::form::{apply_event_semantics_resource, write_event_semantics_resource};
    use morph1c_core::ir::Uuid;
    for minor in [20, 21] {
        for none_type in [false, true] {
            let body = nil_fixture(minor, none_type);
            let owner = Uuid([44; 16]);
            let resource = write_event_semantics_resource(&body, owner)
                .unwrap()
                .unwrap();
            let mut native = read(
                &write(&body, FormDialect::Designer, minor),
                FormDialect::Designer,
                minor,
            );
            assert_ne!(digest(&native), digest(&body));
            apply_event_semantics_resource(&mut native, owner, &resource).unwrap();
            assert_eq!(digest(&native), digest(&body));
            assert_eq!(
                digest(&read(
                    &write(&native, FormDialect::Edt, minor),
                    FormDialect::Edt,
                    minor
                )),
                digest(&body)
            );
            let original = digest(&native);
            native.items[1].events[0].handler = "CurrentNativeEdit".into();
            apply_event_semantics_resource(&mut native, owner, &resource).unwrap();
            assert_ne!(digest(&native), original);
            assert!(
                String::from_utf8(write(&native, FormDialect::Edt, minor))
                    .unwrap()
                    .contains("<name>CurrentNativeEdit</name>")
            );
            let source: serde_json::Value = serde_json::from_slice(&resource).unwrap();
            let mut cases = Vec::new();
            let mut value = source.clone();
            value["unknown"] = serde_json::json!(true);
            cases.push(value);
            let mut value = source.clone();
            value["records"][0]["path"][0]["id"] = serde_json::json!(99);
            cases.push(value);
            let mut value = source.clone();
            value["records"][0]["path"][0]["kind"] = serde_json::json!("Table");
            cases.push(value);
            let mut value = source.clone();
            value["records"][0]["owner"]["owners"]["UnknownEvent"] = serde_json::json!("Body");
            cases.push(value);
            let mut value = source.clone();
            value["records"][0]["owner"]["owners"]
                .as_object_mut()
                .unwrap()
                .remove("OnChange");
            cases.push(value);
            let mut value = source.clone();
            let row = value["records"][0].clone();
            value["records"].as_array_mut().unwrap().push(row);
            cases.push(value);
            for value in cases {
                let mut current = native.clone();
                assert!(
                    apply_event_semantics_resource(
                        &mut current,
                        owner,
                        &serde_json::to_vec(&value).unwrap()
                    )
                    .is_err()
                );
            }
            for duplicate in [
                "\"OnChange\":\"Body\",\"OnChange\":\"Body\"",
                "\"OnChange\":\"Extension\",\"OnChange\":\"Body\"",
            ] {
                let corrupt = String::from_utf8(serde_json::to_vec(&source).unwrap())
                    .unwrap()
                    .replace("\"OnChange\":\"Body\"", duplicate);
                assert!(
                    apply_event_semantics_resource(&mut native.clone(), owner, corrupt.as_bytes())
                        .is_err()
                );
            }
            assert!(
                apply_event_semantics_resource(&mut native.clone(), Uuid([45; 16]), &resource)
                    .is_err()
            );
            native.items[1].events.remove(0);
            assert!(apply_event_semantics_resource(&mut native, owner, &resource).is_err());
        }
    }
}
#[test]
fn public_nil_owner_type_exact_strip_current_edits_and_forged_hashes() {
    use ibcmd_edt::{
        ConversionOptions, Project, ReaderLimits, edt_to_xml, read_xml_source, xml_to_edt,
    };
    use ibcmd_xml::source_tree::{SourceEntry, SourceTree};
    use morph1c_core::ir::{MetadataObject, NamedFormBody, ObjectKind, PropertyValue, Token, Uuid};
    use morph1c_pipeline::{ConvertOptions, Format, read_config, write_config};
    for minor in [20, 21] {
        let fixture = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/subsystem-ci/src"
        ));
        let mut cfg = read_config(Format::Designer, fixture, &ConvertOptions::default())
            .unwrap()
            .0;
        let mut object = MetadataObject::new(
            ObjectKind::new("CommonForm"),
            "EventContainers",
            Uuid([44; 16]),
        );
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
        let mut body = nil_fixture(minor, true);
        body.items.push(
            independent_extension_fixture(minor, "LabelField")
                .items
                .remove(0),
        );
        object.form_bodies.push(NamedFormBody {
            name: "EventContainers".into(),
            body,
            ordinary_body: None,
            module: None,
            help: vec![],
            help_resources: vec![],
        });
        cfg.objects.push(object);
        let directory = tempfile::tempdir().unwrap();
        with_roundtrip_target(FormatVersion::new(2, minor), || {
            write_config(Format::Designer, &cfg, directory.path())
        })
        .unwrap();
        let path = directory.path().join("Configuration.xml");
        let root = String::from_utf8(std::fs::read(&path).unwrap())
            .unwrap()
            .replace(
                "</Language>",
                "</Language>\r\n\t\t\t<CommonForm>EventContainers</CommonForm>",
            );
        std::fs::write(path, root).unwrap();
        let options = ConversionOptions {
            edt_version: "2025.2.3".into(),
            xml_dialect: format!("2.{minor}"),
            runtime_version: Some(if minor == 20 { "8.3.27" } else { "8.5.1" }.into()),
        };
        let original = read_xml_source(directory.path(), ReaderLimits::default()).unwrap();
        let conversion = xml_to_edt(&original, &options).unwrap();
        assert!(
            conversion
                .extensions
                .iter()
                .any(|e| e.id == "ibcmd-form-event-semantics/1"
                    && e.resources == 1
                    && e.references == 2)
        );
        let generated = conversion.tree;
        assert!(
            edt_to_xml(&Project::from_tree(generated.clone()).unwrap(), &options)
                .unwrap()
                .tree
                == original
        );
        let form_path = "src/CommonForms/EventContainers/Form.form";
        let entry = generated
            .entries()
            .iter()
            .find(|e| e.path().as_str() == form_path)
            .unwrap();
        let mut current = read(entry.bytes(), FormDialect::Edt, minor);
        if let Some((_, PropertyValue::Bool(value))) = current.items[2]
            .ext_info
            .iter_mut()
            .find(|(_, v)| matches!(v, PropertyValue::Bool(_)))
        {
            *value = !*value;
        }
        let field = &mut current.items[1];
        field.events[0].name = "Текущая/Ссылка".into();
        field.events[0].handler = "EditedCurrentHandler".into();
        field.events.remove(1);
        field.events.push(event("AddedCurrent"));
        field.field_event_owners.as_mut().unwrap().owners = field
            .events
            .iter()
            .map(|e| {
                (
                    e.name.clone(),
                    morph1c_core::ir::form::FieldEventOwner::Body,
                )
            })
            .collect();
        let edited = write(&current, FormDialect::Edt, minor);
        let mut manifest: serde_json::Value = serde_json::from_slice(
            generated
                .entries()
                .iter()
                .find(|e| e.path().as_str() == ".ibcmd-provenance/manifest.json")
                .unwrap()
                .bytes(),
        )
        .unwrap();
        manifest["generated"][form_path] = serde_json::json!(hash(&edited));
        let alter = |strip: bool, edit: bool| {
            SourceTree::new(
                generated
                    .entries()
                    .iter()
                    .filter(|e| !strip || !e.path().as_str().starts_with(".ibcmd-provenance/"))
                    .map(|e| {
                        SourceEntry::from_bytes(
                            e.path().clone(),
                            if e.path().as_str() == form_path && edit {
                                edited.clone()
                            } else if e.path().as_str() == ".ibcmd-provenance/manifest.json" && edit
                            {
                                serde_json::to_vec_pretty(&manifest).unwrap()
                            } else {
                                e.bytes().to_vec()
                            },
                        )
                        .unwrap()
                    })
                    .collect(),
            )
            .unwrap()
        };
        assert!(edt_to_xml(&Project::from_tree(alter(false, true)).unwrap(), &options).is_err());
        for edit in [false, true] {
            let converted =
                edt_to_xml(&Project::from_tree(alter(true, edit)).unwrap(), &options).unwrap();
            assert!(
                converted
                    .extensions
                    .iter()
                    .any(|e| e.id == "ibcmd-form-event-semantics/1" && e.references == 2)
            );
            let native = converted
                .tree
                .entries()
                .iter()
                .find(|e| e.path().as_str() == "CommonForms/EventContainers/Ext/Form.xml")
                .unwrap();
            let resource = converted
                .tree
                .entries()
                .iter()
                .find(|e| {
                    e.path().as_str()
                        == "CommonForms/EventContainers/Ext/ibcmd-form-event-semantics.v1.json"
                })
                .unwrap();
            let mut decoded = read(native.bytes(), FormDialect::Designer, minor);
            formats_xml::form::apply_event_semantics_resource(
                &mut decoded,
                Uuid([44; 16]),
                resource.bytes(),
            )
            .unwrap();
            let expected = if edit {
                current.clone()
            } else {
                read(entry.bytes(), FormDialect::Edt, minor)
            };
            assert_eq!(digest(&decoded), digest(&expected));
            assert!(xml_to_edt(&converted.tree, &options).is_ok());
        }
    }
}

#[test]
fn sdk_independent_none_type_with_attached_extension_is_semantic_and_reversible() {
    use formats_xml::form::{apply_event_semantics_resource, write_event_semantics_resource};
    use morph1c_core::ir::Uuid;
    for minor in [20, 21] {
        let body = fixture(minor);
        let mut none = body.clone();
        none.items[1].field_type_none = true;
        let edt = write(&none, FormDialect::Edt, minor);
        let returned = read(&edt, FormDialect::Edt, minor);
        assert_eq!(digest(&returned), digest(&none));
        assert_ne!(digest(&none), digest(&body));
        assert!(returned.items[1].field_event_owners.is_none());
        let resource = write_event_semantics_resource(&none, Uuid([44; 16]))
            .unwrap()
            .unwrap();
        let mut native = read(
            &write(&none, FormDialect::Designer, minor),
            FormDialect::Designer,
            minor,
        );
        apply_event_semantics_resource(&mut native, Uuid([44; 16]), &resource).unwrap();
        assert_eq!(digest(&native), digest(&none));
        native.items[1].field_type_none = false;
        assert!(
            write_event_semantics_resource(&native, Uuid([44; 16]))
                .unwrap()
                .is_none()
        );
        assert_eq!(digest(&native), digest(&body));
    }
}

fn independent_extension_fixture(minor: u16, kind: &str) -> FormBody {
    let mut seed = FormBody::new();
    seed.items
        .push(FormItem::new(FormControlKind::new(kind), "Independent", 7));
    let bytes = String::from_utf8(write(&seed, FormDialect::Edt, minor)).unwrap();
    let mut bytes = bytes.replace(&format!("<type>{kind}</type>"), "<type>None</type>");
    let slot = if kind == "LabelField" {
        "useCopy"
    } else {
        "threeState"
    };
    let start = bytes.find("<extInfo ").unwrap();
    let end = start + bytes[start..].find('>').unwrap();
    let child = format!("<{slot}>true</{slot}>");
    if bytes.as_bytes()[end - 1] == b'/' {
        bytes.replace_range(end - 1..end + 1, &format!(">{child}</extInfo>"));
    } else {
        let close = bytes[start..].find("</extInfo>").unwrap() + start;
        bytes.insert_str(close, &child);
    }
    read(bytes.as_bytes(), FormDialect::Edt, minor)
}
#[test]
fn independent_none_type_label_and_checkbox_extensions_keep_current_payload() {
    use formats_xml::form::{apply_event_semantics_resource, write_event_semantics_resource};
    use morph1c_core::ir::Uuid;
    for minor in [20, 21] {
        for kind in ["LabelField", "CheckBoxField"] {
            let mut body = independent_extension_fixture(minor, kind);
            assert_eq!(body.items[0].kind.as_str(), "InputField");
            assert!(body.items[0].field_type_none);
            assert_eq!(
                body.items[0].field_extension_kind.as_deref(),
                Some(format!("form:{kind}ExtInfo").as_str())
            );
            body.items[0].events = vec![event("StartChoice"), event("OnChange"), event(SYMBOL)];
            let returned = read(
                &write(&body, FormDialect::Edt, minor),
                FormDialect::Edt,
                minor,
            );
            assert_eq!(digest(&returned), digest(&body));
            let owner = Uuid([77; 16]);
            let resource = write_event_semantics_resource(&body, owner)
                .unwrap()
                .unwrap();
            let native_bytes = write(&body, FormDialect::Designer, minor);
            let native_text = std::str::from_utf8(&native_bytes).unwrap();
            assert!(native_text.contains("<InputField "));
            assert!(!native_text.contains(&format!("<{kind} ")));
            let mut native = read(&native_bytes, FormDialect::Designer, minor);
            apply_event_semantics_resource(&mut native, owner, &resource).unwrap();
            assert_eq!(digest(&native), digest(&body));
            assert_eq!(
                digest(&read(
                    &write(&native, FormDialect::Edt, minor),
                    FormDialect::Edt,
                    minor
                )),
                digest(&body)
            );
            native.items[0].events[0].handler = "CurrentNotCached".into();
            assert_ne!(digest(&native), digest(&body));
            let new_resource = write_event_semantics_resource(&native, owner)
                .unwrap()
                .unwrap();
            let mut current = read(
                &write(&native, FormDialect::Designer, minor),
                FormDialect::Designer,
                minor,
            );
            apply_event_semantics_resource(&mut current, owner, &new_resource).unwrap();
            assert_eq!(current.items[0].events, native.items[0].events);
            // A CURRENT property edit is owned by the semantic payload, rather
            // than being replaced by the original extension's values.
            if let Some((_, morph1c_core::ir::PropertyValue::Bool(value))) = native.items[0]
                .ext_info
                .iter_mut()
                .find(|(_, v)| matches!(v, morph1c_core::ir::PropertyValue::Bool(_)))
            {
                *value = !*value;
                let changed = write_event_semantics_resource(&native, owner)
                    .unwrap()
                    .unwrap();
                assert_ne!(hash(&changed), hash(&resource));
                let mut current = read(
                    &write(&native, FormDialect::Designer, minor),
                    FormDialect::Designer,
                    minor,
                );
                apply_event_semantics_resource(&mut current, owner, &changed).unwrap();
                assert_eq!(current.items[0].ext_info, native.items[0].ext_info);
            }
            let mut explicit = body.clone();
            explicit.items[0].field_type_none = false;
            assert!(write_form(FormDialect::Edt, &explicit).is_err());
            assert!(write_form(FormDialect::Designer, &explicit).is_err());
            let source: serde_json::Value = serde_json::from_slice(&resource).unwrap();
            for extra in ["actual_extension", "extension_payload"] {
                let mut corrupt = source.clone();
                corrupt["records"][0][extra] = serde_json::json!(null);
                assert!(
                    apply_event_semantics_resource(
                        &mut read(&native_bytes, FormDialect::Designer, minor),
                        owner,
                        &serde_json::to_vec(&corrupt).unwrap()
                    )
                    .is_err()
                );
            }
            // Deep derive-deserialized types have the same strict shape policy
            // as the resource envelope; no unknown/duplicate keys disappear.
            let font: morph1c_core::ir::form::FontRef =
                serde_json::from_value(serde_json::json!({"font_ref":null,"height":"11.0"}))
                    .unwrap();
            let mut corrupt = source.clone();
            let mut valid_font = serde_json::to_value(&font).unwrap();
            valid_font["unknown"] = serde_json::json!(true);
            corrupt["records"][0]["extension_payload"]["font"] = valid_font;
            let err = apply_event_semantics_resource(
                &mut read(&native_bytes, FormDialect::Designer, minor),
                owner,
                &serde_json::to_vec(&corrupt).unwrap(),
            )
            .unwrap_err();
            assert!(err.to_string().contains("unknown nested resource field"));
            let mut corrupt = source.clone();
            let mut child = serde_json::to_value(FormItem::new(
                FormControlKind::new("InputField"),
                "Nested",
                9,
            ))
            .unwrap();
            child["unknown"] = serde_json::json!(true);
            corrupt["records"][0]["extension_payload"]["additions"] = serde_json::json!([child]);
            let err = apply_event_semantics_resource(
                &mut read(&native_bytes, FormDialect::Designer, minor),
                owner,
                &serde_json::to_vec(&corrupt).unwrap(),
            )
            .unwrap_err();
            assert!(err.to_string().contains("unknown nested resource field"));
            let mut corrupt = source.clone();
            corrupt["records"][0]["extension_payload"]["font"] =
                serde_json::to_value(&font).unwrap();
            let encoded = serde_json::to_string(&corrupt).unwrap();
            for duplicate in [
                "\"height\":\"11.0\",\"height\":\"11.0\"",
                "\"height\":\"12.0\",\"height\":\"11.0\"",
            ] {
                let dup = encoded.replace("\"height\":\"11.0\"", duplicate);
                assert_ne!(dup, encoded);
                let err = apply_event_semantics_resource(
                    &mut read(&native_bytes, FormDialect::Designer, minor),
                    owner,
                    dup.as_bytes(),
                )
                .unwrap_err();
                assert!(err.to_string().contains("duplicate resource object key"));
            }
            let compact = serde_json::to_string(&source).unwrap();
            for duplicate in [
                "\"font\":null,\"font\":null",
                "\"font\":{\"unknown\":true},\"font\":null",
            ] {
                let corrupt = compact.replace("\"font\":null", duplicate);
                assert_ne!(corrupt, compact);
                assert!(
                    apply_event_semantics_resource(
                        &mut read(&native_bytes, FormDialect::Designer, minor),
                        owner,
                        corrupt.as_bytes()
                    )
                    .is_err()
                );
            }
            let mut corrupt = source.clone();
            let properties = corrupt["records"][0]["extension_payload"]["properties"]
                .as_array_mut()
                .unwrap();
            properties.push(properties[0].clone());
            assert!(
                apply_event_semantics_resource(
                    &mut read(&native_bytes, FormDialect::Designer, minor),
                    owner,
                    &serde_json::to_vec(&corrupt).unwrap()
                )
                .is_err()
            );
        }
    }
}

#[test]
fn independent_gantt_payload_owns_nested_current_fields_without_orphan_records() {
    use formats_xml::form::{apply_event_semantics_resource, write_event_semantics_resource};
    use morph1c_core::ir::Uuid;
    for minor in [20, 21] {
        let mut seed = FormBody::new();
        let mut field = FormItem::new(FormControlKind::new("GanttChartField"), "Gantt", 8);
        let mut table = FormItem::new(FormControlKind::new("Table"), "NestedTable", 9);
        table
            .children
            .push(nil_fixture(minor, true).items.remove(1));
        field.auto_table = Some(Box::new(table));
        seed.items.push(field);
        let bytes = String::from_utf8(write(&seed, FormDialect::Edt, minor))
            .unwrap()
            .replace("<type>GanttChartField</type>", "<type>None</type>");
        let body = read(bytes.as_bytes(), FormDialect::Edt, minor);
        let owner = Uuid([88; 16]);
        let resource = write_event_semantics_resource(&body, owner)
            .unwrap()
            .unwrap();
        let records: serde_json::Value = serde_json::from_slice(&resource).unwrap();
        assert_eq!(records["records"].as_array().unwrap().len(), 1);
        let mut native = read(
            &write(&body, FormDialect::Designer, minor),
            FormDialect::Designer,
            minor,
        );
        with_source_version(Some(FormatVersion::new(2, minor)), || {
            apply_event_semantics_resource(&mut native, owner, &resource)
        })
        .unwrap();
        assert_eq!(digest(&native), digest(&body));
        if minor == 21 {
            let mut wrong = read(
                &write(&body, FormDialect::Designer, minor),
                FormDialect::Designer,
                minor,
            );
            assert!(
                with_source_version(Some(FormatVersion::new(2, 20)), || {
                    apply_event_semantics_resource(&mut wrong, owner, &resource)
                })
                .is_err()
            );
        }
        native.items[0].auto_table.as_mut().unwrap().children[0].events[0].handler =
            "CurrentNested".into();
        let changed = write_event_semantics_resource(&native, owner)
            .unwrap()
            .unwrap();
        assert_ne!(hash(&changed), hash(&resource));
        let mut current = read(
            &write(&native, FormDialect::Designer, minor),
            FormDialect::Designer,
            minor,
        );
        apply_event_semantics_resource(&mut current, owner, &changed).unwrap();
        assert_eq!(digest(&current), digest(&native));
        assert_eq!(
            digest(&read(
                &write(&current, FormDialect::Edt, minor),
                FormDialect::Edt,
                minor
            )),
            digest(&native)
        );
    }
}
