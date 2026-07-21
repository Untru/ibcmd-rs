//! Deterministic diagnostics and fail-closed loss handling.

use std::cmp::Ordering;
use std::collections::{BTreeMap, btree_map::Entry};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::marker::PhantomData;
use std::str::FromStr;

use serde::de::{IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use crate::artifact::ProfileId;

/// Maximum encoded length of a stable diagnostic code.
pub const MAX_DIAGNOSTIC_CODE_BYTES: usize = 128;
/// Maximum number of segments in either diagnostic path.
pub const MAX_PATH_SEGMENTS: usize = 64;
/// Maximum encoded length of one named path segment.
pub const MAX_PATH_NAME_BYTES: usize = 128;
/// Maximum encoded length of a diagnostic message or recovery hint.
pub const MAX_DIAGNOSTIC_TEXT_BYTES: usize = 4096;
/// Maximum number of deterministic context entries.
pub const MAX_CONTEXT_ENTRIES: usize = 64;
/// Maximum encoded length of a context key.
pub const MAX_CONTEXT_KEY_BYTES: usize = 128;
/// Maximum encoded length of a context value.
pub const MAX_CONTEXT_VALUE_BYTES: usize = 4096;

/// A stable, open diagnostic identifier such as `migration.unsupported-feature`.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DiagnosticCode(Box<str>);

impl DiagnosticCode {
    /// Validates a borrowed code before retaining it.
    pub fn new(input: &str) -> Result<Self, ParseDiagnosticCodeError> {
        validate_diagnostic_code(input)?;
        Ok(Self(input.into()))
    }

    /// Parses a stable diagnostic code.
    pub fn parse(input: &str) -> Result<Self, ParseDiagnosticCodeError> {
        Self::new(input)
    }

    /// Returns the exact canonical code.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn validate_diagnostic_code(input: &str) -> Result<(), ParseDiagnosticCodeError> {
    if input.is_empty() {
        return Err(ParseDiagnosticCodeError::new("diagnostic code is empty"));
    }
    if input.len() > MAX_DIAGNOSTIC_CODE_BYTES {
        return Err(ParseDiagnosticCodeError::new(
            "diagnostic code exceeds 128 bytes",
        ));
    }

    let bytes = input.as_bytes();
    if !bytes[0].is_ascii_lowercase() {
        return Err(ParseDiagnosticCodeError::new(
            "diagnostic code must start with a lowercase ASCII letter",
        ));
    }
    if !bytes[bytes.len() - 1].is_ascii_lowercase() && !bytes[bytes.len() - 1].is_ascii_digit() {
        return Err(ParseDiagnosticCodeError::new(
            "diagnostic code must end with a lowercase ASCII letter or digit",
        ));
    }

    let mut previous_was_separator = false;
    for byte in bytes {
        let separator = matches!(byte, b'.' | b'-');
        if !byte.is_ascii_lowercase() && !byte.is_ascii_digit() && !separator {
            return Err(ParseDiagnosticCodeError::new(
                "diagnostic code contains an invalid character",
            ));
        }
        if separator && previous_was_separator {
            return Err(ParseDiagnosticCodeError::new(
                "diagnostic code contains adjacent separators",
            ));
        }
        previous_was_separator = separator;
    }
    Ok(())
}

impl Display for DiagnosticCode {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for DiagnosticCode {
    type Err = ParseDiagnosticCodeError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Self::new(input)
    }
}

impl Serialize for DiagnosticCode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for DiagnosticCode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(ParseFromStringVisitor::<Self>(PhantomData))
    }
}

/// Error returned when a stable diagnostic code is invalid.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ParseDiagnosticCodeError {
    message: &'static str,
}

impl ParseDiagnosticCodeError {
    const fn new(message: &'static str) -> Self {
        Self { message }
    }
}

impl Display for ParseDiagnosticCodeError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message)
    }
}

impl Error for ParseDiagnosticCodeError {}

struct ParseFromStringVisitor<T>(PhantomData<fn() -> T>);

impl<'de, T> Visitor<'de> for ParseFromStringVisitor<T>
where
    T: FromStr,
    T::Err: Display,
{
    type Value = T;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("a valid bounded string")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        value.parse().map_err(E::custom)
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct BoundedString<const MAXIMUM: usize, const ALLOW_EMPTY: bool>(Box<str>);

impl<'de, const MAXIMUM: usize, const ALLOW_EMPTY: bool> Deserialize<'de>
    for BoundedString<MAXIMUM, ALLOW_EMPTY>
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(BoundedStringVisitor::<MAXIMUM, ALLOW_EMPTY>)
    }
}

struct BoundedStringVisitor<const MAXIMUM: usize, const ALLOW_EMPTY: bool>;

impl<'de, const MAXIMUM: usize, const ALLOW_EMPTY: bool> Visitor<'de>
    for BoundedStringVisitor<MAXIMUM, ALLOW_EMPTY>
{
    type Value = BoundedString<MAXIMUM, ALLOW_EMPTY>;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "a control-free string of at most {MAXIMUM} bytes"
        )
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        validate_text("bounded string", value, MAXIMUM, ALLOW_EMPTY).map_err(E::custom)?;
        Ok(BoundedString(value.into()))
    }
}

