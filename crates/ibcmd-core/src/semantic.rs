//! Cross-platform semantic equality and digests.
//!
//! The digest is computed from a versioned, domain-separated byte encoding.
//! Every integer has an explicit big-endian width, every variable byte string
//! is prefixed by a big-endian `u64` length, and no serde representation,
//! native `usize`, hash-map iteration, debug output, OS path, or line-ending
//! normalization participates.

use std::fmt::{self, Display, Formatter};

use serde::{Serialize, Serializer};
use sha2::{Digest, Sha256};

use crate::asset::{Asset, AssetReference};
use crate::diagnostic::{ObjectPath, PathSegment, PropertyPath};
use crate::identity::ObjectUuid;
use crate::model::{CanonicalConfiguration, CanonicalObject};
use crate::opaque::OpaqueFacet;
use crate::validate::ValidatedConfiguration;
use crate::value::{CanonicalField, CanonicalValue, CanonicalValueKind};

const SEMANTIC_DOMAIN_V1: &[u8] = b"ibcmd-canonical-semantic-v1\0";

/// Cross-platform SHA-256 of the versioned canonical semantic encoding.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SemanticDigest([u8; 32]);

impl SemanticDigest {
    /// Returns the exact 32 digest bytes.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl Display for SemanticDigest {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl Serialize for SemanticDigest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

/// Computes a semantic digest only after complete graph validation.
pub fn semantic_digest(configuration: &ValidatedConfiguration<'_>) -> SemanticDigest {
    let mut encoder = SemanticEncoder::new();
    encode_configuration(configuration.configuration(), &mut encoder);
    encoder.finish()
}

/// Compares two validated models using their versioned semantic digests.
pub fn semantic_eq(left: &ValidatedConfiguration<'_>, right: &ValidatedConfiguration<'_>) -> bool {
    semantic_digest(left) == semantic_digest(right)
}

impl ValidatedConfiguration<'_> {
    /// Computes this validated model's versioned semantic digest.
    pub fn semantic_digest(&self) -> SemanticDigest {
        semantic_digest(self)
    }

    /// Compares semantic content with another validated model.
    pub fn semantically_eq(&self, other: &ValidatedConfiguration<'_>) -> bool {
        semantic_eq(self, other)
    }
}

struct SemanticEncoder {
    hasher: Sha256,
}

impl SemanticEncoder {
    fn new() -> Self {
        let mut hasher = Sha256::new();
        hasher.update(SEMANTIC_DOMAIN_V1);
        Self { hasher }
    }

    fn finish(self) -> SemanticDigest {
        let digest = self.hasher.finalize();
        let mut bytes = [0_u8; 32];
        bytes.copy_from_slice(&digest);
        SemanticDigest(bytes)
    }

    fn raw(&mut self, bytes: &[u8]) {
        self.hasher.update(bytes);
    }

    fn tag(&mut self, tag: u8) {
        self.raw(&[tag]);
    }

    fn bool(&mut self, value: bool) {
        self.tag(u8::from(value));
    }

