use formats_xml::form::{FormDialect, read_form, write_form};
use morph1c_core::{
    ir::{FormBody, FormEvent, FormRootExtInfo},
    version::{FormatVersion, with_roundtrip_target, with_source_version},
};
use sha2::{Digest, Sha256};

const GUID: &str = "9cc34712-da5f-4faa-a653-343d2085fbe8";
fn body() -> FormBody {
    let mut body = FormBody::new();
    body.events = vec![
        FormEvent {
            name: "OnOpen".into(),
            handler: "OpenHandler".into(),
        },
        FormEvent {
            name: GUID.into(),
            handler: "GuidHandler".into(),
        },
    ];
    body
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
fn digest(body: &FormBody) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(body).unwrap()))
}

#[test]
fn symbolic_root_guid_is_current_ordered_identity_not_a_named_extension_event() {
    for minor in [20, 21] {
        let seed = body();
        for dialect in [FormDialect::Edt, FormDialect::Designer] {
            let bytes = write(&seed, dialect, minor);
            let mut current = read(&bytes, dialect, minor);
            assert!(current.root_ext_info.is_none());
            assert_eq!(current.events, seed.events);
            assert!(write(&current, dialect, minor) == bytes);
            let original = digest(&current);
            current.events[1].handler = "CurrentHandler".into();
            assert_ne!(digest(&current), original);
            let returned = read(&write(&current, dialect, minor), dialect, minor);
            assert_eq!(returned.events, current.events);
            current.events.reverse();
            assert_eq!(
                read(&write(&current, dialect, minor), dialect, minor).events,
                current.events
            );
            current.events[0].name = GUID.to_ascii_uppercase();
            assert_eq!(
                read(&write(&current, dialect, minor), dialect, minor).events,
                current.events
            );
            current.events.remove(0);
            assert_eq!(
                read(&write(&current, dialect, minor), dialect, minor).events,
                current.events
            );
        }
    }
}

#[test]
fn unknown_text_malformed_guid_duplicate_and_wrong_owner_fail_closed() {
    for name in [
        "UnknownEvent",
        "BeforeWrite",
        "9cc34712-da5f-4faa-a653-343d2085fbe",
        "9cc34712xda5f-4faa-a653-343d2085fbe8",
        "9cc34712-da5f-4faa-a653-343d2085fbeg",
        "{9cc34712-da5f-4faa-a653-343d2085fbe8}",
    ] {
        let mut value = body();
        value.events[1].name = name.into();
        for dialect in [FormDialect::Edt, FormDialect::Designer] {
            assert!(
                with_roundtrip_target(FormatVersion::new(2, 20), || write_form(dialect, &value))
                    .is_err()
            );
            let valid = write(&body(), dialect, 20);
            let bad = String::from_utf8(valid).unwrap().replace(GUID, name);
            assert!(
                with_source_version(Some(FormatVersion::new(2, 20)), || read_form(
                    dialect,
                    bad.as_bytes()
                ))
                .is_err()
            );
        }
    }
    let mut duplicate = body();
    duplicate.events.push(duplicate.events[1].clone());
    assert!(
        with_roundtrip_target(FormatVersion::new(2, 20), || write_form(
            FormDialect::Edt,
            &duplicate
        ))
        .is_err()
    );
    let mut misplaced = body();
    let event = misplaced.events.pop().unwrap();
    misplaced.root_ext_info = Some(FormRootExtInfo {
        kind: "form:CatalogFormExtInfo".into(),
        events: vec![event],
        user_settings_group: None,
    });
    for dialect in [FormDialect::Edt, FormDialect::Designer] {
        assert!(
            with_roundtrip_target(FormatVersion::new(2, 20), || write_form(
                dialect, &misplaced
            ))
            .is_err()
        );
    }
}

