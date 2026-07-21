//! Immutable content-addressed binary assets.

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use serde::de::{IgnoredAny, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use crate::storage::Sha256Digest;

/// Maximum number of bytes retained by one canonical asset.
pub const MAX_ASSET_BYTES: usize = 33_554_432;
/// Maximum encoded length of an open media-kind token.
pub const MAX_MEDIA_KIND_BYTES: usize = 256;

/// Failure to construct or revalidate an immutable asset.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AssetBuildError {
    /// A media kind was empty.
    EmptyMediaKind,
    /// A media kind exceeded its encoded bound.
    MediaKindTooLong {
        /// Maximum accepted UTF-8 bytes.
        maximum: usize,
        /// Actual UTF-8 bytes.
        actual: usize,
    },
    /// A media kind contained whitespace, a control character, or non-ASCII text.
    InvalidMediaKind,
    /// Asset bytes exceeded their retained bound.
    AssetTooLarge {
        /// Maximum accepted bytes.
        maximum: usize,
        /// Actual bytes.
        actual: u64,
    },
    /// Serialized length metadata did not match the retained bytes.
    LengthMismatch {
        /// Declared length.
        declared: u64,
        /// Actual length.
        actual: u64,
    },
    /// Serialized digest metadata did not match the retained bytes.
    DigestMismatch {
        /// Declared digest.
        declared: Sha256Digest,
        /// Digest computed from exact bytes.
        actual: Sha256Digest,
    },
}

impl Display for AssetBuildError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyMediaKind => formatter.write_str("asset media kind is empty"),
            Self::MediaKindTooLong { maximum, actual } => write!(
                formatter,
                "asset media kind exceeds {maximum} bytes (actual {actual})"
            ),
            Self::InvalidMediaKind => formatter.write_str(
                "asset media kind must contain only visible non-whitespace ASCII characters",
            ),
            Self::AssetTooLarge { maximum, actual } => {
                write!(formatter, "asset exceeds {maximum} bytes (actual {actual})")
            }
            Self::LengthMismatch { declared, actual } => write!(
                formatter,
                "asset length metadata is {declared}, but exact bytes contain {actual} bytes"
            ),
            Self::DigestMismatch { declared, actual } => write!(
                formatter,
                "asset SHA-256 metadata is {declared}, but exact bytes hash to {actual}"
            ),
        }
    }
}

impl Error for AssetBuildError {}

/// An open, bounded media-kind token retained exactly.
///
/// Unknown future values remain valid. No registry lookup or case folding is
/// performed by the canonical layer.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MediaKind(Box<str>);

impl MediaKind {
    /// Validates and retains an open media-kind token.
    pub fn new(value: &str) -> Result<Self, AssetBuildError> {
        validate_media_kind(value)?;
        Ok(Self(value.into()))
    }

    /// Returns the exact retained token.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the conventional opaque binary media kind.
    pub fn octet_stream() -> Self {
        Self("application/octet-stream".into())
    }
}

fn validate_media_kind(value: &str) -> Result<(), AssetBuildError> {
    if value.is_empty() {
        return Err(AssetBuildError::EmptyMediaKind);
    }
    if value.len() > MAX_MEDIA_KIND_BYTES {
        return Err(AssetBuildError::MediaKindTooLong {
            maximum: MAX_MEDIA_KIND_BYTES,
            actual: value.len(),
        });
    }
    if !value.bytes().all(|byte| byte.is_ascii_graphic()) {
        return Err(AssetBuildError::InvalidMediaKind);
    }
    Ok(())
}

impl Display for MediaKind {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Serialize for MediaKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

struct MediaKindVisitor;

impl<'de> Visitor<'de> for MediaKindVisitor {
    type Value = MediaKind;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "a visible ASCII media-kind token of at most {MAX_MEDIA_KIND_BYTES} bytes"
        )
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        MediaKind::new(value).map_err(E::custom)
    }
}

impl<'de> Deserialize<'de> for MediaKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(MediaKindVisitor)
    }
}

#[derive(Debug)]
pub(crate) struct BoundedBytes<const MAXIMUM: usize>(Box<[u8]>);

impl<const MAXIMUM: usize> BoundedBytes<MAXIMUM> {
    pub(crate) fn into_boxed_slice(self) -> Box<[u8]> {
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

/// Digest, length, and media metadata for an asset stored elsewhere.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct AssetReference {
    sha256: Sha256Digest,
    byte_len: u64,
    media_kind: MediaKind,
}

impl AssetReference {
    /// Constructs a reference whose declared size is within the canonical bound.
    pub fn new(
        sha256: Sha256Digest,
        byte_len: u64,
        media_kind: MediaKind,
    ) -> Result<Self, AssetBuildError> {
        if byte_len > MAX_ASSET_BYTES as u64 {
            return Err(AssetBuildError::AssetTooLarge {
                maximum: MAX_ASSET_BYTES,
                actual: byte_len,
            });
        }
        Ok(Self {
            sha256,
            byte_len,
            media_kind,
        })
    }

    /// Returns the referenced content digest.
    pub const fn sha256(&self) -> Sha256Digest {
        self.sha256
    }

    /// Returns the referenced byte length.
    pub const fn byte_len(&self) -> u64 {
        self.byte_len
    }

    /// Returns the exact open media-kind token.
    pub const fn media_kind(&self) -> &MediaKind {
        &self.media_kind
    }

