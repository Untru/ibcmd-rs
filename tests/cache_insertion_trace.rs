#[path = "support/cache_insertion_trace.rs"]
mod diagnostic;

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use diagnostic::{
    FileBinding, KeyOrigin, Locator, ManifestV1, ProjectionInputs, RowBinding, RowOrigin, RowRole,
    coverage, first_raw_difference, hash,
};
use ibcmd_rs::metadata_model::brace::{Brace, NIL_UUID, serialize_row};
use ibcmd_rs::restructure::caches::facts;

const SOURCE: &str = "b74c7a9668aa0c9ad797ba5de064b2697013499f";
const ROOT: &str = "11111111-1111-4111-8111-111111111111";
const CHILD: &str = "22222222-2222-4222-8222-222222222222";
const SET: &str = "33333333-3333-4333-8333-333333333333";
const TYPE: &str = "44444444-4444-4444-8444-444444444444";
const CLASS: &str = "55555555-5555-4555-8555-555555555555";

fn a(value: impl ToString) -> Brace {
    Brace::atom(value)
}
fn l(values: Vec<Brace>) -> Brace {
    Brace::list(values)
}
fn record(key: &str, parent: &str) -> Vec<Brace> {
    vec![
        a(key),
        a(parent),
        a(0),
        Brace::str(key),
        l(vec![a(0)]),
        a(0),
        a(0),
    ]
}

fn registry(records: &[(&str, &str)]) -> Brace {
    let mut body = vec![a(records.len())];
    for (key, parent) in records {
        body.extend(record(key, parent));
    }
    l(vec![a(4), l(vec![a(1), a(CLASS)]), l(body)])
}

fn sets(keys: &[&str]) -> Brace {
    let mut body = vec![a(keys.len())];
    for key in keys {
        body.extend([
            a(key),
            l(vec![
                Brace::str("Pattern"),
                l(vec![Brace::str("#"), a(TYPE)]),
                l(vec![Brace::str("S"), a(1024), a(1)]),
            ]),
        ]);
    }
    l(vec![a(0), l(body)])
}

fn links(keys: &[&str]) -> Brace {
    let mut body = vec![a(0), a(keys.len())];
    for key in keys {
        body.push(l(vec![
            Brace::str("#"),
            a(facts::METADATA_REF),
            l(vec![a(1), a(key)]),
        ]));
    }
    l(vec![
        Brace::str("#"),
        a("9cd510d6-abfc-11d4-9434-004095e12fc7"),
        l(body),
    ])
}

fn help(entries: &[(&str, Vec<(usize, Brace)>)]) -> Brace {
    let mut body = vec![a(entries.len())];
    for (key, props) in entries {
        body.extend([a(key), a(props.len())]);
        for (id, value) in props {
            body.extend([a(id), value.clone()]);
        }
    }
    l(vec![a(0), l(body)])
}

fn fixture() -> [Brace; 3] {
    [
        registry(&[(ROOT, NIL_UUID), (CHILD, ROOT)]),
        sets(&[SET]),
        help(&[
            (
                SET,
                vec![(23, l(vec![Brace::str("opaque-preserved"), a(TYPE)]))],
            ),
            (
                CHILD,
                vec![(7, l(vec![Brace::str("S"), Brace::str("raw\r\nproperty")]))],
            ),
            (NIL_UUID, vec![(23, links(&[SET])), (24, links(&[SET]))]),
        ]),
    ]
}

fn project(trees: &[Brace; 3]) -> anyhow::Result<diagnostic::Coverage> {
    coverage(
        &serialize_row(&trees[0]),
        &serialize_row(&trees[1]),
        &serialize_row(&trees[2]),
        ROOT,
    )
}

fn fields(tree: &mut Brace, index: usize) -> &mut Vec<Brace> {
    tree.as_list_mut().unwrap()[index].as_list_mut().unwrap()
}

#[test]
fn complete_key_bijection_preserves_owner_only_nodes_and_full_values_without_trace_authority() {
    let trees = fixture();
    let result = project(&trees).unwrap();
    assert_eq!(result.registry.records.len(), 2); // ROOT is visited but emits no help key.
    assert_eq!(result.registry.children(0), vec![1]);
    assert_eq!(
        result.origins,
        vec![
            KeyOrigin::TypeSet { set_ordinal: 0 },
            KeyOrigin::MetadataRecord { record_ordinal: 1 },
            KeyOrigin::NilAggregate
        ]
    );
    assert_eq!(
        result.help.entries[0].props[0].1,
        l(vec![Brace::str("opaque-preserved"), a(TYPE)])
    );
    assert_eq!(
        result.help.entries[2].props,
        vec![("23".into(), links(&[SET])), ("24".into(), links(&[SET]))]
    );
    assert_eq!(
        result.sets.sets[0].1[1],
        l(vec![Brace::str("S"), a(1024), a(1)])
    );
    assert_eq!(
        result.pending_atoms,
        ["A1.3", "A1.4", "A1.5", "A1.6", "A1.7"]
    );
    assert!(result.first_visit_authority.is_none());
    assert_eq!(
        result.graph_completeness,
        diagnostic::GraphCompleteness::Partial
    );
    assert_eq!(result.trace_status, diagnostic::TraceStatus::NotIdentified);
}

#[test]
fn real_growing_rosters_have_no_corpus_count_limit() {
    for size in [9usize, 65, 513, 1025, 2049, 2377] {
        let ids: Vec<_> = (1..=size)
            .map(|n| uuid::Uuid::from_u128(n as u128).hyphenated().to_string())
            .collect();
        let mut records = vec![(ROOT, NIL_UUID)];
        records.extend(ids.iter().map(|id| (id.as_str(), ROOT)));
        let mut entries: Vec<_> = ids
            .iter()
            .map(|id| (id.as_str(), vec![(5, l(vec![Brace::str("B"), a(0)]))]))
            .collect();
        entries.push((NIL_UUID, vec![(23, links(&[]))]));
        let result = project(&[registry(&records), sets(&[]), help(&entries)]).unwrap();
        assert_eq!(result.registry.records.len(), size + 1);
        assert_eq!(result.origins.len(), size + 1);
        assert!(
            result.origins[..size]
                .iter()
                .all(|origin| matches!(origin, KeyOrigin::MetadataRecord { .. }))
        );
    }
}

#[test]
fn counts_empty_lists_overflow_and_trailing_members_refuse_before_loose_parsers() {
    project(&fixture()).unwrap();
    for role in 0..3 {
        for invalid in [
            a(usize::MAX),
            a("-1"),
            a("+1"),
            a("999999999999999999999999999999999999"),
        ] {
            let mut trees = fixture();
            let body_index = if role == 0 { 2 } else { 1 };
            fields(&mut trees[role], body_index)[0] = invalid;
            assert!(project(&trees).is_err(), "role {role}");
        }
        let mut trees = fixture();
        let body_index = if role == 0 { 2 } else { 1 };
        fields(&mut trees[role], body_index).clear();
        assert!(project(&trees).is_err());
        let mut trees = fixture();
        trees[role].as_list_mut().unwrap().push(a(99));
        assert!(project(&trees).is_err());
    }
    let mut trees = fixture();
    fields(&mut trees[0], 1).clear();
    assert!(project(&trees).is_err());
    let mut trees = fixture();
    fields(&mut trees[2], 1)[2] = a(usize::MAX); // A real entry's property count.
    assert!(project(&trees).is_err());
}

#[test]
fn duplicate_declarations_domains_properties_and_full_uuid_grammar_refuse() {
    project(&fixture()).unwrap();
    let duplicate_registry = registry(&[(ROOT, NIL_UUID), (CHILD, ROOT), (CHILD, ROOT)]);
    let mut trees = fixture();
    trees[0] = duplicate_registry;
    assert!(project(&trees).is_err());
    let mut trees = fixture();
    trees[1] = sets(&[SET, SET]);
    assert!(project(&trees).is_err());
    let mut trees = fixture();
    fields(&mut trees[1], 1)[1] = a(CHILD);
    assert!(project(&trees).is_err()); // Even a non-emitted set cannot collide with an owner.
    let mut trees = fixture();
    trees[2] = help(&[
        (CHILD, vec![]),
        (CHILD, vec![]),
        (NIL_UUID, vec![(23, links(&[]))]),
    ]);
    assert!(project(&trees).is_err());
    let mut trees = fixture();
    trees[2] = help(&[
        (CHILD, vec![(7, a(1)), (7, a(2))]),
        (NIL_UUID, vec![(23, links(&[]))]),
    ]);
    assert!(project(&trees).is_err());
    let mut trees = fixture();
    fields(&mut trees[1], 1)[2]
        .as_list_mut()
        .unwrap()
        .push(l(vec![Brace::str("#"), a(TYPE.to_uppercase())]));
    assert!(project(&trees).is_err());
    for invalid in [
        "11111111",
        "{11111111-1111-4111-8111-111111111111}",
        "not-a-uuid",
    ] {
        let mut trees = fixture();
        fields(&mut trees[0], 2)[1] = a(invalid);
        assert!(project(&trees).is_err());
    }
    let mut trees = fixture();
    fields(&mut trees[0], 1).extend([a(CLASS)]);
    fields(&mut trees[0], 1)[0] = a(2);
    assert!(project(&trees).is_err());
}

