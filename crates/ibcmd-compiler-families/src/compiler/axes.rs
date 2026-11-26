//! Independent coordinates of a native compiler invocation.

use ibcmd_core::artifact::StorageProfileId;
use ibcmd_core::version::{CompatibilityMode, ContainerRevision, PlatformBuild, XmlDialect};

/// Independent coordinates used by one compiler invocation.
///
/// No coordinate is inferred from another. The legacy selector accepts only
/// its exact verified coordinates and fails closed for every future value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileAxes {
    xml_dialect: XmlDialect,
    platform_build: Option<PlatformBuild>,
    compatibility_mode: Option<CompatibilityMode>,
    storage_profile: StorageProfileId,
    container_revision: Option<ContainerRevision>,
}

impl CompileAxes {
    /// Creates an exact compiler coordinate set without deriving missing axes.
    pub fn new(
        xml_dialect: XmlDialect,
        platform_build: Option<PlatformBuild>,
        compatibility_mode: Option<CompatibilityMode>,
        storage_profile: StorageProfileId,
        container_revision: Option<ContainerRevision>,
    ) -> Self {
        Self {
            xml_dialect,
            platform_build,
            compatibility_mode,
            storage_profile,
            container_revision,
        }
    }

    /// Returns the exact source XML dialect.
    pub const fn xml_dialect(&self) -> &XmlDialect {
        &self.xml_dialect
    }

    /// Returns the independently supplied platform build, when known.
    pub const fn platform_build(&self) -> Option<&PlatformBuild> {
        self.platform_build.as_ref()
    }

    /// Returns the independently supplied compatibility mode, when known.
    pub const fn compatibility_mode(&self) -> Option<&CompatibilityMode> {
        self.compatibility_mode.as_ref()
    }

    /// Returns the exact target storage profile.
    pub const fn storage_profile(&self) -> &StorageProfileId {
        &self.storage_profile
    }

    /// Returns the independently supplied physical-container revision.
    pub const fn container_revision(&self) -> Option<&ContainerRevision> {
        self.container_revision.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn explicit_axes() -> CompileAxes {
        CompileAxes::new(
            XmlDialect::parse("2.20").unwrap(),
            Some(PlatformBuild::parse("8.3.27.1989").unwrap()),
            Some(CompatibilityMode::parse("Version8_3_24").unwrap()),
            StorageProfileId::parse("storage:mssql-test").unwrap(),
            Some(ContainerRevision::parse("revision-1").unwrap()),
        )
    }

    #[test]
    fn explicit_axes_remain_independent() {
        let axes = explicit_axes();
        assert_eq!(axes.xml_dialect().to_string(), "2.20");
        assert_eq!(
            axes.platform_build().map(ToString::to_string).as_deref(),
            Some("8.3.27.1989")
        );
        assert_eq!(
            axes.compatibility_mode()
                .map(ToString::to_string)
                .as_deref(),
            Some("Version8_3_24")
        );
        assert_eq!(axes.storage_profile().as_str(), "storage:mssql-test");
        assert_eq!(
            axes.container_revision()
                .map(ToString::to_string)
                .as_deref(),
            Some("revision-1")
        );
    }
}
