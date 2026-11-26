//! The cache rows against the native ones for the cases of the `ddl` track: c (a new catalog),
//! h (a new tabular section) and the types case (attributes only). See `tests_corpus.rs` for the
//! snapshots; every test skips itself without the lab.

use std::collections::{BTreeMap, BTreeSet};

use crate::metadata_model::brace::{Brace, parse_row, serialize_row};
use crate::restructure::caches::facts::{ObjectFacts, tabular_class};
use crate::restructure::caches::help_props::HelpProps;
use crate::restructure::caches::names_tables::{NamesTables, NewTable};
use crate::restructure::caches::order::iteration_order;
use crate::restructure::caches::owner_map::OwnerMap;
use crate::restructure::caches::plan::{
    catalog_shape, document_shape, reference_type_slots, section_traversal,
};
use crate::restructure::caches::root::{
    CATALOG_CLASS, DOCUMENT_CLASS, collection_of, collections, kind_of_class,
};
use crate::restructure::caches::synonyms::{PROPERTY_SECTIONS, Synonyms, insertion_order};
use crate::restructure::caches::tests_corpus::{ROOT_ROW, Snap, lab_snap, row_name};
use crate::restructure::caches::type_index::TypeIndex;
use crate::restructure::caches::type_sets::{FAMILIES, TypeSets, member_id};
use crate::restructure::caches::xdto_types::{
    XdtoModel, document_standard_properties, standard_properties,
};

fn root_of(snap: &Snap) -> Brace {
    parse_row(&snap.config(ROOT_ROW).unwrap()).unwrap()
}

// ---------------------------------------------------------------------------------------------
// every row through its decoder
// ---------------------------------------------------------------------------------------------

