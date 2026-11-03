//! Canonical actual-source profile admission, independent of any database target.
//! ReFS/remote identity support needs a separately reviewed adapter; no path ID fallback.
use super::{FileIdentity, SourceChangeError, io};
use std::{
    fs::File,
    path::{Component, Path, Prefix},
};
use windows_sys::Win32::Storage::FileSystem::{
    BY_HANDLE_FILE_INFORMATION, GetDriveTypeW, GetVolumeInformationByHandleW,
};

// SDK GetDriveTypeW return code; windows-sys places it behind an unrelated
// WindowsProgramming feature. Keep this documented ABI value local.
// https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getdrivetypew
const DRIVE_FIXED: u32 = 3;

fn unsupported() -> SourceChangeError {
    io(
        "unsupported Windows source profile: requires canonical local fixed NTFS and verified handle identity",
    )
}

pub(super) fn root_path(path: &Path) -> Result<(), SourceChangeError> {
    let root = canonical_drive_root(path)?;
    // SAFETY: a fixed initialized UTF-16 drive root with a trailing separator
    // and NUL remains live throughout this synchronous classification call.
    let kind = unsafe { GetDriveTypeW(root.as_ptr()) };
    admit_drive(kind)
}

fn canonical_drive_root(path: &Path) -> Result<[u16; 4], SourceChangeError> {
    let mut components = path.components();
    let drive = match components.next() {
        Some(Component::Prefix(prefix)) => match prefix.kind() {
            Prefix::Disk(drive) | Prefix::VerbatimDisk(drive) => drive,
            _ => return Err(unsupported()),
        },
        _ => return Err(unsupported()),
    };
    if !drive.is_ascii_alphabetic()
        || !matches!(components.next(), Some(Component::RootDir))
        || components.any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(unsupported());
    }
    Ok([u16::from(drive), u16::from(b':'), u16::from(b'\\'), 0])
}

fn admit_drive(kind: u32) -> Result<(), SourceChangeError> {
    if kind != DRIVE_FIXED {
        return Err(unsupported());
    }
    Ok(())
}

pub(super) fn handle(
    file: &File,
    information: &BY_HANDLE_FILE_INFORMATION,
) -> Result<(), SourceChangeError> {
    use std::{os::windows::io::AsRawHandle, ptr::null_mut};
    let mut serial = 0;
    let mut filesystem = [0u16; 32];
    // SAFETY: the exact source File owns a live HANDLE; output buffers are
    // initialized and sized in WCHARs; all unused optional outputs are null.
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
        return Err(io(format!(
            "Windows source filesystem query failed: {}",
            std::io::Error::last_os_error()
        )));
    }
    let length = filesystem
        .iter()
        .position(|unit| *unit == 0)
        .ok_or_else(unsupported)?;
    let filesystem = String::from_utf16(&filesystem[..length]).map_err(|_| unsupported())?;
    let index =
        (u64::from(information.nFileIndexHigh) << 32) | u64::from(information.nFileIndexLow);
    admit_handle(&filesystem, serial, information.dwVolumeSerialNumber, index)
}

fn admit_handle(
    filesystem: &str,
    volume: u32,
    identity_volume: u32,
    index: u64,
) -> Result<(), SourceChangeError> {
    if !filesystem.eq_ignore_ascii_case("NTFS") || volume != identity_volume || index == 0 {
        return Err(unsupported());
    }
    Ok(())
}

pub(super) fn same_volume(
    root: FileIdentity,
    member: FileIdentity,
) -> Result<(), SourceChangeError> {
    if root.0 != member.0 {
        return Err(unsupported());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_profile_refuses_remote_nonfixed_unknown_and_non_ntfs_facts() {
        assert!(admit_drive(DRIVE_FIXED).is_ok());
        for kind in [0, 1, 2, 4, 5, 6] {
            assert!(admit_drive(kind).is_err());
        }
        assert!(admit_handle("NTFS", 7, 7, 42).is_ok());
        for filesystem in ["", "ReFS", "FAT", "exFAT", "CsvFS", "unknown"] {
            assert!(admit_handle(filesystem, 7, 7, 42).is_err());
        }
        assert!(admit_handle("NTFS", 7, 8, 42).is_err());
        assert!(admit_handle("NTFS", 7, 7, 0).is_err());
    }

    #[test]
    fn source_member_volume_is_bound_to_source_root_not_a_target() {
        let root = FileIdentity(7, 42);
        assert!(same_volume(root, FileIdentity(7, 99)).is_ok());
        assert!(same_volume(root, FileIdentity(8, 99)).is_err());
    }

    #[test]
    fn canonical_root_namespace_is_bounded_without_claiming_raw_operator_policy() {
        for path in [r"C:\owned\source", r"\\?\C:\owned\source"] {
            assert_eq!(
                canonical_drive_root(Path::new(path)).unwrap(),
                [67, 58, 92, 0]
            );
        }
        for path in [
            r"\\server\share\source",
            r"\\?\UNC\server\share\source",
            r"\\.\PhysicalDrive0",
            r"C:source",
            r"source",
            r"\\?\C:\owned\..\source",
        ] {
            assert!(canonical_drive_root(Path::new(path)).is_err(), "{path}");
        }
    }
}
