//! The plan of case a2 against what the native apply did (fixtures cut from the lab twin).

pub(super) use super::test_plan_support::*;

use crate::metadata_model::brace::parse_row;
use crate::restructure::names::{DbNames, deflate, inflate};
use crate::restructure::plan::{Inputs, Phase, plan};
use crate::restructure::schema::{DbSchema, SqlType};

const FIXTURES: &str = "tests/fixtures/native-evidence/restructure";

const NATIVE_SCHEMA: &[u8] = include_bytes!(
    "../../../../tests/fixtures/native-evidence/restructure/dbschema-a2-native-excerpt.txt"
);

const STATEMENTS: &str =
    include_str!("../../../../docs/apply/evidence/restructuring/a2-structure-statements.sql");

fn error_of(inputs: &Inputs) -> String {
    format!("{:#}", plan(inputs, &options()).unwrap_err())
}

fn lf(text: &str) -> String {
    text.replace("\r\n", "\n")
}

#[test]
fn the_fixtures_are_where_the_tests_expect_them() {
    assert!(
        std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
            .join(FIXTURES)
            .join("catalog-reference20-new.deflate")
            .exists()
    );
}

#[test]
fn plans_case_a2_like_the_platform_did() {
    let plan = plan(&inputs(OLD_ROW, NEW_ROW), &options()).unwrap();
    assert_eq!(plan.objects.len(), 1);
    assert_eq!(plan.objects[0].object, "Reference20");
    assert_eq!(plan.objects[0].additions.len(), 1);
    let addition = &plan.objects[0].additions[0];
    assert_eq!(addition.uuid, "c60cdc87-198a-4f6e-8f17-76bcb1b1914b");
    assert_eq!(addition.name, "ДемоНовыйРеквизит");
    // The counter is shared with the extensions: 11033 there, 10824 here.
    assert_eq!(addition.number, 11034);
    assert_eq!(addition.field.name, "Fld11034");
    assert!(addition.field.nullable);
    assert_eq!(addition.position, 14);

    // The new entry is the platform's, byte for byte; ConfigChngR stays last.
    let ours = DbSchema::parse(&plan.new_schema).unwrap();
    let native = DbSchema::parse(NATIVE_SCHEMA).unwrap();
    assert_eq!(
        ours.tables()[ours.position("Reference20").unwrap()],
        native.tables()[native.position("Reference20").unwrap()]
    );
    assert_eq!(ours.len(), 4);
    assert_eq!(ours.position("ConfigChngR"), Some(3));
    assert_eq!(ours.position("Reference20"), Some(2));

    // DBNames: one more entry, the header moved to it.
    let names = DbNames::parse(&plan.new_names_text).unwrap();
    assert_eq!(names.max, 11034);
    let last = names.entries.last().unwrap();
    assert_eq!(
        (last.uuid.as_str(), last.kind.as_str(), last.number),
        ("c60cdc87-198a-4f6e-8f17-76bcb1b1914b", "Fld", 11034)
    );
    assert_eq!(inflate(&plan.new_names_row).unwrap(), plan.new_names_text);
    assert_eq!(
        crate::restructure::names::parse_version(&plan.names_version_row).unwrap(),
        VERSION
    );
}

#[test]
fn the_statements_are_the_platforms_for_the_object() {
    let plan = plan(&inputs(OLD_ROW, NEW_ROW), &options()).unwrap();
    let statements = plan.statements();
    let trace = lf(STATEMENTS);

    // Everything the platform executed verbatim: create table, create index, drop, rename.
    for statement in statements.iter().filter(|s| {
        matches!(
            s.phase,
            Phase::Create | Phase::Indexes | Phase::DropOld | Phase::Rename
        )
    }) {
        if statement.sql.starts_with("drop table") {
            assert!(trace.contains(&statement.sql), "{}", statement.sql);
        } else if let Some(rename) = statement.sql.strip_prefix("EXEC sp_rename ") {
            // The trace passes the names as parameters; the arguments are the same.
            let arguments: Vec<&str> = rename.trim_end_matches(';').split(", ").collect();
            let first = arguments[0].trim_matches(|c| c == 'N' || c == '\'');
            let second = arguments[1].trim_matches(|c| c == 'N' || c == '\'');
            let needle = format!("-- args: N'{first}',N'{second}'");
            assert!(trace.contains(&needle), "{needle}");
        } else {
            assert!(
                trace.contains(&statement.sql),
                "not in the trace:\n{}",
                statement.sql
            );
        }
    }

    // The copy: the platform's statement with its parameters written as literals.
    let copies: Vec<_> = statements
        .iter()
        .filter(|s| s.phase == Phase::Load)
        .collect();
    assert_eq!(copies.len(), 3);
    for copy in copies {
        let with_parameter = copy.sql.replace("N''", "@P1");
        assert!(
            trace.contains(&with_parameter),
            "not in the trace:\n{with_parameter}"
        );
    }

    // The guard comes first, the publication last.
    assert_eq!(statements.first().unwrap().phase, Phase::Guard);
    assert_eq!(statements.last().unwrap().label, "ConfigSave: emptied");
    let publish: Vec<_> = statements
        .iter()
        .filter(|s| s.phase == Phase::Publish)
        .map(|s| s.label.as_str())
        .collect();
    assert_eq!(
        publish,
        [
            "SchemaStorage: idle, the new schema, empty generations",
            "DBSchema",
            "Params DBNames",
            "Params DBNamesVersion-DBNames",
            "Config: the changed staged rows replace the stored ones",
            "ConfigSave: emptied"
        ]
    );
}

