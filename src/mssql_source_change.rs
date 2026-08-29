//! Fail-closed classification for direct MSSQL source activation.
//!
//! This module deliberately stops before compilation, SQL rendering, or
//! publication.  Its output is the typed input those later phases may accept.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

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
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceFileDigest {
    path: String,
    size_bytes: u64,
    sha256: [u8; SHA256_BYTES],
    verified_bytes: Option<Vec<u8>>,
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
        })
    }

    pub fn for_bytes(path: impl Into<String>, bytes: &[u8]) -> Result<Self, SourceChangeError> {
        let digest: [u8; SHA256_BYTES] = Sha256::digest(bytes).into();
        let mut file = Self::new(path, bytes.len() as u64, digest)?;
        file.verified_bytes = Some(bytes.to_vec());
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
        self.verified_bytes.as_deref()
    }
}

/// Immutable, alias-safe inventory of every file under a source root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceInventory {
    by_windows_key: BTreeMap<String, SourceFileDigest>,
}

impl SourceInventory {
    pub fn from_files(files: Vec<SourceFileDigest>) -> Result<Self, SourceChangeError> {
        let mut by_windows_key = BTreeMap::new();
        for file in files {
            let key = windows_path_key(&file.path);
            if let Some(previous) = by_windows_key.insert(key, file.clone()) {
                return Err(SourceChangeError::WindowsPathCollision {
                    first: previous.path,
                    second: file.path,
                });
            }
        }
        Ok(Self { by_windows_key })
    }

