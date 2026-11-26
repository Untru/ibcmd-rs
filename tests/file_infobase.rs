//! `infobase config export|import --db-path` (#373–#375) on file infobases
//! written by 1C:Enterprise 8.5.1.1150 (`tests/fixtures/file-infobase`). No
//! platform runs here: the export is compared with the platform's export of
//! the same file, and an import is read back through the export.

use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/file-infobase")
}

/// A fresh scratch directory for one test, removed on drop.
struct Scratch(PathBuf);

impl std::ops::Deref for Scratch {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn scratch(tag: &str) -> Scratch {
    static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let sequence = SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "ibcmd-file-infobase-{}-{sequence}-{tag}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    Scratch(dir)
}

/// Unpacks `<name>.1CD.gz` into `<dir>/<name>/1Cv8.1CD`; returns the infobase
/// directory.
fn infobase(dir: &Path, name: &str) -> PathBuf {
    let mut packed = Vec::new();
    flate2::read::GzDecoder::new(
        fs::File::open(fixtures().join(format!("{name}.1CD.gz"))).unwrap(),
    )
    .read_to_end(&mut packed)
    .unwrap();
    let db = dir.join(name);
    fs::create_dir_all(&db).unwrap();
    fs::write(db.join("1Cv8.1CD"), packed).unwrap();
    db
}

fn run(args: &[&std::ffi::OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .args(args)
        .output()
        .expect("run ibcmd-rs")
}

fn ok(output: &Output) {
    assert!(
        output.status.success(),
        "exit {:?}\nstdout: {}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn db_path(db: &Path) -> std::ffi::OsString {
    let mut arg = std::ffi::OsString::from("--db-path=");
    arg.push(db);
    arg
}

fn export(db: &Path, out: &Path) -> Output {
    run(&[
        "infobase".as_ref(),
        "config".as_ref(),
        "export".as_ref(),
        &db_path(db),
        out.as_os_str(),
    ])
}

fn import(db: &Path, tree: &Path) -> Output {
    run(&[
        "infobase".as_ref(),
        "config".as_ref(),
        "import".as_ref(),
        &db_path(db),
        tree.as_os_str(),
    ])
}

fn files(root: &Path) -> BTreeMap<String, Vec<u8>> {
    walkdir::WalkDir::new(root)
        .into_iter()
        .map(Result::unwrap)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| {
            let relative = entry
                .path()
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            (relative, fs::read(entry.path()).unwrap())
        })
        .collect()
}

/// The trees hold the same files, byte for byte, but for `ignored`.
fn assert_same_tree(expected: &Path, actual: &Path, ignored: &[&str]) {
    let keep = |map: BTreeMap<String, Vec<u8>>| {
        map.into_iter()
            .filter(|(path, _)| !ignored.contains(&path.as_str()))
            .collect::<BTreeMap<_, _>>()
    };
    let (expected, actual) = (keep(files(expected)), keep(files(actual)));
    let missing: Vec<_> = expected
        .keys()
        .filter(|k| !actual.contains_key(*k))
        .collect();
    let extra: Vec<_> = actual
        .keys()
        .filter(|k| !expected.contains_key(*k))
        .collect();
    let differ: Vec<_> = expected
        .iter()
        .filter(|(k, v)| actual.get(*k).is_some_and(|a| a != *v))
        .map(|(k, _)| k)
        .collect();
    assert!(
        missing.is_empty() && extra.is_empty() && differ.is_empty(),
        "missing {missing:?}, extra {extra:?}, differ {differ:?}"
    );
}

#[test]
fn the_export_reproduces_the_platforms_export_of_the_file() {
    let dir = scratch("export");
    let db = infobase(&dir, "tiny");
    let out = dir.join("xml");
    ok(&export(&db, &out));
    assert_same_tree(&fixtures().join("tiny-xml"), &out, &[]);
    // The data file is read, never written.
    assert!(!ibcmd_filedb::backup_file(&db).exists());
}

#[cfg(unix)]
#[test]
fn an_import_into_an_empty_infobase_stages_the_whole_tree() {
    let dir = scratch("import-empty");
    let db = infobase(&dir, "empty");
    let output = import(&db, &fixtures().join("tiny-xml"));
    ok(&output);
    assert!(
        ibcmd_filedb::backup_file(&db).is_file(),
        "the original is kept"
    );
    let mut staged = ibcmd_filedb::FileDb::open(&db).unwrap();
    assert!(
        staged.config_rows("CONFIG").unwrap().is_empty(),
        "CONFIG is the apply's"
    );
    let rows = staged.config_rows("CONFIGSAVE").unwrap();
    for service in ["root", "version", "versions", "deleted"] {
        assert!(rows.iter().any(|row| row.file_name == service), "{service}");
    }
    assert_eq!(staged.verify_index("CONFIGSAVE").unwrap(), rows.len());
    drop(staged);
    // The export publishes a completed stage: the tree comes back, but for
    // the generation ids of ConfigDumpInfo.xml, which every import renews.
    let out = dir.join("xml");
    ok(&export(&db, &out));
    assert_same_tree(&fixtures().join("tiny-xml"), &out, &["ConfigDumpInfo.xml"]);
}

#[cfg(unix)]
#[test]
fn an_import_onto_the_same_configuration_loads_the_edit() {
    let dir = scratch("import-patch");
    let db = infobase(&dir, "tiny");
    let tree = dir.join("tree");
    for (path, bytes) in files(&fixtures().join("tiny-xml")) {
        let target = tree.join(&path);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(target, bytes).unwrap();
    }
    let module = tree.join("CommonModules/ФайловаяБазаПроверка/Ext/Module.bsl");
    let text = fs::read(&module).unwrap();
    let edited = String::from_utf8(text)
        .unwrap()
        .replace("Возврат 42;", "Возврат 43;");
    fs::write(&module, edited).unwrap();

    let output = import(&db, &tree);
    ok(&output);
    let report = String::from_utf8_lossy(&output.stdout).to_string()
        + &String::from_utf8_lossy(&output.stderr);
    assert!(!report.contains("panicked"), "{report}");
    let out = dir.join("xml");
    ok(&export(&db, &out));
    assert_same_tree(&tree, &out, &["ConfigDumpInfo.xml"]);
}

#[cfg(unix)]
#[test]
fn a_russian_infobase_is_imported_with_its_collation() {
    let dir = scratch("import-ru");
    let db = infobase(&dir, "ru-empty");
    ok(&import(&db, &fixtures().join("tiny-xml")));
    let mut staged = ibcmd_filedb::FileDb::open(&db).unwrap();
    assert_eq!(staged.locale(), "ru_RU");
    staged.verify_index("CONFIGSAVE").unwrap();
    drop(staged);
    let out = dir.join("xml");
    ok(&export(&db, &out));
    assert_same_tree(&fixtures().join("tiny-xml"), &out, &["ConfigDumpInfo.xml"]);
}

#[test]
fn an_infobase_in_an_unmeasured_locale_is_not_imported() {
    let dir = scratch("import-de");
    let db = infobase(&dir, "de-empty");
    let before = fs::read(db.join("1Cv8.1CD")).unwrap();
    let output = import(&db, &fixtures().join("tiny-xml"));
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    #[cfg(any(unix, windows))]
    assert!(stderr.contains("de_DE"), "{stderr}");
    #[cfg(not(any(unix, windows)))]
    assert!(stderr.contains("not supported on this OS"), "{stderr}");
    assert_eq!(fs::read(db.join("1Cv8.1CD")).unwrap(), before);
    assert_eq!(fs::read_dir(&db).unwrap().count(), 1);
    // Reading it is the crate's test (`native_files.rs`): this infobase holds
    // no configuration, which the export does not publish for any storage.
}

#[test]
fn broken_files_fail_closed() {
    let dir = scratch("broken");
    let db = infobase(&dir, "tiny");
    let data = db.join("1Cv8.1CD");
    let whole = fs::read(&data).unwrap();
    for (tag, bytes) in [
        ("truncated", whole[..whole.len() / 3].to_vec()),
        ("not-a-1cd", b"PK\x03\x04 not an infobase".to_vec()),
        ("huge-object", {
            // The root object's length (page 2, offset 16) far past the file:
            // refused, not allocated (an allocation that size aborts).
            let mut bytes = whole.clone();
            bytes[2 * 8192 + 16..2 * 8192 + 24]
                .copy_from_slice(&0xFFFF_FFFF_FFFF_FF00u64.to_le_bytes());
            bytes
        }),
    ] {
        fs::write(&data, &bytes).unwrap();
        let out = dir.join(format!("xml-{tag}"));
        let output = export(&db, &out);
        assert!(!output.status.success(), "{tag}: the export succeeded");
        assert!(output.status.code().is_some(), "{tag}: killed by a signal");
        assert_ne!(output.status.code(), Some(134), "{tag}: aborted");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!stderr.contains("panicked"), "{tag}: {stderr}");
        let output = import(&db, &fixtures().join("tiny-xml"));
        assert!(!output.status.success(), "{tag}: the import succeeded");
        assert_eq!(
            fs::read(&data).unwrap(),
            bytes,
            "{tag}: the file was changed"
        );
        assert!(!ibcmd_filedb::backup_file(&db).exists(), "{tag}");
    }
    // No infobase in the directory at all.
    let output = export(&dir.join("absent"), &dir.join("xml-absent"));
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("1Cv8.1CD"));
}

