//! Descriptor for the existing partial MSSQL Config/ConfigSave path.
//!
//! This boundary deliberately contains no SQL connection, filesystem path,
//! process, or platform-executable type. Those remain orchestration details in
//! the legacy root modules.

use ibcmd_core::artifact::{DbmsKind, ProfileId, StorageProfileId};
use ibcmd_core::capability::{
    CapabilityDeclaration, CapabilityEvaluation, CapabilitySet, ImplementationLevel,
    PreservationLevel, bootstrap_capability, convert_capability, export_capability,
    inspect_capability, overlay_capability, repack_capability,
};
use ibcmd_core::profile::CapabilityId;
use ibcmd_core::version::XmlDialect;

use crate::legacy_version::{InfobaseConfigSourceVersion, LegacyVersionAxes};

/// Failure to bind caller-supplied axes to the fixed legacy MSSQL storage boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MssqlLegacyStorageProfileConflict {
    expected: StorageProfileId,
    actual: StorageProfileId,
}

impl MssqlLegacyStorageProfileConflict {
    /// The storage profile required by this provider.
    pub const fn expected(&self) -> &StorageProfileId {
        &self.expected
    }

    /// The conflicting storage profile supplied by the caller.
    pub const fn actual(&self) -> &StorageProfileId {
        &self.actual
    }
}

impl std::fmt::Display for MssqlLegacyStorageProfileConflict {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "legacy MSSQL storage profile conflict: expected {}, got {}",
            self.expected, self.actual
        )
    }
}

impl std::error::Error for MssqlLegacyStorageProfileConflict {}

/// Stable identity of the root legacy MSSQL provider.
pub const LEGACY_MSSQL_PROVIDER_ID: &str = "provider:mssql-legacy";
/// Stable logical identity of its Config/ConfigSave storage boundary.
pub const LEGACY_MSSQL_STORAGE_PROFILE_ID: &str = "storage:mssql-config-configsave";

/// Platform-independent descriptor for the existing partial MSSQL adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MssqlLegacyAdapter {
    provider_id: ProfileId,
    storage_profile_id: StorageProfileId,
    dbms: DbmsKind,
    version_axes: LegacyVersionAxes,
    capabilities: CapabilitySet,
}

impl MssqlLegacyAdapter {
    /// Wraps explicitly separated version axes without inferring coordinates.
    pub fn new(
        version_axes: LegacyVersionAxes,
    ) -> std::result::Result<Self, MssqlLegacyStorageProfileConflict> {
        let expected = StorageProfileId::parse(LEGACY_MSSQL_STORAGE_PROFILE_ID)
            .expect("legacy MSSQL storage profile identifier is valid");
        let version_axes = match version_axes.storage_profile() {
            None => version_axes.with_storage_profile(expected.clone()),
            Some(actual) if actual == &expected => version_axes,
            Some(actual) => {
                return Err(MssqlLegacyStorageProfileConflict {
                    expected,
                    actual: actual.clone(),
                });
            }
        };
        let storage_profile_id = version_axes
            .storage_profile()
            .expect("bound legacy MSSQL axes have a storage profile")
            .clone();
        Ok(Self {
            provider_id: ProfileId::parse(LEGACY_MSSQL_PROVIDER_ID)
                .expect("legacy MSSQL provider identifier is valid"),
            storage_profile_id,
            dbms: DbmsKind::mssql(),
            version_axes,
            capabilities: legacy_capabilities(),
        })
    }

    /// Wraps a historical selector after converting it to independent axes.
    pub fn from_legacy_selector(selector: InfobaseConfigSourceVersion) -> Self {
        Self::new(selector.version_axes()).expect("legacy selectors never supply a storage profile")
    }

    /// Returns the stable provider identity.
    pub const fn provider_id(&self) -> &ProfileId {
        &self.provider_id
    }

    /// Returns the stable Config/ConfigSave storage identity.
    pub const fn storage_profile_id(&self) -> &StorageProfileId {
        &self.storage_profile_id
    }

    /// Returns the stable DBMS family without exposing connection types.
    pub const fn dbms(&self) -> &DbmsKind {
        &self.dbms
    }

    /// Returns every independently supplied version coordinate.
    pub const fn version_axes(&self) -> &LegacyVersionAxes {
        &self.version_axes
    }

    /// Returns the exact XML dialect used by the legacy XML codecs.
    pub const fn xml_dialect(&self) -> &XmlDialect {
        self.version_axes.xml_dialect()
    }

    /// Returns the bounded, independent capability declarations.
    pub const fn capabilities(&self) -> &CapabilitySet {
        &self.capabilities
    }

    /// Evaluates one exact operation without inferring another capability.
    pub fn evaluate_capability(
        &self,
        capability: &CapabilityId,
        preservation: PreservationLevel,
        base_available: bool,
    ) -> CapabilityEvaluation {
        self.capabilities
            .evaluate(capability, preservation, base_available)
    }

    /// Narrows the exact XML dialect for calls into old closed codecs.
    pub fn legacy_selector(&self) -> Option<InfobaseConfigSourceVersion> {
        self.version_axes.legacy_selector()
    }
}

