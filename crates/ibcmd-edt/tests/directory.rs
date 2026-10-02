use ibcmd_edt::{
    ConversionOptions, Project, ReaderLimits, edt_to_xml, read_directory_source, read_xml_source,
    xml_to_edt,
};
use ibcmd_xml::source_tree::{SourceTree, publish_new_with_limits};
fn options() -> ConversionOptions {
    ConversionOptions {
        edt_version: "2025.2.3".into(),
        xml_dialect: "2.21".into(),
        runtime_version: Some("8.5.1".into()),
    }
}
fn fixture() -> std::path::PathBuf {
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/subsystem-ci/src"
    )
    .into()
}
fn source() -> SourceTree {
    read_xml_source(fixture(), ReaderLimits::default()).unwrap()
}
#[test]
fn generated_project_uses_installed_edt_configuration_builders_and_natures() {
    let tree = xml_to_edt(&source(), &options()).unwrap().tree;
    let descriptor = tree
        .entries()
        .iter()
        .find(|entry| entry.path().as_str() == ".project")
        .unwrap();
    let text = std::str::from_utf8(descriptor.bytes()).unwrap();
    assert_eq!(text.matches("<buildCommand>").count(), 1);
    assert_eq!(text.matches("<nature>").count(), 2);
    let configuration = text
        .find("com._1c.g5.v8.dt.core.V8ConfigurationNature")
        .unwrap();
    let xtext = text
        .find("org.eclipse.xtext.ui.shared.xtextNature")
        .unwrap();
    assert!(configuration < xtext);
    assert!(text.contains("org.eclipse.xtext.ui.shared.xtextBuilder"));
    assert!(!text.contains("com.e1c.langtool"));
}
#[test]
fn disk_and_memory_both_routes_have_identical_outputs_models_and_accounting() {
    let root = tempfile::tempdir().unwrap();
    let memory = xml_to_edt(&source(), &options()).unwrap();
    let directory = read_directory_source(fixture())
        .unwrap()
        .xml_to_edt(&options())
        .unwrap();
    assert_eq!(
        serde_json::to_value(&directory.canonical).unwrap(),
        serde_json::to_value(&memory.canonical).unwrap()
    );
    assert_eq!(directory.extensions, memory.extensions);
    let disposition = |value| match value {
        ibcmd_edt::Disposition::Converted => "converted",
        ibcmd_edt::Disposition::Retained => "retained",
    };
    assert_eq!(
        directory
            .accounting
            .iter()
            .map(|file| (file.path.clone(), disposition(file.disposition)))
            .collect::<Vec<_>>(),
        memory
            .accounting
            .iter()
            .map(|file| (file.path.to_string(), disposition(file.disposition)))
            .collect::<Vec<_>>()
    );
    let edt = root.path().join("edt");
    directory.publish_new(&edt).unwrap();
    assert_eq!(
        read_xml_source(&edt, ReaderLimits::default()).unwrap(),
        memory.tree
    );
    let returned = read_directory_source(&edt)
        .unwrap()
        .edt_to_xml(&options())
        .unwrap();
    let xml = root.path().join("xml");
    returned.publish_new(&xml).unwrap();
    assert_eq!(
        read_xml_source(&xml, ReaderLimits::default()).unwrap(),
        source()
    );
    assert_eq!(
        serde_json::to_value(&returned.canonical).unwrap(),
        serde_json::to_value(
            &edt_to_xml(&Project::from_tree(memory.tree).unwrap(), &options())
                .unwrap()
                .canonical
        )
        .unwrap()
    );
    assert!(returned.publish_new(&xml).is_err());
}
#[test]
fn stripped_provenance_disk_route_matches_the_public_typed_memory_route() {
    let root = tempfile::tempdir().unwrap();
    let generated = xml_to_edt(&source(), &options()).unwrap().tree;
    let stripped = SourceTree::new(
        generated
            .entries()
            .iter()
            .filter(|entry| !entry.path().as_str().starts_with(".ibcmd-provenance/"))
            .cloned()
            .collect(),
    )
    .unwrap();
    let memory = edt_to_xml(&Project::from_tree(stripped.clone()).unwrap(), &options()).unwrap();
    let edt = root.path().join("edt");
    publish_new_with_limits(&stripped, &edt, ReaderLimits::default()).unwrap();
    let directory = read_directory_source(&edt)
        .unwrap()
        .edt_to_xml(&options())
        .unwrap();
    let xml = root.path().join("xml");
    directory.publish_new(&xml).unwrap();
    assert_eq!(
        read_xml_source(xml, ReaderLimits::default()).unwrap(),
        memory.tree
    );
}
#[test]
fn unknown_source_files_and_changed_or_forged_provenance_are_not_waived() {
    let root = tempfile::tempdir().unwrap();
    let native = root.path().join("native");
    publish_new_with_limits(&source(), &native, ReaderLimits::default()).unwrap();
    std::fs::write(native.join("unclaimed.bin"), b"unsupported body").unwrap();
    assert!(
        read_directory_source(&native)
            .unwrap()
            .xml_to_edt(&options())
            .is_err()
    );
    let edt = root.path().join("edt");
    let generated = xml_to_edt(&source(), &options()).unwrap();
    publish_new_with_limits(&generated.tree, &edt, ReaderLimits::default()).unwrap();
    std::fs::write(
        edt.join("DT-INF/PROJECT.PMF"),
        b"Manifest-Version: 1.0\r\nRuntime-Version: 8.5.1\r\n\r\n",
    )
    .unwrap();
    assert!(
        read_directory_source(&edt)
            .unwrap()
            .edt_to_xml(&options())
            .unwrap_err()
            .to_string()
            .contains("stale provenance")
    );
    let manifest = edt.join(".ibcmd-provenance/manifest.json");
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&manifest).unwrap()).unwrap();
    use sha2::{Digest, Sha256};
    value["generated"]["DT-INF/PROJECT.PMF"] = serde_json::Value::String(format!(
        "{:x}",
        Sha256::digest(std::fs::read(edt.join("DT-INF/PROJECT.PMF")).unwrap())
    ));
    // Forging a lexical-only PMF edit is still permitted only because complete
    // current typed semantics and the independently decoded original match.
    std::fs::write(&manifest, serde_json::to_vec(&value).unwrap()).unwrap();
    let restored = read_directory_source(&edt)
        .unwrap()
        .edt_to_xml(&options())
        .unwrap();
    assert_eq!(
        serde_json::to_value(&restored.canonical).unwrap(),
        serde_json::to_value(&generated.canonical).unwrap()
    );
    std::fs::write(
        edt.join(".ibcmd-provenance/xml/unknown.bin"),
        b"extra original",
    )
    .unwrap();
    assert!(
        read_directory_source(&edt)
            .unwrap()
            .edt_to_xml(&options())
            .is_err()
    );
}
#[test]
fn forged_generated_hash_cannot_hide_a_real_metadata_edit() {
    let root = tempfile::tempdir().unwrap();
    let edt = root.path().join("edt");
    let generated = xml_to_edt(&source(), &options()).unwrap();
    publish_new_with_limits(&generated.tree, &edt, ReaderLimits::default()).unwrap();
    let path = "src/Configuration/Configuration.mdo";
    let original = std::fs::read_to_string(edt.join(path)).unwrap();
    let edited = original.replace(
        "<name>Конфигурация</name>",
        "<name>EditedConfiguration</name>",
    );
    assert_ne!(original, edited);
    std::fs::write(edt.join(path), &edited).unwrap();
    let manifest_path = edt.join(".ibcmd-provenance/manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
    use sha2::{Digest, Sha256};
    manifest["generated"][path] =
        serde_json::Value::String(format!("{:x}", Sha256::digest(edited.as_bytes())));
    std::fs::write(manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    assert!(
        read_directory_source(&edt)
            .unwrap()
            .edt_to_xml(&options())
            .unwrap_err()
            .to_string()
            .contains("stale provenance")
    );
}
#[test]
fn qualified_or_namespace_rebound_eclipse_controls_are_not_consumed_by_local_name() {
    let root = tempfile::tempdir().unwrap();
    let generated = xml_to_edt(&source(), &options()).unwrap().tree;
    let stripped = SourceTree::new(
        generated
            .entries()
            .iter()
            .filter(|entry| !entry.path().as_str().starts_with(".ibcmd-provenance/"))
            .cloned()
            .collect(),
    )
    .unwrap();
    let original = std::str::from_utf8(
        stripped
            .entries()
            .iter()
            .find(|entry| entry.path().as_str() == ".project")
            .unwrap()
            .bytes(),
    )
    .unwrap();
    let cases = [
        original.replace("projectDescription", "xml:projectDescription"),
        original
            .replace("<name>", "<xml:name>")
            .replace("</name>", "</xml:name>"),
        original.replace("projectDescription", "x:projectDescription"),
        original
            .replace(
                "<projectDescription>",
                "<x:projectDescription xmlns:x='urn:custom'>",
            )
            .replace("</projectDescription>", "</x:projectDescription>"),
        original.replace(
            "<projectDescription>",
            "<projectDescription xmlns='urn:custom'>",
        ),
        original
            .replace("<name>", "<x:name>")
            .replace("</name>", "</x:name>"),
        original.replace("<buildSpec>", "<buildSpec xmlns='urn:custom'>"),
    ];
    for (index, project) in cases.iter().enumerate() {
        let path = root.path().join(format!("case-{index}"));
        publish_new_with_limits(&stripped, &path, ReaderLimits::default()).unwrap();
        std::fs::write(path.join(".project"), project).unwrap();
        assert!(
            read_directory_source(&path)
                .unwrap()
                .edt_to_xml(&options())
                .is_err()
        );
        let memory = read_xml_source(&path, ReaderLimits::default()).unwrap();
        assert!(edt_to_xml(&Project::from_tree(memory).unwrap(), &options()).is_err());
    }
}
