//! Original regular-file custody for the source inventory, not a source model.
use super::{SourceChangeError, path_to_slash};
use sha2::{Digest, Sha256};
#[cfg(windows)]
use std::fs;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

fn io(error: impl std::fmt::Display) -> SourceChangeError {
    SourceChangeError::Io(error.to_string())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(super) struct FileIdentity(pub u64, pub u64);

#[derive(Clone, Debug, Eq, PartialEq)]
struct Stamp {
    identity: FileIdentity,
    length: u64,
    modified: SystemTime,
    #[cfg(unix)]
    changed: (i64, i64),
}

fn stamp(file: &File, regular: bool) -> Result<Stamp, SourceChangeError> {
    let metadata = file.metadata().map_err(io)?;
    if (regular && !metadata.is_file()) || (!regular && !metadata.is_dir()) {
        return Err(io(
            "source handle is not the required regular file/directory",
        ));
    }
    #[cfg(unix)]
    let (identity, links, changed) = {
        use std::os::unix::fs::MetadataExt;
        (
            FileIdentity(metadata.dev(), metadata.ino()),
            metadata.nlink(),
            (metadata.ctime(), metadata.ctime_nsec()),
        )
    };
    #[cfg(windows)]
    let (identity, links) = {
        use std::mem::MaybeUninit;
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::{
            BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_REPARSE_POINT, GetFileInformationByHandle,
        };
        let mut information = MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::zeroed();
        // SAFETY: file owns a live handle and information is writable storage.
        if unsafe { GetFileInformationByHandle(file.as_raw_handle(), information.as_mut_ptr()) }
            == 0
        {
            return Err(io(std::io::Error::last_os_error()));
        }
        // SAFETY: the successful call initialized the entire structure.
        let information = unsafe { information.assume_init() };
        if information.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(io("source handle is a reparse point"));
        }
        (
            FileIdentity(
                u64::from(information.dwVolumeSerialNumber),
                (u64::from(information.nFileIndexHigh) << 32)
                    | u64::from(information.nFileIndexLow),
            ),
            u64::from(information.nNumberOfLinks),
        )
    };
    #[cfg(not(any(unix, windows)))]
    return Err(io("original-file identity is unavailable on this host"));
    #[cfg(any(unix, windows))]
    {
        if regular && links != 1 {
            return Err(io("source hard-link alias is not admitted"));
        }
        Ok(Stamp {
            identity,
            length: metadata.len(),
            modified: metadata.modified().map_err(io)?,
            #[cfg(unix)]
            changed,
        })
    }
}

#[derive(Debug)]
pub(super) struct RootAnchor {
    pub(super) path: PathBuf,
    file: File,
    identity: FileIdentity,
    _ancestors: Vec<File>,
}

impl RootAnchor {
    pub(super) fn open(path: &Path) -> Result<Arc<Self>, SourceChangeError> {
        #[cfg(windows)]
        let (file, ancestors) = {
            let mut ancestors = Vec::new();
            for ancestor in path
                .ancestors()
                .skip(1)
                .filter(|p| !p.as_os_str().is_empty())
            {
                let parent = open_windows(ancestor, true)?;
                stamp(&parent, false)?;
                ancestors.try_reserve(1).map_err(io)?;
                ancestors.push(parent);
            }
            (open_windows(path, true)?, ancestors)
        };
        #[cfg(unix)]
        let (file, ancestors) = {
            use rustix::fs::{Mode, OFlags, open, openat};
            if !path.is_absolute() {
                return Err(io("source anchor must be absolute"));
            }
            let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
            let mut file = File::from(open("/", flags, Mode::empty()).map_err(io)?);
            let mut ancestors = Vec::new();
            for component in path.components() {
                match component {
                    Component::RootDir => (),
                    Component::Normal(name) => {
                        let next =
                            File::from(openat(&file, name, flags, Mode::empty()).map_err(io)?);
                        stamp(&next, false)?;
                        ancestors.try_reserve(1).map_err(io)?;
                        ancestors.push(file);
                        file = next;
                    }
                    _ => return Err(io("noncanonical absolute source anchor")),
                }
            }
            (file, ancestors)
        };
        #[cfg(not(any(unix, windows)))]
        return Err(io("anchored source reads are unavailable on this host"));
        #[cfg(any(unix, windows))]
        {
            let identity = stamp(&file, false)?.identity;
            Ok(Arc::new(Self {
                path: path.to_owned(),
                file,
                identity,
                _ancestors: ancestors,
            }))
        }
    }

    pub(super) fn require_path(&self) -> Result<(), SourceChangeError> {
        // This new handle checks the name only; it never replaces our original.
        let current = Self::open(&self.path)?;
        if current.identity != self.identity || stamp(&self.file, false)?.identity != self.identity
        {
            return Err(SourceChangeError::HeldRootChanged);
        }
        Ok(())
    }

