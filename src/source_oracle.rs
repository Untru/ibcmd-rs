//! Offline three-way evidence for native ibcmd, EDT, and ibcmd-rs exports.
//!
//! This module intentionally only reads three caller-supplied directories.  It does
//! not start EDT, a JVM, ibcmd, or any database client.  Equality is raw SHA-256
//! equality; branch labels are hypotheses for investigation, never causal proof.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::cli::SourceThreeWayOracleArgs;

pub const SOURCE_THREE_WAY_ORACLE_SCHEMA_VERSION: u32 = 1;
const MAX_VERSION_BYTES: usize = 512;

#[derive(Debug, Clone, Serialize, Eq, PartialEq)]
pub struct SourceOracleLimits {
    pub max_files: usize,
    pub max_total_bytes: u64,
    pub max_file_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Eq, PartialEq)]
pub struct SourceOracleToolVersions {
    pub native_ibcmd: String,
    pub edt_import_export: String,
    pub ibcmd_rs: String,
}

#[derive(Debug, Clone, Serialize, Eq, PartialEq)]
pub struct SourceOracleTree {
    pub root: String,
    pub file_count: usize,
    pub total_bytes: u64,
    pub tree_sha256: String,
}

#[derive(Debug, Clone, Serialize, Eq, PartialEq)]
pub struct SourceOracleSummary {
    pub all_equal: usize,
    pub native_edt_not_ours: usize,
    pub native_ours_not_edt: usize,
    pub edt_ours_not_native: usize,
    pub all_different: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SourceOracleAgreement {
    AllEqual,
    NativeEdtNotOurs,
    NativeOursNotEdt,
    EdtOursNotNative,
    AllDifferent,
}

impl SourceOracleAgreement {
    fn candidate_interpretation(&self) -> &'static str {
        match self {
            Self::AllEqual => "no raw divergence observed",
            Self::NativeEdtNotOurs => {
                "candidate: ibcmd-rs decoder, model, schema, or writer divergence; hashes alone do not prove a layer"
            }
            Self::NativeOursNotEdt => {
                "candidate: EDT import/export oracle divergence; hashes alone do not prove a cause"
            }
            Self::EdtOursNotNative => {
                "candidate: native export, storage state, or version divergence; hashes alone do not prove a cause"
            }
            Self::AllDifferent => "unclassified; hashes alone do not identify a responsible layer",
        }
    }
}

#[derive(Debug, Clone, Serialize, Eq, PartialEq)]
pub struct SourceOracleRow {
    pub path: String,
    pub native_sha256: Option<String>,
    pub native_size_bytes: Option<u64>,
    pub edt_sha256: Option<String>,
    pub edt_size_bytes: Option<u64>,
    pub ours_sha256: Option<String>,
    pub ours_size_bytes: Option<u64>,
    pub agreement: SourceOracleAgreement,
    pub candidate_interpretation: String,
}

#[derive(Debug, Clone, Serialize, Eq, PartialEq)]
pub struct SourceThreeWayOracleReport {
    pub schema_version: u32,
    pub mode: String,
    pub source_version: String,
    pub tool_versions: SourceOracleToolVersions,
    pub limits: SourceOracleLimits,
    pub native: SourceOracleTree,
    pub edt: SourceOracleTree,
    pub ours: SourceOracleTree,
    pub summary: SourceOracleSummary,
    pub rows: Vec<SourceOracleRow>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct FileHash {
    sha256: String,
    size_bytes: u64,
}

#[derive(Debug, Clone)]
struct TreeSnapshot {
    tree: SourceOracleTree,
    files: BTreeMap<String, FileHash>,
}

#[derive(Debug)]
struct OutputPlan {
    requested_parent: PathBuf,
    canonical_parent: PathBuf,
    parent_guard: File,
    parent_identity: FileIdentity,
    json_final: PathBuf,
    markdown_final: PathBuf,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct FileIdentity {
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(windows)]
    volume_serial_number: u32,
    #[cfg(windows)]
    file_index: u64,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum PublishFailurePoint {
    None,
    AfterFirstPublish,
}

pub fn run_source_three_way_oracle(
    args: &SourceThreeWayOracleArgs,
) -> Result<SourceThreeWayOracleReport> {
    validate_nonempty("source version", &args.source_version)?;
    validate_nonempty("native tool version", &args.native_tool_version)?;
    validate_nonempty("EDT tool version", &args.edt_tool_version)?;
    validate_nonempty("ibcmd-rs tool version", &args.ours_tool_version)?;
    let limits = SourceOracleLimits {
        max_files: args.max_files,
        max_total_bytes: args.max_total_bytes,
        max_file_bytes: args.max_file_bytes,
    };
    if limits.max_files == 0
        || limits.max_total_bytes == 0
        || limits.max_file_bytes == 0
        || limits.max_file_bytes == u64::MAX
    {
        bail!(
            "three-way oracle limits must be positive and max-file-bytes must be less than u64::MAX"
        );
    }
    let output_plan = preflight_outputs(
        &args.output,
        &args.markdown,
        [
            args.native.as_path(),
            args.edt.as_path(),
            args.ours.as_path(),
        ],
    )?;

    let native = snapshot_tree(&args.native, &limits)?;
    let edt = snapshot_tree(&args.edt, &limits)?;
    let ours = snapshot_tree(&args.ours, &limits)?;
    let report = build_report(
        args.source_version.clone(),
        SourceOracleToolVersions {
            native_ibcmd: args.native_tool_version.clone(),
            edt_import_export: args.edt_tool_version.clone(),
            ibcmd_rs: args.ours_tool_version.clone(),
        },
        limits,
        native,
        edt,
        ours,
    );
    publish_source_three_way_oracle_artifacts(&report, &output_plan, PublishFailurePoint::None)?;
    Ok(report)
}

fn validate_nonempty(label: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        bail!("{label} must be supplied explicitly and cannot be empty");
    }
    if value.len() > MAX_VERSION_BYTES {
        bail!("{label} exceeds {MAX_VERSION_BYTES} UTF-8 bytes");
    }
    if value.chars().any(char::is_control) {
        bail!("{label} must not contain control characters");
    }
    Ok(())
}

fn preflight_outputs<'a>(
    json: &Path,
    markdown: &Path,
    input_roots: impl IntoIterator<Item = &'a Path>,
) -> Result<OutputPlan> {
    let input_roots = input_roots.into_iter().collect::<Vec<_>>();
    let (json_parent, json_name) = output_parent_and_name(json)?;
    let (markdown_parent, markdown_name) = output_parent_and_name(markdown)?;
    let canonical_json_parent = canonicalize_safe_parent(&json_parent)?;
    let canonical_markdown_parent = canonicalize_safe_parent(&markdown_parent)?;
    if !paths_equal(&canonical_json_parent, &canonical_markdown_parent) {
        bail!("JSON and Markdown artifacts must have the same canonical parent directory");
    }
    if os_strings_equal(&json_name, &markdown_name) {
        bail!("JSON and Markdown destinations must be different paths");
    }
    let json_final = canonical_json_parent.join(&json_name);
    let markdown_final = canonical_json_parent.join(&markdown_name);
    ensure_destination_absent(&json_final)?;
    ensure_destination_absent(&markdown_final)?;
    for root in &input_roots {
        let absolute_root = fs::canonicalize(root)
            .with_context(|| format!("failed to canonicalize input root {}", root.display()))?;
        if path_is_within(&canonical_json_parent, &absolute_root) {
            bail!(
                "oracle artifacts must be outside immutable input tree {}",
                root.display()
            );
        }
    }
    let parent_guard = open_parent_guard(&canonical_json_parent)?;
    let parent_identity = file_identity(&parent_guard)?;
    Ok(OutputPlan {
        requested_parent: json_parent,
        canonical_parent: canonical_json_parent,
        parent_guard,
        parent_identity,
        json_final,
        markdown_final,
    })
}

