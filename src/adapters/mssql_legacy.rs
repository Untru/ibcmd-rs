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

/// The two physical configuration tables exposed by the legacy MSSQL store.
///
/// This deliberately keeps the physical spelling at the adapter boundary: dump
/// orchestration must make semantic decisions from this role, rather than from
/// a SQL table name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MssqlConfigurationTableRole {
    Current,
    Saved,
}

impl MssqlConfigurationTableRole {
    /// Returns the one canonical SQL name for this storage role.
    pub(crate) const fn sql_name(self) -> &'static str {
        match self {
            Self::Current => "Config",
            Self::Saved => "ConfigSave",
        }
    }
}

/// Scope of an export inventory before it is projected into a report-specific
/// root-metadata or source-asset scope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MssqlExportInventoryScope {
    Full,
    Scoped,
}

/// One-time semantic inventory decision for a physical configuration table.
///
/// `MssqlExportInventoryPlan` is intentionally computed before either eager or
/// streamed dumping begins.  It prevents equivalent Config/ConfigSave rules
/// from drifting across row processing and report construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MssqlExportInventoryPlan {
    role: MssqlConfigurationTableRole,
    root_metadata_scope: MssqlExportInventoryScope,
    source_asset_scope: MssqlExportInventoryScope,
    config_dump_info_eligible: bool,
}

impl MssqlExportInventoryPlan {
    #[allow(clippy::too_many_arguments)]
    pub(crate) const fn new(
        role: MssqlConfigurationTableRole,
        has_selection: bool,
        extract_metadata_xml: bool,
        extract_module_text: bool,
        write_binary_rows: bool,
        allow_config_dump_info: bool,
    ) -> Self {
        let current_unfiltered_metadata = matches!(role, MssqlConfigurationTableRole::Current)
            && !has_selection
            && extract_metadata_xml;
        let root_metadata_scope = if current_unfiltered_metadata {
            MssqlExportInventoryScope::Full
        } else {
            MssqlExportInventoryScope::Scoped
        };
        let source_asset_scope = if current_unfiltered_metadata && !write_binary_rows {
            MssqlExportInventoryScope::Full
        } else {
            MssqlExportInventoryScope::Scoped
        };
        let config_dump_info_eligible = current_unfiltered_metadata
            && !write_binary_rows
            && extract_module_text
            && allow_config_dump_info;
        Self {
            role,
            root_metadata_scope,
            source_asset_scope,
            config_dump_info_eligible,
        }
    }

    pub(crate) const fn role(self) -> MssqlConfigurationTableRole {
        self.role
    }

    pub(crate) const fn root_metadata_scope(self) -> MssqlExportInventoryScope {
        self.root_metadata_scope
    }

    pub(crate) const fn source_asset_scope(self) -> MssqlExportInventoryScope {
        self.source_asset_scope
    }

    pub(crate) const fn config_dump_info_eligible(self) -> bool {
        self.config_dump_info_eligible
    }

    /// A strict root gate is meaningful only for the current configuration
    /// identity. Saved configuration is always observational.
    pub(crate) const fn is_strict_current_identity(self) -> bool {
        matches!(self.role, MssqlConfigurationTableRole::Current)
    }
}

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

/// Strictly decoded physical UUID slots 13 and 14 of a Task owner record.
///
/// These slots belong to the legacy MSSQL storage layout, not to the semantic
/// Task model.  Keep their shape and zero-UUID normalization at this adapter
/// boundary so the metadata writer receives only the already-decoded value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MssqlLegacyTaskInternalUuidSlots {
    pub(crate) field_13: Option<String>,
    pub(crate) field_14: Option<String>,
}

/// The exact Task owner slot whose physical UUID payload was malformed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MssqlLegacyTaskInternalUuidSlotError {
    field_index: usize,
}

impl MssqlLegacyTaskInternalUuidSlotError {
    pub(crate) const fn field_index(self) -> usize {
        self.field_index
    }
}

/// Decodes the two observed Task-internal UUID slots without accepting a
/// partial, non-UUID, or non-canonical payload.
pub(crate) fn decode_task_internal_uuid_slots(
    fields: &[&str],
) -> Result<MssqlLegacyTaskInternalUuidSlots, MssqlLegacyTaskInternalUuidSlotError> {
    Ok(MssqlLegacyTaskInternalUuidSlots {
        field_13: decode_task_internal_uuid_slot(fields.get(13), 13)?,
        field_14: decode_task_internal_uuid_slot(fields.get(14), 14)?,
    })
}

fn decode_task_internal_uuid_slot(
    value: Option<&&str>,
    field_index: usize,
) -> Result<Option<String>, MssqlLegacyTaskInternalUuidSlotError> {
    let uuid = value
        .map(|value| value.trim())
        .filter(|value| is_uuid(value))
        .ok_or(MssqlLegacyTaskInternalUuidSlotError { field_index })?;
    let uuid = uuid.to_ascii_lowercase();
    Ok((uuid != "00000000-0000-0000-0000-000000000000").then_some(uuid))
}

fn is_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => byte == b'-',
            _ => byte.is_ascii_hexdigit(),
        })
}

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
    fn decodes_task_internal_uuid_slots_strictly_and_normalizes_zero() {
        let zero = "00000000-0000-0000-0000-000000000000";
        let uuid = "3BAE698F-934B-414C-A2AC-A10A09D5D428";
        let mut fields = vec!["0"; 15];
        fields[13] = zero;
        fields[14] = uuid;

        assert_eq!(
            decode_task_internal_uuid_slots(&fields).unwrap(),
            MssqlLegacyTaskInternalUuidSlots {
                field_13: None,
                field_14: Some(uuid.to_ascii_lowercase()),
            }
        );

        fields[14] = "not-a-uuid";
        assert_eq!(
            decode_task_internal_uuid_slots(&fields)
                .unwrap_err()
                .field_index(),
            14
        );
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

    #[test]
    fn configuration_role_owns_the_only_physical_table_mapping() {
        assert_eq!(MssqlConfigurationTableRole::Current.sql_name(), "Config");
        assert_eq!(MssqlConfigurationTableRole::Saved.sql_name(), "ConfigSave");
    }

    #[test]
    fn export_inventory_plan_matrix_is_closed_over_role_and_options() {
        for role in [
            MssqlConfigurationTableRole::Current,
            MssqlConfigurationTableRole::Saved,
        ] {
            for has_selection in [false, true] {
                for metadata in [false, true] {
                    for module in [false, true] {
                        for binary in [false, true] {
                            for allow_dump_info in [false, true] {
                                let plan = MssqlExportInventoryPlan::new(
                                    role,
                                    has_selection,
                                    metadata,
                                    module,
                                    binary,
                                    allow_dump_info,
                                );
                                let full_root = role == MssqlConfigurationTableRole::Current
                                    && !has_selection
                                    && metadata;
                                assert_eq!(plan.role(), role);
                                assert_eq!(
                                    plan.root_metadata_scope(),
                                    if full_root {
                                        MssqlExportInventoryScope::Full
                                    } else {
                                        MssqlExportInventoryScope::Scoped
                                    }
                                );
                                assert_eq!(
                                    plan.source_asset_scope(),
                                    if full_root && !binary {
                                        MssqlExportInventoryScope::Full
                                    } else {
                                        MssqlExportInventoryScope::Scoped
                                    }
                                );
                                assert_eq!(
                                    plan.config_dump_info_eligible(),
                                    full_root && !binary && module && allow_dump_info
                                );
                                assert_eq!(
                                    plan.is_strict_current_identity(),
                                    role == MssqlConfigurationTableRole::Current
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
