use ibcmd_edt::{
    ConversionOptions, Project, ReaderLimits, edt_to_xml, read_xml_source, xml_to_edt,
};
use ibcmd_xml::source_tree::{SourceEntry, SourcePath, SourceTree};
use sha2::{Digest, Sha256};

fn options() -> ConversionOptions {
    ConversionOptions {
        edt_version: "2025.2.3".into(),
        xml_dialect: "2.21".into(),
        runtime_version: Some("8.5.1".into()),
    }
}
fn fixture() -> SourceTree {
    read_xml_source(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/subsystem-ci/src"
        ),
        ReaderLimits::default(),
    )
    .unwrap()
}
#[test]
fn client_interface_preserves_absent_top_region_and_rejects_unknown_region() {
    let head = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<ClientApplicationInterface xmlns=\"http://v8.1c.ru/8.2/managed-application/core\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xsi:type=\"InterfaceLayouter\">\r\n";
    let left = "\t<left>\r\n\t\t<group id=\"11111111-1111-1111-1111-111111111111\">\r\n\t\t\t<group>\r\n\t\t\t\t<panel id=\"22222222-2222-2222-2222-222222222222\">\r\n\t\t\t\t\t<uuid>b553047f-c9aa-4157-978d-448ecad24248</uuid>\r\n\t\t\t\t</panel>\r\n\t\t\t</group>\r\n\t\t</group>\r\n\t</left>\r\n";
    let mut body = format!("\u{feff}{head}{left}");
    for (_, id) in morph1c_core::ir::STANDARD_CLIENT_PANELS {
        body.push_str(&format!("\t<panelDef id=\"{id}\"/>\r\n"));
    }
    body.push_str("</ClientApplicationInterface>");
    let source = mutate(
        &fixture(),
        "Ext/ClientApplicationInterface.xml",
        body.as_bytes().to_vec(),
    );
    let edt = xml_to_edt(&source, &options()).unwrap();
    let cai = edt
        .tree
        .entries()
        .iter()
        .find(|e| {
            e.path()
                .as_str()
                .ends_with("ClientApplicationInterface.cai")
        })
        .unwrap();
    let text = std::str::from_utf8(cai.bytes()).unwrap();
    assert!(!text.contains("<top "));
    assert!(text.contains("<left "));
    let stripped = SourceTree::new(
        edt.tree
            .entries()
            .iter()
            .filter(|e| !e.path().as_str().starts_with(".ibcmd-provenance/"))
            .cloned()
            .collect(),
    )
    .unwrap();
    let returned = edt_to_xml(&Project::from_tree(stripped.clone()).unwrap(), &options()).unwrap();
    assert_eq!(
        returned
            .tree
            .entries()
            .iter()
            .find(|e| e.path().as_str() == "Ext/ClientApplicationInterface.xml")
            .unwrap()
            .bytes(),
        body.as_bytes()
    );
    let bad = mutate(
        &stripped,
        cai.path().as_str(),
        text.replace("<left ", "<right ")
            .replace("</left>", "</right>")
            .into_bytes(),
    );
    assert!(edt_to_xml(&Project::from_tree(bad).unwrap(), &options()).is_err());
}
fn mutate(tree: &SourceTree, path: &str, bytes: Vec<u8>) -> SourceTree {
    let mut entries = tree
        .entries()
        .iter()
        .filter(|e| e.path().as_str() != path)
        .cloned()
        .collect::<Vec<_>>();
    entries.push(SourceEntry::from_bytes(SourcePath::new(path).unwrap(), bytes).unwrap());
    SourceTree::new(entries).unwrap()
}
#[test]
fn exact_unchanged_xml_return_and_canonical_properties() {
    let xml = fixture();
    let converted = xml_to_edt(&xml, &options()).unwrap();
    assert_eq!(converted.accounting.len(), xml.entries().len());
    assert!(
        converted
            .canonical
            .objects()
            .iter()
            .any(|o| !o.properties().is_empty())
    );
    assert!(
        converted
            .tree
            .entries()
            .iter()
            .any(|e| e.path().as_str() == ".project")
    );
    let returned = edt_to_xml(&Project::from_tree(converted.tree).unwrap(), &options()).unwrap();
    assert_eq!(returned.tree, xml);
}
#[test]
fn provenance_stripped_typed_conversion() {
    let xml = fixture();
    let generated = xml_to_edt(&xml, &options()).unwrap();
    let typed = SourceTree::new(
        generated
            .tree
            .entries()
            .iter()
            .filter(|e| !e.path().as_str().starts_with(".ibcmd-provenance/"))
            .cloned()
            .collect(),
    )
    .unwrap();
    let returned = edt_to_xml(&Project::from_tree(typed).unwrap(), &options()).unwrap();
    assert!(
        returned
            .tree
            .entries()
            .iter()
            .any(|e| e.path().as_str() == "Configuration.xml")
    );
    assert!(!returned.canonical.is_empty());
}
#[test]
fn missing_runtime_and_profile_mismatch_are_errors() {
    let mut o = options();
    o.runtime_version = None;
    assert!(xml_to_edt(&fixture(), &o).is_err());
    o.runtime_version = Some("8.3.27".into());
    assert!(xml_to_edt(&fixture(), &o).is_err());
    o = options();
    o.edt_version = "latest".into();
    assert!(xml_to_edt(&fixture(), &o).is_err());
}
#[test]
fn added_deleted_or_edited_project_files_do_not_restore_xml() {
    let generated = xml_to_edt(&fixture(), &options()).unwrap().tree;
    let changed = mutate(
        &generated,
        "DT-INF/PROJECT.PMF",
        b"Manifest-Version: 1.0\r\nRuntime-Version: 8.5.1\r\n\r\n".to_vec(),
    );
    assert!(
        edt_to_xml(&Project::from_tree(changed).unwrap(), &options())
            .unwrap_err()
            .to_string()
            .contains("stale provenance")
    );
    let added = mutate(&generated, "src/notes.bin", vec![1, 2]);
    assert!(edt_to_xml(&Project::from_tree(added).unwrap(), &options()).is_err());
    let deleted = SourceTree::new(
        generated
            .entries()
            .iter()
            .filter(|e| e.path().as_str() != ".settings/org.eclipse.core.resources.prefs")
            .cloned()
            .collect(),
    )
    .unwrap();
    assert!(edt_to_xml(&Project::from_tree(deleted).unwrap(), &options()).is_err());
}
#[test]
fn unknown_source_files_fail_closed() {
    let source = mutate(&fixture(), "unknown.bin", vec![8]);
    assert!(
        xml_to_edt(&source, &options())
            .unwrap_err()
            .to_string()
            .contains("refusing to skip")
    );
}
#[test]
fn traversal_before_codec_and_depth_before_recursive_parser() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("bad.mdo"),
        b"<root><Name>../../escape</Name></root>",
    )
    .unwrap();
    assert!(read_xml_source(tmp.path(), ReaderLimits::default()).is_err());
    std::fs::write(
        tmp.path().join("bad.mdo"),
        format!("{}{}", "<x>".repeat(129), "</x>".repeat(129)),
    )
    .unwrap();
    assert!(
        read_xml_source(tmp.path(), ReaderLimits::default())
            .unwrap_err()
            .to_string()
            .contains("depth budget")
    );
    std::fs::write(
        tmp.path().join("bad.mdo"),
        b"<root><ChildObjects><Form>../../escape</Form></ChildObjects></root>",
    )
    .unwrap();
    assert!(read_xml_source(tmp.path(), ReaderLimits::default()).is_err());
}

