//! Actual owner/sealing handlers with only the original driver boundary
//! substituted. No SQL connection, native process, or publication receipt.
use super::*;
use crate::sql::{ScriptVariables, SqlExec, SqlLogin, SqlTarget, SqlTools, SqlValue};
use std::cell::{Cell, RefCell};
use std::io::{Error, Read};
use std::rc::Rc;

#[derive(Clone, Copy, Default)]
enum Fault {
    #[default]
    None,
    Boot,
    Commit,
    ForeignTerminal,
    MissingTerminal,
    Create,
    RowColumn(usize),
    MissingRow,
    ExtraRow,
    WrongDomain,
    PaddedName,
    LateTerminal,
}
#[derive(Default)]
struct Boundary {
    calls: Vec<String>,
    domains: Vec<SqlRow>,
    rows: Vec<SqlRow>,
    fault: Fault,
    insert_bytes: usize,
    insert_rows: usize,
}
struct Driver {
    memory: Rc<RefCell<Boundary>>,
    drops: Rc<Cell<usize>>,
}
impl Drop for Driver {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
fn value(param: SqlParam<'_>) -> SqlValue {
    match param {
        SqlParam::U8(x) => SqlValue::Int(i64::from(x)),
        SqlParam::I32(x) => SqlValue::Int(i64::from(x)),
        SqlParam::I64(x) => SqlValue::Int(x),
        SqlParam::Text(x) => SqlValue::Text(x.to_owned()),
        SqlParam::Binary(x) => SqlValue::Binary(x.to_vec()),
    }
}
impl OriginalSqlDriver for Driver {
    fn simple_rows(&mut self, sql: &str, each: &mut dyn FnMut(SqlRow) -> Result<()>) -> Result<()> {
        self.read_rows(sql, &[], each)
    }

