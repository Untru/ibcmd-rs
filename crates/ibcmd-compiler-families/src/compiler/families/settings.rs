//! Base-free native compiler for SettingsStorage metadata.

use ibcmd_core::identity::ObjectUuid;
use ibcmd_core::profile::EffectiveProfile;
use ibcmd_core::storage::StoragePatchEntry;
use ibcmd_core::validate::ValidatedConfiguration;

use super::super::CompileAxes;
use super::super::graph::BootstrapGraph;
use super::utility::{
    UtilityFamily, UtilityMetadataProfile, compile_utility_metadata, decode_utility_blob,
};

pub use super::utility::{
    UtilityBuildError, UtilityNativeIr, UtilityProfileError, UtilityTabularNativeIr,
};

/// Exact platform/storage layout selected for one SettingsStorage compiler invocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettingsStorageMetadataProfile(UtilityMetadataProfile);

impl SettingsStorageMetadataProfile {
    pub fn from_effective(profile: &EffectiveProfile) -> Result<Self, UtilityProfileError> {
        UtilityMetadataProfile::from_effective(profile, UtilityFamily::SettingsStorage).map(Self)
    }
}

pub fn compile_settings_storage_metadata(
    validated: &ValidatedConfiguration<'_>,
    graph: &BootstrapGraph,
    object_uuid: ObjectUuid,
    axes: &CompileAxes,
    profile: &SettingsStorageMetadataProfile,
) -> Result<StoragePatchEntry, UtilityBuildError> {
    compile_utility_metadata(validated, graph, object_uuid, axes, &profile.0)
}

pub fn decode_settings_storage_blob(
    blob: &[u8],
    profile: &SettingsStorageMetadataProfile,
) -> Result<UtilityNativeIr, UtilityBuildError> {
    decode_utility_blob(blob, &profile.0)
}
