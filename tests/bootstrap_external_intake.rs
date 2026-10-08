//! Independently authored MIT inputs. These tests do not build native artifacts.
use ibcmd_core::{
    artifact::ProfileId,
    diagnostic::{ObjectPath, PathSegment},
    identity::ObjectUuid,
    model::{CanonicalObject, CanonicalObjectParts},
    value::{CanonicalField, CanonicalText, CanonicalValue},
};
use ibcmd_rs::compiler::{
    external_intake::{ExternalIntake, ExternalIntakeError},
    graph::BootstrapStorageOwner,
};
use ibcmd_schema::external_artifact::ExternalArtifactKind;
use ibcmd_xml::{
    XmlReader,
    metadata::{MetadataEnvelope, decode_external_root},
    source_tree::{
        ReaderLimits, SourceEntry, SourcePath, SourceTree, read_source_tree,
        read_source_tree_strict,
    },
};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
const MAIN: &str = "11000000-0000-4000-8000-000000000438";
const OBJECT: &str = "22000000-0000-4000-8000-000000000438";
const TYPE: &str = "33000000-0000-4000-8000-000000000438";
const VALUE: &str = "44000000-0000-4000-8000-000000000438";
const ROOT_PATH: &str = "arbitrary-file.xml";
const MODULE_PATH: &str = "arbitrary-file/Ext/ObjectModule.bsl";
const MODULE: &[u8] = b"// Own lexical bytes\r\nProcedure Own() Export\r\nEndProcedure\r\n";
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!(
            "ibcmd-external-intake-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn write(&self, path: &str, bytes: &[u8]) {
        let p = self.0.join(path);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, bytes).unwrap();
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn profile(version: &str) -> ProfileId {
    ProfileId::parse(&format!("xml-{version}")).unwrap()
}
fn path() -> ObjectPath {
    ObjectPath::new(vec![PathSegment::name("OwnedExternal").unwrap()]).unwrap()
}
fn fixture(kind: ExternalArtifactKind, version: &str, children: &str) -> Vec<u8> {
    let props=kind.properties().iter().copied().filter(|x|kind.property_available(x,version)).map(|n|format!("<{n}>{}</{n}>",match n {"Name"=>"Own","Synonym"=>"<v8:item><v8:lang>ru</v8:lang><v8:content>Own label</v8:content></v8:item>","Comment"=>"Own comment",_=>""})).collect::<String>();
    format!("\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<!--own lexical--><MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" version=\"{version}\"><{} uuid=\"{MAIN}\"><InternalInfo><xr:ContainedObject><xr:ClassId>{}</xr:ClassId><xr:ObjectId>{OBJECT}</xr:ObjectId></xr:ContainedObject><xr:GeneratedType name=\"{}.Own\" category=\"Object\"><xr:TypeId>{TYPE}</xr:TypeId><xr:ValueId>{VALUE}</xr:ValueId></xr:GeneratedType></InternalInfo><Properties>{props}</Properties><ChildObjects>{children}</ChildObjects></{}></MetaDataObject>\r\n",kind.external_kind(),kind.class_id(),kind.object_type_prefix(),kind.external_kind()).into_bytes()
}
fn entry(path: &str, bytes: &[u8]) -> SourceEntry {
    SourceEntry::from_bytes(SourcePath::new(path).unwrap(), bytes.to_vec()).unwrap()
}
fn tree(kind: ExternalArtifactKind, version: &str) -> SourceTree {
    SourceTree::new(vec![
        entry(ROOT_PATH, &fixture(kind, version, "")),
        entry(MODULE_PATH, MODULE),
    ])
    .unwrap()
}
fn from_tree(tree: SourceTree, version: &str) -> ExternalIntake {
    ExternalIntake::from_tree(tree, None, profile(version), path()).unwrap()
}
fn equal(a: &ExternalIntake, b: &ExternalIntake) {
    assert_eq!(a.tree(), b.tree());
    assert_eq!(a.root_path(), b.root_path());
    assert_eq!(a.envelope().root(), b.envelope().root());
    assert_eq!(a.envelope().descendants(), b.envelope().descendants());
    assert_eq!(
        a.envelope().configuration().unwrap(),
        b.envelope().configuration().unwrap()
    );
    assert_eq!(
        a.envelope().external_source_binding(),
        b.envelope().external_source_binding()
    );
    assert_eq!(a.identities(), b.identities());
    assert_eq!(a.graph(), b.graph());
    assert_eq!(a.claims(), b.claims());
}
fn changed(envelope: &MetadataEnvelope, kind: ExternalArtifactKind) -> MetadataEnvelope {
    let r = envelope.root();
    let mut p = CanonicalObjectParts::new(
        r.identity().clone(),
        r.kind().clone(),
        r.provenance().clone(),
    );
    p.owner = r.owner();
    p.properties = r.properties().to_vec();
    p.references = r.references().to_vec();
    p.generated_types = r.generated_types().to_vec();
    p.assets = r.assets().to_vec();
    p.opaque_facets = r.opaque_facets().clone();
    for (field, value) in [
        ("Name", "Edited".to_owned()),
        ("Comment", "CURRENT & text".to_owned()),
        (
            "ObjectTypeName",
            format!("{}.Edited", kind.object_type_prefix()),
        ),
    ] {
        let target = p
            .properties
            .iter_mut()
            .find(|x| x.name().as_str() == field)
            .unwrap();
        *target = CanonicalField::named(
            field,
            CanonicalValue::text(CanonicalText::new(&value).unwrap()),
        )
        .unwrap();
    }
    envelope
        .clone()
        .with_model(
            CanonicalObject::new(p).unwrap(),
            envelope.descendants().to_vec(),
        )
        .unwrap()
}
#[test]
fn strict_external_direct_and_directory_share_full_snapshot_graph_and_owned_module() {
    for kind in [
        ExternalArtifactKind::DataProcessor,
        ExternalArtifactKind::Report,
    ] {
        for version in ["2.20", "2.21"] {
            let scratch = Scratch::new();
            let expected = tree(kind, version);
            for e in expected.entries() {
                scratch.write(e.path().as_str(), e.bytes());
            }
            let directory = ExternalIntake::read(
                &scratch.0,
                profile(version),
                path(),
                ReaderLimits::default(),
            )
            .unwrap();
            let file = ExternalIntake::read(
                scratch.0.join(ROOT_PATH),
                profile(version),
                path(),
                ReaderLimits::default(),
            )
            .unwrap();
            equal(&directory, &file);
            equal(&directory, &from_tree(expected, version));
            assert_eq!(directory.claims().len(), 1);
            let c = &directory.claims()[0];
            assert_eq!(c.owner(), ObjectUuid::parse(OBJECT).unwrap());
            assert_ne!(c.owner(), ObjectUuid::parse(MAIN).unwrap());
            assert_eq!(c.source_path().as_str(), MODULE_PATH);
            assert_eq!(c.route().suffix(), ".0");
            c.content().verify_bytes(MODULE).unwrap();
            assert!(directory.graph().contains_key(MAIN));
            let key = format!("{OBJECT}.0");
            let storage = directory
                .graph()
                .entries()
                .iter()
                .find(|x| x.key().as_str() == key)
                .unwrap();
            assert!(
                matches!(storage.owner(),BootstrapStorageOwner::Object {uuid,suffix:Some(s)} if *uuid==c.owner() && s.as_str()==".0")
            );
            let root = directory
                .tree()
                .entries()
                .iter()
                .find(|x| x.path() == directory.root_path())
                .unwrap();
            assert_eq!(root.bytes(), fixture(kind, version, ""));
        }
    }
}
#[test]
fn external_current_metadata_and_asset_bytes_preserve_complete_inverse_and_originals() {
    for kind in [
        ExternalArtifactKind::DataProcessor,
        ExternalArtifactKind::Report,
    ] {
        for version in ["2.20", "2.21"] {
            let original = from_tree(tree(kind, version), version);
            let before = original.tree().clone();
            let current = changed(original.envelope(), kind);
            let current_before = current.root().clone();
            let new_module =
                "\u{feff}// CURRENT exact\r\nProcedure Edited() Export\r\nEndProcedure\r\n"
                    .as_bytes();
            let edits = vec![entry(MODULE_PATH, new_module)];
            let prepared = original.with_current(&current, &edits).unwrap();
            assert_eq!(prepared.envelope().root(), current.root());
            assert_eq!(prepared.envelope().descendants(), current.descendants());
            assert_eq!(
                prepared.envelope().configuration().unwrap(),
                current.configuration().unwrap()
            );
            assert_eq!(prepared.identities(), original.identities());
            assert_eq!(prepared.graph(), original.graph());
            assert_ne!(
                prepared.claims()[0].content(),
                original.claims()[0].content()
            );
            prepared.claims()[0]
                .content()
                .verify_bytes(new_module)
                .unwrap();
            assert_eq!(
                prepared
                    .tree()
                    .entries()
                    .iter()
                    .find(|x| x.path().as_str() == MODULE_PATH)
                    .unwrap()
                    .bytes(),
                new_module
            );
            let scratch = Scratch::new();
            for e in prepared.tree().entries() {
                scratch.write(e.path().as_str(), e.bytes());
            }
            let returned = ExternalIntake::read(
                &scratch.0,
                profile(version),
                path(),
                ReaderLimits::default(),
            )
            .unwrap();
            equal(&prepared, &returned);
            equal(
                &prepared,
                &prepared.with_current(prepared.envelope(), &[]).unwrap(),
            );
            assert_eq!(original.tree(), &before);
            assert_eq!(current.root(), &current_before);
            assert_eq!(edits[0].bytes(), new_module);
        }
    }
}
#[test]
fn retained_intake_never_rereads_changed_files_during_current_preparation() {
    for kind in [
        ExternalArtifactKind::DataProcessor,
        ExternalArtifactKind::Report,
    ] {
        for version in ["2.20", "2.21"] {
            let scratch = Scratch::new();
            for e in tree(kind, version).entries() {
                scratch.write(e.path().as_str(), e.bytes());
            }
            let intake = ExternalIntake::read(
                &scratch.0,
                profile(version),
                path(),
                ReaderLimits::default(),
            )
            .unwrap();
            scratch.write(ROOT_PATH, b"not XML now");
            scratch.write(MODULE_PATH, b"changed disk payload");
            let prepared = intake.with_current(intake.envelope(), &[]).unwrap();
            equal(&intake, &prepared);
            assert!(
                ExternalIntake::read(
                    &scratch.0,
                    profile(version),
                    path(),
                    ReaderLimits::default()
                )
                .is_err()
            );
        }
    }
}
#[test]
fn strict_external_unknown_orphan_hidden_and_ambiguous_inputs_refuse_in_both_modes() {
    for kind in [
        ExternalArtifactKind::DataProcessor,
        ExternalArtifactKind::Report,
    ] {
        for version in ["2.20", "2.21"] {
            for extra in [
                "orphan.bin",
                "other/Ext/ObjectModule.bsl",
                "arbitrary-file/Ext/ManagerModule.bsl",
                "arbitrary-file/Ext/Help.xml",
                ".git/probe.txt",
                "target/probe.txt",
                ".idea/probe.txt",
                ".vscode/probe.txt",
            ] {
                let scratch = Scratch::new();
                for e in tree(kind, version).entries() {
                    scratch.write(e.path().as_str(), e.bytes());
                }
                scratch.write(
                    extra,
                    if extra.ends_with(".xml") {
                        b"<Help/>"
                    } else {
                        b"own extra"
                    },
                );
                let selected = scratch.0.join(ROOT_PATH);
                for input in [scratch.0.as_path(), selected.as_path()] {
                    assert!(
                        ExternalIntake::read(
                            input,
                            profile(version),
                            path(),
                            ReaderLimits::default()
                        )
                        .is_err(),
                        "{extra}"
                    );
                }
                if extra.starts_with(".git/") {
                    assert_eq!(read_source_tree(&scratch.0).unwrap().entries().len(), 2);
                    assert!(read_source_tree_strict(&scratch.0, ReaderLimits::default()).is_err());
                }
            }
            let scratch = Scratch::new();
            for e in tree(kind, version).entries() {
                scratch.write(e.path().as_str(), e.bytes());
            }
            let second = String::from_utf8(fixture(kind, version, ""))
                .unwrap()
                .replace(MAIN, "55000000-0000-4000-8000-000000000438");
            scratch.write("another-root.xml", second.as_bytes());
            assert!(matches!(
                ExternalIntake::read(
                    &scratch.0,
                    profile(version),
                    path(),
                    ReaderLimits::default()
                ),
                Err(ExternalIntakeError::RootCount { actual: 2 })
            ));
            assert!(matches!(
                ExternalIntake::read(
                    scratch.0.join(ROOT_PATH),
                    profile(version),
                    path(),
                    ReaderLimits::default()
                ),
                Err(ExternalIntakeError::RootCount { actual: 2 })
            ));
            let scratch = Scratch::new();
            for e in tree(kind, version).entries() {
                scratch.write(e.path().as_str(), e.bytes());
            }
            scratch.write("not-a-package.xml", b"<Other/>");
            assert!(matches!(
                ExternalIntake::read(
                    scratch.0.join("not-a-package.xml"),
                    profile(version),
                    path(),
                    ReaderLimits::default()
                ),
                Err(ExternalIntakeError::WrongSelectedRoot { .. })
            ));
            let scratch = Scratch::new();
            for e in tree(kind, version).entries() {
                scratch.write(e.path().as_str(), e.bytes());
            }
            fs::create_dir(scratch.0.join("empty-orphan")).unwrap();
            assert!(
                ExternalIntake::read(
                    &scratch.0,
                    profile(version),
                    path(),
                    ReaderLimits::default()
                )
                .is_err()
            );
        }
    }
}
#[test]
fn unresolved_named_metadata_and_embedded_source_are_not_successfully_consumed() {
    for kind in [
        ExternalArtifactKind::DataProcessor,
        ExternalArtifactKind::Report,
    ] {
        for version in ["2.20", "2.21"] {
            for declaration in ["<Form>OwnForm</Form>", "<Template>OwnTemplate</Template>"] {
                let t =
                    SourceTree::new(vec![entry(ROOT_PATH, &fixture(kind, version, declaration))])
                        .unwrap();
                assert!(matches!(
                    ExternalIntake::from_tree(t, None, profile(version), path()),
                    Err(ExternalIntakeError::Unsupported {
                        coordinate: "declared Form/Template metadata and body ownership (A1 remainder)",
                        ..
                    })
                ));
            }
            let embedded = "<Attribute uuid=\"66000000-0000-4000-8000-000000000438\"><Properties><Name>OwnAttr</Name></Properties></Attribute>";
            let t =
                SourceTree::new(vec![entry(ROOT_PATH, &fixture(kind, version, embedded))]).unwrap();
            assert!(ExternalIntake::from_tree(t, None, profile(version), path()).is_err());
        }
    }
}
#[test]
fn current_asset_wrong_owner_duplicates_invalid_text_and_foreign_source_refuse_atomically() {
    for kind in [
        ExternalArtifactKind::DataProcessor,
        ExternalArtifactKind::Report,
    ] {
        for version in ["2.20", "2.21"] {
            let original = from_tree(tree(kind, version), version);
            let before = original.tree().clone();
            let edit = entry(MODULE_PATH, b"// edited\r\n");
            let bad = entry(MODULE_PATH, &[255, 254]);
            for edits in [
                vec![entry("orphan/Ext/ObjectModule.bsl", MODULE)],
                vec![edit.clone(), edit.clone()],
                vec![bad],
            ] {
                assert!(original.with_current(original.envelope(), &edits).is_err());
                assert_eq!(original.tree(), &before);
            }
            let foreign_bytes = String::from_utf8(fixture(kind, version, ""))
                .unwrap()
                .replace("Own comment", "Foreign authored origin");
            let foreign = decode_external_root(
                &XmlReader::from_slice(foreign_bytes.as_bytes()).unwrap(),
                profile(version),
                path(),
            )
            .unwrap();
            assert!(matches!(
                original.with_current(&foreign, &[]),
                Err(ExternalIntakeError::CurrentMismatch { .. })
            ));
            assert_eq!(original.tree(), &before);
        }
    }
}
#[test]
fn strict_snapshot_preserves_closed_root_namespace_class_property_and_profile_admission() {
    for kind in [
        ExternalArtifactKind::DataProcessor,
        ExternalArtifactKind::Report,
    ] {
        for version in ["2.20", "2.21"] {
            let source = String::from_utf8(fixture(kind, version, "")).unwrap();
            for invalid in [
                source.replace(kind.class_id(), "99000000-0000-4000-8000-000000000438"),
                source.replace(TYPE, VALUE),
                source.replace("</Properties>", "<Unknown>value</Unknown></Properties>"),
                source.replace("</Properties>", "<Name>Duplicate</Name></Properties>"),
                source.replace("http://v8.1c.ru/8.3/MDClasses", "urn:foreign:metadata"),
            ] {
                let scratch = Scratch::new();
                scratch.write(ROOT_PATH, invalid.as_bytes());
                scratch.write(MODULE_PATH, MODULE);
                let selected = scratch.0.join(ROOT_PATH);
                for input in [scratch.0.as_path(), selected.as_path()] {
                    assert!(
                        ExternalIntake::read(
                            input,
                            profile(version),
                            path(),
                            ReaderLimits::default()
                        )
                        .is_err()
                    );
                }
                assert_eq!(fs::read(selected).unwrap(), invalid.as_bytes());
                assert_eq!(fs::read(scratch.0.join(MODULE_PATH)).unwrap(), MODULE);
            }
            let scratch = Scratch::new();
            scratch.write(ROOT_PATH, source.as_bytes());
            let unsupported = ProfileId::parse("xml-9.99").unwrap();
            assert!(
                ExternalIntake::read(&scratch.0, unsupported, path(), ReaderLimits::default())
                    .is_err()
            );
            assert_eq!(
                fs::read(scratch.0.join(ROOT_PATH)).unwrap(),
                source.as_bytes()
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn strict_external_symlink_and_nonregular_sources_refuse_without_following() {
    use std::os::unix::{fs::symlink, net::UnixListener};
    for version in ["2.20", "2.21"] {
        let scratch = Scratch::new();
        let outside = Scratch::new();
        for e in tree(ExternalArtifactKind::DataProcessor, version).entries() {
            scratch.write(e.path().as_str(), e.bytes());
        }
        let admit = || {
            ExternalIntake::read(
                &scratch.0,
                profile(version),
                path(),
                ReaderLimits::default(),
            )
            .unwrap()
        };
        let refuse_path = |unsafe_path: PathBuf| {
            let error = ExternalIntake::read(
                &scratch.0,
                profile(version),
                path(),
                ReaderLimits::default(),
            )
            .unwrap_err();
            assert!(matches!(error, ExternalIntakeError::Input { reason, .. }
                if reason.contains(&unsafe_path.display().to_string())));
        };
        admit();
        outside.write("outside.bsl", MODULE);
        fs::remove_file(scratch.0.join(MODULE_PATH)).unwrap();
        symlink(outside.0.join("outside.bsl"), scratch.0.join(MODULE_PATH)).unwrap();
        refuse_path(scratch.0.join(MODULE_PATH));
        assert_eq!(fs::read(outside.0.join("outside.bsl")).unwrap(), MODULE);
        fs::remove_file(scratch.0.join(MODULE_PATH)).unwrap();
        scratch.write(MODULE_PATH, MODULE);
        admit();
        let socket = UnixListener::bind(scratch.0.join("own.sock")).unwrap();
        refuse_path(scratch.0.join("own.sock"));
        drop(socket);
        fs::remove_file(scratch.0.join("own.sock")).unwrap();
        admit();
        symlink(&outside.0, scratch.0.join("linked-dir")).unwrap();
        refuse_path(scratch.0.join("linked-dir"));
    }
}

// Physical alias selection never changes the memory adapter's exact path contract.
#[test]
fn memory_selected_path_remains_exact_for_both_external_kinds_and_profiles() {
    for kind in [
        ExternalArtifactKind::DataProcessor,
        ExternalArtifactKind::Report,
    ] {
        for version in ["2.20", "2.21"] {
            let original = tree(kind, version);
            let exact = SourcePath::new(ROOT_PATH).unwrap();
            let alias = SourcePath::new(ROOT_PATH.to_uppercase()).unwrap();
            let admitted =
                ExternalIntake::from_tree(original.clone(), Some(&exact), profile(version), path())
                    .unwrap();
            equal(&admitted, &from_tree(original.clone(), version));
            assert!(matches!(
                ExternalIntake::from_tree(original.clone(), Some(&alias), profile(version), path()),
                Err(ExternalIntakeError::WrongSelectedRoot { .. })
            ));
            assert_eq!(admitted.tree(), &original);
        }
    }
}

#[cfg(windows)]
#[test]
fn windows_physical_case_alias_retains_actual_census_spelling_and_complete_source() {
    for kind in [
        ExternalArtifactKind::DataProcessor,
        ExternalArtifactKind::Report,
    ] {
        for version in ["2.20", "2.21"] {
            for root_name in [ROOT_PATH, "arbitrary-Ж-file.xml"] {
                let scratch = Scratch::new();
                let module_name = format!(
                    "{}/Ext/ObjectModule.bsl",
                    root_name.strip_suffix(".xml").unwrap()
                );
                let original = SourceTree::new(vec![
                    entry(root_name, &fixture(kind, version, "")),
                    entry(&module_name, MODULE),
                ])
                .unwrap();
                for e in original.entries() {
                    scratch.write(e.path().as_str(), e.bytes());
                }
                let alias = scratch.0.join(root_name.to_uppercase());
                assert!(alias.is_file()); // OS resolves this spelling, not a Rust casefold assumption.
                let direct =
                    ExternalIntake::read(&alias, profile(version), path(), ReaderLimits::default())
                        .unwrap();
                let directory = ExternalIntake::read(
                    &scratch.0,
                    profile(version),
                    path(),
                    ReaderLimits::default(),
                )
                .unwrap();
                let memory = from_tree(original.clone(), version);
                equal(&direct, &directory);
                equal(&direct, &memory);
                assert_eq!(direct.root_path().as_str(), root_name);
                assert_eq!(direct.tree(), &original);
                assert_eq!(direct.claims()[0].source_path().as_str(), module_name);
                direct.claims()[0].content().verify_bytes(MODULE).unwrap();
                let metadata_name = direct
                    .envelope()
                    .root()
                    .properties()
                    .iter()
                    .find(|field| field.name().as_str() == "Name")
                    .unwrap();
                assert_eq!(
                    metadata_name.value(),
                    &CanonicalValue::text(CanonicalText::new("Own").unwrap())
                );
                assert_ne!(root_name, "Own");
                let no_edit = direct.with_current(direct.envelope(), &[]).unwrap();
                equal(&direct, &no_edit);

                // A physically distinct file's alias is never allowed to select the unique root.
                scratch.write("not-a-package.xml", b"<Other/>");
                assert!(matches!(
                    ExternalIntake::read(
                        scratch.0.join("NOT-A-PACKAGE.XML"),
                        profile(version),
                        path(),
                        ReaderLimits::default(),
                    ),
                    Err(ExternalIntakeError::WrongSelectedRoot { .. })
                ));
            }
        }
    }
}

#[cfg(unix)]
#[test]
fn unix_selected_spelling_follows_actual_filesystem_identity_without_casefold() {
    for kind in [
        ExternalArtifactKind::DataProcessor,
        ExternalArtifactKind::Report,
    ] {
        for version in ["2.20", "2.21"] {
            let scratch = Scratch::new();
            let original = tree(kind, version);
            for e in original.entries() {
                scratch.write(e.path().as_str(), e.bytes());
            }
            let alias = scratch.0.join(ROOT_PATH.to_uppercase());
            let returned =
                ExternalIntake::read(&alias, profile(version), path(), ReaderLimits::default());
            // Unix includes both case-sensitive Linux and case-insensitive APFS.
            // No spelling is invented: admission follows the OS's actual lookup.
            if alias.exists() {
                equal(&returned.unwrap(), &from_tree(original.clone(), version));
            } else {
                assert!(matches!(returned, Err(ExternalIntakeError::Input { .. })));
            }
            let exact = ExternalIntake::read(
                scratch.0.join(ROOT_PATH),
                profile(version),
                path(),
                ReaderLimits::default(),
            )
            .unwrap();
            equal(&exact, &from_tree(original, version));
        }
    }
}
