//! Admission of the initial local fixed-NTFS snapshot identity profile.
//! The existing 64-bit file index is not a unique ReFS identity; do not fall
//! back to path metadata or silently admit other filesystem/network profiles.
use crate::{Error, Result};
use std::{
    fs::File,
    path::{Component, Path, Prefix},
};

const DRIVE_FIXED: u32 = 3;

pub(super) fn identity(path: &Path, file: &File) -> Result<[u8; 12]> {
    let root = drive_root(path)?;
    let drive_kind = drive_kind(&root);
    let (volume, filesystem) = volume(file)?;
    let id = super::identity(file, false)?;
    admit(drive_kind, &filesystem, volume, id)?;
    let mut encoded = [0; 12];
    encoded[..4].copy_from_slice(&id.0.to_le_bytes());
    encoded[4..].copy_from_slice(&id.1.to_le_bytes());
    Ok(encoded)
}

fn drive_root(path: &Path) -> Result<[u16; 4]> {
    let mut components = path.components();
    let drive = match components.next() {
        Some(Component::Prefix(prefix)) => match prefix.kind() {
            Prefix::Disk(drive) | Prefix::VerbatimDisk(drive) => drive,
            _ => return Err(unsupported()),
        },
        _ => return Err(unsupported()),
    };
    if !drive.is_ascii_alphabetic() || !matches!(components.next(), Some(Component::RootDir)) {
        return Err(unsupported());
    }
    Ok([u16::from(drive), u16::from(b':'), u16::from(b'\\'), 0])
}

fn admit(drive: u32, filesystem: &str, volume: u32, id: (u32, u64)) -> Result<()> {
    if drive != DRIVE_FIXED
        || !filesystem.eq_ignore_ascii_case("NTFS")
        || volume != id.0
        || id.1 == 0
    {
        return Err(unsupported());
    }
    Ok(())
}

fn unsupported() -> Error {
    Error::Refused(
        "Windows snapshot requires a local fixed NTFS drive and a verified handle identity".into(),
    )
}

#[allow(unsafe_code)]
fn drive_kind(root: &[u16; 4]) -> u32 {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetDriveTypeW(root: *const u16) -> u32;
    }
    // SAFETY: fixed-size initialized UTF-16 drive root with required trailing
    // separator and NUL; valid for the duration of the synchronous call.
    unsafe { GetDriveTypeW(root.as_ptr()) }
}

