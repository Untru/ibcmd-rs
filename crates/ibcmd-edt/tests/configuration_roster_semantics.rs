use morph1c_core::ir::semantic_view::ConfigurationSemanticView;
use morph1c_core::ir::{Configuration, MetadataObject, ObjectKind, PropertyValue, Uuid};
use morph1c_core::spec::metadata::configuration as cfg;
use morph1c_core::version::FormatVersion;

fn roster(pairs: &[(&str, &str)]) -> PropertyValue {
    PropertyValue::List(
        pairs
            .iter()
            .map(|(kind, name)| {
                PropertyValue::List(vec![
                    PropertyValue::Str((*kind).into()),
                    PropertyValue::Str((*name).into()),
                ])
            })
            .collect(),
    )
}

fn configuration(kind: &str, value: PropertyValue) -> Configuration {
    let mut object = MetadataObject::new(ObjectKind::new(kind), "Current", Uuid([0x23; 16]));
    object.properties.push((cfg::F_CHILD_OBJECTS, value));
    Configuration {
        source_version: Some(FormatVersion::new(2, 21)),
        properties: vec![],
        objects: vec![object],
    }
}

fn semantic(config: &Configuration) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(&ConfigurationSemanticView {
        configuration: config,
        template_body: morph1c_pipeline::dcs_template_semantic_body,
    })
}

#[test]
fn only_palette_block_location_is_physical_and_source_ir_is_unchanged() {
    let source = configuration(
        "Configuration",
        roster(&[
            ("Language", "CurrentLanguage"),
            ("DefinedType", "D"),
            ("CommonCommand", "C2"),
            ("CommonCommand", "C1"),
            ("PaletteColor", "Z"),
            ("PaletteColor", "A"),
        ]),
    );
    let native = configuration(
        "Configuration",
        roster(&[
            ("Language", "CurrentLanguage"),
            ("DefinedType", "D"),
            ("PaletteColor", "Z"),
            ("PaletteColor", "A"),
            ("CommonCommand", "C2"),
            ("CommonCommand", "C1"),
        ]),
    );
    let before = serde_json::to_vec(&source).unwrap();
    assert_ne!(before, serde_json::to_vec(&native).unwrap());
    assert_eq!(semantic(&source).unwrap(), semantic(&native).unwrap());
    assert_eq!(serde_json::to_vec(&source).unwrap(), before);

    let mut root_properties = source.clone();
    root_properties.properties = root_properties.objects[0].properties.clone();
    root_properties.objects.clear();
    let mut native_root = native.clone();
    native_root.properties = native_root.objects[0].properties.clone();
    native_root.objects.clear();
    assert_eq!(
        semantic(&root_properties).unwrap(),
        semantic(&native_root).unwrap()
    );
}

#[test]
fn current_names_membership_duplicates_and_each_kind_order_remain_semantic() {
    let original = configuration(
        "Configuration",
        roster(&[
            ("DefinedType", "D"),
            ("CommonCommand", "C2"),
            ("CommonCommand", "C1"),
            ("PaletteColor", "Z"),
            ("PaletteColor", "A"),
        ]),
    );
    let expected = semantic(&original).unwrap();
    for changed in [
        vec![
            ("DefinedType", "D"),
            ("CommonCommand", "C2"),
            ("CommonCommand", "C1"),
            ("PaletteColor", "Edited"),
            ("PaletteColor", "A"),
        ],
        vec![
            ("DefinedType", "D"),
            ("CommonCommand", "C2"),
            ("CommonCommand", "C1"),
            ("PaletteColor", "A"),
            ("PaletteColor", "Z"),
        ],
        vec![
            ("DefinedType", "D"),
            ("CommonCommand", "C2"),
            ("CommonCommand", "C1"),
            ("PaletteColor", "Z"),
        ],
        vec![
            ("DefinedType", "D"),
            ("CommonCommand", "C2"),
            ("CommonCommand", "C1"),
            ("PaletteColor", "Z"),
            ("PaletteColor", "A"),
            ("PaletteColor", "A"),
        ],
        vec![
            ("DefinedType", "D"),
            ("CommonCommand", "C1"),
            ("CommonCommand", "C2"),
            ("PaletteColor", "Z"),
            ("PaletteColor", "A"),
        ],
        vec![
            ("CommonCommand", "C2"),
            ("DefinedType", "D"),
            ("CommonCommand", "C1"),
            ("PaletteColor", "Z"),
            ("PaletteColor", "A"),
        ],
    ] {
        assert_ne!(
            semantic(&configuration("Configuration", roster(&changed))).unwrap(),
            expected
        );
    }
    let mut changed = original.clone();
    changed.objects[0].properties.push((
        cfg::F_ALLOWED_INCOMING_SHARE_TYPES,
        PropertyValue::List(vec![
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
        ]),
    ));
    let first = semantic(&changed).unwrap();
    let PropertyValue::List(rows) = &mut changed.objects[0].properties.last_mut().unwrap().1 else {
        unreachable!()
    };
    rows.reverse();
    assert_ne!(semantic(&changed).unwrap(), first);
}

#[test]
fn malformed_rosters_are_rejected_and_other_object_kinds_keep_order() {
    for malformed in [
        PropertyValue::Str("not a roster".into()),
        PropertyValue::List(vec![PropertyValue::Int(1)]),
        PropertyValue::List(vec![PropertyValue::List(vec![PropertyValue::Str(
            "PaletteColor".into(),
        )])]),
        roster(&[("UnknownKind", "Name")]),
        roster(&[("Configuration", "Name")]),
        roster(&[("Catalog.Attribute", "Name")]),
        roster(&[("PaletteColor", "")]),
    ] {
        assert!(semantic(&configuration("Configuration", malformed)).is_err());
    }
    let first = configuration(
        "CommonModule",
        roster(&[("CommonCommand", "C"), ("PaletteColor", "P")]),
    );
    let second = configuration(
        "CommonModule",
        roster(&[("PaletteColor", "P"), ("CommonCommand", "C")]),
    );
    assert_ne!(semantic(&first).unwrap(), semantic(&second).unwrap());
}
