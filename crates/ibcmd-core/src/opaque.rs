//! Anchored opaque facets and fail-closed emission permits.

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use serde::de::{IgnoredAny, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use crate::artifact::ProfileId;
use crate::asset::{Asset, AssetBuildError, MediaKind};
use crate::diagnostic::{Diagnostic, DiagnosticCode, Severity};
use crate::provenance::{CanonicalAnchor, SourceProvenance};
use crate::storage::Sha256Digest;

/// Stable diagnostic code for an opaque same-profile boundary violation.
pub const CROSS_PROFILE_OPAQUE_EMIT_CODE: &str = "opaque.cross-profile-emit-forbidden";
/// Maximum encoded length of an open placement-kind token.
pub const MAX_OPAQUE_PLACEMENT_KIND_BYTES: usize = 128;
/// Maximum number of ordered opaque facets in one collection.
pub const MAX_OPAQUE_FACETS: usize = 16_384;
/// Maximum aggregate variable-sized bytes retained by one facet collection.
pub const MAX_OPAQUE_RETAINED_BYTES: usize = 67_108_864;

/// Failure to construct or revalidate bounded opaque data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OpaqueBuildError {
    /// A placement kind was empty.
    EmptyPlacementKind,
    /// A placement kind exceeded its encoded bound.
    PlacementKindTooLong {
        /// Maximum accepted bytes.
        maximum: usize,
        /// Actual bytes.
        actual: usize,
    },
    /// A placement kind violated the stable open-token grammar.
    InvalidPlacementKind,
    /// Content-addressed bytes were invalid.
    Asset(AssetBuildError),
    /// A facet collection exceeded its item bound.
    TooManyFacets {
        /// Maximum accepted facets.
        maximum: usize,
        /// Actual facets, when known.
        actual: usize,
    },
    /// A facet collection exceeded its aggregate retained-byte budget.
    RetainedBytesExceeded {
        /// Maximum accepted retained bytes.
        maximum: usize,
        /// Actual retained bytes.
        actual: usize,
    },
    /// Aggregate retained-byte arithmetic overflowed.
    RetainedByteCountOverflow,
}

impl Display for OpaqueBuildError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPlacementKind => formatter.write_str("opaque placement kind is empty"),
            Self::PlacementKindTooLong { maximum, actual } => write!(
                formatter,
                "opaque placement kind exceeds {maximum} bytes (actual {actual})"
            ),
            Self::InvalidPlacementKind => formatter.write_str(
                "opaque placement kind must be a stable ASCII token using letters, digits, '.', '-', '_', or ':'",
            ),
            Self::Asset(error) => write!(formatter, "invalid opaque bytes: {error}"),
            Self::TooManyFacets { maximum, actual } => write!(
                formatter,
                "opaque facet collection exceeds {maximum} facets (actual {actual})"
            ),
            Self::RetainedBytesExceeded { maximum, actual } => write!(
                formatter,
                "opaque facets exceed aggregate retained-byte budget {maximum} (actual {actual})"
            ),
            Self::RetainedByteCountOverflow => {
                formatter.write_str("opaque retained-byte count overflowed")
            }
        }
    }
}

impl Error for OpaqueBuildError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Asset(error) => Some(error),
            _ => None,
        }
    }
}

impl From<AssetBuildError> for OpaqueBuildError {
    fn from(error: AssetBuildError) -> Self {
        Self::Asset(error)
    }
}

/// An open stable token describing how bytes were placed at their anchor.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OpaquePlacementKind(Box<str>);

impl OpaquePlacementKind {
    /// Validates and retains an open placement-kind token.
    pub fn new(value: &str) -> Result<Self, OpaqueBuildError> {
        validate_placement_kind(value)?;
        Ok(Self(value.into()))
    }

