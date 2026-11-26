//! The tree index `cf export --index` writes (`.ibcmd/index.tsv`): the
//! exported file's digest, the dialect, when it was written, the file's own
//! record of each entry's version (`versions` / `configinfo`), and every
//! exported file's digest and storage key. A load onto the same file diffs
//! the tree against it instead of exporting the base again; an update to a
//! newer file (`crate::update`) compares the versions.
//!
//! Change detection as git's index does it: each file's size and mtime are
//! kept from when it was hashed, and a file is skipped only while both are
//! exactly as kept and the mtime is older than the index's write time less a
//! two-second margin (an edit in the same mtime tick as the export). A file
//! given back an old date (Explorer, robocopy, `cp -p`, an unpacked archive)
//! has another mtime than the kept one and is hashed.
//!
//! ```text
//! # ibcmd-rs tree index 2
//! base<TAB><sha256 of the exported file>
//! dialect<TAB>2.20
//! written<TAB><unix time, ns>
//! version<TAB><entry><TAB><configVersion>                            (one per versioned entry)
//! <sha256><TAB><storage key or -><TAB><size>.<mtime ns><TAB><path>   (one per file)
//! ```
//!
//! `cf export --update` marks the tree (`update-in-progress`) while it
//! rewrites it; a tree still marked mixes two files and is refused.

use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

use super::{TreeDiff, keys_by_output, relative_files};
use crate::{
    legacy_version::InfobaseConfigSourceVersion,
    mssql_dump::{StorageImageSourceExportReport, stored_config_versions},
};

pub const INDEX_DIR: &str = ".ibcmd";
const INDEX_FILE: &str = "index.tsv";
const HEADER: &str = "# ibcmd-rs tree index 2";
const UPDATE_MARK: &str = "update-in-progress";
/// Files written this close before the index was are hashed anyway: a
/// filesystem's mtime granularity can put an edit made right after the
/// export at the same tick.
const MARGIN_NS: u128 = 2_000_000_000;

pub struct TreeIndex {
    pub base: String,
    pub dialect: String,
    written_ns: u128,
    /// Entry → configVersion, as the indexed file records them.
    pub versions: BTreeMap<String, String>,
    files: BTreeMap<String, IndexedFile>,
}

/// One exported file as the index keeps it.
struct IndexedFile {
    digest: String,
    key: Option<String>,
    /// `<size>.<mtime ns>` when the digest was taken.
    stamp: String,
}

impl TreeIndex {
    /// Output path → storage key, as `keys_by_output` gives it.
    pub fn keys(&self) -> BTreeMap<String, String> {
        self.files
            .iter()
            .filter_map(|(path, file)| file.key.clone().map(|key| (path.clone(), key)))
            .collect()
    }

    /// The exported files' paths.
    pub fn paths(&self) -> impl Iterator<Item = &String> {
        self.files.keys()
    }

    /// Path → (digest, stamp).
    fn digests(&self) -> BTreeMap<String, (String, String)> {
        self.files
            .iter()
            .map(|(path, file)| (path.clone(), (file.digest.clone(), file.stamp.clone())))
            .collect()
    }
}

/// Whether an update of `tree` was begun and never finished.
pub fn update_interrupted(tree: &Path) -> bool {
    tree.join(INDEX_DIR).join(UPDATE_MARK).exists()
}

/// Marks `tree` as being rewritten, before the first file changes.
pub fn begin_update(tree: &Path) -> Result<()> {
    let dir = tree.join(INDEX_DIR);
    fs::create_dir_all(&dir).with_context(|| format!("failed to create {}", dir.display()))?;
    fs::write(dir.join(UPDATE_MARK), "").context("failed to mark the tree as being updated")
}

/// Clears the mark of `begin_update`, once the index names the new file.
pub fn finish_update(tree: &Path) -> Result<()> {
    fs::remove_file(tree.join(INDEX_DIR).join(UPDATE_MARK))
        .context("failed to clear the update mark")
}

pub fn index_path(tree: &Path) -> PathBuf {
    tree.join(INDEX_DIR).join(INDEX_FILE)
}

/// SHA-256 of a file, streamed (a configuration runs to gigabytes).
pub fn file_sha256(path: &Path) -> Result<String> {
    let mut file =
        fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 20];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex(&hasher.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn now_ns() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default()
}

