//! Unix compatibility controls for the shared snapshot API. Windows acceptance
//! is provided separately by windows_snapshot and must run on Windows CI.
#![cfg(unix)]

use ibcmd_filedb::{Error, ReadLimits, backup_file, open_for_write, open_for_write_with_limits};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

struct Scratch(PathBuf);
impl Scratch {
    fn new(tag: &str) -> Self {
        let base = std::env::var_os("TMPDIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("target/test-fixtures"));
        let root = base.join(format!(
            "filedb-snapshot-unix-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let mut bytes = Vec::new();
        flate2::read::GzDecoder::new(
            fs::File::open(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../tests/fixtures/file-infobase/tiny.1CD.gz"),
            )
            .unwrap(),
        )
        .read_to_end(&mut bytes)
        .unwrap();
        fs::write(root.join("1Cv8.1CD"), bytes).unwrap();
        Self(root)
    }
    fn data(&self) -> PathBuf {
        self.0.join("1Cv8.1CD")
    }
    fn assert_no_outputs(&self) {
        for suffix in [".ibcmd-rs.tmp", ".ibcmd-rs.bak", ".ibcmd-rs.bak.new"] {
            assert!(!self.0.join(format!("1Cv8.1CD{suffix}")).exists());
        }
        assert!(!backup_file(&self.0).exists());
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

#[test]
fn snapshot_preserves_unix_identity_and_full_content_digest() {
    let dir = Scratch::new("compatibility");
    let before = fs::read(dir.data()).unwrap();
    let session = open_for_write(&dir.0).unwrap();
    let metadata = session.metadata().unwrap();
    // This is the established Unix token format, including every file byte.
    let mut legacy = Sha256::new();
    legacy.update(metadata.dev().to_le_bytes());
    legacy.update(metadata.ino().to_le_bytes());
    legacy.update(&before);
    let expected = format!("{:x}", legacy.finalize());
    assert_eq!(session.snapshot().unwrap(), expected);
    assert_eq!(session.snapshot().unwrap(), expected);
    drop(session);
    assert_eq!(fs::read(dir.data()).unwrap(), before);
    dir.assert_no_outputs();
}

#[test]
fn cancelled_snapshot_refuses_without_database_or_recovery_mutation() {
    let dir = Scratch::new("cancelled");
    let before = fs::read(dir.data()).unwrap();
    let cancelled = Arc::new(AtomicBool::new(false));
    let callback = cancelled.clone();
    let session = open_for_write_with_limits(
        &dir.0,
        ReadLimits {
            max_records: 1_000_000,
            max_binary_bytes: 1 << 30,
            max_object_bytes: 1 << 30,
        },
        move || {
            if callback.load(Ordering::SeqCst) {
                return Err(Error::Refused("snapshot cancelled".into()));
            }
            Ok(())
        },
        &mut |_| Ok(()),
    )
    .unwrap();
    cancelled.store(true, Ordering::SeqCst);
    assert!(
        session
            .snapshot()
            .unwrap_err()
            .to_string()
            .contains("snapshot cancelled")
    );
    drop(session);
    assert_eq!(fs::read(dir.data()).unwrap(), before);
    dir.assert_no_outputs();
}