/// `files` with every uuid but the fixed class ids masked: the platform
/// makes up a new configuration for an empty `Config` at each export, with
/// fresh uuids (two exports of one empty infobase differ only in them).
fn masked(root: &Path) -> BTreeMap<String, String> {
    let mask = |mut text: String| {
        for prefix in ["uuid=\"", "<xr:ObjectId>"] {
            let mut from = 0;
            while let Some(at) = text[from..].find(prefix) {
                let start = from + at + prefix.len();
                if text
                    .get(start..start + 36)
                    .is_some_and(|id| id.len() == 36 && id.matches('-').count() == 4)
                {
                    text.replace_range(start..start + 36, "U");
                }
                from = start;
            }
        }
        text
    };
    files(root)
        .into_iter()
        .map(|(path, bytes)| (path, mask(String::from_utf8(bytes).unwrap())))
        .collect()
}

fn export_empty(platform: &str, language: &str) -> (Scratch, PathBuf) {
    let dir = scratch(&format!("export-empty-{platform}-{language}"));
    let db = infobase(&dir, "empty");
    let out = dir.join("xml");
    let mut arg = std::ffi::OsString::from("--platform=");
    arg.push(platform);
    ok(&Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .env("IBCMD_RS_LANGUAGE", language)
        .args(["infobase", "config", "export"])
        .arg(db_path(&db))
        .arg(arg)
        .arg(&out)
        .output()
        .unwrap());
    (dir, out)
}