/// `(<size>.<mtime ns>, mtime ns)` of a file.
fn stamp(path: &Path) -> Result<(String, u128)> {
    let metadata =
        fs::metadata(path).with_context(|| format!("failed to read {}", path.display()))?;
    let mtime = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|elapsed| elapsed.as_nanos())
        .with_context(|| format!("no modification time for {}", path.display()))?;
    Ok((format!("{}.{mtime}", metadata.len()), mtime))
}

/// A file's digest and the stamp it was taken at (the stamp first: an edit
/// racing the hash leaves a stale stamp, which the next diff hashes again).
fn hashed(path: &Path) -> Result<(String, String)> {
    let (stamp, _) = stamp(path)?;
    Ok((file_sha256(path)?, stamp))
}

/// The versions `file` records (empty when it records none or is unreadable).
pub fn file_versions(file: &Path) -> BTreeMap<String, String> {
    let Ok(metadata) = fs::metadata(file) else {
        return BTreeMap::new();
    };
    let Ok(source) = fs::File::open(file) else {
        return BTreeMap::new();
    };
    let limits = ibcmd_core::limits::ResourceLimits::for_input_bytes(metadata.len());
    let profile = ibcmd_core::artifact::StorageProfileId::parse("storage:cf-cli")
        .expect("static profile id is valid");
    let Ok(archive) = ibcmd_cf::archive::decode_packed_archive(source, limits, profile) else {
        return BTreeMap::new();
    };
    archive_versions(&archive)
}

/// The versions an open archive records; only the version entries' payloads
/// are copied.
pub fn archive_versions(archive: &ibcmd_cf::archive::PackedCfArchive) -> BTreeMap<String, String> {
    let entries = archive
        .entries()
        .iter()
        .map(|entry| {
            let name = entry.name().to_owned();
            let payload = match name.as_str() {
                "versions" | "configinfo" => entry.payload().to_vec(),
                _ => Vec::new(),
            };
            (name, payload)
        })
        .collect::<Vec<_>>();
    stored_config_versions(&entries).unwrap_or_default()
}

/// Writes the index of `tree`, the export of `base` described by `report`;
/// `versions` are the ones `base` records (`archive_versions`).
pub fn write_tree_index(
    tree: &Path,
    base: &Path,
    report: &StorageImageSourceExportReport,
    dialect: InfobaseConfigSourceVersion,
    versions: &BTreeMap<String, String>,
) -> Result<()> {
    use rayon::prelude::*;
    let keys = keys_by_output(report);
    let digests = relative_files(tree)?
        .into_par_iter()
        .map(|path| hashed(&tree.join(&path)).map(|digest| (path, digest)))
        .collect::<Result<BTreeMap<_, _>>>()?;
    write_index_text(
        tree,
        &file_sha256(base)?,
        dialect.as_str(),
        now_ns(),
        versions,
        &digests,
        &keys,
    )
}

/// Rewrites the index of `tree` for a new base (a load's output): the files
/// it loaded (`applied`) hashed again, every other digest as it was --
/// nothing else differs between the tree and the new file. The keys are
/// `fresh_keys` (the new file's export) when given: a compiled load gives
/// added files entries the old index cannot name.
pub fn refresh_tree_index(
    tree: &Path,
    base: &Path,
    index: &TreeIndex,
    applied: &[String],
    removed: &[String],
    fresh_keys: Option<&BTreeMap<String, String>>,
) -> Result<()> {
    let written = now_ns();
    let mut digests = index.digests();
    let mut keys = fresh_keys.cloned().unwrap_or_else(|| index.keys());
    for path in removed {
        digests.remove(path);
        keys.remove(path);
    }
    for path in applied {
        digests.insert(path.clone(), hashed(&tree.join(path))?);
    }
    write_index_text(
        tree,
        &file_sha256(base)?,
        &index.dialect,
        written,
        &file_versions(base),
        &digests,
        &keys,
    )
}

