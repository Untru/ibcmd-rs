//! Base-free native compiler for ExchangePlan metadata.

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
    BusinessObjectBuildError, BusinessObjectNativeIr, BusinessObjectProfileError,
    BusinessObjectTabularNativeIr,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExchangePlanMetadataProfile(BusinessObjectMetadataProfile);

impl ExchangePlanMetadataProfile {
    pub fn from_effective(profile: &EffectiveProfile) -> Result<Self, BusinessObjectProfileError> {
        BusinessObjectMetadataProfile::from_effective(profile, BusinessObjectFamily::ExchangePlan)
            .map(Self)
    }
}

pub fn compile_exchange_plan_metadata(
    validated: &ValidatedConfiguration<'_>,
    graph: &BootstrapGraph,
    object_uuid: ObjectUuid,
    axes: &CompileAxes,
    profile: &ExchangePlanMetadataProfile,
) -> Result<StoragePatchEntry, BusinessObjectBuildError> {
    compile_business_object(validated, graph, object_uuid, axes, &profile.0)
}

pub fn decode_exchange_plan_blob(
    blob: &[u8],
    profile: &ExchangePlanMetadataProfile,
) -> Result<BusinessObjectNativeIr, BusinessObjectBuildError> {
    decode_business_object_blob(blob, &profile.0)
}
