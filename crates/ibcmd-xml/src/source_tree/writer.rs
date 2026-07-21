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
    tree.validate()?;
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
    let reread = read_source_tree(&temp)?;
    if &reread != tree {
        return Err(SourceTreeError::PathConflict {
            first: SourcePath::new("staging")?,
            second: SourcePath::new("tree")?,
        });
    }
    destination_absent(dest)?;
    fs::rename(&temp, dest)?;
    let mut guard = guard;
    guard.keep = true;
    Ok(())
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
