//! Cleanroom WebSocketClient layout, public CF routes and module ownership.
//! No SDK/foreign fixture bytes are copied into this suite.

use ibcmd_cf::payload::{PayloadEncoding, encode_payload};
use ibcmd_core::{
    artifact::ProfileId,
    diagnostic::{ObjectPath, PathSegment, PropertyPath},
    identity::{LogicalIdentity, ObjectUuid},
    limits::ResourceLimits,
    model::{CanonicalConfiguration, CanonicalObject, CanonicalObjectParts, MetadataKind},
    provenance::{CanonicalAnchor, SourceProvenance},
    validate::validate_configuration,
};
use ibcmd_rs::{
    compiler::{
        families::assets::{SourceAssetCodec, SourceAssetRegistry},
        graph::{ObjectStorageRoute, build_bootstrap_graph},
        identity::collect_bootstrap_identities,
        root::{ConfigurationBodyProperties, compile_configuration_body, compile_root},
        version::{SpecialEntryProfile, compile_version},
    },
    metadata_model::{
        DescriptorContext,
        brace::{Brace, parse_row, serialize_row},
        common::Header,
        compile_descriptor,
        export::{configuration_objects, object_names, write_document},
        websocket_client::WebSocketClient,
    },
    module_blob::{pack_module_blob_bytes_base_free, unpack_module_blob_text},
    profile_registry::load_bundled_profile_registry,
};
use ibcmd_schema::websocket_client::WebSocketClientLayout as Layout;
use ibcmd_v8::writer::{Format15Document, Format15Element, write_format15_to_vec};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

const CONFIG: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const CLIENT: &str = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
const MODULE: &[u8] = b"\xef\xbb\xbf// Public cleanroom module witness.\r\nProcedure SocketWitness() Export\r\nEndProcedure\r\n";

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static SEQUENCE: AtomicUsize = AtomicUsize::new(0);
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "ibcmd-websocket-{}-{nonce}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn client() -> WebSocketClient {
    WebSocketClient {
        header: Header {
            uuid: CLIENT.into(),
            name: "SocketWitness".into(),
            synonym: vec![("ru".into(), "Клиент \"связи\"".into())],
            comment: "cleanroom".into(),
        },
        predefined: false,
        auto_connect: true,
        server_url: "wss://example.invalid/socket".into(),
        user: "public-user".into(),
        password: "PUBLIC-NONSECRET".into(),
        headers: vec![
            ("X-A".into(), "A&B<typed>".into()),
            ("X-B".into(), "quoted \"value\"".into()),
        ],
        use_os_proxy: true,
        use_os_authentication: false,
        timeout: 47,
    }
}

fn recompile(xml: &str, version: &str) -> anyhow::Result<Brace> {
    let scratch = Scratch::new();
    let path = scratch.0.join("WebSocketClients/SocketWitness.xml");
    fs::create_dir(path.parent().unwrap())?;
    fs::write(&path, xml)?;
    let context = DescriptorContext::new(&scratch.0, version)?;
    parse_row(&compile_descriptor(
        Layout::KIND,
        &path,
        xml.as_bytes(),
        &context,
    )?)
}

#[test]
fn websocket_all_boolean_slots_and_authored_pairs_roundtrip_both_profiles() {
    for version in ["2.20", "2.21"] {
        for bits in 0..16 {
            let mut value = client();
            value.predefined = bits & 1 != 0;
            value.auto_connect = bits & 2 != 0;
            value.use_os_proxy = bits & 4 != 0;
            value.use_os_authentication = bits & 8 != 0;
            let row = value.to_brace();
            let fields = row.at(&[1]).unwrap().as_list().unwrap();
            for (index, flag) in [
                (2, value.predefined),
                (7, value.use_os_proxy),
                (8, value.use_os_authentication),
                (10, value.auto_connect),
            ] {
                assert_eq!(fields[index], Brace::flag(flag));
            }
            assert_eq!(fields[3], Brace::str("wss://example.invalid/socket"));
            assert_eq!(fields[4], Brace::str("public-user"));
            assert_eq!(fields[5], Brace::str("PUBLIC-NONSECRET"));
            assert_eq!(fields[9], Brace::num(47));
            assert_eq!(WebSocketClient::from_brace(&row).unwrap(), value);
            let xml = write_document(&value.to_xml().unwrap(), version);
            assert!(xml.contains("<xr:Item>"));
            assert!(xml.contains("xsi:type=\"v8:KeyAndValue\""));
            assert!(xml.contains("A&amp;B&lt;typed&gt;"));
            assert_eq!(recompile(&xml, version).unwrap(), row);
            assert_eq!(
                object_names(Layout::KIND, &row).unwrap().full_name,
                "WebSocketClient.SocketWitness"
            );
        }
    }
}