#[test]
fn the_phase_text_is_the_statements_with_assertions() {
    let plan = plan(&inputs(OLD_ROW, NEW_ROW), &options()).unwrap();
    let text = plan.phase_sql("@now").unwrap();
    let trace = lf(STATEMENTS);
    // The DDL is the platform's, verbatim, in the platform's order.
    let mut from = 0;
    for statement in plan
        .statements()
        .iter()
        .filter(|s| matches!(s.phase, Phase::Create | Phase::Indexes | Phase::DropOld))
    {
        assert!(trace.contains(&statement.sql), "{}", statement.sql);
        let at = text[from..]
            .find(&statement.sql)
            .unwrap_or_else(|| panic!("not in the phase text, in order:\n{}", statement.sql));
        from += at + statement.sql.len();
    }
    // Every rename, table and index.
    assert_eq!(text.matches("EXEC sp_rename").count(), 13);
    // No parameter marker: the values are literals.
    assert!(!text.contains("@P1"), "a parameter marker");
    // The guards and the assertions.
    for number in [57400, 57401, 57402, 57403, 57404, 57405] {
        assert!(text.contains(&format!("THROW {number},")), "{number}");
    }
    // The publication uses the caller's timestamp and touches neither Config nor the caches.
    assert!(text.contains("Modified = @now WHERE FileName = N'DBNames'"));
    assert!(!text.contains("dbo.Config"));
    assert!(!text.contains(".si"));
    // The old rows the guards compare with are the plan's inputs.
    assert!(text.contains(&plan.old_schema_sha256.to_ascii_uppercase()));
    assert!(text.contains(&plan.old_names_sha256.to_ascii_uppercase()));
    // The alter method is not for the apply.
    let mut alter = plan.clone();
    alter.method = crate::restructure::plan::Method::AlterAdd;
    assert!(alter.phase_sql("@now").is_err());
}

#[test]
fn the_phase_text_asserts_that_what_the_extensions_keep_is_untouched() {
    let plan = plan(&inputs(OLD_ROW, NEW_ROW), &options()).unwrap();
    let text = plan.phase_sql("@now").unwrap();
    // The fingerprint is taken before the first guard and compared after the last publication step.
    let before = text.find("SET @ext_before").expect("the state before");
    let guards = text.find("-- restructure: guards").unwrap();
    let publication = text.find("-- restructure: publication").unwrap();
    let after = text.find("SET @ext_after").expect("the state after");
    assert!(before < guards && guards < publication && publication < after);
    assert!(text.contains("IF @ext_after <> @ext_before THROW 57406,"));
    // What it reads: the extension schema, the extensions' DBNames rows, the registry and its bookkeeping,
    // the X1 tables; nothing of the main configuration.
    for part in [
        "SchemaID <> 0",
        "N'DBNames%-Ext-%'",
        "dbo._ExtensionsInfo",
        "dbo._ExtensionsRestruct",
        "dbo._ExtensionsRestructNGS",
        "LIKE N'%X1'",
    ] {
        assert!(text.contains(part), "{part}");
    }
}

