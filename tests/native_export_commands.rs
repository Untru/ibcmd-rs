//! Native query protocols measured on 8.5.1.1592; own tiny fixture only.
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Output},
};
// Native failure is -1; POSIX process status retains its low byte.
const FAILED: i32 = if cfg!(windows) { -1 } else { 255 };

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/file-infobase")
}
fn lab(tag: &str) -> (crate::common::Scratch, PathBuf) {
    // IBCMD_TEST_KEEP_FIXTURES=1 keeps the lab for debugging.
    let root = crate::common::Scratch::adopt(std::env::temp_dir().join(format!(
        "native-export-{}-{tag}-{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    )));
    let db = root.join("db");
    fs::create_dir_all(&db).unwrap();
    let mut bytes = Vec::new();
    flate2::read::GzDecoder::new(fs::File::open(fixture().join("tiny.1CD.gz")).unwrap())
        .read_to_end(&mut bytes)
        .unwrap();
    fs::write(db.join("1Cv8.1CD"), bytes).unwrap();
    (root, db)
}
fn run(db: &Path, words: &[&str], options: &[OsString]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .args(["infobase", "config", "export"])
        .args(words)
        .arg(format!("--db-path={}", db.display()))
        .arg("--platform=8.5.1")
        .args(options)
        .output()
        .unwrap()
}
fn opt(name: &str, value: &Path) -> OsString {
    let mut s = OsString::from(format!("--{name}="));
    s.push(value);
    s
}
fn ok(o: &Output) {
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(
        o.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
}
fn tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    walkdir::WalkDir::new(root)
        .into_iter()
        .map(Result::unwrap)
        .filter(|e| e.file_type().is_file())
        .map(|e| {
            (
                e.path()
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/"),
                fs::read(e.path()).unwrap(),
            )
        })
        .collect()
}
#[test]
fn info_stdout_and_directory_match_native_bytes_without_database_changes() {
    let (root, db) = lab("info");
    let before = fs::read(db.join("1Cv8.1CD")).unwrap();
    let expected = fs::read(fixture().join("tiny-xml/ConfigDumpInfo.xml")).unwrap();
    let stdout = run(&db, &["info"], &[]);
    ok(&stdout);
    assert_eq!(
        stdout.stdout,
        expected.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&expected)
    );
    let out = root.join("info");
    fs::create_dir(&out).unwrap();
    let output = run(&db, &["info"], &[opt("out", &out)]);
    ok(&output);
    assert_eq!(fs::read(out.join("ConfigDumpInfo.xml")).unwrap(), expected);
    assert_eq!(output.stdout,"[INFO] Получение информации о состоянии конфигурации информационной базы...\n[INFO] Получение информации о состоянии конфигурации информационной базы успешно завершено.\n".as_bytes());
    assert_eq!(fs::read(db.join("1Cv8.1CD")).unwrap(), before);
}
#[test]
fn status_protocol_and_reference_changes_match_native() {
    let (root, db) = lab("status");
    let current = run(&db, &["info"], &[]);
    ok(&current);
    let xml = String::from_utf8(current.stdout).unwrap();
    let reference = root.join("base.xml");
    fs::write(&reference, &xml).unwrap();
    let same = run(&db, &["status"], &[opt("base", &reference)]);
    ok(&same);
    assert!(same.stdout.is_empty());
    let target = root.join("status.txt");
    ok(&run(
        &db,
        &["status"],
        &[opt("base", &reference), opt("out", &target)],
    ));
    assert_eq!(fs::read(&target).unwrap(), b"\xef\xbb\xbf");
    let changed = xml.replace("c57df04f818190488e0bdade6c7ca93900000000", &"0".repeat(40));
    assert_ne!(changed, xml);
    fs::write(&reference, changed).unwrap();
    for (extra, expected) in [
        (
            vec![],
            "modified: CommonModule.ФайловаяБазаПроверка.Module\n",
        ),
        (
            vec![OsString::from("--short")],
            "M: CommonModule.ФайловаяБазаПроверка.Module\n",
        ),
    ] {
        let mut args = vec![opt("base", &reference)];
        args.extend(extra);
        let output = run(&db, &["status"], &args);
        ok(&output);
        assert_eq!(output.stdout, expected.as_bytes());
    }
    fs::write(
        &reference,
        xml.replace(
            "CommonModule.ФайловаяБазаПроверка\"",
            "CommonModule.PreviousName\"",
        ),
    )
    .unwrap();
    let renamed = run(&db, &["status"], &[opt("base", &reference)]);
    ok(&renamed);
    assert_eq!(renamed.stdout, b"modified: all\n");
}
#[test]
fn objects_select_only_native_files_and_fail_before_writing_unknown_objects() {
    let (root, db) = lab("objects");
    let out = root.join("out");
    let output = run(
        &db,
        &["objects"],
        &[opt("out", &out), "CommonModule.ФайловаяБазаПроверка".into()],
    );
    ok(&output);
    let expected = tree(&fixture().join("tiny-xml"))
        .into_iter()
        .filter(|(name, _)| name.starts_with("CommonModules/"))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(tree(&out), expected);
    let absent = root.join("absent");
    let output = run(
        &db,
        &["objects"],
        &[opt("out", &absent), "CommonModule.NoSuchLabObject".into()],
    );
    assert_eq!(output.status.code(), Some(FAILED));
    assert!(!absent.exists());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "[ERROR] Операция невозможна, при выполнении экспорта объектов конфигурации в XML обнаружены ошибки: Объект метаданных CommonModule.NoSuchLabObject не существует в конфигурации.\n"
    );
}
#[test]
fn native_base_noop_creates_no_directory_and_conflict_precedes_database_read() {
    let (root, db) = lab("base");
    let current = run(&db, &["info"], &[]);
    ok(&current);
    let base = root.join("base.xml");
    fs::write(&base, current.stdout).unwrap();
    let out = root.join("out");
    let output = run(
        &db,
        &[],
        &[opt("base", &base), out.clone().into_os_string()],
    );
    ok(&output);
    assert!(!out.exists());
    let output = run(
        &root.join("does-not-exist"),
        &[],
        &[opt("base", &base), "--sync".into(), out.into_os_string()],
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        "Опции base и sync не могут быть использованы одновременно.\n".as_bytes()
    );
}
#[test]
fn malformed_status_and_invalid_file_target_match_native_exit_codes() {
    let (root, db) = lab("errors");
    let missing = run(&db, &["status"], &[]);
    assert_eq!(missing.status.code(), Some(2));
    assert!(missing.stdout.is_empty());
    assert_eq!(
        missing.stderr,
        "Не указан путь к файлу ConfigDumpInfo.xml\n".as_bytes()
    );
    let info = run(&db, &["info"], &[]);
    ok(&info);
    let base = root.join("base.xml");
    fs::write(&base, info.stdout).unwrap();
    let target = root.join("dir");
    fs::create_dir(&target).unwrap();
    let error = run(&db, &["status"], &[opt("base", &base), opt("out", &target)]);
    assert_eq!(error.status.code(), Some(FAILED));
    assert_eq!(error.stderr,"[ERROR] Операция невозможна, при вычислении изменений конфигурации информационной базы относительно ранее сохраненного состояния обнаружены ошибки: Ошибка приемника потока\n".as_bytes());
    assert!(!fs::read_dir(&root).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".ibcmd-export-")
    }));
}