#[test]
fn websocket_empty_pairs_duplicate_keys_and_integer_range_are_not_capped() {
    for version in ["2.20", "2.21"] {
        for timeout in [i64::MIN, 0, 30, i64::MAX] {
            for headers in [
                Vec::new(),
                vec![
                    (String::new(), String::new()),
                    ("X".into(), "one".into()),
                    ("X".into(), "two".into()),
                ],
            ] {
                let mut value = client();
                value.headers = headers;
                value.timeout = timeout;
                let row = value.to_brace();
                let xml = write_document(&value.to_xml().unwrap(), version);
                assert_eq!(recompile(&xml, version).unwrap(), row);
            }
        }
    }
}

#[test]
fn websocket_malformed_native_shapes_counts_and_flags_are_refused() {
    let good = client().to_brace();
    for path in [
        vec![0],
        vec![2],
        vec![1, 0],
        vec![1, 2],
        vec![1, 7],
        vec![1, 8],
        vec![1, 10],
        vec![1, 6, 0],
    ] {
        let mut bad = good.clone();
        let mut node = &mut bad;
        for index in &path[..path.len() - 1] {
            node = &mut node.as_list_mut().unwrap()[*index];
        }
        node.as_list_mut().unwrap()[*path.last().unwrap()] = Brace::num(9);
        let error = WebSocketClient::from_brace(&bad).unwrap_err();
        assert!(format!("{error:#}").contains("WebSocketClient"));
    }
    let mut bad = good.clone();
    bad.as_list_mut().unwrap().push(Brace::num(0));
    assert!(WebSocketClient::from_brace(&bad).is_err());
    let mut bad = good.clone();
    bad.as_list_mut().unwrap()[1].as_list_mut().unwrap().pop();
    assert!(WebSocketClient::from_brace(&bad).is_err());
    let mut bad = good;
    bad.as_list_mut().unwrap()[1].as_list_mut().unwrap()[6]
        .as_list_mut()
        .unwrap()[1] = Brace::num(1);
    assert!(WebSocketClient::from_brace(&bad).is_err());
}

#[test]
fn websocket_unsupported_headers_are_refused_instead_of_dropped() {
    let xml = write_document(&client().to_xml().unwrap(), "2.20");
    for bad in [
        xml.replace(
            "<xr:CheckState>0</xr:CheckState>",
            "<xr:CheckState>1</xr:CheckState>",
        ),
        xml.replace("xsi:type=\"v8:KeyAndValue\"", "xsi:type=\"xs:string\""),
        xml.replace("xsi:type=\"xs:string\"", "xsi:type=\"xs:integer\""),
        xml.replace(
            "<xr:Presentation/>",
            "<xr:Presentation>lost text</xr:Presentation>",
        ),
        xml.replace(
            "</Properties>",
            "<UnknownProperty>must not disappear</UnknownProperty></Properties>",
        ),
    ] {
        assert!(recompile(&bad, "2.20").is_err());
    }
}