#[test]
fn retained_xml_with_updated_hash_still_must_match_typed_edt() {
    let generated = xml_to_edt(&fixture(), &options()).unwrap().tree;
    let path = ".ibcmd-provenance/xml/Configuration.xml";
    let original = generated
        .entries()
        .iter()
        .find(|e| e.path().as_str() == path)
        .unwrap();
    let changed = String::from_utf8(original.bytes().to_vec())
        .unwrap()
        .replacen("<Name>", "<Name>Tampered", 1)
        .into_bytes();
    let changed_hash = format!("{:x}", Sha256::digest(&changed));
    let tree = mutate(&generated, path, changed);
    let manifest = tree
        .entries()
        .iter()
        .find(|e| e.path().as_str() == ".ibcmd-provenance/manifest.json")
        .unwrap();
    let mut value: serde_json::Value = serde_json::from_slice(manifest.bytes()).unwrap();
    value["original"]["Configuration.xml"] = serde_json::Value::String(changed_hash);
    let tree = mutate(
        &tree,
        ".ibcmd-provenance/manifest.json",
        serde_json::to_vec(&value).unwrap(),
    );
    let error = edt_to_xml(&Project::from_tree(tree).unwrap(), &options()).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("retained original XML disagrees"),
        "{error}"
    );
}

