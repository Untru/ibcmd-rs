//! Tabular sections in the plan (S1-E), on the fixtures of case a2: the catalog `_ДемоПартнеры` has two
//! sections (`VT155` with 3 attributes, `VT159` with 13, two of them indexed). The staged rows are the stored
//! one with a section cloned, an attribute cloned, a section removed and so on, edited as brace trees; the
//! cloned parts get fresh uuids and primitive types (a reference or a composite type is not mapped to a
//! field). The cases traced on the platform (e1, e3, e4) are in `tests_corpus`.

pub(super) use super::test_sections_support::*;

use crate::metadata_model::brace::{Brace, parse_row};
use crate::restructure::names::{DbNames, deflate, inflate};
use crate::restructure::plan::{Phase, plan};
use crate::restructure::schema::{DbSchema, TableView};
use crate::restructure::tests_plan::{OLD_ROW, inputs, options};

fn error_of(tree: &Brace) -> String {
    format!("{:#}", plan(&staged(tree), &options()).unwrap_err())
}

/// An own attribute added to the catalog, so that the images differ by an attribute (the plan looks at the
/// sections only when something in the images is new or removed).
fn add_own_attribute(root: &mut Brace) {
    let attributes = collection(root, "cf4abea7-37b2-11d4-940f-008048da11f9");
    let mut own = fresh(&attributes[2], 5, true);
    rename(&mut own, "Клиент", "ДемоРеквизит");
    own.as_list_mut().unwrap()[0].as_list_mut().unwrap()[2] = Brace::num(0);
    attributes.push(own);
    recount(attributes);
}

#[test]
fn a_new_section_is_a_created_table_numbered_after_the_stored_names() {
    let plan = plan(&staged(&with_new_section()), &options()).unwrap();
    let object = &plan.objects[0];
    assert_eq!(object.object, "Reference20");
    assert!(object.additions.is_empty());
    let [section] = object.sections.as_slice() else {
        panic!("{:?}", object.sections)
    };
    // The extensions of the fixture hold 11033 at most: the shared counter goes on from there.
    let created = section.created.unwrap();
    assert_eq!((created.table_number, created.line_no), (11034, 11035));
    assert_eq!(section.table, "VT11034");
    let numbers: Vec<u64> = section.additions.iter().map(|a| a.number).collect();
    assert_eq!(numbers, [11036, 11037, 11038]);
    assert_eq!(
        section
            .additions
            .iter()
            .map(|a| a.position)
            .collect::<Vec<_>>(),
        [1, 2, 3]
    );
    assert_eq!(
        object.changes(),
        "new tabular sections VT11034 = НоваяТЧ (3 attribute(s))"
    );

    // DBNames: the two entries of the section, then one for each attribute.
    let names = DbNames::parse(&plan.new_names_text).unwrap();
    assert_eq!(names.max, 11038);
    let tail: Vec<(&str, u64)> = names.entries[names.entries.len() - 5..]
        .iter()
        .map(|entry| (entry.kind.as_str(), entry.number))
        .collect();
    assert_eq!(
        tail,
        [
            ("VT", 11034),
            ("LineNo", 11035),
            ("Fld", 11036),
            ("Fld", 11037),
            ("Fld", 11038)
        ]
    );

    // DBSchema: the sub-table is appended to the object's list, its first field is the line number.
    let schema = DbSchema::parse(&plan.new_schema).unwrap();
    let view = schema
        .view(schema.position("Reference20").unwrap())
        .unwrap();
    let subtables = view.subtables().unwrap();
    assert_eq!(
        subtables.iter().map(TableView::name).collect::<Vec<_>>(),
        ["VT155", "VT159", "VT11034"]
    );
    let fields = subtables[2].fields().unwrap();
    assert_eq!(
        fields.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(),
        ["LineNo11035", "Fld11036", "Fld11037", "Fld11038"]
    );
    assert_eq!(subtables[2].owner_name(), "Reference20");
    assert!(subtables[2].indexes().unwrap().is_empty());

    // The tables: the old three are copied, the new one is created and left empty.
    let created: Vec<(&str, bool)> = object
        .tables
        .iter()
        .map(|table| (table.table.name.as_str(), table.create))
        .collect();
    assert_eq!(
        created,
        [
            ("_Reference20", false),
            ("_Reference20_VT155", false),
            ("_Reference20_VT159", false),
            ("_Reference20_VT11034", true)
        ]
    );
    let statements = plan.statements();
    let labels: Vec<&str> = statements.iter().map(|s| s.label.as_str()).collect();
    assert!(labels.contains(&"create _Reference20_VT11034NG"));
    assert!(
        !labels
            .iter()
            .any(|l| l.contains("copy _Reference20_VT11034"))
    );
    assert_eq!(
        statements.iter().filter(|s| s.phase == Phase::Load).count(),
        3
    );
    assert_eq!(
        statements
            .iter()
            .filter(|s| s.phase == Phase::DropOld)
            .count(),
        3
    );
    // No cache row without cache rows: the options of the fixtures leave the caches alone.
    assert!(plan.caches.is_empty());
}

