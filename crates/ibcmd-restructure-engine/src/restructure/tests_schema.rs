//! Tests of the schema model against the native evidence of case a2 (fixtures in
//! `tests/fixtures/native-evidence/restructure`, the statement trace in `docs/apply/evidence`).

use crate::restructure::schema::*;

const STAGED: &[u8] = include_bytes!(
    "../../../../tests/fixtures/native-evidence/restructure/dbschema-a2-staged-excerpt.txt"
);
const NATIVE: &[u8] = include_bytes!(
    "../../../../tests/fixtures/native-evidence/restructure/dbschema-a2-native-excerpt.txt"
);
/// The statements of the native apply of case a2, verbatim.
const STATEMENTS: &str =
    include_str!("../../../../docs/apply/evidence/restructuring/a2-structure-statements.sql");

fn lf(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn names(schema: &DbSchema) -> Vec<String> {
    schema
        .tables()
        .iter()
        .map(|table| TableView::new(table).unwrap().name().to_owned())
        .collect()
}

#[test]
fn schema_texts_round_trip_byte_for_byte() {
    for text in [STAGED, NATIVE] {
        let schema = DbSchema::parse(text).unwrap();
        assert_eq!(schema.len(), 4);
        assert_eq!(schema.to_text(), text);
    }
    assert_eq!(
        names(&DbSchema::parse(NATIVE).unwrap()),
        ["Reference20", "DbCopiesUpdates", "DbCopies", "ConfigChngR"]
    );
}

#[test]
fn a_header_count_that_disagrees_is_refused() {
    let text = String::from_utf8(STAGED.to_vec())
        .unwrap()
        .replacen("{4,", "{5,", 1);
    assert!(DbSchema::parse(text.as_bytes()).is_err());
}

#[test]
fn the_create_table_statements_are_what_the_platform_ran() {
    let native = DbSchema::parse(NATIVE).unwrap();
    let statements = lf(STATEMENTS);
    let mut checked = 0;
    for name in ["Reference20", "DbCopiesUpdates", "DbCopies", "ConfigChngR"] {
        let view = native.named(name).unwrap();
        for table in physical_tables(&view).unwrap() {
            let sql = create_table_sql(&table, "NG");
            assert!(statements.contains(&sql), "not in the native trace:\n{sql}");
            checked += 1;
        }
    }
    // Reference20 with two sub-tables, the two upgraded system tables, ConfigChngR with its extension table.
    assert_eq!(checked, 7);
}

#[test]
fn the_index_statements_come_in_the_order_the_platform_created_them() {
    let native = DbSchema::parse(NATIVE).unwrap();
    let object = native.named("Reference20").unwrap();
    let mut generated = Vec::new();
    for table in physical_tables(&object).unwrap() {
        for index in &table.indexes {
            generated.push(create_index_sql(&table.name, index, "NG"));
        }
    }
    // The first occurrences, in trace order, of the CREATE INDEX statements on Reference20.
    let mut traced: Vec<String> = Vec::new();
    for line in lf(STATEMENTS).lines() {
        if line.starts_with("CREATE ")
            && line.contains("_Reference20")
            && !traced.iter().any(|seen| seen == line)
        {
            traced.push(line.to_owned());
        }
    }
    assert_eq!(generated, traced);
    assert_eq!(generated.len(), 10);
}

#[test]
fn the_staged_entry_with_the_new_field_is_the_platforms_new_entry() {
    let staged = DbSchema::parse(STAGED).unwrap();
    let native = DbSchema::parse(NATIVE).unwrap();
    let mut entry = staged.tables()[staged.position("Reference20").unwrap()].clone();
    let field = FieldEntry::new(
        "Fld11034",
        true,
        vec![TypeEntry::new("S", 0x8000_0000 | 50, 0, "", 0)],
    );
    // After Fld6357, before the data separator Fld2683.
    insert_field(&mut entry, 14, &field).unwrap();
    assert_eq!(
        entry,
        native.tables()[native.position("Reference20").unwrap()]
    );
    assert!(insert_field(&mut entry, 99, &field).is_err());
}

#[test]
fn a_rebuilt_table_moves_before_the_change_registration_table() {
    let staged = DbSchema::parse(STAGED).unwrap();
    let mut moved = staged.clone();
    let entry = moved.remove("Reference20").unwrap();
    moved.insert_before("ConfigChngR", entry);
    assert_eq!(
        names(&moved),
        ["DbCopiesUpdates", "DbCopies", "Reference20", "ConfigChngR"]
    );
    assert_eq!(moved.len(), staged.len());
}

/// The stored list with `ConfigChngR` where a test wants it (the entries are the four of case a2's excerpt).
fn schema_in_order(order: &[&str]) -> DbSchema {
    let staged = DbSchema::parse(STAGED).unwrap();
    DbSchema::from_tables(
        order
            .iter()
            .map(|name| staged.tables()[staged.position(name).unwrap()].clone())
            .collect(),
    )
}

/// Rebuilds the tables in the order given, as `plan()` does: out of their place, into the platform's.
fn rebuild(schema: &mut DbSchema, tables: &[&str]) {
    for name in tables {
        let entry = schema.remove(name).unwrap();
        schema.insert_rebuilt(entry);
    }
}

#[test]
fn rebuilt_tables_go_ahead_of_the_change_registration_table_when_that_is_the_last_table() {
    // The БСП: the platform rebuilds ConfigChngR in every apply, after the others; it is the last table.
    let mut schema =
        schema_in_order(&["DbCopiesUpdates", "DbCopies", "Reference20", "ConfigChngR"]);
    rebuild(&mut schema, &["Reference20", "DbCopiesUpdates"]);
    assert_eq!(
        names(&schema),
        ["DbCopies", "Reference20", "DbCopiesUpdates", "ConfigChngR"]
    );
}

#[test]
fn rebuilt_tables_go_to_the_end_when_the_change_registration_table_is_in_the_middle() {
    // The ERP УХ (track rcheck, `b1`, `c1`): the register is empty, the platform does not rebuild it, it stays where it
    // is and the rebuilt tables are appended to the end of the list in the order they were rebuilt.
    let mut schema =
        schema_in_order(&["DbCopiesUpdates", "ConfigChngR", "Reference20", "DbCopies"]);
    rebuild(&mut schema, &["Reference20", "DbCopiesUpdates"]);
    assert_eq!(
        names(&schema),
        ["ConfigChngR", "DbCopies", "Reference20", "DbCopiesUpdates"]
    );
    // Without the named tables the order is the one it was (what rcheck's `schema_order.py` checks on the real lists).
    let before = schema_in_order(&["DbCopiesUpdates", "ConfigChngR", "Reference20", "DbCopies"]);
    let without = |schema: &DbSchema| -> Vec<String> {
        names(schema)
            .into_iter()
            .filter(|name| name != "Reference20" && name != "DbCopiesUpdates")
            .collect()
    };
    assert_eq!(without(&schema), without(&before));
}

#[test]
fn created_tables_come_before_the_rebuilt_ones_in_both_placements() {
    // S1-F: the tables the platform makes anew come first, the rebuilt ones after them; both take the place of the list.
    for (order, expected) in [
        (
            ["DbCopiesUpdates", "DbCopies", "Reference20", "ConfigChngR"],
            ["DbCopies", "DbCopiesUpdates", "Reference20", "ConfigChngR"],
        ),
        (
            ["DbCopiesUpdates", "ConfigChngR", "Reference20", "DbCopies"],
            ["ConfigChngR", "DbCopies", "DbCopiesUpdates", "Reference20"],
        ),
    ] {
        let mut schema = schema_in_order(&order);
        // DbCopiesUpdates stands for the created table, Reference20 for the rebuilt one.
        rebuild(&mut schema, &["DbCopiesUpdates"]);
        rebuild(&mut schema, &["Reference20"]);
        assert_eq!(names(&schema), expected);
    }
}

#[test]
fn column_names_follow_the_type_entries() {
    let composite = FieldEntry::new(
        "Fld158",
        false,
        vec![
            TypeEntry::new("E", 0, 0, "", 0),
            TypeEntry::new("L", 0, 0, "", 0),
            TypeEntry::new("N", 17, 5, "", 0),
            TypeEntry::new("T", 0, 0, "", 0),
            TypeEntry::new("S", 0x8000_0400, 0, "", 0),
            TypeEntry::new("R", 0, 0, "", 4),
        ],
    );
    let columns: Vec<_> = field_columns(&composite)
        .unwrap()
        .into_iter()
        .map(|column| column.name)
        .collect();
    assert_eq!(
        columns,
        [
            "_Fld158_TYPE",
            "_Fld158_L",
            "_Fld158_N",
            "_Fld158_T",
            "_Fld158_S",
            "_Fld158_RTRef",
            "_Fld158_RRRef"
        ]
    );
    let typed = FieldEntry::new(
        "Fld1",
        false,
        vec![TypeEntry::new("R", 0, 0, "Reference20", 3)],
    );
    assert_eq!(field_columns(&typed).unwrap()[0].name, "_Fld1RRef");
    let untyped = FieldEntry::new("Fld1", true, vec![TypeEntry::new("R", 0, 0, "", 4)]);
    let columns = field_columns(&untyped).unwrap();
    assert_eq!(
        columns.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
        ["_Fld1TRef", "_Fld1RRef"]
    );
    assert!(columns.iter().all(|column| column.nullable));
    let unknown = FieldEntry::new("Fld1", false, vec![TypeEntry::new("Q", 0, 0, "", 0)]);
    assert!(field_columns(&unknown).is_err());
}

#[test]
fn sql_types_follow_the_entry_tags() {
    let t = |tag: &str, a: u64, b: u64| sql_type(&TypeEntry::new(tag, a, b, "", 0)).unwrap();
    assert_eq!(t("B", 16, 0), SqlType::Binary(16));
    assert_eq!(t("B", 0x8000_0000, 0), SqlType::VarBinaryMax);
    assert_eq!(t("B", 0x8000_0010, 0), SqlType::VarBinary(16));
    assert_eq!(t("L", 0, 0), SqlType::Binary(1));
    assert_eq!(t("N", 15, 2), SqlType::Numeric(15, 2));
    assert_eq!(t("T", 0, 0), SqlType::DateTime2);
    assert_eq!(t("S", 0x8000_0032, 0), SqlType::NVarChar(50));
    assert_eq!(t("S", 0x8000_0000, 0), SqlType::NVarCharMax);
    assert_eq!(t("S", 20, 0), SqlType::NChar(20));
    assert_eq!(t("V", 0, 0), SqlType::Timestamp);
    let integer = TypeEntry {
        six: Some(1),
        ..TypeEntry::new("N", 9, 0, "", 0)
    };
    assert_eq!(sql_type(&integer).unwrap(), SqlType::Int);
    let wide = TypeEntry {
        six: Some(1),
        ..TypeEntry::new("N", 15, 0, "", 0)
    };
    assert_eq!(sql_type(&wide).unwrap(), SqlType::BigInt);
    assert_eq!(SqlType::Numeric(7, 0).ddl(), "numeric(7, 0)");
}

/// The entry of `Reference20` of the fixture with the given index list (in the platform's layout).
fn entry_with_indexes(indexes: &str) -> crate::metadata_model::brace::Brace {
    let mut entry = DbSchema::parse(STAGED).unwrap().tables()[0].clone();
    entry.as_list_mut().unwrap()[6] =
        crate::metadata_model::brace::parse_row(indexes.as_bytes()).unwrap();
    entry
}

#[test]
fn deleting_a_field_takes_its_own_index_out_and_the_field_out_of_the_date_index() {
    // The index list of a document with an additional-order attribute (case b2): the date index lists the
    // attribute last, the attribute has an index of its own.
    let indexes = "{3,\r\n{\"ByDocNum\",1,\r\n{2,\"Number\",\"ID\"},0,0,0,\r\n{0},0,0},\r\n{\"ByDocDate\",1,\r\n{4,\"Date_Time\",\"ID\",\"Marked\",\"Fld154\"},0,0,0,\r\n{0},0,0},\r\n{\"ByFieldFld154\",1,\r\n{4,\"Fld154\",\"Date_Time\",\"ID\",\"Marked\"},0,0,0,\r\n{0},0,0}}";
    let mut entry = entry_with_indexes(indexes);
    let removed = remove_field_indexes(&mut entry, "Fld154").unwrap();
    assert_eq!(removed, ["ByDocDate (- Fld154)", "ByFieldFld154"]);
    let list = entry.as_list().unwrap()[6].clone();
    let text = crate::metadata_model::brace::serialize(&list);
    assert!(text.starts_with("{2,"), "{text}");
    assert!(text.contains("\"ByDocDate\""), "{text}");
    assert!(
        text.contains("{3,\"Date_Time\",\"ID\",\"Marked\"}"),
        "{text}"
    );
    assert!(!text.contains("Fld154"), "{text}");

    // An index of another kind that names the field is refused.
    let other = "{2,\r\n{\"ByDocNum\",1,\r\n{2,\"Number\",\"ID\"},0,0,0,\r\n{0},0,0},\r\n{\"ByOther\",1,\r\n{2,\"Fld154\",\"ID\"},0,0,0,\r\n{0},0,0}}";
    let mut refused = entry_with_indexes(other);
    assert!(
        remove_field_indexes(&mut refused, "Fld154")
            .unwrap_err()
            .to_string()
            .contains("not one of the indexes of an indexed attribute")
    );
}

/// A table entry (the one of `Reference20` of the fixture) with the given fields, all booleans, and the given
/// index list.
fn entry_with(fields: &[&str], indexes: &str) -> crate::metadata_model::brace::Brace {
    let mut entry = entry_with_indexes(indexes);
    let mut list = vec![crate::metadata_model::brace::Brace::atom(fields.len())];
    list.extend(fields.iter().map(|name| {
        FieldEntry::new(name, false, vec![TypeEntry::new("L", 0, 0, "", 0)]).to_brace()
    }));
    entry.as_list_mut().unwrap()[4] = crate::metadata_model::brace::Brace::List(list);
    entry
}

fn index_names(entry: &crate::metadata_model::brace::Brace) -> Vec<String> {
    TableView::new(entry)
        .unwrap()
        .indexes()
        .unwrap()
        .into_iter()
        .map(|index| format!("{} {}", index.name, index.fields.join(",")))
        .collect()
}

/// An index list from `(name, fields)`.
fn index_list(indexes: &[(&str, &str)]) -> String {
    let mut text = format!("{{{}", indexes.len());
    for (name, fields) in indexes {
        let count = fields.split(',').count();
        text.push_str(&format!(
            ",\r\n{{\"{name}\",1,\r\n{{{count},{}}},0,0,0,\r\n{{0}},0,0}}",
            fields
                .split(',')
                .map(|field| format!("\"{field}\""))
                .collect::<Vec<_>>()
                .join(",")
        ));
    }
    text.push('}');
    text
}

const CATALOG_FIELDS: &[&str] = &[
    "ID",
    "Version",
    "Marked",
    "PredefinedID",
    "ParentID",
    "Folder",
    "Code",
    "Description",
    "Fld1",
    "Fld2",
    "Fld3",
    "Fld4",
    "Fld5",
];
const DOCUMENT_FIELDS: &[&str] = &[
    "ID",
    "Version",
    "Marked",
    "Date_Time",
    "Number",
    "Posted",
    "Fld1",
    "Fld2",
    "Fld3",
    "Fld4",
];

#[test]
fn an_index_of_a_hierarchical_attribute_is_a_pair_in_the_order_of_the_fields() {
    let existing = index_list(&[
        ("ByPredefinedIDNotUniq", "PredefinedID"),
        ("ParentDescr", "ParentID,Folder,Description,ID"),
        ("Descr", "Description,ID"),
        ("ByParentFieldFld2", "ParentID,Folder,Fld2,ID"),
        ("ByFieldFld2", "Fld2,ID"),
        ("ByParentFieldFld5", "ParentID,Folder,Fld5,ID"),
        ("ByFieldFld5", "Fld5,ID"),
    ]);
    // Between the pairs of Fld2 and Fld5 (by the position of the field in the table).
    let mut entry = entry_with(CATALOG_FIELDS, &existing);
    let added = add_field_indexes(&mut entry, "Fld3", false).unwrap();
    assert_eq!(added, ["ByParentFieldFld3", "ByFieldFld3"]);
    assert_eq!(
        index_names(&entry),
        [
            "ByPredefinedIDNotUniq PredefinedID",
            "ParentDescr ParentID,Folder,Description,ID",
            "Descr Description,ID",
            "ByParentFieldFld2 ParentID,Folder,Fld2,ID",
            "ByFieldFld2 Fld2,ID",
            "ByParentFieldFld3 ParentID,Folder,Fld3,ID",
            "ByFieldFld3 Fld3,ID",
            "ByParentFieldFld5 ParentID,Folder,Fld5,ID",
            "ByFieldFld5 Fld5,ID",
        ]
    );
    // Before the first pair, and after the last one.
    let added = add_field_indexes(&mut entry, "Fld1", true).unwrap();
    assert_eq!(added, ["ByParentFieldFld1", "ByFieldFld1"]);
    let names = index_names(&entry);
    assert_eq!(
        &names[3..5],
        [
            "ByParentFieldFld1 ParentID,Folder,Fld1,Description,ID,Marked",
            "ByFieldFld1 Fld1,Description,ID,Marked"
        ]
    );
    // The reverse is the removal of the pair.
    let mut back = entry.clone();
    assert_eq!(
        remove_field_indexes(&mut back, "Fld1").unwrap(),
        ["ByParentFieldFld1", "ByFieldFld1"]
    );
    assert_eq!(index_names(&back).len(), names.len() - 2);
    // A table with no attribute index yet takes the pair at the end.
    let mut bare = entry_with(
        CATALOG_FIELDS,
        &index_list(&[
            ("ByPredefinedIDNotUniq", "PredefinedID"),
            ("ParentDescr", "ParentID,Folder,Description,ID"),
        ]),
    );
    add_field_indexes(&mut bare, "Fld4", false).unwrap();
    assert_eq!(index_names(&bare).len(), 4);
    assert_eq!(index_names(&bare)[3], "ByFieldFld4 Fld4,ID");
    // The count in the list follows.
    let text = crate::metadata_model::brace::serialize(&entry.as_list().unwrap()[6].clone());
    assert!(text.starts_with("{11,"), "{text}");
}

#[test]
fn an_index_of_a_flat_catalog_or_a_document_is_one_entry() {
    let flat_fields: Vec<&str> = CATALOG_FIELDS
        .iter()
        .copied()
        .filter(|name| !matches!(*name, "ParentID" | "Folder"))
        .collect();
    let mut flat = entry_with(
        &flat_fields,
        &index_list(&[
            ("ByPredefinedIDNotUniq", "PredefinedID"),
            ("Descr", "Description,ID"),
            ("ByFieldFld2", "Fld2,ID"),
        ]),
    );
    assert_eq!(
        add_field_indexes(&mut flat, "Fld1", false).unwrap(),
        ["ByFieldFld1"]
    );
    assert_eq!(
        index_names(&flat),
        [
            "ByPredefinedIDNotUniq PredefinedID",
            "Descr Description,ID",
            "ByFieldFld1 Fld1,ID",
            "ByFieldFld2 Fld2,ID"
        ]
    );

    // A document: a plain index, and the additional order that the date index lists last.
    let existing = index_list(&[
        ("ByDocNum", "Number,ID"),
        ("ByDocDate", "Date_Time,ID,Marked"),
        ("ByField5561", "Fld3,ID"),
    ]);
    let mut document = entry_with(DOCUMENT_FIELDS, &existing);
    assert_eq!(
        add_field_indexes(&mut document, "Fld4", false).unwrap(),
        ["ByFieldFld4"]
    );
    assert_eq!(
        add_field_indexes(&mut document, "Fld1", true).unwrap(),
        ["ByFieldFld1", "ByDocDate (+ Fld1)"]
    );
    assert_eq!(
        index_names(&document),
        [
            "ByDocNum Number,ID",
            "ByDocDate Date_Time,ID,Marked,Fld1",
            "ByFieldFld1 Fld1,Date_Time,ID,Marked",
            "ByField5561 Fld3,ID",
            "ByFieldFld4 Fld4,ID"
        ]
    );
    // A second additional-order attribute is not traced; neither is removing the first out of order.
    assert!(
        add_field_indexes(&mut document, "Fld2", true)
            .unwrap_err()
            .to_string()
            .contains("several are not traced")
    );
    // The reverse of the additional order is the removal of the entry and of the tail.
    assert_eq!(
        remove_field_indexes(&mut document, "Fld1").unwrap(),
        ["ByDocDate (- Fld1)", "ByFieldFld1"]
    );
    assert_eq!(index_names(&document)[1], "ByDocDate Date_Time,ID,Marked");
}

#[test]
fn an_index_that_the_platform_was_not_traced_on_is_refused() {
    let hierarchical = index_list(&[
        ("ByPredefinedIDNotUniq", "PredefinedID"),
        ("Descr", "Description,ID"),
    ]);
    // A hierarchical catalog without the parent index to take the prefix from.
    let mut entry = entry_with(CATALOG_FIELDS, &hierarchical);
    assert!(
        add_field_indexes(&mut entry, "Fld1", false)
            .unwrap_err()
            .to_string()
            .contains("ParentDescr")
    );
    // A field that is not there, one that is indexed already, a subordinate catalog.
    let mut entry = entry_with(
        CATALOG_FIELDS,
        &index_list(&[
            ("ParentDescr", "ParentID,Folder,Description,ID"),
            ("ByParentFieldFld1", "ParentID,Folder,Fld1,ID"),
            ("ByFieldFld1", "Fld1,ID"),
        ]),
    );
    assert!(add_field_indexes(&mut entry, "Fld99", false).is_err());
    assert!(
        add_field_indexes(&mut entry, "Fld1", false)
            .unwrap_err()
            .to_string()
            .contains("already names")
    );
    let mut owned: Vec<&str> = CATALOG_FIELDS.to_vec();
    owned.push("OwnerID");
    let mut entry = entry_with(
        &owned,
        &index_list(&[("ParentDescr", "ParentID,Folder,Description,ID")]),
    );
    assert!(
        add_field_indexes(&mut entry, "Fld1", false)
            .unwrap_err()
            .to_string()
            .contains("subordinate")
    );
}

#[test]
fn a_subtable_entry_is_the_one_the_platform_stores() {
    // Both sub-tables of the fixture's catalog are rebuilt from their parts: the line number field, the fields,
    // the declared indexes (not unique).
    let schema = DbSchema::parse(STAGED).unwrap();
    let object = schema.tables()[schema.position("Reference20").unwrap()].clone();
    let view = TableView::new(&object).unwrap();
    for stored in view.subtables().unwrap() {
        let fields = stored.fields().unwrap();
        let line = fields[0].name.clone();
        let indexes: Vec<(String, Vec<String>)> = stored
            .indexes()
            .unwrap()
            .into_iter()
            .map(|index| {
                assert!(!index.unique && !index.clustered);
                (index.name, index.fields)
            })
            .collect();
        let built = subtable_entry(stored.name(), "Reference20", &line, &fields[1..], &indexes);
        let position = view
            .subtables()
            .unwrap()
            .iter()
            .position(|other| other.name() == stored.name())
            .unwrap();
        let original = object.as_list().unwrap()[5].as_list().unwrap()[1 + position].clone();
        assert_eq!(built, original, "{}", stored.name());
    }
}

#[test]
fn a_subtable_is_pushed_to_the_list_and_counted() {
    let schema = DbSchema::parse(STAGED).unwrap();
    let mut object = schema.tables()[schema.position("Reference20").unwrap()].clone();
    let before = TableView::new(&object).unwrap().subtables().unwrap().len();
    let fields = [FieldEntry::new(
        "Fld9001",
        false,
        vec![TypeEntry::new("S", 0x8000_0000 | 10, 0, "", 0)],
    )];
    let indexes = [(
        "ByFieldFld9001".to_owned(),
        vec!["Fld9001".to_owned(), "ID".to_owned()],
    )];
    push_subtable(
        &mut object,
        subtable_entry("VT9000", "Reference20", "LineNo9002", &fields, &indexes),
    )
    .unwrap();
    let view = TableView::new(&object).unwrap();
    let subtables = view.subtables().unwrap();
    assert_eq!(subtables.len(), before + 1);
    let added = subtables.last().unwrap();
    assert_eq!(
        (added.name(), added.kind(), added.owner_name()),
        ("VT9000", "I", "Reference20")
    );
    assert_eq!(
        added
            .fields()
            .unwrap()
            .iter()
            .map(|f| f.name.as_str())
            .collect::<Vec<_>>(),
        ["LineNo9002", "Fld9001"]
    );
    assert_eq!(added.indexes().unwrap()[0].fields, ["Fld9001", "ID"]);
    // The physical table follows: the owner key and the separator first, the line number last of the keys.
    let tables = physical_tables(&view).unwrap();
    let created = tables.last().unwrap();
    assert_eq!(created.name, "_Reference20_VT9000");
    let columns: Vec<&str> = created.columns.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(
        columns,
        [
            "_Reference20_IDRRef",
            "_Fld2683",
            "_KeyField",
            "_LineNo9002",
            "_Fld9001"
        ]
    );
    assert!(
        created
            .indexes
            .iter()
            .any(|index| index.columns == ["_Fld2683", "_Fld9001", "_Reference20_IDRRef"])
    );
}
