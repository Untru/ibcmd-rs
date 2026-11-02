//! Actual Windows FileDbPlan staging on the public tiny 8.5.1 fixture.
//! Each compiler invocation lives in a fresh exact child; no global reset.
#![cfg(windows)]

use ibcmd_filedb::{ConfigRow, FileDb, ReadLimits};
use ibcmd_rs::{
    file_import::{self, ReadControl, Selection},
    metadata_model::xml::parse_element_tree,
    mssql_dump::config_save::{ConfigSaveRequest, SavedConfiguration, save_config},
    mssql_source_change::SourceInventoryLimits,
    platform,
    sql::SqlExec,
    stored_objects::{ConfigSource, StoredConfiguration},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::{Cursor, Read},
    os::windows::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

const MODULE: &str = "CommonModules/ФайловаяБазаПроверка/Ext/Module.bsl";
const DESCRIPTOR: &str = "CommonModules/ФайловаяБазаПроверка.xml";
const LANGUAGE: &str = "Languages/English.xml";
const COMPRESSED_SHA: &str = "090455ade0bae8bb328bce6890db1d1dd4be204f3dce1081cc4f13da65652809";
const DATABASE_SHA: &str = "02e7daca5be7329e505d1dadcf5f1ea762f9dd735e926fe4ab1f8a16daaf1646";
const CHILD: &str = "IBCMD_WINDOWS_PLAN_CHILD";
const ROOT: &str = "IBCMD_WINDOWS_PLAN_ROOT";
const PHASE: &str = "IBCMD_WINDOWS_PLAN_PHASE";
const PREVIOUS_BACKUP: &[u8] = b"owned previous recovery artifact";
const STAGE_COMMENT: &str = "<Comment>Windows first stage</Comment>";

type PackedRows = BTreeMap<String, Vec<u8>>;

#[derive(Serialize, Deserialize)]
struct Marker {
    nonce: String,
    test: String,
    executable_sha256: String,
    compressed_sha256: String,
    database_sha256: String,
    fixture_platform: String,
}

// No Drop cleanup: these are retained evidence, not TempDir-owned files.
struct Case {
    root: PathBuf,
    marker: Marker,
}
impl Case {
    fn create(test: &str) -> Self {
        // Parent setup only copies/reads public fixtures and has no cleanup.
        // Each compiler child receives and verifies KEEP=1 explicitly.
        let nonce = uuid::Uuid::new_v4().to_string();
        let root = std::env::temp_dir().join(format!("ibcmd-windows-plan-{nonce}"));
        fs::create_dir(&root).unwrap();
        let compressed = fs::read(fixture().join("tiny.1CD.gz")).unwrap();
        assert_eq!(sha(&compressed), COMPRESSED_SHA);
        let mut database = Vec::new();
        flate2::read::GzDecoder::new(compressed.as_slice())
            .read_to_end(&mut database)
            .unwrap();
        assert_eq!(database.len(), 3_514_368);
        assert_eq!(sha(&database), DATABASE_SHA);
        fs::create_dir(root.join("db")).unwrap();
        fs::write(root.join("db/1Cv8.1CD"), database).unwrap();
        copy_tree(&fixture().join("tiny-xml"), &root.join("source"));
        fs::write(ibcmd_filedb::backup_file(&root.join("db")), PREVIOUS_BACKUP).unwrap();
        let marker = Marker {
            nonce,
            test: test.into(),
            executable_sha256: sha_file(&std::env::current_exe().unwrap()),
            compressed_sha256: COMPRESSED_SHA.into(),
            database_sha256: DATABASE_SHA.into(),
            fixture_platform: "1C 8.5.1.1150 Linux; metadata 8.5.1/XML2.21".into(),
        };
        json(&root.join("owned-case.json"), &marker);
        let case = Self { root, marker };
        let db = FileDb::open(&case.db()).unwrap();
        assert_eq!(db.page_size(), 8192);
        assert_eq!(db.locale(), "en_US_POSIX");
        assert!(db.collation().is_some());
        drop(db);
        case
    }
    fn child(test: &str) -> Option<(Self, String)> {
        let nonce = match std::env::var(CHILD) {
            Ok(nonce) => nonce,
            Err(std::env::VarError::NotPresent) => {
                assert!(std::env::var_os(ROOT).is_none());
                assert!(std::env::var_os(PHASE).is_none());
                return None;
            }
            Err(error) => panic!("invalid child sentinel: {error}"),
        };
        assert_eq!(
            std::env::var("IBCMD_TEST_KEEP_FIXTURES").as_deref(),
            Ok("1")
        );
        let root = PathBuf::from(std::env::var_os(ROOT).unwrap());
        assert!(root.is_absolute());
        assert_eq!(
            root.file_name().unwrap(),
            format!("ibcmd-windows-plan-{nonce}").as_str()
        );
        let marker: Marker =
            serde_json::from_slice(&fs::read(root.join("owned-case.json")).unwrap()).unwrap();
        assert_eq!(marker.nonce, nonce);
        assert_eq!(marker.test, test);
        assert_eq!(marker.compressed_sha256, COMPRESSED_SHA);
        assert_eq!(marker.database_sha256, DATABASE_SHA);
        assert_eq!(
            marker.executable_sha256,
            sha_file(&std::env::current_exe().unwrap())
        );
        assert!(
            !fs::symlink_metadata(&root)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        let phase = std::env::var(PHASE).unwrap();
        Some((Self { root, marker }, phase))
    }
    fn db(&self) -> PathBuf {
        self.root.join("db")
    }
    fn source(&self) -> PathBuf {
        self.root.join("source")
    }
    fn run(&self, phase: &str) {
        const CHILD_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);
        const LOG_LIMIT: u64 = 16 * 1024 * 1024;
        assert!(
            phase
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-')
        );
        let logs = self.root.join(format!("child-{phase}"));
        fs::create_dir(&logs).unwrap();
        let temporary = logs.join("tmp");
        fs::create_dir(&temporary).unwrap();
        let stdout_path = logs.join("stdout.txt");
        let stderr_path = logs.join("stderr.txt");
        // Stream before waiting so a stopped compiler still leaves partial logs.
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                &self.marker.test,
                "--nocapture",
                "--test-threads=1",
            ])
            .env(CHILD, &self.marker.nonce)
            .env(ROOT, &self.root)
            .env(PHASE, phase)
            .env("IBCMD_TEST_KEEP_FIXTURES", "1")
            .env("TMP", &temporary)
            .env("TEMP", &temporary)
            .env("TMPDIR", &temporary)
            .stdout(Stdio::from(fs::File::create_new(&stdout_path).unwrap()))
            .stderr(Stdio::from(fs::File::create_new(&stderr_path).unwrap()))
            .spawn()
            .unwrap();
        let started = std::time::Instant::now();
        let mut timed_out = false;
        let mut output_cap_exceeded = false;
        let mut kill_error = None;
        let status = loop {
            let status = child.try_wait().unwrap();
            output_cap_exceeded |= fs::metadata(&stdout_path).unwrap().len() > LOG_LIMIT
                || fs::metadata(&stderr_path).unwrap().len() > LOG_LIMIT;
            timed_out |= started.elapsed() >= CHILD_TIMEOUT;
            if timed_out || output_cap_exceeded {
                // A phase is the exact test child, not another parent dispatcher.
                // The hosted outer runner also bounds/kills the full parent tree.
                if status.is_none() {
                    kill_error = child.kill().err().map(|error| error.to_string());
                }
                break status.unwrap_or_else(|| child.wait().unwrap());
            }
            if let Some(status) = status {
                break status;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        };
        output_cap_exceeded |= fs::metadata(&stdout_path).unwrap().len() > LOG_LIMIT
            || fs::metadata(&stderr_path).unwrap().len() > LOG_LIMIT;
        json(
            &logs.join("process.json"),
            &serde_json::json!({
                "test": self.marker.test, "phase": phase, "exit": status.code(),
                "executable_sha256": self.marker.executable_sha256,
                "stdout_sha256": sha_file(&stdout_path), "stderr_sha256": sha_file(&stderr_path),
                "timed_out": timed_out, "output_cap_exceeded": output_cap_exceeded,
                "timeout_seconds": CHILD_TIMEOUT.as_secs(), "stream_budget_bytes": LOG_LIMIT,
                "kill_error": kill_error,
            }),
        );
        // Caps are polled; burst writes can overshoot. Never read an oversized
        // stream into RAM, and never classify a timeout/cap failure as success.
        assert!(!timed_out, "child {phase} timed out; logs retained");
        assert!(
            !output_cap_exceeded,
            "child {phase} exceeded log budget; logs retained"
        );
        assert!(
            status.success(),
            "child {phase} failed; retained stdout/stderr under owned case"
        );
        let stdout_bytes = fs::read(&stdout_path).unwrap();
        let stdout = String::from_utf8_lossy(&stdout_bytes);
        assert!(
            stdout.contains("running 1 test"),
            "missing exact child selection"
        );
        assert!(
            stdout.contains("1 passed; 0 failed; 0 ignored;"),
            "missing child result"
        );
        let receipt: serde_json::Value =
            serde_json::from_slice(&fs::read(logs.join("completed.json")).unwrap()).unwrap();
        assert_eq!(receipt["nonce"], self.marker.nonce);
        assert_eq!(receipt["test"], self.marker.test);
        assert_eq!(receipt["phase"], phase);
        assert_eq!(receipt["executable_sha256"], self.marker.executable_sha256);
        println!("Windows Plan child {phase}: 1 selected, 0 ignored, passed");
    }
    fn complete(&self, phase: &str) {
        json(
            &self.root.join(format!("child-{phase}/completed.json")),
            &serde_json::json!({
                "nonce": self.marker.nonce, "test": self.marker.test, "phase": phase,
                "executable_sha256": self.marker.executable_sha256,
            }),
        );
    }
}

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/file-infobase")
}
// Serialize parent bodies even when a generic integration runner uses many threads.
// Child processes have independent engine/cancellation globals.
fn serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
fn sha_file(path: &Path) -> String {
    let mut reader = fs::File::open(path).unwrap();
    let mut digest = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let count = reader.read(&mut buffer).unwrap();
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    format!("{:x}", digest.finalize())
}
fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn json(path: &Path, value: &impl Serialize) {
    fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}
