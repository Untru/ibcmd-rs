//! Neutral, ordered representation of configuration-storage entries.
//!
//! The types in this module deliberately do not expose SQL rows, CF container
//! records, filesystem paths, or process handles. Storage adapters translate
//! their native records into this model and explicitly choose the source
//! order carried by [`StorageImage`] and [`StoragePatch`].

use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::marker::PhantomData;

use serde::de::{IgnoredAny, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use sha2::{Digest, Sha256};

use crate::artifact::StorageProfileId;

/// Maximum UTF-8 size of a logical storage-entry name.
pub const MAX_STORAGE_NAME_BYTES: usize = 1_024;
/// Maximum UTF-8 size of a logical storage-entry key.
pub const MAX_STORAGE_KEY_BYTES: usize = 1_024;
/// Maximum UTF-8 size of an open compression-kind identifier.
pub const MAX_COMPRESSION_KIND_BYTES: usize = 64;
/// Maximum UTF-8 size of entry provenance.
pub const MAX_STORAGE_PROVENANCE_BYTES: usize = 4_096;
/// Maximum size of either opaque attributes or a raw element header.
pub const MAX_OPAQUE_STORAGE_METADATA_BYTES: usize = 1_048_576;
/// Maximum retained size of one packed or unpacked payload.
pub const MAX_STORAGE_PAYLOAD_BYTES: usize = 536_870_912;
/// Maximum number of parts in one logical entry.
pub const MAX_MULTIPART_PARTS: u32 = 65_536;
/// Maximum number of physical entries in one image.
pub const MAX_STORAGE_ENTRIES: usize = 262_144;
/// Default aggregate budget for heap-retained entry buffers in one image.
///
/// The 512 MiB budget counts logical names and keys, compression and origin
/// strings, opaque attributes and headers, and both packed and unpacked payload
/// buffers. Per-entry limits remain independently enforced.
pub const MAX_STORAGE_IMAGE_RETAINED_BYTES: usize = 536_870_912;
/// Maximum UTF-8 size of a stable storage-patch blocker reason.
pub const MAX_STORAGE_PATCH_REASON_BYTES: usize = 4_096;
/// Maximum number of target entries in one storage patch.
pub const MAX_STORAGE_PATCH_ENTRIES: usize = 262_144;
/// Default aggregate budget for heap-retained buffers in one storage patch.
///
/// The 512 MiB budget counts target keys and provenance, compiled payloads,
/// required base keys, and blocker reasons.
pub const MAX_STORAGE_PATCH_RETAINED_BYTES: usize = 536_870_912;

/// Error returned while constructing or deserializing neutral storage data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StorageBuildError {
    /// A required text value is empty.
    EmptyText { field: &'static str },
    /// A text value exceeds its public UTF-8 bound.
    TextTooLong {
        field: &'static str,
        maximum: usize,
        actual: usize,
    },
    /// A text value contains a control character.
    ControlCharacter { field: &'static str },
    /// An open compression identifier does not use the stable grammar.
    InvalidCompressionKind,
    /// An opaque metadata field exceeds its public byte bound.
    OpaqueMetadataTooLarge {
        field: &'static str,
        maximum: usize,
        actual: usize,
    },
    /// A payload exceeds its public byte bound.
    PayloadTooLarge { maximum: usize, actual: usize },
    /// Serialized payload length metadata does not match the retained bytes.
    PayloadSizeMismatch { declared: u64, actual: u64 },
    /// Serialized payload digest metadata does not match the retained bytes.
    PayloadDigestMismatch {
        declared: Sha256Digest,
        actual: Sha256Digest,
    },
    /// A serialized SHA-256 digest is not canonical lowercase hexadecimal.
    InvalidSha256,
    /// Multipart count is zero or exceeds the supported bound.
    InvalidPartCount { count: u32 },
    /// Multipart part index is outside its declared count.
    PartIndexOutOfRange { index: u32, count: u32 },
    /// `stored` compression was paired with different packed/unpacked bytes.
    StoredPayloadMismatch,
    /// The image contains too many physical entries.
    TooManyEntries { maximum: usize, actual: usize },
    /// Summing heap-retained entry buffers overflowed the platform `usize`.
    RetainedByteCountOverflow,
    /// Aggregate heap-retained entry buffers exceed the image budget.
    ImageRetainedBytesExceeded { maximum: usize, actual: usize },
    /// The same logical key and part index occur more than once.
    DuplicateEntryPart { key: String, part_index: u32 },
    /// Parts of one logical key are not encountered in strict zero-based order.
    UnexpectedMultipartOrder {
        key: String,
        expected: u32,
        actual: u32,
    },
    /// Parts of one logical key disagree about their logical name.
    ConflictingLogicalName { key: String },
    /// Parts of one logical key disagree about their declared part count.
    ConflictingPartCount {
        key: String,
        expected: u32,
        actual: u32,
    },
    /// Parts of one logical key disagree about source origin.
    ConflictingMultipartOrigin { key: String },
    /// A multipart logical entry is missing one or more declared parts.
    IncompleteMultipart {
        key: String,
        expected: u32,
        actual: usize,
    },
    /// Entries from different source storage profiles were mixed in one image.
    MixedSourceProfiles { expected: String, actual: String },
}

impl Display for StorageBuildError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyText { field } => write!(formatter, "{field} is empty"),
            Self::TextTooLong {
                field,
                maximum,
                actual,
            } => write!(
                formatter,
                "{field} is {actual} bytes, exceeding the {maximum}-byte bound"
            ),
            Self::ControlCharacter { field } => {
                write!(formatter, "{field} contains a control character")
            }
            Self::InvalidCompressionKind => formatter.write_str(
                "compression kind must be a bounded ASCII identifier starting and ending with an alphanumeric character",
            ),
            Self::OpaqueMetadataTooLarge {
                field,
                maximum,
                actual,
            } => write!(
                formatter,
                "{field} is {actual} bytes, exceeding the {maximum}-byte bound"
            ),
            Self::PayloadTooLarge { maximum, actual } => write!(
                formatter,
                "storage payload is {actual} bytes, exceeding the {maximum}-byte bound"
            ),
            Self::PayloadSizeMismatch { declared, actual } => write!(
                formatter,
                "serialized payload size {declared} does not match {actual} retained bytes"
            ),
            Self::PayloadDigestMismatch { declared, actual } => write!(
                formatter,
                "serialized payload digest {declared} does not match computed digest {actual}"
            ),
            Self::InvalidSha256 => {
                formatter.write_str("SHA-256 digest must contain exactly 64 lowercase hex digits")
            }
            Self::InvalidPartCount { count } => write!(
                formatter,
                "multipart count {count} is outside 1..={MAX_MULTIPART_PARTS}"
            ),
            Self::PartIndexOutOfRange { index, count } => write!(
                formatter,
                "multipart index {index} is outside declared part count {count}"
            ),
            Self::StoredPayloadMismatch => formatter.write_str(
                "stored compression requires byte-identical packed and unpacked payloads",
            ),
            Self::TooManyEntries { maximum, actual } => write!(
                formatter,
                "storage image contains {actual} entries, exceeding the {maximum}-entry bound"
            ),
            Self::RetainedByteCountOverflow => {
                formatter.write_str("storage image retained-byte count overflow")
            }
            Self::ImageRetainedBytesExceeded { maximum, actual } => write!(
                formatter,
                "storage image retains {actual} bytes, exceeding the {maximum}-byte aggregate budget"
            ),
            Self::DuplicateEntryPart { key, part_index } => write!(
                formatter,
                "duplicate storage entry identity `{key}` part {part_index}"
            ),
            Self::UnexpectedMultipartOrder {
                key,
                expected,
                actual,
            } => write!(
                formatter,
                "multipart storage entry `{key}` expected part {expected}, encountered {actual}"
            ),
            Self::ConflictingLogicalName { key } => write!(
                formatter,
                "multipart storage entry `{key}` has conflicting logical names"
            ),
            Self::ConflictingPartCount {
                key,
                expected,
                actual,
            } => write!(
                formatter,
                "multipart storage entry `{key}` declares both {expected} and {actual} parts"
            ),
            Self::ConflictingMultipartOrigin { key } => write!(
                formatter,
                "multipart storage entry `{key}` has conflicting source origin"
            ),
            Self::IncompleteMultipart {
                key,
                expected,
                actual,
            } => write!(
                formatter,
                "multipart storage entry `{key}` contains {actual} of {expected} declared parts"
            ),
            Self::MixedSourceProfiles { expected, actual } => write!(
                formatter,
                "storage image mixes source profiles `{expected}` and `{actual}`"
            ),
        }
    }
}