    fn u32(&mut self, value: u32) {
        self.raw(&value.to_be_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.raw(&value.to_be_bytes());
    }

    fn count(&mut self, value: usize) {
        let value = u64::try_from(value).expect("bounded canonical count fits into u64");
        self.u64(value);
    }

    fn bytes(&mut self, value: &[u8]) {
        self.count(value.len());
        self.raw(value);
    }

    fn string(&mut self, value: &str) {
        self.bytes(value.as_bytes());
    }

    fn uuid(&mut self, value: ObjectUuid) {
        self.raw(value.as_bytes());
    }
}

fn encode_configuration(configuration: &CanonicalConfiguration, encoder: &mut SemanticEncoder) {
    let mut objects = configuration.objects().iter().collect::<Vec<_>>();
    objects.sort_by(|left, right| {
        left.identity()
            .path()
            .cmp(right.identity().path())
            .then_with(|| left.identity().uuid().cmp(&right.identity().uuid()))
    });
    encoder.count(objects.len());
    for object in objects {
        encode_object(object, encoder);
    }
}

fn encode_object(object: &CanonicalObject, encoder: &mut SemanticEncoder) {
    encoder.uuid(object.identity().uuid());
    encode_object_path(object.identity().path(), encoder);
    encoder.string(object.kind().as_str());
    match object.owner() {
        Some(owner) => {
            encoder.tag(1);
            encoder.uuid(owner);
        }
        None => encoder.tag(0),
    }

    encode_properties(object.properties(), encoder);

    encoder.count(object.references().len());
    for reference in object.references() {
        encoder.string(reference.kind().as_str());
        encoder.uuid(reference.target());
    }

    encoder.count(object.generated_types().len());
    for generated_type in object.generated_types() {
        encoder.uuid(generated_type.uuid());
        encoder.string(generated_type.kind().as_str());
    }

    encoder.count(object.assets().len());
    for asset in object.assets() {
        encode_asset_reference(asset, encoder);
    }

    encoder.count(object.opaque_facets().len());
    for facet in object.opaque_facets().as_slice() {
        encode_opaque_facet(facet, encoder);
    }

    // Source profile, source locator, and object provenance are evidence, not
    // semantic configuration content, and are intentionally excluded.
}

fn encode_properties(properties: &[CanonicalField], encoder: &mut SemanticEncoder) {
    encoder.count(properties.len());
    for property in properties {
        encoder.string(property.name().as_str());
        encode_value(property.value(), encoder);
    }
}

fn encode_value(value: &CanonicalValue, encoder: &mut SemanticEncoder) {
    match value.kind() {
        CanonicalValueKind::Null => encoder.tag(0),
        CanonicalValueKind::Bool(value) => {
            encoder.tag(1);
            encoder.bool(value);
        }
        CanonicalValueKind::Integer(value) => {
            encoder.tag(2);
            encoder.string(value.as_str());
        }
        CanonicalValueKind::Decimal(value) => {
            encoder.tag(3);
            encoder.string(value.as_str());
        }
        CanonicalValueKind::Text(value) => {
            encoder.tag(4);
            encoder.string(value.as_str());
        }
        CanonicalValueKind::EnumToken(value) => {
            encoder.tag(5);
            encoder.string(value.as_str());
        }
        CanonicalValueKind::Reference(value) => {
            encoder.tag(6);
            encoder.string(value.kind());
            encoder.string(value.target());
        }
        CanonicalValueKind::Record(fields) => {
            encoder.tag(7);
            encode_properties(fields, encoder);
        }
        CanonicalValueKind::Sequence(values) => {
            encoder.tag(8);
            encoder.count(values.len());
            for value in values {
                encode_value(value, encoder);
            }
        }
        CanonicalValueKind::Binary(asset) => {
            encoder.tag(9);
            encode_asset(asset, encoder);
        }
        CanonicalValueKind::AssetReference(reference) => {
            encoder.tag(10);
            encode_asset_reference(reference, encoder);
        }
    }
}

fn encode_asset(asset: &Asset, encoder: &mut SemanticEncoder) {
    encoder.u64(asset.byte_len());
    encoder.raw(asset.sha256().as_bytes());
    encoder.string(asset.media_kind().as_str());
}

fn encode_asset_reference(reference: &AssetReference, encoder: &mut SemanticEncoder) {
    encoder.u64(reference.byte_len());
    encoder.raw(reference.sha256().as_bytes());
    encoder.string(reference.media_kind().as_str());
}

fn encode_opaque_facet(facet: &OpaqueFacet, encoder: &mut SemanticEncoder) {
    encode_object_path(facet.anchor().object_path(), encoder);
    encode_property_path(facet.anchor().property_path(), encoder);
    encoder.string(facet.placement().kind().as_str());
    encoder.u32(facet.placement().ordinal());
    encoder.u64(facet.byte_len());
    encoder.raw(facet.sha256().as_bytes());
    encoder.string(facet.media_kind().as_str());
    // The exact source profile and source locator are deliberately excluded.
}

fn encode_object_path(path: &ObjectPath, encoder: &mut SemanticEncoder) {
    encode_path_segments(path.segments(), encoder);
}

fn encode_property_path(path: &PropertyPath, encoder: &mut SemanticEncoder) {
    encode_path_segments(path.segments(), encoder);
}

fn encode_path_segments(segments: &[PathSegment], encoder: &mut SemanticEncoder) {
    encoder.count(segments.len());
    for segment in segments {
        if let Some(name) = segment.as_name() {
            encoder.tag(0);
            encoder.string(name);
        } else if let Some(index) = segment.as_index() {
            encoder.tag(1);
            encoder.u32(index);
        } else {
            unreachable!("validated path segment is name or index");
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::artifact::ProfileId;
    use crate::asset::{Asset, MediaKind};
    use crate::diagnostic::PathSegment;
    use crate::identity::LogicalIdentity;
    use crate::model::{
        CanonicalObject, CanonicalObjectParts, GeneratedType, GeneratedTypeKind, MetadataKind,
        ObjectReference, ReferenceKind,
    };
    use crate::opaque::{OpaqueFacet, OpaqueFacets, OpaquePlacement};
    use crate::provenance::{CanonicalAnchor, SourceProvenance};
    use crate::validate::validate_configuration;
    use crate::value::{
        CanonicalDecimal, CanonicalInteger, CanonicalText, CanonicalValue, EnumToken,
        UnresolvedReference,
    };

    use super::*;

    fn uuid(suffix: u32) -> ObjectUuid {
        ObjectUuid::parse(&format!("00000000-0000-0000-0000-{suffix:012x}")).unwrap()
    }

    fn path(name: &str) -> ObjectPath {
        ObjectPath::new(vec![
            PathSegment::name("objects").unwrap(),
            PathSegment::name(name).unwrap(),
        ])
        .unwrap()
    }

    fn provenance(profile: &str, object_path: &ObjectPath) -> SourceProvenance {
        let locator = format!("fixture:semantic:{profile}");
        SourceProvenance::with_locator(
            ProfileId::parse(profile).unwrap(),
            CanonicalAnchor::new(object_path.clone(), PropertyPath::root()),
            &locator,
        )
        .unwrap()
    }

    fn opaque(profile: &str, object_path: &ObjectPath, bytes: &[u8]) -> OpaqueFacets {
        let locator = format!("fixture:opaque:{profile}");
        let facet = OpaqueFacet::new(
            SourceProvenance::with_locator(
                ProfileId::parse(profile).unwrap(),
                CanonicalAnchor::new(
                    object_path.clone(),
                    PropertyPath::new(vec![PathSegment::name("unknown").unwrap()]).unwrap(),
                ),
                &locator,
            )
            .unwrap(),
            OpaquePlacement::new("xml:child", 2).unwrap(),
            bytes.to_vec(),
            MediaKind::new("application/x-vendor+xml").unwrap(),
        )
        .unwrap();
        OpaqueFacets::new(vec![facet]).unwrap()
    }

    fn scalar_properties(text: &str) -> Vec<CanonicalField> {
        let inline = Asset::from_bytes(vec![1, 2, 3], "application/octet-stream").unwrap();
        let reference = Asset::from_bytes(vec![4, 5, 6], "application/x-reference")
            .unwrap()
            .as_reference();
        vec![
            CanonicalField::named("null", CanonicalValue::null()).unwrap(),
            CanonicalField::named("bool", CanonicalValue::boolean(true)).unwrap(),
            CanonicalField::named(
                "integer",
                CanonicalValue::integer(CanonicalInteger::new("-12345678901234567890").unwrap()),
            )
            .unwrap(),
            CanonicalField::named(
                "decimal",
                CanonicalValue::decimal(CanonicalDecimal::new("12.345").unwrap()),
            )
            .unwrap(),
            CanonicalField::named(
                "text",
                CanonicalValue::text(CanonicalText::new(text).unwrap()),
            )
            .unwrap(),
            CanonicalField::named(
                "enum",
                CanonicalValue::enum_token(EnumToken::new("future:token").unwrap()),
            )
            .unwrap(),
            CanonicalField::named(
                "unresolved",
                CanonicalValue::reference(
                    UnresolvedReference::new("metadata:object", "logical:target").unwrap(),
                ),
            )
            .unwrap(),
            CanonicalField::named(
                "record",
                CanonicalValue::record(vec![
                    CanonicalField::named("inside", CanonicalValue::boolean(false)).unwrap(),
                ])
                .unwrap(),
            )
            .unwrap(),
            CanonicalField::named(
                "sequence",
                CanonicalValue::sequence(vec![
                    CanonicalValue::null(),
                    CanonicalValue::boolean(true),
                ])
                .unwrap(),
            )
            .unwrap(),
            CanonicalField::named("binary", CanonicalValue::binary(inline).unwrap()).unwrap(),
            CanonicalField::named(
                "asset_reference",
                CanonicalValue::asset_reference(reference),
            )
            .unwrap(),
        ]
    }

    fn golden_objects(profile: &str, text: &str, opaque_bytes: &[u8]) -> Vec<CanonicalObject> {
        let parent_path = path("parent");
        let mut parent = CanonicalObjectParts::new(
            LogicalIdentity::new(uuid(1), parent_path.clone()),
            MetadataKind::new("Catalog").unwrap(),
            provenance(profile, &parent_path),
        );
        parent.properties = scalar_properties(text);
        parent.generated_types.push(GeneratedType::new(
            uuid(101),
            GeneratedTypeKind::new("ObjectType").unwrap(),
        ));
        parent.assets.push(
            Asset::from_bytes(vec![7, 8, 9], "image/x-semantic")
                .unwrap()
                .as_reference(),
        );
        parent.opaque_facets = opaque(profile, &parent_path, opaque_bytes);

        let child_path = path("child");
        let mut child = CanonicalObjectParts::new(
            LogicalIdentity::new(uuid(2), child_path.clone()),
            MetadataKind::new("CatalogAttribute").unwrap(),
            provenance(profile, &child_path),
        );
        child.owner = Some(uuid(1));
        child.references.push(ObjectReference::new(
            ReferenceKind::new("generated:type").unwrap(),
            uuid(101),
        ));
        vec![
            CanonicalObject::new(parent).unwrap(),
            CanonicalObject::new(child).unwrap(),
        ]
    }

    fn digest(objects: Vec<CanonicalObject>) -> SemanticDigest {
        let configuration = CanonicalConfiguration::new(objects).unwrap();
        validate_configuration(&configuration)
            .unwrap()
            .semantic_digest()
    }

    #[test]
    fn semantic_digest_has_cross_platform_golden_encoding() {
        let digest = digest(golden_objects("profile:one", "line1\r\nline2", b"opaque"));
        assert_eq!(
            digest.to_string(),
            "01d7eb3a3cf111c2d2ba32399852815d715c4849cd98aa6172e0f34408e3a1fc"
        );
        assert_eq!(serde_json::to_string(&digest).unwrap().len(), 66);
    }

    #[test]
    fn shuffled_object_insertion_is_semantically_equal() {
        let objects = golden_objects("profile:one", "text", b"opaque");
        let reversed = objects.iter().cloned().rev().collect::<Vec<_>>();
        let left_configuration = CanonicalConfiguration::new(objects).unwrap();
        let right_configuration = CanonicalConfiguration::new(reversed).unwrap();
        let left = validate_configuration(&left_configuration).unwrap();
        let right = validate_configuration(&right_configuration).unwrap();
        assert!(semantic_eq(&left, &right));
    }

    #[test]
    fn ordered_properties_and_exact_line_endings_change_digest() {
        let ordered = golden_objects("profile:one", "line1\r\nline2", b"opaque");
        let mut reordered = ordered.clone();
        let mut parent = CanonicalObjectParts::new(
            reordered[0].identity().clone(),
            reordered[0].kind().clone(),
            reordered[0].provenance().clone(),
        );
        parent.properties = reordered[0].properties().iter().cloned().rev().collect();
        parent.generated_types = reordered[0].generated_types().to_vec();
        parent.assets = reordered[0].assets().to_vec();
        parent.opaque_facets = reordered[0].opaque_facets().clone();
        reordered[0] = CanonicalObject::new(parent).unwrap();
        assert_ne!(digest(ordered.clone()), digest(reordered));

        let lf = golden_objects("profile:one", "line1\nline2", b"opaque");
        assert_ne!(digest(ordered), digest(lf));
    }

    #[test]
    fn properties_assets_and_opaque_payloads_all_affect_digest() {
        let baseline = golden_objects("profile:one", "text", b"opaque-a");
        let changed_property = golden_objects("profile:one", "changed", b"opaque-a");
        let changed_opaque = golden_objects("profile:one", "text", b"opaque-b");
        assert_ne!(digest(baseline.clone()), digest(changed_property));
        assert_ne!(digest(baseline.clone()), digest(changed_opaque));

        let mut changed_asset = baseline.clone();
        let mut parent = CanonicalObjectParts::new(
            changed_asset[0].identity().clone(),
            changed_asset[0].kind().clone(),
            changed_asset[0].provenance().clone(),
        );
        parent.properties = changed_asset[0].properties().to_vec();
        parent.generated_types = changed_asset[0].generated_types().to_vec();
        parent.assets.push(
            Asset::from_bytes(vec![9, 8, 7], "image/x-semantic")
                .unwrap()
                .as_reference(),
        );
        parent.opaque_facets = changed_asset[0].opaque_facets().clone();
        changed_asset[0] = CanonicalObject::new(parent).unwrap();
        assert_ne!(digest(baseline), digest(changed_asset));
    }

    #[test]
    fn source_profile_and_locator_are_non_semantic() {
        let first = golden_objects("profile:one", "text", b"opaque");
        let second = golden_objects("profile:two", "text", b"opaque");
        assert_eq!(digest(first), digest(second));
    }
}
