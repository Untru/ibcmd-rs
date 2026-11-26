//! The body-profile selection the root's profile-gated body codecs share:
//! a codec admits an effective profile only when its storage profile is
//! the one supported and the profile declares the codec's layout constant.
//! With it, the codecs of the command interface and of the interface assets
//! beside it, which read nothing else of the root.

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use ibcmd_core::artifact::{ProfileId, StorageProfileId};
use ibcmd_core::profile::EffectiveProfile;
use ibcmd_core::version::PlatformBuild;

pub mod command_interface;
pub mod interface_assets;

const SUPPORTED_STORAGE_PROFILE: &str = "storage:mssql-config-configsave";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectedBodyProfile {
    profile_id: ProfileId,
    #[allow(dead_code)]
    platform_build: PlatformBuild,
    #[allow(dead_code)]
    storage_profile: StorageProfileId,
}

impl SelectedBodyProfile {
    pub fn from_effective(
        profile: &EffectiveProfile,
        key: &'static str,
        expected: &'static str,
    ) -> Result<Self, BodyProfileError> {
        let platform_build = profile
            .platform_build
            .as_ref()
            .map(|value| value.value.clone())
            .ok_or_else(|| BodyProfileError::MissingCoordinate {
                profile: profile.id.clone(),
                coordinate: "platform_build",
            })?;
        let storage_profile = profile
            .storage_profile
            .as_ref()
            .map(|value| value.value.clone())
            .ok_or_else(|| BodyProfileError::MissingCoordinate {
                profile: profile.id.clone(),
                coordinate: "storage_profile",
            })?;
        if storage_profile.as_str() != SUPPORTED_STORAGE_PROFILE {
            return Err(BodyProfileError::UnsupportedCoordinate {
                profile: profile.id.clone(),
                coordinate: "storage_profile",
                value: storage_profile.to_string(),
            });
        }
        let actual =
            profile
                .constants
                .get(key)
                .ok_or_else(|| BodyProfileError::MissingConstant {
                    profile: profile.id.clone(),
                    key,
                })?;
        if actual.value != expected {
            return Err(BodyProfileError::UnsupportedLayout {
                profile: profile.id.clone(),
                key,
                value: actual.value.clone(),
            });
        }
        Ok(Self {
            profile_id: profile.id.clone(),
            platform_build,
            storage_profile,
        })
    }

    pub const fn profile_id(&self) -> &ProfileId {
        &self.profile_id
    }

    #[cfg(any(test, feature = "test-fixtures"))]
    pub fn fixture(profile_id: &str) -> Self {
        Self {
            profile_id: ProfileId::parse(profile_id).unwrap(),
            platform_build: PlatformBuild::parse("8.3.27.1989").unwrap(),
            storage_profile: StorageProfileId::parse(SUPPORTED_STORAGE_PROFILE).unwrap(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BodyProfileError {
    MissingCoordinate {
        profile: ProfileId,
        coordinate: &'static str,
    },
    MissingConstant {
        profile: ProfileId,
        key: &'static str,
    },
    UnsupportedCoordinate {
        profile: ProfileId,
        coordinate: &'static str,
        value: String,
    },
    UnsupportedLayout {
        profile: ProfileId,
        key: &'static str,
        value: String,
    },
}

impl Display for BodyProfileError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCoordinate {
                profile,
                coordinate,
            } => write!(
                formatter,
                "profile `{profile}` has no `{coordinate}` coordinate"
            ),
            Self::MissingConstant { profile, key } => {
                write!(formatter, "profile `{profile}` has no `{key}` constant")
            }
            Self::UnsupportedCoordinate {
                profile,
                coordinate,
                value,
            } => write!(
                formatter,
                "profile `{profile}` declares unsupported `{coordinate}` value `{value}`"
            ),
            Self::UnsupportedLayout {
                profile,
                key,
                value,
            } => write!(
                formatter,
                "profile `{profile}` declares unsupported body layout `{key}={value}`"
            ),
        }
    }
}

impl Error for BodyProfileError {}