#[cfg(windows)]
fn open_parent_guard(parent: &Path) -> Result<File> {
    use std::os::windows::fs::OpenOptionsExt;

    const FILE_SHARE_READ: u32 = 0x0000_0001;
    const FILE_SHARE_WRITE: u32 = 0x0000_0002;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(parent)
        .with_context(|| format!("failed to hold output parent {}", parent.display()))
}

#[cfg(not(windows))]
fn open_parent_guard(parent: &Path) -> Result<File> {
    File::open(parent).with_context(|| format!("failed to hold output parent {}", parent.display()))
}

fn output_parent_and_name(path: &Path) -> Result<(PathBuf, OsString)> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .context("failed to determine current directory")?
            .join(path)
    };
    let parent = absolute
        .parent()
        .ok_or_else(|| anyhow!("oracle artifact has no parent: {}", path.display()))?;
    let name = absolute
        .file_name()
        .ok_or_else(|| anyhow!("oracle artifact has no file name: {}", path.display()))?;
    if name.is_empty() {
        bail!(
            "oracle artifact file name cannot be empty: {}",
            path.display()
        );
    }
    Ok((parent.to_path_buf(), name.to_os_string()))
}

fn canonicalize_safe_parent(parent: &Path) -> Result<PathBuf> {
    let metadata = fs::symlink_metadata(parent)
        .with_context(|| format!("failed to stat output parent {}", parent.display()))?;
    if metadata.file_type().is_symlink() || metadata_is_reparse_point(&metadata) {
        bail!(
            "output parent must not be a symlink or reparse point: {}",
            parent.display()
        );
    }
    if !metadata.is_dir() {
        bail!(
            "oracle artifact parent is not a directory: {}",
            parent.display()
        );
    }
    fs::canonicalize(parent)
        .with_context(|| format!("failed to canonicalize output parent {}", parent.display()))
}

fn snapshot_tree(root: &Path, limits: &SourceOracleLimits) -> Result<TreeSnapshot> {
    let root_metadata = fs::symlink_metadata(root)
        .with_context(|| format!("failed to stat input root {}", root.display()))?;
    if root_metadata.file_type().is_symlink() || metadata_is_reparse_point(&root_metadata) {
        bail!(
            "input root must not be a symlink or reparse point: {}",
            root.display()
        );
    }
    if !root_metadata.is_dir() {
        bail!(
            "source oracle input root is not a directory: {}",
            root.display()
        );
    }
    let root = fs::canonicalize(root)
        .with_context(|| format!("failed to canonicalize input root {}", root.display()))?;
    let mut files = BTreeMap::new();
    let mut total_bytes = 0_u64;
    for entry in WalkDir::new(&root).follow_links(false) {
        let entry =
            entry.with_context(|| format!("failed to walk input root {}", root.display()))?;
        if entry.file_type().is_symlink() || is_reparse_point(&entry)? {
            bail!(
                "input tree contains a symlink or reparse point: {}",
                entry.path().display()
            );
        }
        if !entry.file_type().is_file() {
            continue;
        }
        if files.len() == limits.max_files {
            bail!(
                "input root {} exceeds max-files={}",
                root.display(),
                limits.max_files
            );
        }
        let relative = entry.path().strip_prefix(&root).with_context(|| {
            format!(
                "failed to make {} relative to {}",
                entry.path().display(),
                root.display()
            )
        })?;
        let path = normalized_relative_path(relative)?;
        let remaining_total_bytes = limits
            .max_total_bytes
            .checked_sub(total_bytes)
            .ok_or_else(|| anyhow!("input byte accounting underflow under {}", root.display()))?;
        let file = stable_sha256_file(entry.path(), limits.max_file_bytes, remaining_total_bytes)?;
        total_bytes = total_bytes
            .checked_add(file.size_bytes)
            .ok_or_else(|| anyhow!("input byte total overflow under {}", root.display()))?;
        if files.insert(path.clone(), file).is_some() {
            bail!(
                "duplicate normalized source path {path} under {}",
                root.display()
            );
        }
    }
    let tree_sha256 = tree_sha256(&files);
    Ok(TreeSnapshot {
        tree: SourceOracleTree {
            root: root.display().to_string(),
            file_count: files.len(),
            total_bytes,
            tree_sha256,
        },
        files,
    })
}

