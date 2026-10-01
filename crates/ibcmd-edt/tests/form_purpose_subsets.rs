use ibcmd_edt::{
    ConversionOptions, Project, ReaderLimits, edt_to_xml, read_xml_source, xml_to_edt,
};
use ibcmd_xml::source_tree::{SourceEntry, SourcePath, SourceTree};
use morph1c_core::{
    ir::{FormBody, MetadataObject, NamedFormBody, ObjectKind, PropertyValue, Uuid},
    spec::metadata::report_form_ref,
    version::{FormatVersion, with_roundtrip_target},
};
use morph1c_pipeline::{
    ConvertOptions, Format, read_config, registry::FormatRegistry, write_config,
};
use sha2::{Digest, Sha256};

const EDT_PATH: &str = "src/Reports/PurposeReport/PurposeReport.mdo";

fn source(purposes: &[&str]) -> SourceTree {
    source_version(purposes, 21)
}

fn source_version(purposes: &[&str], minor: u16) -> SourceTree {
    let options = ConvertOptions::default().with_target_version(FormatVersion::new(2, minor));
    let mut config = read_config(
        Format::Designer,
        std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/subsystem-ci/src"
        )),
        &options,
    )
    .unwrap()
    .0;
    let mut owner =
        MetadataObject::new(ObjectKind::new("Report"), "PurposeReport", Uuid([0x51; 16]));
    let mut reference = MetadataObject::new(
        ObjectKind::new("Report.FormRef"),
        "Choice",
        Uuid([0x52; 16]),
    );
    reference.properties.push((
        report_form_ref::F_USE_PURPOSES,
        PropertyValue::List(
            purposes
                .iter()
                .map(|value| PropertyValue::Str((*value).into()))
                .collect(),
        ),
    ));
    owner.children.push(reference);
    owner.form_bodies.push(NamedFormBody {
        name: "Choice".into(),
        body: FormBody::new(),
        ordinary_body: None,
        module: None,
        help: vec![],
        help_resources: vec![],
    });
    config.objects.push(owner);
    let directory = tempfile::tempdir().unwrap();
    with_roundtrip_target(FormatVersion::new(2, minor), || {
        write_config(Format::Designer, &config, directory.path()).unwrap();
    });
    let root_path = directory.path().join("Configuration.xml");
    let root = String::from_utf8(std::fs::read(&root_path).unwrap())
        .unwrap()
        .replace(
            "</Language>",
            "</Language>\r\n\t\t\t<Report>PurposeReport</Report>",
        );
    std::fs::write(root_path, root).unwrap();
    read_xml_source(directory.path(), ReaderLimits::default()).unwrap()
}
fn options() -> ConversionOptions {
    version_options(21)
}

fn version_options(minor: u16) -> ConversionOptions {
    ConversionOptions {
        edt_version: "2025.2.3".into(),
        xml_dialect: format!("2.{minor}"),
        runtime_version: Some(if minor == 20 { "8.3.27" } else { "8.5.1" }.into()),
    }
}
fn strip(tree: &SourceTree) -> SourceTree {
    SourceTree::new(
        tree.entries()
            .iter()
            .filter(|entry| !entry.path().as_str().starts_with(".ibcmd-provenance/"))
            .cloned()
            .collect(),
    )
    .unwrap()
}
fn replace(tree: &SourceTree, path: &str, bytes: Vec<u8>) -> SourceTree {
    let mut entries = tree
        .entries()
        .iter()
        .filter(|entry| entry.path().as_str() != path)
        .cloned()
        .collect::<Vec<_>>();
    entries.push(SourceEntry::from_bytes(SourcePath::new(path).unwrap(), bytes).unwrap());
    SourceTree::new(entries).unwrap()
}
fn native_purposes(tree: &SourceTree) -> PropertyValue {
    let bytes = tree
        .entries()
        .iter()
        .find(|entry| entry.path().as_str() == "Reports/PurposeReport/Forms/Choice.xml")
        .unwrap()
        .bytes();
    let document = formats_xml::parse(bytes).unwrap();
    let form = document
        .root
        .children
        .iter()
        .find(|node| node.local == "Form")
        .unwrap();
    let properties = form
        .children
        .iter()
        .find(|node| node.local == "Properties")
        .unwrap();
    let host = properties
        .children
        .iter()
        .find(|node| node.local == "UsePurposes")
        .unwrap();
    match formats_xml::configuration::decode_use_purposes_designer(host) {
        morph1c_core::engine::Decoded::Present(value) => value,
        _ => panic!("typed native application purposes required"),
    }
}