impl Error for StorageBuildError {}

fn validate_text(
    value: &str,
    field: &'static str,
    maximum: usize,
) -> Result<(), StorageBuildError> {
    if value.is_empty() {
        return Err(StorageBuildError::EmptyText { field });
    }
    if value.len() > maximum {
        return Err(StorageBuildError::TextTooLong {
            field,
            maximum,
            actual: value.len(),
        });
    }
    if value.chars().any(char::is_control) {
        return Err(StorageBuildError::ControlCharacter { field });
    }
    Ok(())
}

macro_rules! bounded_storage_text {
    ($name:ident, $maximum:expr, $field:literal, $documentation:literal) => {
        #[doc = $documentation]
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(Box<str>);

        impl $name {
            /// Validates borrowed text before copying it into the model.
            pub fn new(value: &str) -> Result<Self, StorageBuildError> {
                validate_text(value, $field, $maximum)?;
                Ok(Self(value.into()))
            }

            /// Returns the exact validated text.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl Display for $name {
            fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
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
                deserializer.deserialize_str(StorageTextVisitor::<Self>(PhantomData))
            }
        }
    };
}

bounded_storage_text!(
    StorageName,
    MAX_STORAGE_NAME_BYTES,
    "storage logical name",
    "A bounded logical display name for a physical storage entry."
);
bounded_storage_text!(
    StorageKey,
    MAX_STORAGE_KEY_BYTES,
    "storage logical key",
    "A bounded logical identity key shared by all parts of an entry."
);
bounded_storage_text!(
    StorageProvenance,
    MAX_STORAGE_PROVENANCE_BYTES,
    "storage provenance",
    "Bounded adapter- or fixture-provided provenance for retained storage bytes."
);
bounded_storage_text!(
    StoragePatchReason,
    MAX_STORAGE_PATCH_REASON_BYTES,
    "storage patch reason",
    "A bounded, non-empty stable explanation for a blocked storage-patch entry."
);

struct StorageTextVisitor<T>(PhantomData<fn() -> T>);

trait ParseStorageText: Sized {
    fn parse_storage_text(value: &str) -> Result<Self, StorageBuildError>;
}

impl ParseStorageText for StorageName {
    fn parse_storage_text(value: &str) -> Result<Self, StorageBuildError> {
        Self::new(value)
    }
}

impl ParseStorageText for StorageKey {
    fn parse_storage_text(value: &str) -> Result<Self, StorageBuildError> {
        Self::new(value)
    }
}

impl ParseStorageText for StorageProvenance {
    fn parse_storage_text(value: &str) -> Result<Self, StorageBuildError> {
        Self::new(value)
    }
}

impl ParseStorageText for StoragePatchReason {
    fn parse_storage_text(value: &str) -> Result<Self, StorageBuildError> {
        Self::new(value)
    }
}

impl<'de, T> Visitor<'de> for StorageTextVisitor<T>
where
    T: ParseStorageText,
{
    type Value = T;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("a bounded non-empty storage string without control characters")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        T::parse_storage_text(value).map_err(E::custom)
    }
}

/// Open identifier describing the payload compression representation.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompressionKind(Box<str>);

impl CompressionKind {
    /// Validates an open compression identifier without inferring a codec.
    pub fn new(value: &str) -> Result<Self, StorageBuildError> {
        validate_text(value, "compression kind", MAX_COMPRESSION_KIND_BYTES)?;
        let bytes = value.as_bytes();
        if !bytes[0].is_ascii_alphanumeric()
            || !bytes[bytes.len() - 1].is_ascii_alphanumeric()
            || !bytes.iter().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b':')
            })
        {
            return Err(StorageBuildError::InvalidCompressionKind);
        }
        Ok(Self(value.into()))
    }

    /// Returns the canonical `stored` representation.
    pub fn stored() -> Self {
        Self("stored".into())
    }

    /// Returns the canonical `raw-deflate` representation.
    pub fn raw_deflate() -> Self {
        Self("raw-deflate".into())
    }

    /// Returns the exact open identifier.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns whether packed bytes are declared to be stored verbatim.
    pub fn is_stored(&self) -> bool {
        self.as_str() == "stored"
    }
}

impl Display for CompressionKind {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Serialize for CompressionKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl ParseStorageText for CompressionKind {
    fn parse_storage_text(value: &str) -> Result<Self, StorageBuildError> {
        Self::new(value)
    }
}

impl<'de> Deserialize<'de> for CompressionKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(StorageTextVisitor::<Self>(PhantomData))
    }
}

/// Zero-based part identity for one physical part of a logical entry.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct MultipartIdentity {
    part_index: u32,
    part_count: u32,
}

impl MultipartIdentity {
    /// Creates and validates a multipart identity.
    pub fn new(part_index: u32, part_count: u32) -> Result<Self, StorageBuildError> {
        if part_count == 0 || part_count > MAX_MULTIPART_PARTS {
            return Err(StorageBuildError::InvalidPartCount { count: part_count });
        }
        if part_index >= part_count {
            return Err(StorageBuildError::PartIndexOutOfRange {
                index: part_index,
                count: part_count,
            });
        }
        Ok(Self {
            part_index,
            part_count,
        })
    }

    /// Returns the identity of a non-multipart entry.
    pub const fn single() -> Self {
        Self {
            part_index: 0,
            part_count: 1,
        }
    }

    /// Returns the zero-based part index.
    pub const fn part_index(self) -> u32 {
        self.part_index
    }

    /// Returns the declared number of parts.
    pub const fn part_count(self) -> u32 {
        self.part_count
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMultipartIdentity {
    part_index: u32,
    part_count: u32,
}

impl<'de> Deserialize<'de> for MultipartIdentity {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawMultipartIdentity::deserialize(deserializer)?;
        Self::new(raw.part_index, raw.part_count).map_err(de::Error::custom)
    }
}

/// A canonical SHA-256 digest serialized as lowercase hexadecimal.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Sha256Digest([u8; 32]);

impl Sha256Digest {
    /// Computes the stable digest of exact bytes.
    pub fn for_bytes(bytes: &[u8]) -> Self {
        let digest = Sha256::digest(bytes);
        let mut value = [0_u8; 32];
        value.copy_from_slice(&digest);
        Self(value)
    }

    /// Parses exactly 64 lowercase hexadecimal digits.
    pub fn parse(value: &str) -> Result<Self, StorageBuildError> {
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(StorageBuildError::InvalidSha256);
        }
        let mut bytes = [0_u8; 32];
        for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
            bytes[index] = (hex_nibble(pair[0]) << 4) | hex_nibble(pair[1]);
        }
        Ok(Self(bytes))
    }

    /// Returns the raw 32 digest bytes.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

fn hex_nibble(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        _ => unreachable!("digest input was validated"),
    }
}

impl Display for Sha256Digest {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl Serialize for Sha256Digest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

struct Sha256Visitor;

impl<'de> Visitor<'de> for Sha256Visitor {
    type Value = Sha256Digest;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("exactly 64 lowercase hexadecimal SHA-256 digits")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Sha256Digest::parse(value).map_err(E::custom)
    }
}

impl<'de> Deserialize<'de> for Sha256Digest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(Sha256Visitor)
    }
}

#[derive(Debug)]
struct BoundedBytes<const MAXIMUM: usize>(Box<[u8]>);

impl<const MAXIMUM: usize> BoundedBytes<MAXIMUM> {
    fn into_boxed_slice(self) -> Box<[u8]> {
        self.0
    }
}

struct BoundedBytesVisitor<const MAXIMUM: usize>;