/// Machine-readable diagnostic severity.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Informational event that does not affect the operation.
    Info,
    /// Recoverable or explicitly accepted condition.
    Warning,
    /// Fail-closed condition.
    Error,
}

impl Severity {
    const fn sort_rank(self) -> u8 {
        match self {
            Self::Error => 0,
            Self::Warning => 1,
            Self::Info => 2,
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
enum PathSegmentValue {
    Name(Box<str>),
    Index(u32),
}

/// One validated segment in an object or property path.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PathSegment(PathSegmentValue);

impl PathSegment {
    /// Creates a named segment after validating its borrowed value.
    pub fn name(value: &str) -> Result<Self, DiagnosticBuildError> {
        validate_text("path segment", value, MAX_PATH_NAME_BYTES, false)?;
        Ok(Self(PathSegmentValue::Name(value.into())))
    }

    /// Creates an indexed segment.
    pub const fn index(value: u32) -> Self {
        Self(PathSegmentValue::Index(value))
    }

    /// Returns the named value, when this is a named segment.
    pub fn as_name(&self) -> Option<&str> {
        match &self.0 {
            PathSegmentValue::Name(value) => Some(value),
            PathSegmentValue::Index(_) => None,
        }
    }

    /// Returns the index, when this is an indexed segment.
    pub const fn as_index(&self) -> Option<u32> {
        match self.0 {
            PathSegmentValue::Index(value) => Some(value),
            PathSegmentValue::Name(_) => None,
        }
    }
}

impl Serialize for PathSegment {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for PathSegment {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(tag = "kind", content = "value", rename_all = "snake_case")]
        enum RawSegment {
            Name(BoundedString<MAX_PATH_NAME_BYTES, false>),
            Index(u32),
        }

        match RawSegment::deserialize(deserializer)? {
            RawSegment::Name(value) => Ok(Self(PathSegmentValue::Name(value.0))),
            RawSegment::Index(value) => Ok(Self::index(value)),
        }
    }
}

fn write_path(segments: &[PathSegment], formatter: &mut Formatter<'_>) -> fmt::Result {
    formatter.write_str("$")?;
    for segment in segments {
        match &segment.0 {
            PathSegmentValue::Name(value) => {
                formatter.write_str("/name:")?;
                for character in value.chars() {
                    match character {
                        '~' => formatter.write_str("~0")?,
                        '/' => formatter.write_str("~1")?,
                        _ => character.fmt(formatter)?,
                    }
                }
            }
            PathSegmentValue::Index(value) => write!(formatter, "/index:{value}")?,
        }
    }
    Ok(())
}

trait BoundedPath: Sized {
    fn from_bounded_segments(segments: Vec<PathSegment>) -> Self;
}

struct BoundedPathVisitor<T>(PhantomData<fn() -> T>);

impl<'de, T> Visitor<'de> for BoundedPathVisitor<T>
where
    T: BoundedPath,
{
    type Value = T;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "a diagnostic path containing at most {MAX_PATH_SEGMENTS} segments"
        )
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut segments = Vec::with_capacity(
            sequence
                .size_hint()
                .unwrap_or_default()
                .min(MAX_PATH_SEGMENTS),
        );
        while segments.len() < MAX_PATH_SEGMENTS {
            let Some(segment) = sequence.next_element::<PathSegment>()? else {
                return Ok(T::from_bounded_segments(segments));
            };
            segments.push(segment);
        }

        if sequence.next_element::<IgnoredAny>()?.is_some() {
            return Err(de::Error::custom(format_args!(
                "path contains more than {MAX_PATH_SEGMENTS} segments"
            )));
        }
        Ok(T::from_bounded_segments(segments))
    }
}

macro_rules! diagnostic_path {
    ($name:ident, $documentation:literal) => {
        #[doc = $documentation]
        #[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
        pub struct $name(Vec<PathSegment>);

        impl $name {
            /// Creates a path after enforcing the public segment-count bound.
            pub fn new(segments: Vec<PathSegment>) -> Result<Self, DiagnosticBuildError> {
                if segments.len() > MAX_PATH_SEGMENTS {
                    return Err(DiagnosticBuildError::TooManyPathSegments {
                        maximum: MAX_PATH_SEGMENTS,
                    });
                }
                Ok(Self(segments))
            }

            /// Returns the root path.
            pub const fn root() -> Self {
                Self(Vec::new())
            }

            /// Appends a segment while preserving the bound.
            pub fn push(&mut self, segment: PathSegment) -> Result<(), DiagnosticBuildError> {
                if self.0.len() == MAX_PATH_SEGMENTS {
                    return Err(DiagnosticBuildError::TooManyPathSegments {
                        maximum: MAX_PATH_SEGMENTS,
                    });
                }
                self.0.push(segment);
                Ok(())
            }

            /// Returns path segments in source order.
            pub fn segments(&self) -> &[PathSegment] {
                &self.0
            }
        }

        impl Display for $name {
            fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
                write_path(&self.0, formatter)
            }
        }

        impl BoundedPath for $name {
            fn from_bounded_segments(segments: Vec<PathSegment>) -> Self {
                Self(segments)
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                self.0.serialize(serializer)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                deserializer.deserialize_seq(BoundedPathVisitor::<Self>(PhantomData))
            }
        }
    };
}

