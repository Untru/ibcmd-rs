use formats_xml::form::{FormDialect, read_form, write_form};
use morph1c_core::ir::form::NativeFormEventOrder;
use morph1c_core::ir::{FormBody, FormEvent, FormRootExtInfo};
use sha2::{Digest, Sha256};

fn event(name: &str) -> FormEvent {
    FormEvent {
        name: name.into(),
        handler: format!("Handle{name}"),
    }
}
fn fixture() -> FormBody {
    let mut body = FormBody::new();
    body.events = vec![event("OnOpen"), event("OnCreateAtServer")];
    body.root_ext_info = Some(FormRootExtInfo {
        kind: "form:CatalogFormExtInfo".into(),
        events: vec![event("AfterWrite")],
        user_settings_group: None,
    });
    read_form(
        FormDialect::Edt,
        &write_form(FormDialect::Edt, &body).unwrap(),
    )
    .unwrap()
}
fn fingerprint(body: &FormBody) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(body).unwrap()))
}
fn native_names(body: &FormBody) -> Vec<String> {
    let bytes = write_form(FormDialect::Designer, body).unwrap();
    let document = formats_xml::parse(&bytes).unwrap();
    document
        .root
        .child("Events")
        .unwrap()
        .children
        .iter()
        .map(|element| element.attr("name").unwrap().value.clone())
        .collect()
}

#[test]
fn owners_and_handler_values_are_ordered_semantic_data() {
    let body = fixture();
    let baseline = fingerprint(&body);
    let mut changed = body.clone();
    changed.root_ext_info.as_mut().unwrap().events[0].handler = "ChangedHandler".into();
    assert_ne!(fingerprint(&changed), baseline);
    let returned = read_form(
        FormDialect::Edt,
        &write_form(FormDialect::Edt, &changed).unwrap(),
    )
    .unwrap();
    assert_eq!(fingerprint(&returned), fingerprint(&changed));
    changed = body.clone();
    changed.events.reverse();
    assert_ne!(fingerprint(&changed), baseline);
    let returned = read_form(
        FormDialect::Edt,
        &write_form(FormDialect::Edt, &changed).unwrap(),
    )
    .unwrap();
    assert_eq!(fingerprint(&returned), fingerprint(&changed));
    changed = body.clone();
    let misplaced = changed.root_ext_info.as_mut().unwrap().events.remove(0);
    changed.events.push(misplaced);
    assert_ne!(fingerprint(&changed), baseline);
    assert!(write_form(FormDialect::Edt, &changed).is_err());
}

#[test]
fn source_order_facet_replays_names_only_and_edits_invalidate_replay() {
    let mut body = fixture();
    let baseline = fingerprint(&body);
    body.native_event_order = Some(NativeFormEventOrder {
        extension_kind: Some("form:CatalogFormExtInfo".into()),
        merged: vec![
            "OnOpen".into(),
            "AfterWrite".into(),
            "OnCreateAtServer".into(),
        ],
        root: vec!["OnOpen".into(), "OnCreateAtServer".into()],
        extension: vec!["AfterWrite".into()],
    });
    assert_eq!(fingerprint(&body), baseline);
    assert_eq!(
        native_names(&body),
        ["OnOpen", "AfterWrite", "OnCreateAtServer"]
    );
    body.root_ext_info.as_mut().unwrap().events[0].handler = "Edited".into();
    let bytes = write_form(FormDialect::Designer, &body).unwrap();
    assert!(
        String::from_utf8(bytes)
            .unwrap()
            .contains(">Edited</Event>")
    );
    body.events.reverse();
    assert_ne!(
        native_names(&body),
        ["OnOpen", "AfterWrite", "OnCreateAtServer"]
    );
    body.events[0].name = "OnClose".into();
    assert!(native_names(&body).contains(&"OnClose".into()));
    assert!(!native_names(&body).contains(&"OnCreateAtServer".into()));
    let mut changed_owner = fixture();
    changed_owner.native_event_order = body.native_event_order.clone();
    changed_owner.root_ext_info.as_mut().unwrap().kind = "form:DocumentFormExtInfo".into();
    assert_ne!(
        native_names(&changed_owner),
        ["OnOpen", "AfterWrite", "OnCreateAtServer"]
    );
}