/// The state after case a2 (the attribute is there) and the stage that removes it again.
fn removing_a2() -> Inputs {
    let added = plan(&inputs(OLD_ROW, NEW_ROW), &options()).unwrap();
    let mut back = inputs(NEW_ROW, OLD_ROW);
    back.schema = added.new_schema.clone();
    back.main_names = added.new_names_row.clone();
    back
}

#[test]
fn removing_the_attribute_of_case_a2_gives_the_stored_entry_back() {
    let plan = plan(&removing_a2(), &options()).unwrap();
    assert_eq!(plan.objects.len(), 1);
    let object = &plan.objects[0];
    assert_eq!(object.object, "Reference20");
    assert!(object.additions.is_empty());
    assert_eq!(object.removals.len(), 1);
    let removal = &object.removals[0];
    assert_eq!(removal.name, "ДемоНовыйРеквизит");
    assert_eq!(removal.field.name, "Fld11034");
    assert_eq!(removal.number, 11034);
    assert!(removal.indexes.is_empty());
    assert!(
        object
            .changes()
            .contains("removed attributes Fld11034 = ДемоНовыйРеквизит")
    );

    // The entry is the one the schema had before the attribute: adding and removing are inverse.
    let ours = DbSchema::parse(&plan.new_schema).unwrap();
    let before = DbSchema::parse(STAGED_SCHEMA).unwrap();
    assert_eq!(
        ours.tables()[ours.position("Reference20").unwrap()],
        before.tables()[before.position("Reference20").unwrap()]
    );
    // Every table of the schema before is there unchanged (the rebuilt one moved before ConfigChngR).
    assert_eq!(ours.len(), before.len());
    for position in 0..before.len() {
        let name = before.view(position).unwrap().name().to_owned();
        assert_eq!(
            ours.tables()[ours.position(&name).unwrap()],
            before.tables()[position],
            "{name}"
        );
    }
    // The names keep the entry and the number: DBNames is what it was.
    let added = super::plan::plan(&inputs(OLD_ROW, NEW_ROW), &options()).unwrap();
    assert_eq!(plan.new_names_text, added.new_names_text);

    // The rebuilt table has no column of the attribute, and the copy does not read it.
    let table = &plan.objects[0].tables[0];
    assert!(
        table
            .table
            .columns
            .iter()
            .all(|column| column.name != "_Fld11034")
    );
    assert!(
        table
            .old_columns
            .iter()
            .any(|column| column.name == "_Fld11034")
    );
    assert!(
        table
            .insert_columns
            .iter()
            .all(|column| column != "_Fld11034")
    );
    assert!(
        table
            .insert_values
            .iter()
            .all(|value| !value.contains("_Fld11034"))
    );
    // The statements are the platform's for the rebuild: create, copy, indexes, drop, rename.
    let statements = plan.statements();
    assert_eq!(
        statements.iter().filter(|s| s.phase == Phase::Load).count(),
        3
    );
    assert!(!statements.iter().any(|s| s.sql.contains("_Fld11034NG")));
}

#[test]
fn a_retyped_attribute_is_refused_in_a_removal_stage() {
    // The removal of one attribute and a change of another one's type is refused.
    let text = String::from_utf8(inflate(NEW_ROW).unwrap()).unwrap();
    let start = text.find("\"Клиент\"").unwrap();
    let pattern = start + text[start..].find("{\"Pattern\"").unwrap();
    let mut changed = text.clone();
    changed.replace_range(
        pattern..pattern + "{\"Pattern\",\r\n{\"B\"}".len(),
        "{\"Pattern\",\r\n{\"S\",10,1}",
    );
    let mut back = removing_a2();
    back.staged.new_descriptors =
        [(CATALOG.to_owned(), deflate(changed.as_bytes()).unwrap())].into();
    assert!(error_of(&back).contains("changes its type"));
}

#[test]
fn the_deleted_row_may_also_name_the_rows_of_a_pending_online_update() {
    let mut back = removing_a2();
    // What the platform's import writes for a target with an online update pending, and this
    // program's too: the attribute (flag 1) and the update's Config rows (flag 0).
    let update =
        "a627e390-8fad-4a95-afe6-674f54813188_dynupdate_06cb0442-0c47-4fad-986a-f08f28287c1b";
    let list = format!(
        "\u{feff}5,\"c60cdc87-198a-4f6e-8f17-76bcb1b1914b\",1,\"{update}\",0,\"{update}.0\",0,\
         \"DynamicallyUpdated\",0,\"versions_dynupdate_06cb0442-0c47-4fad-986a-f08f28287c1b\",0"
    );
    back.staged.deleted = Some(deflate(list.as_bytes()).unwrap());
    assert!(plan(&back, &options()).is_ok());
    // Only as Config rows: the same name with the flag of an element is not one.
    back.staged.deleted = Some(deflate(format!("\u{feff}1,\"{update}\",1").as_bytes()).unwrap());
    assert!(error_of(&back).contains("deletes a627e390"));
    // And no other row: an update row is named by its `_dynupdate_` mark.
    back.staged.deleted =
        Some(deflate("\u{feff}1,\"aaaaaaaa-0000-4000-8000-000000000000\",0".as_bytes()).unwrap());
    assert!(error_of(&back).contains("deletes aaaaaaaa"));
}

