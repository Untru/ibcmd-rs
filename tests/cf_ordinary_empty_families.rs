//! Independently authored metadata records from the existing family schemas.
//! No foreign CF, stored row, UUID or XML fixture is embedded here.

use flate2::{Compression, write::DeflateEncoder};
use ibcmd_cf::archive::decode_packed_archive;
use ibcmd_cf::payload::{PayloadEncoding, decode_payload};
use ibcmd_core::{
    artifact::{ProfileId, StorageProfileId},
    diagnostic::{ObjectPath, PathSegment, PropertyPath},
    identity::{LogicalIdentity, ObjectUuid},
    limits::ResourceLimits,
    model::{CanonicalConfiguration, CanonicalObject, CanonicalObjectParts, MetadataKind},
    provenance::{CanonicalAnchor, SourceProvenance},
    validate::validate_configuration,
};
use ibcmd_rs::{
    cli::InfobaseConfigSourceVersion,
    compiler::{
        graph::{ObjectStorageRoute, build_bootstrap_graph},
        identity::collect_bootstrap_identities,
        root::{ConfigurationBodyProperties, compile_configuration_body, compile_root},
        version::{SpecialEntryProfile, compile_version},
    },
    mssql_dump::{
        ForeignReferences, export_packed_cf_archive_to_source, export_packed_entries_to_source,
    },
    profile_registry::load_bundled_profile_registry,
};
use ibcmd_v8::writer::{Format15Document, Format15Element, write_format15_to_vec};
use std::{
    fs,
    io::{Cursor, Write},
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

const NIL: &str = "00000000-0000-0000-0000-000000000000";

fn uuid(seed: usize) -> String {
    format!("00000000-0000-4000-8000-{seed:012x}")
}

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static SEQUENCE: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "ibcmd-empty-families-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn header(seed: usize, name: &str) -> String {
    format!(
        "{{3,{{1,0,{}}},\"{name}\",{{1,\"en\",\"{name} label\"}},\"authored comment\",0,0,{NIL},0}}",
        uuid(seed)
    )
}

fn criterion(pattern: &str) -> String {
    let generated = (11..15).map(uuid).collect::<Vec<_>>().join(",");
    format!(
        "{{1,{{14,{generated},{{2,{},{pattern}}},{{0,0}},1,{NIL},{NIL},{{0}},{{0}},{{0}}}},2,{{00867c40-06b1-11d6-a3c7-0050bae0a776,0}},{{23fa3b84-220a-40e9-8331-e588bed87f7d,0}}}}",
        header(10, "EmptyCriterion")
    )
}

fn accounts(reference: &str) -> String {
    accounts_with_collections(reference, "{0}", "{0}")
}

fn accounts_with_collections(reference: &str, attributes: &str, sections: &str) -> String {
    let mut fields = vec!["0".to_owned(); 57];
    fields[0] = "32".into();
    for (offset, field) in fields[1..15].iter_mut().enumerate() {
        *field = uuid(110 + offset);
    }
    fields[15] = format!("{{0,{}}}", header(100, "EmptyAccounts"));
    fields[16] = "1".into();
    fields[18] = "{0,0}".into();
    fields[19] = reference.into();
    fields[21] = "\"\"".into();
    fields[22] = "8".into();
    fields[23] = "40".into();
    fields[27] = "1".into();
    for slot in [28, 29, 30, 40, 41, 42] {
        fields[slot] = NIL.into();
    }
    fields[31] = "2".into();
    fields[33] = "{1,{0,0}}".into();
    fields[38] = attributes.into();
    fields[39] = sections.into();
    for field in &mut fields[43..48] {
        *field = "{0}".into();
    }
    fields[48] = "{0,{0}}".into();
    fields[49] = "1".into();
    fields[50] = "{1,{0,0}}".into();
    fields[52] = "{1,2,0}".into();
    let collections = [
        "0df30176-6865-4787-9fc8-609eb144174f",
        "3daea016-69b7-4ed4-9453-127911372fe6",
        "4c7fec95-d1bd-4508-8a01-f1db090d9af8",
        "5372e285-03db-4f8c-8565-fe56f1aea40e",
        "6e65cbf5-daa8-4d8d-bef8-59723f4e5777",
        "78bd1243-c4df-46c3-8138-e147465cb9a4",
        "c70ca527-5042-4cad-a315-dcb4007e32a3",
    ]
    .map(|id| format!("{{{id},0}}"))
    .join(",");
    format!("{{1,{{{}}},7,{collections}}}", fields.join(","))
}