fn normalized_relative_path(path: &Path) -> Result<String> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => parts.push(
                part.to_str()
                    .ok_or_else(|| {
                        anyhow!(
                            "source path contains a non-UTF-8 component and cannot be represented unambiguously"
                        )
                    })?
                    .to_owned(),
            ),
            Component::CurDir => {}
            _ => bail!(
                "source path contains an unsupported component: {}",
                path.display()
            ),
        }
    }
    if parts.is_empty() {
        bail!("source path cannot be empty");
    }
    Ok(parts.join("/"))
}

fn stable_sha256_file(
    path: &Path,
    max_file_bytes: u64,
    remaining_total_bytes: u64,
) -> Result<FileHash> {
    let mut file = open_input_file(path)?;
    let initial = file
        .metadata()
        .with_context(|| format!("failed to stat opened input file {}", path.display()))?;
    if !initial.is_file() {
        bail!("input path is no longer a regular file: {}", path.display());
    }
    let size_bytes = initial.len();
    if size_bytes > max_file_bytes {
        bail!(
            "input file {} exceeds max-file-bytes={max_file_bytes}",
            path.display()
        );
    }
    if size_bytes > remaining_total_bytes {
        bail!(
            "input tree exceeds max-total-bytes while accounting {}",
            path.display()
        );
    }
    let initial_state = stable_file_state(&file)?;
    let initial_link_metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect input path {}", path.display()))?;
    if initial_link_metadata.file_type().is_symlink()
        || metadata_is_reparse_point(&initial_link_metadata)
    {
        bail!(
            "input path became a symlink or reparse point before hashing: {}",
            path.display()
        );
    }
    ensure_same_path_state(path, &initial_state)?;
    let first_hash =
        hash_open_file_pass(&mut file, path, size_bytes, max_file_bytes, &initial_state)?;
    let second_hash =
        hash_open_file_pass(&mut file, path, size_bytes, max_file_bytes, &initial_state)?;
    if first_hash != second_hash {
        bail!(
            "input file changed between hash passes; expected immutable tree: {}",
            path.display()
        );
    }
    Ok(FileHash {
        sha256: first_hash,
        size_bytes,
    })
}