    fn run_batch(&mut self, sql: &str) -> Result<()> {
        let mut m = self.memory.borrow_mut();
        m.calls.push(sql.to_owned());
        if matches!(m.fault, Fault::Create) && sql == CREATE_RELATIONS {
            bail!("driver lost CREATE response");
        }
        Ok(())
    }
    fn execute(&mut self, sql: &str, params: &[SqlParam<'_>]) -> Result<u64> {
        let mut m = self.memory.borrow_mut();
        m.calls.push(sql.to_owned());
        if sql == INSERT_DOMAIN {
            m.domains.push(SqlRow {
                result_set: 0,
                values: params.iter().copied().map(value).collect(),
            });
            return Ok(1);
        }
        ensure!(
            sql.starts_with("INSERT INTO #ibcmd_owned_rows"),
            "unexpected boundary statement"
        );
        ensure!(
            params.len().is_multiple_of(COLUMNS.len()),
            "broken physical parameter shape"
        );
        let mut count = 0;
        for row in params.chunks_exact(COLUMNS.len()) {
            let SqlParam::Binary(bytes) = row[13] else {
                bail!("wrong bound payload type");
            };
            let mut values = row[..6].iter().copied().map(value).collect::<Vec<_>>();
            let SqlParam::Text(name) = row[4] else {
                bail!("wrong name type");
            };
            let name_bytes = utf16_bytes(name)?;
            values.push(SqlValue::Binary(name_bytes.clone()));
            values.push(SqlValue::Int(i64::try_from(name_bytes.len())?));
            values.extend(row[6..13].iter().copied().map(value));
            // A mock driver result is based on actual parameter bytes; the
            // production owner must independently compare every returned field.
            values.push(SqlValue::Binary(Sha256::digest(bytes).to_vec()));
            values.push(SqlValue::Int(i64::try_from(bytes.len())?));
            m.rows.push(SqlRow {
                result_set: 0,
                values,
            });
            m.insert_rows += 1;
            m.insert_bytes = m.insert_bytes.checked_add(bytes.len()).unwrap();
            count += 1;
        }
        Ok(count)
    }
    fn read_rows(
        &mut self,
        sql: &str,
        _params: &[SqlParam<'_>],
        each: &mut dyn FnMut(SqlRow) -> Result<()>,
    ) -> Result<()> {
        let mut m = self.memory.borrow_mut();
        m.calls.push(sql.to_owned());
        if sql == "SELECT @@SPID;" {
            if matches!(m.fault, Fault::Boot) {
                bail!("lost identity response");
            }
            return each(SqlRow {
                result_set: 0,
                values: vec![SqlValue::Int(42)],
            });
        }
        if sql == READ_DOMAINS {
            let mut rows = m.domains.clone();
            rows.sort_by_key(|r| (r.i64(0).unwrap(), r.i64(1).unwrap()));
            if matches!(m.fault, Fault::WrongDomain) && !rows.is_empty() {
                rows[0].values[1] = SqlValue::Int(99);
            }
            for row in rows {
                each(row)?;
            }
            return Ok(());
        }
        if sql == READ_ROWS {
            let mut rows = m.rows.clone();
            match m.fault {
                Fault::RowColumn(n) if !rows.is_empty() => {
                    rows[0].values[n] = match &rows[0].values[n] {
                        SqlValue::Int(x) => SqlValue::Int(x + 1),
                        SqlValue::Text(x) => SqlValue::Text(format!("{x}x")),
                        SqlValue::Binary(x) => {
                            let mut y = x.clone();
                            y[0] ^= 1;
                            SqlValue::Binary(y)
                        }
                        _ => unreachable!(),
                    };
                }
                Fault::MissingRow => {
                    rows.pop();
                }
                Fault::ExtraRow if !rows.is_empty() => rows.push(rows[0].clone()),
                Fault::PaddedName if !rows.is_empty() => {
                    let name = format!("{} ", rows[0].text(4)?);
                    let raw = utf16_bytes(&name)?;
                    rows[0].values[4] = SqlValue::Text(name);
                    rows[0].values[5] = SqlValue::Binary(raw.clone());
                    rows[0].values[6] = SqlValue::Binary(raw.clone());
                    rows[0].values[7] = SqlValue::Int(i64::try_from(raw.len())?);
                }
                _ => {}
            }
            for row in rows {
                each(row)?;
            }
            return Ok(());
        }
        ensure!(
            [BEGIN, COMMIT, ROLLBACK].contains(&sql),
            "unexpected terminal query"
        );
        if sql == COMMIT && matches!(m.fault, Fault::Commit) {
            bail!("lost COMMIT acknowledgement");
        }
        if matches!(m.fault, Fault::MissingTerminal) {
            return Ok(());
        }
        each(SqlRow {
            result_set: 0,
            values: vec![
                SqlValue::Int(if matches!(m.fault, Fault::ForeignTerminal) {
                    43
                } else {
                    42
                }),
                SqlValue::Int(if sql == BEGIN { 1 } else { 0 }),
            ],
        })?;
        if matches!(m.fault, Fault::LateTerminal) {
            bail!("lost final stream after terminal row");
        }
        Ok(())
    }
}
fn owner(fault: Fault) -> (OwnedSqlCommand, Rc<RefCell<Boundary>>, Rc<Cell<usize>>) {
    let memory = Rc::new(RefCell::new(Boundary {
        fault,
        ..Default::default()
    }));
    let drops = Rc::new(Cell::new(0));
    let command = OwnedSqlCommand::from_original(Box::new(Driver {
        memory: memory.clone(),
        drops: drops.clone(),
    }));
    (command, memory, drops)
}
const DOMAIN: InputDomain = InputDomain {
    relation: InputRelation::Proposed,
    table: StorageTable::ConfigSave,
};
const EXPECTED: InputDomain = InputDomain {
    relation: InputRelation::Expected,
    table: StorageTable::Config,
};
fn row<'a>(name: &'a str, binary: &'a [u8]) -> InputRow<'a> {
    InputRow {
        domain: DOMAIN,
        file_name: name,
        part_no: 0,
        creation: "2026-10-09 00:00:00.000",
        modified: "2026-10-09 00:00:00.001",
        attributes: 7,
        data_size: 123,
        binary,
        sha256: Sha256::digest(binary).into(),
    }
}
fn empty() -> SealedSqlInput<'static> {
    SealedSqlInput::new(&[DOMAIN, EXPECTED], []).unwrap()
}

#[test]
fn same_actual_owner_relation_and_known_commit_order() {
    let input = SealedSqlInput::new(&[DOMAIN, EXPECTED], [row("root", b"actual")]).unwrap();
    let (mut command, memory, drops) = owner(Fault::None);
    command.transfer_input(&input).unwrap();
    assert_eq!(command.state(), OwnedSqlState::InputSealed);
    assert_eq!(memory.borrow().insert_rows, 1);
    command.begin().unwrap();
    command.commit().unwrap();
    assert_eq!(command.state(), OwnedSqlState::Committed);
    let m = memory.borrow();
    assert_eq!(m.calls.first().unwrap(), "SELECT @@SPID;");
    assert_eq!(m.calls.last().unwrap(), COMMIT);
    assert_eq!(
        m.calls
            .iter()
            .filter(|x| x.as_str() == CREATE_RELATIONS)
            .count(),
        1
    );
    assert_eq!(m.calls.iter().filter(|x| x.as_str() == BEGIN).count(), 1);
    assert_eq!(m.calls.iter().filter(|x| x.as_str() == COMMIT).count(), 1);
    assert!(!m.calls.iter().any(|x| x.starts_with("USE ")));
    assert_eq!(drops.get(), 0);
    drop(m);
    drop(command);
    assert_eq!(drops.get(), 1);
}
#[test]
fn lost_commit_is_sticky_unknown_and_retains_original_until_host_drop() {
    let (mut command, memory, drops) = owner(Fault::Commit);
    command.transfer_input(&empty()).unwrap();
    command.begin().unwrap();
    assert!(command.commit().is_err());
    assert_eq!(command.state(), OwnedSqlState::Unknown);
    assert!(command.retained_error().is_some());
    assert_eq!(drops.get(), 0);
    let calls = memory.borrow().calls.len();
    assert!(command.rollback().is_err());
    assert!(command.commit().is_err());
    assert!(command.transfer_input(&empty()).is_err());
    assert_eq!(memory.borrow().calls.len(), calls);
    drop(command);
    assert_eq!(drops.get(), 1);
}
#[test]
fn acknowledged_rollback_only_before_any_uncertain_commit() {
    let (mut command, memory, _) = owner(Fault::None);
    command.transfer_input(&empty()).unwrap();
    command.begin().unwrap();
    command.rollback().unwrap();
    assert_eq!(command.state(), OwnedSqlState::RolledBackKnown);
    let calls = memory.borrow().calls.len();
    assert!(command.commit().is_err());
    assert!(command.begin().is_err());
    assert_eq!(memory.borrow().calls.len(), calls);
}
#[test]
fn foreign_or_absent_terminal_never_proves_known_outcome() {
    for fault in [Fault::ForeignTerminal, Fault::MissingTerminal] {
        let (mut command, memory, drops) = owner(Fault::None);
        command.transfer_input(&empty()).unwrap();
        command.begin().unwrap();
        memory.borrow_mut().fault = fault;
        assert!(command.commit().is_err());
        assert_eq!(command.state(), OwnedSqlState::Unknown);
        assert_eq!(drops.get(), 0);
        if matches!(fault, Fault::ForeignTerminal) {
            assert_eq!(command.retained_observation().unwrap().i64(0).unwrap(), 43);
        } else {
            assert!(command.retained_observation().is_none());
        }
    }
}
#[test]
fn lost_initial_identity_or_create_keeps_original_unknown() {
    let (command, _, drops) = owner(Fault::Boot);
    assert_eq!(command.state(), OwnedSqlState::Unknown);
    assert!(command.retained_error().is_some());
    assert_eq!(drops.get(), 0);
    drop(command);
    assert_eq!(drops.get(), 1);
    let (mut command, _, drops) = owner(Fault::Create);
    assert!(command.transfer_input(&empty()).is_err());
    assert_eq!(command.state(), OwnedSqlState::Unknown);
    assert_eq!(drops.get(), 0);
}
#[test]
fn whole_full_header_digest_key_and_extent_negative_controls() {
    let input = SealedSqlInput::new(&[DOMAIN, EXPECTED], [row("root", b"actual")]).unwrap();
    let (mut positive, _, _) = owner(Fault::None);
    positive.transfer_input(&input).unwrap();
    for fault in (0..17).map(Fault::RowColumn).chain([
        Fault::MissingRow,
        Fault::ExtraRow,
        Fault::WrongDomain,
    ]) {
        let (mut command, memory, drops) = owner(fault);
        assert!(command.transfer_input(&input).is_err());
        assert_eq!(command.state(), OwnedSqlState::Unknown);
        assert_eq!(drops.get(), 0);
        let before = memory.borrow().calls.len();
        assert!(command.begin().is_err());
        assert_eq!(memory.borrow().calls.len(), before);
    }
}
#[test]
fn whole_input_sealing_refuses_late_duplicate_alias_foreign_hash_header() {
    let valid = row("Name", b"actual");
    let positive = SealedSqlInput::new(&[DOMAIN], [valid]).unwrap();
    assert_eq!(positive.row_count(), 1);
    assert!(SealedSqlInput::new(&[DOMAIN], [valid, valid]).is_err());
    assert!(SealedSqlInput::new(&[DOMAIN], [valid, row("name", b"actual")]).is_err());
    assert!(SealedSqlInput::new(&[DOMAIN, DOMAIN], []).is_err());
    for bad in [
        InputRow {
            domain: EXPECTED,
            ..valid
        },
        InputRow {
            part_no: -1,
            ..valid
        },
        InputRow {
            data_size: -1,
            ..valid
        },
        InputRow {
            sha256: [0; 32],
            ..valid
        },
        InputRow {
            creation: "1234567890123456789012345678",
            ..valid
        },
        InputRow {
            file_name: "bad\0key",
            ..valid
        },
    ] {
        assert!(SealedSqlInput::new(&[DOMAIN], [valid, bad]).is_err());
    }
    let (command, memory, _) = owner(Fault::None);
    assert_eq!(command.state(), OwnedSqlState::Created);
    assert_eq!(memory.borrow().calls, vec!["SELECT @@SPID;"]);
}
#[test]
fn transport_offset_total_and_empty_domains_are_exact() {
    let input =
        SealedSqlInput::new(&[DOMAIN, EXPECTED], [row("a", b"ab"), row("b", b"cde")]).unwrap();
    assert_eq!(input.row_count(), 2);
    assert_eq!(input.byte_length(), 5);
    assert_eq!(input.domains().len(), 2);
    assert_eq!(input.rows[0].offset, 0);
    assert_eq!(input.rows[1].offset, 2);
    let (mut command, _, _) = owner(Fault::None);
    command.transfer_input(&input).unwrap();
}
struct LateReader {
    bytes: &'static [u8],
    offset: usize,
}
impl Read for LateReader {
    fn read(&mut self, into: &mut [u8]) -> std::io::Result<usize> {
        if self.offset == self.bytes.len() {
            return Err(Error::other("late original input error"));
        }
        let n = (self.bytes.len() - self.offset).min(into.len());
        into[..n].copy_from_slice(&self.bytes[self.offset..self.offset + n]);
        self.offset += n;
        Ok(n)
    }
}
#[test]
fn early_commit_then_late_reader_parse_or_utf8_error_dispatches_no_batch() {
    let (mut command, memory, _) = owner(Fault::None);
    assert!(
        command
            .dispatch_unproven_reader(
                LateReader {
                    bytes: b"COMMIT;\nGO\n",
                    offset: 0
                },
                ScriptVariables::Literal
            )
            .is_err()
    );
    assert!(
        command
            .dispatch_unproven_reader(&b"COMMIT;\nGO\n:r missing"[..], ScriptVariables::Literal)
            .is_err()
    );
    assert!(
        command
            .dispatch_unproven_reader(&b"COMMIT;\nGO\nGO 0"[..], ScriptVariables::Literal)
            .is_err()
    );
    assert!(
        command
            .dispatch_unproven_reader(&b"COMMIT;\nGO\n\xff"[..], ScriptVariables::Literal)
            .is_err()
    );
    assert_eq!(command.state(), OwnedSqlState::Created);
    assert_eq!(memory.borrow().calls, vec!["SELECT @@SPID;"]);
}
#[test]
fn sealed_reader_uses_same_legacy_splitter_raw_hash_and_exact_batch_order() {
    for text in [
        "\u{feff}SELECT 1;\r\nGO 2\r\nSELECT 2",
        "SELECT N'a\nGO\nb';\nGO\n",
        "/* nested /* c */\nGO\n*/\nSELECT [a\nGO\nb];\nGO\n",
        "SELECT 1\nGO TO x",
        "",
        "SELECT $(X)",
    ] {
        let sealed = SealedSqlScript::read(text.as_bytes(), ScriptVariables::Literal).unwrap();
        assert_eq!(
            sealed.batches(),
            crate::sql::mssql::split_batches(text, ScriptVariables::Literal).unwrap()
        );
        assert_eq!(
            sealed.sha256(),
            &<[u8; 32]>::from(Sha256::digest(text.as_bytes()))
        );
        assert_eq!(sealed.byte_length(), text.len());
        let (mut command, memory, _) = owner(Fault::None);
        assert_eq!(
            command.dispatch_unproven_script(&sealed).unwrap(),
            sealed.batches().len()
        );
        assert_eq!(
            memory.borrow().calls[1..],
            sealed
                .batches()
                .iter()
                .map(|b| b.text.clone())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            command.state(),
            if sealed.batches().is_empty() {
                OwnedSqlState::Created
            } else {
                OwnedSqlState::Unknown
            }
        );
        if !sealed.batches().is_empty() {
            assert!(command.commit().is_err());
            assert!(command.rollback().is_err());
        }
    }
}
#[test]
fn arbitrary_acknowledged_commit_never_grants_closed_transaction_authority() {
    let (mut command, memory, _) = owner(Fault::None);
    assert_eq!(
        command
            .dispatch_unproven_reader(
                &b"BEGIN TRAN;\nGO\nCOMMIT;\nGO\n"[..],
                ScriptVariables::Literal
            )
            .unwrap(),
        2
    );
    assert_eq!(command.state(), OwnedSqlState::Unknown);
    let count = memory.borrow().calls.len();
    assert!(command.begin().is_err());
    assert!(command.commit().is_err());
    assert!(command.rollback().is_err());
    assert_eq!(memory.borrow().calls.len(), count);
}
#[test]
fn additive_detached_and_tools_capabilities_refuse_without_dispatch() {
    let detached = SqlExec::detached("no database");
    assert!(detached.open_owned_command().is_err());
    let tools = SqlExec::with_tools(
        SqlTarget {
            server: "unused".to_owned(),
            database: None,
            login: SqlLogin::Integrated,
            trust_server_certificate: false,
        },
        SqlTools::new(std::path::Path::new("not-installed-sqlcmd"), None),
    );
    assert!(tools.open_owned_command().is_err());
}
#[test]
fn actual_129_rows_and_over_32_mib_parameters_have_no_aggregate_ceiling() {
    let bytes = vec![0x73; 260_145];
    let names = (0..129).map(|n| format!("row-{n}")).collect::<Vec<_>>();
    let input = SealedSqlInput::new(&[DOMAIN], names.iter().map(|name| row(name, &bytes))).unwrap();
    assert_eq!(input.row_count(), 129);
    assert!(input.byte_length() > 32 * 1024 * 1024);
    let (mut command, memory, _) = owner(Fault::None);
    command.transfer_input(&input).unwrap();
    let m = memory.borrow();
    assert_eq!(m.insert_rows, 129);
    assert_eq!(m.insert_bytes, 129 * bytes.len());
    assert_eq!(command.state(), OwnedSqlState::InputSealed);
}
#[test]
fn actual_single_over_32_mib_row_is_a_larger_request_not_a_refused_row() {
    let bytes = vec![0x61; 32 * 1024 * 1024 + 1];
    let input = SealedSqlInput::new(&[DOMAIN], [row("large", &bytes)]).unwrap();
    let (mut command, memory, _) = owner(Fault::None);
    command.transfer_input(&input).unwrap();
    assert_eq!(memory.borrow().insert_rows, 1);
    assert_eq!(memory.borrow().insert_bytes, bytes.len());
    assert_eq!(
        memory
            .borrow()
            .calls
            .iter()
            .filter(|x| x.starts_with("INSERT INTO #ibcmd_owned_rows"))
            .count(),
        1
    );
}
#[test]
fn natural_parameter_shape_and_long_unicode_key_keep_exact_typed_values() {
    let name = "ДлинноеИмя".repeat(600);
    let input = SealedSqlInput::new(&[DOMAIN], [row(&name, b"x")]).unwrap();
    let (mut command, memory, _) = owner(Fault::None);
    command.transfer_input(&input).unwrap();
    assert_eq!(memory.borrow().rows[0].text(4).unwrap(), name);
    assert_eq!(COLUMNS.len(), 14);
    let statement = super::super::mssql::insert_statement(ROW_TABLE, &COLUMNS, 1);
    assert_eq!(statement.matches('@').count(), COLUMNS.len());
    assert_eq!(memory.borrow().insert_rows, 1);
}

#[test]
fn trailing_space_raw_name_change_cannot_pass_a_padded_bin2_comparison() {
    let input = SealedSqlInput::new(&[DOMAIN], [row("root", b"same payload")]).unwrap();
    let (mut positive, memory, _) = owner(Fault::None);
    positive.transfer_input(&input).unwrap();
    assert_eq!(
        memory.borrow().rows[0].binary(6).unwrap(),
        utf16_bytes("root").unwrap()
    );
    let (mut changed, _, drops) = owner(Fault::PaddedName);
    assert!(changed.transfer_input(&input).is_err());
    assert_eq!(changed.state(), OwnedSqlState::Unknown);
    assert_eq!(
        changed.retained_observation().unwrap().text(4).unwrap(),
        "root "
    );
    assert_eq!(drops.get(), 0);
    // Retained captured spelling is not normalized: a genuine standalone
    // trailing-space name has different raw bytes and survives transport.
    let literal = SealedSqlInput::new(&[DOMAIN], [row("root ", b"same payload")]).unwrap();
    let (mut command, memory, _) = owner(Fault::None);
    command.transfer_input(&literal).unwrap();
    assert_eq!(
        memory.borrow().rows[0].binary(6).unwrap(),
        utf16_bytes("root ").unwrap()
    );
}

#[test]
fn apparent_zero_transaction_row_then_late_eof_is_still_unknown() {
    let (mut command, memory, drops) = owner(Fault::None);
    command.transfer_input(&empty()).unwrap();
    command.begin().unwrap();
    memory.borrow_mut().fault = Fault::LateTerminal;
    assert!(command.commit().is_err());
    assert_eq!(command.retained_observation().unwrap().i64(1).unwrap(), 0);
    assert_eq!(command.state(), OwnedSqlState::Unknown);
    assert!(command.retained_error().is_some());
    assert_eq!(drops.get(), 0);
    let before = memory.borrow().calls.len();
    assert!(command.rollback().is_err());
    assert!(command.commit().is_err());
    assert_eq!(memory.borrow().calls.len(), before);
}
#[test]
fn checked_parameter_extent_uses_actual_utf16_without_changing_legacy_estimate() {
    let text = "AЯ🙂";
    let parameter = SqlParam::Text(text);
    assert_eq!(parameter.checked_wire_bytes().unwrap(), 8);
    assert_eq!(parameter.wire_bytes(), text.len() * 2);
    assert_eq!(SqlParam::Binary(b"actual").checked_wire_bytes().unwrap(), 6);
}