    /// Returns the exact retained token.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn validate_placement_kind(value: &str) -> Result<(), OpaqueBuildError> {
    if value.is_empty() {
        return Err(OpaqueBuildError::EmptyPlacementKind);
    }
    if value.len() > MAX_OPAQUE_PLACEMENT_KIND_BYTES {
        return Err(OpaqueBuildError::PlacementKindTooLong {
            maximum: MAX_OPAQUE_PLACEMENT_KIND_BYTES,
            actual: value.len(),
        });
    }
    let bytes = value.as_bytes();
    if !bytes[0].is_ascii_alphanumeric()
        || !bytes[bytes.len() - 1].is_ascii_alphanumeric()
        || !bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b':'))
    {
        return Err(OpaqueBuildError::InvalidPlacementKind);
    }
    Ok(())
}

impl Display for OpaquePlacementKind {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Serialize for OpaquePlacementKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

struct OpaquePlacementKindVisitor;

impl<'de> Visitor<'de> for OpaquePlacementKindVisitor {
    type Value = OpaquePlacementKind;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "a stable open placement token of at most {MAX_OPAQUE_PLACEMENT_KIND_BYTES} bytes"
        )
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        OpaquePlacementKind::new(value).map_err(E::custom)
    }
}

impl<'de> Deserialize<'de> for OpaquePlacementKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(OpaquePlacementKindVisitor)
    }
}

/// Exact source-relative placement at an anchor.
///
/// `kind` remains open for future adapters; `ordinal` preserves order within
/// that placement class without assigning vendor-specific meaning in core.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpaquePlacement {
    kind: OpaquePlacementKind,
    ordinal: u32,
}

impl OpaquePlacement {
    /// Creates a placement from an open stable kind and exact ordinal.
    pub fn new(kind: &str, ordinal: u32) -> Result<Self, OpaqueBuildError> {
        Ok(Self {
            kind: OpaquePlacementKind::new(kind)?,
            ordinal,
        })
    }

    /// Creates a placement from already validated parts.
    pub const fn from_parts(kind: OpaquePlacementKind, ordinal: u32) -> Self {
        Self { kind, ordinal }
    }

    /// Returns the exact open placement kind.
    pub const fn kind(&self) -> &OpaquePlacementKind {
        &self.kind
    }

    /// Returns the exact zero-based source ordinal declared by the adapter.
    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }

    fn retained_byte_len(&self) -> usize {
        self.kind.as_str().len()
    }
}

/// Anchored unknown bytes that are safe to pass through only to their source profile.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpaqueFacet {
    provenance: SourceProvenance,
    placement: OpaquePlacement,
    asset: Asset,
}

impl OpaqueFacet {
    /// Retains exact bounded bytes with computed length and SHA-256 metadata.
    pub fn new(
        provenance: SourceProvenance,
        placement: OpaquePlacement,
        bytes: Vec<u8>,
        media_kind: MediaKind,
    ) -> Result<Self, OpaqueBuildError> {
        Ok(Self::from_asset(
            provenance,
            placement,
            Asset::new(bytes, media_kind)?,
        ))
    }

    /// Attaches an already validated immutable asset to an anchor and placement.
    pub const fn from_asset(
        provenance: SourceProvenance,
        placement: OpaquePlacement,
        asset: Asset,
    ) -> Self {
        Self {
            provenance,
            placement,
            asset,
        }
    }

    /// Returns exact source provenance.
    pub const fn provenance(&self) -> &SourceProvenance {
        &self.provenance
    }

    /// Returns the exact source profile.
    pub fn source_profile(&self) -> &ProfileId {
        self.provenance.source_profile()
    }

    /// Returns the stable object/property anchor.
    pub const fn anchor(&self) -> &CanonicalAnchor {
        self.provenance.anchor()
    }

    /// Returns source-relative placement.
    pub const fn placement(&self) -> &OpaquePlacement {
        &self.placement
    }

    fn bytes(&self) -> &[u8] {
        self.asset.bytes()
    }

    /// Returns exact byte length metadata.
    pub const fn byte_len(&self) -> u64 {
        self.asset.byte_len()
    }

    /// Returns the stable SHA-256 digest.
    pub const fn sha256(&self) -> Sha256Digest {
        self.asset.sha256()
    }