#[test]
fn an_infobase_without_a_configuration_exports_the_default_one() {
    // `empty-xml-*`: the platform's export of an empty infobase (8.5.1.1529
    // ibcmd, 8.3.27.2214 Designer), interface language Russian (#469).
    for (platform, expected) in [("8.3.27", "empty-xml-8.3.27"), ("8.5.1", "empty-xml-8.5.1")] {
        let (_dir, out) = export_empty(platform, "ru");
        assert_eq!(
            masked(&fixtures().join(expected)),
            masked(&out),
            "{platform}"
        );
    }
    // An English interface (8.5.1.1150 on Linux): `tiny-source` is that
    // export with one common module added.
    let (_dir, out) = export_empty("8.5.1", "en");
    let mut expected = masked(&fixtures().join("tiny-source"));
    expected.retain(|path, _| !path.starts_with("CommonModules"));
    let configuration = expected.get_mut("Configuration.xml").unwrap();
    *configuration = configuration.replace(
        "\t\t\t<CommonModule>ФайловаяБазаПроверка</CommonModule>\r\n",
        "",
    );
    let mut actual = masked(&out);
    actual.remove("ConfigDumpInfo.xml");
    assert_eq!(expected, actual);
}

#[test]
fn empty_configuration_supports_base_and_sync_exports() {
    let (_base_dir, base_out) = export_empty("8.5.1", "en");
    let base = base_out.join("ConfigDumpInfo.xml");
    for (tag, use_base, sync) in [
        ("base", true, false),
        ("sync", false, true),
        ("base-sync", true, true),
    ] {
        let dir = scratch(&format!("empty-{tag}"));
        let db = infobase(&dir, "empty");
        let out = dir.join("xml");
        if sync {
            fs::create_dir_all(out.join("Catalogs")).unwrap();
            fs::write(out.join("Catalogs/Stale.xml"), "obsolete").unwrap();
            fs::write(out.join("notes.txt"), "keep me").unwrap();
        }
        let mut command = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"));
        // Native infobase export has a distinct no-op protocol. The custom
        // source exporter retains independent --base/--sync composition.
        command
            .env("IBCMD_RS_LANGUAGE", "en")
            .args([
                "mssql-dump-config",
                "--main-configuration",
                "--source-version=2.21",
                "--extract-metadata-xml",
                "--extract-module-text",
                "--no-binary-rows",
                "--file-infobase",
            ])
            .arg(&db)
            .arg("--output-dir")
            .arg(&out);
        if use_base {
            command.arg("--base").arg(&base);
        }
        if sync {
            command.arg("--sync");
        }
        ok(&command.output().unwrap());
        let mut actual = masked(&out);
        // The custom CLI additionally emits its diagnostic manifest; compare
        // every configuration file against the native default XML as before.
        assert!(actual.remove("manifest.json").is_some());
        if sync {
            assert_eq!(actual.remove("notes.txt").as_deref(), Some("keep me"));
            assert!(!out.join("Catalogs/Stale.xml").exists());
        }
        assert_eq!(masked(&base_out), actual, "{tag}");
    }
}

