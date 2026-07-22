//! Bootstrap identities projected from the validated canonical graph.
//!
//! This module deliberately reuses `ibcmd-core` UUIDs, generated types and
//! validation proof. It does not introduce a second semantic graph.

use std::error::Error;
use std::fmt::{self, Display, Formatter, Write as _};

use ibcmd_core::identity::ObjectUuid;
use ibcmd_core::model::{GeneratedType, MetadataKind};
use ibcmd_core::validate::ValidatedConfiguration;
use sha2::{Digest, Sha256};

/// Failure to project storage identities from an encode-ready model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BootstrapIdentityError {
    /// No canonical object identifies the configuration root.
    MissingConfiguration,
    /// More than one canonical object identifies itself as the configuration.
    MultipleConfigurations {
        /// UUID of the first configuration object.
        first: ObjectUuid,
        /// UUID of the conflicting configuration object.
        duplicate: ObjectUuid,
    },
    /// The nil UUID cannot name a native metadata entry.
    NilObjectUuid {
        /// Declared object kind.
        kind: String,
    },
    /// The nil UUID cannot participate in the generated-type namespace.
    NilGeneratedTypeUuid {
        /// UUID of the owning canonical object.
        owner: ObjectUuid,
        /// Open generated-type role.
        kind: String,
    },
}

impl Display for BootstrapIdentityError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingConfiguration => {
                formatter.write_str("bootstrap graph has no Configuration object")
            }
            Self::MultipleConfigurations { first, duplicate } => write!(
                formatter,
                "bootstrap graph has multiple Configuration objects: {first} and {duplicate}"
            ),
            Self::NilObjectUuid { kind } => {
                write!(formatter, "bootstrap {kind} object uses the nil UUID")
            }
            Self::NilGeneratedTypeUuid { owner, kind } => write!(
                formatter,
                "bootstrap generated type {kind} owned by {owner} uses the nil UUID"
            ),
        }
    }
}

impl Error for BootstrapIdentityError {}

/// A deterministic generation identity used by the native `versions` map.
///
/// This is intentionally distinct from [`ObjectUuid`]: a generation identifies
/// exact compiled bytes, not a metadata object in the canonical graph.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GenerationUuid(ObjectUuid);

impl GenerationUuid {
    /// Returns the exact 16 UUID bytes.
    pub const fn as_bytes(&self) -> &[u8; 16] {
        self.0.as_bytes()
    }
}

impl Display for GenerationUuid {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, formatter)
    }
}

/// One canonical object and its generated-type identities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootstrapObjectIdentity {
    uuid: ObjectUuid,
    kind: MetadataKind,
    owner: Option<ObjectUuid>,
    generated_types: Vec<GeneratedType>,
    source_index: usize,
}

impl BootstrapObjectIdentity {
    /// Returns the exact metadata-object UUID.
    pub const fn uuid(&self) -> ObjectUuid {
        self.uuid
    }

    /// Returns the exact open metadata family.
    pub const fn kind(&self) -> &MetadataKind {
        &self.kind
    }

    /// Returns the owning object UUID, when one was declared.
    pub const fn owner(&self) -> Option<ObjectUuid> {
        self.owner
    }

    /// Returns generated types in canonical declaration order.
    pub fn generated_types(&self) -> &[GeneratedType] {
        &self.generated_types
    }

    /// Returns the object's index in the validated canonical configuration.
    pub const fn source_index(&self) -> usize {
        self.source_index
    }
}

/// Deterministic identity projection used by the bootstrap storage graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootstrapIdentities {
    configuration_uuid: ObjectUuid,
    objects: Vec<BootstrapObjectIdentity>,
}

impl BootstrapIdentities {
    /// Returns the single canonical Configuration UUID.
    pub const fn configuration_uuid(&self) -> ObjectUuid {
        self.configuration_uuid
    }

    /// Returns objects in UUID order, independent of source enumeration order.
    pub fn objects(&self) -> &[BootstrapObjectIdentity] {
        &self.objects
    }