#[test]
fn status_directory_target_does_not_mask_input_or_inventory_errors() {
    let (root, db) = lab("status-error-order");
    let target = root.join("out");
    fs::create_dir(&target).unwrap();
    let base = root.join("base.xml");
    for contents in [None, Some(b"invalid inventory".as_slice())] {
        if let Some(contents) = contents {
            fs::write(&base, contents).unwrap();
        }
        let ordinary = run(&db, &["status"], &[opt("base", &base)]);
        let directory = run(&db, &["status"], &[opt("base", &base), opt("out", &target)]);
        assert_eq!(ordinary.status.code(), Some(FAILED));
        assert_eq!(directory.status.code(), Some(FAILED));
        assert_eq!(directory.stderr, ordinary.stderr);
        assert!(!String::from_utf8_lossy(&directory.stderr).contains("Ошибка приемника потока"));
    }
    let bad = root.join("malformed.1CD");
    fs::write(&bad, b"not a database").unwrap();
    let ordinary = run(&bad, &["status"], &[opt("base", &base)]);
    let directory = run(
        &bad,
        &["status"],
        &[opt("base", &base), opt("out", &target)],
    );
    assert_eq!(directory.status.code(), Some(FAILED));
    assert_eq!(directory.stderr, ordinary.stderr);
    assert!(tree(&target).is_empty());
    assert!(!fs::read_dir(&root).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".ibcmd-export-")
    }));
}

#[test]
fn sync_failure_has_no_success_period_before_error_suffix() {
    let (root, db) = lab("sync-failure-punctuation");
    let target = root.join("out");
    fs::create_dir(&target).unwrap();
    let output = run(&db, &[], &["--sync".into(), target.into_os_string()]);
    assert_eq!(output.status.code(), Some(FAILED));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.ends_with("завершен с ошибкой\n"), "{stderr}");
    assert!(!stderr.contains("завершен. с ошибкой"));
}

#[test]
fn query_file_target_never_reinterprets_directory_required_spelling() {
    let (root, db) = lab("query-directory-spelling");
    let base = root.join("base.xml");
    fs::write(&base, run(&db, &["info"], &[]).stdout).unwrap();
    let sentinel = root.join("sentinel.txt");
    fs::write(&sentinel, b"untouched").unwrap();
    let absent = root.join("must-not-exist");
    let separators = if cfg!(windows) {
        vec!["/", "\\"]
    } else {
        vec!["/"]
    };
    for target in [&sentinel, &absent] {
        for separator in &separators {
            for suffix in [format!("{separator}."), separator.to_string()] {
                let mut spelling = target.as_os_str().to_owned();
                spelling.push(suffix);
                let output = run(
                    &db,
                    &["status"],
                    &[opt("base", &base), opt("out", Path::new(&spelling))],
                );
                assert_eq!(output.status.code(), Some(FAILED));
            }
        }
    }
    let traversal = root.join("missing/../sentinel.txt");
    let output = run(
        &db,
        &["status"],
        &[opt("base", &base), opt("out", &traversal)],
    );
    assert_eq!(output.status.code(), Some(FAILED));
    assert_eq!(fs::read(&sentinel).unwrap(), b"untouched");
    assert!(!absent.exists());
    assert!(!root.join("missing").exists());
}

#[cfg(unix)]
#[test]
fn query_output_resolves_directory_aliases_but_rejects_leaf_links() {
    use std::os::unix::fs::symlink;
    let (root, db) = lab("query-alias");
    let real = root.join("real");
    let alias = root.join("alias");
    fs::create_dir_all(real.join("info")).unwrap();
    symlink(&real, &alias).unwrap();
    let info = run(&db, &["info"], &[opt("out", &alias.join("info"))]);
    ok(&info);
    let base = real.join("info/ConfigDumpInfo.xml");
    assert_eq!(
        fs::read(&base).unwrap(),
        fs::read(fixture().join("tiny-xml/ConfigDumpInfo.xml")).unwrap()
    );
    let target = alias.join("missing/nested/chosen-name.txt");
    ok(&run(
        &db,
        &["status"],
        &[opt("base", &base), opt("out", &target)],
    ));
    assert_eq!(
        fs::read(real.join("missing/nested/chosen-name.txt")).unwrap(),
        b"\xef\xbb\xbf"
    );
    let sentinel = real.join("sentinel");
    fs::write(&sentinel, b"untouched").unwrap();
    let linked = alias.join("linked.txt");
    symlink(&sentinel, &linked).unwrap();
    let dangling = alias.join("dangling.txt");
    symlink(real.join("must-not-exist"), &dangling).unwrap();
    for leaf in [&linked, &dangling] {
        let output = run(&db, &["status"], &[opt("base", &base), opt("out", leaf)]);
        assert_eq!(output.status.code(), Some(FAILED));
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("linked output path is not allowed")
        );
        assert!(fs::symlink_metadata(leaf).unwrap().file_type().is_symlink());
    }
    assert_eq!(fs::read(&sentinel).unwrap(), b"untouched");
    assert!(!real.join("must-not-exist").exists());
}

fn recorded_lab(case: &str) -> (crate::common::Scratch, PathBuf, PathBuf) {
    let root = crate::common::Scratch::adopt(std::env::temp_dir().join(format!(
        "native-export-recorded-{}-{}",
        case,
        uuid::Uuid::new_v4()
    )));
    let db = root.join("db");
    fs::create_dir_all(&db).unwrap();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/native-export-commands")
        .join(case);
    let mut bytes = Vec::new();
    flate2::read::GzDecoder::new(fs::File::open(fixture.join("database.1CD.gz")).unwrap())
        .read_to_end(&mut bytes)
        .unwrap();
    fs::write(db.join("1Cv8.1CD"), bytes).unwrap();
    (root, db, fixture)
}
#[test]
fn real_native_change_add_delete_rename_matrix_is_byte_exact() {
    for case in ["change", "add", "delete", "rename"] {
        let (root, db, recorded) = recorded_lab(case);
        let info = run(&db, &["info"], &[]);
        ok(&info);
        let expected = fs::read(recorded.join("full/ConfigDumpInfo.xml")).unwrap();
        assert_eq!(
            info.stdout,
            expected.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&expected),
            "{case} info"
        );
        let base = fixture().join("tiny-xml/ConfigDumpInfo.xml");
        let status = run(&db, &["status"], &[opt("base", &base)]);
        ok(&status);
        assert_eq!(
            status.stdout,
            fs::read(recorded.join("status.txt")).unwrap(),
            "{case} status"
        );
        let out = root.join("base");
        let output = run(
            &db,
            &[],
            &[opt("base", &base), out.clone().into_os_string()],
        );
        if matches!(case, "delete" | "rename") {
            assert_eq!(output.status.code(), Some(FAILED));
            assert!(!out.exists());
            assert_eq!(output.stderr,"[ERROR] Операция невозможна, при выполнении экспорта изменений конфигурации в XML обнаружены ошибки: Требуется экспортировать конфигурацию полностью.\n".as_bytes());
        } else {
            ok(&output);
            assert_eq!(tree(&out), tree(&recorded.join("base")), "{case} base");
        }
        let sync = root.join("sync");
        for (name, bytes) in tree(&fixture().join("tiny-xml")) {
            let target = sync.join(name);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(target, bytes).unwrap();
        }
        let before = tree(&sync);
        let output = run(&db, &[], &["--sync".into(), sync.clone().into_os_string()]);
        if matches!(case, "delete" | "rename") {
            assert_eq!(output.status.code(), Some(FAILED));
            assert_eq!(tree(&sync), before);
        } else {
            ok(&output);
            assert_eq!(tree(&sync), tree(&recorded.join("full")), "{case} sync");
        }
    }
}
#[test]
fn native_recursion_and_configuration_selection_match_recorded_files() {
    let (root, db, recorded) = recorded_lab("owned");
    for (tag, flags) in [
        ("shallow", vec![]),
        ("recursive", vec![OsString::from("--recursive")]),
    ] {
        let out = root.join(tag);
        let mut args = vec![opt("out", &out)];
        args.extend(flags);
        args.push("Catalog.CorpusList".into());
        let output = run(&db, &["objects"], &args);
        ok(&output);
        assert_eq!(tree(&out), tree(&recorded.join(tag)), "{tag}");
    }
    let (root, db) = lab("configuration-recursion");
    let out = root.join("recursive");
    let output = run(
        &db,
        &["objects"],
        &[
            opt("out", &out),
            "--recursive".into(),
            "Configuration.Configuration".into(),
        ],
    );
    ok(&output);
    let mut expected = tree(&fixture().join("tiny-xml"));
    expected.remove("ConfigDumpInfo.xml");
    assert_eq!(tree(&out), expected);
}
#[test]
#[cfg(unix)]
fn linked_object_output_is_rejected_without_writing_outside_destination() {
    let (root, db) = lab("links");
    let out = root.join("out");
    let outside = root.join("outside");
    fs::create_dir(&out).unwrap();
    fs::create_dir(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, out.join("CommonModules")).unwrap();
    let result = run(
        &db,
        &["objects"],
        &[opt("out", &out), "CommonModule.ФайловаяБазаПроверка".into()],
    );
    assert_eq!(result.status.code(), Some(FAILED));
    assert!(tree(&outside).is_empty());
}