diagnostic_path!(
    ObjectPath,
    "Structured path to an artifact or metadata object."
);
diagnostic_path!(
    PropertyPath,
    "Structured path to a property within an object."
);

/// Failure to construct a bounded diagnostic value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiagnosticBuildError {
    /// A required text value was empty.
    EmptyText {
        /// Field name.
        field: &'static str,
    },
    /// A text value exceeded its encoded bound.
    TextTooLong {
        /// Field name.
        field: &'static str,
        /// Maximum accepted bytes.
        maximum: usize,
    },
    /// A text value contained a control character.
    ControlCharacter {
        /// Field name.
        field: &'static str,
    },
    /// A path exceeded its segment bound.
    TooManyPathSegments {
        /// Maximum accepted segments.
        maximum: usize,
    },
    /// Context contained too many entries.
    TooManyContextEntries {
        /// Maximum accepted entries.
        maximum: usize,
    },
    /// A context key was supplied twice.
    DuplicateContextKey {
        /// Duplicate key.
        key: String,
    },
    /// A context key violated its stable grammar.
    InvalidContextKey {
        /// Invalid key.
        key: String,
    },
}

impl Display for DiagnosticBuildError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyText { field } => write!(formatter, "{field} is empty"),
            Self::TextTooLong { field, maximum } => {
                write!(formatter, "{field} exceeds {maximum} bytes")
            }
            Self::ControlCharacter { field } => {
                write!(formatter, "{field} contains a control character")
            }
            Self::TooManyPathSegments { maximum } => {
                write!(formatter, "path contains more than {maximum} segments")
            }
            Self::TooManyContextEntries { maximum } => {
                write!(formatter, "context contains more than {maximum} entries")
            }
            Self::DuplicateContextKey { key } => write!(formatter, "duplicate context key `{key}`"),
            Self::InvalidContextKey { key } => write!(formatter, "invalid context key `{key}`"),
        }
    }
}

impl Error for DiagnosticBuildError {}

fn validate_text(
    field: &'static str,
    value: &str,
    maximum: usize,
    allow_empty: bool,
) -> Result<(), DiagnosticBuildError> {
    if value.is_empty() && !allow_empty {
        return Err(DiagnosticBuildError::EmptyText { field });
    }
    if value.len() > maximum {
        return Err(DiagnosticBuildError::TextTooLong { field, maximum });
    }
    if value.chars().any(char::is_control) {
        return Err(DiagnosticBuildError::ControlCharacter { field });
    }
    Ok(())
}

fn validate_context_key(key: &str) -> Result<(), DiagnosticBuildError> {
    validate_text("context key", key, MAX_CONTEXT_KEY_BYTES, false)?;
    let bytes = key.as_bytes();
    if !bytes[0].is_ascii_alphanumeric()
        || !bytes[bytes.len() - 1].is_ascii_alphanumeric()
        || !bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(DiagnosticBuildError::InvalidContextKey {
            key: key.to_owned(),
        });
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ContextKey(Box<str>);

impl Display for ContextKey {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ContextKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(ContextKeyVisitor)
    }
}

struct ContextKeyVisitor;

impl<'de> Visitor<'de> for ContextKeyVisitor {
    type Value = ContextKey;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("a bounded diagnostic context key")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        validate_context_key(value).map_err(E::custom)?;
        Ok(ContextKey(value.into()))
    }
}

/// One stable, path-addressed diagnostic.
///
/// JSON fields have the fixed order `code`, `severity`, `object_path`,
/// `property_path`, `source_profile`, `target_profile`, `message`,
/// `recovery_hint`, and `context`. Paths are arrays of tagged segments and
/// context is key-sorted, so serialization is deterministic.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Diagnostic {
    code: DiagnosticCode,
    severity: Severity,
    object_path: ObjectPath,
    property_path: PropertyPath,
    source_profile: Option<ProfileId>,
    target_profile: Option<ProfileId>,
    message: Box<str>,
    recovery_hint: Option<Box<str>>,
    context: BTreeMap<String, String>,
}

impl Diagnostic {
    /// Creates a bounded diagnostic with empty optional fields and context.
    pub fn new(
        code: DiagnosticCode,
        severity: Severity,
        object_path: ObjectPath,
        property_path: PropertyPath,
        message: &str,
    ) -> Result<Self, DiagnosticBuildError> {
        validate_text(
            "diagnostic message",
            message,
            MAX_DIAGNOSTIC_TEXT_BYTES,
            false,
        )?;
        Ok(Self {
            code,
            severity,
            object_path,
            property_path,
            source_profile: None,
            target_profile: None,
            message: message.into(),
            recovery_hint: None,
            context: BTreeMap::new(),
        })
    }

