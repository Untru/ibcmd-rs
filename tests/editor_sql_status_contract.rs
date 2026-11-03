//! Recording SQL acceptance preparation; no TDS, platform or live write claim.
use anyhow::{Result, bail};
use flate2::{
    Compression,
    read::{DeflateDecoder, GzDecoder},
    write::DeflateEncoder,
};
use ibcmd_rs::{
    cancel, platform,
    sql::{Dbms, ScriptVariables, SqlClient, SqlExec, SqlParam, SqlRow, SqlValue},
    stored_objects::{ConfigSource, StatusKind, StoredConfiguration},
};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

type Rows = BTreeMap<String, Vec<u8>>;
const MODULE: &str = "CommonModule.ФайловаяБазаПроверка";
const DB: &str = "status_contract";

#[derive(Default)]
struct Recording {
    active: Rows,
    stage: Rows,
    queries: Vec<String>,
    fail: bool,
    mutating_attempts: usize,
}
struct ReadOnlySql(Arc<Mutex<Recording>>);

/// This deliberately models only plain one-part fixture rows. It refuses new
/// query shapes instead of silently treating an unimplemented SELECT as empty.
impl SqlClient for ReadOnlySql {
    fn dbms(&self) -> Dbms {
        Dbms::SqlServer
    }
    fn max_connections(&self) -> usize {
        1
    }
    fn run_script(&self, _: &str, _: ScriptVariables) -> Result<()> {
        self.0.lock().unwrap().mutating_attempts += 1;
        bail!("status attempted script")
    }
    fn execute(&self, _: &str, _: &[SqlParam<'_>]) -> Result<u64> {
        self.0.lock().unwrap().mutating_attempts += 1;
        bail!("status attempted execute")
    }
    fn write_rows(&self, _: &str, _: &[&str], _: &[Vec<SqlParam<'_>>]) -> Result<u64> {
        self.0.lock().unwrap().mutating_attempts += 1;
        bail!("status attempted bulk write")
    }
    fn query_json(&self, _: &str) -> Result<Option<String>> {
        bail!("unexpected JSON query")
    }
    fn read_rows(
        &self,
        query: &str,
        params: &[SqlParam<'_>],
        each: &mut dyn FnMut(SqlRow) -> Result<()>,
    ) -> Result<()> {
        assert!(params.is_empty());
        let upper = query.to_ascii_uppercase();
        let forbidden = [
            "INSERT", "UPDATE", "DELETE", "MERGE", "DROP", "ALTER", "CREATE", "EXEC", "EXECUTE",
            "TRUNCATE", "INTO",
        ];
        if upper
            .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
            .any(|token| forbidden.contains(&token))
        {
            bail!("mutating SQL is forbidden in status recording");
        }
        let select = query
            .trim_start()
            .strip_prefix("SET NOCOUNT ON;")
            .unwrap_or(query)
            .trim_start();
        let header = select.starts_with("SELECT FileName AS file_name,");
        let binary = select.starts_with("SELECT FileName, PartNo, DataSize, BinaryData");
        if !header && !binary {
            bail!("unmodeled SQL projection: {select}");
        }
        let mut state = self.0.lock().unwrap();
        state.queries.push(query.into());
        assert!(state.queries.len() <= 2048, "bounded contract query budget");
        if state.fail {
            bail!("injected SQL read failure");
        }
        let config = query.contains("[status_contract].dbo.[Config]");
        let saved = query.contains("[status_contract].dbo.[ConfigSave]");
        if !config && !saved {
            bail!("unexpected database/table");
        }
        let mut rows = if config {
            state.active.clone()
        } else {
            Rows::new()
        };
        if saved {
            rows.extend(state.stage.clone());
        }
        drop(state);
        // Apply only the outer published-name selection, never the overlay's
        // internal predicate. Dynamic aliases and multipart rows are out of scope.
        if let Some((_, clause)) = query.rsplit_once("WHERE FileName IN (") {
            let list = clause.split_once(')').expect("closed IN selection").0;
            let names: Vec<_> = list
                .split(", ")
                .map(|s| {
                    s.strip_prefix("N'")
                        .and_then(|s| s.strip_suffix('\''))
                        .expect("literal name")
                        .replace("''", "'")
                })
                .collect();
            rows.retain(|name, _| names.contains(name));
        } else if let Some((_, clause)) = query.rsplit_once("WHERE FileName >= N'") {
            let (first, rest) = clause
                .split_once("' AND FileName <= N'")
                .expect("bounded range");
            let last = rest.split('\'').next().unwrap();
            rows.retain(|name, _| name.as_str() >= first && name.as_str() <= last);
        } else if query.contains("WHERE CHARINDEX(N'.', FileName) = 0") {
            rows.retain(|name, _| !name.contains('.'));
        } else if query.contains("\nWHERE ") {
            bail!("unmodeled SQL selection");
        }
        for (name, bytes) in rows {
            let mut values = vec![
                SqlValue::Text(name),
                SqlValue::Int(0),
                SqlValue::Int(bytes.len() as i64),
            ];
            if binary {
                values.push(SqlValue::Binary(bytes));
            }
            each(SqlRow {
                result_set: 0,
                values,
            })?;
        }
        Ok(())
    }
}

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("editor-sql-status-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if std::env::var_os("IBCMD_TEST_KEEP_FIXTURES").is_none() {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn tree(root: &Path) -> BTreeMap<PathBuf, String> {
    fn visit(root: &Path, dir: &Path, out: &mut BTreeMap<PathBuf, String>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let metadata = fs::symlink_metadata(&path).unwrap();
            assert!(!metadata.file_type().is_symlink());
            if metadata.is_dir() {
                visit(root, &path, out);
            } else {
                out.insert(
                    path.strip_prefix(root).unwrap().to_owned(),
                    digest(&fs::read(path).unwrap()),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    visit(root, root, &mut out);
    out
}
fn configuration(state: Arc<Mutex<Recording>>) -> StoredConfiguration {
    let sql = SqlExec::with_client(
        SqlExec::detached("recording only; no network")
            .target()
            .clone(),
        Box::new(ReadOnlySql(state)),
    );
    StoredConfiguration::new(
        ConfigSource::Database {
            sql,
            database: DB.into(),
        },
        platform::parse("8.5.1").unwrap(),
    )
}
fn pack(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}
fn changed_versions(rows: &Rows, generation: &str) -> Vec<u8> {
    pack(&ibcmd_rs::metadata_model::root::versions_row(
        &rows.keys().cloned().collect::<Vec<_>>(),
        || generation.to_owned(),
    ))
}

#[test]
fn recording_sql_status_contract() {
    const CHILD: &str = "IBCMD_SQL_STATUS_CONTRACT_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "recording_sql_status_contract", "--nocapture"])
            .env(CHILD, "1")
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success() && stdout.contains("test result: ok. 1 passed; 0 failed;"),
            "{stdout}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let fixture = Fixture::new();
    let public =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/file-infobase/tiny.1CD.gz");
    let mut bytes = Vec::new();
    GzDecoder::new(fs::File::open(public).unwrap())
        .read_to_end(&mut bytes)
        .unwrap();
    fs::write(fixture.join("1Cv8.1CD"), &bytes).unwrap();
    let mut db = ibcmd_filedb::FileDb::open(&fixture.join("1Cv8.1CD")).unwrap();
    let mut active = Rows::new();
    for row in db.config_rows("CONFIG").unwrap() {
        assert_eq!(row.part_no, 0);
        active.insert(row.file_name, row.binary);
    }
    drop(db);
    let state = Arc::new(Mutex::new(Recording {
        active,
        ..Recording::default()
    }));
    let mut config = configuration(state.clone());
    let source = fixture.join("source");
    let baseline = fixture.join("baseline");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&baseline).unwrap();
    let names = vec![MODULE.into()];
    let progress = |_, _, _: &str| {};
    let exported = config
        .main_view_mut()
        .export_complete(&names, &progress)
        .unwrap();
    assert!(!exported[0].files.is_empty());
    for (path, data) in &exported[0].files {
        let target = source.join(path);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(target, data).unwrap();
    }
    let reference = config.native_config_dump_info().unwrap();
    let reference_path = baseline.join("ConfigDumpInfo.xml");
    fs::write(&reference_path, &reference).unwrap();
    let before = tree(&source);
    let same = config
        .status_with_versions(Some(&names), &source, &baseline, &progress)
        .unwrap();
    assert_eq!(same.objects.len(), 1);
    assert_eq!(same.objects[0].status, StatusKind::Same);
    let evidence = same.objects[0].database_version_status.as_ref().unwrap();
    assert!(evidence.conclusive);
    assert_eq!(evidence.changed, Some(false));
    assert_eq!(evidence.object_version, same.objects[0].version);
    assert_eq!(evidence.database_snapshot, same.database_snapshot);
    assert_eq!(evidence.baseline_identity, digest(&reference));
    assert_eq!(tree(&source), before, "status must never mutate source");

    state.lock().unwrap().queries.clear();
    config.children(None).unwrap();
    assert!(
        !state
            .lock()
            .unwrap()
            .queries
            .iter()
            .any(|q| q.contains("[ConfigSave]")),
        "active tree must remain active"
    );
    state.lock().unwrap().queries.clear();
    config.main_view_mut().children(None).unwrap();
    assert!(
        state
            .lock()
            .unwrap()
            .queries
            .iter()
            .any(|q| q.contains("[ConfigSave]")),
        "main tree probes stage"
    );

    let module_path = exported[0]
        .files
        .keys()
        .find(|path| path.ends_with("Module.bsl"))
        .unwrap();
    let original = fs::read(source.join(module_path)).unwrap();
    let mut edit = original.clone();
    edit.extend_from_slice(b"\n// local contract edit\n");
    fs::write(source.join(module_path), edit).unwrap();
    let local_tree = tree(&source);
    let local = config
        .status_with_versions(Some(&names), &source, &baseline, &progress)
        .unwrap();
    assert_eq!(local.objects[0].status, StatusKind::Modified);
    assert_eq!(
        local.objects[0]
            .database_version_status
            .as_ref()
            .unwrap()
            .changed,
        Some(false)
    );
    assert_eq!(tree(&source), local_tree);

    // A native version change is independent of unchanged exported bytes.
    let snapshot =
        ibcmd_rs::native_export_status::ConfigDumpInfoSnapshot::parse(&reference).unwrap();
    let version = &snapshot
        .entries
        .iter()
        .find(|e| e.name == format!("{MODULE}.Module"))
        .unwrap()
        .config_version;
    let changed_reference = String::from_utf8(reference.clone())
        .unwrap()
        .replace(version, &"0".repeat(version.len()));
    fs::write(&reference_path, &changed_reference).unwrap();
    let both = config
        .status_with_versions(Some(&names), &source, &baseline, &progress)
        .unwrap();
    assert_eq!(both.objects[0].status, StatusKind::Modified);
    assert_eq!(
        both.objects[0]
            .database_version_status
            .as_ref()
            .unwrap()
            .changed,
        Some(true)
    );
    fs::write(source.join(module_path), &original).unwrap();
    let database_only_change = config
        .status_with_versions(Some(&names), &source, &baseline, &progress)
        .unwrap();
    assert_eq!(database_only_change.objects[0].status, StatusKind::Same);
    assert_eq!(
        database_only_change.objects[0]
            .database_version_status
            .as_ref()
            .unwrap()
            .changed,
        Some(true)
    );
    fs::write(&reference_path, &reference).unwrap();

    fs::rename(&source, fixture.join("preserved-source")).unwrap();
    fs::create_dir(&source).unwrap();
    assert_eq!(
        config
            .status_with_versions(Some(&names), &source, &baseline, &progress)
            .unwrap()
            .objects[0]
            .status,
        StatusKind::NotExported
    );
    fs::create_dir_all(source.join("CommonModules")).unwrap();
    fs::write(source.join("CommonModules/LocalOnly.xml"), b"local-only").unwrap();
    let absent = vec!["CommonModule.LocalOnly".into()];
    let retained_local_tree = tree(&source);
    assert_eq!(
        config.status(Some(&absent), &source, &progress).unwrap()[0].status,
        StatusKind::NotInBase
    );

    // The fixture transition models completed staging/apply, never SQL writes.
    {
        let mut s = state.lock().unwrap();
        s.stage = s.active.clone();
        let versions = changed_versions(&s.active, "20000000-0000-4000-8000-000000000001");
        s.stage.insert("versions".into(), versions);
    }
    let staged = config.native_config_dump_info().unwrap();
    assert_ne!(staged, reference);
    {
        let mut s = state.lock().unwrap();
        s.active = std::mem::take(&mut s.stage);
    }
    assert_eq!(
        config.native_config_dump_info().unwrap(),
        staged,
        "modeled apply preserves main native state"
    );
    for unfinished in ["missing-versions", "commit", "payload.new"] {
        {
            let mut s = state.lock().unwrap();
            s.stage = s.active.clone();
            let versions = changed_versions(&s.active, "30000000-0000-4000-8000-000000000001");
            s.stage.insert("versions".into(), versions);
        }
        assert_ne!(
            config.native_config_dump_info().unwrap(),
            staged,
            "complete candidate must differ before refusal control"
        );
        {
            let mut s = state.lock().unwrap();
            if unfinished == "missing-versions" {
                s.stage.remove("versions");
            } else {
                s.stage.insert(unfinished.into(), b"unfinished".to_vec());
            }
        }
        assert_eq!(
            config.native_config_dump_info().unwrap(),
            staged,
            "unfinished {unfinished} must fall back to active inventory"
        );
    }
    state.lock().unwrap().stage.clear();

    let renamed = String::from_utf8(reference.clone())
        .unwrap()
        .replace(MODULE, "CommonModule.Previous");
    let deleted = String::from_utf8(reference.clone()).unwrap().replace("</ConfigVersions>",
        "<Metadata name=\"CommonModule.Removed\" id=\"40000000-0000-4000-8000-000000000001\" configVersion=\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"/></ConfigVersions>");
    for historical in [renamed, deleted] {
        fs::write(&reference_path, historical).unwrap();
        let full = config
            .status_with_versions(Some(&names), &source, &baseline, &progress)
            .unwrap();
        assert!(full.full_required, "rename/delete requires full evidence");
        assert!(full.objects.iter().all(|row| {
            let evidence = row.database_version_status.as_ref().unwrap();
            !evidence.conclusive && evidence.changed.is_none()
        }));
    }
    fs::write(&reference_path, &reference).unwrap();

    state.lock().unwrap().fail = true;
    let error = config
        .status_with_versions(Some(&names), &source, &baseline, &progress)
        .unwrap_err();
    assert!(
        format!("{error:#}").contains("injected SQL read failure"),
        "expected the injected SQL refusal: {error:#}"
    );
    state.lock().unwrap().fail = false;
    let flag = Arc::new(AtomicBool::new(false));
    {
        let _scope = cancel::scope(flag.clone());
        let stop = |_, _, _: &str| flag.store(true, Ordering::Relaxed);
        let error = config
            .status_with_versions(Some(&names), &source, &baseline, &stop)
            .unwrap_err();
        assert!(cancel::is_cancelled(&error), "{error:#}");
    }
    let drift = |_, _, _: &str| fs::write(&reference_path, &changed_reference).unwrap();
    let error = config
        .status_with_versions(Some(&names), &source, &baseline, &drift)
        .unwrap_err();
    assert!(
        error.to_string().contains("changed during status"),
        "{error:#}"
    );
    fs::write(&reference_path, &reference).unwrap();
    let sql_drift = |_, _, _: &str| {
        let mut s = state.lock().unwrap();
        let mut raw = Vec::new();
        DeflateDecoder::new(&s.active["versions"][..])
            .read_to_end(&mut raw)
            .unwrap();
        let text = String::from_utf8(raw).unwrap().replace(
            "20000000-0000-4000-8000-000000000001",
            "30000000-0000-4000-8000-000000000001",
        );
        s.active.insert("versions".into(), pack(text.as_bytes()));
    };
    let error = config
        .status_with_versions(Some(&names), &source, &baseline, &sql_drift)
        .unwrap_err();
    assert!(
        error.to_string().contains("changed during status"),
        "{error:#}"
    );
    assert_eq!(
        fs::read(fixture.join("1Cv8.1CD")).unwrap(),
        bytes,
        "public fixture copy is read-only"
    );
    assert_eq!(
        state.lock().unwrap().mutating_attempts,
        0,
        "all status outcomes must avoid writer entry points"
    );
    assert_eq!(
        tree(&source),
        retained_local_tree,
        "failure/cancel/drift paths must not write source"
    );
}

#[test]
fn recording_client_refuses_writes_and_unknown_reads() {
    let client = ReadOnlySql(Arc::new(Mutex::new(Recording::default())));
    assert!(
        client
            .run_script("anything", ScriptVariables::Literal)
            .is_err()
    );
    assert!(client.execute("anything", &[]).is_err());
    assert!(client.write_rows("anything", &[], &[]).is_err());
    for query in [
        "SELECT 1",
        "UPDATE [status_contract].dbo.[Config] SET BinaryData=0x00",
        "SELECT FileName AS file_name, PartNo AS part_no, DataSize AS data_size FROM [status_contract].dbo.[Config]; DELETE FROM [status_contract].dbo.[ConfigSave]",
    ] {
        assert!(
            client
                .read_rows(query, &[], &mut |_| panic!("refused query emitted rows"))
                .is_err()
        );
    }
}