#[test]
fn custom_configuration_object_selection_keeps_its_existing_scope() {
    let (root, db) = lab("custom-config");
    let out = root.join("out");
    let result = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .args(["mssql-dump-config", "--file-infobase"])
        .arg(&db)
        .args([
            "--object",
            "Configuration",
            "--main-configuration",
            "--platform",
            "8.5.1",
            "--extract-metadata-xml",
            "--extract-module-text",
            "--no-binary-rows",
            "-o",
        ])
        .arg(&out)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let mut actual = tree(&out);
    actual.remove("manifest.json");
    assert_eq!(
        actual,
        BTreeMap::from([(
            "Configuration.xml".to_owned(),
            fs::read(fixture().join("tiny-xml/Configuration.xml")).unwrap()
        )])
    );
}

#[test]
fn native_version_comparison_ignores_xml_header_and_requires_object_names() {
    let (root, db) = lab("header");
    let current = run(&db, &["info"], &[]);
    ok(&current);
    let xml = String::from_utf8(current.stdout).unwrap();
    let reference = root.join("older.xml");
    fs::write(
        &reference,
        xml.replace("version=\"2.21\"", "version=\"2.20\""),
    )
    .unwrap();
    let status = run(&db, &["status"], &[opt("base", &reference)]);
    ok(&status);
    assert!(status.stdout.is_empty());
    let out = root.join("out");
    let export = run(
        &db,
        &[],
        &[opt("base", &reference), out.clone().into_os_string()],
    );
    ok(&export);
    assert!(!out.exists());
    // Native 8.5.1.1592 keeps a changed row incremental even with a 2.20 reference header.
    let changed = xml
        .replace("version=\"2.21\"", "version=\"2.20\"")
        .replace("c57df04f818190488e0bdade6c7ca93900000000", &"0".repeat(40));
    fs::write(&reference, changed).unwrap();
    ok(&run(
        &db,
        &[],
        &[opt("base", &reference), out.clone().into_os_string()],
    ));
    let written = tree(&out);
    assert_eq!(written.len(), 2);
    assert_eq!(
        written["ConfigDumpInfo.xml"],
        fs::read(fixture().join("tiny-xml/ConfigDumpInfo.xml")).unwrap()
    );
    let module = "CommonModules/ФайловаяБазаПроверка/Ext/Module.bsl";
    assert_eq!(
        written[module],
        fs::read(fixture().join("tiny-xml").join(module)).unwrap()
    );
    let objects = run(&db, &["objects"], &[opt("out", &out)]);
    assert_eq!(objects.status.code(), Some(2));
    assert!(objects.stdout.is_empty());
    assert_eq!(
        objects.stderr,
        "Не указано ни одного объекта метаданных.\n".as_bytes()
    );
}

#[test]
fn empty_published_inventory_matches_native_and_malformed_nonempty_stays_closed() {
    let (root, db) = lab("empty-info");
    let mut bytes = Vec::new();
    flate2::read::GzDecoder::new(fs::File::open(fixture().join("empty.1CD.gz")).unwrap())
        .read_to_end(&mut bytes)
        .unwrap();
    fs::write(db.join("1Cv8.1CD"), bytes).unwrap();
    for (platform, version) in [("8.3.27", "2.20"), ("8.5.1", "2.21")] {
        let output = run(&db, &["info"], &[format!("--platform={platform}").into()]);
        ok(&output);
        let expected = fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "tests/fixtures/native-export-commands/empty-info-{version}.xml"
        )))
        .unwrap();
        assert_eq!(
            output.stdout,
            expected.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&expected)
        );
    }
    // Default platform resolution must also tolerate a genuinely empty table.
    let output = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .env_remove("IBCMD_RS_PLATFORM")
        .args(["infobase", "config", "export", "info"])
        .arg(opt("db-path", &db))
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        output.stdout,
        fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/native-export-commands/empty-info-2.20.xml")
        )
        .unwrap()
        .strip_prefix(b"\xef\xbb\xbf")
        .unwrap()
    );
    let warning = format!(
        "warning: no platform is configured for {}; assuming 8.3.27 (XML 2.20). Name it with --platform, a [[database]] entry or `platform` in ibcmd-rs.toml, or IBCMD_RS_PLATFORM.\n",
        fs::canonicalize(&db).unwrap().display()
    );
    assert_eq!(output.stderr, warning.as_bytes());
    let bad = root.join("malformed.1CD");
    fs::write(&bad, b"not a file database").unwrap();
    let output = run(&bad, &["info"], &[]);
    assert_eq!(output.status.code(), Some(FAILED));
    assert!(output.stdout.is_empty());
}

#[test]
fn empty_query_outputs_fail_before_database_read_or_cwd_publication() {
    let (root, db) = lab("empty-object-output");
    let cwd = root.join("selected-cwd");
    fs::create_dir_all(cwd.join("CommonModules")).unwrap();
    let sentinel = cwd.join("CommonModules/ФайловаяБазаПроверка.xml");
    fs::write(&sentinel, b"keep this source").unwrap();
    for database in [&db, &root.join("missing-db")] {
        for out in [
            vec![OsString::from("--out=")],
            vec!["--out".into(), "".into()],
        ] {
            let output = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
                .current_dir(&cwd)
                .args(["infobase", "config", "export", "objects"])
                .arg(opt("db-path", database))
                .arg("--platform=8.5.1")
                .args(out)
                .arg("CommonModule.ФайловаяБазаПроверка")
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(2));
            assert!(output.stdout.is_empty());
            assert!(
                String::from_utf8_lossy(&output.stderr)
                    .contains("Не указано значение параметра: out")
            );
            assert_eq!(fs::read(&sentinel).unwrap(), b"keep this source");
            assert_eq!(tree(&cwd).len(), 1);
        }
    }
}

