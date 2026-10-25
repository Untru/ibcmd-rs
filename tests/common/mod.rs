//! Helpers shared by the external-object (.epf/.erf) tests.
#![allow(dead_code)]

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::{Mutex, PoisonError},
};

pub fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/external")
        .join(name)
}

/// Every file under `root` by its `/`-separated relative path.
pub fn files(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    if !root.exists() {
        return out;
    }
    for entry in walkdir::WalkDir::new(root) {
        let entry = entry.expect("walk");
        if entry.file_type().is_file() {
            let rel = entry
                .path()
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            out.insert(rel, fs::read(entry.path()).unwrap());
        }
    }
    out
}

pub fn assert_tree_eq(expected: &Path, actual: &Path) {
    let (e, a) = (files(expected), files(actual));
    let missing: Vec<_> = e.keys().filter(|k| !a.contains_key(*k)).collect();
    let extra: Vec<_> = a.keys().filter(|k| !e.contains_key(*k)).collect();
    let differ: Vec<_> = e
        .keys()
        .filter(|k| a.get(*k).is_some_and(|v| v != &e[*k]))
        .collect();
    assert!(
        missing.is_empty() && extra.is_empty() && differ.is_empty(),
        "tree mismatch\n missing: {missing:?}\n extra: {extra:?}\n differ: {differ:?}"
    );
}

pub fn export(input: &Path, out: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .args(["cf", "export"])
        .arg(input)
        .arg(out)
        .args(["--source-version", "2.20", "--overwrite"])
        .output()
        .expect("run ibcmd-rs")
}

/// A scratch directory per test tag, cleared here and reused by the next run
/// (a per-process name left a new one behind on every run).
///
/// Most suites run as modules of one test process (`tests/integration.rs`),
/// in parallel with each other, so a tag belongs to the suite file that asks
/// for it first: the same tag from another suite would clear the directory
/// under a running test, and fails here instead. Only the tags of one run are
/// compared: an ignored test's tags are checked when it runs with the rest
/// (`-- --include-ignored`).
#[track_caller]
pub fn temp_dir(tag: &str) -> PathBuf {
    claim_tag(tag, std::panic::Location::caller().file());
    if std::env::var_os("IBCMD_TEST_KEEP_FIXTURES").as_deref() == Some(std::ffi::OsStr::new("1")) {
        return std::env::temp_dir().join(format!("ibcmd-external-{tag}-{}", uuid::Uuid::new_v4()));
    }
    let dir = std::env::temp_dir().join(format!("ibcmd-external-{tag}"));
    let _ = fs::remove_dir_all(&dir);
    dir
}

/// Records `suite` (a source file) as the owner of `tag`; panics when another
/// file owns it.
pub fn claim_tag(tag: &str, suite: &'static str) {
    static OWNERS: Mutex<BTreeMap<String, &'static str>> = Mutex::new(BTreeMap::new());
    let mut owners = OWNERS.lock().unwrap_or_else(PoisonError::into_inner);
    let owner = *owners.entry(tag.to_owned()).or_insert(suite);
    assert_eq!(
        owner, suite,
        "scratch tag `{tag}` is used by both {owner} and {suite}: give each suite its own tags"
    );
}