fn copy_tree(source: &Path, target: &Path) {
    fs::create_dir(target).unwrap();
    for entry in walkdir::WalkDir::new(source).min_depth(1) {
        let entry = entry.unwrap();
        assert!(!entry.file_type().is_symlink());
        let path = target.join(entry.path().strip_prefix(source).unwrap());
        if entry.file_type().is_dir() {
            fs::create_dir(path).unwrap();
        } else {
            assert!(entry.file_type().is_file());
            fs::copy(entry.path(), path).unwrap();
        }
    }
}
fn bytes(db: &Path) -> Vec<u8> {
    fs::read(ibcmd_filedb::data_file(db)).unwrap()
}
fn backup(db: &Path) -> Vec<u8> {
    fs::read(ibcmd_filedb::backup_file(db)).unwrap()
}
fn rows(db: &Path, table: &str) -> Vec<ConfigRow> {
    let mut result = FileDb::open(db).unwrap().config_rows(table).unwrap();
    result.sort_by(|a, b| (&a.file_name, a.part_no).cmp(&(&b.file_name, b.part_no)));
    result
}
fn control() -> ReadControl {
    ReadControl {
        limits: ReadLimits {
            max_records: 100_000,
            max_binary_bytes: 256 * 1024 * 1024,
            max_object_bytes: 256 * 1024 * 1024,
        },
        check: Arc::new(|| Ok(())),
    }
}
fn source_limits() -> SourceInventoryLimits {
    SourceInventoryLimits {
        max_files: 128,
        max_file_bytes: 4 * 1024 * 1024,
        max_total_bytes: 16 * 1024 * 1024,
    }
}
fn prepare(case: &Case, selected: &str) -> file_import::Plan {
    let before = bytes(&case.db());
    let previous = backup(&case.db());
    let plan = file_import::prepare_guarded_with_source_limits(
        &case.db(),
        &case.source(),
        &Selection {
            files: vec![selected.into()],
            ..Selection::default()
        },
        platform::parse_flag("8.5.1").unwrap(),
        &mut |metadata| {
            assert!(metadata.is_file());
            Ok(())
        },
        control(),
        source_limits(),
    )
    .unwrap();
    assert!(plan.summary().ready);
    assert_eq!(plan.summary().files, vec![selected]);
    assert!(plan.summary().missing_references.is_empty());
    assert_eq!(
        plan.summary().target,
        fs::canonicalize(ibcmd_filedb::data_file(&case.db())).unwrap()
    );
    assert_eq!(bytes(&case.db()), before, "prepare wrote target");
    assert_eq!(
        backup(&case.db()),
        previous,
        "prepare wrote previous backup"
    );
    plan
}
fn replace_text(source: &Path, path: &str, from: &str, to: &str) {
    let text = fs::read_to_string(source.join(path)).unwrap();
    assert_eq!(text.matches(from).count(), 1);
    fs::write(source.join(path), text.replacen(from, to, 1)).unwrap();
}
fn edit_module(source: &Path) -> Vec<u8> {
    let expected =
        fs::read_to_string(source.join(MODULE)).unwrap() + "\r\n// Windows selected stage\r\n";
    fs::write(source.join(MODULE), &expected).unwrap();
    expected.into_bytes()
}
fn publish(case: &Case, selected: &str) {
    let before = bytes(&case.db());
    let config = rows(&case.db(), "CONFIG");
    let plan = prepare(case, selected);
    let report = plan
        .publish_guarded(&case.db(), &mut |metadata| {
            assert!(metadata.is_file());
            Ok(())
        })
        .unwrap();
    assert_eq!(report.table, "CONFIGSAVE");
    assert_eq!(
        report.backup,
        ibcmd_filedb::backup_file(&fs::canonicalize(ibcmd_filedb::data_file(&case.db())).unwrap())
    );
    assert_eq!(report.rows_after, rows(&case.db(), "CONFIGSAVE").len());
    assert_eq!(
        backup(&case.db()),
        before,
        "backup is not exact prior database"
    );
    assert_eq!(rows(&case.db(), "CONFIG"), config, "CONFIG tuple changed");
    // Same storage predicate as config/pending; this is not an RPC invocation.
    assert!(
        !FileDb::open(&case.db())
            .unwrap()
            .config_headers("CONFIGSAVE")
            .unwrap()
            .is_empty()
    );
    json(
        &case.root.join(format!(
            "published-{}.json",
            if selected == MODULE {
                "module"
            } else {
                "descriptor"
            }
        )),
        &serde_json::json!({
            "table": report.table, "rows_after": report.rows_after, "pending": true,
            "before_sha256": sha(&before), "after_sha256": sha(&bytes(&case.db())), "backup_sha256": sha(&backup(&case.db())),
        }),
    );
}
fn cf_map(path: &Path) -> PackedRows {
    let bytes = fs::read(path).unwrap();
    let archive = ibcmd_cf::archive::decode_packed_archive(
        Cursor::new(&bytes),
        ibcmd_core::limits::ResourceLimits::for_input_bytes(bytes.len() as u64),
        ibcmd_core::artifact::StorageProfileId::parse("storage:cf-cli").unwrap(),
    )
    .unwrap();
    let mut map = BTreeMap::new();
    for entry in archive.entries() {
        assert!(
            map.insert(entry.name().into(), entry.payload().to_vec())
                .is_none()
        );
    }
    map
}
fn packed_stage(stage: &[ConfigRow]) -> PackedRows {
    assert!(!stage.is_empty());
    let stamp = stage[0].creation;
    let mut groups: BTreeMap<&str, Vec<&ConfigRow>> = BTreeMap::new();
    for row in stage {
        assert_eq!(row.creation, stamp);
        assert_eq!(row.modified, stamp);
        assert_eq!(row.attributes, 0);
        groups.entry(&row.file_name).or_default().push(row);
    }
    let mut map = BTreeMap::new();
    for (name, parts) in groups {
        let mut payload = Vec::new();
        for (index, part) in parts.iter().enumerate() {
            assert_eq!(part.part_no as usize, index);
            assert_eq!(part.data_size, parts[0].data_size);
            payload.extend_from_slice(&part.binary);
        }
        assert_eq!(payload.len() as u64, parts[0].data_size);
        if name == "deleted" {
            assert_eq!(parts.len(), 1);
        }
        map.insert(name.into(), payload);
    }
    let deleted = &map["deleted"];
    let mut decoder = flate2::read::DeflateDecoder::new(deleted.as_slice());
    let mut plain = Vec::new();
    decoder.by_ref().take(5).read_to_end(&mut plain).unwrap();
    assert_eq!(
        plain, b"\xef\xbb\xbf0",
        "only the exact empty deleted marker is admitted"
    );
    assert_eq!(decoder.total_in(), deleted.len() as u64);
    map
}
fn dump_id(name: &str) -> String {
    let root =
        parse_element_tree(&fs::read(fixture().join("tiny-xml/ConfigDumpInfo.xml")).unwrap())
            .unwrap();
    let matches: Vec<_> = root
        .child("ConfigVersions")
        .unwrap()
        .children_named("Metadata")
        .filter(|entry| entry.attr("name") == Some(name))
        .collect();
    assert_eq!(matches.len(), 1);
    matches[0].attr("id").unwrap().into()
}
fn inspect(case: &Case, phase: &str) {
    let output = case.root.join(phase);
    fs::create_dir(&output).unwrap();
    let sql = SqlExec::detached("Windows public file fixture must not use SQL");
    for (name, configuration) in [
        ("main", SavedConfiguration::Main),
        ("database", SavedConfiguration::Database),
    ] {
        let report = save_config(&ConfigSaveRequest {
            sql: &sql,
            database: &format!("public_windows_plan_{}_{}", case.marker.nonce, phase),
            rows_dir: None,
            file_infobase: Some(&case.db()),
            configuration,
            output: &output.join(format!("{name}.cf")),
            overwrite: false,
        })
        .unwrap();
        assert!(report.elements > 0);
        assert!(report.left_out.is_empty());
    }
    let stage = packed_stage(&rows(&case.db(), "CONFIGSAVE"));
    // Reader/overlay consistency, not an independent native compiled-row oracle.
    assert_eq!(stage, cf_map(&output.join("main.cf")));
    assert!(stage.contains_key(&dump_id("CommonModule.ФайловаяБазаПроверка")));
    assert!(stage.contains_key(&dump_id("Language.English")));
    json(&output.join("stage-packed.json"), &stage);
    let mut config = StoredConfiguration::new(
        ConfigSource::FileDb(case.db()),
        platform::parse_flag("8.5.1").unwrap(),
    );
    for (name, path) in [
        ("descriptor", DESCRIPTOR),
        ("module", MODULE),
        ("language", LANGUAGE),
    ] {
        let (files, normalized) = config.read_file(path).unwrap();
        fs::write(
            output.join(format!("{name}.xml-or-bsl")),
            &files.files[&normalized],
        )
        .unwrap();
    }
}