#[test]
fn the_deleted_row_of_a_removal_names_the_removed_attribute() {
    let mut back = removing_a2();
    // The platform's row for the removal: the id of the attribute, flag 1.
    let list = "\u{feff}1,\"c60cdc87-198a-4f6e-8f17-76bcb1b1914b\",1";
    back.staged.deleted = Some(deflate(list.as_bytes()).unwrap());
    assert!(plan(&back, &options()).is_ok());
    // Uppercase ids, several parts of the list, the same result; another id is refused.
    back.staged.deleted = Some(deflate(list.to_uppercase().as_bytes()).unwrap());
    assert!(plan(&back, &options()).is_ok());
    back.staged.deleted =
        Some(deflate("\u{feff}1,\"c60cdc87-198a-4f6e-8f17-76bcb1b1914c\",1".as_bytes()).unwrap());
    assert!(error_of(&back).contains("deletes c60cdc87-198a-4f6e-8f17-76bcb1b1914c"));
    back.staged.deleted =
        Some(deflate("\u{feff}1,\"c60cdc87-198a-4f6e-8f17-76bcb1b1914b\",0".as_bytes()).unwrap());
    assert!(error_of(&back).contains("the flag 0"));
    // An empty list is fine for any stage.
    back.staged.deleted = Some(deflate("\u{feff}0".as_bytes()).unwrap());
    assert!(plan(&back, &options()).is_ok());
    assert_eq!(
        crate::restructure::plan::parse_deleted(&deflate(list.as_bytes()).unwrap()).unwrap(),
        [("c60cdc87-198a-4f6e-8f17-76bcb1b1914b".to_owned(), 1)]
    );
}

fn widened_client() -> Vec<u8> {
    widen_client(
        &client_as_string(OLD_ROW, 50),
        "{\"S\",50,1}",
        "{\"S\",200,1}",
    )
}

#[test]
fn widening_a_string_changes_the_type_of_its_field_only() {
    let staged = string_client_inputs(&widened_client());
    let plan = plan(&staged, &options()).unwrap();
    assert_eq!(plan.objects.len(), 1);
    let object = &plan.objects[0];
    assert_eq!(object.object, "Reference20");
    assert!(object.additions.is_empty() && object.removals.is_empty());
    assert_eq!(object.widenings.len(), 1);
    let widening = &object.widenings[0];
    assert_eq!(widening.name, "Клиент");
    assert_eq!(widening.after.name, "Fld151");
    assert_eq!((widening.from, widening.to), (50, 200));
    assert_eq!(widening.before.types[0].a, 0x8000_0000 | 50);
    assert_eq!(widening.after.types[0].a, 0x8000_0000 | 200);
    assert_eq!(
        object.changes(),
        "widened attributes Fld151 = Клиент (50 -> 200)"
    );

    // The entry is the stored one with that one type entry replaced: the fields keep their places, the
    // indexes stay, DBNames is what it was, no cache is touched.
    let ours = DbSchema::parse(&plan.new_schema).unwrap();
    let before = DbSchema::parse(&staged.schema).unwrap();
    let fields = |schema: &DbSchema| {
        schema
            .view(schema.position("Reference20").unwrap())
            .unwrap()
            .fields()
            .unwrap()
    };
    let (stored, now) = (fields(&before), fields(&ours));
    assert_eq!(stored.len(), now.len());
    for (was, is) in stored.iter().zip(&now) {
        if was.name == "Fld151" {
            assert_eq!(is, &widening.after);
            assert_ne!(was, is);
        } else {
            assert_eq!(was, is);
        }
    }
    assert_eq!(
        plan.new_names_text,
        DbNames::parse(NAMES).unwrap().to_text(),
        "DBNames does not change"
    );
    assert!(plan.caches.is_empty());

    // The rebuilt tables: the column is longer, the copy reads it as it is, the indexes are the same.
    let rebuilt = &object.tables[0];
    let (was, is) = (
        rebuilt
            .old_columns
            .iter()
            .find(|c| c.name == "_Fld151")
            .unwrap(),
        rebuilt
            .table
            .columns
            .iter()
            .find(|c| c.name == "_Fld151")
            .unwrap(),
    );
    assert_eq!(was.sql_type, SqlType::NVarChar(50));
    assert_eq!(is.sql_type, SqlType::NVarChar(200));
    assert!(rebuilt.insert_values.contains(&"T1._Fld151".to_owned()));
    let statements = plan.statements();
    assert_eq!(
        statements.iter().filter(|s| s.phase == Phase::Load).count(),
        3
    );
    assert!(
        statements
            .iter()
            .any(|s| s.sql.contains("_Fld151 nvarchar(200)"))
    );
}

