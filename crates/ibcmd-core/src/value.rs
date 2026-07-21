//! Ordered, lossless canonical values.
//!
//! Values use an externally tagged one-entry map on the wire, for example
//! `{"integer":"42"}` or `{"sequence":[...]}`. That representation lets
//! deserialization select a bounded streaming visitor before retaining a
//! record, sequence, nested value, or byte payload.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::marker::PhantomData;
use std::str::FromStr;

use serde::de::{DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use crate::asset::{Asset, AssetReference};

/// Maximum encoded length of arbitrary canonical text.
pub const MAX_CANONICAL_TEXT_BYTES: usize = 1_048_576;
/// Maximum encoded length of a field, enum, or reference token.
pub const MAX_CANONICAL_TOKEN_BYTES: usize = 1_024;
/// Maximum encoded length of one canonical integer or decimal.
pub const MAX_CANONICAL_NUMBER_BYTES: usize = 4_096;
/// Maximum number of direct items in one record or sequence.
pub const MAX_CANONICAL_COLLECTION_ITEMS: usize = 16_384;
/// Maximum nested record/sequence depth, with the root at depth zero.
pub const MAX_CANONICAL_DEPTH: usize = 64;
/// Maximum total value nodes retained by one canonical value.
pub const MAX_CANONICAL_NODES: usize = 131_072;
/// Maximum aggregate variable-sized bytes retained by one canonical value.
pub const MAX_CANONICAL_RETAINED_BYTES: usize = 67_108_864;

/// Failure to construct or revalidate a canonical value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ValueBuildError {
    /// A required string was empty.
    EmptyText {
        /// Logical field name.
        field: &'static str,
    },
    /// A string exceeded its UTF-8 byte bound.
    TextTooLong {
        /// Logical field name.
        field: &'static str,
        /// Maximum accepted bytes.
        maximum: usize,
        /// Actual bytes.
        actual: usize,
    },
    /// A token contained a control character.
    ControlCharacter {
        /// Logical field name.
        field: &'static str,
    },
    /// An integer was not in canonical base-10 notation.
    InvalidInteger,
    /// A decimal was not in canonical fixed-point notation.
    InvalidDecimal,
    /// A record or sequence exceeded its direct item bound.
    TooManyCollectionItems {
        /// Maximum accepted items.
        maximum: usize,
        /// Actual items, when known.
        actual: usize,
    },
    /// A record supplied the same exact field name more than once.
    DuplicateField {
        /// Duplicate field name.
        name: String,
    },
    /// Nested values exceeded the public depth bound.
    DepthExceeded {
        /// Maximum accepted depth.
        maximum: usize,
        /// Actual depth.
        actual: usize,
    },
    /// A value exceeded its aggregate node bound.
    TooManyNodes {
        /// Maximum accepted nodes.
        maximum: usize,
        /// Actual nodes.
        actual: usize,
    },
    /// A value exceeded its aggregate retained-byte budget.
    RetainedBytesExceeded {
        /// Maximum accepted retained bytes.
        maximum: usize,
        /// Actual retained bytes.
        actual: usize,
    },
    /// Aggregate retained-byte arithmetic overflowed.
    RetainedByteCountOverflow,
}

impl Display for ValueBuildError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyText { field } => write!(formatter, "{field} is empty"),
            Self::TextTooLong {
                field,
                maximum,
                actual,
            } => write!(
                formatter,
                "{field} exceeds {maximum} bytes (actual {actual})"
            ),
            Self::ControlCharacter { field } => {
                write!(formatter, "{field} contains a control character")
            }
            Self::InvalidInteger => formatter.write_str(
                "integer must use canonical base-10 notation without a plus sign or leading zeros",
            ),
            Self::InvalidDecimal => formatter.write_str(
                "decimal must use canonical fixed-point notation without exponent, redundant zeros, or negative zero",
            ),
            Self::TooManyCollectionItems { maximum, actual } => write!(
                formatter,
                "canonical collection exceeds {maximum} items (actual {actual})"
            ),
            Self::DuplicateField { name } => {
                write!(formatter, "canonical record contains duplicate field `{name}`")
            }
            Self::DepthExceeded { maximum, actual } => write!(
                formatter,
                "canonical value exceeds nesting depth {maximum} (actual {actual})"
            ),
            Self::TooManyNodes { maximum, actual } => write!(
                formatter,
                "canonical value exceeds {maximum} nodes (actual {actual})"
            ),
            Self::RetainedBytesExceeded { maximum, actual } => write!(
                formatter,
                "canonical value exceeds aggregate retained-byte budget {maximum} (actual {actual})"
            ),
            Self::RetainedByteCountOverflow => {
                formatter.write_str("canonical retained-byte count overflowed")
            }
        }
    }
}

impl Error for ValueBuildError {}

