//! Rights lexical framing is source-only; all current ordered values remain semantic.
//! This test is staged in the lab until the frozen Role census has completed.
use formats_xml::registry::SidecarFormat;
use formats_xml::rights::{detect_designer_rights_version, read, write};
use ibcmd_edt::{
    ConversionOptions, Project, ReaderLimits, edt_to_xml, read_directory_source, read_xml_source,
    xml_to_edt,
};
use ibcmd_xml::source_tree::{SourceEntry, SourcePath, SourceTree, publish_new_with_limits};
use morph1c_core::ir::{MetadataObject, ObjectKind, PropertyValue, Uuid};
use morph1c_core::spec::metadata::configuration as configuration_spec;
use morph1c_core::spec::metadata::role::{RightRestriction, RightsTable};
use morph1c_core::version::{FormatVersion, with_roundtrip_target};
use morph1c_pipeline::{ConvertOptions, Format, read_config, write_config};
use sha2::{Digest, Sha256};

const NATIVE_PATH: &str = "Roles/FramingRole/Ext/Rights.xml";
const EDT_PATH: &str = "src/Roles/FramingRole/Rights.rights";
const MANIFEST: &str = ".ibcmd-provenance/manifest.json";

// Independently authored XML. Only markup uses `eol`; leaf values are separate.
// Native mixed CRLF/LF and standalone CR are deliberate authored spelling.
fn native_input(minor: u16, lf_markup: bool, mixed: bool) -> Vec<u8> {
    let eol = if lf_markup { "\n" } else { "\r\n" };
    let mut text = String::new();
    if !lf_markup {
        text.push('\u{feff}');
    }
    text.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
    text.push_str(eol);
    text.push_str(&format!("<Rights xmlns=\"http://v8.1c.ru/8.2/roles\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xsi:type=\"Rights\" version=\"2.{minor}\">"));
    text.push_str(eol);
    for line in [
        "\t<setForNewObjects>false</setForNewObjects>",
        "\t<setForAttributesByDefault>true</setForAttributesByDefault>",
        "\t<independentRightsOfChildObjects>false</independentRightsOfChildObjects>",
        "\t<object>",
        "\t\t<name>Catalog.Example</name>",
        "\t\t<right>",
        "\t\t\t<name>Read</name>",
        "\t\t\t<value>true</value>",
    ] {
        text.push_str(line);
        text.push_str(eol);
    }
    let conditions = if mixed {
        [
            "FIRST &amp;X\r\nOR Y\nNEXT\rTAIL",
            "SECOND\nNEXT\r\nLAST",
            "",
            "NO_NEWLINE\rTAIL",
        ]
    } else {
        [
            "FIRST &amp;X\nOR Y\nNEXT\rTAIL",
            "SECOND\nNEXT\nLAST",
            "",
            "NO_NEWLINE\rTAIL",
        ]
    };
    for (field, condition) in [Some("Field"), Some("Field"), None, Some("")]
        .into_iter()
        .zip(conditions)
    {
        text.push_str("\t\t\t<restrictionByCondition>");
        text.push_str(eol);
        if let Some(field) = field {
            text.push_str("\t\t\t\t<field>");
            text.push_str(field);
            text.push_str("</field>");
            text.push_str(eol);
        }
        text.push_str("\t\t\t\t<condition>");
        text.push_str(condition);
        text.push_str("</condition>");
        text.push_str(eol);
        text.push_str("\t\t\t</restrictionByCondition>");
        text.push_str(eol);
    }
    for line in [
        "\t\t</right>",
        "\t</object>",
        "\t<restrictionTemplate>",
        "\t\t<name>Template</name>",
    ] {
        text.push_str(line);
        text.push_str(eol);
    }
    text.push_str("\t\t<condition>");
    text.push_str(if mixed {
        "TEMPLATE\nNEXT\r\nLAST\rTAIL"
    } else {
        "TEMPLATE\nNEXT\nLAST\rTAIL"
    });
    text.push_str("</condition>");
    text.push_str(eol);
    text.push_str("\t</restrictionTemplate>");
    text.push_str(eol);
    text.push_str("</Rights>");
    if lf_markup {
        text.push_str(eol);
    }
    text.into_bytes()
}
fn emit(table: &RightsTable, format: SidecarFormat, minor: u16) -> Vec<u8> {
    with_roundtrip_target(FormatVersion::new(2, minor), || write(table, format)).unwrap()
}
fn semantic(table: &RightsTable) -> Vec<u8> {
    serde_json::to_vec(table).unwrap()
}
fn edited(mut table: RightsTable, mutation: usize) -> RightsTable {
    match mutation {
        0 => table.set_for_new_objects = !table.set_for_new_objects,
        1 => table.objects[0].rights[0].restrictions[0].field = None,
        2 => {
            table.objects[0].rights[0].restrictions[0].condition =
                "CURRENT &lt;VALUE\r\nSECOND\rTAIL".into()
        }
        3 => table.objects[0].rights[0].restrictions.swap(0, 1),
        4 => table.objects[0].rights[0]
            .restrictions
            .push(RightRestriction {
                field: Some("CURRENT_FIELD".into()),
                condition: "APPENDED\r\nNEXT".into(),
            }),
        5 => {
            table.objects[0].rights[0].restrictions.remove(1);
        }
        6 => table.restriction_templates[0].condition = "CURRENT_TEMPLATE\r\nNEXT".into(),
        7 => table.objects[0].rights[0].restrictions[2].field = Some("".into()),
        _ => unreachable!(),
    }
    table
}
#[test]
fn independently_authored_native_framings_regenerate_own_bytes_and_cross_semantics() {
    for minor in [20, 21] {
        for (lf, mixed) in [(true, false), (false, true)] {
            let raw = native_input(minor, lf, mixed);
            assert_eq!(
                detect_designer_rights_version(&raw),
                Some(FormatVersion::new(2, minor))
            );
            let native = read(&raw, SidecarFormat::DesignerRights).unwrap();
            assert_eq!(emit(&native, SidecarFormat::DesignerRights, minor), raw);
            let edt_bytes = emit(&native, SidecarFormat::EdtRights, minor);
            let edt = read(&edt_bytes, SidecarFormat::EdtRights).unwrap();
            assert_eq!(semantic(&edt), semantic(&native));
            assert_eq!(edt, native);
            assert_eq!(emit(&edt, SidecarFormat::EdtRights, minor), edt_bytes);
            let cross_back = emit(&edt, SidecarFormat::DesignerRights, minor);
            assert_eq!(
                read(&cross_back, SidecarFormat::DesignerRights).unwrap(),
                native
            );
            // The bare codec has no provenance transport across serialized dialects.
            // It must use standard target framing, not pretend original spelling survived.
            assert_ne!(cross_back, raw);
            assert!(cross_back.starts_with(b"\xef\xbb\xbf"));
            assert!(cross_back.ends_with(b"</Rights>"));
            assert_eq!(
                detect_designer_rights_version(&emit(
                    &native,
                    SidecarFormat::DesignerRights,
                    if minor == 20 { 21 } else { 20 }
                )),
                Some(FormatVersion::new(2, if minor == 20 { 21 } else { 20 }))
            );
        }
    }
}
#[test]
fn every_current_semantic_edit_invalidates_patterns_without_losing_current_data() {
    for minor in [20, 21] {
        for lf in [true, false] {
            let raw = native_input(minor, lf, true);
            let table = read(&raw, SidecarFormat::DesignerRights).unwrap();
            for mutation in 0..8 {
                let current = edited(table.clone(), mutation);
                assert_ne!(semantic(&current), semantic(&table));
                for format in [SidecarFormat::DesignerRights, SidecarFormat::EdtRights] {
                    let output = emit(&current, format, minor);
                    let reparsed = read(&output, format).unwrap();
                    assert_eq!(semantic(&reparsed), semantic(&current));
                    assert_eq!(reparsed, current);
                    if format == SidecarFormat::DesignerRights {
                        assert_eq!(output.starts_with(b"\xef\xbb\xbf"), !lf);
                        assert_eq!(output.ends_with(b"</Rights>\n"), lf);
                        // Same source framing survives; original mixed leaf spelling does not.
                        assert!(
                            !String::from_utf8(output)
                                .unwrap()
                                .contains("FIRST &amp;X\r\nOR Y\nNEXT")
                        );
                    }
                }
            }
        }
    }
}
#[test]
fn malformed_source_pattern_counts_and_hash_cannot_replay_previous_condition_spelling() {
    for minor in [20, 21] {
        let table = read(
            &native_input(minor, false, true),
            SidecarFormat::DesignerRights,
        )
        .unwrap();
        let before = semantic(&table);
        for damage in 0..4 {
            let mut forged = table.clone();
            let layout = forged.source_layout.as_mut().expect("read source layout");
            match damage {
                0 => layout.canonical_sha256 = [0; 32],
                1 => {
                    layout.condition_newlines.pop();
                }
                2 => layout.condition_newlines[0].clear(),
                3 => layout.condition_newlines.push(vec![]),
                _ => unreachable!(),
            }
            // Lexical details are excluded from semantic JSON and equality only.
            assert_eq!(semantic(&forged), before);
            assert_eq!(forged, table);
            let output = emit(&forged, SidecarFormat::DesignerRights, minor);
            let mut standard = table.clone();
            standard.source_layout = None;
            assert_eq!(
                output,
                emit(&standard, SidecarFormat::DesignerRights, minor)
            );
            assert!(!String::from_utf8_lossy(&output).contains("FIRST &amp;X\r\nOR Y\nNEXT"));
            assert_eq!(read(&output, SidecarFormat::DesignerRights).unwrap(), table);
            let mut changed = edited(forged, 2);
            changed.objects[0].rights[0].restrictions.swap(0, 1);
            let output = emit(&changed, SidecarFormat::DesignerRights, minor);
            assert_eq!(
                semantic(&read(&output, SidecarFormat::DesignerRights).unwrap()),
                semantic(&changed)
            );
            assert!(String::from_utf8_lossy(&output).contains("CURRENT &lt;VALUE\nSECOND\rTAIL"));
        }
    }
}