#[test]
fn a_widening_can_come_with_a_new_attribute() {
    // The staged row is the one of case a2 (a new attribute) with "Клиент" widened.
    let new = widen_client(
        &client_as_string(NEW_ROW, 50),
        "{\"S\",50,1}",
        "{\"S\",60,1}",
    );
    let plan = plan(&string_client_inputs(&new), &options()).unwrap();
    let object = &plan.objects[0];
    assert_eq!((object.additions.len(), object.widenings.len()), (1, 1));
    assert_eq!(
        object.changes(),
        "new attributes Fld11034 = ДемоНовыйРеквизит; widened attributes Fld151 = Клиент (50 -> 60)"
    );
}

#[test]
fn only_a_longer_limit_of_a_variable_string_is_a_widening() {
    let refusal = |to: &str| {
        let staged = widen_client(&client_as_string(OLD_ROW, 50), "{\"S\",50,1}", to);
        error_of(&string_client_inputs(&staged))
    };
    // A shorter limit, an unlimited string, a fixed one, a string that becomes another type: every one is
    // refused with its reason, and every reason says that the type changes.
    for (to, why) in [
        ("{\"S\",20,1}", "the limit is shorter"),
        ("{\"S\"}", "an unlimited string"),
        ("{\"S\",50,0}", "a fixed string"),
        ("{\"S\",200,0}", "a fixed string"),
        ("{\"N\",10,2,0}", "not a string"),
        ("{\"B\"}", "not a string"),
    ] {
        let text = refusal(to);
        assert!(text.contains(why), "{to}: {text}");
        assert!(
            text.contains("attribute Клиент changes its type"),
            "{to}: {text}"
        );
    }
    // The stored limit is the same but the text differs (a fixed string of the same length is not this
    // either): the refusal names it.
    let same = widen_client(
        &client_as_string(OLD_ROW, 50),
        "{\"S\",50,1}",
        "{\"S\",50,0}",
    );
    assert!(error_of(&string_client_inputs(&same)).contains("a fixed string"));
    // A string that was unlimited: the widening does not apply.
    let unlimited = widen_client(&client_as_string(OLD_ROW, 50), "{\"S\",50,1}", "{\"S\"}");
    assert!(error_of(&string_client_inputs(&unlimited)).contains("an unlimited string"));
}

#[test]
fn a_widening_is_refused_when_the_stored_field_disagrees() {
    // The schema holds a field of another length than the metadata says.
    let mut tampered = string_client_inputs(&widened_client());
    let schema = String::from_utf8(tampered.schema.clone()).unwrap();
    let wrong = schema.replacen(
        &format!("{{\"S\",{},0,\"\",0}}", 0x8000_0000u64 | 50),
        &format!("{{\"S\",{},0,\"\",0}}", 0x8000_0000u64 | 51),
        1,
    );
    assert_ne!(wrong, schema);
    tampered.schema = wrong.into_bytes();
    assert!(error_of(&tampered).contains("differs from what the generator makes"));
}

#[test]
fn a_widening_of_a_tabular_section_attribute_is_refused() {
    // The first limited string of the fixture sits in a tabular section: an attribute of an old section
    // may not change its type (only new ones join).
    let staged = retype_after(OLD_ROW, "", "{\"S\",50,1}", "{\"S\",60,1}");
    assert!(
        error_of(&inputs(OLD_ROW, &staged))
            .contains("removes, moves or changes an attribute of the tabular section")
    );
}