#[test]
fn websocket_headers_resolve_namespace_aliases_and_local_bindings_before_compilation() {
    for version in ["2.20", "2.21"] {
        let xml = write_document(&client().to_xml().unwrap(), version);
        let aliases = xml
            .replace("xr:", "r:")
            .replace("xmlns:xr=", "xmlns:r=")
            .replace("v8:", "d:")
            .replace("xmlns:v8=", "xmlns:d=")
            .replace("xs:", "s:")
            .replace("xmlns:xs=", "xmlns:s=")
            .replace("xsi:", "i:")
            .replace("xmlns:xsi=", "xmlns:i=");
        assert_eq!(recompile(&aliases, version).unwrap(), client().to_brace());
        let localized = xml
            .replace(
                "<xr:Item>",
                "<r:Item xmlns:r=\"http://v8.1c.ru/8.3/xcf/readable\">",
            )
            .replace("</xr:Item>", "</r:Item>");
        assert_eq!(recompile(&localized, version).unwrap(), client().to_brace());
        for bad in [
            xml.replace("http://v8.1c.ru/8.3/xcf/readable", "urn:wrong-readable"),
            xml.replace(
                "http://www.w3.org/2001/XMLSchema-instance",
                "urn:wrong-instance",
            ),
            xml.replace("<xr:Item>", "<xr:Item xmlns:xr=\"urn:wrong-local\">"),
            xml.replace("xsi:type=", "type="),
            xml.replace("<v8:Key ", "<xr:Key ")
                .replace("</v8:Key>", "</xr:Key>"),
            xml.replace("<Headers xsi:type=\"xr:ValueList\">", "<Headers>"),
        ] {
            assert!(
                recompile(&bad, version).is_err(),
                "namespace/type admission must refuse {bad}"
            );
        }
    }
}

fn archive(value: &WebSocketClient, with_module: bool) -> Vec<u8> {
    archive_with_row(value.to_brace(), with_module)
}

fn archive_with_row(row: Brace, with_module: bool) -> Vec<u8> {
    let id = ProfileId::parse("platform-8.3.27.1989").unwrap();
    let objects = [(CONFIG, "Configuration"), (CLIENT, Layout::KIND)].map(|(uuid, kind)| {
        let path = ObjectPath::new(vec![PathSegment::name(kind).unwrap()]).unwrap();
        CanonicalObject::new(CanonicalObjectParts::new(
            LogicalIdentity::new(ObjectUuid::parse(uuid).unwrap(), path.clone()),
            MetadataKind::new(kind).unwrap(),
            SourceProvenance::new(id.clone(), CanonicalAnchor::new(path, PropertyPath::root())),
        ))
        .unwrap()
    });
    let configuration = CanonicalConfiguration::new(objects.to_vec()).unwrap();
    let validated = validate_configuration(&configuration).unwrap();
    let identities = collect_bootstrap_identities(&validated).unwrap();
    let routes = identities
        .objects()
        .iter()
        .map(|object| ObjectStorageRoute::new(object.uuid(), vec![]).unwrap())
        .collect();
    let graph = build_bootstrap_graph(&identities, id.clone(), routes).unwrap();
    let profiles = load_bundled_profile_registry().unwrap();
    let profile = SpecialEntryProfile::from_effective(profiles.get(&id).unwrap()).unwrap();
    let properties =
        ConfigurationBodyProperties::minimal("SocketConfiguration", profile.compatibility());
    let mut elements = [
        compile_root(&graph, &profile).unwrap(),
        compile_version(&graph, &profile).unwrap(),
        compile_configuration_body(&identities, &graph, &profile, &properties).unwrap(),
    ]
    .into_iter()
    .map(|entry| {
        Format15Element::named(
            entry.target().key().as_str(),
            Some(entry.outcome().compiled_payload().unwrap().bytes().to_vec()),
        )
    })
    .collect::<Vec<_>>();
    let bytes = serialize_row(&row);
    elements.push(Format15Element::named(
        CLIENT,
        Some(
            encode_payload(
                PayloadEncoding::RawDeflate,
                &bytes,
                ResourceLimits::default(),
            )
            .unwrap(),
        ),
    ));
    if with_module {
        elements.push(Format15Element::named(
            format!("{CLIENT}.0"),
            Some(pack_module_blob_bytes_base_free(MODULE, None).unwrap().blob),
        ));
    }
    write_format15_to_vec(&Format15Document::new(7, elements)).unwrap()
}

