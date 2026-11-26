//! Windows anchors directory names by denying DELETE access for each ancestor.
//! Leaf operations stay inside that held chain; identity never falls back to paths.
use crate::{Error, Result};
use std::os::windows::fs::OpenOptionsExt;
use std::{
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
};

const FILE_SHARE_READ: u32 = 1;
const FILE_SHARE_WRITE: u32 = 2;
const FILE_SHARE_DELETE: u32 = 4;
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x00200000;
const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x02000000;
const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

#[repr(C)]
struct FileInformation {
    attributes: u32,
    creation: [u32; 2],
    access: [u32; 2],
    write: [u32; 2],
    volume: u32,
    size_high: u32,
    size_low: u32,
    links: u32,
    index_high: u32,
    index_low: u32,
}
// BY_HANDLE_FILE_INFORMATION has thirteen DWORDs with DWORD alignment on both ABIs.
const _: [(); 52] = [(); std::mem::size_of::<FileInformation>()];
const _: [(); 4] = [(); std::mem::align_of::<FileInformation>()];
type Identity = (u32, u64);

#[allow(unsafe_code)]
fn identity(file: &File, directory: bool) -> std::io::Result<Identity> {
    use std::{mem::MaybeUninit, os::windows::io::AsRawHandle};
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetFileInformationByHandle(
            handle: *mut std::ffi::c_void,
            info: *mut FileInformation,
        ) -> i32;
    }
    let mut info = MaybeUninit::<FileInformation>::zeroed();
    // SAFETY: File owns a live HANDLE; repr(C), size/alignment assertions above
    // match the SDK output record. Read it only after the successful OS call.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), info.as_mut_ptr()) } == 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: successful GetFileInformationByHandle initialized the record.
    let info = unsafe { info.assume_init() };
    if info.attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || (info.attributes & FILE_ATTRIBUTE_DIRECTORY != 0) != directory
    {
        return Err(std::io::Error::other(
            "database custody requires a plain file/directory",
        ));
    }
    Ok((
        info.volume,
        (u64::from(info.index_high) << 32) | u64::from(info.index_low),
    ))
}

fn metadata_handle(path: &Path, directory: bool, share: u32) -> std::io::Result<File> {
    OpenOptions::new()
        .access_mode(0)
        .share_mode(share)
        .custom_flags(
            FILE_FLAG_OPEN_REPARSE_POINT
                | if directory {
                    FILE_FLAG_BACKUP_SEMANTICS
                } else {
                    0
                },
        )
        .open(path)
}