fn validate_bounded_text(
    field: &'static str,
    value: &str,
    maximum: usize,
    allow_empty: bool,
    control_free: bool,
) -> Result<(), ValueBuildError> {
    if value.is_empty() && !allow_empty {
        return Err(ValueBuildError::EmptyText { field });
    }
    if value.len() > maximum {
        return Err(ValueBuildError::TextTooLong {
            field,
            maximum,
            actual: value.len(),
        });
    }
    if control_free && value.chars().any(char::is_control) {
        return Err(ValueBuildError::ControlCharacter { field });
    }
    Ok(())
}

struct ParseStringVisitor<T>(PhantomData<fn() -> T>);

impl<'de, T> Visitor<'de> for ParseStringVisitor<T>
where
    T: FromStr,
    T::Err: Display,
{
    type Value = T;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("a bounded canonical string")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        value.parse().map_err(E::custom)
    }
}

macro_rules! bounded_string_type {
    (
        $(#[$metadata:meta])*
        $name:ident, $field:literal, $maximum:expr, $allow_empty:expr, $control_free:expr
    ) => {
        $(#[$metadata])*
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(Box<str>);

        impl $name {
            /// Validates borrowed text before retaining it.
            pub fn new(value: &str) -> Result<Self, ValueBuildError> {
                validate_bounded_text(
                    $field,
                    value,
                    $maximum,
                    $allow_empty,
                    $control_free,
                )?;
                Ok(Self(value.into()))
            }

            /// Returns the exact retained text.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl Display for $name {
            fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl FromStr for $name {
            type Err = ValueBuildError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::new(value)
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
                deserializer.deserialize_str(ParseStringVisitor::<Self>(PhantomData))
            }
        }
    };
}

bounded_string_type! {
    /// Arbitrary exact UTF-8 text. Empty text and control characters are retained.
    CanonicalText, "canonical text", MAX_CANONICAL_TEXT_BYTES, true, false
}

bounded_string_type! {
    /// An open enum token. Unknown future tokens remain first-class values.
    EnumToken, "enum token", MAX_CANONICAL_TOKEN_BYTES, false, true
}

bounded_string_type! {
    /// An exact, case-sensitive canonical record field name.
    FieldName, "field name", MAX_CANONICAL_TOKEN_BYTES, false, true
}

bounded_string_type! {
    /// Opaque target coordinate of an unresolved reference.
    ReferenceTarget, "reference target", MAX_CANONICAL_TOKEN_BYTES, false, true
}

/// An arbitrary-precision integer in canonical base-10 text form.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CanonicalInteger(Box<str>);

impl CanonicalInteger {
    /// Validates canonical integer notation before retaining it.
    pub fn new(value: &str) -> Result<Self, ValueBuildError> {
        validate_bounded_text(
            "canonical integer",
            value,
            MAX_CANONICAL_NUMBER_BYTES,
            false,
            true,
        )?;
        if !is_canonical_integer(value) {
            return Err(ValueBuildError::InvalidInteger);
        }
        Ok(Self(value.into()))
    }

    /// Returns exact canonical notation.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn is_canonical_integer(value: &str) -> bool {
    let digits = value.strip_prefix('-').unwrap_or(value);
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return false;
    }
    if digits.len() > 1 && digits.starts_with('0') {
        return false;
    }
    value != "-0"
}

impl Display for CanonicalInteger {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for CanonicalInteger {
    type Err = ValueBuildError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl Serialize for CanonicalInteger {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for CanonicalInteger {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(ParseStringVisitor::<Self>(PhantomData))
    }
}

/// An arbitrary-precision decimal in canonical fixed-point text form.
///
/// Exponents, plus signs, leading integer zeros, trailing fractional zeros,
/// and every spelling of negative zero are rejected. Scale-zero decimals use
/// integer notation, so `1` is valid while `1.0` is not.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CanonicalDecimal(Box<str>);

impl CanonicalDecimal {
    /// Validates canonical decimal notation before retaining it.
    pub fn new(value: &str) -> Result<Self, ValueBuildError> {
        validate_bounded_text(
            "canonical decimal",
            value,
            MAX_CANONICAL_NUMBER_BYTES,
            false,
            true,
        )?;
        if !is_canonical_decimal(value) {
            return Err(ValueBuildError::InvalidDecimal);
        }
        Ok(Self(value.into()))
    }

    /// Returns exact canonical notation.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn is_canonical_decimal(value: &str) -> bool {
    let unsigned = value.strip_prefix('-').unwrap_or(value);
    let Some((integer, fraction)) = unsigned.split_once('.') else {
        return is_canonical_integer(value);
    };
    if integer.is_empty()
        || fraction.is_empty()
        || !integer.bytes().all(|byte| byte.is_ascii_digit())
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
        || (integer.len() > 1 && integer.starts_with('0'))
        || fraction.ends_with('0')
    {
        return false;
    }
    true
}

impl Display for CanonicalDecimal {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for CanonicalDecimal {
    type Err = ValueBuildError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl Serialize for CanonicalDecimal {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for CanonicalDecimal {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(ParseStringVisitor::<Self>(PhantomData))
    }
}

/// A reference retained without requiring the target graph to be available.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnresolvedReference {
    kind: EnumToken,
    target: ReferenceTarget,
}

impl UnresolvedReference {
    /// Creates a generic unresolved reference from open, bounded strings.
    pub fn new(kind: &str, target: &str) -> Result<Self, ValueBuildError> {
        Ok(Self {
            kind: EnumToken::new(kind)?,
            target: ReferenceTarget::new(target)?,
        })
    }

    /// Creates a reference from already validated parts.
    pub const fn from_parts(kind: EnumToken, target: ReferenceTarget) -> Self {
        Self { kind, target }
    }

    /// Returns the exact open reference-kind token.
    pub const fn kind_token(&self) -> &EnumToken {
        &self.kind
    }

    /// Returns the exact reference-kind string.
    pub fn kind(&self) -> &str {
        self.kind.as_str()
    }

    /// Returns the exact opaque target coordinate.
    pub const fn target_value(&self) -> &ReferenceTarget {
        &self.target
    }

    /// Returns the exact opaque target string.
    pub fn target(&self) -> &str {
        self.target.as_str()
    }

    fn retained_byte_len(&self) -> usize {
        self.kind.as_str().len() + self.target.as_str().len()
    }
}

/// One exact field in an ordered canonical record.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CanonicalField {
    name: FieldName,
    value: CanonicalValue,
}

impl CanonicalField {
    /// Creates a field from validated parts.
    pub const fn new(name: FieldName, value: CanonicalValue) -> Self {
        Self { name, value }
    }

    /// Validates a borrowed field name and retains the supplied value.
    pub fn named(name: &str, value: CanonicalValue) -> Result<Self, ValueBuildError> {
        Ok(Self::new(FieldName::new(name)?, value))
    }

    /// Returns the exact case-sensitive field name.
    pub const fn name(&self) -> &FieldName {
        &self.name
    }

    /// Returns the field value.
    pub const fn value(&self) -> &CanonicalValue {
        &self.value
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CanonicalValueInner {
    Null,
    Bool(bool),
    Integer(CanonicalInteger),
    Decimal(CanonicalDecimal),
    Text(CanonicalText),
    EnumToken(EnumToken),
    Reference(UnresolvedReference),
    Record(Vec<CanonicalField>),
    Sequence(Vec<CanonicalValue>),
    Binary(Asset),
    AssetReference(AssetReference),
}

/// One immutable, typed, platform-independent canonical value.
///
/// Inner variants and collection vectors are private so every construction
/// path applies the same depth, count, duplicate, and aggregate-byte policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalValue {
    inner: CanonicalValueInner,
}

/// Borrowed view of a [`CanonicalValue`] variant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalValueKind<'a> {
    /// Explicit null.
    Null,
    /// Boolean scalar.
    Bool(bool),
    /// Arbitrary-precision integer.
    Integer(&'a CanonicalInteger),
    /// Canonical fixed-point decimal.
    Decimal(&'a CanonicalDecimal),
    /// Exact UTF-8 text.
    Text(&'a CanonicalText),
    /// Open enum token.
    EnumToken(&'a EnumToken),
    /// Generic unresolved reference.
    Reference(&'a UnresolvedReference),
    /// Exact source-ordered fields.
    Record(&'a [CanonicalField]),
    /// Exact source-ordered values.
    Sequence(&'a [CanonicalValue]),
    /// Inline immutable content-addressed bytes.
    Binary(&'a Asset),
    /// Metadata-only content-addressed asset reference.
    AssetReference(&'a AssetReference),
}

impl CanonicalValue {
    /// Creates explicit null.
    pub const fn null() -> Self {
        Self {
            inner: CanonicalValueInner::Null,
        }
    }

    /// Creates a boolean scalar.
    pub const fn boolean(value: bool) -> Self {
        Self {
            inner: CanonicalValueInner::Bool(value),
        }
    }

    /// Retains an already canonical integer.
    pub const fn integer(value: CanonicalInteger) -> Self {
        Self {
            inner: CanonicalValueInner::Integer(value),
        }
    }

    /// Retains an already canonical decimal.
    pub const fn decimal(value: CanonicalDecimal) -> Self {
        Self {
            inner: CanonicalValueInner::Decimal(value),
        }
    }

    /// Retains already bounded exact text.
    pub const fn text(value: CanonicalText) -> Self {
        Self {
            inner: CanonicalValueInner::Text(value),
        }
    }

    /// Retains an open enum token without interpreting unknown values.
    pub const fn enum_token(value: EnumToken) -> Self {
        Self {
            inner: CanonicalValueInner::EnumToken(value),
        }
    }

    /// Retains a generic unresolved reference.
    pub const fn reference(value: UnresolvedReference) -> Self {
        Self {
            inner: CanonicalValueInner::Reference(value),
        }
    }

    /// Creates an ordered record. Exact duplicate names are rejected.
    ///
    /// Names are case-sensitive and the input is never sorted. Consequently,
    /// records with identical fields in different orders remain unequal.
    pub fn record(fields: Vec<CanonicalField>) -> Result<Self, ValueBuildError> {
        let value = Self {
            inner: CanonicalValueInner::Record(fields),
        };
        validate_root(&value)?;
        Ok(value)
    }

    /// Creates an ordered sequence without sorting or deduplicating it.
    pub fn sequence(values: Vec<Self>) -> Result<Self, ValueBuildError> {
        let value = Self {
            inner: CanonicalValueInner::Sequence(values),
        };
        validate_root(&value)?;
        Ok(value)
    }

    /// Retains bounded inline content-addressed bytes.
    pub fn binary(asset: Asset) -> Result<Self, ValueBuildError> {
        let value = Self {
            inner: CanonicalValueInner::Binary(asset),
        };
        validate_root(&value)?;
        Ok(value)
    }

    /// Retains a metadata-only content-addressed reference.
    pub const fn asset_reference(reference: AssetReference) -> Self {
        Self {
            inner: CanonicalValueInner::AssetReference(reference),
        }
    }

    /// Returns a borrowed typed view.
    pub fn kind(&self) -> CanonicalValueKind<'_> {
        match &self.inner {
            CanonicalValueInner::Null => CanonicalValueKind::Null,
            CanonicalValueInner::Bool(value) => CanonicalValueKind::Bool(*value),
            CanonicalValueInner::Integer(value) => CanonicalValueKind::Integer(value),
            CanonicalValueInner::Decimal(value) => CanonicalValueKind::Decimal(value),
            CanonicalValueInner::Text(value) => CanonicalValueKind::Text(value),
            CanonicalValueInner::EnumToken(value) => CanonicalValueKind::EnumToken(value),
            CanonicalValueInner::Reference(value) => CanonicalValueKind::Reference(value),
            CanonicalValueInner::Record(fields) => CanonicalValueKind::Record(fields),
            CanonicalValueInner::Sequence(values) => CanonicalValueKind::Sequence(values),
            CanonicalValueInner::Binary(asset) => CanonicalValueKind::Binary(asset),
            CanonicalValueInner::AssetReference(reference) => {
                CanonicalValueKind::AssetReference(reference)
            }
        }
    }

    /// Returns ordered record fields, when this is a record.
    pub fn as_record(&self) -> Option<&[CanonicalField]> {
        match &self.inner {
            CanonicalValueInner::Record(fields) => Some(fields),
            _ => None,
        }
    }

    /// Returns ordered sequence values, when this is a sequence.
    pub fn as_sequence(&self) -> Option<&[Self]> {
        match &self.inner {
            CanonicalValueInner::Sequence(values) => Some(values),
            _ => None,
        }
    }

    /// Returns aggregate variable-sized retained bytes.
    pub fn retained_byte_len(&self) -> usize {
        let mut budget = ValueBudget::default();
        validate_value(self, 0, &mut budget)
            .expect("private canonical value invariants remain valid");
        budget.retained_bytes
    }
}

impl Serialize for CanonicalValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(1))?;
        match &self.inner {
            CanonicalValueInner::Null => map.serialize_entry("null", &())?,
            CanonicalValueInner::Bool(value) => map.serialize_entry("bool", value)?,
            CanonicalValueInner::Integer(value) => map.serialize_entry("integer", value)?,
            CanonicalValueInner::Decimal(value) => map.serialize_entry("decimal", value)?,
            CanonicalValueInner::Text(value) => map.serialize_entry("text", value)?,
            CanonicalValueInner::EnumToken(value) => map.serialize_entry("enum_token", value)?,
            CanonicalValueInner::Reference(value) => map.serialize_entry("reference", value)?,
            CanonicalValueInner::Record(value) => map.serialize_entry("record", value)?,
            CanonicalValueInner::Sequence(value) => map.serialize_entry("sequence", value)?,
            CanonicalValueInner::Binary(value) => map.serialize_entry("binary", value)?,
            CanonicalValueInner::AssetReference(value) => {
                map.serialize_entry("asset_reference", value)?
            }
        }
        map.end()
    }
}

#[derive(Default)]
struct ValueBudget {
    nodes: usize,
    retained_bytes: usize,
}

impl ValueBudget {
    fn add_node(&mut self) -> Result<(), ValueBuildError> {
        self.nodes = self
            .nodes
            .checked_add(1)
            .ok_or(ValueBuildError::TooManyNodes {
                maximum: MAX_CANONICAL_NODES,
                actual: usize::MAX,
            })?;
        if self.nodes > MAX_CANONICAL_NODES {
            return Err(ValueBuildError::TooManyNodes {
                maximum: MAX_CANONICAL_NODES,
                actual: self.nodes,
            });
        }
        Ok(())
    }

    fn retain(&mut self, bytes: usize) -> Result<(), ValueBuildError> {
        self.retained_bytes = self
            .retained_bytes
            .checked_add(bytes)
            .ok_or(ValueBuildError::RetainedByteCountOverflow)?;
        if self.retained_bytes > MAX_CANONICAL_RETAINED_BYTES {
            return Err(ValueBuildError::RetainedBytesExceeded {
                maximum: MAX_CANONICAL_RETAINED_BYTES,
                actual: self.retained_bytes,
            });
        }
        Ok(())
    }
}

fn validate_root(value: &CanonicalValue) -> Result<(), ValueBuildError> {
    validate_value(value, 0, &mut ValueBudget::default())
}

fn validate_value(
    value: &CanonicalValue,
    depth: usize,
    budget: &mut ValueBudget,
) -> Result<(), ValueBuildError> {
    if depth > MAX_CANONICAL_DEPTH {
        return Err(ValueBuildError::DepthExceeded {
            maximum: MAX_CANONICAL_DEPTH,
            actual: depth,
        });
    }
    budget.add_node()?;
    match &value.inner {
        CanonicalValueInner::Null | CanonicalValueInner::Bool(_) => {}
        CanonicalValueInner::Integer(value) => budget.retain(value.as_str().len())?,
        CanonicalValueInner::Decimal(value) => budget.retain(value.as_str().len())?,
        CanonicalValueInner::Text(value) => budget.retain(value.as_str().len())?,
        CanonicalValueInner::EnumToken(value) => budget.retain(value.as_str().len())?,
        CanonicalValueInner::Reference(value) => budget.retain(value.retained_byte_len())?,
        CanonicalValueInner::Record(fields) => {
            validate_collection_len(fields.len())?;
            let mut names = BTreeSet::new();
            for field in fields {
                if !names.insert(field.name.clone()) {
                    return Err(ValueBuildError::DuplicateField {
                        name: field.name.as_str().to_owned(),
                    });
                }
                budget.retain(field.name.as_str().len())?;
                validate_value(&field.value, depth + 1, budget)?;
            }
        }
        CanonicalValueInner::Sequence(values) => {
            validate_collection_len(values.len())?;
            for child in values {
                validate_value(child, depth + 1, budget)?;
            }
        }
        CanonicalValueInner::Binary(asset) => budget.retain(asset.retained_byte_len())?,
        CanonicalValueInner::AssetReference(reference) => {
            budget.retain(reference.retained_byte_len())?
        }
    }
    Ok(())
}

fn validate_collection_len(actual: usize) -> Result<(), ValueBuildError> {
    if actual > MAX_CANONICAL_COLLECTION_ITEMS {
        return Err(ValueBuildError::TooManyCollectionItems {
            maximum: MAX_CANONICAL_COLLECTION_ITEMS,
            actual,
        });
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "snake_case")]
enum ValueVariant {
    Null,
    Bool,
    Integer,
    Decimal,
    Text,
    EnumToken,
    Reference,
    Record,
    Sequence,
    Binary,
    AssetReference,
}

struct CanonicalValueSeed<'a> {
    depth: usize,
    budget: &'a mut ValueBudget,
}

impl<'de> DeserializeSeed<'de> for CanonicalValueSeed<'_> {
    type Value = CanonicalValue;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        if self.depth > MAX_CANONICAL_DEPTH {
            return Err(de::Error::custom(ValueBuildError::DepthExceeded {
                maximum: MAX_CANONICAL_DEPTH,
                actual: self.depth,
            }));
        }
        self.budget.add_node().map_err(de::Error::custom)?;
        deserializer.deserialize_map(CanonicalValueVisitor {
            depth: self.depth,
            budget: self.budget,
        })
    }
}

struct CanonicalValueVisitor<'a> {
    depth: usize,
    budget: &'a mut ValueBudget,
}

impl<'de> Visitor<'de> for CanonicalValueVisitor<'_> {
    type Value = CanonicalValue;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("an externally tagged one-entry canonical value map")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let variant = map
            .next_key::<ValueVariant>()?
            .ok_or_else(|| de::Error::custom("canonical value map is empty"))?;
        let inner = match variant {
            ValueVariant::Null => {
                map.next_value::<()>()?;
                CanonicalValueInner::Null
            }
            ValueVariant::Bool => CanonicalValueInner::Bool(map.next_value::<bool>()?),
            ValueVariant::Integer => {
                let value = map.next_value::<CanonicalInteger>()?;
                self.budget
                    .retain(value.as_str().len())
                    .map_err(de::Error::custom)?;
                CanonicalValueInner::Integer(value)
            }
            ValueVariant::Decimal => {
                let value = map.next_value::<CanonicalDecimal>()?;
                self.budget
                    .retain(value.as_str().len())
                    .map_err(de::Error::custom)?;
                CanonicalValueInner::Decimal(value)
            }
            ValueVariant::Text => {
                let value = map.next_value::<CanonicalText>()?;
                self.budget
                    .retain(value.as_str().len())
                    .map_err(de::Error::custom)?;
                CanonicalValueInner::Text(value)
            }
            ValueVariant::EnumToken => {
                let value = map.next_value::<EnumToken>()?;
                self.budget
                    .retain(value.as_str().len())
                    .map_err(de::Error::custom)?;
                CanonicalValueInner::EnumToken(value)
            }
            ValueVariant::Reference => {
                let value = map.next_value::<UnresolvedReference>()?;
                self.budget
                    .retain(value.retained_byte_len())
                    .map_err(de::Error::custom)?;
                CanonicalValueInner::Reference(value)
            }
            ValueVariant::Record => {
                CanonicalValueInner::Record(map.next_value_seed(RecordSeed {
                    child_depth: self.depth + 1,
                    budget: self.budget,
                })?)
            }
            ValueVariant::Sequence => {
                CanonicalValueInner::Sequence(map.next_value_seed(SequenceSeed {
                    child_depth: self.depth + 1,
                    budget: self.budget,
                })?)
            }
            ValueVariant::Binary => {
                let value = map.next_value::<Asset>()?;
                self.budget
                    .retain(value.retained_byte_len())
                    .map_err(de::Error::custom)?;
                CanonicalValueInner::Binary(value)
            }
            ValueVariant::AssetReference => {
                let value = map.next_value::<AssetReference>()?;
                self.budget
                    .retain(value.retained_byte_len())
                    .map_err(de::Error::custom)?;
                CanonicalValueInner::AssetReference(value)
            }
        };
        if map.next_key::<IgnoredAny>()?.is_some() {
            return Err(de::Error::custom(
                "canonical value map must contain exactly one variant",
            ));
        }
        Ok(CanonicalValue { inner })
    }
}

struct SequenceSeed<'a> {
    child_depth: usize,
    budget: &'a mut ValueBudget,
}

impl<'de> DeserializeSeed<'de> for SequenceSeed<'_> {
    type Value = Vec<CanonicalValue>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(SequenceVisitor {
            child_depth: self.child_depth,
            budget: self.budget,
        })
    }
}