fn calculation(base: &str) -> String {
    calculation_with_collections(base, "{0}", "{0}")
}

fn calculation_with_collections(base: &str, attributes: &str, sections: &str) -> String {
    let mut fields = vec!["0".to_owned(); 63];
    fields[0] = "35".into();
    fields[1] = format!("{{0,{}}}", header(200, "EmptyCalculation"));
    for (offset, field) in fields[2..24].iter_mut().enumerate() {
        *field = uuid(210 + offset);
    }
    fields[24] = "1".into();
    fields[25] = "8".into();
    fields[26] = "1".into();
    fields[28] = base.into();
    fields[30] = "40".into();
    for slot in [32, 33, 34, 45, 46, 47] {
        fields[slot] = NIL.into();
    }
    fields[35] = "1".into();
    fields[36] = "{0,0}".into();
    fields[38] = "2".into();
    fields[40] = "{1,{0,0}}".into();
    fields[43] = attributes.into();
    fields[44] = sections.into();
    for field in &mut fields[48..53] {
        *field = "{0}".into();
    }
    fields[53] = "1".into();
    fields[54] = "{0,{0}}".into();
    fields[55] = "1".into();
    fields[56] = "{1,{0,0}}".into();
    fields[58] = "{1,2,0}".into();
    let collections = [
        "054aa8cf-faa6-4634-aef4-1087ca0d88fc",
        "0dc22ad2-476a-4794-afae-cfa7ed251752",
        "2e90c75b-2f0c-4899-a7d4-5426eaefc96e",
        "3daea016-69b7-4ed4-9453-127911372fe6",
        "a7f8f92a-7a4b-484b-937e-42d242e64144",
    ]
    .map(|id| format!("{{{id},0}}"))
    .join(",");
    format!("{{1,{{{}}},5,{collections}}}", fields.join(","))
}

fn packed(text: &str) -> Vec<u8> {
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(text.as_bytes()).unwrap();
    encoder.finish().unwrap()
}

fn export(
    rows: Vec<(String, String)>,
    profile: InfobaseConfigSourceVersion,
) -> (Scratch, serde_json::Value) {
    let elements = rows
        .into_iter()
        .map(|(name, text)| Format15Element::named(name, Some(packed(&text))))
        .collect();
    let bytes = write_format15_to_vec(&Format15Document::new(7, elements)).unwrap();
    let archive = decode_packed_archive(
        Cursor::new(bytes),
        ResourceLimits::default(),
        StorageProfileId::parse("storage:ordinary-empty-cleanroom").unwrap(),
    )
    .unwrap();
    let scratch = Scratch::new();
    let report = export_packed_cf_archive_to_source(archive, &scratch.0, true, profile).unwrap();
    (scratch, serde_json::to_value(report).unwrap())
}

fn all_rows() -> Vec<(String, String)> {
    vec![
        (uuid(10), criterion("{\"Pattern\"}")),
        (uuid(100), accounts(NIL)),
        (uuid(200), calculation("{0,0}")),
    ]
}

