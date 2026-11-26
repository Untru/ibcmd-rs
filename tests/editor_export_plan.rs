//! Staged editor export acceptance. Each fixture is removed with its guard,
//! also when the test fails (docs/testing.md, "Temporary directories").
mod common;

use ibcmd_rs::stored_objects::{ConfigSource, StoredConfiguration};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};
fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/file-infobase")
        .join(name)
}
fn setup() -> (common::Scratch, StoredConfiguration) {
    let root = common::Scratch::adopt(
        std::env::temp_dir().join(format!("editor-export-{}", uuid::Uuid::new_v4())),
    );
    fs::create_dir_all(root.join("db")).unwrap();
    let mut bytes = Vec::new();
    flate2::read::GzDecoder::new(fs::File::open(fixture("tiny.1CD.gz")).unwrap())
        .read_to_end(&mut bytes)
        .unwrap();
    let db = root.join("db/1Cv8.1CD");
    fs::write(&db, bytes).unwrap();
    fs::create_dir(root.join("source")).unwrap();
    fs::create_dir(root.join("stage")).unwrap();
    let config = StoredConfiguration::new(
        ConfigSource::FileDb(db),
        ibcmd_rs::platform::parse("8.5.1").unwrap(),
    );
    (root, config)
}
#[test]
fn selective_stage_matches_full_native_bytes_and_never_writes_source() {
    let (root, mut config) = setup();
    let source = root.join("source");
    let stage = root.join("stage");
    let info = fs::read(fixture("tiny-xml/ConfigDumpInfo.xml")).unwrap();
    fs::write(source.join("ConfigDumpInfo.xml"), &info).unwrap();
    fs::write(source.join("README.md"), b"unrelated").unwrap();
    let name = "CommonModule.ФайловаяБазаПроверка".to_owned();
    let plan = config
        .prepare_editor_export(&[name.clone()], false, &source, &stage, &|_, _, _| {})
        .unwrap();
    assert_eq!(plan.objects, vec![name]);
    assert!(plan.files.iter().any(|n| n.ends_with("Module.bsl")));
    for path in plan.files {
        assert_eq!(
            fs::read(stage.join(&path)).unwrap(),
            fs::read(fixture("tiny-xml").join(path)).unwrap()
        );
    }
    assert_eq!(fs::read(source.join("ConfigDumpInfo.xml")).unwrap(), info);
    assert_eq!(fs::read(source.join("README.md")).unwrap(), b"unrelated");
    assert_eq!(fs::read_dir(&source).unwrap().count(), 2);
}
#[test]
fn changed_is_native_version_based_and_preserves_noop_inventory() {
    let (root, mut config) = setup();
    let source = root.join("source");
    let stage = root.join("stage");
    let info = fs::read(fixture("tiny-xml/ConfigDumpInfo.xml")).unwrap();
    fs::write(source.join("ConfigDumpInfo.xml"), &info).unwrap();
    let plan = config
        .prepare_editor_export(&[], true, &source, &stage, &|_, _, _| {})
        .unwrap();
    assert!(plan.objects.is_empty());
    assert!(plan.files.is_empty());
    assert!(plan.removed.is_empty());
    let changed = String::from_utf8(info)
        .unwrap()
        .replace("a86111168911c14b90b4fc704b63ebe600000000", &"0".repeat(40));
    fs::write(source.join("ConfigDumpInfo.xml"), changed).unwrap();
    let plan = config
        .prepare_editor_export(&[], true, &source, &stage, &|_, _, _| {})
        .unwrap();
    assert!(!plan.objects.is_empty());
    assert_eq!(
        fs::read(stage.join("ConfigDumpInfo.xml")).unwrap(),
        fs::read(fixture("tiny-xml/ConfigDumpInfo.xml")).unwrap()
    );
}
#[test]
fn deleted_descriptor_and_assets_are_planned_but_never_deleted() {
    let (root, mut config) = setup();
    let source = root.join("source");
    let stage = root.join("stage");
    let mut info = ibcmd_xml::config_dump_info::ConfigDumpInfo::parse(
        &fs::read(fixture("tiny-xml/ConfigDumpInfo.xml")).unwrap(),
    )
    .unwrap();
    info.entries.push(ibcmd_xml::config_dump_info::Entry {
        name: "CommonModule.Deleted".into(),
        id: "00000000-0000-0000-0000-000000000999".into(),
        config_version: "gone".into(),
        children: vec![],
    });
    fs::write(
        source.join("ConfigDumpInfo.xml"),
        info.to_native_bytes().unwrap(),
    )
    .unwrap();
    fs::create_dir_all(source.join("CommonModules/Deleted/Ext")).unwrap();
    fs::write(source.join("CommonModules/Deleted.xml"), b"old").unwrap();
    fs::write(
        source.join("CommonModules/Deleted/Ext/Module.bsl"),
        b"old module",
    )
    .unwrap();
    fs::write(
        source.join("CommonModules/Deleted/.ibcmd-note"),
        b"unrelated note",
    )
    .unwrap();
    let plan = config
        .prepare_editor_export(&[], true, &source, &stage, &|_, _, _| {})
        .unwrap();
    assert!(!plan.removed.iter().any(|p| p.contains(".ibcmd-note")));
    assert!(plan.removed.contains(&"CommonModules/Deleted.xml".into()));
    assert!(
        plan.removed
            .contains(&"CommonModules/Deleted/Ext/Module.bsl".into())
    );
    assert!(source.join("CommonModules/Deleted.xml").exists());
    assert_eq!(
        fs::read(stage.join("ConfigDumpInfo.xml")).unwrap(),
        fs::read(fixture("tiny-xml/ConfigDumpInfo.xml")).unwrap()
    );
}
#[test]
fn overlapping_stage_is_rejected_before_writing() {
    let (root, mut config) = setup();
    let source = root.join("source");
    let stage = source.join("nested");
    fs::create_dir(&stage).unwrap();
    assert!(
        config
            .prepare_editor_export(
                &["Configuration".into()],
                false,
                &source,
                &stage,
                &|_, _, _| {}
            )
            .is_err()
    );
    assert_eq!(fs::read_dir(stage).unwrap().count(), 0);
}