#[test]
fn query_failure_reports_replace_stale_success_and_preserve_original_error() {
    let (root, db) = lab("failure-reports");
    let base = root.join("malformed-reference.xml");
    fs::write(&base, b"not inventory XML").unwrap();
    let report = root.join("report.json");
    for (command, options) in [
        (
            "objects",
            vec![
                opt("out", &root.join("objects")),
                "CommonModule.NoSuchObject".into(),
            ],
        ),
        ("status", vec![opt("base", &base)]),
        ("info", vec![opt("out", &root.join("missing-directory"))]),
    ] {
        fs::write(&report, br#"{"operation":"previous run","ok":true}"#).unwrap();
        let mut options = options;
        options.push(opt("report", &report));
        let output = run(&db, &[command], &options);
        assert_eq!(output.status.code(), Some(FAILED));
        let value: serde_json::Value = serde_json::from_slice(&fs::read(&report).unwrap()).unwrap();
        assert_eq!(value["ok"], false);
        assert_eq!(
            value["operation"],
            format!("infobase config export {command}")
        );
        let message = value["error"].as_str().unwrap();
        assert!(!message.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains(message));
    }
    let invalid_report = root.join("report-directory");
    fs::create_dir(&invalid_report).unwrap();
    fs::write(invalid_report.join("keep"), b"unchanged").unwrap();
    let output = run(
        &db,
        &["objects"],
        &[
            opt("out", &root.join("out")),
            opt("report", &invalid_report),
            "CommonModule.NoSuchObject".into(),
        ],
    );
    assert_eq!(output.status.code(), Some(FAILED));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Объект метаданных CommonModule.NoSuchObject не существует"));
    assert!(stderr.contains("failed to write the report"));
    assert_eq!(fs::read(invalid_report.join("keep")).unwrap(), b"unchanged");
}

#[cfg(unix)]
#[test]
fn object_root_and_ancestor_links_are_rejected_before_any_target_file_changes() {
    use std::os::unix::fs::symlink;
    let (root, db) = lab("object-root-aliases");
    for nested in [false, true] {
        let tag = if nested { "ancestor" } else { "root" };
        let target = root.join(format!("{tag}-target"));
        fs::create_dir(&target).unwrap();
        let alias = root.join(format!("{tag}-alias"));
        symlink(&target, &alias).unwrap();
        let (out, actual) = if nested {
            fs::create_dir(target.join("nested")).unwrap();
            (alias.join("nested"), target.join("nested"))
        } else {
            (alias, target.clone())
        };
        fs::create_dir(actual.join("CommonModules")).unwrap();
        let sentinel = actual.join("CommonModules/ФайловаяБазаПроверка.xml");
        fs::write(&sentinel, b"keep target bytes").unwrap();
        let before = tree(&target);
        let report = root.join(format!("{tag}-report.json"));
        let output = run(
            &db,
            &["objects"],
            &[
                opt("out", &out),
                opt("report", &report),
                "CommonModule.ФайловаяБазаПроверка".into(),
            ],
        );
        assert_eq!(output.status.code(), Some(FAILED));
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("linked output path is not allowed")
        );
        assert_eq!(tree(&target), before);
        let value: serde_json::Value = serde_json::from_slice(&fs::read(report).unwrap()).unwrap();
        assert_eq!(value["ok"], false);
    }
}

#[test]
fn native_empty_base_and_sync_noops_preserve_their_output_contracts() {
    let (root, db) = lab("empty-incremental-noop");
    let mut bytes = Vec::new();
    flate2::read::GzDecoder::new(fs::File::open(fixture().join("empty.1CD.gz")).unwrap())
        .read_to_end(&mut bytes)
        .unwrap();
    fs::write(db.join("1Cv8.1CD"), bytes).unwrap();
    let current = run(&db, &["info"], &[]);
    ok(&current);
    let base = root.join("base.xml");
    fs::write(&base, &current.stdout).unwrap();
    let absent = root.join("absent-output");
    ok(&run(
        &db,
        &[],
        &[opt("base", &base), absent.clone().into_os_string()],
    ));
    assert!(
        !absent.exists(),
        "native equal-base export must stay a no-op"
    );
    let output = root.join("sync-output");
    fs::create_dir(&output).unwrap();
    fs::write(output.join("ConfigDumpInfo.xml"), current.stdout).unwrap();
    fs::write(output.join("notes.txt"), b"keep local bytes").unwrap();
    let before = tree(&output);
    ok(&run(
        &db,
        &[],
        &["--sync".into(), output.clone().into_os_string()],
    ));
    assert_eq!(
        tree(&output),
        before,
        "native unchanged sync must not publish or prune"
    );
}

#[test]
fn postgres_query_routes_refuse_before_connecting_or_publishing() {
    use std::{
        io::ErrorKind,
        net::TcpListener,
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        },
        thread,
        time::Duration,
    };

    let root = std::env::temp_dir().join(format!("native-pg-query-{}", uuid::Uuid::new_v4()));
    let _scratch = crate::common::Scratch::adopt(&root);
    fs::create_dir(&root).unwrap();
    let reference_bytes = fs::read(fixture().join("tiny-xml/ConfigDumpInfo.xml")).unwrap();
    let mut failures = Vec::new();
    for from_settings in [false, true] {
        for query in ["info", "status", "objects"] {
            let origin = if from_settings { "settings" } else { "flags" };
            let case = root.join(format!("{origin}-{query}"));
            fs::create_dir(&case).unwrap();
            let reference = case.join("reference.xml");
            fs::write(&reference, &reference_bytes).unwrap();
            let defaults = case.join("empty.toml");
            fs::write(&defaults, b"").unwrap();
            let destination = case.join("output");
            if query == "info" {
                fs::create_dir(&destination).unwrap();
                fs::write(destination.join("keep.txt"), b"keep existing output").unwrap();
            }
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let server = listener.local_addr().unwrap().to_string();
            let connections = Arc::new(AtomicUsize::new(0));
            let done = Arc::new(AtomicBool::new(false));
            let counted = Arc::clone(&connections);
            let finished = Arc::clone(&done);
            let trap = thread::spawn(move || {
                loop {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            counted.fetch_add(1, Ordering::SeqCst);
                            drop(stream); // A bad probe must fail promptly instead of waiting for a reply.
                        }
                        Err(error) if error.kind() == ErrorKind::WouldBlock => {
                            if finished.load(Ordering::SeqCst) {
                                break;
                            }
                            thread::sleep(Duration::from_millis(2));
                        }
                        Err(error) => panic!("synthetic PostgreSQL listener: {error}"),
                    }
                }
            });
            let mut command = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"));
            command
                .current_dir(&case)
                .env("IBCMD_RS_CONFIG", &defaults)
                .env_remove("IBCMD_RS_PLATFORM")
                .env_remove("IBCMD_RS_DB_SERVER")
                .env_remove("IBCMD_RS_DB_USER")
                .env_remove("IBCMD_DB_PSW")
                .args(["infobase", "config", "export", query])
                .arg(opt("out", &destination));
            if query == "status" {
                command.arg(opt("base", &reference));
            } else if query == "objects" {
                command.arg("CommonModule.ФайловаяБазаПроверка");
            }
            if from_settings {
                let settings = case.join("connection.json");
                fs::write(
                    &settings,
                    serde_json::to_vec(&serde_json::json!({
                        "vrunner": {
                            "dbms-type": "PostgreSQL",
                            "dbms-server": server,
                            "dbms-base": "synthetic_query",
                            "dbms-user": "synthetic"
                        }
                    }))
                    .unwrap(),
                )
                .unwrap();
                command.arg(opt("settings", &settings));
            } else {
                command.args([
                    "--dbms=PostgreSQL".to_owned(),
                    format!("--db-server={server}"),
                    "--db-name=synthetic_query".to_owned(),
                    "--db-user=synthetic".to_owned(),
                ]);
            }
            let before = tree(&case);
            let output = command.output().unwrap();
            done.store(true, Ordering::SeqCst);
            trap.join().unwrap();
            let connections = connections.load(Ordering::SeqCst);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let unchanged = tree(&case) == before && destination.exists() == (query == "info");
            eprintln!(
                "{origin}/{query}: code={:?}, connections={connections}, unchanged={unchanged}",
                output.status.code()
            );
            if output.status.code() != Some(1)
                || !stderr.contains("для PostgreSQL не поддерживается")
                || !stderr.contains("config export, config import и config load")
                || connections != 0
                || !unchanged
            {
                failures.push(format!("{origin}/{query}: code={:?}, connections={connections}, unchanged={unchanged}, stderr={stderr}", output.status.code()));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "unsupported query regressions:\n{}",
        failures.join("\n")
    );
}

