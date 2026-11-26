//! Locks that keep ibcmd-rs and the platform out of each other's way.
//!
//! On Linux the platform holds a POSIX record lock on `1Cv8.1CD` for as long
//! as the infobase is open (observed with 8.5.1.1150: a read lock on one byte
//! at offset `i64::MAX - 1`). ibcmd-rs locks the whole range: a write lock
//! for import, a read lock for export, so it fails instead of waiting when the
//! platform holds an incompatible lock, and the platform cannot open the
//! infobase while ibcmd-rs writes it.
//!
//! On Windows the platform opens the file with sharing restrictions; ibcmd-rs
//! opens it with no write sharing (export) or no sharing at all (import), so
//! an open infobase is a sharing violation.

use std::fs::{File, OpenOptions};
use std::path::Path;

use crate::error::{Error, Result};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    Read,
    Write,
}

fn busy(path: &Path) -> Error {
    Error::Busy(format!(
        "Информационная база {} открыта платформой или другим процессом: закройте её и повторите",
        path.display()
    ))
}

/// Opens the file and takes the lock; the lock lives as long as the file.
pub(crate) fn open_locked(path: &Path, mode: Mode) -> Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(mode == Mode::Write);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_SHARE_READ: u32 = 1;
        // Refuse a write through a leaf reparse point rather than opening its target.
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x00200000;
        if mode == Mode::Write {
            options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
        }
        options.share_mode(if mode == Mode::Write {
            0
        } else {
            FILE_SHARE_READ
        });
    }
    let file = match options.open(path) {
        Ok(file) => file,
        #[cfg(windows)]
        Err(err) if err.raw_os_error() == Some(32) || err.raw_os_error() == Some(33) => {
            return Err(busy(path));
        }
        Err(err) => return Err(Error::Io(err)),
    };
    #[cfg(unix)]
    posix_lock(&file, mode).map_err(|err| match err.raw_os_error() {
        Some(libc::EAGAIN) | Some(libc::EACCES) => busy(path),
        _ => Error::Io(err),
    })?;
    Ok(file)
}

#[cfg(unix)]
pub(crate) fn lock_write_descriptor(file: &File) -> Result<()> {
    posix_lock(file, Mode::Write).map_err(|error| match error.raw_os_error() {
        Some(libc::EAGAIN) | Some(libc::EACCES) => {
            Error::Busy("file database descriptor is busy".into())
        }
        _ => Error::Io(error),
    })
}

pub(crate) fn lock_read_descriptor(file: &File) -> Result<()> {
    #[cfg(unix)]
    {
        posix_lock(file, Mode::Read).map_err(|error| match error.raw_os_error() {
            Some(libc::EAGAIN) | Some(libc::EACCES) => {
                Error::Busy("file database descriptor is busy".into())
            }
            _ => Error::Io(error),
        })
    }
    #[cfg(not(unix))]
    {
        let _ = file;
        Err(Error::Refused(
            "descriptor-based file database reading is not supported on this OS".into(),
        ))
    }
}

#[cfg(unix)]
#[allow(unsafe_code)]
fn posix_lock(file: &File, mode: Mode) -> std::io::Result<()> {
    use std::os::unix::io::AsRawFd;
    // SAFETY: `flock` is a plain C struct; zeroed is a valid value for every
    // field, and fcntl only reads it for F_SETLK.
    let mut lock: libc::flock = unsafe { std::mem::zeroed() };
    lock.l_type = match mode {
        Mode::Read => libc::F_RDLCK,
        Mode::Write => libc::F_WRLCK,
    } as _;
    lock.l_whence = libc::SEEK_SET as _;
    lock.l_start = 0;
    lock.l_len = 0; // to the end of every possible offset
    // SAFETY: the descriptor is open for the lifetime of `file`, and `lock`
    // points to an initialized struct.
    let rc = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETLK, &lock) };
    if rc == -1 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}