/// Rewrites the index of `tree` after an incremental export of `base`
/// (whose versions are `versions`): the files it rewrote hashed again under
/// their keys, the files it removed gone, everything else as it was.
pub fn update_tree_index(
    tree: &Path,
    base: &Path,
    index: &TreeIndex,
    versions: &BTreeMap<String, String>,
    rewritten: &[(String, String)],
    removed: &[String],
) -> Result<()> {
    let written = now_ns();
    let mut digests = index.digests();
    let mut keys = index.keys();
    for path in removed {
        digests.remove(path);
        keys.remove(path);
    }
    for (path, key) in rewritten {
        if !tree.join(path).exists() {
            continue;
        }
        digests.insert(path.clone(), hashed(&tree.join(path))?);
        if !key.is_empty() {
            keys.insert(path.clone(), key.clone());
        }
    }
    write_index_text(
        tree,
        &file_sha256(base)?,
        &index.dialect,
        written,
        versions,
        &digests,
        &keys,
    )
}

fn write_index_text(
    tree: &Path,
    base_sha256: &str,
    dialect: &str,
    written: u128,
    versions: &BTreeMap<String, String>,
    digests: &BTreeMap<String, (String, String)>,
    keys: &BTreeMap<String, String>,
) -> Result<()> {
    let mut text =
        format!("{HEADER}\nbase\t{base_sha256}\ndialect\t{dialect}\nwritten\t{written}\n");
    for (entry, version) in versions {
        text.push_str(&format!("version\t{entry}\t{version}\n"));
    }
    for (path, (digest, stamp)) in digests {
        let key = keys.get(path).map_or("-", String::as_str);
        text.push_str(&format!("{digest}\t{key}\t{stamp}\t{path}\n"));
    }
    let dir = tree.join(INDEX_DIR);
    fs::create_dir_all(&dir).with_context(|| format!("failed to create {}", dir.display()))?;
    // Written aside and renamed over: the index is the old one or the new one.
    let staged = dir.join(format!("{INDEX_FILE}.new"));
    fs::write(&staged, text).context("failed to write the tree index")?;
    fs::rename(&staged, dir.join(INDEX_FILE)).context("failed to replace the tree index")
}

/// The index of `tree`, when there is one this version reads.
pub fn read_tree_index(tree: &Path) -> Option<TreeIndex> {
    let text = fs::read_to_string(index_path(tree)).ok()?;
    let mut lines = text.lines();
    if lines.next()? != HEADER {
        return None;
    }
    let mut field = |name: &str| {
        lines
            .next()
            .and_then(|line| line.strip_prefix(&format!("{name}\t")))
            .map(str::to_owned)
    };
    let base = field("base")?;
    let dialect = field("dialect")?;
    let written_ns = field("written")?.parse().ok()?;
    let mut versions = BTreeMap::new();
    let mut files = BTreeMap::new();
    for line in lines {
        if let Some(rest) = line.strip_prefix("version\t") {
            let (entry, version) = rest.split_once('\t')?;
            versions.insert(entry.to_owned(), version.to_owned());
            continue;
        }
        let mut parts = line.splitn(4, '\t');
        let (digest, key, stamp, path) =
            (parts.next()?, parts.next()?, parts.next()?, parts.next()?);
        let key = (key != "-").then(|| key.to_owned());
        files.insert(
            path.to_owned(),
            IndexedFile {
                digest: digest.to_owned(),
                key,
                stamp: stamp.to_owned(),
            },
        );
    }
    Some(TreeIndex {
        base,
        dialect,
        written_ns,
        versions,
        files,
    })
}

/// The tree's changes against its index: the diff a full re-export of the
/// indexed base would give.
pub fn diff_against_index(tree: &Path, index: &TreeIndex) -> Result<TreeDiff> {
    let present = relative_files(tree)?;
    let mut diff = TreeDiff::default();
    let horizon = index.written_ns.saturating_sub(MARGIN_NS);
    for path in &present {
        let Some(file) = index.files.get(path) else {
            diff.added.push(path.clone());
            continue;
        };
        let full = tree.join(path);
        let (stamp, mtime) = stamp(&full)?;
        if stamp == file.stamp && mtime < horizon {
            continue;
        }
        if file_sha256(&full)? != file.digest {
            diff.changed.push(path.clone());
        }
    }
    for path in index.files.keys() {
        if !present.contains(path) {
            diff.removed.push(path.clone());
        }
    }
    Ok(diff)
}
