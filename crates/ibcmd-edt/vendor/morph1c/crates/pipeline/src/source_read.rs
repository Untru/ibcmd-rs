//! Fallible regular-file reads for source body codecs; no payload quota or
//! allocation based on the file's declared size. The source snapshot separately
//! binds content hashes before/after the entire conversion.
use std::fs::{File, Metadata, OpenOptions};
use std::io::{self, Read};
use std::path::Path;

fn regular(metadata: &Metadata) -> bool {
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return false;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // FILE_ATTRIBUTE_REPARSE_POINT: links/junctions are never source files.
        if metadata.file_attributes() & 0x400 != 0 {
            return false;
        }
    }
    true
}

fn same_file(left: &Metadata, right: &Metadata) -> bool {
    if !regular(left)
        || !regular(right)
        || left.len() != right.len()
        || left.modified().ok() != right.modified().ok()
        || left.created().ok() != right.created().ok()
    {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if left.dev() != right.dev() || left.ino() != right.ino() {
            return false;
        }
    }
    true
}

fn ancestors(path: &Path) -> io::Result<()> {
    for parent in path
        .ancestors()
        .skip(1)
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        let metadata = std::fs::symlink_metadata(parent)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "source parent is not a regular directory",
            ));
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "source parent is a reparse point",
                ));
            }
        }
    }
    Ok(())
}

pub(crate) fn read_regular_source(path: &Path) -> io::Result<Vec<u8>> {
    read_with_before_open(path, || {})
}

fn read_with_before_open(path: &Path, before_open: impl FnOnce()) -> io::Result<Vec<u8>> {
    ancestors(path)?;
    let initial = std::fs::symlink_metadata(path)?;
    if !regular(&initial) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "source body is not a regular file",
        ));
    }
    before_open();
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // FILE_SHARE_READ only: disallow concurrent writes or pathname replacement
        // while this handle is live. FILE_FLAG_OPEN_REPARSE_POINT avoids following
        // a leaf link swapped between the pathname check and open.
        options.share_mode(1).custom_flags(0x0020_0000);
    }
    let mut file: File = options.open(path)?;
    if !same_file(&initial, &file.metadata()?) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "source body changed before read",
        ));
    }
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 64 * 1024];
    loop {
        let count = match file.read(&mut chunk) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        if count == 0 {
            break;
        }
        let length = bytes.len().checked_add(count).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::OutOfMemory,
                "source body length exceeds address space",
            )
        })?;
        if u64::try_from(length).map_or(true, |length| length > initial.len()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "source body grew during read",
            ));
        }
        bytes.try_reserve(count).map_err(|error| {
            io::Error::new(
                io::ErrorKind::OutOfMemory,
                format!("source body allocation failed: {error}"),
            )
        })?;
        bytes.extend_from_slice(&chunk[..count]);
    }
    ancestors(path)?;
    if u64::try_from(bytes.len()).ok() != Some(initial.len())
        || !same_file(&initial, &file.metadata()?)
        || !same_file(&initial, &std::fs::symlink_metadata(path)?)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "source body changed during read",
        ));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutation_between_check_and_open_is_rejected() {
        let path = std::env::temp_dir().join(format!(
            "ibcmd-source-body-{}-{}.bin",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, b"before").unwrap();
        let result =
            read_with_before_open(&path, || std::fs::write(&path, b"changed-length").unwrap());
        std::fs::remove_file(&path).unwrap();
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::InvalidData);
    }
}