#[test]
fn windows_two_stage_plan_preserves_config_complete_stage_and_backup() {
    const TEST: &str = "windows_two_stage_plan_preserves_config_complete_stage_and_backup";
    if let Some((case, phase)) = Case::child(TEST) {
        match phase.as_str() {
            "stage1" => publish(&case, DESCRIPTOR),
            "stage2" => publish(&case, MODULE),
            "inspect1" | "inspect2" => inspect(&case, &phase),
            _ => panic!("unknown positive child phase"),
        }
        case.complete(&phase);
        return;
    }
    let _serial = serial();
    let case = Case::create(TEST);
    let config = rows(&case.db(), "CONFIG");
    replace_text(&case.source(), DESCRIPTOR, "<Comment/>", STAGE_COMMENT);
    case.run("stage1");
    case.run("inspect1");
    let descriptor = fs::read(case.root.join("inspect1/descriptor.xml-or-bsl")).unwrap();
    assert!(String::from_utf8_lossy(&descriptor).contains(STAGE_COMMENT));
    replace_text(&case.source(), DESCRIPTOR, STAGE_COMMENT, "<Comment/>");
    replace_text(
        &case.source(),
        LANGUAGE,
        "<Comment/>",
        "<Comment>unselected source edit</Comment>",
    );
    let expected = edit_module(&case.source());
    let before_stage2 = bytes(&case.db());
    case.run("stage2");
    case.run("inspect2");
    assert_eq!(rows(&case.db(), "CONFIG"), config);
    assert_eq!(backup(&case.db()), before_stage2);
    assert_eq!(
        fs::read(case.root.join("inspect2/module.xml-or-bsl")).unwrap(),
        expected
    );
    assert_eq!(
        fs::read(case.root.join("inspect2/descriptor.xml-or-bsl")).unwrap(),
        descriptor
    );
    assert_eq!(
        fs::read(case.root.join("inspect2/language.xml-or-bsl")).unwrap(),
        fs::read(fixture().join("tiny-xml").join(LANGUAGE)).unwrap()
    );
    let stage1: PackedRows =
        serde_json::from_slice(&fs::read(case.root.join("inspect1/stage-packed.json")).unwrap())
            .unwrap();
    let stage2: PackedRows =
        serde_json::from_slice(&fs::read(case.root.join("inspect2/stage-packed.json")).unwrap())
            .unwrap();
    for id in [
        dump_id("CommonModule.ФайловаяБазаПроверка"),
        dump_id("Language.English"),
        "deleted".into(),
    ] {
        assert_eq!(
            stage2[&id], stage1[&id],
            "unselected staged payload {id} changed"
        );
    }
    assert_eq!(
        cf_map(&case.root.join("inspect1/database.cf")),
        cf_map(&case.root.join("inspect2/database.cf"))
    );
}

