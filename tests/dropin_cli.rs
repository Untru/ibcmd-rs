//! The drop-in command line, run as a process: every native mode and
//! command either served or refused by name, in Russian, without a panic and
//! without any database (the cases here fail before a connection would be
//! opened). Exit codes as the platform's: 2 for a malformed line, -1 (255 on
//! POSIX) for a failed operation; 1 for what ibcmd-rs does not serve.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use ibcmd_rs::dropin::parse::{NodeKind, OTHER_MODES, command_paths};

fn run(args: &[&str]) -> Output {
    run_with_stdin(args, None)
}

fn run_with_stdin(args: &[&str], stdin: Option<&str>) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .args(args)
        // nothing may be launched: no PATH to find a platform or sqlcmd on
        .env("PATH", "")
        .env_remove("IBCMD_DB_PSW")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(text) = stdin {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(text.as_bytes())
            .unwrap();
    } else {
        drop(child.stdin.take());
    }
    child.wait_with_output().unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).unwrap()
}

/// What ibcmd-rs does not serve.
const UNSUPPORTED: i32 = 1;
/// A malformed or incomplete command line, as the platform's.
const MALFORMED: i32 = 2;
/// A failed operation, as the platform's: `exit(-1)`.
const FAILED: i32 = if cfg!(windows) { -1 } else { 255 };

fn assert_refused(args: &[&str], stream_has: &str) -> Output {
    assert_exit(args, UNSUPPORTED, stream_has)
}

fn assert_malformed(args: &[&str], stream_has: &str) -> Output {
    assert_exit(args, MALFORMED, stream_has)
}

