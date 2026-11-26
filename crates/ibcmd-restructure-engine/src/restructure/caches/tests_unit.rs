//! Small synthetic checks of the cache decoders and writers (no lab needed).

use crate::metadata_model::brace::Brace;
use crate::restructure::caches::help_props::{Entry as HelpEntry, HelpProps};
use crate::restructure::caches::names_tables::{
    Entry as NameEntry, NamesTables, NewTable, TableRef,
};
use crate::restructure::caches::owner_map::OwnerMap;
use crate::restructure::caches::root::{CATALOG_CLASS, Collection, kind_of_class};
use crate::restructure::caches::synonyms::{Section as SynonymSection, Synonyms};
use crate::restructure::caches::type_index::{Entry, Section, TypeIndex, TypeSlot};
use crate::restructure::caches::type_sets::TypeSets;
use crate::restructure::caches::xdto_types::{
    CatalogShape, DocumentShape, Family, XdtoModel, primitive_property, section_property_line,
    section_row_block,
};

/// `0000000n-0000-0000-0000-000000000000`: `Data1` is `n`, the bucket of a small table.
fn id(n: u32) -> String {
    format!("{n:08x}-0000-0000-0000-000000000000")
}

fn slot(type_id: u32, value_id: u32, index: u32) -> TypeSlot {
    TypeSlot {
        type_id: id(type_id),
        value_id: id(value_id),
        index,
    }
}

#[test]
fn a_type_index_round_trips_and_a_section_is_refilled_in_hash_order() {
    let index = TypeIndex {
        sections: vec![Section {
            class: id(0xa0),
            entries: vec![Entry {
                object: id(1),
                types: vec![slot(0x100, 0x200, 0), slot(0x101, 0x201, 4)],
            }],
        }],
    };
    let text = index.render();
    assert!(text.starts_with(&[0xEF, 0xBB, 0xBF]));
    assert_eq!(TypeIndex::parse(&text).unwrap(), index);

    let mut again = index.clone();
    let added = Entry {
        object: id(9),
        types: vec![slot(0x102, 0x202, 0)],
    };
    // 1, 9 (the same bucket of 8: it goes to the front of the run), 2
    again
        .refill_section(&id(0xa0), &[id(1), id(9), id(2)], vec![added.clone()])
        .unwrap_err(); // 2 has no entry
    let mut again = index.clone();
    let second = Entry {
        object: id(2),
        types: vec![slot(0x103, 0x203, 0)],
    };
    again.section_mut(&id(0xa0)).unwrap().entries.push(second);
    again
        .refill_section(&id(0xa0), &[id(1), id(9), id(2)], vec![added])
        .unwrap();
    let order: Vec<String> = again
        .section(&id(0xa0))
        .unwrap()
        .entries
        .iter()
        .map(|e| e.object.clone())
        .collect();
    assert_eq!(order, [id(9), id(1), id(2)]);
    // a key that is there twice is refused
    let mut twice = again.clone();
    let clash = twice.section(&id(0xa0)).unwrap().entries[0].clone();
    assert!(
        twice
            .refill_section(&id(0xa0), &[id(1), id(9), id(2)], vec![clash])
            .is_err()
    );
}

fn table(name: &str, object: u32, number: u64) -> NameEntry {
    NameEntry {
        name: format!("Catalog.{name}"),
        russian_name: format!("Справочник.{name}"),
        object: id(object),
        a: 1,
        b: 0,
        table: Some(TableRef {
            sql: format!("Reference{number}"),
            number,
            type_id: id(0x300 + object),
        }),
    }
}

#[test]
fn a_new_table_entry_goes_after_the_predecessor() {
    let mut names = NamesTables {
        entries: vec![table("A", 1, 10), table("C", 3, 12)],
        rest: vec![Brace::List(vec![Brace::num(0)])],
    };
    assert_eq!(NamesTables::parse(&names.render()).unwrap(), names);
    let new = NewTable {
        kind: "Catalog",
        name: "B",
        object: &id(2),
        table_number: 11,
        ref_type_id: &id(0x302),
        predecessor: Some(&id(1)),
        successor: Some(&id(3)),
    };
    names.add_object(&new).unwrap();
    let listed: Vec<&str> = names.entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(listed, ["Catalog.A", "Catalog.B", "Catalog.C"]);
    assert_eq!(names.entries[1], table("B", 2, 11));
    // the second list stays, the same object twice and an unknown kind are refused
    assert_eq!(names.rest.len(), 1);
    assert!(names.add_object(&new).is_err());
    let unknown = NewTable {
        kind: "Constant",
        ..new
    };
    assert!(names.add_object(&unknown).is_err());
    // without a neighbour there is no place
    let alone = NewTable {
        object: &id(4),
        predecessor: None,
        successor: None,
        ..new
    };
    assert!(names.add_object(&alone).is_err());
}