fn declaration(
    capability: CapabilityId,
    implementation: ImplementationLevel,
) -> CapabilityDeclaration {
    CapabilityDeclaration::new(capability, implementation, PreservationLevel::None)
        .expect("built-in legacy capability declaration is valid")
}

fn legacy_capabilities() -> CapabilitySet {
    CapabilitySet::new(vec![
        declaration(inspect_capability(), ImplementationLevel::Compiled),
        declaration(export_capability(), ImplementationLevel::Compiled),
        declaration(overlay_capability(), ImplementationLevel::NeedsBase),
        CapabilityDeclaration::unsupported(repack_capability()),
        CapabilityDeclaration::unsupported(bootstrap_capability()),
        CapabilityDeclaration::unsupported(convert_capability()),
    ])
    .expect("built-in legacy capabilities are unique and bounded")
}

#[cfg(test)]
mod tests {
    use ibcmd_core::capability::{
        CapabilityEvaluation, ImplementationLevel, PreservationLevel, bootstrap_capability,
        export_capability, overlay_capability,
    };

    use super::*;

    fn axes(storage_profile: Option<StorageProfileId>) -> LegacyVersionAxes {
        LegacyVersionAxes::new(
            XmlDialect::parse("2.20").unwrap(),
            None,
            None,
            storage_profile,
            None,
        )
    }

    #[test]
    fn binds_missing_storage_profile_to_the_provider_boundary() {
        let adapter = MssqlLegacyAdapter::new(axes(None)).unwrap();
        assert_eq!(
            adapter.storage_profile_id().as_str(),
            LEGACY_MSSQL_STORAGE_PROFILE_ID
        );
        assert_eq!(
            adapter
                .version_axes()
                .storage_profile()
                .map(StorageProfileId::as_str),
            Some(LEGACY_MSSQL_STORAGE_PROFILE_ID)
        );
    }

    #[test]
    fn accepts_the_exact_provider_storage_profile() {
        let fixed = StorageProfileId::parse(LEGACY_MSSQL_STORAGE_PROFILE_ID).unwrap();
        let adapter = MssqlLegacyAdapter::new(axes(Some(fixed))).unwrap();
        assert_eq!(
            adapter.storage_profile_id().as_str(),
            LEGACY_MSSQL_STORAGE_PROFILE_ID
        );
    }

    #[test]
    fn rejects_a_conflicting_storage_profile() {
        let actual = StorageProfileId::parse("storage:other").unwrap();
        let error = MssqlLegacyAdapter::new(axes(Some(actual.clone()))).unwrap_err();
        assert_eq!(error.expected().as_str(), LEGACY_MSSQL_STORAGE_PROFILE_ID);
        assert_eq!(error.actual(), &actual);
    }

    #[test]
    fn descriptor_has_stable_identity_and_no_version_inference() {
        let adapter = MssqlLegacyAdapter::from_legacy_selector(InfobaseConfigSourceVersion::V2_21);
        assert_eq!(adapter.provider_id().as_str(), LEGACY_MSSQL_PROVIDER_ID);
        assert_eq!(
            adapter.storage_profile_id().as_str(),
            LEGACY_MSSQL_STORAGE_PROFILE_ID
        );
        assert!(adapter.dbms().is_mssql());
        assert_eq!(adapter.xml_dialect().to_string(), "2.21");
        assert_eq!(adapter.version_axes().platform_build(), None);
        assert_eq!(
            adapter
                .version_axes()
                .storage_profile()
                .map(StorageProfileId::as_str),
            Some(LEGACY_MSSQL_STORAGE_PROFILE_ID)
        );
    }

    #[test]
    fn overlay_requires_base_and_bootstrap_remains_unsupported() {
        let adapter = MssqlLegacyAdapter::from_legacy_selector(InfobaseConfigSourceVersion::V2_20);
        let overlay = overlay_capability();
        assert_eq!(
            adapter.evaluate_capability(&overlay, PreservationLevel::None, false),
            CapabilityEvaluation::BaseRequired
        );
        assert_eq!(
            adapter.evaluate_capability(&overlay, PreservationLevel::None, true),
            CapabilityEvaluation::Available {
                implementation: ImplementationLevel::NeedsBase,
                preservation: PreservationLevel::None,
            }
        );
        assert!(
            adapter
                .capabilities()
                .get(&overlay)
                .unwrap()
                .requires_base_blob()
        );
        assert_eq!(
            adapter.evaluate_capability(&bootstrap_capability(), PreservationLevel::None, true),
            CapabilityEvaluation::Unsupported
        );
    }

    #[test]
    fn partial_export_does_not_imply_semantic_preservation_or_bootstrap() {
        let adapter = MssqlLegacyAdapter::from_legacy_selector(InfobaseConfigSourceVersion::V2_20);
        assert!(
            adapter
                .evaluate_capability(&export_capability(), PreservationLevel::None, false)
                .is_available()
        );
        assert_eq!(
            adapter.evaluate_capability(&export_capability(), PreservationLevel::Semantic, false),
            CapabilityEvaluation::InsufficientPreservation {
                available: PreservationLevel::None,
                requested: PreservationLevel::Semantic,
            }
        );
        assert_eq!(
            adapter.evaluate_capability(&bootstrap_capability(), PreservationLevel::None, false),
            CapabilityEvaluation::Unsupported
        );
    }
}
