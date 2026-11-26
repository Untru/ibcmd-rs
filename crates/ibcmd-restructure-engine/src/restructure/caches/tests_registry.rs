//! The object registry (`1a621f0f`) and the attribute lists of the descriptors, against the БСП
//! corpus and the native cases. Every test skips itself without the lab.

use crate::metadata_model::brace::parse_row;
use crate::mssql_config_apply::si;
use crate::restructure::caches::facts::ObjectFacts;
use crate::restructure::caches::members::Members;
use crate::restructure::caches::registry::{object_records, subtree_text};
use crate::restructure::caches::root::{class_of_kind, collection_of, collections};
use crate::restructure::caches::tests_corpus::{ROOT_ROW, lab_snap, row_name};

#[test]
fn the_registry_lists_the_objects_in_the_roots_order_and_their_own_records_follow() {
    for name in ["pristine", "c2", "m", "t1_before"] {
        let snap = lab_snap!(name);
        let registry = snap.params(row_name("1a621f0f")).unwrap();
        let main = si::parse(&registry).unwrap();
        let root = parse_row(&snap.config(ROOT_ROW).unwrap()).unwrap();
        let all = collections(&root);
        let owner = main.records[0].uuid.clone();
        let eol = main.eol;
        let (mut checked, mut with_others, mut records) = (0, 0, 0);
        for kind in ["Catalog", "Document"] {
            let class = class_of_kind(kind).unwrap();
            let listed = collection_of(&all, class).unwrap();
            let kind_index = main.kind_of_class(class).unwrap();
            let registered: Vec<&str> = main
                .records
                .iter()
                .filter(|record| record.kind == kind_index && record.parent == owner)
                .map(|record| record.uuid.as_str())
                .collect();
            assert_eq!(
                registered,
                listed
                    .objects
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
                "{name}: the {kind} records are not in the root's order"
            );
            for uuid in &listed.objects {
                let row = parse_row(&snap.config(uuid).unwrap()).unwrap();
                let facts = ObjectFacts::parse(kind, &row).unwrap();
                let members = Members::parse(kind, &row).unwrap();
                let made = object_records(&main, kind, &facts, &members, &owner).unwrap();
                let index = main.index_of(uuid).unwrap();
                // the records of the subtree the descriptor explains (a document lists its forms
                // between the attributes and the sections)
                let ours: std::collections::BTreeSet<&str> =
                    made.iter().map(|record| record.uuid.as_str()).collect();
                let stored = (index..main.subtree_end(index))
                    .filter(|i| ours.contains(main.records[*i].uuid.as_str()))
                    .map(|i| {
                        String::from_utf8_lossy(
                            &registry[main.records[i].start..main.records[i].end],
                        )
                        .into_owned()
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                let text = made
                    .iter()
                    .map(|record| record.render(eol))
                    .collect::<Vec<_>>()
                    .join(",");
                if stored != text {
                    let at = stored
                        .bytes()
                        .zip(text.bytes())
                        .position(|(a, b)| a != b)
                        .unwrap_or(stored.len().min(text.len()));
                    let boundary = |s: &str, mut i: usize| {
                        i = i.min(s.len());
                        while !s.is_char_boundary(i) {
                            i -= 1;
                        }
                        i
                    };
                    let from = boundary(&stored, at.saturating_sub(120));
                    let from = boundary(&text, from);
                    panic!(
                        "{name}: {kind} {} differs from the descriptor's records\nstored: {}\nmade:   {}",
                        facts.name,
                        &stored[from..boundary(&stored, at + 200)],
                        &text[from..boundary(&text, at + 200)]
                    );
                }
                if !members.others.is_empty() {
                    with_others += 1;
                    assert!(main.subtree_end(index) > index + made.len());
                } else {
                    assert_eq!(
                        subtree_text(&registry, &main, index),
                        text,
                        "{name}: {kind} {} has records the descriptor does not explain",
                        facts.name
                    );
                }
                checked += 1;
                records += made.len();
            }
        }
        eprintln!(
            "{name}: {checked} objects, {records} records equal, {with_others} objects with forms or the like after them"
        );
        assert!(checked >= 139);
    }
}