fn assert_exit(args: &[&str], code: i32, stream_has: &str) -> Output {
    let output = run(args);
    let (stdout, stderr) = (text(&output.stdout), text(&output.stderr));
    assert_eq!(
        output.status.code(),
        Some(code),
        "{args:?}\n{stdout}\n{stderr}"
    );
    assert!(
        stdout.contains(stream_has) || stderr.contains(stream_has),
        "{args:?}: expected {stream_has:?}\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(!stderr.contains("panicked"), "{args:?}: {stderr}");
    assert!(
        !stderr.contains("unrecognized subcommand"),
        "{args:?}: {stderr}"
    );
    output
}

struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "ibcmd-rs-dropin-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn arg(&self) -> &str {
        self.0.to_str().unwrap()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn every_other_native_mode_is_refused_by_name() {
    for (mode, _) in OTHER_MODES {
        for args in [
            vec![*mode],
            vec![*mode, "--help"],
            vec![*mode, "list", "--x=1"],
        ] {
            assert_refused(
                &args,
                &format!(
                    "Режим `{mode}` не поддерживается в этой версии ibcmd-rs (планируется в следующих)"
                ),
            );
        }
    }
}

#[test]
fn every_native_infobase_command_is_served_or_refused_by_name() {
    for (path, kind) in command_paths() {
        let mut args = vec!["infobase"];
        args.extend(path.iter().copied());
        args.extend(["--dbms=MSSQLServer", "--db-name=ibcmd_rs_dropin_test"]);
        match kind {
            NodeKind::Unsupported => {
                assert_refused(
                    &args,
                    &format!(
                        "Команда `infobase {}` не поддерживается в этой версии ibcmd-rs (планируется в следующих)",
                        path.join(" ")
                    ),
                );
            }
            NodeKind::Group => {
                assert_malformed(&args, "Указана неполная команда");
            }
            // served: without its path it asks for one
            NodeKind::Export | NodeKind::Import | NodeKind::Save => {
                assert_malformed(&args, "Не указано значение параметра");
            }
            // served: without its directory it asks for one
            NodeKind::ImportFiles => {
                assert_malformed(&args, "Не указано значение параметра: base-dir");
            }
            // served, and it needs no path: it starts, and stops at the
            // connection (a user without a password never reaches the server)
            NodeKind::Apply => {
                args.push("--db-user=sa");
                let output =
                    assert_exit(&args, FAILED, "не указан пароль пользователя сервера СУБД");
                assert_eq!(
                    text(&output.stdout),
                    "[INFO] Обновление конфигурации базы данных...\n"
                );
            }
        }
    }
    assert_malformed(&["infobase"], "Указана неполная команда");
    assert_malformed(&["infobase", "config"], "ibcmd-rs infobase config load");
    assert_malformed(&["infobase", "bogus"], "Указана неполная команда");
}

#[test]
fn unsupported_options_and_malformed_lines_are_refused() {
    let out = TempDir::new("options");
    for (option, code, needle) in [
        (
            "--archive",
            UNSUPPORTED,
            "Параметр `--archive` команды `infobase config export`",
        ),
        ("--file=a.cf", UNSUPPORTED, "Параметр `--file`"),
        ("--sync=1", MALFORMED, "Ошибка разбора параметра: sync"),
        (
            "--base=",
            MALFORMED,
            "Недопустимое значение параметра --base: ",
        ),
        ("--remote=http://h:1545", UNSUPPORTED, "Параметр `--remote`"),
        ("--pid=1", UNSUPPORTED, "Параметр `--pid`"),
        ("--bogus", MALFORMED, "Ошибка разбора параметра: --bogus"),
        ("-T4", MALFORMED, "Ошибка разбора параметра: -T4"),
        (
            "--threads=many",
            MALFORMED,
            "Недопустимое значение параметра --threads: many",
        ),
    ] {
        assert_exit(
            &[
                "infobase",
                "config",
                "export",
                "--dbms=MSSQLServer",
                "--db-name=b",
                option,
                out.arg(),
            ],
            code,
            needle,
        );
    }
    for (option, needle) in [
        (
            "--out=a.cf",
            "Параметр `--out` команды `infobase config import`",
        ),
        (
            "--extension=E",
            "Параметр `--extension` команды `infobase config import`",
        ),
    ] {
        assert_refused(
            &[
                "infobase",
                "config",
                "import",
                "--db-name=b",
                option,
                out.arg(),
            ],
            needle,
        );
    }
    // `--base` and `--sync` update the export of the configuration; an
    // extension is exported in full only
    for option in ["--base=i.xml", "--sync"] {
        let name = option.split('=').next().unwrap();
        assert_refused(
            &[
                "infobase",
                "config",
                "export",
                "--dbms=MSSQLServer",
                "--db-name=b",
                "-e",
                "E",
                option,
                out.arg(),
            ],
            &format!("Параметр `{name}` команды `infobase config export --extension`"),
        );
    }
    for dbms in ["PostgreSQL", "IBMDB2", "OracleDatabase"] {
        assert_refused(
            &[
                "infobase",
                "config",
                "export",
                &format!("--dbms={dbms}"),
                out.arg(),
            ],
            &format!("СУБД `{dbms}` не поддерживается"),
        );
    }
    assert_malformed(
        &["infobase", "config", "export", "--dbms=Foo", out.arg()],
        "Указанный тип СУБД не поддерживается: 'Foo'",
    );
    assert_malformed(
        &["infobase", "config", "export", "--platform=8.4", out.arg()],
        "Недопустимое значение параметра --platform: 8.4 (известные версии: ",
    );
    assert_malformed(
        &[
            "infobase",
            "config",
            "export",
            "--platform=8.3.27",
            "--source-version=2.20",
            out.arg(),
        ],
        "Параметры --platform и --source-version нельзя указывать вместе",
    );
    // no database at all: the platform would open a file infobase; the
    // export starts and fails on the connection
    assert_exit(
        &["infobase", "config", "export", out.arg()],
        FAILED,
        "файловые информационные базы не поддерживаются",
    );
    assert_refused(
        &[
            "infobase",
            "config",
            "export",
            "--db-path=C:\\ib",
            out.arg(),
        ],
        "Файловые информационные базы не поддерживаются",
    );
    let archive = out.path().join("tree.zip");
    fs::write(&archive, b"PK").unwrap();
    assert_refused(
        &[
            "infobase",
            "config",
            "import",
            "--db-name=b",
            archive.to_str().unwrap(),
        ],
        "Импорт конфигурации из архива",
    );
}

#[test]
fn export_refuses_a_non_empty_directory_as_the_platform_does() {
    let out = TempDir::new("not-empty");
    fs::write(out.path().join("x.txt"), b"x").unwrap();
    let report = out.path().join("report.json");
    let output = run(&[
        "infobase",
        "config",
        "export",
        "--dbms=MSSQLServer",
        "--db-server=localhost",
        "--db-name=ibcmd_rs_dropin_test",
        "--force",
        &format!("--report={}", report.display()),
        out.arg(),
    ]);
    assert_eq!(output.status.code(), Some(FAILED));
    assert_eq!(
        text(&output.stdout),
        "[INFO] Экспорт конфигурации в XML...\n"
    );
    assert_eq!(
        text(&output.stderr).trim_end(),
        format!(
            "[ERROR] Операция невозможна, при выполнении экспорта конфигурации в XML обнаружены ошибки: Каталог {} не пуст.",
            out.arg()
        )
    );
    // the directory is left as it was; the report records the failure
    assert!(out.path().join("x.txt").is_file());
    let report: serde_json::Value = serde_json::from_slice(&fs::read(&report).unwrap()).unwrap();
    assert_eq!(report["ok"], false);
    assert_eq!(report["operation"], "infobase config export");
}

#[test]
fn an_extension_export_is_served_and_refuses_a_non_empty_directory_as_well() {
    // `--extension` is served: the line is not refused as unsupported, and the
    // directory is checked before any database is read
    let out = TempDir::new("extension-not-empty");
    fs::write(out.path().join("x.txt"), b"x").unwrap();
    for option in [vec!["--extension=E"], vec!["-e", "E"]] {
        let mut args = vec![
            "infobase",
            "config",
            "export",
            "--dbms=MSSQLServer",
            "--db-server=localhost",
            "--db-name=ibcmd_rs_dropin_test",
        ];
        args.extend(option);
        args.push(out.arg());
        let output = assert_exit(&args, FAILED, "не пуст");
        assert!(
            !text(&output.stderr).contains("не поддерживается"),
            "{args:?}: {}",
            text(&output.stderr)
        );
        assert!(out.path().join("x.txt").is_file());
    }
}

#[test]
fn import_reports_a_missing_tree_in_the_platforms_words() {
    let out = TempDir::new("missing-tree");
    let missing = out.path().join("no-such-tree");
    let output = run(&[
        "infobase",
        "config",
        "import",
        "--dbms=MSSQLServer",
        "--db-name=ibcmd_rs_dropin_test",
        missing.to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(FAILED));
    assert_eq!(
        text(&output.stdout),
        "[INFO] Импорт конфигурации из XML...\n"
    );
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("каталог файлов конфигурации не найден"),
        "{stderr}"
    );
    assert!(
        stderr
            .trim_end()
            .ends_with("[ERROR] Импорт конфигурации из XML завершен с ошибкой"),
        "{stderr}"
    );
}

#[test]
fn save_starts_in_the_platforms_words_and_stops_at_the_connection() {
    // served: it starts, and stops at the connection (a user without a
    // password never reaches the server); no file is written
    let out = TempDir::new("save");
    let file = out.path().join("saved.cf");
    let report = out.path().join("report.json");
    let output = run(&[
        "infobase",
        "config",
        "save",
        "--dbms=MSSQLServer",
        "--db-name=ibcmd_rs_dropin_test",
        "--db-user=sa",
        "--db",
        &format!("--report={}", report.display()),
        file.to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(FAILED));
    assert_eq!(text(&output.stdout), "[INFO] Выгрузка конфигурации...\n");
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("не указан пароль пользователя сервера СУБД"),
        "{stderr}"
    );
    assert!(
        stderr
            .trim_end()
            .ends_with("[ERROR] Выгрузка конфигурации завершена с ошибкой"),
        "{stderr}"
    );
    assert!(!file.exists());
    let report: serde_json::Value = serde_json::from_slice(&fs::read(&report).unwrap()).unwrap();
    assert_eq!(report["ok"], false);
    assert_eq!(report["operation"], "infobase config save");
    // `--extension` is the platform's, not served yet; a missing file is the
    // command line's error
    assert_refused(
        &[
            "infobase",
            "config",
            "save",
            "--db-name=b",
            "--extension=E",
            "x.cf",
        ],
        "Параметр `--extension` команды `infobase config save` не поддерживается",
    );
    assert_malformed(
        &["infobase", "config", "save", "--db-name=b"],
        "Не указано значение параметра: путь к файлу конфигурации",
    );
    // load is still refused by name
    assert_refused(
        &["infobase", "config", "load", "--db-name=b", "x.cf"],
        "Команда `infobase config load` не поддерживается",
    );
}

#[test]
fn the_database_password_can_come_from_stdin() {
    let out = TempDir::new("stdin");
    fs::write(out.path().join("x.txt"), b"x").unwrap();
    // -W reads the password; the run then stops at the non-empty directory,
    // before any connection.
    let output = run_with_stdin(
        &[
            "infobase",
            "config",
            "export",
            "--dbms=MSSQLServer",
            "--db-name=b",
            "--db-user=sa",
            "-W",
            out.arg(),
        ],
        Some("secret\r\n"),
    );
    assert_eq!(output.status.code(), Some(FAILED));
    assert!(text(&output.stderr).contains("не пуст"));
    // without -W and without a password the user is told how to give one
    let output = run(&[
        "infobase",
        "config",
        "export",
        "--dbms=MSSQLServer",
        "--db-name=b",
        "--db-user=sa",
        out.arg(),
    ]);
    assert_eq!(output.status.code(), Some(FAILED));
    assert!(
        text(&output.stderr).contains("не указан пароль пользователя сервера СУБД"),
        "{}",
        text(&output.stderr)
    );
}

#[test]
fn help_and_version() {
    for args in [
        vec!["infobase", "--help"],
        vec!["infobase", "-?"],
        vec!["infobase", "config", "export", "-h"],
        vec!["infobase", "config", "apply", "--help"],
        vec!["help", "infobase"],
    ] {
        let output = run(&args);
        assert_eq!(output.status.code(), Some(0), "{args:?}");
        let help = text(&output.stdout);
        assert!(
            help.contains("Режим управления информационной базой"),
            "{args:?}"
        );
        assert!(help.contains("--db-server"), "{args:?}");
        // served, and no longer named among what is not
        assert!(help.contains("--extension=<name> | -e <name>"), "{args:?}");
        assert!(
            !help.contains("export --base, --file, --extension"),
            "{args:?}"
        );
    }
    let output = run(&["help"]);
    assert_eq!(output.status.code(), Some(0));
    assert!(text(&output.stdout).contains("Поддерживаемые режимы"));
    assert!(text(&output.stdout).contains("config apply"));
    // the help of the mode describes apply, its words and its refusals
    let output = run(&["help", "infobase"]);
    let help = text(&output.stdout);
    for word in [
        "--dynamic=<auto|disable|prompt|force>",
        "--session-terminate=<disable|prompt|force>",
        "--recovery-backup=<file>",
        "--i-have-a-backup",
        "требуется штатный config apply",
    ] {
        assert!(help.contains(word), "{word}");
    }
    let output = run(&["help", "source-diff"]);
    assert_eq!(output.status.code(), Some(0));
    assert!(text(&output.stdout).contains("Usage"));
    assert_refused(&["help", "server"], "Режим `server` не поддерживается");
    assert_malformed(&["help", "bogus"], "Неизвестный режим: bogus");
    let output = run(&["infobase", "--version"]);
    assert_eq!(output.status.code(), Some(0));
    assert!(text(&output.stdout).starts_with("ibcmd-rs "));
    // the platform's `ibcmd -v`
    for flag in ["-v", "-V", "--version"] {
        let output = run(&[flag]);
        assert_eq!(output.status.code(), Some(0), "{flag}");
        assert!(text(&output.stdout).starts_with("ibcmd-rs "), "{flag}");
    }
}

#[test]
fn apply_words_and_refusals_are_the_platforms_before_anything_runs() {
    // every case fails at the command line, before a connection: nothing on
    // stdout, the platform's exit code (2) for what its parser refuses too,
    // ibcmd-rs's (1) for what this version does not serve
    let common = [
        "infobase",
        "config",
        "apply",
        "--dbms=MSSQLServer",
        "--db-name=b",
    ];
    let refused = "не поддерживается в этой версии ibcmd-rs";
    for (option, code, needle) in [
        // measured on 8.3.27.2214: the words are case-sensitive, a missing word is a wrong one
        (
            "--dynamic=bogus",
            MALFORMED,
            "Некорректное значение параметра: dynamic",
        ),
        (
            "--dynamic=AUTO",
            MALFORMED,
            "Некорректное значение параметра: dynamic",
        ),
        (
            "--dynamic=",
            MALFORMED,
            "Некорректное значение параметра: dynamic",
        ),
        (
            "--dynamic",
            MALFORMED,
            "Некорректное значение параметра: dynamic",
        ),
        (
            "--session-terminate=bogus",
            MALFORMED,
            "Некорректное значение параметра: session-terminate",
        ),
        ("--force=yes", MALFORMED, "Ошибка разбора параметра: force"),
        ("--bogus", MALFORMED, "Ошибка разбора параметра: --bogus"),
        (
            "--exclusivity=maybe",
            MALFORMED,
            "Недопустимое значение параметра --exclusivity: maybe",
        ),
        (
            "--recovery-backup=",
            MALFORMED,
            "Недопустимое значение параметра --recovery-backup: ",
        ),
        (
            "--extension=E",
            UNSUPPORTED,
            "Параметр `--extension` команды `infobase config apply`",
        ),
        ("--remote=http://h:1545", UNSUPPORTED, "Параметр `--remote`"),
        ("--pid=1", UNSUPPORTED, "Параметр `--pid`"),
        (
            "--sqlcmd=C:\\sql\\SQLCMD.EXE",
            UNSUPPORTED,
            "работает только через встроенный клиент SQL Server",
        ),
    ] {
        let mut args = common.to_vec();
        args.push(option);
        let output = assert_exit(&args, code, needle);
        assert_eq!(text(&output.stdout), "", "{option}");
        if code == UNSUPPORTED && !option.starts_with("--sqlcmd") {
            assert!(text(&output.stderr).contains(refused), "{option}");
        }
    }
    for dbms in ["PostgreSQL", "IBMDB2", "OracleDatabase"] {
        assert_refused(
            &["infobase", "config", "apply", &format!("--dbms={dbms}")],
            &format!("СУБД `{dbms}` не поддерживается"),
        );
    }
    // what the platform accepts is accepted: the short spellings, the
    // session options that need nobody connected, a stray argument; the
    // run stops at the connection (a user without a password)
    for extra in [
        vec!["-F"],
        vec![
            "--force",
            "--dynamic=disable",
            "--session-terminate=disable",
        ],
        vec![
            "--dynamic=auto",
            "--session-terminate=force",
            "--session-terminate-message=lab",
        ],
        vec!["--dynamic=prompt", "--session-terminate=prompt"],
        // the dynamic apply is served (#347): it parses, and the run stops at the connection like the others
        vec!["--dynamic=force"],
        vec!["--dynamic=force", "--session-terminate=force"],
        vec!["--recovery-backup=F:/lab/before.bak"],
        vec!["--i-have-a-backup"],
        vec!["--platform=8.5.1"],
        vec!["stray"],
    ] {
        let mut args = common.to_vec();
        args.push("--db-user=sa");
        args.extend(extra.iter().copied());
        let output = assert_exit(&args, FAILED, "не указан пароль пользователя сервера СУБД");
        assert_eq!(
            text(&output.stdout),
            "[INFO] Обновление конфигурации базы данных...\n",
            "{extra:?}"
        );
    }
}

#[test]
fn apply_failure_is_told_in_the_platforms_shape_and_recorded() {
    let out = TempDir::new("apply-failure");
    let report = out.path().join("report.json");
    let output = run(&[
        "infobase",
        "config",
        "apply",
        "--dbms=MSSQLServer",
        "--db-server=localhost",
        "--db-name=ibcmd_rs_dropin_test",
        "--db-user=sa",
        "--force",
        &format!("--report={}", report.display()),
    ]);
    assert_eq!(output.status.code(), Some(FAILED));
    assert_eq!(
        text(&output.stdout),
        "[INFO] Обновление конфигурации базы данных...\n"
    );
    let stderr = text(&output.stderr);
    let lines = stderr.lines().collect::<Vec<_>>();
    assert!(lines.len() >= 2, "{stderr}");
    assert!(
        lines[0].starts_with("[ERROR] не указан пароль пользователя сервера СУБД"),
        "{stderr}"
    );
    assert_eq!(
        lines[lines.len() - 1],
        "[ERROR] Обновление конфигурации базы данных завершено с ошибкой"
    );
    let report: serde_json::Value = serde_json::from_slice(&fs::read(&report).unwrap()).unwrap();
    assert_eq!(report["ok"], false);
    assert_eq!(report["operation"], "infobase config apply");
}

/// `infobase config import files` (Untru/ibcmd-rs#363): the platform's lines,
/// the files checked against `--base-dir` before any connection, what is not
/// served refused with exit code 1.
#[test]
fn import_files_checks_the_files_before_the_connection() {
    let out = TempDir::new("import-files");
    let tree = out.path().join("tree");
    for file in [
        "Configuration.xml",
        "Catalogs/X.xml",
        "Catalogs/X/Ext/ObjectModule.bsl",
    ] {
        let path = tree.join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"x").unwrap();
    }
    let sparse = out.path().join("sparse");
    fs::create_dir_all(sparse.join("Catalogs/X/Ext")).unwrap();
    fs::write(sparse.join("Catalogs/X.xml"), b"x").unwrap();
    fs::write(sparse.join("Catalogs/X/Ext/ObjectModule.bsl"), b"x").unwrap();
    let base_dir = format!("--base-dir={}", tree.display());
    let sparse_dir = format!("--base-dir={}", sparse.display());
    fn line<'a>(extra: &[&'a str]) -> Vec<&'a str> {
        let mut args = vec![
            "infobase",
            "config",
            "import",
            "files",
            "--dbms=MSSQLServer",
            "--db-name=ibcmd_rs_dropin_test",
            "--db-user=sa",
        ];
        args.extend_from_slice(extra);
        args
    }

    // the command line
    assert_malformed(
        &line(&[&base_dir]),
        "Не указано значение параметра: файлы конфигурации для загрузки",
    );
    assert_malformed(
        &line(&[&base_dir, "--verify", "--no-check", "Catalogs/X.xml"]),
        "Параметры --verify и --no-check нельзя указывать вместе",
    );
    assert_malformed(
        &[
            "infobase",
            "config",
            "import",
            "--partial",
            tree.to_str().unwrap(),
        ],
        "Ошибка разбора параметра: --partial",
    );
    assert_refused(
        &line(&[&base_dir, "--extension=E", "Catalogs/X.xml"]),
        "Параметр `--extension` команды `infobase config import files`",
    );
    assert_refused(
        &line(&[&base_dir, "--base-free", "Catalogs/X.xml"]),
        "Параметр `--base-free` не применяется",
    );

    // the files, before any connection: the platform's first line, the
    // reason, and the exit code
    for (extra, code, needle) in [
        (
            vec![base_dir.as_str(), "../elsewhere.xml"],
            FAILED,
            "находится вне каталога",
        ),
        (
            vec![base_dir.as_str(), "Catalogs/Y.xml"],
            UNSUPPORTED,
            "удаление файлов и объектов частичной загрузкой не поддерживается",
        ),
        (
            vec![base_dir.as_str(), "Configuration.xml"],
            UNSUPPORTED,
            "частичная загрузка Configuration.xml не поддерживается",
        ),
        (
            vec![sparse_dir.as_str(), "Catalogs/X/Ext/ObjectModule.bsl"],
            UNSUPPORTED,
            "укажите --partial",
        ),
    ] {
        let output = assert_exit(&line(&extra), code, needle);
        assert_eq!(
            text(&output.stdout),
            "[INFO] Импорт файлов конфигурации из XML...\n",
            "{extra:?}"
        );
    }
    let report = out.path().join("report.json");
    let report_arg = format!("--report={}", report.display());
    let output = assert_exit(
        &line(&[&base_dir, &report_arg, "Catalogs/Y.xml"]),
        UNSUPPORTED,
        "удаление",
    );
    assert!(!text(&output.stderr).contains("завершен с ошибкой"));
    let recorded: serde_json::Value = serde_json::from_slice(&fs::read(&report).unwrap()).unwrap();
    assert_eq!(recorded["ok"], false);
    assert_eq!(recorded["operation"], "infobase config import files");

    // files it can place: it goes on, and stops at the connection (a user
    // without a password never reaches the server)
    for extra in [
        vec![
            base_dir.as_str(),
            "Catalogs/X/Ext/ObjectModule.bsl",
            "Catalogs/X.xml",
        ],
        vec![
            sparse_dir.as_str(),
            "--partial",
            "--no-check",
            "Catalogs/X/Ext/ObjectModule.bsl",
        ],
    ] {
        let output = assert_exit(
            &line(&extra),
            FAILED,
            "не указан пароль пользователя сервера СУБД",
        );
        assert_eq!(
            text(&output.stdout),
            "[INFO] Импорт файлов конфигурации из XML...\n"
        );
        assert!(
            text(&output.stderr)
                .trim_end()
                .ends_with("[ERROR] Импорт файлов конфигурации из XML завершен с ошибкой"),
            "{}",
            text(&output.stderr)
        );
    }
}