    /// Looks up an object by exact UUID.
    pub fn object(&self, uuid: ObjectUuid) -> Option<&BootstrapObjectIdentity> {
        self.objects
            .binary_search_by_key(&uuid, BootstrapObjectIdentity::uuid)
            .ok()
            .map(|index| &self.objects[index])
    }

    /// Returns whether a generated type is present in the validated namespace.
    pub fn contains_generated_type(&self, uuid: ObjectUuid) -> bool {
        self.objects.iter().any(|object| {
            object
                .generated_types
                .iter()
                .any(|item| item.uuid() == uuid)
        })
    }

    /// Returns the complete generated-type count.
    pub fn generated_type_count(&self) -> usize {
        self.objects
            .iter()
            .map(|object| object.generated_types.len())
            .sum()
    }
}

/// Projects bootstrap identities from a graph that already passed core
/// duplicate, ownership, reference and cycle validation.
pub fn collect_bootstrap_identities(
    validated: &ValidatedConfiguration<'_>,
) -> Result<BootstrapIdentities, BootstrapIdentityError> {
    let mut objects = Vec::with_capacity(validated.configuration().objects().len());
    let mut configuration_uuid = None;

    for (source_index, object) in validated.configuration().objects().iter().enumerate() {
        let uuid = object.identity().uuid();
        if is_nil(uuid) {
            return Err(BootstrapIdentityError::NilObjectUuid {
                kind: object.kind().as_str().to_owned(),
            });
        }
        if object.kind().as_str() == "Configuration" {
            if let Some(first) = configuration_uuid.replace(uuid) {
                return Err(BootstrapIdentityError::MultipleConfigurations {
                    first,
                    duplicate: uuid,
                });
            }
        }
        for generated_type in object.generated_types() {
            if is_nil(generated_type.uuid()) {
                return Err(BootstrapIdentityError::NilGeneratedTypeUuid {
                    owner: uuid,
                    kind: generated_type.kind().as_str().to_owned(),
                });
            }
        }
        objects.push(BootstrapObjectIdentity {
            uuid,
            kind: object.kind().clone(),
            owner: object.owner(),
            generated_types: object.generated_types().to_vec(),
            source_index,
        });
    }

    let configuration_uuid =
        configuration_uuid.ok_or(BootstrapIdentityError::MissingConfiguration)?;
    objects.sort_by_key(BootstrapObjectIdentity::uuid);
    Ok(BootstrapIdentities {
        configuration_uuid,
        objects,
    })
}

fn is_nil(uuid: ObjectUuid) -> bool {
    uuid.as_bytes().iter().all(|byte| *byte == 0)
}

/// Derives an RFC 9562 UUIDv8 from domain-separated, length-prefixed fields.
///
/// UUIDv8 is used deliberately: generation identifiers are stable storage
/// identities, not vendor object UUIDs and not random UUIDv4 values.
pub(crate) fn derive_generation_uuid_v8(domain: &[u8], fields: &[&[u8]]) -> GenerationUuid {
    let mut hasher = Sha256::new();
    hasher.update(b"ibcmd-bootstrap-derived-uuid-v1\0");
    hash_field(&mut hasher, domain);
    for field in fields {
        hash_field(&mut hasher, field);
    }
    let digest = hasher.finalize();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x80;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;

    let mut text = String::with_capacity(36);
    for (index, byte) in bytes.iter().enumerate() {
        if matches!(index, 4 | 6 | 8 | 10) {
            text.push('-');
        }
        write!(&mut text, "{byte:02x}").expect("writing to String cannot fail");
    }
    GenerationUuid(ObjectUuid::parse(&text).expect("derived UUID text is canonical"))
}

fn hash_field(hasher: &mut Sha256, field: &[u8]) {
    let length = u64::try_from(field.len()).expect("slice length fits into u64");
    hasher.update(length.to_le_bytes());
    hasher.update(field);
}