#[test]
fn a_partial_stage_is_planned_like_a_whole_one() {
    // `import files --partial` stages the changed descriptor and root, version, versions only.
    let mut partial = inputs(OLD_ROW, NEW_ROW);
    partial.staged.old_files = ["root", "version", "versions", "other-file", CATALOG]
        .map(str::to_owned)
        .into();
    partial.staged.new_files = ["root", "version", "versions", CATALOG]
        .map(str::to_owned)
        .into();
    partial.staged.deleted = None;
    let whole = plan(&inputs(OLD_ROW, NEW_ROW), &options()).unwrap();
    let delta = plan(&partial, &options()).unwrap();
    assert_eq!(delta.new_schema, whole.new_schema);
    assert_eq!(delta.new_names_text, whole.new_names_text);
}

#[test]
fn refuses_what_it_cannot_prove() {
    let base = inputs(OLD_ROW, NEW_ROW);

    // No attribute is new.
    assert!(error_of(&inputs(OLD_ROW, OLD_ROW)).contains("adds no attribute"));

    // An attribute is removed that DBNames has no number for (the images swapped).
    assert!(error_of(&inputs(NEW_ROW, OLD_ROW)).contains("has no number"));

    // An object is added.
    let mut more = base.clone();
    more.staged
        .new_files
        .insert("aaaaaaaa-0000-4000-8000-000000000000".to_owned());
    assert!(error_of(&more).contains("adds 1"));

    // The deleted marker names something that is not an attribute the stage removes.
    let mut deleting = base.clone();
    deleting.staged.deleted =
        Some(deflate("\u{feff}1,\"aaaaaaaa-0000-4000-8000-000000000000\",1".as_bytes()).unwrap());
    assert!(error_of(&deleting).contains("deletes aaaaaaaa"));
    // ... or has a list that does not parse.
    deleting.staged.deleted = Some(deflate("\u{feff}1".as_bytes()).unwrap());
    assert!(error_of(&deleting).contains("deleted row"));

    // A stored attribute changes its type.
    let text = String::from_utf8(inflate(NEW_ROW).unwrap()).unwrap();
    let start = text.find("\"Клиент\"").unwrap();
    let pattern = start + text[start..].find("{\"Pattern\"").unwrap();
    let mut changed = text.clone();
    changed.replace_range(
        pattern..pattern + "{\"Pattern\",\r\n{\"B\"}".len(),
        "{\"Pattern\",\r\n{\"S\",10,1}",
    );
    assert_ne!(changed, text);
    assert!(parse_row(changed.as_bytes()).is_ok());
    let retyped = inputs(OLD_ROW, &deflate(changed.as_bytes()).unwrap());
    assert!(error_of(&retyped).contains("changes its type"));

    // The stored schema disagrees with what the generator makes of the attribute.
    let schema = String::from_utf8(STAGED_SCHEMA.to_vec()).unwrap();
    let at = schema.find("\"Fld151\"").unwrap();
    let mut wrong = schema.clone();
    let boolean = at + wrong[at..].find("{\"L\",0,0,\"\",0}").unwrap();
    wrong.replace_range(
        boolean..boolean + "{\"L\",0,0,\"\",0}".len(),
        "{\"S\",10,0,\"\",0}",
    );
    let mut tampered = base.clone();
    tampered.schema = wrong.into_bytes();
    assert!(error_of(&tampered).contains("differs from what the generator makes"));

    // A companion table of the object exists: the change registration table and an empty
    // predefined-data table are harmless (case a2), a predefined-data table with rows is not.
    let mut with_companion = base.clone();
    let mut extra = DbSchema::parse(STAGED_SCHEMA).unwrap();
    let copy = extra.tables()[extra.position("DbCopies").unwrap()].clone();
    let renamed = String::from_utf8(DbSchema::from_tables(vec![copy]).to_text())
        .unwrap()
        .replace("\"DbCopies\"", "\"RefSInf4877\"");
    let companion = DbSchema::parse(renamed.as_bytes()).unwrap().tables()[0].clone();
    extra.insert_before("ConfigChngR", companion);
    with_companion.schema = extra.to_text();
    assert!(plan(&with_companion, &options()).is_ok());
    with_companion
        .predefined_tables
        .insert("RefSInf4877".to_owned());
    assert!(error_of(&with_companion).contains("companion table"));
}

