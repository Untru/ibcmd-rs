//! Strict, non-publishing external root/ObjectModule source intake.
//! Known ownership claims are not native body compilation or EPF/ERF readiness.
use std::collections::BTreeSet;
use std::fmt::{self, Display, Formatter};
use std::path::Path;

use ibcmd_core::artifact::ProfileId;
use ibcmd_core::asset::{AssetReference, MediaKind};
use ibcmd_core::diagnostic::ObjectPath;
use ibcmd_core::identity::ObjectUuid;
use ibcmd_core::validate::validate_configuration;
use ibcmd_core::value::CanonicalValueKind;
use ibcmd_xml::XmlReader;
use ibcmd_xml::metadata::{
    MetadataEnvelope, PackageIntent, decode_external_root, encode_external_root,
    inspect_package_identity,
};
use ibcmd_xml::source_tree::{
    ReaderLimits, SourceEntry, SourcePath, SourceTree, read_source_tree_strict,
};

use super::artifact::ArtifactScope;
use super::families::assets::{SourceAssetRegistry, SourceAssetRole, SourceAssetRoute};
use super::graph::{BootstrapGraph, ObjectStorageRoute, StorageSuffix, build_artifact_graph};
use super::identity::{BootstrapIdentities, collect_artifact_identities};

#[derive(Debug)]
pub enum ExternalIntakeError {
    Input {
        path: String,
        reason: String,
    },
    RootCount {
        actual: usize,
    },
    WrongSelectedRoot {
        selected: String,
        actual: String,
    },
    Unsupported {
        path: String,
        coordinate: &'static str,
    },
    Unconsumed {
        path: String,
    },
    CurrentMismatch {
        coordinate: &'static str,
    },
}
impl Display for ExternalIntakeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Input { path, reason } => write!(f, "external input `{path}`: {reason}"),
            Self::RootCount { actual } => write!(
                f,
                "external intake needs exactly one package root, found {actual}"
            ),
            Self::WrongSelectedRoot { selected, actual } => {
                write!(f, "selected `{selected}` is not package root `{actual}`")
            }
            Self::Unsupported { path, coordinate } => {
                write!(f, "external intake `{path}`: unsupported {coordinate}")
            }
            Self::Unconsumed { path } => write!(f, "external intake unconsumed source `{path}`"),
            Self::CurrentMismatch { coordinate } => {
                write!(f, "external CURRENT mismatch: {coordinate}")
            }
        }
    }
}
impl std::error::Error for ExternalIntakeError {}
fn input(path: impl Into<String>, error: impl Display) -> ExternalIntakeError {
    ExternalIntakeError::Input {
        path: path.into(),
        reason: error.to_string(),
    }
}

/// Exact ownership/reference of bytes already held by the existing SourceTree.
/// No second payload cache or semantic model is stored by a claim.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalAssetClaim {
    source_path: SourcePath,
    owner: ObjectUuid,
    route: &'static SourceAssetRoute,
    content: AssetReference,
}
impl ExternalAssetClaim {
    pub fn source_path(&self) -> &SourcePath {
        &self.source_path
    }
    pub const fn owner(&self) -> ObjectUuid {
        self.owner
    }
    pub const fn route(&self) -> &'static SourceAssetRoute {
        self.route
    }
    pub fn content(&self) -> &AssetReference {
        &self.content
    }
}

