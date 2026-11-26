//! The files and folders of a source tree, listed once, so that asking
//! whether one exists does not open it.
//!
//! The base-free stage asks that of every object's `Ext/Help.xml`,
//! `Ext/ObjectModule.bsl`, `Ext/Form.xml`, `Ext/Template.xml` and the like,
//! hundreds of thousands of times on ERP УХ, most of them for files that do
//! not exist. Each such probe is an `NtCreateFile`, and while other workers'
//! first reads keep the virus scanner busy, a probe waits its turn with them:
//! ERP УХ's stage spent 46 % of its sampled worker time inside the
//! `NtCreateFile` of `Path::exists` and `Path::is_file`, as much as in the
//! reads themselves.
//!
//! The stage lists its tree up front instead ([`walk`], every folder on a task
//! of the file-bound pool) and installs the list on each worker thread while
//! it prepares an object ([`install`]); [`exists`], [`is_file`] and [`is_dir`]
//! then answer from the list. Whatever the list cannot answer exactly is asked
//! of the file system, as before: no list on this thread, a path outside the
//! tree or not spelled as plain names, a name spelled in another case than on
//! disk. A tree the list cannot describe exactly (a folder that cannot be
//! read, a link, a name that is not Unicode, two names that differ only in
//! case) gets no list at all.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::marker::PhantomData;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use rayon::prelude::*;

use crate::parallel;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    File,
    Dir,
}

/// What a listing knows of a path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Answer {
    Present(Kind),
    Absent,
    /// The file system has to be asked.
    Unknown,
}

/// Every file and folder under a root, by relative path.
pub(crate) struct SourceListing {
    root: PathBuf,
    /// The path relative to the root, its names joined by `\`, as spelled on
    /// disk; the root itself is `""`.
    exact: HashMap<Box<str>, Kind>,
    /// The same paths lowercased: the file system ignores case, so a path
    /// found here but not in `exact` exists under another spelling.
    folded: HashSet<Box<str>>,
}

impl std::fmt::Debug for SourceListing {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SourceListing")
            .field("root", &self.root)
            .field("entries", &self.exact.len())
            .finish()
    }
}

/// A walk of a tree: its files and, when the tree allows one, its listing.
pub(crate) struct TreeWalk {
    /// Every regular file under the root, links not followed and unreadable
    /// folders skipped (as `WalkDir` does), in no particular order.
    pub files: Vec<PathBuf>,
    pub listing: Option<Arc<SourceListing>>,
}

#[derive(Default)]
struct WalkPart {
    files: Vec<PathBuf>,
    entries: Vec<(String, Kind)>,
    /// Nothing under the folder was skipped or left unnamed.
    complete: bool,
}

/// Lists `root` on the file-bound pool: one task per folder.
pub(crate) fn walk(root: &Path) -> TreeWalk {
    let part = parallel::install_io_bound(|| walk_folder(root, ""))
        .unwrap_or_else(|_| walk_folder(root, ""));
    let listing = if part.complete {
        SourceListing::from_entries(root, part.entries).map(Arc::new)
    } else {
        None
    };
    TreeWalk {
        files: part.files,
        listing,
    }
}

fn walk_folder(dir: &Path, key: &str) -> WalkPart {
    let Ok(entries) = fs::read_dir(dir) else {
        return WalkPart::default();
    };
    let mut part = WalkPart {
        complete: true,
        ..WalkPart::default()
    };
    let mut folders = Vec::new();
    for entry in entries {
        let Ok(entry) = entry else {
            part.complete = false;
            continue;
        };
        // Not followed: a link is neither a file nor a folder here, as it is
        // not to `WalkDir`; the file system would follow it, the list cannot.
        let Ok(kind) = entry.file_type() else {
            part.complete = false;
            continue;
        };
        let kind = if kind.is_dir() {
            Kind::Dir
        } else if kind.is_file() {
            Kind::File
        } else {
            part.complete = false;
            continue;
        };
        let name = entry.file_name();
        let child_key = match name.to_str() {
            Some(name) if key.is_empty() => name.to_string(),
            Some(name) => format!("{key}\\{name}"),
            None => {
                part.complete = false;
                String::new()
            }
        };
        match kind {
            Kind::Dir => folders.push((entry.path(), child_key.clone())),
            Kind::File => part.files.push(entry.path()),
        }
        if !child_key.is_empty() {
            part.entries.push((child_key, kind));
        }
    }
    let children = folders
        .par_iter()
        .map(|(path, key)| walk_folder(path, key))
        .collect::<Vec<_>>();
    for child in children {
        part.complete &= child.complete;
        part.files.extend(child.files);
        part.entries.extend(child.entries);
    }
    part
}

impl SourceListing {
    /// `None` when two entries differ only in case (a case-sensitive folder).
    fn from_entries(root: &Path, entries: Vec<(String, Kind)>) -> Option<Self> {
        let mut exact = HashMap::with_capacity(entries.len() + 1);
        let mut folded = HashSet::with_capacity(entries.len() + 1);
        exact.insert(Box::<str>::from(""), Kind::Dir);
        folded.insert(Box::<str>::from(""));
        for (key, kind) in entries {
            if !folded.insert(key.to_lowercase().into_boxed_str()) {
                return None;
            }
            exact.insert(key.into_boxed_str(), kind);
        }
        Some(Self {
            root: root.to_path_buf(),
            exact,
            folded,
        })
    }