fn refusal(case: &Case, scenario: &str) {
    if matches!(scenario, "source-remove" | "source-rename") {
        fs::create_dir(case.source().join("empty-before-plan")).unwrap();
    }
    let plan = prepare(case, MODULE);
    let mut current = case.db();
    let mut extra = None;
    match scenario {
        "source-edit" => {
            fs::write(
                case.source().join(MODULE),
                b"// real concurrent source mutation\r\n",
            )
            .unwrap();
        }
        "source-file-add" => {
            fs::write(
                case.source().join("unselected-after-plan.txt"),
                b"real unselected addition",
            )
            .unwrap();
        }
        "source-file-remove" => {
            fs::remove_file(case.source().join(LANGUAGE)).unwrap();
        }
        "source-file-rename" => {
            fs::rename(
                case.source().join(LANGUAGE),
                case.source().join("retained-language.xml"),
            )
            .unwrap();
        }
        "source-add" => {
            fs::create_dir(case.source().join("empty-after-plan")).unwrap();
        }
        "source-remove" => {
            fs::remove_dir(case.source().join("empty-before-plan")).unwrap();
        }
        "source-rename" => {
            fs::rename(
                case.source().join("empty-before-plan"),
                case.source().join("renamed-after-plan"),
            )
            .unwrap();
        }
        "target-stage" => {
            let original = bytes(&case.db());
            let config = rows(&case.db(), "CONFIG");
            // External conflict setup only, never a positive Plan replacement.
            ibcmd_filedb::open_for_write(&case.db())
                .unwrap()
                .replace("CONFIGSAVE", &config)
                .unwrap();
            assert_ne!(
                bytes(&case.db()),
                original,
                "external target mutation did not happen"
            );
        }
        "current-target" => {
            let other = Case::create("owned-current-target");
            current = other.db();
            extra = Some(other);
        }
        "target-identity" => {
            let data = ibcmd_filedb::data_file(&case.db());
            let same = bytes(&case.db());
            fs::rename(&data, case.db().join("retained-original.1CD")).unwrap();
            fs::write(&data, &same).unwrap();
            assert_eq!(bytes(&case.db()), same);
        }
        "work-artifact" => {
            fs::write(sibling(&case.db(), ".ibcmd-rs.tmp"), b"owned stale work").unwrap();
        }
        "backup-artifact" => {
            fs::write(
                sibling(&case.db(), ".ibcmd-rs.bak.new"),
                b"owned stale recovery",
            )
            .unwrap();
        }
        _ => panic!("unknown refusal scenario"),
    }
    let before = bytes(&case.db());
    let old_backup = backup(&case.db());
    let extra_before = extra
        .as_ref()
        .map(|other| (bytes(&other.db()), backup(&other.db())));
    let error = plan.publish_guarded(&current, &mut |_| Ok(())).unwrap_err();
    assert!(!ibcmd_rs::cancel::is_cancelled(&error));
    assert!(!error.chain().any(|cause| matches!(
        cause.downcast_ref::<ibcmd_filedb::Error>(),
        Some(ibcmd_filedb::Error::Published { .. })
    )));
    let text = format!("{error:#}");
    match scenario {
        s if s.starts_with("source-") => {
            assert!(text.contains("source"), "wrong source refusal: {text}")
        }
        "current-target" => assert!(text.contains("connection changed"), "{text}"),
        "target-stage" | "target-identity" => {
            assert!(text.contains("changed since import preflight"), "{text}")
        }
        "work-artifact" => assert_eq!(
            fs::read(sibling(&case.db(), ".ibcmd-rs.tmp")).unwrap(),
            b"owned stale work"
        ),
        "backup-artifact" => assert_eq!(
            fs::read(sibling(&case.db(), ".ibcmd-rs.bak.new")).unwrap(),
            b"owned stale recovery"
        ),
        _ => unreachable!(),
    }
    assert_eq!(bytes(&case.db()), before);
    assert_eq!(backup(&case.db()), old_backup);
    if let Some(other) = extra {
        let (data, bak) = extra_before.unwrap();
        assert_eq!(bytes(&other.db()), data);
        assert_eq!(backup(&other.db()), bak);
    }
    json(
        &case.root.join("refusal.json"),
        &serde_json::json!({"scenario": scenario, "error": text, "target_sha256": sha(&before), "backup_sha256": sha(&old_backup)}),
    );
}
fn sibling(db: &Path, suffix: &str) -> PathBuf {
    let data = ibcmd_filedb::data_file(db);
    let mut name = data.file_name().unwrap().to_os_string();
    name.push(suffix);
    data.with_file_name(name)
}
fn single(test: &str, scenario: &str) {
    if let Some((case, phase)) = Case::child(test) {
        assert_eq!(phase, "refusal");
        refusal(&case, scenario);
        case.complete(&phase);
        return;
    }
    let _serial = serial();
    let case = Case::create(test);
    edit_module(&case.source());
    case.run("refusal");
}
macro_rules! refusal_tests {
    ($($name:ident => $scenario:literal),+ $(,)?) => {$ (
        #[test] fn $name() { single(stringify!($name), $scenario); }
    )+};
}
refusal_tests! {
    windows_plan_refuses_selected_source_edit => "source-edit",
    windows_plan_refuses_unselected_file_add => "source-file-add",
    windows_plan_refuses_unselected_file_remove => "source-file-remove",
    windows_plan_refuses_unselected_file_rename => "source-file-rename",
    windows_plan_refuses_empty_directory_add => "source-add",
    windows_plan_refuses_empty_directory_remove => "source-remove",
    windows_plan_refuses_empty_directory_rename => "source-rename",
    windows_plan_refuses_external_configsave_change => "target-stage",
    windows_plan_refuses_changed_current_target => "current-target",
    windows_plan_refuses_same_bytes_new_target_identity => "target-identity",
    windows_plan_preserves_preexisting_work_artifact => "work-artifact",
    windows_plan_preserves_preexisting_recovery_artifact => "backup-artifact",
}

