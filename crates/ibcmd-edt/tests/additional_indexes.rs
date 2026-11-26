use formats_xml::additional_indexes::{read, write};
use ibcmd_edt::{
    ConversionOptions, Project, ReaderLimits, edt_to_xml, read_directory_source, read_xml_source,
    xml_to_edt,
};
use ibcmd_xml::source_tree::{SourceEntry, SourcePath, SourceTree};
use morph1c_core::ir::{
    MetadataObject, Module, ModuleBody, ObjectKind, PropertyValue, Uuid,
    additional_indexes::AdditionalIndex,
};
use morph1c_core::version::{FormatVersion, with_roundtrip_target, with_source_version};
use morph1c_pipeline::{ConvertOptions, Format, read_config, write_config};
use std::path::{Path, PathBuf};

const OWNER: &str = "СтроковыеКонтактыВзаимодействий";
const XML: &str = "Catalogs/СтроковыеКонтактыВзаимодействий/Ext/AdditionalIndexes.xml";
const EDT: &str = "src/Catalogs/СтроковыеКонтактыВзаимодействий/AdditionalIndexes.aindex";
fn indexes() -> Vec<AdditionalIndex> {
    vec![
        AdditionalIndex {
            id: Uuid([0x35; 16]),
            name: "Current & Index".into(),
            table: format!("Catalog.{OWNER}"),
            indexed_fields: vec!["Code".into(), "Description".into(), "Code".into()],
            additional_fields: vec!["Ref".into()],
        },
        AdditionalIndex {
            id: Uuid([0x36; 16]),
            name: "Second".into(),
            table: format!("Catalog.{OWNER}"),
            indexed_fields: vec!["Description".into()],
            additional_fields: vec![],
        },
    ]
}
fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/subsystem-ci/src")
}
fn options(minor: u16) -> ConversionOptions {
    ConversionOptions {
        edt_version: "2025.2.3".into(),
        xml_dialect: format!("2.{minor}"),
        runtime_version: Some(if minor == 20 { "8.3.27" } else { "8.5.1" }.into()),
    }
}
fn bytes(indexes: &[AdditionalIndex], format: Format, minor: u16) -> Vec<u8> {
    with_roundtrip_target(FormatVersion::new(2, minor), || {
        write(indexes, format).unwrap()
    })
}
fn replace(tree: &SourceTree, path: &str, bytes: Vec<u8>) -> SourceTree {
    let mut entries = tree
        .entries()
        .iter()
        .filter(|e| e.path().as_str() != path)
        .cloned()
        .collect::<Vec<_>>();
    entries.push(SourceEntry::from_bytes(SourcePath::new(path).unwrap(), bytes).unwrap());
    SourceTree::new(entries).unwrap()
}
fn strip(tree: &SourceTree) -> SourceTree {
    SourceTree::new(
        tree.entries()
            .iter()
            .filter(|e| !e.path().as_str().starts_with(".ibcmd-provenance/"))
            .cloned()
            .collect(),
    )
    .unwrap()
}
fn entry<'a>(tree: &'a SourceTree, path: &str) -> &'a [u8] {
    tree.entries()
        .iter()
        .find(|e| e.path().as_str() == path)
        .unwrap()
        .bytes()
}
fn native(minor: u16, indexes: Option<Vec<AdditionalIndex>>) -> SourceTree {
    let mut config = read_config(Format::Designer, &fixture(), &ConvertOptions::default())
        .unwrap()
        .0;
    config
        .objects
        .iter_mut()
        .find(|obj| obj.name == OWNER)
        .unwrap()
        .additional_indexes = indexes;
    let root = tempfile::tempdir().unwrap();
    with_roundtrip_target(FormatVersion::new(2, minor), || {
        write_config(Format::Designer, &config, root.path())
    })
    .unwrap();
    read_xml_source(root.path(), ReaderLimits::default()).unwrap()
}

#[test]
fn both_index_syntaxes_preserve_all_current_values_order_and_target_version() {
    for minor in [20, 21] {
        for format in [Format::Designer, Format::Edt] {
            for source in [vec![], indexes()] {
                let encoded = bytes(&source, format, minor);
                assert_eq!(read(&encoded, format).unwrap(), source);
                if format == Format::Designer {
                    assert!(
                        String::from_utf8_lossy(&encoded)
                            .contains(&format!("version=\"2.{minor}\""))
                    );
                }
            }
        }
        let original = String::from_utf8(bytes(&indexes(), Format::Designer, minor)).unwrap();
        let alias = original
            .replace(
                "xmlns=\"http://v8.1c.ru/8.3/xcf/extrnprops\"",
                "xmlns:q=\"http://v8.1c.ru/8.3/xcf/extrnprops\"",
            )
            .replace("<Additional", "<q:Additional")
            .replace("</Additional", "</q:Additional")
            .replace("<Name", "<q:Name")
            .replace("</Name", "</q:Name")
            .replace("<Table", "<q:Table")
            .replace("</Table", "</q:Table")
            .replace("<Indexed", "<q:Indexed")
            .replace("</Indexed", "</q:Indexed")
            .replace("<Field", "<q:Field")
            .replace("</Field", "</q:Field");
        assert_eq!(read(alias.as_bytes(), Format::Designer).unwrap(), indexes());
    }
}