    fn answer(&self, path: &Path) -> Answer {
        let Ok(relative) = path.strip_prefix(&self.root) else {
            return Answer::Unknown;
        };
        let mut key = String::with_capacity(relative.as_os_str().len());
        for component in relative.components() {
            let Component::Normal(name) = component else {
                return Answer::Unknown;
            };
            let Some(name) = name.to_str() else {
                return Answer::Unknown;
            };
            // Names the file system reads as another name: a trailing dot or
            // space it drops, a short 8.3 alias, a stream.
            if name.ends_with(['.', ' ']) || name.contains(['~', ':']) {
                return Answer::Unknown;
            }
            if !key.is_empty() {
                key.push('\\');
            }
            key.push_str(name);
        }
        if let Some(kind) = self.exact.get(key.as_str()) {
            return Answer::Present(*kind);
        }
        if self.folded.contains(key.to_lowercase().as_str()) {
            return Answer::Unknown;
        }
        Answer::Absent
    }
}

thread_local! {
    static CURRENT: RefCell<Option<Arc<SourceListing>>> = const { RefCell::new(None) };
}

/// The listing installed on this thread until dropped; the one before is
/// restored then.
pub(crate) struct Installed {
    previous: Option<Arc<SourceListing>>,
    // Restores this thread's listing: it must be dropped where it was made.
    _thread_bound: PhantomData<*const ()>,
}

impl Drop for Installed {
    fn drop(&mut self) {
        let previous = self.previous.take();
        let _ = CURRENT.try_with(|current| *current.borrow_mut() = previous);
    }
}

/// Answers this thread's probes from `listing` until the guard is dropped.
pub(crate) fn install(listing: Option<Arc<SourceListing>>) -> Installed {
    let previous = CURRENT
        .try_with(|current| current.replace(listing))
        .ok()
        .flatten();
    Installed {
        previous,
        _thread_bound: PhantomData,
    }
}

fn answer(path: &Path) -> Answer {
    CURRENT
        .try_with(|current| {
            current
                .borrow()
                .as_ref()
                .map(|listing| listing.answer(path))
        })
        .ok()
        .flatten()
        .unwrap_or(Answer::Unknown)
}

/// `path.exists()`, from this thread's listing when it knows.
pub(crate) fn exists(path: &Path) -> bool {
    match answer(path) {
        Answer::Present(_) => true,
        Answer::Absent => false,
        Answer::Unknown => path.exists(),
    }
}

/// `path.is_file()`, from this thread's listing when it knows.
pub(crate) fn is_file(path: &Path) -> bool {
    match answer(path) {
        Answer::Present(kind) => kind == Kind::File,
        Answer::Absent => false,
        Answer::Unknown => path.is_file(),
    }
}

/// `path.is_dir()`, from this thread's listing when it knows.
pub(crate) fn is_dir(path: &Path) -> bool {
    match answer(path) {
        Answer::Present(kind) => kind == Kind::Dir,
        Answer::Absent => false,
        Answer::Unknown => path.is_dir(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(files: &[&str]) -> crate::test_scratch::TestScratch {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-source-listing-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        for file in files {
            let path = root.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, b"x").unwrap();
        }
        crate::test_scratch::TestScratch::adopt(root)
    }

    #[test]
    fn a_listing_answers_every_probe_as_the_file_system_does() {
        let root = tree(&[
            "Configuration.xml",
            "Catalogs/A.xml",
            "Catalogs/A/Ext/ObjectModule.bsl",
            "Catalogs/A/Forms/F/Ext/Form.xml",
            "Languages/Русский.xml",
        ]);
        let walked = walk(&root);
        let listing = walked.listing.clone().expect("a plain tree is listed");
        assert_eq!(walked.files.len(), 5);
        let probes = [
            "",
            "Configuration.xml",
            "Catalogs",
            "Catalogs/A.xml",
            "Catalogs/A/Ext",
            "Catalogs/A/Ext/ObjectModule.bsl",
            "Catalogs/A/Ext/ManagerModule.bsl",
            "Catalogs/A/Ext/ObjectModule.bin",
            "Catalogs/A/Forms/F/Ext/Form.xml",
            "Catalogs/A/Forms/F/Ext/Form.xml/Nested",
            "Catalogs/B/Ext/Help.xml",
            "Languages/Русский.xml",
            // another spelling: the file system ignores case
            "catalogs/a/ext/objectmodule.bsl",
            "Languages/РУССКИЙ.xml",
            // not plain names
            "Catalogs/../Configuration.xml",
            "Catalogs/A.xml.",
        ];
        let _installed = install(Some(listing.clone()));
        for probe in probes {
            let path = root.join(probe);
            assert_eq!(exists(&path), path.exists(), "exists {probe}");
            assert_eq!(is_file(&path), path.is_file(), "is_file {probe}");
            assert_eq!(is_dir(&path), path.is_dir(), "is_dir {probe}");
        }
        assert_eq!(
            listing.answer(&root.join("Catalogs/A/Ext/ManagerModule.bsl")),
            Answer::Absent
        );
        assert_eq!(
            listing.answer(&root.join("catalogs/a.xml")),
            Answer::Unknown
        );
        assert_eq!(
            listing.answer(Path::new("C:/elsewhere/A.xml")),
            Answer::Unknown
        );
    }

    #[test]
    fn a_listing_is_only_answered_on_the_thread_it_is_installed_on() {
        let root = tree(&["Catalogs/A.xml"]);
        let listing = walk(&root).listing;
        let path = root.join("Catalogs/A.xml");
        {
            let _installed = install(listing);
            assert_eq!(answer(&path), Answer::Present(Kind::File));
            std::thread::scope(|scope| {
                scope.spawn(|| assert_eq!(answer(&path), Answer::Unknown));
            });
        }
        assert_eq!(answer(&path), Answer::Unknown);
    }
}
