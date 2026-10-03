//! Publish recovery/script files completely, without clobbering another writer.
//! The final path appears only after an adjacent temporary file is synchronized.
//! As in the CF writer, filesystems without hard links refuse publication.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(crate) fn write_new_or_identical(path: &Path, bytes: &[u8]) -> io::Result<()> {
    match identical_existing(path, bytes)? {
        Some(true) => return Ok(()),
        Some(false) => return Err(existing_artifact(path)),
        None => {}
    }
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let (temporary, mut output) = create_temporary(parent)?;
    output.write_all(bytes)?;
    output.flush()?;
    output.sync_all()?;
    drop(output);

    match fs::hard_link(&temporary.path, path) {
        Ok(()) => Ok(()),
        Err(error) => match identical_existing(path, bytes)? {
            Some(true) => Ok(()),
            Some(false) => Err(existing_artifact(path)),
            None => Err(error),
        },
    }
}

fn existing_artifact(path: &Path) -> io::Error {
    io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!("refusing to overwrite existing artifact {}", path.display()),
    )
}

/// Compare at most the proposed length, without allocating the existing file.
fn identical_existing(path: &Path, bytes: &[u8]) -> io::Result<Option<bool>> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if !metadata.is_file() || metadata.len() != bytes.len() as u64 {
        return Ok(Some(false));
    }
    let mut input = File::open(path)?;
    let mut buffer = [0_u8; 16 * 1024];
    for expected in bytes.chunks(buffer.len()) {
        let read = &mut buffer[..expected.len()];
        input.read_exact(read)?;
        if read != expected {
            return Ok(Some(false));
        }
    }
    let mut extra = [0_u8; 1];
    Ok(Some(input.read(&mut extra)? == 0))
}

struct TemporaryFile {
    path: PathBuf,
}

impl Drop for TemporaryFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn create_temporary(parent: &Path) -> io::Result<(TemporaryFile, File)> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos();
    for _ in 0..128 {
        let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!(
            ".ibcmd-artifact-{}-{nonce}-{counter}.tmp",
            std::process::id()
        ));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((TemporaryFile { path }, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "failed to allocate a unique artifact temporary file",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let parent = std::env::temp_dir();
            let (temporary, file) = create_temporary(&parent).unwrap();
            drop(file);
            let path = temporary.path.with_extension("tests");
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            // Only remove the directory created by this test under its temp root.
            let Ok(target) = fs::canonicalize(&self.0) else {
                return;
            };
            let Ok(root) = fs::canonicalize(std::env::temp_dir()) else {
                return;
            };
            if target.parent() == Some(root.as_path())
                && target.file_name().is_some_and(|name| {
                    let name = name.to_string_lossy();
                    name.starts_with(".ibcmd-artifact-") && name.ends_with(".tests")
                })
            {
                let _ = fs::remove_dir_all(target);
            }
        }
    }

    #[test]
    fn retries_accept_identical_bytes_and_preserve_different_existing_artifacts() {
        let directory = TestDirectory::new();
        let path = directory.0.join("nested/recovery.json");
        write_new_or_identical(&path, b"complete recovery").unwrap();
        write_new_or_identical(&path, b"complete recovery").unwrap();
        let error = write_new_or_identical(&path, b"different recovery").unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(path).unwrap(), b"complete recovery");
        assert_eq!(fs::read_dir(directory.0.join("nested")).unwrap().count(), 1);
    }

    #[test]
    fn interrupted_preparation_exposes_no_partial_final_file_and_cleans_temporary() {
        let directory = TestDirectory::new();
        let destination = directory.0.join("recovery.json");
        let (temporary, mut output) = create_temporary(&directory.0).unwrap();
        output.write_all(b"{partial").unwrap();
        assert!(!destination.exists());
        drop(output);
        drop(temporary);
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 0);
        write_new_or_identical(&destination, b"{complete}").unwrap();
        assert_eq!(fs::read(destination).unwrap(), b"{complete}");
    }

    #[test]
    fn competing_different_writers_publish_exactly_one_complete_artifact() {
        let directory = TestDirectory::new();
        let destination = directory.0.join("recovery.json");
        let barrier = Arc::new(Barrier::new(4));
        let writers: Vec<_> = (0_u8..4)
            .map(|value| {
                let destination = destination.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    let payload = vec![value; 256 * 1024];
                    barrier.wait();
                    (value, write_new_or_identical(&destination, &payload))
                })
            })
            .collect();
        let mut winner = None;
        for writer in writers {
            let (value, result) = writer.join().unwrap();
            match result {
                Ok(()) => assert!(winner.replace(value).is_none()),
                Err(error) => assert_eq!(error.kind(), io::ErrorKind::AlreadyExists),
            }
        }
        assert_eq!(
            fs::read(destination).unwrap(),
            vec![winner.unwrap(); 256 * 1024]
        );
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
    }

    #[test]
    fn competing_identical_writers_all_succeed_without_leftover_temporary_files() {
        let directory = TestDirectory::new();
        let destination = directory.0.join("recovery.json");
        let barrier = Arc::new(Barrier::new(4));
        let writers: Vec<_> = (0..4)
            .map(|_| {
                let destination = destination.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    write_new_or_identical(&destination, &vec![42; 256 * 1024])
                })
            })
            .collect();
        for writer in writers {
            writer.join().unwrap().unwrap();
        }
        assert_eq!(fs::read(destination).unwrap(), vec![42; 256 * 1024]);
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
    }
}