    pub fn files(&self) -> impl ExactSizeIterator<Item = &SourceFileDigest> {
        self.by_windows_key.values()
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
#[derive(Debug)]
pub struct HeldSourceRoot {
    requested_root: PathBuf,
    canonical_root: PathBuf,
    baseline: SourceInventory,
    limits: SourceInventoryLimits,
    #[cfg(windows)]
    _root_guard: fs::File,
}

impl HeldSourceRoot {
    pub fn open(root: &Path, limits: SourceInventoryLimits) -> Result<Self, SourceChangeError> {
        validate_limits(limits)?;
        let canonical_root = canonical_directory(root)?;
        #[cfg(windows)]
        let root_guard = open_root_guard(&canonical_root)?;
        let baseline = capture_inventory(&canonical_root, limits, &BTreeSet::new())?;
        Ok(Self {
            requested_root: root.to_path_buf(),
            canonical_root,
            baseline,
            limits,
            #[cfg(windows)]
            _root_guard: root_guard,
        })
    }

    pub fn canonical_root(&self) -> &Path {
        &self.canonical_root
    }

    pub fn baseline(&self) -> &SourceInventory {
        &self.baseline
    }

    pub fn capture_current(&self) -> Result<SourceInventory, SourceChangeError> {
        let current_root = canonical_directory(&self.requested_root)?;
        if !paths_equal_windows(&current_root, &self.canonical_root) {
            return Err(SourceChangeError::HeldRootChanged);
        }
        capture_inventory(&current_root, self.limits, &BTreeSet::new())
    }

    pub fn classify_current(
        &self,
        selected_path: &Path,
        target: ActivationTarget,
        mode: ActivationMode,
    ) -> Result<SourceActivationInput, SourceChangeError> {
        let selected = resolve_selected_path(&self.canonical_root, selected_path)?;
        let retention = candidate_retention_paths(&selected);
        let current_root = canonical_directory(&self.requested_root)?;
        if !paths_equal_windows(&current_root, &self.canonical_root) {
            return Err(SourceChangeError::HeldRootChanged);
        }
        let current = capture_inventory(&current_root, self.limits, &retention)?;
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
    bytes: Vec<u8>,
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

    let body = classify_supported_body(&active_selected.path, active)?;
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
        verified_sources.push(VerifiedSourceFile {
            path: source.path.clone(),
            sha256: digest,
            bytes: bytes.to_vec(),
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
    if let Some(form_dir_len) = managed_form_dir_len(&parts) {
        let form_dir = parts[..form_dir_len].join("/");
        paths.insert(windows_path_key(&format!("{form_dir}/Ext/Form.xml")));
        paths.insert(windows_path_key(&format!("{form_dir}/Ext/Form/Module.bsl")));
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
        if active_file.size_bytes != proposed_file.size_bytes
            || active_file.sha256 != proposed_file.sha256
        {
            changed.push(active_file.path.clone());
        }
    }
    changed.sort_by_key(|path| windows_path_key(path));
    Ok(changed)
}

fn classify_supported_body(
    path: &str,
    inventory: &SourceInventory,
) -> Result<SourceBodyKind, SourceChangeError> {
    let parts = path.split('/').collect::<Vec<_>>();
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
    canonical_root: &Path,
    limits: SourceInventoryLimits,
    retain_bytes: &BTreeSet<String>,
) -> Result<SourceInventory, SourceChangeError> {
    let mut files = Vec::new();
    let mut total = 0_u64;
    for entry in WalkDir::new(canonical_root).follow_links(false) {
        let entry = entry.map_err(|error| SourceChangeError::Io(error.to_string()))?;
        if entry.path() == canonical_root {
            continue;
        }
        if entry.file_type().is_symlink() || path_has_reparse_attribute(entry.path())? {
            return Err(SourceChangeError::LinkOrReparsePoint(
                entry.path().display().to_string(),
            ));
        }
        if !entry.file_type().is_file() {
            continue;
        }
        if files.len() >= limits.max_files {
            return Err(SourceChangeError::InventoryLimit("file count"));
        }
        let canonical_file = fs::canonicalize(entry.path())
            .map_err(|error| SourceChangeError::Io(error.to_string()))?;
        if !path_is_within_windows(&canonical_file, canonical_root) {
            return Err(SourceChangeError::PathEscapesRoot(
                entry.path().display().to_string(),
            ));
        }
        let relative = canonical_file.strip_prefix(canonical_root).map_err(|_| {
            SourceChangeError::PathEscapesRoot(canonical_file.display().to_string())
        })?;
        let relative = path_to_slash(relative)?;
        let metadata = fs::metadata(&canonical_file)
            .map_err(|error| SourceChangeError::Io(error.to_string()))?;
        if metadata.len() > limits.max_file_bytes {
            return Err(SourceChangeError::InventoryLimit("single file size"));
        }
        total = total
            .checked_add(metadata.len())
            .ok_or(SourceChangeError::InventoryLimit("total byte size"))?;
        if total > limits.max_total_bytes {
            return Err(SourceChangeError::InventoryLimit("total byte size"));
        }
        let mut file = fs::File::open(&canonical_file)
            .map_err(|error| SourceChangeError::Io(error.to_string()))?;
        let mut hasher = Sha256::new();
        let retain = retain_bytes.contains(&windows_path_key(&relative));
        let mut retained = retain.then(|| Vec::with_capacity(metadata.len() as usize));
        let mut buffer = [0_u8; 64 * 1024];
        let mut read_total = 0_u64;
        loop {
            let read = file
                .read(&mut buffer)
                .map_err(|error| SourceChangeError::Io(error.to_string()))?;
            if read == 0 {
                break;
            }
            read_total += read as u64;
            if read_total > metadata.len() || read_total > limits.max_file_bytes {
                return Err(SourceChangeError::FileChangedDuringRead(relative));
            }
            hasher.update(&buffer[..read]);
            if let Some(bytes) = &mut retained {
                bytes.extend_from_slice(&buffer[..read]);
            }
        }
        let post_metadata = file
            .metadata()
            .map_err(|error| SourceChangeError::Io(error.to_string()))?;
        if read_total != metadata.len()
            || post_metadata.len() != metadata.len()
            || metadata.modified().ok() != post_metadata.modified().ok()
        {
            return Err(SourceChangeError::FileChangedDuringRead(relative));
        }
        let mut source = SourceFileDigest::new(relative, read_total, hasher.finalize().into())?;
        source.verified_bytes = retained;
        files.push(source);
    }
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

#[cfg(windows)]
fn open_root_guard(path: &Path) -> Result<fs::File, SourceChangeError> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_READ, FILE_SHARE_WRITE,
    };

    fs::OpenOptions::new()
        .access_mode(0)
        // Deliberately omit FILE_SHARE_DELETE: the held root cannot be
        // replaced or renamed between classification and later consumption.
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)
        .map_err(|error| SourceChangeError::Io(error.to_string()))
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

fn windows_path_key(path: &str) -> String {
    path.chars().flat_map(char::to_lowercase).collect()
}

fn paths_equal_text_windows(left: &str, right: &str) -> bool {
    windows_path_key(left) == windows_path_key(right)
}

fn paths_equal_windows(left: &Path, right: &Path) -> bool {
    left.to_string_lossy()
        .eq_ignore_ascii_case(&right.to_string_lossy())
}

fn path_is_within_windows(candidate: &Path, root: &Path) -> bool {
    let candidate = candidate.components().collect::<Vec<_>>();
    let root = root.components().collect::<Vec<_>>();
    candidate.len() >= root.len()
        && candidate.iter().zip(root.iter()).all(|(left, right)| {
            left.as_os_str()
                .to_string_lossy()
                .eq_ignore_ascii_case(&right.as_os_str().to_string_lossy())
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

    fn common_module(value: &str) -> SourceInventory {
        inventory(&[
            ("Configuration.xml", "root"),
            ("CommonModules/Work.xml", "metadata"),
            ("CommonModules/Work/Ext/Module.bsl", value),
        ])
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
            &managed_form("old body", "old module"),
            &managed_form("new body", "new module"),
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
            ("Enums/Status/Forms/Card/Ext/Form.xml", "old"),
        ]);
        let proposed = inventory(&[
            ("Enums/Status.xml", "owner"),
            ("Enums/Status/Forms/Card.xml", "form"),
            ("Enums/Status/Forms/Card/Ext/Form.xml", "new"),
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
            ("Configuration.xml", "changed root"),
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
            file("Configuration.xml", "root"),
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