fn report(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["ok"], true, "{report}");
    report
}
fn export(cf: &Path, target: &Path, version: &str) -> Value {
    report(
        Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
            .args(["cf", "export"])
            .arg(cf)
            .arg(target)
            .args(["--source-version", version, "--fail-on-opaque"])
            .env("PATH", "")
            .output()
            .unwrap(),
    )
}

#[test]
fn websocket_public_base_free_cf_routes_keep_owner_headers_and_module_both_profiles() {
    let route = SourceAssetRegistry
        .route_by_suffix(Layout::KIND, ".0")
        .unwrap();
    assert_eq!(route.codec(), SourceAssetCodec::Module);
    assert_eq!(route.relative_path(), "Ext/Module.bsl");
    for version in ["2.20", "2.21"] {
        for with_module in [false, true] {
            let scratch = Scratch::new();
            let first = scratch.0.join("first.cf");
            fs::write(&first, archive(&client(), with_module)).unwrap();
            let source = scratch.0.join("source");
            let r = export(&first, &source, version);
            assert_eq!(r["export"]["storage"]["failed"], 0, "{r}");
            assert_eq!(r["export"]["storage"]["opaque"], 0, "{r}");
            let configuration = fs::read_to_string(source.join("Configuration.xml")).unwrap();
            assert!(
                configuration.contains("<WebSocketClient>SocketWitness</WebSocketClient>"),
                "{configuration}"
            );
            let ws = source.join("WebSocketClients/SocketWitness.xml");
            let expected = fs::read(&ws).unwrap();
            let decoded = recompile(std::str::from_utf8(&expected).unwrap(), version).unwrap();
            assert_eq!(decoded, client().to_brace());
            let module = source.join("WebSocketClients/SocketWitness/Ext/Module.bsl");
            if with_module {
                assert_eq!(fs::read(&module).unwrap(), MODULE);
            } else {
                assert!(!module.exists());
            }
            let rebuilt = scratch.0.join("rebuilt.cf");
            report(
                Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
                    .args(["cf", "bootstrap"])
                    .arg(&source)
                    .arg(&rebuilt)
                    .args(["--base-free", "--source-version", version])
                    .env("PATH", "")
                    .output()
                    .unwrap(),
            );
            for target_version in ["2.20", "2.21"] {
                let output = scratch.0.join(format!("export-{target_version}"));
                export(&rebuilt, &output, target_version);
                assert_eq!(
                    recompile(
                        &fs::read_to_string(output.join("WebSocketClients/SocketWitness.xml"))
                            .unwrap(),
                        target_version
                    )
                    .unwrap(),
                    decoded
                );
                if with_module {
                    assert_eq!(
                        fs::read(output.join("WebSocketClients/SocketWitness/Ext/Module.bsl"))
                            .unwrap(),
                        MODULE
                    );
                } else {
                    assert!(
                        !output
                            .join("WebSocketClients/SocketWitness/Ext/Module.bsl")
                            .exists()
                    );
                }
            }
        }
    }
}

#[test]
fn websocket_root_family_uses_existing_seven_groups_and_canonical_uuid() {
    let scratch = Scratch::new();
    let archive = archive(&client(), false);
    let cf = scratch.0.join("root.cf");
    fs::write(&cf, archive).unwrap();
    let output = scratch.0.join("xml");
    export(&cf, &output, "2.20");
    let configuration = fs::read_to_string(output.join("Configuration.xml")).unwrap();
    let context = DescriptorContext::new(&output, "2.20").unwrap();
    let bytes = compile_descriptor(
        "Configuration",
        &output.join("Configuration.xml"),
        configuration.as_bytes(),
        &context,
    )
    .unwrap();
    let row = parse_row(&bytes).unwrap();
    assert_eq!(row.at(&[2]), Some(&Brace::num(7)));
    let family = row.at(&[3, 1, 23]).unwrap().as_list().unwrap();
    assert_eq!(
        family,
        [
            Brace::atom(Layout::FAMILY_UUID),
            Brace::num(1),
            Brace::uuid(CLIENT)
        ]
    );
    assert!(
        configuration_objects(&row)
            .unwrap()
            .contains(&(Layout::KIND.into(), CLIENT.into()))
    );
    // The module codec stores the exact source bytes, not a guessed encoding.
    assert_eq!(
        unpack_module_blob_text(&pack_module_blob_bytes_base_free(MODULE, None).unwrap().blob)
            .unwrap(),
        MODULE
    );
}

