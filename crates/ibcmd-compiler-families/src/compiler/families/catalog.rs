//! Base-free native compiler for Catalog metadata.

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

/// Exact platform/storage layout selected for one Catalog compiler invocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogMetadataProfile(BusinessObjectMetadataProfile);

impl CatalogMetadataProfile {
    /// Selects the Catalog layout without deriving one version axis from another.
    pub fn from_effective(profile: &EffectiveProfile) -> Result<Self, BusinessObjectProfileError> {
        BusinessObjectMetadataProfile::from_effective(profile, BusinessObjectFamily::Catalog)
            .map(Self)
    }
}

/// Compiles a validated Catalog and all of its embedded metadata into one row.
pub fn compile_catalog_metadata(
    validated: &ValidatedConfiguration<'_>,
    graph: &BootstrapGraph,
    object_uuid: ObjectUuid,
    axes: &CompileAxes,
    profile: &CatalogMetadataProfile,
) -> Result<StoragePatchEntry, BusinessObjectBuildError> {
    compile_business_object(validated, graph, object_uuid, axes, &profile.0)
}

/// Strictly decodes an evidenced Catalog primary row into inventory IR.
pub fn decode_catalog_blob(
    blob: &[u8],
    profile: &CatalogMetadataProfile,
) -> Result<BusinessObjectNativeIr, BusinessObjectBuildError> {
    decode_business_object_blob(blob, &profile.0)
}
