//! Base-free native compiler entry point for CalculationRegister Recalculation metadata.

use ibcmd_core::identity::ObjectUuid;
use ibcmd_core::profile::EffectiveProfile;
use ibcmd_core::storage::StoragePatchEntry;
use ibcmd_core::validate::ValidatedConfiguration;

use super::super::CompileAxes;
use super::super::graph::BootstrapGraph;
use super::business_object::{
    BusinessObjectFamily, BusinessObjectMetadataProfile, compile_business_object,
    decode_business_object_blob,
};

pub use super::business_object::{
    BusinessObjectBuildError as RecalculationBuildError,
    BusinessObjectNativeIr as RecalculationNativeIr,
    BusinessObjectProfileError as RecalculationProfileError,
};

/// Exact platform/storage layout selected for one Recalculation compilation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecalculationMetadataProfile(BusinessObjectMetadataProfile);

impl RecalculationMetadataProfile {
    pub fn from_effective(profile: &EffectiveProfile) -> Result<Self, RecalculationProfileError> {
        BusinessObjectMetadataProfile::from_effective(profile, BusinessObjectFamily::Recalculation)
            .map(Self)
    }
}

/// Compiles a validated Recalculation and its dimensions into one native row.
pub fn compile_recalculation_metadata(
    validated: &ValidatedConfiguration<'_>,
    graph: &BootstrapGraph,
    object_uuid: ObjectUuid,
    axes: &CompileAxes,
    profile: &RecalculationMetadataProfile,
) -> Result<StoragePatchEntry, RecalculationBuildError> {
    compile_business_object(validated, graph, object_uuid, axes, &profile.0)
}

/// Strictly decodes an evidenced Recalculation row into inventory IR.
pub fn decode_recalculation_blob(
    blob: &[u8],
    profile: &RecalculationMetadataProfile,
) -> Result<RecalculationNativeIr, RecalculationBuildError> {
    decode_business_object_blob(blob, &profile.0)
}