struct SequenceVisitor<'a> {
    child_depth: usize,
    budget: &'a mut ValueBudget,
}

impl<'de> Visitor<'de> for SequenceVisitor<'_> {
    type Value = Vec<CanonicalValue>;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "an ordered canonical sequence of at most {MAX_CANONICAL_COLLECTION_ITEMS} values"
        )
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::with_capacity(
            sequence
                .size_hint()
                .unwrap_or_default()
                .min(MAX_CANONICAL_COLLECTION_ITEMS),
        );
        while values.len() < MAX_CANONICAL_COLLECTION_ITEMS {
            let Some(value) = sequence.next_element_seed(CanonicalValueSeed {
                depth: self.child_depth,
                budget: &mut *self.budget,
            })?
            else {
                return Ok(values);
            };
            values.push(value);
        }
        if sequence.next_element::<IgnoredAny>()?.is_some() {
            return Err(de::Error::custom(format_args!(
                "canonical collection exceeds {MAX_CANONICAL_COLLECTION_ITEMS} items"
            )));
        }
        Ok(values)
    }
}

struct RecordSeed<'a> {
    child_depth: usize,
    budget: &'a mut ValueBudget,
}

impl<'de> DeserializeSeed<'de> for RecordSeed<'_> {
    type Value = Vec<CanonicalField>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(RecordVisitor {
            child_depth: self.child_depth,
            budget: self.budget,
        })
    }
}

