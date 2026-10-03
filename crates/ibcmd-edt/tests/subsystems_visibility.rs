use morph1c_core::ir::{CommandInterface, RoleVisibility, SubsystemVisibility};
use morph1c_core::version::{FormatVersion, with_roundtrip_target};
use morph1c_pipeline::{ConvertOptions, Format, read_config, write_config};
use sha2::{Digest, Sha256};

fn options(version: FormatVersion) -> ConvertOptions {
    ConvertOptions::default().with_target_version(version)
}
fn seed() -> morph1c_core::ir::Configuration {
    read_config(
        Format::Designer,
        std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/subsystem-ci/src"
        )),
        &options(FormatVersion::new(2, 21)),
    )
    .unwrap()
    .0
}
fn set_ci(config: &mut morph1c_core::ir::Configuration) {
    config
        .objects
        .iter_mut()
        .find(|o| o.kind.as_str() == "Configuration")
        .unwrap()
        .root_command_interface = Some(CommandInterface {
        native_groups_order: None,
        subsystems_visibility: vec![SubsystemVisibility {
            subsystem: "Subsystem.Управление".into(),
            common_visible: true,
            role_values: vec![
                RoleVisibility {
                    role: "Role.Read".into(),
                    visible: true,
                },
                RoleVisibility {
                    role: "Role.Hidden".into(),
                    visible: false,
                },
            ],
        }],
        commands: vec![],
        placement: vec![],
        order: vec![],
        subsystems_order: vec!["Subsystem.Управление".into()],
    });
}
fn writer(
    format: Format,
    config: &morph1c_core::ir::Configuration,
    path: &std::path::Path,
    version: FormatVersion,
) {
    with_roundtrip_target(version, || write_config(format, config, path)).unwrap();
}
fn ci_path(format: Format) -> &'static str {
    if format == Format::Edt {
        "Configuration/CommandInterface.cmi"
    } else {
        "Ext/CommandInterface.xml"
    }
}
#[test]
fn ordered_subsystem_common_and_role_values_survive_both_dialects_and_current_edits() {
    for version in [FormatVersion::new(2, 20), FormatVersion::new(2, 21)] {
        let mut config = seed();
        set_ci(&mut config);
        let original = config
            .objects
            .iter()
            .find(|o| o.kind.as_str() == "Configuration")
            .unwrap()
            .root_command_interface
            .clone();
        for format in [Format::Edt, Format::Designer, Format::Edt] {
            let dir = tempfile::tempdir().unwrap();
            writer(format, &config, dir.path(), version);
            config = read_config(format, dir.path(), &options(version))
                .unwrap()
                .0;
            assert_eq!(
                config
                    .objects
                    .iter()
                    .find(|o| o.kind.as_str() == "Configuration")
                    .unwrap()
                    .root_command_interface,
                original
            );
        }
        let root = config
            .objects
            .iter_mut()
            .find(|o| o.kind.as_str() == "Configuration")
            .unwrap();
        let row = &mut root
            .root_command_interface
            .as_mut()
            .unwrap()
            .subsystems_visibility[0];
        row.common_visible = false;
        row.role_values.reverse();
        row.role_values[0].visible = true;
        let dir = tempfile::tempdir().unwrap();
        writer(Format::Designer, &config, dir.path(), version);
        let read = read_config(Format::Designer, dir.path(), &options(version))
            .unwrap()
            .0;
        assert_ne!(
            read.objects
                .iter()
                .find(|o| o.kind.as_str() == "Configuration")
                .unwrap()
                .root_command_interface,
            original
        );
        assert_eq!(
            read.objects
                .iter()
                .find(|o| o.kind.as_str() == "Configuration")
                .unwrap()
                .root_command_interface,
            config
                .objects
                .iter()
                .find(|o| o.kind.as_str() == "Configuration")
                .unwrap()
                .root_command_interface
        );
    }
}
#[test]
fn subsystem_visibility_rejects_unknown_duplicate_attributes_namespaces_and_fields() {
    let mut config = seed();
    set_ci(&mut config);
    let dir = tempfile::tempdir().unwrap();
    writer(Format::Edt, &config, dir.path(), FormatVersion::new(2, 21));
    let path = dir.path().join(ci_path(Format::Edt));
    let original = std::fs::read_to_string(&path).unwrap();
    for changed in [
        original.replace(
            "<subsystemsVisibility>",
            "<subsystemsVisibility extra=\"true\">",
        ),
        original.replace("</subsystem>", "</subsystem><unknown/>"),
        original.replace("<visible>", "<visible><common>false</common>"),
        original.replace("<subsystem>", "<subsystem><nested/>"),
        original
            .replace("<subsystem>", "<x:subsystem xmlns:x=\"urn:unknown\">")
            .replace("</subsystem>", "</x:subsystem>"),
        original.replace("Role.Read", "Unknown.Read"),
        original.replace("<value>true</value>", "<value>1</value>"),
        original.replace(
            "</subsystemsVisibility>",
            "</subsystemsVisibility><subsystemsVisibility/>",
        ),
    ] {
        assert_ne!(changed, original);
        std::fs::write(&path, changed).unwrap();
        assert!(read_config(Format::Edt, dir.path(), &options(FormatVersion::new(2, 21))).is_err());
    }
}
#[test]
fn public_exact_return_stripped_conversion_and_forged_hash_edits_are_distinct() {
    use ibcmd_edt::{
        ConversionOptions, Project, ReaderLimits, edt_to_xml, read_xml_source, xml_to_edt,
    };
    use ibcmd_xml::source_tree::{SourceEntry, SourceTree};
    let mut config = seed();
    set_ci(&mut config);
    let dir = tempfile::tempdir().unwrap();
    writer(
        Format::Designer,
        &config,
        dir.path(),
        FormatVersion::new(2, 21),
    );
    let source = read_xml_source(dir.path(), ReaderLimits::default()).unwrap();
    let options = ConversionOptions {
        edt_version: "2025.2.3".into(),
        xml_dialect: "2.21".into(),
        runtime_version: Some("8.5.1".into()),
    };
    let generated = xml_to_edt(&source, &options).unwrap().tree;
    assert_eq!(
        edt_to_xml(&Project::from_tree(generated.clone()).unwrap(), &options)
            .unwrap()
            .tree,
        source
    );
    let stripped = SourceTree::new(
        generated
            .entries()
            .iter()
            .filter(|e| !e.path().as_str().starts_with(".ibcmd-provenance/"))
            .cloned()
            .collect(),
    )
    .unwrap();
    let typed = edt_to_xml(&Project::from_tree(stripped).unwrap(), &options)
        .unwrap()
        .tree;
    assert_eq!(
        typed
            .entries()
            .iter()
            .find(|e| e.path().as_str() == "Ext/CommandInterface.xml")
            .unwrap()
            .bytes(),
        source
            .entries()
            .iter()
            .find(|e| e.path().as_str() == "Ext/CommandInterface.xml")
            .unwrap()
            .bytes()
    );
    let path = "src/Configuration/CommandInterface.cmi";
    let changed = String::from_utf8(
        generated
            .entries()
            .iter()
            .find(|e| e.path().as_str() == path)
            .unwrap()
            .bytes()
            .to_vec(),
    )
    .unwrap()
    .replace("        <common>true</common>\r\n", "")
    .into_bytes();
    assert_ne!(
        changed,
        generated
            .entries()
            .iter()
            .find(|e| e.path().as_str() == path)
            .unwrap()
            .bytes()
    );
    let mut manifest: serde_json::Value = serde_json::from_slice(
        generated
            .entries()
            .iter()
            .find(|e| e.path().as_str() == ".ibcmd-provenance/manifest.json")
            .unwrap()
            .bytes(),
    )
    .unwrap();
    let hash = format!("{:x}", Sha256::digest(&changed));
    manifest["generated"][path] = serde_json::json!(hash);
    let changed_tree = SourceTree::new(
        generated
            .entries()
            .iter()
            .map(|entry| {
                if entry.path().as_str() == path {
                    SourceEntry::from_bytes(entry.path().clone(), changed.clone()).unwrap()
                } else if entry.path().as_str() == ".ibcmd-provenance/manifest.json" {
                    SourceEntry::from_bytes(
                        entry.path().clone(),
                        serde_json::to_vec(&manifest).unwrap(),
                    )
                    .unwrap()
                } else {
                    entry.clone()
                }
            })
            .collect(),
    )
    .unwrap();
    let error = edt_to_xml(&Project::from_tree(changed_tree).unwrap(), &options)
        .unwrap_err()
        .to_string();
    assert!(error.contains("stale provenance"), "{error}");
}
#[test]
#[ignore = "Real immutable UH83/UH85 CMI paired witness; IBCMD_EDT_LAB on F"]
fn genuine_subsystem_visibility_is_exact_against_original_and_installed_sdk() {
    let lab =
        std::path::PathBuf::from(std::env::var_os("IBCMD_EDT_LAB").expect("lab evidence required"));
    for (prepared, version) in [
        ("oracle-uha83-r1", FormatVersion::new(2, 20)),
        ("oracle-uha85-r2", FormatVersion::new(2, 21)),
    ] {
        let edt = std::fs::read(lab.join(prepared).join(
            "authentic-workspace/OracleConfiguration/src/Configuration/CommandInterface.cmi",
        ))
        .unwrap();
        let native = std::fs::read(
            lab.join(prepared)
                .join("edt-native-xml/Ext/CommandInterface.xml"),
        )
        .unwrap();
        let config = seed();
        let dir = tempfile::tempdir().unwrap();
        writer(Format::Edt, &config, dir.path(), version);
        std::fs::write(dir.path().join(ci_path(Format::Edt)), &edt).unwrap();
        let decoded = read_config(Format::Edt, dir.path(), &options(version))
            .unwrap()
            .0;
        let ci = decoded
            .objects
            .iter()
            .find(|o| o.kind.as_str() == "Configuration")
            .unwrap()
            .root_command_interface
            .as_ref()
            .unwrap();
        assert!(!ci.subsystems_visibility.is_empty());
        let output = tempfile::tempdir().unwrap();
        writer(Format::Designer, &decoded, output.path(), version);
        assert_eq!(
            std::fs::read(output.path().join(ci_path(Format::Designer))).unwrap(),
            native
        );
        let decoded_native = read_config(Format::Designer, output.path(), &options(version))
            .unwrap()
            .0;
        assert_eq!(
            decoded_native
                .objects
                .iter()
                .find(|o| o.kind.as_str() == "Configuration")
                .unwrap()
                .root_command_interface
                .as_ref(),
            Some(ci)
        );
        let returned = tempfile::tempdir().unwrap();
        writer(Format::Edt, &decoded_native, returned.path(), version);
        assert_eq!(
            std::fs::read(returned.path().join(ci_path(Format::Edt))).unwrap(),
            edt
        );
    }
}