fn hash_open_file_pass(
    file: &mut File,
    path: &Path,
    expected_size: u64,
    max_file_bytes: u64,
    initial_state: &StableFileState,
) -> Result<String> {
    file.seek(SeekFrom::Start(0))
        .with_context(|| format!("failed to rewind {}", path.display()))?;
    let read_limit = expected_size
        .checked_add(1)
        .ok_or_else(|| anyhow!("accounted input size is too large to enforce safely"))?;
    debug_assert!(read_limit <= max_file_bytes + 1);
    let mut bounded = file.take(read_limit);
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut bytes_read = 0_u64;
    loop {
        let read = bounded
            .read(&mut buffer)
            .with_context(|| format!("failed to read {}", path.display()))?;
        if read == 0 {
            break;
        }
        bytes_read = bytes_read
            .checked_add(read as u64)
            .ok_or_else(|| anyhow!("input byte counter overflow for {}", path.display()))?;
        hasher.update(&buffer[..read]);
    }
    if bytes_read != expected_size {
        bail!(
            "input file size changed while hashing {}: expected {expected_size}, read {bytes_read}",
            path.display()
        );
    }
    ensure_same_file_state(path, initial_state, bounded.get_ref())?;
    ensure_same_path_state(path, initial_state)?;
    let link_metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect input path {}", path.display()))?;
    if link_metadata.file_type().is_symlink() || metadata_is_reparse_point(&link_metadata) {
        bail!(
            "input path became a symlink or reparse point while hashing: {}",
            path.display()
        );
    }
    Ok(format!("{:x}", hasher.finalize()))
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct StableFileState {
    size: u64,
    modified: SystemTime,
    identity: FileIdentity,
}

fn stable_file_state(file: &File) -> Result<StableFileState> {
    let metadata = file
        .metadata()
        .context("failed to stat opened input file")?;
    Ok(StableFileState {
        size: metadata.len(),
        modified: metadata
            .modified()
            .context("failed to read input file modification time")?,
        identity: file_identity(file)?,
    })
}

fn ensure_same_file_state(
    path: &Path,
    expected: &StableFileState,
    actual_file: &File,
) -> Result<()> {
    let actual = stable_file_state(actual_file)?;
    if &actual != expected {
        bail!(
            "input file changed or was replaced while hashing: {}",
            path.display()
        );
    }
    Ok(())
}

fn ensure_same_path_state(path: &Path, expected: &StableFileState) -> Result<()> {
    let current = open_input_file(path)
        .with_context(|| format!("failed to reopen input path {}", path.display()))?;
    ensure_same_file_state(path, expected, &current)
}

#[cfg(windows)]
fn open_input_file(path: &Path) -> Result<File> {
    use std::os::windows::fs::OpenOptionsExt;

    const FILE_SHARE_READ: u32 = 0x0000_0001;
    const FILE_SHARE_WRITE: u32 = 0x0000_0002;
    OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .open(path)
        .with_context(|| {
            format!(
                "failed to open {} without delete sharing for stable hashing",
                path.display()
            )
        })
}

#[cfg(not(windows))]
fn open_input_file(path: &Path) -> Result<File> {
    File::open(path).with_context(|| format!("failed to open {}", path.display()))
}

fn is_reparse_point(entry: &walkdir::DirEntry) -> Result<bool> {
    let metadata = entry
        .metadata()
        .with_context(|| format!("failed to stat {}", entry.path().display()))?;
    Ok(metadata_is_reparse_point(&metadata))
}

#[cfg(windows)]
fn metadata_is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn metadata_is_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

#[cfg(unix)]
fn file_identity(file: &File) -> Result<FileIdentity> {
    use std::os::unix::fs::MetadataExt;

    let metadata = file
        .metadata()
        .context("failed to stat open file identity")?;
    Ok(FileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}

#[cfg(windows)]
fn file_identity(file: &File) -> Result<FileIdentity> {
    use std::mem::MaybeUninit;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
    };

    let mut information = MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::zeroed();
    let succeeded =
        unsafe { GetFileInformationByHandle(file.as_raw_handle(), information.as_mut_ptr()) };
    if succeeded == 0 {
        return Err(std::io::Error::last_os_error())
            .context("failed to read stable Windows file identity");
    }
    let information = unsafe { information.assume_init() };
    Ok(FileIdentity {
        volume_serial_number: information.dwVolumeSerialNumber,
        file_index: (u64::from(information.nFileIndexHigh) << 32)
            | u64::from(information.nFileIndexLow),
    })
}

#[cfg(not(any(unix, windows)))]
fn file_identity(_file: &File) -> Result<FileIdentity> {
    Ok(FileIdentity {})
}

fn ensure_destination_absent(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(_) => bail!(
            "refusing to overwrite existing oracle artifact {}",
            path.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error)
            .with_context(|| format!("failed to inspect oracle artifact {}", path.display())),
    }
}

#[cfg(windows)]
fn paths_equal(left: &Path, right: &Path) -> bool {
    left.to_string_lossy()
        .eq_ignore_ascii_case(&right.to_string_lossy())
}

#[cfg(not(windows))]
fn paths_equal(left: &Path, right: &Path) -> bool {
    left == right
}

#[cfg(windows)]
fn os_strings_equal(left: &OsString, right: &OsString) -> bool {
    left.to_string_lossy()
        .eq_ignore_ascii_case(&right.to_string_lossy())
}

#[cfg(not(windows))]
fn os_strings_equal(left: &OsString, right: &OsString) -> bool {
    left == right
}

#[cfg(windows)]
fn path_is_within(path: &Path, root: &Path) -> bool {
    let path = path.to_string_lossy();
    let root = root.to_string_lossy();
    if path.eq_ignore_ascii_case(&root) {
        return true;
    }
    let root_with_separator = format!("{}{}", root.trim_end_matches(['\\', '/']), '\\');
    path.get(..root_with_separator.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(&root_with_separator))
}

#[cfg(not(windows))]
fn path_is_within(path: &Path, root: &Path) -> bool {
    path.starts_with(root)
}

fn tree_sha256(files: &BTreeMap<String, FileHash>) -> String {
    let mut hasher = Sha256::new();
    for (path, file) in files {
        hasher.update(path.as_bytes());
        hasher.update([0]);
        hasher.update(file.sha256.as_bytes());
        hasher.update([0]);
        hasher.update(file.size_bytes.to_string().as_bytes());
        hasher.update(b"\n");
    }
    format!("{:x}", hasher.finalize())
}

