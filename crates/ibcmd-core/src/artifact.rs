//! Version-independent artifact and profile identities.

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::marker::PhantomData;
use std::str::FromStr;

use serde::de::Visitor;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

/// Maximum encoded length of an artifact or profile identifier.
pub const MAX_IDENTIFIER_BYTES: usize = 128;

/// Error returned when an artifact or profile identifier is invalid.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParseIdentifierError {
    message: &'static str,
}

impl ParseIdentifierError {
    const fn new(message: &'static str) -> Self {
        Self { message }
    }
}

impl Display for ParseIdentifierError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message)
    }
}

impl Error for ParseIdentifierError {}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct BoundedId(Box<str>);

impl BoundedId {
    fn new(input: &str) -> Result<Self, ParseIdentifierError> {
        validate_identifier(input)?;
        Ok(Self(input.into()))
    }

    fn known(input: &'static str) -> Self {
        debug_assert!(validate_identifier(input).is_ok());
        Self(input.into())
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

fn validate_identifier(input: &str) -> Result<(), ParseIdentifierError> {
    if input.is_empty() {
        return Err(ParseIdentifierError::new("identifier is empty"));
    }
    if input.len() > MAX_IDENTIFIER_BYTES {
        return Err(ParseIdentifierError::new("identifier exceeds 128 bytes"));
    }

    let bytes = input.as_bytes();
    if !bytes[0].is_ascii_alphanumeric() {
        return Err(ParseIdentifierError::new(
            "identifier must start with an ASCII letter or digit",
        ));
    }
    if !bytes[bytes.len() - 1].is_ascii_alphanumeric() {
        return Err(ParseIdentifierError::new(
            "identifier must end with an ASCII letter or digit",
        ));
    }
    if !bytes
        .iter()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b':'))
    {
        return Err(ParseIdentifierError::new(
            "identifier contains an invalid character",
        ));
    }
    Ok(())
}