impl<'de, const MAXIMUM: usize> Visitor<'de> for BoundedBytesVisitor<MAXIMUM> {
    type Value = BoundedBytes<MAXIMUM>;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "a byte sequence containing at most {MAXIMUM} bytes"
        )
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut bytes = Vec::with_capacity(sequence.size_hint().unwrap_or_default().min(MAXIMUM));
        while bytes.len() < MAXIMUM {
            let Some(byte) = sequence.next_element::<u8>()? else {
                return Ok(BoundedBytes(bytes.into_boxed_slice()));
            };
            bytes.push(byte);
        }
        if sequence.next_element::<IgnoredAny>()?.is_some() {
            return Err(de::Error::custom(format_args!(
                "byte sequence exceeds {MAXIMUM} bytes"
            )));
        }
        Ok(BoundedBytes(bytes.into_boxed_slice()))
    }

    fn visit_bytes<E>(self, value: &[u8]) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if value.len() > MAXIMUM {
            return Err(E::custom(format_args!(
                "byte sequence exceeds {MAXIMUM} bytes"
            )));
        }
        Ok(BoundedBytes(value.into()))
    }

    fn visit_byte_buf<E>(self, value: Vec<u8>) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if value.len() > MAXIMUM {
            return Err(E::custom(format_args!(
                "byte sequence exceeds {MAXIMUM} bytes"
            )));
        }
        Ok(BoundedBytes(value.into_boxed_slice()))
    }
}

impl<'de, const MAXIMUM: usize> Deserialize<'de> for BoundedBytes<MAXIMUM> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_bytes(BoundedBytesVisitor::<MAXIMUM>)
    }
}

/// Opaque adapter bytes retained without assigning storage-specific meaning.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct OpaqueStorageMetadata {
    attributes: Box<[u8]>,
    raw_header: Box<[u8]>,
}

impl OpaqueStorageMetadata {
    /// Retains bounded opaque attributes and raw-header bytes.
    pub fn new(attributes: Vec<u8>, raw_header: Vec<u8>) -> Result<Self, StorageBuildError> {
        validate_opaque_bytes("storage attributes", &attributes)?;
        validate_opaque_bytes("raw storage header", &raw_header)?;
        Ok(Self {
            attributes: attributes.into_boxed_slice(),
            raw_header: raw_header.into_boxed_slice(),
        })
    }

    /// Returns empty opaque metadata when an adapter has no such projection.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Returns exact opaque storage attributes.
    pub fn attributes(&self) -> &[u8] {
        &self.attributes
    }

    /// Returns the exact raw element header.
    pub fn raw_header(&self) -> &[u8] {
        &self.raw_header
    }
}

fn validate_opaque_bytes(field: &'static str, bytes: &[u8]) -> Result<(), StorageBuildError> {
    if bytes.len() > MAX_OPAQUE_STORAGE_METADATA_BYTES {
        return Err(StorageBuildError::OpaqueMetadataTooLarge {
            field,
            maximum: MAX_OPAQUE_STORAGE_METADATA_BYTES,
            actual: bytes.len(),
        });
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawOpaqueStorageMetadata {
    attributes: BoundedBytes<MAX_OPAQUE_STORAGE_METADATA_BYTES>,
    raw_header: BoundedBytes<MAX_OPAQUE_STORAGE_METADATA_BYTES>,
}

impl<'de> Deserialize<'de> for OpaqueStorageMetadata {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawOpaqueStorageMetadata::deserialize(deserializer)?;
        Self::new(
            raw.attributes.into_boxed_slice().into_vec(),
            raw.raw_header.into_boxed_slice().into_vec(),
        )
        .map_err(de::Error::custom)
    }
}

/// Exact payload bytes plus redundant, validated stable metadata.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StoragePayload {
    byte_len: u64,
    sha256: Sha256Digest,
    bytes: Box<[u8]>,
}

impl StoragePayload {
    /// Retains bounded payload bytes and computes stable metadata.
    pub fn new(bytes: Vec<u8>) -> Result<Self, StorageBuildError> {
        if bytes.len() > MAX_STORAGE_PAYLOAD_BYTES {
            return Err(StorageBuildError::PayloadTooLarge {
                maximum: MAX_STORAGE_PAYLOAD_BYTES,
                actual: bytes.len(),
            });
        }
        let byte_len =
            u64::try_from(bytes.len()).map_err(|_| StorageBuildError::PayloadTooLarge {
                maximum: MAX_STORAGE_PAYLOAD_BYTES,
                actual: bytes.len(),
            })?;
        let sha256 = Sha256Digest::for_bytes(&bytes);
        Ok(Self {
            byte_len,
            sha256,
            bytes: bytes.into_boxed_slice(),
        })
    }

    fn from_serialized(
        byte_len: u64,
        sha256: Sha256Digest,
        bytes: Box<[u8]>,
    ) -> Result<Self, StorageBuildError> {
        let actual_len =
            u64::try_from(bytes.len()).map_err(|_| StorageBuildError::PayloadTooLarge {
                maximum: MAX_STORAGE_PAYLOAD_BYTES,
                actual: bytes.len(),
            })?;
        if byte_len != actual_len {
            return Err(StorageBuildError::PayloadSizeMismatch {
                declared: byte_len,
                actual: actual_len,
            });
        }
        let actual_digest = Sha256Digest::for_bytes(&bytes);
        if sha256 != actual_digest {
            return Err(StorageBuildError::PayloadDigestMismatch {
                declared: sha256,
                actual: actual_digest,
            });
        }
        Ok(Self {
            byte_len,
            sha256,
            bytes,
        })
    }

    /// Returns the exact payload bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Returns the payload length in bytes.
    pub const fn byte_len(&self) -> u64 {
        self.byte_len
    }

    /// Returns the stable SHA-256 digest.
    pub const fn sha256(&self) -> Sha256Digest {
        self.sha256
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStoragePayload {
    byte_len: u64,
    sha256: Sha256Digest,
    bytes: BoundedBytes<MAX_STORAGE_PAYLOAD_BYTES>,
}

impl<'de> Deserialize<'de> for StoragePayload {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawStoragePayload::deserialize(deserializer)?;
        Self::from_serialized(raw.byte_len, raw.sha256, raw.bytes.into_boxed_slice())
            .map_err(de::Error::custom)
    }
}

/// Packed and unpacked forms retained together for lossless adapters.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoragePayloads {
    packed: StoragePayload,
    unpacked: StoragePayload,
}

impl StoragePayloads {
    /// Creates independently bounded packed and unpacked payloads.
    pub fn new(packed: Vec<u8>, unpacked: Vec<u8>) -> Result<Self, StorageBuildError> {
        Ok(Self {
            packed: StoragePayload::new(packed)?,
            unpacked: StoragePayload::new(unpacked)?,
        })
    }

    /// Returns the retained packed payload.
    pub const fn packed(&self) -> &StoragePayload {
        &self.packed
    }

    /// Returns the retained unpacked payload.
    pub const fn unpacked(&self) -> &StoragePayload {
        &self.unpacked
    }
}

/// Source storage profile and exact provenance attached to an entry.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageOrigin {
    source_profile: StorageProfileId,
    provenance: StorageProvenance,
}

impl StorageOrigin {
    /// Creates a source origin from already validated components.
    pub fn new(source_profile: StorageProfileId, provenance: StorageProvenance) -> Self {
        Self {
            source_profile,
            provenance,
        }
    }

    /// Returns the exact source storage profile.
    pub const fn source_profile(&self) -> &StorageProfileId {
        &self.source_profile
    }

    /// Returns the exact bounded provenance.
    pub const fn provenance(&self) -> &StorageProvenance {
        &self.provenance
    }
}

/// One physical part of a neutral logical storage entry.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StorageEntry {
    logical_name: StorageName,
    logical_key: StorageKey,
    multipart: MultipartIdentity,
    opaque_metadata: OpaqueStorageMetadata,
    payloads: StoragePayloads,
    compression: CompressionKind,
    origin: StorageOrigin,
}

impl StorageEntry {
    /// Creates an entry after enforcing cross-field payload invariants.
    pub fn new(
        logical_name: StorageName,
        logical_key: StorageKey,
        multipart: MultipartIdentity,
        opaque_metadata: OpaqueStorageMetadata,
        payloads: StoragePayloads,
        compression: CompressionKind,
        origin: StorageOrigin,
    ) -> Result<Self, StorageBuildError> {
        if compression.is_stored() && payloads.packed.bytes != payloads.unpacked.bytes {
            return Err(StorageBuildError::StoredPayloadMismatch);
        }
        Ok(Self {
            logical_name,
            logical_key,
            multipart,
            opaque_metadata,
            payloads,
            compression,
            origin,
        })
    }