fn build_report(
    source_version: String,
    tool_versions: SourceOracleToolVersions,
    limits: SourceOracleLimits,
    native: TreeSnapshot,
    edt: TreeSnapshot,
    ours: TreeSnapshot,
) -> SourceThreeWayOracleReport {
    let paths = native
        .files
        .keys()
        .chain(edt.files.keys())
        .chain(ours.files.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut summary = SourceOracleSummary {
        all_equal: 0,
        native_edt_not_ours: 0,
        native_ours_not_edt: 0,
        edt_ours_not_native: 0,
        all_different: 0,
    };
    let mut rows = Vec::with_capacity(paths.len());
    for path in paths {
        let native_file = native.files.get(&path);
        let edt_file = edt.files.get(&path);
        let ours_file = ours.files.get(&path);
        let agreement = classify_agreement(
            native_file.map(|file| file.sha256.as_str()),
            edt_file.map(|file| file.sha256.as_str()),
            ours_file.map(|file| file.sha256.as_str()),
        );
        increment_summary(&mut summary, &agreement);
        rows.push(SourceOracleRow {
            path,
            native_sha256: native_file.map(|file| file.sha256.clone()),
            native_size_bytes: native_file.map(|file| file.size_bytes),
            edt_sha256: edt_file.map(|file| file.sha256.clone()),
            edt_size_bytes: edt_file.map(|file| file.size_bytes),
            ours_sha256: ours_file.map(|file| file.sha256.clone()),
            ours_size_bytes: ours_file.map(|file| file.size_bytes),
            candidate_interpretation: agreement.candidate_interpretation().to_string(),
            agreement,
        });
    }
    SourceThreeWayOracleReport {
        schema_version: SOURCE_THREE_WAY_ORACLE_SCHEMA_VERSION,
        mode: "offline_research_only".to_string(),
        source_version,
        tool_versions,
        limits,
        native: native.tree,
        edt: edt.tree,
        ours: ours.tree,
        summary,
        rows,
    }
}

pub fn classify_agreement(
    native: Option<&str>,
    edt: Option<&str>,
    ours: Option<&str>,
) -> SourceOracleAgreement {
    if native == edt && edt == ours {
        SourceOracleAgreement::AllEqual
    } else if native == edt {
        SourceOracleAgreement::NativeEdtNotOurs
    } else if native == ours {
        SourceOracleAgreement::NativeOursNotEdt
    } else if edt == ours {
        SourceOracleAgreement::EdtOursNotNative
    } else {
        SourceOracleAgreement::AllDifferent
    }
}

fn increment_summary(summary: &mut SourceOracleSummary, agreement: &SourceOracleAgreement) {
    match agreement {
        SourceOracleAgreement::AllEqual => summary.all_equal += 1,
        SourceOracleAgreement::NativeEdtNotOurs => summary.native_edt_not_ours += 1,
        SourceOracleAgreement::NativeOursNotEdt => summary.native_ours_not_edt += 1,
        SourceOracleAgreement::EdtOursNotNative => summary.edt_ours_not_native += 1,
        SourceOracleAgreement::AllDifferent => summary.all_different += 1,
    }
}

pub fn write_source_three_way_oracle_artifacts(
    report: &SourceThreeWayOracleReport,
    json: &Path,
    markdown: &Path,
) -> Result<()> {
    let plan = preflight_outputs(json, markdown, std::iter::empty::<&Path>())?;
    publish_source_three_way_oracle_artifacts(report, &plan, PublishFailurePoint::None)
}

fn publish_source_three_way_oracle_artifacts(
    report: &SourceThreeWayOracleReport,
    plan: &OutputPlan,
    failure_point: PublishFailurePoint,
) -> Result<()> {
    let json_text = serde_json::to_string_pretty(report)?;
    let markdown_text = render_source_three_way_oracle_markdown(report);
    let mut transaction = ArtifactTransaction::default();
    let result = (|| {
        revalidate_output_parent(plan)?;
        ensure_destination_absent(&plan.json_final)?;
        ensure_destination_absent(&plan.markdown_final)?;

        let (json_temp, mut json_file) = create_temp_artifact(plan, "json")?;
        transaction.temps.push(json_temp.clone());
        write_sync_artifact(&mut json_file, json_text.as_bytes(), &json_temp)?;

        let (markdown_temp, mut markdown_file) = create_temp_artifact(plan, "markdown")?;
        transaction.temps.push(markdown_temp.clone());
        write_sync_artifact(&mut markdown_file, markdown_text.as_bytes(), &markdown_temp)?;

        revalidate_output_parent(plan)?;
        ensure_destination_absent(&plan.json_final)?;
        ensure_destination_absent(&plan.markdown_final)?;
        publish_without_overwrite(&json_temp, &plan.json_final)?;
        transaction.finals.push(plan.json_final.clone());
        if failure_point == PublishFailurePoint::AfterFirstPublish {
            bail!("injected failure after first oracle artifact publication");
        }

        revalidate_output_parent(plan)?;
        ensure_destination_absent(&plan.markdown_final)?;
        publish_without_overwrite(&markdown_temp, &plan.markdown_final)?;
        transaction.finals.push(plan.markdown_final.clone());
        drop(markdown_file);
        drop(json_file);
        transaction.remove_temps()?;
        Ok(())
    })();
    match result {
        Ok(()) => {
            transaction.commit();
            Ok(())
        }
        Err(primary) => match transaction.rollback() {
            Ok(()) => Err(primary),
            Err(cleanup) => Err(anyhow!(
                "{primary:#}; additionally failed to roll back oracle artifacts: {cleanup:#}"
            )),
        },
    }
}

fn create_temp_artifact(plan: &OutputPlan, label: &str) -> Result<(PathBuf, File)> {
    for _ in 0..16 {
        let path = plan.canonical_parent.join(format!(
            ".source-three-way-oracle-{label}-{}.tmp",
            uuid::Uuid::new_v4()
        ));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("failed to create oracle temp artifact {}", path.display())
                });
            }
        }
    }
    bail!(
        "failed to allocate a unique oracle temp artifact in {}",
        plan.canonical_parent.display()
    )
}

fn write_sync_artifact(output: &mut File, bytes: &[u8], path: &Path) -> Result<()> {
    output
        .write_all(bytes)
        .with_context(|| format!("failed to write oracle artifact {}", path.display()))?;
    output
        .flush()
        .with_context(|| format!("failed to flush oracle artifact {}", path.display()))?;
    output
        .sync_all()
        .with_context(|| format!("failed to sync oracle artifact {}", path.display()))
}

fn publish_without_overwrite(temp: &Path, final_path: &Path) -> Result<()> {
    fs::hard_link(temp, final_path).with_context(|| {
        format!(
            "failed to publish oracle artifact {} without overwrite",
            final_path.display()
        )
    })
}

