//! Scratch paths of the root unit tests.
//!
//! A test owns each directory or file it creates under the system temporary
//! directory through a [`TestScratch`], which removes it when dropped: at the
//! end of the test and also when an assertion fails. `IBCMD_TEST_KEEP_FIXTURES=1`
//! keeps them for debugging (docs/testing.md, "Temporary directories").

use std::{
    fs,
    path::{Path, PathBuf},
};

/// Whether the test fixtures are kept for debugging:
/// `IBCMD_TEST_KEEP_FIXTURES=1`.
pub(crate) fn keep_fixtures() -> bool {
    std::env::var_os("IBCMD_TEST_KEEP_FIXTURES").is_some_and(|value| value == "1")
}

/// A directory or file under the temporary directory that one test owns.
pub(crate) struct TestScratch(PathBuf);

impl TestScratch {
    /// Creates the empty directory `<prefix>-<uuid>`.
    pub(crate) fn new(prefix: &str) -> Self {
        let path = std::env::temp_dir().join(format!("{prefix}-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    /// Owns `path`, which the test creates itself (or never does).
    pub(crate) fn adopt(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }
}

impl std::ops::Deref for TestScratch {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

impl AsRef<Path> for TestScratch {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestScratch {
    fn drop(&mut self) {
        if !keep_fixtures() {
            remove(&self.0);
        }
    }
}

/// Removes a directory tree or a file, retrying a failed removal for up to two
/// seconds (Windows refuses to delete a file another process still has open);
/// an absent path is not an error.
fn remove(path: &Path) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scratch_is_removed_on_success_and_unwind() {
        let kept = keep_fixtures();
        let dir = {
            let scratch = TestScratch::new("test-scratch-success");
            fs::write(scratch.join("file"), b"owned").unwrap();
            scratch.to_path_buf()
        };
        assert_eq!(dir.exists(), kept);
        let file = std::env::temp_dir().join(format!("test-scratch-file-{}", uuid::Uuid::new_v4()));
        drop({
            let scratch = TestScratch::adopt(&file);
            fs::write(&*scratch, b"owned").unwrap();
            scratch
        });
        assert_eq!(file.exists(), kept);
        let mut unwound = None;
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let scratch = TestScratch::new("test-scratch-unwind");
                unwound = Some(scratch.to_path_buf());
                panic!("synthetic test failure");
            }))
            .is_err()
        );
        assert_eq!(unwound.unwrap().exists(), kept);
    }
}