    /// Returns the exact open media kind.
    pub const fn media_kind(&self) -> &MediaKind {
        self.asset.media_kind()
    }

    /// Grants byte emission only when the requested profile exactly matches the source.
    ///
    /// The returned permit has no public constructor. A different profile
    /// produces a stable, path-addressed error diagnostic and never yields a
    /// byte view that an emitter could accidentally use.
    pub fn emit_permit(
        &self,
        target_profile: &ProfileId,
    ) -> Result<OpaqueEmitPermit<'_>, OpaqueEmitError> {
        if self.source_profile() == target_profile {
            return Ok(OpaqueEmitPermit { facet: self });
        }
        Err(OpaqueEmitError {
            diagnostic: Box::new(self.cross_profile_diagnostic(target_profile)),
        })
    }

    fn cross_profile_diagnostic(&self, target_profile: &ProfileId) -> Diagnostic {
        let diagnostic = Diagnostic::new(
            DiagnosticCode::new(CROSS_PROFILE_OPAQUE_EMIT_CODE)
                .expect("static opaque diagnostic code is valid"),
            Severity::Error,
            self.anchor().object_path().clone(),
            self.anchor().property_path().clone(),
            "opaque bytes cannot be emitted for a different profile without an explicit migration rule",
        )
        .expect("static opaque diagnostic message is bounded")
        .with_profiles(
            Some(self.source_profile().clone()),
            Some(target_profile.clone()),
        );
        let digest = self.sha256().to_string();
        diagnostic
            .with_context("opaque.sha256", &digest)
            .expect("digest context is bounded")
            .with_context("opaque.placement", self.placement.kind.as_str())
            .expect("validated placement context is bounded")
    }

    fn retained_byte_len(&self) -> Result<usize, OpaqueBuildError> {
        self.asset
            .retained_byte_len()
            .checked_add(self.provenance.retained_byte_len())
            .and_then(|value| value.checked_add(self.placement.retained_byte_len()))
            .ok_or(OpaqueBuildError::RetainedByteCountOverflow)
    }
}

/// Proof that a particular facet passed the exact same-profile check.
///
/// Its sole field and constructor are private. External callers can inspect
/// or forward bytes only after obtaining a permit from [`OpaqueFacet::emit_permit`].
#[derive(Clone, Copy, Debug)]
pub struct OpaqueEmitPermit<'a> {
    facet: &'a OpaqueFacet,
}

impl<'a> OpaqueEmitPermit<'a> {
    /// Returns the permitted facet.
    pub const fn facet(self) -> &'a OpaqueFacet {
        self.facet
    }

    /// Returns exact bytes covered by this same-profile permit.
    pub fn bytes(self) -> &'a [u8] {
        self.facet.bytes()
    }

    /// Returns the only target profile for which the permit is valid.
    pub fn target_profile(self) -> &'a ProfileId {
        self.facet.source_profile()
    }
}

/// Fail-closed opaque-emission error with a machine-readable diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpaqueEmitError {
    diagnostic: Box<Diagnostic>,
}

impl OpaqueEmitError {
    /// Returns the structured error diagnostic.
    pub fn diagnostic(&self) -> &Diagnostic {
        self.diagnostic.as_ref()
    }

    /// Consumes the error and returns its structured diagnostic.
    pub fn into_diagnostic(self) -> Diagnostic {
        *self.diagnostic
    }
}

impl Display for OpaqueEmitError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: {}",
            self.diagnostic.code(),
            self.diagnostic.message()
        )
    }
}

impl Error for OpaqueEmitError {}

/// Exact ordered opaque facets attached to a canonical object or property.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct OpaqueFacets {
    facets: Vec<OpaqueFacet>,
}

impl OpaqueFacets {
    /// Validates an ordered collection without sorting or deduplicating it.
    pub fn new(facets: Vec<OpaqueFacet>) -> Result<Self, OpaqueBuildError> {
        validate_facets(&facets, MAX_OPAQUE_FACETS, MAX_OPAQUE_RETAINED_BYTES)?;
        Ok(Self { facets })
    }

