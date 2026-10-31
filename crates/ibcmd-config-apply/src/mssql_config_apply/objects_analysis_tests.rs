//! `analyze` against a database that is a table of canned answers.

use super::*;
use crate::sql::{Dbms, ScriptVariables, SqlRow};

const OWNER: &str = "e983391b-e96a-4156-8d6d-b9f0be65c600";
const OLD_FORM: &str = "8d86192b-f421-4328-983d-40179c7aa2c9";
const NEW_FORM: &str = "ade5fa33-4a02-5feb-b400-a41f22e208be";
const NEW_TEMPLATE: &str = "0808b882-fde2-508b-9da9-4c3765442bbf";
const FORMS: &str = "d5b0e5ed-256d-401c-9c36-f630cafd8a62";
const TEMPLATES: &str = "3daea016-69b7-4ed4-9453-127911372fe6";
const NIL: &str = "00000000-0000-0000-0000-000000000000";
const SCAN: &str = "FROM [testdb].dbo.ConfigSave WHERE PartNo = 0 AND FileName LIKE";

/// Canned answers: the first rule whose text the query contains (and whose
/// first parameter, when it names one, matches) answers.
#[derive(Default)]
struct Canned {
    rules: Vec<(String, Option<String>, Vec<Vec<SqlValue>>)>,
}

impl Canned {
    fn rule(&mut self, needle: &str, param: Option<&str>, rows: Vec<Vec<SqlValue>>) {
        self.rules
            .push((needle.to_owned(), param.map(str::to_owned), rows));
    }
}

