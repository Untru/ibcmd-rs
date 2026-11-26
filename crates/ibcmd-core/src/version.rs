//! Independent version axes used by standalone conversion profiles.

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::marker::PhantomData;
use std::str::FromStr;

use serde::de::Visitor;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

const MIN_DOTTED_COMPONENTS: usize = 2;
const MAX_DOTTED_COMPONENTS: usize = 8;
const MAX_DOTTED_COMPONENT_DIGITS: usize = 10;
const MAX_IDENTIFIER_BYTES: usize = 64;

/// An open, canonical, numerically ordered dotted version.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DottedVersion(Box<[u32]>);

impl DottedVersion {
    /// Builds a dotted version from two to eight numeric components.
    pub fn new(components: Vec<u32>) -> Result<Self, ParseDottedVersionError> {
        validate_component_count(components.len())?;
        Ok(Self(components.into_boxed_slice()))
    }

    /// Returns the numeric components in source order.
    pub fn components(&self) -> &[u32] {
        &self.0
    }
}

impl Display for DottedVersion {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        for (index, component) in self.0.iter().enumerate() {
            if index != 0 {
                formatter.write_str(".")?;
            }
            component.fmt(formatter)?;
        }
        Ok(())
    }
}

impl FromStr for DottedVersion {
    type Err = ParseDottedVersionError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let mut components = Vec::with_capacity(MAX_DOTTED_COMPONENTS);
        for (index, raw_component) in input.split('.').enumerate() {
            if index == MAX_DOTTED_COMPONENTS {
                return Err(ParseDottedVersionError::new(format!(
                    "a dotted version must contain at most {MAX_DOTTED_COMPONENTS} components"
                )));
            }
            if raw_component.is_empty() {
                return Err(ParseDottedVersionError::component(
                    index,
                    "component is empty",
                ));
            }
            if raw_component.len() > MAX_DOTTED_COMPONENT_DIGITS {
                return Err(ParseDottedVersionError::component(
                    index,
                    "component contains more than 10 digits",
                ));
            }
            if !raw_component.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(ParseDottedVersionError::component(
                    index,
                    "component must contain ASCII digits only",
                ));
            }
            if raw_component.len() > 1 && raw_component.starts_with('0') {
                return Err(ParseDottedVersionError::component(
                    index,
                    "leading zeroes are not allowed",
                ));
            }
            let component = raw_component
                .parse::<u32>()
                .map_err(|_| ParseDottedVersionError::component(index, "component exceeds u32"))?;
            components.push(component);
        }
        validate_component_count(components.len())?;
        Ok(Self(components.into_boxed_slice()))
    }
}

fn validate_component_count(count: usize) -> Result<(), ParseDottedVersionError> {
    if (MIN_DOTTED_COMPONENTS..=MAX_DOTTED_COMPONENTS).contains(&count) {
        Ok(())
    } else {
        Err(ParseDottedVersionError::new(format!(
            "a dotted version must contain {MIN_DOTTED_COMPONENTS} to {MAX_DOTTED_COMPONENTS} components, found {count}"
        )))
    }
}

/// Error returned when parsing a [`DottedVersion`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseDottedVersionError {
    message: String,
}

impl ParseDottedVersionError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    fn component(index: usize, message: &str) -> Self {
        Self::new(format!("component {}: {message}", index + 1))
    }
}

impl Display for ParseDottedVersionError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for ParseDottedVersionError {}

macro_rules! dotted_axis {
    ($name:ident, $documentation:literal) => {
        #[doc = $documentation]
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(DottedVersion);

        impl $name {
            /// Builds this axis from an already validated dotted version.
            pub fn new(version: DottedVersion) -> Self {
                Self(version)
            }

            /// Parses this axis without inferring any other version axis.
            pub fn parse(input: &str) -> Result<Self, ParseDottedVersionError> {
                input.parse()
            }

            /// Returns the validated dotted version carried by this axis.
            pub fn as_version(&self) -> &DottedVersion {
                &self.0
            }
        }

        impl Display for $name {
            fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }

        impl FromStr for $name {
            type Err = ParseDottedVersionError;

            fn from_str(input: &str) -> Result<Self, Self::Err> {
                Ok(Self::new(input.parse()?))
            }
        }
    };
}