#[test]
#[ignore = "Hash-bound genuine UH83/UH85 symbolic Event reference witnesses on F"]
fn genuine_both_runtime_root_guids_keep_exact_source_and_current_handlers() {
    let lab = std::path::PathBuf::from(std::env::var_os("IBCMD_EDT_LAB").unwrap());
    let witness: serde_json::Value = serde_json::from_slice(
        &std::fs::read(lab.join("uha-guid-root-event-primary-r2/result.json")).unwrap(),
    )
    .unwrap();
    for row in witness["sources"].as_array().unwrap() {
        let bytes = std::fs::read(row["path"].as_str().unwrap()).unwrap();
        assert_eq!(
            format!("{:x}", Sha256::digest(&bytes)),
            row["sha256"].as_str().unwrap()
        );
        let minor = row["minor"].as_u64().unwrap_or(20) as u16;
        let dialect = if row["label"].as_str().unwrap().ends_with("edt") {
            FormDialect::Edt
        } else {
            FormDialect::Designer
        };
        let mut value = read(&bytes, dialect, minor);
        assert!(value.root_ext_info.is_none());
        assert_eq!(value.events.len(), row["events"].as_array().unwrap().len());
        for (actual, expected) in value.events.iter().zip(row["events"].as_array().unwrap()) {
            assert_eq!(actual.name, expected["name"].as_str().unwrap());
            assert_eq!(
                format!("{:x}", Sha256::digest(actual.handler.as_bytes())),
                expected["handler_sha256"].as_str().unwrap()
            );
        }
        assert!(
            write(&value, dialect, minor) == bytes,
            "genuine same-source form bytes changed"
        );
        let index = value
            .events
            .iter()
            .position(|event| event.name == GUID)
            .unwrap();
        value.events[index].handler = "CurrentSyntheticHandler".into();
        // Whole same-source bytes were checked above. Cross the witnessed
        // event scope independently; pictures require the separate full-config
        // semantic resource attachment and must not be stripped or waived here.
        let mut event_scope = FormBody::new();
        event_scope.events = value.events.clone();
        for target in [FormDialect::Edt, FormDialect::Designer] {
            let returned = read(&write(&event_scope, target, minor), target, minor);
            assert_eq!(returned.events, event_scope.events);
            assert!(returned.root_ext_info.is_none());
        }
    }
}

#[test]
fn public_exact_return_stripped_guid_and_rehashed_handler_edit_use_current_semantics() {
    use ibcmd_edt::{
        ConversionOptions, Project, ReaderLimits, edt_to_xml, read_xml_source, xml_to_edt,
    };
    use ibcmd_xml::source_tree::{SourceEntry, SourceTree};
    use morph1c_core::ir::{MetadataObject, NamedFormBody, ObjectKind, PropertyValue, Token, Uuid};
    use morph1c_pipeline::{ConvertOptions, Format, read_config, write_config};
    let fixture = std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/subsystem-ci/src"
    ));
    let mut configuration = read_config(Format::Designer, fixture, &ConvertOptions::default())
        .unwrap()
        .0;
    let mut object = MetadataObject::new(ObjectKind::new("CommonForm"), "GuidRoot", Uuid([44; 16]));
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
    object.form_bodies.push(NamedFormBody {
        name: "GuidRoot".into(),
        body: body(),
        ordinary_body: None,
        module: None,
        help: vec![],
        help_resources: vec![],
    });
    configuration.objects.push(object);
    let directory = tempfile::tempdir().unwrap();
    with_roundtrip_target(FormatVersion::new(2, 20), || {
        write_config(Format::Designer, &configuration, directory.path())
    })
    .unwrap();
    let path = directory.path().join("Configuration.xml");
    let root = String::from_utf8(std::fs::read(&path).unwrap())
        .unwrap()
        .replace(
            "</Language>",
            "</Language>\r\n\t\t\t<CommonForm>GuidRoot</CommonForm>",
        );
    std::fs::write(path, root).unwrap();
    let options = ConversionOptions {
        edt_version: "2025.2.3".into(),
        xml_dialect: "2.20".into(),
        runtime_version: Some("8.3.27".into()),
    };
    let original = read_xml_source(directory.path(), ReaderLimits::default()).unwrap();
    let generated = xml_to_edt(&original, &options).unwrap().tree;
    assert!(
        edt_to_xml(&Project::from_tree(generated.clone()).unwrap(), &options)
            .unwrap()
            .tree
            == original
    );
    let form_path = "src/CommonForms/GuidRoot/Form.form";
    let entry = generated
        .entries()
        .iter()
        .find(|entry| entry.path().as_str() == form_path)
        .unwrap();
    let mut current = read(entry.bytes(), FormDialect::Edt, 20);
    current.events[1].handler = "CurrentHandler".into();
    let edited = write(&current, FormDialect::Edt, 20);
    let mut manifest: serde_json::Value = serde_json::from_slice(
        generated
            .entries()
            .iter()
            .find(|entry| entry.path().as_str() == ".ibcmd-provenance/manifest.json")
            .unwrap()
            .bytes(),
    )
    .unwrap();
    manifest["generated"][form_path] = serde_json::json!(format!("{:x}", Sha256::digest(&edited)));
    let alter = |strip: bool| {
        SourceTree::new(
            generated
                .entries()
                .iter()
                .filter(|entry| !strip || !entry.path().as_str().starts_with(".ibcmd-provenance/"))
                .map(|entry| {
                    let bytes = if entry.path().as_str() == form_path {
                        edited.clone()
                    } else if entry.path().as_str() == ".ibcmd-provenance/manifest.json" {
                        serde_json::to_vec_pretty(&manifest).unwrap()
                    } else {
                        entry.bytes().to_vec()
                    };
                    SourceEntry::from_bytes(entry.path().clone(), bytes).unwrap()
                })
                .collect(),
        )
        .unwrap()
    };
    assert!(edt_to_xml(&Project::from_tree(alter(false)).unwrap(), &options).is_err());
    let returned = edt_to_xml(&Project::from_tree(alter(true)).unwrap(), &options)
        .unwrap()
        .tree;
    let native = returned
        .entries()
        .iter()
        .find(|entry| entry.path().as_str() == "CommonForms/GuidRoot/Ext/Form.xml")
        .unwrap();
    let decoded = read(native.bytes(), FormDialect::Designer, 20);
    assert_eq!(decoded.events, current.events);
    assert!(decoded.root_ext_info.is_none());
}