#[test]
fn additional_index_unknown_cells_namespace_versions_and_tail_are_refused() {
    for format in [Format::Designer, Format::Edt] {
        let valid = String::from_utf8(bytes(&indexes(), format, 21)).unwrap();
        let root_end = if format == Format::Edt {
            "</aindex:AdditionalIndexes>"
        } else {
            "</AdditionalIndexes>"
        };
        let name = if format == Format::Edt {
            "name"
        } else {
            "Name"
        };
        for changed in [
            valid.replace(root_end, &format!("<unknown/>{root_end}")),
            valid.replace(&format!("<{name}>"), &format!("<{name} unknown=\"true\">")),
            valid.replace(
                &format!("<{name}>"),
                &format!("<{name}><unknown/></{name}><{name}>"),
            ),
            valid
                .replace("http://v8.1c.ru/8.3/xcf/extrnprops", "urn:wrong")
                .replace("http://g5.1c.ru/v8/dt/md/aindex", "urn:wrong"),
            format!("{valid}<another/>"),
            format!("{valid}trailing-value"),
        ] {
            assert!(read(changed.as_bytes(), format).is_err(), "{format:?}");
        }
    }
    let xml = String::from_utf8(bytes(&indexes(), Format::Designer, 21)).unwrap();
    assert!(
        read(
            xml.replace("version=\"2.21\"", "version=\"9.99\"")
                .as_bytes(),
            Format::Designer
        )
        .is_err()
    );
    assert!(
        with_source_version(Some(FormatVersion::new(2, 20)), || read(
            xml.as_bytes(),
            Format::Designer
        ))
        .is_err()
    );
    let edt = String::from_utf8(bytes(&indexes(), Format::Edt, 21)).unwrap();
    assert!(
        read(
            edt.replace("<path>", "<path xmlns=\"http://g5.1c.ru/v8/dt/md/aindex\">")
                .as_bytes(),
            Format::Edt
        )
        .is_err()
    );
}

#[test]
fn public_index_routes_are_exact_and_edits_never_replay_old_index_values() {
    for minor in [20, 21] {
        for attachment in [None, Some(vec![]), Some(indexes())] {
            let original = native(minor, attachment);
            let options = options(minor);
            let generated = xml_to_edt(&original, &options).unwrap().tree;
            let returned = edt_to_xml(&Project::from_tree(generated.clone()).unwrap(), &options)
                .unwrap()
                .tree;
            assert_eq!(returned, original);
            let root = tempfile::tempdir().unwrap();
            ibcmd_xml::source_tree::publish_new(&original, root.path().join("source")).unwrap();
            let disk = read_directory_source(root.path().join("source"))
                .unwrap()
                .xml_to_edt(&options)
                .unwrap();
            disk.publish_new(root.path().join("edt")).unwrap();
            let disk = read_directory_source(root.path().join("edt"))
                .unwrap()
                .edt_to_xml(&options)
                .unwrap();
            disk.publish_new(root.path().join("returned")).unwrap();
            assert_eq!(
                read_xml_source(root.path().join("returned"), ReaderLimits::default()).unwrap(),
                original
            );
            if original.entries().iter().all(|e| e.path().as_str() != XML) {
                continue;
            }
            let current = read(entry(&generated, EDT), Format::Edt).unwrap();
            assert_eq!(
                read(entry(&original, XML), Format::Designer).unwrap(),
                current
            );
        }
        let original = native(minor, Some(indexes()));
        let options = options(minor);
        let generated = xml_to_edt(&original, &options).unwrap().tree;
        let mut edited = indexes();
        edited[0].name = "Changed current name".into();
        edited[0].id = Uuid([0x39; 16]);
        edited[0].indexed_fields.reverse();
        edited[0].additional_fields.push("Code".into());
        edited.reverse();
        let changed = replace(&generated, EDT, bytes(&edited, Format::Edt, minor));
        assert!(edt_to_xml(&Project::from_tree(changed.clone()).unwrap(), &options).is_err());
        let current = edt_to_xml(&Project::from_tree(strip(&changed)).unwrap(), &options)
            .unwrap()
            .tree;
        assert_eq!(
            read(entry(&current, XML), Format::Designer).unwrap(),
            edited
        );
        let mut invalid = edited;
        invalid[0].table = "Catalog.OtherOwner".into();
        let changed = replace(&generated, EDT, bytes(&invalid, Format::Edt, minor));
        assert!(edt_to_xml(&Project::from_tree(strip(&changed)).unwrap(), &options).is_err());
    }
}