struct RecordVisitor<'a> {
    child_depth: usize,
    budget: &'a mut ValueBudget,
}

impl<'de> Visitor<'de> for RecordVisitor<'_> {
    type Value = Vec<CanonicalField>;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "an ordered canonical record of at most {MAX_CANONICAL_COLLECTION_ITEMS} unique fields"
        )
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut fields = Vec::with_capacity(
            sequence
                .size_hint()
                .unwrap_or_default()
                .min(MAX_CANONICAL_COLLECTION_ITEMS),
        );
        let mut names = BTreeSet::new();
        while fields.len() < MAX_CANONICAL_COLLECTION_ITEMS {
            let Some(field) = sequence.next_element_seed(CanonicalFieldSeed {
                value_depth: self.child_depth,
                budget: &mut *self.budget,
            })?
            else {
                return Ok(fields);
            };
            if !names.insert(field.name.clone()) {
                return Err(de::Error::custom(ValueBuildError::DuplicateField {
                    name: field.name.as_str().to_owned(),
                }));
            }
            fields.push(field);
        }
        if sequence.next_element::<IgnoredAny>()?.is_some() {
            return Err(de::Error::custom(format_args!(
                "canonical collection exceeds {MAX_CANONICAL_COLLECTION_ITEMS} items"
            )));
        }
        Ok(fields)
    }
}

