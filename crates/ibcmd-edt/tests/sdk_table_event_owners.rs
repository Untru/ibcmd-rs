use formats_xml::form::{FormDialect, read_form, write_form};
use morph1c_core::ir::form::NativeFormEventOrder;
use morph1c_core::ir::{DynamicListExt, FormBody, FormControlKind, FormEvent, FormItem};
use morph1c_core::version::{FormatVersion, with_roundtrip_target, with_source_version};
use sha2::{Digest, Sha256};

fn event(name: &str) -> FormEvent {
    FormEvent {
        name: name.into(),
        handler: format!("Handle{name}"),
    }
}
fn fixture() -> FormBody {
    let mut body = FormBody::new();
    let mut table = FormItem::new(FormControlKind::new("Table"), "List", 1);
    table.events = vec![event("BeforeRowChange"), event("OnChange")];
    table.dynamic_list_ext = Some(DynamicListExt {
        fields: Vec::new(),
        events: vec![event("OnGetDataAtServer")],
    });
    body.items.push(table);
    read_form(
        FormDialect::Edt,
        &write_form(FormDialect::Edt, &body).unwrap(),
    )
    .unwrap()
}
fn digest(body: &FormBody) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(body).unwrap()))
}
fn native_names(body: &FormBody) -> Vec<String> {
    let bytes = write_form(FormDialect::Designer, body).unwrap();
    let doc = formats_xml::parse(&bytes).unwrap();
    doc.root
        .child("ChildItems")
        .unwrap()
        .child("Table")
        .unwrap()
        .child("Events")
        .unwrap()
        .children
        .iter()
        .map(|event| event.attr("name").unwrap().value.clone())
        .collect()
}

#[test]
fn table_owners_handlers_and_orders_are_semantic() {
    let body = fixture();
    let bytes = write_form(FormDialect::Designer, &body).unwrap();
    let native = read_form(FormDialect::Designer, &bytes).unwrap();
    assert_eq!(native.items[0].events, body.items[0].events);
    assert_eq!(
        native.items[0].dynamic_list_ext.as_ref().unwrap().events,
        body.items[0].dynamic_list_ext.as_ref().unwrap().events
    );
    assert_eq!(write_form(FormDialect::Designer, &native).unwrap(), bytes);
    let mut edited = native;
    let before = digest(&edited);
    edited.items[0].dynamic_list_ext.as_mut().unwrap().events[0].handler =
        "CurrentEditedHandler".into();
    assert_ne!(digest(&edited), before);
    let edt = write_form(FormDialect::Edt, &edited).unwrap();
    assert_eq!(
        digest(&read_form(FormDialect::Edt, &edt).unwrap()),
        digest(&edited)
    );
    assert!(
        String::from_utf8(write_form(FormDialect::Designer, &edited).unwrap())
            .unwrap()
            .contains(">CurrentEditedHandler</Event>")
    );
    edited.items[0].events.reverse();
    assert_ne!(digest(&edited), before);
    let edt = write_form(FormDialect::Edt, &edited).unwrap();
    assert_eq!(
        digest(&read_form(FormDialect::Edt, &edt).unwrap()),
        digest(&edited)
    );
}

#[test]
fn replay_uses_current_values_and_requires_exact_owner_orders_and_kind() {
    let mut body = fixture();
    body.items[0].events.reverse();
    let baseline = digest(&body);
    body.items[0].native_table_event_order = Some(NativeFormEventOrder {
        extension_kind: Some("form:DynamicListTableExtInfo".into()),
        merged: vec![
            "OnChange".into(),
            "BeforeRowChange".into(),
            "OnGetDataAtServer".into(),
        ],
        root: vec!["OnChange".into(), "BeforeRowChange".into()],
        extension: vec!["OnGetDataAtServer".into()],
    });
    assert_eq!(digest(&body), baseline);
    let replay = native_names(&body);
    assert_eq!(replay, ["OnChange", "BeforeRowChange", "OnGetDataAtServer"]);
    body.items[0].events.reverse();
    assert_ne!(native_names(&body), replay);
    body.items[0].events.reverse();
    body.items[0]
        .native_table_event_order
        .as_mut()
        .unwrap()
        .extension_kind = Some("form:UnknownExtInfo".into());
    assert_ne!(native_names(&body), replay);
    body.items[0].dynamic_list_ext = None;
    assert!(!native_names(&body).contains(&"OnGetDataAtServer".into()));
    body.items[0].kind = FormControlKind::new("UnknownControl");
    assert!(write_form(FormDialect::Designer, &body).is_err());
}