fn members(ids: &[u32]) -> Vec<Brace> {
    ids.iter()
        .map(|n| Brace::List(vec![Brace::str("#"), Brace::atom(id(*n))]))
        .collect()
}

#[test]
fn the_type_ids_of_a_new_catalog_join_the_sorted_sets() {
    let sets_of = |unsorted: bool| TypeSets {
        sets: vec![
            (
                "e61ef7b8-f3e1-4f4b-8ac7-676e90524997".to_owned(),
                members(&[0x10, 0x30]),
            ),
            (
                "e2cb8e3e-31d7-4ebb-8cd2-3187c9586dce".to_owned(),
                members(if unsorted {
                    &[0x30, 0x10]
                } else {
                    &[0x10, 0x30]
                }),
            ),
            (
                "280f5f0e-9c8a-49cc-bf6d-4d296cc17a63".to_owned(),
                members(&[0x05, 0x40]),
            ),
        ],
    };
    let mut sets = sets_of(false);
    assert_eq!(TypeSets::parse(&sets.render()).unwrap(), sets);
    // Object 0x20 (index 0) and Ref 0x21 (index 1)
    sets.add_object("Catalog", &[(0, id(0x20)), (1, id(0x21))])
        .unwrap();
    assert_eq!(sets.sets[0].1, members(&[0x10, 0x21, 0x30]));
    assert_eq!(sets.sets[1].1, members(&[0x10, 0x20, 0x30]));
    assert_eq!(sets.sets[2].1, members(&[0x05, 0x21, 0x40]));
    assert!(
        sets.add_object("Catalog", &[(1, id(0x21))]).is_err(),
        "a type id twice"
    );
    assert!(
        sets_of(true)
            .add_object("Catalog", &[(0, id(0x20))])
            .is_err(),
        "an unsorted set"
    );
}

#[test]
fn a_catalog_joins_the_owner_map_in_hash_order() {
    let flat = |keys: &[(u32, &[u32])]| {
        let mut section = vec![Brace::num(keys.len() as i64)];
        for (key, owners) in keys {
            section.push(Brace::atom(id(*key)));
            section.push(Brace::num(owners.len() as i64));
            section.extend(owners.iter().map(|owner| Brace::atom(id(*owner))));
        }
        Brace::List(section)
    };
    let row = Brace::List(vec![
        Brace::atom("0"),
        flat(&[(0x71, &[])]),
        flat(&[(1, &[]), (2, &[1])]),
    ]);
    let text = crate::metadata_model::brace::serialize_row(&row);
    let mut map = OwnerMap::parse(&text).unwrap();
    assert_eq!(map.render(), text);
    // catalogs in the root's order: 1, the new 5, 2 -- three buckets, in insertion order
    map.add_catalog(&[id(1), id(5), id(2)], &id(5), &[])
        .unwrap();
    let expected = Brace::List(vec![
        Brace::atom("0"),
        flat(&[(0x71, &[])]),
        flat(&[(1, &[]), (5, &[]), (2, &[1])]),
    ]);
    assert_eq!(
        map.render(),
        crate::metadata_model::brace::serialize_row(&expected)
    );
    assert!(
        map.add_catalog(&[id(7), id(8)], &id(8), &[]).is_err(),
        "no section has the keys of these catalogs"
    );
}

#[test]
fn a_presentation_joins_its_section_and_explanation_is_refused() {
    let collections = vec![Collection {
        class: CATALOG_CLASS.to_owned(),
        objects: vec![id(1), id(5), id(2)],
    }];
    let pairs = vec![("ru".to_owned(), "Демо".to_owned())];
    let mut synonyms = Synonyms {
        sections: (0..8)
            .map(|index| SynonymSection {
                entries: if index == 3 {
                    vec![
                        (
                            id(1),
                            Synonyms::value(&[("ru".to_owned(), "Один".to_owned())]),
                        ),
                        (
                            id(2),
                            Synonyms::value(&[("ru".to_owned(), "Два".to_owned())]),
                        ),
                    ]
                } else {
                    Vec::new()
                },
            })
            .collect(),
    };
    // the section 3 of the fixture has two entries: the new one goes to the hash order of three
    synonyms
        .add_object(
            "ObjectPresentation",
            &id(5),
            &pairs,
            &collections,
            &kind_of_class,
        )
        .unwrap();
    let keys: Vec<&str> = synonyms.sections[3]
        .entries
        .iter()
        .map(|(k, _)| k.as_str())
        .collect();
    assert_eq!(
        keys,
        [id(1), id(5), id(2)].map(|k| Box::leak(k.into_boxed_str()) as &str)
    );
    assert_eq!(synonyms.sections[3].entries[1].1, Synonyms::value(&pairs));
    assert_eq!(Synonyms::parse(&synonyms.render()).unwrap(), synonyms);
    assert!(
        synonyms
            .add_object("Explanation", &id(5), &pairs, &collections, &kind_of_class)
            .is_err()
    );
    assert!(
        synonyms
            .add_object(
                "ObjectPresentation",
                &id(5),
                &pairs,
                &collections,
                &kind_of_class
            )
            .is_err(),
        "the same object twice"
    );
}