#[test]
fn every_cache_row_round_trips_through_its_decoder() {
    for name in ["pristine", "c2", "m", "t1_before"] {
        let snap = lab_snap!(name);
        let text = |short: &str| snap.params(row_name(short)).unwrap();
        assert!(
            NamesTables::parse(&text("a07b62f0")).unwrap().render() == text("a07b62f0"),
            "{name} a07b62f0"
        );
        assert!(
            OwnerMap::parse(&text("42ed49cc")).unwrap().render() == text("42ed49cc"),
            "{name} 42ed49cc"
        );
        assert!(
            Synonyms::parse(&text("facbfffe")).unwrap().render() == text("facbfffe"),
            "{name} facbfffe"
        );
        assert!(
            TypeSets::parse(&text("fe8acd6a")).unwrap().render() == text("fe8acd6a"),
            "{name} fe8acd6a"
        );
        assert!(
            HelpProps::parse(&text("c4629235")).unwrap().render() == text("c4629235"),
            "{name} c4629235"
        );
        assert!(
            XdtoModel::parse(&text("ea13a2c9")).unwrap().render() == text("ea13a2c9"),
            "{name} ea13a2c9"
        );
        // the rest are plain brace text in the platform's layout
        for short in [
            "0b698dcd", "1a621f0f", "215d232c", "59274b8d", "c40aafd6", "c77bc206", "cf8b5e0f",
            "e05c0074", "fd1b2a86",
        ] {
            let bytes = text(short);
            assert!(
                serialize_row(&parse_row(&bytes).unwrap()) == bytes,
                "{name} {short}"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// the type index
// ---------------------------------------------------------------------------------------------

fn traversal(snap: &Snap, root: &Brace, class: &str) -> Vec<String> {
    let collections = collections(root);
    section_traversal(&collections, class, &|uuid| snap.descriptor(uuid)).unwrap()
}

#[test]
fn the_tabular_section_section_is_the_hash_order_of_the_traversal() {
    for name in ["pristine", "m"] {
        let snap = lab_snap!(name);
        let root = root_of(&snap);
        let index = TypeIndex::parse(&snap.params(row_name("2203278d")).unwrap()).unwrap();
        for (kind, class) in [("Catalog", CATALOG_CLASS), ("Document", DOCUMENT_CLASS)] {
            let section = index.section(tabular_class(kind).unwrap()).unwrap();
            let dump: Vec<&str> = section.entries.iter().map(|e| e.object.as_str()).collect();
            let order = traversal(&snap, &root, class);
            assert_eq!(order.len(), dump.len(), "{name} {kind}");
            let expected = iteration_order(order.iter().map(String::as_str)).unwrap();
            assert_eq!(dump, expected, "{name} {kind}");
        }
    }
}

#[test]
fn the_generated_types_of_an_entry_are_the_ones_of_its_descriptor() {
    let snap = lab_snap!("c2");
    let index = TypeIndex::parse(&snap.params(row_name("2203278d")).unwrap()).unwrap();
    let mut checked = 0;
    for section in &index.sections {
        let Some(kind) = kind_of_class(&section.class) else {
            continue;
        };
        if !["Catalog", "Document", "ExchangePlan"].contains(&kind) {
            continue;
        }
        for entry in &section.entries {
            let facts = ObjectFacts::parse(kind, &snap.descriptor(&entry.object).unwrap()).unwrap();
            assert_eq!(
                reference_type_slots(&facts).unwrap(),
                entry.types,
                "{kind} {}",
                facts.name
            );
            checked += 1;
        }
    }
    assert!(checked > 140, "{checked}");
    // the tabular sections: their own type and the type of their row
    for (kind, class) in [("Catalog", CATALOG_CLASS), ("Document", DOCUMENT_CLASS)] {
        let sections = index.section(tabular_class(kind).unwrap()).unwrap();
        let mut from_descriptors = BTreeMap::new();
        for object in &collection_of(&collections(&root_of(&snap)), class)
            .unwrap()
            .objects
        {
            let facts = ObjectFacts::parse(kind, &snap.descriptor(object).unwrap()).unwrap();
            for section in facts.sections {
                from_descriptors.insert(section.uuid.clone(), section.types.clone());
            }
        }
        assert_eq!(from_descriptors.len(), sections.entries.len(), "{kind}");
        for entry in &sections.entries {
            assert_eq!(
                entry.types.iter().map(|t| t.index).collect::<Vec<_>>(),
                [0, 1]
            );
            let [(t0, v0), (t1, v1)] = &from_descriptors[&entry.object];
            assert_eq!(
                (
                    &entry.types[0].type_id,
                    &entry.types[0].value_id,
                    &entry.types[1].type_id,
                    &entry.types[1].value_id
                ),
                (t0, v0, t1, v1)
            );
        }
    }
}

#[test]
fn an_object_removed_from_every_row_comes_back_where_the_platform_had_it() {
    let snap = lab_snap!("pristine");
    let root = root_of(&snap);
    let collections = collections(&root);

    let index = TypeIndex::parse(&snap.params(row_name("2203278d")).unwrap()).unwrap();
    let names = NamesTables::parse(&snap.params(row_name("a07b62f0")).unwrap()).unwrap();
    let synonyms = Synonyms::parse(&snap.params(row_name("facbfffe")).unwrap()).unwrap();
    let mut readded = 0;
    for (kind, class) in [("Catalog", CATALOG_CLASS), ("Document", DOCUMENT_CLASS)] {
        let catalogs = collection_of(&collections, class).unwrap().clone();
        for (position, object) in catalogs.objects.iter().enumerate() {
            let facts = ObjectFacts::parse(kind, &snap.descriptor(object).unwrap()).unwrap();

            // 2203278d
            let mut again = index.clone();
            let section = again.section_mut(class).unwrap();
            let at = section
                .entries
                .iter()
                .position(|e| e.object == *object)
                .unwrap();
            let removed = section.entries.remove(at);
            again
                .refill_section(class, &catalogs.objects, vec![removed])
                .unwrap();
            assert_eq!(again, index, "2203278d, {}", facts.name);

            // a07b62f0
            let mut again = names.clone();
            let at = again
                .entries
                .iter()
                .position(|e| e.object == *object)
                .unwrap();
            let removed = again.entries.remove(at);
            let table = removed.table.clone().unwrap();
            let predecessor = position
                .checked_sub(1)
                .map(|i| catalogs.objects[i].as_str());
            let successor = catalogs.objects.get(position + 1).map(String::as_str);
            again
                .add_object(&NewTable {
                    kind,
                    name: &facts.name,
                    object,
                    table_number: table.number,
                    ref_type_id: &table.type_id,
                    predecessor,
                    successor,
                })
                .unwrap();
            assert_eq!(again, names, "a07b62f0, {}", facts.name);

            // facbfffe: every property the catalog has
            for property in [
                "ListPresentation",
                "ExtendedListPresentation",
                "ObjectPresentation",
                "ExtendedObjectPresentation",
            ] {
                let pairs = facts.localized(property).unwrap();
                if pairs.is_empty() {
                    continue;
                }
                let mut again = synonyms.clone();
                let &(_, section_index) = PROPERTY_SECTIONS
                    .iter()
                    .find(|(name, _)| *name == property)
                    .unwrap();
                let section = &mut again.sections[section_index];
                let at = section
                    .entries
                    .iter()
                    .position(|(key, _)| key == object)
                    .unwrap();
                section.entries.remove(at);
                again
                    .add_object(property, object, &pairs, &collections, &kind_of_class)
                    .unwrap();
                assert_eq!(again, synonyms, "facbfffe {property}, {}", facts.name);
                readded += 1;
            }
        }
    }
    assert!(readded > 100, "{readded}");
}

#[test]
fn the_sections_of_facbfffe_are_filled_kind_by_kind_in_the_roots_order() {
    let mut exact = Vec::new();
    for name in ["pristine", "c2"] {
        let snap = lab_snap!(name);
        let root = root_of(&snap);
        let collections = collections(&root);
        let synonyms = Synonyms::parse(&snap.params(row_name("facbfffe")).unwrap()).unwrap();
        for (index, section) in synonyms.sections.iter().enumerate() {
            let keys: BTreeSet<String> = section.entries.iter().map(|(k, _)| k.clone()).collect();
            let Ok(insertion) = insertion_order(&keys, &collections, &kind_of_class) else {
                continue; // keys the root does not list (sections 0 and 7)
            };
            let order = iteration_order(insertion.iter().copied()).unwrap();
            let dump: Vec<&str> = section.entries.iter().map(|(k, _)| k.as_str()).collect();
            if order == dump {
                exact.push((name, index));
            } else {
                eprintln!(
                    "facbfffe {name} section {index} ({} entries) is not reproduced",
                    dump.len()
                );
            }
        }
    }
    eprintln!("facbfffe exact: {exact:?}");
    for section in [2, 3, 4, 5, 6] {
        assert!(exact.contains(&("pristine", section)), "section {section}");
        assert!(exact.contains(&("c2", section)), "section {section}");
    }
}

#[test]
fn the_families_of_the_type_sets_are_the_sets_of_the_corpus() {
    for name in ["pristine", "c2"] {
        let snap = lab_snap!(name);
        let index = TypeIndex::parse(&snap.params(row_name("2203278d")).unwrap()).unwrap();
        let sets = TypeSets::parse(&snap.params(row_name("fe8acd6a")).unwrap()).unwrap();
        for (set, families) in FAMILIES {
            let mut expected = BTreeSet::new();
            for section in &index.sections {
                let Some(kind) = kind_of_class(&section.class) else {
                    continue;
                };
                for (family_kind, family_index) in *families {
                    if *family_kind != kind {
                        continue;
                    }
                    for entry in &section.entries {
                        for slot in &entry.types {
                            if slot.index == *family_index {
                                expected.insert(slot.type_id.clone());
                            }
                        }
                    }
                }
            }
            let members = &sets.sets.iter().find(|(key, _)| key == set).unwrap().1;
            let ids: Vec<String> = members
                .iter()
                .map(|member| member_id(member).unwrap().to_owned())
                .collect();
            assert_eq!(
                ids.iter().cloned().collect::<BTreeSet<_>>(),
                expected,
                "{name}: set {set}"
            );
            assert!(
                ids.windows(2).all(|pair| pair[0] < pair[1]),
                "{name}: set {set} is sorted"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// c4629235 and the XDTO standard properties, all catalogs
// ---------------------------------------------------------------------------------------------

#[test]
fn a_catalogs_entry_of_c4629235_is_made_of_its_descriptor() {
    let snap = lab_snap!("c2");
    let root = root_of(&snap);
    let help = HelpProps::parse(&snap.params(row_name("c4629235")).unwrap()).unwrap();
    let by_key: BTreeMap<&str, _> = help
        .entries
        .iter()
        .map(|entry| (entry.key.as_str(), entry))
        .collect();
    let catalogs = collection_of(&collections(&root), CATALOG_CLASS)
        .unwrap()
        .clone();
    for object in &catalogs.objects {
        let facts = ObjectFacts::parse("Catalog", &snap.descriptor(object).unwrap()).unwrap();
        let has_help = snap.has_config(&format!("{object}.1"));
        let expected = HelpProps::catalog_entry(&facts, has_help).unwrap();
        let native = by_key[object.as_str()];
        // property 24 lists the type sets that mention the object: not made of the descriptor
        let without_24: Vec<_> = native
            .props
            .iter()
            .filter(|(id, _)| id != "24")
            .cloned()
            .collect();
        assert_eq!(expected.props, without_24, "{}", facts.name);
    }
}

#[test]
fn a_documents_entry_of_c4629235_is_made_of_its_descriptor() {
    let snap = lab_snap!("c2");
    let root = root_of(&snap);
    let help = HelpProps::parse(&snap.params(row_name("c4629235")).unwrap()).unwrap();
    let by_key: BTreeMap<&str, _> = help
        .entries
        .iter()
        .map(|entry| (entry.key.as_str(), entry))
        .collect();
    let documents = collection_of(&collections(&root), DOCUMENT_CLASS)
        .unwrap()
        .clone();
    assert_eq!(documents.objects.len(), 25);
    for object in &documents.objects {
        let facts = ObjectFacts::parse("Document", &snap.descriptor(object).unwrap()).unwrap();
        let has_help = snap.has_config(&format!("{object}.1"));
        let expected = HelpProps::document_entry(&facts, has_help).unwrap();
        let native = by_key[object.as_str()];
        let without_24: Vec<_> = native
            .props
            .iter()
            .filter(|(id, _)| id != "24")
            .cloned()
            .collect();
        assert_eq!(expected.props, without_24, "{}", facts.name);
    }
}

#[test]
fn the_standard_xdto_properties_follow_the_shape_of_the_document() {
    let snap = lab_snap!("pristine");
    let root = root_of(&snap);
    let model = XdtoModel::parse(&snap.params(row_name("ea13a2c9")).unwrap()).unwrap();
    let documents = collection_of(&collections(&root), DOCUMENT_CLASS)
        .unwrap()
        .clone();
    for object in &documents.objects {
        let facts = ObjectFacts::parse("Document", &snap.descriptor(object).unwrap()).unwrap();
        let open = format!(
            "\t\t<objectType name=\"DocumentObject.{}\">\r\n",
            facts.name
        );
        let at = model.xml.find(&open).unwrap() + open.len();
        let expected: String =
            document_standard_properties(&facts.name, document_shape(&facts).unwrap()).concat();
        assert!(model.xml[at..].starts_with(&expected), "{}", facts.name);
    }
}

#[test]
fn the_standard_xdto_properties_follow_the_shape_of_the_catalog() {
    let snap = lab_snap!("pristine");
    let root = root_of(&snap);
    let model = XdtoModel::parse(&snap.params(row_name("ea13a2c9")).unwrap()).unwrap();
    let catalogs = collection_of(&collections(&root), CATALOG_CLASS)
        .unwrap()
        .clone();
    for object in &catalogs.objects {
        let facts = ObjectFacts::parse("Catalog", &snap.descriptor(object).unwrap()).unwrap();
        let open = format!("\t\t<objectType name=\"CatalogObject.{}\">\r\n", facts.name);
        let at = model.xml.find(&open).unwrap() + open.len();
        let expected: String =
            standard_properties(&facts.name, catalog_shape(&facts).unwrap()).concat();
        assert!(model.xml[at..].starts_with(&expected), "{}", facts.name);
    }
}
