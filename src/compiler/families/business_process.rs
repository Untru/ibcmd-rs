//! Base-free native compiler for BusinessProcess metadata.

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
pub struct BusinessProcessMetadataProfile(BusinessObjectMetadataProfile);

impl BusinessProcessMetadataProfile {
    pub fn from_effective(profile: &EffectiveProfile) -> Result<Self, BusinessObjectProfileError> {
        BusinessObjectMetadataProfile::from_effective(
            profile,
            BusinessObjectFamily::BusinessProcess,
        )
        .map(Self)
    }
}

pub fn compile_business_process_metadata(
    validated: &ValidatedConfiguration<'_>,
    graph: &BootstrapGraph,
    object_uuid: ObjectUuid,
    axes: &CompileAxes,
    profile: &BusinessProcessMetadataProfile,
) -> Result<StoragePatchEntry, BusinessObjectBuildError> {
    compile_business_object(validated, graph, object_uuid, axes, &profile.0)
}

pub fn decode_business_process_blob(
    blob: &[u8],
    profile: &BusinessProcessMetadataProfile,
) -> Result<BusinessObjectNativeIr, BusinessObjectBuildError> {
    decode_business_object_blob(blob, &profile.0)
}
