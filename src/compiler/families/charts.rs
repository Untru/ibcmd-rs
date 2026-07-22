//! Base-free native compiler entry points for 1C chart metadata.

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
    BusinessObjectBuildError as ChartBuildError, BusinessObjectNativeIr as ChartNativeIr,
    BusinessObjectProfileError as ChartProfileError,
};

/// Chart families whose layouts evolve independently through platform profiles.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ChartFamily {
    CharacteristicTypes,
    Accounts,
    CalculationTypes,
}

impl ChartFamily {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CharacteristicTypes => "ChartOfCharacteristicTypes",
            Self::Accounts => "ChartOfAccounts",
            Self::CalculationTypes => "ChartOfCalculationTypes",
        }
    }

    const fn native_family(self) -> BusinessObjectFamily {
        match self {
            Self::CharacteristicTypes => BusinessObjectFamily::ChartOfCharacteristicTypes,
            Self::Accounts => BusinessObjectFamily::ChartOfAccounts,
            Self::CalculationTypes => BusinessObjectFamily::ChartOfCalculationTypes,
        }
    }
}

/// Exact platform/storage layout selected for one chart compilation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChartMetadataProfile {
    family: ChartFamily,
    inner: BusinessObjectMetadataProfile,
}

impl ChartMetadataProfile {
    pub fn from_effective_for_family(
        profile: &EffectiveProfile,
        family: ChartFamily,
    ) -> Result<Self, ChartProfileError> {
        Ok(Self {
            family,
            inner: BusinessObjectMetadataProfile::from_effective(profile, family.native_family())?,
        })
    }

    pub const fn family(&self) -> ChartFamily {
        self.family
    }
}

/// Compiles a validated chart and its evidenced references into one native row.
pub fn compile_chart_metadata(
    validated: &ValidatedConfiguration<'_>,
    graph: &BootstrapGraph,
    object_uuid: ObjectUuid,
    axes: &CompileAxes,
    profile: &ChartMetadataProfile,
) -> Result<StoragePatchEntry, ChartBuildError> {
    compile_business_object(validated, graph, object_uuid, axes, &profile.inner)
}

/// Strictly decodes an evidenced chart primary row into inventory IR.
pub fn decode_chart_blob(
    blob: &[u8],
    profile: &ChartMetadataProfile,
) -> Result<ChartNativeIr, ChartBuildError> {
    decode_business_object_blob(blob, &profile.inner)
}