#[cfg(test)]
mod tests {
    use ibcmd_core::artifact::ProfileId;
    use ibcmd_core::diagnostic::{ObjectPath, PathSegment, PropertyPath};
    use ibcmd_core::identity::LogicalIdentity;
    use ibcmd_core::model::{
        CanonicalConfiguration, CanonicalObject, CanonicalObjectParts, GeneratedTypeKind,
    };
    use ibcmd_core::provenance::{CanonicalAnchor, SourceProvenance};
    use ibcmd_core::validate::validate_configuration;

    use super::*;

    fn uuid(value: u32) -> ObjectUuid {
        ObjectUuid::parse(&format!("00000000-0000-4000-8000-{value:012x}")).unwrap()
    }

    fn object(value: u32, kind: &str, generated: &[u32]) -> CanonicalObject {
        let path =
            ObjectPath::new(vec![PathSegment::name(&format!("object-{value}")).unwrap()]).unwrap();
        let provenance = SourceProvenance::new(
            ProfileId::parse("profile:test").unwrap(),
            CanonicalAnchor::new(path.clone(), PropertyPath::root()),
        );
        let mut parts = CanonicalObjectParts::new(
            LogicalIdentity::new(uuid(value), path),
            MetadataKind::new(kind).unwrap(),
            provenance,
        );
        parts.generated_types = generated
            .iter()
            .map(|value| {
                GeneratedType::new(uuid(*value), GeneratedTypeKind::new("generated").unwrap())
            })
            .collect();
        CanonicalObject::new(parts).unwrap()
    }

    #[test]
    fn identity_projection_is_uuid_ordered_and_retains_source_indexes() {
        let configuration = CanonicalConfiguration::new(vec![
            object(3, "Catalog", &[103]),
            object(1, "Configuration", &[]),
            object(2, "Constant", &[102]),
        ])
        .unwrap();
        let validated = validate_configuration(&configuration).unwrap();
        let identities = collect_bootstrap_identities(&validated).unwrap();

        assert_eq!(identities.configuration_uuid(), uuid(1));
        assert_eq!(
            identities
                .objects()
                .iter()
                .map(BootstrapObjectIdentity::uuid)
                .collect::<Vec<_>>(),
            vec![uuid(1), uuid(2), uuid(3)]
        );
        assert_eq!(identities.object(uuid(3)).unwrap().source_index(), 0);
        assert!(identities.contains_generated_type(uuid(102)));
        assert_eq!(identities.generated_type_count(), 2);
    }

    #[test]
    fn identity_projection_requires_exactly_one_non_nil_configuration() {
        let missing = CanonicalConfiguration::new(vec![object(2, "Catalog", &[])]).unwrap();
        assert_eq!(
            collect_bootstrap_identities(&validate_configuration(&missing).unwrap()),
            Err(BootstrapIdentityError::MissingConfiguration)
        );

        let duplicate = CanonicalConfiguration::new(vec![
            object(1, "Configuration", &[]),
            object(2, "Configuration", &[]),
        ])
        .unwrap();
        assert!(matches!(
            collect_bootstrap_identities(&validate_configuration(&duplicate).unwrap()),
            Err(BootstrapIdentityError::MultipleConfigurations { .. })
        ));
    }

    #[test]
    fn derived_ids_are_stable_domain_separated_uuid_v8_values() {
        let first = derive_generation_uuid_v8(b"entry", &[b"a", b"bc"]);
        let repeated = derive_generation_uuid_v8(b"entry", &[b"a", b"bc"]);
        let regrouped = derive_generation_uuid_v8(b"entry", &[b"ab", b"c"]);
        let other_domain = derive_generation_uuid_v8(b"row", &[b"a", b"bc"]);

        assert_eq!(first, repeated);
        assert_ne!(first, regrouped);
        assert_ne!(first, other_domain);
        assert_eq!(first.as_bytes()[6] >> 4, 8);
        assert_eq!(first.as_bytes()[8] >> 6, 2);
    }
}
