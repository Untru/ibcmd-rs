//! Clean-room Windows controls for the snapshot and complete publication chain.
//! No platform process or VM is involved; Windows CI must execute these tests.
#![cfg(windows)]

use ibcmd_filedb::{
    Error, FileDb, ReadLimits, backup_file, open_for_write, open_for_write_with_limits,
};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

struct Scratch(PathBuf);
impl Scratch {
    fn new(tag: &str) -> Self {
        let base = std::env::var_os("TMPDIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("target/test-fixtures"));
        let root = base.join(format!(
            "windows-filedb-snapshot-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("db")).unwrap();
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/file-infobase/tiny.1CD.gz");
        let mut bytes = Vec::new();
        flate2::read::GzDecoder::new(fs::File::open(fixture).unwrap())
            .read_to_end(&mut bytes)
            .unwrap();
        fs::write(root.join("db/1Cv8.1CD"), bytes).unwrap();
        Self(root)
    }
    fn db(&self) -> PathBuf {
        self.0.join("db")
    }
    fn data(&self) -> PathBuf {
        self.db().join("1Cv8.1CD")
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        if std::env::var_os("IBCMD_TEST_KEEP_FIXTURES").as_deref()
            != Some(std::ffi::OsStr::new("1"))
        {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}
fn assert_no_work(dir: &Scratch) {
    for suffix in [".ibcmd-rs.tmp", ".ibcmd-rs.bak.new"] {
        assert!(!dir.db().join(format!("1Cv8.1CD{suffix}")).exists());
    }
}

#[test]
fn snapshot_holds_exclusive_data_and_ancestor_custody() {
    let dir = Scratch::new("held");
    let original = fs::read(dir.data()).unwrap();
    let session = open_for_write(&dir.db()).unwrap();
    let snapshot = session.snapshot().unwrap();
    assert_eq!(snapshot.len(), 64);
    assert_eq!(session.snapshot().unwrap(), snapshot);
    assert!(OpenOptions::new().write(true).open(dir.data()).is_err());
    assert!(fs::remove_file(dir.data()).is_err());
    assert!(fs::rename(dir.data(), dir.db().join("rebound.1CD")).is_err());
    assert!(fs::rename(dir.db(), dir.0.join("rebound-db")).is_err());
    assert_eq!(session.snapshot().unwrap(), snapshot);
    drop(session);
    assert_eq!(fs::read(dir.data()).unwrap(), original);
    assert!(!backup_file(&dir.db()).exists());
    assert_no_work(&dir);
}

#[test]
fn preexisting_ancestor_delete_access_refuses_before_database_or_recovery_mutation() {
    use std::os::windows::fs::OpenOptionsExt;
    let dir = Scratch::new("ancestor-delete");
    let original = fs::read(dir.data()).unwrap();
    // DELETE is ordinary per-handle access on this owned clean-room directory;
    // no ACL, privilege or access setting is changed. The new held ancestor's
    // omission of SHARE_DELETE must conflict with this already-open access.
    let delete = OpenOptions::new()
        .access_mode(0x00010000) // DELETE
        .share_mode(1 | 2 | 4)
        .custom_flags(0x02000000 | 0x00200000) // BACKUP_SEMANTICS | OPEN_REPARSE_POINT
        .open(dir.db())
        .unwrap();
    let error = match open_for_write(&dir.db()) {
        Ok(_) => panic!("ancestor DELETE access was not excluded"),
        Err(error) => error,
    };
    assert!(matches!(error, Error::Io(ref error) if error.raw_os_error() == Some(32)));
    drop(delete);
    assert_eq!(fs::read(dir.data()).unwrap(), original);
    assert!(!backup_file(&dir.db()).exists());
    assert_no_work(&dir);
}

#[test]
fn same_bytes_on_a_distinct_live_identity_invalidate_snapshot() {
    let dir = Scratch::new("identity");
    let original = fs::read(dir.data()).unwrap();
    let before = open_for_write(&dir.db()).unwrap().snapshot().unwrap();
    // Retain the old file object so its identity cannot be recycled for the
    // new leaf in this control. This does not claim universal ID-reuse safety.
    let retained = fs::File::open(dir.data()).unwrap();
    fs::rename(dir.data(), dir.db().join("old.1CD")).unwrap();
    fs::write(dir.data(), &original).unwrap();
    let after = open_for_write(&dir.db()).unwrap().snapshot().unwrap();
    assert_ne!(before, after);
    assert_eq!(fs::read(dir.data()).unwrap(), original);
    drop(retained);
    assert_no_work(&dir);
}

#[test]
fn content_change_in_place_between_sessions_invalidates_snapshot() {
    let dir = Scratch::new("content");
    let before = open_for_write(&dir.db()).unwrap().snapshot().unwrap();
    // An extra byte beyond the declared pages preserves this tiny fixture's
    // parsed tables while changing the same file object's complete contents.
    OpenOptions::new()
        .append(true)
        .open(dir.data())
        .unwrap()
        .write_all(b"x")
        .unwrap();
    let after = open_for_write(&dir.db()).unwrap().snapshot().unwrap();
    assert_ne!(before, after);
    assert_no_work(&dir);
}

#[test]
fn cancellation_during_snapshot_leaves_database_and_artifacts_unchanged() {
    let dir = Scratch::new("cancel");
    let original = fs::read(dir.data()).unwrap();
    let armed = Arc::new(AtomicBool::new(false));
    let calls = Arc::new(AtomicUsize::new(0));
    let callback_armed = armed.clone();
    let callback_calls = calls.clone();
    let session = open_for_write_with_limits(
        &dir.db(),
        ReadLimits {
            max_records: 1_000_000,
            max_binary_bytes: 1 << 30,
            max_object_bytes: 1 << 30,
        },
        move || {
            if callback_armed.load(Ordering::SeqCst)
                && callback_calls.fetch_add(1, Ordering::SeqCst) >= 2
            {
                return Err(Error::Refused("cancelled during Windows snapshot".into()));
            }
            Ok(())
        },
        &mut |_| Ok(()),
    )
    .unwrap();
    armed.store(true, Ordering::SeqCst);
    let error = session.snapshot().unwrap_err();
    assert!(
        error
            .to_string()
            .contains("cancelled during Windows snapshot")
    );
    assert_eq!(calls.load(Ordering::SeqCst), 3);
    drop(session);
    assert_eq!(fs::read(dir.data()).unwrap(), original);
    assert!(!backup_file(&dir.db()).exists());
    assert_no_work(&dir);
}

#[test]
fn snapshot_to_guarded_publication_keeps_base_and_exact_recovery() {
    let dir = Scratch::new("publication");
    let original = fs::read(dir.data()).unwrap();
    let config = FileDb::open(&dir.db())
        .unwrap()
        .config_rows("CONFIG")
        .unwrap();
    let expected = config.clone();
    let backup = backup_file(&dir.db());
    fs::write(&backup, b"previous backup generation").unwrap();
    let session = open_for_write(&dir.db()).unwrap();
    assert_eq!(session.snapshot().unwrap(), session.snapshot().unwrap());
    let mut guards = 0;
    session
        .replace_guarded("CONFIGSAVE", &expected, |metadata| {
            guards += 1;
            assert_eq!(metadata.len(), original.len() as u64);
            assert!(OpenOptions::new().write(true).open(dir.data()).is_err());
            assert!(fs::remove_file(dir.data()).is_err());
            assert!(fs::rename(dir.db(), dir.0.join("rebound-db")).is_err());
            Ok(())
        })
        .unwrap();
    assert_eq!(guards, 2);
    let mut db = FileDb::open(&dir.db()).unwrap();
    assert_eq!(db.config_rows("CONFIG").unwrap(), config);
    assert_eq!(db.config_rows("CONFIGSAVE").unwrap(), expected);
    drop(db);
    assert_eq!(fs::read(&backup).unwrap(), original);
    assert_no_work(&dir);
}

#[test]
fn final_guard_refusal_keeps_database_and_previous_backup() {
    let dir = Scratch::new("final-refusal");
    let original = fs::read(dir.data()).unwrap();
    let config = FileDb::open(&dir.db())
        .unwrap()
        .config_rows("CONFIG")
        .unwrap();
    let backup = backup_file(&dir.db());
    fs::write(&backup, b"previous backup generation").unwrap();
    let session = open_for_write(&dir.db()).unwrap();
    session.snapshot().unwrap();
    let mut guards = 0;
    let error = session
        .replace_guarded("CONFIGSAVE", &config, |_| {
            guards += 1;
            if guards == 2 {
                return Err(Error::Refused("final guard refused".into()));
            }
            Ok(())
        })
        .unwrap_err();
    assert_eq!(guards, 2);
    assert!(error.to_string().contains("final guard refused"));
    assert_eq!(fs::read(dir.data()).unwrap(), original);
    assert_eq!(fs::read(backup).unwrap(), b"previous backup generation");
    assert_no_work(&dir);
}
