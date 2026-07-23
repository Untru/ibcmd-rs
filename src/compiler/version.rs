//! Profile-gated codec for the native `version` service entry.

use std::error::Error;
use std::fmt::{self, Display, Formatter, Write as _};

use ibcmd_core::artifact::ProfileId;
use ibcmd_core::profile::EffectiveProfile;
use ibcmd_core::storage::StoragePatchEntry;

use super::graph::{BootstrapGraph, SpecialEntryKind};
use super::root::{SpecialEntryBuildError, compiled_special_entry, ensure_graph_profile};

const ROOT_LAYOUT_KEY: &str = "bootstrap.root.layout";
const CONFIGURATION_LAYOUT_KEY: &str = "bootstrap.configuration.layout";
const VERSION_LAYOUT_KEY: &str = "bootstrap.version.layout";
const VERSIONS_LAYOUT_KEY: &str = "bootstrap.versions.layout";
const VERSION_COMPATIBILITY_KEY: &str = "bootstrap.version.compatibility";

const ROOT_LAYOUT: &str = "root-v2-empty-tail";
const CONFIGURATION_LAYOUT: &str = "configuration-v68-seven-sections-v1";
const VERSION_LAYOUT: &str = "version-v216";
const VERSIONS_LAYOUT: &str = "versions-v1";

/// Failure to select an exact, evidence-backed special-entry layout.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SpecialEntryProfileError {
    /// A required constant is absent from the effective target profile.
    MissingConstant {
        /// Selected profile.
        profile: ProfileId,
        /// Missing constant name.
        key: &'static str,
    },
    /// A layout constant is present but is not implemented by this codec.
    UnsupportedLayout {
        /// Selected profile.
        profile: ProfileId,
        /// Constant name.
        key: &'static str,
        /// Exact unsupported value.
        value: String,
    },
    /// The compatibility selector is not a canonical positive decimal value.
    InvalidCompatibility {
        /// Selected profile.
        profile: ProfileId,
        /// Exact invalid value.
        value: String,
    },
}

impl Display for SpecialEntryProfileError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingConstant { profile, key } => {
                write!(
                    formatter,
                    "profile `{profile}` has no required constant `{key}`"
                )
            }
            Self::UnsupportedLayout {
                profile,
                key,
                value,
            } => write!(
                formatter,
                "profile `{profile}` declares unsupported `{key}` value `{value}`"
            ),
            Self::InvalidCompatibility { profile, value } => write!(
                formatter,
                "profile `{profile}` declares invalid `{VERSION_COMPATIBILITY_KEY}` value `{value}`"
            ),
        }
    }
}

impl Error for SpecialEntryProfileError {}

/// Exact profile projection needed by all three special-entry codecs.
///
/// Construction validates the complete layout cohort up front. Callers cannot
/// accidentally combine a supported `root` layout with an unknown `versions`
/// layout from a future platform profile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpecialEntryProfile {
    profile_id: ProfileId,
    compatibility: u32,
}

impl SpecialEntryProfile {
    /// Selects the implemented special-entry cohort from an effective profile.
    pub fn from_effective(profile: &EffectiveProfile) -> Result<Self, SpecialEntryProfileError> {
        require_layout(profile, ROOT_LAYOUT_KEY, ROOT_LAYOUT)?;
        require_layout(profile, VERSION_LAYOUT_KEY, VERSION_LAYOUT)?;
        require_layout(profile, VERSIONS_LAYOUT_KEY, VERSIONS_LAYOUT)?;
        require_layout(profile, CONFIGURATION_LAYOUT_KEY, CONFIGURATION_LAYOUT)?;

        let compatibility = required_constant(profile, VERSION_COMPATIBILITY_KEY)?;
        let parsed = compatibility.parse::<u32>().ok().filter(|value| *value > 0);
        let compatibility = match parsed {
            Some(value) if value.to_string() == compatibility => value,
            _ => {
                return Err(SpecialEntryProfileError::InvalidCompatibility {
                    profile: profile.id.clone(),
                    value: compatibility.to_owned(),
                });
            }
        };

        Ok(Self {
            profile_id: profile.id.clone(),
            compatibility,
        })
    }

    /// Returns the exact profile that supplied the layout contract.
    pub const fn profile_id(&self) -> &ProfileId {
        &self.profile_id
    }

    /// Returns the exact native compatibility selector written to `version`.
    pub const fn compatibility(&self) -> u32 {
        self.compatibility
    }

    #[cfg(test)]
    pub(crate) fn fixture(profile_id: &str, compatibility: u32) -> Self {
        Self {
            profile_id: ProfileId::parse(profile_id).unwrap(),
            compatibility,
        }
    }
}

fn required_constant<'a>(
    profile: &'a EffectiveProfile,
    key: &'static str,
) -> Result<&'a str, SpecialEntryProfileError> {
    profile
        .constants
        .get(key)
        .map(|constant| constant.value.as_str())
        .ok_or_else(|| SpecialEntryProfileError::MissingConstant {
            profile: profile.id.clone(),
            key,
        })
}

fn require_layout(
    profile: &EffectiveProfile,
    key: &'static str,
    expected: &'static str,
) -> Result<(), SpecialEntryProfileError> {
    let value = required_constant(profile, key)?;
    if value == expected {
        Ok(())
    } else {
        Err(SpecialEntryProfileError::UnsupportedLayout {
            profile: profile.id.clone(),
            key,
            value: value.to_owned(),
        })
    }
}