    /// Returns the logical display name.
    pub const fn logical_name(&self) -> &StorageName {
        &self.logical_name
    }

    /// Returns the logical identity key.
    pub const fn logical_key(&self) -> &StorageKey {
        &self.logical_key
    }

    /// Returns the physical multipart identity.
    pub const fn multipart(&self) -> MultipartIdentity {
        self.multipart
    }

    /// Returns opaque storage attributes and raw-header bytes.
    pub const fn opaque_metadata(&self) -> &OpaqueStorageMetadata {
        &self.opaque_metadata
    }

    /// Returns the exact opaque storage attributes.
    pub fn attributes(&self) -> &[u8] {
        self.opaque_metadata.attributes()
    }

    /// Returns the exact raw element header.
    pub fn raw_header(&self) -> &[u8] {
        self.opaque_metadata.raw_header()
    }

    /// Returns packed and unpacked payload forms.
    pub const fn payloads(&self) -> &StoragePayloads {
        &self.payloads
    }

    /// Returns the exact packed payload bytes.
    pub fn packed_payload(&self) -> &[u8] {
        self.payloads.packed().bytes()
    }

    /// Returns the exact unpacked payload bytes.
    pub fn unpacked_payload(&self) -> &[u8] {
        self.payloads.unpacked().bytes()
    }

    /// Returns the open compression identifier.
    pub const fn compression(&self) -> &CompressionKind {
        &self.compression
    }

    /// Returns source profile and provenance.
    pub const fn origin(&self) -> &StorageOrigin {
        &self.origin
    }

    /// Returns the source storage profile directly.
    pub const fn source_profile(&self) -> &StorageProfileId {
        self.origin.source_profile()
    }

    /// Returns exact entry provenance directly.
    pub const fn provenance(&self) -> &StorageProvenance {
        self.origin.provenance()
    }

    /// Counts every heap-retained buffer governed by the image byte budget.
    pub fn retained_byte_len(&self) -> Result<usize, StorageBuildError> {
        [
            self.logical_name.as_str().len(),
            self.logical_key.as_str().len(),
            self.compression.as_str().len(),
            self.origin.source_profile.as_str().len(),
            self.origin.provenance.as_str().len(),
            self.opaque_metadata.attributes.len(),
            self.opaque_metadata.raw_header.len(),
            self.payloads.packed.bytes.len(),
            self.payloads.unpacked.bytes.len(),
        ]
        .into_iter()
        .try_fold(0_usize, |total, length| {
            total
                .checked_add(length)
                .ok_or(StorageBuildError::RetainedByteCountOverflow)
        })
    }

    /// Computes a platform-independent digest of every entry field.
    pub fn sha256(&self) -> Sha256Digest {
        let mut hasher = Sha256::new();
        hasher.update(b"ibcmd-storage-entry-v1\0");
        hash_length_prefixed(&mut hasher, self.logical_name.as_str().as_bytes());
        hash_length_prefixed(&mut hasher, self.logical_key.as_str().as_bytes());
        hasher.update(self.multipart.part_index.to_le_bytes());
        hasher.update(self.multipart.part_count.to_le_bytes());
        hash_length_prefixed(&mut hasher, self.opaque_metadata.attributes());
        hash_length_prefixed(&mut hasher, self.opaque_metadata.raw_header());
        hash_length_prefixed(&mut hasher, self.payloads.packed.bytes());
        hash_length_prefixed(&mut hasher, self.payloads.unpacked.bytes());
        hash_length_prefixed(&mut hasher, self.compression.as_str().as_bytes());
        hash_length_prefixed(&mut hasher, self.origin.source_profile.as_str().as_bytes());
        hash_length_prefixed(&mut hasher, self.origin.provenance.as_str().as_bytes());
        digest_from_hasher(hasher)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStorageEntry {
    logical_name: StorageName,
    logical_key: StorageKey,
    multipart: MultipartIdentity,
    opaque_metadata: OpaqueStorageMetadata,
    payloads: StoragePayloads,
    compression: CompressionKind,
    origin: StorageOrigin,
}

impl<'de> Deserialize<'de> for StorageEntry {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawStorageEntry::deserialize(deserializer)?;
        Self::new(
            raw.logical_name,
            raw.logical_key,
            raw.multipart,
            raw.opaque_metadata,
            raw.payloads,
            raw.compression,
            raw.origin,
        )
        .map_err(de::Error::custom)
    }
}

fn hash_length_prefixed(hasher: &mut Sha256, bytes: &[u8]) {
    let length = u64::try_from(bytes.len()).expect("slice length fits into u64");
    hasher.update(length.to_le_bytes());
    hasher.update(bytes);
}

fn digest_from_hasher(hasher: Sha256) -> Sha256Digest {
    let digest = hasher.finalize();
    let mut value = [0_u8; 32];
    value.copy_from_slice(&digest);
    Sha256Digest(value)
}

fn checked_image_retained_bytes(
    current: usize,
    entry: &StorageEntry,
    maximum: usize,
) -> Result<usize, StorageBuildError> {
    let actual = current
        .checked_add(entry.retained_byte_len()?)
        .ok_or(StorageBuildError::RetainedByteCountOverflow)?;
    if actual > maximum {
        return Err(StorageBuildError::ImageRetainedBytesExceeded { maximum, actual });
    }
    Ok(actual)
}

#[derive(Debug)]
struct BoundedStorageEntries<const MAXIMUM_ENTRIES: usize, const MAXIMUM_RETAINED_BYTES: usize>(
    Vec<StorageEntry>,
);

struct BoundedStorageEntriesVisitor<
    const MAXIMUM_ENTRIES: usize,
    const MAXIMUM_RETAINED_BYTES: usize,
>;

impl<'de, const MAXIMUM_ENTRIES: usize, const MAXIMUM_RETAINED_BYTES: usize> Visitor<'de>
    for BoundedStorageEntriesVisitor<MAXIMUM_ENTRIES, MAXIMUM_RETAINED_BYTES>
{
    type Value = BoundedStorageEntries<MAXIMUM_ENTRIES, MAXIMUM_RETAINED_BYTES>;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "an ordered storage image containing at most {MAXIMUM_ENTRIES} entries and retaining at most {MAXIMUM_RETAINED_BYTES} bytes"
        )
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut entries = Vec::with_capacity(
            sequence
                .size_hint()
                .unwrap_or_default()
                .min(MAXIMUM_ENTRIES),
        );
        let mut retained_bytes = 0_usize;
        while entries.len() < MAXIMUM_ENTRIES {
            let Some(entry) = sequence.next_element::<StorageEntry>()? else {
                return Ok(BoundedStorageEntries(entries));
            };
            retained_bytes =
                checked_image_retained_bytes(retained_bytes, &entry, MAXIMUM_RETAINED_BYTES)
                    .map_err(de::Error::custom)?;
            entries.push(entry);
        }
        if sequence.next_element::<IgnoredAny>()?.is_some() {
            return Err(de::Error::custom(format_args!(
                "storage image exceeds {MAXIMUM_ENTRIES} entries"
            )));
        }
        Ok(BoundedStorageEntries(entries))
    }
}

impl<'de, const MAXIMUM_ENTRIES: usize, const MAXIMUM_RETAINED_BYTES: usize> Deserialize<'de>
    for BoundedStorageEntries<MAXIMUM_ENTRIES, MAXIMUM_RETAINED_BYTES>
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(
            BoundedStorageEntriesVisitor::<MAXIMUM_ENTRIES, MAXIMUM_RETAINED_BYTES>,
        )
    }
}

/// Ordered neutral storage snapshot.
///
/// [`StorageImage::new`] never sorts its input. Callers choose and can recover
/// exact source order through [`StorageImage::entries`].
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct StorageImage {
    entries: Vec<StorageEntry>,
}

impl StorageImage {
    /// Validates an image while preserving the exact supplied entry order.
    pub fn new(entries: Vec<StorageEntry>) -> Result<Self, StorageBuildError> {
        validate_image(&entries)?;
        Ok(Self { entries })
    }

    /// Returns physical entries in their explicitly supplied source order.
    pub fn entries(&self) -> &[StorageEntry] {
        &self.entries
    }

    /// Returns the number of physical entries.
    pub const fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns whether the image contains no physical entries.
    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Consumes the image without changing source order.
    pub fn into_entries(self) -> Vec<StorageEntry> {
        self.entries
    }

