//! Fail-closed classification for direct MSSQL source activation.
//!
//! This module deliberately stops before compilation, SQL rendering, or
//! publication.  Its output is the typed input those later phases may accept.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};

mod files;
use files::{FileIdentity, OriginalFile, RootAnchor};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

const SHA256_BYTES: usize = 32;

/// Bounded filesystem census used by the safety gate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceInventoryLimits {
    pub max_files: usize,
    pub max_file_bytes: u64,
    pub max_total_bytes: u64,
}

impl Default for SourceInventoryLimits {
    fn default() -> Self {
        Self {
            max_files: 500_000,
            max_file_bytes: 256 * 1024 * 1024,
            max_total_bytes: 4 * 1024 * 1024 * 1024,
        }
    }
}

/// Exact digest of one source file, addressed with `/` separators.
#[derive(Clone, Debug)]
pub struct SourceFileDigest {
    path: String,
    size_bytes: u64,
    sha256: [u8; SHA256_BYTES],
    verified_bytes: Option<Arc<Vec<u8>>>,
    identity: Option<FileIdentity>,
    original: Option<Arc<OriginalFile>>,
}

impl SourceFileDigest {
    pub fn new(
        path: impl Into<String>,
        size_bytes: u64,
        sha256: [u8; SHA256_BYTES],
    ) -> Result<Self, SourceChangeError> {
        let path = normalize_relative_path(&path.into())?;
        Ok(Self {
            path,
            size_bytes,
            sha256,
            verified_bytes: None,
            identity: None,
            original: None,
        })
    }

    pub fn for_bytes(path: impl Into<String>, bytes: &[u8]) -> Result<Self, SourceChangeError> {
        let digest: [u8; SHA256_BYTES] = Sha256::digest(bytes).into();
        let mut file = Self::new(path, bytes.len() as u64, digest)?;
        let mut retained = Vec::new();
        retained
            .try_reserve_exact(bytes.len())
            .map_err(|e| SourceChangeError::Io(e.to_string()))?;
        retained.extend_from_slice(bytes);
        file.verified_bytes = Some(Arc::new(retained));
        Ok(file)
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub const fn size_bytes(&self) -> u64 {
        self.size_bytes
    }

    pub const fn sha256(&self) -> &[u8; SHA256_BYTES] {
        &self.sha256
    }

    fn verified_bytes(&self) -> Option<&[u8]> {
        self.verified_bytes.as_ref().map(|bytes| bytes.as_slice())
    }
}

// Semantic inventory equality never grants original-file custody.
impl PartialEq for SourceFileDigest {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
            && self.size_bytes == other.size_bytes
            && self.sha256 == other.sha256
            && self.verified_bytes == other.verified_bytes
    }
}
impl Eq for SourceFileDigest {}

#[derive(Clone, Copy, Debug)]
enum CensusAdmission {
    CallerBudget(SourceInventoryLimits),
    OriginalSource,
}

impl CensusAdmission {
    fn check(self, count: usize, length: u64, total: u64) -> Result<(), SourceChangeError> {
        if let Self::CallerBudget(limits) = self {
            if count > limits.max_files {
                return Err(SourceChangeError::InventoryLimit("file count"));
            }
            if length > limits.max_file_bytes {
                return Err(SourceChangeError::InventoryLimit("single file size"));
            }
            if total > limits.max_total_bytes {
                return Err(SourceChangeError::InventoryLimit("total byte size"));
            }
        }
        Ok(())
    }
}

/// Immutable, alias-safe inventory of every file under a source root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceInventory {
    by_windows_key: BTreeMap<String, SourceFileDigest>,
}

impl SourceInventory {
    pub fn from_files(files: Vec<SourceFileDigest>) -> Result<Self, SourceChangeError> {
        let mut by_windows_key = BTreeMap::<String, SourceFileDigest>::new();
        for file in files {
            let key = windows_path_key(&file.path);
            if let Some(previous) = by_windows_key.get(&key) {
                return Err(SourceChangeError::WindowsPathCollision {
                    first: previous.path.clone(),
                    second: file.path,
                });
            }
            by_windows_key.insert(key, file);
        }
        Ok(Self { by_windows_key })
    }

    pub fn files(&self) -> impl ExactSizeIterator<Item = &SourceFileDigest> {
        self.by_windows_key.values()
    }

    /// Digest-only projection for the existing compiler tree transformation.
    /// It carries no retained body or original-file capability.
    pub(crate) fn digest_projection(&self) -> Self {
        let mut projection = self.clone();
        for member in projection.by_windows_key.values_mut() {
            member.verified_bytes = None;
            member.original = None;
        }
        projection
    }

    pub fn file(&self, path: &str) -> Result<Option<&SourceFileDigest>, SourceChangeError> {
        let path = normalize_relative_path(path)?;
        Ok(self.by_windows_key.get(&windows_path_key(&path)))
    }
}

/// A canonical root plus the baseline inventory captured while it was held.
///
/// The root is re-canonicalized before every subsequent capture. Replacement
/// by a junction/symlink therefore fails instead of silently changing scope.
pub struct HeldSourceRoot {
    requested_root: PathBuf,
    canonical_root: PathBuf,
    baseline: SourceInventory,
    admission: CensusAdmission,
    anchor: Arc<RootAnchor>,
    consumed: Mutex<BTreeMap<String, ConsumedSourceFile>>,
}

#[derive(Debug)]
struct ConsumedSourceFile {
    original: Arc<OriginalFile>,
    bytes: Arc<Vec<u8>>,
}

impl fmt::Debug for HeldSourceRoot {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HeldSourceRoot")
            .field("canonical_root", &self.canonical_root)
            .field("file_count", &self.baseline.files().len())
            .field("admission", &self.admission)
            .finish_non_exhaustive()
    }
}

impl HeldSourceRoot {
    pub fn open(root: &Path, limits: SourceInventoryLimits) -> Result<Self, SourceChangeError> {
        validate_limits(limits)?;
        let canonical_root = canonical_directory(root)?;
        Self::open_with_admission(
            root,
            canonical_root,
            CensusAdmission::CallerBudget(limits),
            &BTreeSet::new(),
        )
    }

    fn open_with_admission(
        root: &Path,
        canonical_root: PathBuf,
        admission: CensusAdmission,
        retained: &BTreeSet<String>,
    ) -> Result<Self, SourceChangeError> {
        let anchor = RootAnchor::open(&canonical_root)?;
        let baseline = capture_inventory(&anchor, admission, retained)?;
        Ok(Self {
            requested_root: root.to_owned(),
            canonical_root,
            baseline,
            admission,
            anchor,
            consumed: Mutex::new(BTreeMap::new()),
        })
    }

    pub(crate) fn open_source_operation(
        root: &Path,
        selected: &str,
    ) -> Result<Self, SourceChangeError> {
        let selected = normalize_relative_path(selected)?;
        Self::open_with_admission(
            root,
            canonical_directory(root)?,
            CensusAdmission::OriginalSource,
            &candidate_retention_paths(&selected),
        )
    }

    /// Private compiler admission has no caller-selected resource budget.
    /// It still captures every file and does not authorize absent members.
    pub(crate) fn open_compiler_operation(root: &Path) -> Result<Self, SourceChangeError> {
        Self::open_with_admission(
            root,
            canonical_directory(root)?,
            CensusAdmission::OriginalSource,
            &BTreeSet::new(),
        )
    }