#[test]
fn module_bytes_accounted_and_edits_not_hidden() {
    let xml = mutate(
        &fixture(),
        "Ext/ManagedApplicationModule.bsl",
        b"// offline proof\r\n".to_vec(),
    );
    let generated = xml_to_edt(&xml, &options()).unwrap();
    assert!(
        generated
            .canonical
            .objects()
            .iter()
            .any(|o| !o.assets().is_empty())
    );
    let path = "src/Configuration/ManagedApplicationModule.bsl";
    assert!(
        generated
            .tree
            .entries()
            .iter()
            .any(|e| e.path().as_str() == path)
    );
    let changed = mutate(&generated.tree, path, b"// edited module\r\n".to_vec());
    assert!(edt_to_xml(&Project::from_tree(changed.clone()).unwrap(), &options()).is_err());
    let clean = SourceTree::new(
        changed
            .entries()
            .iter()
            .filter(|e| !e.path().as_str().starts_with(".ibcmd-provenance/"))
            .cloned()
            .collect(),
    )
    .unwrap();
    let returned = edt_to_xml(&Project::from_tree(clean).unwrap(), &options()).unwrap();
    assert_eq!(
        returned
            .tree
            .entries()
            .iter()
            .find(|e| e.path().as_str() == "Ext/ManagedApplicationModule.bsl")
            .unwrap()
            .bytes(),
        b"\xef\xbb\xbf// edited module\r\n"
    );
}

#[test]
fn malformed_control_and_unknown_dt_inf_are_rejected() {
    let generated = xml_to_edt(&fixture(), &options()).unwrap().tree;
    let controls = mutate(&generated, "DT-INF/custom.bin", vec![1, 2]);
    assert!(edt_to_xml(&Project::from_tree(controls).unwrap(), &options()).is_err());
    let project = generated
        .entries()
        .iter()
        .find(|e| e.path().as_str() == ".project")
        .unwrap();
    let text = String::from_utf8(project.bytes().to_vec())
        .unwrap()
        .replace("<comment></comment>", "<comment>keep me</comment>");
    assert!(
        edt_to_xml(
            &Project::from_tree(mutate(&generated, ".project", text.into_bytes())).unwrap(),
            &options()
        )
        .is_err()
    );
}

#[cfg(unix)]
#[test]
fn literal_backslash_filename_is_rejected_before_path_normalization() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join(r"bad\name.bsl"), b"module").unwrap();
    let error = read_xml_source(directory.path(), ReaderLimits::default()).unwrap_err();
    assert!(error.to_string().contains("non-portable project filename"));
}