dotted_axis!(
    PlatformBuild,
    "A concrete platform build, independent of dialect and compatibility mode."
);
dotted_axis!(
    XmlDialect,
    "An XML/XCF serialization dialect, independent of the platform build."
);

/// An open, lossless compatibility-mode identifier.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompatibilityMode(String);

impl CompatibilityMode {
    /// Builds a compatibility mode after validating its borrowed identifier.
    pub fn new(identifier: &str) -> Result<Self, ParseCompatibilityModeError> {
        validate_identifier(identifier).map_err(ParseCompatibilityModeError::new)?;
        Ok(Self(identifier.to_owned()))
    }

    /// Parses this axis without inferring any other version axis.
    pub fn parse(input: &str) -> Result<Self, ParseCompatibilityModeError> {
        input.parse()
    }

    /// Returns the compatibility-mode identifier.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for CompatibilityMode {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl FromStr for CompatibilityMode {
    type Err = ParseCompatibilityModeError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Self::new(input)
    }
}

/// Error returned when parsing a [`CompatibilityMode`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseCompatibilityModeError {
    message: &'static str,
}

impl ParseCompatibilityModeError {
    fn new(message: &'static str) -> Self {
        Self { message }
    }
}

impl Display for ParseCompatibilityModeError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message)
    }
}

impl Error for ParseCompatibilityModeError {}

/// An open native-storage version number.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StorageVersion(u32);

impl StorageVersion {
    /// Builds a storage version without imposing a closed set of known values.
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    /// Parses a canonical unsigned decimal storage version.
    pub fn parse(input: &str) -> Result<Self, ParseStorageVersionError> {
        input.parse()
    }

    /// Returns the numeric storage version.
    pub const fn value(self) -> u32 {
        self.0
    }
}

impl Display for StorageVersion {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for StorageVersion {
    type Err = ParseStorageVersionError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        if input.is_empty() {
            return Err(ParseStorageVersionError::new("storage version is empty"));
        }
        if !input.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(ParseStorageVersionError::new(
                "storage version must contain ASCII digits only",
            ));
        }
        if input.len() > 1 && input.starts_with('0') {
            return Err(ParseStorageVersionError::new(
                "storage version must not contain leading zeroes",
            ));
        }
        input
            .parse::<u32>()
            .map(Self::new)
            .map_err(|_| ParseStorageVersionError::new("storage version exceeds u32"))
    }
}

/// Error returned when parsing a [`StorageVersion`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseStorageVersionError {
    message: &'static str,
}

impl ParseStorageVersionError {
    fn new(message: &'static str) -> Self {
        Self { message }
    }
}

impl Display for ParseStorageVersionError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message)
    }
}

impl Error for ParseStorageVersionError {}

/// An open physical-container revision identifier.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ContainerRevision(String);

impl ContainerRevision {
    /// Builds a revision after validating its borrowed identifier.
    pub fn new(identifier: &str) -> Result<Self, ParseContainerRevisionError> {
        validate_identifier(identifier).map_err(ParseContainerRevisionError::new)?;
        Ok(Self(identifier.to_owned()))
    }

    /// Parses a validated revision identifier.
    pub fn parse(input: &str) -> Result<Self, ParseContainerRevisionError> {
        input.parse()
    }

    /// Returns the revision identifier.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for ContainerRevision {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl FromStr for ContainerRevision {
    type Err = ParseContainerRevisionError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Self::new(input)
    }
}

fn validate_identifier(input: &str) -> Result<(), &'static str> {
    if input.is_empty() {
        return Err("identifier is empty");
    }
    if input.len() > MAX_IDENTIFIER_BYTES {
        return Err("identifier exceeds 64 bytes");
    }
    let mut bytes = input.bytes();
    if !bytes.next().is_some_and(|byte| byte.is_ascii_alphabetic()) {
        return Err("identifier must start with an ASCII letter");
    }
    if !bytes.all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_')) {
        return Err("identifier contains an invalid character");
    }
    Ok(())
}