#[test]
fn duplicate_empty_unknown_extension_and_wrong_owner_bindings_fail_closed() {
    let body = fixture();
    let bytes = String::from_utf8(write_form(FormDialect::Edt, &body).unwrap()).unwrap();
    for (from, to) in [
        ("OnOpen", ""),
        ("AfterWrite", "UnknownEvent"),
        ("AfterWrite", "OnOpen"),
        ("OnCreateAtServer", "OnOpen"),
    ] {
        assert!(read_form(FormDialect::Edt, bytes.replace(from, to).as_bytes()).is_err());
    }
    let mut changed = body;
    changed.events.push(event("OnOpen"));
    assert!(write_form(FormDialect::Designer, &changed).is_err());
}

#[test]
fn known_runtime_events_have_physical_guids_without_changing_owned_arrays() {
    let mut body = fixture();
    body.events.insert(0, event("OnPasteFromClipboard"));
    let before = fingerprint(&body);
    let names = native_names(&body);
    assert_eq!(
        names,
        [
            "AfterWrite",
            "OnPasteFromClipboard",
            "OnOpen",
            "OnCreateAtServer"
        ]
    );
    assert_eq!(fingerprint(&body), before);
    for (name, guid) in [
        ("ValueChoice", "0bf5cb1e-85d7-4344-8e8e-e8e131006339"),
        ("BeforeStart", "36205ca4-af87-4708-b594-00ffb647b887"),
        ("BeforeExecute", "ea0a9886-1607-44fe-a446-2cc57548f57d"),
        ("AfterComposeResult", "b634e40b-c7cd-471b-9d2d-03406a7ee2b2"),
        ("OnComposeResult", "acb39a89-2fa6-4a11-a764-5597b67f5fff"),
        ("OnSettingsChange", "58a9c022-69bc-495e-aab1-32be2210fb79"),
    ] {
        assert_eq!(morph1c_core::ir::form_event_guid(name, false), Some(guid));
    }
    assert_eq!(
        morph1c_core::ir::form_event_guid("UnknownEvent", false),
        None
    );
}