/// An indexed CF inventory includes the actual root/version service entries
/// and a complete canonical Configuration. A header-only stand-in cannot own
/// its versions inventory. This does not require the pending root rewrite.
fn indexed_rows() -> Vec<(String, String)> {
    let id = ProfileId::parse("platform-8.3.27.1989").unwrap();
    let objects = [
        (1000, "Configuration"),
        (10, "FilterCriterion"),
        (100, "ChartOfAccounts"),
        (200, "ChartOfCalculationTypes"),
    ]
    .map(|(seed, kind)| {
        let path = ObjectPath::new(vec![PathSegment::name(kind).unwrap()]).unwrap();
        CanonicalObject::new(CanonicalObjectParts::new(
            LogicalIdentity::new(ObjectUuid::parse(&uuid(seed)).unwrap(), path.clone()),
            MetadataKind::new(kind).unwrap(),
            SourceProvenance::new(id.clone(), CanonicalAnchor::new(path, PropertyPath::root())),
        ))
        .unwrap()
    });
    let configuration = CanonicalConfiguration::new(objects.to_vec()).unwrap();
    let validated = validate_configuration(&configuration).unwrap();
    let identities = collect_bootstrap_identities(&validated).unwrap();
    let routes = identities
        .objects()
        .iter()
        .map(|object| ObjectStorageRoute::new(object.uuid(), vec![]).unwrap())
        .collect();
    let graph = build_bootstrap_graph(&identities, id.clone(), routes).unwrap();
    let profiles = load_bundled_profile_registry().unwrap();
    let profile = SpecialEntryProfile::from_effective(profiles.get(&id).unwrap()).unwrap();
    let properties = ConfigurationBodyProperties::minimal("EmptyFamilies", profile.compatibility());
    let mut rows = all_rows();
    for entry in [
        compile_root(&graph, &profile).unwrap(),
        compile_version(&graph, &profile).unwrap(),
        compile_configuration_body(&identities, &graph, &profile, &properties).unwrap(),
    ] {
        let text = decode_payload(
            PayloadEncoding::RawDeflate,
            entry.outcome().compiled_payload().unwrap().bytes(),
            ResourceLimits::default(),
        )
        .unwrap()
        .into_bytes();
        rows.push((
            entry.target().key().as_str().into(),
            String::from_utf8(text).unwrap(),
        ));
    }
    rows
}

#[test]
fn ordinary_empty_families_export_complete_owned_metadata_from_public_cf_both_profiles() {
    for profile in [
        InfobaseConfigSourceVersion::V2_20,
        InfobaseConfigSourceVersion::V2_21,
    ] {
        let (scratch, report) = export(all_rows(), profile);
        assert_eq!(report["storage"]["failed"], 0, "{report}");
        assert_eq!(report["storage"]["opaque"], 0, "{report}");
        for (path, kind, name, owner, pairs, first) in [
            (
                "FilterCriteria/EmptyCriterion.xml",
                "FilterCriterion",
                "EmptyCriterion",
                10,
                2,
                11,
            ),
            (
                "ChartsOfAccounts/EmptyAccounts.xml",
                "ChartOfAccounts",
                "EmptyAccounts",
                100,
                7,
                110,
            ),
            (
                "ChartsOfCalculationTypes/EmptyCalculation.xml",
                "ChartOfCalculationTypes",
                "EmptyCalculation",
                200,
                11,
                210,
            ),
        ] {
            let xml = fs::read_to_string(scratch.0.join(path)).unwrap();
            assert!(
                xml.contains(&format!("<{kind} uuid=\"{}\">", uuid(owner))),
                "{xml}"
            );
            assert!(xml.contains(&format!("<Name>{name}</Name>")), "{xml}");
            assert!(xml.contains("authored comment"));
            assert!(xml.contains(&format!("{name} label")));
            assert_eq!(xml.matches("<xr:GeneratedType ").count(), pairs, "{xml}");
            for offset in 0..pairs {
                assert!(
                    xml.contains(&format!(
                        "<xr:TypeId>{}</xr:TypeId>",
                        uuid(first + 2 * offset)
                    )),
                    "{xml}"
                );
                assert!(
                    xml.contains(&format!(
                        "<xr:ValueId>{}</xr:ValueId>",
                        uuid(first + 2 * offset + 1)
                    )),
                    "{xml}"
                );
            }
            assert!(xml.contains("<UseStandardCommands>true</UseStandardCommands>"));
            assert!(!xml.contains("<StandardAttributes>"));
            assert!(
                !xml.contains("<StandardTabularSections"),
                "authored absence must remain absent: {xml}"
            );
            let dialect = match profile {
                InfobaseConfigSourceVersion::V2_20 => "2.20",
                InfobaseConfigSourceVersion::V2_21 => "2.21",
            };
            assert!(xml.contains(&format!("version=\"{dialect}\"")), "{xml}");
            match kind {
                "FilterCriterion" => {
                    assert!(xml.contains("<Type/>"));
                    assert!(xml.contains("<Content/>"));
                }
                "ChartOfAccounts" => {
                    assert!(xml.contains("<ExtDimensionTypes/>"));
                    assert!(xml.contains("<CodeLength>8</CodeLength>"));
                    assert!(xml.contains("<MaxExtDimensionCount>0</MaxExtDimensionCount>"));
                }
                _ => {
                    assert!(xml.contains("<BaseCalculationTypes/>"));
                    assert!(xml.contains("<CodeType>String</CodeType>"));
                    assert!(xml.contains(
                        "<DependenceOnCalculationTypes>DontUse</DependenceOnCalculationTypes>"
                    ));
                }
            }
        }
    }
}

