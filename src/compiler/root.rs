//! Exact `root` entry codec and the explicit Configuration-body blocker.

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::io::{self, Write};

use flate2::Compression;
use flate2::write::DeflateEncoder;
use ibcmd_core::artifact::ProfileId;
use ibcmd_core::storage::{
    MultipartIdentity, StorageBuildError, StorageKey, StoragePatchBuildError, StoragePatchEntry,
    StoragePatchOutcome, StoragePatchTarget, StorageProvenance,
};

use super::graph::{BootstrapGraph, BootstrapGraphError, SpecialEntryKind};
use super::version::{SpecialEntryProfile, SpecialEntryProfileError};

const UTF8_BOM: &[u8; 3] = b"\xef\xbb\xbf";

/// Failure to compile an evidence-backed special entry.
#[derive(Debug)]
pub enum SpecialEntryBuildError {
    /// The selected profile has no exact special-entry layout.
    Profile(SpecialEntryProfileError),
    /// The bootstrap graph cannot resolve or verify an entry.
    Graph(BootstrapGraphError),
    /// A neutral storage component rejected a derived value.
    Storage(StorageBuildError),
    /// A neutral storage patch component rejected a payload or outcome.
    Patch(StoragePatchBuildError),
    /// Raw-DEFLATE encoding failed.
    Deflate(io::Error),
    /// Physical routes and special-entry layouts came from different profiles.
    ProfileMismatch {
        /// Profile used to resolve graph routes.
        graph: ProfileId,
        /// Profile used to resolve special layouts.
        special: ProfileId,
    },
    /// A serialized service-entry pair count overflowed its machine bound.
    PairCountOverflow {
        /// Service entry being serialized.
        entry: &'static str,
    },
}

impl Display for SpecialEntryBuildError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Profile(source) => {
                write!(formatter, "unsupported special-entry profile: {source}")
            }
            Self::Graph(source) => write!(formatter, "invalid bootstrap graph: {source}"),
            Self::Storage(source) => {
                write!(formatter, "invalid special-entry storage value: {source}")
            }
            Self::Patch(source) => write!(formatter, "invalid special-entry patch: {source}"),
            Self::Deflate(source) => {
                write!(formatter, "failed to raw-deflate special entry: {source}")
            }
            Self::ProfileMismatch { graph, special } => write!(
                formatter,
                "bootstrap graph profile `{graph}` differs from special-entry profile `{special}`"
            ),
            Self::PairCountOverflow { entry } => {
                write!(formatter, "{entry} pair count overflowed usize")
            }
        }
    }
}

impl Error for SpecialEntryBuildError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Profile(source) => Some(source),
            Self::Graph(source) => Some(source),
            Self::Storage(source) => Some(source),
            Self::Patch(source) => Some(source),
            Self::Deflate(source) => Some(source),
            Self::ProfileMismatch { .. } => None,
            Self::PairCountOverflow { .. } => None,
        }
    }
}

impl From<SpecialEntryProfileError> for SpecialEntryBuildError {
    fn from(source: SpecialEntryProfileError) -> Self {
        Self::Profile(source)
    }
}

impl From<BootstrapGraphError> for SpecialEntryBuildError {
    fn from(source: BootstrapGraphError) -> Self {
        Self::Graph(source)
    }
}

impl From<StorageBuildError> for SpecialEntryBuildError {
    fn from(source: StorageBuildError) -> Self {
        Self::Storage(source)
    }
}

impl From<StoragePatchBuildError> for SpecialEntryBuildError {
    fn from(source: StoragePatchBuildError) -> Self {
        Self::Patch(source)
    }
}

/// Compiles the exact evidence-backed native `root` row.
///
/// Plaintext is UTF-8 BOM followed by `{2,<configuration UUID>,}`. The empty
/// third field and trailing comma are part of the native contract.
pub fn compile_root(
    graph: &BootstrapGraph,
    profile: &SpecialEntryProfile,
) -> Result<StoragePatchEntry, SpecialEntryBuildError> {
    ensure_graph_profile(graph, profile)?;
    graph.validate_special_references()?;
    let mut plaintext = Vec::with_capacity(80);
    plaintext.extend_from_slice(UTF8_BOM);
    write!(&mut plaintext, "{{2,{},}}", graph.configuration_uuid())
        .expect("writing to Vec cannot fail");
    compiled_special_entry(SpecialEntryKind::Root, &plaintext, profile)
}

/// Returns an explicit fail-closed outcome for the Configuration metadata
/// body until a profile-backed native layout compiler is available.
///
/// This function exists so callers cannot accidentally treat the three
/// service codecs as a complete bootstrap implementation.
pub fn configuration_body_blocker(
    graph: &BootstrapGraph,
    profile: &SpecialEntryProfile,
) -> Result<StoragePatchEntry, SpecialEntryBuildError> {
    ensure_graph_profile(graph, profile)?;
    let key = graph.configuration_uuid().to_string();
    Ok(StoragePatchEntry::new(
        StoragePatchTarget::new(
            StorageKey::new(&key)?,
            MultipartIdentity::single(),
            special_provenance(profile, "configuration-body")?,
        ),
        StoragePatchOutcome::unsupported(
            "configuration body has no evidence-backed base-free layout compiler for the selected target profile",
        )?,
    ))
}

