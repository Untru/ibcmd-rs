use super::*;
use crate::sql::{Dbms, ScriptVariables, SqlParam, SqlValue};
use flate2::{Compression, write::DeflateEncoder};
use std::io::Write;
use std::sync::Mutex;

fn row(name: &str, bytes: &[u8]) -> MainStorageRow {
    MainStorageRow {
        file_name: name.into(),
        part_no: 0,
        creation: "4026-10-03 00:01:02.000".into(),
        modified: "4026-10-03 00:01:02.000".into(),
        attributes: 0,
        data_size: bytes.len() as u64,
        binary_data: bytes.to_vec(),
    }
}
fn versions(generation: Uuid) -> Vec<u8> {
    let mut w = DeflateEncoder::new(Vec::new(), Compression::default());
    write!(w, "{{1,0,\"\",{generation},0}}").unwrap();
    w.finish().unwrap()
}
fn fixture() -> (LiveArtifact, CapturedImage, CapturedImage) {
    let old = Uuid::new_v4();
    let new = Uuid::new_v4();
    let identity = DatabaseIdentity {
        server: "LOCAL".into(),
        database: "owned_one".into(),
        database_guid: Uuid::new_v4().to_string(),
        family_guid: Uuid::new_v4().to_string(),
        recovery_fork: Uuid::new_v4().to_string(),
    };
    let name = Uuid::new_v4().to_string();
    let pre = vec![
        row("root", b"root-before"),
        row("version", b"version-before"),
        row("versions", &versions(old)),
        row(&name, b"body-before"),
    ];
    let stage = vec![
        row("root", b"root-after"),
        row("version", b"version-after"),
        row("versions", &versions(new)),
        row(&name, b"body-after"),
    ];
    let recovery = crate::mssql_main_activation::MainActivationRecoverySnapshot {
        old_generation: old.to_string(),
        new_generation: new.to_string(),
        overwritten_config_rows: pre.clone(),
        retained_config_rows: vec![],
        prior_config_dynamically_updated: None,
        prior_params_dynamically_updated: Some(row("DynamicallyUpdated", b"exact-marker")),
        staged_rows: stage.clone(),
    };
    let token = hex(&Sha256::digest(serde_json::to_vec(&recovery).unwrap()));
    let artifact = LiveArtifact {
        format: 1,
        sql_engine_version: LIVE_SQL_VERSION.into(),
        verified_platform_profile: "platform-8.3.27.2214".into(),
        storage_schema_sha256: "a".repeat(64),
        operation: Uuid::new_v4().to_string(),
        identity: identity.clone(),
        tail: std::env::temp_dir()
            .join("owned-live-undo.trn")
            .to_string_lossy()
            .into_owned(),
        recovery_token: token,
        cluster_id: Uuid::new_v4().to_string(),
        infobase_id: Uuid::new_v4().to_string(),
        recovery,
    };
    let mut before = CapturedImage {
        identity,
        schema: "a".repeat(64),
        ib_version: 1,
        platform_version_req: 2,
        tables: std::array::from_fn(|_| BTreeMap::new()),
    };
    for r in pre {
        before.tables[0].insert((r.file_name.clone(), r.part_no), r);
    }
    let unrelated = row("unrelated", b"untouched-part");
    before.tables[0].insert((unrelated.file_name.clone(), 0), unrelated);
    for r in &stage {
        before.tables[1].insert((r.file_name.clone(), r.part_no), r.clone());
    }
    let marker = artifact
        .recovery
        .prior_params_dynamically_updated
        .clone()
        .unwrap();
    before.tables[2].insert((marker.file_name.clone(), 0), marker);
    before.tables[2].insert(
        ("siVersions".into(), 0),
        row("siVersions", b"must-not-change"),
    );
    before.tables[3].insert(
        ("extension".into(), 0),
        row("extension", b"must-not-change"),
    );
    let mut after = before.clone();
    for r in stage {
        after.tables[0].insert((r.file_name.clone(), r.part_no), r);
    }
    after.tables[1].clear();
    after.tables[2].remove(&("DynamicallyUpdated".into(), 0));
    (artifact, before, after)
}