fn aggregate_fixture() -> SourceTree {
    let source = fixture();
    let configuration = source
        .entries()
        .iter()
        .find(|e| e.path().as_str() == "Configuration.xml")
        .unwrap();
    let xml = String::from_utf8(configuration.bytes().to_vec()).unwrap();
    let root_start = xml.find("<MetaDataObject").unwrap();
    let root_end = xml[root_start..].find('>').unwrap() + root_start + 1;
    let header = &xml[..root_end];
    let descriptor = format!(
        "{header}<AccumulationRegister uuid=\"99999999-9999-9999-9999-999999999999\"><Properties><Name>TestAggregates</Name><RegisterType>Turnovers</RegisterType></Properties><ChildObjects/></AccumulationRegister></MetaDataObject>"
    );
    let changed = xml.replace(
        "</ChildObjects>",
        "<AccumulationRegister>TestAggregates</AccumulationRegister></ChildObjects>",
    );
    let source = mutate(&source, "Configuration.xml", changed.into_bytes());
    let source = mutate(
        &source,
        "AccumulationRegisters/TestAggregates.xml",
        descriptor.into_bytes(),
    );
    mutate(&source,"AccumulationRegisters/TestAggregates/Ext/Aggregates.xml",br#"<?xml version="1.0" encoding="UTF-8"?><AccumulationRegisterAggregates xmlns="http://v8.1c.ru/8.3/xcf/extrnprops" xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" version="2.21"><Aggregate id="aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"><Use>Always</Use><Periodicity>Day</Periodicity><Dimensions/></Aggregate></AccumulationRegisterAggregates>"#.to_vec())
}
#[test]
fn palette_metadata_edited_rgb_survives_without_provenance() {
    let source = fixture();
    let config = source
        .entries()
        .iter()
        .find(|e| e.path().as_str() == "Configuration.xml")
        .unwrap();
    let xml = String::from_utf8(config.bytes().to_vec()).unwrap();
    let start = xml.find("<MetaDataObject").unwrap();
    let end = start + xml[start..].find('>').unwrap() + 1;
    let descriptor = format!(
        "{}<PaletteColor uuid=\"99999999-9999-9999-9999-999999999999\"><Properties><Name>TestPalette</Name><Synonym/><Comment/><Color>#FFEC9D</Color></Properties></PaletteColor></MetaDataObject>",
        &xml[..end]
    );
    let source = mutate(
        &source,
        "Configuration.xml",
        xml.replace(
            "</ChildObjects>",
            "<PaletteColor>TestPalette</PaletteColor></ChildObjects>",
        )
        .into_bytes(),
    );
    let source = mutate(
        &source,
        "PaletteColors/TestPalette.xml",
        descriptor.as_bytes().to_vec(),
    );
    let generated = xml_to_edt(&source, &options()).unwrap().tree;
    let path = "src/PaletteColors/TestPalette/TestPalette.mdo";
    let color = generated
        .entries()
        .iter()
        .find(|e| e.path().as_str() == path)
        .unwrap();
    let edited = std::str::from_utf8(color.bytes())
        .unwrap()
        .replace("<blue>157</blue>", "<blue>128</blue>");
    let tree = mutate(&generated, path, edited.into_bytes());
    let stripped = SourceTree::new(
        tree.entries()
            .iter()
            .filter(|e| !e.path().as_str().starts_with(".ibcmd-provenance/"))
            .cloned()
            .collect(),
    )
    .unwrap();
    let returned = edt_to_xml(&Project::from_tree(stripped).unwrap(), &options()).unwrap();
    let color = returned
        .tree
        .entries()
        .iter()
        .find(|e| e.path().as_str() == "PaletteColors/TestPalette.xml")
        .unwrap();
    assert!(
        std::str::from_utf8(color.bytes())
            .unwrap()
            .contains("<Color>#FFEC80</Color>")
    );
    let registry =
        morph1c_pipeline::registry::FormatRegistry::for_format(morph1c_pipeline::Format::Designer)
            .unwrap();
    assert!(
        (registry.get("PaletteColor").unwrap().read)(
            descriptor
                .replace("version=\"2.21\"", "version=\"2.20\"")
                .as_bytes()
        )
        .unwrap_err()
        .contains("2.21")
    );
}
#[test]
fn typed_extra_tampering_and_explicit_edited_conversion() {
    let generated = xml_to_edt(&aggregate_fixture(), &options()).unwrap().tree;
    let original_path =
        ".ibcmd-provenance/xml/AccumulationRegisters/TestAggregates/Ext/Aggregates.xml";
    let original = generated
        .entries()
        .iter()
        .find(|e| e.path().as_str() == original_path)
        .unwrap();
    let changed = String::from_utf8(original.bytes().to_vec())
        .unwrap()
        .replace(
            "<Periodicity>Day</Periodicity>",
            "<Periodicity>Month</Periodicity>",
        )
        .into_bytes();
    let hash = format!("{:x}", Sha256::digest(&changed));
    let tree = mutate(&generated, original_path, changed);
    let manifest = tree
        .entries()
        .iter()
        .find(|e| e.path().as_str() == ".ibcmd-provenance/manifest.json")
        .unwrap();
    let mut value: serde_json::Value = serde_json::from_slice(manifest.bytes()).unwrap();
    value["original"]["AccumulationRegisters/TestAggregates/Ext/Aggregates.xml"] =
        serde_json::Value::String(hash);
    let tree = mutate(
        &tree,
        ".ibcmd-provenance/manifest.json",
        serde_json::to_vec(&value).unwrap(),
    );
    assert!(
        edt_to_xml(&Project::from_tree(tree).unwrap(), &options())
            .unwrap_err()
            .to_string()
            .contains("disagrees")
    );
    let path = "src/AccumulationRegisters/TestAggregates/TestAggregates.mdo";
    let descriptor = generated
        .entries()
        .iter()
        .find(|e| e.path().as_str() == path)
        .unwrap();
    let changed = String::from_utf8(descriptor.bytes().to_vec())
        .unwrap()
        .replace(
            "<periodicity>Day</periodicity>",
            "<periodicity>Month</periodicity>",
        )
        .into_bytes();
    let tree = mutate(&generated, path, changed);
    let clean = SourceTree::new(
        tree.entries()
            .iter()
            .filter(|e| !e.path().as_str().starts_with(".ibcmd-provenance/"))
            .cloned()
            .collect(),
    )
    .unwrap();
    let converted = edt_to_xml(&Project::from_tree(clean).unwrap(), &options()).unwrap();
    let body = converted
        .tree
        .entries()
        .iter()
        .find(|e| e.path().as_str() == "AccumulationRegisters/TestAggregates/Ext/Aggregates.xml")
        .unwrap();
    assert!(
        std::str::from_utf8(body.bytes())
            .unwrap()
            .contains("<Periodicity>Month</Periodicity>")
    );
}