#[test]
fn an_entry_of_the_properties_row_goes_where_a_last_inserted_key_goes() {
    let entry = |n: u32| HelpEntry {
        key: id(n),
        props: vec![(
            "3".to_owned(),
            Brace::List(vec![Brace::str("N"), Brace::num(1)]),
        )],
    };
    let mut help = HelpProps {
        entries: vec![entry(1), entry(2)],
    };
    assert_eq!(HelpProps::parse(&help.render()).unwrap(), help);
    // 2 entries + the new one: 8 buckets; 0x0a & 7 = 2 is the bucket of key 2 -> in front of it
    help.add_entry_approximately(entry(0x0a)).unwrap();
    let keys: Vec<String> = help.entries.iter().map(|e| e.key.clone()).collect();
    assert_eq!(keys, [id(1), id(0x0a), id(2)]);
    // an empty bucket: the end
    help.add_entry_approximately(entry(0x0d)).unwrap();
    assert_eq!(help.entries.last().unwrap().key, id(0x0d));
    assert!(help.add_entry_approximately(entry(1)).is_err());
}

const CRLF: &str = "\r\n";

fn model() -> XdtoModel {
    let lines = [
        "<model>",
        "\t<package>",
        "\t\t<valueType xmlns:d3p1=\"http://v8.1c.ru/8.1/data/enterprise\" name=\"CatalogRef.A\" base=\"d3p1:AnyDBRef\"/>",
        "\t\t<valueType xmlns:d3p1=\"http://v8.1c.ru/8.1/data/enterprise\" name=\"CatalogRef.C\" base=\"d3p1:AnyDBRef\"/>",
        "\t\t<valueType xmlns:d3p1=\"http://v8.1c.ru/8.1/data/enterprise\" name=\"DocumentRef.D\" base=\"d3p1:AnyDBRef\"/>",
        "\t\t<objectType name=\"CatalogObject.A\">",
        "\t\t\t<property name=\"Code\" type=\"xs:string\"/>",
        "\t\t</objectType>",
        "\t\t<objectType name=\"CatalogTabularSectionRow.C.T1\">",
        "\t\t\t<property name=\"X\" type=\"xs:string\"/>",
        "\t\t</objectType>",
        "\t\t<objectType name=\"CatalogObject.C\">",
        "\t\t\t<property name=\"Code\" type=\"xs:string\"/>",
        "\t\t\t<property xmlns:d4p1=\"http://v8.1c.ru/8.1/data/enterprise/current-config\" name=\"T1\" type=\"d4p1:CatalogTabularSectionRow.C.T1\" lowerBound=\"0\" upperBound=\"99999\"/>",
        "\t\t</objectType>",
        "\t\t<objectType name=\"DocumentObject.D\">",
        "\t\t\t<property name=\"Number\" type=\"xs:string\"/>",
        "\t\t</objectType>",
        "\t</package>",
        "</model>",
    ];
    XdtoModel::from_xml(lines.join(CRLF) + CRLF)
}

fn position(model: &XdtoModel, needle: &str) -> usize {
    model
        .xml
        .find(needle)
        .unwrap_or_else(|| panic!("{needle} is not in the model"))
}