fn final_gate(test: &str, cancel: bool) {
    if let Some((case, phase)) = Case::child(test) {
        assert_eq!(phase, "final-gate");
        let flag = Arc::new(AtomicBool::new(false));
        let _scope = ibcmd_rs::cancel::scope(flag.clone());
        let plan = prepare(&case, MODULE);
        let before = bytes(&case.db());
        let previous = backup(&case.db());
        let mut calls = 0;
        let error = plan
            .publish_guarded(&case.db(), &mut |metadata| {
                assert!(metadata.is_file());
                calls += 1;
                if calls == 3 {
                    // Verified work and original recovery image exist at the final boundary.
                    assert!(sibling(&case.db(), ".ibcmd-rs.tmp").is_file());
                    assert!(sibling(&case.db(), ".ibcmd-rs.bak.new").is_file());
                    if cancel {
                        flag.store(true, Ordering::SeqCst);
                    } else {
                        anyhow::bail!("owned final policy rejection");
                    }
                }
                Ok(())
            })
            .unwrap_err();
        assert_eq!(
            calls, 3,
            "did not reach the actual final publication callback"
        );
        if cancel {
            assert!(ibcmd_rs::cancel::is_cancelled(&error));
        } else {
            assert!(format!("{error:#}").contains("owned final policy rejection"));
        }
        assert_eq!(bytes(&case.db()), before);
        assert_eq!(backup(&case.db()), previous);
        json(
            &case.root.join("final-gate.json"),
            &serde_json::json!({"policy_callbacks":calls,"typed_cancelled":ibcmd_rs::cancel::is_cancelled(&error)}),
        );
        case.complete(&phase);
        return;
    }
    let _serial = serial();
    let case = Case::create(test);
    edit_module(&case.source());
    case.run("final-gate");
}
#[test]
fn windows_plan_final_policy_refuses_without_publication() {
    final_gate(
        "windows_plan_final_policy_refuses_without_publication",
        false,
    );
}
#[test]
fn windows_plan_late_cancellation_remains_typed() {
    final_gate("windows_plan_late_cancellation_remains_typed", true);
}

