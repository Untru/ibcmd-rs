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