#[test]
#[ignore = "Read-only complete observed 11-form root-GUID cohort per UH runtime on F"]
fn genuine_all_observed_guid_forms_bind_native_edt_and_sdk_sources() {
    use std::{collections::BTreeSet, path::PathBuf};
    let lab = PathBuf::from(std::env::var_os("IBCMD_EDT_LAB").unwrap());
    let census_path = lab.join("GUID-event-census-r1.json");
    let census_bytes = std::fs::read(&census_path).unwrap();
    let census: serde_json::Value = serde_json::from_slice(&census_bytes).unwrap();
    let mut rows = Vec::new();
    let mut failures = 0;
    for version in census["versions"].as_array().unwrap() {
        let modern = version["version"] == "UH85";
        let minor = if modern { 21 } else { 20 };
        let edt = lab.join(if modern {
            "oracle-uha85-r2/authentic-workspace/OracleConfiguration"
        } else {
            "oracle-uha83-r1/authentic-workspace/OracleConfiguration"
        });
        let native = if modern {
            PathBuf::from(r"F:\ibcmd\lab\v85\native\uha_20260923\native")
        } else {
            PathBuf::from(r"F:\ibcmd\lab\04\release-20261001\rc\out\uha8327_db_r1\tree")
        };
        let sdk = lab.join(if modern {
            "native-reference-uha85-affinity-r1/native-xml"
        } else {
            "native-reference-uha83-r1/native-xml"
        });
        let anomalies = version["anomalies"].as_array().unwrap();
        assert_eq!(anomalies.len(), 11);
        let mut guids = BTreeSet::new();
        let mut references = 0;
        for anomaly in anomalies {
            let path = anomaly["path"].as_str().unwrap();
            let owner = path
                .strip_prefix("src/")
                .unwrap()
                .strip_suffix("/Form.form")
                .unwrap();
            for event in anomaly["events"].as_array().unwrap() {
                guids.insert(event["guid"].as_str().unwrap().to_owned());
                references += 1;
            }
            let mut events = None;
            for (label, dialect, source) in [
                ("edt", FormDialect::Edt, edt.join(path)),
                (
                    "native",
                    FormDialect::Designer,
                    native.join(owner).join("Ext/Form.xml"),
                ),
                (
                    "sdk",
                    FormDialect::Designer,
                    sdk.join(owner).join("Ext/Form.xml"),
                ),
            ] {
                let bytes = std::fs::read(&source).unwrap();
                let source_sha = format!("{:x}", Sha256::digest(&bytes));
                if label == "edt" {
                    assert_eq!(source_sha, anomaly["sha256"].as_str().unwrap());
                }
                let mut row = serde_json::json!({"version":version["version"],"label":label,"source":source.to_string_lossy(),"bytes":bytes.len(),"sha256":source_sha});
                let result = (|| -> Result<(), String> {
                    let value = with_source_version(Some(FormatVersion::new(2, minor)), || {
                        read_form(dialect, &bytes)
                    })
                    .map_err(|error| format!("read: {error}"))?;
                    let generated = with_roundtrip_target(FormatVersion::new(2, minor), || {
                        write_form(dialect, &value)
                    })
                    .map_err(|error| format!("write: {error}"))?;
                    row["generated_sha256"] =
                        serde_json::json!(format!("{:x}", Sha256::digest(&generated)));
                    row["raw_exact"] = serde_json::json!(generated == bytes);
                    let returned = with_source_version(Some(FormatVersion::new(2, minor)), || {
                        read_form(dialect, &generated)
                    })
                    .map_err(|error| format!("read generated: {error}"))?;
                    row["current_semantics_equal"] =
                        serde_json::json!(digest(&returned) == digest(&value));
                    if digest(&returned) != digest(&value) {
                        return Err("same-source current typed form semantics differ".into());
                    }
                    if generated != bytes {
                        if let Some(report) = std::env::var_os("IBCMD_GUID_EVENT_REPORT") {
                            let directory =
                                std::path::PathBuf::from(report).with_extension("witnesses");
                            std::fs::create_dir_all(&directory).unwrap();
                            let original_path =
                                directory.join(format!("{}-source.xml", rows.len()));
                            let generated_path =
                                directory.join(format!("{}-generated.xml", rows.len()));
                            std::fs::write(&original_path, &bytes).unwrap();
                            std::fs::write(&generated_path, &generated).unwrap();
                            row["raw_source_witness"] = serde_json::json!(original_path);
                            row["raw_generated_witness"] = serde_json::json!(generated_path);
                        }
                        if dialect == FormDialect::Designer {
                            return Err("native same-source entire form bytes differ".into());
                        }
                    }
                    if let Some(expected) = &events {
                        if expected != &value.events {
                            return Err("three-source ordered root event bindings differ".into());
                        }
                    } else {
                        events = Some(value.events.clone());
                    }
                    if label == "edt" {
                        for event in anomaly["events"].as_array().unwrap() {
                            assert_eq!(event["scope"], "root");
                            let guid = event["guid"].as_str().unwrap();
                            let ordinal = event["ordinal"].as_u64().unwrap() as usize;
                            if value
                                .events
                                .get(ordinal)
                                .is_none_or(|event| event.name != guid)
                            {
                                return Err("observed GUID identity/order changed".into());
                            }
                        }
                    }
                    Ok(())
                })();
                match result {
                    Ok(()) => row["status"] = serde_json::json!("SCOPED_PASS"),
                    Err(error) => {
                        failures += 1;
                        row["status"] = serde_json::json!("FAIL_NOT_ACCEPTED");
                        row["error"] = serde_json::json!(error);
                    }
                }
                let after = std::fs::read(&source).unwrap();
                row["source_unchanged"] = serde_json::json!(after == bytes);
                assert!(
                    after == bytes,
                    "source changed during read-only cohort check"
                );
                rows.push(row);
            }
        }
        assert_eq!(references, 25);
        assert_eq!(guids.len(), 8);
    }
    assert_eq!(rows.len(), 66);
    if let Some(report) = std::env::var_os("IBCMD_GUID_EVENT_REPORT") {
        let runtime = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("vendor/morph1c/crates/formats-xml/src/form/event_owners.rs");
        let test = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/sdk_guid_root_events.rs");
        std::fs::write(report,serde_json::to_vec_pretty(&serde_json::json!({"scope":"66 current same-source typed form regenerations with native byte equality and retained EDT raw differences; 22 three-source ordered GUID cohort event comparisons; no cross-body/whole-configuration acceptance claim", "census_source":census_path,"census_sha256":format!("{:x}",Sha256::digest(&census_bytes)),"event_owner_source_sha256":format!("{:x}",Sha256::digest(std::fs::read(runtime).unwrap())),"test_source_sha256":format!("{:x}",Sha256::digest(std::fs::read(test).unwrap())),"failures":failures,"rows":rows})).unwrap()).unwrap();
    }
    assert_eq!(failures, 0, "all cohort failures preserved in F report");
}