#[test]
fn empty_configuration_has_complete_zero_inventories() {
    let dir = scratch("empty-completeness");
    let db = infobase(&dir, "empty");
    let out = dir.join("xml");
    let output = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .env("IBCMD_RS_LANGUAGE", "en")
        .args([
            "mssql-dump-config",
            "--main-configuration",
            "--extract-metadata-xml",
            "--extract-module-text",
            "--no-binary-rows",
            "--require-complete-source-assets",
            "--require-complete-root-metadata",
            "--platform=8.5.1",
        ])
        .arg("--file-infobase")
        .arg(&db)
        .arg("-o")
        .arg(&out)
        .output()
        .unwrap();
    ok(&output);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let assets = &report["source_assets"];
    assert_eq!(assets["scope"], "full");
    assert_eq!(assets["status"], "complete");
    assert_eq!(assets["candidate_set_complete"], true);
    assert_eq!(assets["expected"], 0);
    assert!(out.join("Configuration.xml").is_file());
}

fn save(db: &Path, cf: &Path, database_configuration: bool) -> Output {
    let mut args: Vec<&std::ffi::OsStr> =
        vec!["infobase".as_ref(), "config".as_ref(), "save".as_ref()];
    let path = db_path(db);
    args.push(&path);
    if database_configuration {
        args.push("--db".as_ref());
    }
    args.push(cf.as_os_str());
    run(&args)
}

#[test]
fn a_file_infobase_is_saved_as_a_cf_of_its_configuration() {
    // #352: the .cf holds the rows the file stores; its export is the
    // platform's export of the file (`tiny-xml`).
    let dir = scratch("save");
    let db = infobase(&dir, "tiny");
    for database_configuration in [false, true] {
        let cf = dir.join(format!("saved-{database_configuration}.cf"));
        fs::write(&cf, b"previous saved output").unwrap();
        ok(&save(&db, &cf, database_configuration));
        let out = dir.join(format!("xml-{database_configuration}"));
        ok(&run(&[
            "cf".as_ref(),
            "export".as_ref(),
            cf.as_os_str(),
            out.as_os_str(),
            "--platform".as_ref(),
            "8.5.1".as_ref(),
        ]));
        assert_same_tree(&fixtures().join("tiny-xml"), &out, &[]);
    }
    assert!(!ibcmd_filedb::backup_file(&db).exists());
}

#[test]
fn a_completed_file_stage_is_saved_with_its_deleted_row() {
    let dir = scratch("save-completed-stage");
    for name in ["empty", "tiny"] {
        let db = infobase(&dir, name);
        ok(&import(&db, &fixtures().join("tiny-xml")));
        let before = fs::read(db.join("1Cv8.1CD")).unwrap();
        let cf = dir.join(format!("staged-{name}.cf"));
        ok(&save(&db, &cf, false));
        let archive = ibcmd_cf::archive::decode_packed_archive(
            fs::File::open(&cf).unwrap(),
            ibcmd_core::limits::ResourceLimits::for_input_bytes(fs::metadata(&cf).unwrap().len()),
            ibcmd_core::artifact::StorageProfileId::parse("storage:test").unwrap(),
        )
        .unwrap();
        let (_, _, entries) = archive.into_parts();
        let entries: BTreeMap<_, _> = entries
            .into_iter()
            .map(|entry| entry.into_parts())
            .collect();
        let mut source = ibcmd_filedb::FileDb::open(&db).unwrap();
        for row in source.config_rows("CONFIGSAVE").unwrap() {
            assert_eq!(
                entries.get(&row.file_name),
                Some(&row.binary),
                "{}",
                row.file_name
            );
        }
        drop(source);
        let out = dir.join(format!("xml-staged-{name}"));
        ok(&run(&[
            "cf".as_ref(),
            "export".as_ref(),
            cf.as_os_str(),
            out.as_os_str(),
            "--platform".as_ref(),
            "8.5.1".as_ref(),
        ]));
        assert_same_tree(&fixtures().join("tiny-xml"), &out, &["ConfigDumpInfo.xml"]);
        assert_eq!(fs::read(db.join("1Cv8.1CD")).unwrap(), before);
    }
}

