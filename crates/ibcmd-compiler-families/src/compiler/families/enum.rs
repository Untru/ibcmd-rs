//! Base-free native compiler for Enum metadata.

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

/// Exact platform/storage layout selected for one Enum compiler invocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnumMetadataProfile(UtilityMetadataProfile);

impl EnumMetadataProfile {
    pub fn from_effective(profile: &EffectiveProfile) -> Result<Self, UtilityProfileError> {
        UtilityMetadataProfile::from_effective(profile, UtilityFamily::Enum).map(Self)
    }
}

pub fn compile_enum_metadata(
    validated: &ValidatedConfiguration<'_>,
    graph: &BootstrapGraph,
    object_uuid: ObjectUuid,
    axes: &CompileAxes,
    profile: &EnumMetadataProfile,
) -> Result<StoragePatchEntry, UtilityBuildError> {
    compile_utility_metadata(validated, graph, object_uuid, axes, &profile.0)
}

pub fn decode_enum_blob(
    blob: &[u8],
    profile: &EnumMetadataProfile,
) -> Result<UtilityNativeIr, UtilityBuildError> {
    decode_utility_blob(blob, &profile.0)
}
