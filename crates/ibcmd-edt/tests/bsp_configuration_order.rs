use formats_xml::read::parse;
use morph1c_core::{
    ir::{FieldId, MetadataObject, PropertyValue, Token},
    spec::metadata::configuration as cfg,
    version::{FormatVersion, with_roundtrip_target, with_source_version},
};
use morph1c_pipeline::{Format, registry::FormatRegistry};

fn read(bytes: &[u8]) -> MetadataObject {
    (FormatRegistry::for_format(Format::Designer)
        .unwrap()
        .get("Configuration")
        .unwrap()
        .read)(bytes)
    .unwrap()
}
fn write(obj: &MetadataObject, target: FormatVersion, source: FormatVersion) -> Vec<u8> {
    with_source_version(Some(source), || {
        with_roundtrip_target(target, || {
            (FormatRegistry::for_format(Format::Designer)
                .unwrap()
                .get("Configuration")
                .unwrap()
                .write)(obj)
            .unwrap()
        })
    })
}
fn properties(bytes: &[u8]) -> Vec<String> {
    let doc = parse(bytes).unwrap();
    let config = doc.root.children.first().unwrap();
    let props = config
        .children
        .iter()
        .find(|e| e.local == "Properties")
        .unwrap();
    props.children.iter().map(|e| e.local.clone()).collect()
}
fn value(obj: &MetadataObject, field: FieldId) -> &PropertyValue {
    &obj.properties
        .iter()
        .find(|(id, _)| *id == field)
        .unwrap()
        .1
}
fn set(obj: &mut MetadataObject, field: FieldId, token: &str) {
    let val = PropertyValue::Enum(Token::new(token));
    if let Some((_, old)) = obj.properties.iter_mut().find(|(id, _)| *id == field) {
        *old = val;
    } else {
        obj.properties.push((field, val));
    }
}
fn fixture() -> MetadataObject {
    read(include_bytes!(
        "fixtures/subsystem-ci/src/Configuration.xml"
    ))
}

#[test]
fn effective_target_orders_current_scalars_without_using_source_version() {
    let mut obj = fixture();
    set(&mut obj, cfg::F_MAIN_WINDOW_MODE, "Kiosk");
    set(&mut obj, cfg::F_CLIENT_APPLICATION_THEME, "Dark");
    for target in [FormatVersion::new(2, 20), FormatVersion::new(2, 21)] {
        let source = if target.minor == 21 {
            FormatVersion::new(2, 20)
        } else {
            FormatVersion::new(2, 21)
        };
        let bytes = write(&obj, target, source);
        let names = properties(&bytes);
        let index = |tag: &str| names.iter().position(|s| s == tag).unwrap();
        if target.minor == 21 {
            assert!(
                index("MainClientApplicationWindowInterfaceVariant")
                    < index("ClientApplicationTheme")
            );
            assert!(index("ClientApplicationTheme") < index("MainClientApplicationWindowMode"));
            assert!(
                index("MainClientApplicationWindowMode")
                    < index("ClientApplicationWindowsOpenVariant")
            );
        } else {
            assert!(!names.iter().any(|s| s == "ClientApplicationTheme"));
            assert!(
                !names
                    .iter()
                    .any(|s| s == "MainClientApplicationWindowInterfaceVariant")
            );
        }
        let reread = read(&bytes);
        assert_eq!(
            value(&reread, cfg::F_MAIN_WINDOW_MODE),
            value(&obj, cfg::F_MAIN_WINDOW_MODE)
        );
        if target.minor == 21 {
            assert_eq!(
                value(&reread, cfg::F_CLIENT_APPLICATION_THEME),
                value(&obj, cfg::F_CLIENT_APPLICATION_THEME)
            );
        }
        assert_eq!(bytes, write(&obj, target, target));
        assert!(
            String::from_utf8(bytes)
                .unwrap()
                .contains(&format!("version=\"{target}\""))
        );
    }
}

