use morph1c_core::ir::{ConfigBlob, ParentConfigurationResource};
use morph1c_core::version::{FormatVersion, with_roundtrip_target};
use morph1c_pipeline::{ConvertOptions, Format, read_config, write_config};
use std::collections::BTreeMap;
use std::path::Path;

fn seed() -> morph1c_core::ir::Configuration {
    read_config(
        Format::Designer,
        Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/subsystem-ci/src"
        )),
        &ConvertOptions::default().with_target_version(FormatVersion::new(2, 21)),
    )
    .unwrap()
    .0
}
fn root(c: &mut morph1c_core::ir::Configuration) -> &mut morph1c_core::ir::MetadataObject {
    c.objects
        .iter_mut()
        .find(|o| o.kind.as_str() == "Configuration")
        .unwrap()
}
fn example() -> morph1c_core::ir::Configuration {
    let mut c = seed();
    let r = root(&mut c);
    r.config_blobs.push(ConfigBlob {
        slot: "ParentConfigurations".into(),
        bytes: vec![0xff, 0, 1, 2],
        mobile_signature_lexical: None,
    });
    r.parent_configuration_resources = vec![
        ParentConfigurationResource {
            path: "nested/opaque.xml".into(),
            bytes: b"<?xml bad <!DOCTYPE raw>\xff\0".to_vec(),
        },
        ParentConfigurationResource {
            path: "original.cf".into(),
            bytes: vec![0xff, 0, 3, 4],
        },
        ParentConfigurationResource {
            path: "plain.txt".into(),
            bytes: b"complete current opaque body".to_vec(),
        },
    ];
    for name in [
        "target/payload.xml",
        ".git/config",
        ".idea/raw.mdo",
        ".vscode/data.cf",
    ] {
        r.parent_configuration_resources
            .push(ParentConfigurationResource {
                path: name.into(),
                bytes: b"<?xml opaque invalid\xff".to_vec(),
            });
    }
    r.parent_configuration_resources
        .sort_by(|a, b| a.path.cmp(&b.path));
    c
}
fn write(
    format: Format,
    c: &morph1c_core::ir::Configuration,
    path: &Path,
    version: FormatVersion,
) -> Result<(), morph1c_pipeline::ConvertError> {
    with_roundtrip_target(version, || write_config(format, c, path)).map(|_| ())
}
fn prefix(format: Format) -> &'static str {
    if format == Format::Edt {
        "Configuration/ParentConfigurations"
    } else {
        "Ext/ParentConfigurations"
    }
}
fn files(path: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut result = BTreeMap::new();
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else {
                result.insert(
                    p.strip_prefix(path)
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .replace('\\', "/"),
                    std::fs::read(p).unwrap(),
                );
            }
        }
    }
    result
}
#[test]
fn recursive_opaque_resources_are_current_and_complete_in_both_dialects() {
    for version in [FormatVersion::new(2, 20), FormatVersion::new(2, 21)] {
        let mut c = example();
        let expected = root(&mut c).parent_configuration_resources.clone();
        for format in [Format::Designer, Format::Edt, Format::Designer] {
            let dir = tempfile::tempdir().unwrap();
            write(format, &c, dir.path(), version).unwrap();
            c = read_config(
                format,
                dir.path(),
                &ConvertOptions::default().with_target_version(version),
            )
            .unwrap()
            .0;
            assert_eq!(root(&mut c).parent_configuration_resources, expected);
        }
        let before = serde_json::to_vec(
            &morph1c_core::ir::semantic_view::ConfigurationSemanticView {
                configuration: &c,
                template_body: |_, _| Ok(None),
            },
        )
        .unwrap();
        root(&mut c)
            .parent_configuration_resources
            .iter_mut()
            .find(|r| r.path == "original.cf")
            .unwrap()
            .bytes[0] = 12;
        let after = serde_json::to_vec(
            &morph1c_core::ir::semantic_view::ConfigurationSemanticView {
                configuration: &c,
                template_body: |_, _| Ok(None),
            },
        )
        .unwrap();
        assert_ne!(before, after);
        let dir = tempfile::tempdir().unwrap();
        write(Format::Edt, &c, dir.path(), version).unwrap();
        assert_eq!(
            std::fs::read(dir.path().join(prefix(Format::Edt)).join("original.cf")).unwrap()[0],
            12
        );
    }
}
#[test]
fn parent_resources_require_unique_current_owner_and_safe_unambiguous_paths() {
    for path in [
        "../escape.cf",
        "a/../escape.cf",
        "/escape.cf",
        "a\\escape.cf",
        "a:escape.cf",
        "nul.cf",
        "trailing./file",
    ] {
        let mut c = example();
        root(&mut c).parent_configuration_resources[2].path = path.into();
        let out = tempfile::tempdir().unwrap();
        assert!(write(Format::Edt, &c, out.path(), FormatVersion::new(2, 21)).is_err());
        assert!(
            !out.path()
                .join(prefix(Format::Edt))
                .join("original.cf")
                .exists()
        );
    }
    for paths in [["A.cf", "a.cf"], ["a", "a/child.cf"]] {
        let mut c = example();
        let resources = &mut root(&mut c).parent_configuration_resources;
        resources.truncate(2);
        resources[0].path = paths[0].into();
        resources[1].path = paths[1].into();
        assert!(
            write(
                Format::Edt,
                &c,
                tempfile::tempdir().unwrap().path(),
                FormatVersion::new(2, 21)
            )
            .is_err()
        );
    }
    let mut c = example();
    root(&mut c).config_blobs.clear();
    assert!(
        write(
            Format::Edt,
            &c,
            tempfile::tempdir().unwrap().path(),
            FormatVersion::new(2, 21)
        )
        .is_err()
    );
    let mut c = example();
    let parent = root(&mut c).config_blobs[0].clone();
    root(&mut c).config_blobs.push(parent);
    assert!(
        write(
            Format::Edt,
            &c,
            tempfile::tempdir().unwrap().path(),
            FormatVersion::new(2, 21)
        )
        .is_err()
    );
    let native = tempfile::tempdir().unwrap();
    let c = seed();
    write(
        Format::Designer,
        &c,
        native.path(),
        FormatVersion::new(2, 21),
    )
    .unwrap();
    std::fs::create_dir_all(native.path().join(prefix(Format::Designer))).unwrap();
    std::fs::write(
        native
            .path()
            .join(prefix(Format::Designer))
            .join("orphan.cf"),
        b"unknown",
    )
    .unwrap();
    assert!(
        read_config(
            Format::Designer,
            native.path(),
            &ConvertOptions::default().with_target_version(FormatVersion::new(2, 21))
        )
        .is_err()
    );
}
#[test]
fn directory_exact_return_stripped_current_and_forged_hash_edit_are_distinct() {
    use ibcmd_edt::{ConversionOptions, read_directory_project, read_directory_source};
    use sha2::{Digest, Sha256};
    let c = example();
    let native = tempfile::tempdir().unwrap();
    write(
        Format::Designer,
        &c,
        native.path(),
        FormatVersion::new(2, 21),
    )
    .unwrap();
    let options = ConversionOptions {
        edt_version: "2025.2.3".into(),
        xml_dialect: "2.21".into(),
        runtime_version: Some("8.5.1".into()),
    };
    let source = read_directory_source(native.path()).unwrap();
    let edt = source.xml_to_edt(&options).unwrap();
    let owner = tempfile::tempdir().unwrap();
    let generated = owner.path().join("generated");
    edt.publish_new(&generated).unwrap();
    let project = read_directory_project(&generated).unwrap();
    let returned = project.edt_to_xml(&options).unwrap();
    let return_path = owner.path().join("returned");
    returned.publish_new(&return_path).unwrap();
    assert_eq!(files(native.path()), files(&return_path));
    let stripped = owner.path().join("stripped");
    for (p, b) in files(&generated)
        .into_iter()
        .filter(|(p, _)| !p.starts_with(".ibcmd-provenance/"))
    {
        let path = stripped.join(p);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b).unwrap();
    }
    let current = read_directory_project(&stripped)
        .unwrap()
        .edt_to_xml(&options)
        .unwrap();
    let current_path = owner.path().join("current");
    current.publish_new(&current_path).unwrap();
    assert_eq!(files(native.path()), files(&current_path));
    let edited = "src/Configuration/ParentConfigurations/original.cf";
    std::fs::write(generated.join(edited), b"CURRENT edited body").unwrap();
    let path = generated.join(".ibcmd-provenance/manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    manifest["generated"] = serde_json::Value::Object(
        files(&generated)
            .into_iter()
            .filter(|(p, _)| !p.starts_with(".ibcmd-provenance/"))
            .map(|(p, b)| {
                (
                    p,
                    serde_json::Value::String(format!("{:x}", Sha256::digest(b))),
                )
            })
            .collect(),
    );
    std::fs::write(path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    let rejected = read_directory_project(&generated)
        .unwrap()
        .edt_to_xml(&options);
    assert!(
        rejected.is_err(),
        "edited CURRENT resources cannot replay original bytes"
    );
    std::fs::write(stripped.join(edited), b"CURRENT edited body").unwrap();
    let current = read_directory_project(&stripped)
        .unwrap()
        .edt_to_xml(&options)
        .unwrap();
    let edit_path = owner.path().join("edited-current");
    current.publish_new(&edit_path).unwrap();
    assert_eq!(
        std::fs::read(edit_path.join("Ext/ParentConfigurations/original.cf")).unwrap(),
        b"CURRENT edited body"
    );
}
#[test]
#[ignore = "requires genuine immutable F: laboratory resources"]
fn genuine_parent_resource_matches_all_four_independent_sources_without_payload_reinterpretation() {
    use sha2::{Digest, Sha256};
    let lab = std::path::PathBuf::from(std::env::var_os("IBCMD_EDT_LAB").expect("lab"));
    let proof: serde_json::Value = serde_json::from_slice(
        &std::fs::read(lab.join("parent-configuration-resource-witness-r1.json")).unwrap(),
    )
    .unwrap();
    let mut expected = None;
    for source in proof["sources"].as_array().unwrap() {
        let dir = std::path::PathBuf::from(source["root"].as_str().unwrap());
        for row in source["entries"].as_array().unwrap() {
            let bytes = std::fs::read(dir.join(row["path"].as_str().unwrap())).unwrap();
            assert_eq!(bytes.len() as u64, row["bytes"].as_u64().unwrap());
            assert_eq!(
                format!("{:x}", Sha256::digest(&bytes)),
                row["sha256"].as_str().unwrap()
            );
            if let Some(e) = &expected {
                assert!(e == &bytes, "genuine resource bytes differ");
            } else {
                expected = Some(bytes);
            }
        }
    }
    let mut c = example();
    root(&mut c).parent_configuration_resources = vec![ParentConfigurationResource {
        path: proof["sources"][0]["entries"][0]["path"]
            .as_str()
            .unwrap()
            .into(),
        bytes: expected.unwrap(),
    }];
    for format in [Format::Designer, Format::Edt] {
        let out = tempfile::tempdir().unwrap();
        write(format, &c, out.path(), FormatVersion::new(2, 21)).unwrap();
        let mut read = read_config(
            format,
            out.path(),
            &ConvertOptions::default().with_target_version(FormatVersion::new(2, 21)),
        )
        .unwrap()
        .0;
        assert!(
            root(&mut c).parent_configuration_resources
                == root(&mut read).parent_configuration_resources,
            "genuine current resource path/payload did not survive"
        );
    }
}
#[test]
fn memory_exact_and_stripped_routes_keep_xml_named_parent_payloads_opaque() {
    use ibcmd_edt::{
        ConversionOptions, Project, ReaderLimits, edt_to_xml, read_xml_source, xml_to_edt,
    };
    let mut c = example();
    root(&mut c)
        .parent_configuration_resources
        .push(ParentConfigurationResource {
            path: "payload.mdo".into(),
            bytes: b"<!DOCTYPE opaque malformed".to_vec(),
        });
    let native = tempfile::tempdir().unwrap();
    write(
        Format::Designer,
        &c,
        native.path(),
        FormatVersion::new(2, 21),
    )
    .unwrap();
    let options = ConversionOptions {
        edt_version: "2025.2.3".into(),
        xml_dialect: "2.21".into(),
        runtime_version: Some("8.5.1".into()),
    };
    let source = read_xml_source(native.path(), ReaderLimits::default()).unwrap();
    let edt = xml_to_edt(&source, &options).unwrap();
    let returned = edt_to_xml(&Project::from_tree(edt.tree.clone()).unwrap(), &options).unwrap();
    assert_eq!(returned.tree, source);
    let stripped = ibcmd_xml::source_tree::SourceTree::new(
        edt.tree
            .entries()
            .iter()
            .filter(|e| !e.path().as_str().starts_with(".ibcmd-provenance/"))
            .cloned()
            .collect(),
    )
    .unwrap();
    let current = edt_to_xml(&Project::from_tree(stripped).unwrap(), &options).unwrap();
    assert_eq!(current.tree, source);
}
#[cfg(unix)]
#[test]
fn physical_backslash_filename_is_rejected_instead_of_becoming_a_directory() {
    let c = example();
    let source = tempfile::tempdir().unwrap();
    write(
        Format::Designer,
        &c,
        source.path(),
        FormatVersion::new(2, 21),
    )
    .unwrap();
    std::fs::write(
        source
            .path()
            .join(prefix(Format::Designer))
            .join("literal\\name.cf"),
        b"raw",
    )
    .unwrap();
    assert!(
        read_config(
            Format::Designer,
            source.path(),
            &ConvertOptions::default().with_target_version(FormatVersion::new(2, 21))
        )
        .is_err()
    );
}
