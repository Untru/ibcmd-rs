//! `infobase config import --dbms=PostgreSQL` (#370) against a live server.
//! The test stages `tests/fixtures/file-infobase/tiny-source` in an infobase
//! the platform made (its `configsave` is replaced) and reads the stage back
//! through the export, which publishes it over `config`:
//!
//! ```text
//! IBCMD_RS_PG_SERVER=127.0.0.1 IBCMD_RS_PG_IMPORT_DATABASE=<scratch infobase>
//! ```
//!
//! (`IBCMD_RS_PG_USER`, `IBCMD_RS_PG_PASSWORD` as for the export test).
//! Without them it checks nothing: CI has no PostgreSQL server.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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

fn run(args: &[String]) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"));
    // The password goes through the environment, never the arguments this
    // prints on a failure.
    if let Ok(password) = std::env::var("IBCMD_RS_PG_PASSWORD") {
        command.env("IBCMD_DB_PSW", password);
    }
    let output = command.args(args).output().unwrap();
    assert!(
        output.status.success(),
        "{args:?}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

#[test]
fn a_tree_imported_into_postgresql_exports_back() {
    let (Ok(server), Ok(database)) = (
        std::env::var("IBCMD_RS_PG_SERVER"),
        std::env::var("IBCMD_RS_PG_IMPORT_DATABASE"),
    ) else {
        eprintln!("IBCMD_RS_PG_* not set: no PostgreSQL server to import into");
        return;
    };
    let user = std::env::var("IBCMD_RS_PG_USER").unwrap_or_else(|_| "postgres".to_string());
    let connection = vec![
        "--dbms=PostgreSQL".to_string(),
        format!("--db-server={server}"),
        format!("--db-name={database}"),
        format!("--db-user={user}"),
    ];
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/file-infobase");
    let source = fixtures.join("tiny-source");
    let mut import = vec!["infobase", "config", "import"]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    import.extend(connection.iter().cloned());
    import.push(source.display().to_string());
    run(&import);

    let out: PathBuf =
        std::env::temp_dir().join(format!("ibcmd-rs-pg-import-{}", std::process::id()));
    let _scratch = crate::common::Scratch::adopt(&out);
    let _ = fs::remove_dir_all(&out);
    let mut export = vec!["infobase", "config", "export", "--platform=8.5.1"]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    export.extend(connection);
    export.push(out.display().to_string());
    run(&export);
    // The platform's export of the same tree, but for the versions its own
    // import draws anew.
    let mut expected = files(&fixtures.join("tiny-xml"));
    let mut actual = files(&out);
    expected.remove("ConfigDumpInfo.xml");
    actual.remove("ConfigDumpInfo.xml");
    assert_eq!(
        expected.keys().collect::<Vec<_>>(),
        actual.keys().collect::<Vec<_>>()
    );
    for (path, bytes) in &expected {
        assert!(actual[path] == *bytes, "{path} differs");
    }
}