#[test]
fn bot_and_sequence_module_slots_are_carried_by_the_actual_pipeline() {
    for minor in [20, 21] {
        let mut config = read_config(Format::Designer, &fixture(), &ConvertOptions::default())
            .unwrap()
            .0;
        let root = config
            .objects
            .iter_mut()
            .find(|obj| obj.kind.as_str() == "Configuration")
            .unwrap();
        let roster = root
            .properties
            .iter_mut()
            .find(|(field, _)| {
                *field == morph1c_core::spec::metadata::configuration::F_CHILD_OBJECTS
            })
            .unwrap();
        let PropertyValue::List(rows) = &mut roster.1 else {
            panic!("roster")
        };
        for (kind, name) in [("Bot", "Notifications"), ("Sequence", "Documents")] {
            rows.push(PropertyValue::List(vec![
                PropertyValue::Str(kind.into()),
                PropertyValue::Str(name.into()),
            ]));
        }
        for (kind, name, slot, id) in [
            ("Bot", "Notifications", "Module", 0x44),
            ("Sequence", "Documents", "RecordSetModule", 0x45),
        ] {
            let mut owner = MetadataObject::new(ObjectKind::new(kind), name, Uuid([id; 16]));
            owner.modules.push(Module {
                slot: slot.into(),
                body: ModuleBody::Text(
                    "// Current module\r\nПроцедура Тест()\r\nКонецПроцедуры\r\n".into(),
                ),
            });
            config.objects.push(owner);
        }
        let directory = tempfile::tempdir().unwrap();
        with_roundtrip_target(FormatVersion::new(2, minor), || {
            write_config(Format::Designer, &config, directory.path())
        })
        .unwrap();
        let source = read_xml_source(directory.path(), ReaderLimits::default()).unwrap();
        let options = options(minor);
        let generated = xml_to_edt(&source, &options).unwrap().tree;
        for path in [
            "src/Bots/Notifications/Module.bsl",
            "src/Sequences/Documents/RecordSetModule.bsl",
        ] {
            assert!(
                std::str::from_utf8(entry(&generated, path))
                    .unwrap()
                    .contains("Current module")
            );
        }
        assert_eq!(
            edt_to_xml(&Project::from_tree(generated.clone()).unwrap(), &options)
                .unwrap()
                .tree,
            source
        );
        let returned = edt_to_xml(&Project::from_tree(strip(&generated)).unwrap(), &options)
            .unwrap()
            .tree;
        for path in [
            "Bots/Notifications/Ext/Module.bsl",
            "Sequences/Documents/Ext/RecordSetModule.bsl",
        ] {
            assert_eq!(entry(&returned, path), entry(&source, path));
        }
    }
}

#[test]
#[ignore = "read-only hash-bound genuine UH index pairs on F"]
fn genuine_uh_index_pairs_preserve_complete_current_values() {
    use sha2::{Digest, Sha256};
    let lab = PathBuf::from(std::env::var_os("IBCMD_INDEX_LAB").unwrap());
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(lab.join("sources.json")).unwrap()).unwrap();
    let pairs = manifest.as_array().unwrap();
    assert_eq!(pairs.len(), 3);
    let mut records = Vec::new();
    for (position, pair) in pairs.iter().enumerate() {
        let mut original = Vec::new();
        for (key, format) in [("native", Format::Designer), ("edt", Format::Edt)] {
            let path = pair[key]["path"].as_str().unwrap();
            let source = std::fs::read(path).unwrap();
            assert_eq!(
                format!("{:x}", Sha256::digest(&source)),
                pair[key]["sha256"].as_str().unwrap()
            );
            let values = read(&source, format).unwrap();
            for minor in [20, 21] {
                for target in [Format::Designer, Format::Edt] {
                    assert_eq!(
                        read(&bytes(&values, target, minor), target).unwrap(),
                        values
                    );
                }
            }
            assert_eq!(std::fs::read(path).unwrap(), source);
            original.push(values);
        }
        assert_eq!(original[0], original[1]);
        records.push(serde_json::json!({"pair":position,"indexes":original[0].len(),
            "indexed_fields":original[0].iter().map(|index|index.indexed_fields.len()).sum::<usize>(),
            "additional_fields":original[0].iter().map(|index|index.additional_fields.len()).sum::<usize>(),
            "both_formats_and_profiles":"PASS","sources_unchanged":true}));
    }
    std::fs::write(
        lab.join("result.json"),
        serde_json::to_vec_pretty(&records).unwrap(),
    )
    .unwrap();
}