    pub(crate) fn retained_byte_len(&self) -> usize {
        32 + self.media_kind.as_str().len()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAssetReference {
    sha256: Sha256Digest,
    byte_len: u64,
    media_kind: MediaKind,
}

impl<'de> Deserialize<'de> for AssetReference {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawAssetReference::deserialize(deserializer)?;
        Self::new(raw.sha256, raw.byte_len, raw.media_kind).map_err(de::Error::custom)
    }
}

/// Immutable exact bytes identified by content, length, and media kind.
///
/// All fields are private. Deserialization streams through a bounded byte
/// visitor and revalidates both redundant metadata fields before constructing
/// the value.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Asset {
    byte_len: u64,
    sha256: Sha256Digest,
    media_kind: MediaKind,
    bytes: Box<[u8]>,
}

impl Asset {
    /// Retains exact bounded bytes and computes content metadata.
    pub fn new(bytes: Vec<u8>, media_kind: MediaKind) -> Result<Self, AssetBuildError> {
        validate_asset_size(bytes.len() as u64)?;
        let byte_len = bytes.len() as u64;
        let sha256 = Sha256Digest::for_bytes(&bytes);
        Ok(Self {
            byte_len,
            sha256,
            media_kind,
            bytes: bytes.into_boxed_slice(),
        })
    }

    /// Convenience constructor that validates an open media-kind string.
    pub fn from_bytes(bytes: Vec<u8>, media_kind: &str) -> Result<Self, AssetBuildError> {
        Self::new(bytes, MediaKind::new(media_kind)?)
    }

    fn from_serialized(
        byte_len: u64,
        sha256: Sha256Digest,
        media_kind: MediaKind,
        bytes: Box<[u8]>,
    ) -> Result<Self, AssetBuildError> {
        validate_asset_size(byte_len)?;
        let actual_len = bytes.len() as u64;
        if byte_len != actual_len {
            return Err(AssetBuildError::LengthMismatch {
                declared: byte_len,
                actual: actual_len,
            });
        }
        let actual_digest = Sha256Digest::for_bytes(&bytes);
        if sha256 != actual_digest {
            return Err(AssetBuildError::DigestMismatch {
                declared: sha256,
                actual: actual_digest,
            });
        }
        Ok(Self {
            byte_len,
            sha256,
            media_kind,
            bytes,
        })
    }

    /// Returns exact immutable bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Returns exact byte length metadata.
    pub const fn byte_len(&self) -> u64 {
        self.byte_len
    }

    /// Returns the stable SHA-256 digest.
    pub const fn sha256(&self) -> Sha256Digest {
        self.sha256
    }

    /// Returns the exact open media-kind token.
    pub const fn media_kind(&self) -> &MediaKind {
        &self.media_kind
    }

    /// Creates a metadata-only reference to these exact bytes.
    pub fn as_reference(&self) -> AssetReference {
        AssetReference {
            sha256: self.sha256,
            byte_len: self.byte_len,
            media_kind: self.media_kind.clone(),
        }
    }

    /// Returns bytes retained by variable-sized asset fields.
    pub fn retained_byte_len(&self) -> usize {
        self.bytes.len() + 32 + self.media_kind.as_str().len()
    }
}

fn validate_asset_size(actual: u64) -> Result<(), AssetBuildError> {
    if actual > MAX_ASSET_BYTES as u64 {
        return Err(AssetBuildError::AssetTooLarge {
            maximum: MAX_ASSET_BYTES,
            actual,
        });
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAsset {
    byte_len: u64,
    sha256: Sha256Digest,
    media_kind: MediaKind,
    bytes: BoundedBytes<MAX_ASSET_BYTES>,
}

impl<'de> Deserialize<'de> for Asset {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawAsset::deserialize(deserializer)?;
        Self::from_serialized(
            raw.byte_len,
            raw.sha256,
            raw.media_kind,
            raw.bytes.into_boxed_slice(),
        )
        .map_err(de::Error::custom)
    }
}

/// Explicit name for an [`Asset`] when an API wants to emphasize content addressing.
pub type ContentAsset = Asset;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_round_trip_revalidates_digest_and_length() {
        let asset = Asset::from_bytes(vec![0, 1, 2, 255], "application/x-future").unwrap();
        assert_eq!(asset.as_reference().sha256(), asset.sha256());
        let json = serde_json::to_string(&asset).unwrap();
        assert_eq!(serde_json::from_str::<Asset>(&json).unwrap(), asset);

        let tampered_digest = json.replacen(&asset.sha256().to_string(), &"0".repeat(64), 1);
        assert!(serde_json::from_str::<Asset>(&tampered_digest).is_err());
        let tampered_length = json.replacen("\"byte_len\":4", "\"byte_len\":3", 1);
        assert!(serde_json::from_str::<Asset>(&tampered_length).is_err());
    }

    #[test]
    fn metadata_and_streamed_bytes_are_bounded() {
        assert!(
            AssetReference::new(
                Sha256Digest::for_bytes(&[]),
                MAX_ASSET_BYTES as u64 + 1,
                MediaKind::octet_stream(),
            )
            .is_err()
        );
        assert!(serde_json::from_str::<BoundedBytes<3>>("[1,2,3]").is_ok());
        assert!(serde_json::from_str::<BoundedBytes<3>>("[1,2,3,4]").is_err());
        assert!(MediaKind::new("application/x-vendor.future+bin").is_ok());
        assert!(MediaKind::new("application / bad").is_err());
    }
}