/// One retained authored inventory with canonical metadata and A0 owner graph.
/// Asset claims prove source ownership only; they never authorize an archive.
#[derive(Clone, Debug)]
pub struct ExternalIntake {
    tree: SourceTree,
    root_path: SourcePath,
    envelope: MetadataEnvelope,
    identities: BootstrapIdentities,
    graph: BootstrapGraph,
    claims: Vec<ExternalAssetClaim>,
}
impl ExternalIntake {
    /// Reads a direct root file or its owner directory. Both modes inventory
    /// the entire owner directory exactly once; root kind comes from XML.
    pub fn read(
        source: impl AsRef<Path>,
        profile: ProfileId,
        path: ObjectPath,
        limits: ReaderLimits,
    ) -> Result<Self, ExternalIntakeError> {
        let source = source.as_ref();
        let metadata = std::fs::symlink_metadata(source)
            .map_err(|e| input(source.display().to_string(), e))?;
        #[cfg(windows)]
        let redirected = {
            use std::os::windows::fs::MetadataExt;
            metadata.file_type().is_symlink() || metadata.file_attributes() & 0x400 != 0
        };
        #[cfg(not(windows))]
        let redirected = metadata.file_type().is_symlink();
        if redirected || !metadata.is_file() && !metadata.is_dir() {
            return Err(input(
                source.display().to_string(),
                "nonregular or redirected input",
            ));
        }
        // Retain the selected physical file, not its caller-supplied spelling.
        // Windows read sharing also prevents replacement while the census is read.
        let selected_file = if metadata.is_file() {
            Some(open_selected_file(source)?)
        } else {
            None
        };
        let directory = if selected_file.is_some() {
            source
                .parent()
                .filter(|x| !x.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."))
        } else {
            source
        };
        let tree = read_source_tree_strict(directory, limits)
            .map_err(|e| input(directory.display().to_string(), e))?;
        let selected = selected_file
            .as_ref()
            .map(|file| selected_census_path(directory, &tree, file))
            .transpose()?;
        Self::from_tree(tree, selected.as_ref(), profile, path)
    }
    /// Current caller-owned immutable source bytes, not a filesystem discovery
    /// receipt. No omitted disk files are inferred from this memory constructor.
    pub fn from_tree(
        tree: SourceTree,
        selected: Option<&SourcePath>,
        profile: ProfileId,
        path: ObjectPath,
    ) -> Result<Self, ExternalIntakeError> {
        tree.validate().map_err(|e| input("source tree", e))?;
        let mut candidates = Vec::new();
        for (index, entry) in tree.entries().iter().enumerate() {
            if entry.path().as_str().contains('/') {
                continue;
            }
            if selected != Some(entry.path())
                && !entry.path().as_str().to_ascii_lowercase().ends_with(".xml")
            {
                continue;
            }
            let document = XmlReader::from_slice(entry.bytes())
                .map_err(|e| input(entry.path().as_str(), e))?;
            if let Some(identity) =
                inspect_package_identity(&document).map_err(|e| input(entry.path().as_str(), e))?
            {
                candidates.push((index, identity, document));
            }
        }
        if candidates.len() != 1 {
            return Err(ExternalIntakeError::RootCount {
                actual: candidates.len(),
            });
        }
        let (index, identity, document) = candidates.pop().expect("one current root");
        let root_entry = &tree.entries()[index];
        if selected.is_some_and(|x| x != root_entry.path()) {
            return Err(ExternalIntakeError::WrongSelectedRoot {
                selected: selected.expect("checked selected").to_string(),
                actual: root_entry.path().to_string(),
            });
        }
        if !matches!(
            identity.intent,
            PackageIntent::ExternalDataProcessor | PackageIntent::ExternalReport
        ) {
            return Err(ExternalIntakeError::Unsupported {
                path: root_entry.path().to_string(),
                coordinate: "non-external package intent",
            });
        }
        let envelope = decode_external_root(&document, profile.clone(), path)
            .map_err(|e| input(root_entry.path().as_str(), e))?;
        // Root-only codec deliberately retains these declarations, but complete
        // intake may not advertise them as consumed without their typed owners.
        if envelope.root().properties().iter().any(|field| {
            matches!(field.name().as_str(), "ChildForms" | "ChildTemplates")
                && matches!(field.value().kind(), CanonicalValueKind::Sequence(items) if !items.is_empty())
        }) {
            return Err(ExternalIntakeError::Unsupported { path: root_entry.path().to_string(), coordinate: "declared Form/Template metadata and body ownership (A1 remainder)" });
        }
        let scope = ArtifactScope::from_source_identity(identity)
            .map_err(|e| input(root_entry.path().as_str(), e))?;
        let configuration = envelope
            .configuration()
            .map_err(|e| input(root_entry.path().as_str(), e))?;
        let validated = validate_configuration(&configuration)
            .map_err(|e| input(root_entry.path().as_str(), format!("{e:?}")))?;
        let identities = collect_artifact_identities(&validated, scope)
            .map_err(|e| input(root_entry.path().as_str(), e))?;
        let registry = SourceAssetRegistry;
        let family = envelope.root().kind().as_str();
        let route = registry
            .route(family, SourceAssetRole::ObjectModule)
            .ok_or_else(|| {
                input(
                    root_entry.path().as_str(),
                    "missing canonical ObjectModule route",
                )
            })?;
        let expected = registry
            .source_path(
                Path::new(root_entry.path().as_str()),
                family,
                SourceAssetRole::ObjectModule,
            )
            .and_then(|x| x.to_str().map(|x| x.replace('\\', "/")))
            .ok_or_else(|| {
                input(
                    root_entry.path().as_str(),
                    "invalid canonical owner directory",
                )
            })?;
        let mut claims = Vec::new();
        for (source_index, entry) in tree.entries().iter().enumerate() {
            if source_index == index {
                continue;
            }
            if entry.path().as_str() != expected {
                return Err(ExternalIntakeError::Unconsumed {
                    path: entry.path().to_string(),
                });
            }
            std::str::from_utf8(entry.bytes()).map_err(|e| input(entry.path().as_str(), e))?;
            let content = AssetReference::new(
                entry.digest(),
                entry.bytes().len() as u64,
                MediaKind::new("text/x-1c-bsl").map_err(|e| input(entry.path().as_str(), e))?,
            )
            .map_err(|e| input(entry.path().as_str(), e))?;
            content
                .verify_bytes(entry.bytes())
                .map_err(|e| input(entry.path().as_str(), e))?;
            claims.push(ExternalAssetClaim {
                source_path: entry.path().clone(),
                owner: scope.root_uuid(),
                route,
                content,
            });
        }
        let suffixes = claims
            .iter()
            .map(|c| {
                StorageSuffix::new(c.route.suffix()).map_err(|e| input(c.source_path.as_str(), e))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let object_route = ObjectStorageRoute::new(scope.root_uuid(), suffixes)
            .map_err(|e| input(root_entry.path().as_str(), e))?;
        let graph = build_artifact_graph(&identities, profile, vec![object_route])
            .map_err(|e| input(root_entry.path().as_str(), e))?;
        let root_path = root_entry.path().clone();
        Ok(Self {
            tree,
            root_path,
            envelope,
            identities,
            graph,
            claims,
        })
    }
    pub fn tree(&self) -> &SourceTree {
        &self.tree
    }
    pub fn root_path(&self) -> &SourcePath {
        &self.root_path
    }
    pub fn envelope(&self) -> &MetadataEnvelope {
        &self.envelope
    }
    pub fn identities(&self) -> &BootstrapIdentities {
        &self.identities
    }
    pub fn graph(&self) -> &BootstrapGraph {
        &self.graph
    }
    pub fn claims(&self) -> &[ExternalAssetClaim] {
        &self.claims
    }

    /// Pure current edit preparation: each supplied asset must replace an exact
    /// already-claimed owner path, once. Root metadata uses its existing typed
    /// editor. The complete new inventory is re-admitted before it is returned.
    pub fn with_current(
        &self,
        current: &MetadataEnvelope,
        asset_edits: &[SourceEntry],
    ) -> Result<Self, ExternalIntakeError> {
        let profile = self.envelope.root().provenance().source_profile();
        let bytes = encode_external_root(current, profile)
            .map_err(|e| input(self.root_path.as_str(), e))?;
        if current.external_source_binding() != self.envelope.external_source_binding()
            || current.root().identity().path() != self.envelope.root().identity().path()
            || current.source_document() != self.envelope.source_document()
        {
            return Err(ExternalIntakeError::CurrentMismatch {
                coordinate: "root binding/path",
            });
        }
        let mut seen = BTreeSet::new();
        for edit in asset_edits {
            if !self.claims.iter().any(|x| x.source_path == *edit.path())
                || !seen.insert(edit.path())
            {
                return Err(ExternalIntakeError::CurrentMismatch {
                    coordinate: "unknown/duplicate asset owner path",
                });
            }
        }
        let mut entries = self.tree.entries().to_vec();
        for entry in &mut entries {
            if entry.path() == &self.root_path {
                *entry = SourceEntry::from_bytes(self.root_path.clone(), bytes.clone())
                    .map_err(|e| input(self.root_path.as_str(), e))?;
            } else if let Some(edit) = asset_edits.iter().find(|x| x.path() == entry.path()) {
                *entry = edit.clone();
            }
        }
        let tree = SourceTree::new(entries).map_err(|e| input("CURRENT source tree", e))?;
        let result = Self::from_tree(
            tree,
            Some(&self.root_path),
            profile.clone(),
            current.root().identity().path().clone(),
        )?;
        if result.envelope.root() != current.root()
            || result.envelope.descendants() != current.descendants()
            || result.envelope.external_source_binding() != current.external_source_binding()
            || result.identities != self.identities
        {
            return Err(ExternalIntakeError::CurrentMismatch {
                coordinate: "complete root/identity forward equality",
            });
        }
        Ok(result)
    }
}

// Stable OS handle APIs already used by source_oracle::file_identity. These
// identities select actual census spelling; they do not infer package intent.
#[cfg(unix)]
fn selected_file_identity(file: &std::fs::File) -> std::io::Result<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    let metadata = file.metadata()?;
    Ok((metadata.dev(), metadata.ino()))
}
#[cfg(windows)]
fn selected_file_identity(file: &std::fs::File) -> std::io::Result<(u64, u64)> {
    use std::mem::MaybeUninit;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
    };
    let mut information = MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::zeroed();
    // SAFETY: the retained File owns a live handle and the output storage is valid.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), information.as_mut_ptr()) } == 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: successful GetFileInformationByHandle initializes the entire record.
    let information = unsafe { information.assume_init() };
    Ok((
        u64::from(information.dwVolumeSerialNumber),
        (u64::from(information.nFileIndexHigh) << 32) | u64::from(information.nFileIndexLow),
    ))
}
#[cfg(not(any(unix, windows)))]
fn selected_file_identity(_file: &std::fs::File) -> std::io::Result<(u64, u64)> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "selected-file physical identity is unavailable on this platform",
    ))
}
fn open_selected_file(path: &Path) -> Result<std::fs::File, ExternalIntakeError> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ,
        };
        options
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .share_mode(FILE_SHARE_READ);
    }
    let file = options
        .open(path)
        .map_err(|e| input(path.display().to_string(), e))?;
    let metadata = file
        .metadata()
        .map_err(|e| input(path.display().to_string(), e))?;
    let path_metadata =
        std::fs::symlink_metadata(path).map_err(|e| input(path.display().to_string(), e))?;
    #[cfg(windows)]
    let redirected = {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0 || path_metadata.file_attributes() & 0x400 != 0
    };
    #[cfg(not(windows))]
    let redirected = path_metadata.file_type().is_symlink();
    if !metadata.is_file() || !path_metadata.is_file() || redirected {
        return Err(input(
            path.display().to_string(),
            "nonregular or redirected selected file",
        ));
    }
    Ok(file)
}
fn selected_census_path(
    directory: &Path,
    tree: &SourceTree,
    selected: &std::fs::File,
) -> Result<SourcePath, ExternalIntakeError> {
    let identity = selected_file_identity(selected).map_err(|e| input("selected file", e))?;
    let mut matched = None;
    for entry in tree
        .entries()
        .iter()
        .filter(|entry| !entry.path().as_str().contains('/'))
    {
        let physical = directory.join(entry.path().as_str());
        // Only metadata/handle identity is queried. XML and all asset bytes come
        // exclusively from the already retained SourceTree, never a second read.
        let file = open_selected_file(&physical)?;
        let actual = selected_file_identity(&file).map_err(|e| input(entry.path().as_str(), e))?;
        if actual == identity {
            if matched.is_some() {
                return Err(input(
                    "selected file",
                    "physical identity aliases multiple census entries",
                ));
            }
            matched = Some(entry.path().clone());
        }
    }
    matched.ok_or_else(|| {
        input(
            "selected file",
            "physical identity is absent from the retained census",
        )
    })
}