fn revalidate_output_parent(plan: &OutputPlan) -> Result<()> {
    let metadata = fs::symlink_metadata(&plan.requested_parent).with_context(|| {
        format!(
            "failed to revalidate output parent {}",
            plan.requested_parent.display()
        )
    })?;
    if metadata.file_type().is_symlink() || metadata_is_reparse_point(&metadata) {
        bail!(
            "output parent became a symlink or reparse point: {}",
            plan.requested_parent.display()
        );
    }
    if !metadata.is_dir() {
        bail!(
            "output parent is no longer a directory: {}",
            plan.requested_parent.display()
        );
    }
    let canonical = fs::canonicalize(&plan.requested_parent).with_context(|| {
        format!(
            "failed to recanonicalize output parent {}",
            plan.requested_parent.display()
        )
    })?;
    let held_identity = file_identity(&plan.parent_guard)?;
    let current_parent_guard = open_parent_guard(&plan.requested_parent)?;
    let current_identity = file_identity(&current_parent_guard)?;
    if !paths_equal(&canonical, &plan.canonical_parent)
        || current_identity != plan.parent_identity
        || held_identity != plan.parent_identity
    {
        bail!(
            "output parent identity changed before publication: {}",
            plan.requested_parent.display()
        );
    }
    Ok(())
}

#[derive(Default)]
struct ArtifactTransaction {
    temps: Vec<PathBuf>,
    finals: Vec<PathBuf>,
    committed: bool,
}

impl ArtifactTransaction {
    fn remove_temps(&mut self) -> Result<()> {
        let temps = self.temps.clone();
        for path in temps {
            remove_if_exists(&path)?;
            self.temps.retain(|candidate| candidate != &path);
        }
        Ok(())
    }

    fn rollback(&mut self) -> Result<()> {
        let mut errors = Vec::new();
        for path in self.finals.iter().rev().chain(self.temps.iter().rev()) {
            if let Err(error) = remove_if_exists(path) {
                errors.push(format!("{}: {error:#}", path.display()));
            }
        }
        self.finals.clear();
        self.temps.clear();
        if errors.is_empty() {
            Ok(())
        } else {
            bail!(errors.join("; "))
        }
    }

    fn commit(&mut self) {
        self.committed = true;
        self.finals.clear();
        self.temps.clear();
    }
}

impl Drop for ArtifactTransaction {
    fn drop(&mut self) {
        if !self.committed {
            let _ = self.rollback();
        }
    }
}

fn remove_if_exists(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("failed to remove {}", path.display())),
    }
}

pub fn render_source_three_way_oracle_markdown(report: &SourceThreeWayOracleReport) -> String {
    let mut markdown = String::from("# Three-way source oracle\n\n");
    markdown.push_str("Offline research-only comparison of caller-supplied, pre-existing trees. It does not launch EDT, a JVM, ibcmd, or database tooling. Candidate interpretations are hypotheses; SHA-256 equality alone cannot prove a decoder, model, schema, writer, storage, or version cause.\n\n");
    markdown.push_str("## Provenance\n\n");
    markdown.push_str(&format!(
        "- Source version: {}\n",
        markdown_plain_text(&report.source_version)
    ));
    markdown.push_str(&format!(
        "- Native ibcmd: {}\n",
        markdown_plain_text(&report.tool_versions.native_ibcmd)
    ));
    markdown.push_str(&format!(
        "- EDT import/export: {}\n",
        markdown_plain_text(&report.tool_versions.edt_import_export)
    ));
    markdown.push_str(&format!(
        "- ibcmd-rs: {}\n",
        markdown_plain_text(&report.tool_versions.ibcmd_rs)
    ));
    markdown.push_str(&format!(
        "- Limits: files={}, total bytes={}, per file={}\n\n",
        report.limits.max_files, report.limits.max_total_bytes, report.limits.max_file_bytes
    ));
    markdown.push_str("## Tree hashes\n\n| Tree | Files | Bytes | SHA-256 (`path + NUL + file SHA-256 + NUL + size + LF`) |\n|---|---:|---:|---|\n");
    for (label, tree) in [
        ("native", &report.native),
        ("edt", &report.edt),
        ("ours", &report.ours),
    ] {
        markdown.push_str(&format!(
            "| {} | {} | {} | `{}` |\n",
            label, tree.file_count, tree.total_bytes, tree.tree_sha256
        ));
    }
    markdown.push_str(
        "\n## Agreement summary\n\n| Branch | Paths | Candidate interpretation |\n|---|---:|---|\n",
    );
    for (agreement, count) in [
        (SourceOracleAgreement::AllEqual, report.summary.all_equal),
        (
            SourceOracleAgreement::NativeEdtNotOurs,
            report.summary.native_edt_not_ours,
        ),
        (
            SourceOracleAgreement::NativeOursNotEdt,
            report.summary.native_ours_not_edt,
        ),
        (
            SourceOracleAgreement::EdtOursNotNative,
            report.summary.edt_ours_not_native,
        ),
        (
            SourceOracleAgreement::AllDifferent,
            report.summary.all_different,
        ),
    ] {
        markdown.push_str(&format!(
            "| `{}` | {} | {} |\n",
            agreement_name(&agreement),
            count,
            agreement.candidate_interpretation()
        ));
    }
    markdown.push_str("\n## Path-level evidence\n\n| Path | Native SHA-256 | EDT SHA-256 | ibcmd-rs SHA-256 | Branch |\n|---|---|---|---|---|\n");
    for row in &report.rows {
        markdown.push_str(&format!(
            "| {} | {} | {} | {} | `{}` |\n",
            markdown_plain_text(&row.path),
            row.native_sha256.as_deref().unwrap_or("—"),
            row.edt_sha256.as_deref().unwrap_or("—"),
            row.ours_sha256.as_deref().unwrap_or("—"),
            agreement_name(&row.agreement)
        ));
    }
    markdown
}