/// Error returned when parsing a [`ContainerRevision`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseContainerRevisionError {
    message: &'static str,
}

impl ParseContainerRevisionError {
    fn new(message: &'static str) -> Self {
        Self { message }
    }
}

impl Display for ParseContainerRevisionError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message)
    }
}

impl Error for ParseContainerRevisionError {}

macro_rules! serde_string {
    ($type:ty) => {
        impl Serialize for $type {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.collect_str(self)
            }
        }

        impl<'de> Deserialize<'de> for $type {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                deserializer.deserialize_str(ParseStringVisitor::<$type>(PhantomData))
            }
        }
    };
}

struct ParseStringVisitor<T>(PhantomData<fn() -> T>);

impl<'de, T> Visitor<'de> for ParseStringVisitor<T>
where
    T: FromStr,
    T::Err: Display,
{
    type Value = T;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("a valid version string")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        value.parse().map_err(E::custom)
    }
}

serde_string!(DottedVersion);
serde_string!(PlatformBuild);
serde_string!(XmlDialect);
serde_string!(CompatibilityMode);
serde_string!(StorageVersion);
serde_string!(ContainerRevision);

#[cfg(test)]
mod tests {
    use std::any::TypeId;

    use super::*;

    #[test]
    fn parses_required_dotted_versions_canonically() {
        for value in ["2.17", "2.20", "2.21"] {
            let dialect: XmlDialect = value.parse().unwrap();
            assert_eq!(dialect.to_string(), value);
        }
        for value in ["8.3.24.1819", "8.3.27.1989", "8.5.1.1150", "9.1.0.42"] {
            let build: PlatformBuild = value.parse().unwrap();
            assert_eq!(build.to_string(), value);
        }
    }

    #[test]
    fn dotted_versions_use_numeric_total_ordering() {
        let mut versions = [
            "9.1.0.42",
            "2.20",
            "8.3.27.1989",
            "2.9",
            "2.17",
            "2.1.0",
            "2.1",
        ]
        .map(|value| value.parse::<DottedVersion>().unwrap());
        versions.sort();
        assert_eq!(
            versions.map(|version| version.to_string()),
            [
                "2.1",
                "2.1.0",
                "2.9",
                "2.17",
                "2.20",
                "8.3.27.1989",
                "9.1.0.42"
            ]
        );
    }

    #[test]
    fn rejects_invalid_dotted_versions() {
        for value in [
            "",
            "2",
            "1.2.3.4.5.6.7.8.9",
            ".17",
            "2.",
            "2..17",
            " 2.17",
            "2.17 ",
            "+2.17",
            "-2.17",
            "02.17",
            "2.017",
            "2.4294967296",
            "2.12345678901",
        ] {
            assert!(value.parse::<DottedVersion>().is_err(), "{value}");
        }

        let many_components = format!("{}1", "1.".repeat(10_000));
        let error = many_components.parse::<DottedVersion>().unwrap_err();
        assert!(error.to_string().contains("at most 8 components"));
    }

    #[test]
    fn dotted_axes_are_distinct_types_with_explicit_access() {
        let raw: DottedVersion = "8.3.24.1819".parse().unwrap();
        let platform = PlatformBuild::new(raw.clone());
        let dialect = XmlDialect::parse("2.20").unwrap();
        let compatibility = CompatibilityMode::parse("Version8_3_20").unwrap();

        assert_eq!(platform.as_version(), &raw);
        assert_eq!(dialect.as_version().to_string(), "2.20");
        assert_eq!(compatibility.as_str(), "Version8_3_20");
        assert_ne!(TypeId::of::<PlatformBuild>(), TypeId::of::<XmlDialect>());
        assert_ne!(
            TypeId::of::<XmlDialect>(),
            TypeId::of::<CompatibilityMode>()
        );
    }

