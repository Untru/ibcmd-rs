//! Base-free native compiler entry points for register metadata.

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
    BusinessObjectBuildError as RegisterBuildError, BusinessObjectNativeIr as RegisterNativeIr,
    BusinessObjectProfileError as RegisterProfileError,
};

/// Register families whose 8.3.27 native layouts have independent profile keys.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RegisterFamily {
    Information,
    Accumulation,
    Accounting,
    Calculation,
}

impl RegisterFamily {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Information => "InformationRegister",
            Self::Accumulation => "AccumulationRegister",
            Self::Accounting => "AccountingRegister",
            Self::Calculation => "CalculationRegister",
        }
    }

    const fn native_family(self) -> BusinessObjectFamily {
        match self {
            Self::Information => BusinessObjectFamily::InformationRegister,
            Self::Accumulation => BusinessObjectFamily::AccumulationRegister,
            Self::Accounting => BusinessObjectFamily::AccountingRegister,
            Self::Calculation => BusinessObjectFamily::CalculationRegister,
        }
    }
}

/// Exact platform/storage layout selected for one register compilation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegisterMetadataProfile {
    family: RegisterFamily,
    inner: BusinessObjectMetadataProfile,
}

impl RegisterMetadataProfile {
    pub fn from_effective_for_family(
        profile: &EffectiveProfile,
        family: RegisterFamily,
    ) -> Result<Self, RegisterProfileError> {
        Ok(Self {
            family,
            inner: BusinessObjectMetadataProfile::from_effective(profile, family.native_family())?,
        })
    }

    pub const fn family(&self) -> RegisterFamily {
        self.family
    }
}

/// Compiles a validated register and its evidenced references into one native row.
pub fn compile_register_metadata(
    validated: &ValidatedConfiguration<'_>,
    graph: &BootstrapGraph,
    object_uuid: ObjectUuid,
    axes: &CompileAxes,
    profile: &RegisterMetadataProfile,
) -> Result<StoragePatchEntry, RegisterBuildError> {
    compile_business_object(validated, graph, object_uuid, axes, &profile.inner)
}

/// Strictly decodes an evidenced register primary row into inventory IR.
pub fn decode_register_blob(
    blob: &[u8],
    profile: &RegisterMetadataProfile,
) -> Result<RegisterNativeIr, RegisterBuildError> {
    decode_business_object_blob(blob, &profile.inner)
}
