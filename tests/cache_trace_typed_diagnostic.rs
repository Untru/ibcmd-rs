//! Cleanroom typed observations. No native rows, byte-exact generator, or platform actor.
#[path = "support/cache_insertion_trace.rs"]
mod diagnostic;

use ibcmd_rs::metadata_model::brace::{Brace, NIL_UUID, serialize_row};
use ibcmd_rs::mssql_config_apply::si;
use ibcmd_rs::restructure::caches::{
    facts,
    help_props::{Entry, HelpProps},
    root,
    slots::{OwnerSlotRole, RecordMap},
    trace_diagnostic::{self as trace, Coordinate, Descriptor, EdgeKind, Inputs, KeyOrigin},
    type_index::{self, TypeIndex, TypeSlot},
    type_sets::TypeSets,
};

fn id(n: u128) -> String {
    uuid::Uuid::from_u128(n).hyphenated().to_string()
}
fn a(value: impl ToString) -> Brace {
    Brace::atom(value)
}
fn l(values: Vec<Brace>) -> Brace {
    Brace::list(values)
}
fn reference(id: &str) -> Brace {
    l(vec![
        Brace::str("#"),
        a(facts::METADATA_REF),
        l(vec![a(1), a(id)]),
    ])
}
fn links(ids: &[String]) -> Brace {
    let mut body = vec![a(0), a(ids.len())];
    body.extend(ids.iter().map(|id| reference(id)));
    l(vec![
        Brace::str("#"),
        a("9cd510d6-abfc-11d4-9434-004095e12fc7"),
        l(body),
    ])
}

struct Fixture {
    registry_raw: Vec<u8>,
    registry: si::SiMain,
    sets: TypeSets,
    help: HelpProps,
    index: TypeIndex,
    rows: Vec<Brace>,
}
impl Fixture {
    fn new() -> Self {
        let mut body = vec![a(4)];
        for n in 1..=4 {
            body.extend([
                a(id(n)),
                a(if n == 1 { NIL_UUID.to_owned() } else { id(1) }),
                a(0),
                Brace::str(format!("Own{n}")),
                l(vec![a(1), a(0)]),
                a(0),
                a(0),
            ]);
        }
        let registry_raw = serialize_row(&l(vec![
            a(4),
            l(vec![a(1), a(root::CATALOG_CLASS)]),
            l(body),
        ]));
        let registry = si::parse(&registry_raw).unwrap();
        let set = id(50);
        let sets = TypeSets {
            sets: vec![(set.clone(), vec![l(vec![Brace::str("#"), a(id(60))])])],
        };
        // Deliberately differs from owner order; properties/row order must not change.
        let help = HelpProps {
            entries: vec![
                Entry {
                    key: id(4),
                    props: vec![("7".into(), Brace::str("Current\r\nValue"))],
                },
                Entry {
                    key: set.clone(),
                    props: vec![(
                        "23".into(),
                        l(vec![Brace::str("opaque-current"), a(id(60))]),
                    )],
                },
                Entry {
                    key: id(2),
                    props: vec![("5".into(), l(vec![Brace::str("B"), a(1)]))],
                },
                Entry {
                    key: NIL_UUID.into(),
                    props: vec![("23".into(), links(&[set]))],
                },
            ],
        };
        let map = RecordMap::new("Catalog", 57).unwrap();
        let index = TypeIndex {
            sections: vec![type_index::Section {
                class: root::CATALOG_CLASS.into(),
                entries: (1..=4)
                    .map(|n| type_index::Entry {
                        object: id(n),
                        types: map
                            .generated
                            .iter()
                            .enumerate()
                            .map(|(ordinal, _)| {
                                let (type_id, value_id) = generated_pair(n, ordinal);
                                TypeSlot {
                                    type_id,
                                    value_id,
                                    index: ordinal as u32,
                                }
                            })
                            .collect(),
                    })
                    .collect(),
            }],
        };
        let rows = (1..=4)
            .map(|n| descriptor(n, if n == 2 || n == 3 { Some(4) } else { None }))
            .collect();
        Self {
            registry_raw,
            registry,
            sets,
            help,
            index,
            rows,
        }
    }
    fn inspect(&self) -> anyhow::Result<trace::Diagnostic> {
        let descriptors: Vec<_> = self
            .rows
            .iter()
            .enumerate()
            .map(|(n, row)| Descriptor {
                row_ordinal: n,
                owner: self.registry.records[n].uuid.as_str(),
                kind: "Catalog",
                row,
            })
            .collect();
        trace::inspect(&Inputs {
            root: &id(1),
            registry: &self.registry,
            sets: &self.sets,
            help: &self.help,
            type_index: Some(&self.index),
            descriptors: &descriptors,
        })
    }
}