#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "snake_case")]
enum FieldKey {
    Name,
    Value,
}

struct CanonicalFieldSeed<'a> {
    value_depth: usize,
    budget: &'a mut ValueBudget,
}

impl<'de> DeserializeSeed<'de> for CanonicalFieldSeed<'_> {
    type Value = CanonicalField;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(CanonicalFieldVisitor {
            value_depth: self.value_depth,
            budget: self.budget,
        })
    }
}

struct CanonicalFieldVisitor<'a> {
    value_depth: usize,
    budget: &'a mut ValueBudget,
}

impl<'de> Visitor<'de> for CanonicalFieldVisitor<'_> {
    type Value = CanonicalField;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("a canonical field with exactly `name` and `value`")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut name = None;
        let mut value = None;
        while let Some(key) = map.next_key::<FieldKey>()? {
            match key {
                FieldKey::Name => {
                    if name.is_some() {
                        return Err(de::Error::duplicate_field("name"));
                    }
                    let parsed = map.next_value::<FieldName>()?;
                    self.budget
                        .retain(parsed.as_str().len())
                        .map_err(de::Error::custom)?;
                    name = Some(parsed);
                }
                FieldKey::Value => {
                    if value.is_some() {
                        return Err(de::Error::duplicate_field("value"));
                    }
                    value = Some(map.next_value_seed(CanonicalValueSeed {
                        depth: self.value_depth,
                        budget: self.budget,
                    })?);
                }
            }
        }
        Ok(CanonicalField {
            name: name.ok_or_else(|| de::Error::missing_field("name"))?,
            value: value.ok_or_else(|| de::Error::missing_field("value"))?,
        })
    }
}