#[test]
fn owner_root_kind_cycle_forward_orphan_and_reopened_subtree_are_not_inferred() {
    let mut trees = fixture();
    for records in [
        vec![(CHILD, NIL_UUID)],
        vec![(ROOT, NIL_UUID), (CHILD, CHILD)],
        vec![(ROOT, NIL_UUID), (CHILD, SET)],
        vec![(ROOT, NIL_UUID), (CHILD, NIL_UUID)],
        vec![(ROOT, NIL_UUID), (CHILD, SET), (SET, CHILD)],
        vec![(ROOT, NIL_UUID), (CHILD, ROOT), (SET, ROOT), (TYPE, CHILD)],
    ] {
        trees[0] = registry(&records);
        assert!(project(&trees).is_err());
    }
    let mut trees = fixture();
    fields(&mut trees[0], 2)[3] = a(1);
    assert!(project(&trees).is_err());
    let mut trees = fixture();
    fields(&mut trees[0], 2)[3] = a(usize::MAX);
    assert!(project(&trees).is_err());
    let mut trees = fixture();
    trees[0] = registry(&[(ROOT, NIL_UUID), (CHILD, ROOT), (TYPE, CHILD)]);
    assert_eq!(project(&trees).unwrap().registry.children(1), vec![2]);
}

#[test]
fn nil_class_is_not_a_registry_class_and_unknown_nonnil_class_stays_partial() {
    project(&fixture()).unwrap();
    let mut trees = fixture();
    fields(&mut trees[0], 1)[1] = a(NIL_UUID);
    assert!(project(&trees).is_err());
    fields(&mut trees[0], 1)[1] = a("66666666-6666-4666-8666-666666666666");
    let admitted = project(&trees).unwrap();
    assert_eq!(
        admitted.registry.classes,
        vec!["66666666-6666-4666-8666-666666666666"]
    );
    assert_eq!(
        admitted.graph_completeness,
        diagnostic::GraphCompleteness::Partial
    );
    assert_eq!(
        admitted.trace_status,
        diagnostic::TraceStatus::NotIdentified
    );
    assert!(admitted.first_visit_authority.is_none());
}

#[test]
fn unresolved_keys_and_nil_wrapper_shape_counts_classes_duplicates_are_closed() {
    project(&fixture()).unwrap();
    let mut trees = fixture();
    fields(&mut trees[2], 1)[1] = a(TYPE);
    assert!(project(&trees).is_err());
    for value in [
        a(0),
        l(vec![]),
        links(&[]),
        links(&[SET, SET]),
        links(&[TYPE]),
    ] {
        let mut trees = fixture();
        trees[2] = help(&[(SET, vec![(23, a(0))]), (NIL_UUID, vec![(23, value)])]);
        assert!(project(&trees).is_err());
    }
    for mutation in 0..5 {
        let mut value = links(&[SET]);
        match mutation {
            0 => value.as_list_mut().unwrap()[1] = a(CLASS),
            1 => fields(&mut value, 2)[1] = a(2),
            2 => fields(&mut value, 2)[2].as_list_mut().unwrap()[1] = a(CLASS),
            3 => fields(&mut value, 2)[2].as_list_mut().unwrap().push(a(99)),
            _ => {
                fields(&mut value, 2)[2].as_list_mut().unwrap()[2]
                    .as_list_mut()
                    .unwrap()[0] = a(0)
            }
        }
        let mut trees = fixture();
        trees[2] = help(&[(SET, vec![(23, a(0))]), (NIL_UUID, vec![(23, value)])]);
        assert!(project(&trees).is_err());
    }
    let mut trees = fixture();
    trees[2] = help(&[(SET, vec![(23, a(0))])]);
    assert!(project(&trees).is_err());
    let mut trees = fixture();
    trees[2] = help(&[(SET, vec![]), (NIL_UUID, vec![(23, links(&[SET]))])]);
    assert!(project(&trees).is_err());
}