#[test]
fn actual_report_form_purposes_survive_both_routes_without_provenance() {
    for minor in [20, 21] {
        let options = version_options(minor);
        for purposes in [
            &[][..],
            &["PersonalComputer"][..],
            &["MobileDevice"][..],
            &["PersonalComputer", "MobileDevice"][..],
            &["MobileDevice", "PersonalComputer"][..],
        ] {
            let original = source_version(purposes, minor);
            let expected = native_purposes(&original);
            let generated = xml_to_edt(&original, &options).unwrap().tree;
            let bytes = generated
                .entries()
                .iter()
                .find(|entry| entry.path().as_str() == EDT_PATH)
                .unwrap()
                .bytes();
            let registry = FormatRegistry::for_format(Format::Edt).unwrap();
            let owner = (registry.get("Report").unwrap().read)(bytes).unwrap();
            assert_eq!(
                owner.children[0].get(report_form_ref::F_USE_PURPOSES),
                Some(&expected)
            );
            assert_eq!(
                edt_to_xml(&Project::from_tree(generated.clone()).unwrap(), &options)
                    .unwrap()
                    .tree,
                original
            );
            let regenerated = edt_to_xml(&Project::from_tree(strip(&generated)).unwrap(), &options)
                .unwrap()
                .tree;
            assert_eq!(native_purposes(&regenerated), expected);
        }
    }
}

#[test]
fn form_purpose_edit_cannot_replay_rehashed_old_source() {
    let generated = xml_to_edt(&source(&["PersonalComputer"]), &options())
        .unwrap()
        .tree;
    let descriptor = generated
        .entries()
        .iter()
        .find(|entry| entry.path().as_str() == EDT_PATH)
        .unwrap()
        .bytes();
    let edited = String::from_utf8(descriptor.to_vec())
        .unwrap()
        .replace(
            "<usePurposes>PersonalComputer</usePurposes>",
            "<usePurposes>MobileDevice</usePurposes>",
        )
        .into_bytes();
    assert_ne!(edited, descriptor);
    let changed = replace(&generated, EDT_PATH, edited.clone());
    let mut manifest: serde_json::Value = serde_json::from_slice(
        changed
            .entries()
            .iter()
            .find(|entry| entry.path().as_str() == ".ibcmd-provenance/manifest.json")
            .unwrap()
            .bytes(),
    )
    .unwrap();
    manifest["generated"][EDT_PATH] =
        serde_json::Value::String(format!("{:x}", Sha256::digest(&edited)));
    let forged = replace(
        &changed,
        ".ibcmd-provenance/manifest.json",
        serde_json::to_vec(&manifest).unwrap(),
    );
    assert!(edt_to_xml(&Project::from_tree(forged).unwrap(), &options()).is_err());
    let current = edt_to_xml(&Project::from_tree(strip(&changed)).unwrap(), &options())
        .unwrap()
        .tree;
    assert_eq!(
        native_purposes(&current),
        PropertyValue::List(vec![PropertyValue::Str("MobileDevice".into())])
    );
}

#[test]
fn malformed_or_unknown_form_purposes_fail_without_provenance() {
    let generated = xml_to_edt(&source(&["PersonalComputer"]), &options())
        .unwrap()
        .tree;
    let descriptor = generated
        .entries()
        .iter()
        .find(|entry| entry.path().as_str() == EDT_PATH)
        .unwrap()
        .bytes();
    let text = std::str::from_utf8(descriptor).unwrap();
    for changed in [
        text.replace(
            "<usePurposes>PersonalComputer</usePurposes>",
            "<usePurposes>UnknownPurpose</usePurposes>",
        ),
        text.replace(
            "<usePurposes>PersonalComputer</usePurposes>",
            "<usePurposes>PlatformApplication</usePurposes>",
        ),
        text.replace("<usePurposes>", "<usePurposes extra='yes'>"),
        text.replace(
            "<usePurposes>PersonalComputer</usePurposes>",
            "<usePurposes><unknown/></usePurposes>",
        ),
    ] {
        assert_ne!(changed.as_bytes(), descriptor);
        let current = replace(&strip(&generated), EDT_PATH, changed.into_bytes());
        assert!(edt_to_xml(&Project::from_tree(current).unwrap(), &options()).is_err());
    }
}