#[test]
fn a_new_section_keeps_the_indexes_of_its_indexed_attributes_not_unique() {
    let mut root = with_new_section();
    let sections = collection(&mut root, SECTIONS);
    let last = sections.last_mut().unwrap();
    let attributes = attributes_of(last);
    set_indexing(&mut attributes[3], 1);
    let plan = plan(&staged(&root), &options()).unwrap();
    let section = &plan.objects[0].sections[0];
    assert_eq!(section.indexes, ["ByFieldFld11037"]);
    let schema = DbSchema::parse(&plan.new_schema).unwrap();
    let view = schema
        .view(schema.position("Reference20").unwrap())
        .unwrap();
    let indexes = view.subtables().unwrap()[2].indexes().unwrap();
    assert_eq!(indexes.len(), 1);
    assert_eq!(indexes[0].name, "ByFieldFld11037");
    assert!(!indexes[0].unique);
    assert_eq!(indexes[0].fields, ["Fld11037", "ID"]);
    // The physical table has the index on the separator, the field and the owner key.
    let table = plan.objects[0].tables.last().unwrap();
    assert!(
        table
            .table
            .indexes
            .iter()
            .any(|index| index.columns == ["_Fld2683", "_Fld11037", "_Reference20_IDRRef"])
    );
}

#[test]
fn new_attributes_of_an_old_section_go_after_their_predecessor_with_their_indexes_in_field_order() {
    // The 13 attributes of `VT159`; two of them are indexed (`ByFieldFld161`, `ByFieldFld162`). One new
    // attribute first (indexed), one in the middle, one last (indexed).
    let mut root = tree();
    let sections = collection(&mut root, SECTIONS);
    let template = attributes_of(&mut sections[3]).clone();
    let attributes = attributes_of(&mut sections[3]);
    let mut first = fresh(&template[2], 2, true);
    set_indexing(&mut first, 1);
    let mut middle = fresh(&template[2], 3, true);
    set_indexing(&mut middle, 0);
    let mut last = fresh(&template[2], 4, true);
    set_indexing(&mut last, 1);
    attributes.push(last);
    attributes.insert(8, middle);
    attributes.insert(2, first);
    recount(attributes);
    let plan = plan(&staged(&root), &options()).unwrap();
    let object = &plan.objects[0];
    let [section] = object.sections.as_slice() else {
        panic!("{:?}", object.sections)
    };
    assert!(section.created.is_none());
    assert_eq!(section.table, "VT159");
    // The numbers come in the order of the descriptor.
    assert_eq!(
        section
            .additions
            .iter()
            .map(|a| a.number)
            .collect::<Vec<_>>(),
        [11034, 11035, 11036]
    );
    assert_eq!(section.additions[0].position, 1);
    assert_eq!(section.indexes, ["ByFieldFld11034", "ByFieldFld11036"]);
    let changes = object.changes();
    assert!(
        changes.starts_with("new attributes of tabular sections "),
        "{changes}"
    );
    assert!(
        changes.contains("Fld11034 = ") && changes.contains("Fld11036 = "),
        "{changes}"
    );

    let schema = DbSchema::parse(&plan.new_schema).unwrap();
    let view = schema
        .view(schema.position("Reference20").unwrap())
        .unwrap();
    let subtable = view.subtables().unwrap()[1];
    assert_eq!(subtable.name(), "VT159");
    let fields: Vec<String> = subtable
        .fields()
        .unwrap()
        .into_iter()
        .map(|f| f.name)
        .collect();
    assert_eq!(fields.len(), 1 + 13 + 3);
    assert_eq!(fields[0], "LineNo160");
    assert_eq!(fields[1], "Fld11034");
    assert_eq!(fields.last().unwrap(), "Fld11036");
    let indexes: Vec<String> = subtable
        .indexes()
        .unwrap()
        .into_iter()
        .map(|index| index.name)
        .collect();
    assert_eq!(
        indexes,
        [
            "ByFieldFld11034",
            "ByFieldFld161",
            "ByFieldFld162",
            "ByFieldFld11036"
        ]
    );
    // Nothing is created: every table is copied, the new columns get defaults.
    assert!(object.tables.iter().all(|table| !table.create));
    let sql = plan.phase_sql("@now").unwrap();
    assert!(sql.contains("_Fld11034"));
    assert!(sql.contains("_Fld11036"));
}