impl<'de> Deserialize<'de> for CanonicalValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        CanonicalValueSeed {
            depth: 0,
            budget: &mut ValueBudget::default(),
        }
        .deserialize(deserializer)
    }
}

impl<'de> Deserialize<'de> for CanonicalField {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        CanonicalFieldSeed {
            value_depth: 0,
            budget: &mut ValueBudget::default(),
        }
        .deserialize(deserializer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn integer(value: &str) -> CanonicalValue {
        CanonicalValue::integer(CanonicalInteger::new(value).unwrap())
    }

    #[test]
    fn ordered_records_and_sequences_round_trip_without_reordering() {
        let first = CanonicalValue::record(vec![
            CanonicalField::named("z", integer("2")).unwrap(),
            CanonicalField::named("a", integer("1")).unwrap(),
        ])
        .unwrap();
        let shuffled = CanonicalValue::record(vec![
            CanonicalField::named("a", integer("1")).unwrap(),
            CanonicalField::named("z", integer("2")).unwrap(),
        ])
        .unwrap();
        assert_ne!(first, shuffled);
        assert_eq!(first.as_record().unwrap()[0].name().as_str(), "z");

        let sequence = CanonicalValue::sequence(vec![first.clone(), shuffled.clone()]).unwrap();
        let reverse = CanonicalValue::sequence(vec![shuffled, first]).unwrap();
        assert_ne!(sequence, reverse);
        let json = serde_json::to_string(&sequence).unwrap();
        assert_eq!(
            serde_json::from_str::<CanonicalValue>(&json).unwrap(),
            sequence
        );
    }

    #[test]
    fn future_enum_and_typed_scalars_and_reference_round_trip() {
        let value = CanonicalValue::record(vec![
            CanonicalField::named("null", CanonicalValue::null()).unwrap(),
            CanonicalField::named("bool", CanonicalValue::boolean(true)).unwrap(),
            CanonicalField::named("integer", integer("-123456789012345678901234567890")).unwrap(),
            CanonicalField::named(
                "decimal",
                CanonicalValue::decimal(CanonicalDecimal::new("-0.00125").unwrap()),
            )
            .unwrap(),
            CanonicalField::named(
                "text",
                CanonicalValue::text(CanonicalText::new("exact\ntext").unwrap()),
            )
            .unwrap(),
            CanonicalField::named(
                "enum",
                CanonicalValue::enum_token(EnumToken::new("vendor:future-v9").unwrap()),
            )
            .unwrap(),
            CanonicalField::named(
                "reference",
                CanonicalValue::reference(
                    UnresolvedReference::new("metadata:object", "uuid:future-target").unwrap(),
                ),
            )
            .unwrap(),
        ])
        .unwrap();
        let json = serde_json::to_string(&value).unwrap();
        let decoded = serde_json::from_str::<CanonicalValue>(&json).unwrap();
        assert_eq!(decoded, value);
        let fields = decoded.as_record().unwrap();
        assert!(matches!(
            fields[5].value().kind(),
            CanonicalValueKind::EnumToken(token) if token.as_str() == "vendor:future-v9"
        ));
    }

    #[test]
    fn inline_binary_and_asset_reference_are_distinct_typed_values() {
        let asset = Asset::from_bytes(vec![1, 2, 3, 4], "application/x-canonical-test").unwrap();
        let reference = asset.as_reference();
        let binary = CanonicalValue::binary(asset).unwrap();
        let referenced = CanonicalValue::asset_reference(reference.clone());

        assert!(matches!(
            binary.kind(),
            CanonicalValueKind::Binary(value) if value.sha256() == reference.sha256()
        ));
        assert!(matches!(
            referenced.kind(),
            CanonicalValueKind::AssetReference(value) if value == &reference
        ));
        for value in [binary, referenced] {
            let json = serde_json::to_string(&value).unwrap();
            assert_eq!(
                serde_json::from_str::<CanonicalValue>(&json).unwrap(),
                value
            );
        }
    }

    #[test]
    fn duplicate_policy_is_exact_case_sensitive_rejection() {
        let duplicate = CanonicalValue::record(vec![
            CanonicalField::named("Name", CanonicalValue::null()).unwrap(),
            CanonicalField::named("Name", CanonicalValue::boolean(false)).unwrap(),
        ]);
        assert!(matches!(
            duplicate,
            Err(ValueBuildError::DuplicateField { name }) if name == "Name"
        ));
        assert!(
            CanonicalValue::record(vec![
                CanonicalField::named("Name", CanonicalValue::null()).unwrap(),
                CanonicalField::named("name", CanonicalValue::null()).unwrap(),
            ])
            .is_ok()
        );

        let json =
            r#"{"record":[{"name":"x","value":{"null":null}},{"name":"x","value":{"bool":true}}]}"#;
        assert!(serde_json::from_str::<CanonicalValue>(json).is_err());
    }

    #[test]
    fn canonical_number_grammar_rejects_ambiguous_spellings() {
        for invalid in ["", "+1", "01", "-0", "1.0", "1e2", " 1"] {
            assert!(CanonicalInteger::new(invalid).is_err());
        }
        for invalid in ["", "+1", "01", "-0", "1.0", "1.20", ".1", "1.", "1e2"] {
            assert!(CanonicalDecimal::new(invalid).is_err(), "{invalid}");
        }
        for valid in ["0", "1", "-1", "0.01", "-0.01", "12.345"] {
            assert!(CanonicalDecimal::new(valid).is_ok(), "{valid}");
        }
    }

    #[test]
    fn public_deserializer_enforces_text_collection_depth_and_aggregate_bounds() {
        let oversized = "x".repeat(MAX_CANONICAL_TEXT_BYTES + 1);
        assert!(CanonicalText::new(&oversized).is_err());
        assert!(
            serde_json::from_value::<CanonicalValue>(serde_json::json!({"text": oversized}))
                .is_err()
        );

        let items = std::iter::repeat_n("{\"null\":null}", MAX_CANONICAL_COLLECTION_ITEMS + 1)
            .collect::<Vec<_>>()
            .join(",");
        let too_many = format!("{{\"sequence\":[{items}]}}");
        assert!(serde_json::from_str::<CanonicalValue>(&too_many).is_err());

        let mut too_deep = String::new();
        for _ in 0..=MAX_CANONICAL_DEPTH {
            too_deep.push_str("{\"sequence\":[");
        }
        too_deep.push_str("{\"null\":null}");
        for _ in 0..=MAX_CANONICAL_DEPTH {
            too_deep.push_str("]}");
        }
        assert!(serde_json::from_str::<CanonicalValue>(&too_deep).is_err());

        let mut built = CanonicalValue::null();
        for _ in 0..MAX_CANONICAL_DEPTH {
            built = CanonicalValue::sequence(vec![built]).unwrap();
        }
        assert!(matches!(
            CanonicalValue::sequence(vec![built]),
            Err(ValueBuildError::DepthExceeded { .. })
        ));

        let mut budget = ValueBudget::default();
        assert!(matches!(
            budget.retain(MAX_CANONICAL_RETAINED_BYTES + 1),
            Err(ValueBuildError::RetainedBytesExceeded { .. })
        ));
    }
}
