//! The conservative gate's rules for body rows of several parts (the platform
//! cuts a value at 10 MB), against a database that is a table of canned
//! answers.

use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;
use crate::mssql_config_apply::versions::deflate_row;
use crate::sql::{Dbms, ScriptVariables, SqlRow, SqlValue};

const CONFIG: &str = "66193438-abc5-410b-a1f1-a204102d1a62";
const TEMPLATE: &str = "0808b882-fde2-508b-9da9-4c3765442bbf";
const CATALOG: &str = "11111111-2222-4333-8444-555555555555";

/// Answers the two reads the role check makes: the `root` row, and the scan of
/// the active descriptors (there are none). Counts the reads.
#[derive(Default)]
struct Canned {
    reads: AtomicUsize,
    /// The staged and the active bytes of the `root` row, when the gate compares them.
    root_pair: Option<(Vec<u8>, Vec<u8>)>,
}

impl SqlClient for Canned {
    fn dbms(&self) -> Dbms {
        Dbms::SqlServer
    }
    fn max_connections(&self) -> usize {
        1
    }
    fn run_script(&self, _script: &str, _variables: ScriptVariables) -> Result<()> {
        unreachable!("the gate only reads")
    }
    fn execute(&self, _statement: &str, _params: &[SqlParam<'_>]) -> Result<u64> {
        unreachable!("the gate only reads")
    }
    fn read_rows(
        &self,
        query: &str,
        _params: &[SqlParam<'_>],
        each: &mut dyn FnMut(SqlRow) -> Result<()>,
    ) -> Result<()> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        if query.contains("FileName IN (N'root')") {
            let (staged, active) = self.root_pair.clone().expect("a root pair");
            return each(SqlRow {
                result_set: 0,
                values: vec![SqlValue::Binary(staged), SqlValue::Binary(active)],
            });
        }
        if query.contains("FileName = N'root'") {
            let root = format!("\u{feff}{{2,{CONFIG},0}}");
            return each(SqlRow {
                result_set: 0,
                values: vec![SqlValue::Binary(deflate_row(root.as_bytes()).unwrap())],
            });
        }
        if query.contains("FileName LIKE N'________-____-____-____-____________'") {
            return Ok(());
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

fn meta(name: &str, part: i32, sha256: &str) -> RowMeta {
    RowMeta {
        name: name.to_owned(),
        part,
        data_size: 1,
        byte_len: 1,
        attributes: 0,
        creation: String::new(),
        modified: String::new(),
        sha256: sha256.to_owned(),
    }
}

/// Runs the conservative gate on `staged` against `active`; `kinds` names the
/// objects the caller has analysed (the gate reads no other object kind here).
fn check(
    client: &Canned,
    staged: &[RowMeta],
    active: &[RowMeta],
    kinds: &[(&str, &'static str)],
) -> GateVerdict {
    let active: HashMap<(String, i32), RowMeta> =
        active.iter().map(|row| (row.key(), row.clone())).collect();
    let accepted_new_rows = HashSet::new();
    let accepted_owner_descriptors = HashSet::new();
    let consumed_rows = HashSet::new();
    let new_object_kinds: HashMap<String, &'static str> = kinds
        .iter()
        .map(|(uuid, kind)| ((*uuid).to_owned(), *kind))
        .collect();
    ConservativeGate::default()
        .check(&GateInput {
            client,
            database: "testdb",
            staged,
            active: &active,
            accepted_new_rows: &accepted_new_rows,
            accepted_owner_descriptors: &accepted_owner_descriptors,
            new_object_kinds: &new_object_kinds,
            consumed_rows: &consumed_rows,
            removed_rows: &consumed_rows,
        })
        .unwrap()
}

fn blocker_reasons(verdict: &GateVerdict) -> Vec<String> {
    verdict
        .blockers
        .iter()
        .map(|blocker| format!("{}: {}", blocker.row, blocker.reason))
        .collect()
}

const BOM: char = char::from_u32(0xFEFF).unwrap();

/// The bytes of a row the platform stores as one stored deflate block.
fn stored_block(text: &[u8]) -> Vec<u8> {
    let length = u16::try_from(text.len()).unwrap();
    let mut out = vec![1];
    out.extend(length.to_le_bytes());
    out.extend((!length).to_le_bytes());
    out.extend(text);
    out
}

#[test]
fn a_root_row_in_another_layout_is_no_change() {
    let text = format!("{BOM}{{2,{CONFIG},}}");
    let client = Canned {
        root_pair: Some((
            deflate_row(text.as_bytes()).unwrap(),
            stored_block(text.as_bytes()),
        )),
        ..Canned::default()
    };
    let verdict = check(
        &client,
        &[meta("root", 0, "AA")],
        &[meta("root", 0, "BB")],
        &[],
    );
    assert!(!verdict.restructuring_required, "{:?}", verdict.blockers);
    assert_eq!(verdict.stats.descriptors_layout_only, 1);
}

#[test]
fn a_root_row_that_names_another_configuration_is_refused() {
    let staged = format!("{BOM}{{2,{CATALOG},}}");
    let active = format!("{BOM}{{2,{CONFIG},}}");
    let client = Canned {
        root_pair: Some((
            deflate_row(staged.as_bytes()).unwrap(),
            stored_block(active.as_bytes()),
        )),
        ..Canned::default()
    };
    let verdict = check(
        &client,
        &[meta("root", 0, "AA")],
        &[meta("root", 0, "BB")],
        &[],
    );
    assert_eq!(
        blocker_reasons(&verdict),
        ["root: the service row root changes"]
    );
}

#[test]
fn a_row_of_two_parts_is_one_change() {
    // the first part is unchanged, the second one differs: the row changed, and
    // its first part stands for it in the role check
    let name = format!("{TEMPLATE}.0");
    let client = Canned::default();
    let verdict = check(
        &client,
        &[meta(&name, 0, "a"), meta(&name, 1, "b")],
        &[meta(&name, 0, "a"), meta(&name, 1, "c")],
        &[(TEMPLATE, "Template")],
    );
    assert!(
        !verdict.restructuring_required,
        "{:?}",
        blocker_reasons(&verdict)
    );
    assert_eq!(verdict.stats.extra_parts, 1);
    assert_eq!(verdict.stats.rows_identical, 0);
    assert_eq!(verdict.stats.bodies_by_role.get("Template"), Some(&1));
}

#[test]
fn a_row_of_two_parts_that_did_not_change_needs_no_role_check() {
    let name = format!("{TEMPLATE}.0");
    let client = Canned::default();
    let verdict = check(
        &client,
        &[meta(&name, 0, "a"), meta(&name, 1, "b")],
        &[meta(&name, 0, "a"), meta(&name, 1, "b")],
        &[],
    );
    assert!(!verdict.restructuring_required);
    assert_eq!(verdict.stats.rows_identical, 2);
    assert_eq!(verdict.stats.extra_parts, 0);
    assert_eq!(client.reads.load(Ordering::SeqCst), 0);
}

#[test]
fn a_new_row_of_two_parts_counts_its_extra_part() {
    // no active row at all: a new body of an object the caller has analysed
    let name = format!("{TEMPLATE}.0");
    let client = Canned::default();
    let verdict = check(
        &client,
        &[meta(&name, 0, "a"), meta(&name, 1, "b")],
        &[],
        &[(TEMPLATE, "Template")],
    );
    assert!(
        !verdict.restructuring_required,
        "{:?}",
        blocker_reasons(&verdict)
    );
    assert_eq!(verdict.stats.extra_parts, 1);
}

#[test]
fn a_part_whose_first_part_is_not_staged_is_refused() {
    let name = format!("{TEMPLATE}.0");
    let client = Canned::default();
    let verdict = check(
        &client,
        &[meta(&name, 1, "b")],
        &[meta(&name, 0, "a"), meta(&name, 1, "c")],
        &[(TEMPLATE, "Template")],
    );
    assert!(verdict.restructuring_required);
    let reasons = blocker_reasons(&verdict);
    assert!(
        reasons
            .iter()
            .any(|reason| reason.contains("first part is not staged")),
        "{reasons:?}"
    );
}

#[test]
fn a_row_of_several_parts_is_never_compared_as_text() {
    // predefined data is a role the gate admits only when the text is unchanged;
    // the comparison reads one part, so a row of two parts cannot be proved
    let name = format!("{CATALOG}.1c");
    let client = Canned::default();
    let verdict = check(
        &client,
        &[meta(&name, 0, "a"), meta(&name, 1, "b")],
        &[meta(&name, 0, "a"), meta(&name, 1, "c")],
        &[(CATALOG, "Catalog")],
    );
    assert!(verdict.restructuring_required);
    let reasons = blocker_reasons(&verdict);
    assert!(
        reasons
            .iter()
            .any(|reason| reason.contains("several parts")),
        "{reasons:?}"
    );
}

#[test]
fn a_single_part_over_a_row_of_several_parts_is_refused_the_same_way() {
    let name = format!("{CATALOG}.1c");
    let client = Canned::default();
    let verdict = check(
        &client,
        &[meta(&name, 0, "b")],
        &[meta(&name, 0, "a"), meta(&name, 1, "c")],
        &[(CATALOG, "Catalog")],
    );
    assert!(verdict.restructuring_required);
    assert!(
        blocker_reasons(&verdict)
            .iter()
            .any(|reason| reason.contains("several parts"))
    );
}

#[test]
fn a_consumed_row_is_not_judged_and_an_unconsumed_one_is_refused() {
    let client = Canned::default();
    let staged = [meta("deleted", 0, "a")];
    let judge = |consumed: &HashSet<String>| {
        let active: HashMap<(String, i32), RowMeta> = HashMap::new();
        let accepted = HashSet::new();
        let kinds = HashMap::new();
        ConservativeGate::default()
            .check(&GateInput {
                client: &client,
                database: "testdb",
                staged: &staged,
                active: &active,
                accepted_new_rows: &accepted,
                accepted_owner_descriptors: &accepted,
                new_object_kinds: &kinds,
                consumed_rows: consumed,
                removed_rows: &accepted,
            })
            .unwrap()
    };
    let refused = judge(&HashSet::new());
    assert!(refused.restructuring_required);
    let mut consumed = HashSet::new();
    consumed.insert("deleted".to_owned());
    let passed = judge(&consumed);
    assert!(
        !passed.restructuring_required,
        "{:?}",
        blocker_reasons(&passed)
    );
}

#[test]
fn a_descriptor_has_one_part() {
    let client = Canned::default();
    let verdict = check(
        &client,
        &[meta(CATALOG, 0, "a"), meta(CATALOG, 1, "b")],
        &[meta(CATALOG, 0, "a")],
        &[],
    );
    assert!(verdict.restructuring_required);
    assert!(
        blocker_reasons(&verdict)
            .iter()
            .any(|reason| reason.contains("a part number other than 0"))
    );
}

/// A gate that answers what it was told, counts its questions and hands over a phase when it has one.
struct Scripted {
    name: &'static str,
    refuses: bool,
    judges: bool,
    asked: std::rc::Rc<std::cell::Cell<usize>>,
    phase: std::cell::RefCell<Option<StructurePhase>>,
}

impl Scripted {
    fn new(name: &'static str, refuses: bool) -> Self {
        Self {
            name,
            refuses,
            judges: false,
            asked: Default::default(),
            phase: Default::default(),
        }
    }
}

impl StructuralGate for Scripted {
    fn name(&self) -> &'static str {
        self.name
    }
    fn check(&self, _input: &GateInput<'_>) -> Result<GateVerdict> {
        self.asked.set(self.asked.get() + 1);
        let mut verdict = GateVerdict {
            gate: self.name.to_owned(),
            ..GateVerdict::default()
        };
        if self.refuses {
            verdict.block("row", format!("{} refuses", self.name));
        }
        Ok(verdict)
    }
    fn take_structure(&self) -> Option<StructurePhase> {
        self.phase.borrow_mut().take()
    }
    fn judges_deleted_row(&self) -> bool {
        self.judges
    }
}

fn ask(gate: &dyn StructuralGate) -> GateVerdict {
    let client = Canned::default();
    let none = HashSet::new();
    gate.check(&GateInput {
        client: &client,
        database: "testdb",
        staged: &[],
        active: &HashMap::new(),
        accepted_new_rows: &none,
        accepted_owner_descriptors: &none,
        new_object_kinds: &HashMap::new(),
        consumed_rows: &none,
        removed_rows: &none,
    })
    .unwrap()
}

#[test]
fn what_the_first_gate_passes_never_reaches_the_second() {
    let second = Scripted::new("s1", true);
    let asked = second.asked.clone();
    let pair = FirstThen::new(
        Box::new(Scripted::new("apply-check", false)),
        Box::new(second),
    );
    let verdict = ask(&pair);
    assert!(!verdict.restructuring_required);
    assert_eq!(verdict.gate, "apply-check");
    // the second gate is not asked, and there is no phase to take
    assert_eq!(asked.get(), 0);
    assert!(pair.take_structure().is_none());
    // the pair is named for the gate that answers for a restructuring
    assert_eq!(pair.name(), "s1");
}

#[test]
fn what_the_first_gate_refuses_is_the_seconds_to_pass_with_its_phase() {
    let mut second = Scripted::new("s1", false);
    second.judges = true;
    *second.phase.borrow_mut() = Some(StructurePhase {
        consumed_staged_rows: 1,
        tables: vec!["_Reference1".to_owned()],
        ..StructurePhase::default()
    });
    let asked = second.asked.clone();
    let pair = FirstThen::new(
        Box::new(Scripted::new("apply-check", true)),
        Box::new(second),
    );
    let verdict = ask(&pair);
    assert!(!verdict.restructuring_required);
    assert_eq!(verdict.gate, "s1");
    assert_eq!(asked.get(), 1);
    let phase = pair.take_structure().expect("the second gate's phase");
    assert_eq!(phase.tables, ["_Reference1"]);
    // taken once
    assert!(pair.take_structure().is_none());
    // the pair judges the `deleted` row as the second gate does
    assert!(pair.judges_deleted_row());
}

#[test]
fn what_both_refuse_is_refused_for_the_reasons_of_both_the_first_gates_first() {
    let pair = FirstThen::new(
        Box::new(Scripted::new("apply-check", true)),
        Box::new(Scripted::new("s1", true)),
    );
    let verdict = ask(&pair);
    // the decision and the name are the second gate's, the reasons are the two lists
    assert!(verdict.restructuring_required);
    assert_eq!(verdict.gate, "s1");
    assert_eq!(
        blocker_reasons(&verdict),
        ["row: apply-check refuses", "row: s1 refuses"]
    );
    assert!(pair.take_structure().is_none());
    assert!(!pair.judges_deleted_row());
}

/// A gate with a given number of refusals.
struct Listing {
    rows: usize,
    omitted: usize,
}

impl StructuralGate for Listing {
    fn name(&self) -> &'static str {
        "listing"
    }
    fn check(&self, _input: &GateInput<'_>) -> Result<GateVerdict> {
        let mut verdict = GateVerdict {
            gate: "listing".to_owned(),
            ..GateVerdict::default()
        };
        for row in 0..self.rows {
            verdict.block(&format!("row{row}"), "listing refuses");
        }
        verdict.blockers_omitted += self.omitted;
        Ok(verdict)
    }
}

#[test]
fn the_two_lists_are_capped_and_the_rest_is_counted() {
    let pair = FirstThen::new(
        Box::new(Listing {
            rows: MAX_LISTED_BLOCKERS - 1,
            omitted: 7,
        }),
        Box::new(Scripted::new("s1", true)),
    );
    let verdict = ask(&pair);
    assert!(verdict.restructuring_required);
    // the first's 199 lines and the second's one fill the list; nothing is lost silently
    assert_eq!(verdict.blockers.len(), MAX_LISTED_BLOCKERS);
    assert_eq!(verdict.blockers[0].row, "row0");
    assert_eq!(verdict.blockers[MAX_LISTED_BLOCKERS - 1].row, "row");
    assert_eq!(verdict.blockers_omitted, 7);

    let pair = FirstThen::new(
        Box::new(Listing {
            rows: MAX_LISTED_BLOCKERS,
            omitted: 0,
        }),
        Box::new(Scripted::new("s1", true)),
    );
    let verdict = ask(&pair);
    assert_eq!(verdict.blockers.len(), MAX_LISTED_BLOCKERS);
    assert_eq!(verdict.blockers_omitted, 1);
}
