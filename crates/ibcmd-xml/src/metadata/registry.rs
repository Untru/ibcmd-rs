use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use ibcmd_core::artifact::ProfileId;
use ibcmd_core::diagnostic::ObjectPath;
use ibcmd_core::family::{FamilyId, MAX_FAMILY_CODECS};
use ibcmd_core::opaque::OpaqueEmitError;

use super::common::inspect_metadata_family;
use super::fallback::FallbackEmitError;
use super::{MetadataDecodeError, MetadataEnvelope, decode_metadata_envelope};
use crate::XmlDocument;

/// Object-safe extension point for one exact XML metadata family.
pub trait MetadataFamilyCodec: Send + Sync {
    fn family_id(&self) -> &FamilyId;
    fn decode(
        &self,
        document: &XmlDocument,
        source: ProfileId,
        path: ObjectPath,
    ) -> Result<MetadataEnvelope, MetadataDecodeError>;
    fn encode(
        &self,
        envelope: &MetadataEnvelope,
        target: &ProfileId,
    ) -> Result<Vec<u8>, MetadataEncodeError>;
}

#[derive(Default)]
pub struct MetadataRegistry {
    codecs: BTreeMap<FamilyId, Box<dyn MetadataFamilyCodec>>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MetadataRegistryError {
    DuplicateFamily(FamilyId),
    TooManyCodecs { maximum: usize },
}
impl Display for MetadataRegistryError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl Error for MetadataRegistryError {}
impl MetadataRegistry {
    pub fn register(
        &mut self,
        codec: Box<dyn MetadataFamilyCodec>,
    ) -> Result<(), MetadataRegistryError> {
        let family = codec.family_id().clone();
        if self.codecs.contains_key(&family) {
            return Err(MetadataRegistryError::DuplicateFamily(family));
        }
        if self.codecs.len() == MAX_FAMILY_CODECS {
            return Err(MetadataRegistryError::TooManyCodecs {
                maximum: MAX_FAMILY_CODECS,
            });
        }
        self.codecs.insert(family, codec);
        Ok(())
    }
    pub fn contains(&self, family: &FamilyId) -> bool {
        self.codecs.contains_key(family)
    }
    pub fn len(&self) -> usize {
        self.codecs.len()
    }
    pub fn is_empty(&self) -> bool {
        self.codecs.is_empty()
    }
    pub fn decode(
        &self,
        family: &FamilyId,
        document: &XmlDocument,
        source: ProfileId,
        path: ObjectPath,
    ) -> Result<MetadataEnvelope, MetadataDecodeError> {
        let actual = inspect_metadata_family(document)?;
        if &actual != family {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "declared family differs from metadata object",
            ));
        }
        match self.codecs.get(family) {
            Some(codec) => codec.decode(document, source, path),
            None => decode_metadata_envelope(document, source, path),
        }
    }
    pub fn encode(
        &self,
        envelope: &MetadataEnvelope,
        target: &ProfileId,
    ) -> Result<Vec<u8>, MetadataEncodeError> {
        let family = FamilyId::parse(envelope.root().kind().as_str())
            .map_err(|x| MetadataEncodeError::Xml(x.to_string()))?;
        match self.codecs.get(&family) {
            Some(codec) => codec.encode(envelope, target),
            None => envelope.emit(target),
        }
    }
}
#[derive(Debug)]
pub enum MetadataEncodeError {
    Opaque(OpaqueEmitError),
    ModelChanged {
        object_path: ObjectPath,
    },
    Xml(String),
    UnsupportedProfile {
        object_path: ObjectPath,
        profile: ProfileId,
    },
    ProfileVersionMismatch {
        object_path: ObjectPath,
    },
    InvalidModel {
        object_path: ObjectPath,
        field: &'static str,
    },
}
impl From<FallbackEmitError> for MetadataEncodeError {
    fn from(value: FallbackEmitError) -> Self {
        match value {
            FallbackEmitError::Write(x) => Self::Xml(x.to_string()),
        }
    }
}
impl Display for MetadataEncodeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl Error for MetadataEncodeError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::XmlReader;
    use ibcmd_core::diagnostic::{ObjectPath, PathSegment};
    use ibcmd_core::model::{CanonicalObject, CanonicalObjectParts, MetadataKind};
    use ibcmd_core::value::{CanonicalField, CanonicalText, CanonicalValue};

    struct Mock {
        family: FamilyId,
    }
    impl MetadataFamilyCodec for Mock {
        fn family_id(&self) -> &FamilyId {
            &self.family
        }
        fn decode(
            &self,
            document: &XmlDocument,
            source: ProfileId,
            path: ObjectPath,
        ) -> Result<MetadataEnvelope, MetadataDecodeError> {
            let generic = decode_metadata_envelope(document, source, path)?;
            let mut parts = CanonicalObjectParts::new(
                generic.root().identity().clone(),
                MetadataKind::new("X").unwrap(),
                generic.root().provenance().clone(),
            );
            parts.properties.push(
                CanonicalField::named(
                    "codec",
                    CanonicalValue::text(CanonicalText::new("mock").unwrap()),
                )
                .unwrap(),
            );
            parts.opaque_facets = generic.root().opaque_facets().clone();
            MetadataEnvelope::from_parts(
                CanonicalObject::new(parts).unwrap(),
                Vec::new(),
                document.clone(),
            )
        }
        fn encode(
            &self,
            _: &MetadataEnvelope,
            _: &ProfileId,
        ) -> Result<Vec<u8>, MetadataEncodeError> {
            Ok(b"mock-codec".to_vec())
        }
    }
    fn envelope() -> MetadataEnvelope {
        let document = XmlReader::from_slice(b"<MetaDataObject><X uuid='11111111-1111-4111-8111-111111111111'><Properties><Name>X</Name></Properties></X></MetaDataObject>").unwrap();
        decode_metadata_envelope(
            &document,
            ProfileId::parse("xml:2.20").unwrap(),
            ObjectPath::new(vec![PathSegment::name("x").unwrap()]).unwrap(),
        )
        .unwrap()
    }
    #[test]
    fn registered_codec_changes_route() {
        let mut registry = MetadataRegistry::default();
        registry
            .register(Box::new(Mock {
                family: FamilyId::parse("X").unwrap(),
            }))
            .unwrap();
        assert_eq!(
            registry
                .encode(&envelope(), &ProfileId::parse("xml:2.21").unwrap())
                .unwrap(),
            b"mock-codec"
        );
    }
    #[test]
    fn duplicate_codec_is_rejected_without_replacement() {
        let mut registry = MetadataRegistry::default();
        registry
            .register(Box::new(Mock {
                family: FamilyId::parse("X").unwrap(),
            }))
            .unwrap();
        assert!(matches!(
            registry.register(Box::new(Mock {
                family: FamilyId::parse("X").unwrap()
            })),
            Err(MetadataRegistryError::DuplicateFamily(_))
        ));
        assert_eq!(registry.len(), 1);
    }
    #[test]
    fn registered_decode_can_construct_checked_family_ir() {
        let mut registry = MetadataRegistry::default();
        let family = FamilyId::parse("X").unwrap();
        registry
            .register(Box::new(Mock {
                family: family.clone(),
            }))
            .unwrap();
        let document = XmlReader::from_slice(b"<MetaDataObject><X uuid='11111111-1111-4111-8111-111111111111'><Properties><Name>X</Name></Properties></X></MetaDataObject>").unwrap();
        let decoded = registry
            .decode(
                &family,
                &document,
                ProfileId::parse("xml-2.20").unwrap(),
                ObjectPath::root(),
            )
            .unwrap();
        assert_eq!(decoded.root().properties()[0].name().as_str(), "codec");
        assert_eq!(decoded.source_document(), &document);
        assert!(!decoded.source_model_unchanged());
        assert_eq!(
            registry
                .encode(&decoded, &ProfileId::parse("xml-2.20").unwrap())
                .unwrap(),
            b"mock-codec"
        );
    }
    #[test]
    fn public_model_construction_disables_unregistered_fallback() {
        let decoded = envelope();
        let expected_path = decoded.root().identity().path().clone();
        let rebuilt = MetadataEnvelope::from_parts(
            decoded.root().clone(),
            decoded.descendants().to_vec(),
            decoded.source_document().clone(),
        )
        .unwrap();
        assert!(!rebuilt.source_model_unchanged());
        assert!(matches!(
            MetadataRegistry::default().encode(&rebuilt, &ProfileId::parse("xml:2.20").unwrap()),
            Err(MetadataEncodeError::ModelChanged { object_path }) if object_path == expected_path
        ));

        let decoded = envelope();
        let root = decoded.root().clone();
        let descendants = decoded.descendants().to_vec();
        let rebuilt = decoded.with_model(root, descendants).unwrap();
        assert!(!rebuilt.source_model_unchanged());
        assert!(matches!(
            MetadataRegistry::default().encode(&rebuilt, &ProfileId::parse("xml:2.20").unwrap()),
            Err(MetadataEncodeError::ModelChanged { object_path }) if object_path == expected_path
        ));
    }
    #[test]
    fn wrong_declared_family_never_invokes_codec() {
        let mut registry = MetadataRegistry::default();
        registry
            .register(Box::new(Mock {
                family: FamilyId::parse("Wrong").unwrap(),
            }))
            .unwrap();
        let document = XmlReader::from_slice(b"<MetaDataObject><X uuid='11111111-1111-4111-8111-111111111111'><Properties><Name>X</Name></Properties></X></MetaDataObject>").unwrap();
        assert!(matches!(
            registry.decode(
                &FamilyId::parse("Wrong").unwrap(),
                &document,
                ProfileId::parse("xml-2.20").unwrap(),
                ObjectPath::root()
            ),
            Err(MetadataDecodeError::InvalidEnvelope(_))
        ));
    }
    #[test]
    fn registry_enforces_core_codec_bound() {
        let mut registry = MetadataRegistry::default();
        for index in 0..MAX_FAMILY_CODECS {
            registry
                .register(Box::new(Mock {
                    family: FamilyId::parse(&format!("f{index}")).unwrap(),
                }))
                .unwrap();
        }
        assert!(matches!(
            registry.register(Box::new(Mock {
                family: FamilyId::parse("overflow").unwrap()
            })),
            Err(MetadataRegistryError::TooManyCodecs {
                maximum: MAX_FAMILY_CODECS
            })
        ));
    }
}
