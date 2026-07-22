//! Base-free native compiler for DataProcessor metadata.

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

/// Exact platform/storage layout selected for one DataProcessor compiler invocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataProcessorMetadataProfile(UtilityMetadataProfile);

impl DataProcessorMetadataProfile {
    pub fn from_effective(profile: &EffectiveProfile) -> Result<Self, UtilityProfileError> {
        UtilityMetadataProfile::from_effective(profile, UtilityFamily::DataProcessor).map(Self)
    }
}

pub fn compile_data_processor_metadata(
    validated: &ValidatedConfiguration<'_>,
    graph: &BootstrapGraph,
    object_uuid: ObjectUuid,
    axes: &CompileAxes,
    profile: &DataProcessorMetadataProfile,
) -> Result<StoragePatchEntry, UtilityBuildError> {
    compile_utility_metadata(validated, graph, object_uuid, axes, &profile.0)
}

pub fn decode_data_processor_blob(
    blob: &[u8],
    profile: &DataProcessorMetadataProfile,
) -> Result<UtilityNativeIr, UtilityBuildError> {
    decode_utility_blob(blob, &profile.0)
}