    /// Returns the common source profile, if the image is non-empty.
    pub fn source_profile(&self) -> Option<&StorageProfileId> {
        self.entries
            .first()
            .map(|entry| entry.origin.source_profile())
    }

    /// Computes an order-sensitive platform-independent image digest.
    pub fn sha256(&self) -> Sha256Digest {
        let mut hasher = Sha256::new();
        hasher.update(b"ibcmd-storage-image-v1\0");
        let count = u64::try_from(self.entries.len()).expect("entry count fits into u64");
        hasher.update(count.to_le_bytes());
        for entry in &self.entries {
            hasher.update(entry.sha256().as_bytes());
        }
        digest_from_hasher(hasher)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStorageImage {
    entries: BoundedStorageEntries<MAX_STORAGE_ENTRIES, MAX_STORAGE_IMAGE_RETAINED_BYTES>,
}

impl<'de> Deserialize<'de> for StorageImage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawStorageImage::deserialize(deserializer)?;
        Self::new(raw.entries.0).map_err(de::Error::custom)
    }
}

struct MultipartState<'a> {
    logical_name: &'a StorageName,
    part_count: u32,
    origin: &'a StorageOrigin,
    seen_parts: BTreeSet<u32>,
    next_part_index: u32,
}

fn validate_image(entries: &[StorageEntry]) -> Result<(), StorageBuildError> {
    validate_image_with_retained_byte_limit(entries, MAX_STORAGE_IMAGE_RETAINED_BYTES)
}

fn validate_image_with_retained_byte_limit(
    entries: &[StorageEntry],
    maximum_retained_bytes: usize,
) -> Result<(), StorageBuildError> {
    if entries.len() > MAX_STORAGE_ENTRIES {
        return Err(StorageBuildError::TooManyEntries {
            maximum: MAX_STORAGE_ENTRIES,
            actual: entries.len(),
        });
    }

    let mut source_profile = None::<&StorageProfileId>;
    let mut groups = BTreeMap::<&StorageKey, MultipartState<'_>>::new();
    let mut retained_bytes = 0_usize;
    for entry in entries {
        retained_bytes =
            checked_image_retained_bytes(retained_bytes, entry, maximum_retained_bytes)?;
        match source_profile {
            Some(expected) if expected != entry.origin.source_profile() => {
                return Err(StorageBuildError::MixedSourceProfiles {
                    expected: expected.as_str().to_owned(),
                    actual: entry.origin.source_profile().as_str().to_owned(),
                });
            }
            None => source_profile = Some(entry.origin.source_profile()),
            _ => {}
        }

        match groups.entry(&entry.logical_key) {
            Entry::Vacant(slot) => {
                if entry.multipart.part_index != 0 {
                    return Err(StorageBuildError::UnexpectedMultipartOrder {
                        key: entry.logical_key.as_str().to_owned(),
                        expected: 0,
                        actual: entry.multipart.part_index,
                    });
                }
                slot.insert(MultipartState {
                    logical_name: &entry.logical_name,
                    part_count: entry.multipart.part_count,
                    origin: &entry.origin,
                    seen_parts: BTreeSet::from([entry.multipart.part_index]),
                    next_part_index: 1,
                });
            }
            Entry::Occupied(mut slot) => {
                let state = slot.get_mut();
                if state.seen_parts.contains(&entry.multipart.part_index) {
                    return Err(StorageBuildError::DuplicateEntryPart {
                        key: entry.logical_key.as_str().to_owned(),
                        part_index: entry.multipart.part_index,
                    });
                }
                if state.logical_name != &entry.logical_name {
                    return Err(StorageBuildError::ConflictingLogicalName {
                        key: entry.logical_key.as_str().to_owned(),
                    });
                }
                if state.part_count != entry.multipart.part_count {
                    return Err(StorageBuildError::ConflictingPartCount {
                        key: entry.logical_key.as_str().to_owned(),
                        expected: state.part_count,
                        actual: entry.multipart.part_count,
                    });
                }
                if state.origin != &entry.origin {
                    return Err(StorageBuildError::ConflictingMultipartOrigin {
                        key: entry.logical_key.as_str().to_owned(),
                    });
                }
                if entry.multipart.part_index != state.next_part_index {
                    return Err(StorageBuildError::UnexpectedMultipartOrder {
                        key: entry.logical_key.as_str().to_owned(),
                        expected: state.next_part_index,
                        actual: entry.multipart.part_index,
                    });
                }
                state.seen_parts.insert(entry.multipart.part_index);
                state.next_part_index += 1;
            }
        }
    }

    for (key, state) in groups {
        if state.seen_parts.len() != state.part_count as usize {
            return Err(StorageBuildError::IncompleteMultipart {
                key: key.as_str().to_owned(),
                expected: state.part_count,
                actual: state.seen_parts.len(),
            });
        }
    }
    Ok(())
}

/// Error returned while constructing a bounded storage patch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StoragePatchBuildError {
    /// A patch component failed its own storage-model validation.
    Storage(StorageBuildError),
    /// The patch contains too many target entries.
    TooManyEntries { maximum: usize, actual: usize },
    /// Summing heap-retained patch buffers overflowed the platform `usize`.
    RetainedByteCountOverflow,
    /// Aggregate heap-retained patch buffers exceed the patch budget.
    RetainedBytesExceeded { maximum: usize, actual: usize },
    /// The same target key and part index occur more than once.
    DuplicateTarget { key: String, part_index: u32 },
}

impl Display for StoragePatchBuildError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(source) => write!(formatter, "invalid storage patch component: {source}"),
            Self::TooManyEntries { maximum, actual } => write!(
                formatter,
                "storage patch contains {actual} entries, exceeding the {maximum}-entry bound"
            ),
            Self::RetainedByteCountOverflow => {
                formatter.write_str("storage patch retained-byte count overflow")
            }
            Self::RetainedBytesExceeded { maximum, actual } => write!(
                formatter,
                "storage patch retains {actual} bytes, exceeding the {maximum}-byte aggregate budget"
            ),
            Self::DuplicateTarget { key, part_index } => write!(
                formatter,
                "duplicate storage patch target `{key}` part {part_index}"
            ),
        }
    }
}

impl Error for StoragePatchBuildError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Storage(source) => Some(source),
            _ => None,
        }
    }
}

impl From<StorageBuildError> for StoragePatchBuildError {
    fn from(source: StorageBuildError) -> Self {
        Self::Storage(source)
    }
}

/// Identity and provenance of one storage-patch destination.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StoragePatchTarget {
    key: StorageKey,
    multipart: MultipartIdentity,
    provenance: StorageProvenance,
}

impl StoragePatchTarget {
    /// Creates a target from already validated neutral storage components.
    pub fn new(
        key: StorageKey,
        multipart: MultipartIdentity,
        provenance: StorageProvenance,
    ) -> Self {
        Self {
            key,
            multipart,
            provenance,
        }
    }

    /// Returns the target logical key.
    pub const fn key(&self) -> &StorageKey {
        &self.key
    }

    /// Returns the target physical part identity.
    pub const fn multipart(&self) -> MultipartIdentity {
        self.multipart
    }

    /// Returns the exact compiler-provided target provenance.
    pub const fn provenance(&self) -> &StorageProvenance {
        &self.provenance
    }

    fn retained_byte_len(&self) -> Result<usize, StoragePatchBuildError> {
        self.key
            .as_str()
            .len()
            .checked_add(self.provenance.as_str().len())
            .ok_or(StoragePatchBuildError::RetainedByteCountOverflow)
    }

    fn hash_into(&self, hasher: &mut Sha256) {
        hash_length_prefixed(hasher, self.key.as_str().as_bytes());
        hasher.update(self.multipart.part_index.to_le_bytes());
        hasher.update(self.multipart.part_count.to_le_bytes());
        hash_length_prefixed(hasher, self.provenance.as_str().as_bytes());
    }
}

/// Result of compiling one neutral storage-patch target.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum StoragePatchOutcome {
    /// The target was compiled without consulting a base artifact.
    Compiled(StoragePayload),
    /// The target can only be produced by overlaying a required base entry.
    NeedsBase {
        reason: StoragePatchReason,
        required: StorageKey,
    },
    /// No supported compiler can produce the target.
    Unsupported { reason: StoragePatchReason },
}