#[test]
fn websocket_current_xml_edits_and_module_body_survive_base_free_rebuild() {
    for version in ["2.20", "2.21"] {
        let scratch = Scratch::new();
        let first = scratch.0.join("first.cf");
        fs::write(&first, archive(&client(), true)).unwrap();
        let source = scratch.0.join("source");
        export(&first, &source, version);
        let mut current = client();
        current.predefined = true;
        current.auto_connect = false;
        current.use_os_proxy = false;
        current.use_os_authentication = true;
        current.timeout = 79;
        current.server_url = "wss://example.invalid/edited".into();
        current.user = "edited-public-user".into();
        current.password = "PUBLIC-NONSECRET-EDIT".into();
        current.headers.reverse();
        current.headers.push(("X-C".into(), "edited&value".into()));
        fs::write(
            source.join("WebSocketClients/SocketWitness.xml"),
            write_document(&current.to_xml().unwrap(), version),
        )
        .unwrap();
        let edited_module = b"\xef\xbb\xbf// Edited cleanroom module.\r\nProcedure CurrentSocket() Export\r\nEndProcedure\r\n";
        fs::write(
            source.join("WebSocketClients/SocketWitness/Ext/Module.bsl"),
            edited_module,
        )
        .unwrap();
        let rebuilt = scratch.0.join("rebuilt.cf");
        report(
            Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
                .args(["cf", "bootstrap"])
                .arg(&source)
                .arg(&rebuilt)
                .args(["--base-free", "--source-version", version])
                .env("PATH", "")
                .output()
                .unwrap(),
        );
        let output = scratch.0.join("output");
        export(&rebuilt, &output, version);
        assert_eq!(
            recompile(
                &fs::read_to_string(output.join("WebSocketClients/SocketWitness.xml")).unwrap(),
                version
            )
            .unwrap(),
            current.to_brace()
        );
        assert_eq!(
            fs::read(output.join("WebSocketClients/SocketWitness/Ext/Module.bsl")).unwrap(),
            edited_module
        );
        let extracted = scratch.0.join("module-entry");
        report(
            Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
                .args(["cf", "extract"])
                .arg(&rebuilt)
                .arg(format!("{CLIENT}.0"))
                .arg(&extracted)
                .env("PATH", "")
                .output()
                .unwrap(),
        );
        assert_eq!(
            unpack_module_blob_text(&fs::read(extracted.join("packed.bin")).unwrap()).unwrap(),
            edited_module
        );
    }
}

#[test]
fn websocket_declared_malformed_descriptor_does_not_become_form_or_partial_xml() {
    for version in ["2.20", "2.21"] {
        let scratch = Scratch::new();
        let mut row = client().to_brace();
        row.as_list_mut().unwrap()[1].as_list_mut().unwrap()[6]
            .as_list_mut()
            .unwrap()[0] = Brace::num(3);
        let cf = scratch.0.join("bad.cf");
        fs::write(&cf, archive_with_row(row, false)).unwrap();
        let output = scratch.0.join("output");
        let result = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
            .args(["cf", "export"])
            .arg(&cf)
            .arg(&output)
            .args(["--source-version", version, "--fail-on-opaque"])
            .env("PATH", "")
            .output()
            .unwrap();
        assert!(!result.status.success());
        let status: Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(status["ok"], false, "{status}");
        let diagnostics = format!("{} {}", status, String::from_utf8_lossy(&result.stderr));
        assert!(diagnostics.contains("WebSocketClient"), "{diagnostics}");
        assert!(!output.join("WebSocketClients/SocketWitness.xml").exists());
        assert!(!output.join("Forms/SocketWitness.xml").exists());
    }
}