#[test]
fn full_capture_refuses_header_digest_ancillary_marker_generation_and_stage_drift() {
    for mode in [
        "config_header",
        "config_payload",
        "params",
        "cas",
        "casseve",
        "unexplained",
        "stage",
        "marker",
        "generation",
        "preimage",
        "coverage",
        "schema",
        "identity",
        "alias",
    ] {
        let (mut a, mut before, mut after) = fixture();
        match mode {
            "config_header" => {
                after.tables[0]
                    .get_mut(&("unrelated".into(), 0))
                    .unwrap()
                    .attributes = 1
            }
            "config_payload" => {
                after.tables[0]
                    .get_mut(&("unrelated".into(), 0))
                    .unwrap()
                    .binary_data = b"other".to_vec()
            }
            "params" => {
                after.tables[2].remove(&("siVersions".into(), 0));
            }
            "cas" => {
                after.tables[3]
                    .get_mut(&("extension".into(), 0))
                    .unwrap()
                    .modified = "different".into()
            }
            "casseve" => {
                after.tables[4].insert(("extra".into(), 0), row("extra", b"extra"));
            }
            "unexplained" => {
                after.tables[0].insert(("other".into(), 0), row("other", b"other"));
            }
            "stage" => after.tables[1] = before.tables[1].clone(),
            "marker" => {
                after.tables[2].insert(
                    ("DynamicallyUpdated".into(), 0),
                    row("DynamicallyUpdated", b"other"),
                );
            }
            "generation" => {
                after.tables[0]
                    .get_mut(&("versions".into(), 0))
                    .unwrap()
                    .binary_data = versions(Uuid::new_v4())
            }
            "preimage" => {
                before.tables[0]
                    .get_mut(&("root".into(), 0))
                    .unwrap()
                    .attributes = 1
            }
            "coverage" => {
                a.recovery.overwritten_config_rows.pop();
                a.recovery_token = hex(&Sha256::digest(serde_json::to_vec(&a.recovery).unwrap()));
            }
            "schema" => after.schema = "b".repeat(64),
            "identity" => after.identity.family_guid = Uuid::new_v4().to_string(),
            "alias" => {
                let r = row("a_dynupdate_x", b"old-history");
                before.tables[0].insert((r.file_name.clone(), 0), r.clone());
                after.tables[0].insert((r.file_name.clone(), 0), r);
            }
            _ => unreachable!(),
        }
        assert!(
            GuardedUndoPlan::from_live(&a, before, after).is_err(),
            "{mode}"
        );
    }
}

#[test]
fn transaction_guards_precede_writes_and_witness_follows_full_restored_postimage() {
    let (a, before, after) = fixture();
    let p = GuardedUndoPlan::from_live(&a, before, after).unwrap();
    let sql = p.render(true).unwrap();
    let first = sql.find("DELETE FROM dbo.[Config]").unwrap();
    for guard in [
        "USE [master]",
        "ibcmd-rs:live:",
        "SERIALIZABLE",
        "undo database identity/state changed",
        "ibcmd-rs:main-activation",
        "undo schema/trigger boundary",
        "no other target SQL sessions",
        "undo LIVE header identity/flags",
        "undo LIVE operation/chain",
        "Config full undo snapshot changed",
        "ConfigCAS full undo snapshot changed",
        "ConfigCASSave full undo snapshot changed",
    ] {
        assert!(sql.find(guard).unwrap() < first, "{guard}");
    }
    assert!(sql.find("sp_addextendedproperty").unwrap() < sql.find("COMMIT TRANSACTION").unwrap());
    let last_cas = sql.rfind("Params full undo snapshot changed").unwrap();
    assert!(first < last_cas && last_cas < sql.find("sp_addextendedproperty").unwrap());
    assert!(sql.contains("DATALENGTH(CONVERT(nvarchar(max),@Witness))"));
    assert!(sql.contains("CONVERT(varbinary(max),CONVERT(nvarchar(max),@Witness))"));
    assert!(sql.contains("COLLATE Latin1_General_100_BIN2"));
    assert!(sql.contains("NameBytes varbinary(256)"));
    assert!(sql.contains("CONVERT(varbinary(256),FileName)="));
    assert!(
        !sql.contains("INSERT dbo.[ConfigSave]") && !sql.contains("DELETE FROM dbo.[ConfigSave]")
    );
    assert!(!sql.contains("INSERT dbo.[ConfigCAS]") && !sql.contains("INSERT dbo.[Files]"));
    assert!(sql.contains("IF @@ROWCOUNT<>0") && sql.contains("IF @@ROWCOUNT<>1"));
    assert!(sql.contains("IF @@TRANCOUNT>0 ROLLBACK; USE [master]"));
    assert!(!sql.contains("ROLLBACK IMMEDIATE") && !sql.contains("ALTER DATABASE"));
}

