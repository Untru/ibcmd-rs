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

/// Whether test fixtures are kept for debugging: `IBCMD_TEST_KEEP_FIXTURES=1`.
pub fn keep_fixtures() -> bool {
    std::env::var_os("IBCMD_TEST_KEEP_FIXTURES").is_some_and(|value| value == "1")
}

/// A directory or file under the system temporary directory that one test
/// owns. Dropping it removes the path: at the end of the test and also when
/// the test fails. `IBCMD_TEST_KEEP_FIXTURES=1` keeps it for debugging
/// (docs/testing.md, "Temporary directories").
pub struct Scratch(PathBuf);

impl Scratch {
    /// Creates the empty directory `<prefix>-<uuid>`.
    pub fn new(prefix: &str) -> Self {
        let path = std::env::temp_dir().join(format!("{prefix}-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    /// Owns `path`, which the test creates itself (or never does).
    pub fn adopt(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }

    /// Removes the path now, retrying while it stays in use for up to
    /// `patience`: a process the test started may still be closing or
    /// writing a file in it.
    pub fn remove_within(self, patience: std::time::Duration) {
        if !keep_fixtures() {
            remove(&self.0, patience);
        }
        std::mem::forget(self);
    }
}

impl std::ops::Deref for Scratch {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

impl AsRef<Path> for Scratch {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl AsRef<std::ffi::OsStr> for Scratch {
    fn as_ref(&self) -> &std::ffi::OsStr {
        self.0.as_os_str()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if !keep_fixtures() {
            remove(&self.0, std::time::Duration::from_secs(2));
        }
    }
}

/// Removes a directory tree or a file, retrying a failed removal for up to
/// `patience` (Windows refuses to delete a file another process still has
/// open); an absent path is not an error.
fn remove(path: &Path, patience: std::time::Duration) {
    let deadline = std::time::Instant::now() + patience;
    loop {
        let removed = match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(path),
            Ok(_) => fs::remove_file(path),
            Err(_) => return,
        };
        if removed.is_ok() || std::time::Instant::now() >= deadline {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

/// A fresh path `ibcmd-external-<tag>-<uuid>` under the temporary directory,
/// not yet created and removed with the returned guard.
///
/// Most suites run as modules of one test process (`tests/integration.rs`),
/// in parallel with each other, so a tag belongs to the suite file that asks
/// for it first and fails here when another file asks for it too: a retained
/// directory (`IBCMD_TEST_KEEP_FIXTURES=1`) then names its suite. Only the
/// tags of one run are compared: an ignored test's tags are checked when it
/// runs with the rest (`-- --include-ignored`).
#[track_caller]
pub fn temp_dir(tag: &str) -> Scratch {
    claim_tag(tag, std::panic::Location::caller().file());
    Scratch::adopt(
        std::env::temp_dir().join(format!("ibcmd-external-{tag}-{}", uuid::Uuid::new_v4())),
    )
}

/// A fresh file path `ibcmd-external-<tag>-<uuid>.<extension>`, as
/// [`temp_dir`].
#[track_caller]
pub fn temp_file(tag: &str, extension: &str) -> Scratch {
    claim_tag(tag, std::panic::Location::caller().file());
    Scratch::adopt(std::env::temp_dir().join(format!(
        "ibcmd-external-{tag}-{}.{extension}",
        uuid::Uuid::new_v4()
    )))
}

/// The fixed directory `ibcmd-external-<tag>`, cleared here and left in place
/// after the test, for tools that look for it by name: the VS Code tests read
/// the `editor-server-protocol` and `editor-server-export` corpora
/// (`tests/editor_server.rs`, `Corpus::kept`). They are the only test
/// directories a run leaves behind, and the CI leftover check allows them.
/// Two runs on one machine share them.
#[track_caller]
pub fn kept_temp_dir(tag: &str) -> PathBuf {
    claim_tag(tag, std::panic::Location::caller().file());
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