fn budget(test: &str, source_budget: bool) {
    if let Some((case, phase)) = Case::child(test) {
        assert_eq!(phase, "budget");
        let before = bytes(&case.db());
        let previous = backup(&case.db());
        let mut read = control();
        let mut source = source_limits();
        if source_budget {
            source.max_file_bytes = 1;
        } else {
            read.limits.max_object_bytes = 1;
        }
        let result = file_import::prepare_guarded_with_source_limits(
            &case.db(),
            &case.source(),
            &Selection {
                files: vec![MODULE.into()],
                ..Selection::default()
            },
            platform::parse_flag("8.5.1").unwrap(),
            &mut |_| Ok(()),
            read,
            source,
        );
        let error = match result {
            Ok(_) => panic!("finite budget accepted oversized fixture"),
            Err(error) => error,
        };
        assert!(
            format!("{error:#}").contains("limit"),
            "wrong budget refusal: {error:#}"
        );
        assert_eq!(bytes(&case.db()), before);
        assert_eq!(backup(&case.db()), previous);
        case.complete(&phase);
        return;
    }
    let _serial = serial();
    let case = Case::create(test);
    edit_module(&case.source());
    case.run("budget");
}
#[test]
fn windows_plan_source_budget_refuses_before_writes() {
    budget("windows_plan_source_budget_refuses_before_writes", true);
}
#[test]
fn windows_plan_target_budget_refuses_before_writes() {
    budget("windows_plan_target_budget_refuses_before_writes", false);
}