#[test]
#[ignore = "requires installed SDK jars plus extracted/hash-bound runtime .type witnesses"]
fn installed_sdk_tables_agree() {
    let lab = std::path::PathBuf::from(std::env::var_os("IBCMD_EDT_LAB").unwrap());
    let evidence: serde_json::Value = serde_json::from_slice(
        &std::fs::read(lab.join("bsp83-authentic-events-repeat-r1/sdk-event-owner-features.json"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(evidence["jars"].as_array().unwrap().len(), 2);
    for jar in evidence["jars"].as_array().unwrap() {
        let bytes = std::fs::read(jar["path"].as_str().unwrap()).unwrap();
        assert_eq!(
            format!("{:x}", Sha256::digest(bytes)),
            jar["sha256"].as_str().unwrap()
        );
    }
    let rows = evidence["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 32);
    let mut events = 0;
    for row in rows {
        let bytes = std::fs::read(row["extracted_path"].as_str().unwrap()).unwrap();
        assert_eq!(
            format!("{:x}", Sha256::digest(&bytes)),
            row["sha256"].as_str().unwrap()
        );
        let doc = formats_xml::parse(&bytes).unwrap();
        let actual = doc
            .root
            .children
            .iter()
            .filter(|child| child.local == "events")
            .collect::<Vec<_>>();
        let captured = row["events"].as_array().unwrap();
        assert_eq!(actual.len(), captured.len());
        for (actual, captured) in actual.iter().zip(captured) {
            let name = actual.attr("name").unwrap().value.as_str();
            let guid = actual.attr("uuid").unwrap().value.as_str();
            assert_eq!(name, captured["name"].as_str().unwrap());
            assert_eq!(guid, captured["uuid"].as_str().unwrap());
            // SDK excludes same-named base events from extension owners. Task's
            // ActivationProcessing therefore uses the root Form event identity.
            if row["owner"] == "TASK_FORM_EXT_INFO" && name == "ActivationProcessing" {
                continue;
            }
            assert_eq!(
                morph1c_core::ir::form_event_guid(name, false),
                Some(guid),
                "{} {name}",
                row["owner"]
            );
            events += 1;
        }
    }
    assert!(events > 200);
}

#[test]
#[ignore = "requires hash-bound genuine UH accounting record-set/native/EDT/SDK witnesses"]
fn accounting_record_set_has_real_sdk_owner_and_cross_semantics() {
    use formats_xml::form::{bind_picture_semantics, resolve_common_picture_transparency};
    use morph1c_core::ir::{MetadataObject, ObjectKind};
    use morph1c_core::version::{FormatVersion, with_roundtrip_target, with_source_version};
    use morph1c_pipeline::{Format, attach_form_body, write_form_bodies};
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    let lab = PathBuf::from(std::env::var_os("IBCMD_EDT_LAB").unwrap());
    let witness: serde_json::Value = serde_json::from_slice(
        &std::fs::read(lab.join("uha-accounting-record-set-root-witness-r1.json")).unwrap(),
    )
    .unwrap();
    let mut bytes = Vec::new();
    for row in witness["rows"].as_array().unwrap() {
        let content = std::fs::read(row["path"].as_str().unwrap()).unwrap();
        assert_eq!(
            format!("{:x}", Sha256::digest(&content)),
            row["sha256"].as_str().unwrap()
        );
        bytes.push(content);
    }
    assert_eq!(bytes.len(), 3);
    assert_eq!(bytes[0], bytes[2], "native and fresh SDK entire form bytes");
    let v = FormatVersion::new(2, 20);
    let native_body =
        with_source_version(Some(v), || read_form(FormDialect::Designer, &bytes[0])).unwrap();
    let authentic =
        with_source_version(Some(v), || read_form(FormDialect::Edt, &bytes[1])).unwrap();
    assert_eq!(
        native_body.root_ext_info.as_ref().unwrap().kind,
        "form:RecordSetFormExtInfo"
    );
    assert_eq!(native_body.events.len(), 5);
    assert_eq!(native_body.root_ext_info.as_ref().unwrap().events.len(), 6);
    assert_eq!(native_body.events, authentic.events);
    assert_eq!(native_body.root_ext_info, authentic.root_ext_info);
    assert_eq!(
        with_roundtrip_target(v, || write_form(FormDialect::Designer, &native_body)).unwrap(),
        bytes[0]
    );
    let path = PathBuf::from(witness["rows"][0]["path"].as_str().unwrap());
    let dir = path.parent().unwrap().parent().unwrap();
    let name = dir.file_name().unwrap().to_str().unwrap();
    let anchor = dir.with_extension("xml");
    let descriptor = formats_xml::parse(&std::fs::read(&anchor).unwrap()).unwrap();
    let uuid = formats_xml::children::parse_uuid(
        &descriptor
            .root
            .child("Form")
            .unwrap()
            .attr("uuid")
            .unwrap()
            .value,
    )
    .unwrap();
    // The body lane uses a CommonForm carrier with the actual declared Form UUID;
    // this test does not claim parent-metadata conversion acceptance.
    let mut source = MetadataObject::new(ObjectKind::new("CommonForm"), name, uuid);
    with_source_version(Some(v), || {
        attach_form_body(Format::Designer, "CommonForm", &anchor, &mut source)
    })
    .unwrap();
    let edt_root = lab.join("oracle-uha83-r1/authentic-workspace/OracleConfiguration/src");
    let mut pictures = BTreeMap::new();
    for item in std::fs::read_dir(edt_root.join("CommonPictures")).unwrap() {
        let item = item.unwrap();
        let name = item.file_name();
        let data = std::fs::read(item.path().join(&name).with_extension("mdo")).unwrap();
        let parsed = formats_xml::parse(&data).unwrap();
        assert!(
            pictures
                .insert(
                    format!("CommonPicture.{}", name.to_str().unwrap()),
                    parsed.root.child("transparentPixel").is_some()
                )
                .is_none()
        );
    }
    resolve_common_picture_transparency(&mut source.form_bodies[0].body, &pictures, false).unwrap();
    bind_picture_semantics(&mut source.form_bodies[0].body, uuid, false).unwrap();
    let scratch = tempfile::tempdir_in(&lab).unwrap();
    let generated_anchor = scratch.path().join(name).join(format!("{name}.mdo"));
    with_roundtrip_target(v, || {
        write_form_bodies(Format::Edt, &generated_anchor, &source)
    })
    .unwrap();
    let mut returned = MetadataObject::new(ObjectKind::new("CommonForm"), name, uuid);
    with_source_version(Some(v), || {
        attach_form_body(Format::Edt, "CommonForm", &generated_anchor, &mut returned)
    })
    .unwrap();
    resolve_common_picture_transparency(&mut returned.form_bodies[0].body, &pictures, true)
        .unwrap();
    bind_picture_semantics(&mut returned.form_bodies[0].body, uuid, true).unwrap();
    assert_eq!(
        format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&source.form_bodies).unwrap())
        ),
        format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&returned.form_bodies).unwrap())
        ),
        "complete typed body, modules, sidecars and picture semantics"
    );
    let mut edited = native_body;
    edited.root_ext_info.as_mut().unwrap().kind =
        "form:AccountingRegisterRecordSetFormExtInfo".into();
    assert!(write_form(FormDialect::Edt, &edited).is_err());
}

