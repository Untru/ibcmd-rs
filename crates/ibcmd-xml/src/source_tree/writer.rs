use super::*;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
pub struct SourceTreeWriter;
impl SourceTreeWriter {
    pub fn publish_new(tree: &SourceTree, dest: impl AsRef<Path>) -> Result<(), SourceTreeError> {
        publish_new(tree, dest)
    }
}
pub fn publish_new(tree: &SourceTree, dest: impl AsRef<Path>) -> Result<(), SourceTreeError> {
    publish_new_with_limits(tree, dest, ReaderLimits::default())
}
/// Publish with explicit bounded inventory limits, including complete ERP trees.
pub fn publish_new_with_limits(
    tree: &SourceTree,
    dest: impl AsRef<Path>,
    limits: ReaderLimits,
) -> Result<(), SourceTreeError> {
    let limits = limits.validate()?;
    tree.validate()?;
    if tree.entries().len() > limits.files {
        return Err(SourceTreeError::TooManyFiles);
    }
    count_directories(tree.entries(), limits.directories)?;
    let mut total = 0usize;
    for entry in tree.entries() {
        if entry.bytes().len() > limits.asset_bytes {
            return Err(SourceTreeError::AssetTooLarge {
                path: entry.path().clone(),
                actual: entry.bytes().len(),
            });
        }
        total = total
            .checked_add(entry.bytes().len())
            .filter(|value| *value <= limits.total_bytes)
            .ok_or(SourceTreeError::TotalTooLarge)?;
    }
    let dest = dest.as_ref();
    destination_absent(dest)?;
    let parent = dest
        .parent()
        .ok_or_else(|| SourceTreeError::UnsafePath("destination without parent".into()))?;
    let stem = dest.file_name().and_then(|x| x.to_str()).unwrap_or("tree");
    let mut temp = None;
    for number in 0_u32..1024 {
        let candidate = parent.join(format!(".{stem}.ibcmd-new-{number}"));
        match fs::create_dir(&candidate) {
            Ok(()) => {
                temp = Some(candidate);
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e.into()),
        }
    }
    let temp = temp.ok_or(SourceTreeError::TemporaryNameExhausted)?;
    let guard = Temp {
        path: temp.clone(),
        keep: false,
    };
    for e in tree.entries() {
        let p = temp.join(e.path().as_str());
        if let Some(x) = p.parent() {
            fs::create_dir_all(x)?
        }
        let mut f = File::create(p)?;
        f.write_all(e.bytes())?;
        f.sync_all()?;
    }
    reader::verify_with_limits(&temp, tree, limits)?;
    destination_absent(dest)?;
    rename_directory_new(&temp, dest)?;
    let mut guard = guard;
    guard.keep = true;
    Ok(())
}

#[cfg(target_os = "linux")]
/// Atomically renames a caller-validated staged directory without replacement.
#[doc(hidden)]
pub fn rename_directory_new(source: &Path, dest: &Path) -> std::io::Result<()> {
    rustix::fs::renameat_with(
        rustix::fs::CWD,
        source,
        rustix::fs::CWD,
        dest,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .map_err(std::io::Error::from)
}

#[cfg(windows)]
/// Atomically renames a caller-validated staged directory without replacement.
#[doc(hidden)]
pub fn rename_directory_new(source: &Path, dest: &Path) -> std::io::Result<()> {
    // std::fs::rename can replace an empty directory on Windows. Use the
    // safe wrapper around MoveFileExW with no replacement flag instead.
    renamore::rename_exclusive(source, dest)
}

#[cfg(not(any(windows, target_os = "linux")))]
/// Refuses atomic publication on systems without an exclusive rename primitive.
#[doc(hidden)]
pub fn rename_directory_new(_source: &Path, _dest: &Path) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "atomic directory publication without replacement is not supported on this OS",
    ))
}

fn destination_absent(path: &Path) -> Result<(), SourceTreeError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Err(SourceTreeError::ExistingDestination),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}
struct Temp {
    path: std::path::PathBuf,
    keep: bool,
}
impl Drop for Temp {
    fn drop(&mut self) {
        if !self.keep {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn final_publication_never_replaces_newly_created_empty_directory() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-directory-publish-race-{}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        let staged = root.join("stage");
        let destination = root.join("output");
        fs::create_dir(&staged).unwrap();
        fs::write(staged.join("new.bin"), b"new payload").unwrap();
        destination_absent(&destination).unwrap();
        // Another writer creates an empty destination after the pre-check.
        fs::create_dir(&destination).unwrap();
        assert!(rename_directory_new(&staged, &destination).is_err());
        assert!(staged.join("new.bin").exists());
        assert!(!destination.join("new.bin").exists());
        fs::remove_dir_all(&root).unwrap();
    }
}