impl SqlClient for Canned {
    fn dbms(&self) -> Dbms {
        Dbms::SqlServer
    }
    fn max_connections(&self) -> usize {
        1
    }
    fn run_script(&self, _script: &str, _variables: ScriptVariables) -> Result<()> {
        unreachable!("the analysis only reads")
    }
    fn execute(&self, _statement: &str, _params: &[SqlParam<'_>]) -> Result<u64> {
        unreachable!("the analysis only reads")
    }
    fn read_rows(
        &self,
        query: &str,
        params: &[SqlParam<'_>],
        each: &mut dyn FnMut(SqlRow) -> Result<()>,
    ) -> Result<()> {
        for (needle, param, rows) in &self.rules {
            let param_matches = match (param, params.first()) {
                (None, _) => true,
                (Some(want), Some(SqlParam::Text(got))) => want == got,
                _ => false,
            };
            if query.contains(needle.as_str()) && param_matches {
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

fn blob(plain: &str) -> SqlValue {
    SqlValue::Binary(deflate_row(plain.as_bytes()).unwrap())
}

fn owner_active() -> String {
    format!("\u{feff}{{1,{{{TEMPLATES},0}},{{{FORMS},1,{OLD_FORM}}},{{7,x}}}}")
}

fn owner_staged(rest: &str) -> String {
    format!(
        "\u{feff}{{1,{{{TEMPLATES},1,{NEW_TEMPLATE}}},{{{FORMS},2,{OLD_FORM},{NEW_FORM}}},{{7,{rest}}}}}"
    )
}

fn form_descriptor(uuid: &str) -> String {
    format!(
        "\u{feff}{{1,\r\n{{1,\r\n{{0,\r\n{{13,\r\n{{3,\r\n{{1,0,{uuid}}},\"Форма\",\r\n{{1,\"ru\",\"Форма\"}},\"\",0,0,{NIL},0}},0,1,\r\n{{2,\r\n{{\"#\",1708fdaa-cbce-4289-b373-07a5a74bee91,1}}\r\n}}\r\n}},\r\n{{0}}\r\n}}\r\n}},0}}"
    )
}

fn template_descriptor(uuid: &str) -> String {
    format!(
        "\u{feff}{{1,\r\n{{2,0,\r\n{{3,\r\n{{1,0,{uuid}}},\"Макет\",\r\n{{1,\"ru\",\"Макет\"}},\"\",0,0,{NIL},0}}\r\n}},0}}"
    )
}

/// The main search-information row: the configuration, the owner (kind 1)
/// and its existing form (kind 2).
fn search_information() -> String {
    let cfg = "66193438-abc5-410b-a1f1-a204102d1a62";
    let record = |uuid: &str, parent: &str, kind: usize, name: &str| {
        format!("{uuid},{parent},{kind},\"{name}\",\r\n{{1,1,\r\n{{\"ru\",\"{name}\"}}\r\n}},0,0")
    };
    // a second data processor with a form and a template tells that forms
    // come before templates
    let other = "0d0d0d0d-0000-4000-8000-000000000001";
    let records = [
        record(cfg, NIL, 0, "Конфигурация"),
        record(OWNER, cfg, 1, "Обработка"),
        record(OLD_FORM, OWNER, 2, "Форма"),
        record(other, cfg, 1, "Другая"),
        record("0d0d0d0d-0000-4000-8000-000000000002", other, 2, "Форма"),
        record("0d0d0d0d-0000-4000-8000-000000000003", other, 3, "Макет"),
    ];
    format!(
        "\u{feff}{{4,\r\n{{4,cf4abeab-37b2-11d4-940f-008048da11f9,bf845118-327b-4682-b5c6-285d2a0eb296,{FORMS},{TEMPLATES}}},\r\n{{{},{}}}\r\n}}",
        records.len(),
        records.join(",")
    )
}

fn si_versions() -> String {
    "\u{feff}{0,1,\"1a621f0f-5568-4183-bd9f-f6ef670e7090.si\",11111111-1111-1111-1111-111111111111}"
        .to_owned()
}

fn row(name: &str) -> RowMeta {
    RowMeta {
        name: name.to_owned(),
        part: 0,
        data_size: 1,
        byte_len: 1,
        attributes: 0,
        creation: String::new(),
        modified: String::new(),
        sha256: format!("hash of {name}"),
    }
}

struct Fixture {
    client: Canned,
    staged: Vec<RowMeta>,
    active: HashMap<(String, i32), RowMeta>,
}

/// The staged descriptors the scan of `ConfigSave` returns.
fn scan_rows(owner_rest: &str, extra: &[&str]) -> Vec<Vec<SqlValue>> {
    let mut rows = vec![
        vec![text(OWNER), blob(&owner_staged(owner_rest))],
        vec![text(NEW_FORM), blob(&form_descriptor(NEW_FORM))],
        vec![text(NEW_TEMPLATE), blob(&template_descriptor(NEW_TEMPLATE))],
    ];
    for uuid in extra {
        rows.push(vec![text(uuid), blob(&form_descriptor(uuid))]);
    }
    rows
}

/// A stage of a new form and a new template of the owner, and the owner's
/// descriptor with `owner_rest` where the active one has `x`.
fn fixture(owner_rest: &str, extra: &[&str]) -> Fixture {
    let mut client = Canned::default();
    let db = "[testdb].dbo";
    client.rule(SCAN, None, scan_rows(owner_rest, extra));
    client.rule(
        &format!("FROM {db}.Config WHERE FileName = @P1"),
        Some(OWNER),
        vec![vec![blob(&owner_active())]],
    );
    client.rule(
        &format!("FROM {db}._ConfigChngR ORDER BY 1, 3"),
        None,
        vec![
            vec![SqlValue::Int(989), text("000003DD"), text("AA")],
            vec![SqlValue::Int(989), text("000003DD"), text("BB")],
            vec![SqlValue::Int(989), text("000003DD"), text("CC")],
        ],
    );
    client.rule(
        "OBJECT_ID(N'[testdb].dbo._Node989'",
        None,
        vec![vec![SqlValue::Int(1)]],
    );
    // AA, BB are ordinary nodes; CC is the plan's own node
    client.rule(
        &format!("FROM {db}._Node989"),
        None,
        vec![
            vec![text("AA"), SqlValue::Int(0), SqlValue::Int(0)],
            vec![text("BB"), SqlValue::Int(0), SqlValue::Int(0)],
            vec![text("CC"), SqlValue::Int(1), SqlValue::Int(0)],
        ],
    );
    client.rule(
        "FileName LIKE N'%.si'",
        None,
        vec![vec![
            text("1a621f0f-5568-4183-bd9f-f6ef670e7090.si"),
            SqlValue::Int(10),
            text("ABCD"),
            blob(&search_information()),
        ]],
    );
    client.rule("AND PartNo <> 0", None, vec![vec![SqlValue::Int(0)]]);
    client.rule(
        "FileName = N'siVersions'",
        None,
        vec![vec![
            SqlValue::Int(5),
            text("EF01"),
            SqlValue::Binary(si_versions().into_bytes()),
        ]],
    );
    let mut names = vec![
        OWNER.to_owned(),
        NEW_FORM.to_owned(),
        format!("{NEW_FORM}.0"),
        NEW_TEMPLATE.to_owned(),
        format!("{NEW_TEMPLATE}.0"),
    ];
    names.extend(extra.iter().map(|uuid| (*uuid).to_owned()));
    let staged: Vec<RowMeta> = names.iter().map(|name| row(name)).collect();
    let mut active = HashMap::new();
    active.insert(row(OWNER).key(), row(OWNER));
    Fixture {
        client,
        staged,
        active,
    }
}

fn run(fixture: &Fixture) -> Analysis {
    analyze(&AnalysisInput {
        client: &fixture.client,
        database: "testdb",
        staged: &fixture.staged,
        active: &fixture.active,
        has_change_registrations: true,
    })
    .unwrap()
}

#[test]
fn a_form_and_a_template_of_an_existing_owner_are_accepted_whole() {
    let analysis = run(&fixture("x", &[]));
    assert!(analysis.blockers.is_empty(), "{:?}", analysis.blockers);
    let new = analysis.new;
    assert_eq!(new.objects.len(), 2);
    assert!(new.owners.contains(OWNER));
    assert_eq!(new.kinds[NEW_FORM], "Form");
    assert_eq!(new.kinds[NEW_TEMPLATE], "Template");
    // both descriptors and both bodies are accepted rows; the owner is not a new row
    assert!(new.rows.contains(NEW_FORM) && new.rows.contains(&format!("{NEW_FORM}.0")));
    assert!(new.rows.contains(NEW_TEMPLATE));
    assert!(!new.rows.contains(OWNER));
    // registered for the ordinary nodes, not for the plan's own node
    let nodes: Vec<&str> = new
        .nodes
        .iter()
        .map(|node| node.reference.as_str())
        .collect();
    assert_eq!(nodes, vec!["AA", "BB"]);
    // the search information gains a record per object and a new version
    assert_eq!(new.search_info_records, 2);
    let names: Vec<&str> = new
        .search_info
        .iter()
        .map(|rewrite| rewrite.file_name.as_str())
        .collect();
    assert_eq!(
        names,
        vec!["1a621f0f-5568-4183-bd9f-f6ef670e7090.si", "siVersions"]
    );
    assert!(new.search_info[0].set_creation && !new.search_info[1].set_creation);
    let edited = si::parse(&inflate_row(&new.search_info[0].new_bytes).unwrap()).unwrap();
    let order: Vec<&str> = edited
        .records
        .iter()
        .map(|record| record.uuid.as_str())
        .collect();
    // behind the owner's existing form (forms come before templates)
    assert_eq!(&order[1..5], &[OWNER, OLD_FORM, NEW_FORM, NEW_TEMPLATE]);
    assert_eq!(order.len(), 8, "six records and two new ones");
    assert_eq!(edited.records[3].kind, 2);
    assert_eq!(edited.records[4].kind, 3);
    let versions = String::from_utf8(new.search_info[1].new_bytes.clone()).unwrap();
    assert!(!versions.contains("11111111-1111-1111-1111-111111111111"));
    assert!(versions.starts_with("\u{feff}{0,1,\"1a621f0f-5568-4183-bd9f-f6ef670e7090.si\","));
}

#[test]
fn an_owner_that_changes_more_than_its_lists_is_refused() {
    let analysis = run(&fixture("y", &[]));
    assert!(
        analysis
            .blockers
            .iter()
            .any(|blocker| blocker.row == OWNER
                && blocker.reason.contains("changes more than the list")),
        "{:?}",
        analysis.blockers
    );
}

#[test]
fn a_new_object_no_owner_lists_is_refused() {
    let stray = "12345678-1234-4234-8234-123456789abc";
    let analysis = run(&fixture("x", &[stray]));
    assert!(
        analysis
            .blockers
            .iter()
            .any(|blocker| blocker.row == stray && blocker.reason.contains("no staged descriptor")),
        "{:?}",
        analysis.blockers
    );
}

#[test]
fn a_form_with_a_body_the_apply_does_not_know_is_refused() {
    let mut fixture = fixture("x", &[]);
    fixture.staged.push(row(&format!("{NEW_FORM}.5")));
    let analysis = run(&fixture);
    assert!(
        analysis.blockers.iter().any(
            |blocker| blocker.row == NEW_FORM && blocker.reason.contains("only [\"0\", \"1\"]")
        ),
        "{:?}",
        analysis.blockers
    );
}

#[test]
fn a_body_row_an_existing_object_gains_is_appended_to_its_registration() {
    let mut client = Canned::default();
    let object = "8d86192b-f421-4328-983d-40179c7aa2c9";
    let id = Uuid::parse_str(object).unwrap();
    client.rule(
        "SELECT FileName FROM [testdb].dbo.Config WHERE PartNo = 0 AND FileName IN",
        None,
        vec![vec![text(object)]],
    );
    client.rule(
        "SELECT DISTINCT CONVERT(varchar(32), _MDObjID, 2)",
        None,
        vec![vec![text(&super::super::model::hex_upper(
            &id.to_bytes_le(),
        ))]],
    );
    client.rule(
        "FROM [testdb].dbo._ConfigChngR ORDER BY 1, 3",
        None,
        vec![vec![SqlValue::Int(989), text("000003DD"), text("AA")]],
    );
    client.rule(
        "OBJECT_ID(N'[testdb].dbo._Node989'",
        None,
        vec![vec![SqlValue::Int(1)]],
    );
    client.rule(
        "FROM [testdb].dbo._Node989",
        None,
        vec![vec![text("AA"), SqlValue::Int(0), SqlValue::Int(0)]],
    );
    let staged = vec![row(&format!("{object}.1"))];
    let analysis = analyze(&AnalysisInput {
        client: &client,
        database: "testdb",
        staged: &staged,
        active: &HashMap::new(),
        has_change_registrations: true,
    })
    .unwrap();
    assert!(analysis.blockers.is_empty(), "{:?}", analysis.blockers);
    assert_eq!(analysis.new.bodies.len(), 1);
    assert_eq!(analysis.new.bodies[0].object, object);
    assert!(analysis.new.objects.is_empty());
    assert!(analysis.new.rows.contains(&format!("{object}.1")));
}

#[test]
fn a_body_row_of_a_nested_object_is_refused() {
    let mut client = Canned::default();
    client.rule(
        "SELECT FileName FROM [testdb].dbo.Config WHERE PartNo = 0 AND FileName IN",
        None,
        Vec::new(),
    );
    client.rule(
        "SELECT DISTINCT CONVERT(varchar(32), _MDObjID, 2)",
        None,
        Vec::new(),
    );
    let nested = "ba059c99-b392-42dd-8e4e-2e425601fce6";
    let staged = vec![row(&format!("{nested}.2"))];
    let analysis = analyze(&AnalysisInput {
        client: &client,
        database: "testdb",
        staged: &staged,
        active: &HashMap::new(),
        has_change_registrations: true,
    })
    .unwrap();
    assert_eq!(analysis.blockers.len(), 1);
    assert!(
        analysis.blockers[0]
            .reason
            .contains("without a descriptor row of its own")
    );
    assert!(analysis.new.bodies.is_empty());
}

#[test]
fn nothing_new_nothing_to_ask() {
    let client = Canned::default();
    let staged = vec![row(OWNER)];
    let mut active = HashMap::new();
    active.insert(row(OWNER).key(), row(OWNER));
    let analysis = analyze(&AnalysisInput {
        client: &client,
        database: "testdb",
        staged: &staged,
        active: &active,
        has_change_registrations: true,
    })
    .unwrap();
    assert!(analysis.blockers.is_empty() && analysis.new.is_empty());
}