fn generated_pair(n: u128, ordinal: usize) -> (String, String) {
    if n == 4 && ordinal == 1 {
        (id(60), id(61))
    } else {
        (
            id(n * 100 + ordinal as u128 * 2),
            id(n * 100 + ordinal as u128 * 2 + 1),
        )
    }
}

// Uses the actual owner Slot declarations rather than a copied positions/UUID table.
fn descriptor(n: u128, shared: Option<u128>) -> Brace {
    let map = RecordMap::new("Catalog", 57).unwrap();
    let length = map
        .names
        .values()
        .copied()
        .chain(map.generated.iter().map(|(_, at)| at + 1))
        .chain([map.header])
        .max()
        .unwrap()
        + 1;
    let mut record = vec![a(0); length];
    record[0] = a(57);
    let inspected = RecordMap::inspect("Catalog", &record).unwrap();
    let defaults: Vec<_> = inspected
        .shared_present_slots()
        .filter_map(|slot| {
            let at = slot.actual_range()?.start;
            match slot.role() {
                OwnerSlotRole::MetadataReference(_) => Some((at, a(NIL_UUID))),
                OwnerSlotRole::MetadataReferenceList(_) => Some((at, l(vec![a(0), a(0)]))),
                _ => None,
            }
        })
        .collect();
    for (at, value) in defaults {
        record[at] = value;
    }
    for (ordinal, (_, at)) in map.generated.iter().enumerate() {
        let (type_id, value_id) = generated_pair(n, ordinal);
        record[*at] = a(type_id);
        record[*at + 1] = a(value_id);
    }
    let header = l(vec![
        a(3),
        l(vec![a(1), a(0), a(id(n))]),
        Brace::str(format!("Own{n}")),
        l(vec![a(0)]),
        Brace::str(""),
        a(0),
        a(0),
        a(NIL_UUID),
        a(0),
    ]);
    record[map.header] = l(vec![a(0), header]);
    if let Some(target) = shared {
        record[map.names["Owners"]] = l(vec![a(0), a(1), reference(&id(target))]);
    }
    l(vec![a(1), l(record), a(0)])
}

#[test]
fn actual_key_origins_owner_shared_referent_type_and_nil_are_separate_and_immutable() {
    let fixture = Fixture::new();
    let before = (
        fixture.help.clone(),
        fixture.sets.clone(),
        fixture.rows.clone(),
        fixture.index.clone(),
    );
    let result = fixture.inspect().unwrap();
    assert_eq!(
        result
            .keys()
            .iter()
            .map(|key| &key.origin)
            .collect::<Vec<_>>(),
        vec![
            &KeyOrigin::Metadata { record: 3 },
            &KeyOrigin::TypeSet { set: 0 },
            &KeyOrigin::Metadata { record: 1 },
            &KeyOrigin::NilAggregate
        ]
    );
    assert!(!result.empty_references().is_empty());
    let shared: Vec<_> = result
        .edges()
        .iter()
        .filter(|edge| edge.kind == EdgeKind::MetadataReference && edge.target == id(4))
        .collect();
    assert_eq!(
        shared
            .iter()
            .map(|edge| edge.from.clone())
            .collect::<Vec<_>>(),
        vec![id(2), id(3)]
    );
    let map = RecordMap::new("Catalog", 57).unwrap();
    assert!(
        matches!(&shared[0].coordinate, Coordinate::Descriptor { row: 1, path, .. } if path == &vec![1, map.names["Owners"], 2])
    );
    let typed = result
        .edges()
        .iter()
        .find(|edge| edge.kind == EdgeKind::GeneratedTypeMember)
        .unwrap();
    assert_eq!(typed.referent, Some(id(4)));
    assert_eq!(
        typed.declaration,
        Some(Coordinate::TypeIndex {
            section: 0,
            entry: 3,
            type_ordinal: 1
        })
    );
    assert_eq!(
        typed.coordinate,
        Coordinate::TypeSet {
            set: 0,
            member: Some(0)
        }
    );
    assert_eq!(
        result
            .edges()
            .iter()
            .filter(|edge| edge.kind == EdgeKind::NilSetMember)
            .count(),
        1
    );
    assert!(result.first_visit_authority().is_none());
    assert!(result.insertion_candidate().is_none());
    assert!(
        result
            .residues()
            .iter()
            .any(|r| r.reason.contains("compatibility unbound"))
    );
    assert_eq!(
        (
            fixture.help.clone(),
            fixture.sets.clone(),
            fixture.rows.clone(),
            fixture.index.clone()
        ),
        before
    );
}

