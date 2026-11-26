use morph1c_core::ir::{CaiRegion, ClientApplicationInterface};
use morph1c_pipeline::{ConvertOptions, Format, read_config, write_config};
use std::{
    fs,
    path::{Path, PathBuf},
};

const NATIVE: &[u8] = include_bytes!("fixtures/client-interface/direct.xml");
const EDT_LF: &[u8] = include_bytes!("fixtures/client-interface/direct-lf.cai");

/// A scratch directory, removed with the value (also when the test fails)
/// unless `IBCMD_TEST_KEEP_FIXTURES=1`.
fn scratch(label: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(&format!("ibcmd-cai-{label}-"))
        .disable_cleanup(keep_fixtures())
        .tempdir()
        .unwrap()
}

/// Whether the fixtures are kept for debugging: `IBCMD_TEST_KEEP_FIXTURES=1`.
fn keep_fixtures() -> bool {
    std::env::var_os("IBCMD_TEST_KEEP_FIXTURES").is_some_and(|value| value == "1")
}

fn native_source(root: &Path, cai: &[u8]) -> PathBuf {
    let source = root.join("native");
    let fixture = ibcmd_edt::read_xml_source(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/subsystem-ci/src"
        ),
        ibcmd_edt::ReaderLimits::default(),
    )
    .unwrap();
    for entry in fixture.entries() {
        let path = source.join(entry.path().as_str());
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, entry.bytes()).unwrap();
    }
    fs::create_dir_all(source.join("Ext")).unwrap();
    fs::write(source.join("Ext/ClientApplicationInterface.xml"), cai).unwrap();
    source
}

fn interface(config: &morph1c_core::ir::Configuration) -> &ClientApplicationInterface {
    config
        .objects
        .iter()
        .find_map(|obj| obj.client_application_interface.as_ref())
        .unwrap()
}

#[test]
fn witnessed_direct_panels_and_five_definitions_are_modelled_without_synthetic_groups() {
    let root = scratch("direct");
    let root = root.path().to_path_buf();
    let source = native_source(&root, NATIVE);
    let config = read_config(Format::Designer, &source, &ConvertOptions::default())
        .unwrap()
        .0;
    let cai = interface(&config);
    assert!(matches!(&cai.top, Some(CaiRegion::Panel(panel)) if panel.name == "OpenedPanel"));
    assert!(matches!(&cai.left, Some(CaiRegion::Panel(panel)) if panel.name == "SectionPanel"));
    assert_eq!(cai.available_panels.len(), 5);
    assert!(!cai.available_panels.iter().any(|name| name == "ToolsPanel"));
    // Canonical IR serialization must retain direct shape and the declared subset.
    let mut restored: ClientApplicationInterface =
        serde_json::from_value(serde_json::to_value(cai).unwrap()).unwrap();
    assert_eq!(
        serde_json::to_value(&restored).unwrap(),
        serde_json::to_value(cai).unwrap()
    );
    assert!(matches!(&restored.top, Some(CaiRegion::Panel(_))));
    if let Some(CaiRegion::Panel(panel)) = &mut restored.left {
        panel.id = "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa".to_owned();
    }
    assert_eq!(restored.available_panels, cai.available_panels);
    let mut edited_config = config.clone();
    *edited_config
        .objects
        .iter_mut()
        .find_map(|obj| obj.client_application_interface.as_mut())
        .unwrap() = restored;
    let edited = root.join("edited-native");
    write_config(Format::Designer, &edited_config, &edited).unwrap();
    let edited_bytes = fs::read(edited.join("Ext/ClientApplicationInterface.xml")).unwrap();
    assert_eq!(
        edited_bytes,
        String::from_utf8(NATIVE.to_vec())
            .unwrap()
            .replace(
                "00000000-0000-0000-0000-00000000000a",
                "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"
            )
            .as_bytes()
    );

    let edt = root.join("edt");
    write_config(Format::Edt, &config, &edt).unwrap();
    let generated = fs::read(edt.join("Configuration/ClientApplicationInterface.cai")).unwrap();
    assert_eq!(
        String::from_utf8(generated)
            .unwrap()
            .replace("\r\n", "\n")
            .as_bytes(),
        EDT_LF
    );
    // Replace only the one sidecar with actual LF-only installed-EDT bytes.
    fs::write(
        edt.join("Configuration/ClientApplicationInterface.cai"),
        EDT_LF,
    )
    .unwrap();
    let from_edt = read_config(Format::Edt, &edt, &ConvertOptions::default())
        .unwrap()
        .0;
    assert!(interface(&from_edt).edt_lf);
    assert_eq!(
        serde_json::to_value(interface(&config)).unwrap(),
        serde_json::to_value(interface(&from_edt)).unwrap()
    );
    let returned_edt = root.join("edt-returned");
    write_config(Format::Edt, &from_edt, &returned_edt).unwrap();
    assert_eq!(
        fs::read(returned_edt.join("Configuration/ClientApplicationInterface.cai")).unwrap(),
        EDT_LF
    );
    let returned_native = root.join("native-returned");
    write_config(Format::Designer, &from_edt, &returned_native).unwrap();
    assert_eq!(
        fs::read(returned_native.join("Ext/ClientApplicationInterface.xml")).unwrap(),
        NATIVE
    );
}

