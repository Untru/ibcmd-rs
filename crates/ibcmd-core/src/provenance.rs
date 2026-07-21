//! Stable, bounded source coordinates for canonical data.

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use serde::de::Visitor;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use crate::artifact::ProfileId;
use crate::diagnostic::{ObjectPath, PropertyPath};

/// Maximum encoded length of an optional source locator.
pub const MAX_PROVENANCE_LOCATOR_BYTES: usize = 4_096;

/// Failure to build bounded source provenance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProvenanceBuildError {
    /// A supplied locator was empty.
    EmptyLocator,
    /// A supplied locator exceeded its encoded bound.
    LocatorTooLong {
        /// Maximum accepted UTF-8 bytes.
        maximum: usize,
        /// Actual UTF-8 bytes.
        actual: usize,
    },
    /// A supplied locator contained a control character.
    ControlCharacter,
}

impl Display for ProvenanceBuildError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyLocator => formatter.write_str("source provenance locator is empty"),
            Self::LocatorTooLong { maximum, actual } => write!(
                formatter,
                "source provenance locator exceeds {maximum} bytes (actual {actual})"
            ),
            Self::ControlCharacter => {
                formatter.write_str("source provenance locator contains a control character")
            }
        }
    }
}

impl Error for ProvenanceBuildError {}

/// An optional stable source-side coordinate such as a manifest or entry key.
///
/// This value is deliberately opaque to the canonical layer. It is retained
/// exactly and is never interpreted as a filesystem path or vendor object.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProvenanceLocator(Box<str>);

impl ProvenanceLocator {
    /// Retains a bounded, control-free source locator.
    pub fn new(value: &str) -> Result<Self, ProvenanceBuildError> {
        validate_locator(value)?;
        Ok(Self(value.into()))
    }

    /// Returns the exact retained locator.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn validate_locator(value: &str) -> Result<(), ProvenanceBuildError> {
    if value.is_empty() {
        return Err(ProvenanceBuildError::EmptyLocator);
    }
    if value.len() > MAX_PROVENANCE_LOCATOR_BYTES {
        return Err(ProvenanceBuildError::LocatorTooLong {
            maximum: MAX_PROVENANCE_LOCATOR_BYTES,
            actual: value.len(),
        });
    }
    if value.chars().any(char::is_control) {
        return Err(ProvenanceBuildError::ControlCharacter);
    }
    Ok(())
}

impl Display for ProvenanceLocator {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Serialize for ProvenanceLocator {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

struct ProvenanceLocatorVisitor;

impl<'de> Visitor<'de> for ProvenanceLocatorVisitor {
    type Value = ProvenanceLocator;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "a non-empty, control-free source locator of at most {MAX_PROVENANCE_LOCATOR_BYTES} bytes"
        )
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        ProvenanceLocator::new(value).map_err(E::custom)
    }
}

impl<'de> Deserialize<'de> for ProvenanceLocator {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(ProvenanceLocatorVisitor)
    }
}

/// Stable object and property coordinates within the canonical model.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanonicalAnchor {
    object_path: ObjectPath,
    property_path: PropertyPath,
}

impl CanonicalAnchor {
    /// Creates an anchor from already bounded diagnostic paths.
    pub const fn new(object_path: ObjectPath, property_path: PropertyPath) -> Self {
        Self {
            object_path,
            property_path,
        }
    }

    /// Returns the stable object coordinate.
    pub fn object_path(&self) -> &ObjectPath {
        &self.object_path
    }

    /// Returns the stable property coordinate.
    pub fn property_path(&self) -> &PropertyPath {
        &self.property_path
    }

    pub(crate) fn retained_byte_len(&self) -> usize {
        self.object_path
            .segments()
            .iter()
            .chain(self.property_path.segments())
            .filter_map(|segment| segment.as_name())
            .map(str::len)
            .sum()
    }
}

/// Exact source profile and stable anchor for one retained canonical value.
///
/// The profile is never inferred from the locator or anchor. The optional
/// locator is extra evidence only and remains bounded opaque text.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceProvenance {
    source_profile: ProfileId,
    anchor: CanonicalAnchor,
    locator: Option<ProvenanceLocator>,
}

impl SourceProvenance {
    /// Creates provenance with an exact profile and anchor.
    pub const fn new(source_profile: ProfileId, anchor: CanonicalAnchor) -> Self {
        Self {
            source_profile,
            anchor,
            locator: None,
        }
    }

    /// Creates provenance with additional bounded source-side evidence.
    pub fn with_locator(
        source_profile: ProfileId,
        anchor: CanonicalAnchor,
        locator: &str,
    ) -> Result<Self, ProvenanceBuildError> {
        Ok(Self {
            source_profile,
            anchor,
            locator: Some(ProvenanceLocator::new(locator)?),
        })
    }

    /// Returns the exact profile declared by the source adapter.
    pub fn source_profile(&self) -> &ProfileId {
        &self.source_profile
    }

    /// Returns the stable object/property anchor.
    pub const fn anchor(&self) -> &CanonicalAnchor {
        &self.anchor
    }

    /// Returns optional source-side evidence.
    pub fn locator(&self) -> Option<&ProvenanceLocator> {
        self.locator.as_ref()
    }

    pub(crate) fn retained_byte_len(&self) -> usize {
        self.source_profile.as_str().len()
            + self.anchor.retained_byte_len()
            + self
                .locator
                .as_ref()
                .map_or(0, |value| value.as_str().len())
    }
}

#[cfg(test)]
mod tests {
    use crate::diagnostic::PathSegment;

    use super::*;

    fn anchor() -> CanonicalAnchor {
        CanonicalAnchor::new(
            ObjectPath::new(vec![PathSegment::name("catalogs").unwrap()]).unwrap(),
            PropertyPath::new(vec![PathSegment::name("future_property").unwrap()]).unwrap(),
        )
    }

    #[test]
    fn exact_profile_anchor_and_locator_round_trip() {
        let value = SourceProvenance::with_locator(
            ProfileId::parse("platform:8.5.1").unwrap(),
            anchor(),
            "fixture:canonical/one",
        )
        .unwrap();
        let encoded = serde_json::to_string(&value).unwrap();
        assert_eq!(
            serde_json::from_str::<SourceProvenance>(&encoded).unwrap(),
            value
        );
        assert_eq!(value.source_profile().as_str(), "platform:8.5.1");
        assert_eq!(
            value.anchor().property_path().to_string(),
            "$/name:future_property"
        );
    }

    #[test]
    fn locator_bounds_are_enforced_by_constructor_and_deserializer() {
        let oversized = "x".repeat(MAX_PROVENANCE_LOCATOR_BYTES + 1);
        assert!(ProvenanceLocator::new(&oversized).is_err());
        assert!(serde_json::from_value::<ProvenanceLocator>(serde_json::json!(oversized)).is_err());
        assert!(ProvenanceLocator::new("bad\nlocator").is_err());
    }
}
