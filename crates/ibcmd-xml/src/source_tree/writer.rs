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
    publish_owned(tree, dest.as_ref(), None, |_| Ok(()))
}
/// Publish with a deliberate caller budget; normal publication is quota-free.
pub fn publish_new_with_limits(
    tree: &SourceTree,
    dest: impl AsRef<Path>,
    limits: ReaderLimits,
) -> Result<(), SourceTreeError> {
    publish_owned(tree, dest.as_ref(), Some(limits.validate()?), |_| Ok(()))
}
// The closure is an OS scheduling boundary only: production supplies no action;
// owner tests can mutate the physical stage before the SAME mandatory verifier.
fn publish_owned(
    tree: &SourceTree,
    dest: &Path,
    limits: Option<ReaderLimits>,
    before_verify: impl FnOnce(&Path) -> Result<(), SourceTreeError>,
) -> Result<(), SourceTreeError> {
    let closed = reader::ClosedOutput::prepare(tree, limits)?;
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
    before_verify(&temp)?;
    closed.verify(&temp)?;
    destination_absent(dest)?;
    rename_directory_new(&temp, dest)?;
    let mut guard = guard;
    guard.keep = true;
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
/// Atomically renames a caller-validated staged directory without replacement.
#[doc(hidden)]
pub fn rename_directory_new(source: &Path, dest: &Path) -> std::io::Result<()> {
    // Rustix maps NOREPLACE to Linux RENAME_NOREPLACE and macOS RENAME_EXCL.
    // An unavailable exclusive primitive returns an error; there is no
    // replacing-rename fallback after the caller's destination pre-check.
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

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
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
    fn exclusive_publication_moves_complete_directory_to_absent_destination() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-directory-publish-success-{}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        let staged = root.join("stage");
        let destination = root.join("output");
        fs::create_dir(&staged).unwrap();
        fs::create_dir(staged.join("nested")).unwrap();
        fs::write(staged.join("new.bin"), b"new payload").unwrap();
        fs::write(staged.join("nested/body.bin"), b"complete body").unwrap();
        rename_directory_new(&staged, &destination).unwrap();
        assert!(!staged.exists());
        assert_eq!(
            fs::read(destination.join("new.bin")).unwrap(),
            b"new payload"
        );
        assert_eq!(
            fs::read(destination.join("nested/body.bin")).unwrap(),
            b"complete body"
        );
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn exclusive_publication_preserves_existing_directory_and_file() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-directory-publish-collision-{}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        let staged = root.join("stage");
        let destination = root.join("output");
        fs::create_dir(&staged).unwrap();
        fs::write(staged.join("new.bin"), b"new payload").unwrap();
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("original.bin"), b"original payload").unwrap();
        assert!(rename_directory_new(&staged, &destination).is_err());
        assert_eq!(
            fs::read(destination.join("original.bin")).unwrap(),
            b"original payload"
        );
        assert!(!destination.join("new.bin").exists());
        let file = root.join("existing.bin");
        fs::write(&file, b"original file").unwrap();
        assert!(rename_directory_new(&staged, &file).is_err());
        assert_eq!(fs::read(&file).unwrap(), b"original file");
        assert_eq!(fs::read(staged.join("new.bin")).unwrap(), b"new payload");
        fs::remove_dir_all(&root).unwrap();
    }

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
    static NEXT_CLOSED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    fn closed_temp() -> Temp {
        let number = NEXT_CLOSED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "ibcmd-closed-source-{}-{number}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Temp { path, keep: false }
    }
    fn closed_tree() -> SourceTree {
        SourceTree::new(vec![
            SourceEntry::from_bytes(
                SourcePath::new("nested/Module.bsl").unwrap(),
                b"original".to_vec(),
            )
            .unwrap(),
        ])
        .unwrap()
    }
    #[test]
    fn closed_publisher_rejects_each_unmodelled_operational_tree_before_rename() {
        for name in [".git", "target", ".idea", ".vscode"] {
            let temp = closed_temp();
            let dest = temp.path.join("out");
            let result = publish_owned(&closed_tree(), &dest, None, |stage| {
                fs::create_dir(stage.join(name))?;
                fs::write(stage.join(name).join("foreign.bin"), b"extra")?;
                Ok(())
            });
            assert!(result.is_err(), "{name}");
            assert!(!dest.exists());
            assert!(fs::read_dir(&temp.path).unwrap().next().is_none());
        }
    }
    #[test]
    fn closed_publisher_rejects_extra_regular_file_and_empty_orphans() {
        for name in ["extra.bin", "orphan", "nested/empty"] {
            let temp = closed_temp();
            let dest = temp.path.join("out");
            assert!(
                publish_owned(&closed_tree(), &dest, None, |stage| {
                    if name.ends_with(".bin") {
                        fs::write(stage.join(name), b"extra")?;
                    } else {
                        fs::create_dir(stage.join(name))?;
                    }
                    Ok(())
                })
                .is_err(),
                "{name}"
            );
            assert!(!dest.exists());
        }
    }
    #[test]
    fn closed_publisher_rejects_current_file_drift_disappearance_and_type_substitution() {
        for action in 0..3 {
            let temp = closed_temp();
            let dest = temp.path.join("out");
            assert!(
                publish_owned(&closed_tree(), &dest, None, |stage| {
                    let file = stage.join("nested/Module.bsl");
                    match action {
                        0 => fs::write(file, b"modified")?,
                        1 => fs::remove_file(file)?,
                        _ => {
                            fs::remove_file(&file)?;
                            fs::create_dir(file)?;
                        }
                    }
                    Ok(())
                })
                .is_err()
            );
            assert!(!dest.exists());
        }
    }
    #[test]
    fn closed_publisher_accepts_expected_parent_resource_and_refuses_its_extra_sibling() {
        let payload = b"<?xml opaque invalid\xff";
        let tree = SourceTree::new(vec![
            SourceEntry::from_bytes(
                SourcePath::new("Ext/ParentConfigurations/target/payload.bin").unwrap(),
                payload.to_vec(),
            )
            .unwrap(),
        ])
        .unwrap();
        let temp = closed_temp();
        let dest = temp.path.join("out");
        publish_new(&tree, &dest).unwrap();
        assert_eq!(read_source_tree(&dest).unwrap(), tree);
        assert_eq!(
            fs::read(dest.join("Ext/ParentConfigurations/target/payload.bin")).unwrap(),
            payload
        );
        let other = temp.path.join("refused");
        assert!(
            publish_owned(&tree, &other, None, |stage| {
                fs::write(
                    stage.join("Ext/ParentConfigurations/target/extra.bin"),
                    b"unowned",
                )?;
                Ok(())
            })
            .is_err()
        );
        assert!(!other.exists());
    }
    #[test]
    fn closed_verifier_directly_refuses_tree_alias_and_empty_directory_without_publication() {
        let temp = closed_temp();
        fs::create_dir(temp.path.join("nested")).unwrap();
        fs::write(temp.path.join("nested/Module.bsl"), b"original").unwrap();
        reader::ClosedOutput::prepare(&closed_tree(), None)
            .unwrap()
            .verify(&temp.path)
            .unwrap();
        fs::create_dir(temp.path.join("nested/orphan")).unwrap();
        assert!(
            reader::ClosedOutput::prepare(&closed_tree(), None)
                .unwrap()
                .verify(&temp.path)
                .is_err()
        );
    }
    #[test]
    fn closed_empty_root_success_and_early_destination_sentinel_are_distinct_gates() {
        let temp = closed_temp();
        let tree = SourceTree::new(vec![]).unwrap();
        let dest = temp.path.join("empty");
        publish_new(&tree, &dest).unwrap();
        assert_eq!(read_source_tree(&dest).unwrap(), tree);
        fs::write(dest.join("sentinel.bin"), b"original sentinel").unwrap();
        let mut called = false;
        assert!(matches!(
            publish_owned(&closed_tree(), &dest, None, |_| {
                called = true;
                Ok(())
            }),
            Err(SourceTreeError::ExistingDestination)
        ));
        assert!(!called); // The existing destination refused before the verifier scheduling seam.
        assert_eq!(
            fs::read(dest.join("sentinel.bin")).unwrap(),
            b"original sentinel"
        );
    }
}