macro_rules! open_id_type {
    (
        $(#[$type_meta:meta])*
        pub struct $name:ident;
        $(
            known {
                $(
                    $(#[$known_meta:meta])*
                    $constructor:ident, $predicate:ident => $known:literal;
                )*
            }
        )?
    ) => {
        $(#[$type_meta])*
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(BoundedId);

        impl $name {
            /// Builds an identifier after validating the borrowed input.
            pub fn new(input: &str) -> Result<Self, ParseIdentifierError> {
                BoundedId::new(input).map(Self)
            }

            /// Parses an identifier without inferring any other coordinate.
            pub fn parse(input: &str) -> Result<Self, ParseIdentifierError> {
                Self::new(input)
            }

            /// Returns the exact validated identifier.
            pub fn as_str(&self) -> &str {
                self.0.as_str()
            }

            $(
                $(
                    $(#[$known_meta])*
                    pub fn $constructor() -> Self {
                        Self(BoundedId::known($known))
                    }

                    #[doc = concat!("Returns whether this identifier is canonical `", $known, "`.")]
                    pub fn $predicate(&self) -> bool {
                        self.as_str() == $known
                    }
                )*
            )?
        }

        impl Display for $name {
            fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl FromStr for $name {
            type Err = ParseIdentifierError;

            fn from_str(input: &str) -> Result<Self, Self::Err> {
                Self::new(input)
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                deserializer.deserialize_str(IdentifierVisitor::<Self>(PhantomData))
            }
        }
    };
}

struct IdentifierVisitor<T>(PhantomData<fn() -> T>);

impl<'de, T> Visitor<'de> for IdentifierVisitor<T>
where
    T: FromStr,
    T::Err: Display,
{
    type Value = T;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("a bounded ASCII identifier string")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        value.parse().map_err(E::custom)
    }
}

open_id_type! {
    /// A representation family such as CF, XML, or an infobase.
    ///
    /// This coordinate intentionally contains no platform, dialect, storage,
    /// or container version. Unknown identifiers remain opaque names; no
    /// version is parsed from or inferred from them.
    pub struct ArtifactFormat;
    known {
        /// Returns the canonical `cf` format.
        cf, is_cf => "cf";
        /// Returns the canonical `xml` format.
        xml, is_xml => "xml";
        /// Returns the canonical `infobase` format.
        infobase, is_infobase => "infobase";
    }
}

open_id_type! {
    /// The logical kind of an artifact, independent of its format.
    pub struct ArtifactKind;
    known {
        /// Returns the canonical `configuration` kind.
        configuration, is_configuration => "configuration";
        /// Returns the canonical `extension` kind.
        extension, is_extension => "extension";
        /// Returns the canonical `epf` kind.
        epf, is_epf => "epf";
        /// Returns the canonical `erf` kind.
        erf, is_erf => "erf";
    }
}

open_id_type! {
    /// A DBMS family used by an infobase storage adapter.
    pub struct DbmsKind;
    known {
        /// Returns the canonical `mssql` DBMS kind.
        mssql, is_mssql => "mssql";
        /// Returns the canonical `postgresql` DBMS kind.
        postgresql, is_postgresql => "postgresql";
        /// Returns the canonical `file` DBMS kind.
        file, is_file => "file";
    }
}

open_id_type! {
    /// An opaque identifier for a coordinated conversion profile.
    pub struct ProfileId;
}

open_id_type! {
    /// An opaque identifier for a complete logical storage profile.
    ///
    /// This is deliberately distinct from both [`ProfileId`] and a numeric
    /// storage-format version.
    pub struct StorageProfileId;
}

#[cfg(test)]
mod tests {
    use std::any::TypeId;
    use std::collections::HashSet;

    use super::*;

    fn assert_json_round_trip<T>(value: T, expected: &str)
    where
        T: Serialize + for<'de> Deserialize<'de> + Eq + fmt::Debug,
    {
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(json, format!(r#""{expected}""#));
        assert_eq!(serde_json::from_str::<T>(&json).unwrap(), value);
    }

    #[test]
    fn known_formats_kinds_and_dbms_values_are_canonical() {
        let formats = [
            (ArtifactFormat::cf(), "cf"),
            (ArtifactFormat::xml(), "xml"),
            (ArtifactFormat::infobase(), "infobase"),
        ];
        for (format, expected) in formats {
            assert_eq!(format.as_str(), expected);
            assert_eq!(ArtifactFormat::parse(expected).unwrap(), format);
        }
        assert!(ArtifactFormat::cf().is_cf());
        assert!(ArtifactFormat::xml().is_xml());
        assert!(ArtifactFormat::infobase().is_infobase());

        let kinds = [
            (ArtifactKind::configuration(), "configuration"),
            (ArtifactKind::extension(), "extension"),
            (ArtifactKind::epf(), "epf"),
            (ArtifactKind::erf(), "erf"),
        ];
        for (kind, expected) in kinds {
            assert_eq!(kind.as_str(), expected);
            assert_eq!(ArtifactKind::parse(expected).unwrap(), kind);
        }
        assert!(ArtifactKind::configuration().is_configuration());
        assert!(ArtifactKind::extension().is_extension());
        assert!(ArtifactKind::epf().is_epf());
        assert!(ArtifactKind::erf().is_erf());

        let dbms_kinds = [
            (DbmsKind::mssql(), "mssql"),
            (DbmsKind::postgresql(), "postgresql"),
            (DbmsKind::file(), "file"),
        ];
        for (dbms, expected) in dbms_kinds {
            assert_eq!(dbms.as_str(), expected);
            assert_eq!(DbmsKind::parse(expected).unwrap(), dbms);
        }
        assert!(DbmsKind::mssql().is_mssql());
        assert!(DbmsKind::postgresql().is_postgresql());
        assert!(DbmsKind::file().is_file());
    }

    #[test]
    fn every_known_format_and_kind_serializes_independently() {
        for format in ["cf", "xml", "infobase"] {
            for kind in ["configuration", "extension", "epf", "erf"] {
                assert_json_round_trip(ArtifactFormat::parse(format).unwrap(), format);
                assert_json_round_trip(ArtifactKind::parse(kind).unwrap(), kind);
            }
        }
    }

    #[test]
    fn unknown_values_round_trip_exactly() {
        assert_json_round_trip(
            ArtifactFormat::parse("future-format").unwrap(),
            "future-format",
        );
        assert_json_round_trip(ArtifactKind::parse("future-kind").unwrap(), "future-kind");
        assert_json_round_trip(DbmsKind::parse("future-dbms").unwrap(), "future-dbms");
        assert_json_round_trip(
            ProfileId::parse("profile:future-9.1").unwrap(),
            "profile:future-9.1",
        );
        assert_json_round_trip(
            StorageProfileId::parse("storage:future_42").unwrap(),
            "storage:future_42",
        );
    }

    #[test]
    fn identifiers_enforce_grammar_and_length_before_copying() {
        let accepted = format!("a{}", "1".repeat(MAX_IDENTIFIER_BYTES - 1));
        let rejected = format!("a{}", "1".repeat(MAX_IDENTIFIER_BYTES));
        assert_eq!(accepted.len(), MAX_IDENTIFIER_BYTES);
        assert_eq!(rejected.len(), MAX_IDENTIFIER_BYTES + 1);

        assert!(ArtifactFormat::parse(&accepted).is_ok());
        assert!(ArtifactKind::parse(&accepted).is_ok());
        assert!(DbmsKind::parse(&accepted).is_ok());
        assert!(ProfileId::parse(&accepted).is_ok());
        assert!(StorageProfileId::parse(&accepted).is_ok());

        for invalid in [
            "",
            " leading",
            "trailing ",
            "two words",
            "path/value",
            "path\\value",
            ".leading",
            "trailing-",
            "кириллица",
        ] {
            assert!(ArtifactFormat::parse(invalid).is_err(), "{invalid}");
            assert!(ProfileId::parse(invalid).is_err(), "{invalid}");
        }
        assert!(ArtifactFormat::parse(&rejected).is_err());
        assert!(ArtifactKind::parse(&rejected).is_err());
        assert!(DbmsKind::parse(&rejected).is_err());
        assert!(ProfileId::parse(&rejected).is_err());
        assert!(StorageProfileId::parse(&rejected).is_err());
    }

    #[test]
    fn serde_accepts_strings_only() {
        for json in ["1", "{}", "[]", "null", "true"] {
            assert!(serde_json::from_str::<ArtifactFormat>(json).is_err());
            assert!(serde_json::from_str::<ArtifactKind>(json).is_err());
            assert!(serde_json::from_str::<DbmsKind>(json).is_err());
            assert!(serde_json::from_str::<ProfileId>(json).is_err());
            assert!(serde_json::from_str::<StorageProfileId>(json).is_err());
        }

        let too_long = format!("a{}", "1".repeat(MAX_IDENTIFIER_BYTES));
        let json = serde_json::to_string(&too_long).unwrap();
        assert!(serde_json::from_str::<ArtifactFormat>(&json).is_err());
        assert!(serde_json::from_str::<ProfileId>(&json).is_err());
    }

    #[test]
    fn public_identity_types_remain_distinct() {
        let ids = HashSet::from([
            TypeId::of::<ArtifactFormat>(),
            TypeId::of::<ArtifactKind>(),
            TypeId::of::<DbmsKind>(),
            TypeId::of::<ProfileId>(),
            TypeId::of::<StorageProfileId>(),
        ]);
        assert_eq!(ids.len(), 5);

        let profile = ProfileId::parse("same-id").unwrap();
        let storage = StorageProfileId::parse("same-id").unwrap();
        assert_eq!(profile.as_str(), storage.as_str());
    }
}