#[test]
fn framing_support_keeps_unknown_namespace_structure_and_trailing_bytes_strict() {
    for lf in [true, false] {
        let raw = native_input(21, lf, true);
        let text = String::from_utf8(raw).unwrap();
        for malformed in [
            text.replace("http://v8.1c.ru/8.2/roles", "urn:unknown"),
            text.replace("xsi:type=\"Rights\"", "xsi:type=\"Unknown\""),
            text.replacen("<condition>", "<condition unknown=\"true\">", 1),
            text.replacen("<condition>FIRST", "<condition><Unknown/>FIRST", 1),
            text.replace("<value>true</value>", "<value>unknown</value>"),
            format!("{text}\n\n"),
            format!("{text}unclaimed"),
            text.replace("version=\"2.21\"", "version=\"999.999\""),
        ] {
            assert!(read(malformed.as_bytes(), SidecarFormat::DesignerRights).is_err());
        }
    }
}
fn options(minor: u16) -> ConversionOptions {
    ConversionOptions {
        edt_version: "2025.2.3".into(),
        xml_dialect: format!("2.{minor}"),
        runtime_version: Some(if minor == 20 { "8.3.27" } else { "8.5.1" }.into()),
    }
}
fn source(minor: u16, lf: bool) -> SourceTree {
    let convert = ConvertOptions::default().with_target_version(FormatVersion::new(2, minor));
    let mut cfg = read_config(
        Format::Designer,
        std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/subsystem-ci/src"
        )),
        &convert,
    )
    .unwrap()
    .0;
    cfg.source_version = Some(FormatVersion::new(2, minor));
    let mut role = MetadataObject::new(ObjectKind::new("Role"), "FramingRole", Uuid([0x69; 16]));
    role.rights = Some(
        read(
            &native_input(minor, lf, true),
            SidecarFormat::DesignerRights,
        )
        .unwrap(),
    );
    cfg.objects.push(role);
    let root = cfg
        .objects
        .iter_mut()
        .find(|obj| obj.kind.as_str() == "Configuration")
        .unwrap();
    let PropertyValue::List(roster) = &mut root
        .properties
        .iter_mut()
        .find(|(id, _)| *id == configuration_spec::F_CHILD_OBJECTS)
        .unwrap()
        .1
    else {
        panic!("typed configuration roster")
    };
    roster.push(PropertyValue::List(vec![
        PropertyValue::Str("Role".into()),
        PropertyValue::Str("FramingRole".into()),
    ]));
    let directory = tempfile::tempdir().unwrap();
    with_roundtrip_target(FormatVersion::new(2, minor), || {
        write_config(Format::Designer, &cfg, directory.path()).unwrap()
    });
    // Keep the independent physical source as the test oracle, never a codec-produced echo.
    std::fs::write(
        directory.path().join(NATIVE_PATH),
        native_input(minor, lf, true),
    )
    .unwrap();
    read_xml_source(directory.path(), ReaderLimits::default()).unwrap()
}
fn bytes<'a>(tree: &'a SourceTree, path: &str) -> &'a [u8] {
    tree.entries()
        .iter()
        .find(|entry| entry.path().as_str() == path)
        .unwrap()
        .bytes()
}
fn replace(tree: &SourceTree, path: &str, content: Vec<u8>) -> SourceTree {
    let mut entries = tree
        .entries()
        .iter()
        .filter(|entry| entry.path().as_str() != path)
        .cloned()
        .collect::<Vec<_>>();
    entries.push(SourceEntry::from_bytes(SourcePath::new(path).unwrap(), content).unwrap());
    SourceTree::new(entries).unwrap()
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
fn rights(tree: &SourceTree, path: &str, format: SidecarFormat) -> RightsTable {
    read(bytes(tree, path), format).unwrap()
}
fn manifest(tree: &SourceTree) -> serde_json::Value {
    serde_json::from_slice(bytes(tree, MANIFEST)).unwrap()
}
fn rehash(tree: &SourceTree, section: &str, path: &str, content: &[u8]) -> SourceTree {
    let mut value = manifest(tree);
    value[section][path] = serde_json::json!(format!("{:x}", Sha256::digest(content)));
    replace(tree, MANIFEST, serde_json::to_vec(&value).unwrap())
}
fn disk_return(tree: &SourceTree, minor: u16) -> SourceTree {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("input");
    publish_new_with_limits(tree, &input, ReaderLimits::default()).unwrap();
    let conversion = read_directory_source(input)
        .unwrap()
        .edt_to_xml(&options(minor))
        .unwrap();
    let out = temp.path().join("output");
    conversion.publish_new(&out).unwrap();
    read_xml_source(out, ReaderLimits::default()).unwrap()
}
#[test]
fn public_memory_and_directory_unchanged_return_preserve_complete_original_bytes() {
    for minor in [20, 21] {
        for lf in [true, false] {
            let original = source(minor, lf);
            let generated = xml_to_edt(&original, &options(minor)).unwrap().tree;
            let returned = edt_to_xml(
                &Project::from_tree(generated.clone()).unwrap(),
                &options(minor),
            )
            .unwrap()
            .tree;
            assert_eq!(returned, original);
            assert_eq!(disk_return(&generated, minor), original);
            let temporary = tempfile::tempdir().unwrap();
            let native = temporary.path().join("native");
            publish_new_with_limits(&original, &native, ReaderLimits::default()).unwrap();
            let directory = read_directory_source(native)
                .unwrap()
                .xml_to_edt(&options(minor))
                .unwrap();
            let edt = temporary.path().join("edt");
            directory.publish_new(&edt).unwrap();
            let directory_generated = read_xml_source(edt, ReaderLimits::default()).unwrap();
            assert_eq!(directory_generated, generated);
            assert_eq!(disk_return(&directory_generated, minor), original);
        }
    }
}
#[test]
fn public_stripped_current_edits_remain_typed_and_forged_digests_cannot_restore_old_data() {
    for minor in [20, 21] {
        let original = source(minor, false);
        let generated = xml_to_edt(&original, &options(minor)).unwrap().tree;
        let old = rights(&generated, EDT_PATH, SidecarFormat::EdtRights);
        let mut current = edited(old.clone(), 2);
        current = edited(current, 3);
        current = edited(current, 4);
        current = edited(current, 1);
        let changed_bytes = emit(&current, SidecarFormat::EdtRights, minor);
        let changed = replace(&generated, EDT_PATH, changed_bytes.clone());
        assert!(
            edt_to_xml(
                &Project::from_tree(changed.clone()).unwrap(),
                &options(minor)
            )
            .is_err()
        );
        let forged = rehash(&changed, "generated", EDT_PATH, &changed_bytes);
        assert!(
            edt_to_xml(
                &Project::from_tree(forged.clone()).unwrap(),
                &options(minor)
            )
            .is_err()
        );
        let temp = tempfile::tempdir().unwrap();
        let bad_dir = temp.path().join("forged");
        publish_new_with_limits(&forged, &bad_dir, ReaderLimits::default()).unwrap();
        assert!(
            read_directory_source(bad_dir)
                .unwrap()
                .edt_to_xml(&options(minor))
                .is_err()
        );
        let stripped = strip(&changed);
        let output = edt_to_xml(
            &Project::from_tree(stripped.clone()).unwrap(),
            &options(minor),
        )
        .unwrap()
        .tree;
        assert_eq!(
            semantic(&rights(&output, NATIVE_PATH, SidecarFormat::DesignerRights)),
            semantic(&current)
        );
        let disk_output = disk_return(&stripped, minor);
        assert_eq!(
            semantic(&rights(
                &disk_output,
                NATIVE_PATH,
                SidecarFormat::DesignerRights
            )),
            semantic(&current)
        );
        assert_eq!(disk_output, output);
        // Rehashing the retained original cannot hide a CURRENT semantic mismatch either.
        let payload = format!(".ibcmd-provenance/xml/{NATIVE_PATH}");
        let altered_original = emit(
            &edited(
                rights(&original, NATIVE_PATH, SidecarFormat::DesignerRights),
                6,
            ),
            SidecarFormat::DesignerRights,
            minor,
        );
        let altered = replace(&generated, &payload, altered_original.clone());
        let forged_original = rehash(&altered, "original", NATIVE_PATH, &altered_original);
        assert!(
            edt_to_xml(
                &Project::from_tree(forged_original.clone()).unwrap(),
                &options(minor)
            )
            .is_err()
        );
        let bad_original_dir = temp.path().join("forged-original");
        publish_new_with_limits(&forged_original, &bad_original_dir, ReaderLimits::default())
            .unwrap();
        assert!(
            read_directory_source(bad_original_dir)
                .unwrap()
                .edt_to_xml(&options(minor))
                .is_err()
        );
    }
}