    /// Adds independent source and target profile coordinates.
    pub fn with_profiles(
        mut self,
        source_profile: Option<ProfileId>,
        target_profile: Option<ProfileId>,
    ) -> Self {
        self.source_profile = source_profile;
        self.target_profile = target_profile;
        self
    }

    /// Adds a bounded recovery hint.
    pub fn with_recovery_hint(mut self, hint: &str) -> Result<Self, DiagnosticBuildError> {
        validate_text("recovery hint", hint, MAX_DIAGNOSTIC_TEXT_BYTES, false)?;
        self.recovery_hint = Some(hint.into());
        Ok(self)
    }

    /// Adds one deterministic context entry without overwriting an existing key.
    pub fn with_context(mut self, key: &str, value: &str) -> Result<Self, DiagnosticBuildError> {
        validate_context_key(key)?;
        validate_text("context value", value, MAX_CONTEXT_VALUE_BYTES, true)?;
        if self.context.len() == MAX_CONTEXT_ENTRIES {
            return Err(DiagnosticBuildError::TooManyContextEntries {
                maximum: MAX_CONTEXT_ENTRIES,
            });
        }
        match self.context.entry(key.to_owned()) {
            Entry::Vacant(entry) => {
                entry.insert(value.to_owned());
            }
            Entry::Occupied(entry) => {
                return Err(DiagnosticBuildError::DuplicateContextKey {
                    key: entry.key().clone(),
                });
            }
        }
        Ok(self)
    }

    /// Returns the stable code.
    pub fn code(&self) -> &DiagnosticCode {
        &self.code
    }

    /// Returns the current severity.
    pub const fn severity(&self) -> Severity {
        self.severity
    }

    /// Returns the object path.
    pub fn object_path(&self) -> &ObjectPath {
        &self.object_path
    }

    /// Returns the property path.
    pub fn property_path(&self) -> &PropertyPath {
        &self.property_path
    }

    /// Returns the source profile, when known.
    pub fn source_profile(&self) -> Option<&ProfileId> {
        self.source_profile.as_ref()
    }

    /// Returns the target profile, when known.
    pub fn target_profile(&self) -> Option<&ProfileId> {
        self.target_profile.as_ref()
    }

    /// Returns the human-readable message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns the recovery hint, when supplied.
    pub fn recovery_hint(&self) -> Option<&str> {
        self.recovery_hint.as_deref()
    }

    /// Returns context entries in key order.
    pub fn context(&self) -> &BTreeMap<String, String> {
        &self.context
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDiagnostic {
    code: DiagnosticCode,
    severity: Severity,
    object_path: ObjectPath,
    property_path: PropertyPath,
    source_profile: Option<ProfileId>,
    target_profile: Option<ProfileId>,
    message: BoundedString<MAX_DIAGNOSTIC_TEXT_BYTES, false>,
    recovery_hint: Option<BoundedString<MAX_DIAGNOSTIC_TEXT_BYTES, false>>,
    #[serde(deserialize_with = "deserialize_unique_context")]
    context: BTreeMap<ContextKey, BoundedString<MAX_CONTEXT_VALUE_BYTES, true>>,
}

impl<'de> Deserialize<'de> for Diagnostic {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawDiagnostic::deserialize(deserializer)?;
        Ok(Self {
            code: raw.code,
            severity: raw.severity,
            object_path: raw.object_path,
            property_path: raw.property_path,
            source_profile: raw.source_profile,
            target_profile: raw.target_profile,
            message: raw.message.0,
            recovery_hint: raw.recovery_hint.map(|value| value.0),
            context: raw
                .context
                .into_iter()
                .map(|(key, value)| (String::from(key.0), String::from(value.0)))
                .collect(),
        })
    }
}

fn deserialize_unique_context<'de, D>(
    deserializer: D,
) -> Result<BTreeMap<ContextKey, BoundedString<MAX_CONTEXT_VALUE_BYTES, true>>, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_map(UniqueContextVisitor)
}

struct UniqueContextVisitor;

impl<'de> Visitor<'de> for UniqueContextVisitor {
    type Value = BTreeMap<ContextKey, BoundedString<MAX_CONTEXT_VALUE_BYTES, true>>;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("an object with unique diagnostic context keys")
    }

    fn visit_map<A>(self, mut access: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = BTreeMap::new();
        while values.len() < MAX_CONTEXT_ENTRIES {
            let Some(key) = access.next_key::<ContextKey>()? else {
                return Ok(values);
            };
            let value = access.next_value::<BoundedString<MAX_CONTEXT_VALUE_BYTES, true>>()?;
            match values.entry(key) {
                Entry::Vacant(entry) => {
                    entry.insert(value);
                }
                Entry::Occupied(entry) => {
                    return Err(de::Error::custom(format_args!(
                        "duplicate diagnostic context key `{}`",
                        entry.key()
                    )));
                }
            }
        }

        if access.next_key::<IgnoredAny>()?.is_some() {
            return Err(de::Error::custom(format_args!(
                "context contains more than {MAX_CONTEXT_ENTRIES} entries"
            )));
        }
        Ok(values)
    }
}

