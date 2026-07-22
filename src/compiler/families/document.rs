//! Base-free native compiler for Document metadata.

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

/// Exact platform/storage layout selected for one Document compiler invocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocumentMetadataProfile(BusinessObjectMetadataProfile);

impl DocumentMetadataProfile {
    /// Selects the Document layout without deriving one version axis from another.
    pub fn from_effective(profile: &EffectiveProfile) -> Result<Self, BusinessObjectProfileError> {
        BusinessObjectMetadataProfile::from_effective(profile, BusinessObjectFamily::Document)
            .map(Self)
    }
}

/// Compiles a validated Document and all of its embedded metadata into one row.
pub fn compile_document_metadata(
    validated: &ValidatedConfiguration<'_>,
    graph: &BootstrapGraph,
    object_uuid: ObjectUuid,
    axes: &CompileAxes,
    profile: &DocumentMetadataProfile,
) -> Result<StoragePatchEntry, BusinessObjectBuildError> {
    compile_business_object(validated, graph, object_uuid, axes, &profile.0)
}

/// Strictly decodes an evidenced Document primary row into inventory IR.
pub fn decode_document_blob(
    blob: &[u8],
    profile: &DocumentMetadataProfile,
) -> Result<BusinessObjectNativeIr, BusinessObjectBuildError> {
    decode_business_object_blob(blob, &profile.0)
}