#[test]
fn switching_the_index_on_adds_the_entries_of_the_attribute_and_nothing_else() {
    for (mode, tail, columns) in [
        (
            1u8,
            vec!["Fld151", "ID"],
            vec!["_Fld2683", "_ParentIDRRef", "_Folder", "_Fld151", "_IDRRef"],
        ),
        (
            2u8,
            vec!["Fld151", "Description", "ID", "Marked"],
            vec![
                "_Fld2683",
                "_ParentIDRRef",
                "_Folder",
                "_Fld151",
                "_Description",
                "_IDRRef",
                "_Marked",
            ],
        ),
    ] {
        let indexed = client_indexing(OLD_ROW, 0, mode);
        let plan = plan(&inputs(OLD_ROW, &indexed), &options()).unwrap();
        assert_eq!(plan.objects.len(), 1);
        let object = &plan.objects[0];
        assert!(
            object.additions.is_empty()
                && object.removals.is_empty()
                && object.widenings.is_empty()
        );
        assert_eq!(object.switches.len(), 1);
        let switch = &object.switches[0];
        assert_eq!(
            (switch.name.as_str(), switch.field.as_str()),
            ("Клиент", "Fld151")
        );
        assert_eq!((switch.from, switch.to), (0, i64::from(mode)));
        assert_eq!(switch.added, ["ByParentFieldFld151", "ByFieldFld151"]);
        assert!(switch.removed.is_empty());
        assert_eq!(
            object.changes(),
            format!(
                "switched indexes Fld151 = Клиент (DontIndex -> {})",
                if mode == 1 {
                    "Index"
                } else {
                    "IndexWithAdditionalOrder"
                }
            )
        );
        // The entry: the stored one with the pair after the standard indexes; the fields and DBNames are
        // what they were; no cache is written; all three tables are rebuilt.
        let ours = DbSchema::parse(&plan.new_schema).unwrap();
        let view = ours.view(ours.position("Reference20").unwrap()).unwrap();
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
                "ByParentFieldFld151",
                "ByFieldFld151"
            ]
        );
        assert_eq!(indexes[6].fields, tail);
        let before = DbSchema::parse(STAGED_SCHEMA).unwrap();
        let stored = before
            .view(before.position("Reference20").unwrap())
            .unwrap();
        assert_eq!(view.fields().unwrap(), stored.fields().unwrap());
        assert_eq!(
            plan.new_names_text,
            DbNames::parse(NAMES).unwrap().to_text(),
            "DBNames does not change"
        );
        assert!(plan.caches.is_empty());
        assert_eq!(object.tables.len(), 3);
        // The main table has the two indexes more, in the order of the entry; the copy is the same.
        let main = &object.tables[0];
        assert!(
            main.table
                .indexes
                .iter()
                .any(|index| index.columns == columns),
            "{:?}",
            main.table.indexes
        );
        assert_eq!(main.insert_columns.len(), main.old_columns.len() - 1);
    }
}

#[test]
fn switching_the_index_off_is_the_reverse_of_switching_it_on() {
    let indexed = client_indexing(OLD_ROW, 0, 1);
    let on = plan(&inputs(OLD_ROW, &indexed), &options()).unwrap();
    let mut back = inputs(&indexed, OLD_ROW);
    back.schema = on.new_schema.clone();
    let off = plan(&back, &options()).unwrap();
    let switch = &off.objects[0].switches[0];
    assert_eq!((switch.from, switch.to), (1, 0));
    assert_eq!(switch.removed, ["ByParentFieldFld151", "ByFieldFld151"]);
    // The entry is the stored one again (the rebuilt table moves before ConfigChngR, so compare by name).
    let ours = DbSchema::parse(&off.new_schema).unwrap();
    let before = DbSchema::parse(STAGED_SCHEMA).unwrap();
    assert_eq!(
        ours.tables()[ours.position("Reference20").unwrap()],
        before.tables()[before.position("Reference20").unwrap()]
    );

    // An attribute the metadata says is indexed but the stored table has no index for.
    let mut lying = inputs(&indexed, OLD_ROW);
    lying.schema = STAGED_SCHEMA.to_vec();
    assert!(error_of(&lying).contains("has no index for Fld151"));
}