#[test]
fn a_catalog_and_its_sections_take_their_places_in_the_xdto_model() {
    let mut model = model();
    let shape = CatalogShape {
        hierarchical: false,
        hierarchy_type: 0,
        owners: 0,
        code_length: 9,
        code_type: 1,
        description_length: 25,
    };
    let attribute = primitive_property("Реквизит", "xs:string", true);
    let section = section_row_block(
        Family::Catalog,
        "B",
        "T",
        &[primitive_property("Y", "xs:decimal", false)],
    );
    let section_property = section_property_line(Family::Catalog, "B", "T");
    model
        .add_catalog(
            "B",
            shape,
            Some("A"),
            Some("C"),
            &[attribute, section_property],
            &[section],
        )
        .unwrap();
    let reference = position(&model, "name=\"CatalogRef.B\"");
    assert!(
        position(&model, "name=\"CatalogRef.A\"") < reference
            && reference < position(&model, "name=\"CatalogRef.C\"")
    );
    let row = position(&model, "<objectType name=\"CatalogTabularSectionRow.B.T\">");
    let object = position(&model, "<objectType name=\"CatalogObject.B\">");
    assert!(position(&model, "<objectType name=\"CatalogObject.A\">") < row && row < object);
    assert!(
        object
            < position(
                &model,
                "<objectType name=\"CatalogTabularSectionRow.C.T1\">"
            )
    );
    // the standard properties of the shape, the attribute, then the section's property
    let block = &model.xml[object..];
    let ref_at = block.find("name=\"Ref\"").unwrap();
    let code_at = block.find("name=\"Code\" type=\"xs:string\"/>").unwrap();
    let description_at = block.find("name=\"Description\"").unwrap();
    let predefined_at = block.find("name=\"PredefinedDataName\"").unwrap();
    let attribute_at = block.find("name=\"Реквизит\"").unwrap();
    let section_at = block
        .find("name=\"T\" type=\"d4p1:CatalogTabularSectionRow.B.T\"")
        .unwrap();
    assert!(ref_at < code_at && code_at < description_at && description_at < predefined_at);
    assert!(predefined_at < attribute_at && attribute_at < section_at);
    assert!(
        model
            .add_catalog("B", shape, Some("A"), Some("C"), &[], &[])
            .is_err(),
        "the same twice"
    );

    // a first catalog goes before the successor's block (its row types included)
    let mut first = self::model();
    first
        .add_catalog("Z", shape, None, Some("C"), &[], &[])
        .unwrap();
    assert!(
        position(&first, "<objectType name=\"CatalogObject.Z\">")
            < position(
                &first,
                "<objectType name=\"CatalogTabularSectionRow.C.T1\">"
            )
    );
    assert!(
        position(&first, "<objectType name=\"CatalogObject.A\">")
            < position(&first, "<objectType name=\"CatalogObject.Z\">")
    );
}

#[test]
fn a_tabular_section_goes_before_its_owner_and_into_its_properties() {
    let mut model = model();
    model
        .add_tabular_section(
            "C",
            "T2",
            Some("T1"),
            &[primitive_property("Y", "xs:decimal", false)],
        )
        .unwrap();
    let old_row = position(
        &model,
        "<objectType name=\"CatalogTabularSectionRow.C.T1\">",
    );
    let new_row = position(
        &model,
        "<objectType name=\"CatalogTabularSectionRow.C.T2\">",
    );
    let owner = position(&model, "<objectType name=\"CatalogObject.C\">");
    assert!(old_row < new_row && new_row < owner);
    let after_old = position(&model, "name=\"T1\" type=\"d4p1:");
    let after_new = position(&model, "name=\"T2\" type=\"d4p1:");
    assert!(owner < after_old && after_old < after_new);
    // the first section of an object that has some must say which one it follows
    assert!(
        self::model()
            .add_tabular_section("C", "T0", None, &[])
            .is_err()
    );
    // a section of an object without any: at the end of the properties
    let mut model = self::model();
    model.add_tabular_section("A", "T", None, &[]).unwrap();
    assert!(
        position(&model, "name=\"Code\"")
            < position(
                &model,
                "name=\"T\" type=\"d4p1:CatalogTabularSectionRow.A.T\""
            )
    );
}

#[test]
fn a_document_takes_the_places_of_a_catalog_with_its_own_standard_properties() {
    let mut model = model();
    model
        .add_document(
            "E",
            DocumentShape { number_type: 1 },
            Some("D"),
            None,
            &[],
            &[],
        )
        .unwrap();
    assert!(
        position(&model, "name=\"DocumentRef.D\"") < position(&model, "name=\"DocumentRef.E\"")
    );
    let object = position(&model, "<objectType name=\"DocumentObject.E\">");
    let block = &model.xml[object..];
    let names = ["Ref", "DeletionMark", "Date", "Number", "Posted"];
    let mut last = 0;
    for name in names {
        let at = block.find(&format!("name=\"{name}\"")).unwrap();
        assert!(at >= last, "{name} is out of order");
        last = at;
    }
    assert!(block.contains("name=\"Number\" type=\"xs:string\"/>"));
    // the row of a document's section
    model
        .add_document_section(
            "E",
            "Товары",
            None,
            &[primitive_property("Количество", "xs:decimal", false)],
        )
        .unwrap();
    assert!(
        position(
            &model,
            "<objectType name=\"DocumentTabularSectionRow.E.Товары\">"
        ) < position(
            &model,
            "name=\"Товары\" type=\"d4p1:DocumentTabularSectionRow.E.Товары\""
        )
    );
    assert!(model.xml.contains("upperBound=\"99999\""));
}

#[test]
fn a_model_row_round_trips_through_its_envelope() {
    let model = model();
    let text = model.render();
    assert!(text.starts_with(&[0xEF, 0xBB, 0xBF]));
    let back = XdtoModel::parse(&text).unwrap();
    assert_eq!(back.xml, model.xml);
    assert_eq!(back.render(), text);
    assert_eq!(
        XdtoModel::from_stored(&model.to_stored().unwrap()).unwrap(),
        back
    );
}