impl StoragePatchOutcome {
    /// Retains bounded compiled bytes and computes their digest.
    pub fn compiled(bytes: Vec<u8>) -> Result<Self, StoragePatchBuildError> {
        Ok(Self::Compiled(StoragePayload::new(bytes)?))
    }

    /// Creates a base-dependent outcome with a bounded, non-empty reason.
    pub fn needs_base(required: StorageKey, reason: &str) -> Result<Self, StoragePatchBuildError> {
        Ok(Self::NeedsBase {
            reason: StoragePatchReason::new(reason)?,
            required,
        })
    }

    /// Creates an unsupported outcome with a bounded, non-empty reason.
    pub fn unsupported(reason: &str) -> Result<Self, StoragePatchBuildError> {
        Ok(Self::Unsupported {
            reason: StoragePatchReason::new(reason)?,
        })
    }

    /// Returns compiled payload bytes and digest when this target is ready.
    pub const fn compiled_payload(&self) -> Option<&StoragePayload> {
        match self {
            Self::Compiled(payload) => Some(payload),
            Self::NeedsBase { .. } | Self::Unsupported { .. } => None,
        }
    }

    /// Returns the stable blocker reason, if any.
    pub const fn reason(&self) -> Option<&StoragePatchReason> {
        match self {
            Self::Compiled(_) => None,
            Self::NeedsBase { reason, .. } | Self::Unsupported { reason } => Some(reason),
        }
    }

    fn retained_byte_len(&self) -> Result<usize, StoragePatchBuildError> {
        match self {
            Self::Compiled(payload) => Ok(payload.bytes().len()),
            Self::NeedsBase { reason, required } => reason
                .as_str()
                .len()
                .checked_add(required.as_str().len())
                .ok_or(StoragePatchBuildError::RetainedByteCountOverflow),
            Self::Unsupported { reason } => Ok(reason.as_str().len()),
        }
    }

    fn hash_into(&self, hasher: &mut Sha256) {
        match self {
            Self::Compiled(payload) => {
                hasher.update([0]);
                hasher.update(payload.byte_len().to_le_bytes());
                hasher.update(payload.sha256().as_bytes());
                hash_length_prefixed(hasher, payload.bytes());
            }
            Self::NeedsBase { reason, required } => {
                hasher.update([1]);
                hash_length_prefixed(hasher, required.as_str().as_bytes());
                hash_length_prefixed(hasher, reason.as_str().as_bytes());
            }
            Self::Unsupported { reason } => {
                hasher.update([2]);
                hash_length_prefixed(hasher, reason.as_str().as_bytes());
            }
        }
    }
}

/// One ordered target and its pure compilation outcome.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StoragePatchEntry {
    target: StoragePatchTarget,
    outcome: StoragePatchOutcome,
}

impl StoragePatchEntry {
    /// Pairs one validated target with its compilation outcome.
    pub fn new(target: StoragePatchTarget, outcome: StoragePatchOutcome) -> Self {
        Self { target, outcome }
    }

    /// Returns the target identity and provenance.
    pub const fn target(&self) -> &StoragePatchTarget {
        &self.target
    }

    /// Returns the target compilation outcome.
    pub const fn outcome(&self) -> &StoragePatchOutcome {
        &self.outcome
    }

    /// Counts every heap-retained buffer governed by the patch byte budget.
    pub fn retained_byte_len(&self) -> Result<usize, StoragePatchBuildError> {
        self.target
            .retained_byte_len()?
            .checked_add(self.outcome.retained_byte_len()?)
            .ok_or(StoragePatchBuildError::RetainedByteCountOverflow)
    }
}

/// A typed reason why a storage patch cannot be consumed as fully compiled.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum StoragePatchBlocker {
    /// A target requires a specific base entry before it can be compiled.
    NeedsBase {
        target: StoragePatchTarget,
        reason: StoragePatchReason,
        required: StorageKey,
    },
    /// A target is unsupported by the compiler.
    Unsupported {
        target: StoragePatchTarget,
        reason: StoragePatchReason,
    },
}

impl StoragePatchBlocker {
    /// Returns the blocked patch target.
    pub const fn target(&self) -> &StoragePatchTarget {
        match self {
            Self::NeedsBase { target, .. } | Self::Unsupported { target, .. } => target,
        }
    }

    /// Returns the stable blocker reason.
    pub const fn reason(&self) -> &StoragePatchReason {
        match self {
            Self::NeedsBase { reason, .. } | Self::Unsupported { reason, .. } => reason,
        }
    }

    /// Returns the required base key for a base-dependent target.
    pub const fn required(&self) -> Option<&StorageKey> {
        match self {
            Self::NeedsBase { required, .. } => Some(required),
            Self::Unsupported { .. } => None,
        }
    }
}

/// Complete, ordered blocker set returned by storage-patch preflight.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoragePatchPreflightError {
    blockers: Vec<StoragePatchBlocker>,
}

impl StoragePatchPreflightError {
    /// Returns blockers in their original patch order.
    pub fn blockers(&self) -> &[StoragePatchBlocker] {
        &self.blockers
    }

    /// Consumes the error without changing blocker order.
    pub fn into_blockers(self) -> Vec<StoragePatchBlocker> {
        self.blockers
    }
}

impl Display for StoragePatchPreflightError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "storage patch preflight found {} blocking entries",
            self.blockers.len()
        )
    }
}

impl Error for StoragePatchPreflightError {}

/// A target whose compiled payload is proven available after preflight.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CompiledStoragePatchEntry {
    target: StoragePatchTarget,
    payload: StoragePayload,
}

impl CompiledStoragePatchEntry {
    /// Returns the compiled target identity and provenance.
    pub const fn target(&self) -> &StoragePatchTarget {
        &self.target
    }

    /// Returns exact compiled bytes, length, and digest.
    pub const fn payload(&self) -> &StoragePayload {
        &self.payload
    }

    /// Consumes the entry into its validated components.
    pub fn into_parts(self) -> (StoragePatchTarget, StoragePayload) {
        (self.target, self.payload)
    }
}

/// Bounded neutral patch whose exact supplied target order is preserved.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct StoragePatch {
    entries: Vec<StoragePatchEntry>,
}

impl StoragePatch {
    /// Validates a patch without sorting or otherwise changing supplied order.
    pub fn new(entries: Vec<StoragePatchEntry>) -> Result<Self, StoragePatchBuildError> {
        validate_patch_with_limits(
            &entries,
            MAX_STORAGE_PATCH_ENTRIES,
            MAX_STORAGE_PATCH_RETAINED_BYTES,
        )?;
        Ok(Self { entries })
    }

    /// Returns entries in their explicitly supplied compiler order.
    pub fn entries(&self) -> &[StoragePatchEntry] {
        &self.entries
    }

    /// Returns the number of patch targets.
    pub const fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns whether this patch has no targets.
    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Consumes the patch without changing supplied order.
    pub fn into_entries(self) -> Vec<StoragePatchEntry> {
        self.entries
    }

    /// Computes an order-sensitive platform-independent patch digest.
    pub fn sha256(&self) -> Sha256Digest {
        let mut hasher = Sha256::new();
        hasher.update(b"ibcmd-storage-patch-v1\0");
        let count = u64::try_from(self.entries.len()).expect("entry count fits into u64");
        hasher.update(count.to_le_bytes());
        for entry in &self.entries {
            entry.target.hash_into(&mut hasher);
            entry.outcome.hash_into(&mut hasher);
        }
        digest_from_hasher(hasher)
    }

    /// Succeeds only when every patch target has a compiled payload.
    pub fn preflight(&self) -> Result<(), StoragePatchPreflightError> {
        let blockers = collect_patch_blockers(&self.entries);
        if blockers.is_empty() {
            Ok(())
        } else {
            Err(StoragePatchPreflightError { blockers })
        }
    }

    /// Consumes an all-compiled patch into ordered target/payload entries.
    pub fn into_compiled(
        self,
    ) -> Result<Vec<CompiledStoragePatchEntry>, StoragePatchPreflightError> {
        let blockers = collect_patch_blockers(&self.entries);
        if !blockers.is_empty() {
            return Err(StoragePatchPreflightError { blockers });
        }

        Ok(self
            .entries
            .into_iter()
            .map(|entry| {
                let StoragePatchEntry { target, outcome } = entry;
                let StoragePatchOutcome::Compiled(payload) = outcome else {
                    unreachable!("non-compiled outcomes were rejected by preflight")
                };
                CompiledStoragePatchEntry { target, payload }
            })
            .collect())
    }
}

