//! `analyze` and `plan_search_info` against a database that is a table of canned answers.

use super::*;
use crate::sql::{Dbms, ScriptVariables, SqlParam, SqlRow};

const CFG: &str = "66193438-abc5-410b-a1f1-a204102d1a62";
const OWNER: &str = "e983391b-e96a-4156-8d6d-b9f0be65c600";
const FORM_A: &str = "8d86192b-f421-4328-983d-40179c7aa2c9";
const FORM_B: &str = "f2f2f2f2-0000-4000-8000-000000000002";
const TEMPLATE_A: &str = "4f549963-efb2-4d60-9681-322a4740fb01";
const FORMS: &str = "d5b0e5ed-256d-401c-9c36-f630cafd8a62";
const TEMPLATES: &str = "3daea016-69b7-4ed4-9453-127911372fe6";
const NIL: &str = "00000000-0000-0000-0000-000000000000";
const OTHER: &str = "0d0d0d0d-0000-4000-8000-000000000001";

/// Canned answers: the first rule whose text the query contains answers.
#[derive(Default)]
struct Canned {
    rules: Vec<(String, Vec<Vec<SqlValue>>)>,
}

impl Canned {
    fn rule(&mut self, needle: &str, rows: Vec<Vec<SqlValue>>) -> &mut Self {
        self.rules.push((needle.to_owned(), rows));
        self
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

fn blob(plain: &str) -> SqlValue {
    SqlValue::Binary(deflate_row(plain.as_bytes()).unwrap())
}

fn owner_active() -> String {
    format!("\u{feff}{{1,{{{TEMPLATES},1,{TEMPLATE_A}}},{{{FORMS},2,{FORM_A},{FORM_B}}},{{7,x}}}}")
}

fn owner_staged(rest: &str) -> String {
    format!("\u{feff}{{1,{{{TEMPLATES},0}},{{{FORMS},1,{FORM_B}}},{{7,{rest}}}}}")
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

fn versions(names: &[&str]) -> SqlValue {
    let entries = names
        .iter()
        .map(|name| format!(",\"{name}\",22222222-2222-2222-2222-222222222222"))
        .collect::<String>();
    blob(&format!(
        "\u{feff}{{1,{},\"\",11111111-1111-1111-1111-111111111111{entries}}}",
        names.len() + 1
    ))
}

fn meta(name: &str) -> RowMeta {
    RowMeta {
        name: name.to_owned(),
        part: 0,
        data_size: 10,
        byte_len: 10,
        attributes: 0,
        creation: String::new(),
        modified: String::new(),
        sha256: "00".to_owned(),
    }
}

/// A database with the owner, two forms of it (one to stay) and a template, and a stage that takes
/// the first form and the template out.
fn database() -> Canned {
    let mut client = Canned::default();
    client
        .rule(
            "WHERE LEFT(FileName, 36) IN",
            vec![
                vec![text(TEMPLATE_A), SqlValue::Int(0)],
                vec![text(&format!("{TEMPLATE_A}.0")), SqlValue::Int(0)],
                vec![text(FORM_A), SqlValue::Int(0)],
                vec![text(&format!("{FORM_A}.0")), SqlValue::Int(0)],
                vec![text(&format!("{FORM_A}.1")), SqlValue::Int(0)],
            ],
        )
        .rule(
            "FROM [testdb].dbo.Config WHERE PartNo = 0 AND FileName LIKE",
            vec![
                vec![text(OWNER), blob(&owner_active())],
                vec![text(FORM_A), blob(&form_descriptor(FORM_A))],
                vec![text(FORM_B), blob(&form_descriptor(FORM_B))],
                vec![text(TEMPLATE_A), blob(&template_descriptor(TEMPLATE_A))],
            ],
        )
        .rule(
            "FROM [testdb].dbo.ConfigSave WHERE PartNo = 0 AND FileName LIKE",
            vec![
                vec![text(OWNER), blob(&owner_staged("x"))],
                vec![text(FORM_B), blob(&form_descriptor(FORM_B))],
            ],
        )
        .rule(
            "dbo.Params WHERE PartNo = 0 AND (FileName = N'DBNames'",
            vec![vec![
                text("DBNames"),
                blob("{2,{0d0d0d0d-0000-4000-8000-000000000009,\"Reference\",5}}"),
            ]],
        )
        .rule(
            "FROM [testdb].dbo.ConfigSave WHERE FileName = N'versions'",
            vec![vec![versions(&[OWNER, FORM_B])]],
        );
    client
}

fn names() -> Vec<String> {
    vec![
        TEMPLATE_A.to_owned(),
        format!("{TEMPLATE_A}.0"),
        FORM_A.to_owned(),
        format!("{FORM_A}.0"),
        format!("{FORM_A}.1"),
    ]
}

fn staged() -> Vec<RowMeta> {
    vec![meta(OWNER), meta(FORM_B), meta("versions")]
}

fn run(client: &Canned, names: &[String], staged: &[RowMeta]) -> RemovalAnalysis {
    analyze(&RemovalInput {
        client,
        database: "testdb",
        names,
        overlay_rows: &HashSet::new(),
        staged,
    })
    .unwrap()
}

fn reasons(analysis: &RemovalAnalysis) -> String {
    analysis
        .blockers
        .iter()
        .map(|blocker| format!("{}: {}", blocker.row, blocker.reason))
        .collect::<Vec<_>>()
        .join(" | ")
}

#[test]
fn a_form_and_a_template_the_owner_no_longer_lists_are_accounted_for_row_by_row() {
    let analysis = run(&database(), &names(), &staged());
    assert!(analysis.blockers.is_empty(), "{}", reasons(&analysis));
    let removals = analysis.removals;
    assert_eq!(removals.objects.len(), 2);
    let form = removals
        .objects
        .iter()
        .find(|object| object.uuid == FORM_A)
        .unwrap();
    assert_eq!(
        (form.kind, form.owner.as_str(), form.name.as_str()),
        ("Form", OWNER, "Форма")
    );
    assert_eq!(form.class, FORMS);
    assert_eq!(
        form.rows,
        vec![FORM_A, &format!("{FORM_A}.0"), &format!("{FORM_A}.1")]
    );
    let template = removals
        .objects
        .iter()
        .find(|object| object.uuid == TEMPLATE_A)
        .unwrap();
    assert_eq!(
        (template.kind, template.class.as_str()),
        ("Template", TEMPLATES)
    );
    assert_eq!(removals.rows.len(), 5);
    assert_eq!(removals.accounted.len(), 5);
    assert!(removals.accounted.contains(&format!("{FORM_A}.1")));
    // the owner's descriptor is the only one that may differ by the references
    assert_eq!(
        removals.owners,
        [OWNER.to_owned()].into_iter().collect::<HashSet<_>>()
    );
    assert_eq!(removals.objects[0].object_hex.len(), 32);
}

#[test]
fn a_row_of_the_object_the_list_leaves_out_refuses_the_object() {
    let mut list = names();
    list.retain(|name| !name.ends_with(".1"));
    let analysis = run(&database(), &list, &staged());
    assert!(analysis.removals.is_empty());
    let why = reasons(&analysis);
    assert!(
        why.contains("the list must name the object's rows exactly"),
        "{why}"
    );
}

#[test]
fn a_name_that_config_does_not_hold_refuses_the_object() {
    let mut list = names();
    list.push(format!("{FORM_A}.7"));
    let analysis = run(&database(), &list, &staged());
    assert!(analysis.removals.is_empty());
    assert!(
        reasons(&analysis).contains("Config does not hold"),
        "{}",
        reasons(&analysis)
    );
}

#[test]
fn a_body_row_of_an_object_that_stays_is_not_a_removal_of_an_object() {
    let list = vec![format!("{FORM_B}.0")];
    let analysis = run(&database(), &list, &staged());
    assert!(analysis.removals.is_empty());
    assert!(
        reasons(&analysis).contains("an object that stays"),
        "{}",
        reasons(&analysis)
    );
}

#[test]
fn a_service_row_or_an_odd_name_is_not_a_removable_object() {
    let list = vec!["versions".to_owned(), FORM_A.to_owned()];
    let analysis = run(&database(), &list, &staged());
    assert!(analysis.removals.is_empty());
    assert!(
        reasons(&analysis).contains("neither an object's descriptor"),
        "{}",
        reasons(&analysis)
    );
}

#[test]
fn an_owner_the_stage_does_not_carry_leaves_a_reference_and_refuses() {
    let mut client = database();
    client
        .rules
        .retain(|(needle, _)| !needle.contains("ConfigSave WHERE PartNo"));
    client.rule(
        "FROM [testdb].dbo.ConfigSave WHERE PartNo = 0 AND FileName LIKE",
        vec![vec![text(FORM_B), blob(&form_descriptor(FORM_B))]],
    );
    let analysis = run(&client, &names(), &[meta(FORM_B), meta("versions")]);
    assert!(analysis.removals.is_empty());
    assert!(
        reasons(&analysis).contains("is not staged"),
        "{}",
        reasons(&analysis)
    );
}

#[test]
fn an_owner_that_changes_more_than_the_list_of_its_forms_refuses() {
    let mut client = database();
    client
        .rules
        .retain(|(needle, _)| !needle.contains("ConfigSave WHERE PartNo"));
    client.rule(
        "FROM [testdb].dbo.ConfigSave WHERE PartNo = 0 AND FileName LIKE",
        vec![vec![text(OWNER), blob(&owner_staged("y"))]],
    );
    let analysis = run(&client, &names(), &staged());
    assert!(analysis.removals.is_empty());
    assert!(
        reasons(&analysis).contains("more than the removed references"),
        "{}",
        reasons(&analysis)
    );
}

#[test]
fn another_descriptor_that_mentions_the_object_refuses_it() {
    let mut client = database();
    client
        .rules
        .retain(|(needle, _)| !needle.contains("dbo.Config WHERE PartNo"));
    client.rule(
        "FROM [testdb].dbo.Config WHERE PartNo = 0 AND FileName LIKE",
        vec![
            vec![text(OWNER), blob(&owner_active())],
            vec![text(FORM_A), blob(&form_descriptor(FORM_A))],
            vec![text(TEMPLATE_A), blob(&template_descriptor(TEMPLATE_A))],
            // a subsystem's content, say
            vec![
                text(OTHER),
                blob(&format!("\u{feff}{{1,{{5,1,{FORM_A}}}}}")),
            ],
        ],
    );
    let analysis = run(&client, &names(), &staged());
    let why = reasons(&analysis);
    assert!(
        why.contains(&format!(
            "{FORM_A}: 2 descriptors of Config mention the object"
        )),
        "{why}"
    );
    // the owner's staged descriptor drops the form as well as the template, so the template alone does
    // not explain it: nothing is accounted for (and a list with a reason against it is refused whole)
    assert!(analysis.removals.is_empty());
    assert!(why.contains("more than the removed references"), "{why}");
}

#[test]
fn a_staged_descriptor_that_still_mentions_the_object_refuses_it() {
    let mut client = database();
    client
        .rules
        .retain(|(needle, _)| !needle.contains("ConfigSave WHERE PartNo"));
    client.rule(
        "FROM [testdb].dbo.ConfigSave WHERE PartNo = 0 AND FileName LIKE",
        vec![
            vec![text(OWNER), blob(&owner_staged("x"))],
            vec![
                text(OTHER),
                blob(&format!("\u{feff}{{1,{{5,1,{FORM_A}}}}}")),
            ],
        ],
    );
    let analysis = run(&client, &names(), &staged());
    assert!(
        reasons(&analysis).contains(&format!("the staged descriptor {OTHER} still mentions")),
        "{}",
        reasons(&analysis)
    );
}

#[test]
fn an_object_the_names_of_the_database_know_has_a_table_and_stays_for_the_native_apply() {
    let mut client = database();
    client
        .rules
        .retain(|(needle, _)| !needle.contains("FileName = N'DBNames'"));
    client.rule(
        "dbo.Params WHERE PartNo = 0 AND (FileName = N'DBNames'",
        vec![vec![
            text("DBNames"),
            blob(&format!("{{2,{{{FORM_A},\"Reference\",5}}}}")),
        ]],
    );
    let analysis = run(&client, &names(), &staged());
    assert!(
        reasons(&analysis).contains("names the object"),
        "{}",
        reasons(&analysis)
    );
    assert_eq!(analysis.removals.objects.len(), 1);
    assert_eq!(analysis.removals.objects[0].uuid, TEMPLATE_A);
}

#[test]
fn a_staged_versions_row_that_still_lists_a_removed_row_refuses_the_object() {
    let mut client = database();
    client
        .rules
        .retain(|(needle, _)| !needle.contains("N'versions'"));
    client.rule(
        "FROM [testdb].dbo.ConfigSave WHERE FileName = N'versions'",
        vec![vec![versions(&[OWNER, &format!("{FORM_A}.0")])]],
    );
    let analysis = run(&client, &names(), &staged());
    assert!(
        reasons(&analysis).contains(&format!("still lists {FORM_A}.0")),
        "{}",
        reasons(&analysis)
    );
}

#[test]
fn an_object_that_no_owned_group_lists_is_not_a_form_or_template() {
    // the same uuid listed in a group of another class: a subsystem
    let subsystems = "37f2fa9a-b276-11d4-9435-004095e12fc7";
    let owner = format!("\u{feff}{{1,{{{subsystems},1,{FORM_A}}},{{7,x}}}}");
    let staged_owner = format!("\u{feff}{{1,{{{subsystems},0}},{{7,x}}}}");
    let mut client = Canned::default();
    client
        .rule(
            "WHERE LEFT(FileName, 36) IN",
            vec![vec![text(FORM_A), SqlValue::Int(0)]],
        )
        .rule(
            "FROM [testdb].dbo.Config WHERE PartNo = 0 AND FileName LIKE",
            vec![
                vec![text(OWNER), blob(&owner)],
                vec![text(FORM_A), blob(&form_descriptor(FORM_A))],
            ],
        )
        .rule(
            "FROM [testdb].dbo.ConfigSave WHERE PartNo = 0 AND FileName LIKE",
            vec![vec![text(OWNER), blob(&staged_owner)]],
        );
    let analysis = run(&client, &[FORM_A.to_owned()], &[meta(OWNER)]);
    assert!(analysis.removals.is_empty());
    assert!(
        reasons(&analysis).contains("only a removed form or template is supported"),
        "{}",
        reasons(&analysis)
    );
}

#[test]
fn the_stage_that_removes_an_object_and_stages_rows_of_it_is_contradictory() {
    let analysis = run(
        &database(),
        &names(),
        &[meta(OWNER), meta(&format!("{FORM_A}.0")), meta("versions")],
    );
    assert!(
        reasons(&analysis).contains("stages rows of it too"),
        "{}",
        reasons(&analysis)
    );
    assert!(analysis.removals.is_empty());
}

// --- the search information ---------------------------------------------------------------------------

const MAIN: &str = "1a621f0f-5568-4183-bd9f-f6ef670e7090.si";

fn record(uuid: &str, parent: &str, kind: usize, name: &str) -> String {
    format!("{uuid},{parent},{kind},\"{name}\",\r\n{{1,1,\r\n{{\"ru\",\"{name}\"}}\r\n}},0,0")
}

fn main_row() -> String {
    let records = [
        record(CFG, NIL, 0, "Конфигурация"),
        record(OWNER, CFG, 1, "Обработка"),
        record(FORM_A, OWNER, 2, "Форма"),
        record(FORM_B, OWNER, 2, "Вторая"),
        record(TEMPLATE_A, OWNER, 3, "Макет"),
    ];
    format!(
        "\u{feff}{{4,\r\n{{4,cf4abeab-37b2-11d4-940f-008048da11f9,bf845118-327b-4682-b5c6-285d2a0eb296,{FORMS},{TEMPLATES}}},\r\n{{{},{}}}\r\n}}",
        records.len(),
        records.join(",")
    )
}

fn properties_row() -> String {
    format!(
        "\u{feff}{{0,\r\n{{3,\r\n{FORM_A},1,0,\r\n{{\"S\",\"v8config://v8cfgHelp/mdobject/id{FORM_A}/038b5c85-fb1c-4082-9c4c-e69f8928bf3a\"}},{OTHER},2,2,\r\n{{\"N\",0}},5,\r\n{{\"B\",1}},{OWNER},1,3,\r\n{{\"#\",fc01b5df-97fe-449b-83d4-218a090e681e,7}}\r\n}}\r\n}}"
    )
}

const VERSIONS: &str = "\u{feff}{0,3,\"1a621f0f-5568-4183-bd9f-f6ef670e7090.si\",11111111-1111-1111-1111-111111111111,\"c4629235-4823-4320-b8b5-1d08f4c6d612.si\",22222222-2222-2222-2222-222222222222,\"2203278d-ef4f-4f68-98f1-feb257d53ecc.si\",33333333-3333-3333-3333-333333333333}";

fn si_row(name: &str, size: i64, sha: &str, plain: &str) -> Vec<SqlValue> {
    vec![
        text(name),
        SqlValue::Int(size),
        text(sha),
        SqlValue::Binary(deflate_row(plain.as_bytes()).unwrap()),
    ]
}

fn search_information() -> Canned {
    let mut client = Canned::default();
    client.rule(
        "FileName LIKE N'%.si' OR FileName = N'siVersions'",
        vec![
            si_row(MAIN, 100, "AAAA", &main_row()),
            si_row(PROPERTIES_ROW, 200, "BBBB", &properties_row()),
            vec![
                text("siVersions"),
                SqlValue::Int(VERSIONS.len() as i64),
                text("CCCC"),
                SqlValue::Binary(VERSIONS.as_bytes().to_vec()),
            ],
        ],
    );
    client
}

fn removed_objects() -> Removals {
    let object = |uuid: &str, kind: &'static str, class: &str, name: &str| RemovedObject {
        uuid: uuid.to_owned(),
        kind,
        owner: OWNER.to_owned(),
        name: name.to_owned(),
        class: class.to_owned(),
        rows: vec![uuid.to_owned()],
        object_hex: String::new(),
    };
    Removals {
        objects: vec![
            object(FORM_A, "Form", FORMS, "Форма"),
            object(TEMPLATE_A, "Template", TEMPLATES, "Макет"),
        ],
        ..Removals::default()
    }
}

fn inflated(rewrite: &ParamsRewrite) -> String {
    String::from_utf8(strip_bom(&inflate_row(&rewrite.new_bytes).unwrap()).to_vec()).unwrap()
}

#[test]
fn the_records_and_the_property_entries_of_the_removed_objects_go_and_the_versions_follow() {
    let (rewrites, summary) = plan_search_info(
        &search_information(),
        "testdb",
        &removed_objects(),
        Vec::new(),
    )
    .unwrap();
    assert_eq!(
        (summary.records_removed, summary.property_entries_removed),
        (2, 1)
    );
    let names: Vec<&str> = rewrites
        .iter()
        .map(|rewrite| rewrite.file_name.as_str())
        .collect();
    assert_eq!(names, vec![MAIN, PROPERTIES_ROW, "siVersions"]);
    // each rewrite guards the row the plan saw
    assert_eq!(
        (
            rewrites[0].old_data_size,
            rewrites[0].old_sha256_hex.as_str()
        ),
        (100, "AAAA")
    );
    assert_eq!(
        (
            rewrites[1].old_data_size,
            rewrites[1].old_sha256_hex.as_str()
        ),
        (200, "BBBB")
    );
    assert_eq!(rewrites[2].old_sha256_hex, "CCCC");
    assert!(rewrites[0].set_creation && rewrites[1].set_creation && !rewrites[2].set_creation);
    // the main row lost exactly the two records, and its count
    let main = si::parse(inflate_row(&rewrites[0].new_bytes).unwrap().as_slice()).unwrap();
    let left: Vec<&str> = main
        .records
        .iter()
        .map(|record| record.uuid.as_str())
        .collect();
    assert_eq!(left, vec![CFG, OWNER, FORM_B]);
    assert!(inflated(&rewrites[0]).starts_with("{4,"));
    assert!(inflated(&rewrites[0]).contains("{3,"), "the count follows");
    // the properties row lost the form's entry only
    let properties = inflated(&rewrites[1]);
    assert!(!properties.contains(FORM_A));
    assert!(properties.starts_with("{0,\r\n{2,\r\n"));
    assert!(properties.contains(&format!("{OTHER},2,2,")));
    assert!(properties.contains(&format!("{OWNER},1,3,")));
    // siVersions: the two rows have new versions, the third keeps its own
    let versions = String::from_utf8(rewrites[2].new_bytes.clone()).unwrap();
    assert!(!versions.contains("11111111-1111-1111-1111-111111111111"));
    assert!(!versions.contains("22222222-2222-2222-2222-222222222222"));
    assert!(versions.contains("33333333-3333-3333-3333-333333333333"));
    assert!(versions.starts_with("\u{feff}{0,3,"));
}

#[test]
fn the_edit_goes_on_top_of_a_row_another_edit_of_the_same_stage_has_rewritten() {
    // a new form of FORM_B's owner was inserted by the analysis of the new objects
    let with_new = main_row()
        .replace(
            &record(FORM_B, OWNER, 2, "Вторая"),
            &format!(
                "{},{}",
                record(FORM_B, OWNER, 2, "Вторая"),
                record("ade5fa33-4a02-5feb-b400-a41f22e208be", OWNER, 2, "Новая")
            ),
        )
        .replacen("{5,", "{6,", 1);
    let earlier = ParamsRewrite {
        file_name: MAIN.to_owned(),
        old_data_size: 100,
        old_sha256_hex: "AAAA".to_owned(),
        new_bytes: deflate_row(with_new.as_bytes()).unwrap(),
        set_creation: true,
    };
    let (rewrites, _) = plan_search_info(
        &search_information(),
        "testdb",
        &removed_objects(),
        vec![earlier],
    )
    .unwrap();
    let main = si::parse(inflate_row(&rewrites[0].new_bytes).unwrap().as_slice()).unwrap();
    let left: Vec<&str> = main
        .records
        .iter()
        .map(|record| record.uuid.as_str())
        .collect();
    assert_eq!(
        left,
        vec![CFG, OWNER, FORM_B, "ade5fa33-4a02-5feb-b400-a41f22e208be"],
        "the new record stays, the removed ones go"
    );
    // the digest is still the one of the stored row
    assert_eq!(
        (
            rewrites[0].old_data_size,
            rewrites[0].old_sha256_hex.as_str()
        ),
        (100, "AAAA")
    );
}

#[test]
fn a_record_that_is_not_the_objects_own_stops_the_edit() {
    // the search information gives the form another name than its descriptor does
    let mut removals = removed_objects();
    removals.objects[0].name = "Другая".to_owned();
    let error = plan_search_info(&search_information(), "testdb", &removals, Vec::new())
        .expect_err("a mismatch is an error");
    assert!(format!("{error:#}").contains("names"), "{error:#}");
    // another owner
    let mut removals = removed_objects();
    removals.objects[0].owner = OTHER.to_owned();
    assert!(plan_search_info(&search_information(), "testdb", &removals, Vec::new()).is_err());
    // another class than the owner files it under
    let mut removals = removed_objects();
    removals.objects[0].class = TEMPLATES.to_owned();
    assert!(plan_search_info(&search_information(), "testdb", &removals, Vec::new()).is_err());
}

#[test]
fn an_object_the_search_information_does_not_list_stops_the_edit() {
    let mut removals = removed_objects();
    removals.objects[0].uuid = "12345678-0000-4000-8000-000000000000".to_owned();
    let error = plan_search_info(&search_information(), "testdb", &removals, Vec::new())
        .expect_err("no row lists the object");
    assert!(
        format!("{error:#}").contains("exactly one is expected"),
        "{error:#}"
    );
}

#[test]
fn a_properties_row_without_the_objects_is_left_as_it_is() {
    let mut client = Canned::default();
    let bare = properties_row().replace(
        &format!("{FORM_A},1,0,\r\n{{\"S\",\"v8config://v8cfgHelp/mdobject/id{FORM_A}/038b5c85-fb1c-4082-9c4c-e69f8928bf3a\"}},"),
        "",
    ).replacen("{3,", "{2,", 1);
    client.rule(
        "FileName LIKE N'%.si' OR FileName = N'siVersions'",
        vec![
            si_row(MAIN, 100, "AAAA", &main_row()),
            si_row(PROPERTIES_ROW, 200, "BBBB", &bare),
            vec![
                text("siVersions"),
                SqlValue::Int(VERSIONS.len() as i64),
                text("CCCC"),
                SqlValue::Binary(VERSIONS.as_bytes().to_vec()),
            ],
        ],
    );
    let (rewrites, summary) =
        plan_search_info(&client, "testdb", &removed_objects(), Vec::new()).unwrap();
    assert_eq!(summary.property_entries_removed, 0);
    let names: Vec<&str> = rewrites
        .iter()
        .map(|rewrite| rewrite.file_name.as_str())
        .collect();
    assert_eq!(names, vec![MAIN, "siVersions"]);
    let versions = String::from_utf8(rewrites[1].new_bytes.clone()).unwrap();
    assert!(
        versions.contains("22222222-2222-2222-2222-222222222222"),
        "an untouched row keeps its version"
    );
}