// Measure this host's destination replacement denial without touching real
// recovery artifacts. The same directory/prefix is used by the writer.
fn measured_backup_replacement_denial(case: &Case) -> i32 {
    use std::os::windows::fs::MetadataExt;
    let data = fs::canonicalize(ibcmd_filedb::data_file(&case.db())).unwrap();
    let parent = data.parent().unwrap();
    let target = parent.join("owned-rename-probe-target");
    let before = parent.join("owned-rename-probe-before");
    let after = parent.join("owned-rename-probe-after");
    fs::write(&target, b"public original probe").unwrap();
    fs::write(&before, b"public unheld replacement").unwrap();
    fs::rename(&before, &target).unwrap();
    assert!(!before.exists());
    assert_eq!(fs::read(&target).unwrap(), b"public unheld replacement");
    let attributes = fs::metadata(&target).unwrap().file_attributes();
    assert_eq!(
        attributes,
        fs::metadata(ibcmd_filedb::backup_file(&case.db()))
            .unwrap()
            .file_attributes()
    );
    let held = fs::OpenOptions::new()
        .read(true)
        .share_mode(
            windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ
                | windows_sys::Win32::Storage::FileSystem::FILE_SHARE_WRITE,
        )
        .open(&target)
        .unwrap();
    fs::write(&after, b"public after-close replacement").unwrap();
    let error = fs::rename(&after, &target)
        .expect_err("no-DELETE destination handle must deny replacement");
    let code = error
        .raw_os_error()
        .expect("Windows denial must retain its OS code");
    assert!(
        matches!(code, 5 | 32),
        "unexpected measured Windows denial: {error}"
    );
    assert_eq!(fs::read(&target).unwrap(), b"public unheld replacement");
    assert_eq!(fs::read(&after).unwrap(), b"public after-close replacement");
    assert_eq!(fs::metadata(&target).unwrap().file_attributes(), attributes);
    drop(held);
    fs::rename(&after, &target).unwrap();
    assert!(!after.exists());
    assert_eq!(
        fs::read(&target).unwrap(),
        b"public after-close replacement"
    );
    json(
        &case.root.join("recovery-denial-probe.json"),
        &serde_json::json!({
            "without_handle_rename_passed": true, "held_rename_denied": true,
            "after_drop_rename_passed": true, "denial_raw_os_error": code,
            "held_target_preserved": true, "held_source_preserved": true,
            "same_parent_attrs_match": true, "held_attrs_unchanged": true,
        }),
    );
    code
}