/// Compiles the exact evidence-backed native `version` row.
pub fn compile_version(
    graph: &BootstrapGraph,
    profile: &SpecialEntryProfile,
) -> Result<StoragePatchEntry, SpecialEntryBuildError> {
    ensure_graph_profile(graph, profile)?;
    graph.validate_special_references()?;
    let mut plaintext = String::with_capacity(40);
    plaintext.push('\u{feff}');
    write!(
        &mut plaintext,
        "{{\r\n{{216,0,\r\n{{{},0}}\r\n}}\r\n}}",
        profile.compatibility()
    )
    .expect("writing to String cannot fail");
    compiled_special_entry(SpecialEntryKind::Version, plaintext.as_bytes(), profile)
}

#[cfg(test)]
mod tests {
    use ibcmd_core::profile::{ProfileSourceKind, parse_profile_source, resolve_profiles};

    use crate::compiler::root::inflate_for_test;

    use super::*;

    fn effective(constants: &str) -> EffectiveProfile {
        let json = format!(
            r#"{{
                "schema_version": 1,
                "id": "platform-test",
                "status": "experimental",
                "constants": {constants}
            }}"#
        );
        let document =
            parse_profile_source("fixture.json", ProfileSourceKind::Bundled, &json).unwrap();
        resolve_profiles([document])
            .unwrap()
            .get(&ProfileId::parse("platform-test").unwrap())
            .unwrap()
            .clone()
    }

    fn valid_constants() -> &'static str {
        r#"{
            "bootstrap.root.layout": "root-v2-empty-tail",
            "bootstrap.configuration.layout": "configuration-v68-seven-sections-v1",
            "bootstrap.version.layout": "version-v216",
            "bootstrap.versions.layout": "versions-v1",
            "bootstrap.version.compatibility": "80327"
        }"#
    }

    #[test]
    fn effective_profile_requires_the_complete_exact_layout_cohort() {
        let profile = SpecialEntryProfile::from_effective(&effective(valid_constants())).unwrap();
        assert_eq!(profile.profile_id().as_str(), "platform-test");
        assert_eq!(profile.compatibility(), 80_327);

        let missing = effective(
            r#"{
                "bootstrap.root.layout": "root-v2-empty-tail",
                "bootstrap.version.layout": "version-v216",
                "bootstrap.version.compatibility": "80327"
            }"#,
        );
        assert!(matches!(
            SpecialEntryProfile::from_effective(&missing),
            Err(SpecialEntryProfileError::MissingConstant {
                key: VERSIONS_LAYOUT_KEY,
                ..
            })
        ));

        let future = effective(&valid_constants().replace("version-v216", "version-v217"));
        assert!(matches!(
            SpecialEntryProfile::from_effective(&future),
            Err(SpecialEntryProfileError::UnsupportedLayout {
                key: VERSION_LAYOUT_KEY,
                ..
            })
        ));
    }

    #[test]
    fn only_the_evidenced_bundled_platform_profile_selects_this_cohort() {
        let registry = crate::profile_registry::load_bundled_profile_registry().unwrap();
        let supported = registry
            .get(&ProfileId::parse("platform-8.3.27.1989").unwrap())
            .unwrap();
        let selected = SpecialEntryProfile::from_effective(supported).unwrap();
        assert_eq!(selected.compatibility(), 80_327);

        for profile in ["platform-8.3.24.1819", "platform-8.5.1.1150"] {
            let effective = registry.get(&ProfileId::parse(profile).unwrap()).unwrap();
            assert!(matches!(
                SpecialEntryProfile::from_effective(effective),
                Err(SpecialEntryProfileError::MissingConstant { .. })
            ));
        }
    }

    #[test]
    fn version_plaintext_matches_the_exact_native_golden() {
        use ibcmd_core::artifact::ProfileId;
        use ibcmd_core::diagnostic::{ObjectPath, PathSegment, PropertyPath};
        use ibcmd_core::identity::{LogicalIdentity, ObjectUuid};
        use ibcmd_core::model::{
            CanonicalConfiguration, CanonicalObject, CanonicalObjectParts, MetadataKind,
        };
        use ibcmd_core::provenance::{CanonicalAnchor, SourceProvenance};
        use ibcmd_core::validate::validate_configuration;

        use crate::compiler::graph::{ObjectStorageRoute, build_bootstrap_graph};
        use crate::compiler::identity::collect_bootstrap_identities;

        let uuid = ObjectUuid::parse("61ee2494-c14a-4992-8c93-8e78b20bea27").unwrap();
        let path = ObjectPath::new(vec![PathSegment::name("configuration").unwrap()]).unwrap();
        let provenance = SourceProvenance::new(
            ProfileId::parse("profile:test").unwrap(),
            CanonicalAnchor::new(path.clone(), PropertyPath::root()),
        );
        let object = CanonicalObject::new(CanonicalObjectParts::new(
            LogicalIdentity::new(uuid, path),
            MetadataKind::new("Configuration").unwrap(),
            provenance,
        ))
        .unwrap();
        let configuration = CanonicalConfiguration::new(vec![object]).unwrap();
        let validated = validate_configuration(&configuration).unwrap();
        let identities = collect_bootstrap_identities(&validated).unwrap();
        let graph = build_bootstrap_graph(
            &identities,
            ProfileId::parse("platform-test").unwrap(),
            vec![ObjectStorageRoute::new(uuid, Vec::new()).unwrap()],
        )
        .unwrap();

        let first = compile_version(
            &graph,
            &SpecialEntryProfile::fixture("platform-test", 80_327),
        )
        .unwrap();
        let second = compile_version(
            &graph,
            &SpecialEntryProfile::fixture("platform-test", 80_327),
        )
        .unwrap();
        assert_eq!(first, second);
        assert_eq!(
            inflate_for_test(first.outcome().compiled_payload().unwrap().bytes()),
            b"\xef\xbb\xbf{\r\n{216,0,\r\n{80327,0}\r\n}\r\n}"
        );
    }
}