pub(super) struct Custody {
    parent: PathBuf,
    directories: Vec<(PathBuf, File, Identity)>,
}
impl Custody {
    pub(super) fn open(parent: &Path) -> std::io::Result<Self> {
        let parent = std::path::absolute(parent)?;
        let mut directories = Vec::new();
        // Hold each ancestor before resolving its child. No held directory can
        // be renamed/deleted, including between the final guard and publication.
        for path in parent.ancestors().collect::<Vec<_>>().into_iter().rev() {
            let file = metadata_handle(path, true, FILE_SHARE_READ | FILE_SHARE_WRITE)?;
            let id = identity(&file, true)?;
            directories.push((path.to_owned(), file, id));
        }
        Ok(Self {
            parent,
            directories,
        })
    }
    pub(super) fn checked(&self, path: &Path) -> std::io::Result<PathBuf> {
        let path = std::path::absolute(path)?;
        if path.parent() != Some(self.parent.as_path()) || path.file_name().is_none() {
            return Err(std::io::Error::other(
                "mutation outside held database directory",
            ));
        }
        for (name, held, expected) in &self.directories {
            let current = metadata_handle(name, true, FILE_SHARE_READ | FILE_SHARE_WRITE)?;
            if identity(held, true)? != *expected || identity(&current, true)? != *expected {
                return Err(std::io::Error::other(
                    "file database parent directory was rebound",
                ));
            }
        }
        Ok(path)
    }
    pub(super) fn require_leaf(&self, path: &Path, original: &File) -> Result<()> {
        let path = self.checked(path)?;
        // Attribute-only access coexists with the exclusive data-file handle;
        // sharing all of its accesses does not weaken that existing handle.
        let named = metadata_handle(
            &path,
            false,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
        )?;
        if identity(original, false)? != identity(&named, false)? {
            return Err(Error::Refused("file database leaf identity changed".into()));
        }
        Ok(())
    }
    fn plain_leaf(&self, path: &Path, missing_ok: bool) -> std::io::Result<PathBuf> {
        let path = self.checked(path)?;
        match metadata_handle(
            &path,
            false,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
        ) {
            Ok(file) => {
                identity(&file, false)?;
            }
            Err(error) if missing_ok && error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => return Err(error),
        }
        Ok(path)
    }
    pub(super) fn create_new(&self, path: &Path, read: bool) -> std::io::Result<File> {
        let path = self.checked(path)?;
        OpenOptions::new()
            .write(true)
            .read(read)
            .create_new(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_DELETE)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path)
    }
    pub(super) fn hard_link(&self, source: &Path, target: &Path) -> std::io::Result<()> {
        fs::hard_link(self.plain_leaf(source, false)?, self.checked(target)?)
    }
    pub(super) fn rename(&self, source: &Path, target: &Path) -> std::io::Result<()> {
        fs::rename(
            self.plain_leaf(source, false)?,
            self.plain_leaf(target, true)?,
        )
    }
    pub(super) fn remove(&self, path: &Path) -> std::io::Result<()> {
        fs::remove_file(self.plain_leaf(path, false)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::write_paths::WritePaths;
    use std::io::Write;
    fn root() -> crate::test_scratch::TestScratch {
        let root = std::env::temp_dir().join(format!(
            "windows-filedb-custody-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        crate::test_scratch::TestScratch::adopt(root)
    }
    #[test]
    fn held_ancestors_and_exclusive_leaf_survive_both_publication_steps() {
        let root = root();
        let parent = root.join("outer/db");
        fs::create_dir_all(&parent).unwrap();
        let data = parent.join("data");
        let work = parent.join("work");
        let pending = parent.join("pending");
        let backup = parent.join("backup");
        fs::write(&data, b"original").unwrap();
        fs::write(&backup, b"old backup").unwrap();
        let paths = WritePaths::open(&data).unwrap();
        let original = paths.open_locked(&data).unwrap();
        assert!(OpenOptions::new().write(true).open(&data).is_err());
        assert!(fs::rename(&data, parent.join("moved-data")).is_err());
        let mut copy = paths.create_new(&work, true).unwrap();
        copy.write_all(b"published").unwrap();
        paths.require_binding(&data, &original).unwrap();
        paths.require_leaf(&work, &copy).unwrap();
        paths.hard_link(&data, &pending).unwrap();
        paths.require_leaf(&pending, &original).unwrap();
        assert!(paths.create_new(&pending, true).is_err());
        drop(original);
        crate::publish_files(&paths, &work, &data, &pending, &backup, |from, to| {
            assert!(
                fs::rename(root.join("outer"), root.join("rebound")).is_err(),
                "directory custody must remain held after final guard"
            );
            paths.rename(from, to)
        })
        .unwrap();
        assert_eq!(fs::read(&data).unwrap(), b"published");
        assert_eq!(fs::read(&backup).unwrap(), b"original");
        drop(copy);
        drop(paths);
        fs::rename(root.join("outer"), root.join("rebound")).unwrap();
    }
    #[test]
    fn equal_bytes_do_not_hide_a_rebound_leaf_or_an_outside_mutation() {
        let root = root();
        let parent = root.join("db");
        fs::create_dir(&parent).unwrap();
        let data = parent.join("data");
        fs::write(&data, b"same").unwrap();
        let paths = WritePaths::open(&data).unwrap();
        let held = File::open(&data).unwrap();
        paths.require_binding(&data, &held).unwrap();
        fs::rename(&data, parent.join("retained")).unwrap();
        fs::write(&data, b"same").unwrap();
        assert!(paths.require_leaf(&data, &held).is_err());
        let outside = root.join("outside");
        fs::write(&outside, b"untouched").unwrap();
        assert!(paths.create_new(&outside, true).is_err());
        assert!(paths.remove(&outside).is_err());
        assert!(paths.rename(&data, &outside).is_err());
        assert!(paths.hard_link(&outside, &parent.join("alias")).is_err());
        assert_eq!(fs::read(&outside).unwrap(), b"untouched");
        drop(held);
        drop(paths);
    }
    #[test]
    fn directory_junctions_are_rejected_without_following_their_target() {
        let root = root();
        let target = root.join("target");
        let alias = root.join("alias");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("data"), b"untouched").unwrap();
        // Directory junctions require no symlink privilege and exercise the
        // same FILE_ATTRIBUTE_REPARSE_POINT guard as symbolic links.
        let output = std::process::Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(&alias)
            .arg(&target)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "junction fixture failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(WritePaths::open(&alias.join("data")).is_err());
        assert_eq!(fs::read(target.join("data")).unwrap(), b"untouched");
        fs::remove_dir(&alias).unwrap();
    }
}