#[test]
fn missing_descriptor_and_unresolved_type_have_actual_coordinates_and_no_candidate() {
    let fixture = Fixture::new();
    let result = trace::inspect(&Inputs {
        root: &id(1),
        registry: &fixture.registry,
        sets: &fixture.sets,
        help: &fixture.help,
        type_index: None,
        descriptors: &[],
    })
    .unwrap();
    assert!(result.residues().iter().any(|r| r.coordinate
        == Coordinate::TypeSet {
            set: 0,
            member: Some(0)
        }));
    assert!(
        result
            .residues()
            .iter()
            .any(|r| matches!(r.coordinate, Coordinate::Registry { record: 0, .. }))
    );
    assert!(result.insertion_candidate().is_none());
}

#[test]
fn duplicate_metadata_identity_is_refused_even_after_public_dto_mutation() {
    let mut fixture = Fixture::new();
    fixture.registry.records[3].uuid = fixture.registry.records[2].uuid.clone();
    assert!(
        fixture
            .inspect()
            .unwrap_err()
            .to_string()
            .contains("duplicate metadata")
    );
}

#[test]
fn duplicate_set_and_ambiguous_metadata_set_declarations_refuse() {
    let mut fixture = Fixture::new();
    fixture.sets.sets.push(fixture.sets.sets[0].clone());
    assert!(
        fixture
            .inspect()
            .unwrap_err()
            .to_string()
            .contains("duplicate type-set")
    );
    fixture.sets.sets.pop();
    fixture.sets.sets[0].0 = id(2);
    assert!(
        fixture
            .inspect()
            .unwrap_err()
            .to_string()
            .contains("ambiguous metadata/set")
    );
}

#[test]
fn duplicate_help_properties_and_keys_refuse_without_masking_values() {
    let mut fixture = Fixture::new();
    fixture.help.entries.push(fixture.help.entries[0].clone());
    assert!(
        fixture
            .inspect()
            .unwrap_err()
            .to_string()
            .contains("duplicate help identity")
    );
    fixture.help.entries.pop();
    fixture.help.entries[0].props.push(("007".into(), a(0)));
    assert!(
        fixture
            .inspect()
            .unwrap_err()
            .to_string()
            .contains("duplicate help property")
    );
}

#[test]
fn missing_nil_and_mismatched_nil_set_bijection_refuse() {
    let mut fixture = Fixture::new();
    let nil = fixture.help.entries.pop().unwrap();
    assert!(
        fixture
            .inspect()
            .unwrap_err()
            .to_string()
            .contains("missing nil")
    );
    fixture.help.entries.push(nil);
    fixture.help.entries[3].props[0].1 = links(&[]);
    assert!(
        fixture
            .inspect()
            .unwrap_err()
            .to_string()
            .contains("bijection")
    );
}

#[test]
fn foreign_help_key_and_nil_member_refuse() {
    let mut fixture = Fixture::new();
    fixture.help.entries[0].key = id(999);
    assert!(
        fixture
            .inspect()
            .unwrap_err()
            .to_string()
            .contains("unresolved help key")
    );
    fixture.help.entries[0].key = id(4);
    fixture.help.entries[3].props[0].1 = links(&[id(999)]);
    assert!(
        fixture
            .inspect()
            .unwrap_err()
            .to_string()
            .contains("foreign set")
    );
}