fn compare_diagnostics(left: &Diagnostic, right: &Diagnostic) -> Ordering {
    left.severity
        .sort_rank()
        .cmp(&right.severity.sort_rank())
        .then_with(|| left.code.cmp(&right.code))
        .then_with(|| left.object_path.cmp(&right.object_path))
        .then_with(|| left.property_path.cmp(&right.property_path))
        .then_with(|| left.source_profile.cmp(&right.source_profile))
        .then_with(|| left.target_profile.cmp(&right.target_profile))
        .then_with(|| left.message.cmp(&right.message))
        .then_with(|| left.recovery_hint.cmp(&right.recovery_hint))
        .then_with(|| left.context.cmp(&right.context))
}

/// Canonically ordered collection of diagnostics.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct DiagnosticReport {
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticReport {
    /// Creates an empty report.
    pub const fn new() -> Self {
        Self {
            diagnostics: Vec::new(),
        }
    }

    /// Creates and canonically sorts a report.
    pub fn from_diagnostics(mut diagnostics: Vec<Diagnostic>) -> Self {
        diagnostics.sort_by(compare_diagnostics);
        Self { diagnostics }
    }

    /// Inserts a diagnostic while preserving canonical order.
    pub fn push(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
        self.diagnostics.sort_by(compare_diagnostics);
    }

    /// Returns diagnostics in canonical order.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Returns whether the report contains at least one error.
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error)
    }
}

impl<'de> Deserialize<'de> for DiagnosticReport {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawReport {
            diagnostics: Vec<Diagnostic>,
        }

        let raw = RawReport::deserialize(deserializer)?;
        Ok(Self::from_diagnostics(raw.diagnostics))
    }
}

/// User-selected policy for a known loss.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LossPolicy {
    /// Reject the operation. This is the fail-closed default.
    #[default]
    Error,
    /// Continue with a warning only for a codec-declared loss.
    Warn,
    /// Drop only when the codec explicitly declares that exact loss droppable.
    DropExplicitly,
}

/// Permission granted by the codec for one exact loss code.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CodecLossPermission {
    /// The loss can be accepted as a warning but not explicitly dropped.
    WarnOnly,
    /// The loss can be accepted as a warning or explicitly dropped.
    DropAllowed,
}

/// Codec-owned declaration of a known loss.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CodecLossDeclaration {
    code: DiagnosticCode,
    permission: CodecLossPermission,
    reason: Box<str>,
}

impl CodecLossDeclaration {
    /// Declares a loss code and its maximum allowed disposition.
    pub fn new(
        code: DiagnosticCode,
        permission: CodecLossPermission,
        reason: &str,
    ) -> Result<Self, DiagnosticBuildError> {
        validate_text(
            "codec loss reason",
            reason,
            MAX_DIAGNOSTIC_TEXT_BYTES,
            false,
        )?;
        Ok(Self {
            code,
            permission,
            reason: reason.into(),
        })
    }

    /// Returns the exact declared diagnostic code.
    pub fn code(&self) -> &DiagnosticCode {
        &self.code
    }

    /// Returns the codec permission.
    pub const fn permission(&self) -> CodecLossPermission {
        self.permission
    }

    /// Returns the declaration reason.
    pub fn reason(&self) -> &str {
        &self.reason
    }
}

impl<'de> Deserialize<'de> for CodecLossDeclaration {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawDeclaration {
            code: DiagnosticCode,
            permission: CodecLossPermission,
            reason: BoundedString<MAX_DIAGNOSTIC_TEXT_BYTES, false>,
        }

        let raw = RawDeclaration::deserialize(deserializer)?;
        Ok(Self {
            code: raw.code,
            permission: raw.permission,
            reason: raw.reason.0,
        })
    }
}

/// Observable result kind returned only by [`evaluate_loss`].
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LossDispositionKind {
    /// Operation may continue while reporting a warning.
    ContinueWithWarning,
    /// Codec-declared loss was explicitly accepted as a drop.
    DroppedExplicitly,
}

/// Opaque evaluated loss. Its disposition cannot be constructed by callers.
#[must_use = "an allowed loss disposition must be handled explicitly"]
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct LossDisposition {
    kind: LossDispositionKind,
    diagnostic: Diagnostic,
}

impl LossDisposition {
    /// Returns the evaluated disposition kind.
    pub const fn kind(&self) -> LossDispositionKind {
        self.kind
    }

    /// Returns the normalized diagnostic.
    pub fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }
}

/// Fail-closed loss-policy evaluation error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LossPolicyError {
    /// The fail-closed error policy rejected the loss.
    Rejected {
        /// Original error diagnostic.
        diagnostic: Box<Diagnostic>,
    },
    /// Loss evaluation requires an error diagnostic as its input.
    DiagnosticMustBeError {
        /// Actual severity.
        actual: Severity,
    },
    /// A non-error policy had no codec declaration.
    MissingCodecDeclaration {
        /// Undeclared diagnostic code.
        code: DiagnosticCode,
    },
    /// The codec declaration was for a different code.
    DeclarationCodeMismatch {
        /// Diagnostic code.
        diagnostic_code: DiagnosticCode,
        /// Declared code.
        declared_code: DiagnosticCode,
    },
    /// Explicit drop was requested for a warning-only declaration.
    ExplicitDropNotPermitted {
        /// Exact diagnostic code.
        code: DiagnosticCode,
    },
}