#[test]
fn empty_admission_does_not_accept_missing_tokens_or_unresolved_nonempty_references() {
    for profile in [
        InfobaseConfigSourceVersion::V2_20,
        InfobaseConfigSourceVersion::V2_21,
    ] {
        for (owner, row, path) in [
            (10, criterion("{}"), "FilterCriteria/EmptyCriterion.xml"),
            (
                10,
                criterion("{\"Types\"}"),
                "FilterCriteria/EmptyCriterion.xml",
            ),
            (
                10,
                criterion("{\"Pattern\",{\"#\",00000000-0000-4000-8000-000000009999}}"),
                "FilterCriteria/EmptyCriterion.xml",
            ),
            (
                100,
                accounts(&uuid(999)),
                "ChartsOfAccounts/EmptyAccounts.xml",
            ),
            (100, accounts("{0,0}"), "ChartsOfAccounts/EmptyAccounts.xml"),
            (
                200,
                calculation("{0,1}"),
                "ChartsOfCalculationTypes/EmptyCalculation.xml",
            ),
            (
                200,
                calculation("{0,0,0}"),
                "ChartsOfCalculationTypes/EmptyCalculation.xml",
            ),
            (
                100,
                accounts(NIL).replacen(",7,{", ",8,{", 1),
                "ChartsOfAccounts/EmptyAccounts.xml",
            ),
            (
                200,
                calculation("{0,0}").replacen(",5,{", ",6,{", 1),
                "ChartsOfCalculationTypes/EmptyCalculation.xml",
            ),
            (
                100,
                accounts(NIL).replacen("{1,", "{99,", 1),
                "ChartsOfAccounts/EmptyAccounts.xml",
            ),
            (
                200,
                calculation("{0,0}").replacen("{1,", "{99,", 1),
                "ChartsOfCalculationTypes/EmptyCalculation.xml",
            ),
            (
                100,
                accounts(NIL).replace(
                    "{4c7fec95-d1bd-4508-8a01-f1db090d9af8,0}",
                    &format!("{{4c7fec95-d1bd-4508-8a01-f1db090d9af8,1,{}}}", uuid(998)),
                ),
                "ChartsOfAccounts/EmptyAccounts.xml",
            ),
            (
                200,
                calculation("{0,0}").replace(
                    "{2e90c75b-2f0c-4899-a7d4-5426eaefc96e,0}",
                    &format!("{{2e90c75b-2f0c-4899-a7d4-5426eaefc96e,1,{}}}", uuid(998)),
                ),
                "ChartsOfCalculationTypes/EmptyCalculation.xml",
            ),
            (
                100,
                accounts_with_collections(NIL, "{0,0}", "{0}"),
                "ChartsOfAccounts/EmptyAccounts.xml",
            ),
            (
                100,
                accounts_with_collections(NIL, "{0}", "{0,0}"),
                "ChartsOfAccounts/EmptyAccounts.xml",
            ),
            (
                200,
                calculation_with_collections("{0,0}", "{1,{1,0}}", "{0}"),
                "ChartsOfCalculationTypes/EmptyCalculation.xml",
            ),
            (
                200,
                calculation_with_collections("{0,0}", "{0}", "{1,{0,0}}"),
                "ChartsOfCalculationTypes/EmptyCalculation.xml",
            ),
        ] {
            let (scratch, report) = export(vec![(uuid(owner), row)], profile);
            assert!(
                !scratch.0.join(path).exists(),
                "owner {owner}, path {path}: {report}"
            );
            assert!(
                report["storage"]["opaque"].as_u64().unwrap()
                    + report["storage"]["failed"].as_u64().unwrap()
                    > 0,
                "unsupported record must remain accounted: {report}"
            );
        }
    }
}