pub(crate) fn ensure_graph_profile(
    graph: &BootstrapGraph,
    profile: &SpecialEntryProfile,
) -> Result<(), SpecialEntryBuildError> {
    if graph.profile_id() == profile.profile_id() {
        Ok(())
    } else {
        Err(SpecialEntryBuildError::ProfileMismatch {
            graph: graph.profile_id().clone(),
            special: profile.profile_id().clone(),
        })
    }
}

pub(crate) fn compiled_special_entry(
    kind: SpecialEntryKind,
    plaintext: &[u8],
    profile: &SpecialEntryProfile,
) -> Result<StoragePatchEntry, SpecialEntryBuildError> {
    let bytes = raw_deflate(plaintext)?;
    Ok(StoragePatchEntry::new(
        StoragePatchTarget::new(
            StorageKey::new(kind.key())?,
            MultipartIdentity::single(),
            special_provenance(profile, kind.key())?,
        ),
        StoragePatchOutcome::compiled(bytes)?,
    ))
}

pub(crate) fn special_provenance(
    profile: &SpecialEntryProfile,
    role: &str,
) -> Result<StorageProvenance, StorageBuildError> {
    StorageProvenance::new(&format!(
        "bootstrap:{}:{role}",
        profile.profile_id().as_str()
    ))
}

fn raw_deflate(plaintext: &[u8]) -> Result<Vec<u8>, SpecialEntryBuildError> {
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::best());
    encoder
        .write_all(plaintext)
        .map_err(SpecialEntryBuildError::Deflate)?;
    encoder.finish().map_err(SpecialEntryBuildError::Deflate)
}

#[cfg(test)]
pub(crate) fn inflate_for_test(bytes: &[u8]) -> Vec<u8> {
    use std::io::Read;

    let mut decoder = flate2::read::DeflateDecoder::new(bytes);
    let mut plaintext = Vec::new();
    decoder.read_to_end(&mut plaintext).unwrap();
    plaintext
}

#[cfg(test)]
mod tests {
    use ibcmd_core::artifact::ProfileId;
    use ibcmd_core::diagnostic::{ObjectPath, PathSegment, PropertyPath};
    use ibcmd_core::identity::{LogicalIdentity, ObjectUuid};
    use ibcmd_core::model::{
        CanonicalConfiguration, CanonicalObject, CanonicalObjectParts, MetadataKind,
    };
    use ibcmd_core::provenance::{CanonicalAnchor, SourceProvenance};
    use ibcmd_core::storage::StoragePatchOutcome;
    use ibcmd_core::validate::validate_configuration;

    use crate::compiler::graph::{ObjectStorageRoute, build_bootstrap_graph};
    use crate::compiler::identity::collect_bootstrap_identities;

    use super::*;

    const CONFIGURATION_UUID: &str = "61ee2494-c14a-4992-8c93-8e78b20bea27";

    fn graph() -> BootstrapGraph {
        let uuid = ObjectUuid::parse(CONFIGURATION_UUID).unwrap();
        let path = ObjectPath::new(vec![PathSegment::name("configuration").unwrap()]).unwrap();
        let provenance = SourceProvenance::new(
            ProfileId::parse("profile:test").unwrap(),
            CanonicalAnchor::new(path.clone(), PropertyPath::root()),
        );
        let object = CanonicalObject::new(CanonicalObjectParts::new(
            LogicalIdentity::new(uuid, path),
            MetadataKind::new("Configuration").unwrap(),
            provenance,
        ))
        .unwrap();
        let configuration = CanonicalConfiguration::new(vec![object]).unwrap();
        let validated = validate_configuration(&configuration).unwrap();
        let identities = collect_bootstrap_identities(&validated).unwrap();
        build_bootstrap_graph(
            &identities,
            ProfileId::parse("platform-8.3.27.1989").unwrap(),
            vec![ObjectStorageRoute::new(uuid, Vec::new()).unwrap()],
        )
        .unwrap()
    }

    fn profile() -> SpecialEntryProfile {
        SpecialEntryProfile::fixture("platform-8.3.27.1989", 80_327)
    }

    #[test]
    fn root_plaintext_matches_exact_native_golden_and_is_deterministic() {
        let graph = graph();
        let first = compile_root(&graph, &profile()).unwrap();
        let second = compile_root(&graph, &profile()).unwrap();
        assert_eq!(first, second);
        let payload = first.outcome().compiled_payload().unwrap();
        assert_eq!(
            inflate_for_test(payload.bytes()),
            format!("\u{feff}{{2,{CONFIGURATION_UUID},}}").as_bytes()
        );
    }

    #[test]
    fn missing_configuration_body_remains_an_explicit_preflight_blocker() {
        let entry = configuration_body_blocker(&graph(), &profile()).unwrap();
        assert_eq!(entry.target().key().as_str(), CONFIGURATION_UUID);
        assert!(matches!(
            entry.outcome(),
            StoragePatchOutcome::Unsupported { reason }
                if reason.as_str().contains("no evidence-backed base-free layout compiler")
        ));
    }

    #[test]
    fn graph_and_special_layout_profiles_cannot_be_mixed() {
        let error = compile_root(
            &graph(),
            &SpecialEntryProfile::fixture("platform-other", 80_327),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            SpecialEntryBuildError::ProfileMismatch { graph, special }
                if graph.as_str() == "platform-8.3.27.1989"
                    && special.as_str() == "platform-other"
        ));
    }
}
