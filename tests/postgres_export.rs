//! `infobase config export --dbms=PostgreSQL` (#369) against a live server.
//! The test needs an infobase the platform made and its own export of it:
//!
//! ```text
//! IBCMD_RS_PG_SERVER=127.0.0.1 IBCMD_RS_PG_DATABASE=<db> IBCMD_RS_PG_USER=postgres
//! IBCMD_RS_PG_EXPECTED=<native ibcmd export of that database>
//! ```
//!
//! (`IBCMD_RS_PG_PASSWORD` when the server asks for one). Without them it
//! checks nothing: CI has no PostgreSQL server.

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

#[test]
fn the_export_of_a_postgresql_infobase_is_the_platforms() {
    let (Ok(server), Ok(database), Ok(expected)) = (
        std::env::var("IBCMD_RS_PG_SERVER"),
        std::env::var("IBCMD_RS_PG_DATABASE"),
        std::env::var("IBCMD_RS_PG_EXPECTED"),
    ) else {
        eprintln!("IBCMD_RS_PG_* not set: no PostgreSQL server to export from");
        return;
    };
    let user = std::env::var("IBCMD_RS_PG_USER").unwrap_or_else(|_| "postgres".to_string());
    let out: PathBuf =
        std::env::temp_dir().join(format!("ibcmd-rs-pg-export-{}", std::process::id()));
    let _scratch = crate::common::Scratch::adopt(&out);
    let _ = fs::remove_dir_all(&out);
    let mut command = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"));
    command
        .args(["infobase", "config", "export", "--dbms=PostgreSQL"])
        .arg(format!("--db-server={server}"))
        .arg(format!("--db-name={database}"))
        .arg(format!("--db-user={user}"));
    // The password goes through the environment, not the command line.
    if let Ok(password) = std::env::var("IBCMD_RS_PG_PASSWORD") {
        command.env("IBCMD_DB_PSW", password);
    }
    let output = command.arg(&out).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let (expected, actual) = (files(Path::new(&expected)), files(&out));
    let differ: std::collections::BTreeSet<_> = expected
        .keys()
        .chain(actual.keys())
        .filter(|path| expected.get(*path) != actual.get(*path))
        .collect();
    assert!(
        differ.is_empty(),
        "{} files differ: {differ:?}",
        differ.len()
    );
}