#[test]
fn nonempty_primitive_criterion_remains_typed_on_both_profiles() {
    for profile in [
        InfobaseConfigSourceVersion::V2_20,
        InfobaseConfigSourceVersion::V2_21,
    ] {
        let (scratch, report) = export(
            vec![(uuid(10), criterion("{\"Pattern\",{\"S\",23,1}}"))],
            profile,
        );
        assert_eq!(report["storage"]["opaque"], 0, "{report}");
        assert_eq!(report["storage"]["failed"], 0, "{report}");
        let xml = fs::read_to_string(scratch.0.join("FilterCriteria/EmptyCriterion.xml")).unwrap();
        assert!(xml.contains("<v8:Type>xs:string</v8:Type>"), "{xml}");
        assert!(xml.contains("<v8:Length>23</v8:Length>"), "{xml}");
        assert!(!xml.contains("<Type/>"), "{xml}");
    }
}

#[test]
fn nonempty_chart_references_still_require_their_declared_family() {
    // Exercise the existing explicit foreign-reference adapter used by external
    // objects, independently of the empty ordinary-CF admission above.
    for profile in [
        InfobaseConfigSourceVersion::V2_20,
        InfobaseConfigSourceVersion::V2_21,
    ] {
        for (owner, reference, expected_kind, path, property) in [
            (
                100,
                uuid(900),
                "ChartOfCharacteristicTypes",
                "ChartsOfAccounts/EmptyAccounts.xml",
                "ExtDimensionTypes",
            ),
            (
                200,
                uuid(901),
                "ChartOfCalculationTypes",
                "ChartsOfCalculationTypes/EmptyCalculation.xml",
                "BaseCalculationTypes",
            ),
        ] {
            for family in [expected_kind, "Catalog"] {
                let row = if owner == 100 {
                    accounts(&reference)
                } else {
                    calculation(&format!(
                        "{{0,1,{{\"#\",157fa490-4ce9-11d4-9415-008048da11f9,{{1,{reference}}}}}}}"
                    ))
                };
                let scratch = Scratch::new();
                let foreign = ForeignReferences {
                    objects: [(reference.clone(), format!("{family}.Referenced"))].into(),
                    types: Vec::new(),
                };
                let report = export_packed_entries_to_source(
                    "storage:cleanroom-reference-control",
                    vec![(uuid(owner), packed(&row))],
                    &scratch.0,
                    true,
                    profile,
                    Some(&foreign),
                )
                .unwrap();
                let report = serde_json::to_value(report).unwrap();
                if family == expected_kind {
                    let xml = fs::read_to_string(scratch.0.join(path))
                        .unwrap_or_else(|error| panic!("{error}: {report}"));
                    let expected = if property == "BaseCalculationTypes" {
                        format!(
                            "<BaseCalculationTypes>\r\n\t\t\t\t<xr:Item xsi:type=\"xr:MDObjectRef\">{family}.Referenced</xr:Item>\r\n\t\t\t</BaseCalculationTypes>"
                        )
                    } else {
                        format!("<{property}>{family}.Referenced</{property}>")
                    };
                    assert!(xml.contains(&expected), "{xml}");
                    assert_eq!(report["storage"]["opaque"], 0, "{report}");
                    assert_eq!(report["storage"]["failed"], 0, "{report}");
                } else {
                    assert!(!scratch.0.join(path).exists(), "{report}");
                    assert!(
                        report["storage"]["opaque"].as_u64().unwrap()
                            + report["storage"]["failed"].as_u64().unwrap()
                            > 0,
                        "{report}"
                    );
                }
            }
        }
    }
}

#[test]
fn malformed_or_duplicate_versions_never_fabricate_an_index() {
    for profile in [
        InfobaseConfigSourceVersion::V2_20,
        InfobaseConfigSourceVersion::V2_21,
    ] {
        for version_text in [
            format!(
                "{{1,2,\"{}\",12345678-9abc-4def-8012-3456789abcde}}",
                uuid(10)
            ),
            format!(
                "{{1,2,\"{}\",12345678-9abc-4def-8012-3456789abcde,\"{}\",12345678-9abc-4def-8012-3456789abcde}}",
                uuid(10),
                uuid(10)
            ),
        ] {
            let mut rows = indexed_rows();
            rows.push(("versions".into(), version_text));
            let (scratch, report) = export(rows, profile);
            assert!(!scratch.0.join("ConfigDumpInfo.xml").exists(), "{report}");
            assert!(
                report["storage"]["failed"].as_u64().unwrap() > 0,
                "malformed service row must be accounted: {report}"
            );
            assert!(scratch.0.join("FilterCriteria/EmptyCriterion.xml").exists());
            assert!(
                scratch
                    .0
                    .join("ChartsOfAccounts/EmptyAccounts.xml")
                    .exists()
            );
            assert!(
                scratch
                    .0
                    .join("ChartsOfCalculationTypes/EmptyCalculation.xml")
                    .exists()
            );
        }
    }
}