#[test]
fn a_new_section_and_new_own_attributes_take_their_numbers_in_the_order_of_the_descriptor() {
    let mut root = with_new_section();
    // A new own attribute, cloned from the first one and made a string: it is numbered before the section.
    add_own_attribute(&mut root);
    let plan = plan(&staged(&root), &options()).unwrap();
    let object = &plan.objects[0];
    assert_eq!(object.additions.len(), 1);
    assert_eq!(object.additions[0].number, 11034);
    let section = &object.sections[0];
    assert_eq!(section.created.unwrap().table_number, 11035);
    assert_eq!(section.created.unwrap().line_no, 11036);
    assert_eq!(section.additions[0].number, 11037);
    assert!(object.changes().starts_with("new attributes Fld11034"));
}

#[test]
fn what_is_not_built_about_sections_is_refused_with_the_reason() {
    // A section removed.
    let mut root = tree();
    let sections = collection(&mut root, SECTIONS);
    sections.pop();
    recount(sections);
    assert!(
        error_of(&root).contains("removes a tabular section"),
        "{}",
        error_of(&root)
    );

    // The sections swapped.
    let mut root = tree();
    add_own_attribute(&mut root);
    let sections = collection(&mut root, SECTIONS);
    sections.swap(2, 3);
    assert!(
        error_of(&root).contains("moves a tabular section"),
        "{}",
        error_of(&root)
    );

    // A new section before the old ones.
    let mut root = tree();
    let sections = collection(&mut root, SECTIONS);
    let copy = fresh(&sections[2], 1, true);
    sections.insert(2, copy);
    recount(sections);
    assert!(
        error_of(&root).contains("puts a new one before the old ones"),
        "{}",
        error_of(&root)
    );

    // A stored section renamed.
    let mut root = tree();
    add_own_attribute(&mut root);
    let sections = collection(&mut root, SECTIONS);
    rename(&mut sections[2], "ДополнительныеРеквизиты", "Другая");
    assert!(
        error_of(&root).contains("changes the tabular section"),
        "{}",
        error_of(&root)
    );

    // An attribute of a stored section removed, or retyped.
    let mut root = tree();
    let sections = collection(&mut root, SECTIONS);
    let attributes = attributes_of(&mut sections[3]);
    attributes.remove(4);
    recount(attributes);
    let text = error_of(&root);
    assert!(
        text.contains("removes, moves or changes an attribute of the tabular section"),
        "{text}"
    );
    let mut root = tree();
    add_own_attribute(&mut root);
    let sections = collection(&mut root, SECTIONS);
    let attributes = attributes_of(&mut sections[3]);
    attributes.swap(3, 4);
    let text = error_of(&root);
    assert!(
        text.contains("removes, moves or changes an attribute of the tabular section"),
        "{text}"
    );

    // An indexed attribute of a stored section switched off.
    let mut root = tree();
    add_own_attribute(&mut root);
    let sections = collection(&mut root, SECTIONS);
    let attributes = attributes_of(&mut sections[3]);
    let indexed = (2..attributes.len())
        .find(|&at| {
            attributes[at].as_list().unwrap()[0].as_list().unwrap()[2].as_atom() == Some("1")
        })
        .expect("an indexed attribute in the section");
    set_indexing(&mut attributes[indexed], 0);
    let text = error_of(&root);
    assert!(
        text.contains("removes, moves or changes an attribute of the tabular section"),
        "{text}"
    );
}