#[allow(unsafe_code)]
fn volume(file: &File) -> Result<(u32, String)> {
    use std::{ffi::c_void, os::windows::io::AsRawHandle, ptr::null_mut};
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetVolumeInformationByHandleW(
            file: *mut c_void,
            name: *mut u16,
            name_len: u32,
            serial: *mut u32,
            component_len: *mut u32,
            flags: *mut u32,
            filesystem: *mut u16,
            filesystem_len: u32,
        ) -> i32;
    }
    let mut serial = 0;
    let mut filesystem = [0u16; 32];
    // SAFETY: File owns a live HANDLE; output buffers are initialized, writable
    // and sized in WCHARs. Unrequested optional outputs are explicitly null.
    let ok = unsafe {
        GetVolumeInformationByHandleW(
            file.as_raw_handle(),
            null_mut(),
            0,
            &mut serial,
            null_mut(),
            null_mut(),
            filesystem.as_mut_ptr(),
            filesystem.len() as u32,
        )
    };
    if ok == 0 {
        return Err(Error::Refused(format!(
            "Windows snapshot volume query failed: {}",
            std::io::Error::last_os_error()
        )));
    }
    let length = filesystem
        .iter()
        .position(|unit| *unit == 0)
        .ok_or_else(unsupported)?;
    let filesystem = String::from_utf16(&filesystem[..length]).map_err(|_| unsupported())?;
    Ok((serial, filesystem))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_profile_refuses_remote_unknown_filesystem_and_unusable_identity() {
        assert!(admit(DRIVE_FIXED, "NTFS", 7, (7, 42)).is_ok());
        for drive in [0, 1, 2, 4, 5, 6] {
            assert!(admit(drive, "NTFS", 7, (7, 42)).is_err());
        }
        for filesystem in ["", "FAT", "exFAT", "ReFS", "CsvFS", "unknown"] {
            assert!(admit(DRIVE_FIXED, filesystem, 7, (7, 42)).is_err());
        }
        assert!(admit(DRIVE_FIXED, "NTFS", 7, (8, 42)).is_err());
        assert!(admit(DRIVE_FIXED, "NTFS", 7, (7, 0)).is_err());
    }

    fn scratch(tag: &str) -> std::path::PathBuf {
        let base = std::env::var_os("TMPDIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("target/test-fixtures"));
        let root = base.join(format!(
            "windows-snapshot-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        root
    }
    fn cleanup(root: std::path::PathBuf) {
        if std::env::var_os("IBCMD_TEST_KEEP_FIXTURES").as_deref()
            != Some(std::ffi::OsStr::new("1"))
        {
            std::fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn windows_publication_faults_preserve_database_and_recovery_generations() {
        use crate::write_paths::WritePaths;
        use std::{fs, io::Write};
        for fail_at in 1..=2 {
            let root = scratch("publish-fault");
            let data = root.join("data");
            let work = root.join("work");
            let pending = root.join("pending");
            let backup = root.join("backup");
            fs::write(&data, b"A").unwrap();
            fs::write(&backup, b"B").unwrap();
            let paths = WritePaths::open(&data).unwrap();
            let original = paths.open_locked(&data).unwrap();
            let mut copy = paths.create_new(&work, true).unwrap();
            copy.write_all(b"C").unwrap();
            paths.require_leaf(&work, &copy).unwrap();
            paths.hard_link(&data, &pending).unwrap();
            paths.require_leaf(&pending, &original).unwrap();
            // Production must release the exclusive original before Windows
            // replacement. Ancestor/work custody remains held through both steps.
            drop(original);
            let mut step = 0;
            let error =
                crate::publish_files(&paths, &work, &data, &pending, &backup, |from, to| {
                    step += 1;
                    assert!(fs::rename(&root, root.with_extension("rebound")).is_err());
                    if step == fail_at {
                        Err(std::io::Error::other("injected rename EIO"))
                    } else {
                        paths.rename(from, to)
                    }
                })
                .unwrap_err();
            assert_eq!(step, fail_at);
            assert_eq!(fs::read(&backup).unwrap(), b"B");
            if fail_at == 1 {
                assert_eq!(fs::read(&data).unwrap(), b"A");
                assert!(!pending.exists());
                assert!(matches!(error, Error::Io(_)));
            } else {
                assert_eq!(fs::read(&data).unwrap(), b"C");
                assert_eq!(fs::read(&pending).unwrap(), b"A");
                assert!(matches!(error, Error::Published { .. }));
            }
            assert!(!work.exists());
            drop(copy);
            drop(paths);
            cleanup(root);
        }
    }

    #[test]
    fn windows_fallback_backup_failure_preserves_prior_generations_and_refuses_reuse() {
        use crate::write_paths::WritePaths;
        use std::{fs, io::Write};
        let root = scratch("backup-fault");
        let data = root.join("data");
        let backup = root.join("backup");
        let pending = root.join("pending");
        fs::write(&data, b"A").unwrap();
        fs::write(&backup, b"B").unwrap();
        let paths = WritePaths::open(&data).unwrap();
        let original = paths.open_locked(&data).unwrap();
        let no_link = || Err(std::io::Error::other("injected hard-link unavailable"));
        let error = crate::prepare_backup_file(&paths, &original, &pending, no_link, |copy| {
            copy.write_all(b"partial")?;
            Err(Error::Refused("injected backup cancellation".into()))
        })
        .unwrap_err();
        assert!(error.to_string().contains("injected backup cancellation"));
        assert!(!pending.exists());
        assert_eq!(fs::read(&backup).unwrap(), b"B");
        fs::write(&pending, b"pre-existing recovery").unwrap();
        let error = crate::prepare_backup_file(&paths, &original, &pending, no_link, |_| {
            panic!("a pre-existing recovery file must not be touched")
        })
        .unwrap_err();
        assert!(error.to_string().contains("pre-existing backup work file"));
        assert_eq!(fs::read(&pending).unwrap(), b"pre-existing recovery");
        drop(original);
        assert_eq!(fs::read(&data).unwrap(), b"A");
        drop(paths);
        cleanup(root);
    }

    #[test]
    fn snapshot_profile_refuses_unc_devices_and_drive_relative_names() {
        for path in [
            r"\\server\share\db\1Cv8.1CD",
            r"\\?\UNC\server\share\db",
            r"\\.\PhysicalDrive0",
            r"C:db\1Cv8.1CD",
            r"db\1Cv8.1CD",
        ] {
            assert!(drive_root(Path::new(path)).is_err(), "{path}");
        }
        for path in [r"C:\db\1Cv8.1CD", r"\\?\C:\db\1Cv8.1CD"] {
            assert_eq!(drive_root(Path::new(path)).unwrap(), [67, 58, 92, 0]);
        }
    }
}