#[test]
fn config_dump_info_uses_each_own_stored_version_and_family_both_profiles() {
    let version = "12345678-9abc-4def-8012-3456789abcde";
    let mut rows = indexed_rows();
    rows.push(("versions".into(), format!("{{1,5,\"\",00000000-0000-4000-8000-000000000999,\"{}\",{version},\"{}\",{version},\"{}\",{version},\"{}\",{version}}}", uuid(1000), uuid(10), uuid(100), uuid(200))));
    for profile in [
        InfobaseConfigSourceVersion::V2_20,
        InfobaseConfigSourceVersion::V2_21,
    ] {
        let (scratch, report) = export(rows.clone(), profile);
        let xml = fs::read_to_string(scratch.0.join("ConfigDumpInfo.xml"))
            .unwrap_or_else(|error| panic!("{error}: {report}"));
        for (kind, name, seed) in [
            ("FilterCriterion", "EmptyCriterion", 10),
            ("ChartOfAccounts", "EmptyAccounts", 100),
            ("ChartOfCalculationTypes", "EmptyCalculation", 200),
        ] {
            assert!(xml.contains(&format!("name=\"{kind}.{name}\" id=\"{}\" configVersion=\"78563412bc9aef4d80123456789abcde00000000\"", uuid(seed))), "{xml}");
        }
        assert!(xml.contains(&format!("name=\"Configuration.EmptyFamilies\" id=\"{}\" configVersion=\"78563412bc9aef4d80123456789abcde00000000\"", uuid(1000))), "{xml}");
        assert_eq!(xml.matches("configVersion=").count(), 4);
        assert!(
            !xml.contains("000000000999"),
            "generation is not an object: {xml}"
        );
    }
}

#[test]
fn canonical_descriptor_compiler_retains_native_absence_on_reexport() {
    // This checks the already-existing canonical descriptor compiler, not the
    // separate standalone register_native bootstrap route. The latter still
    // requires its own native/CF acceptance for complete configuration reuse.
    for profile in [
        InfobaseConfigSourceVersion::V2_20,
        InfobaseConfigSourceVersion::V2_21,
    ] {
        let (first, report) = export(all_rows(), profile);
        assert_eq!(report["storage"]["opaque"], 0, "{report}");
        let context =
            ibcmd_rs::metadata_model::DescriptorContext::new(&first.0, profile.as_str()).unwrap();
        let mut compiled = Vec::new();
        let mut expected = Vec::new();
        for (kind, path, seed) in [
            ("FilterCriterion", "FilterCriteria/EmptyCriterion.xml", 10),
            ("ChartOfAccounts", "ChartsOfAccounts/EmptyAccounts.xml", 100),
            (
                "ChartOfCalculationTypes",
                "ChartsOfCalculationTypes/EmptyCalculation.xml",
                200,
            ),
        ] {
            let full_path = first.0.join(path);
            let xml = fs::read(&full_path).unwrap();
            let row =
                ibcmd_rs::metadata_model::compile_descriptor(kind, &full_path, &xml, &context)
                    .unwrap();
            compiled.push((uuid(seed), String::from_utf8(row).unwrap()));
            expected.push((path, xml));
        }
        let (second, report) = export(compiled, profile);
        assert_eq!(report["storage"]["opaque"], 0, "{report}");
        assert_eq!(report["storage"]["failed"], 0, "{report}");
        for (path, xml) in expected {
            assert_eq!(
                fs::read(second.0.join(path)).unwrap(),
                xml,
                "complete own descriptor bytes changed: {path}"
            );
        }
    }
}