fn mutate_descriptor(
    db: &Path,
    id: &str,
    mutate: impl FnOnce(&mut ibcmd_rs::metadata_model::brace::Brace),
) {
    use ibcmd_rs::metadata_model::brace::{parse_row, serialize_row};
    use std::io::Write;
    let mut source = ibcmd_filedb::FileDb::open(db).unwrap();
    let mut rows = source.config_rows("CONFIG").unwrap();
    drop(source);
    let row = rows.iter_mut().find(|row| row.file_name == id).unwrap();
    let mut plain = Vec::new();
    flate2::read::DeflateDecoder::new(row.binary.as_slice())
        .read_to_end(&mut plain)
        .unwrap();
    let mut descriptor = parse_row(&plain).unwrap();
    mutate(&mut descriptor);
    let mut packed = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
    packed.write_all(&serialize_row(&descriptor)).unwrap();
    row.binary = packed.finish().unwrap();
    row.data_size = row.binary.len() as u64;
    ibcmd_filedb::replace_config_table(db, "CONFIG", &rows).unwrap();
}

fn install_unadmitted_common_module_descriptor(db: &Path) {
    use ibcmd_rs::metadata_model::brace::Brace;
    mutate_descriptor(db, "6d1c2a0e-1cd0-4f00-9a00-000000000001", |descriptor| {
        let fields = descriptor.as_list_mut().unwrap()[1].as_list_mut().unwrap();
        assert_eq!(fields[0].as_atom(), Some("12"));
        assert_eq!(fields[8].as_atom(), Some("0"));
        // A clean-room unknown ReturnValuesReuse discriminator leaves the object's
        // identity and its independent, valid module body intact.
        fields[8] = Brace::num(99);
    });
}