#[test]
#[ignore = "requires genuine immutable BSP forms and installed SDK owner witnesses"]
fn all_bsp_native_owner_bindings_and_cross_fingerprints_agree() {
    use morph1c_core::version::{FormatVersion, with_roundtrip_target, with_source_version};
    use std::path::PathBuf;
    let lab = PathBuf::from(std::env::var_os("IBCMD_EDT_LAB").unwrap());
    let manifest: serde_json::Value = serde_json::from_slice(
        &std::fs::read(lab.join("oracle-bsp83-r1/native-before.json")).unwrap(),
    )
    .unwrap();
    let native = PathBuf::from(manifest["root"].as_str().unwrap());
    let edt = lab.join("oracle-bsp83-r1/authentic-workspace/OracleConfiguration/src");
    let mut count = 0;
    let mut rows = vec![];
    let mut failures = 0;
    for file in manifest["files"].as_array().unwrap() {
        let path = file["path"].as_str().unwrap();
        if !path.ends_with("/Ext/Form.xml") {
            continue;
        }
        count += 1;
        let original = std::fs::read(native.join(path)).unwrap();
        let authentic_path = edt
            .join(path.strip_suffix("/Ext/Form.xml").unwrap())
            .join("Form.form");
        let authentic = std::fs::read(&authentic_path).unwrap();
        let v = FormatVersion::new(2, 20);
        let mut row = serde_json::json!({"path":path,"native_sha256":format!("{:x}",Sha256::digest(&original)),"authentic_edt_sha256":format!("{:x}",Sha256::digest(&authentic))});
        let result = (|| -> Result<(), String> {
            if row["native_sha256"] != file["sha256"] {
                return Err("original manifest SHA mismatch".into());
            }
            let body = with_source_version(Some(v), || read_form(FormDialect::Designer, &original))
                .map_err(|e| format!("native read: {e}"))?;
            let actual = with_source_version(Some(v), || read_form(FormDialect::Edt, &authentic))
                .map_err(|e| format!("authentic EDT read: {e}"))?;
            if body.events != actual.events {
                return Err("ordered root owner bindings differ".into());
            }
            if body.root_ext_info.as_ref().map(|info| &info.events)
                != actual.root_ext_info.as_ref().map(|info| &info.events)
            {
                return Err("ordered extension owner bindings differ".into());
            }
            let regenerated = with_roundtrip_target(v, || write_form(FormDialect::Designer, &body))
                .map_err(|e| format!("native write: {e}"))?;
            if regenerated != original {
                return Err("native source bytes differ".into());
            }
            row["root_events"] = serde_json::json!(body.events.len());
            row["extension_events"] = serde_json::json!(
                body.root_ext_info
                    .as_ref()
                    .map_or(0, |info| info.events.len())
            );
            Ok(())
        })();
        match result {
            Ok(()) => row["result"] = serde_json::json!("PASS"),
            Err(error) => {
                failures += 1;
                row["result"] = serde_json::json!("FAIL");
                row["error"] = serde_json::json!(error);
            }
        }
        rows.push(row);
    }
    assert_eq!(count, 1108);
    if let Some(report) = std::env::var_os("IBCMD_EVENT_OWNER_REPORT") {
        std::fs::write(
            report,
            serde_json::to_vec_pretty(&serde_json::json!({"scope":"all BSP83 ordered event owner bindings against authentic EDT and native whole-body byte regeneration; full body/module/sidecar fingerprints use separate sdk_form_root_ext census","forms":count,"failures":failures,"rows":rows})).unwrap(),
        )
        .unwrap();
    }
    assert_eq!(failures, 0, "all failures retained in report");
}