    pub(super) fn open_file(
        self: &Arc<Self>,
        relative: &str,
    ) -> Result<OriginalFile, SourceChangeError> {
        let normalized = path_to_slash(Path::new(relative))?;
        if normalized != relative {
            return Err(io("original source spelling is not canonical"));
        }
        self.require_path()?;
        #[cfg(unix)]
        let (file, parents) = {
            use rustix::fs::{Mode, OFlags, openat};
            let mut parents = Vec::<File>::new();
            let mut components = Path::new(relative).components().peekable();
            while let Some(component) = components.next() {
                let Component::Normal(name) = component else {
                    return Err(io("invalid anchored source path"));
                };
                let directory = components.peek().is_some();
                let parent = parents.last().unwrap_or(&self.file);
                let mut flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
                if directory {
                    flags |= OFlags::DIRECTORY;
                } else {
                    flags |= OFlags::NONBLOCK;
                }
                let child = File::from(openat(parent, name, flags, Mode::empty()).map_err(io)?);
                stamp(&child, !directory)?;
                parents.try_reserve(1).map_err(io)?;
                parents.push(child);
            }
            let file = parents
                .pop()
                .ok_or_else(|| io("empty original file path"))?;
            (file, parents)
        };
        #[cfg(windows)]
        let (file, parents) = {
            let mut parents = Vec::new();
            let mut path = self.path.clone();
            let mut components = Path::new(relative).components().peekable();
            while let Some(component) = components.next() {
                let Component::Normal(name) = component else {
                    return Err(io("invalid anchored source path"));
                };
                path.push(name);
                if components.peek().is_some() {
                    let directory = open_windows(&path, true)?;
                    stamp(&directory, false)?;
                    parents.try_reserve(1).map_err(io)?;
                    parents.push(directory);
                }
            }
            let file = open_windows(&path, false)?;
            // Held ancestor directories exclude delete/rename, and this final
            // name is opened as the reparse object itself, never its target.
            self.require_path()?;
            (file, parents)
        };
        #[cfg(not(any(unix, windows)))]
        return Err(io("anchored source reads are unavailable on this host"));
        #[cfg(any(unix, windows))]
        {
            let original = stamp(&file, true)?;
            Ok(OriginalFile {
                root: self.clone(),
                relative: relative.to_owned(),
                file: Mutex::new(file),
                original,
                _parents: parents,
            })
        }
    }
}

#[cfg(windows)]
fn open_windows(path: &Path, directory: bool) -> Result<File, SourceChangeError> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ,
    };
    let mut options = fs::OpenOptions::new();
    if directory {
        options.access_mode(0);
    } else {
        options.read(true);
    }
    options
        .share_mode(FILE_SHARE_READ)
        .custom_flags(
            FILE_FLAG_OPEN_REPARSE_POINT
                | if directory {
                    FILE_FLAG_BACKUP_SEMANTICS
                } else {
                    0
                },
        )
        .open(path)
        .map_err(io)
}

#[derive(Debug)]
pub(super) struct OriginalFile {
    root: Arc<RootAnchor>,
    relative: String,
    file: Mutex<File>,
    original: Stamp,
    // Ancestors are retained while this original can be consumed.
    _parents: Vec<File>,
}

impl OriginalFile {
    pub(super) fn identity(&self) -> FileIdentity {
        self.original.identity
    }
    pub(super) fn length(&self) -> u64 {
        self.original.length
    }

    pub(super) fn read(
        &self,
        retain: bool,
    ) -> Result<([u8; 32], Option<Arc<Vec<u8>>>), SourceChangeError> {
        self.read_into(retain, None)
    }

    pub(super) fn extend_hash(
        &self,
        expected: &[u8; 32],
        hash: &mut Sha256,
    ) -> Result<(), SourceChangeError> {
        let (digest, _) = self.read_into(false, Some(hash))?;
        if &digest != expected {
            return Err(SourceChangeError::FileChangedDuringRead(
                self.relative.clone(),
            ));
        }
        Ok(())
    }

    fn read_into(
        &self,
        retain: bool,
        mut external_hash: Option<&mut Sha256>,
    ) -> Result<([u8; 32], Option<Arc<Vec<u8>>>), SourceChangeError> {
        self.root.require_path()?;
        let mut file = self.file.lock().map_err(io)?;
        if stamp(&file, true)? != self.original {
            return Err(SourceChangeError::FileChangedDuringRead(
                self.relative.clone(),
            ));
        }
        file.seek(SeekFrom::Start(0)).map_err(io)?;
        let mut bytes = if retain {
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(usize::try_from(self.original.length).map_err(io)?)
                .map_err(io)?;
            Some(bytes)
        } else {
            None
        };
        let mut total = 0_u64;
        let mut hash = Sha256::new();
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let count = file.read(&mut buffer).map_err(io)?;
            if count == 0 {
                break;
            }
            total = total
                .checked_add(count as u64)
                .ok_or_else(|| io("source extent overflow"))?;
            if total > self.original.length {
                return Err(SourceChangeError::FileChangedDuringRead(
                    self.relative.clone(),
                ));
            }
            hash.update(&buffer[..count]);
            if let Some(hash) = &mut external_hash {
                hash.update(&buffer[..count]);
            }
            if let Some(bytes) = &mut bytes {
                bytes.extend_from_slice(&buffer[..count]);
            }
        }
        if total != self.original.length || stamp(&file, true)? != self.original {
            return Err(SourceChangeError::FileChangedDuringRead(
                self.relative.clone(),
            ));
        }
        drop(file);
        self.require_name()?;
        Ok((hash.finalize().into(), bytes.map(Arc::new)))
    }

    fn require_name(&self) -> Result<(), SourceChangeError> {
        // Current name/ancestor inspection is separate from reading the held
        // original; a matching replacement's bytes cannot adopt its identity.
        let current = self.root.open_file(&self.relative)?;
        if current.original != self.original {
            return Err(SourceChangeError::FileChangedDuringRead(
                self.relative.clone(),
            ));
        }
        Ok(())
    }

    pub(super) fn require_digest(&self, expected: &[u8; 32]) -> Result<(), SourceChangeError> {
        let (digest, _) = self.read(false)?;
        if &digest != expected {
            return Err(SourceChangeError::FileChangedDuringRead(
                self.relative.clone(),
            ));
        }
        Ok(())
    }
}
