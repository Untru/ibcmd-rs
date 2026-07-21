//! Explicit bridge from historical CLI selectors to independent core axes.

use clap::ValueEnum;
use ibcmd_core::artifact::StorageProfileId;
use ibcmd_core::version::{CompatibilityMode, ContainerRevision, PlatformBuild, XmlDialect};

/// Historical closed XML selector retained for CLI and legacy codec compatibility.
///
/// Platform-looking aliases remain accepted only because old commands exposed
/// them on `--source-version`. They select an XML dialect and never populate a
/// platform build or any other independent version axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum InfobaseConfigSourceVersion {
    /// Legacy XML dialect 2.20.
    #[value(name = "2.20", alias = "20", alias = "8.3", alias = "8.3.27")]
    V2_20,
    /// Legacy XML dialect 2.21.
    #[value(name = "2.21", alias = "21", alias = "8.5", alias = "8.5.1")]
    V2_21,
}

impl InfobaseConfigSourceVersion {
    /// Returns the historical canonical CLI value.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::V2_20 => "2.20",
            Self::V2_21 => "2.21",
        }
    }

    /// Converts the closed selector immediately into independent typed axes.
    pub fn version_axes(self) -> LegacyVersionAxes {
        LegacyVersionAxes::new(
            XmlDialect::parse(self.as_str()).expect("legacy XML dialects are valid"),
            None,
            None,
            None,
            None,
        )
    }
}

/// Independent coordinates carried across the root legacy boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyVersionAxes {
    xml_dialect: XmlDialect,
    platform_build: Option<PlatformBuild>,
    compatibility_mode: Option<CompatibilityMode>,
    storage_profile: Option<StorageProfileId>,
    container_revision: Option<ContainerRevision>,
}

impl LegacyVersionAxes {
    /// Creates separated axes without deriving any coordinate from another.
    pub const fn new(
        xml_dialect: XmlDialect,
        platform_build: Option<PlatformBuild>,
        compatibility_mode: Option<CompatibilityMode>,
        storage_profile: Option<StorageProfileId>,
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

    /// Returns the exact XML dialect used by the legacy XML path.
    pub const fn xml_dialect(&self) -> &XmlDialect {
        &self.xml_dialect
    }

    /// Returns an independently supplied platform build, when present.
    pub const fn platform_build(&self) -> Option<&PlatformBuild> {
        self.platform_build.as_ref()
    }

    /// Returns an independently supplied compatibility mode, when present.
    pub const fn compatibility_mode(&self) -> Option<&CompatibilityMode> {
        self.compatibility_mode.as_ref()
    }

    /// Returns an independently supplied logical storage profile, when present.
    pub const fn storage_profile(&self) -> Option<&StorageProfileId> {
        self.storage_profile.as_ref()
    }

    /// Binds the storage coordinate at a provider boundary.
    pub(crate) fn with_storage_profile(mut self, storage_profile: StorageProfileId) -> Self {
        self.storage_profile = Some(storage_profile);
        self
    }

    /// Returns an independently supplied container revision, when present.
    pub const fn container_revision(&self) -> Option<&ContainerRevision> {
        self.container_revision.as_ref()
    }

    /// Narrows a supported exact dialect back into the old codec selector.
    ///
    /// This is a legacy-only bridge, not platform inference or migration.
    pub fn legacy_selector(&self) -> Option<InfobaseConfigSourceVersion> {
        match self.xml_dialect.as_version().components() {
            [2, 20] => Some(InfobaseConfigSourceVersion::V2_20),
            [2, 21] => Some(InfobaseConfigSourceVersion::V2_21),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_alias(value: &str) -> InfobaseConfigSourceVersion {
        <InfobaseConfigSourceVersion as ValueEnum>::from_str(value, true).unwrap()
    }

    #[test]
    fn historical_names_and_platform_shaped_aliases_only_select_xml() {
        for alias in ["2.20", "20", "8.3", "8.3.27"] {
            let axes = parse_alias(alias).version_axes();
            assert_eq!(axes.xml_dialect().to_string(), "2.20");
            assert_eq!(axes.platform_build(), None);
            assert_eq!(axes.compatibility_mode(), None);
            assert_eq!(axes.storage_profile(), None);
            assert_eq!(axes.container_revision(), None);
        }
        for alias in ["2.21", "21", "8.5", "8.5.1"] {
            let axes = parse_alias(alias).version_axes();
            assert_eq!(axes.xml_dialect().to_string(), "2.21");
            assert_eq!(axes.platform_build(), None);
        }
    }

    #[test]
    fn independently_supplied_platform_build_does_not_change_xml() {
        let axes = LegacyVersionAxes::new(
            XmlDialect::parse("2.20").unwrap(),
            Some(PlatformBuild::parse("8.5.1.1150").unwrap()),
            None,
            None,
            None,
        );
        assert_eq!(axes.xml_dialect().to_string(), "2.20");
        assert_eq!(
            axes.platform_build().map(ToString::to_string).as_deref(),
            Some("8.5.1.1150")
        );
        assert_eq!(
            axes.legacy_selector(),
            Some(InfobaseConfigSourceVersion::V2_20)
        );
    }
}