    /// Returns facets in exact source order.
    pub fn as_slice(&self) -> &[OpaqueFacet] {
        &self.facets
    }

    /// Returns the number of facets.
    pub const fn len(&self) -> usize {
        self.facets.len()
    }

    /// Returns whether no opaque facets are retained.
    pub const fn is_empty(&self) -> bool {
        self.facets.is_empty()
    }

    /// Consumes the collection without changing order.
    pub fn into_vec(self) -> Vec<OpaqueFacet> {
        self.facets
    }
}

impl Serialize for OpaqueFacets {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.facets.serialize(serializer)
    }
}

fn validate_facets(
    facets: &[OpaqueFacet],
    maximum_facets: usize,
    maximum_retained_bytes: usize,
) -> Result<(), OpaqueBuildError> {
    if facets.len() > maximum_facets {
        return Err(OpaqueBuildError::TooManyFacets {
            maximum: maximum_facets,
            actual: facets.len(),
        });
    }
    let mut retained_bytes = 0_usize;
    for facet in facets {
        retained_bytes = checked_retained_bytes(
            retained_bytes,
            facet.retained_byte_len()?,
            maximum_retained_bytes,
        )?;
    }
    Ok(())
}

fn checked_retained_bytes(
    current: usize,
    additional: usize,
    maximum: usize,
) -> Result<usize, OpaqueBuildError> {
    let actual = current
        .checked_add(additional)
        .ok_or(OpaqueBuildError::RetainedByteCountOverflow)?;
    if actual > maximum {
        return Err(OpaqueBuildError::RetainedBytesExceeded { maximum, actual });
    }
    Ok(actual)
}

struct BoundedOpaqueFacets<const MAXIMUM_FACETS: usize, const MAXIMUM_RETAINED_BYTES: usize>(
    Vec<OpaqueFacet>,
);

struct BoundedOpaqueFacetsVisitor<const MAXIMUM_FACETS: usize, const MAXIMUM_RETAINED_BYTES: usize>;

impl<'de, const MAXIMUM_FACETS: usize, const MAXIMUM_RETAINED_BYTES: usize> Visitor<'de>
    for BoundedOpaqueFacetsVisitor<MAXIMUM_FACETS, MAXIMUM_RETAINED_BYTES>
{
    type Value = BoundedOpaqueFacets<MAXIMUM_FACETS, MAXIMUM_RETAINED_BYTES>;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "an ordered collection of at most {MAXIMUM_FACETS} opaque facets retaining at most {MAXIMUM_RETAINED_BYTES} bytes"
        )
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut facets =
            Vec::with_capacity(sequence.size_hint().unwrap_or_default().min(MAXIMUM_FACETS));
        let mut retained_bytes = 0_usize;
        while facets.len() < MAXIMUM_FACETS {
            let Some(facet) = sequence.next_element::<OpaqueFacet>()? else {
                return Ok(BoundedOpaqueFacets(facets));
            };
            retained_bytes = checked_retained_bytes(
                retained_bytes,
                facet.retained_byte_len().map_err(de::Error::custom)?,
                MAXIMUM_RETAINED_BYTES,
            )
            .map_err(de::Error::custom)?;
            facets.push(facet);
        }
        if sequence.next_element::<IgnoredAny>()?.is_some() {
            return Err(de::Error::custom(format_args!(
                "opaque facet collection exceeds {MAXIMUM_FACETS} facets"
            )));
        }
        Ok(BoundedOpaqueFacets(facets))
    }
}

impl<'de, const MAXIMUM_FACETS: usize, const MAXIMUM_RETAINED_BYTES: usize> Deserialize<'de>
    for BoundedOpaqueFacets<MAXIMUM_FACETS, MAXIMUM_RETAINED_BYTES>
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer
            .deserialize_seq(BoundedOpaqueFacetsVisitor::<MAXIMUM_FACETS, MAXIMUM_RETAINED_BYTES>)
    }
}