fn tree(root: &Path) -> std::collections::BTreeMap<String, Vec<u8>> {
    walkdir::WalkDir::new(root)
        .into_iter()
        .map(Result::unwrap)
        .filter(|e| e.file_type().is_file())
        .map(|e| {
            (
                e.path()
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/"),
                fs::read(e.path()).unwrap(),
            )
        })
        .collect()
}
#[test]
fn changed_add_modify_delete_and_rename_reconcile_to_native_full_inventory() {
    for case in ["add", "change", "delete", "rename"] {
        let (root, _) = setup();
        let source = root.join("source");
        let stage = root.join("stage");
        let native = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/native-export-commands")
            .join(case);
        let mut db = Vec::new();
        flate2::read::GzDecoder::new(fs::File::open(native.join("database.1CD.gz")).unwrap())
            .read_to_end(&mut db)
            .unwrap();
        fs::write(root.join("db/1Cv8.1CD"), db).unwrap();
        let mut files = tree(&fixture("tiny-xml"));
        for (name, bytes) in &files {
            let path = source.join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, bytes).unwrap();
        }
        fs::write(source.join("README.md"), b"unrelated").unwrap();
        let mut config = StoredConfiguration::new(
            ConfigSource::FileDb(root.join("db/1Cv8.1CD")),
            ibcmd_rs::platform::parse("8.5.1").unwrap(),
        );
        let plan = config
            .prepare_editor_export(&[], true, &source, &stage, &|_, _, _| {})
            .unwrap();
        for name in &plan.removed {
            files.remove(name);
        }
        for name in &plan.files {
            files.insert(name.clone(), fs::read(stage.join(name)).unwrap());
        }
        assert_eq!(files, tree(&native.join("full")), "case {case}");
        assert_eq!(fs::read(source.join("README.md")).unwrap(), b"unrelated");
    }
}
#[test]
fn group_and_multiselection_produce_same_object_bytes_and_inventory() {
    for selectors in [
        vec!["Configuration/CommonModules".to_owned()],
        vec![
            "CommonModule.ФайловаяБазаПроверка".to_owned(),
            "CommonModule.ФайловаяБазаПроверка".to_owned(),
        ],
    ] {
        let (root, mut config) = setup();
        let source = root.join("source");
        let stage = root.join("stage");
        fs::write(
            source.join("ConfigDumpInfo.xml"),
            fs::read(fixture("tiny-xml/ConfigDumpInfo.xml")).unwrap(),
        )
        .unwrap();
        let plan = config
            .prepare_editor_export(&selectors, false, &source, &stage, &|_, _, _| {})
            .unwrap();
        assert_eq!(plan.objects, vec!["CommonModule.ФайловаяБазаПроверка"]);
        for file in plan.files {
            assert_eq!(
                fs::read(stage.join(&file)).unwrap(),
                fs::read(fixture("tiny-xml").join(&file)).unwrap(),
                "{file}"
            );
        }
    }
}
#[test]
fn partial_inventory_keeps_unselected_version_and_never_advertises_it_current() {
    let (root, mut config) = setup();
    let source = root.join("source");
    let stage = root.join("stage");
    let mut info = ibcmd_xml::config_dump_info::ConfigDumpInfo::parse(
        &fs::read(fixture("tiny-xml/ConfigDumpInfo.xml")).unwrap(),
    )
    .unwrap();
    let language = info
        .entries
        .iter_mut()
        .find(|e| e.name == "Language.English")
        .unwrap();
    language.config_version = "old-language".into();
    fs::write(
        source.join("ConfigDumpInfo.xml"),
        info.to_native_bytes().unwrap(),
    )
    .unwrap();
    config
        .prepare_editor_export(
            &["CommonModule.ФайловаяБазаПроверка".into()],
            false,
            &source,
            &stage,
            &|_, _, _| {},
        )
        .unwrap();
    let after = ibcmd_xml::config_dump_info::ConfigDumpInfo::parse(
        &fs::read(stage.join("ConfigDumpInfo.xml")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        after
            .entries
            .iter()
            .find(|e| e.name == "Language.English")
            .unwrap()
            .config_version,
        "old-language"
    );
}

#[path = "common/selected_export_fixture.rs"]
mod selected_fixture;

#[test]
fn incomplete_maps_never_plan_removals_or_publish_even_after_observational_cache_hits() {
    let mut failures = Vec::new();
    for asset in [false, true] {
        for cached in [false, true] {
            for direct in [false, true] {
                let (root, mut config) = setup();
                let db = root.join("db/1Cv8.1CD");
                if asset {
                    selected_fixture::install_unadmitted_form_asset(&db, true);
                } else {
                    selected_fixture::install_unadmitted_common_module_descriptor(&db);
                }
                let name = if asset {
                    "CommonForm.OpaqueForm"
                } else {
                    "CommonModule.ФайловаяБазаПроверка"
                }
                .to_owned();
                let source = root.join("source");
                let stage = root.join("stage");
                for path in if asset {
                    vec![
                        "CommonForms/OpaqueForm.xml",
                        "CommonForms/OpaqueForm/Ext/Form.xml",
                        "CommonForms/OpaqueForm/Ext/Form/Module.bsl",
                    ]
                } else {
                    vec![
                        "CommonModules/ФайловаяБазаПроверка.xml",
                        "CommonModules/ФайловаяБазаПроверка/Ext/Module.bsl",
                    ]
                } {
                    let target = source.join(path);
                    fs::create_dir_all(target.parent().unwrap()).unwrap();
                    fs::write(target, b"keep locally edited selected file").unwrap();
                }
                fs::write(
                    source.join("ConfigDumpInfo.xml"),
                    config.native_config_dump_info().unwrap(),
                )
                .unwrap();
                fs::write(source.join("sentinel.txt"), b"keep unrelated file").unwrap();
                let before = tree(&source);
                let database = fs::read(&db).unwrap();
                let view = if direct {
                    &mut config
                } else {
                    config.main_view_mut()
                };
                if cached {
                    view.export(&[name.clone(), "Language.English".into()], &|_, _, _| {})
                        .unwrap();
                }
                let result = if direct {
                    config
                        .export_into(
                            &[name.clone(), "Language.English".into()],
                            &source,
                            &|_, _, _| {},
                        )
                        .map(|_| ())
                } else {
                    config
                        .prepare_editor_export(
                            &[name.clone(), "Language.English".into()],
                            false,
                            &source,
                            &stage,
                            &|_, _, _| {},
                        )
                        .map(|plan| {
                            eprintln!(
                                "unsafe plan removed={:?} files={:?}",
                                plan.removed, plan.files
                            );
                        })
                };
                let preserved = tree(&source) == before && tree(&stage).is_empty();
                eprintln!(
                    "editor completeness asset={asset} cached={cached} direct={direct}: refused={}, preserved={preserved}",
                    result.is_err()
                );
                if result.is_ok() || !preserved {
                    failures.push(format!("asset={asset} cached={cached} direct={direct}: refused={}, preserved={preserved}",result.is_err()));
                }
                assert_eq!(fs::read(&db).unwrap(), database);
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn accepted_aliases_produce_the_canonical_plan_and_version_map() {
    use ibcmd_rs::metadata_model::brace::Brace;
    fn rename(node: &mut Brace) {
        match node {
            Brace::Str(name) if name == "Configuration" => *name = "CustomConfiguration".into(),
            Brace::List(nodes) => nodes.iter_mut().for_each(rename),
            _ => {}
        }
    }
    for custom in [false, true] {
        let (root, mut config) = setup();
        if custom {
            selected_fixture::mutate_descriptor(
                &root.join("db/1Cv8.1CD"),
                "afb733a6-eae8-418b-b73c-fb9eb581e093",
                rename,
            );
        }
        let source = root.join("source");
        let current = config.native_config_dump_info().unwrap();
        let mut old = ibcmd_xml::config_dump_info::ConfigDumpInfo::parse(&current).unwrap();
        for entry in &mut old.entries {
            entry.config_version = "old-version".into();
        }
        fs::write(
            source.join("ConfigDumpInfo.xml"),
            old.to_native_bytes().unwrap(),
        )
        .unwrap();
        for aliases in [
            vec![
                "CommonModule.ФайловаяБазаПроверка",
                "commonmodule.файловаябазапроверка",
                "COMMONMODULE.ФайловаяБазаПроверка",
            ],
            vec![
                "Configuration",
                if custom {
                    "configuration.customconfiguration"
                } else {
                    "configuration.configuration"
                },
            ],
        ] {
            let mut reference = None;
            for (i, alias) in aliases.iter().enumerate() {
                let stage = root.join(format!("alias-{}-{i}", aliases[0]));
                fs::create_dir(&stage).unwrap();
                let plan = config
                    .prepare_editor_export(
                        &[(*alias).into()],
                        false,
                        &source,
                        &stage,
                        &|_, _, _| {},
                    )
                    .unwrap();
                let actual = (serde_json::to_value(&plan).unwrap(), tree(&stage));
                if let Some(expected) = &reference {
                    assert_eq!(&actual, expected, "alias={alias}, custom={custom}");
                } else {
                    reference = Some(actual);
                }
            }
        }
    }
}

#[test]
fn strict_editor_selection_keeps_unselected_losses_and_valid_legacy_fallback_outside_its_proof() {
    for legacy in [false, true] {
        let (root, mut config) = setup();
        let db = root.join("db/1Cv8.1CD");
        let name = if legacy {
            selected_fixture::install_legacy_fallback_object(&db, true, None)
        } else {
            selected_fixture::install_unadmitted_form_asset(&db, true);
            "Language.English".into()
        };
        // Populate a different cached object: completeness must cover each cache miss,
        // without expanding the selected proof to unsupported unselected rows.
        config
            .main_view_mut()
            .export(&["CommonModule.ФайловаяБазаПроверка".into()], &|_, _, _| {})
            .unwrap();
        let plan = config
            .prepare_editor_export(
                &[name],
                false,
                &root.join("source"),
                &root.join("stage"),
                &|_, _, _| {},
            )
            .unwrap();
        assert!(!plan.files.is_empty());
        if !legacy {
            assert_eq!(
                fs::read(root.join("stage/Languages/English.xml")).unwrap(),
                fs::read(fixture("tiny-xml/Languages/English.xml")).unwrap()
            );
        }
    }
}

fn install_owned_fixture(root: &Path, invalid: bool) {
    use ibcmd_rs::metadata_model::brace::Brace;
    let mut bytes = Vec::new();
    flate2::read::GzDecoder::new(
        fs::File::open(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/native-export-commands/owned/database.1CD.gz"),
        )
        .unwrap(),
    )
    .read_to_end(&mut bytes)
    .unwrap();
    let recorded = root.join("recorded.1CD");
    fs::write(&recorded, bytes).unwrap();
    let mut db = ibcmd_filedb::FileDb::open(&recorded).unwrap();
    let rows = db.config_rows("CONFIG").unwrap();
    drop(db);
    let target = root.join("db/1Cv8.1CD");
    // The export-only recorded collation stays untouched; mutation uses the
    // public own writable tiny database and the same metadata rows.
    ibcmd_filedb::replace_config_table(&target, "CONFIG", &rows).unwrap();
    if invalid {
        selected_fixture::mutate_descriptor(
            &target,
            "c099c30b-8a08-45b0-be7d-9594366e78a5",
            |descriptor| {
                let wrapper = descriptor.as_list_mut().unwrap()[1].as_list_mut().unwrap();
                let form = wrapper[1].as_list_mut().unwrap();
                assert_eq!(form[0].as_atom(), Some("13"));
                assert_eq!(form[2].as_atom(), Some("0"));
                form[2] = Brace::num(99);
            },
        );
    }
}
#[test]
fn nested_selected_semantics_are_checked_on_active_and_main_views_even_with_cached_files() {
    for cached in [false, true] {
        for direct in [false, true] {
            for recursive in [false, true] {
                let (root, mut config) = setup();
                install_owned_fixture(&root, true);
                let name = if recursive {
                    "Catalog.CorpusList"
                } else {
                    "Catalog.CorpusList.Form.ListForm"
                }
                .to_owned();
                let source = root.join("source");
                let stage = root.join("stage");
                let descriptor = source.join("Catalogs/CorpusList/Forms/ListForm.xml");
                fs::create_dir_all(descriptor.parent().unwrap()).unwrap();
                fs::write(descriptor, b"preserve local form descriptor").unwrap();
                fs::write(
                    source.join("ConfigDumpInfo.xml"),
                    config.native_config_dump_info().unwrap(),
                )
                .unwrap();
                let before = tree(&source);
                let view = if direct {
                    &mut config
                } else {
                    config.main_view_mut()
                };
                if cached {
                    view.export(&[name.clone()], &|_, _, _| {}).unwrap();
                }
                let result = if direct {
                    config
                        .export_into(&[name.clone()], &source, &|_, _, _| {})
                        .map(|_| ())
                } else {
                    config
                        .prepare_editor_export(
                            &[name.clone()],
                            false,
                            &source,
                            &stage,
                            &|_, _, _| {},
                        )
                        .map(|_| ())
                };
                eprintln!(
                    "nested editor export direct={direct} cached={cached} recursive={recursive}: {result:?}"
                );
                assert!(
                    result.is_err(),
                    "direct={direct} cached={cached} recursive={recursive}"
                );
                assert_eq!(tree(&source), before);
                assert!(tree(&stage).is_empty());
            }
        }
    }
}
#[test]
fn certified_and_uncertified_cache_entries_mix_without_losing_owned_recursion() {
    let (root, mut config) = setup();
    install_owned_fixture(&root, false);
    let names = vec!["Catalog.CorpusList".to_owned()];
    let source = root.join("source");
    let stage = root.join("stage");
    let expected = tree(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/native-export-commands/owned/recursive"),
    );
    let plan = config
        .prepare_editor_export(&names, false, &source, &stage, &|_, _, _| {})
        .unwrap();
    let actual = tree(&stage)
        .into_iter()
        .filter(|(path, _)| path != "ConfigDumpInfo.xml")
        .collect::<std::collections::BTreeMap<_, _>>();
    assert_eq!(actual, expected);
    let mut with_root = names.clone();
    with_root.push("Configuration".into());
    // First object is certified; the Configuration map was only observed.
    config
        .main_view_mut()
        .export(&["Configuration".into()], &|_, _, _| {})
        .unwrap();
    let next = root.join("next");
    fs::create_dir(&next).unwrap();
    let mixed = config
        .prepare_editor_export(&with_root, false, &source, &next, &|_, _, _| {})
        .unwrap();
    for path in plan
        .files
        .iter()
        .filter(|p| p.as_str() != "ConfigDumpInfo.xml")
    {
        assert_eq!(
            fs::read(stage.join(path)).unwrap(),
            fs::read(next.join(path)).unwrap()
        );
    }
    assert!(mixed.files.contains(&"Configuration.xml".into()));
    let cached = root.join("cached");
    fs::create_dir(&cached).unwrap();
    let conversions = std::cell::Cell::new(0);
    config
        .prepare_editor_export(&with_root, false, &source, &cached, &|_, _, message| {
            if message.starts_with("export of ") {
                conversions.set(conversions.get() + 1);
            }
        })
        .unwrap();
    assert_eq!(
        conversions.get(),
        0,
        "only a strict proof can be reused without conversion"
    );
    assert_eq!(tree(&cached), tree(&next));
}