#[test]
fn duplicate_generated_owner_type_and_category_are_rejected() {
    let mut fixture = Fixture::new();
    let duplicate = fixture.index.sections[0].entries[0].clone();
    fixture.index.sections[0].entries.push(duplicate);
    assert!(
        fixture
            .inspect()
            .unwrap_err()
            .to_string()
            .contains("duplicate type-index owner")
    );
    fixture.index.sections[0].entries.pop();
    fixture.index.sections[0].entries[0].types.push(TypeSlot {
        type_id: id(60),
        value_id: id(62),
        index: 9,
    });
    assert!(
        fixture
            .inspect()
            .unwrap_err()
            .to_string()
            .contains("ambiguous generated TypeId")
    );
    fixture.index.sections[0].entries[0]
        .types
        .last_mut()
        .unwrap()
        .type_id = id(63);
    fixture.index.sections[0].entries[0]
        .types
        .last_mut()
        .unwrap()
        .index = 1;
    assert!(
        fixture
            .inspect()
            .unwrap_err()
            .to_string()
            .contains("duplicate type category")
    );
}

#[test]
fn unresolved_metadata_target_is_partial_at_exact_slot_without_empty_success() {
    let mut fixture = Fixture::new();
    fixture.rows[1] = descriptor(2, Some(999));
    let result = fixture.inspect().unwrap();
    let edge = result
        .edges()
        .iter()
        .find(|e| e.kind == EdgeKind::MetadataReference && e.target == id(999))
        .unwrap();
    assert!(edge.referent.is_none());
    assert!(
        result
            .residues()
            .iter()
            .any(|r| r.coordinate == edge.coordinate && r.reason.contains("unresolved metadata"))
    );
    assert!(result.insertion_candidate().is_none());
}

#[test]
fn duplicate_descriptor_and_wrong_own_header_refuse() {
    let fixture = Fixture::new();
    let owner = id(2);
    let wrong = descriptor(4, None);
    let descriptors = [Descriptor {
        row_ordinal: 7,
        owner: &owner,
        kind: "Catalog",
        row: &wrong,
    }];
    assert!(
        trace::inspect(&Inputs {
            root: &id(1),
            registry: &fixture.registry,
            sets: &fixture.sets,
            help: &fixture.help,
            type_index: Some(&fixture.index),
            descriptors: &descriptors
        })
        .unwrap_err()
        .to_string()
        .contains("own identity/name")
    );
    let correct = descriptor(2, None);
    let descriptors = [
        Descriptor {
            row_ordinal: 7,
            owner: &owner,
            kind: "Catalog",
            row: &correct,
        },
        Descriptor {
            row_ordinal: 8,
            owner: &owner,
            kind: "Catalog",
            row: &correct,
        },
    ];
    assert!(
        trace::inspect(&Inputs {
            root: &id(1),
            registry: &fixture.registry,
            sets: &fixture.sets,
            help: &fixture.help,
            type_index: Some(&fixture.index),
            descriptors: &descriptors
        })
        .unwrap_err()
        .to_string()
        .contains("duplicate descriptor owner")
    );
}

#[test]
fn owner_preorder_invalid_uuid_and_nil_class_refuse() {
    let mut fixture = Fixture::new();
    fixture.registry.records[1].parent = id(4);
    assert!(
        fixture
            .inspect()
            .unwrap_err()
            .to_string()
            .contains("owner/preorder")
    );
    fixture.registry.records[1].parent = id(1);
    fixture.help.entries[0].key = "bad-uuid".into();
    assert!(
        fixture
            .inspect()
            .unwrap_err()
            .to_string()
            .contains("invalid UUID")
    );
    fixture.help.entries[0].key = id(4);
    fixture.registry.classes[0] = NIL_UUID.into();
    assert!(
        fixture
            .inspect()
            .unwrap_err()
            .to_string()
            .contains("nil declaration")
    );
}