impl<'de> Deserialize<'de> for OpaqueFacets {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let bounded =
            BoundedOpaqueFacets::<MAX_OPAQUE_FACETS, MAX_OPAQUE_RETAINED_BYTES>::deserialize(
                deserializer,
            )?;
        Ok(Self { facets: bounded.0 })
    }
}

#[cfg(test)]
mod tests {
    use crate::diagnostic::{ObjectPath, PathSegment, PropertyPath};

    use super::*;

    fn provenance(profile: &str) -> SourceProvenance {
        SourceProvenance::with_locator(
            ProfileId::parse(profile).unwrap(),
            CanonicalAnchor::new(
                ObjectPath::new(vec![PathSegment::name("objects").unwrap()]).unwrap(),
                PropertyPath::new(vec![PathSegment::name("future").unwrap()]).unwrap(),
            ),
            "fixture:opaque",
        )
        .unwrap()
    }

    fn facet(profile: &str, ordinal: u32, bytes: &[u8]) -> OpaqueFacet {
        OpaqueFacet::new(
            provenance(profile),
            OpaquePlacement::new("xml:child", ordinal).unwrap(),
            bytes.to_vec(),
            MediaKind::new("application/x-vendor+xml").unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn same_profile_permit_exposes_exact_bytes() {
        let facet = facet("profile:source", 0, b"<future/>");
        let target = ProfileId::parse("profile:source").unwrap();
        let permit = facet.emit_permit(&target).unwrap();
        assert_eq!(permit.bytes(), b"<future/>");
        assert_eq!(permit.target_profile(), facet.source_profile());
    }

    #[test]
    fn cross_profile_emit_fails_with_path_addressed_error_diagnostic() {
        let facet = facet("profile:source", 3, b"unknown");
        let target = ProfileId::parse("profile:target").unwrap();
        let error = facet.emit_permit(&target).unwrap_err();
        let diagnostic = error.diagnostic();
        assert_eq!(diagnostic.code().as_str(), CROSS_PROFILE_OPAQUE_EMIT_CODE);
        assert_eq!(diagnostic.severity(), Severity::Error);
        assert_eq!(diagnostic.object_path(), facet.anchor().object_path());
        assert_eq!(diagnostic.property_path(), facet.anchor().property_path());
        assert_eq!(diagnostic.source_profile(), Some(facet.source_profile()));
        assert_eq!(diagnostic.target_profile(), Some(&target));
    }

    #[test]
    fn facet_asset_deserialization_rejects_tampered_digest() {
        let facet = facet("profile:source", 0, b"opaque");
        let json = serde_json::to_string(&facet).unwrap();
        let tampered = json.replacen(&facet.sha256().to_string(), &"0".repeat(64), 1);
        assert!(serde_json::from_str::<OpaqueFacet>(&tampered).is_err());
    }

    #[test]
    fn facet_collection_preserves_order_and_streams_bounds() {
        let first = facet("profile:source", 0, b"first");
        let second = facet("profile:source", 1, b"second");
        let ordered = OpaqueFacets::new(vec![first.clone(), second.clone()]).unwrap();
        let reversed = OpaqueFacets::new(vec![second, first]).unwrap();
        assert_ne!(ordered, reversed);
        let json = serde_json::to_string(&ordered).unwrap();
        assert_eq!(
            serde_json::from_str::<OpaqueFacets>(&json).unwrap(),
            ordered
        );

        assert!(serde_json::from_str::<BoundedOpaqueFacets<1, { usize::MAX }>>(&json).is_err());
        assert!(serde_json::from_str::<BoundedOpaqueFacets<2, 1>>(&json).is_err());
    }

    #[test]
    fn placement_kind_is_open_but_bounded() {
        let future = OpaquePlacement::new("future:lexical-slot-v2", 42).unwrap();
        assert_eq!(future.kind().as_str(), "future:lexical-slot-v2");
        assert_eq!(future.ordinal(), 42);
        assert!(OpaquePlacement::new("bad placement", 0).is_err());
        assert!(OpaquePlacement::new(&"x".repeat(MAX_OPAQUE_PLACEMENT_KIND_BYTES + 1), 0).is_err());
    }
}