#[test]
fn what_is_not_built_about_a_new_section_is_refused_with_the_reason() {
    // A section with a type that has no field yet (the fixture's own types are references).
    let mut root = tree();
    let sections = collection(&mut root, SECTIONS);
    let copy = fresh(&sections[2], 1, false);
    sections.push(copy);
    recount(sections);
    let text = error_of(&root);
    assert!(text.contains("of tabular section"), "{text}");
    assert!(text.contains("not mapped to fields yet"), "{text}");

    // No attribute.
    let mut root = with_new_section();
    add_own_attribute(&mut root);
    let sections = collection(&mut root, SECTIONS);
    let attributes = attributes_of(sections.last_mut().unwrap());
    attributes.truncate(2);
    attributes[1] = Brace::num(0);
    let text = error_of(&root);
    assert!(text.contains("with no attributes"), "{text}");

    // A line number of another length: the newer record version carries it.
    let mut root = with_new_section();
    let sections = collection(&mut root, SECTIONS);
    let wrapper = sections.last_mut().unwrap().as_list_mut().unwrap()[0]
        .as_list_mut()
        .unwrap();
    wrapper[0] = Brace::num(2);
    wrapper.push(Brace::num(6));
    let text = error_of(&root);
    assert!(text.contains("line number length other than 5"), "{text}");
    // ... the same section with the 5 every section has is let through.
    let mut root = with_new_section();
    let sections = collection(&mut root, SECTIONS);
    let wrapper = sections.last_mut().unwrap().as_list_mut().unwrap()[0]
        .as_list_mut()
        .unwrap();
    wrapper[0] = Brace::num(2);
    wrapper.push(Brace::num(5));
    assert!(plan(&staged(&root), &options()).is_ok());

    // An attribute in the new section that the stored image has (moved from another place).
    let mut root = with_new_section();
    let sections = collection(&mut root, SECTIONS);
    let stored = attributes_of(&mut sections[2])[2].clone();
    let attributes = attributes_of(sections.last_mut().unwrap());
    attributes[2] = stored;
    let text = error_of(&root);
    assert!(text.contains("of the stored image"), "{text}");

    // The additional order in a section attribute: not traced.
    let mut root = with_new_section();
    let sections = collection(&mut root, SECTIONS);
    let attributes = attributes_of(sections.last_mut().unwrap());
    set_indexing(&mut attributes[2], 2);
    let text = error_of(&root);
    assert!(
        text.contains("only DontIndex and Index are traced in a tabular section"),
        "{text}"
    );
}

#[test]
fn a_stage_that_removes_and_adds_a_section_is_refused() {
    // The removal of the attribute of case a2 (the state after its addition) with a new section.
    let added = plan(
        &inputs(OLD_ROW, crate::restructure::tests_plan::NEW_ROW),
        &options(),
    )
    .unwrap();
    let mut root = parse_row(&inflate(crate::restructure::tests_plan::NEW_ROW).unwrap()).unwrap();
    let attributes = collection(&mut root, "cf4abea7-37b2-11d4-940f-008048da11f9");
    attributes.pop();
    recount(attributes);
    let sections = collection(&mut root, SECTIONS);
    let mut copy = fresh(&sections[2], 1, true);
    rename(&mut copy, "ДополнительныеРеквизиты", "НоваяТЧ");
    sections.push(copy);
    recount(sections);
    let mut back = inputs(crate::restructure::tests_plan::NEW_ROW, &stage(&root));
    back.schema = added.new_schema.clone();
    back.main_names = added.new_names_row.clone();
    back.staged.deleted =
        Some(deflate("\u{feff}1,\"c60cdc87-198a-4f6e-8f17-76bcb1b1914b\",1".as_bytes()).unwrap());
    let text = format!("{:#}", plan(&back, &options()).unwrap_err());
    assert!(
        text.contains("removes an attribute and adds a tabular section"),
        "{text}"
    );
}

