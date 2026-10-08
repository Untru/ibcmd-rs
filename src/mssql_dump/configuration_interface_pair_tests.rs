//! Own production-compiled Configuration controls; no copied native fixture.
use super::*;
use crate::metadata_model::{
    DescriptorContext,
    brace::{Brace, parse_row, serialize, serialize_row},
    compile_descriptor,
    export::{ExportContext, NameIndex, export_descriptor},
    objects::parts::compatibility,
};

const ROOT_UUID: &str = "10000000-0000-4000-8000-000000000456";

fn fixture(dialect: &str, mode: &str, v76: bool) -> (Brace, DescriptorContext) {
    fixture_on_compatibility(dialect, mode, v76, "Version8_3_27")
}

fn fixture_on_compatibility(
    dialect: &str,
    mode: &str,
    v76: bool,
    compatibility_name: &str,
) -> (Brace, DescriptorContext) {
    let storage_dialect = if v76 { "2.21" } else { dialect };
    let info = [
        "9cd510cd-abfc-11d4-9434-004095e12fc7",
        "9fcd25a0-4822-11d4-9414-008048da11f9",
        "e3687481-0a87-462c-a166-9f34594f9bba",
        "9de14907-ec23-4a07-96f0-85521cb6b53b",
        "51f2d5d8-ea4d-4064-8892-82951750031e",
        "e68182ea-4237-4383-967f-90c1e3370bc7",
        "fb282519-d103-4dd3-bc12-cb271d631dfc",
    ].iter().enumerate().map(|(index, class)| format!(
        "<xr:ContainedObject><xr:ClassId>{class}</xr:ClassId><xr:ObjectId>20000000-0000-4000-8000-{:012}</xr:ObjectId></xr:ContainedObject>", index+1
    )).collect::<String>();
    let children = if v76 {
        "<CommonForm>LayoutWitness</CommonForm>"
    } else {
        ""
    };
    let source = format!(
        "<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" version=\"{storage_dialect}\"><Configuration uuid=\"{ROOT_UUID}\"><InternalInfo>{info}</InternalInfo><Properties><Name>OwnInterfacePair</Name><Synonym/><Comment/><DefaultRunMode>ManagedApplication</DefaultRunMode><ScriptVariant>Russian</ScriptVariant><CompatibilityMode>{compatibility_name}</CompatibilityMode><InterfaceCompatibilityMode>{mode}</InterfaceCompatibilityMode></Properties><ChildObjects>{children}</ChildObjects></Configuration></MetaDataObject>"
    );
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!(
        "ibcmd-pair-owned-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("Configuration.xml");
    std::fs::write(&path, &source).unwrap();
    let mut files = vec![(
        path.clone(),
        std::sync::Arc::new(source.as_bytes().to_vec()),
    )];
    if v76 {
        // Actual selected source/storage proof used by the production layout
        // selector: a declared own managed form whose 2.21 root may omit its
        // WindowOpeningMode and Group. No caller-supplied layout callback.
        let descriptor = "<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" version=\"2.21\"><CommonForm uuid=\"30000000-0000-4000-8000-000000000456\"><Properties><Name>LayoutWitness</Name><Synonym/><Comment/><FormType>Managed</FormType></Properties></CommonForm></MetaDataObject>";
        let form = "<Form xmlns=\"http://v8.1c.ru/8.3/xcf/logform\" version=\"2.21\">\r\n\t<AutoCommandBar name=\"FormCommandBar\" id=\"-1\">\r\n\t\t<Autofill>true</Autofill>\r\n\t</AutoCommandBar>\r\n</Form>";
        let descriptor_path = root.join("CommonForms/LayoutWitness.xml");
        std::fs::create_dir_all(root.join("CommonForms/LayoutWitness/Ext")).unwrap();
        std::fs::write(&descriptor_path, descriptor).unwrap();
        std::fs::write(root.join("CommonForms/LayoutWitness/Ext/Form.xml"), form).unwrap();
        files.push((
            descriptor_path,
            std::sync::Arc::new(descriptor.as_bytes().to_vec()),
        ));
    }
    let context = DescriptorContext::with_files(&root, storage_dialect, &files).unwrap();
    let bytes = compile_descriptor("Configuration", &path, source.as_bytes(), &context).unwrap();
    (parse_row(&bytes).unwrap(), context)
}

fn tuple(row: &Brace) -> &[Brace] {
    row.at(&[3, 1, 1]).unwrap().as_list().unwrap()
}

fn tuple_mut(row: &mut Brace) -> &mut Vec<Brace> {
    row.as_list_mut().unwrap()[3].as_list_mut().unwrap()[1]
        .as_list_mut()
        .unwrap()[1]
        .as_list_mut()
        .unwrap()
}

fn canonical(row: &Brace, context: &DescriptorContext, dialect: &str) -> anyhow::Result<String> {
    export_descriptor(
        "Configuration",
        &serialize_row(row),
        &ExportContext {
            names: NameIndex::from_config_index(&context.index),
            version: dialect.to_owned(),
            compat: compatibility(context),
        },
    )
}

fn physical(
    row: &Brace,
    context: &DescriptorContext,
    version: InfobaseConfigSourceVersion,
) -> std::result::Result<ExtractedMetadataSourceXml, MetadataSourceExtractionDiagnostic> {
    let object_refs = context
        .index
        .objects
        .iter()
        .map(|(name, entry)| (entry.uuid.clone(), name.clone()))
        .collect::<BTreeMap<_, _>>();
    let row = MetadataTextRow {
        file_name: ROOT_UUID.to_owned(),
        text: serialize(row),
        object_code: Some(2),
        header: Some(MetadataHeader {
            uuid: ROOT_UUID.to_owned(),
            name: "OwnInterfacePair".to_owned(),
            synonyms: vec![],
            comment: String::new(),
            template_type_code: None,
        }),
        kind: Some("Configuration".to_owned()),
        folder: Some(""),
    };
    extract_metadata_source_xml_from_text_row_audited(
        &row,
        &BTreeMap::new(),
        &BTreeSet::new(),
        &object_refs,
        &object_refs,
        &object_refs,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        version,
    )
}

#[test]
fn compiled_v76_pairs_preserve_complete_xml_and_current_edits_in_admitted_profile() {
    let dialect = "2.21";
    let version = InfobaseConfigSourceVersion::V2_21;
    let modes = [
        ("Taxi", 3, 3),
        ("TaxiEnableVersion8_2", 2, 2),
        ("Version8_5EnableTaxi", 3, 6),
    ];
    for (name, a, b) in modes {
        let (row, context) = fixture(dialect, name, true);
        let fields = tuple(&row);
        assert_eq!(fields.len(), 77);
        assert_eq!(fields[0].as_atom(), Some("76"));
        assert_eq!(fields[38], Brace::num(a));
        assert_eq!(fields[62], Brace::num(b));
        let exported = canonical(&row, &context, dialect).unwrap();
        assert!(exported.contains(&format!(
            "<InterfaceCompatibilityMode>{name}</InterfaceCompatibilityMode>"
        )));
        let read = physical(&row, &context, version).unwrap();
        assert_eq!(read.relative_path, PathBuf::from("Configuration.xml"));
        assert_eq!(
            read.xml,
            exported.as_bytes(),
            "complete canonical/physical XML: {dialect}/{name}"
        );
        let returned = compile_descriptor(
            "Configuration",
            &context.root.join("Configuration.xml"),
            exported.as_bytes(),
            &context,
        )
        .unwrap();
        assert_eq!(
            parse_row(&returned).unwrap(),
            row,
            "complete current stored semantics: {dialect}/{name}"
        );
        // A current XML edit is encoded afresh, not masked by old digits.
        let replacement = if name == "Taxi" {
            "TaxiEnableVersion8_2"
        } else {
            "Taxi"
        };
        let edited_xml = exported.replace(
            &format!("<InterfaceCompatibilityMode>{name}</InterfaceCompatibilityMode>"),
            &format!("<InterfaceCompatibilityMode>{replacement}</InterfaceCompatibilityMode>"),
        );
        let edited = parse_row(
            &compile_descriptor(
                "Configuration",
                &context.root.join("Configuration.xml"),
                edited_xml.as_bytes(),
                &context,
            )
            .unwrap(),
        )
        .unwrap();
        let mut expected = row.clone();
        let (first, second) = if replacement == "Taxi" {
            (3, 3)
        } else {
            (2, 2)
        };
        tuple_mut(&mut expected)[38] = Brace::num(first);
        tuple_mut(&mut expected)[62] = Brace::num(second);
        assert_eq!(edited, expected);
        assert_eq!(canonical(&edited, &context, dialect).unwrap(), edited_xml);
        assert_eq!(
            physical(&edited, &context, version).unwrap().xml,
            edited_xml.as_bytes()
        );
    }
}

#[test]
fn known_v76_invalid_pair_is_an_addressed_publication_failure_not_a_generic_root() {
    for version in [
        InfobaseConfigSourceVersion::V2_20,
        InfobaseConfigSourceVersion::V2_21,
    ] {
        let (original, context) = fixture("2.21", "Taxi", true);
        // Establish the SAME complete producer input before negative edits.
        assert_eq!(
            physical(&original, &context, InfobaseConfigSourceVersion::V2_21)
                .unwrap()
                .xml,
            canonical(&original, &context, "2.21").unwrap().as_bytes()
        );
        for invalid in [
            Brace::num(0),
            Brace::num(2),
            Brace::num(5),
            Brace::atom("03"),
            Brace::str("3"),
            Brace::List(vec![Brace::num(3)]),
        ] {
            let mut changed = original.clone();
            tuple_mut(&mut changed)[62] = invalid;
            assert!(canonical(&changed, &context, "2.21").is_err());
            let failure = physical(&changed, &context, version).err().unwrap();
            assert_eq!(failure.family, "Configuration");
            assert_eq!(failure.class, MetadataSourceFailureClass::Malformed);
            assert_eq!(failure.parser_stage, "interface_compatibility_pair");
            assert!(matches!(
                failure.structural_signature.as_str(),
                "unknown_v76_interface_pair" | "invalid_v76_interface_scalar"
            ));
        }
        let mut shortened = original.clone();
        tuple_mut(&mut shortened).remove(62);
        assert!(canonical(&shortened, &context, "2.21").is_err());
        let failure = physical(&shortened, &context, version).err().unwrap();
        assert_eq!(failure.structural_signature, "invalid_v76_tuple_arity");
        assert_eq!(failure.parser_stage, "interface_compatibility_pair");
        assert!(
            refs::configuration_v76_interface_mode(
                &serialize(&original),
                "90000000-0000-4000-8000-000000000456"
            )
            .unwrap()
            .is_none()
        );
        assert_eq!(tuple(&original)[38], Brace::num(3));
        assert_eq!(tuple(&original)[62], Brace::num(3));
    }
}

#[test]
fn normalized_member38_must_agree_with_the_owned_v76_pair() {
    let (row, _) = fixture("2.21", "Taxi", true);
    let stored = tuple(&row)
        .iter()
        .take(61)
        .map(serialize)
        .collect::<Vec<_>>();
    let mut fields = stored.iter().map(String::as_str).collect::<Vec<_>>();
    fields[0] = "68";
    fields[38] = "2";
    assert_eq!(
        configuration_properties_evidence::parse_configuration_properties_evidenced_default_block_on(&fields, Some(ibcmd_schema::configuration_root::V76InterfaceCompatibility::Taxi)),
        Err(configuration_properties_evidence::ConfigurationPropertiesEvidenceError::UnrecognizedDigit {field:"InterfaceCompatibilityMode",byte:b'2'})
    );
}

#[test]
fn older_single_coordinate_modes_keep_their_original_mapping_both_editions() {
    for (dialect, version) in [
        ("2.20", InfobaseConfigSourceVersion::V2_20),
        ("2.21", InfobaseConfigSourceVersion::V2_21),
    ] {
        for (mode, value) in [("Version8_2", 0), ("TaxiEnableVersion8_2", 2), ("Taxi", 3)] {
            let (row, context) = fixture(dialect, mode, false);
            assert_eq!(tuple(&row).len(), 61);
            assert_eq!(tuple(&row)[0].as_atom(), Some("68"));
            assert_eq!(tuple(&row)[38], Brace::num(value));
            assert_eq!(
                refs::configuration_v76_interface_mode(&serialize(&row), ROOT_UUID).unwrap(),
                None
            );
            let exported = canonical(&row, &context, dialect).unwrap();
            assert!(exported.contains(&format!(
                "<InterfaceCompatibilityMode>{mode}</InterfaceCompatibilityMode>"
            )));
            assert_eq!(
                physical(&row, &context, version).unwrap().xml,
                exported.as_bytes()
            );
        }
    }
}

#[test]
fn all_known_v76_modes_keep_whole_root_profile_refusal_without_partial_publication() {
    for mode in ["Taxi", "TaxiEnableVersion8_2", "Version8_5EnableTaxi"] {
        let (row, context) = fixture("2.21", mode, true);
        let original = row.clone();
        assert_eq!(
            physical(&row, &context, InfobaseConfigSourceVersion::V2_21)
                .unwrap()
                .xml,
            canonical(&row, &context, "2.21").unwrap().as_bytes()
        );
        let failure = canonical(&row, &context, "2.20").err().unwrap();
        assert!(
            failure
                .to_string()
                .contains("does not read a configuration")
        );
        let failure = physical(&row, &context, InfobaseConfigSourceVersion::V2_20)
            .err()
            .unwrap();
        assert_eq!(failure.class, MetadataSourceFailureClass::Unsupported);
        assert_eq!(failure.family, "Configuration");
        assert_eq!(failure.parser_stage, "configuration_root_profile");
        assert_eq!(failure.structural_signature, "v76_root_requires_xml_2_21");
        assert!(
            refs::extract_configuration_source_xml(
                &serialize(&row),
                ROOT_UUID,
                &BTreeMap::new(),
                InfobaseConfigSourceVersion::V2_20
            )
            .is_none()
        );
        assert_eq!(row, original);
    }
}

#[test]
fn v76_new_interface_mode_refuses_unrepresentable_read_and_source_editions() {
    let (row, context) = fixture("2.21", "Version8_5EnableTaxi", true);
    let positive = canonical(&row, &context, "2.21").unwrap();
    assert_eq!(
        physical(&row, &context, InfobaseConfigSourceVersion::V2_21)
            .unwrap()
            .xml,
        positive.as_bytes()
    );
    assert_eq!(tuple(&row)[38], Brace::num(3));
    assert_eq!(tuple(&row)[62], Brace::num(6));
    let failure = canonical(&row, &context, "2.20").err().unwrap();
    assert!(
        failure
            .to_string()
            .contains("does not read a configuration")
    );
    let failure = physical(&row, &context, InfobaseConfigSourceVersion::V2_20)
        .err()
        .unwrap();
    assert_eq!(failure.class, MetadataSourceFailureClass::Unsupported);
    assert_eq!(failure.family, "Configuration");
    assert_eq!(failure.parser_stage, "configuration_root_profile");
    assert_eq!(failure.structural_signature, "v76_root_requires_xml_2_21");
    assert!(
        refs::extract_configuration_source_xml(
            &serialize(&row),
            ROOT_UUID,
            &BTreeMap::new(),
            InfobaseConfigSourceVersion::V2_20
        )
        .is_none()
    );
    // Keep the SAME admitted storage graph: only the XML declaration changes.
    let source = positive.replacen("version=\"2.21\"", "version=\"2.20\"", 1);
    assert_ne!(source, positive);
    let failure = compile_descriptor(
        "Configuration",
        &context.root.join("Configuration.xml"),
        source.as_bytes(),
        &context,
    )
    .err()
    .unwrap();
    assert!(failure.to_string().contains("requires XML 2.21"));
    assert_eq!(
        parse_row(
            &compile_descriptor(
                "Configuration",
                &context.root.join("Configuration.xml"),
                positive.as_bytes(),
                &context
            )
            .unwrap()
        )
        .unwrap(),
        row
    );
    assert_eq!(tuple(&row)[62], Brace::num(6));
}

#[test]
fn true_v85_compatibility_does_not_become_an_admitted_v83_reading() {
    let (row, context) = fixture_on_compatibility("2.21", "Taxi", true, "Version8_5_1");
    assert_eq!(tuple(&row)[26], Brace::num(80501));
    assert_eq!(tuple(&row)[43], Brace::num(80501));
    assert!(canonical(&row, &context, "2.21").is_ok());
    let failure = canonical(&row, &context, "2.20").err().unwrap();
    assert!(
        failure
            .to_string()
            .contains("does not read a configuration")
    );
    assert_eq!(tuple(&row)[38], Brace::num(3));
    assert_eq!(tuple(&row)[62], Brace::num(3));
}

#[test]
#[ignore = "Retained F-lab native83/native85 pairs; ROOT runs with original SHA pins"]
fn genuine_native_v76_taxi_and_v68_taxi_match_complete_configuration_xml() {
    use sha2::{Digest, Sha256};
    let base = Path::new("F:/ibcmd/lab/05/pr-review-2026-10-07");
    for (profile, run, dialect, version, row_sha, xml_sha) in [
        (
            "83",
            "baseline-v2-8.3.27.2214",
            "2.20",
            InfobaseConfigSourceVersion::V2_20,
            "081ee1f6803ebac13debcca228a23e1558a60833a3bab2dabc026e2abb61d959",
            "7384023cc190cf62763b983b2f882627f33f14c7ed84f6f4071ab873c46a9e9a",
        ),
        (
            "85",
            "baseline-v3-8.5.1.1529",
            "2.21",
            InfobaseConfigSourceVersion::V2_21,
            "1f20289abe5db807f0c8782b9a4ba13192febf3f5f7a9031e318f8a8020cb409",
            "32e97cb2bf2bd6cb1a81412b615e6c89a7edbe63178a1ce826f58d6f43460c9b",
        ),
    ] {
        let bytes = fs::read(
            base.join("cf-remaining-families-440-audit-v1")
                .join(format!("{profile}-configuration-row.txt")),
        )
        .unwrap();
        let tree = base
            .join("public-native-fixture-e0b93d/planned-runs")
            .join(run)
            .join("native/configuration-xml");
        let expected = fs::read(tree.join("Configuration.xml")).unwrap();
        assert_eq!(format!("{:x}", Sha256::digest(&bytes)), row_sha);
        assert_eq!(format!("{:x}", Sha256::digest(&expected)), xml_sha);
        let row = parse_row(&bytes).unwrap();
        let uuid = row.at(&[1, 0]).unwrap().as_atom().unwrap();
        let context = DescriptorContext::new(&tree, dialect).unwrap();
        let refs = context
            .index
            .objects
            .iter()
            .map(|(name, entry)| (entry.uuid.clone(), name.clone()))
            .collect::<BTreeMap<_, _>>();
        let text = String::from_utf8(bytes.clone()).unwrap();
        let actual = refs::extract_configuration_source_xml(&text, uuid, &refs, version).unwrap();
        assert_eq!(
            actual.as_bytes(),
            expected,
            "physical complete native {profile}"
        );
        assert_eq!(
            canonical(&row, &context, dialect).unwrap().as_bytes(),
            expected,
            "canonical complete native {profile}"
        );
        assert_eq!(
            format!(
                "{:x}",
                Sha256::digest(fs::read(tree.join("Configuration.xml")).unwrap())
            ),
            xml_sha
        );
    }
}
