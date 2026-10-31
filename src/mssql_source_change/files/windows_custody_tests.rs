//! Discriminating Windows source-directory controls: no selected leaf may
//! accidentally supply the sharing restriction these tests attribute to dirs.
use super::RootAnchor;
use std::os::windows::fs::OpenOptionsExt;
use std::{
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
};
use windows_sys::Win32::Storage::FileSystem::{
    DELETE, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_DELETE,
    FILE_SHARE_READ, FILE_SHARE_WRITE,
};

struct Fixture {
    owned: PathBuf,
    outer: PathBuf,
    source: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let base = std::env::var_os("TMPDIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("target/test-fixtures"));
        fs::create_dir_all(&base).unwrap();
        let owned = base.join(format!("source-directory-custody-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&owned).unwrap();
        let owned = fs::canonicalize(owned).unwrap();
        let outer = owned.join("outer");
        let source = outer.join("source");
        fs::create_dir_all(&source).unwrap();
        Self {
            owned,
            outer,
            source,
        }
    }
    fn ancestor_rename_roundtrip(&self) {
        let moved = self.owned.join("renamed-outer");
        fs::rename(&self.outer, &moved).unwrap();
        fs::rename(moved, &self.outer).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if std::env::var_os("IBCMD_TEST_KEEP_FIXTURES").as_deref()
            != Some(std::ffi::OsStr::new("1"))
        {
            let _ = fs::remove_dir_all(&self.owned);
        }
    }
}
fn delete_handle(path: &Path) -> std::io::Result<File> {
    OpenOptions::new()
        .access_mode(DELETE)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
}
fn sharing_violation<T: std::fmt::Debug>(result: std::io::Result<T>) {
    let error = result.expect_err("expected a sharing denial, not a permissive metadata handle");
    assert_eq!(error.raw_os_error(), Some(32), "wrong refusal: {error}");
}

#[test]
fn windows_source_directories_deny_delete_without_any_leaf_handle() {
    let fixture = Fixture::new();
    assert!(fs::read_dir(&fixture.source).unwrap().next().is_none());
    fixture.ancestor_rename_roundtrip();
    let held = RootAnchor::open(&fixture.source).unwrap();
    // The source tree is empty; no regular-file handle can hide weak directory custody.
    sharing_violation(delete_handle(&fixture.source));
    sharing_violation(delete_handle(&fixture.outer));
    sharing_violation(fs::rename(
        &fixture.source,
        fixture.outer.join("moved-source"),
    ));
    sharing_violation(fs::rename(
        &fixture.outer,
        fixture.owned.join("moved-outer"),
    ));
    sharing_violation(fs::remove_dir(&fixture.source));
    held.require_path().unwrap();
    drop(held);
    fixture.ancestor_rename_roundtrip();
    drop(delete_handle(&fixture.source).unwrap());
    drop(delete_handle(&fixture.outer).unwrap());
}

#[test]
fn windows_preexisting_ancestor_delete_access_refuses_source_custody() {
    for at_root in [false, true] {
        let fixture = Fixture::new();
        // Establish that this profile/path is admissible before adding only the DELETE open.
        drop(RootAnchor::open(&fixture.source).unwrap());
        let path = if at_root {
            &fixture.source
        } else {
            &fixture.outer
        };
        let delete = delete_handle(path).unwrap();
        let result = RootAnchor::open(&fixture.source);
        assert!(
            result.is_err(),
            "existing ancestor DELETE access was not excluded"
        );
        drop(result);
        drop(delete);
        drop(RootAnchor::open(&fixture.source).unwrap());
        fixture.ancestor_rename_roundtrip();
    }
}

#[test]
fn windows_zero_access_directory_probe_is_not_rename_custody() {
    let fixture = Fixture::new();
    fixture.ancestor_rename_roundtrip();
    let probe = OpenOptions::new()
        .access_mode(0)
        .share_mode(FILE_SHARE_READ)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(&fixture.outer)
        .unwrap();
    // This explicit negative control uses metadata-only access independently
    // of the production helper. It must remain a probe, not asserted custody.
    fixture.ancestor_rename_roundtrip();
    drop(probe);
}

#[test]
fn windows_directory_anchor_preserves_idle_child_edits_without_leaf_custody() {
    let fixture = Fixture::new();
    let held = RootAnchor::open(&fixture.source).unwrap();
    let nested = fixture.source.join("nested");
    fs::create_dir(&nested).unwrap();
    let payload = nested.join("payload.txt");
    fs::write(&payload, b"before").unwrap();
    fs::write(&payload, b"after").unwrap();
    assert_eq!(fs::read(&payload).unwrap(), b"after");
    // Root custody is not a frozen subtree; detached nested paths stay editable.
    fs::rename(&nested, fixture.source.join("renamed-nested")).unwrap();
    held.require_path().unwrap();
    drop(held);
    fixture.ancestor_rename_roundtrip();
}
