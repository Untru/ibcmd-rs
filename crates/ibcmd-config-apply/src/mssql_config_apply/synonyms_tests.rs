//! A changed synonym in the object registry: the headers of a descriptor row, the edit of the registry text and the
//! rewrites against a database that is a table of canned answers.

use super::*;
use crate::mssql_config_apply::versions::deflate_row;
use crate::sql::{Dbms, ScriptVariables, SqlRow, SqlValue};

const CFG: &str = "66193438-abc5-410b-a1f1-a204102d1a62";
const OWNER: &str = "e983391b-e96a-4156-8d6d-b9f0be65c600";
const ATTRIBUTE: &str = "8d86192b-f421-4328-983d-40179c7aa2c9";
const OTHER: &str = "f2f2f2f2-0000-4000-8000-000000000002";
const NIL: &str = "00000000-0000-0000-0000-000000000000";
const MAIN: &str = "1a621f0f-5568-4183-bd9f-f6ef670e7090.si";

/// A descriptor row: the object's header and one attribute's, each `{3,{1,0,<uuid>},"Name",<synonym>,"",0,0,<nil>,0}`.
fn descriptor(object: &str, attribute: &str) -> String {
    format!(
        "\u{feff}{{1,\r\n{{1,\r\n{{3,\r\n{{1,0,{OWNER}}},\"Обработка\",\r\n{object},\"\",0,0,{NIL},0}}\r\n}},\r\n{{2,{{27,{{2,x}},\r\n{{3,\r\n{{1,0,{ATTRIBUTE}}},\"Реквизит\",\r\n{attribute},\"\",0,0,{NIL},0}}}}}}\r\n}}"
    )
}

fn ru(text: &str) -> String {
    format!("{{1,\"ru\",\"{text}\"}}")
}

#[test]
fn the_headers_of_an_object_and_of_its_children_give_their_synonyms_by_uuid() {
    let row = parse_row(
        descriptor(
            &ru("Обработка"),
            "{2,\"ru\",\"Реквизит\",\"en\",\"Attribute\"}",
        )
        .as_bytes(),
    )
    .unwrap();
    let headers = header_synonyms(&row);
    assert_eq!(headers.len(), 2);
    assert_eq!(
        headers[OWNER],
        vec![("ru".to_owned(), "Обработка".to_owned())]
    );
    assert_eq!(
        headers[ATTRIBUTE],
        vec![
            ("en".to_owned(), "Attribute".to_owned()),
            ("ru".to_owned(), "Реквизит".to_owned())
        ],
        "sorted by language"
    );
}

#[test]
fn a_synonym_changed_is_a_change_and_a_synonym_kept_or_a_new_header_is_not() {
    let active = descriptor(&ru("Обработка"), &ru("Реквизит"));
    // the object's synonym
    let staged = descriptor(&ru("Обработка (новая)"), &ru("Реквизит"));
    let changes = changes_between(active.as_bytes(), staged.as_bytes()).unwrap();
    assert_eq!(
        changes,
        vec![SynonymChange {
            uuid: OWNER.to_owned(),
            synonyms: vec![("ru".to_owned(), "Обработка (новая)".to_owned())],
        }]
    );
    // the attribute's, and a language added
    let staged = descriptor(
        &ru("Обработка"),
        "{2,\"ru\",\"Реквизит\",\"en\",\"Attribute\"}",
    );
    let changes = changes_between(active.as_bytes(), staged.as_bytes()).unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].uuid, ATTRIBUTE);
    assert_eq!(changes[0].synonyms.len(), 2);
    // the same text, or a text that differs elsewhere only: nothing
    assert!(
        changes_between(active.as_bytes(), active.as_bytes())
            .unwrap()
            .is_empty()
    );
    let staged = active.replace(
        "\"Реквизит\",\r\n{1,\"ru\",\"Реквизит\"},\"\"",
        "\"Реквизит\",\r\n{1,\"ru\",\"Реквизит\"},\"комментарий\"",
    );
    assert_ne!(staged, active);
    assert!(
        changes_between(active.as_bytes(), staged.as_bytes())
            .unwrap()
            .is_empty()
    );
    // a header the active row lacks (a new attribute) is the restructuring's, not a change of a synonym
    let body = active.trim_end().strip_suffix('}').unwrap();
    let staged = format!(
        "{body},\r\n{{3,\r\n{{1,0,{OTHER}}},\"Новый\",\r\n{},\"\",0,0,{NIL},0}}}}",
        ru("Новый")
    );
    let added = header_synonyms(&parse_row(staged.as_bytes()).unwrap());
    assert!(
        added.contains_key(OTHER),
        "the new header is in the staged row"
    );
    assert!(
        changes_between(active.as_bytes(), staged.as_bytes())
            .unwrap()
            .is_empty()
    );
}

