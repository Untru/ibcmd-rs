use super::super::{
    readset::{Builder, Limits},
    selected::recording_edit,
};
use super::*;
use crate::{
    mssql_live_continue::DatabaseIdentity,
    sql::{ScriptVariables, SqlRow, SqlSession, SqlValue},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};
const BODY: &str = "00000000-0000-4000-8000-000000000101.0";
const NEIGHBOUR: &str = "00000000-0000-4000-8000-000000000102.0";
fn profile() -> Profile {
    Profile {
        identity: DatabaseIdentity {
            server: "recording".into(),
            database: "public_synthetic".into(),
            database_guid: uuid::Uuid::from_u128(1).to_string(),
            family_guid: uuid::Uuid::from_u128(2).to_string(),
            recovery_fork: uuid::Uuid::from_u128(3).to_string(),
        },
        sql_version: "recording-only".into(),
        schema: [11; 32],
    }
}
fn header(tag: u8) -> Vec<u8> {
    let mut h = Vec::new();
    for b in [tag, tag + 1, tag + 2] {
        h.extend_from_slice(&1u64.to_le_bytes());
        h.push(b);
    }
    h
}
fn original() -> Readset {
    let p = profile();
    let mut b = Builder::new(p.identity, p.schema, Limits::default(), Arc::new(|| Ok(()))).unwrap();
    for (table, name, no, size, data, tag) in [
        (Table::Config, BODY, 0, 6, b"abc".as_slice(), 1),
        (Table::Config, BODY, 1, 6, b"def", 1),
        (Table::Config, "versions", 0, 3, b"old", 1),
        (Table::Config, NEIGHBOUR, 0, 3, b"raw", 1),
        (Table::ConfigSave, BODY, 0, 6, b"old", 7),
        (Table::ConfigSave, BODY, 1, 6, b"old", 7),
        (Table::ConfigSave, "versions", 0, 2, b"sv", 7),
        (Table::ConfigSave, NEIGHBOUR, 0, 5, b"stage", 9),
    ] {
        b.push(
            table,
            Input {
                key: Key {
                    file_name: name.into(),
                    part_no: no,
                },
                part: Part {
                    data_size: size,
                    header: header(tag),
                    binary: Arc::new(data.to_vec()),
                },
            },
        )
        .unwrap();
    }
    b.finish().unwrap()
}
fn edit(original: &Readset) -> ModuleEdit {
    recording_edit(
        original,
        BTreeMap::from([
            (BODY.into(), b"XY".to_vec()),
            ("versions".into(), b"newV".to_vec()),
        ]),
    )
}
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Fault {
    #[default]
    None,
    StaleHeader,
    Identity,
    Schema,
    BadSeal,
    Uncommittable,
    ZeroSession,
    OtherSession,
    PostBytes,
    MissingHeader,
    DeleteCount,
    InsertCount,
    ReadFailure,
    CommitLost,
    RollbackFailure,
    CancelTemp,
    CancelPost,
    LateCancel,
}
struct Record {
    db: Readset,
    events: Vec<String>,
    fault: Fault,
    cancel: Arc<AtomicBool>,
}
struct Client {
    state: Arc<Mutex<Record>>,
    opens: AtomicUsize,
}
impl Client {
    fn new(db: Readset, fault: Fault) -> Self {
        Self {
            state: Arc::new(Mutex::new(Record {
                db,
                events: vec![],
                fault,
                cancel: Arc::new(AtomicBool::new(false)),
            })),
            opens: AtomicUsize::new(0),
        }
    }
    fn check(&self) -> Check {
        let cancel = self.state.lock().unwrap().cancel.clone();
        Arc::new(move || {
            if cancel.load(Ordering::SeqCst) {
                anyhow::bail!("recorded-cancel");
            }
            Ok(())
        })
    }
    fn events(&self) -> Vec<String> {
        self.state.lock().unwrap().events.clone()
    }
}
struct Session {
    state: Arc<Mutex<Record>>,
    working: Readset,
    temp: Vec<Input>,
    deleted: BTreeSet<String>,
    reads: usize,
    begun: bool,
    poisoned: bool,
    terminal: bool,
}
fn str_param<'a>(p: &'a [SqlParam<'a>], n: usize) -> &'a str {
    match p[n] {
        SqlParam::Text(v) => v,
        _ => panic!("text parameter"),
    }
}
fn int_param(p: &[SqlParam<'_>], n: usize) -> i64 {
    match p[n] {
        SqlParam::I64(v) => v,
        _ => panic!("integer parameter"),
    }
}
impl Session {
    fn start(&mut self) -> Result<()> {
        ensure!(
            !self.poisoned && !self.terminal,
            "recorded unusable session"
        );
        self.poisoned = true;
        Ok(())
    }
    fn frame_rows(&self, params: &[SqlParam<'_>]) -> Vec<SqlRow> {
        let p = profile();
        let fault = self.state.lock().unwrap().fault;
        let mut frames = vec![SqlRow {
            result_set: 0,
            values: vec![
                SqlValue::Text(if fault == Fault::Identity {
                    "changed".into()
                } else {
                    p.identity.server
                }),
                SqlValue::Text(p.identity.database),
                SqlValue::Text(p.identity.database_guid),
                SqlValue::Text(p.identity.family_guid),
                SqlValue::Text(p.identity.recovery_fork),
                SqlValue::Text(p.sql_version),
                SqlValue::Binary(if fault == Fault::Schema {
                    vec![12; 32]
                } else {
                    p.schema.to_vec()
                }),
            ],
        }];
        let mut counts = [0i64; 2];
        let mut sizes = [0i64; 2];
        for (index, table) in [(0, Table::Config), (1, Table::ConfigSave)] {
            for (key, part) in self.working.parts(table) {
                counts[index] += 1;
                sizes[index] += part.binary.len() as i64;
                let mut offset = 0;
                let mut fields = Vec::new();
                for _ in 0..3 {
                    let len =
                        u64::from_le_bytes(part.header[offset..offset + 8].try_into().unwrap())
                            as usize;
                    offset += 8;
                    fields.push(part.header[offset..offset + len].to_vec());
                    offset += len;
                }
                assert_eq!(offset, part.header.len());
                if fault == Fault::StaleHeader
                    && self.reads == 1
                    && index == 1
                    && key.file_name == NEIGHBOUR
                {
                    fields[0][0] ^= 1;
                }
                let mut data = part.binary.as_ref().clone();
                if fault == Fault::PostBytes
                    && self.reads == 2
                    && index == 1
                    && key.file_name == NEIGHBOUR
                {
                    data[0] ^= 1;
                }
                frames.push(SqlRow {
                    result_set: index + 1,
                    values: vec![
                        SqlValue::Text(key.file_name.clone()),
                        SqlValue::Int(i64::from(key.part_no)),
                        SqlValue::Int(part.data_size as i64),
                        SqlValue::Binary(fields[0].clone()),
                        SqlValue::Binary(fields[1].clone()),
                        SqlValue::Binary(fields[2].clone()),
                        SqlValue::Binary(data),
                    ],
                });
            }
        }
        frames.push(SqlRow {
            result_set: 3,
            values: vec![
                SqlValue::Text(str_param(params, 6).into()),
                SqlValue::Int(counts[0]),
                SqlValue::Int(counts[1]),
                SqlValue::Int(sizes[0]),
                SqlValue::Int(sizes[1]),
                SqlValue::Int(if fault == Fault::BadSeal { 0 } else { 1 }),
                SqlValue::Int(if fault == Fault::Uncommittable { -1 } else { 1 }),
                SqlValue::Int(if fault == Fault::ZeroSession {
                    0
                } else if fault == Fault::OtherSession && self.reads == 2 {
                    86
                } else {
                    85
                }),
            ],
        });
        frames
    }
}
impl SqlSession for Session {
    fn run_batch(&mut self, sql: &str) -> Result<()> {
        self.start()?;
        let fault = self.state.lock().unwrap().fault;
        if sql.starts_with("-- ibcmd-editor:begin") {
            ensure!(!self.begun, "already begun");
            assert!(
                sql.find("USE [master]").unwrap() < sql.find("USE [public_synthetic]").unwrap()
            );
            assert!(sql.contains("sp_getapplock") && !sql.contains("sp_releaseapplock"));
            assert!(sql.contains("@LockOwner='Session'"));
            assert!(sql.contains("backupset") && sql.contains("57269"));
            self.begun = true;
            self.state.lock().unwrap().events.push("begin".into());
        } else if sql == TEMP {
            ensure!(self.begun, "temp before transaction");
            self.state.lock().unwrap().events.push("temp".into());
        } else if sql == COMMIT {
            ensure!(self.begun, "commit without transaction");
            let mut record = self.state.lock().unwrap();
            record.events.push("commit".into());
            record.db = self.working.clone();
            self.begun = false;
            if fault == Fault::CommitLost {
                anyhow::bail!("commit response lost");
            }
            if fault == Fault::LateCancel {
                record.cancel.store(true, Ordering::SeqCst);
            }
        } else {
            anyhow::bail!("unexpected session batch");
        }
        self.poisoned = false;
        Ok(())
    }
    fn execute(&mut self, sql: &str, params: &[SqlParam<'_>]) -> Result<u64> {
        self.start()?;
        ensure!(self.begun, "DML without transaction");
        let fault = self.state.lock().unwrap().fault;
        let name = str_param(params, 0).to_string();
        assert!(sql.contains("CONVERT(varbinary(max),CONVERT(nvarchar(128),"));
        if sql.starts_with("-- ibcmd-editor:populate") {
            assert_eq!(params.len(), 4);
            assert!(sql.contains("@@ROWCOUNT<>1"));
            if fault == Fault::MissingHeader {
                anyhow::bail!("metadata provenance is not unique");
            }
            let metadata = self
                .working
                .parts(Table::ConfigSave)
                .chain(self.working.parts(Table::Config))
                .find(|(key, _)| key.file_name == name && key.part_no == 0)
                .unwrap()
                .1
                .header
                .clone();
            let bytes = match params[2] {
                SqlParam::Binary(v) => v.to_vec(),
                _ => panic!("payload"),
            };
            self.temp.push(Input {
                key: Key {
                    file_name: name,
                    part_no: int_param(params, 3) as u32,
                },
                part: Part {
                    data_size: int_param(params, 1) as u64,
                    header: metadata,
                    binary: Arc::new(bytes),
                },
            });
            let mut r = self.state.lock().unwrap();
            r.events.push("populate".into());
            if fault == Fault::CancelTemp {
                r.cancel.store(true, Ordering::SeqCst);
            }
        } else if sql.starts_with("-- ibcmd-editor:delete") {
            assert_eq!(params.len(), 2);
            assert!(
                sql.contains("DELETE s FROM dbo.ConfigSave s WHERE")
                    && sql.contains("@@ROWCOUNT<>@P2")
            );
            self.state
                .lock()
                .unwrap()
                .events
                .push(format!("delete:{name}"));
            let count = self
                .working
                .parts(Table::ConfigSave)
                .filter(|(key, _)| key.file_name == name)
                .count();
            ensure!(
                count as i64 == int_param(params, 1) && fault != Fault::DeleteCount,
                "delete count changed"
            );
            self.deleted.insert(name);
        } else if sql.starts_with("-- ibcmd-editor:insert") {
            assert_eq!(params.len(), 2);
            assert!(sql.contains("@@ROWCOUNT<>@P2"));
            self.state
                .lock()
                .unwrap()
                .events
                .push(format!("insert:{name}"));
            ensure!(
                self.deleted.contains(&name),
                "insert before selected delete"
            );
            let parts = self
                .temp
                .iter()
                .filter(|input| input.key.file_name == name)
                .map(|input| Input {
                    key: input.key.clone(),
                    part: input.part.clone(),
                })
                .collect::<Vec<_>>();
            ensure!(
                parts.len() as i64 == int_param(params, 1) && fault != Fault::InsertCount,
                "insert count changed"
            );
            self.working = self.working.with_selected_stage(parts)?;
        } else {
            anyhow::bail!("unexpected statement/pool writer");
        }
        self.poisoned = false;
        Ok(0) // NOCOUNT means this driver count is NOT an admission proof.
    }
    fn read_rows(
        &mut self,
        sql: &str,
        params: &[SqlParam<'_>],
        each: &mut dyn FnMut(SqlRow) -> Result<()>,
    ) -> Result<()> {
        self.start()?;
        ensure!(self.begun, "read outside held transaction");
        assert!(!sql.contains("USE __DATABASE__") && !sql.contains("USE ["));
        let config = sql
            .find("FROM dbo.Config WITH (TABLOCKX,HOLDLOCK)")
            .unwrap();
        let saved = sql
            .find("FROM dbo.ConfigSave WITH (TABLOCKX,HOLDLOCK)")
            .unwrap();
        let params_lock = sql
            .find("FROM dbo.Params WITH (TABLOCKX,HOLDLOCK)")
            .unwrap();
        assert!(config < saved && saved < params_lock);
        assert!(sql.contains("snapshot dynamic generation is unsupported"));
        assert!(!sql.contains(" ROLLBACK TRANSACTION;\n SELECT @P7"));
        self.reads += 1;
        self.state.lock().unwrap().events.extend([
            "lock-config".into(),
            "lock-configsave".into(),
            "lock-params".into(),
            format!("read:{}", self.reads),
        ]);
        if self.state.lock().unwrap().fault == Fault::ReadFailure {
            anyhow::bail!("read transport failed");
        }
        for row in self.frame_rows(params) {
            each(row)?;
        }
        let r = self.state.lock().unwrap();
        if r.fault == Fault::CancelPost && self.reads == 2 {
            r.cancel.store(true, Ordering::SeqCst);
        }
        self.poisoned = false;
        Ok(())
    }
    fn rollback_only(&mut self) -> Result<()> {
        self.state.lock().unwrap().events.push("rollback".into());
        ensure!(
            !self.poisoned && self.begun && !self.terminal,
            "rollback protocol/transaction unusable"
        );
        self.terminal = true;
        ensure!(
            self.state.lock().unwrap().fault != Fault::RollbackFailure,
            "rollback acknowledgement lost"
        );
        self.begun = false;
        Ok(())
    }
}
impl SqlClient for Client {
    fn dbms(&self) -> Dbms {
        Dbms::SqlServer
    }
    fn max_connections(&self) -> usize {
        1
    }
    fn with_dedicated_session(
        &self,
        work: &mut dyn FnMut(&mut dyn SqlSession) -> Result<()>,
    ) -> Result<()> {
        self.opens.fetch_add(1, Ordering::SeqCst);
        let working = self.state.lock().unwrap().db.clone();
        let mut session = Session {
            state: self.state.clone(),
            working,
            temp: vec![],
            deleted: BTreeSet::new(),
            reads: 0,
            begun: false,
            poisoned: false,
            terminal: false,
        };
        let result = work(&mut session);
        self.state.lock().unwrap().events.push("close".into());
        if result.is_ok() && session.poisoned {
            anyhow::bail!("swallowed request failure");
        }
        if self.state.lock().unwrap().fault == Fault::LateCancel {
            anyhow::bail!("outer late cancellation");
        }
        result
    }
    fn run_script(&self, _: &str, _: ScriptVariables) -> Result<()> {
        panic!("pooled/script fallback")
    }
    fn execute(&self, _: &str, _: &[SqlParam<'_>]) -> Result<u64> {
        panic!("pooled execute")
    }
    fn read_rows(
        &self,
        _: &str,
        _: &[SqlParam<'_>],
        _: &mut dyn FnMut(SqlRow) -> Result<()>,
    ) -> Result<()> {
        panic!("pooled read")
    }
    fn query_json(&self, _: &str) -> Result<Option<String>> {
        panic!("pooled JSON")
    }
    fn write_rows(&self, _: &str, _: &[&str], _: &[Vec<SqlParam<'_>>]) -> Result<u64> {
        panic!("multi-session bulk writer")
    }
}
fn run(fault: Fault) -> (Report, Client, Readset) {
    let original = original();
    let client = Client::new(original.clone(), fault);
    let report = stage(
        &client,
        &original,
        &edit(&original),
        &profile(),
        &WritePolicy::new(2, 4096).unwrap(),
        client.check(),
        &|| Ok(()),
    );
    (report, client, original)
}
fn single_part_selected_with_multipart_neighbour() -> Readset {
    original()
        .with_selected_stage(
            [
                (BODY, 0, 2, b"st".as_slice(), 7),
                (NEIGHBOUR, 0, 6, b"nei".as_slice(), 9),
                (NEIGHBOUR, 1, 6, b"ghb".as_slice(), 9),
            ]
            .into_iter()
            .map(|(name, number, size, data, tag)| Input {
                key: Key {
                    file_name: name.into(),
                    part_no: number,
                },
                part: Part {
                    data_size: size,
                    header: header(tag),
                    binary: Arc::new(data.to_vec()),
                },
            }),
        )
        .unwrap()
}

#[test]
fn single_part_policy_keeps_multipart_neighbours_and_effective_headers() {
    let before = single_part_selected_with_multipart_neighbour();
    let client = Client::new(before.clone(), Fault::None);
    let report = stage(
        &client,
        &before,
        &edit(&before),
        &profile(),
        &WritePolicy::existing_single_part(4, 4096).unwrap(),
        client.check(),
        &|| Ok(()),
    );
    assert_eq!(report.state, State::Committed, "{:?}", report.error);
    let record = client.state.lock().unwrap();
    assert!(
        before
            .parts(Table::Config)
            .eq(record.db.parts(Table::Config))
    );
    assert!(
        before
            .parts(Table::ConfigSave)
            .filter(|(k, _)| k.file_name == NEIGHBOUR)
            .eq(record
                .db
                .parts(Table::ConfigSave)
                .filter(|(k, _)| k.file_name == NEIGHBOUR))
    );
    let selected: Vec<_> = record
        .db
        .parts(Table::ConfigSave)
        .filter(|(k, _)| k.file_name == BODY)
        .collect();
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].0.part_no, 0);
    assert_eq!(selected[0].1.header, header(7));
    assert_eq!(selected[0].1.binary.as_slice(), b"XY");
}

#[test]
fn single_part_policy_refuses_selected_multipart_or_expansion_before_session() {
    let one = single_part_selected_with_multipart_neighbour();
    let multipart_versions = one
        .with_selected_stage((0..2).map(|no| Input {
            key: Key {
                file_name: "versions".into(),
                part_no: no,
            },
            part: Part {
                data_size: 4,
                header: header(7),
                binary: Arc::new(b"vv".to_vec()),
            },
        }))
        .unwrap();
    for (before, cap) in [(original(), 4), (multipart_versions, 4), (one, 3)] {
        let client = Client::new(before.clone(), Fault::None);
        let report = stage(
            &client,
            &before,
            &edit(&before),
            &profile(),
            &WritePolicy::existing_single_part(cap, 4096).unwrap(),
            client.check(),
            &|| Ok(()),
        );
        assert_eq!(report.state, State::NotPublished);
        assert_eq!(client.opens.load(Ordering::SeqCst), 0);
        assert!(client.events().is_empty());
        before
            .require_equal(&client.state.lock().unwrap().db)
            .unwrap();
    }
}

#[test]
fn commit_is_one_session_preserves_neighbours_and_shrinks_selected_tail() {
    let (report, client, original) = run(Fault::None);
    assert_eq!(report.state, State::Committed, "{:?}", report.error);
    assert_eq!(client.opens.load(Ordering::SeqCst), 1);
    let r = client.state.lock().unwrap();
    assert!(original.parts(Table::Config).eq(r.db.parts(Table::Config)));
    assert!(
        original
            .parts(Table::ConfigSave)
            .filter(|(k, _)| k.file_name == NEIGHBOUR)
            .eq(r
                .db
                .parts(Table::ConfigSave)
                .filter(|(k, _)| k.file_name == NEIGHBOUR))
    );
    let selected =
        r.db.parts(Table::ConfigSave)
            .filter(|(k, _)| k.file_name == BODY)
            .collect::<Vec<_>>();
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].1.binary.as_slice(), b"XY");
    assert_eq!(selected[0].1.header, header(7));
    assert_eq!(
        r.events.iter().filter(|v| v.as_str() == "commit").count(),
        1
    );
    let last_insert = r
        .events
        .iter()
        .rposition(|v| v.starts_with("insert:"))
        .unwrap();
    assert_eq!(r.events[last_insert], "insert:versions");
    assert_eq!(r.events.last().unwrap(), "close");
}
#[test]
fn stale_full_state_and_invalid_identity_schema_or_seal_dispatch_no_dml() {
    for fault in [
        Fault::StaleHeader,
        Fault::Identity,
        Fault::Schema,
        Fault::BadSeal,
        Fault::Uncommittable,
        Fault::ZeroSession,
        Fault::ReadFailure,
    ] {
        let (report, client, _) = run(fault);
        assert_ne!(report.state, State::Committed);
        assert!(report.error.is_some());
        assert!(
            !client
                .events()
                .iter()
                .any(|v| v.starts_with("delete:") || v.starts_with("insert:") || v == "temp")
        );
        assert_eq!(client.opens.load(Ordering::SeqCst), 1);
    }
}
#[test]
fn post_write_mismatch_or_session_change_rolls_back_without_commit() {
    for fault in [Fault::PostBytes, Fault::OtherSession, Fault::CancelPost] {
        let (report, client, original) = run(fault);
        assert_eq!(report.state, State::RolledBack, "{:?}", report.error);
        assert!(!client.events().iter().any(|v| v == "commit"));
        original
            .require_equal(&client.state.lock().unwrap().db)
            .unwrap();
    }
}
#[test]
fn metadata_and_rowcount_failures_never_commit_and_keep_uncertainty() {
    for fault in [Fault::MissingHeader, Fault::DeleteCount, Fault::InsertCount] {
        let (report, client, original) = run(fault);
        assert_eq!(report.state, State::Unknown);
        assert!(report.cleanup_uncertain);
        assert!(!client.events().iter().any(|v| v == "commit"));
        original
            .require_equal(&client.state.lock().unwrap().db)
            .unwrap();
    }
}
#[test]
fn cancellation_at_temp_rolls_back_before_any_application_write() {
    let (report, client, original) = run(Fault::CancelTemp);
    assert_eq!(report.state, State::RolledBack);
    assert!(
        !client
            .events()
            .iter()
            .any(|v| v.starts_with("delete:") || v == "commit")
    );
    original
        .require_equal(&client.state.lock().unwrap().db)
        .unwrap();
}
#[test]
fn commit_response_loss_is_unknown_without_rollback_or_retry() {
    let (report, client, original) = run(Fault::CommitLost);
    assert_eq!(report.state, State::Unknown);
    assert_eq!(client.opens.load(Ordering::SeqCst), 1);
    assert!(!client.events().iter().any(|v| v == "rollback"));
    assert!(
        original
            .require_equal(&client.state.lock().unwrap().db)
            .is_err()
    );
}
#[test]
fn late_cancel_cannot_revoke_acknowledged_commit() {
    let (report, client, _) = run(Fault::LateCancel);
    assert_eq!(report.state, State::Committed);
    assert!(report.error.is_some());
    assert_eq!(client.opens.load(Ordering::SeqCst), 1);
}
#[test]
fn source_guard_and_budget_fail_before_open_and_noop_never_opens() {
    let original = original();
    let client = Client::new(original.clone(), Fault::None);
    let policy = WritePolicy::new(2, 4096).unwrap();
    let r = stage(
        &client,
        &original,
        &edit(&original),
        &profile(),
        &policy,
        client.check(),
        &|| anyhow::bail!("source changed"),
    );
    assert_eq!(r.state, State::NotPublished);
    assert_eq!(client.opens.load(Ordering::SeqCst), 0);
    let r = stage(
        &client,
        &original,
        &edit(&original),
        &profile(),
        &WritePolicy::new(2, 1).unwrap(),
        client.check(),
        &|| Ok(()),
    );
    assert_eq!(r.state, State::NotPublished);
    assert_eq!(client.opens.load(Ordering::SeqCst), 0);
    let empty = recording_edit(&original, BTreeMap::new());
    let r = stage(
        &client,
        &original,
        &empty,
        &profile(),
        &policy,
        client.check(),
        &|| Ok(()),
    );
    assert_eq!(r.state, State::Unchanged);
    assert_eq!(client.opens.load(Ordering::SeqCst), 0);
}
#[test]
fn rollback_failure_is_unknown_and_never_retried() {
    let original = original();
    let client = Client::new(original.clone(), Fault::RollbackFailure);
    let calls = AtomicUsize::new(0);
    let guard = || {
        if calls.fetch_add(1, Ordering::SeqCst) >= 2 {
            anyhow::bail!("final source changed");
        }
        Ok(())
    };
    let r = stage(
        &client,
        &original,
        &edit(&original),
        &profile(),
        &WritePolicy::new(2, 4096).unwrap(),
        client.check(),
        &guard,
    );
    assert_eq!(r.state, State::Unknown);
    assert!(r.cleanup_uncertain);
    assert_eq!(client.opens.load(Ordering::SeqCst), 1);
    assert!(!client.events().iter().any(|v| v == "commit"));
}

#[test]
fn final_guard_cancellation_rolls_back_without_commit_dispatch() {
    let original = original();
    let client = Client::new(original.clone(), Fault::None);
    let cancel = client.state.lock().unwrap().cancel.clone();
    let calls = AtomicUsize::new(0);
    let guard = || {
        if calls.fetch_add(1, Ordering::SeqCst) == 3 {
            cancel.store(true, Ordering::SeqCst);
        }
        Ok(())
    };
    let report = stage(
        &client,
        &original,
        &edit(&original),
        &profile(),
        &WritePolicy::new(2, 4096).unwrap(),
        client.check(),
        &guard,
    );
    assert_eq!(calls.load(Ordering::SeqCst), 4);
    assert_eq!(report.state, State::RolledBack);
    assert!(
        report
            .error
            .unwrap()
            .to_string()
            .contains("recorded-cancel")
    );
    assert_eq!(client.opens.load(Ordering::SeqCst), 1);
    assert!(!client.events().iter().any(|event| event == "commit"));
    original
        .require_equal(&client.state.lock().unwrap().db)
        .unwrap();
}

#[test]
fn parameter_value_budget_rejects_below_and_accepts_exact_or_above() {
    // Longest population request: BODY, two I64 parameters and two payload bytes.
    // Use explicit scalar widths independently of the implementation's sum.
    let exact = BODY.len() * 2 + 8 + 2 + 8;
    for budget in [exact - 1, exact, exact + 1] {
        let original = original();
        let client = Client::new(original.clone(), Fault::None);
        let report = stage(
            &client,
            &original,
            &edit(&original),
            &profile(),
            &WritePolicy::new(2, budget).unwrap(),
            client.check(),
            &|| Ok(()),
        );
        if budget < exact {
            assert_eq!(report.state, State::NotPublished);
            assert_eq!(client.opens.load(Ordering::SeqCst), 0);
            original
                .require_equal(&client.state.lock().unwrap().db)
                .unwrap();
        } else {
            assert_eq!(report.state, State::Committed, "{:?}", report.error);
            assert_eq!(client.opens.load(Ordering::SeqCst), 1);
        }
    }
}

// SQL16 measured control: invalid NULL and absent zero never authorize visibility.
const HELD_BIRTH_SQL: &str = "SELECT @@SPID,HAS_PERMS_BY_NAME(NULL,NULL,'VIEW SERVER PERFORMANCE STATE'),CONVERT(varbinary(8),login_time) FROM sys.dm_exec_sessions WHERE session_id=@@SPID";
// One read_rows call retains one observer lease. Twenty observations permit at
// most nineteen 50ms waits; blocking SQL/network/destructor time is not hard-bounded.
const CLOSED_SESSION_SQL: &str = r#"
SET NOCOUNT ON;
DECLARE @Attempt int=0,@Reader int,@Depth int,@State int,@Visible int,@Any bigint,@Owned bigint;
WHILE @Attempt<20
BEGIN
 SELECT @Any=COUNT_BIG(*),@Owned=COUNT_BIG(CASE WHEN CONVERT(varbinary(8),login_time)=@P2 THEN 1 END)
 FROM sys.dm_exec_sessions WHERE session_id=@P1;
 SET @Visible=HAS_PERMS_BY_NAME(NULL,NULL,'VIEW SERVER PERFORMANCE STATE');
 SET @Reader=@@SPID;
 SET @Depth=@@TRANCOUNT;
 SET @State=XACT_STATE();
 SELECT @Reader,@Depth,@State,@Visible,@Any,@Owned;
 SET @Attempt+=1;
 IF @P1<=0 OR @P3<=0 OR @Reader<>@P3 OR @Reader=@P1 OR @Depth<>0 OR @State<>0
    OR @Visible IS NULL OR @Visible<>1 OR @Any NOT IN (0,1) OR @Owned NOT IN (0,1)
    OR @Owned>@Any OR @Owned=0 OR @Attempt>=20 BREAK;
 WAITFOR DELAY '00:00:00.050';
END;
"#;
fn require_visibility(visible: SqlValue) -> Result<()> {
    match visible {
        SqlValue::Int(1) => Ok(()),
        SqlValue::Int(0) => anyhow::bail!("server visibility permission is absent"),
        SqlValue::Null => anyhow::bail!("server visibility permission check is invalid"),
        _ => anyhow::bail!("invalid server visibility result"),
    }
}
fn read_session_birth(row: SqlRow, held: i64) -> Result<[u8; 8]> {
    ensure!(row.result_set == 0, "invalid session identity result set");
    let [SqlValue::Int(spid), visible, SqlValue::Binary(birth)]: [SqlValue; 3] = row
        .values
        .try_into()
        .map_err(|_| anyhow::anyhow!("invalid session identity shape"))?
    else {
        anyhow::bail!("invalid session identity types");
    };
    require_visibility(visible)?;
    ensure!(
        held > 0 && held == spid,
        "session identity differs from held capture"
    );
    birth
        .try_into()
        .map_err(|_| anyhow::anyhow!("invalid session birth width"))
}
#[derive(Clone, Copy, Debug)]
struct ClosureFlags {
    held_positive: bool,
    observer_positive: bool,
    observer_distinct: bool,
    observer_unchanged: bool,
    depth_zero: bool,
    state_zero: bool,
    spid_present: bool,
    captured_session_present: bool,
}
struct ClosureObserver {
    held: i64,
    reader: i64,
    samples: usize,
    closed: bool,
    first: Option<ClosureFlags>,
    last: Option<ClosureFlags>,
}
impl ClosureObserver {
    fn new(held: i64, reader: i64) -> Self {
        Self {
            held,
            reader,
            samples: 0,
            closed: false,
            first: None,
            last: None,
        }
    }
    fn accept(&mut self, row: SqlRow) -> Result<()> {
        ensure!(
            !self.closed && self.samples < 20,
            "closure frame after completion or limit"
        );
        ensure!(
            row.result_set == self.samples,
            "closure result sequence changed"
        );
        let [
            SqlValue::Int(reader),
            SqlValue::Int(depth),
            SqlValue::Int(state),
            visible,
            SqlValue::Int(any),
            SqlValue::Int(owned),
        ]: [SqlValue; 6] = row
            .values
            .try_into()
            .map_err(|_| anyhow::anyhow!("invalid closure result shape"))?
        else {
            anyhow::bail!("invalid closure result types");
        };
        require_visibility(visible)?;
        ensure!(
            (0..=1).contains(&any) && (0..=1).contains(&owned) && owned <= any,
            "invalid closure counts"
        );
        let flags = ClosureFlags {
            held_positive: self.held > 0,
            observer_positive: reader > 0,
            observer_distinct: reader != self.held,
            observer_unchanged: reader == self.reader,
            depth_zero: depth == 0,
            state_zero: state == 0,
            spid_present: any == 1,
            captured_session_present: owned == 1,
        };
        ensure!(
            flags.held_positive
                && flags.observer_positive
                && flags.observer_distinct
                && flags.observer_unchanged
                && flags.depth_zero
                && flags.state_zero,
            "dedicated session closure invariant failed: {flags:?}"
        );
        self.first.get_or_insert(flags);
        self.last = Some(flags);
        self.samples += 1;
        self.closed = !flags.captured_session_present;
        Ok(())
    }
    fn finish(&self) -> Result<()> {
        ensure!(
            self.closed && self.samples > 0,
            "captured session closure unconfirmed: samples={} first={:?} last={:?}",
            self.samples,
            self.first,
            self.last
        );
        Ok(())
    }
    fn reused_spid(&self) -> bool {
        self.last
            .is_some_and(|v| v.spid_present && !v.captured_session_present)
    }
}
fn closure_row(index: usize, visible: SqlValue, any: i64, owned: i64) -> SqlRow {
    SqlRow {
        result_set: index,
        values: vec![
            SqlValue::Int(85),
            SqlValue::Int(0),
            SqlValue::Int(0),
            visible,
            SqlValue::Int(any),
            SqlValue::Int(owned),
        ],
    }
}
#[test]
fn closure_observation_samples_state_after_dmv_query() {
    let count = CLOSED_SESSION_SQL.find("SELECT @Any=COUNT_BIG(*)").unwrap();
    let permission = CLOSED_SESSION_SQL
        .find("SET @Visible=HAS_PERMS_BY_NAME")
        .unwrap();
    let reader = CLOSED_SESSION_SQL.find("SET @Reader=@@SPID;").unwrap();
    let depth = CLOSED_SESSION_SQL.find("SET @Depth=@@TRANCOUNT;").unwrap();
    let state = CLOSED_SESSION_SQL.find("SET @State=XACT_STATE();").unwrap();
    let emit = CLOSED_SESSION_SQL
        .find("SELECT @Reader,@Depth,@State,@Visible,@Any,@Owned;")
        .unwrap();
    let wait = CLOSED_SESSION_SQL.find("WAITFOR DELAY").unwrap();
    assert!(count < permission && permission < reader && reader < depth && depth < state);
    assert!(state < emit && emit < wait);
    // Measured SQL16 SELECT assignment reports statement context state=1 even
    // without permission lookup; the oracle must sample in standalone SET.
    assert!(!CLOSED_SESSION_SQL.contains("SELECT @Reader="));
    assert!(!CLOSED_SESSION_SQL.contains("SELECT @Depth="));
    assert!(!CLOSED_SESSION_SQL.contains("SELECT @State="));
}

#[test]
fn closure_permission_oracle_uses_server_scope_and_distinguishes_null_from_zero() {
    assert!(
        CLOSED_SESSION_SQL.contains("HAS_PERMS_BY_NAME(NULL,NULL,'VIEW SERVER PERFORMANCE STATE')")
    );
    assert!(!CLOSED_SESSION_SQL.contains("HAS_PERMS_BY_NAME(NULL,'SERVER'"));
    let once = |visible, remaining, held| -> Result<()> {
        let mut observer = ClosureObserver::new(held, 85);
        observer.accept(closure_row(0, visible, remaining, remaining))?;
        observer.finish()
    };
    assert_eq!(
        once(SqlValue::Null, 0, 86).unwrap_err().to_string(),
        "server visibility permission check is invalid"
    );
    assert_eq!(
        once(SqlValue::Int(0), 0, 86).unwrap_err().to_string(),
        "server visibility permission is absent"
    );
    once(SqlValue::Int(1), 0, 86).unwrap();
    assert!(once(SqlValue::Int(1), 1, 86).is_err());
    assert!(once(SqlValue::Int(1), 0, 85).is_err());
    assert!(once(SqlValue::Int(1), 0, 0).is_err());
}
#[test]
fn closure_observation_requires_disappearance_and_refuses_exhaustion() {
    let mut observer = ClosureObserver::new(86, 85);
    observer
        .accept(closure_row(0, SqlValue::Int(1), 1, 1))
        .unwrap();
    assert!(observer.finish().is_err()); // Pending never means PASS.
    observer
        .accept(closure_row(1, SqlValue::Int(1), 0, 0))
        .unwrap();
    observer.finish().unwrap();
    assert_eq!(observer.samples, 2);
    assert!(
        observer
            .accept(closure_row(2, SqlValue::Int(1), 0, 0))
            .is_err()
    );
    let mut persistent = ClosureObserver::new(86, 85);
    for index in 0..20 {
        persistent
            .accept(closure_row(index, SqlValue::Int(1), 1, 1))
            .unwrap();
    }
    assert!(persistent.finish().is_err());
    assert!(
        persistent
            .accept(closure_row(20, SqlValue::Int(1), 0, 0))
            .is_err()
    );
}
#[test]
fn closure_observation_rejects_changed_reader_state_and_malformed_sequence() {
    for (column, value) in [
        (0, 84),
        (0, 0),
        (0, 86),
        (1, 1),
        (2, -1),
        (2, 1),
        (3, 0),
        (4, 2),
        (5, -1),
    ] {
        let mut observer = ClosureObserver::new(86, 85);
        observer
            .accept(closure_row(0, SqlValue::Int(1), 1, 1))
            .unwrap();
        let mut bad = closure_row(1, SqlValue::Int(1), 0, 0);
        bad.values[column] = SqlValue::Int(value);
        assert!(observer.accept(bad).is_err());
        assert!(observer.finish().is_err());
    }
    let mut observer = ClosureObserver::new(86, 85);
    assert!(
        observer
            .accept(closure_row(1, SqlValue::Int(1), 0, 0))
            .is_err()
    );
    assert!(
        observer
            .accept(closure_row(0, SqlValue::Int(1), 0, 1))
            .is_err()
    );
}
#[test]
fn closure_birth_binding_preserves_visibility_and_identifies_reused_spid() {
    let birth = |id, visible, bytes| SqlRow {
        result_set: 0,
        values: vec![SqlValue::Int(id), visible, SqlValue::Binary(bytes)],
    };
    assert_eq!(
        read_session_birth(birth(86, SqlValue::Int(1), vec![7; 8]), 86).unwrap(),
        [7; 8]
    );
    assert!(read_session_birth(birth(86, SqlValue::Int(1), vec![7; 7]), 86).is_err());
    assert!(read_session_birth(birth(85, SqlValue::Int(1), vec![7; 8]), 86).is_err());
    assert!(read_session_birth(birth(86, SqlValue::Null, vec![7; 8]), 86).is_err());
    let mut observer = ClosureObserver::new(86, 85);
    observer
        .accept(closure_row(0, SqlValue::Int(1), 1, 0))
        .unwrap();
    observer.finish().unwrap();
    assert!(observer.reused_spid());
    // A birth collision conservatively stays Pending rather than accepting.
    let mut collision = ClosureObserver::new(86, 85);
    collision
        .accept(closure_row(0, SqlValue::Int(1), 1, 1))
        .unwrap();
    assert!(collision.finish().is_err());
}

// Operator-only read/rollback gate. No population, application DML or COMMIT.
#[test]
#[ignore = "sole operator: exact disposable public SQL identity, TLS and idle window required"]
fn public_sql_held_capture_rolls_back_and_closes_dedicated_session() -> Result<()> {
    use crate::sql::{SqlLogin, SqlTarget, mssql::MssqlClient};
    use std::time::{Duration, Instant};
    ensure!(
        cfg!(windows),
        "public integrated-auth acceptance requires Windows"
    );
    const DATABASE: &str = "IBCMDLAB_PUBLIC_SQL_151c08962516426e96ccf4e48d240ff9";
    const GUID: &str = "9000716B-B12F-45BE-BB5B-27FB88534F95";
    ensure!(
        std::env::var("IBCMD_SQL_IMPORT_PUBLIC_FIXTURE").as_deref() == Ok(GUID),
        "explicit disposable public fixture opt-in required"
    );
    let endpoint = std::env::var("IBCMD_SQL_IMPORT_PUBLIC_ENDPOINT")
        .context("operator-measured certificate-valid TCP endpoint required")?;
    ensure!(
        !endpoint.trim().is_empty() && !endpoint.contains(['\0', '\r', '\n']),
        "invalid public SQL endpoint"
    );
    let profile = Profile {
        identity: DatabaseIdentity {
            server: "WIN-LHDQGRLB1L1".into(),
            database: DATABASE.into(),
            database_guid: GUID.into(),
            family_guid: "DF1E79FC-77D9-4DF6-AF59-C753F3AF6EFC".into(),
            recovery_fork: "DF1E79FC-77D9-4DF6-AF59-C753F3AF6EFC".into(),
        },
        sql_version: "16.0.1000.6".into(),
        schema: [
            0xad, 0xa2, 0x1b, 0x51, 0x3d, 0xe5, 0x5c, 0x54, 0x3d, 0xed, 0x7d, 0xe7, 0x24, 0xd1,
            0x34, 0x41, 0x52, 0x31, 0x78, 0xd0, 0x78, 0xd5, 0xf8, 0x58, 0xc9, 0x1d, 0xce, 0x0c,
            0xac, 0x10, 0xb6, 0x26,
        ],
    };
    let client = MssqlClient::new(
        SqlTarget {
            server: endpoint,
            database: Some(DATABASE.into()),
            login: SqlLogin::Integrated,
            trust_server_certificate: false,
        },
        1,
    )?;
    let started = Instant::now();
    let check: Check = Arc::new(move || {
        ensure!(
            started.elapsed() < Duration::from_secs(30),
            "held capture deadline exceeded"
        );
        Ok(())
    });
    let limits = Limits {
        max_parts: 1024,
        max_part_bytes: 1024 * 1024,
        max_payload_bytes: 8 * 1024 * 1024,
        max_metadata_bytes: 262144,
        max_name_units: 128,
    };
    let before = super::super::snapshot::capture(&client, &profile, limits, check.clone())?;
    ensure!(
        before.parts(Table::Config).count() == 8
            && before.parts(Table::ConfigSave).count() == 0
            && before
                .parts(Table::Config)
                .map(|(_, p)| p.binary.len())
                .sum::<usize>()
                == 3313,
        "public fixture differs from the measured baseline"
    );
    let mut baseline_reader = None;
    client.read_rows("SELECT @@SPID", &[], &mut |row| {
        ensure!(
            baseline_reader.is_none() && row.result_set == 0,
            "observer identity repeated"
        );
        let [SqlValue::Int(reader)]: [SqlValue; 1] = row
            .values
            .try_into()
            .map_err(|_| anyhow::anyhow!("invalid observer identity shape"))?
        else {
            anyhow::bail!("invalid observer identity type");
        };
        ensure!(reader > 0, "invalid observer identity");
        baseline_reader = Some(reader);
        Ok(())
    })?;
    let baseline_reader = baseline_reader.context("observer identity missing")?;
    println!(
        "PUBLIC_HELD_PHASE baseline_ms={}",
        started.elapsed().as_millis()
    );
    let mut held_birth = None;
    let mut held_spid = None;
    let mut rollback_ack = false;
    let mut entries = 0usize;
    client.with_dedicated_session(&mut |session| {
        ensure!(entries == 0, "dedicated callback repeated");
        entries += 1;
        check()?;
        session.run_batch(&begin(&profile))?;
        let reads = (|| -> Result<()> {
            let first = capture_held(session, &profile, limits, check.clone())?;
            before.require_equal(&first.readset)?;
            let second = capture_held(session, &profile, limits, check.clone())?;
            ensure!(
                first.session_id > 0 && first.session_id == second.session_id,
                "held captures changed SQL sessions"
            );
            first.readset.require_equal(&second.readset)?;
            session.read_rows(HELD_BIRTH_SQL, &[], &mut |row| {
                ensure!(held_birth.is_none(), "session birth repeated");
                held_birth = Some(read_session_birth(row, first.session_id)?);
                Ok(())
            })?;
            ensure!(held_birth.is_some(), "session birth missing");
            held_spid = Some(first.session_id);
            check()?;
            Ok(())
        })();
        // Even a failed comparison gets one terminal cleanup attempt. An unusable
        // stream may refuse it; success is never claimed without acknowledgement.
        let rollback = session.rollback_only();
        reads?;
        rollback?;
        rollback_ack = true;
        println!(
            "PUBLIC_HELD_PHASE rollback_ack_ms={}",
            started.elapsed().as_millis()
        );
        Ok(())
    })?;
    println!(
        "PUBLIC_HELD_PHASE dedicated_return_ms={}",
        started.elapsed().as_millis()
    );
    ensure!(
        entries == 1 && rollback_ack,
        "held rollback was not acknowledged"
    );
    let spid = held_spid.context("held session seal missing")?;
    check()?;
    let birth = held_birth.context("captured session birth missing")?;
    // Keep one observer lease for the fully drained batch. A reused numeric
    // SPID with a different birth is not the captured session. Timestamp
    // collisions conservatively remain Pending; this is not a TCP-ACK proof.
    let mut observer = ClosureObserver::new(spid, baseline_reader);
    let observing = Instant::now();
    println!(
        "PUBLIC_HELD_PHASE observation_start_ms={}",
        started.elapsed().as_millis()
    );
    let observation_result = client.read_rows(
        CLOSED_SESSION_SQL,
        &[
            SqlParam::I64(spid),
            SqlParam::Binary(&birth),
            SqlParam::I64(baseline_reader),
        ],
        &mut |row| {
            check()?;
            observer.accept(row)
        },
    );
    println!(
        "PUBLIC_HELD_CLOSURE samples={} observation_ms={} batch_ok={} closed_observed={} reused_spid={} first={:?} last={:?}",
        observer.samples,
        observing.elapsed().as_millis(),
        observation_result.is_ok(),
        observer.closed,
        observer.reused_spid(),
        observer.first,
        observer.last
    );
    observation_result?;
    observer.finish()?;
    let after = super::super::snapshot::capture(&client, &profile, limits, check.clone())?;
    before.require_equal(&after)?;
    check()?;
    println!(
        "PUBLIC_HELD_PHASE fresh_capture_ms={}",
        started.elapsed().as_millis()
    );
    drop(client);
    println!(
        "PUBLIC_HELD_PHASE client_dropped_ms={}",
        started.elapsed().as_millis()
    );
    println!(
        "PUBLIC_SQL_HELD_CAPTURE_ROLLBACK_PASS captures=2 same_session=true transaction_seals=true rollback_ack=true session_closed=true readsets_unchanged=true application_dml=false"
    );
    Ok(())
}

fn public_stage_context(
    deadline: std::time::Duration,
) -> Result<(crate::sql::mssql::MssqlClient, Profile, Limits, Check)> {
    use crate::sql::{SqlLogin, SqlTarget, mssql::MssqlClient};
    use std::time::Instant;
    ensure!(
        cfg!(windows),
        "public integrated-auth acceptance requires Windows"
    );
    const DATABASE: &str = "IBCMDLAB_PUBLIC_SQL_151c08962516426e96ccf4e48d240ff9";
    const GUID: &str = "9000716B-B12F-45BE-BB5B-27FB88534F95";
    ensure!(
        std::env::var("IBCMD_SQL_IMPORT_PUBLIC_STAGE_CONFIRM").as_deref() == Ok(GUID),
        "separate disposable staging experiment opt-in required"
    );
    ensure!(
        std::env::var("IBCMD_SQL_IMPORT_PUBLIC_FIXTURE").as_deref() == Ok(GUID),
        "explicit disposable public fixture opt-in required"
    );
    let endpoint = std::env::var("IBCMD_SQL_IMPORT_PUBLIC_ENDPOINT")
        .context("operator-measured certificate-valid TCP endpoint required")?;
    ensure!(
        !endpoint.trim().is_empty() && !endpoint.contains(['\0', '\r', '\n']),
        "invalid public SQL endpoint"
    );
    let profile = Profile {
        identity: DatabaseIdentity {
            server: "WIN-LHDQGRLB1L1".into(),
            database: DATABASE.into(),
            database_guid: GUID.into(),
            family_guid: "DF1E79FC-77D9-4DF6-AF59-C753F3AF6EFC".into(),
            recovery_fork: "DF1E79FC-77D9-4DF6-AF59-C753F3AF6EFC".into(),
        },
        sql_version: "16.0.1000.6".into(),
        schema: [
            0xad, 0xa2, 0x1b, 0x51, 0x3d, 0xe5, 0x5c, 0x54, 0x3d, 0xed, 0x7d, 0xe7, 0x24, 0xd1,
            0x34, 0x41, 0x52, 0x31, 0x78, 0xd0, 0x78, 0xd5, 0xf8, 0x58, 0xc9, 0x1d, 0xce, 0x0c,
            0xac, 0x10, 0xb6, 0x26,
        ],
    };
    let client = MssqlClient::new(
        SqlTarget {
            server: endpoint,
            database: Some(DATABASE.into()),
            login: SqlLogin::Integrated,
            trust_server_certificate: false,
        },
        1,
    )?;
    let started = Instant::now();
    let check: Check = Arc::new(move || {
        ensure!(
            started.elapsed() < deadline,
            "held capture deadline exceeded"
        );
        Ok(())
    });
    let limits = Limits {
        max_parts: 1024,
        max_part_bytes: 1024 * 1024,
        max_payload_bytes: 8 * 1024 * 1024,
        max_metadata_bytes: 262144,
        max_name_units: 128,
    };
    Ok((client, profile, limits, check))
}

// Operator-only, separate permission and opt-in AFTER the corrected held gate.
// Never executed by CI. No native activation or production route is enabled.
#[test]
#[ignore = "sole operator AFTER held PASS: explicit public staging experiment, native writer profile unadmitted"]
fn public_sql_existing_module_stage_experiment() -> Result<()> {
    let (client, profile, limits, check) =
        public_stage_context(std::time::Duration::from_secs(30))?;
    let before = super::super::snapshot::capture(&client, &profile, limits, check.clone())?;
    ensure!(
        before.parts(Table::Config).count() == 8
            && before.parts(Table::ConfigSave).count() == 0
            && before
                .parts(Table::Config)
                .map(|(_, p)| p.binary.len())
                .sum::<usize>()
                == 3313,
        "public fixture differs from the measured baseline"
    );
    // Immutable literal input, not editor source-custody admission. The codec
    // coordinate is the declared seed codec; native 2342 is not writer-admitted.
    const TEXT: &[u8] = b"Procedure PublicGuardedStage() Export\nEndProcedure\n";
    let registry = crate::profile_registry::load_bundled_profile_registry()?;
    let codec_id = ibcmd_core::artifact::ProfileId::parse("platform-8.3.27.1989")?;
    let codec = crate::compiler::families::modules::CommonModuleProfile::from_effective(
        registry
            .get(&codec_id)
            .context("seed codec profile missing")?,
    )?;
    let edit = super::super::selected::prepare_module(
        &before,
        "CommonModules/Portable/Ext/Module.bsl",
        TEXT,
        &codec,
        super::super::selected::Limits::default(),
        &check,
    )?;
    let route = crate::compiler::families::assets::SourceAssetRegistry
        .route(
            "CommonModule",
            crate::compiler::families::assets::SourceAssetRole::Module,
        )
        .context("CommonModule body route missing")?;
    let body = format!("20000000-0000-4000-8000-000000000001{}", route.suffix());
    ensure!(
        edit.replacements().keys().cloned().collect::<BTreeSet<_>>()
            == BTreeSet::from([body.clone(), "versions".into()]),
        "public experiment selected delta differs"
    );
    let policy = WritePolicy::new(1024 * 1024, 4 * 1024 * 1024)?;
    let expected = physical_plan(&before, &edit, &policy, &check)?.expected;
    let report = stage(
        &client,
        &before,
        &edit,
        &profile,
        &policy,
        check.clone(),
        &|| Ok(()),
    );
    // Print outcome BEFORE assertions: committed-but-verification-failed and
    // Unknown must never be reported as no write or automatically replayed.
    println!(
        "PUBLIC_SQL_EXPERIMENTAL_STAGE_OUTCOME state={:?} cleanup_uncertain={} automatic_retry=false",
        report.state, report.cleanup_uncertain
    );
    ensure!(
        report.state == State::Committed,
        "experimental staging was not confirmed committed"
    );
    ensure!(
        report.error.is_none() && !report.cleanup_uncertain,
        "committed experiment needs reconciliation"
    );
    let after = super::super::snapshot::capture(&client, &profile, limits, check.clone())?;
    expected.require_equal(&after)?;
    ensure!(
        before.parts(Table::Config).eq(after.parts(Table::Config)),
        "experiment changed CONFIG before native apply"
    );
    let mut packed = Vec::new();
    for (_, part) in after
        .parts(Table::ConfigSave)
        .filter(|(key, _)| key.file_name == body)
    {
        check()?;
        packed.extend_from_slice(&part.binary);
    }
    ensure!(
        crate::module_blob::unpack_module_blob_text(&packed)? == TEXT,
        "staged module differs from literal input"
    );
    check()?;
    drop(client);
    println!(
        "PUBLIC_SQL_EXPERIMENTAL_STAGE_PASS experimental_staging_only=true codec_profile=platform-8.3.27.1989 actual_native_build=8.3.27.2342 target_platform_admitted=false native_apply_verified=false nonempty_staging_preservation_measured=false automatic_retry=false"
    );
    Ok(())
}

// Needs a native-prepared real neighbour change. Never seed arbitrary SQL rows
// or count an unchanged overlay as a changed neighbouring object.
#[test]
#[ignore = "sole operator: prepared public neighbour stage, explicit opt-in and exact prestate review"]
fn public_sql_nonempty_stage_cancel_and_stale_preserve_neighbors() -> Result<()> {
    use std::cell::Cell;
    ensure!(
        std::env::var("IBCMD_SQL_IMPORT_PUBLIC_NEIGHBOUR_CONFIRM").as_deref()
            == Ok("9000716B-B12F-45BE-BB5B-27FB88534F95"),
        "separate nonempty-neighbour experiment opt-in required"
    );
    let (client, profile, limits, check) =
        public_stage_context(std::time::Duration::from_secs(60))?;
    let before = super::super::snapshot::capture(&client, &profile, limits, check.clone())?;
    ensure!(
        before.parts(Table::Config).count() == 8
            && before
                .parts(Table::Config)
                .map(|(_, part)| part.binary.len())
                .sum::<usize>()
                == 3327,
        "active public fixture differs from confirmed native activation"
    );
    let route = crate::compiler::families::assets::SourceAssetRegistry
        .route(
            "CommonModule",
            crate::compiler::families::assets::SourceAssetRole::Module,
        )
        .context("CommonModule body route missing")?;
    let body = format!("20000000-0000-4000-8000-000000000001{}", route.suffix());
    let selected = BTreeSet::from([body.clone(), "versions".to_owned()]);
    let neighbours: Vec<_> = before
        .parts(Table::ConfigSave)
        .filter(|(key, _)| !selected.contains(&key.file_name))
        .collect();
    ensure!(
        !neighbours.is_empty(),
        "nonempty unselected staging is required"
    );
    ensure!(
        neighbours.iter().any(|(key, part)| {
            before
                .parts(Table::Config)
                .find(|(old, _)| old == key)
                .is_none_or(|(_, old)| old.binary != part.binary || old.data_size != part.data_size)
        }),
        "neighbour staging must contain a real payload change or added object"
    );
    let registry = crate::profile_registry::load_bundled_profile_registry()?;
    let codec = crate::compiler::families::modules::CommonModuleProfile::from_effective(
        registry
            .get(&ibcmd_core::artifact::ProfileId::parse(
                "platform-8.3.27.1989",
            )?)
            .context("seed codec profile missing")?,
    )?;
    let prepare = |text: &[u8]| {
        super::super::selected::prepare_module(
            &before,
            "CommonModules/Portable/Ext/Module.bsl",
            text,
            &codec,
            super::super::selected::Limits::default(),
            &check,
        )
    };
    let edit = prepare(b"Procedure PublicNeighbourStage() Export\nEndProcedure\n")?;
    let stale_edit = prepare(b"Procedure PublicStalePlan() Export\nEndProcedure\n")?;
    ensure!(
        edit.replacements().keys().cloned().collect::<BTreeSet<_>>() == selected
            && stale_edit
                .replacements()
                .keys()
                .cloned()
                .collect::<BTreeSet<_>>()
                == selected,
        "experiment may replace only selected body and versions"
    );
    let policy = WritePolicy::new(1024 * 1024, 4 * 1024 * 1024)?;
    let expected = physical_plan(&before, &edit, &policy, &check)?.expected;

    // The fourth synchronous guard is after DML/post-readset equality and
    // immediately before COMMIT. Refuse there, not after acknowledgement.
    let guards = Cell::new(0usize);
    let cancelled = stage(
        &client,
        &before,
        &edit,
        &profile,
        &policy,
        check.clone(),
        &|| {
            let call = guards.get() + 1;
            guards.set(call);
            if call == 4 {
                return Err(crate::cancel::Cancelled.into());
            }
            ensure!(call < 4, "unexpected cancellation guard sequence");
            Ok(())
        },
    );
    println!(
        "PUBLIC_SQL_NEIGHBOUR_CANCEL_OUTCOME state={:?} cleanup_uncertain={} automatic_retry=false",
        cancelled.state, cancelled.cleanup_uncertain
    );
    ensure!(
        guards.get() == 4
            && cancelled.state == State::RolledBack
            && !cancelled.cleanup_uncertain
            && cancelled
                .error
                .as_ref()
                .is_some_and(crate::cancel::is_cancelled),
        "precommit cancellation did not acknowledge rollback"
    );
    let after_cancel = super::super::snapshot::capture(&client, &profile, limits, check.clone())?;
    before.require_equal(&after_cancel)?;

    let committed = stage(
        &client,
        &before,
        &edit,
        &profile,
        &policy,
        check.clone(),
        &|| Ok(()),
    );
    println!(
        "PUBLIC_SQL_NEIGHBOUR_STAGE_OUTCOME state={:?} cleanup_uncertain={} automatic_retry=false",
        committed.state, committed.cleanup_uncertain
    );
    ensure!(
        committed.state == State::Committed
            && committed.error.is_none()
            && !committed.cleanup_uncertain,
        "neighbour staging commit needs reconciliation"
    );
    let after = super::super::snapshot::capture(&client, &profile, limits, check.clone())?;
    expected.require_equal(&after)?;
    ensure!(
        before.parts(Table::Config).eq(after.parts(Table::Config)),
        "CONFIG changed before native apply"
    );
    ensure!(
        neighbours.iter().all(|(key, part)| after
            .parts(Table::ConfigSave)
            .any(|(found, actual)| found == *key && actual == *part)),
        "an unselected staged header/part/payload changed"
    );

    let expected_conflict = before
        .require_equal(&after)
        .err()
        .context("first commit did not invalidate the old readset")?
        .to_string();
    // A second genuine plan was prepared against the same original snapshot.
    // The acknowledged first commit makes it stale. The held readset CAS must
    // fail before TEMP/population/the third (pre-DML) guard is reached.
    let stale_guards = Cell::new(0usize);
    let rejected = stage(
        &client,
        &before,
        &stale_edit,
        &profile,
        &policy,
        check.clone(),
        &|| {
            stale_guards.set(stale_guards.get() + 1);
            Ok(())
        },
    );
    println!(
        "PUBLIC_SQL_NEIGHBOUR_STALE_OUTCOME state={:?} cleanup_uncertain={} automatic_retry=false",
        rejected.state, rejected.cleanup_uncertain
    );
    ensure!(
        stale_guards.get() == 2
            && rejected.state == State::RolledBack
            && rejected.error.as_ref().map(ToString::to_string).as_deref()
                == Some(expected_conflict.as_str())
            && !rejected.cleanup_uncertain,
        "stale plan did not refuse at held full-readset CAS"
    );
    let final_state = super::super::snapshot::capture(&client, &profile, limits, check.clone())?;
    expected.require_equal(&final_state)?;
    check()?;
    drop(client);
    println!(
        "PUBLIC_SQL_NEIGHBOUR_CASES_PASS cancellation_rollback=true stale_refused=true unselected_stage_exact=true native_apply_verified=false target_platform_admitted=false automatic_retry=false"
    );
    Ok(())
}

#[path = "transaction_selected_acceptance.rs"]
mod selected_acceptance;

// Operator-only PUBLICFORM experiment, bound to the native CF
// eaf574099239a975adb2a301b7e3d73743da00617e5c721a919329e6b5533bad SQL baseline.
// Native export receipt1485ec09/fullrelease3b56d3ab verified the active graph;
// a retained-seal lookup pins this exact single-part Form body without payload egress.
// These bindings are experimental test inputs, not a native writer admission.
#[test]
#[ignore = "sole operator: new native-CF PUBLICFORM baseline and separate staging opt-in required; native writer profile unadmitted"]
fn public_sql_existing_form_module_stage_experiment() -> Result<()> {
    use crate::{
        compiler::bodies::form::decode_native_managed_form_module_layout,
        server::sql_import::{selected::form::prepare_form_module_layout, snapshot},
        sql::{SqlLogin, SqlTarget, mssql::MssqlClient},
    };
    use sha2::{Digest, Sha256};
    use std::time::{Duration, Instant};

    const SERVER: &str = "WIN-LHDQGRLB1L1";
    const DATABASE: &str = "IBCMDLAB_PUBLIC_FORM_SQL_9eb812dd4de64d75bbd9655fd565717d";
    const GUID: &str = "9DCEF2DD-3C19-418C-A4C4-8F1BF5D38B92";
    const FAMILY: &str = "8B053531-FCFB-4564-8BA6-20DDD0CDE200";
    const FORK: &str = "8B053531-FCFB-4564-8BA6-20DDD0CDE200";
    const SQL_VERSION: &str = "16.0.1000.6";
    const PACKED_FORM_BODY_SHA256: [u8; 32] = [
        0x7f, 0x98, 0x90, 0x04, 0xe1, 0xb2, 0x2b, 0xd2, 0x89, 0x6b, 0x74, 0xcd, 0xf8, 0xc4, 0xb2,
        0xdf, 0x9b, 0xc0, 0x56, 0x2e, 0xae, 0x5f, 0x50, 0x69, 0xbe, 0x80, 0x6f, 0xf8, 0xb7, 0x30,
        0x43, 0xff,
    ];
    const PATH: &str = "Documents/ImportDocument/Forms/Main/Ext/Form/Module.bsl";
    const BODY_KEY: &str = "60000000-0000-4000-8000-000000000200.0";
    ensure!(
        cfg!(windows),
        "public integrated-auth acceptance requires Windows"
    );
    ensure!(
        [SERVER, DATABASE, GUID, FAMILY, FORK, SQL_VERSION]
            .iter()
            .all(|value| !value.is_empty())
            && DATABASE.starts_with("IBCMDLAB_PUBLIC_FORM_SQL_")
            && PACKED_FORM_BODY_SHA256 != [0; 32],
        "measured fresh PUBLICFORM baseline bindings are not installed"
    );
    for value in [GUID, FAMILY, FORK] {
        uuid::Uuid::parse_str(value)?;
    }
    ensure!(
        std::env::var("IBCMD_SQL_IMPORT_PUBLIC_FORM_FIXTURE").as_deref() == Ok(GUID)
            && std::env::var("IBCMD_SQL_IMPORT_PUBLIC_FORM_STAGE_CONFIRM").as_deref() == Ok(GUID),
        "separate fresh PUBLICFORM fixture and staging opt-ins required"
    );
    let endpoint = std::env::var("IBCMD_SQL_IMPORT_PUBLIC_ENDPOINT")
        .context("operator-measured certificate-valid TCP endpoint required")?;
    ensure!(
        !endpoint.trim().is_empty() && !endpoint.contains(['\0', '\r', '\n']),
        "invalid public SQL endpoint"
    );
    let started = Instant::now();
    let check: Check = Arc::new(move || {
        ensure!(
            started.elapsed() < Duration::from_secs(60),
            "PUBLICFORM experiment deadline exceeded"
        );
        Ok(())
    });
    let limits = Limits {
        max_parts: 1024,
        max_part_bytes: 1024 * 1024,
        max_payload_bytes: 8 * 1024 * 1024,
        max_metadata_bytes: 262144,
        max_name_units: 128,
    };
    let client = MssqlClient::new(
        SqlTarget {
            server: endpoint,
            database: Some(DATABASE.into()),
            login: SqlLogin::Integrated,
            trust_server_certificate: false,
        },
        1,
    )?;
    // Existing private canonical observer validates only physical schema and
    // execution context; its CommonModule name grants no Form/native admission.
    let observed =
        snapshot::observe_common_module_profile(&client, DATABASE, limits, check.clone())?;
    let profile = observed.snapshot_profile();
    let expected_identity = DatabaseIdentity {
        server: SERVER.into(),
        database: DATABASE.into(),
        database_guid: GUID.into(),
        family_guid: FAMILY.into(),
        recovery_fork: FORK.into(),
    };
    ensure!(
        profile.identity == expected_identity && profile.sql_version == SQL_VERSION,
        "PUBLICFORM observed identity or SQL version differs from measured baseline"
    );
    let before = snapshot::capture(&client, profile, limits, check.clone())?;
    ensure!(
        before.parts(Table::ConfigSave).count() == 0,
        "fresh PUBLICFORM native-applied baseline unexpectedly has staging"
    );
    let selected = before
        .parts(Table::Config)
        .filter(|(key, _)| key.file_name == BODY_KEY)
        .collect::<Vec<_>>();
    ensure!(
        selected.len() == 1 && selected[0].0.part_no == 0,
        "PUBLICFORM baseline selected body is not one existing part"
    );
    let old_packed = selected[0].1.binary.as_slice();
    let actual_hash: [u8; 32] = Sha256::digest(old_packed).into();
    ensure!(
        actual_hash == PACKED_FORM_BODY_SHA256,
        "PUBLICFORM native body pin changed"
    );
    let old = decode_native_managed_form_module_layout(old_packed)?;
    let old_text = old.module_text();
    ensure!(
        old_text.matches("Номер = 1;").count() == 1
            && old_text.contains("Процедура ПриСозданииНаСервере(")
            && old_text.contains("&НаСервере"),
        "PUBLICFORM tiny native handler is not the measured starting module"
    );
    let edited = old_text.replacen("Номер = 1;", "Номер = 2;", 1);
    ensure!(
        edited != old_text && edited.replace("Номер = 2;", "Номер = 1;") == old_text,
        "PUBLICFORM edit changed more than the selected statement"
    );
    // Immutable decoded storage text, NOT filesystem/source-custody admission.
    // Native consumer2342 is distinct from the structural module-only witness.
    let edit = prepare_form_module_layout(
        &before,
        PATH,
        edited.as_bytes(),
        super::super::selected::Limits::default(),
        &check,
    )?;
    ensure!(
        edit.replacements().keys().cloned().collect::<BTreeSet<_>>()
            == BTreeSet::from([BODY_KEY.into(), "versions".into()]),
        "PUBLICFORM compiler changed an unselected row"
    );
    let changed = decode_native_managed_form_module_layout(&edit.replacements()[BODY_KEY])?;
    ensure!(
        changed.module_text() == edited
            && old.parsed().revision == changed.parsed().revision
            && old.parsed().layout == changed.parsed().layout
            && old.parsed().trailing == changed.parsed().trailing
            && old.parsed().trailing_fields == changed.parsed().trailing_fields,
        "PUBLICFORM compiler changed non-module Form state"
    );
    // The pinned native body carries the proven event; exact non-module IR
    // equality preserves its layout/event/attributes without rebuilding Form.xml.
    let versions = before
        .parts(Table::Config)
        .filter(|(key, _)| key.file_name == "versions")
        .collect::<Vec<_>>();
    ensure!(
        versions.len() == 1 && versions[0].0.part_no == 0,
        "PUBLICFORM baseline versions is not one existing part"
    );
    crate::mssql_dump::validate_sql_versions_change(
        &crate::module_blob::inflate_raw(&versions[0].1.binary)?,
        &crate::module_blob::inflate_raw(&edit.replacements()["versions"])?,
        &BTreeSet::from([BODY_KEY.into()]),
    )?;
    let policy = WritePolicy::existing_single_part(1024 * 1024, 4 * 1024 * 1024)?;
    let expected = physical_plan(&before, &edit, &policy, &check)?.expected;
    let report = stage(
        &client,
        &before,
        &edit,
        profile,
        &policy,
        check.clone(),
        &|| Ok(()),
    );
    // Print before assertions: Unknown requires reconciliation, never retry.
    println!(
        "PUBLIC_SQL_FORM_EXPERIMENTAL_STAGE_OUTCOME state={:?} cleanup_uncertain={} automatic_retry=false",
        report.state, report.cleanup_uncertain
    );
    ensure!(
        report.state == State::Committed,
        "PUBLICFORM commit not confirmed; reconcile before any other write; automatic retry forbidden"
    );
    ensure!(
        report.error.is_none() && !report.cleanup_uncertain,
        "PUBLICFORM committed outcome needs reconciliation"
    );
    let after = snapshot::capture(&client, profile, limits, check.clone())?;
    expected.require_equal(&after)?;
    ensure!(
        before.parts(Table::Config).eq(after.parts(Table::Config)),
        "PUBLICFORM staging changed CONFIG before native apply"
    );
    ensure!(
        before
            .parts(Table::ConfigSave)
            .filter(|(key, _)| key.file_name != BODY_KEY && key.file_name != "versions")
            .eq(after
                .parts(Table::ConfigSave)
                .filter(|(key, _)| key.file_name != BODY_KEY && key.file_name != "versions")),
        "PUBLICFORM staging changed an unselected staging entry"
    );
    let staged = after
        .parts(Table::ConfigSave)
        .filter(|(key, _)| key.file_name == BODY_KEY)
        .collect::<Vec<_>>();
    ensure!(
        staged.len() == 1 && staged[0].0.part_no == 0,
        "PUBLICFORM staged body is not one part"
    );
    ensure!(
        staged[0].1.binary.as_slice() == edit.replacements()[BODY_KEY].as_slice(),
        "PUBLICFORM staged body bytes differ from planned compiler output"
    );
    let actual = decode_native_managed_form_module_layout(&staged[0].1.binary)?;
    ensure!(
        actual.parsed() == changed.parsed(),
        "PUBLICFORM staged typed body differs from verified plan"
    );
    check()?;
    drop(client);
    println!(
        "PUBLIC_SQL_FORM_EXPERIMENTAL_STAGE_PASS experimental_staging_only=true codec_profile=native-managed-form-module-envelope-v1 actual_native_build=8.3.27.2342 target_platform_admitted=false native_apply_verified=false form_xml_supported=false initial_staging_empty=true automatic_retry=false"
    );
    Ok(())
}