fn collect_patch_blockers(entries: &[StoragePatchEntry]) -> Vec<StoragePatchBlocker> {
    entries
        .iter()
        .filter_map(|entry| match &entry.outcome {
            StoragePatchOutcome::Compiled(_) => None,
            StoragePatchOutcome::NeedsBase { reason, required } => {
                Some(StoragePatchBlocker::NeedsBase {
                    target: entry.target.clone(),
                    reason: reason.clone(),
                    required: required.clone(),
                })
            }
            StoragePatchOutcome::Unsupported { reason } => Some(StoragePatchBlocker::Unsupported {
                target: entry.target.clone(),
                reason: reason.clone(),
            }),
        })
        .collect()
}

fn checked_patch_retained_bytes(
    current: usize,
    entry: &StoragePatchEntry,
    maximum: usize,
) -> Result<usize, StoragePatchBuildError> {
    let actual = current
        .checked_add(entry.retained_byte_len()?)
        .ok_or(StoragePatchBuildError::RetainedByteCountOverflow)?;
    if actual > maximum {
        return Err(StoragePatchBuildError::RetainedBytesExceeded { maximum, actual });
    }
    Ok(actual)
}

fn validate_patch_with_limits(
    entries: &[StoragePatchEntry],
    maximum_entries: usize,
    maximum_retained_bytes: usize,
) -> Result<(), StoragePatchBuildError> {
    if entries.len() > maximum_entries {
        return Err(StoragePatchBuildError::TooManyEntries {
            maximum: maximum_entries,
            actual: entries.len(),
        });
    }

    let mut targets = BTreeSet::<(&StorageKey, u32)>::new();
    let mut retained_bytes = 0_usize;
    for entry in entries {
        let part_index = entry.target.multipart.part_index;
        if !targets.insert((&entry.target.key, part_index)) {
            return Err(StoragePatchBuildError::DuplicateTarget {
                key: entry.target.key.as_str().to_owned(),
                part_index,
            });
        }
        retained_bytes =
            checked_patch_retained_bytes(retained_bytes, entry, maximum_retained_bytes)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn origin() -> StorageOrigin {
        StorageOrigin::new(
            StorageProfileId::parse("storage:test").unwrap(),
            StorageProvenance::new("fixture:storage-unit").unwrap(),
        )
    }

    fn entry(key: &str, part_index: u32, part_count: u32, bytes: &[u8]) -> StorageEntry {
        StorageEntry::new(
            StorageName::new(key).unwrap(),
            StorageKey::new(key).unwrap(),
            MultipartIdentity::new(part_index, part_count).unwrap(),
            OpaqueStorageMetadata::empty(),
            StoragePayloads::new(bytes.to_vec(), bytes.to_vec()).unwrap(),
            CompressionKind::stored(),
            origin(),
        )
        .unwrap()
    }

    fn patch_target(
        key: &str,
        part_index: u32,
        part_count: u32,
        provenance: &str,
    ) -> StoragePatchTarget {
        StoragePatchTarget::new(
            StorageKey::new(key).unwrap(),
            MultipartIdentity::new(part_index, part_count).unwrap(),
            StorageProvenance::new(provenance).unwrap(),
        )
    }

    fn compiled_patch_entry(key: &str, bytes: &[u8]) -> StoragePatchEntry {
        StoragePatchEntry::new(
            patch_target(key, 0, 1, "fixture:patch-unit"),
            StoragePatchOutcome::compiled(bytes.to_vec()).unwrap(),
        )
    }

    #[test]
    fn storage_patch_models_three_bounded_outcomes() {
        let compiled = StoragePatchOutcome::compiled(b"compiled".to_vec()).unwrap();
        let payload = compiled.compiled_payload().unwrap();
        assert_eq!(payload.bytes(), b"compiled");
        assert_eq!(payload.sha256(), Sha256Digest::for_bytes(b"compiled"));
        assert!(compiled.reason().is_none());

        let needs_base = StoragePatchOutcome::needs_base(
            StorageKey::new("base:key").unwrap(),
            "base-entry-required",
        )
        .unwrap();
        assert!(matches!(
            &needs_base,
            StoragePatchOutcome::NeedsBase { reason, required }
                if reason.as_str() == "base-entry-required" && required.as_str() == "base:key"
        ));

        let unsupported = StoragePatchOutcome::unsupported("unsupported-family").unwrap();
        assert!(matches!(
            &unsupported,
            StoragePatchOutcome::Unsupported { reason }
                if reason.as_str() == "unsupported-family"
        ));
        assert!(matches!(
            StoragePatchOutcome::unsupported(""),
            Err(StoragePatchBuildError::Storage(
                StorageBuildError::EmptyText {
                    field: "storage patch reason"
                }
            ))
        ));
        assert!(matches!(
            StoragePatchOutcome::unsupported("unstable\nreason"),
            Err(StoragePatchBuildError::Storage(
                StorageBuildError::ControlCharacter {
                    field: "storage patch reason"
                }
            ))
        ));
    }

    #[test]
    fn storage_patch_preserves_order_and_has_a_deterministic_digest() {
        let patch = StoragePatch::new(vec![
            compiled_patch_entry("b", b"two"),
            compiled_patch_entry("a", b"one"),
        ])
        .unwrap();
        assert_eq!(patch.entries()[0].target().key().as_str(), "b");
        assert_eq!(patch.entries()[1].target().key().as_str(), "a");
        assert_eq!(patch.sha256(), patch.clone().sha256());
        assert_eq!(
            patch.sha256().to_string(),
            "1bcaeb5f65b178791a0b23c1be009bedc15be0969adc0bb8d8fc0341d15548f4"
        );

        let reversed = StoragePatch::new(vec![
            compiled_patch_entry("a", b"one"),
            compiled_patch_entry("b", b"two"),
        ])
        .unwrap();
        assert_ne!(patch.sha256(), reversed.sha256());
    }

    #[test]
    fn storage_patch_rejects_duplicate_target_identity() {
        let duplicate = StoragePatch::new(vec![
            StoragePatchEntry::new(
                patch_target("same", 0, 2, "compiler:first"),
                StoragePatchOutcome::compiled(vec![1]).unwrap(),
            ),
            StoragePatchEntry::new(
                patch_target("same", 0, 3, "compiler:second"),
                StoragePatchOutcome::compiled(vec![2]).unwrap(),
            ),
        ]);
        assert!(matches!(
            duplicate,
            Err(StoragePatchBuildError::DuplicateTarget {
                key,
                part_index: 0
            }) if key == "same"
        ));
    }

    #[test]
    fn storage_patch_enforces_count_and_aggregate_byte_bounds() {
        let first = compiled_patch_entry("a", b"first");
        let second = StoragePatchEntry::new(
            patch_target("b", 0, 1, "fixture:second"),
            StoragePatchOutcome::needs_base(StorageKey::new("base:b").unwrap(), "requires-base")
                .unwrap(),
        );
        assert!(matches!(
            validate_patch_with_limits(&[first.clone(), second.clone()], 1, usize::MAX),
            Err(StoragePatchBuildError::TooManyEntries {
                maximum: 1,
                actual: 2
            })
        ));

        let aggregate = first
            .retained_byte_len()
            .unwrap()
            .checked_add(second.retained_byte_len().unwrap())
            .unwrap();
        assert!(validate_patch_with_limits(&[first.clone(), second.clone()], 2, aggregate).is_ok());
        assert!(matches!(
            validate_patch_with_limits(&[first.clone(), second], 2, aggregate - 1),
            Err(StoragePatchBuildError::RetainedBytesExceeded { .. })
        ));
        assert!(matches!(
            checked_patch_retained_bytes(usize::MAX, &first, usize::MAX),
            Err(StoragePatchBuildError::RetainedByteCountOverflow)
        ));
    }

    #[test]
    fn storage_patch_preflight_is_typed_and_all_compiled_only() {
        let ready = StoragePatch::new(vec![
            compiled_patch_entry("first", b"one"),
            compiled_patch_entry("second", b"two"),
        ])
        .unwrap();
        assert!(ready.preflight().is_ok());
        let compiled = ready.into_compiled().unwrap();
        assert_eq!(compiled[0].target().key().as_str(), "first");
        assert_eq!(compiled[0].payload().bytes(), b"one");
        assert_eq!(compiled[1].target().key().as_str(), "second");
        assert_eq!(compiled[1].payload().bytes(), b"two");

        let blocked = StoragePatch::new(vec![
            compiled_patch_entry("ready", b"ready"),
            StoragePatchEntry::new(
                patch_target("overlay", 0, 1, "compiler:overlay"),
                StoragePatchOutcome::needs_base(
                    StorageKey::new("base:overlay").unwrap(),
                    "overlay-only",
                )
                .unwrap(),
            ),
            StoragePatchEntry::new(
                patch_target("unknown", 0, 1, "compiler:unsupported"),
                StoragePatchOutcome::unsupported("no-compiler").unwrap(),
            ),
        ])
        .unwrap();
        let preflight = blocked.preflight().unwrap_err();
        assert_eq!(preflight.blockers().len(), 2);
        assert!(matches!(
            &preflight.blockers()[0],
            StoragePatchBlocker::NeedsBase {
                target,
                reason,
                required
            } if target.key().as_str() == "overlay"
                && reason.as_str() == "overlay-only"
                && required.as_str() == "base:overlay"
        ));
        assert!(matches!(
            &preflight.blockers()[1],
            StoragePatchBlocker::Unsupported { target, reason }
                if target.key().as_str() == "unknown" && reason.as_str() == "no-compiler"
        ));
        assert_eq!(blocked.into_compiled().unwrap_err(), preflight);
    }

    #[test]
    fn sha256_and_ordered_image_are_stable() {
        assert_eq!(
            Sha256Digest::for_bytes(b"abc").to_string(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );

        let image =
            StorageImage::new(vec![entry("b", 0, 1, b"two"), entry("a", 0, 1, b"one")]).unwrap();
        assert_eq!(image.entries()[0].logical_key().as_str(), "b");
        assert_eq!(image.entries()[1].logical_key().as_str(), "a");

        let reversed =
            StorageImage::new(vec![entry("a", 0, 1, b"one"), entry("b", 0, 1, b"two")]).unwrap();
        assert_ne!(image.sha256(), reversed.sha256());
        assert_eq!(image.sha256(), image.clone().sha256());
    }

    #[test]
    fn serde_round_trip_revalidates_payload_metadata_and_stored_bytes() {
        let image = StorageImage::new(vec![entry("root", 0, 1, b"payload")]).unwrap();
        let json = serde_json::to_string(&image).unwrap();
        assert_eq!(serde_json::from_str::<StorageImage>(&json).unwrap(), image);

        let wrong_size = json.replacen("\"byte_len\":7", "\"byte_len\":8", 1);
        assert!(serde_json::from_str::<StorageImage>(&wrong_size).is_err());

        let digest = Sha256Digest::for_bytes(b"payload").to_string();
        let wrong_digest = json.replacen(&digest, &"0".repeat(64), 1);
        assert!(serde_json::from_str::<StorageImage>(&wrong_digest).is_err());

        let mut value = serde_json::to_value(&image).unwrap();
        value["entries"][0]["payloads"]["unpacked"]["bytes"] = serde_json::json!([9]);
        value["entries"][0]["payloads"]["unpacked"]["byte_len"] = serde_json::json!(1);
        value["entries"][0]["payloads"]["unpacked"]["sha256"] =
            serde_json::json!(Sha256Digest::for_bytes(&[9]).to_string());
        assert!(serde_json::from_value::<StorageImage>(value).is_err());
    }

    #[test]
    fn image_budget_counts_all_retained_buffers_with_checked_arithmetic() {
        let first = entry("a", 0, 1, b"first");
        let second = entry("b", 0, 1, b"second");
        let expected_first = first.logical_name().as_str().len()
            + first.logical_key().as_str().len()
            + first.compression().as_str().len()
            + first.source_profile().as_str().len()
            + first.provenance().as_str().len()
            + first.attributes().len()
            + first.raw_header().len()
            + first.packed_payload().len()
            + first.unpacked_payload().len();
        assert_eq!(first.retained_byte_len().unwrap(), expected_first);

        let aggregate = expected_first
            .checked_add(second.retained_byte_len().unwrap())
            .unwrap();
        assert!(
            validate_image_with_retained_byte_limit(&[first.clone(), second.clone()], aggregate)
                .is_ok()
        );
        assert!(matches!(
            validate_image_with_retained_byte_limit(&[first.clone(), second], aggregate - 1),
            Err(StorageBuildError::ImageRetainedBytesExceeded { .. })
        ));
        assert!(matches!(
            checked_image_retained_bytes(usize::MAX, &first, usize::MAX),
            Err(StorageBuildError::RetainedByteCountOverflow)
        ));
    }

    #[test]
    fn image_deserialization_enforces_the_aggregate_budget_while_streaming() {
        let json = serde_json::to_string(&vec![entry("x", 0, 1, b"payload")]).unwrap();
        let error = serde_json::from_str::<BoundedStorageEntries<4, 1>>(&json).unwrap_err();
        assert!(error.to_string().contains("aggregate budget"));
    }

    #[test]
    fn multipart_and_duplicate_invariants_are_fail_closed() {
        assert!(matches!(
            MultipartIdentity::new(0, 0),
            Err(StorageBuildError::InvalidPartCount { .. })
        ));
        assert!(matches!(
            MultipartIdentity::new(2, 2),
            Err(StorageBuildError::PartIndexOutOfRange { .. })
        ));

        let duplicate = StorageImage::new(vec![entry("x", 0, 1, b"a"), entry("x", 0, 1, b"a")]);
        assert!(matches!(
            duplicate,
            Err(StorageBuildError::DuplicateEntryPart { .. })
        ));

        let incomplete = StorageImage::new(vec![entry("x", 0, 2, b"a")]);
        assert!(matches!(
            incomplete,
            Err(StorageBuildError::IncompleteMultipart { .. })
        ));

        let reversed = StorageImage::new(vec![entry("x", 1, 2, b"b"), entry("x", 0, 2, b"a")]);
        assert!(matches!(
            reversed,
            Err(StorageBuildError::UnexpectedMultipartOrder {
                expected: 0,
                actual: 1,
                ..
            })
        ));

        let gap = StorageImage::new(vec![entry("x", 0, 3, b"a"), entry("x", 2, 3, b"c")]);
        assert!(matches!(
            gap,
            Err(StorageBuildError::UnexpectedMultipartOrder {
                expected: 1,
                actual: 2,
                ..
            })
        ));

        let complete =
            StorageImage::new(vec![entry("x", 0, 2, b"a"), entry("x", 1, 2, b"b")]).unwrap();
        assert_eq!(complete.entries()[0].multipart().part_index(), 0);
    }

    #[test]
    fn text_and_byte_visitors_reject_before_domain_retention() {
        let too_long_name = "x".repeat(MAX_STORAGE_NAME_BYTES + 1);
        assert!(StorageName::new(&too_long_name).is_err());
        assert!(serde_json::from_value::<StorageName>(serde_json::json!(too_long_name)).is_err());
        assert!(StorageKey::new("bad\nkey").is_err());

        assert!(serde_json::from_str::<BoundedBytes<3>>("[1,2,3]").is_ok());
        assert!(serde_json::from_str::<BoundedBytes<3>>("[1,2,3,4]").is_err());
    }

    #[test]
    fn compression_kind_is_open_but_stored_is_strict() {
        let future = CompressionKind::new("vendor:future-2").unwrap();
        let json = serde_json::to_string(&future).unwrap();
        assert_eq!(json, "\"vendor:future-2\"");
        assert_eq!(
            serde_json::from_str::<CompressionKind>(&json).unwrap(),
            future
        );
        assert!(CompressionKind::new("raw deflate").is_err());

        let result = StorageEntry::new(
            StorageName::new("x").unwrap(),
            StorageKey::new("x").unwrap(),
            MultipartIdentity::single(),
            OpaqueStorageMetadata::empty(),
            StoragePayloads::new(vec![1], vec![2]).unwrap(),
            CompressionKind::stored(),
            origin(),
        );
        assert!(matches!(
            result,
            Err(StorageBuildError::StoredPayloadMismatch)
        ));
    }
}
