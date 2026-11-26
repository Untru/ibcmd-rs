//! Stable logical identities for canonical metadata objects.

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

use serde::de::Visitor;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use crate::diagnostic::ObjectPath;

/// Failure to parse an exact canonical metadata-object UUID.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParseObjectUuidError {
    /// Canonical UUID text must contain exactly 36 ASCII bytes.
    InvalidLength,
    /// A required hyphen was absent or appeared in a non-canonical position.
    InvalidHyphen,
    /// A character was not a lowercase hexadecimal digit.
    InvalidHexDigit,
}

impl Display for ParseObjectUuidError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLength => {
                formatter.write_str("UUID must contain exactly 36 ASCII characters")
            }
            Self::InvalidHyphen => formatter
                .write_str("UUID must use canonical hyphens at positions 8, 13, 18, and 23"),
            Self::InvalidHexDigit => formatter.write_str(
                "UUID must contain only lowercase hexadecimal digits outside canonical hyphens",
            ),
        }
    }
}

impl Error for ParseObjectUuidError {}

/// A strict lowercase, hyphenated metadata-object UUID.
///
/// The value stores exactly 16 bytes and never generates or normalizes an ID.
/// Text input must already use the canonical `8-4-4-4-12` spelling.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ObjectUuid([u8; 16]);

impl ObjectUuid {
    /// Parses a strict lowercase, hyphenated UUID.
    pub fn parse(value: &str) -> Result<Self, ParseObjectUuidError> {
        let input = value.as_bytes();
        if input.len() != 36 {
            return Err(ParseObjectUuidError::InvalidLength);
        }
        for index in [8, 13, 18, 23] {
            if input[index] != b'-' {
                return Err(ParseObjectUuidError::InvalidHyphen);
            }
        }

        let mut bytes = [0_u8; 16];
        let mut nibble_index = 0_usize;
        for (index, byte) in input.iter().copied().enumerate() {
            if matches!(index, 8 | 13 | 18 | 23) {
                continue;
            }
            let nibble = match byte {
                b'0'..=b'9' => byte - b'0',
                b'a'..=b'f' => byte - b'a' + 10,
                _ => return Err(ParseObjectUuidError::InvalidHexDigit),
            };
            let output_index = nibble_index / 2;
            if nibble_index.is_multiple_of(2) {
                bytes[output_index] = nibble << 4;
            } else {
                bytes[output_index] |= nibble;
            }
            nibble_index += 1;
        }
        Ok(Self(bytes))
    }

    /// Returns the exact 16 identity bytes.
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

impl Display for ObjectUuid {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        for (index, byte) in self.0.iter().enumerate() {
            if matches!(index, 4 | 6 | 8 | 10) {
                formatter.write_str("-")?;
            }
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl FromStr for ObjectUuid {
    type Err = ParseObjectUuidError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl Serialize for ObjectUuid {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

struct ObjectUuidVisitor;

impl<'de> Visitor<'de> for ObjectUuidVisitor {
    type Value = ObjectUuid;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("a canonical lowercase, hyphenated UUID string")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        ObjectUuid::parse(value).map_err(E::custom)
    }
}

impl<'de> Deserialize<'de> for ObjectUuid {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(ObjectUuidVisitor)
    }
}

/// UUID plus a stable, platform-independent logical object path.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogicalIdentity {
    uuid: ObjectUuid,
    path: ObjectPath,
}

impl LogicalIdentity {
    /// Combines an exact UUID with an already bounded stable object path.
    pub const fn new(uuid: ObjectUuid, path: ObjectPath) -> Self {
        Self { uuid, path }
    }

    /// Returns the exact object UUID.
    pub const fn uuid(&self) -> ObjectUuid {
        self.uuid
    }

    /// Returns the stable logical object path.
    pub const fn path(&self) -> &ObjectPath {
        &self.path
    }

    pub(crate) fn retained_byte_len(&self) -> usize {
        16 + self
            .path
            .segments()
            .iter()
            .filter_map(|segment| segment.as_name())
            .map(str::len)
            .sum::<usize>()
    }
}

#[cfg(test)]
mod tests {
    use crate::diagnostic::PathSegment;

    use super::*;

    const UUID: &str = "12345678-90ab-cdef-0123-456789abcdef";

    #[test]
    fn strict_uuid_parsing_and_string_serde_are_canonical() {
        let uuid = ObjectUuid::parse(UUID).unwrap();
        assert_eq!(uuid.to_string(), UUID);
        assert_eq!(serde_json::to_string(&uuid).unwrap(), format!("\"{UUID}\""));
        assert_eq!(
            serde_json::from_str::<ObjectUuid>(&format!("\"{UUID}\"")).unwrap(),
            uuid
        );

        for invalid in [
            "1234567890ab-cdef-0123-456789abcdef",
            "12345678_90ab-cdef-0123-456789abcdef",
            "12345678-90AB-cdef-0123-456789abcdef",
            "12345678-90ab-cdef-0123-456789abcdeg",
            "{12345678-90ab-cdef-0123-456789abcdef}",
        ] {
            assert!(ObjectUuid::parse(invalid).is_err(), "{invalid}");
        }
        assert!(serde_json::from_str::<ObjectUuid>("[1,2,3]").is_err());
    }

    #[test]
    fn logical_identity_retains_typed_path_without_filesystem_semantics() {
        let path = ObjectPath::new(vec![
            PathSegment::name("catalogs").unwrap(),
            PathSegment::name("customers").unwrap(),
        ])
        .unwrap();
        let identity = LogicalIdentity::new(ObjectUuid::parse(UUID).unwrap(), path.clone());
        let json = serde_json::to_string(&identity).unwrap();
        assert_eq!(
            serde_json::from_str::<LogicalIdentity>(&json).unwrap(),
            identity
        );
        assert_eq!(identity.path(), &path);
    }
}