#[test]
fn the_stored_sub_tables_are_checked_before_anything_is_trusted() {
    // The schema has no sub-table for the stored section: the stored state is not what the metadata says.
    let mut broken = staged(&with_new_section());
    let text = String::from_utf8(broken.schema.clone()).unwrap();
    assert!(text.contains("\"VT159\""));
    broken.schema = text.replace("\"VT159\"", "\"VT158\"").into_bytes();
    let text = format!("{:#}", plan(&broken, &options()).unwrap_err());
    assert!(text.contains("no sub-table VT159"), "{text}");
}

/// `Use` of a catalog's section, the element after the record of its wrapper `{v,<record>,<Use>}`.
fn set_section_use(root: &mut Brace, section: usize, usage: i64) {
    let sections = collection(root, SECTIONS);
    sections[section].as_list_mut().unwrap()[0]
        .as_list_mut()
        .unwrap()[2] = Brace::num(usage);
}

#[test]
fn a_section_of_a_hierarchy_used_for_folders_is_not_built() {
    // A stored section used for folders that gets an attribute.
    let mut old = tree();
    set_section_use(&mut old, 3, 1);
    let mut new = old.clone();
    let sections = collection(&mut new, SECTIONS);
    let attributes = attributes_of(&mut sections[3]);
    let added = fresh(&attributes[2], 9, true);
    attributes.push(added);
    recount(attributes);
    let text = format!(
        "{:#}",
        plan(&inputs(&stage(&old), &stage(&new)), &options()).unwrap_err()
    );
    assert!(text.contains("whose use is not ForItem"), "{text}");

    // A new section used for both folders and items.
    let mut root = with_new_section();
    set_section_use(&mut root, 4, 2);
    let text = error_of(&root);
    assert!(text.contains("another use than ForItem"), "{text}");
}

#[test]
fn a_new_attribute_comes_indexed_with_the_entries_of_a_switch() {
    use crate::restructure::tests_plan::NEW_ROW;
    for (mode, tail) in [
        (1, vec!["Fld11034", "ID"]),
        (2, vec!["Fld11034", "Description", "ID", "Marked"]),
    ] {
        let mut root = parse_row(&inflate(NEW_ROW).unwrap()).unwrap();
        let attributes = collection(&mut root, "cf4abea7-37b2-11d4-940f-008048da11f9");
        let last = attributes.len() - 1;
        set_indexing(&mut attributes[last], mode);
        let plan = plan(&inputs(OLD_ROW, &stage(&root)), &options()).unwrap();
        let object = &plan.objects[0];
        let [addition] = object.additions.as_slice() else {
            panic!("{:?}", object.additions)
        };
        assert_eq!(addition.indexing, mode);
        assert_eq!(
            addition.indexes,
            ["ByParentFieldFld11034", "ByFieldFld11034"]
        );
        let schema = DbSchema::parse(&plan.new_schema).unwrap();
        let view = schema
            .view(schema.position("Reference20").unwrap())
            .unwrap();
        let indexes = view.indexes().unwrap();
        let names: Vec<&str> = indexes.iter().map(|index| index.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "ByPredefinedIDNotUniq",
                "ParentCode",
                "ParentDescr",
                "Code",
                "Descr",
                "ByParentFieldFld11034",
                "ByFieldFld11034"
            ]
        );
        assert_eq!(indexes[6].fields, tail);
        // The physical table has the two indexes more than it would without them.
        let plain = crate::restructure::plan::plan(&inputs(OLD_ROW, NEW_ROW), &options()).unwrap();
        assert_eq!(
            object.tables[0].table.indexes.len(),
            plain.objects[0].tables[0].table.indexes.len() + 2
        );
    }
}
