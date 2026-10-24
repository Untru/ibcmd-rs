//! Mutation paths anchored to one held directory, independent of ancestor renames.
#[cfg(windows)]
mod windows;
#[cfg(any(unix, windows))]
use crate::lock;
use crate::{Error, Result};
#[cfg(not(windows))]
use std::fs;
use std::{
    fs::File,
    path::{Path, PathBuf},
};

#[cfg(unix)]
use std::fs::OpenOptions;

pub(crate) struct WritePaths {
    parent: PathBuf,
    #[cfg(unix)]
    directory: File,
    #[cfg(windows)]
    custody: windows::Custody,
}
impl WritePaths {
    pub(crate) fn open(path: &Path) -> std::io::Result<Self> {
        let parent = parent(path).to_path_buf();
        #[cfg(unix)]
        let directory = {
            use std::os::unix::fs::OpenOptionsExt;
            OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_DIRECTORY | libc::O_CLOEXEC)
                .open(&parent)?
        };
        #[cfg(windows)]
        let custody = windows::Custody::open(&parent)?;
        Ok(Self {
            parent,
            #[cfg(unix)]
            directory,
            #[cfg(windows)]
            custody,
        })
    }
    pub(crate) fn open_locked(&self, path: &Path) -> Result<File> {
        #[cfg(unix)]
        {
            let file = self.open_at(path, libc::O_RDWR)?;
            lock::lock_write_descriptor(&file)?;
            self.require_binding(path, &file)?;
            Ok(file)
        }
        #[cfg(windows)]
        {
            let path = self.custody.checked(path)?;
            let file = lock::open_locked(&path, lock::Mode::Write)?;
            self.custody.require_leaf(&path, &file)?;
            Ok(file)
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = path;
            Err(Error::Refused(
                "descriptor-relative file database writes are not supported on this OS".into(),
            ))
        }
    }
    pub(crate) fn create_new(&self, path: &Path, read: bool) -> std::io::Result<File> {
        #[cfg(unix)]
        {
            self.open_at(
                path,
                (if read { libc::O_RDWR } else { libc::O_WRONLY }) | libc::O_CREAT | libc::O_EXCL,
            )
        }
        #[cfg(windows)]
        {
            self.custody.create_new(path, read)
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = (path, read);
            Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "descriptor-relative writes unsupported",
            ))
        }
    }
    pub(crate) fn require_binding(&self, path: &Path, file: &File) -> Result<()> {
        if parent(path) != self.parent {
            return Err(Error::Refused(
                "database outside held parent directory".into(),
            ));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let held = self.directory.metadata()?;
            let named = fs::metadata(&self.parent)?;
            if (held.dev(), held.ino()) != (named.dev(), named.ino()) {
                return Err(Error::Refused(
                    "file database parent directory was rebound".into(),
                ));
            }
        }
        self.require_leaf(path, file)
    }
    // stat device types differ between macOS and Linux.
    #[allow(clippy::unnecessary_cast)]
    pub(crate) fn require_leaf(&self, path: &Path, file: &File) -> Result<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let current = self.stat_at(path)?;
            let original = file.metadata()?;
            if current.st_mode & libc::S_IFMT != libc::S_IFREG
                || (current.st_dev as u64, current.st_ino as u64)
                    != (original.dev(), original.ino())
            {
                return Err(Error::Refused("file database leaf identity changed".into()));
            }
        }
        #[cfg(windows)]
        {
            self.custody.require_leaf(path, file)?;
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = file;
            let current = fs::symlink_metadata(path)?;
            if !current.is_file() || current.file_type().is_symlink() {
                return Err(Error::Refused("file database path was rebound".into()));
            }
        }
        Ok(())
    }
    #[cfg(windows)]
    pub(crate) fn snapshot_identity(&self, path: &Path, file: &File) -> Result<[u8; 12]> {
        self.custody.snapshot_identity(path, file)
    }

    #[allow(unsafe_code)]
    pub(crate) fn hard_link(&self, source: &Path, target: &Path) -> std::io::Result<()> {
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            let source = self.leaf(source)?;
            let target = self.leaf(target)?;
            // SAFETY: held directory fd and NUL-terminated single components
            // remain valid throughout linkat. No ancestor pathname is resolved.
            self.rc(unsafe {
                libc::linkat(
                    self.directory.as_raw_fd(),
                    source.as_ptr(),
                    self.directory.as_raw_fd(),
                    target.as_ptr(),
                    0,
                )
            })
        }
        #[cfg(windows)]
        {
            self.custody.hard_link(source, target)
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = (source, target);
            Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "descriptor-relative writes unsupported",
            ))
        }
    }
    #[allow(unsafe_code)]
    pub(crate) fn rename(&self, source: &Path, target: &Path) -> std::io::Result<()> {
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            let source = self.leaf(source)?;
            let target = self.leaf(target)?;
            // SAFETY: both names are checked single components in our held fd.
            self.rc(unsafe {
                libc::renameat(
                    self.directory.as_raw_fd(),
                    source.as_ptr(),
                    self.directory.as_raw_fd(),
                    target.as_ptr(),
                )
            })
        }
        #[cfg(windows)]
        {
            self.custody.rename(source, target)
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = (source, target);
            Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "descriptor-relative writes unsupported",
            ))
        }
    }
    #[allow(unsafe_code)]
    pub(crate) fn remove(&self, path: &Path) -> std::io::Result<()> {
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            let leaf = self.leaf(path)?;
            // SAFETY: single-component leaf in a live held directory.
            self.rc(unsafe { libc::unlinkat(self.directory.as_raw_fd(), leaf.as_ptr(), 0) })
        }
        #[cfg(windows)]
        {
            self.custody.remove(path)
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = path;
            Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "descriptor-relative writes unsupported",
            ))
        }
    }
    pub(crate) fn sync(&self) {
        #[cfg(unix)]
        {
            let _ = self.directory.sync_all();
        }
    }
    #[cfg(unix)]
    fn leaf(&self, path: &Path) -> std::io::Result<std::ffi::CString> {
        use std::os::unix::ffi::OsStrExt;
        if parent(path) != self.parent {
            return Err(std::io::Error::other(
                "mutation outside held database directory",
            ));
        }
        let name = path
            .file_name()
            .ok_or_else(|| std::io::Error::other("missing mutation leaf"))?;
        std::ffi::CString::new(name.as_bytes())
            .map_err(|_| std::io::Error::other("NUL mutation leaf"))
    }
    #[cfg(unix)]
    fn rc(&self, rc: libc::c_int) -> std::io::Result<()> {
        if rc == -1 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
    #[cfg(unix)]
    #[allow(unsafe_code)]
    fn open_at(&self, path: &Path, flags: libc::c_int) -> std::io::Result<File> {
        use std::os::fd::{AsRawFd, FromRawFd};
        let leaf = self.leaf(path)?;
        // SAFETY: valid directory fd and C string, 0600 mode supplied for O_CREAT.
        let fd = unsafe {
            libc::openat(
                self.directory.as_raw_fd(),
                leaf.as_ptr(),
                flags | libc::O_CLOEXEC | libc::O_NOFOLLOW,
                0o600,
            )
        };
        if fd == -1 {
            return Err(std::io::Error::last_os_error());
        }
        // SAFETY: openat returned a newly owned descriptor, consumed exactly once.
        Ok(unsafe { File::from_raw_fd(fd) })
    }
    #[cfg(unix)]
    #[allow(unsafe_code)]
    fn stat_at(&self, path: &Path) -> std::io::Result<libc::stat> {
        use std::os::fd::AsRawFd;
        let leaf = self.leaf(path)?;
        let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
        // SAFETY: output points to properly sized writable stat storage; read
        // only after fstatat succeeded. The leaf symlink is never followed.
        self.rc(unsafe {
            libc::fstatat(
                self.directory.as_raw_fd(),
                leaf.as_ptr(),
                stat.as_mut_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
            )
        })?;
        Ok(unsafe { stat.assume_init() })
    }
}
fn parent(path: &Path) -> &Path {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    #[test]
    fn ancestor_rebind_after_final_check_cannot_redirect_publication() {
        let root = std::env::temp_dir().join(format!(
            "filedb-dirfd-publish-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let parent = root.join("outer/db");
        fs::create_dir_all(&parent).unwrap();
        let data = parent.join("data");
        let work = parent.join("work");
        let backup = parent.join("backup");
        let pending = parent.join("pending");
        for (name, bytes) in [
            (&data, b"A"),
            (&work, b"C"),
            (&backup, b"B"),
            (&pending, b"A"),
        ] {
            fs::write(name, bytes).unwrap();
        }
        let paths = WritePaths::open(&data).unwrap();
        let original = paths.open_locked(&data).unwrap();
        paths.require_binding(&data, &original).unwrap();
        let mut rebound = false;
        crate::publish_files(&paths, &work, &data, &pending, &backup, |from, to| {
            // Fault exactly AFTER the final check, before the first rename.
            if !rebound {
                fs::rename(root.join("outer"), root.join("retained")).unwrap();
                fs::create_dir_all(&parent).unwrap();
                for name in [&data, &work, &backup, &pending] {
                    fs::write(name, b"unrelated").unwrap();
                }
                rebound = true;
            }
            paths.rename(from, to)
        })
        .unwrap();
        for name in [&data, &work, &backup, &pending] {
            assert_eq!(fs::read(name).unwrap(), b"unrelated");
        }
        assert_eq!(fs::read(root.join("retained/db/data")).unwrap(), b"C");
        assert_eq!(fs::read(root.join("retained/db/backup")).unwrap(), b"A");
        assert!(paths.require_binding(&data, &original).is_err());
        drop(original);
        drop(paths);
        if std::env::var_os("IBCMD_TEST_KEEP_FIXTURES").is_none() {
            fs::remove_dir_all(root).unwrap();
        }
    }
    #[test]
    fn ancestor_rebind_cannot_redirect_copy_creation_or_error_cleanup() {
        let root = std::env::temp_dir().join(format!(
            "filedb-dirfd-copy-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let parent = root.join("db");
        fs::create_dir_all(&parent).unwrap();
        let data = parent.join("data");
        let pending = parent.join("pending");
        fs::write(&data, b"A").unwrap();
        let paths = WritePaths::open(&data).unwrap();
        let original = paths.open_locked(&data).unwrap();
        let error = crate::prepare_backup_file(
            &paths,
            &original,
            &pending,
            || Err(std::io::Error::other("link unavailable")),
            |out| {
                use std::io::Write;
                out.write_all(b"partial")?;
                fs::rename(&parent, root.join("retained")).unwrap();
                fs::create_dir(&parent).unwrap();
                fs::write(&pending, b"unrelated recovery").unwrap();
                Err(Error::Refused("cancel copy".into()))
            },
        )
        .unwrap_err();
        assert!(error.to_string().contains("cancel copy"));
        assert!(!root.join("retained/pending").exists());
        assert_eq!(fs::read(&pending).unwrap(), b"unrelated recovery");
        let work = parent.join("work");
        let _created = paths.create_new(&work, true).unwrap();
        assert!(!work.exists());
        assert!(root.join("retained/work").exists());
        drop(original);
        drop(paths);
        if std::env::var_os("IBCMD_TEST_KEEP_FIXTURES").is_none() {
            fs::remove_dir_all(root).unwrap();
        }
    }
    #[test]
    fn leaf_symlinks_are_never_followed() {
        use std::os::unix::fs::symlink;
        let root = std::env::temp_dir().join(format!(
            "filedb-dirfd-nofollow-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let outside = root.join("outside");
        fs::write(&outside, b"untouched").unwrap();
        let target = root.join("data");
        symlink(&outside, &target).unwrap();
        let paths = WritePaths::open(&target).unwrap();
        assert!(paths.open_locked(&target).is_err());
        assert!(paths.create_new(&target, true).is_err());
        assert_eq!(fs::read(&outside).unwrap(), b"untouched");
        drop(paths);
        if std::env::var_os("IBCMD_TEST_KEEP_FIXTURES").is_none() {
            fs::remove_dir_all(root).unwrap();
        }
    }
}