fn record(uuid: &str, parent: &str, kind: usize, name: &str, synonym: &str) -> String {
    format!("{uuid},{parent},{kind},\"{name}\",\r\n{synonym},0,0")
}

fn block(text: &str) -> String {
    format!("{{1,1,\r\n{{\"ru\",\"{text}\"}}\r\n}}")
}

fn main_row(owner_synonym: &str) -> String {
    let records = [
        record(CFG, NIL, 0, "Конфигурация", &block("Конфигурация")),
        record(OWNER, CFG, 1, "Обработка", &block(owner_synonym)),
        record(ATTRIBUTE, OWNER, 2, "Реквизит", &block("Реквизит")),
    ];
    format!(
        "\u{feff}{{4,\r\n{{2,cf4abeab-37b2-11d4-940f-008048da11f9,bf845118-327b-4682-b5c6-285d2a0eb296}},\r\n{{{},{}}}\r\n}}",
        records.len(),
        records.join(",")
    )
}

#[test]
fn the_registry_takes_the_new_text_in_the_record_and_nowhere_else() {
    let old = main_row("Обработка");
    let new = main_row("Обработка (новая)");
    let (text, count) = set_synonyms(
        old.as_bytes(),
        &[(
            OWNER.to_owned(),
            vec![("ru".to_owned(), "Обработка (новая)".to_owned())],
        )],
    )
    .unwrap();
    assert_eq!(count, 1);
    assert_eq!(
        String::from_utf8(text.clone()).unwrap(),
        new,
        "the text is what the platform writes"
    );
    // written again: nothing to do
    let (again, count) = set_synonyms(
        &text,
        &[(
            OWNER.to_owned(),
            vec![("ru".to_owned(), "Обработка (новая)".to_owned())],
        )],
    )
    .unwrap();
    assert_eq!((again, count), (text, 0));
    // an uuid the registry does not list is left out
    let (same, count) = set_synonyms(
        old.as_bytes(),
        &[(OTHER.to_owned(), vec![("ru".to_owned(), "Нет".to_owned())])],
    )
    .unwrap();
    assert_eq!((same.as_slice(), count), (old.as_bytes(), 0));
}

#[test]
fn quotes_languages_and_an_empty_synonym_are_written_as_the_platform_writes_them() {
    let old = main_row("Обработка");
    let (text, count) = set_synonyms(
        old.as_bytes(),
        &[
            (
                OWNER.to_owned(),
                vec![
                    ("ru".to_owned(), "«Обр» \"а\"".to_owned()),
                    ("en".to_owned(), "Proc".to_owned()),
                ],
            ),
            (ATTRIBUTE.to_owned(), Vec::new()),
        ],
    )
    .unwrap();
    assert_eq!(count, 2);
    let text = String::from_utf8(text).unwrap();
    assert!(text.contains(&format!(
        "{OWNER},{CFG},1,\"Обработка\",\r\n{{1,2,\r\n{{\"en\",\"Proc\"}},\r\n{{\"ru\",\"«Обр» \"\"а\"\"\"}}\r\n}},0,0"
    )), "{text}");
    assert!(
        text.contains(&format!(
            "{ATTRIBUTE},{OWNER},2,\"Реквизит\",\r\n{{1,0}},0,0"
        )),
        "{text}"
    );
    // the row still parses and lists the same three records
    assert_eq!(si::parse(text.as_bytes()).unwrap().records.len(), 3);
}

// --- the rewrites ---------------------------------------------------------------------------------------

/// Canned answers: the first rule whose text the query contains answers.
#[derive(Default)]
struct Canned {
    rules: Vec<(String, Vec<Vec<SqlValue>>)>,
}