#[test]
fn a_switch_the_platform_was_not_traced_on_is_refused() {
    // Index <-> IndexWithAdditionalOrder.
    let index = client_indexing(OLD_ROW, 0, 1);
    let additional = client_indexing(OLD_ROW, 0, 2);
    let mut both = inputs(&index, &additional);
    both.schema = plan(&inputs(OLD_ROW, &index), &options())
        .unwrap()
        .new_schema;
    assert!(
        error_of(&both)
            .contains("only DontIndex <-> Index and DontIndex <-> IndexWithAdditionalOrder")
    );

    // The use of the attribute changes with the index.
    let text = String::from_utf8(inflate(OLD_ROW).unwrap()).unwrap();
    let start = text.find("\"Клиент\"").unwrap();
    let old = "{\"B\",1},1,0,0},0,0,1,1}";
    let at = start + text[start..].find(old).unwrap();
    let mut changed = text.clone();
    changed.replace_range(at..at + old.len(), "{\"B\",1},1,0,0},1,1,1,1}");
    let both_changed = deflate(changed.as_bytes()).unwrap();
    assert!(error_of(&inputs(OLD_ROW, &both_changed)).contains("changes its use or flags"));

    // A table the index generator does not know: the hierarchical catalog without its parent index.
    let mut bare = inputs(OLD_ROW, &index);
    let schema = String::from_utf8(STAGED_SCHEMA.to_vec()).unwrap();
    bare.schema = schema
        .replace("\"ParentDescr\"", "\"ParentDescription\"")
        .into_bytes();
    assert!(error_of(&bare).contains("ParentDescr"));
}

#[test]
fn a_created_table_is_made_in_the_new_generation_and_never_copied_or_dropped() {
    let mut planned = plan(&inputs(OLD_ROW, NEW_ROW), &options()).unwrap();
    // The last sub-table of case a2 stands for a table that does not exist yet.
    let name = {
        let object = &mut planned.objects[0];
        let last = object.tables.pop().unwrap();
        let name = last.table.name.clone();
        object
            .tables
            .push(crate::restructure::plan::TablePlan::created(last.table));
        name
    };
    assert!(planned.objects[0].tables.last().unwrap().create);
    assert!(
        planned.objects[0]
            .tables
            .last()
            .unwrap()
            .insert_columns
            .is_empty()
    );

    let statements = planned.statements();
    let labels: Vec<&str> = statements.iter().map(|s| s.label.as_str()).collect();
    assert!(labels.contains(&format!("create {name}NG").as_str()));
    assert!(labels.contains(&format!("rename {name}NG").as_str()));
    assert!(!labels.contains(&format!("copy {name} into {name}NG").as_str()));
    assert!(!labels.contains(&format!("drop {name}").as_str()));
    // The other two tables are copied and dropped as before.
    assert_eq!(
        statements.iter().filter(|s| s.phase == Phase::Load).count(),
        2
    );
    assert_eq!(
        statements
            .iter()
            .filter(|s| s.phase == Phase::DropOld)
            .count(),
        2
    );

    let sql = planned.phase_sql("@now").unwrap();
    assert!(sql.contains(&format!("create table dbo.{name}NG")));
    assert!(!sql.contains(&format!("INSERT INTO dbo.{name}NG")));
    assert!(!sql.contains(&format!("drop table dbo.{name};")));
    assert!(sql.contains(&format!("EXEC sp_rename N'{name}NG', N'{name}', 'OBJECT';")));
    assert!(sql.contains(&format!("the table {name} to create exists already")));
    assert!(sql.contains(&format!("the created table {name} is not empty")));
    assert!(!sql.contains(&format!("the copy of {name} has another number of rows")));
}

#[test]
fn the_counter_hands_out_numbers_to_the_names_the_plan_publishes() {
    let mut running = crate::restructure::plan::Running {
        next: 20000,
        names_after: DbNames::parse(NAMES).unwrap(),
    };
    let section = "11111111-2222-4333-8444-555555555555";
    assert_eq!(running.allocate(section, "VT").unwrap(), 20000);
    assert_eq!(running.allocate(section, "LineNo").unwrap(), 20001);
    assert_eq!(
        running
            .allocate("66666666-2222-4333-8444-555555555555", "Fld")
            .unwrap(),
        20002
    );
    assert_eq!(running.next, 20003);
    assert_eq!(running.names_after.number_of(section, "VT"), Some(20000));
    assert_eq!(
        running.names_after.number_of(section, "LineNo"),
        Some(20001)
    );
    // An entry of the same uuid and kind twice is the names' refusal, not a second number.
    assert!(running.allocate(section, "VT").is_err());
}