#[test]
fn saving_a_file_infobase_refuses_unfinished_stored_config() {
    let dir = scratch("save-unfinished-config");
    for completed_stage in [false, true] {
        let db = infobase(&dir.join(format!("stage-{completed_stage}")), "tiny");
        if completed_stage {
            ok(&import(&db, &fixtures().join("tiny-xml")));
            // This disposable fixture is deliberately rewritten below.
            fs::remove_file(ibcmd_filedb::backup_file(&db)).unwrap();
        }
        let mut source = ibcmd_filedb::FileDb::open(&db).unwrap();
        let mut rows = source.config_rows("CONFIG").unwrap();
        drop(source);
        let mut unfinished = rows[0].clone();
        unfinished.file_name = "deleted".to_string();
        rows.push(unfinished);
        ibcmd_filedb::replace_config_table(&db, "CONFIG", &rows).unwrap();
        let before = fs::read(db.join("1Cv8.1CD")).unwrap();
        for database_configuration in [false, true] {
            let cf = dir.join(format!(
                "unfinished-{completed_stage}-{database_configuration}.cf"
            ));
            let output = save(&db, &cf, database_configuration);
            assert!(!output.status.success());
            assert!(String::from_utf8_lossy(&output.stderr).contains("deleted"));
            assert!(!cf.exists());
            assert_eq!(fs::read(db.join("1Cv8.1CD")).unwrap(), before);
        }
    }
}

#[test]
fn saving_a_file_infobase_refuses_the_source_file_and_its_aliases() {
    let dir = scratch("save-source-identity");
    let db = infobase(&dir, "tiny");
    let data = db.join("1Cv8.1CD");
    let before = fs::read(&data).unwrap();
    let alias = dir.join("hard-link.cf");
    fs::hard_link(&data, &alias).unwrap();
    let destinations = vec![data.clone(), alias.clone()];
    #[cfg(unix)]
    let destinations = {
        let mut destinations = destinations;
        let symlink = dir.join("symlink.cf");
        std::os::unix::fs::symlink(&data, &symlink).unwrap();
        destinations.push(symlink);
        destinations
    };
    for source in [&db, &data] {
        for destination in &destinations {
            for database_configuration in [false, true] {
                let output = save(source, destination, database_configuration);
                assert!(
                    !output.status.success(),
                    "source was accepted as output: {destination:?}"
                );
                assert!(String::from_utf8_lossy(&output.stderr).contains("source infobase"));
                assert_eq!(fs::read(&data).unwrap(), before);
                assert_eq!(fs::read(&alias).unwrap(), before);
            }
        }
    }
    assert!(!ibcmd_filedb::backup_file(&db).exists());
}

#[test]
fn an_exact_build_named_for_the_export_spells_its_own_defaults() {
    // 8.5.1.1529 writes `ButtonImportance` `Main` on a default button whose
    // record names no importance of its own; the release 8.5.1 keeps the
    // oldest build's spelling (`default-button-xml`).
    let dir = scratch("exact-build");
    let db = infobase(&dir, "default-button");
    let out = dir.join("xml");
    ok(&run(&[
        "infobase".as_ref(),
        "config".as_ref(),
        "export".as_ref(),
        &db_path(&db),
        "--platform=8.5.1.1529".as_ref(),
        out.as_os_str(),
    ]));
    assert_same_tree(&fixtures().join("default-button-xml"), &out, &[]);
}