    pub(crate) fn require_unchanged(&self) -> Result<(), SourceChangeError> {
        let consumed = self
            .consumed
            .lock()
            .map_err(|e| SourceChangeError::Io(e.to_string()))?;
        for (path, consumed) in consumed.iter() {
            let expected = self
                .baseline
                .file(path)?
                .ok_or(SourceChangeError::HeldRootChanged)?;
            consumed.original.require_digest(&expected.sha256)?;
        }
        drop(consumed);
        for source in self.baseline.files() {
            if let Some(original) = &source.original {
                original.require_digest(&source.sha256)?;
            }
        }
        let current = self.capture_current()?;
        if self.baseline.files().len() != current.files().len() {
            return Err(SourceChangeError::HeldRootChanged);
        }
        for original in self.baseline.files() {
            let Some(now) = current.file(original.path())? else {
                return Err(SourceChangeError::HeldRootChanged);
            };
            if original.path != now.path
                || original.identity != now.identity
                || original.size_bytes != now.size_bytes
                || original.sha256 != now.sha256
            {
                return Err(SourceChangeError::FileChangedDuringRead(
                    original.path.clone(),
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn source_bytes(&self, path: &str) -> Result<Arc<Vec<u8>>, SourceChangeError> {
        let source = self
            .baseline
            .file(path)?
            .ok_or_else(|| SourceChangeError::SelectedBodyDoesNotExist(path.to_owned()))?;
        if source.path != path {
            return Err(SourceChangeError::WindowsAliasMismatch {
                requested: path.to_owned(),
                actual: source.path.clone(),
            });
        }
        // Memoisation belongs to this original census; it cannot substitute a
        // cached member from another tree, or authorize a missing pathname.
        let mut consumed = self
            .consumed
            .lock()
            .map_err(|e| SourceChangeError::Io(e.to_string()))?;
        if let Some(consumed) = consumed.get(path) {
            // Immutable returned bytes remain bound to the original digest;
            // require_unchanged checks originals once at the publication gate.
            return Ok(consumed.bytes.clone());
        }
        let original = match &source.original {
            Some(original) => original.clone(),
            None => Arc::new(self.anchor.open_file(path)?),
        };
        if Some(original.identity()) != source.identity || original.length() != source.size_bytes {
            return Err(SourceChangeError::FileChangedDuringRead(path.to_owned()));
        }
        let bytes = if let Some(bytes) = &source.verified_bytes {
            original.require_digest(&source.sha256)?;
            bytes.clone()
        } else {
            let (digest, bytes) = original.read(true)?;
            if digest != source.sha256 {
                return Err(SourceChangeError::FileChangedDuringRead(path.to_owned()));
            }
            bytes.ok_or_else(|| SourceChangeError::UnverifiedSourceBytes(path.to_owned()))?
        };
        // Only bytes just read from the same original (or the originally
        // retained immutable bytes) enter the compiler's shared read owner.
        consumed.insert(
            path.to_owned(),
            ConsumedSourceFile {
                original,
                bytes: bytes.clone(),
            },
        );
        Ok(bytes)
    }

    /// Retain original XML bytes only where this census differs from the other.
    /// The whole census remains authoritative; equal digests, non-XML members,
    /// absent members and spelling mismatches require no semantic byte read.
    pub(crate) fn comparison_inventory(
        &self,
        other: &SourceInventory,
    ) -> Result<SourceInventory, SourceChangeError> {
        let mut comparison = self.baseline.clone();
        for member in comparison.by_windows_key.values_mut() {
            let Some(counterpart) = other.file(&member.path)? else {
                continue;
            };
            if member.path == counterpart.path
                && member.path.ends_with(".xml")
                && (member.size_bytes != counterpart.size_bytes
                    || member.sha256 != counterpart.sha256)
            {
                member.verified_bytes = Some(self.source_bytes(&member.path)?);
            }
        }
        Ok(comparison)
    }

    pub(crate) fn read_original_path(
        &self,
        path: &Path,
    ) -> Result<Arc<Vec<u8>>, SourceChangeError> {
        let relative = path
            .strip_prefix(&self.canonical_root)
            .or_else(|_| path.strip_prefix(&self.requested_root))
            .map_err(|_| SourceChangeError::HeldRootChanged)?;
        self.source_bytes(&path_to_slash(relative)?)
    }

    pub(crate) fn fingerprint_paths(
        &self,
        paths: &[String],
    ) -> Result<[u8; 32], SourceChangeError> {
        let mut hash = Sha256::new();
        for path in paths {
            let source = self
                .baseline
                .file(path)?
                .ok_or_else(|| SourceChangeError::SelectedBodyDoesNotExist(path.clone()))?;
            if source.path() != path {
                return Err(SourceChangeError::WindowsAliasMismatch {
                    requested: path.clone(),
                    actual: source.path.clone(),
                });
            }
            let original = self.anchor.open_file(path)?;
            if Some(original.identity()) != source.identity
                || original.length() != source.size_bytes
            {
                return Err(SourceChangeError::FileChangedDuringRead(path.clone()));
            }
            hash.update((path.len() as u64).to_le_bytes());
            hash.update(path.as_bytes());
            hash.update(source.size_bytes.to_le_bytes());
            original.extend_hash(&source.sha256, &mut hash)?;
        }
        self.require_unchanged()?;
        Ok(hash.finalize().into())
    }

    pub fn canonical_root(&self) -> &Path {
        &self.canonical_root
    }

    pub fn baseline(&self) -> &SourceInventory {
        &self.baseline
    }

    pub fn capture_current(&self) -> Result<SourceInventory, SourceChangeError> {
        let current_root =
            canonical_directory(&self.requested_root).map_err(|error| match error {
                SourceChangeError::LinkOrReparsePoint(_) => SourceChangeError::HeldRootChanged,
                other => other,
            })?;
        if !held_root_paths_equal(&current_root, &self.canonical_root) {
            return Err(SourceChangeError::HeldRootChanged);
        }
        capture_inventory(&self.anchor, self.admission, &BTreeSet::new())
    }

    pub fn classify_current(
        &self,
        selected_path: &Path,
        target: ActivationTarget,
        mode: ActivationMode,
    ) -> Result<SourceActivationInput, SourceChangeError> {
        let current_root =
            canonical_directory(&self.requested_root).map_err(|error| match error {
                SourceChangeError::LinkOrReparsePoint(_) => SourceChangeError::HeldRootChanged,
                other => other,
            })?;
        if !held_root_paths_equal(&current_root, &self.canonical_root) {
            return Err(SourceChangeError::HeldRootChanged);
        }
        // Validate the original ancestor before resolving a selected member;
        // a retargeted ancestor must never supply selected-path authority.
        let selected = resolve_selected_path(&self.canonical_root, selected_path)?;
        let retention = candidate_retention_paths(&selected);
        let current = capture_inventory(&self.anchor, self.admission, &retention)?;
        classify_source_change(&self.baseline, &current, &selected, target, mode)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivationMode {
    Online,
    Exclusive,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ActivationTarget {
    Main,
    Extension { name: String },
}

impl ActivationTarget {
    pub fn extension(name: impl Into<String>) -> Result<Self, SourceChangeError> {
        let name = name.into();
        let utf16_len = name.encode_utf16().count();
        if name.is_empty()
            || name.trim() != name
            || name.chars().any(char::is_control)
            || utf16_len > 128
        {
            return Err(SourceChangeError::InvalidExtensionName);
        }
        Ok(Self::Extension { name })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceChangeState {
    NoOp,
    Changed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceBodyKind {
    Module {
        owner_descriptor: String,
    },
    ManagedForm {
        form_descriptor: String,
        form_body: String,
        form_module: Option<String>,
    },
    Template {
        template_descriptor: String,
        source_paths: Vec<String>,
    },
}

/// Immutable, validated input to compilation and activation-plan construction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceActivationInput {
    target: ActivationTarget,
    mode: ActivationMode,
    selected_path: String,
    body: SourceBodyKind,
    changed_paths: Vec<String>,
    state: SourceChangeState,
    verified_sources: Vec<VerifiedSourceFile>,
}

/// Bytes pinned to the inventory digest used by the classifier.
///
/// Compilation must consume these bytes, never reopen the source path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedSourceFile {
    path: String,
    sha256: [u8; SHA256_BYTES],
    bytes: Arc<Vec<u8>>,
}

impl VerifiedSourceFile {
    pub fn path(&self) -> &str {
        &self.path
    }

    pub const fn sha256(&self) -> &[u8; SHA256_BYTES] {
        &self.sha256
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl SourceActivationInput {
    pub fn target(&self) -> &ActivationTarget {
        &self.target
    }

    pub const fn mode(&self) -> ActivationMode {
        self.mode
    }

    pub fn selected_path(&self) -> &str {
        &self.selected_path
    }

    pub fn body(&self) -> &SourceBodyKind {
        &self.body
    }

    pub fn changed_paths(&self) -> &[String] {
        &self.changed_paths
    }

    pub const fn state(&self) -> SourceChangeState {
        self.state
    }

    pub const fn is_no_op(&self) -> bool {
        matches!(self.state, SourceChangeState::NoOp)
    }

    pub fn verified_sources(&self) -> &[VerifiedSourceFile] {
        &self.verified_sources
    }

    pub fn verified_source(&self, path: &str) -> Option<&VerifiedSourceFile> {
        let key = windows_path_key(path);
        self.verified_sources
            .iter()
            .find(|source| windows_path_key(&source.path) == key)
    }
}

/// Classifies two deterministic inventories without touching the filesystem.
pub fn classify_source_change(
    active: &SourceInventory,
    proposed: &SourceInventory,
    selected_path: &str,
    target: ActivationTarget,
    mode: ActivationMode,
) -> Result<SourceActivationInput, SourceChangeError> {
    validate_target(&target)?;
    let selected_path = normalize_relative_path(selected_path)?;
    let active_selected = active
        .file(&selected_path)?
        .ok_or_else(|| SourceChangeError::SelectedBodyDoesNotExist(selected_path.clone()))?;
    let proposed_selected = proposed
        .file(&selected_path)?
        .ok_or_else(|| SourceChangeError::SelectedBodyWasRemoved(selected_path.clone()))?;
    if active_selected.path != proposed_selected.path {
        return Err(SourceChangeError::WindowsAliasMismatch {
            requested: selected_path,
            actual: proposed_selected.path.clone(),
        });
    }

    // The proposed census retains the body and template descriptor bytes.
    // The following whole-tree diff still requires descriptors and tree shape
    // to remain unchanged; a descriptor is never part of the writable closure.
    let body_inventory =
        if measured_template_dir(&active_selected.path.split('/').collect::<Vec<_>>()).is_some() {
            proposed
        } else {
            active
        };
    let body = classify_supported_body(&active_selected.path, body_inventory)?;
    let changed_paths = inventory_diff(active, proposed)?;
    let closure = dependency_closure(&body, &active_selected.path);
    let outside = changed_paths
        .iter()
        .filter(|path| !closure.contains(&windows_path_key(path)))
        .cloned()
        .collect::<Vec<_>>();
    if !outside.is_empty() {
        return Err(SourceChangeError::ChangesOutsideClosure(outside));
    }
    if !changed_paths.is_empty()
        && !matches!(&body, SourceBodyKind::Template { .. })
        && !changed_paths
            .iter()
            .any(|path| paths_equal_text_windows(path, &active_selected.path))
    {
        return Err(SourceChangeError::SelectedBodyIsNotChanged);
    }

    let mut verified_sources = Vec::new();
    for key in &closure {
        let source = proposed
            .by_windows_key
            .get(key)
            .expect("the validated closure contains existing proposed files");
        let bytes = source
            .verified_bytes()
            .ok_or_else(|| SourceChangeError::UnverifiedSourceBytes(source.path.clone()))?;
        let digest: [u8; SHA256_BYTES] = Sha256::digest(bytes).into();
        if bytes.len() as u64 != source.size_bytes || digest != source.sha256 {
            return Err(SourceChangeError::UnverifiedSourceBytes(
                source.path.clone(),
            ));
        }
        verified_sources
            .try_reserve(1)
            .map_err(|error| SourceChangeError::Io(error.to_string()))?;
        verified_sources.push(VerifiedSourceFile {
            path: source.path.clone(),
            sha256: digest,
            bytes: source
                .verified_bytes
                .clone()
                .expect("verified retained bytes"),
        });
    }
    verified_sources
        .sort_by(|left, right| windows_path_key(&left.path).cmp(&windows_path_key(&right.path)));

    Ok(SourceActivationInput {
        target,
        mode,
        selected_path: active_selected.path.clone(),
        body,
        state: if changed_paths.is_empty() {
            SourceChangeState::NoOp
        } else {
            SourceChangeState::Changed
        },
        changed_paths,
        verified_sources,
    })
}

fn candidate_retention_paths(selected: &str) -> BTreeSet<String> {
    let mut paths = BTreeSet::from([windows_path_key(selected)]);
    let parts = selected.split('/').collect::<Vec<_>>();
    if let Some(ext) = parts.iter().position(|part| *part == "Ext") {
        if ext > 0 {
            paths.insert(windows_path_key(&format!("{}.xml", parts[..ext].join("/"))));
            if ext > 2 {
                paths.insert(windows_path_key(&format!("{}/{}.xml", parts[0], parts[1])));
            }
        }
    }
    if let Some(form_dir_len) = managed_form_dir_len(&parts) {
        let form_dir = parts[..form_dir_len].join("/");
        paths.insert(windows_path_key(&format!("{form_dir}/Ext/Form.xml")));
        paths.insert(windows_path_key(&format!("{form_dir}/Ext/Form/Module.bsl")));
    }
    if let Some(template_dir) = measured_template_dir(&parts) {
        paths.insert(windows_path_key(&format!("{template_dir}.xml")));
        paths.insert(windows_path_key(&format!(
            "{template_dir}/Ext/Template.xml"
        )));
        paths.insert(windows_path_key(&format!(
            "{template_dir}/Ext/Template.txt"
        )));
        // A trailing slash denotes a retained resource subtree, not a file.
        paths.insert(windows_path_key(&format!("{template_dir}/Ext/Template/")));
    }
    paths
}

/// Deterministic byte comparison used after compilation.
pub fn compare_compiled_payload(active: &[u8], proposed: &[u8]) -> SourceChangeState {
    let active_digest: [u8; SHA256_BYTES] = Sha256::digest(active).into();
    let proposed_digest: [u8; SHA256_BYTES] = Sha256::digest(proposed).into();
    if active_digest == proposed_digest && active.len() == proposed.len() {
        SourceChangeState::NoOp
    } else {
        SourceChangeState::Changed
    }
}

fn inventory_diff(
    active: &SourceInventory,
    proposed: &SourceInventory,
) -> Result<Vec<String>, SourceChangeError> {
    let active_keys = active.by_windows_key.keys().collect::<BTreeSet<_>>();
    let proposed_keys = proposed.by_windows_key.keys().collect::<BTreeSet<_>>();
    let additions = proposed_keys.difference(&active_keys).count();
    let removals = active_keys.difference(&proposed_keys).count();
    if additions != 0 || removals != 0 {
        return Err(SourceChangeError::SourceTreeShapeChanged {
            additions,
            removals,
        });
    }
    let mut changed = Vec::new();
    for (key, active_file) in &active.by_windows_key {
        let proposed_file = proposed
            .by_windows_key
            .get(key)
            .expect("equal key sets were established");
        if active_file.path != proposed_file.path {
            return Err(SourceChangeError::WindowsAliasMismatch {
                requested: active_file.path.clone(),
                actual: proposed_file.path.clone(),
            });
        }
        if !source_files_equal(active_file, proposed_file)? {
            changed.push(active_file.path.clone());
        }
    }
    changed.sort_by_key(|path| windows_path_key(path));
    Ok(changed)
}

fn source_files_equal(
    active: &SourceFileDigest,
    proposed: &SourceFileDigest,
) -> Result<bool, SourceChangeError> {
    if active.size_bytes == proposed.size_bytes && active.sha256 == proposed.sha256 {
        return Ok(true);
    }
    if !active.path.ends_with(".xml") {
        return Ok(false);
    }
    let (Some(active_bytes), Some(proposed_bytes)) =
        (active.verified_bytes(), proposed.verified_bytes())
    else {
        return Ok(false);
    };
    let parse = |side: &str, bytes: &[u8]| {
        crate::plan::parse_indexed_xml_values(bytes).map_err(|error| {
            SourceChangeError::Io(format!(
                "failed to parse {side} XML {}: {error:#}",
                active.path
            ))
        })
    };
    Ok(parse("active", active_bytes)? == parse("proposed", proposed_bytes)?)
}

fn classify_supported_body(
    path: &str,
    inventory: &SourceInventory,
) -> Result<SourceBodyKind, SourceChangeError> {
    let parts = path.split('/').collect::<Vec<_>>();
    if let Some(template_dir) = measured_template_dir(&parts) {
        let descriptor = format!("{template_dir}.xml");
        require_existing(
            inventory,
            &format!("{}/{}.xml", parts[0], parts[1]),
            "top-level metadata owner descriptor",
        )?;
        require_existing(inventory, &descriptor, "template descriptor")?;
        let file = inventory
            .file(&descriptor)?
            .expect("required descriptor exists");
        let xml = file
            .verified_bytes()
            .ok_or_else(|| SourceChangeError::UnverifiedSourceBytes(descriptor.clone()))?;
        let properties = crate::module_blob::parse_simple_metadata_xml_properties(xml)
            .map_err(|error| SourceChangeError::Io(error.to_string()))?;
        let kind = crate::module_blob::parse_template_type_from_xml(xml)
            .map_err(|error| SourceChangeError::Io(error.to_string()))?;
        let expected = match parts[0] {
            "Reports" => "SpreadsheetDocument",
            "DataProcessors" => "HTMLDocument",
            "ExchangePlans" => "TextDocument",
            _ => unreachable!("measured_template_dir restricts parent kinds"),
        };
        if properties.kind != "Template" || kind.as_deref() != Some(expected) {
            return Err(SourceChangeError::UnsupportedSourcePath(path.to_owned()));
        }
        let body = format!(
            "{template_dir}/Ext/Template.{}",
            if expected == "TextDocument" {
                "txt"
            } else {
                "xml"
            }
        );
        require_existing(inventory, &body, "template body")?;
        let mut source_paths = vec![inventory.file(&body)?.unwrap().path.clone()];
        if expected == "HTMLDocument" {
            let prefix = windows_path_key(&format!("{template_dir}/Ext/Template/"));
            source_paths.extend(
                inventory
                    .files()
                    .filter(|file| windows_path_key(file.path()).starts_with(&prefix))
                    .map(|file| file.path.clone()),
            );
        }
        if !source_paths
            .iter()
            .any(|candidate| paths_equal_text_windows(candidate, path))
        {
            return Err(SourceChangeError::UnsupportedSourcePath(path.to_owned()));
        }
        source_paths.sort_by_key(|path| windows_path_key(path));
        return Ok(SourceBodyKind::Template {
            template_descriptor: file.path.clone(),
            source_paths,
        });
    }
    if let Some(form_dir_len) = managed_form_dir_len(&parts) {
        let form_dir = parts[..form_dir_len].join("/");
        let descriptor = format!("{form_dir}.xml");
        let form_body = format!("{form_dir}/Ext/Form.xml");
        if form_dir_len == 4 {
            require_existing(
                inventory,
                &format!("{}/{}.xml", parts[0], parts[1]),
                "top-level metadata owner descriptor",
            )?;
        }
        require_existing(inventory, &descriptor, "managed-form descriptor")?;
        require_existing(inventory, &form_body, "managed-form body")?;
        let module = format!("{form_dir}/Ext/Form/Module.bsl");
        let form_module = inventory.file(&module)?.map(|file| file.path.clone());
        return Ok(SourceBodyKind::ManagedForm {
            form_descriptor: inventory
                .file(&descriptor)?
                .expect("required descriptor exists")
                .path
                .clone(),
            form_body: inventory
                .file(&form_body)?
                .expect("required body exists")
                .path
                .clone(),
            form_module,
        });
    }

    let Some((owner_parts, file_name)) = generic_module_owner(&parts) else {
        return Err(SourceChangeError::UnsupportedSourcePath(path.to_owned()));
    };
    if !module_owner_is_supported(owner_parts, file_name) {
        return Err(SourceChangeError::UnsupportedSourcePath(path.to_owned()));
    }
    let descriptor = if owner_parts.is_empty() {
        "Configuration.xml".to_owned()
    } else {
        format!("{}.xml", owner_parts.join("/"))
    };
    if owner_parts.len() == 4 {
        require_existing(
            inventory,
            &format!("{}/{}.xml", owner_parts[0], owner_parts[1]),
            "top-level metadata owner descriptor",
        )?;
    }
    require_existing(inventory, &descriptor, "module owner descriptor")?;
    Ok(SourceBodyKind::Module {
        owner_descriptor: inventory
            .file(&descriptor)?
            .expect("required descriptor exists")
            .path
            .clone(),
    })
}

/// Closed source projection of the three parent/type pairs measured by the
/// native C1 control. Other template kinds and nested forms remain unmeasured.
fn measured_template_dir(parts: &[&str]) -> Option<String> {
    (parts.len() >= 6
        && matches!(parts[0], "Reports" | "DataProcessors" | "ExchangePlans")
        && parts[2] == "Templates"
        && parts[4] == "Ext"
        && (parts.len() == 6 && matches!(parts[5], "Template.xml" | "Template.txt")
            || parts.len() >= 7 && parts[5] == "Template"))
        .then(|| parts[..4].join("/"))
}

/// Source paths whose owner/body layouts have native lifetime evidence on
/// 8.3.27.2214. This is an early route check, not ownership or codec admission.
pub(crate) fn measured_main_source_path(path: &str) -> bool {
    let parts = path.split('/').collect::<Vec<_>>();
    if measured_template_dir(&parts).is_some() {
        return true;
    }
    if managed_form_dir_len(&parts) == Some(2) {
        return true;
    }
    let Some((owner, file)) = generic_module_owner(&parts) else {
        return false;
    };
    let [collection, _] = owner else {
        return false;
    };
    match file {
        "Module.bsl" => *collection == "CommonModules",
        "ObjectModule.bsl" => matches!(
            *collection,
            "Catalogs"
                | "Documents"
                | "Reports"
                | "DataProcessors"
                | "ExchangePlans"
                | "Tasks"
                | "BusinessProcesses"
                | "ChartsOfAccounts"
                | "ChartsOfCalculationTypes"
                | "ChartsOfCharacteristicTypes"
        ),
        "ManagerModule.bsl" => matches!(
            *collection,
            "Catalogs"
                | "Documents"
                | "Reports"
                | "DataProcessors"
                | "ExchangePlans"
                | "Tasks"
                | "BusinessProcesses"
                | "ChartsOfAccounts"
                | "ChartsOfCalculationTypes"
                | "ChartsOfCharacteristicTypes"
                | "Enums"
                | "Constants"
                | "SettingsStorages"
                | "DocumentJournals"
                | "InformationRegisters"
                | "AccumulationRegisters"
        ),
        _ => false,
    }
}

fn managed_form_dir_len(parts: &[&str]) -> Option<usize> {
    let is_body = parts.ends_with(&["Ext", "Form.xml"]);
    let is_module = parts.ends_with(&["Ext", "Form", "Module.bsl"]);
    let suffix_len = if is_body {
        2
    } else if is_module {
        3
    } else {
        return None;
    };
    let form_dir_len = parts.len().checked_sub(suffix_len)?;
    if form_dir_len == 0 {
        return None;
    }
    let is_common_form = form_dir_len == 2 && parts[0] == "CommonForms";
    let is_owned_form =
        form_dir_len == 4 && parts[2] == "Forms" && form_owner_collection_is_supported(parts[0]);
    (is_common_form || is_owned_form).then_some(form_dir_len)
}

fn form_owner_collection_is_supported(collection: &str) -> bool {
    matches!(
        collection,
        "Catalogs"
            | "Documents"
            | "DocumentJournals"
            | "Enums"
            | "Reports"
            | "DataProcessors"
            | "ExchangePlans"
            | "BusinessProcesses"
            | "Tasks"
            | "SettingsStorages"
            | "FilterCriteria"
            | "InformationRegisters"
            | "AccumulationRegisters"
            | "AccountingRegisters"
            | "CalculationRegisters"
            | "ChartsOfAccounts"
            | "ChartsOfCalculationTypes"
            | "ChartsOfCharacteristicTypes"
    )
}

/// Exact source-folder projection of the 14 owner kinds evidenced by
/// `COMMAND_COLLECTION_LIST_MARKERS` in `mssql_dump::refs`.
fn command_owner_collection_is_supported(collection: &str) -> bool {
    matches!(
        collection,
        "DataProcessors"
            | "Catalogs"
            | "Documents"
            | "InformationRegisters"
            | "Reports"
            | "DocumentJournals"
            | "ExchangePlans"
            | "BusinessProcesses"
            | "Tasks"
            | "ChartsOfAccounts"
            | "FilterCriteria"
            | "ChartsOfCharacteristicTypes"
            | "AccountingRegisters"
            | "AccumulationRegisters"
    )
}

fn generic_module_owner<'a>(parts: &'a [&'a str]) -> Option<(&'a [&'a str], &'a str)> {
    if parts.len() < 2 || parts[parts.len() - 2] != "Ext" {
        return None;
    }
    let file_name = *parts.last()?;
    const MODULE_FILES: &[&str] = &[
        "Module.bsl",
        "OrdinaryApplicationModule.bsl",
        "ExternalConnectionModule.bsl",
        "ManagedApplicationModule.bsl",
        "SessionModule.bsl",
        "CommandModule.bsl",
        "ManagerModule.bsl",
        "ValueManagerModule.bsl",
        "RecordSetModule.bsl",
        "ObjectModule.bsl",
    ];
    MODULE_FILES
        .contains(&file_name)
        .then_some((&parts[..parts.len() - 2], file_name))
}

fn module_owner_is_supported(owner: &[&str], file_name: &str) -> bool {
    if owner.is_empty() {
        return matches!(
            file_name,
            "OrdinaryApplicationModule.bsl"
                | "ExternalConnectionModule.bsl"
                | "ManagedApplicationModule.bsl"
                | "SessionModule.bsl"
        );
    }
    let (collection, is_top_level_owner, is_owned_command) = match owner {
        [collection, _name] => (Some(*collection), true, false),
        [collection, _owner, "Commands", _command] => (Some(*collection), false, true),
        _ => return false,
    };
    match file_name {
        "Module.bsl" => matches!(
            (collection, is_top_level_owner),
            (
                Some(
                    "CommonModules"
                        | "HTTPServices"
                        | "WebServices"
                        | "Bots"
                        | "IntegrationServices"
                ),
                true
            )
        ),
        "CommandModule.bsl" => {
            (is_top_level_owner && collection == Some("CommonCommands"))
                || (is_owned_command
                    && collection.is_some_and(command_owner_collection_is_supported))
        }
        "ValueManagerModule.bsl" => is_top_level_owner && collection == Some("Constants"),
        "RecordSetModule.bsl" => matches!(
            (collection, is_top_level_owner),
            (
                Some(
                    "Sequences"
                        | "AccountingRegisters"
                        | "AccumulationRegisters"
                        | "CalculationRegisters"
                        | "InformationRegisters"
                ),
                true
            )
        ),
        "ObjectModule.bsl" => matches!(
            (collection, is_top_level_owner),
            (
                Some(
                    "Catalogs"
                        | "Reports"
                        | "DataProcessors"
                        | "Documents"
                        | "ExchangePlans"
                        | "Tasks"
                        | "BusinessProcesses"
                        | "ChartsOfAccounts"
                        | "ChartsOfCalculationTypes"
                        | "ChartsOfCharacteristicTypes"
                ),
                true
            )
        ),
        "ManagerModule.bsl" => matches!(
            (collection, is_top_level_owner),
            (
                Some(
                    "FilterCriteria"
                        | "Constants"
                        | "SettingsStorages"
                        | "Catalogs"
                        | "Reports"
                        | "DataProcessors"
                        | "Documents"
                        | "Enums"
                        | "ExchangePlans"
                        | "AccountingRegisters"
                        | "AccumulationRegisters"
                        | "CalculationRegisters"
                        | "InformationRegisters"
                        | "DocumentJournals"
                        | "Tasks"
                        | "BusinessProcesses"
                        | "ChartsOfAccounts"
                        | "ChartsOfCalculationTypes"
                        | "ChartsOfCharacteristicTypes"
                ),
                true
            )
        ),
        _ => false,
    }
}

fn dependency_closure(body: &SourceBodyKind, selected: &str) -> BTreeSet<String> {
    match body {
        SourceBodyKind::Module { .. } => BTreeSet::from([windows_path_key(selected)]),
        SourceBodyKind::Template { source_paths, .. } => source_paths
            .iter()
            .map(|path| windows_path_key(path))
            .collect(),
        SourceBodyKind::ManagedForm {
            form_body,
            form_module,
            ..
        } => {
            let mut closure = BTreeSet::from([windows_path_key(form_body)]);
            if let Some(module) = form_module {
                closure.insert(windows_path_key(module));
            }
            closure
        }
    }
}

pub(crate) fn source_body_closure_paths(
    inventory: &SourceInventory,
    selected: &str,
) -> Result<Vec<String>, SourceChangeError> {
    let selected = inventory
        .file(selected)?
        .ok_or_else(|| SourceChangeError::SelectedBodyDoesNotExist(selected.to_owned()))?;
    let body = classify_supported_body(selected.path(), inventory)?;
    Ok(dependency_closure(&body, selected.path())
        .iter()
        .map(|key| inventory.by_windows_key[key].path.clone())
        .collect())
}

fn require_existing(
    inventory: &SourceInventory,
    path: &str,
    role: &'static str,
) -> Result<(), SourceChangeError> {
    if inventory.file(path)?.is_none() {
        return Err(SourceChangeError::MissingRequiredPeer {
            role,
            path: path.to_owned(),
        });
    }
    Ok(())
}

fn capture_inventory(
    anchor: &Arc<RootAnchor>,
    admission: CensusAdmission,
    retain_bytes: &BTreeSet<String>,
) -> Result<SourceInventory, SourceChangeError> {
    let mut files = Vec::new();
    let mut total = 0_u64;
    let mut identities = BTreeSet::new();
    let mut directory_spellings = BTreeMap::<String, String>::new();
    for entry in WalkDir::new(&anchor.path).follow_links(false) {
        let entry = entry.map_err(|error| SourceChangeError::Io(error.to_string()))?;
        if entry.path() == anchor.path {
            continue;
        }
        if entry.file_type().is_symlink() || path_has_reparse_attribute(entry.path())? {
            return Err(SourceChangeError::LinkOrReparsePoint(
                entry.path().display().to_string(),
            ));
        }
        if entry.file_type().is_dir() {
            let relative = path_to_slash(
                entry
                    .path()
                    .strip_prefix(&anchor.path)
                    .map_err(|error| SourceChangeError::Io(error.to_string()))?,
            )?;
            let key = windows_path_key(&relative);
            if let Some(first) = directory_spellings.insert(key, relative.clone()) {
                return Err(SourceChangeError::WindowsPathCollision {
                    first,
                    second: relative,
                });
            }
            continue;
        }
        if !entry.file_type().is_file() {
            return Err(SourceChangeError::Io("nonregular source member".to_owned()));
        }
        let relative = path_to_slash(
            entry
                .path()
                .strip_prefix(&anchor.path)
                .map_err(|e| SourceChangeError::Io(e.to_string()))?,
        )?;
        let original = Arc::new(anchor.open_file(&relative)?);
        total = total
            .checked_add(original.length())
            .ok_or(SourceChangeError::InventoryLimit("total byte overflow"))?;
        let count = files
            .len()
            .checked_add(1)
            .ok_or(SourceChangeError::InventoryLimit("file count overflow"))?;
        admission.check(count, original.length(), total)?;
        if !identities.insert(original.identity()) {
            return Err(SourceChangeError::Io(
                "duplicate physical source identity".to_owned(),
            ));
        }
        let key = windows_path_key(&relative);
        let retain = retain_bytes.contains(&key)
            || retain_bytes
                .iter()
                .any(|prefix| prefix.ends_with('/') && key.starts_with(prefix));
        let (digest, retained) = original.read(retain)?;
        let mut source = SourceFileDigest::new(relative, original.length(), digest)?;
        source.identity = Some(original.identity());
        source.verified_bytes = retained;
        // Census-only handles are dropped promptly; selected normal-operation
        // originals retain their shared immutable bytes and ancestors.
        if retain && matches!(admission, CensusAdmission::OriginalSource) {
            source.original = Some(original);
        }
        files
            .try_reserve(1)
            .map_err(|e| SourceChangeError::Io(e.to_string()))?;
        files.push(source);
    }
    anchor.require_path()?;
    SourceInventory::from_files(files)
}

fn resolve_selected_path(root: &Path, selected: &Path) -> Result<String, SourceChangeError> {
    if selected.is_absolute() {
        return Err(SourceChangeError::InvalidRelativePath(
            selected.display().to_string(),
        ));
    }
    let lexical = path_to_slash(selected)?;
    let canonical = fs::canonicalize(root.join(selected))
        .map_err(|error| SourceChangeError::Io(error.to_string()))?;
    if !canonical.is_file() || !path_is_within_windows(&canonical, root) {
        return Err(SourceChangeError::PathEscapesRoot(
            selected.display().to_string(),
        ));
    }
    let resolved = canonical
        .strip_prefix(root)
        .map_err(|_| SourceChangeError::PathEscapesRoot(selected.display().to_string()))?;
    let resolved = path_to_slash(resolved)?;
    if !paths_equal_text_windows(&lexical, &resolved) {
        return Err(SourceChangeError::WindowsAliasMismatch {
            requested: lexical,
            actual: resolved,
        });
    }
    Ok(resolved)
}

fn canonical_directory(path: &Path) -> Result<PathBuf, SourceChangeError> {
    // A canonical target obtained through a link is not an admitted original
    // spelling. Check each existing ancestor before resolving the name.
    for ancestor in path.ancestors().filter(|p| !p.as_os_str().is_empty()) {
        if path_has_reparse_attribute(ancestor)? {
            return Err(SourceChangeError::LinkOrReparsePoint(
                ancestor.display().to_string(),
            ));
        }
    }
    if path_has_reparse_attribute(path)? {
        return Err(SourceChangeError::LinkOrReparsePoint(
            path.display().to_string(),
        ));
    }
    let canonical =
        fs::canonicalize(path).map_err(|error| SourceChangeError::Io(error.to_string()))?;
    if !canonical.is_dir() {
        return Err(SourceChangeError::SourceRootIsNotDirectory);
    }
    Ok(canonical)
}

#[cfg(windows)]
fn path_has_reparse_attribute(path: &Path) -> Result<bool, SourceChangeError> {
    use std::os::windows::fs::MetadataExt;
    use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;

    let metadata =
        fs::symlink_metadata(path).map_err(|error| SourceChangeError::Io(error.to_string()))?;
    Ok(metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0)
}

#[cfg(not(windows))]
fn path_has_reparse_attribute(path: &Path) -> Result<bool, SourceChangeError> {
    let metadata =
        fs::symlink_metadata(path).map_err(|error| SourceChangeError::Io(error.to_string()))?;
    Ok(metadata.file_type().is_symlink())
}

fn validate_limits(limits: SourceInventoryLimits) -> Result<(), SourceChangeError> {
    if limits.max_files == 0 || limits.max_file_bytes == 0 || limits.max_total_bytes == 0 {
        return Err(SourceChangeError::InvalidInventoryLimits);
    }
    Ok(())
}

fn validate_target(target: &ActivationTarget) -> Result<(), SourceChangeError> {
    if let ActivationTarget::Extension { name } = target {
        ActivationTarget::extension(name.clone())?;
    }
    Ok(())
}

fn path_to_slash(path: &Path) -> Result<String, SourceChangeError> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => parts.push(
                value
                    .to_str()
                    .ok_or(SourceChangeError::NonUnicodePath)?
                    .to_owned(),
            ),
            _ => {
                return Err(SourceChangeError::InvalidRelativePath(
                    path.display().to_string(),
                ));
            }
        }
    }
    normalize_relative_path(&parts.join("/"))
}

fn normalize_relative_path(path: &str) -> Result<String, SourceChangeError> {
    if path.is_empty() || path.starts_with(['/', '\\']) || path.as_bytes().get(1) == Some(&b':') {
        return Err(SourceChangeError::InvalidRelativePath(path.to_owned()));
    }
    let replaced = path.replace('\\', "/");
    let mut normalized = Vec::new();
    for component in replaced.split('/') {
        if component.is_empty() || component == "." || component == ".." {
            return Err(SourceChangeError::InvalidRelativePath(path.to_owned()));
        }
        validate_windows_component(component)?;
        normalized.push(component);
    }
    Ok(normalized.join("/"))
}

fn validate_windows_component(component: &str) -> Result<(), SourceChangeError> {
    if component.ends_with([' ', '.'])
        || component.contains(':')
        || component
            .chars()
            .any(|character| character.is_control() || "<>\"|?*".contains(character))
    {
        return Err(SourceChangeError::UnsafeWindowsComponent(
            component.to_owned(),
        ));
    }
    let stem = component
        .split('.')
        .next()
        .unwrap_or(component)
        .to_ascii_lowercase();
    let looks_like_dos_alias = stem.rsplit_once('~').is_some_and(|(prefix, ordinal)| {
        !prefix.is_empty()
            && prefix.len() <= 6
            && !ordinal.is_empty()
            && ordinal.len() <= 6
            && ordinal.bytes().all(|byte| byte.is_ascii_digit())
    });
    let reserved = matches!(stem.as_str(), "con" | "prn" | "aux" | "nul")
        || stem
            .strip_prefix("com")
            .or_else(|| stem.strip_prefix("lpt"))
            .is_some_and(|number| {
                matches!(number, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
            })
        || ["com¹", "com²", "com³", "lpt¹", "lpt²", "lpt³"].contains(&stem.as_str());
    if reserved || looks_like_dos_alias {
        return Err(SourceChangeError::UnsafeWindowsComponent(
            component.to_owned(),
        ));
    }
    Ok(())
}

/// The one case-insensitive key every path comparison of this module uses.
/// NTFS folds case for every script, not only ASCII: `Справочники` and
/// `СПРАВОЧНИКИ` name one folder, so an ASCII-only fold would call the held
/// root changed or a Cyrillic file outside it (Untru/ibcmd-rs#409, F-14).
fn windows_path_key(path: &str) -> String {
    path.chars().flat_map(char::to_lowercase).collect()
}

fn paths_equal_text_windows(left: &str, right: &str) -> bool {
    windows_path_key(left) == windows_path_key(right)
}

#[cfg(any(windows, test))]
fn paths_equal_windows(left: &Path, right: &Path) -> bool {
    left.to_str()
        .zip(right.to_str())
        .is_some_and(|(left, right)| paths_equal_text_windows(left, right))
}

// Physical roots follow the host's path rules, independently of the Windows
// names used to match metadata in an inventory. Case folding on Unix could
// accept an ancestor link redirected to a different, case-distinct directory.
fn held_root_paths_equal(left: &Path, right: &Path) -> bool {
    #[cfg(windows)]
    {
        paths_equal_windows(left, right)
    }
    #[cfg(not(windows))]
    {
        left == right
    }
}

fn path_is_within_windows(candidate: &Path, root: &Path) -> bool {
    let candidate = candidate.components().collect::<Vec<_>>();
    let root = root.components().collect::<Vec<_>>();
    candidate.len() >= root.len()
        && candidate.iter().zip(root.iter()).all(|(left, right)| {
            left.as_os_str()
                .to_str()
                .zip(right.as_os_str().to_str())
                .is_some_and(|(left, right)| paths_equal_text_windows(left, right))
        })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceChangeError {
    InvalidInventoryLimits,
    SourceRootIsNotDirectory,
    NonUnicodePath,
    InvalidRelativePath(String),
    UnsafeWindowsComponent(String),
    WindowsPathCollision { first: String, second: String },
    WindowsAliasMismatch { requested: String, actual: String },
    HeldRootChanged,
    PathEscapesRoot(String),
    LinkOrReparsePoint(String),
    InventoryLimit(&'static str),
    FileChangedDuringRead(String),
    InvalidExtensionName,
    SelectedBodyDoesNotExist(String),
    SelectedBodyWasRemoved(String),
    UnsupportedSourcePath(String),
    MissingRequiredPeer { role: &'static str, path: String },
    UnverifiedSourceBytes(String),
    SourceTreeShapeChanged { additions: usize, removals: usize },
    ChangesOutsideClosure(Vec<String>),
    SelectedBodyIsNotChanged,
    Io(String),
}

impl Display for SourceChangeError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInventoryLimits => {
                formatter.write_str("source inventory limits must be positive")
            }
            Self::SourceRootIsNotDirectory => formatter.write_str("source root is not a directory"),
            Self::NonUnicodePath => formatter.write_str("source paths must be valid Unicode"),
            Self::InvalidRelativePath(path) => {
                write!(formatter, "invalid relative source path `{path}`")
            }
            Self::UnsafeWindowsComponent(value) => {
                write!(formatter, "unsafe Windows path component `{value}`")
            }
            Self::WindowsPathCollision { first, second } => write!(
                formatter,
                "Windows path collision between `{first}` and `{second}`"
            ),
            Self::WindowsAliasMismatch { requested, actual } => write!(
                formatter,
                "source path alias mismatch: requested `{requested}`, resolved `{actual}`"
            ),
            Self::HeldRootChanged => {
                formatter.write_str("held source root changed since the baseline capture")
            }
            Self::PathEscapesRoot(path) => {
                write!(formatter, "source path escapes the held root: `{path}`")
            }
            Self::LinkOrReparsePoint(path) => write!(
                formatter,
                "link or reparse point is not allowed in the source tree: `{path}`"
            ),
            Self::InventoryLimit(limit) => {
                write!(formatter, "source inventory exceeds the {limit} limit")
            }
            Self::FileChangedDuringRead(path) => {
                write!(formatter, "source file changed while it was read: `{path}`")
            }
            Self::InvalidExtensionName => formatter.write_str(
                "extension name is empty, padded, unsafe, or longer than 128 UTF-16 units",
            ),
            Self::SelectedBodyDoesNotExist(path) => write!(
                formatter,
                "selected body does not exist in the active inventory: `{path}`"
            ),
            Self::SelectedBodyWasRemoved(path) => {
                write!(formatter, "selected body was removed: `{path}`")
            }
            Self::UnsupportedSourcePath(path) => write!(
                formatter,
                "source path is not a supported existing module or managed-form body: `{path}`"
            ),
            Self::MissingRequiredPeer { role, path } => {
                write!(formatter, "missing existing {role} `{path}`")
            }
            Self::UnverifiedSourceBytes(path) => write!(
                formatter,
                "proposed source bytes are absent or do not match the verified digest: `{path}`"
            ),
            Self::SourceTreeShapeChanged {
                additions,
                removals,
            } => write!(
                formatter,
                "source tree shape changed ({additions} additions, {removals} removals)"
            ),
            Self::ChangesOutsideClosure(paths) => write!(
                formatter,
                "changes outside the selected dependency closure: {}",
                paths.join(", ")
            ),
            Self::SelectedBodyIsNotChanged => {
                formatter.write_str("another closure member changed, but the selected body did not")
            }
            Self::Io(message) => write!(formatter, "source filesystem error: {message}"),
        }
    }
}

impl Error for SourceChangeError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    #[test]
    fn held_root_refuses_case_distinct_ancestor_link_retargeting() {
        use std::os::unix::fs::symlink;

        let root =
            std::env::temp_dir().join(format!("ibcmd-rs-held-root-{}", uuid::Uuid::new_v4()));
        let first = root.join("Выгрузка");
        let second = root.join("ВЫГРУЗКА-target");
        fs::create_dir_all(first.join("sources")).unwrap();
        fs::create_dir_all(second.join("sources")).unwrap();
        fs::write(first.join("sources/Module.bsl"), b"first").unwrap();
        fs::write(second.join("sources/Module.bsl"), b"second").unwrap();
        let link = root.join("selected");
        symlink(&first, &link).unwrap();
        assert!(matches!(
            HeldSourceRoot::open(&link.join("sources"), SourceInventoryLimits::default()),
            Err(SourceChangeError::LinkOrReparsePoint(_))
        ));
        fs::remove_file(&link).unwrap();
        let held =
            HeldSourceRoot::open(&first.join("sources"), SourceInventoryLimits::default()).unwrap();
        // The original directory remains alive, but its admitted pathname is
        // redirected to another case-distinct ancestor. Never adopt that target.
        fs::rename(&first, root.join("original-directory")).unwrap();
        symlink(&second, &first).unwrap();
        assert!(matches!(
            held.capture_current(),
            Err(SourceChangeError::HeldRootChanged)
        ));
        assert!(matches!(
            held.classify_current(
                Path::new("Module.bsl"),
                ActivationTarget::Main,
                ActivationMode::Online
            ),
            Err(SourceChangeError::HeldRootChanged)
        ));
        drop(held);
        fs::remove_file(&first).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn every_path_comparison_folds_cyrillic_case() {
        assert!(paths_equal_text_windows(
            "Справочники/Банки",
            "СПРАВОЧНИКИ/банки"
        ));
        assert!(paths_equal_windows(
            Path::new("Справочники"),
            Path::new("СПРАВОЧНИКИ")
        ));
        assert!(path_is_within_windows(
            &Path::new("Выгрузка")
                .join("CommonModules")
                .join("Общий.xml"),
            Path::new("ВЫГРУЗКА"),
        ));
        assert!(!path_is_within_windows(
            &Path::new("Выгрузка2").join("Общий.xml"),
            Path::new("Выгрузка"),
        ));
    }

    fn file(path: &str, value: &str) -> SourceFileDigest {
        SourceFileDigest::for_bytes(path, value.as_bytes()).unwrap()
    }

    fn inventory(files: &[(&str, &str)]) -> SourceInventory {
        SourceInventory::from_files(
            files
                .iter()
                .map(|(path, value)| file(path, value))
                .collect(),
        )
        .unwrap()
    }

    #[test]
    fn xml_formatting_is_not_a_source_change() {
        let active = file(
            "CommonForms/Demo/Ext/Form.xml",
            "<Form><Title>Demo</Title></Form>",
        );
        let proposed = file(
            "CommonForms/Demo/Ext/Form.xml",
            "<?xml version=\"1.0\"?>\n<Form>\n  <Title>Demo</Title>\n</Form>\n",
        );
        assert!(source_files_equal(&active, &proposed).unwrap());
    }

    #[test]
    fn xml_value_change_remains_a_source_change() {
        let active = file(
            "CommonForms/Demo/Ext/Form.xml",
            "<Form><Title>Before</Title></Form>",
        );
        let proposed = file(
            "CommonForms/Demo/Ext/Form.xml",
            "<Form><Title>After</Title></Form>",
        );
        assert!(!source_files_equal(&active, &proposed).unwrap());
    }

    fn common_module(value: &str) -> SourceInventory {
        inventory(&[
            (
                "Configuration.xml",
                "<Configuration><Name>root</Name></Configuration>",
            ),
            ("CommonModules/Work.xml", "metadata"),
            ("CommonModules/Work/Ext/Module.bsl", value),
        ])
    }

    #[test]
    fn measured_main_modules_cover_exact_native_d1_roles() {
        let object = [
            "Catalogs",
            "Documents",
            "Reports",
            "DataProcessors",
            "ExchangePlans",
            "Tasks",
            "BusinessProcesses",
            "ChartsOfAccounts",
            "ChartsOfCalculationTypes",
            "ChartsOfCharacteristicTypes",
        ];
        let manager_only = [
            "Enums",
            "Constants",
            "SettingsStorages",
            "DocumentJournals",
            "InformationRegisters",
            "AccumulationRegisters",
        ];
        let mut roles = 0;
        for (collection, file) in object
            .iter()
            .flat_map(|collection| {
                [
                    (*collection, "ObjectModule.bsl"),
                    (*collection, "ManagerModule.bsl"),
                ]
            })
            .chain(
                manager_only
                    .iter()
                    .map(|collection| (*collection, "ManagerModule.bsl")),
            )
        {
            let owner = format!("{collection}/Owner.xml");
            let selected = format!("{collection}/Owner/Ext/{file}");
            assert!(measured_main_source_path(&selected));
            let active = inventory(&[(&owner, "unchanged metadata"), (&selected, "old")]);
            let proposed = inventory(&[(&owner, "unchanged metadata"), (&selected, "new")]);
            let plan = classify_source_change(
                &active,
                &proposed,
                &selected,
                ActivationTarget::Main,
                ActivationMode::Online,
            )
            .unwrap();
            assert_eq!(plan.changed_paths(), &[selected.clone()]);
            assert_eq!(plan.verified_sources().len(), 1);
            roles += 1;
        }
        assert_eq!(roles, 26);
        for selected in [
            "Constants/Owner/Ext/ObjectModule.bsl",
            "FilterCriteria/Owner/Ext/ManagerModule.bsl",
            "AccountingRegisters/Owner/Ext/ManagerModule.bsl",
            "InformationRegisters/Owner/Ext/RecordSetModule.bsl",
            "Catalogs/Owner/Forms/Card/Ext/Form.xml",
            "CommonModules/X/Extra/Ext/Module.bsl",
        ] {
            assert!(!measured_main_source_path(selected), "{selected}");
        }
    }

    fn template_inventory(
        collection: &str,
        kind: &str,
        body: &str,
        resource: &str,
        owner: &str,
    ) -> SourceInventory {
        let descriptor = format!(
            "<MetaDataObject><Template uuid=\"00000000-0000-4000-8000-000000000001\"><Properties><Name>Main</Name><TemplateType>{kind}</TemplateType></Properties></Template></MetaDataObject>"
        );
        let mut files = vec![
            file(
                &format!("{collection}/Owner.xml"),
                &format!("<Owner><Value>{owner}</Value></Owner>"),
            ),
            file(
                &format!("{collection}/Owner/Templates/Main.xml"),
                &descriptor,
            ),
            file(
                &format!(
                    "{collection}/Owner/Templates/Main/Ext/Template.{}",
                    if kind == "TextDocument" { "txt" } else { "xml" }
                ),
                body,
            ),
        ];
        if kind == "HTMLDocument" {
            files.push(file(
                &format!("{collection}/Owner/Templates/Main/Ext/Template/ru.html"),
                resource,
            ));
            files.push(file(
                &format!("{collection}/Owner/Templates/Main/Ext/Template/logo.png"),
                "same resource",
            ));
        }
        SourceInventory::from_files(files).unwrap()
    }

    #[test]
    fn template_closure_admits_only_native_c1_parent_type_pairs() {
        for (collection, kind, suffix) in [
            ("Reports", "SpreadsheetDocument", "xml"),
            ("ExchangePlans", "TextDocument", "txt"),
            ("DataProcessors", "HTMLDocument", "xml"),
        ] {
            let old = if suffix == "xml" {
                "<document><value>old</value></document>"
            } else {
                "old"
            };
            let new = if suffix == "xml" {
                "<document><value>new</value></document>"
            } else {
                "new"
            };
            let active = template_inventory(collection, kind, old, "old page", "owner");
            let proposed = template_inventory(collection, kind, new, "new page", "owner");
            let selected = format!("{collection}/Owner/Templates/Main/Ext/Template.{suffix}");
            let plan = classify_source_change(
                &active,
                &proposed,
                &selected,
                ActivationTarget::Main,
                ActivationMode::Online,
            )
            .unwrap();
            assert!(matches!(plan.body(), SourceBodyKind::Template { .. }));
            assert_eq!(
                plan.verified_sources().len(),
                if kind == "HTMLDocument" { 3 } else { 1 }
            );
        }
        for (collection, kind) in [
            ("Reports", "TextDocument"),
            ("DataProcessors", "SpreadsheetDocument"),
            ("ExchangePlans", "HTMLDocument"),
            ("Catalogs", "SpreadsheetDocument"),
            ("Reports", "DataCompositionSchema"),
        ] {
            let tree = template_inventory(collection, kind, "<document/>", "page", "owner");
            let selected = format!(
                "{collection}/Owner/Templates/Main/Ext/Template.{}",
                if kind == "TextDocument" { "txt" } else { "xml" }
            );
            assert!(matches!(
                classify_source_change(
                    &tree,
                    &tree,
                    &selected,
                    ActivationTarget::Main,
                    ActivationMode::Online
                ),
                Err(SourceChangeError::UnsupportedSourcePath(_))
            ));
        }
    }

    #[test]
    fn html_resource_edit_pins_whole_existing_asset_and_refuses_owner_drift() {
        let selected = "DataProcessors/Owner/Templates/Main/Ext/Template/ru.html";
        let active = template_inventory(
            "DataProcessors",
            "HTMLDocument",
            "<Template/>",
            "old",
            "owner",
        );
        let proposed = template_inventory(
            "DataProcessors",
            "HTMLDocument",
            "<Template/>",
            "new",
            "owner",
        );
        let plan = classify_source_change(
            &active,
            &proposed,
            selected,
            ActivationTarget::Main,
            ActivationMode::Online,
        )
        .unwrap();
        assert_eq!(plan.changed_paths(), &[selected]);
        assert_eq!(plan.verified_sources().len(), 3);
        assert!(
            classify_source_change(
                &active,
                &proposed,
                "DataProcessors/Owner/Templates/Main/Ext/Template.xml",
                ActivationTarget::Main,
                ActivationMode::Online
            )
            .is_ok()
        );
        assert!(
            plan.verified_source("DataProcessors/Owner/Templates/Main/Ext/Template/logo.png")
                .is_some()
        );
        let drift = template_inventory(
            "DataProcessors",
            "HTMLDocument",
            "<Template/>",
            "new",
            "changed owner",
        );
        assert!(matches!(
            classify_source_change(
                &active,
                &drift,
                selected,
                ActivationTarget::Main,
                ActivationMode::Online
            ),
            Err(SourceChangeError::ChangesOutsideClosure(_))
        ));
        let missing = SourceInventory::from_files(
            proposed
                .files()
                .filter(|file| !file.path().ends_with("logo.png"))
                .cloned()
                .collect(),
        )
        .unwrap();
        assert!(matches!(
            classify_source_change(
                &active,
                &missing,
                selected,
                ActivationTarget::Main,
                ActivationMode::Online
            ),
            Err(SourceChangeError::SourceTreeShapeChanged { .. })
        ));
        let retained = candidate_retention_paths(selected);
        assert!(retained.contains(&windows_path_key("DataProcessors/Owner/Templates/Main.xml")));
        assert!(retained.contains(&windows_path_key(
            "DataProcessors/Owner/Templates/Main/Ext/Template/"
        )));
    }

    #[test]
    fn measured_object_module_edit_refuses_sibling_and_descriptor_changes() {
        let selected = "Catalogs/Owner/Ext/ObjectModule.bsl";
        let active = inventory(&[
            (
                "Catalogs/Owner.xml",
                "<Owner><Value>descriptor</Value></Owner>",
            ),
            (selected, "old"),
            ("Catalogs/Owner/Ext/ManagerModule.bsl", "manager"),
        ]);
        for proposed in [
            inventory(&[
                (
                    "Catalogs/Owner.xml",
                    "<Owner><Value>descriptor</Value></Owner>",
                ),
                (selected, "new"),
                ("Catalogs/Owner/Ext/ManagerModule.bsl", "changed manager"),
            ]),
            inventory(&[
                (
                    "Catalogs/Owner.xml",
                    "<Owner><Value>changed descriptor</Value></Owner>",
                ),
                (selected, "new"),
                ("Catalogs/Owner/Ext/ManagerModule.bsl", "manager"),
            ]),
        ] {
            assert!(matches!(
                classify_source_change(
                    &active,
                    &proposed,
                    selected,
                    ActivationTarget::Main,
                    ActivationMode::Online
                ),
                Err(SourceChangeError::ChangesOutsideClosure(_))
            ));
        }
    }

    fn managed_form(body: &str, module: &str) -> SourceInventory {
        inventory(&[
            ("Configuration.xml", "root"),
            ("Catalogs/Goods.xml", "metadata"),
            ("Catalogs/Goods/Forms/Card.xml", "form metadata"),
            ("Catalogs/Goods/Forms/Card/Ext/Form.xml", body),
            ("Catalogs/Goods/Forms/Card/Ext/Form/Module.bsl", module),
        ])
    }

    #[test]
    fn classifies_existing_module_for_online_main_activation() {
        let plan = classify_source_change(
            &common_module("old"),
            &common_module("new"),
            "CommonModules/Work/Ext/Module.bsl",
            ActivationTarget::Main,
            ActivationMode::Online,
        )
        .unwrap();
        assert_eq!(plan.state(), SourceChangeState::Changed);
        assert_eq!(plan.mode(), ActivationMode::Online);
        assert!(matches!(plan.target(), ActivationTarget::Main));
        assert_eq!(plan.changed_paths(), &["CommonModules/Work/Ext/Module.bsl"]);
        assert!(matches!(plan.body(), SourceBodyKind::Module { .. }));
    }

    #[test]
    fn classifies_existing_form_body_and_module_as_one_closure() {
        let plan = classify_source_change(
            &managed_form("<Form><Title>old</Title></Form>", "old module"),
            &managed_form("<Form><Title>new</Title></Form>", "new module"),
            "Catalogs/Goods/Forms/Card/Ext/Form.xml",
            ActivationTarget::extension("ServiceDesk").unwrap(),
            ActivationMode::Exclusive,
        )
        .unwrap();
        assert_eq!(plan.changed_paths().len(), 2);
        assert!(matches!(plan.target(), ActivationTarget::Extension { .. }));
        assert!(matches!(plan.body(), SourceBodyKind::ManagedForm { .. }));
    }

    #[test]
    fn managed_form_owner_map_accepts_enum_and_rejects_constant_and_sequence() {
        let active = inventory(&[
            ("Enums/Status.xml", "owner"),
            ("Enums/Status/Forms/Card.xml", "form"),
            (
                "Enums/Status/Forms/Card/Ext/Form.xml",
                "<Form><Title>old</Title></Form>",
            ),
        ]);
        let proposed = inventory(&[
            ("Enums/Status.xml", "owner"),
            ("Enums/Status/Forms/Card.xml", "form"),
            (
                "Enums/Status/Forms/Card/Ext/Form.xml",
                "<Form><Title>new</Title></Form>",
            ),
        ]);
        assert!(
            classify_source_change(
                &active,
                &proposed,
                "Enums/Status/Forms/Card/Ext/Form.xml",
                ActivationTarget::Main,
                ActivationMode::Online,
            )
            .is_ok()
        );

        for collection in ["Constants", "Sequences"] {
            let owner = format!("{collection}/Owner.xml");
            let form = format!("{collection}/Owner/Forms/Card.xml");
            let body = format!("{collection}/Owner/Forms/Card/Ext/Form.xml");
            let tree = SourceInventory::from_files(vec![
                file(&owner, "owner"),
                file(&form, "form"),
                file(&body, "body"),
            ])
            .unwrap();
            assert!(
                matches!(
                    classify_source_change(
                        &tree,
                        &tree,
                        &body,
                        ActivationTarget::Main,
                        ActivationMode::Online,
                    ),
                    Err(SourceChangeError::UnsupportedSourcePath(_))
                ),
                "{collection}"
            );
        }
    }

    #[test]
    fn unchanged_payload_is_a_successful_no_op() {
        let plan = classify_source_change(
            &common_module("same"),
            &common_module("same"),
            "CommonModules/Work/Ext/Module.bsl",
            ActivationTarget::Main,
            ActivationMode::Online,
        )
        .unwrap();
        assert!(plan.is_no_op());
        assert!(plan.changed_paths().is_empty());
        assert_eq!(
            compare_compiled_payload(b"same", b"same"),
            SourceChangeState::NoOp
        );
        assert_eq!(
            compare_compiled_payload(b"same", b"different"),
            SourceChangeState::Changed
        );
    }

    #[test]
    fn rejects_root_and_structural_metadata_xml() {
        for selected in ["Configuration.xml", "Catalogs/Goods.xml"] {
            let active = inventory(&[(selected, "old")]);
            let proposed = inventory(&[(selected, "new")]);
            assert!(matches!(
                classify_source_change(
                    &active,
                    &proposed,
                    selected,
                    ActivationTarget::Main,
                    ActivationMode::Online
                ),
                Err(SourceChangeError::UnsupportedSourcePath(_))
            ));
        }
    }

    #[test]
    fn rejects_new_or_removed_files_and_forms() {
        let active = managed_form("old", "module");
        let added = inventory(&[
            ("Configuration.xml", "root"),
            ("Catalogs/Goods.xml", "metadata"),
            ("Catalogs/Goods/Forms/Card.xml", "form metadata"),
            ("Catalogs/Goods/Forms/Card/Ext/Form.xml", "new"),
            ("Catalogs/Goods/Forms/Card/Ext/Form/Module.bsl", "module"),
            ("Catalogs/Goods/Forms/New.xml", "new form"),
        ]);
        assert!(matches!(
            classify_source_change(
                &active,
                &added,
                "Catalogs/Goods/Forms/Card/Ext/Form.xml",
                ActivationTarget::Main,
                ActivationMode::Online
            ),
            Err(SourceChangeError::SourceTreeShapeChanged {
                additions: 1,
                removals: 0
            })
        ));
        let removed = inventory(&[
            ("Configuration.xml", "root"),
            ("Catalogs/Goods.xml", "metadata"),
            ("Catalogs/Goods/Forms/Card.xml", "form metadata"),
            ("Catalogs/Goods/Forms/Card/Ext/Form.xml", "new"),
        ]);
        assert!(matches!(
            classify_source_change(
                &active,
                &removed,
                "Catalogs/Goods/Forms/Card/Ext/Form.xml",
                ActivationTarget::Main,
                ActivationMode::Online
            ),
            Err(SourceChangeError::SourceTreeShapeChanged { .. })
        ));
    }

    #[test]
    fn rejects_changes_outside_selected_module_closure() {
        let active = common_module("old");
        let proposed = inventory(&[
            (
                "Configuration.xml",
                "<Configuration><Name>changed root</Name></Configuration>",
            ),
            ("CommonModules/Work.xml", "metadata"),
            ("CommonModules/Work/Ext/Module.bsl", "new"),
        ]);
        assert!(matches!(
            classify_source_change(&active, &proposed, "CommonModules/Work/Ext/Module.bsl", ActivationTarget::Main, ActivationMode::Online),
            Err(SourceChangeError::ChangesOutsideClosure(paths)) if paths == ["Configuration.xml"]
        ));
    }

    #[test]
    fn rejects_second_module_change() {
        let active = inventory(&[
            ("Configuration.xml", "root"),
            ("CommonModules/A.xml", "meta a"),
            ("CommonModules/A/Ext/Module.bsl", "old a"),
            ("CommonModules/B.xml", "meta b"),
            ("CommonModules/B/Ext/Module.bsl", "old b"),
        ]);
        let proposed = inventory(&[
            ("Configuration.xml", "root"),
            ("CommonModules/A.xml", "meta a"),
            ("CommonModules/A/Ext/Module.bsl", "new a"),
            ("CommonModules/B.xml", "meta b"),
            ("CommonModules/B/Ext/Module.bsl", "new b"),
        ]);
        assert!(matches!(
            classify_source_change(
                &active,
                &proposed,
                "CommonModules/A/Ext/Module.bsl",
                ActivationTarget::Main,
                ActivationMode::Online
            ),
            Err(SourceChangeError::ChangesOutsideClosure(_))
        ));
    }

    #[test]
    fn rejects_form_peer_change_when_selected_file_is_unchanged() {
        assert!(matches!(
            classify_source_change(
                &managed_form("body", "old"),
                &managed_form("body", "new"),
                "Catalogs/Goods/Forms/Card/Ext/Form.xml",
                ActivationTarget::Main,
                ActivationMode::Online,
            ),
            Err(SourceChangeError::SelectedBodyIsNotChanged)
        ));
    }

    #[test]
    fn requires_existing_owner_and_form_descriptors() {
        let module = inventory(&[("CommonModules/Work/Ext/Module.bsl", "new")]);
        assert!(matches!(
            classify_source_change(
                &module,
                &module,
                "CommonModules/Work/Ext/Module.bsl",
                ActivationTarget::Main,
                ActivationMode::Online
            ),
            Err(SourceChangeError::MissingRequiredPeer { .. })
        ));
        let form = inventory(&[("CommonForms/Card/Ext/Form.xml", "body")]);
        assert!(matches!(
            classify_source_change(
                &form,
                &form,
                "CommonForms/Card/Ext/Form.xml",
                ActivationTarget::Main,
                ActivationMode::Online
            ),
            Err(SourceChangeError::MissingRequiredPeer { .. })
        ));
    }

    #[test]
    fn rejects_arbitrary_bsl_even_when_it_has_a_descriptor() {
        let tree = inventory(&[
            ("Templates/Unsafe.xml", "metadata"),
            ("Templates/Unsafe/Ext/Module.bsl", "body"),
        ]);
        assert!(matches!(
            classify_source_change(
                &tree,
                &tree,
                "Templates/Unsafe/Ext/Module.bsl",
                ActivationTarget::Main,
                ActivationMode::Online
            ),
            Err(SourceChangeError::UnsupportedSourcePath(_))
        ));
    }

    #[test]
    fn rejects_fake_nested_native_collection_names() {
        for selected in [
            "Catalogs/Goods/CommonModules/Fake/Ext/Module.bsl",
            "Catalogs/Goods/Nested/Forms/Card/Ext/Form.xml",
            "Catalogs/Goods/Nested/Commands/Run/Ext/CommandModule.bsl",
        ] {
            let descriptor = selected
                .strip_suffix("/Ext/Module.bsl")
                .or_else(|| selected.strip_suffix("/Ext/Form.xml"))
                .or_else(|| selected.strip_suffix("/Ext/CommandModule.bsl"))
                .unwrap();
            let descriptor = format!("{descriptor}.xml");
            let tree = SourceInventory::from_files(vec![
                file(selected, "body"),
                file(&descriptor, "descriptor"),
            ])
            .unwrap();
            assert!(
                matches!(
                    classify_source_change(
                        &tree,
                        &tree,
                        selected,
                        ActivationTarget::Main,
                        ActivationMode::Online,
                    ),
                    Err(SourceChangeError::UnsupportedSourcePath(_))
                ),
                "{selected}"
            );
        }
    }

    #[test]
    fn owned_command_requires_native_shape_and_both_descriptors() {
        let active = inventory(&[
            ("Catalogs/Goods.xml", "owner"),
            ("Catalogs/Goods/Commands/Recount.xml", "command"),
            (
                "Catalogs/Goods/Commands/Recount/Ext/CommandModule.bsl",
                "old",
            ),
        ]);
        let proposed = inventory(&[
            ("Catalogs/Goods.xml", "owner"),
            ("Catalogs/Goods/Commands/Recount.xml", "command"),
            (
                "Catalogs/Goods/Commands/Recount/Ext/CommandModule.bsl",
                "new",
            ),
        ]);
        assert!(
            classify_source_change(
                &active,
                &proposed,
                "Catalogs/Goods/Commands/Recount/Ext/CommandModule.bsl",
                ActivationTarget::Main,
                ActivationMode::Online,
            )
            .is_ok()
        );

        let without_owner = inventory(&[
            ("Catalogs/Goods/Commands/Recount.xml", "command"),
            (
                "Catalogs/Goods/Commands/Recount/Ext/CommandModule.bsl",
                "old",
            ),
        ]);
        assert!(matches!(
            classify_source_change(
                &without_owner,
                &without_owner,
                "Catalogs/Goods/Commands/Recount/Ext/CommandModule.bsl",
                ActivationTarget::Main,
                ActivationMode::Online,
            ),
            Err(SourceChangeError::MissingRequiredPeer { .. })
        ));

        let unproven_owner = inventory(&[
            ("SettingsStorages/Store.xml", "owner"),
            ("SettingsStorages/Store/Commands/Run.xml", "command"),
            (
                "SettingsStorages/Store/Commands/Run/Ext/CommandModule.bsl",
                "body",
            ),
        ]);
        assert!(matches!(
            classify_source_change(
                &unproven_owner,
                &unproven_owner,
                "SettingsStorages/Store/Commands/Run/Ext/CommandModule.bsl",
                ActivationTarget::Main,
                ActivationMode::Online,
            ),
            Err(SourceChangeError::UnsupportedSourcePath(_))
        ));
    }

    #[test]
    fn activation_input_rejects_digest_only_proposed_source() {
        let active = common_module("old");
        let digest: [u8; SHA256_BYTES] = Sha256::digest(b"new").into();
        let proposed = SourceInventory::from_files(vec![
            file(
                "Configuration.xml",
                "<Configuration><Name>root</Name></Configuration>",
            ),
            file("CommonModules/Work.xml", "metadata"),
            SourceFileDigest::new("CommonModules/Work/Ext/Module.bsl", 3, digest).unwrap(),
        ])
        .unwrap();
        assert!(matches!(
            classify_source_change(
                &active,
                &proposed,
                "CommonModules/Work/Ext/Module.bsl",
                ActivationTarget::Main,
                ActivationMode::Online,
            ),
            Err(SourceChangeError::UnverifiedSourceBytes(_))
        ));
    }

    #[test]
    fn windows_case_aliases_collide_and_spelling_drift_is_rejected() {
        assert!(matches!(
            SourceInventory::from_files(vec![
                file("CommonModules/A.xml", "a"),
                file("commonmodules/a.xml", "b")
            ]),
            Err(SourceChangeError::WindowsPathCollision { .. })
        ));
        let active = common_module("old");
        let proposed = inventory(&[
            ("Configuration.xml", "root"),
            ("CommonModules/Work.xml", "metadata"),
            ("commonmodules/Work/Ext/Module.bsl", "new"),
        ]);
        assert!(matches!(
            classify_source_change(
                &active,
                &proposed,
                "CommonModules/Work/Ext/Module.bsl",
                ActivationTarget::Main,
                ActivationMode::Online
            ),
            Err(SourceChangeError::WindowsAliasMismatch { .. })
        ));
    }

    #[test]
    fn rejects_windows_reserved_trailing_ads_and_traversal_paths() {
        for path in [
            "CON/file.bsl",
            "dir./file.bsl",
            "dir /file.bsl",
            "dir/file:stream.bsl",
            "../outside.bsl",
            "a//b.bsl",
            "COMMON~1/file.bsl",
            "COM¹/file.bsl",
            "COM²/file.bsl",
            "COM³/file.bsl",
            "LPT¹/file.bsl",
            "LPT²/file.bsl",
            "LPT³/file.bsl",
        ] {
            assert!(SourceFileDigest::for_bytes(path, b"x").is_err(), "{path}");
        }
    }

    #[test]
    fn validates_extension_names() {
        assert!(ActivationTarget::extension("ServiceDesk").is_ok());
        for name in ["", " padded", "padded ", "bad\0name"] {
            assert!(ActivationTarget::extension(name).is_err());
        }
    }

    #[test]
    fn bounded_capture_and_filesystem_classification_work() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-source-change-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("CommonModules/Work/Ext")).unwrap();
        fs::write(root.join("Configuration.xml"), "root").unwrap();
        fs::write(root.join("CommonModules/Work.xml"), "metadata").unwrap();
        fs::write(root.join("CommonModules/Work/Ext/Module.bsl"), "old").unwrap();
        let held = HeldSourceRoot::open(&root, SourceInventoryLimits::default()).unwrap();
        fs::write(root.join("CommonModules/Work/Ext/Module.bsl"), "new").unwrap();
        let plan = held
            .classify_current(
                Path::new("CommonModules/Work/Ext/Module.bsl"),
                ActivationTarget::Main,
                ActivationMode::Online,
            )
            .unwrap();
        assert_eq!(plan.state(), SourceChangeState::Changed);
        let verified = plan
            .verified_source("CommonModules/Work/Ext/Module.bsl")
            .unwrap();
        assert_eq!(verified.bytes(), b"new");
        // A same-size rewrite after classification cannot alter the bytes
        // handed to compilation.
        fs::write(root.join("CommonModules/Work/Ext/Module.bsl"), "bad").unwrap();
        assert_eq!(verified.bytes(), b"new");
        drop(held);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn held_root_rejects_selected_path_traversal() {
        let base = std::env::temp_dir().join(format!(
            "ibcmd-rs-source-escape-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let root = base.join("root");
        fs::create_dir_all(&root).unwrap();
        fs::write(base.join("outside.bsl"), "outside").unwrap();
        let held = HeldSourceRoot::open(&root, SourceInventoryLimits::default()).unwrap();
        assert!(matches!(
            held.classify_current(
                Path::new("../outside.bsl"),
                ActivationTarget::Main,
                ActivationMode::Online,
            ),
            Err(SourceChangeError::InvalidRelativePath(_))
                | Err(SourceChangeError::PathEscapesRoot(_))
        ));
        drop(held);
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn canonical_paths_and_inventory_use_the_same_unicode_case_key() {
        let root = Path::new("/Лаборатория/Проект");
        assert!(paths_equal_windows(root, Path::new("/лаборатория/проект")));
        assert!(path_is_within_windows(
            Path::new("/лаборатория/ПРОЕКТ/CommonModules/Модуль/Ext/Module.bsl"),
            root
        ));
        assert!(!path_is_within_windows(
            Path::new("/лаборатория/Проект2/Module.bsl"),
            root
        ));
        assert!(!paths_equal_windows(
            root,
            Path::new("/лаборатория/ДругойПроект")
        ));
        let inventory = SourceInventory::from_files(vec![
            SourceFileDigest::for_bytes("CommonModules/Модуль/Ext/Module.bsl", b"source").unwrap(),
        ])
        .unwrap();
        assert!(
            inventory
                .file("commonmodules/модуль/ext/module.bsl")
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn capture_enforces_limits() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-source-limit-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("Configuration.xml"), "root").unwrap();
        let error = HeldSourceRoot::open(
            &root,
            SourceInventoryLimits {
                max_files: 1,
                max_file_bytes: 1,
                max_total_bytes: 1,
            },
        )
        .unwrap_err();
        assert!(matches!(error, SourceChangeError::InventoryLimit(_)));
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn rejects_root_and_entry_reparse_points_when_symlinks_are_available() {
        use std::os::windows::fs::{symlink_dir, symlink_file};

        let base = std::env::temp_dir().join(format!(
            "ibcmd-rs-source-reparse-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let real = base.join("real");
        fs::create_dir_all(&real).unwrap();
        fs::write(real.join("Configuration.xml"), "root").unwrap();

        let root_link = base.join("root-link");
        if symlink_dir(&real, &root_link).is_ok() {
            assert!(matches!(
                HeldSourceRoot::open(&root_link, SourceInventoryLimits::default()),
                Err(SourceChangeError::LinkOrReparsePoint(_))
            ));
        }

        let entry_link = real.join("linked.xml");
        let target = base.join("target.xml");
        fs::write(&target, "target").unwrap();
        if symlink_file(&target, &entry_link).is_ok() {
            assert!(matches!(
                HeldSourceRoot::open(&real, SourceInventoryLimits::default()),
                Err(SourceChangeError::LinkOrReparsePoint(_))
            ));
            fs::remove_file(&entry_link).unwrap();
        }
        if root_link.exists() {
            fs::remove_dir(&root_link).unwrap();
        }
        fs::remove_dir_all(base).unwrap();
    }
}

#[cfg(test)]
mod original_source_tests;