#[test]
fn windows_plan_committed_recovery_error_retains_original() {
    const TEST: &str = "windows_plan_committed_recovery_error_retains_original";
    if let Some((case, phase)) = Case::child(TEST) {
        match phase.as_str() {
            "publish-recovery" => {
                let measured_code = measured_backup_replacement_denial(&case);
                let plan = prepare(&case, MODULE);
                // Publication reports use the canonical consumed target namespace.
                let expected_recovery = sibling(&plan.summary().target, ".ibcmd-rs.bak.new");
                let before = bytes(&case.db());
                let previous = backup(&case.db());
                let config = rows(&case.db(), "CONFIG");
                // Real Windows backup replacement denial after DB publication.
                // This handle is deliberately not a target or source capability.
                let held_backup = fs::OpenOptions::new()
                    .read(true)
                    .share_mode(
                        windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ
                            | windows_sys::Win32::Storage::FileSystem::FILE_SHARE_WRITE,
                    )
                    .open(ibcmd_filedb::backup_file(&case.db()))
                    .unwrap();
                let error = plan
                    .publish_guarded(&case.db(), &mut |_| Ok(()))
                    .unwrap_err();
                assert!(!ibcmd_rs::cancel::is_cancelled(&error));
                let recovery = error
                    .chain()
                    .find_map(|cause| match cause.downcast_ref::<ibcmd_filedb::Error>() {
                        Some(ibcmd_filedb::Error::Published { recovery, error }) => {
                            assert_eq!(error.raw_os_error(), Some(measured_code));
                            Some(recovery.clone())
                        }
                        _ => None,
                    })
                    .expect("must reach committed Error::Published, not an earlier refusal");
                assert_eq!(recovery, expected_recovery);
                assert_eq!(fs::read(&recovery).unwrap(), before);
                assert_eq!(backup(&case.db()), previous);
                assert_ne!(
                    bytes(&case.db()),
                    before,
                    "database was not actually committed"
                );
                assert_eq!(rows(&case.db(), "CONFIG"), config);
                assert!(!rows(&case.db(), "CONFIGSAVE").is_empty());
                json(
                    &case.root.join("committed-recovery.json"),
                    &serde_json::json!({"published":true,"raw_os_error":measured_code,"original_sha256":sha(&before),"recovery_sha256":sha(&fs::read(recovery).unwrap())}),
                );
                drop(held_backup);
            }
            "inspect-recovery" => {
                let mut config = StoredConfiguration::new(
                    ConfigSource::FileDb(case.db()),
                    platform::parse_flag("8.5.1").unwrap(),
                );
                let (files, normalized) = config.read_file(MODULE).unwrap();
                assert_eq!(
                    files.files[&normalized],
                    fs::read(case.source().join(MODULE)).unwrap()
                );
            }
            _ => panic!("unknown recovery child phase"),
        }
        case.complete(&phase);
        return;
    }
    let _serial = serial();
    let case = Case::create(TEST);
    edit_module(&case.source());
    case.run("publish-recovery");
    case.run("inspect-recovery");
}