#[test]
fn repeat_is_readonly_and_requires_exact_witness_plus_all_desired_rows() {
    let (a, before, after) = fixture();
    let p = GuardedUndoPlan::from_live(&a, before, after).unwrap();
    let sql = p.render(false).unwrap();
    assert!(
        !sql.contains("DELETE FROM")
            && !sql.contains("INSERT dbo.")
            && !sql.contains("sp_addextendedproperty")
    );
    assert!(sql.contains("bound undo commit witness missing; no retry authority"));
    assert!(sql.contains("foreign undo witness"));
    for table in TABLES {
        assert!(sql.contains(&format!("{table} full undo snapshot changed")));
    }
    let (_, mut different, after) = fixture();
    different.tables[2]
        .get_mut(&("siVersions".into(), 0))
        .unwrap()
        .creation = "4027-10-03 00:01:02.000".into();
    assert!(GuardedUndoPlan::from_live(&a, different, after).is_err());
}

struct Backend {
    calls: Mutex<Vec<String>>,
    response: Mutex<Option<Result<Vec<SqlRow>>>>,
}
impl SqlClient for Backend {
    fn dbms(&self) -> Dbms {
        Dbms::SqlServer
    }
    fn max_connections(&self) -> usize {
        1
    }
    fn read_rows(
        &self,
        q: &str,
        _: &[SqlParam<'_>],
        f: &mut dyn FnMut(SqlRow) -> Result<()>,
    ) -> Result<()> {
        self.calls.lock().unwrap().push(q.into());
        for row in self
            .response
            .lock()
            .unwrap()
            .take()
            .expect("unexpected SQL dispatch")?
        {
            f(row)?;
        }
        Ok(())
    }
    fn execute(&self, _: &str, _: &[SqlParam<'_>]) -> Result<u64> {
        panic!("undo must retain one session and read commit witness")
    }
    fn run_script(&self, _: &str, _: ScriptVariables) -> Result<()> {
        panic!("unobserved transaction route")
    }
    fn query_json(&self, _: &str) -> Result<Option<String>> {
        panic!("unbounded artifact replay")
    }
    fn write_rows(&self, _: &str, _: &[&str], _: &[Vec<SqlParam<'_>>]) -> Result<u64> {
        panic!("independent sessions cannot own undo transaction")
    }
}
fn backend(response: Result<Vec<SqlRow>>) -> Backend {
    Backend {
        calls: Mutex::new(vec![]),
        response: Mutex::new(Some(response)),
    }
}
fn receipt(p: &GuardedUndoPlan, executed: i64) -> Vec<SqlRow> {
    vec![SqlRow {
        result_set: 0,
        values: vec![SqlValue::Text(p.witness.clone()), SqlValue::Int(executed)],
    }]
}

#[test]
fn backend_consumes_only_exact_single_commit_receipt_and_preserves_errors() {
    let (a, before, after) = fixture();
    let p = GuardedUndoPlan::from_live(&a, before, after).unwrap();
    for mode in [
        "known_commit",
        "known_repeat",
        "lock_refused",
        "postimage_refused",
        "insert_rolledback",
        "lost_commit",
        "bad_receipt",
        "multiple",
        "unexpected_status",
    ] {
        let response = match mode {
            "known_commit" => Ok(receipt(&p, 1)),
            "known_repeat" => Ok(receipt(&p, 0)),
            "bad_receipt" => Ok(vec![SqlRow {
                result_set: 0,
                values: vec![SqlValue::Text("foreign".into()), SqlValue::Int(1)],
            }]),
            "multiple" => Ok([receipt(&p, 1), receipt(&p, 0)].concat()),
            "unexpected_status" => Ok(receipt(&p, 2)),
            _ => Err(anyhow::anyhow!("{mode}")),
        };
        let db = backend(response);
        let result = p.execute(&db, &p.render(true).unwrap());
        assert_eq!(result.is_ok(), mode.starts_with("known_"), "{mode}");
        assert_eq!(db.calls.lock().unwrap().len(), 1);
        if mode == "known_repeat" {
            assert_eq!(result.unwrap(), UndoResult::AlreadyUndone);
        }
    }
}

#[test]
fn timestamp_and_complete_physical_row_parser_refuses_ambiguous_keys_or_overflow() {
    let valid = SqlRow {
        result_set: 0,
        values: vec![
            SqlValue::Int(0),
            SqlValue::Text("payload".into()),
            SqlValue::Int(0),
            SqlValue::Text("4026-10-03 00:00:00.000".into()),
            SqlValue::Text("4026-10-03 00:00:00.000".into()),
            SqlValue::Int(0),
            SqlValue::Int(3),
            SqlValue::Binary(vec![1, 2, 3]),
        ],
    };
    assert!(parse_row(valid.clone()).is_ok());
    for (column, value) in [
        (1, SqlValue::Text("x\n".into())),
        (2, SqlValue::Int(-1)),
        (3, SqlValue::Text("';DROP TABLE dbo.Config;--".into())),
        (5, SqlValue::Int(i64::MAX)),
        (6, SqlValue::Int(-1)),
        (6, SqlValue::Int(2)),
    ] {
        let mut bad = valid.clone();
        bad.values[column] = value;
        assert!(parse_row(bad).is_err());
    }
}

struct ColdHost {
    binding: LifetimeBinding,
    registered: bool,
    loaded: bool,
    cold_reads: Vec<bool>,
}
impl OwnedRuntime for ColdHost {
    fn observe(&mut self) -> Result<super::super::Observation> {
        Ok(super::super::Observation {
            binding: self.binding.clone(),
            registrations: if self.registered {
                std::collections::BTreeSet::from([self.binding.infobase])
            } else {
                Default::default()
            },
            loaded: if self.loaded {
                std::collections::BTreeSet::from([self.binding.infobase])
            } else {
                Default::default()
            },
            worker: self
                .loaded
                .then(|| (Uuid::from_u128(3), self.binding.agent.clone())),
            password_only_admins_exact: true,
            authenticated_inventory_exact: true,
            lease_original_handle_exact: true,
            unknown_administration: false,
        })
    }
    fn register(&mut self) -> Result<Uuid> {
        self.registered = true;
        Ok(self.binding.infobase)
    }
    fn load(&mut self, _: Uuid) -> Result<()> {
        self.loaded = true;
        Ok(())
    }
    fn turn_off(&mut self, _: Uuid, _: &super::super::ProcessIdentity) -> Result<()> {
        bail!("undo must not turn a live worker off")
    }
    fn stop_owned(&mut self, _: &mut super::super::Journal) -> Result<()> {
        Ok(())
    }
    fn require_cold(&mut self) -> Result<()> {
        ensure!(
            !self.cold_reads.is_empty() && self.cold_reads.remove(0),
            "cold process or lease drift"
        );
        Ok(())
    }
}

fn authority(a: &LiveArtifact) -> (super::super::ManagedWorker<ColdHost>, std::path::PathBuf) {
    let identity = super::super::ProcessIdentity {
        pid: 3,
        parent: 2,
        birth_100ns: 100,
        executable: std::env::current_exe().unwrap(),
        command_sha256: "a".repeat(64),
    };
    let binding = LifetimeBinding {
        nonce: Uuid::new_v4(),
        root: std::env::temp_dir(),
        cluster: Uuid::parse_str(&a.cluster_id).unwrap(),
        infobase: Uuid::parse_str(&a.infobase_id).unwrap(),
        database: a.identity.database.clone(),
        agent: identity.clone(),
        ras: identity,
    };
    let path = binding
        .root
        .join(format!("undo-journal-{}.jsonl", binding.nonce));
    let journal = super::super::Journal::create(&path, binding.nonce).unwrap();
    let host = ColdHost {
        binding: binding.clone(),
        registered: false,
        loaded: false,
        cold_reads: vec![true; 12],
    };
    let mut worker =
        super::super::ManagedWorker::from_fresh_creator(host, binding, journal).unwrap();
    worker.register_and_load().unwrap();
    (worker, path)
}

#[test]
fn actual_cold_orchestration_refuses_before_backend_and_never_retries_unknown_commit() {
    for mode in [
        "cold_drift",
        "target_drift",
        "lost_commit",
        "bad_commit_receipt",
        "late_cold_drift",
        "success",
    ] {
        let (a, before, after) = fixture();
        let plan = GuardedUndoPlan::from_live(&a, before, after).unwrap();
        let (mut worker, path) = authority(&a);
        worker.runtime.cold_reads = match mode {
            "cold_drift" => vec![true, false],
            "late_cold_drift" => vec![true, true, false],
            _ => vec![true; 12],
        };
        if mode == "target_drift" {
            worker.binding.database = "another".into();
            worker.runtime.binding.database = "another".into();
        }
        let mut cold = worker.shutdown().unwrap();
        let response = match mode {
            "lost_commit" => Err(anyhow::anyhow!("original COMMIT response lost")),
            "bad_commit_receipt" => Ok(vec![]),
            _ => Ok(receipt(&plan, 1)),
        };
        let client = backend(response);
        let result = plan.undo_once(&mut cold, &client);
        assert_eq!(result.is_ok(), mode == "success", "{mode}");
        assert_eq!(
            client.calls.lock().unwrap().len(),
            usize::from(!matches!(mode, "cold_drift" | "target_drift")),
            "{mode}"
        );
        if mode == "late_cold_drift" {
            assert!(result.unwrap_err().to_string().contains("SQL confirmed"));
        }
        assert!(
            plan.undo_once(&mut cold, &client).is_err(),
            "{mode} must not re-dispatch"
        );
        if mode == "success" {
            let repeat = backend(Ok(receipt(&plan, 0)));
            assert_eq!(
                plan.verify_completed(&mut cold, &repeat).unwrap(),
                UndoResult::AlreadyUndone
            );
            let calls = repeat.calls.lock().unwrap();
            assert_eq!(calls.len(), 1);
            assert!(
                !calls[0].contains("DELETE FROM") && !calls[0].contains("sp_addextendedproperty")
            );
        } else {
            let unused = backend(Ok(receipt(&plan, 0)));
            assert!(plan.verify_completed(&mut cold, &unused).is_err());
            assert!(unused.calls.lock().unwrap().is_empty());
        }
        drop(cold);
        drop(worker);
        let records = std::fs::read_to_string(&path).unwrap();
        if mode == "lost_commit" {
            assert!(records.contains("guarded_undo_sql_unproved"));
        }
        if mode == "late_cold_drift" {
            assert!(records.contains("guarded_undo_sql_confirmed"));
            assert!(!records.contains("rolled_back"));
        }
        std::fs::remove_file(path).unwrap();
    }
}

struct CaptureBackend {
    replies: Mutex<std::collections::VecDeque<Vec<SqlRow>>>,
    calls: std::sync::Arc<Mutex<Vec<String>>>,
}
impl SqlClient for CaptureBackend {
    fn dbms(&self) -> Dbms {
        Dbms::SqlServer
    }
    fn max_connections(&self) -> usize {
        1
    }
    fn read_rows(
        &self,
        query: &str,
        _: &[SqlParam<'_>],
        each: &mut dyn FnMut(SqlRow) -> Result<()>,
    ) -> Result<()> {
        self.calls.lock().unwrap().push(query.into());
        for row in self.replies.lock().unwrap().pop_front().unwrap() {
            each(row)?;
        }
        Ok(())
    }
    fn execute(&self, _: &str, _: &[SqlParam<'_>]) -> Result<u64> {
        panic!("capture never writes")
    }
    fn run_script(&self, _: &str, _: ScriptVariables) -> Result<()> {
        panic!("capture never writes")
    }
    fn query_json(&self, _: &str) -> Result<Option<String>> {
        panic!("unexpected JSON")
    }
    fn write_rows(&self, _: &str, _: &[&str], _: &[Vec<SqlParam<'_>>]) -> Result<u64> {
        panic!("capture never writes")
    }
}

fn probe_rows() -> Vec<SqlRow> {
    let mut lines = vec!["IDENTITY|7|80313".to_owned()];
    for table in [
        "Config",
        "ConfigCAS",
        "ConfigCASSave",
        "ConfigSave",
        "Params",
    ] {
        for (index, col) in [
            "FileName|nvarchar|256|0|0|0",
            "Creation|datetime2|6|19|0|0",
            "Modified|datetime2|6|19|0|0",
            "Attributes|smallint|2|5|0|0",
            "DataSize|bigint|8|19|0|0",
            "BinaryData|varbinary|-1|0|0|0",
            "PartNo|int|4|10|0|0",
        ]
        .iter()
        .enumerate()
        {
            lines.push(format!("COLUMN|{table}|{}|{col}", index + 1));
        }
    }
    lines
        .into_iter()
        .map(|line| SqlRow {
            result_set: 0,
            values: vec![SqlValue::Text(line)],
        })
        .collect()
}
fn captured_row(index: i64, r: &MainStorageRow) -> SqlRow {
    SqlRow {
        result_set: index as usize,
        values: vec![
            SqlValue::Int(index),
            SqlValue::Text(r.file_name.clone()),
            SqlValue::Int(r.part_no as i64),
            SqlValue::Text(r.creation.clone()),
            SqlValue::Text(r.modified.clone()),
            SqlValue::Int(r.attributes as i64),
            SqlValue::Int(r.data_size as i64),
            SqlValue::Binary(r.binary_data.clone()),
        ],
    }
}

#[test]
fn actual_builtin_capture_uses_one_locked_full_query_and_refuses_unknown_or_duplicate_rows() {
    let (a, before, _) = fixture();
    for mode in [
        "full",
        "unknown_table",
        "duplicate",
        "bad_header",
        "bad_profile",
    ] {
        let mut rows = before
            .tables
            .iter()
            .enumerate()
            .flat_map(|(i, rows)| rows.values().map(move |row| captured_row(i as i64, row)))
            .collect::<Vec<_>>();
        let mut probe = probe_rows();
        match mode {
            "unknown_table" => rows[0].values[0] = SqlValue::Int(5),
            "duplicate" => rows.push(rows[0].clone()),
            "bad_header" => rows[0].values[6] = SqlValue::Int(-1),
            "bad_profile" => probe[0].values[0] = SqlValue::Text("IDENTITY|8|80501".into()),
            _ => (),
        }
        let calls = std::sync::Arc::new(Mutex::new(vec![]));
        let client = CaptureBackend {
            replies: Mutex::new([probe, rows].into()),
            calls: calls.clone(),
        };
        let sql = SqlExec::with_client(
            crate::sql::SqlTarget {
                server: "LOCAL".into(),
                database: None,
                login: crate::sql::SqlLogin::Integrated,
                trust_server_certificate: false,
            },
            Box::new(client),
        );
        let image = CapturedImage::capture(&sql, &a.identity);
        assert_eq!(image.is_ok(), mode == "full", "{mode}");
        let calls = calls.lock().unwrap();
        assert_eq!(calls.len(), if mode == "bad_profile" { 1 } else { 2 });
        if mode == "full" {
            let image = image.unwrap();
            assert_eq!(image.tables, before.tables);
            assert_eq!(image.ib_version, 7);
            assert_eq!(image.platform_version_req, 80313);
            assert_eq!(
                image.schema,
                "49ab07a8ddb8c87ae1bdc47b1b5342dc2341dbf99cc777ede57ad8ec539bc0a3"
            );
            assert!(
                calls[1].find("TABLOCKX,HOLDLOCK").unwrap()
                    < calls[1].find("undo schema/trigger boundary").unwrap()
            );
            assert!(
                calls[1]
                    .find("undo database identity/state changed")
                    .unwrap()
                    < calls[1].find("SELECT 0,FileName").unwrap()
            );
            assert!(
                calls[1].contains("BEGIN TRANSACTION") && calls[1].contains("COMMIT TRANSACTION")
            );
            let first_transfer = calls[1].find("SELECT 0,FileName").unwrap();
            for table in TABLES {
                let row_guard = format!(
                    "FROM dbo.[{table}] WITH(UPDLOCK,HOLDLOCK) WHERE DATALENGTH(BinaryData)>{MAX_ROW_BYTES}"
                );
                assert!(calls[1].find(&row_guard).unwrap() < first_transfer);
                let aggregate = format!("FROM dbo.[{table}] WITH(UPDLOCK,HOLDLOCK);\n");
                assert!(calls[1].find(&aggregate).unwrap() < first_transfer);
            }
            assert!(
                calls[1]
                    .find("IF @UndoCaptureRows>50000 OR @UndoCaptureBytes>536870912")
                    .unwrap()
                    < first_transfer
            );
        }
    }
    let tools = SqlExec::with_tools(
        crate::sql::SqlTarget {
            server: "LOCAL".into(),
            database: None,
            login: crate::sql::SqlLogin::Integrated,
            trust_server_certificate: false,
        },
        crate::sql::SqlTools::new(std::path::Path::new("never-spawn-sqlcmd"), None),
    );
    assert!(CapturedImage::capture(&tools, &a.identity).is_err());
}

#[test]
fn effective_session_visibility_is_required_on_each_actual_transaction_census() {
    let (a, before, after) = fixture();
    let plan = GuardedUndoPlan::from_live(&a, before, after).unwrap();
    for write in [false, true] {
        let sql = plan.render(write).unwrap();
        let guard = session_visibility_guard();
        assert!(guard.contains(
            "ISNULL(HAS_PERMS_BY_NAME(NULL,NULL,N'VIEW SERVER PERFORMANCE STATE'),0)<>1"
        ));
        assert_eq!(sql.matches(guard).count(), 2);
        let first_permission = sql.find(guard).unwrap();
        let first_census = sql.find("sys.dm_exec_sessions").unwrap();
        let last_permission = sql.rfind(guard).unwrap();
        let last_census = sql.rfind("sys.dm_exec_sessions").unwrap();
        let commit = sql.find("COMMIT TRANSACTION").unwrap();
        assert!(
            first_permission < first_census
                && last_permission < last_census
                && last_census < commit
        );
        if write {
            assert!(first_census < sql.find("DELETE FROM dbo.[Config]").unwrap());
        }
        // SQL error responses for denied or unknown visibility never become
        // successful commit receipts, even with an otherwise exact plan.
        for message in [
            "permission denied",
            "permission result NULL",
            "partial census refused",
        ] {
            let client = backend(Err(anyhow::anyhow!("{message}")));
            assert!(plan.execute(&client, &sql).is_err());
            assert_eq!(client.calls.lock().unwrap().len(), 1);
        }
    }
}

#[test]
fn capture_bounds_reserve_complete_rows_before_payload_clone_without_large_allocations() {
    assert!(validate_row_size(MAX_ROW_BYTES, MAX_ROW_BYTES as u64).is_ok());
    assert!(validate_row_size(MAX_ROW_BYTES + 1, (MAX_ROW_BYTES + 1) as u64).is_err());
    assert!(validate_row_size(3, 2).is_err());
    assert!(validate_row_size(0, 0).is_ok());
    assert_eq!(
        reserve_capture_budget(MAX_ROWS - 1, MAX_BYTES - 1, 1).unwrap(),
        (MAX_ROWS, MAX_BYTES)
    );
    assert!(reserve_capture_budget(MAX_ROWS, 0, 0).is_err());
    assert!(reserve_capture_budget(1, MAX_BYTES, 1).is_err());
    assert!(reserve_capture_budget(0, 0, MAX_ROW_BYTES + 1).is_err());
    assert!(reserve_capture_budget(usize::MAX, 0, 0).is_err());
    assert!(reserve_capture_budget(0, usize::MAX, 1).is_err());
    // Actual row parsing still accepts the exact borrowed small binary and
    // refuses inconsistent metadata before cloning it into MainStorageRow.
    let mut sql_row = captured_row(0, &row("body", b"abc"));
    sql_row.values[6] = SqlValue::Int(4);
    assert!(parse_row(sql_row).is_err());
}

#[test]
fn exact_write_projection_includes_binary_hex_and_refuses_before_text_allocation() {
    let (a, before, after) = fixture();
    let plan = GuardedUndoPlan::from_live(&a, before, after).unwrap();
    for write in [false, true] {
        let mut size = SqlSize::default();
        plan.render_into(&mut size, write).unwrap();
        let rendered = plan.render(write).unwrap();
        assert_eq!(size.bytes, rendered.len());
    }
    let value = row("a'quoted]key", &[0, 15, 128, 255]);
    let mut projected = SqlSize::default();
    insert_row(&mut projected, "Config", &value).unwrap();
    let mut text = SqlText {
        text: String::with_capacity(projected.bytes),
        projected: projected.bytes,
    };
    insert_row(&mut text, "Config", &value).unwrap();
    assert_eq!(projected.bytes, text.text.len());
    assert!(text.text.contains("N'a''quoted]key'"));
    assert!(text.text.contains("0x000f80ff);"));
    // Numeric reservation exercises the real production hex projection with
    // two legal row lengths; it never creates either a large blob or SQL text.
    let mut too_big = SqlSize::default();
    too_big.add_hex_length(MAX_ROW_BYTES).unwrap();
    assert_eq!(too_big.bytes, MAX_SQL);
    assert!(too_big.add_hex_length(MAX_ROW_BYTES).is_err());
    assert_eq!(too_big.bytes, MAX_SQL);
    assert!(SqlSize::default().add_hex_length(usize::MAX).is_err());
    assert_eq!(bounded_sql_add(MAX_SQL - 1, 1), Some(MAX_SQL));
    assert_eq!(bounded_sql_add(MAX_SQL, 1), None);
    // The actual INSERT path refuses when headers/payload would exceed the
    // accumulated projection, instead of producing an oversized intermediate.
    let mut limited = SqlSize {
        bytes: MAX_SQL - 16,
    };
    assert!(insert_row(&mut limited, "Config", &value).is_err());
    assert!(limited.bytes <= MAX_SQL);
    let mut actual = SqlText {
        text: String::new(),
        projected: 1,
    };
    assert!(actual.binary_hex(&[0]).is_err());
    assert!(actual.text.is_empty());
}