#[test]
fn bare_metadata_slot_is_observed_using_role_not_tagged_reference_scan() {
    let mut fixture = Fixture::new();
    let record = fixture.rows[1].as_list_mut().unwrap()[1]
        .as_list_mut()
        .unwrap();
    let at = RecordMap::inspect("Catalog", record)
        .unwrap()
        .shared_present_slots()
        .find(|slot| matches!(slot.role(), OwnerSlotRole::MetadataReference(_)))
        .unwrap()
        .actual_range()
        .unwrap()
        .start;
    record[at] = a(id(3));
    let result = fixture.inspect().unwrap();
    assert!(result.edges().iter().any(|edge| edge.kind == EdgeKind::MetadataReference && edge.from == id(2)
        && edge.target == id(3) && matches!(&edge.coordinate, Coordinate::Descriptor { row: 1, path, .. } if path == &vec![1, at])));
}

#[test]
fn reference_count_generated_disagreement_and_outer_count_are_refused() {
    let mut fixture = Fixture::new();
    let map = RecordMap::new("Catalog", 57).unwrap();
    fixture.rows[1].as_list_mut().unwrap()[1]
        .as_list_mut()
        .unwrap()[map.names["Owners"]] = l(vec![a(0), a(2), reference(&id(4))]);
    assert!(
        fixture
            .inspect()
            .unwrap_err()
            .to_string()
            .contains("reference list version/count")
    );
    fixture.rows[1] = descriptor(2, Some(4));
    let original = fixture.index.sections[0].entries[0].types[0]
        .value_id
        .clone();
    fixture.index.sections[0].entries[0].types[0].value_id = id(62);
    assert!(
        fixture
            .inspect()
            .unwrap_err()
            .to_string()
            .contains("generated ownership differs")
    );
    fixture.index.sections[0].entries[0].types[0].value_id = original;
    fixture.rows[0].as_list_mut().unwrap()[2] = a(1);
    assert!(
        fixture
            .inspect()
            .unwrap_err()
            .to_string()
            .contains("collection count")
    );
}