struct Files {
    root: PathBuf,
}
impl Files {
    fn new() -> Self {
        let root = fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!("ibcmd-cache-trace-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        Self { root }
    }
    fn manifest(&self, encoding: usize) -> ManifestV1 {
        let predecessor_path = self.root.join("predecessor.json");
        let predecessor = b"generated predecessor";
        fs::write(&predecessor_path, predecessor).unwrap();
        let rows = fixture()
            .iter()
            .zip([RowRole::Registry, RowRole::TypeSets, RowRole::HelpProps])
            .map(|(tree, role)| {
                let plain = serialize_row(tree);
                let packed = if encoding == 0 {
                    plain.clone()
                } else {
                    let mut encoder = flate2::write::DeflateEncoder::new(
                        Vec::new(),
                        flate2::Compression::default(),
                    );
                    encoder.write_all(&plain).unwrap();
                    encoder.finish().unwrap()
                };
                let path = self.root.join(role.filename());
                let (stored, locator) = if encoding == 2 {
                    let mut encoder =
                        flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
                    encoder.write_all(&packed).unwrap();
                    let member = encoder.finish().unwrap();
                    let mut container = b"not-a-member-prefix".to_vec();
                    container.extend(&member);
                    container.extend(b"later-pack-data");
                    fs::write(&path, container).unwrap();
                    (member, Locator::GzipRawDeflateRange { path, offset: 19 })
                } else {
                    fs::write(&path, &packed).unwrap();
                    (
                        packed.clone(),
                        if encoding == 0 {
                            Locator::Plain { path }
                        } else {
                            Locator::RawDeflate { path }
                        },
                    )
                };
                RowBinding {
                    role,
                    origin: RowOrigin {
                        case: "generated".into(),
                        stage: "current".into(),
                        table: "Params".into(),
                        filename: role.filename().into(),
                        part: 0,
                        version: "bound-generated-version".into(),
                    },
                    locator,
                    stored_length: stored.len() as u64,
                    stored_sha256: hash(&stored),
                    packed_length: packed.len() as u64,
                    packed_sha256: hash(&packed),
                    plain_length: plain.len() as u64,
                    plain_sha256: hash(&plain),
                }
            })
            .collect();
        ManifestV1 {
            schema: "cache-insertion-trace-input-v1".into(),
            source_head: SOURCE.into(),
            case: "generated".into(),
            stage: "current".into(),
            snapshot_purpose: "cleanroom-preflight".into(),
            registry_root_uuid: ROOT.into(),
            predecessors: vec![FileBinding {
                path: predecessor_path,
                length: predecessor.len() as u64,
                sha256: hash(predecessor),
            }],
            rows,
        }
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        // Only this newly created UUID directory, never inputs named by a caller manifest.
        if self
            .root
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("ibcmd-cache-trace-")
        {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

#[test]
fn explicit_manifest_all_three_encodings_preserves_literal_bom_crlf_and_payload() {
    for encoding in 0..3 {
        let files = Files::new();
        let manifest = files.manifest(encoding);
        let json = serde_json::to_vec(&manifest).unwrap();
        let admitted: ManifestV1 = serde_json::from_slice(&json).unwrap();
        let inputs = ProjectionInputs::load(admitted, SOURCE).unwrap();
        for (tree, role) in
            fixture()
                .iter()
                .zip([RowRole::Registry, RowRole::TypeSets, RowRole::HelpProps])
        {
            assert_eq!(inputs.raw(role), serialize_row(tree));
            assert!(inputs.raw(role).starts_with(&[0xef, 0xbb, 0xbf]));
            assert!(inputs.raw(role).windows(2).any(|pair| pair == b"\r\n"));
        }
        assert_eq!(inputs.project().unwrap().origins.len(), 3);
        inputs.verify_sources().unwrap();
    }
}

#[test]
fn manifest_missing_mixed_alias_unknown_field_and_identity_hash_mutants_refuse() {
    let files = Files::new();
    let manifest = files.manifest(0);
    ProjectionInputs::load(manifest.clone(), SOURCE)
        .unwrap()
        .project()
        .unwrap();
    for mutant in 0..13 {
        let mut m = manifest.clone();
        match mutant {
            0 => {
                m.rows.pop();
            }
            1 => m.rows.push(m.rows[0].clone()),
            2 => m.rows[1].origin.stage = "other".into(),
            3 => m.rows[1].origin.case = "other".into(),
            4 => m.rows[1].origin.table = "Config".into(),
            5 => m.rows[1].origin.part = 1,
            6 => m.rows[1].origin.filename = "wrong.si".into(),
            7 => m.source_head = "0".repeat(40),
            8 => m.rows[1].stored_sha256 = "0".repeat(64),
            9 => m.rows[1].plain_sha256 = "0".repeat(64),
            10 => m.rows[1].packed_length += 1,
            11 => m.rows[1].locator = m.rows[0].locator.clone(),
            _ => m.predecessors.clear(),
        }
        assert!(
            ProjectionInputs::load(m, SOURCE).is_err(),
            "mutant {mutant}"
        );
    }
    let mut json = serde_json::to_value(&manifest).unwrap();
    json["surprise"] = serde_json::json!(1);
    assert!(serde_json::from_value::<ManifestV1>(json).is_err());
    let mut m = manifest;
    m.rows[0].locator = Locator::Plain {
        path: files.root.join("missing"),
    };
    assert!(ProjectionInputs::load(m, SOURCE).is_err());
}

#[test]
fn retained_input_rejects_late_source_change_without_normalizing_eof_or_line_endings() {
    let files = Files::new();
    let manifest = files.manifest(0);
    let inputs = ProjectionInputs::load(manifest, SOURCE).unwrap();
    inputs.project().unwrap();
    let original = inputs.raw(RowRole::HelpProps).to_vec();
    let mut changed = original.clone();
    changed.push(b'\n');
    fs::write(files.root.join(RowRole::HelpProps.filename()), changed).unwrap();
    assert_eq!(inputs.raw(RowRole::HelpProps), original);
    assert!(inputs.project().is_err());
    assert!(inputs.verify_sources().is_err());
}

#[test]
fn closed_compressed_input_refuses_trailing_truncation_and_out_of_file_ranges() {
    for encoding in [1, 2] {
        let files = Files::new();
        let original = files.manifest(encoding);
        ProjectionInputs::load(original.clone(), SOURCE)
            .unwrap()
            .project()
            .unwrap();
        for truncate in [false, true] {
            let mut m = original.clone();
            let binding = &mut m.rows[0];
            let path = files.root.join("corrupt");
            let original_path = files.root.join(RowRole::Registry.filename());
            let container = fs::read(original_path).unwrap();
            let mut bytes = if encoding == 2 {
                container[19..19 + binding.stored_length as usize].to_vec()
            } else {
                container
            };
            if truncate {
                bytes.truncate(bytes.len() / 2);
            } else {
                bytes.push(0);
            }
            fs::write(&path, &bytes).unwrap();
            binding.locator = if encoding == 2 {
                Locator::GzipRawDeflateRange { path, offset: 0 }
            } else {
                Locator::RawDeflate { path }
            };
            binding.stored_length = bytes.len() as u64;
            binding.stored_sha256 = hash(&bytes);
            if encoding == 1 {
                binding.packed_length = bytes.len() as u64;
                binding.packed_sha256 = hash(&bytes);
            }
            assert!(ProjectionInputs::load(m, SOURCE).is_err());
        }
        let mut m = original;
        m.rows[0].locator = Locator::GzipRawDeflateRange {
            path: files.root.join(RowRole::Registry.filename()),
            offset: u64::MAX,
        };
        assert!(ProjectionInputs::load(m, SOURCE).is_err());
    }
}

#[test]
fn predecessor_closure_is_checked_before_and_after_projection() {
    let files = Files::new();
    let manifest = files.manifest(0);
    let inputs = ProjectionInputs::load(manifest.clone(), SOURCE).unwrap();
    inputs.project().unwrap();
    fs::write(&manifest.predecessors[0].path, b"changed predecessor").unwrap();
    assert!(inputs.verify_sources().is_err());
    assert!(inputs.project().is_err());
    assert!(ProjectionInputs::load(manifest, SOURCE).is_err());
}

#[test]
fn independent_literal_comparison_keeps_bom_crlf_properties_and_eof() {
    let original = serialize_row(&fixture()[2]);
    assert_eq!(first_raw_difference(&original, &original.clone()), None);
    assert_eq!(first_raw_difference(&original, &original[3..]), Some(0));
    let mut eof = original.clone();
    eof.push(b'\n');
    assert_eq!(first_raw_difference(&original, &eof), Some(original.len()));
    let offset = original.windows(2).position(|p| p == b"\r\n").unwrap();
    let mut lf = original.clone();
    lf.remove(offset);
    assert_eq!(first_raw_difference(&original, &lf), Some(offset));
    let mut property = original.clone();
    let offset = original
        .windows(14)
        .position(|p| p == b"opaque-preserv")
        .unwrap();
    property[offset] = b'X';
    assert_eq!(first_raw_difference(&original, &property), Some(offset));
}

// Appended A1.3/A1.4 controls. All preceding thirteen controls remain byte-identical.
const SECOND_SET: &str = "99999999-9999-4999-8999-999999999999";

fn metadata_reference(target: &str) -> Brace {
    l(vec![
        Brace::str("#"),
        a(facts::METADATA_REF),
        l(vec![a(1), a(target)]),
    ])
}

// This cleanroom row exercises the existing owner-facts projection, not a full native codec.
// Unmapped body/version semantics deliberately remain Partial in every graph.
fn observed_catalog_record() -> Brace {
    use ibcmd_rs::metadata_model::{md_base, xml::Element};
    use ibcmd_rs::restructure::caches::slots::RecordMap;
    let map = RecordMap::new("Catalog", 57).unwrap();
    let length = map
        .names
        .values()
        .copied()
        .chain(map.generated.iter().map(|(_, i)| i + 1))
        .chain(std::iter::once(map.header))
        .max()
        .unwrap()
        + 1;
    let mut record = vec![a(0); length];
    record[0] = a(57);
    for (ordinal, (_, position)) in map.generated.iter().enumerate() {
        record[*position] = a(uuid::Uuid::from_u128(100 + ordinal as u128 * 2));
        record[*position + 1] = a(uuid::Uuid::from_u128(101 + ordinal as u128 * 2));
    }
    let props = Element {
        name: "Properties".into(),
        children: vec![Element {
            name: "Name".into(),
            text: CHILD.into(),
            ..Default::default()
        }],
        ..Default::default()
    };
    record[map.header] = l(vec![a(0), md_base(CHILD, &props)]);
    l(vec![a(1), l(record), a(0)])
}

fn facts_fixture() -> (Files, ProjectionInputs, Brace, Brace) {
    use ibcmd_rs::restructure::caches::{
        root,
        slots::{RecordMap, owner_record},
        type_index::{Entry, Section, TypeIndex, TypeSlot},
    };
    let files = Files::new();
    let mut descriptor = observed_catalog_record();
    let (_, tag) = owner_record(&descriptor).unwrap();
    let map = RecordMap::new("Catalog", tag).unwrap();
    let position = map.names["Owners"];
    fields(&mut descriptor, 1)[position] = l(vec![
        a(0),
        a(2),
        metadata_reference(ROOT),
        metadata_reference(ROOT),
    ]);
    let parsed = facts::ObjectFacts::parse("Catalog", &descriptor).unwrap();
    let type_id = parsed.generated[0].type_id.clone();
    let index = TypeIndex {
        sections: vec![Section {
            class: root::CATALOG_CLASS.into(),
            entries: vec![Entry {
                object: CHILD.into(),
                types: parsed
                    .generated
                    .iter()
                    .enumerate()
                    .map(|(ordinal, d)| TypeSlot {
                        type_id: d.type_id.clone(),
                        value_id: d.value_id.clone(),
                        index: ordinal as u32,
                    })
                    .collect(),
            }],
        }],
    };
    let mut trees = fixture();
    trees[0].as_list_mut().unwrap()[1] = l(vec![a(2), a(CLASS), a(root::CATALOG_CLASS)]);
    fields(&mut trees[0], 2)[10] = a(1); // second seven-member record's class index
    trees[1] = sets(&[SET, SECOND_SET]);
    for at in [2, 4] {
        let pattern = fields(&mut trees[1], 1)[at].as_list_mut().unwrap();
        pattern[1] = l(vec![Brace::str("#"), a(&type_id)]);
    }
    trees[2] = help(&[
        (SET, vec![(23, a(0))]),
        (SECOND_SET, vec![(23, a(0))]),
        (CHILD, vec![(7, Brace::str("unchanged current property"))]),
        (
            NIL_UUID,
            vec![
                (23, links(&[SECOND_SET, SET])),
                (24, links(&[SET, SECOND_SET])),
            ],
        ),
    ]);
    let mut manifest = files.manifest(0);
    for (row, tree) in manifest.rows.iter_mut().zip(trees) {
        let bytes = serialize_row(&tree);
        let Locator::Plain { path } = &row.locator else {
            unreachable!()
        };
        fs::write(path, &bytes).unwrap();
        row.stored_length = bytes.len() as u64;
        row.packed_length = bytes.len() as u64;
        row.plain_length = bytes.len() as u64;
        row.stored_sha256 = hash(&bytes);
        row.packed_sha256 = hash(&bytes);
        row.plain_sha256 = hash(&bytes);
    }
    (
        files,
        ProjectionInputs::load(manifest, SOURCE).unwrap(),
        descriptor,
        ibcmd_rs::metadata_model::brace::parse_row(&index.render()).unwrap(),
    )
}

fn additional_manifest(
    files: &Files,
    descriptor: &Brace,
    index: Option<&Brace>,
) -> diagnostic::FactsManifestV1 {
    use diagnostic::{FactRole, FactRowBinding, FactsManifestV1};
    let make = |role, filename: &str, table: &str, tree: &Brace| {
        let bytes = serialize_row(tree);
        let path = files.root.join(format!("fact-{filename}"));
        fs::write(&path, &bytes).unwrap();
        FactRowBinding {
            role,
            origin: RowOrigin {
                case: "generated".into(),
                stage: "current".into(),
                table: table.into(),
                filename: filename.into(),
                part: 0,
                version: "own-facts-version".into(),
            },
            file: FileBinding {
                path,
                length: bytes.len() as u64,
                sha256: hash(&bytes),
            },
        }
    };
    let mut rows = vec![make(
        FactRole::Descriptor {
            owner: CHILD.into(),
        },
        CHILD,
        "Config",
        descriptor,
    )];
    if let Some(index) = index {
        rows.push(make(
            FactRole::TypeIndex,
            "2203278d-ef4f-4f68-98f1-feb257d53ecc.si",
            "Params",
            index,
        ));
    }
    FactsManifestV1 {
        schema: "cache-insertion-trace-facts-v1".into(),
        source_head: SOURCE.into(),
        case: "generated".into(),
        stage: "current".into(),
        rows,
    }
}

#[test]
fn source_bound_facts_keep_shared_reference_occurrences_physical_slots_and_separate_nil_observations()
 {
    use diagnostic::{FactInputs, NilActionKind, ObservationEvent, Target, TypeMemberFact};
    let (files, base, descriptor, index) = facts_fixture();
    let inputs = FactInputs::load(
        &base,
        additional_manifest(&files, &descriptor, Some(&index)),
    )
    .unwrap();
    assert_eq!(inputs.raw(0), Some(serialize_row(&descriptor).as_slice()));
    let graph = inputs.project(&base).unwrap();
    assert_eq!(graph.descriptors.len(), 1);
    let own = &graph.descriptors[0];
    assert_eq!(own.owner, CHILD);
    assert_eq!(own.kind, "Catalog");
    assert_eq!(own.tag, 57);
    assert_eq!(graph.type_index.as_ref().unwrap().sections.len(), 1);
    assert!(
        own.slots
            .windows(2)
            .all(|pair| pair[0].position < pair[1].position)
    );
    assert_eq!(own.facts.uuid, CHILD);
    assert_eq!(graph.generated.len(), own.facts.generated.len());
    for declaration in &graph.generated {
        assert_eq!(declaration.owner, CHILD);
        assert_eq!(
            declaration.class,
            ibcmd_rs::restructure::caches::root::CATALOG_CLASS
        );
        assert!((declaration.category_index as usize) < own.facts.generated.len());
        assert_eq!(
            index.at(&declaration.occurrence.path),
            Some(&a(&declaration.type_id))
        );
        assert_eq!(
            declaration.occurrence.source.filename,
            "2203278d-ef4f-4f68-98f1-feb257d53ecc.si"
        );
    }
    let owners_slot = own.slots.iter().find(|slot| slot.name == "Owners").unwrap();
    assert_eq!(
        descriptor.at(&[1, owners_slot.position]),
        Some(&owners_slot.value)
    );
    assert!(
        graph
            .events
            .iter()
            .any(|e| matches!(e, ObservationEvent::TypeSetVisit {
        member: TypeMemberFact::Primitive { tag, raw }, first_in_domain: None, ..
    } if tag == "S" && raw == &l(vec![Brace::str("S"), a(1024), a(1)])))
    );
    let references: Vec<_> = graph
        .events
        .iter()
        .filter_map(|event| match event {
            ObservationEvent::ReferenceVisit {
                owner,
                owner_identity_proven,
                slot,
                target:
                    Target::Metadata {
                        uuid,
                        registry_ordinal,
                    },
                first_in_domain,
                occurrence,
            } => Some((
                owner,
                owner_identity_proven,
                slot,
                uuid,
                registry_ordinal,
                first_in_domain,
                occurrence,
            )),
            _ => None,
        })
        .collect();
    assert_eq!(references.len(), 2);
    for (owner, proven, slot, target, ordinal, _, occurrence) in &references {
        assert_eq!(owner.as_str(), CHILD);
        assert!(**proven);
        assert_eq!(**slot, Some("Owners"));
        assert_eq!(target.as_str(), ROOT);
        assert_eq!(**ordinal, Some(0));
        assert_eq!(
            descriptor.at(&occurrence.path),
            Some(&metadata_reference(ROOT))
        );
        assert_eq!(occurrence.plain_sha256, hash(&serialize_row(&descriptor)));
    }
    assert!(*references[0].5);
    assert!(!*references[1].5);
    assert_ne!(references[0].6.path, references[1].6.path);
    let type_visits: Vec<_> = graph
        .events
        .iter()
        .filter_map(|event| match event {
            ObservationEvent::TypeSetVisit {
                member: TypeMemberFact::Generated(Target::TypeId { declaration, .. }),
                first_in_domain,
                ..
            } => Some((declaration, first_in_domain)),
            _ => None,
        })
        .collect();
    assert_eq!(type_visits.len(), 2);
    assert!(
        type_visits
            .iter()
            .all(|(declaration, _)| declaration.is_some())
    );
    assert_eq!(
        (*type_visits[0].1, *type_visits[1].1),
        (Some(true), Some(false))
    );
    let nil_links: Vec<_> = graph
        .events
        .iter()
        .filter_map(|event| match event {
            ObservationEvent::NilAction {
                action: NilActionKind::ObservedSetLink,
                set,
                ..
            } => set.as_deref(),
            _ => None,
        })
        .collect();
    assert_eq!(nil_links, vec![SECOND_SET, SET]); // original property23 order, no sorting seed
    assert_eq!(
        graph
            .events
            .iter()
            .filter(|e| matches!(e, ObservationEvent::OwnerVisit { .. }))
            .count(),
        2
    );
    assert!(
        graph
            .events
            .iter()
            .all(|e| !matches!(e, ObservationEvent::EmitKey { .. }))
    );
    assert!(graph.candidate.is_none() && graph.first_visit_authority.is_none());
    assert_eq!(graph.pending_atoms, ["A1.5", "A1.6", "A1.7"]);
    assert_eq!(
        graph.graph_completeness,
        diagnostic::GraphCompleteness::Partial
    );
    assert_eq!(graph.trace_status, diagnostic::TraceStatus::NotIdentified);
}

#[test]
fn missing_unknown_and_nested_reference_facts_never_become_no_refs_or_type_authority() {
    use diagnostic::{FactInputs, ObservationEvent, Target};
    let (files, base, mut descriptor, _) = facts_fixture();
    let map = ibcmd_rs::restructure::caches::slots::RecordMap::new("Catalog", 57).unwrap();
    fields(&mut descriptor, 1)[map.names["Owners"]] =
        l(vec![Brace::str("#"), a(CLASS), metadata_reference(ROOT)]);
    let graph = FactInputs::load(&base, additional_manifest(&files, &descriptor, None))
        .unwrap()
        .project(&base)
        .unwrap();
    for reason in [
        "missing-type-index",
        "missing-descriptor",
        "unknown-tagged-reference-or-value",
        "unresolved-type-id-member",
    ] {
        assert!(graph.partial.iter().any(|p| p.reason == reason), "{reason}");
    }
    assert!(graph.events.iter().any(|event| matches!(event, ObservationEvent::ReferenceVisit { target: Target::Unknown { raw }, .. } if raw == descriptor.at(&[1, map.names["Owners"]]).unwrap())));
    assert!(graph.events.iter().any(|event| matches!(event, ObservationEvent::ReferenceVisit { target: Target::Metadata { uuid, registry_ordinal: Some(0) }, .. } if uuid == ROOT)));
    assert!(graph.candidate.is_none() && graph.first_visit_authority.is_none());
}

#[test]
fn current_type_index_counts_nil_duplicates_owner_class_and_descriptor_pairs_refuse() {
    use diagnostic::FactInputs;
    let (files, base, descriptor, index) = facts_fixture();
    FactInputs::load(
        &base,
        additional_manifest(&files, &descriptor, Some(&index)),
    )
    .unwrap()
    .project(&base)
    .unwrap();
    for mutation in 0..10 {
        let mut mutant = index.clone();
        let body = fields(&mut mutant, 1);
        match mutation {
            0 => body[0] = a(999),
            1 => body[1] = a(NIL_UUID),
            2 => body[3] = a(ROOT),
            3 => body[1] = a(CLASS),
            4 => body[4] = a(usize::MAX),
            5 => body[5] = a(NIL_UUID),
            6 => body[6] = a(NIL_UUID),
            7 => body[8] = body[5].clone(),
            8 => body[9] = body[6].clone(),
            _ => body[10] = body[7].clone(),
        }
        assert!(
            FactInputs::load(
                &base,
                additional_manifest(&files, &descriptor, Some(&mutant))
            )
            .unwrap()
            .project(&base)
            .is_err(),
            "mutation {mutation}"
        );
    }
    let mut mutant = descriptor.clone();
    fields(&mut mutant, 1)[1] = a(TYPE); // syntactically valid but different actual generated pair
    assert!(
        FactInputs::load(&base, additional_manifest(&files, &mutant, Some(&index)))
            .unwrap()
            .project(&base)
            .is_err()
    );
}

#[test]
fn descriptor_identity_counts_and_known_reference_shape_are_closed_before_facts() {
    use diagnostic::FactInputs;
    let (files, base, descriptor, index) = facts_fixture();
    let map = ibcmd_rs::restructure::caches::slots::RecordMap::new("Catalog", 57).unwrap();
    for mutation in 0..7 {
        let mut mutant = descriptor.clone();
        match mutation {
            0 => mutant.as_list_mut().unwrap()[2] = a(999),
            1 => {
                fields(&mut mutant, 1)[map.header].as_list_mut().unwrap()[1]
                    .as_list_mut()
                    .unwrap()[1]
                    .as_list_mut()
                    .unwrap()[2] = a(ROOT)
            }
            2 => {
                fields(&mut mutant, 1)[map.header].as_list_mut().unwrap()[1]
                    .as_list_mut()
                    .unwrap()[2] = Brace::str("other-name")
            }
            3 => {
                fields(&mut mutant, 1)[map.names["Owners"]] = l(vec![
                    Brace::str("#"),
                    a(facts::METADATA_REF),
                    l(vec![a(0), a(NIL_UUID)]),
                ])
            }
            4 => {
                fields(&mut mutant, 1)[map.names["Owners"]] = l(vec![
                    Brace::str("#"),
                    a(facts::METADATA_REF),
                    l(vec![a(1), a(ROOT), a(99)]),
                ])
            }
            5 => {
                fields(&mut mutant, 1)[map.names["Owners"]] = l(vec![
                    Brace::str("wrong-tag"),
                    a(facts::METADATA_REF),
                    l(vec![a(1), a(ROOT)]),
                ])
            }
            _ => {
                fields(&mut mutant, 1)[map.header].as_list_mut().unwrap()[1]
                    .as_list_mut()
                    .unwrap()[3] = l(vec![a(usize::MAX)])
            }
        }
        assert!(
            FactInputs::load(&base, additional_manifest(&files, &mutant, Some(&index)))
                .unwrap()
                .project(&base)
                .is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn fact_ledger_mixed_duplicate_alias_and_late_drift_are_not_inferred_from_paths() {
    use diagnostic::{FactInputs, FactRole};
    let (files, base, descriptor, index) = facts_fixture();
    let manifest = additional_manifest(&files, &descriptor, Some(&index));
    for mutation in 0..6 {
        let mut mutant = manifest.clone();
        match mutation {
            0 => mutant.stage = "other-stage".into(),
            1 => mutant.rows[0].origin.case = "other-case".into(),
            2 => mutant.rows[0].origin.part = 1,
            3 => mutant.rows.push(mutant.rows[0].clone()),
            4 => mutant.rows[0].role = FactRole::Descriptor { owner: ROOT.into() },
            _ => mutant.rows[1].file = mutant.rows[0].file.clone(),
        }
        assert!(
            FactInputs::load(&base, mutant).is_err(),
            "mutation {mutation}"
        );
    }
    let loaded = FactInputs::load(&base, manifest.clone()).unwrap();
    loaded.project(&base).unwrap();
    fs::write(
        &manifest.rows[0].file.path,
        b"late different current descriptor",
    )
    .unwrap();
    assert!(loaded.project(&base).is_err());
}

fn rebound_base(
    files: &Files,
    original: &ProjectionInputs,
    role: RowRole,
    tree: &Brace,
) -> ProjectionInputs {
    let originals: Vec<_> = [RowRole::Registry, RowRole::TypeSets, RowRole::HelpProps]
        .into_iter()
        .map(|role| (role, original.raw(role).to_vec()))
        .collect();
    let mut manifest = files.manifest(0);
    for (binding, (current_role, mut bytes)) in manifest.rows.iter_mut().zip(originals) {
        if current_role == role {
            bytes = serialize_row(tree);
        }
        let Locator::Plain { path } = &binding.locator else {
            unreachable!()
        };
        fs::write(path, &bytes).unwrap();
        binding.stored_length = bytes.len() as u64;
        binding.packed_length = bytes.len() as u64;
        binding.plain_length = bytes.len() as u64;
        binding.stored_sha256 = hash(&bytes);
        binding.packed_sha256 = hash(&bytes);
        binding.plain_sha256 = hash(&bytes);
    }
    ProjectionInputs::load(manifest, SOURCE).unwrap()
}

#[test]
fn generated_value_id_is_not_a_type_id_and_unknown_primitive_payload_is_retained() {
    use diagnostic::{FactInputs, ObservationEvent, Target, TypeMemberFact};
    let (files, base, descriptor, index) = facts_fixture();
    let values = facts::ObjectFacts::parse("Catalog", &descriptor).unwrap();
    let value_id = values.generated[0].value_id.clone();
    let mut sets = ibcmd_rs::metadata_model::brace::parse_row(base.raw(RowRole::TypeSets)).unwrap();
    let pattern = fields(&mut sets, 1)[2].as_list_mut().unwrap();
    pattern[1] = l(vec![Brace::str("#"), a(&value_id)]);
    let unknown = l(vec![Brace::str("B"), Brace::str("unexpected qualifier")]);
    pattern.push(unknown.clone());
    let base = rebound_base(&files, &base, RowRole::TypeSets, &sets);
    let graph = FactInputs::load(
        &base,
        additional_manifest(&files, &descriptor, Some(&index)),
    )
    .unwrap()
    .project(&base)
    .unwrap();
    assert!(graph.generated.iter().any(|d| d.value_id == value_id));
    assert!(graph.events.iter().any(|e| matches!(e, ObservationEvent::TypeSetVisit { member: TypeMemberFact::Generated(Target::TypeId { uuid, declaration: None }), .. } if uuid == &value_id)));
    assert!(graph.events.iter().any(|e| matches!(e, ObservationEvent::TypeSetVisit { member: TypeMemberFact::Unknown { raw }, .. } if raw == &unknown)));
    assert!(
        graph
            .partial
            .iter()
            .any(|p| p.reason == "unknown-primitive-or-member-shape")
    );
    assert!(
        graph
            .partial
            .iter()
            .any(|p| p.reason == "unresolved-type-id-member")
    );
}

#[test]
fn design_time_instance_value_and_unresolved_metadata_edges_do_not_gain_generated_value_authority()
{
    use diagnostic::{FactInputs, ObservationEvent, Target};
    let (files, base, mut descriptor, index) = facts_fixture();
    let parsed = facts::ObjectFacts::parse("Catalog", &descriptor).unwrap();
    let type_id = parsed.generated[0].type_id.clone();
    let value_id = parsed.generated[0].value_id.clone();
    let map = ibcmd_rs::restructure::caches::slots::RecordMap::new("Catalog", 57).unwrap();
    let instance = l(vec![
        Brace::str("#"),
        a(ibcmd_rs::metadata_model::types::DESIGN_TIME_REF_TYPE),
        l(vec![a(0), a(&type_id), a(&value_id)]),
    ]);
    fields(&mut descriptor, 1)[map.names["Owners"]] =
        l(vec![a(0), a(2), instance, metadata_reference(TYPE)]);
    let graph = FactInputs::load(
        &base,
        additional_manifest(&files, &descriptor, Some(&index)),
    )
    .unwrap()
    .project(&base)
    .unwrap();
    assert!(graph.events.iter().any(|e| matches!(e, ObservationEvent::ReferenceVisit { target: Target::DesignTimeValue { type_id: actual_type, value_id: actual_value, type_declaration: Some(_) }, .. } if actual_type == &type_id && actual_value == &value_id)));
    assert!(graph.partial.iter().any(|p| p.reason == "design-time-instance-value-not-resolved-by-generated-value-index"));
    assert!(
        graph
            .partial
            .iter()
            .any(|p| p.reason == "unresolved-metadata-reference")
    );
    assert!(graph.candidate.is_none() && graph.first_visit_authority.is_none());
}

#[test]
fn retained_fact_input_rejects_another_valid_base_and_unknown_layout_stays_partial() {
    use diagnostic::{FactInputs, ObservationEvent};
    let (files, base, descriptor, index) = facts_fixture();
    let retained = FactInputs::load(
        &base,
        additional_manifest(&files, &descriptor, Some(&index)),
    )
    .unwrap();
    retained.project(&base).unwrap();
    let (_other_files, other_base, _, _) = facts_fixture();
    assert!(retained.project(&other_base).is_err());
    let mut registry =
        ibcmd_rs::metadata_model::brace::parse_row(base.raw(RowRole::Registry)).unwrap();
    fields(&mut registry, 2)[10] = a(0); // valid unknown nonnil class from the declared registry
    let base = rebound_base(&files, &base, RowRole::Registry, &registry);
    let graph = FactInputs::load(&base, additional_manifest(&files, &descriptor, None))
        .unwrap()
        .project(&base)
        .unwrap();
    assert!(graph.descriptors.is_empty());
    assert!(
        graph
            .partial
            .iter()
            .any(|p| p.reason == "unsupported-descriptor-class-or-record-map")
    );
    assert!(graph.events.iter().any(|e| matches!(
        e,
        ObservationEvent::ReferenceVisit {
            owner_identity_proven: false,
            ..
        }
    )));
    assert!(graph.candidate.is_none() && graph.first_visit_authority.is_none());
    // This is a reserved contract variant, never produced by the current observed graph.
    assert!(
        !graph
            .events
            .contains(&ObservationEvent::EmitKey { key: CHILD.into() })
    );
}

#[test]
fn canonical_empty_reference_values_keep_raw_occurrences_without_nil_resolution() {
    use diagnostic::{EmptyValueKind, FactInputs, ObservationEvent, Target};
    use ibcmd_rs::metadata_model::{DescriptorContext, types};
    let (files, base, mut descriptor, index) = facts_fixture();
    let context = DescriptorContext::with_files(&files.root, "2.20", &[]).unwrap();
    let metadata_empty = types::metadata_ref_uuid(NIL_UUID);
    let design_empty = types::design_time_ref("", &context).unwrap();
    let map = ibcmd_rs::restructure::caches::slots::RecordMap::new("Catalog", 57).unwrap();
    let position = map.names["Owners"];
    // Empty values inside an unknown envelope and repeated values remain distinct occurrences.
    fields(&mut descriptor, 1)[position] = l(vec![
        Brace::str("#"),
        a(CLASS),
        metadata_empty.clone(),
        design_empty.clone(),
        metadata_reference(ROOT),
        metadata_empty.clone(),
    ]);
    let expected_bytes = serialize_row(&descriptor);
    let manifest = additional_manifest(&files, &descriptor, Some(&index));
    let origin = manifest.rows[0].origin.clone();
    let inputs = FactInputs::load(&base, manifest).unwrap();
    let graph = inputs.project(&base).unwrap();
    assert_eq!(inputs.raw(0), Some(expected_bytes.as_slice()));
    let empty: Vec<_> = graph
        .events
        .iter()
        .filter_map(|event| match event {
            ObservationEvent::EmptyValue {
                owner,
                kind,
                raw,
                occurrence,
            } => Some((owner, kind, raw, occurrence)),
            _ => None,
        })
        .collect();
    assert_eq!(empty.len(), 3);
    for ((owner, kind, raw, occurrence), (offset, expected_kind, expected_raw)) in
        empty.iter().zip([
            (2, EmptyValueKind::MetadataReference, &metadata_empty),
            (3, EmptyValueKind::DesignTimeReference, &design_empty),
            (5, EmptyValueKind::MetadataReference, &metadata_empty),
        ])
    {
        assert_eq!(*owner, CHILD);
        assert_eq!(**kind, expected_kind);
        assert_eq!(*raw, expected_raw);
        assert_eq!(occurrence.path, vec![1, position, offset]);
        assert_eq!(occurrence.source, origin);
        assert_eq!(occurrence.plain_sha256, hash(&expected_bytes));
        assert_eq!(occurrence.registry_span, None);
        assert_eq!(descriptor.at(&occurrence.path), Some(*raw));
    }
    for reason in [
        "empty-metadata-reference-value-semantic-slot-unproved",
        "empty-design-time-reference-value-semantic-slot-unproved",
    ] {
        assert!(graph.partial.iter().any(|p| p.reason == reason));
    }
    assert!(graph.events.iter().any(|e| matches!(e,
        ObservationEvent::ReferenceVisit { target: Target::Metadata { uuid, registry_ordinal: Some(0) }, .. } if uuid == ROOT)));
    assert!(!graph.events.iter().any(|e| matches!(e,
        ObservationEvent::ReferenceVisit { target: Target::Metadata { uuid, .. }, .. } if uuid == NIL_UUID)));
    assert!(!graph.events.iter().any(|e| matches!(e,
        ObservationEvent::ReferenceVisit { target: Target::DesignTimeValue { type_id, .. }, .. } if type_id == NIL_UUID)));
    assert_eq!(
        graph.graph_completeness,
        diagnostic::GraphCompleteness::Partial
    );
    assert_eq!(graph.trace_status, diagnostic::TraceStatus::NotIdentified);
    assert!(graph.candidate.is_none() && graph.first_visit_authority.is_none());
    assert!(
        !graph
            .events
            .iter()
            .any(|e| matches!(e, ObservationEvent::EmitKey { .. }))
    );
}

#[test]
fn empty_value_admission_does_not_admit_malformed_or_nil_type_declarations() {
    use diagnostic::FactInputs;
    use ibcmd_rs::metadata_model::{DescriptorContext, types};
    let (files, base, descriptor, index) = facts_fixture();
    let context = DescriptorContext::with_files(&files.root, "2.20", &[]).unwrap();
    let map = ibcmd_rs::restructure::caches::slots::RecordMap::new("Catalog", 57).unwrap();
    for mutation in 0..6 {
        let mut value = types::design_time_ref("", &context).unwrap();
        match mutation {
            0 => value.as_list_mut().unwrap()[0] = Brace::str("wrong-tag"),
            1 => fields(&mut value, 2)[0] = a(1),
            2 => {
                value.as_list_mut().unwrap().push(a(0));
            }
            3 => {
                fields(&mut value, 2).pop();
            }
            4 => fields(&mut value, 2)[2] = a(ROOT), // nil TypeId with a nonempty instance
            _ => {
                value = l(vec![
                    Brace::str("Pattern"),
                    l(vec![Brace::str("#"), a(NIL_UUID)]),
                ])
            }
        }
        let mut mutant = descriptor.clone();
        fields(&mut mutant, 1)[map.names["Owners"]] = value;
        assert!(
            FactInputs::load(&base, additional_manifest(&files, &mutant, Some(&index)))
                .unwrap()
                .project(&base)
                .is_err(),
            "mutation {mutation}"
        );
    }
}

// A1.5/A1.7 appended controls; the original twenty-three bodies above stay unchanged.
use diagnostic::comparison as cmp;

fn comparison_binding(files: &Files, name: &str, bytes: &[u8]) -> FileBinding {
    let path = files.root.join(name);
    fs::write(&path, bytes).unwrap();
    FileBinding {
        path,
        length: bytes.len() as u64,
        sha256: hash(bytes),
    }
}
fn comparison_json<T: serde::Serialize>(files: &Files, name: &str, value: &T) -> FileBinding {
    comparison_binding(files, name, &serde_json::to_vec(value).unwrap())
}
fn comparison_stage(files: &Files, mut input: ManifestV1, stage: &str) -> cmp::SnapshotDeclaration {
    input.stage = stage.into();
    for row in &mut input.rows {
        row.origin.stage = stage.into();
        row.origin.version = format!("independently-bound-{stage}-version");
    }
    let proof = comparison_binding(
        files,
        &format!("proof-{stage}.json"),
        format!("own generated capture for generated/{stage}").as_bytes(),
    );
    input.predecessors.push(proof.clone());
    let input_manifest = comparison_json(files, &format!("input-{stage}.json"), &input);
    let witness = cmp::CaptureWitnessV1 {
        schema: "cache-trace-capture-witness-v1".into(),
        case: "generated".into(),
        stage: stage.into(),
        purpose: input.snapshot_purpose.clone(),
        source_head: SOURCE.into(),
        registry_root_uuid: ROOT.into(),
        input_manifest_sha256: input_manifest.sha256.clone(),
        provenance: cmp::Provenance::WeakPlainOnlyDiagnostic,
        original_proofs: vec![proof],
    };
    cmp::SnapshotDeclaration {
        case: "generated".into(),
        stage: stage.into(),
        purpose: input.snapshot_purpose,
        registry_root_uuid: ROOT.into(),
        input_manifest,
        capture_witness: comparison_json(files, &format!("capture-{stage}.json"), &witness),
        facts_manifest: None,
    }
}
fn comparison_corpus(files: &Files, input: ManifestV1) -> cmp::RetainedCorpusV1 {
    cmp::RetainedCorpusV1 {
        schema: "cache-trace-retained-corpus-v1".into(),
        input_exporter_source_head: SOURCE.into(),
        compiled_source: cmp::compiled_sources(),
        cases: vec![cmp::RetainedCase {
            id: "generated".into(),
            provenance: cmp::Provenance::WeakPlainOnlyDiagnostic,
            current: comparison_stage(files, input.clone(), "current"),
            baseline: Some(comparison_stage(files, input, "before")),
        }],
    }
}
fn comparison_run(
    files: &Files,
    corpus: &cmp::RetainedCorpusV1,
    output: &Files,
) -> anyhow::Result<cmp::CorpusReport> {
    cmp::run_closed(
        comparison_json(files, "outer.json", corpus),
        &output.root.join("published"),
    )
}

#[test]
fn complete_literal_and_property_diagnostics_compare_same_positive_before_mutants() {
    let files = Files::new();
    let input = ProjectionInputs::load(files.manifest(0), SOURCE).unwrap();
    let current = input.project().unwrap().help;
    let raw = current.render();
    assert_eq!(raw, input.raw(RowRole::HelpProps));
    assert!(cmp::literal(&raw, &raw).first.is_none());
    let positive = cmp::payload(&current, &current).unwrap();
    assert!(positive.property_maps_equal);
    assert!(positive.first_key.is_none());
    for bytes in [
        raw[3..].to_vec(),
        raw[..raw.len() - 1].to_vec(),
        [raw.as_slice(), b"x"].concat(),
        raw.iter().copied().filter(|b| *b != b'\r').collect(),
    ] {
        assert!(cmp::literal(&raw, &bytes).first.is_some());
    }
    let eof = cmp::literal(&raw, &[raw.as_slice(), b"x"].concat())
        .first
        .unwrap();
    assert_eq!(eof.offset, raw.len());
    assert_eq!(eof.expected, cmp::ByteEndpoint::Eof);
    assert_eq!(eof.actual, cmp::ByteEndpoint::Byte(b'x'));
    let mut reordered = current.clone();
    reordered.entries.swap(0, 1);
    let compared = cmp::payload(&current, &reordered).unwrap();
    assert!(compared.first_key.is_some());
    assert!(compared.property_maps_equal); // Key order and complete values are independent facts.
    for mutation in 0..4 {
        let mut changed = current.clone();
        match mutation {
            0 => changed.entries[0].props[0].0 = "99".into(),
            1 => changed.entries[0].props[0].1 = Brace::str("different complete value"),
            2 => changed.entries[2].props.swap(0, 1),
            _ => {
                changed.entries[0].props.pop();
            }
        }
        let difference = cmp::payload(&current, &changed).unwrap();
        assert!(!difference.property_maps_equal);
        assert!(difference.first_property.is_some());
    }
    let mut missing = current.clone();
    missing.entries.remove(0);
    assert_eq!(
        cmp::payload(&current, &missing).unwrap().missing_keys,
        [SET]
    );
    let mut extra = current.clone();
    let mut new_entry = current.entries[0].clone();
    new_entry.key = TYPE.into();
    extra.entries.push(new_entry);
    assert_eq!(cmp::payload(&current, &extra).unwrap().extra_keys, [TYPE]);
    let mut duplicated = current.clone();
    duplicated.entries.push(current.entries[0].clone());
    assert!(cmp::payload(&current, &duplicated).is_err());
    let mut duplicated = current.clone();
    duplicated.entries[0]
        .props
        .push(current.entries[0].props[0].clone());
    assert!(cmp::payload(&current, &duplicated).is_err());
    assert_eq!(current.render(), raw);
    input.verify_sources().unwrap();
}
fn comparison_proposal(input: &ProjectionInputs) -> cmp::DiagnosticProposal {
    let witnesses = cmp::key_witnesses(input).unwrap();
    let keys = input
        .project()
        .unwrap()
        .help
        .entries
        .into_iter()
        .map(|e| e.key)
        .collect::<Vec<_>>();
    cmp::DiagnosticProposal {
        emits: keys
            .into_iter()
            .enumerate()
            .map(|(i, key)| cmp::ProposedEmit {
                occurrence: witnesses[&key].clone(),
                key,
                event_id: format!("explicit-diagnostic-event-{i}"),
            })
            .collect(),
    }
}
#[test]
fn proposal_actual_handler_requires_complete_current_bijection_and_source_occurrence() {
    let files = Files::new();
    let input = ProjectionInputs::load(files.manifest(0), SOURCE).unwrap();
    let original = input.raw(RowRole::HelpProps).to_vec();
    let proposal = comparison_proposal(&input);
    assert_eq!(
        proposal.emits[0].occurrence,
        cmp::key_occurrence(&input, &proposal.emits[0].key).unwrap()
    );
    let default = cmp::compare_current(&input, None).unwrap();
    assert!(default.current_roundtrip.first.is_none());
    assert!(default.proposal.is_none());
    let accepted = cmp::compare_current(&input, Some(&proposal)).unwrap();
    let projected = accepted.proposal.unwrap();
    assert!(projected.untrusted_diagnostic_only);
    assert!(projected.literal.first.is_none()); // These authored keys occupy independent buckets.
    assert!(projected.payload.property_maps_equal);
    assert_eq!(projected.associations.len(), 3);
    assert!(accepted.first_visit_authority.is_none());
    assert!(!accepted.native_acceptance);
    for mutation in 0..11 {
        let mut changed = proposal.clone();
        match mutation {
            0 => {
                changed.emits.pop();
            }
            1 => changed.emits.push(changed.emits[0].clone()),
            2 => changed.emits[0].key = TYPE.into(),
            3 => changed.emits[1].event_id = changed.emits[0].event_id.clone(),
            4 => changed.emits[0].event_id = "bad\0event".into(),
            5 => changed.emits[0].occurrence.source.stage = "other-stage".into(),
            6 => changed.emits[0].occurrence.source.case = "other-case".into(),
            7 => changed.emits[0].occurrence.plain_sha256 = "0".repeat(64),
            8 => changed.emits[0].occurrence.path.push(99),
            9 => changed.emits[0].occurrence.registry_span = Some((0, 99)),
            _ => changed.emits[0].occurrence.source.version = "other-version".into(),
        }
        assert!(
            cmp::compare_current(&input, Some(&changed)).is_err(),
            "mutation {mutation}"
        );
    }
    assert_eq!(input.raw(RowRole::HelpProps), original);
    input.verify_sources().unwrap();
}
#[test]
fn actual_growing_key_rosters_render_through_existing_model_without_native_history_authority() {
    use ibcmd_rs::restructure::caches::order;
    for size in [9usize, 65, 513, 1025, 2049] {
        let files = Files::new();
        let mut manifest = files.manifest(0);
        let ids = (1..size)
            .map(|n| {
                uuid::Uuid::from_u128(((n as u128) << 96) | 1)
                    .hyphenated()
                    .to_string()
            })
            .collect::<Vec<_>>();
        let mut records = vec![(ROOT, NIL_UUID)];
        records.extend(ids.iter().map(|id| (id.as_str(), ROOT)));
        let mut entries = ids
            .iter()
            .map(|id| (id.as_str(), vec![(5, l(vec![Brace::str("B"), a(0)]))]))
            .collect::<Vec<_>>();
        entries.push((NIL_UUID, vec![(23, links(&[]))]));
        for (row, tree) in
            manifest
                .rows
                .iter_mut()
                .zip([registry(&records), sets(&[]), help(&entries)])
        {
            let bytes = serialize_row(&tree);
            let Locator::Plain { path } = &row.locator else {
                unreachable!()
            };
            fs::write(path, &bytes).unwrap();
            row.stored_length = bytes.len() as u64;
            row.packed_length = bytes.len() as u64;
            row.plain_length = bytes.len() as u64;
            row.stored_sha256 = hash(&bytes);
            row.packed_sha256 = hash(&bytes);
            row.plain_sha256 = hash(&bytes);
        }
        let input = ProjectionInputs::load(manifest, SOURCE).unwrap();
        let original = input.project().unwrap().help;
        assert_eq!(original.entries.len(), size);
        let before = cmp::compare_current(&input, None).unwrap();
        assert!(before.current_roundtrip.first.is_none());
        let proposal = comparison_proposal(&input); // Explicit authored declaration order, never native expected order.
        let actual = cmp::compare_current(&input, Some(&proposal))
            .unwrap()
            .proposal
            .unwrap();
        let mut table = order::MsvcTable::new();
        for emit in &proposal.emits {
            table.insert(order::uuid_hash(&emit.key).unwrap());
        }
        let expected_keys = table
            .order()
            .iter()
            .map(|&i| proposal.emits[i].key.clone())
            .collect::<Vec<_>>();
        assert_eq!(actual.model_order, expected_keys);
        assert_eq!(actual.associations.len(), size);
        assert!(actual.payload.property_maps_equal);
        let mut copied = original.clone();
        copied.entries = expected_keys
            .iter()
            .map(|key| {
                original
                    .entries
                    .iter()
                    .find(|e| &e.key == key)
                    .unwrap()
                    .clone()
            })
            .collect();
        assert_eq!(
            cmp::literal(input.raw(RowRole::HelpProps), &copied.render()),
            actual.literal
        );
        assert_eq!(original.render(), input.raw(RowRole::HelpProps));
        assert_eq!(table.buckets(), order::bucket_count(size));
        assert!(actual.untrusted_diagnostic_only);
    }
    let first = [
        "00000001-0000-0000-0000-000000000000",
        "00000002-0000-0000-0000-000000000000",
        "00000009-0000-0000-0000-000000000000",
    ];
    let other = [first[0], first[2], first[1]];
    assert_ne!(first, other);
    assert_eq!(
        order::iteration_order(first).unwrap(),
        order::iteration_order(other).unwrap()
    );
}
#[test]
fn recognized_edge_diagnostics_preserve_domains_occurrences_unknown_and_missing_endpoints() {
    let (files, base, descriptor, index) = facts_fixture();
    let manifest = additional_manifest(&files, &descriptor, Some(&index));
    let loaded = diagnostic::FactInputs::load(&base, manifest).unwrap();
    let original = loaded.project(&base).unwrap();
    let same = loaded.project(&base).unwrap();
    match cmp::edges(Some(&original), Some(&same)) {
        cmp::EdgeComparison::ObservedOnly {
            expected_count,
            actual_count,
            first,
            ..
        } => {
            assert_eq!(expected_count, actual_count);
            assert!(expected_count > 0);
            assert!(first.is_none());
        }
        _ => panic!("actual graph observations lost"),
    }
    assert!(matches!(
        cmp::edges(Some(&original), None),
        cmp::EdgeComparison::Unavailable { .. }
    ));
    for mutation in 0..3 {
        let mut changed = loaded.project(&base).unwrap();
        let at = changed
            .events
            .iter()
            .position(|e| matches!(e, diagnostic::ObservationEvent::ReferenceVisit { .. }))
            .unwrap();
        match mutation {
            0 => {
                let diagnostic::ObservationEvent::ReferenceVisit { target, .. } =
                    &mut changed.events[at]
                else {
                    unreachable!()
                };
                *target = diagnostic::Target::Unknown {
                    raw: l(vec![Brace::str("uncertain"), a(99)]),
                };
            }
            1 => {
                changed.events.remove(at);
            }
            _ => {
                let diagnostic::ObservationEvent::ReferenceVisit { occurrence, .. } =
                    &mut changed.events[at]
                else {
                    unreachable!()
                };
                occurrence.path.push(99);
            }
        }
        match cmp::edges(Some(&original), Some(&changed)) {
            cmp::EdgeComparison::ObservedOnly { first: Some(d), .. } => {
                assert!(d.expected.is_some());
                assert_ne!(d.expected, d.actual);
            }
            _ => panic!("changed endpoint not reported"),
        }
    }
    loaded.verify_sources(&base).unwrap();
}
#[test]
fn closed_corpus_actual_handler_accepts_independent_unchanged_stages_without_proposal() {
    let files = Files::new();
    let output = Files::new();
    let corpus = comparison_corpus(&files, files.manifest(0));
    let report = comparison_run(&files, &corpus, &output).unwrap();
    assert_eq!(report.cases.len(), 1);
    assert!(!report.native_acceptance);
    assert!(report.first_visit_authority.is_none());
    assert_eq!(report.trace_status, "NotIdentified");
    let current = &report.cases[0].current;
    assert_eq!(
        (current.metadata_keys, current.set_keys, current.nil_keys),
        (1, 1, 1)
    );
    assert!(current.comparison.proposal.is_none());
    let compared = report.cases[0].baseline_vs_current.as_ref().unwrap();
    assert!(compared.literal.first.is_none());
    assert!(compared.payload.property_maps_equal);
    assert!(matches!(
        compared.recognized_edges,
        cmp::EdgeComparison::Unavailable { .. }
    ));
    let completion: serde_json::Value =
        serde_json::from_slice(&fs::read(output.root.join("published/completion.json")).unwrap())
            .unwrap();
    assert_eq!(completion["diagnosis_finished"], true);
    assert_eq!(completion["native_acceptance"], false);
    assert!(completion["first_visit_authority"].is_null());
    let bytes = fs::read(output.root.join("published/report.json")).unwrap();
    assert_eq!(completion["report_sha256"], hash(&bytes));
    let saved: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        saved["cases"][0]["current"]["rows"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(
        saved["cases"][0]["current"]["original_proofs"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(saved["cases"][0]["current"]["facts_manifest"].is_null());
    let retry = comparison_run(&files, &corpus, &output);
    assert!(retry.is_err());
    assert_eq!(
        bytes,
        fs::read(output.root.join("published/report.json")).unwrap()
    );
}
#[test]
fn closed_corpus_shared_indexed_pack_members_are_valid_but_ranges_and_bindings_are_not_interchangeable()
 {
    let files = Files::new();
    let mut input = files.manifest(2);
    let mut pack = b"independently-indexed-pack-prefix".to_vec();
    let path = files.root.join("shared-pack.bin");
    for row in &mut input.rows {
        let Locator::GzipRawDeflateRange {
            path: prior,
            offset,
        } = &row.locator
        else {
            unreachable!()
        };
        let stored = fs::read(prior).unwrap();
        let selected = &stored[*offset as usize..*offset as usize + row.stored_length as usize];
        let offset = pack.len() as u64;
        pack.extend_from_slice(selected);
        row.locator = Locator::GzipRawDeflateRange {
            path: path.clone(),
            offset,
        };
    }
    pack.extend_from_slice(b"indexed-unrelated-suffix");
    fs::write(&path, &pack).unwrap();
    let corpus = comparison_corpus(&files, input);
    let output = Files::new();
    let report = comparison_run(&files, &corpus, &output).unwrap();
    assert!(
        report.cases[0]
            .baseline_vs_current
            .as_ref()
            .unwrap()
            .literal
            .first
            .is_none()
    );
    assert_eq!(fs::read(&path).unwrap(), pack);
    let mut bad = corpus.clone();
    let d = &mut bad.cases[0].current;
    let mut rows: ManifestV1 =
        serde_json::from_slice(&fs::read(&d.input_manifest.path).unwrap()).unwrap();
    let Locator::GzipRawDeflateRange { offset, .. } = &mut rows.rows[0].locator else {
        unreachable!()
    };
    *offset += 1; // Partial member cannot impersonate the complete indexed member.
    d.input_manifest = comparison_json(&files, "input-current.json", &rows);
    // Rebind the capture to the deliberate range mutant so the actual member loader is tested.
    let mut witness: cmp::CaptureWitnessV1 =
        serde_json::from_slice(&fs::read(&d.capture_witness.path).unwrap()).unwrap();
    witness.input_manifest_sha256 = d.input_manifest.sha256.clone();
    d.capture_witness = comparison_json(&files, "capture-current.json", &witness);
    let fresh = Files::new();
    assert!(comparison_run(&files, &bad, &fresh).is_err());
    assert!(!fresh.root.join("published").exists());
}
#[test]
fn closed_corpus_wrong_sources_stage_witness_alias_and_existing_output_fail_before_publication() {
    for mutation in 0..9 {
        let files = Files::new();
        let good_output = Files::new();
        let corpus = comparison_corpus(&files, files.manifest(0));
        comparison_run(&files, &corpus, &good_output).unwrap(); // SAME genuine handler input before mutation.
        let mut bad = corpus.clone();
        match mutation {
            0 => bad.compiled_source.comparison = "0".repeat(64),
            1 => bad.input_exporter_source_head = "0".repeat(40),
            2 => bad.cases[0].current.case = "wrong-case".into(),
            3 => bad.cases[0].current.stage = "wrong-stage".into(),
            4 => bad.cases[0].current.input_manifest.sha256 = "0".repeat(64),
            5 => bad.cases.push(bad.cases[0].clone()),
            6 => bad.cases[0].baseline = Some(bad.cases[0].current.clone()),
            7 => bad.cases[0].current.capture_witness = bad.cases[0].current.input_manifest.clone(),
            _ => bad.cases[0].current.registry_root_uuid = CHILD.into(),
        }
        let output = Files::new();
        assert!(
            comparison_run(&files, &bad, &output).is_err(),
            "mutation {mutation}"
        );
        assert!(!output.root.join("published").exists());
    }
    let files = Files::new();
    let corpus = comparison_corpus(&files, files.manifest(0));
    let outer = comparison_json(&files, "outer.json", &corpus);
    let mut unknown = serde_json::to_value(&corpus).unwrap();
    unknown
        .as_object_mut()
        .unwrap()
        .insert("proposal".into(), serde_json::json!([]));
    let rejected = Files::new();
    assert!(
        cmp::run_closed(
            comparison_json(&files, "unknown-outer.json", &unknown),
            &rejected.root.join("published")
        )
        .is_err()
    );
    assert!(!rejected.root.join("published").exists());
    let mut wrong_digest = outer.clone();
    wrong_digest.sha256 = "0".repeat(64);
    assert!(cmp::run_closed(wrong_digest, &rejected.root.join("published")).is_err());
    assert!(cmp::run_closed(outer.clone(), &files.root.join("nested-output")).is_err());
    assert!(!files.root.join("nested-output").exists());
    let output = Files::new();
    fs::create_dir(output.root.join("published")).unwrap();
    fs::write(
        output.root.join("published/keep.bin"),
        b"all prior bytes preserved",
    )
    .unwrap();
    assert!(cmp::run_closed(outer, &output.root.join("published")).is_err());
    assert_eq!(
        fs::read(output.root.join("published/keep.bin")).unwrap(),
        b"all prior bytes preserved"
    );
}
#[test]
fn retained_env_request_has_no_missing_manifest_skip_or_non_f_fallback() {
    use std::ffi::OsString;
    assert!(cmp::EnvRequest::parse(None, None, None).is_err());
    let manifest = Some(OsString::from("F:/explicit/input.json"));
    let digest = Some(OsString::from("1".repeat(64)));
    let output = Some(OsString::from("F:/explicit/new-output"));
    #[cfg(windows)]
    assert!(cmp::EnvRequest::parse(manifest.clone(), digest.clone(), output.clone()).is_ok());
    assert!(cmp::EnvRequest::parse(manifest.clone(), None, output.clone()).is_err());
    assert!(cmp::EnvRequest::parse(manifest.clone(), digest.clone(), None).is_err());
    assert!(
        cmp::EnvRequest::parse(manifest, Some(OsString::from("unknown")), output.clone()).is_err()
    );
    assert!(
        cmp::EnvRequest::parse(Some(OsString::from("C:/wrong/input.json")), digest, output)
            .is_err()
    );
}
#[test]
#[ignore = "requires independently ROOT-frozen complete same-stage manifest, SHA and NEW F output; missing inputs fail"]
fn retained_closed_corpus_projection() {
    let report = cmp::run_from_env()
        .expect("closed retained diagnostic admission/execution failed; no fallback");
    assert!(!report.native_acceptance);
    assert!(report.original_issue_open);
    assert_eq!(report.trace_status, "NotIdentified");
    assert_eq!(report.graph_completeness, "Partial");
    assert!(report.first_visit_authority.is_none());
    assert!(
        report
            .cases
            .iter()
            .all(|c| c.current.comparison.proposal.is_none())
    );
}
#[test]
fn closed_corpus_optional_facts_use_actual_same_base_handler_and_report_full_bindings() {
    let (files, base, descriptor, index) = facts_fixture();
    let raw = [RowRole::Registry, RowRole::TypeSets, RowRole::HelpProps]
        .map(|role| base.raw(role).to_vec());
    let mut input = files.manifest(0);
    for (row, bytes) in input.rows.iter_mut().zip(raw) {
        let Locator::Plain { path } = &row.locator else {
            unreachable!()
        };
        fs::write(path, &bytes).unwrap();
        row.stored_length = bytes.len() as u64;
        row.packed_length = bytes.len() as u64;
        row.plain_length = bytes.len() as u64;
        row.stored_sha256 = hash(&bytes);
        row.packed_sha256 = hash(&bytes);
        row.plain_sha256 = hash(&bytes);
    }
    let mut corpus = comparison_corpus(&files, input);
    let facts = additional_manifest(&files, &descriptor, Some(&index));
    corpus.cases[0].current.facts_manifest =
        Some(comparison_json(&files, "facts-current.json", &facts));
    let output = Files::new();
    let report = comparison_run(&files, &corpus, &output).unwrap();
    let current = &report.cases[0].current;
    assert_eq!(current.fact_rows.len(), 2);
    assert!(current.facts_manifest.is_some());
    assert_eq!(current.covered_keys.len(), 4);
    assert!(!current.observed_edges.is_empty());
    assert!(
        current
            .partial_observations
            .iter()
            .all(|p| !p.reason.is_empty())
    );
    assert!(current.comparison.proposal.is_none());
    assert!(!report.native_acceptance);
    let mut bad = corpus.clone();
    bad.cases[0].current.facts_manifest.as_mut().unwrap().sha256 = "0".repeat(64);
    let failed = Files::new();
    assert!(comparison_run(&files, &bad, &failed).is_err());
    assert!(!failed.root.join("published").exists());
}