impl Display for LossPolicyError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rejected { diagnostic } => {
                write!(formatter, "loss `{}` was rejected", diagnostic.code())
            }
            Self::DiagnosticMustBeError { actual } => {
                write!(formatter, "loss diagnostic must be error, found {actual:?}")
            }
            Self::MissingCodecDeclaration { code } => {
                write!(formatter, "loss `{code}` has no codec declaration")
            }
            Self::DeclarationCodeMismatch {
                diagnostic_code,
                declared_code,
            } => write!(
                formatter,
                "loss `{diagnostic_code}` does not match codec declaration `{declared_code}`"
            ),
            Self::ExplicitDropNotPermitted { code } => {
                write!(formatter, "loss `{code}` is not declared droppable")
            }
        }
    }
}

impl Error for LossPolicyError {}

/// Evaluates one exact loss under a fail-closed policy.
///
/// `Warn` and `DropExplicitly` cannot return a disposition without a matching
/// codec declaration. Only `DropAllowed` can produce `DroppedExplicitly`.
#[must_use = "loss evaluation must be propagated and its disposition handled"]
pub fn evaluate_loss(
    policy: LossPolicy,
    mut diagnostic: Diagnostic,
    declaration: Option<&CodecLossDeclaration>,
) -> Result<LossDisposition, LossPolicyError> {
    if diagnostic.severity != Severity::Error {
        return Err(LossPolicyError::DiagnosticMustBeError {
            actual: diagnostic.severity,
        });
    }

    if policy == LossPolicy::Error {
        return Err(LossPolicyError::Rejected {
            diagnostic: Box::new(diagnostic),
        });
    }

    let declaration = declaration.ok_or_else(|| LossPolicyError::MissingCodecDeclaration {
        code: diagnostic.code.clone(),
    })?;
    if declaration.code != diagnostic.code {
        return Err(LossPolicyError::DeclarationCodeMismatch {
            diagnostic_code: diagnostic.code.clone(),
            declared_code: declaration.code.clone(),
        });
    }

    match policy {
        LossPolicy::Error => unreachable!("handled above"),
        LossPolicy::Warn => {
            diagnostic.severity = Severity::Warning;
            Ok(LossDisposition {
                kind: LossDispositionKind::ContinueWithWarning,
                diagnostic,
            })
        }
        LossPolicy::DropExplicitly => {
            if declaration.permission != CodecLossPermission::DropAllowed {
                return Err(LossPolicyError::ExplicitDropNotPermitted {
                    code: diagnostic.code.clone(),
                });
            }
            diagnostic.severity = Severity::Warning;
            Ok(LossDisposition {
                kind: LossDispositionKind::DroppedExplicitly,
                diagnostic,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code(value: &str) -> DiagnosticCode {
        value.parse().unwrap()
    }

    fn diagnostic(severity: Severity) -> Diagnostic {
        let object_path = ObjectPath::new(vec![
            PathSegment::name("Documents").unwrap(),
            PathSegment::name("Invoice/2026~draft").unwrap(),
        ])
        .unwrap();
        let property_path = PropertyPath::new(vec![
            PathSegment::name("Fields").unwrap(),
            PathSegment::index(2),
        ])
        .unwrap();
        Diagnostic::new(
            code("migration.unsupported-feature"),
            severity,
            object_path,
            property_path,
            "Feature is unsupported",
        )
        .unwrap()
    }

    #[test]
    fn diagnostic_json_is_stable_and_round_trips() {
        let value = diagnostic(Severity::Error)
            .with_profiles(
                Some(ProfileId::parse("source-1").unwrap()),
                Some(ProfileId::parse("target-1").unwrap()),
            )
            .with_recovery_hint("Choose a compatible target")
            .unwrap()
            .with_context("zeta", "2")
            .unwrap()
            .with_context("alpha", "1")
            .unwrap();

        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(
            json,
            r#"{"code":"migration.unsupported-feature","severity":"error","object_path":[{"kind":"name","value":"Documents"},{"kind":"name","value":"Invoice/2026~draft"}],"property_path":[{"kind":"name","value":"Fields"},{"kind":"index","value":2}],"source_profile":"source-1","target_profile":"target-1","message":"Feature is unsupported","recovery_hint":"Choose a compatible target","context":{"alpha":"1","zeta":"2"}}"#
        );
        assert_eq!(serde_json::from_str::<Diagnostic>(&json).unwrap(), value);
    }

    #[test]
    fn path_display_is_typed_and_unambiguous() {
        let named_index = ObjectPath::new(vec![PathSegment::name("[2]").unwrap()]).unwrap();
        let real_index = ObjectPath::new(vec![PathSegment::index(2)]).unwrap();
        let escaped = ObjectPath::new(vec![PathSegment::name("a/b~c").unwrap()]).unwrap();

        assert_eq!(named_index.to_string(), "$/name:[2]");
        assert_eq!(real_index.to_string(), "$/index:2");
        assert_eq!(escaped.to_string(), "$/name:a~1b~0c");
        assert_ne!(named_index.to_string(), real_index.to_string());
    }

    #[test]
    fn report_is_deterministic_for_shuffled_input_and_context() {
        let first = diagnostic(Severity::Warning)
            .with_context("zeta", "2")
            .unwrap()
            .with_context("alpha", "1")
            .unwrap();
        let second = Diagnostic::new(
            code("decode.invalid-reference"),
            Severity::Error,
            ObjectPath::root(),
            PropertyPath::root(),
            "Invalid reference",
        )
        .unwrap();
        let reordered_first = diagnostic(Severity::Warning)
            .with_context("alpha", "1")
            .unwrap()
            .with_context("zeta", "2")
            .unwrap();

        let left = DiagnosticReport::from_diagnostics(vec![first, second.clone()]);
        let right = DiagnosticReport::from_diagnostics(vec![second, reordered_first]);
        assert_eq!(left, right);
        assert_eq!(
            serde_json::to_string(&left).unwrap(),
            serde_json::to_string(&right).unwrap()
        );
        assert!(left.has_errors());
        assert_eq!(left.diagnostics()[0].severity(), Severity::Error);
    }

    #[test]
    fn bounded_values_and_duplicate_context_are_rejected() {
        for invalid in ["", "Upper.case", "a..b", "a_underscore", "a-"] {
            assert!(DiagnosticCode::parse(invalid).is_err(), "{invalid}");
        }
        assert!(DiagnosticCode::parse(&format!("a{}", "b".repeat(128))).is_err());
        assert!(PathSegment::name("").is_err());
        assert!(PathSegment::name("line\nbreak").is_err());
        assert!(PathSegment::name(&"x".repeat(MAX_PATH_NAME_BYTES + 1)).is_err());
        assert!(ObjectPath::new(vec![PathSegment::index(0); MAX_PATH_SEGMENTS + 1]).is_err());
        assert!(
            Diagnostic::new(
                code("test.loss"),
                Severity::Error,
                ObjectPath::root(),
                PropertyPath::root(),
                ""
            )
            .is_err()
        );

        let value = diagnostic(Severity::Error)
            .with_context("key", "one")
            .unwrap();
        assert!(value.with_context("key", "two").is_err());
    }

    #[test]
    fn loss_policy_defaults_to_error_and_rejects() {
        assert_eq!(LossPolicy::default(), LossPolicy::Error);
        let declaration = CodecLossDeclaration::new(
            code("migration.unsupported-feature"),
            CodecLossPermission::DropAllowed,
            "Known downgrade loss",
        )
        .unwrap();
        let result = evaluate_loss(
            LossPolicy::Error,
            diagnostic(Severity::Error),
            Some(&declaration),
        );
        let Err(LossPolicyError::Rejected { diagnostic }) = result else {
            panic!("default policy must return a typed rejection error");
        };
        assert_eq!(diagnostic.code().as_str(), "migration.unsupported-feature");
        assert_eq!(diagnostic.severity(), Severity::Error);
    }

    #[test]
    fn warn_requires_an_exact_codec_declaration() {
        assert!(matches!(
            evaluate_loss(LossPolicy::Warn, diagnostic(Severity::Error), None),
            Err(LossPolicyError::MissingCodecDeclaration { .. })
        ));
        let mismatch = CodecLossDeclaration::new(
            code("migration.other-loss"),
            CodecLossPermission::WarnOnly,
            "Different loss",
        )
        .unwrap();
        assert!(matches!(
            evaluate_loss(
                LossPolicy::Warn,
                diagnostic(Severity::Error),
                Some(&mismatch)
            ),
            Err(LossPolicyError::DeclarationCodeMismatch { .. })
        ));

        let declaration = CodecLossDeclaration::new(
            code("migration.unsupported-feature"),
            CodecLossPermission::WarnOnly,
            "Known loss",
        )
        .unwrap();
        let result = evaluate_loss(
            LossPolicy::Warn,
            diagnostic(Severity::Error),
            Some(&declaration),
        )
        .unwrap();
        assert_eq!(result.kind(), LossDispositionKind::ContinueWithWarning);
        assert_eq!(result.diagnostic().severity(), Severity::Warning);
    }

    #[test]
    fn explicit_drop_requires_exact_drop_permission() {
        assert!(matches!(
            evaluate_loss(
                LossPolicy::DropExplicitly,
                diagnostic(Severity::Error),
                None
            ),
            Err(LossPolicyError::MissingCodecDeclaration { .. })
        ));
        let warn_only = CodecLossDeclaration::new(
            code("migration.unsupported-feature"),
            CodecLossPermission::WarnOnly,
            "Known but not droppable",
        )
        .unwrap();
        assert!(matches!(
            evaluate_loss(
                LossPolicy::DropExplicitly,
                diagnostic(Severity::Error),
                Some(&warn_only)
            ),
            Err(LossPolicyError::ExplicitDropNotPermitted { .. })
        ));
        let mismatch = CodecLossDeclaration::new(
            code("migration.other-loss"),
            CodecLossPermission::DropAllowed,
            "Different droppable loss",
        )
        .unwrap();
        assert!(matches!(
            evaluate_loss(
                LossPolicy::DropExplicitly,
                diagnostic(Severity::Error),
                Some(&mismatch)
            ),
            Err(LossPolicyError::DeclarationCodeMismatch { .. })
        ));
        let droppable = CodecLossDeclaration::new(
            code("migration.unsupported-feature"),
            CodecLossPermission::DropAllowed,
            "Codec can omit this field",
        )
        .unwrap();
        let result = evaluate_loss(
            LossPolicy::DropExplicitly,
            diagnostic(Severity::Error),
            Some(&droppable),
        )
        .unwrap();
        assert_eq!(result.kind(), LossDispositionKind::DroppedExplicitly);
        assert_eq!(result.diagnostic().severity(), Severity::Warning);
    }

    #[test]
    fn non_error_input_cannot_enter_loss_evaluation() {
        let declaration = CodecLossDeclaration::new(
            code("migration.unsupported-feature"),
            CodecLossPermission::DropAllowed,
            "Known loss",
        )
        .unwrap();
        assert!(matches!(
            evaluate_loss(
                LossPolicy::DropExplicitly,
                diagnostic(Severity::Warning),
                Some(&declaration)
            ),
            Err(LossPolicyError::DiagnosticMustBeError { .. })
        ));
    }

    #[test]
    fn policy_and_declaration_serde_are_stable() {
        assert_eq!(
            serde_json::to_string(&LossPolicy::DropExplicitly).unwrap(),
            r#""drop_explicitly""#
        );
        assert_eq!(
            serde_json::from_str::<LossPolicy>(r#""warn""#).unwrap(),
            LossPolicy::Warn
        );
        let declaration = CodecLossDeclaration::new(
            code("migration.unsupported-feature"),
            CodecLossPermission::DropAllowed,
            "Known loss",
        )
        .unwrap();
        let json = serde_json::to_string(&declaration).unwrap();
        assert_eq!(
            serde_json::from_str::<CodecLossDeclaration>(&json).unwrap(),
            declaration
        );
    }

    #[test]
    fn deserialization_enforces_path_bounds() {
        let segments = (0..=MAX_PATH_SEGMENTS)
            .map(|index| format!(r#"{{"kind":"index","value":{index}}}"#))
            .collect::<Vec<_>>()
            .join(",");
        let error = serde_json::from_str::<ObjectPath>(&format!("[{segments}]"))
            .expect_err("the sixty-fifth segment must be rejected");
        assert!(error.to_string().contains("more than 64 segments"));

        let long_name = "x".repeat(MAX_PATH_NAME_BYTES + 1);
        let json = format!(r#"[{{"kind":"name","value":"{long_name}"}}]"#);
        assert!(serde_json::from_str::<PropertyPath>(&json).is_err());
    }

    #[test]
    fn deserialization_enforces_diagnostic_text_and_context_bounds() {
        let base = serde_json::to_string(&diagnostic(Severity::Error)).unwrap();

        let long_message = "x".repeat(MAX_DIAGNOSTIC_TEXT_BYTES + 1);
        let json = base.replace(
            r#""message":"Feature is unsupported""#,
            &format!(r#""message":"{long_message}""#),
        );
        assert!(serde_json::from_str::<Diagnostic>(&json).is_err());

        let long_hint = "x".repeat(MAX_DIAGNOSTIC_TEXT_BYTES + 1);
        let json = base.replace(
            r#""recovery_hint":null"#,
            &format!(r#""recovery_hint":"{long_hint}""#),
        );
        assert!(serde_json::from_str::<Diagnostic>(&json).is_err());

        let entries = (0..=MAX_CONTEXT_ENTRIES)
            .map(|index| format!(r#""key-{index}":"value""#))
            .collect::<Vec<_>>()
            .join(",");
        let json = base.replace(r#""context":{}"#, &format!(r#""context":{{{entries}}}"#));
        let error = serde_json::from_str::<Diagnostic>(&json)
            .expect_err("the sixty-fifth context entry must be rejected");
        assert!(error.to_string().contains("more than 64 entries"));

        let long_value = "x".repeat(MAX_CONTEXT_VALUE_BYTES + 1);
        let json = base.replace(
            r#""context":{}"#,
            &format!(r#""context":{{"key":"{long_value}"}}"#),
        );
        assert!(serde_json::from_str::<Diagnostic>(&json).is_err());

        let duplicate = base.replace(r#""context":{}"#, r#""context":{"key":"one","key":"two"}"#);
        let error = serde_json::from_str::<Diagnostic>(&duplicate)
            .expect_err("duplicate context keys must be rejected");
        assert!(
            error
                .to_string()
                .contains("duplicate diagnostic context key")
        );
    }
}