    #[test]
    fn parse_errors_implement_error() {
        fn assert_error<T: Error>() {}

        assert_error::<ParseDottedVersionError>();
        assert_error::<ParseCompatibilityModeError>();
        assert_error::<ParseStorageVersionError>();
        assert_error::<ParseContainerRevisionError>();
    }

    #[test]
    fn storage_version_is_open_and_canonical() {
        for value in [0, 1, 2, 42, u32::MAX] {
            let version = StorageVersion::new(value);
            assert_eq!(version.to_string(), value.to_string());
            assert_eq!(
                version.to_string().parse::<StorageVersion>().unwrap(),
                version
            );
        }
        for value in ["", " 2", "+2", "02", "4294967296"] {
            assert!(value.parse::<StorageVersion>().is_err(), "{value}");
        }
    }

    #[test]
    fn compatibility_mode_accepts_real_and_future_identifiers() {
        for value in ["Version8_3_20", "Version8_3_27", "Version9_1_Future"] {
            let mode: CompatibilityMode = value.parse().unwrap();
            assert_eq!(mode.as_str(), value);
            assert_eq!(mode.to_string(), value);
        }
        for value in ["", " Version8_3_20", "8_3_20", "Version8/3"] {
            assert!(value.parse::<CompatibilityMode>().is_err(), "{value}");
        }
    }

    #[test]
    fn container_revision_accepts_known_and_future_identifiers() {
        for value in ["Format15", "Format16", "Format17", "FutureFormat-2.1"] {
            let revision: ContainerRevision = value.parse().unwrap();
            assert_eq!(revision.as_str(), value);
            assert_eq!(revision.to_string(), value);
        }
        for value in ["", " Format15", "15Format", "Format 16", "Format/17"] {
            assert!(value.parse::<ContainerRevision>().is_err(), "{value}");
        }
    }

    #[test]
    fn identifiers_enforce_the_same_64_byte_bound() {
        let accepted = format!("V{}", "1".repeat(63));
        let rejected = format!("V{}", "1".repeat(64));

        assert_eq!(accepted.len(), 64);
        assert_eq!(rejected.len(), 65);
        assert!(CompatibilityMode::parse(&accepted).is_ok());
        assert!(ContainerRevision::parse(&accepted).is_ok());
        assert!(CompatibilityMode::parse(&rejected).is_err());
        assert!(ContainerRevision::parse(&rejected).is_err());
    }

    #[test]
    fn all_axes_serialize_as_strings() {
        fn assert_round_trip<T>(value: T, expected: &str)
        where
            T: Serialize + for<'de> Deserialize<'de> + Eq + fmt::Debug,
        {
            let json = serde_json::to_string(&value).unwrap();
            assert_eq!(json, format!(r#""{expected}""#));
            assert_eq!(serde_json::from_str::<T>(&json).unwrap(), value);
        }

        assert_round_trip("2.21".parse::<DottedVersion>().unwrap(), "2.21");
        assert_round_trip("8.5.1.1150".parse::<PlatformBuild>().unwrap(), "8.5.1.1150");
        assert_round_trip("2.20".parse::<XmlDialect>().unwrap(), "2.20");
        assert_round_trip(
            "Version8_3_27".parse::<CompatibilityMode>().unwrap(),
            "Version8_3_27",
        );
        assert_round_trip(StorageVersion::new(7), "7");
        assert_round_trip("Format16".parse::<ContainerRevision>().unwrap(), "Format16");

        assert!(serde_json::from_str::<StorageVersion>("7").is_err());
        assert!(serde_json::from_str::<DottedVersion>("2.21").is_err());
        assert!(serde_json::from_str::<CompatibilityMode>("7").is_err());
        assert!(serde_json::from_str::<ContainerRevision>("16").is_err());

        let too_long = format!("V{}", "1".repeat(64));
        let too_long_json = serde_json::to_string(&too_long).unwrap();
        assert!(serde_json::from_str::<CompatibilityMode>(&too_long_json).is_err());
        assert!(serde_json::from_str::<ContainerRevision>(&too_long_json).is_err());
    }
}