#[test]
fn retained_source_admission_and_original_comparison_compose_without_proposal_or_drift() {
    use diagnostic::{
        FactInputs, FactRole, FactRowBinding, FactsManifestV1, FileBinding, Locator, ManifestV1,
        ProjectionInputs, RowBinding, RowOrigin, RowRole, hash,
    };
    use std::fs;
    struct OwnedDirectory(std::path::PathBuf);
    impl Drop for OwnedDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    let owned = OwnedDirectory(
        fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!("ibcmd-typed-cache-{}", uuid::Uuid::new_v4())),
    );
    fs::create_dir(&owned.0).unwrap();
    let fixture = Fixture::new();
    let source = "05cde8117a4588a1ff893ce8909bdb165ed27101";
    let origin = |table: &str, filename: &str| RowOrigin {
        case: "authored".into(),
        stage: "current".into(),
        table: table.into(),
        filename: filename.into(),
        part: 0,
        version: "own-current-version".into(),
    };
    let predecessor = include_bytes!("../src/restructure/caches/trace_diagnostic.rs");
    let predecessor_path = owned.0.join("source-snapshot.rs");
    fs::write(&predecessor_path, predecessor).unwrap();
    let rows = [
        fixture.registry_raw.clone(),
        fixture.sets.render(),
        fixture.help.render(),
    ]
    .into_iter()
    .zip([RowRole::Registry, RowRole::TypeSets, RowRole::HelpProps])
    .map(|(plain, role)| {
        let path = owned.0.join(role.filename());
        fs::write(&path, &plain).unwrap();
        RowBinding {
            role,
            origin: origin("Params", role.filename()),
            locator: Locator::Plain { path },
            stored_length: plain.len() as u64,
            packed_length: plain.len() as u64,
            plain_length: plain.len() as u64,
            stored_sha256: hash(&plain),
            packed_sha256: hash(&plain),
            plain_sha256: hash(&plain),
        }
    })
    .collect();
    let base = ProjectionInputs::load(
        ManifestV1 {
            schema: "cache-insertion-trace-input-v1".into(),
            source_head: source.into(),
            case: "authored".into(),
            stage: "current".into(),
            snapshot_purpose: "independent cleanroom source control".into(),
            registry_root_uuid: id(1),
            predecessors: vec![FileBinding {
                path: predecessor_path,
                length: predecessor.len() as u64,
                sha256: hash(predecessor),
            }],
            rows,
        },
        source,
    )
    .unwrap();
    let mut bindings = Vec::new();
    for (ordinal, row) in fixture.rows.iter().enumerate() {
        let owner = id(ordinal as u128 + 1);
        let bytes = serialize_row(row);
        let path = owned.0.join(format!("descriptor-{owner}"));
        fs::write(&path, &bytes).unwrap();
        bindings.push(FactRowBinding {
            role: FactRole::Descriptor {
                owner: owner.clone(),
            },
            origin: origin("Config", &owner),
            file: FileBinding {
                path,
                length: bytes.len() as u64,
                sha256: hash(&bytes),
            },
        });
    }
    let bytes = fixture.index.render();
    let path = owned.0.join("type-index");
    fs::write(&path, &bytes).unwrap();
    bindings.push(FactRowBinding {
        role: FactRole::TypeIndex,
        origin: origin("Params", "2203278d-ef4f-4f68-98f1-feb257d53ecc.si"),
        file: FileBinding {
            path,
            length: bytes.len() as u64,
            sha256: hash(&bytes),
        },
    });
    let first = bindings[0].file.path.clone();
    let inputs = FactInputs::load(
        &base,
        FactsManifestV1 {
            schema: "cache-insertion-trace-facts-v1".into(),
            source_head: source.into(),
            case: "authored".into(),
            stage: "current".into(),
            rows: bindings,
        },
    )
    .unwrap();
    let result = diagnostic::typed_current::inspect(&base, &inputs).unwrap();
    assert_eq!(result.diagnostic.keys().len(), fixture.help.entries.len());
    assert_eq!(result.key_sources.len(), fixture.help.entries.len());
    assert_eq!(result.fact_sources.len(), 5);
    assert_eq!(result.facts.descriptors.len(), 4);
    assert!(result.comparison.proposal.is_none());
    assert!(!result.comparison.native_acceptance);
    assert!(result.comparison.first_visit_authority.is_none());
    assert!(result.comparison.current_roundtrip.first.is_none());
    assert!(result.diagnostic.insertion_candidate().is_none());
    // Explicit selected declaration-order experiment, never seeded from expected help ordinals.
    let proposal = diagnostic::comparison::DiagnosticProposal {
        emits: [id(2), id(4), id(50), NIL_UUID.into()]
            .into_iter()
            .enumerate()
            .map(|(ordinal, key)| diagnostic::comparison::ProposedEmit {
                occurrence: result.key_sources[&key].clone(),
                key,
                event_id: format!("authored-selected-{ordinal}"),
            })
            .collect(),
    };
    let selected =
        diagnostic::typed_current::inspect_with_proposal(&base, &inputs, Some(&proposal)).unwrap();
    let compared = selected.comparison.proposal.as_ref().unwrap();
    assert!(compared.untrusted_diagnostic_only);
    assert_eq!(compared.associations.len(), 4);
    assert!(compared.payload.property_maps_equal);
    assert!(compared.payload.first_key.is_some());
    assert!(compared.literal.first.is_some());
    assert!(selected.comparison.first_visit_authority.is_none());
    assert!(!selected.comparison.native_acceptance);
    let mut missing = proposal.clone();
    missing.emits.pop();
    assert!(
        diagnostic::typed_current::inspect_with_proposal(&base, &inputs, Some(&missing))
            .err()
            .unwrap()
            .to_string()
            .contains("bijective")
    );
    let mut wrong_occurrence = proposal.clone();
    wrong_occurrence.emits[0].occurrence.path = vec![0];
    assert!(
        diagnostic::typed_current::inspect_with_proposal(&base, &inputs, Some(&wrong_occurrence))
            .is_err()
    );
    fs::write(first, b"changed source bytes").unwrap();
    assert!(diagnostic::typed_current::inspect(&base, &inputs).is_err());
}