#[test]
fn scalar_reordering_does_not_reorder_current_nested_records() {
    let mut obj = fixture();
    let rows = PropertyValue::List(vec![
        PropertyValue::List(
            ["text/z", "", "z", "0", "false"]
                .map(|s| PropertyValue::Str(s.into()))
                .to_vec(),
        ),
        PropertyValue::List(
            ["text/a", "", "a", "1", "true"]
                .map(|s| PropertyValue::Str(s.into()))
                .to_vec(),
        ),
    ]);
    obj.properties
        .retain(|(id, _)| *id != cfg::F_ALLOWED_INCOMING_SHARE_TYPES);
    obj.properties
        .push((cfg::F_ALLOWED_INCOMING_SHARE_TYPES, rows.clone()));
    let bytes = write(&obj, FormatVersion::new(2, 21), FormatVersion::new(2, 20));
    assert_eq!(
        value(&read(&bytes), cfg::F_ALLOWED_INCOMING_SHARE_TYPES),
        &rows
    );
}

#[test]
#[ignore = "requires immutable genuine BSP83/BSP85 SDK lab captures"]
fn genuine_both_profiles_equal_matched_native_sdk_configuration_bytes() {
    let lab = std::path::PathBuf::from(std::env::var_os("IBCMD_EDT_LAB").expect("IBCMD_EDT_LAB"));
    for (minor, source, expected) in [
        (
            20,
            "accept-bsp83-491ccee5-current-r1/ours-authentic-edt-xml/Configuration.xml",
            "native-reference-bsp83-r2/native-xml/Configuration.xml",
        ),
        (
            21,
            "accept-bsp85-491ccee5-current-r1/ours-authentic-edt-xml/Configuration.xml",
            "native-reference-bsp85-r4/native-xml/Configuration.xml",
        ),
        (
            20,
            "oracle-bsp83-r1/edt-native-xml/Configuration.xml",
            "native-reference-bsp83-r2/native-xml/Configuration.xml",
        ),
        (
            21,
            "oracle-bsp85-r3/edt-native-xml/Configuration.xml",
            "native-reference-bsp85-r4/native-xml/Configuration.xml",
        ),
    ] {
        let original = std::fs::read(lab.join(source)).unwrap();
        let native = std::fs::read(lab.join(expected)).unwrap();
        let obj = read(&original);
        let target = FormatVersion::new(2, minor);
        let opposite = FormatVersion::new(2, if minor == 20 { 21 } else { 20 });
        assert!(
            write(&read(&native), target, opposite) == native,
            "profile 2.{minor} matched native same-source byte mismatch"
        );
        let generated = write(&obj, target, opposite);
        assert!(
            generated == native,
            "profile 2.{minor} genuine Configuration byte mismatch"
        );
    }
}

#[test]
fn versioned_kind_placement_keeps_current_names_and_default_facade_order() {
    use formats_xml::configuration::{
        ConfigDialect, emit_child_objects, emit_child_objects_versioned,
    };
    let pairs = [
        ("DefinedType", "D"),
        ("CommonCommand", "C2"),
        ("CommonCommand", "C1"),
        ("PaletteColor", "Z"),
        ("PaletteColor", "A"),
    ];
    let rows = PropertyValue::List(
        pairs
            .iter()
            .map(|(k, n)| {
                PropertyValue::List(vec![
                    PropertyValue::Str((*k).into()),
                    PropertyValue::Str((*n).into()),
                ])
            })
            .collect(),
    );
    let names = |els: Vec<formats_xml::OutElement>| {
        els.into_iter()
            .next()
            .unwrap()
            .children
            .into_iter()
            .map(|e| e.text.unwrap())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        names(emit_child_objects(ConfigDialect::Designer, &rows).unwrap()),
        ["D", "C2", "C1", "Z", "A"]
    );
    assert_eq!(
        names(
            emit_child_objects_versioned(ConfigDialect::Designer, &rows, FormatVersion::new(2, 20))
                .unwrap()
        ),
        ["D", "C2", "C1", "Z", "A"]
    );
    assert_eq!(
        names(
            emit_child_objects_versioned(ConfigDialect::Designer, &rows, FormatVersion::new(2, 21))
                .unwrap()
        ),
        ["D", "Z", "A", "C2", "C1"]
    );
}