#[test]
fn wrong_owner_duplicate_unknown_and_unknown_ext_kind_fail_closed() {
    let body = fixture();
    let mut edited = body.clone();
    let misplaced = edited.items[0]
        .dynamic_list_ext
        .as_mut()
        .unwrap()
        .events
        .remove(0);
    edited.items[0].events.push(misplaced);
    assert!(write_form(FormDialect::Edt, &edited).is_err());
    assert!(write_form(FormDialect::Designer, &edited).is_err());
    let bytes = String::from_utf8(write_form(FormDialect::Edt, &body).unwrap()).unwrap();
    for (from, to) in [
        ("OnGetDataAtServer", "OnChange"),
        ("BeforeRowChange", "OnChange"),
        ("OnGetDataAtServer", "UnknownEvent"),
        ("form:DynamicListTableExtInfo", "form:UnknownExtInfo"),
    ] {
        assert!(read_form(FormDialect::Edt, bytes.replace(from, to).as_bytes()).is_err());
    }
    let native = String::from_utf8(write_form(FormDialect::Designer, &body).unwrap()).unwrap();
    for (from, to) in [("OnGetDataAtServer", "OnChange"), ("BeforeRowChange", "")] {
        assert!(read_form(FormDialect::Designer, native.replace(from, to).as_bytes()).is_err());
    }
}

