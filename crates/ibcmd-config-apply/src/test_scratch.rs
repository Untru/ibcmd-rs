//! Scratch paths of the unit tests: removed when the guard drops, also when
//! the test fails, unless `IBCMD_TEST_KEEP_FIXTURES=1` keeps them for
//! debugging (docs/testing.md, "Temporary directories").

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
        if keep_fixtures() {
            return;
        }
        // Windows refuses to delete a file another process still has open:
        // a failed removal is retried for up to two seconds.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        loop {
            let removed = match fs::symlink_metadata(&self.0) {
                Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(&self.0),
                Ok(_) => fs::remove_file(&self.0),
                Err(_) => return,
            };
            if removed.is_ok() || std::time::Instant::now() >= deadline {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
}