#[test]
fn production_semantic_view_keeps_current_roster_values_through_physical_mapping() {
    let mut obj = read(&write(
        &fixture(),
        FormatVersion::new(2, 21),
        FormatVersion::new(2, 21),
    ));
    let pairs = [
        ("DefinedType", "D"),
        ("CommonCommand", "C"),
        ("PaletteColor", "Z"),
        ("PaletteColor", "A"),
    ];
    obj.properties.retain(|(id, _)| *id != cfg::F_CHILD_OBJECTS);
    obj.properties.push((
        cfg::F_CHILD_OBJECTS,
        PropertyValue::List(
            pairs
                .iter()
                .map(|(k, n)| {
                    PropertyValue::List(vec![
                        PropertyValue::Str((*k).into()),
                        PropertyValue::Str((*n).into()),
                    ])
                })
                .collect(),
        ),
    ));
    let version = FormatVersion::new(2, 21);
    let reread = read(&write(&obj, version, FormatVersion::new(2, 20)));
    let semantic = |root: MetadataObject| {
        let config = morph1c_core::ir::Configuration {
            source_version: Some(version),
            properties: Vec::new(),
            objects: vec![root],
        };
        serde_json::to_vec(
            &morph1c_core::ir::semantic_view::ConfigurationSemanticView {
                configuration: &config,
                template_body: morph1c_pipeline::dcs_template_semantic_body,
            },
        )
        .unwrap()
    };
    assert!(
        semantic(obj) == semantic(reread),
        "current Configuration roster semantic view changed after native physical-kind placement"
    );
}

fn semantic_root(root: MetadataObject) -> Result<Vec<u8>, serde_json::Error> {
    let config = morph1c_core::ir::Configuration {
        source_version: Some(FormatVersion::new(2, 21)),
        properties: Vec::new(),
        objects: vec![root],
    };
    serde_json::to_vec(
        &morph1c_core::ir::semantic_view::ConfigurationSemanticView {
            configuration: &config,
            template_body: morph1c_pipeline::dcs_template_semantic_body,
        },
    )
}
fn roster(root: &mut MetadataObject, pairs: &[(&str, &str)]) {
    root.properties
        .retain(|(id, _)| *id != cfg::F_CHILD_OBJECTS);
    root.properties.push((
        cfg::F_CHILD_OBJECTS,
        PropertyValue::List(
            pairs
                .iter()
                .map(|(k, n)| {
                    PropertyValue::List(vec![
                        PropertyValue::Str((*k).into()),
                        PropertyValue::Str((*n).into()),
                    ])
                })
                .collect(),
        ),
    ));
}
#[test]
fn semantic_roster_keeps_names_counts_and_each_current_order_authoritative() {
    let mut original = fixture();
    roster(
        &mut original,
        &[
            ("DefinedType", "D"),
            ("CommonCommand", "C"),
            ("PaletteColor", "Z"),
            ("PaletteColor", "A"),
        ],
    );
    let expected = semantic_root(original.clone()).unwrap();
    let original_wire = serde_json::to_vec(&original).unwrap();
    let mut placement = original.clone();
    roster(
        &mut placement,
        &[
            ("DefinedType", "D"),
            ("PaletteColor", "Z"),
            ("PaletteColor", "A"),
            ("CommonCommand", "C"),
        ],
    );
    assert_eq!(expected, semantic_root(placement).unwrap());
    assert_eq!(original_wire, serde_json::to_vec(&original).unwrap());
    for pairs in [
        vec![
            ("DefinedType", "D"),
            ("CommonCommand", "C"),
            ("PaletteColor", "A"),
            ("PaletteColor", "Z"),
        ],
        vec![
            ("DefinedType", "D"),
            ("CommonCommand", "C"),
            ("PaletteColor", "Edited"),
            ("PaletteColor", "A"),
        ],
        vec![
            ("DefinedType", "D"),
            ("CommonCommand", "C"),
            ("PaletteColor", "Z"),
        ],
        vec![
            ("DefinedType", "D"),
            ("CommonCommand", "C"),
            ("PaletteColor", "Z"),
            ("PaletteColor", "A"),
            ("PaletteColor", "A"),
        ],
        vec![
            ("CommonCommand", "C"),
            ("DefinedType", "D"),
            ("PaletteColor", "Z"),
            ("PaletteColor", "A"),
        ],
    ] {
        let mut edited = original.clone();
        roster(&mut edited, &pairs);
        assert_ne!(expected, semantic_root(edited).unwrap());
    }
    for bad in [
        PropertyValue::Str("bad".into()),
        PropertyValue::List(vec![PropertyValue::List(vec![PropertyValue::Str(
            "PaletteColor".into(),
        )])]),
        PropertyValue::List(vec![PropertyValue::List(vec![
            PropertyValue::Str("UnknownKind".into()),
            PropertyValue::Str("A".into()),
        ])]),
    ] {
        let mut edited = original.clone();
        edited
            .properties
            .retain(|(id, _)| *id != cfg::F_CHILD_OBJECTS);
        edited.properties.push((cfg::F_CHILD_OBJECTS, bad));
        assert!(semantic_root(edited).is_err());
    }
    let mut unrelated = original.clone();
    unrelated.kind = morph1c_core::ir::ObjectKind::new("Catalog");
    let mut unrelated_moved = unrelated.clone();
    roster(
        &mut unrelated_moved,
        &[
            ("DefinedType", "D"),
            ("PaletteColor", "Z"),
            ("PaletteColor", "A"),
            ("CommonCommand", "C"),
        ],
    );
    assert_ne!(
        semantic_root(unrelated).unwrap(),
        semantic_root(unrelated_moved).unwrap()
    );
}

