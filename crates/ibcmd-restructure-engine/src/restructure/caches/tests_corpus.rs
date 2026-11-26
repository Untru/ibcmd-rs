//! Corpus cases using the shared native-snapshot helpers.

pub(crate) use crate::restructure::test_corpus_support::*;
use ibcmd_restructure::brace::parse_row;

// ---------------------------------------------------------------------------------------------
// 2203278d: the model against the root sections and the tabular-section section
// ---------------------------------------------------------------------------------------------

use crate::restructure::caches::order::iteration_order;
use crate::restructure::caches::root::collections;
use crate::restructure::caches::type_index::TypeIndex;

#[test]
fn the_type_index_round_trips_byte_for_byte() {
    for name in ["pristine", "c2", "m", "t1_before"] {
        let snap = lab_snap!(name);
        let text = snap.params(row_name("2203278d")).unwrap();
        let index = TypeIndex::parse(&text).unwrap();
        assert_eq!(index.sections.len(), 29);
        assert_eq!(index.render(), text, "{name}");
    }
}

#[test]
fn a_root_section_is_the_hash_order_of_the_roots_collection() {
    for name in ["pristine", "c2", "m"] {
        let snap = lab_snap!(name);
        let root = parse_row(&snap.config(ROOT_ROW).unwrap()).unwrap();
        let collections = collections(&root);
        let index = TypeIndex::parse(&snap.params(row_name("2203278d")).unwrap()).unwrap();
        let mut checked = 0;
        for section in &index.sections {
            let Some(collection) = collections.iter().find(|c| c.class == section.class) else {
                continue;
            };
            let expected = iteration_order(collection.objects.iter().map(String::as_str)).unwrap();
            let dump: Vec<&str> = section.entries.iter().map(|e| e.object.as_str()).collect();
            assert_eq!(dump, expected, "{name}: section {}", section.class);
            checked += 1;
        }
        assert!(checked >= 20, "{name}: {checked} sections checked");
    }
}