#[test]
fn an_exact_build_from_settings_spells_its_own_defaults() {
    let dir = scratch("exact-build-settings");
    let db = infobase(&dir, "default-button");
    let out = dir.join("xml");
    let settings = dir.join("settings.json");
    fs::write(&settings, r#"{"platform-version":"8.5.1.1529"}"#).unwrap();
    ok(&run(&[
        "infobase".as_ref(),
        "config".as_ref(),
        "export".as_ref(),
        &db_path(&db),
        "--settings".as_ref(),
        settings.as_os_str(),
        out.as_os_str(),
    ]));
    assert_same_tree(&fixtures().join("default-button-xml"), &out, &[]);
}

#[test]
fn an_unknown_settings_build_is_refused_before_export() {
    let dir = scratch("unknown-build-settings");
    let db = infobase(&dir, "default-button");
    let out = dir.join("xml");
    let settings = dir.join("settings.json");
    fs::write(&settings, r#"{"platform-version":"8.5.1.9999"}"#).unwrap();
    let output = run(&[
        "infobase".as_ref(),
        "config".as_ref(),
        "export".as_ref(),
        &db_path(&db),
        "--source-version=2.21".as_ref(),
        "--settings".as_ref(),
        settings.as_os_str(),
        out.as_os_str(),
    ]);
    assert!(!output.status.success());
    assert!(!out.exists());
}

#[test]
fn a_cf_loaded_into_a_file_infobase_is_staged_whole() {
    // #353: the .cf's elements staged in CONFIGSAVE as they are; the export
    // publishes the stage, which is `tiny-source` (the versions of
    // `ConfigDumpInfo.xml` are those of the infobase the file was saved of).
    let dir = scratch("load-cf");
    let db = infobase(&dir, "empty");
    let report = dir.join("report.json");
    let mut report_arg = std::ffi::OsString::from("--report=");
    report_arg.push(&report);
    ok(&run(&[
        "infobase".as_ref(),
        "config".as_ref(),
        "load".as_ref(),
        &db_path(&db),
        &report_arg,
        fixtures().join("tiny.cf").as_os_str(),
    ]));
    assert!(ibcmd_filedb::backup_file(&db).is_file());
    let report: serde_json::Value = serde_json::from_slice(&fs::read(&report).unwrap()).unwrap();
    let text = report.to_string();
    assert!(text.contains("\"format\":\"cf\""), "{text}");
    assert!(
        text.contains("\"operation\":\"infobase config load\""),
        "{text}"
    );
    let out = dir.join("xml");
    ok(&export(&db, &out));
    assert_same_tree(&fixtures().join("tiny-xml"), &out, &["ConfigDumpInfo.xml"]);
}

#[test]
fn a_cf_with_an_archived_deleted_row_loads_with_its_own_list() {
    // A platform's `config save` of an unapplied import writes the stage's
    // `deleted` into the file; the load replaces it by the list for this
    // infobase instead of staging two `deleted` rows (review of #494).
    use ibcmd_cf::archive::decode_packed_archive;
    use ibcmd_core::{artifact::StorageProfileId, limits::ResourceLimits};
    use ibcmd_v8::writer::{Format15Document, Format15Element, write_format15_to_vec};
    use std::io::Write;
    let dir = scratch("load-cf-deleted");
    let source = fixtures().join("tiny.cf");
    let file = fs::File::open(&source).unwrap();
    let limits = ResourceLimits::for_input_bytes(file.metadata().unwrap().len());
    let archive = decode_packed_archive(
        file,
        limits,
        StorageProfileId::parse("storage:test").unwrap(),
    )
    .unwrap();
    let (_, _, entries) = archive.into_parts();
    let mut entries = entries
        .into_iter()
        .map(|entry| entry.into_parts())
        .collect::<Vec<_>>();
    let mut encoder =
        flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all("\u{feff}0".as_bytes()).unwrap();
    entries.push(("deleted".to_string(), encoder.finish().unwrap()));
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    let document = Format15Document::new(
        entries.len() as u32,
        entries
            .iter()
            .map(|(name, bytes)| Format15Element::named(name, Some(bytes.clone())))
            .collect(),
    );
    let input = dir.join("archived.cf");
    fs::write(&input, write_format15_to_vec(&document).unwrap()).unwrap();
    let db = infobase(&dir, "empty");
    ok(&run(&[
        "infobase".as_ref(),
        "config".as_ref(),
        "load".as_ref(),
        &db_path(&db),
        input.as_os_str(),
    ]));
    let out = dir.join("xml");
    ok(&export(&db, &out));
    assert_same_tree(&fixtures().join("tiny-xml"), &out, &["ConfigDumpInfo.xml"]);
}