impl SqlClient for Canned {
    fn dbms(&self) -> Dbms {
        Dbms::SqlServer
    }
    fn max_connections(&self) -> usize {
        1
    }
    fn run_script(&self, _script: &str, _variables: ScriptVariables) -> Result<()> {
        unreachable!("the plan only reads")
    }
    fn execute(&self, _statement: &str, _params: &[SqlParam<'_>]) -> Result<u64> {
        unreachable!("the plan only reads")
    }
    fn read_rows(
        &self,
        query: &str,
        _params: &[SqlParam<'_>],
        each: &mut dyn FnMut(SqlRow) -> Result<()>,
    ) -> Result<()> {
        for (needle, rows) in &self.rules {
            if query.contains(needle.as_str()) {
                for values in rows {
                    each(SqlRow {
                        result_set: 0,
                        values: values.clone(),
                    })?;
                }
                return Ok(());
            }
        }
        panic!("no canned answer for: {query}");
    }
    fn query_json(&self, _query: &str) -> Result<Option<String>> {
        unreachable!()
    }
    fn write_rows(
        &self,
        _table: &str,
        _columns: &[&str],
        _rows: &[Vec<SqlParam<'_>>],
    ) -> Result<u64> {
        unreachable!()
    }
}

fn text(value: &str) -> SqlValue {
    SqlValue::Text(value.to_owned())
}

const VERSIONS: &str = "\u{feff}{0,2,\"1a621f0f-5568-4183-bd9f-f6ef670e7090.si\",11111111-1111-1111-1111-111111111111,\"2203278d-ef4f-4f68-98f1-feb257d53ecc.si\",33333333-3333-3333-3333-333333333333}";

fn registry_database() -> Canned {
    let mut client = Canned::default();
    client.rules.push((
        "FileName LIKE N'%.si' OR FileName = N'siVersions'".to_owned(),
        vec![
            vec![
                text(MAIN),
                SqlValue::Int(100),
                text("AAAA"),
                SqlValue::Binary(deflate_row(main_row("Обработка").as_bytes()).unwrap()),
            ],
            vec![
                text("siVersions"),
                SqlValue::Int(VERSIONS.len() as i64),
                text("CCCC"),
                SqlValue::Binary(VERSIONS.as_bytes().to_vec()),
            ],
        ],
    ));
    client
}

fn change(uuid: &str, text: &str) -> SynonymChange {
    SynonymChange {
        uuid: uuid.to_owned(),
        synonyms: vec![("ru".to_owned(), text.to_owned())],
    }
}

fn plain(rewrite: &ParamsRewrite) -> String {
    String::from_utf8(inflate_row(&rewrite.new_bytes).unwrap()).unwrap()
}

#[test]
fn the_registry_and_its_version_are_rewritten_guarding_the_rows_the_plan_saw() {
    let (rewrites, count) = plan_search_info(
        &registry_database(),
        "testdb",
        &[change(OWNER, "Обработка (новая)")],
        Vec::new(),
    )
    .unwrap();
    assert_eq!(count, 1);
    let names: Vec<&str> = rewrites
        .iter()
        .map(|rewrite| rewrite.file_name.as_str())
        .collect();
    assert_eq!(names, vec![MAIN, "siVersions"]);
    assert_eq!(plain(&rewrites[0]), main_row("Обработка (новая)"));
    assert_eq!(
        (
            rewrites[0].old_data_size,
            rewrites[0].old_sha256_hex.as_str()
        ),
        (100, "AAAA")
    );
    assert!(rewrites[0].set_creation && !rewrites[1].set_creation);
    let versions = String::from_utf8(rewrites[1].new_bytes.clone()).unwrap();
    assert!(
        !versions.contains("11111111-1111-1111-1111-111111111111"),
        "the registry has a new version"
    );
    assert!(
        versions.contains("33333333-3333-3333-3333-333333333333"),
        "the other rows keep theirs"
    );
    assert_eq!(rewrites[1].old_sha256_hex, "CCCC");
}

#[test]
fn nothing_is_rewritten_when_the_registry_says_what_the_stage_says() {
    let (rewrites, count) = plan_search_info(
        &registry_database(),
        "testdb",
        &[change(OWNER, "Обработка"), change(OTHER, "Нет такого")],
        Vec::new(),
    )
    .unwrap();
    assert_eq!(count, 0);
    assert!(rewrites.is_empty());
}

#[test]
fn the_edit_goes_on_top_of_a_row_another_edit_of_the_same_stage_has_rewritten() {
    // a restructuring has changed the synonym of the attribute in the same row already
    let earlier = ParamsRewrite {
        file_name: MAIN.to_owned(),
        old_data_size: 100,
        old_sha256_hex: "AAAA".to_owned(),
        new_bytes: deflate_row(
            main_row("Обработка")
                .replace("\"Реквизит\"}", "\"Реквизит2\"}")
                .as_bytes(),
        )
        .unwrap(),
        set_creation: true,
    };
    let (rewrites, count) = plan_search_info(
        &registry_database(),
        "testdb",
        &[change(OWNER, "Обработка (новая)")],
        vec![earlier],
    )
    .unwrap();
    assert_eq!(count, 1);
    let text = plain(&rewrites[0]);
    assert!(text.contains("Обработка (новая)"));
    assert!(text.contains("Реквизит2"), "the earlier edit stays");
    assert_eq!(
        (
            rewrites[0].old_data_size,
            rewrites[0].old_sha256_hex.as_str()
        ),
        (100, "AAAA")
    );
}

#[test]
fn a_database_without_a_registry_is_an_error() {
    let mut client = Canned::default();
    client.rules.push((
        "FileName LIKE N'%.si' OR FileName = N'siVersions'".to_owned(),
        vec![vec![
            text("siVersions"),
            SqlValue::Int(1),
            text("CCCC"),
            SqlValue::Binary(VERSIONS.as_bytes().to_vec()),
        ]],
    ));
    let error = plan_search_info(&client, "testdb", &[change(OWNER, "x")], Vec::new()).unwrap_err();
    assert!(
        format!("{error:#}").contains("no object registry"),
        "{error:#}"
    );
}

#[test]
fn a_stage_equal_to_the_physical_base_reverts_the_effective_alias_synonym() {
    // Physical Config says A, the published alias and registry say B, ConfigSave reverts to A.
    // The reader deliberately needs no physical Config answer: it reads the authoritative staged headers.
    let staged = deflate_row(descriptor(&ru("Обработка"), &ru("Реквизит")).as_bytes()).unwrap();
    let split = staged.len() / 2;
    let mut client = registry_database();
    client.rules[0].1[0][3] =
        SqlValue::Binary(deflate_row(main_row("Обработка (новая)").as_bytes()).unwrap());
    client.rules.push((
        "FROM [testdb].dbo.ConfigSave s WHERE".to_owned(),
        vec![
            vec![
                text(OWNER),
                SqlValue::Int(0),
                SqlValue::Binary(staged[..split].to_vec()),
            ],
            vec![
                text(OWNER),
                SqlValue::Int(1),
                SqlValue::Binary(staged[split..].to_vec()),
            ],
        ],
    ));
    let changes = read_changes(&client, "testdb", &[OWNER.to_owned()]).unwrap();
    assert_eq!(
        changes.len(),
        2,
        "both object's and nested attribute's headers are authoritative"
    );
    let (rewrites, count) = plan_search_info(&client, "testdb", &changes, Vec::new()).unwrap();
    assert_eq!(count, 1);
    assert_eq!(plain(&rewrites[0]), main_row("Обработка"));
    assert_ne!(
        String::from_utf8(rewrites[1].new_bytes.clone()).unwrap(),
        VERSIONS
    );
}

#[test]
fn an_unchanged_stage_keeps_the_registry_and_its_version_untouched() {
    let mut client = registry_database();
    client.rules.push((
        "FROM [testdb].dbo.ConfigSave s WHERE".to_owned(),
        vec![vec![
            text(OWNER),
            SqlValue::Int(0),
            SqlValue::Binary(
                deflate_row(descriptor(&ru("Обработка"), &ru("Реквизит")).as_bytes()).unwrap(),
            ),
        ]],
    ));
    let changes = read_changes(&client, "testdb", &[OWNER.to_owned()]).unwrap();
    let (rewrites, count) = plan_search_info(&client, "testdb", &changes, Vec::new()).unwrap();
    assert_eq!(count, 0);
    assert!(rewrites.is_empty(), "no registry or siVersions churn");
}
