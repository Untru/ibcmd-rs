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
    for size in [9, 65, 513, 1025, 2049, 2377] {
        let ids: Vec<_> = (1..=size)
            .map(|n| uuid::Uuid::from_u128(n).hyphenated().to_string())
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