fn agreement_name(agreement: &SourceOracleAgreement) -> &'static str {
    match agreement {
        SourceOracleAgreement::AllEqual => "all_equal",
        SourceOracleAgreement::NativeEdtNotOurs => "native_edt_not_ours",
        SourceOracleAgreement::NativeOursNotEdt => "native_ours_not_edt",
        SourceOracleAgreement::EdtOursNotNative => "edt_ours_not_native",
        SourceOracleAgreement::AllDifferent => "all_different",
    }
}

fn markdown_plain_text(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '\\' => escaped.push_str("&#92;"),
            '|' => escaped.push_str("&#124;"),
            '`' => escaped.push_str("&#96;"),
            '\r' => escaped.push_str("&#13;"),
            '\n' => escaped.push_str("<br>"),
            character => escaped.push(character),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use std::fs;

    #[cfg(unix)]
    use super::normalized_relative_path;
    use super::{
        PublishFailurePoint, SourceOracleAgreement, SourceOracleLimits, SourceOracleRow,
        SourceOracleSummary, SourceOracleToolVersions, SourceOracleTree,
        SourceThreeWayOracleReport, classify_agreement, preflight_outputs,
        publish_source_three_way_oracle_artifacts, render_source_three_way_oracle_markdown,
    };
    #[cfg(windows)]
    use super::{ensure_same_path_state, open_input_file, stable_file_state};

    #[test]
    fn agreement_classifier_covers_all_five_path_states() {
        assert_eq!(
            classify_agreement(Some("a"), Some("a"), Some("a")),
            SourceOracleAgreement::AllEqual
        );
        assert_eq!(
            classify_agreement(Some("a"), Some("a"), Some("b")),
            SourceOracleAgreement::NativeEdtNotOurs
        );
        assert_eq!(
            classify_agreement(Some("a"), Some("b"), Some("a")),
            SourceOracleAgreement::NativeOursNotEdt
        );
        assert_eq!(
            classify_agreement(Some("b"), Some("a"), Some("a")),
            SourceOracleAgreement::EdtOursNotNative
        );
        assert_eq!(
            classify_agreement(Some("a"), Some("b"), Some("c")),
            SourceOracleAgreement::AllDifferent
        );
    }

    #[test]
    fn paired_publication_rolls_back_first_final_and_all_temps_on_failure() {
        let root = unique_test_root("rollback");
        fs::create_dir(&root).unwrap();
        let json = root.join("report.json");
        let markdown = root.join("report.md");
        let plan =
            preflight_outputs(&json, &markdown, std::iter::empty::<&std::path::Path>()).unwrap();
        let error = publish_source_three_way_oracle_artifacts(
            &hostile_report(),
            &plan,
            PublishFailurePoint::AfterFirstPublish,
        )
        .unwrap_err();
        assert!(error.to_string().contains("injected failure"));
        assert!(!json.exists());
        assert!(!markdown.exists());
        assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
        drop(plan);
        fs::remove_dir(root).unwrap();
    }

    #[cfg(not(windows))]
    #[test]
    fn output_parent_identity_swap_fails_without_artifacts() {
        let root = unique_test_root("parent-swap");
        let output_parent = root.join("output");
        let displaced_parent = root.join("displaced");
        fs::create_dir_all(&output_parent).unwrap();
        let json = output_parent.join("report.json");
        let markdown = output_parent.join("report.md");
        let plan =
            preflight_outputs(&json, &markdown, std::iter::empty::<&std::path::Path>()).unwrap();
        fs::rename(&output_parent, &displaced_parent).unwrap();
        fs::create_dir(&output_parent).unwrap();
        let error = publish_source_three_way_oracle_artifacts(
            &hostile_report(),
            &plan,
            PublishFailurePoint::None,
        )
        .unwrap_err();
        assert!(error.to_string().contains("identity changed"));
        assert_eq!(fs::read_dir(&output_parent).unwrap().count(), 0);
        assert_eq!(fs::read_dir(&displaced_parent).unwrap().count(), 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn held_output_parent_blocks_identity_swap_on_windows() {
        let root = unique_test_root("held-parent");
        let output_parent = root.join("output");
        let displaced_parent = root.join("displaced");
        fs::create_dir_all(&output_parent).unwrap();
        let plan = preflight_outputs(
            &output_parent.join("report.json"),
            &output_parent.join("report.md"),
            std::iter::empty::<&std::path::Path>(),
        )
        .unwrap();
        assert!(fs::rename(&output_parent, &displaced_parent).is_err());
        assert_eq!(fs::read_dir(&output_parent).unwrap().count(), 0);
        drop(plan);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn markdown_entity_escapes_hostile_provenance_and_paths() {
        let markdown = render_source_three_way_oracle_markdown(&hostile_report());
        assert!(markdown.contains("v&#124;&#96;&#92;&lt;&amp;&gt;"));
        assert!(markdown.contains("dir&#92;name&#124;x&#13;<br>&lt;y&gt;&amp;&#96;"));
        assert!(!markdown.contains("dir\\name|x\r\n<y>&`"));
        assert_eq!(
            markdown
                .lines()
                .filter(|line| line.contains("all_equal.txt"))
                .count(),
            0
        );
    }

    #[cfg(windows)]
    #[test]
    fn case_only_output_alias_is_rejected_on_windows() {
        let root = unique_test_root("case-alias");
        fs::create_dir(&root).unwrap();
        let error = preflight_outputs(
            &root.join("Report.JSON"),
            &root.join("report.json"),
            std::iter::empty::<&std::path::Path>(),
        )
        .unwrap_err();
        assert!(error.to_string().contains("different paths"));
        fs::remove_dir(root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn same_size_replacement_with_copied_windows_metadata_has_new_identity() {
        use std::fs::OpenOptions;
        use std::os::windows::fs::MetadataExt;
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Foundation::FILETIME;
        use windows_sys::Win32::Storage::FileSystem::SetFileTime;

        fn filetime(value: u64) -> FILETIME {
            FILETIME {
                dwLowDateTime: value as u32,
                dwHighDateTime: (value >> 32) as u32,
            }
        }

        let root = unique_test_root("same-metadata-replacement");
        fs::create_dir(&root).unwrap();
        let path = root.join("input.bin");
        let replacement = root.join("replacement.bin");
        fs::write(&path, b"original").unwrap();
        fs::write(&replacement, b"replaced").unwrap();

        let original_metadata = fs::metadata(&path).unwrap();
        fs::set_permissions(&replacement, original_metadata.permissions()).unwrap();
        let replacement_file = OpenOptions::new().write(true).open(&replacement).unwrap();
        let creation = filetime(original_metadata.creation_time());
        let access = filetime(original_metadata.last_access_time());
        let write = filetime(original_metadata.last_write_time());
        let succeeded =
            unsafe { SetFileTime(replacement_file.as_raw_handle(), &creation, &access, &write) };
        assert_ne!(
            succeeded,
            0,
            "failed to copy Windows timestamps: {}",
            std::io::Error::last_os_error()
        );
        drop(replacement_file);

        let replacement_metadata = fs::metadata(&replacement).unwrap();
        assert_eq!(replacement_metadata.len(), original_metadata.len());
        assert_eq!(
            replacement_metadata.creation_time(),
            original_metadata.creation_time()
        );
        assert_eq!(
            replacement_metadata.last_access_time(),
            original_metadata.last_access_time()
        );
        assert_eq!(
            replacement_metadata.last_write_time(),
            original_metadata.last_write_time()
        );
        assert_eq!(
            replacement_metadata.file_attributes(),
            original_metadata.file_attributes()
        );

        let original_file = open_input_file(&path).unwrap();
        let original_state = stable_file_state(&original_file).unwrap();
        drop(original_file);
        fs::remove_file(&path).unwrap();
        fs::rename(&replacement, &path).unwrap();

        let error = ensure_same_path_state(&path, &original_state).unwrap_err();
        assert!(error.to_string().contains("changed or was replaced"));
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn distinct_non_utf8_source_names_are_rejected_without_lossy_collision() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let root = unique_test_root("non-utf8");
        fs::create_dir(&root).unwrap();
        let names = [
            OsString::from_vec(vec![b'n', b'a', b'm', b'e', 0xfe]),
            OsString::from_vec(vec![b'n', b'a', b'm', b'e', 0xff]),
        ];
        assert_ne!(names[0], names[1]);
        for name in &names {
            fs::write(root.join(name), b"content").unwrap();
            let error = normalized_relative_path(std::path::Path::new(name)).unwrap_err();
            assert!(error.to_string().contains("non-UTF-8 component"));
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn symlink_output_parent_is_rejected() {
        use std::os::unix::fs::symlink;

        let root = unique_test_root("parent-symlink");
        let real_parent = root.join("real");
        let alias_parent = root.join("alias");
        fs::create_dir_all(&real_parent).unwrap();
        symlink(&real_parent, &alias_parent).unwrap();
        let error = preflight_outputs(
            &alias_parent.join("report.json"),
            &alias_parent.join("report.md"),
            std::iter::empty::<&std::path::Path>(),
        )
        .unwrap_err();
        assert!(error.to_string().contains("symlink or reparse"));
        fs::remove_dir_all(root).unwrap();
    }

    fn unique_test_root(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "ibcmd-rs-source-oracle-{label}-{}",
            uuid::Uuid::new_v4()
        ))
    }

    fn hostile_report() -> SourceThreeWayOracleReport {
        let tree = SourceOracleTree {
            root: "synthetic".to_string(),
            file_count: 1,
            total_bytes: 1,
            tree_sha256: "a".repeat(64),
        };
        SourceThreeWayOracleReport {
            schema_version: 1,
            mode: "offline_research_only".to_string(),
            source_version: "v|`\\<&>".to_string(),
            tool_versions: SourceOracleToolVersions {
                native_ibcmd: "native|`\\<&>".to_string(),
                edt_import_export: "edt|`\\<&>".to_string(),
                ibcmd_rs: "ours|`\\<&>".to_string(),
            },
            limits: SourceOracleLimits {
                max_files: 1,
                max_total_bytes: 1,
                max_file_bytes: 1,
            },
            native: tree.clone(),
            edt: tree.clone(),
            ours: tree,
            summary: SourceOracleSummary {
                all_equal: 1,
                native_edt_not_ours: 0,
                native_ours_not_edt: 0,
                edt_ours_not_native: 0,
                all_different: 0,
            },
            rows: vec![SourceOracleRow {
                path: "dir\\name|x\r\n<y>&`".to_string(),
                native_sha256: Some("a".repeat(64)),
                native_size_bytes: Some(1),
                edt_sha256: Some("a".repeat(64)),
                edt_size_bytes: Some(1),
                ours_sha256: Some("a".repeat(64)),
                ours_size_bytes: Some(1),
                agreement: SourceOracleAgreement::AllEqual,
                candidate_interpretation: "no raw divergence observed".to_string(),
            }],
        }
    }
}