fn lab() -> std::path::PathBuf {
    std::env::var_os("IBCMD_EDT_LAB").unwrap().into()
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn table_event_containers(bytes: &[u8]) -> Vec<&[u8]> {
    use quick_xml::events::Event;
    let mut reader = quick_xml::Reader::from_reader(bytes);
    let mut stack = Vec::new();
    let mut start = None;
    let mut out = Vec::new();
    loop {
        let before = reader.buffer_position() as usize;
        match reader.read_event().unwrap() {
            Event::Start(element) => {
                let name = element.name().as_ref().to_vec();
                if name == b"Events" && stack.last().is_some_and(|parent| parent == b"Table") {
                    start = Some((before, stack.len()));
                }
                stack.push(name);
            }
            Event::End(_) => {
                stack.pop().unwrap();
                if let Some((begin, depth)) = start
                    && depth == stack.len()
                {
                    out.push(&bytes[begin..reader.buffer_position() as usize]);
                    start = None;
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    out
}

#[test]
#[ignore = "requires installed SDK jar/type hash-bound tables"]
fn installed_table_owner_models_and_physical_guids_agree() {
    let rows: serde_json::Value = serde_json::from_slice(
        &std::fs::read(lab().join("sdk-table-event-owner-features-r1.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 4);
    for row in rows.as_array().unwrap() {
        assert_eq!(
            hash(&std::fs::read(row["jar"].as_str().unwrap()).unwrap()),
            row["jar_sha256"]
        );
        let bytes = std::fs::read(row["extracted_path"].as_str().unwrap()).unwrap();
        assert_eq!(hash(&bytes), row["sha256"]);
        let doc = formats_xml::parse(&bytes).unwrap();
        let events: Vec<_> = doc
            .root
            .children
            .iter()
            .filter(|child| child.local == "events")
            .collect();
        assert_eq!(events.len(), row["events"].as_array().unwrap().len());
        let extension = row["entry"]
            .as_str()
            .unwrap()
            .contains("ExtensionForDynamicList");
        let mut body = fixture();
        body.items[0].events.clear();
        body.items[0]
            .dynamic_list_ext
            .as_mut()
            .unwrap()
            .events
            .clear();
        for (parsed, captured) in events.iter().zip(row["events"].as_array().unwrap()) {
            let name = parsed.attr("name").unwrap().value.as_str();
            let guid = parsed.attr("uuid").unwrap().value.as_str();
            assert_eq!(name, captured["name"]);
            assert_eq!(guid, captured["uuid"]);
            assert_eq!(morph1c_core::ir::table_event_guid(name), Some(guid));
            if extension {
                body.items[0]
                    .dynamic_list_ext
                    .as_mut()
                    .unwrap()
                    .events
                    .push(event(name));
            } else {
                body.items[0].events.push(event(name));
            }
        }
        let edt = write_form(FormDialect::Edt, &body).unwrap();
        assert_eq!(
            digest(&body),
            digest(&read_form(FormDialect::Edt, &edt).unwrap())
        );
    }
}

fn table_owners(items: &[FormItem], out: &mut Vec<(String, Vec<FormEvent>, Vec<FormEvent>)>) {
    for item in items {
        if item.kind.as_str() == "Table" {
            out.push((
                item.name.clone(),
                item.events.clone(),
                item.dynamic_list_ext
                    .as_ref()
                    .map(|x| x.events.clone())
                    .unwrap_or_default(),
            ));
        }
        table_owners(&item.children, out);
        table_owners(&item.additions, out);
        if let Some(table) = &item.auto_table {
            table_owners(std::slice::from_ref(table.as_ref()), out);
        }
    }
}

#[test]
#[ignore = "requires five complete genuine native/authentic EDT/repeated SDK witnesses"]
fn genuine_five_forms_preserve_native_bytes_and_cross_owner_handlers() {
    let rows: serde_json::Value = serde_json::from_slice(
        &std::fs::read(lab().join("bsp83-table-event-owner-witness-r1.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 5);
    let version = FormatVersion::new(2, 20);
    let mut extension_events = 0;
    for row in rows.as_array().unwrap() {
        let bytes: Vec<_> = row["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|source| {
                let bytes = std::fs::read(source["path"].as_str().unwrap()).unwrap();
                assert_eq!(hash(&bytes), source["sha256"]);
                bytes
            })
            .collect();
        let native = with_source_version(Some(version), || {
            read_form(FormDialect::Designer, &bytes[0])
        })
        .unwrap();
        let edt =
            with_source_version(Some(version), || read_form(FormDialect::Edt, &bytes[1])).unwrap();
        let sdk = with_source_version(Some(version), || {
            read_form(FormDialect::Designer, &bytes[2])
        })
        .unwrap();
        let native_again =
            with_roundtrip_target(version, || write_form(FormDialect::Designer, &native)).unwrap();
        assert!(
            native_again == bytes[0],
            "native {} {} -> {}",
            row["relative"],
            hash(&bytes[0]),
            hash(&native_again)
        );
        let sdk_again =
            with_roundtrip_target(version, || write_form(FormDialect::Designer, &sdk)).unwrap();
        // SDK root CommandSet physical ordering differs from the platform writer;
        // this witness asserts exact Table Events containers, not whole SDK bytes.
        let before = table_event_containers(&bytes[2]);
        let after = table_event_containers(&sdk_again);
        assert!(before == after, "SDK Table Events {}", row["relative"]);
        let projected =
            with_roundtrip_target(version, || write_form(FormDialect::Edt, &native)).unwrap();
        let returned =
            with_source_version(Some(version), || read_form(FormDialect::Edt, &projected)).unwrap();
        let mut native_owners = Vec::new();
        let mut edt_owners = Vec::new();
        let mut sdk_owners = Vec::new();
        let mut returned_owners = Vec::new();
        table_owners(&native.items, &mut native_owners);
        table_owners(&edt.items, &mut edt_owners);
        table_owners(&sdk.items, &mut sdk_owners);
        table_owners(&returned.items, &mut returned_owners);
        assert_eq!(native_owners, edt_owners, "{}", row["relative"]);
        assert_eq!(native_owners, sdk_owners, "{}", row["relative"]);
        assert_eq!(native_owners, returned_owners, "{}", row["relative"]);
        extension_events += native_owners
            .iter()
            .map(|(_, _, ext)| ext.len())
            .sum::<usize>();
    }
    assert_eq!(extension_events, 6);
}