#[test]
fn selected_object_conversion_refusal_preserves_the_entire_destination() {
    let (root, db) = lab("selected-incomplete-descriptor");
    install_unadmitted_common_module_descriptor(&db);
    let database_before = fs::read(db.join("1Cv8.1CD")).unwrap();
    let mut failures = Vec::new();
    for existing in [false, true] {
        let target = root.join(if existing { "existing" } else { "absent" });
        if existing {
            let module = target.join("CommonModules/ФайловаяБазаПроверка/Ext/Module.bsl");
            fs::create_dir_all(module.parent().unwrap()).unwrap();
            fs::write(module, b"keep selected existing module").unwrap();
            fs::write(target.join("sentinel.txt"), b"keep unrelated local bytes").unwrap();
        }
        let before = if existing {
            tree(&target)
        } else {
            BTreeMap::new()
        };
        let output = run(
            &db,
            &["objects"],
            &[
                opt("out", &target),
                "CommonModule.ФайловаяБазаПроверка".into(),
            ],
        );
        let after = if target.exists() {
            tree(&target)
        } else {
            BTreeMap::new()
        };
        let preserved = after == before && target.exists() == existing;
        eprintln!(
            "selected unsupported descriptor: existing={existing}, code={:?}, preserved={preserved}, files={}",
            output.status.code(),
            after.len()
        );
        if output.status.code() != Some(FAILED) || !preserved {
            failures.push(format!(
                "existing={existing}, code={:?}, preserved={preserved}, files={:?}, stderr={}",
                output.status.code(),
                after.keys(),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    }
    assert_eq!(fs::read(db.join("1Cv8.1CD")).unwrap(), database_before);
    assert!(
        failures.is_empty(),
        "incomplete selected export published output:\n{}",
        failures.join("\n")
    );
}

#[test]
fn unselected_unsupported_descriptor_does_not_block_a_selected_object() {
    let (root, db) = lab("unselected-incomplete-descriptor");
    install_unadmitted_common_module_descriptor(&db);
    let target = root.join("language");
    let output = run(
        &db,
        &["objects"],
        &[opt("out", &target), "Language.English".into()],
    );
    ok(&output);
    assert_eq!(
        tree(&target),
        tree(&fixture().join("tiny-xml"))
            .into_iter()
            .filter(|(name, _)| name.starts_with("Languages/"))
            .collect()
    );
}

fn install_synthetic_object(
    db: &Path,
    kind: &str,
    name: &str,
    uuid: &str,
    object: &[u8],
    body: Option<&[u8]>,
) {
    use ibcmd_rs::metadata_model::{DescriptorContext, compile_descriptor, root};
    use std::{io::Write, sync::Arc};
    let source = fixture().join("tiny-source");
    let configuration = fs::read_to_string(source.join("Configuration.xml"))
        .unwrap()
        .replace(
            "</ChildObjects>",
            &format!("<{kind}>{name}</{kind}></ChildObjects>"),
        )
        .into_bytes();
    let mut files = tree(&source)
        .into_iter()
        .filter(|(name, _)| name.ends_with(".xml"))
        .map(|(name, bytes)| (source.join(name), Arc::new(bytes)))
        .collect::<Vec<_>>();
    *files
        .iter_mut()
        .find(|(path, _)| *path == source.join("Configuration.xml"))
        .unwrap() = (
        source.join("Configuration.xml"),
        Arc::new(configuration.clone()),
    );
    let object_path = source.join(format!("{kind}s/{name}.xml"));
    files.push((object_path.clone(), Arc::new(object.to_vec())));
    let context = DescriptorContext::with_files(&source, "2.21", &files).unwrap();
    let configuration_uuid = root::configuration_facts(&configuration).unwrap().uuid;
    let configuration = compile_descriptor(
        "Configuration",
        &source.join("Configuration.xml"),
        &configuration,
        &context,
    )
    .unwrap();
    let object = compile_descriptor(kind, &object_path, object, &context).unwrap();
    let mut source = ibcmd_filedb::FileDb::open(db).unwrap();
    let mut rows = source.config_rows("CONFIG").unwrap();
    drop(source);
    let template = rows[0].clone();
    let mut replace = |name: String, bytes: &[u8]| {
        let mut packed =
            flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
        packed.write_all(bytes).unwrap();
        let mut row = template.clone();
        row.file_name = name.clone();
        row.binary = packed.finish().unwrap();
        row.data_size = row.binary.len() as u64;
        if let Some(old) = rows.iter_mut().find(|row| row.file_name == name) {
            *old = row;
        } else {
            rows.push(row);
        }
    };
    replace(configuration_uuid, &configuration);
    replace(uuid.into(), &object);
    if let Some(body) = body {
        replace(format!("{uuid}.0"), body);
    }
    let versions = root::versions_row(
        &rows
            .iter()
            .map(|row| row.file_name.clone())
            .collect::<Vec<_>>(),
        || uuid::Uuid::new_v4().to_string(),
    );
    let versions_row = rows
        .iter_mut()
        .find(|row| row.file_name == "versions")
        .unwrap();
    let mut packed = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
    packed.write_all(&versions).unwrap();
    versions_row.binary = packed.finish().unwrap();
    versions_row.data_size = versions_row.binary.len() as u64;
    ibcmd_filedb::replace_config_table(db, "CONFIG", &rows).unwrap();
}

fn install_unadmitted_form_asset(db: &Path, opaque: bool) {
    let form_uuid = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaa1";
    let form = format!(r#"<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.21"><CommonForm uuid="{form_uuid}"><Properties><Name>OpaqueForm</Name><FormType>Managed</FormType><IncludeHelpInContents>false</IncludeHelpInContents><UseStandardCommands>false</UseStandardCommands></Properties></CommonForm></MetaDataObject>"#).into_bytes();
    // This clean-room managed-form body has a valid module and a diagnosed,
    // unsupported dynamic-list property. The form XML must be withheld.
    let attribute = format!(
        r##"{{9,{{17}},0,"List",{{1,0}},{{"Pattern",{{"#",65abad24-838b-4987-8b35-ed9e2bd4d9c8}}}},{{0,{{0,{{"B",1}},0}}}},{{0,{{0,{{"B",1}},0}}}},{{0,0}},{{0,0}},0,0,0,0,{{0,3,"MainTable",{{"#",fc01b5df-97fe-449b-83d4-218a090e681e,11111111-1111-4111-8111-111111111111}},"AutoSaveUserSettings",{{"B",1}},"ItemsViewMode",{{"S","{}"}}}},{{0,0}}}}"##,
        '\u{1}'
    );
    let opaque_radio = r#"{37,{38,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,5,"OpaqueRadio",1,0,{1,0},{1,0},{0},{0},1,0,2,0,2,{1,0},{1,0},1,1,0,3,0,3,1,3,0,{4,0,{0},"",-1,-1,1,0,""},{4,0,{0},"",-1,-1,1,0,""},{3,4,{0}},{7,3,0,1,100},{3,4,{0}},{3,4,{0}},{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{8,{9,2}}}"#;
    let body = if opaque {
        // A diagnosed opaque ChoiceList is normally an observational report,
        // rather than a conversion error. Selected export must reject it too.
        format!("{{4,{{59,1,cccccccc-cccc-4ccc-8ccc-cccccccccccc,{opaque_radio}}},\"Procedure KeepMe()\r\nEndProcedure\r\n\",{{0}}}}")
    } else {
        format!("{{4,{{0}},\"Procedure KeepMe()\r\nEndProcedure\r\n\",{{4,1,{attribute}}}}}")
    }.into_bytes();
    install_synthetic_object(
        db,
        "CommonForm",
        "OpaqueForm",
        form_uuid,
        &form,
        Some(&body),
    );
}

#[test]
fn selected_source_asset_refusal_preserves_existing_and_absent_destinations() {
    let mut failures = Vec::new();
    for opaque in [true, false] {
        let (root, db) = lab("selected-incomplete-asset");
        install_unadmitted_form_asset(&db, opaque);
        let database_before = fs::read(db.join("1Cv8.1CD")).unwrap();
        for existing in [false, true] {
            let target = root.join(if existing { "existing" } else { "absent" });
            if existing {
                fs::create_dir(&target).unwrap();
                fs::write(target.join("sentinel.txt"), b"keep local bytes").unwrap();
            }
            let before = if existing {
                tree(&target)
            } else {
                BTreeMap::new()
            };
            let output = run(
                &db,
                &["objects"],
                &[opt("out", &target), "CommonForm.OpaqueForm".into()],
            );
            let after = if target.exists() {
                tree(&target)
            } else {
                BTreeMap::new()
            };
            let preserved = before == after && target.exists() == existing;
            eprintln!(
                "selected unsupported source asset: opaque={opaque}, existing={existing}, code={:?}, preserved={preserved}, files={}",
                output.status.code(),
                after.len()
            );
            if output.status.code() != Some(FAILED) || !preserved {
                failures.push(format!(
                    "existing={existing}, code={:?}, preserved={preserved}, files={:?}, stderr={}",
                    output.status.code(),
                    after.keys(),
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
        }
        assert_eq!(fs::read(db.join("1Cv8.1CD")).unwrap(), database_before);
        // The same unsupported body is outside this selection and must not turn
        // a complete selected module into a whole-configuration strict check.
        let target = root.join("module");
        ok(&run(
            &db,
            &["objects"],
            &[
                opt("out", &target),
                "CommonModule.ФайловаяБазаПроверка".into(),
            ],
        ));
        assert_eq!(
            tree(&target),
            tree(&fixture().join("tiny-xml"))
                .into_iter()
                .filter(|(name, _)| name.starts_with("CommonModules/"))
                .collect()
        );
    }
    assert!(
        failures.is_empty(),
        "incomplete selected source asset published output:\n{}",
        failures.join("\n")
    );
}

#[test]
fn selected_nested_descriptor_refusal_does_not_hide_behind_the_root_inventory() {
    use ibcmd_rs::metadata_model::brace::Brace;
    let (_recorded_root, recorded_db, recorded) = recorded_lab("owned");
    let mut source = ibcmd_filedb::FileDb::open(&recorded_db).unwrap();
    let rows = source.config_rows("CONFIG").unwrap();
    drop(source);
    // Keep the measured writable collation of the own tiny database. The
    // recorded export-only source uses en_US, which this writer refuses.
    let (root, db) = lab("nested-descriptor");
    ibcmd_filedb::replace_config_table(&db, "CONFIG", &rows).unwrap();
    mutate_descriptor(&db, "c099c30b-8a08-45b0-be7d-9594366e78a5", |descriptor| {
        let wrapper = descriptor.as_list_mut().unwrap()[1].as_list_mut().unwrap();
        assert_eq!(wrapper[0].as_atom(), Some("0"));
        let form = wrapper[1].as_list_mut().unwrap();
        assert_eq!(form[0].as_atom(), Some("13"));
        assert_eq!(form[2].as_atom(), Some("0"));
        form[2] = Brace::num(99); // Unknown IncludeHelpInContents; form type stays valid.
    });
    let database_before = fs::read(db.join("1Cv8.1CD")).unwrap();
    let mut failures = Vec::new();
    for (tag, names) in [
        ("leaf", vec!["Catalog.CorpusList.Form.ListForm".into()]),
        (
            "recursive",
            vec!["--recursive".into(), "Catalog.CorpusList".into()],
        ),
    ] {
        let target = root.join(tag);
        fs::create_dir(&target).unwrap();
        fs::write(target.join("sentinel.txt"), b"unchanged").unwrap();
        let before = tree(&target);
        let mut options = vec![opt("out", &target)];
        options.extend(names);
        let output = run(&db, &["objects"], &options);
        let preserved = tree(&target) == before;
        eprintln!(
            "selected nested descriptor: {tag}, code={:?}, preserved={preserved}",
            output.status.code()
        );
        if output.status.code() != Some(FAILED) || !preserved {
            failures.push(format!(
                "{tag}: code={:?}, preserved={preserved}, stderr={}",
                output.status.code(),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    }
    // Its parent without --recursive does not select this form.
    let target = root.join("shallow");
    ok(&run(
        &db,
        &["objects"],
        &[opt("out", &target), "Catalog.CorpusList".into()],
    ));
    assert_eq!(tree(&target), tree(&recorded.join("shallow")));
    assert_eq!(fs::read(db.join("1Cv8.1CD")).unwrap(), database_before);
    assert!(
        failures.is_empty(),
        "nested descriptor conversion was not checked:\n{}",
        failures.join("\n")
    );
}

#[test]
fn unselected_orphan_form_diagnostics_do_not_expand_selected_completeness() {
    use std::io::Write;
    let (_recorded_root, recorded_db, recorded) = recorded_lab("owned");
    let mut source = ibcmd_filedb::FileDb::open(&recorded_db).unwrap();
    let mut rows = source.config_rows("CONFIG").unwrap();
    drop(source);
    let form_uuid = "c099c30b-8a08-45b0-be7d-9594366e78a5";
    let orphan_uuid = "dddddddd-dddd-4ddd-8ddd-dddddddddddd";
    let mut orphan = rows
        .iter()
        .find(|row| row.file_name == form_uuid)
        .unwrap()
        .clone();
    let mut plain = String::new();
    flate2::read::DeflateDecoder::new(orphan.binary.as_slice())
        .read_to_string(&mut plain)
        .unwrap();
    let mut packed = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
    packed
        .write_all(
            plain
                .replace(form_uuid, orphan_uuid)
                .replace("ListForm", "OrphanForm")
                .as_bytes(),
        )
        .unwrap();
    orphan.file_name = orphan_uuid.into();
    orphan.binary = packed.finish().unwrap();
    orphan.data_size = orphan.binary.len() as u64;
    let mut body = rows
        .iter()
        .find(|row| row.file_name == format!("{form_uuid}.0"))
        .unwrap()
        .clone();
    body.file_name = format!("{orphan_uuid}.0");
    rows.extend([orphan, body]);
    let (root, db) = lab("unselected-orphan-form");
    ibcmd_filedb::replace_config_table(&db, "CONFIG", &rows).unwrap();
    let before = fs::read(db.join("1Cv8.1CD")).unwrap();
    // A broad owner index sees this form with no owner. The selected parent
    // still has its own, valid child identity and its shallow native output.
    for (tag, flags, expected) in [
        ("shallow", vec![], recorded.join("shallow")),
        (
            "recursive",
            vec![OsString::from("--recursive")],
            recorded.join("recursive"),
        ),
    ] {
        let target = root.join(tag);
        let mut options = vec![opt("out", &target), "Catalog.CorpusList".into()];
        options.extend(flags);
        ok(&run(&db, &["objects"], &options));
        assert_eq!(tree(&target), tree(&expected));
    }
    assert_eq!(fs::read(db.join("1Cv8.1CD")).unwrap(), before);
}

fn selected_with_legacy(db: &Path, target: &Path, names: &[&str], legacy: bool) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .args(["infobase", "config", "export", "objects"])
        .arg(opt("db-path", db))
        .arg("--platform=8.5.1")
        .arg(opt("out", target))
        .args(names)
        .env("IBCMD_RS_LEGACY_EXPORT", if legacy { "1" } else { "0" })
        .env_remove("IBCMD_RS_MODEL_EXPORT")
        .output()
        .unwrap()
}

#[test]
fn selected_semantic_refusal_also_checks_narrow_and_legacy_writer_paths() {
    use ibcmd_rs::metadata_model::brace::Brace;
    let (root, db) = lab("invalid-module-bool");
    mutate_descriptor(&db, "6d1c2a0e-1cd0-4f00-9a00-000000000001", |descriptor| {
        let fields = descriptor.as_list_mut().unwrap()[1].as_list_mut().unwrap();
        assert_eq!(fields[2].as_atom(), Some("1"));
        fields[2] = Brace::num(99);
    });
    let mut failures = Vec::new();
    for legacy in [false, true] {
        let target = root.join(if legacy { "legacy" } else { "default" });
        let output =
            selected_with_legacy(&db, &target, &["CommonModule.ФайловаяБазаПроверка"], legacy);
        eprintln!(
            "selected invalid bool: legacy={legacy}, code={:?}, absent={}",
            output.status.code(),
            !target.exists()
        );
        if output.status.code() != Some(FAILED) || target.exists() {
            failures.push(format!(
                "legacy={legacy}, code={:?}, absent={}, stderr={}",
                output.status.code(),
                !target.exists(),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    }
    let (_recorded_root, recorded_db, _) = recorded_lab("owned");
    let mut source = ibcmd_filedb::FileDb::open(&recorded_db).unwrap();
    let rows = source.config_rows("CONFIG").unwrap();
    drop(source);
    for (tag, invalid) in [
        ("numeric", Brace::num(99)),
        ("token", Brace::atom("unknown")),
        ("string", Brace::str("unknown")),
        ("list", Brace::list(vec![Brace::num(0)])),
    ] {
        let (root, db) = lab("invalid-form-bool");
        ibcmd_filedb::replace_config_table(&db, "CONFIG", &rows).unwrap();
        mutate_descriptor(&db, "c099c30b-8a08-45b0-be7d-9594366e78a5", |descriptor| {
            let wrapper = descriptor.as_list_mut().unwrap()[1].as_list_mut().unwrap();
            wrapper[1].as_list_mut().unwrap()[2] = invalid;
        });
        for legacy in [false, true] {
            let target = root.join(if legacy { "legacy" } else { "default" });
            let output =
                selected_with_legacy(&db, &target, &["--recursive", "Catalog.CorpusList"], legacy);
            eprintln!(
                "selected invalid form bool: {tag}, legacy={legacy}, code={:?}, absent={}",
                output.status.code(),
                !target.exists()
            );
            if output.status.code() != Some(FAILED) || target.exists() {
                failures.push(format!(
                    "{tag}, legacy={legacy}, code={:?}, absent={}, stderr={}",
                    output.status.code(),
                    !target.exists(),
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "selected semantic refusal bypassed:\n{}",
        failures.join("\n")
    );
}

#[test]
fn selected_supported_legacy_shape_retains_a_real_model_coverage_fallback() {
    use ibcmd_rs::metadata_model::brace::Brace;
    let (root, db) = lab("supported-old-subsystem");
    let uuid = "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee";
    let xml = format!(
        r#"<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.21"><Subsystem uuid="{uuid}"><Properties><Name>LegacySubsystem</Name><IncludeHelpInContents>false</IncludeHelpInContents><IncludeInCommandInterface>true</IncludeInCommandInterface><UseOneCommand>false</UseOneCommand><Picture/><Explanation/><Content/></Properties><ChildObjects/></Subsystem></MetaDataObject>"#
    );
    install_synthetic_object(
        &db,
        "Subsystem",
        "LegacySubsystem",
        uuid,
        xml.as_bytes(),
        None,
    );
    let expected_path = root.join("current");
    let output = selected_with_legacy(
        &db,
        &expected_path,
        &["--recursive", "Configuration.Configuration"],
        false,
    );
    ok(&output);
    let expected = tree(&expected_path);
    mutate_descriptor(&db, uuid, |descriptor| {
        let fields = descriptor.as_list_mut().unwrap()[1].as_list_mut().unwrap();
        assert_eq!(fields[0].as_atom(), Some("22"));
        // The legacy reader documents code 21 with the same property layout;
        // the model decoder currently accepts only the code-22 subsystem.
        fields[0] = Brace::num(21);
    });
    for legacy in [false, true] {
        let target = root.join(if legacy { "legacy" } else { "model-fallback" });
        let output = selected_with_legacy(
            &db,
            &target,
            &["--recursive", "Configuration.Configuration"],
            legacy,
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!(
            "supported Subsystem21: legacy={legacy}, code={:?}, Some(Err) fallback={}",
            output.status.code(),
            stderr.contains("the legacy converter writes it")
        );

        assert_eq!(output.status.code(), Some(0), "{stderr}");
        assert_eq!(tree(&target), expected);
        if legacy {
            assert!(stderr.is_empty(), "{stderr}");
        } else {
            assert!(
                stderr.contains("Subsystem") && stderr.contains("the legacy converter writes it"),
                "not a real Some(Err) model fallback: {stderr}"
            );
        }
    }
}

fn install_legacy_fallback_object(db: &Path, command: bool, invalid_slot: Option<usize>) -> String {
    use ibcmd_rs::metadata_model::brace::Brace;
    let uuid = "ffffffff-ffff-4fff-8fff-ffffffffffff";
    let (kind, name, properties) = if command {
        (
            "CommonCommand",
            "LegacyCommand",
            "<Group>ActionsPanelTools</Group><Representation>Auto</Representation><Picture/><ToolTip/><Shortcut/><IncludeHelpInContents>false</IncludeHelpInContents><CommandParameterType/><ParameterUseMode>Single</ParameterUseMode><ModifiesData>false</ModifiesData><OnMainServerUnavalableBehavior>Auto</OnMainServerUnavalableBehavior>",
        )
    } else {
        (
            "Subsystem",
            "LegacySubsystem",
            "<IncludeHelpInContents>false</IncludeHelpInContents><IncludeInCommandInterface>true</IncludeInCommandInterface><UseOneCommand>false</UseOneCommand><Picture/><Explanation/><Content/>",
        )
    };
    let xml = format!(
        r#"<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.21"><{kind} uuid="{uuid}"><Properties><Name>{name}</Name>{properties}</Properties><ChildObjects/></{kind}></MetaDataObject>"#
    );
    install_synthetic_object(db, kind, name, uuid, xml.as_bytes(), None);
    mutate_descriptor(db, uuid, |descriptor| {
        let payload = descriptor.as_list_mut().unwrap()[1].as_list_mut().unwrap();
        let fields = if command {
            let body = payload[1].as_list_mut().unwrap();
            let fields = body[2].as_list_mut().unwrap();
            assert_eq!(fields[0].as_atom(), Some("9"));
            assert_eq!(fields.pop().unwrap().as_atom(), Some("0"));
            fields[0] = Brace::num(8);
            fields
        } else {
            assert_eq!(payload[0].as_atom(), Some("22"));
            payload[0] = Brace::num(21);
            payload
        };
        if let Some(slot) = invalid_slot {
            if slot == fields.len() {
                fields.push(Brace::num(99));
            } else {
                fields[slot] = Brace::num(99);
            }
        }
    });
    format!("{kind}.{name}")
}

#[test]
fn selected_legacy_aliases_never_default_present_invalid_scalars() {
    let mut failures = Vec::new();
    for (command, slot) in [
        (false, 2),
        (false, 4),
        (false, 8),
        (true, 6),
        (true, 10),
        (true, 2),
        (true, 11),
        (true, 12),
    ] {
        let (root, db) = lab("legacy-alias-invalid-scalar");
        let name = install_legacy_fallback_object(&db, command, Some(slot));
        let database_before = fs::read(db.join("1Cv8.1CD")).unwrap();
        for legacy in [false, true] {
            for existing in [false, true] {
                let target = root.join(format!("out-{legacy}-{existing}"));
                if existing {
                    fs::create_dir(&target).unwrap();
                    fs::write(target.join("sentinel.txt"), b"unchanged").unwrap();
                }
                let before = if existing {
                    tree(&target)
                } else {
                    BTreeMap::new()
                };
                let output = selected_with_legacy(&db, &target, &[&name], legacy);
                let preserved =
                    target.exists() == existing && (!existing || tree(&target) == before);
                eprintln!(
                    "legacy alias scalar: command={command}, slot={slot}, legacy={legacy}, existing={existing}, code={:?}, preserved={preserved}",
                    output.status.code()
                );
                if output.status.code() != Some(FAILED) || !preserved {
                    failures.push(format!("command={command}, slot={slot}, legacy={legacy}, existing={existing}, code={:?}, preserved={preserved}, stderr={}",output.status.code(),String::from_utf8_lossy(&output.stderr)));
                }
            }
        }
        let target = root.join("unselected-language");
        let output = selected_with_legacy(&db, &target, &["Language.English"], false);
        ok(&output);
        assert_eq!(
            tree(&target),
            tree(&fixture().join("tiny-xml"))
                .into_iter()
                .filter(|(name, _)| name.starts_with("Languages/"))
                .collect()
        );
        assert_eq!(fs::read(db.join("1Cv8.1CD")).unwrap(), database_before);
    }
    assert!(
        failures.is_empty(),
        "legacy aliases silently accepted invalid present scalar values:\n{}",
        failures.join("\n")
    );
}

#[test]
fn selected_common_command_retains_its_own_model_coverage_fallback() {
    let (root, db) = lab("legacy-command-coverage");
    let name = install_legacy_fallback_object(&db, true, None);
    let expected_path = root.join("legacy");
    let output = selected_with_legacy(&db, &expected_path, &[&name], true);
    ok(&output);
    let target = root.join("model-fallback");
    let output = selected_with_legacy(&db, &target, &[&name], false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    eprintln!(
        "CommonCommand8 exact row fallback: code={:?}, stderr={stderr}",
        output.status.code()
    );
    assert_eq!(output.status.code(), Some(0), "{stderr}");
    assert!(
        stderr.contains("ffffffff-ffff-4fff-8fff-ffffffffffff:")
            && stderr.contains("CommonCommand")
            && stderr.contains("the legacy converter writes it"),
        "not this row's Some(Err) fallback: {stderr}"
    );
    assert_eq!(tree(&target), tree(&expected_path));
}
