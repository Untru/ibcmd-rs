use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use ibcmd_rs::metadata_model::brace::{Brace, parse_row, serialize_row};
use ibcmd_rs::metadata_model::{DescriptorContext, compile_descriptor};
use ibcmd_rs::restructure::caches::slots::{
    CompatibilityStatus, OwnerBranch, OwnerSlotRole, PositionConfidence, RecordMap,
    SemanticPayloadStatus, SlotGate, SlotPresence, UnknownLayoutReason, object_identity,
    owner_record,
};

const OWN_UUID: &str = "11111111-1111-4111-8111-111111111111";

struct Compiled {
    root: PathBuf,
    row: Brace,
    bytes: Vec<u8>,
    source: Vec<(PathBuf, Arc<Vec<u8>>)>,
}
impl Drop for Compiled {
    fn drop(&mut self) {
        // This exclusively created directory contains only this fixture's own XML inputs.
        fs::remove_dir_all(&self.root).unwrap();
    }
}

// Independently authored source XML. No native/GPL fixture bytes or descriptor template.
// Required scalar spellings come from the existing compiler property tables.
fn compiled(kind: &str, mode: &str, dialect: &str) -> Compiled {
    let root = std::env::temp_dir().join(format!(
        "ibcmd-slot-inspection-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    fs::create_dir(&root).unwrap();
    let envelope = |body: &str| {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:xs="http://www.w3.org/2001/XMLSchema" version="{dialect}">{body}</MetaDataObject>"#
        )
    };
    let configuration = envelope(&format!(
        r#"<Configuration uuid="22222222-2222-4222-8222-222222222222"><Properties><Name>TestConfiguration</Name><CompatibilityMode>{mode}</CompatibilityMode></Properties><ChildObjects/></Configuration>"#
    ));
    let mut properties = BTreeMap::new();
    for name in [
        "FoldersOnTop",
        "CheckUnique",
        "Autonumbering",
        "UseStandardCommands",
        "IncludeHelpInContents",
        "Hierarchical",
        "LimitLevelCount",
        "QuickChoice",
        "UpdateDataHistoryImmediatelyAfterWrite",
        "ExecuteAfterWriteDataHistoryVersionProcessing",
        "PostInPrivilegedMode",
        "UnpostInPrivilegedMode",
        "DistributedInfoBase",
        "IncludeConfigurationExtensions",
    ] {
        properties.insert(name, "false".to_owned());
    }
    for name in [
        "LevelCount",
        "CodeLength",
        "DescriptionLength",
        "NumberLength",
    ] {
        properties.insert(name, "0".to_owned());
    }
    for (name, value) in [
        ("EditType", "InList"),
        ("CodeSeries", "WholeCatalog"),
        ("CodeType", "String"),
        ("DefaultPresentation", "AsDescription"),
        ("HierarchyType", "HierarchyFoldersAndItems"),
        ("SubordinationUse", "ToItems"),
        ("ChoiceMode", "FromForm"),
        ("DataLockControlMode", "Managed"),
        ("FullTextSearch", "DontUse"),
        ("CodeAllowedLength", "Variable"),
        ("CreateOnInput", "DontUse"),
        ("PredefinedDataUpdate", "Auto"),
        ("ChoiceHistoryOnInput", "Auto"),
        ("DataHistory", "DontUse"),
        ("SearchStringModeOnInputByString", "Begin"),
        ("FullTextSearchOnInputByString", "DontUse"),
        ("ChoiceDataGetModeOnInputByString", "Directly"),
        ("NumberType", "String"),
        ("NumberPeriodicity", "Nonperiodical"),
        ("Posting", "Deny"),
        ("RegisterRecordsDeletion", "AutoDelete"),
        ("RealTimePosting", "Deny"),
        ("SequenceFilling", "AutoFill"),
        ("RegisterRecordsWritingOnPost", "WriteSelected"),
        ("NumberAllowedLength", "Variable"),
    ] {
        properties.insert(name, value.to_owned());
    }
    if kind == "ChartOfCharacteristicTypes" {
        properties.insert("CodeSeries", "WholeCharacteristicKind".to_owned());
        properties.insert("Type", "<v8:Type>xs:string</v8:Type>".to_owned());
    }
    if kind == "Catalog" {
        properties.insert("Owners", "<Item>Catalog.Sample</Item>".to_owned());
    }
    if kind != "Report" {
        properties.insert(
            "InputByString",
            format!("<Field>{kind}.Sample.StandardAttribute.Ref</Field>"),
        );
    }
    let properties = properties
        .into_iter()
        .map(|(name, value)| format!("<{name}>{value}</{name}>"))
        .collect::<String>();
    let categories: &[&str] = match kind {
        "Report" => &["Object", "Manager"],
        "ChartOfCharacteristicTypes" => &[
            "Object",
            "Ref",
            "Selection",
            "List",
            "Characteristic",
            "Manager",
        ],
        _ => &["Object", "Ref", "Selection", "List", "Manager"],
    };
    let mut internal = String::new();
    for (index, category) in categories.iter().enumerate() {
        let type_id = uuid::Uuid::from_u128(0x1000 + index as u128 * 2);
        let value_id = uuid::Uuid::from_u128(0x1001 + index as u128 * 2);
        internal.push_str(&format!(r#"<xr:GeneratedType name="{kind}{category}.Sample" category="{category}"><xr:TypeId>{type_id}</xr:TypeId><xr:ValueId>{value_id}</xr:ValueId></xr:GeneratedType>"#));
    }
    if kind == "ExchangePlan" {
        internal.push_str(&format!("<xr:ThisNode>{OWN_UUID}</xr:ThisNode>"));
    }
    let xml = envelope(&format!(
        r#"<{kind} uuid="{OWN_UUID}"><InternalInfo>{internal}</InternalInfo><Properties><Name>Sample</Name><Synonym/><Comment/>{properties}</Properties><ChildObjects/></{kind}>"#
    ));
    let path = root
        .join(ibcmd_rs::metadata_model::index::collection_of_kind(kind).unwrap())
        .join("Sample.xml");
    fs::create_dir(path.parent().unwrap()).unwrap();
    fs::write(root.join("Configuration.xml"), configuration.as_bytes()).unwrap();
    fs::write(&path, xml.as_bytes()).unwrap();
    let source = vec![
        (
            root.join("Configuration.xml"),
            Arc::new(configuration.into_bytes()),
        ),
        (path.clone(), Arc::new(xml.into_bytes())),
    ];
    let context = DescriptorContext::with_files(&root, dialect, &source).unwrap();
    let bytes = compile_descriptor(kind, &path, &source[1].1, &context).unwrap();
    let row = parse_row(&bytes).unwrap();
    Compiled {
        root,
        row,
        bytes,
        source,
    }
}

fn assert_exact(fixture: &Compiled, kind: &str, tag: i64, branches: &[OwnerBranch]) {
    let before = fixture.row.clone();
    let (record, actual_tag) = owner_record(&fixture.row).unwrap();
    assert_eq!(actual_tag, tag);
    let inspection = RecordMap::inspect(kind, record).unwrap();
    assert_eq!(
        inspection.position_confidence(),
        PositionConfidence::OwnerPositionsExact
    );
    assert_eq!(
        inspection.semantic_payload_status(),
        SemanticPayloadStatus::SemanticPayloadUnchecked
    );
    assert_eq!(
        inspection.compatibility_status(),
        CompatibilityStatus::Unbound
    );
    assert_eq!(
        inspection
            .observations()
            .iter()
            .map(|o| o.branch())
            .collect::<Vec<_>>(),
        branches
    );
    let old = RecordMap::new(kind, tag).unwrap();
    assert_eq!(
        object_identity(&fixture.row, kind).unwrap(),
        (OWN_UUID.to_owned(), "Sample".to_owned())
    );
    let mut generated = Vec::new();
    let mut names = BTreeMap::new();
    let mut header = None;
    for fact in inspection.shared_present_slots() {
        let range = fact.expected_range().unwrap();
        assert_eq!(fact.actual_range(), Some(range.clone()));
        assert_eq!(fact.actual(), &record[range.clone()]);
        assert!(std::ptr::eq(
            fact.actual().as_ptr(),
            record[range.clone()].as_ptr()
        ));
        match fact.role() {
            OwnerSlotRole::GeneratedPair(category) => {
                assert_eq!(fact.expected_width(), 2);
                generated.push((category, range.start));
            }
            OwnerSlotRole::Header | OwnerSlotRole::WrappedHeader => {
                header = Some(range.start);
            }
            role => {
                if let Some(name) = role.name() {
                    names.insert(name, range.start);
                    assert_eq!(old.slot(record, name).unwrap(), &record[range.start]);
                }
            }
        }
    }
    assert_eq!(
        names,
        old.names
            .iter()
            .map(|(name, index)| (*name, *index))
            .collect::<BTreeMap<_, _>>()
    );
    assert_eq!(generated, old.generated);
    assert_eq!(header, Some(old.header));
    let actual_generated = old.generated_types(record).unwrap();
    assert_eq!(actual_generated.len(), generated.len());
    for (pair, (_, index)) in actual_generated.iter().zip(generated) {
        assert_eq!(pair.type_id, record[index].as_atom().unwrap());
        assert_eq!(pair.value_id, record[index + 1].as_atom().unwrap());
    }
    assert_eq!(fixture.row, before);
    assert_eq!(serialize_row(&fixture.row), fixture.bytes);
    for (path, bytes) in &fixture.source {
        assert_eq!(fs::read(path).unwrap(), **bytes);
    }
}

#[test]
fn actual_catalog_old_and_new_keep_every_legacy_map_result() {
    for (mode, tag, branches) in [
        ("Version8_3_24", 56, vec![OwnerBranch::Before8_3_27]),
        (
            "Version8_3_27",
            57,
            vec![OwnerBranch::From8_3_27, OwnerBranch::From8_5_1],
        ),
    ] {
        assert_exact(
            &compiled("Catalog", mode, "2.20"),
            "Catalog",
            tag,
            &branches,
        );
    }
}

#[test]
fn actual_exchange_plan_retains_conditional_not_stored_and_present_tail() {
    for (mode, tag, branches, presence) in [
        (
            "Version8_3_24",
            36,
            vec![OwnerBranch::Before8_3_27],
            SlotPresence::NotStoredByBranch,
        ),
        (
            "Version8_3_27",
            37,
            vec![OwnerBranch::From8_3_27, OwnerBranch::From8_5_1],
            SlotPresence::Present,
        ),
    ] {
        let f = compiled("ExchangePlan", mode, "2.20");
        assert_exact(&f, "ExchangePlan", tag, &branches);
        let (record, _) = owner_record(&f.row).unwrap();
        let i = RecordMap::inspect("ExchangePlan", record).unwrap();
        for branch in i.observations() {
            let tail = branch.slots().last().unwrap();
            assert_eq!(tail.role(), OwnerSlotRole::Constant(1));
            assert_eq!(tail.gates(), &[SlotGate::Modern]);
            assert_eq!(tail.presence(), presence);
            if presence == SlotPresence::Present {
                assert_eq!(tail.actual(), &[Brace::num(1)]);
            } else {
                assert_eq!(tail.expected_range(), None);
                assert!(tail.actual().is_empty());
            }
        }
    }
}

#[test]
fn actual_report_19_and_20_expose_declared_reference_gate() {
    for (mode, dialect, tag, branches, presence) in [
        (
            "Version8_3_27",
            "2.20",
            19,
            vec![OwnerBranch::Before8_3_27, OwnerBranch::From8_3_27],
            SlotPresence::NotStoredByBranch,
        ),
        (
            "Version8_5_1",
            "2.21",
            20,
            vec![OwnerBranch::From8_5_1],
            SlotPresence::Present,
        ),
    ] {
        let f = compiled("Report", mode, dialect);
        assert_exact(&f, "Report", tag, &branches);
        let (record, _) = owner_record(&f.row).unwrap();
        let i = RecordMap::inspect("Report", record).unwrap();
        for branch in i.observations() {
            let tail = branch.slots().last().unwrap();
            assert_eq!(
                tail.role(),
                OwnerSlotRole::MetadataReference("AuxiliaryVariantForm")
            );
            assert_eq!(tail.gates(), &[SlotGate::Since8_5_1]);
            assert_eq!(tail.presence(), presence);
        }
    }
}

#[test]
fn actual_document40_keeps_all_equivalent_recipes_without_binding_compatibility() {
    for (mode, dialect) in [
        ("Version8_3_24", "2.20"),
        ("Version8_3_27", "2.20"),
        ("Version8_5_1", "2.21"),
    ] {
        assert_exact(
            &compiled("Document", mode, dialect),
            "Document",
            40,
            &[
                OwnerBranch::Before8_3_27,
                OwnerBranch::From8_3_27,
                OwnerBranch::From8_5_1,
            ],
        );
    }
}

#[test]
fn actual_reference_list_fields_and_pattern_roles_remain_distinct() {
    let f = compiled("Catalog", "Version8_3_27", "2.20");
    let (record, _) = owner_record(&f.row).unwrap();
    let i = RecordMap::inspect("Catalog", record).unwrap();
    let slots = i.observations()[0].slots();
    let scalar = slots
        .iter()
        .find(|s| s.role() == OwnerSlotRole::MetadataReference("DefaultObjectForm"))
        .unwrap();
    assert!(scalar.actual()[0].as_atom().is_some());
    let list = slots
        .iter()
        .find(|s| s.role() == OwnerSlotRole::MetadataReferenceList("Owners"))
        .unwrap();
    assert_eq!(
        list.actual()[0].at(&[1]).and_then(Brace::as_atom),
        Some("1")
    );
    let fields = slots
        .iter()
        .find(|s| s.role() == OwnerSlotRole::FieldReferenceList("InputByString"))
        .unwrap();
    assert_eq!(
        fields.actual()[0].at(&[1, 1]).and_then(Brace::as_atom),
        Some("1")
    );
    let mut unknown = record.to_vec();
    unknown[scalar.expected_range().unwrap().start] =
        Brace::uuid("99999999-9999-4999-8999-999999999999");
    let unbound = RecordMap::inspect("Catalog", &unknown).unwrap();
    assert_eq!(
        unbound.position_confidence(),
        PositionConfidence::OwnerPositionsExact
    );
    assert_eq!(
        unbound.semantic_payload_status(),
        SemanticPayloadStatus::SemanticPayloadUnchecked
    );
    let c = compiled("ChartOfCharacteristicTypes", "Version8_3_27", "2.20");
    let (record, _) = owner_record(&c.row).unwrap();
    let i = RecordMap::inspect("ChartOfCharacteristicTypes", record).unwrap();
    let pattern = i
        .shared_present_slots()
        .find(|s| s.role() == OwnerSlotRole::TypePattern("Type"))
        .unwrap();
    assert_eq!(
        pattern.actual()[0].at(&[0]).and_then(Brace::as_str),
        Some("Pattern")
    );
}

#[test]
fn actual_truncation_at_pair_header_and_end_cannot_become_exact() {
    let f = compiled("Report", "Version8_5_1", "2.21");
    let (record, tag) = owner_record(&f.row).unwrap();
    let map = RecordMap::new("Report", tag).unwrap();
    for end in [2, map.header, record.len() - 1] {
        let i = RecordMap::inspect("Report", &record[..end]).unwrap();
        assert_eq!(
            i.position_confidence(),
            PositionConfidence::IncompleteOwnerShape
        );
        let missing = i.observations()[0]
            .slots()
            .iter()
            .filter(|s| s.presence() == SlotPresence::MissingDeclaredValue)
            .collect::<Vec<_>>();
        assert!(!missing.is_empty());
        assert!(
            missing
                .iter()
                .all(|s| s.expected_range().unwrap().end > end)
        );
        assert!(
            i.shared_present_slots()
                .all(|s| s.presence() == SlotPresence::Present)
        );
    }
    let i = RecordMap::inspect("Report", &record[..2]).unwrap();
    let pair = &i.observations()[0].slots()[1];
    assert_eq!(pair.expected_width(), 2);
    assert_eq!(pair.actual().len(), 1);
    assert_eq!(pair.presence(), SlotPresence::MissingDeclaredValue);
}

#[test]
fn actual_tail_is_preserved_per_recipe_and_payload_corruption_is_not_admitted() {
    let f = compiled("Document", "Version8_3_27", "2.20");
    let (record, tag) = owner_record(&f.row).unwrap();
    let mut tail = record.to_vec();
    let opaque = Brace::list(vec![Brace::str("Uninterpreted"), Brace::num(987)]);
    tail.push(opaque.clone());
    let i = RecordMap::inspect("Document", &tail).unwrap();
    assert_eq!(
        i.position_confidence(),
        PositionConfidence::IncompleteOwnerShape
    );
    assert_eq!(i.observations().len(), 3);
    for b in i.observations() {
        assert_eq!(b.extra_tail_range(), Some(record.len()..tail.len()));
        assert_eq!(b.extra_tail(), std::slice::from_ref(&opaque));
    }
    let mut malformed = record.to_vec();
    malformed[RecordMap::new("Document", tag).unwrap().header] = Brace::str("NOT A HEADER");
    let i = RecordMap::inspect("Document", &malformed).unwrap();
    assert_eq!(
        i.position_confidence(),
        PositionConfidence::OwnerPositionsExact
    );
    assert_eq!(
        i.semantic_payload_status(),
        SemanticPayloadStatus::SemanticPayloadUnchecked
    );
}

#[test]
fn unknown_kind_and_tag_keep_raw_record_and_legacy_unknown_tag_behavior() {
    let record = vec![Brace::num(999), Brace::str("opaque")];
    for (kind, reason) in [
        ("Catalog", UnknownLayoutReason::UnknownTag),
        ("UndeclaredKind", UnknownLayoutReason::UnknownKind),
    ] {
        let i = RecordMap::inspect(kind, &record).unwrap();
        assert_eq!(i.record(), record.as_slice());
        assert_eq!(i.unknown_reason(), Some(reason));
        assert_eq!(i.position_confidence(), PositionConfidence::UnknownLayout);
        assert!(i.observations().is_empty());
        assert_eq!(i.shared_present_slots().count(), 0);
    }
    let unknown = RecordMap::new("Catalog", 999).unwrap();
    let modern = RecordMap::new("Catalog", 57).unwrap();
    assert_eq!(unknown.names, modern.names);
    assert_eq!(unknown.generated, modern.generated);
    assert_eq!(unknown.header, modern.header);
    for malformed in [
        vec![],
        vec![Brace::str("40")],
        vec![Brace::atom("not-integer")],
    ] {
        assert!(RecordMap::inspect("Document", &malformed).is_err());
    }
}