#[test]
fn legacy_group_and_full_six_panel_table_remain_exact() {
    let root = scratch("legacy");
    let root = root.path().to_path_buf();
    let text = String::from_utf8(NATIVE.to_vec()).unwrap();
    let start = text.find("\t<top>").unwrap();
    let end = text.find("\t<panelDef").unwrap();
    let group = "\t<left>\r\n\t\t<group id=\"11111111-1111-1111-1111-111111111111\">\r\n\t\t\t<group>\r\n\t\t\t\t<panel id=\"22222222-2222-2222-2222-222222222222\">\r\n\t\t\t\t\t<uuid>b553047f-c9aa-4157-978d-448ecad24248</uuid>\r\n\t\t\t\t</panel>\r\n\t\t\t</group>\r\n\t\t</group>\r\n\t</left>\r\n";
    let mut legacy = format!("{}{}", &text[..start], group);
    for (_, id) in morph1c_core::ir::STANDARD_CLIENT_PANELS {
        legacy.push_str(&format!("\t<panelDef id=\"{id}\"/>\r\n"));
    }
    legacy.push_str("</ClientApplicationInterface>");
    assert!(end > start);
    let source = native_source(&root, legacy.as_bytes());
    let config = read_config(Format::Designer, &source, &ConvertOptions::default())
        .unwrap()
        .0;
    assert!(interface(&config).top.is_none());
    assert!(matches!(interface(&config).left, Some(CaiRegion::Group(_))));
    assert_eq!(interface(&config).available_panels.len(), 6);
    let edt = root.join("edt");
    write_config(Format::Edt, &config, &edt).unwrap();
    let back = read_config(Format::Edt, &edt, &ConvertOptions::default())
        .unwrap()
        .0;
    let output = root.join("back");
    write_config(Format::Designer, &back, &output).unwrap();
    assert_eq!(
        fs::read(output.join("Ext/ClientApplicationInterface.xml")).unwrap(),
        legacy.as_bytes()
    );
}

#[test]
fn unknown_missing_duplicate_or_reordered_panel_definitions_are_rejected() {
    let original = String::from_utf8(NATIVE.to_vec()).unwrap();
    let section = "\t<panelDef id=\"b553047f-c9aa-4157-978d-448ecad24248\"/>\r\n";
    for (label, body) in [
        (
            "unknown",
            original.replace(
                "cbab57f2-a0f3-4f0a-89ea-4cb19570ab75",
                "ffffffff-ffff-ffff-ffff-ffffffffffff",
            ),
        ),
        ("missing", original.replace(section, "")),
        (
            "duplicate",
            original.replace(section, &format!("{section}{section}")),
        ),
        (
            "reordered",
            original.replace(section, "").replace(
                "</ClientApplicationInterface>",
                &format!("{section}</ClientApplicationInterface>"),
            ),
        ),
    ] {
        let root = scratch(label);
        let root = root.path().to_path_buf();
        let source = native_source(&root, body.as_bytes());
        assert!(
            read_config(Format::Designer, &source, &ConvertOptions::default()).is_err(),
            "{label}"
        );
    }
}

#[test]
fn lf_framing_does_not_accept_mixed_framing_or_extra_model_cells() {
    for (label, bytes) in [
        (
            "mixed",
            String::from_utf8(EDT_LF.to_vec())
                .unwrap()
                .replacen("\n", "\r\n", 1),
        ),
        (
            "unknown-cell",
            String::from_utf8(EDT_LF.to_vec()).unwrap().replace(
                "name=\"OpenedPanel\"",
                "name=\"OpenedPanel\" unknown=\"lost\"",
            ),
        ),
    ] {
        let root = scratch(label);
        let root = root.path().to_path_buf();
        let source = native_source(&root, NATIVE);
        let config = read_config(Format::Designer, &source, &ConvertOptions::default())
            .unwrap()
            .0;
        let edt = root.join("edt");
        write_config(Format::Edt, &config, &edt).unwrap();
        fs::write(
            edt.join("Configuration/ClientApplicationInterface.cai"),
            bytes,
        )
        .unwrap();
        assert!(
            read_config(Format::Edt, &edt, &ConvertOptions::default()).is_err(),
            "{label}"
        );
    }
}