#[test]
fn public_palette_transport_preserves_exact_source_and_rejects_rehashed_roster_edit() {
    use ibcmd_edt::{
        ConversionOptions, Project, ReaderLimits, edt_to_xml, read_xml_source, xml_to_edt,
    };
    use ibcmd_xml::source_tree::{SourceEntry, SourcePath, SourceTree};
    use morph1c_core::ir::{ObjectKind, Uuid};
    use sha2::{Digest, Sha256};
    let version = FormatVersion::new(2, 21);
    let mut source = read_xml_source(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/subsystem-ci/src"
        ),
        ReaderLimits::default(),
    )
    .unwrap()
    .entries()
    .to_vec();
    let mut root = fixture();
    let PropertyValue::List(rows) = root
        .properties
        .iter_mut()
        .find(|(id, _)| *id == cfg::F_CHILD_OBJECTS)
        .map(|(_, v)| v)
        .unwrap()
    else {
        panic!()
    };
    for (kind, name) in [
        ("CommonCommand", "Example"),
        ("PaletteColor", "Z"),
        ("PaletteColor", "A"),
    ] {
        rows.push(PropertyValue::List(vec![
            PropertyValue::Str(kind.into()),
            PropertyValue::Str(name.into()),
        ]));
    }
    source.retain(|e| e.path().as_str() != "Configuration.xml");
    let bytes = write(&root, version, version);
    source.push(
        SourceEntry::from_bytes(SourcePath::new("Configuration.xml").unwrap(), bytes).unwrap(),
    );
    for (n, name) in [(0x71, "Z"), (0x72, "A")] {
        let mut obj = MetadataObject::new(ObjectKind::new("PaletteColor"), name, Uuid([n; 16]));
        obj.properties.push((
            morph1c_core::spec::metadata::palette_color::F_COLOR,
            PropertyValue::Enum(Token::new("#123456")),
        ));
        let bytes = with_roundtrip_target(version, || {
            (FormatRegistry::for_format(Format::Designer)
                .unwrap()
                .get("PaletteColor")
                .unwrap()
                .write)(&obj)
        })
        .unwrap();
        source.push(
            SourceEntry::from_bytes(
                SourcePath::new(format!("PaletteColors/{name}.xml")).unwrap(),
                bytes,
            )
            .unwrap(),
        );
    }
    let mut obj = MetadataObject::new(
        ObjectKind::new("CommonCommand"),
        "Example",
        Uuid([0x73; 16]),
    );
    obj.properties.push((
        morph1c_core::spec::metadata::common_command::F_GROUP,
        PropertyValue::Enum(Token::new("NavigationPanelOrdinary")),
    ));
    let bytes = with_roundtrip_target(version, || {
        (FormatRegistry::for_format(Format::Designer)
            .unwrap()
            .get("CommonCommand")
            .unwrap()
            .write)(&obj)
    })
    .unwrap();
    source.push(
        SourceEntry::from_bytes(
            SourcePath::new("CommonCommands/Example.xml").unwrap(),
            bytes,
        )
        .unwrap(),
    );
    let source = SourceTree::new(source).unwrap();
    let opts = ConversionOptions {
        edt_version: "2025.2.3".into(),
        xml_dialect: "2.21".into(),
        runtime_version: Some("8.5.1".into()),
    };
    let converted = xml_to_edt(&source, &opts).unwrap();
    let returned = edt_to_xml(&Project::from_tree(converted.tree.clone()).unwrap(), &opts).unwrap();
    assert_eq!(returned.tree, source);
    let stripped = SourceTree::new(
        converted
            .tree
            .entries()
            .iter()
            .filter(|e| !e.path().as_str().starts_with(".ibcmd-provenance/"))
            .cloned()
            .collect(),
    )
    .unwrap();
    let native = edt_to_xml(&Project::from_tree(stripped).unwrap(), &opts).unwrap();
    let current = native
        .tree
        .entries()
        .iter()
        .find(|e| e.path().as_str() == "Configuration.xml")
        .unwrap();
    assert_eq!(
        semantic_root(read(current.bytes())).unwrap(),
        semantic_root(root.clone()).unwrap()
    );
    let path = "src/Configuration/Configuration.mdo";
    let text = std::str::from_utf8(
        converted
            .tree
            .entries()
            .iter()
            .find(|e| e.path().as_str() == path)
            .unwrap()
            .bytes(),
    )
    .unwrap();
    let text = text
        .replace(
            "<paletteColors>PaletteColor.Z</paletteColors>",
            "<paletteColors>PaletteColor.Temporary</paletteColors>",
        )
        .replace(
            "<paletteColors>PaletteColor.A</paletteColors>",
            "<paletteColors>PaletteColor.Z</paletteColors>",
        )
        .replace(
            "<paletteColors>PaletteColor.Temporary</paletteColors>",
            "<paletteColors>PaletteColor.A</paletteColors>",
        );
    assert_ne!(
        text.as_bytes(),
        converted
            .tree
            .entries()
            .iter()
            .find(|e| e.path().as_str() == path)
            .unwrap()
            .bytes()
    );
    let retained_path = ".ibcmd-provenance/xml/Configuration.xml";
    let retained = std::str::from_utf8(
        converted
            .tree
            .entries()
            .iter()
            .find(|e| e.path().as_str() == retained_path)
            .unwrap()
            .bytes(),
    )
    .unwrap()
    .replace(
        "<PaletteColor>Z</PaletteColor>",
        "<PaletteColor>Temporary</PaletteColor>",
    )
    .replace(
        "<PaletteColor>A</PaletteColor>",
        "<PaletteColor>Z</PaletteColor>",
    )
    .replace(
        "<PaletteColor>Temporary</PaletteColor>",
        "<PaletteColor>A</PaletteColor>",
    );
    let mut entries = converted
        .tree
        .entries()
        .iter()
        .filter(|e| {
            e.path().as_str() != path
                && e.path().as_str() != retained_path
                && e.path().as_str() != ".ibcmd-provenance/manifest.json"
        })
        .cloned()
        .collect::<Vec<_>>();
    let mut manifest: serde_json::Value = serde_json::from_slice(
        converted
            .tree
            .entries()
            .iter()
            .find(|e| e.path().as_str() == ".ibcmd-provenance/manifest.json")
            .unwrap()
            .bytes(),
    )
    .unwrap();
    manifest["generated"][path] =
        serde_json::Value::String(format!("{:x}", Sha256::digest(text.as_bytes())));
    manifest["original"]["Configuration.xml"] =
        serde_json::Value::String(format!("{:x}", Sha256::digest(retained.as_bytes())));
    entries.push(
        SourceEntry::from_bytes(
            SourcePath::new(retained_path).unwrap(),
            retained.into_bytes(),
        )
        .unwrap(),
    );
    entries
        .push(SourceEntry::from_bytes(SourcePath::new(path).unwrap(), text.into_bytes()).unwrap());
    entries.push(
        SourceEntry::from_bytes(
            SourcePath::new(".ibcmd-provenance/manifest.json").unwrap(),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap(),
    );
    assert!(
        edt_to_xml(
            &Project::from_tree(SourceTree::new(entries).unwrap()).unwrap(),
            &opts
        )
        .is_err()
    );
}
