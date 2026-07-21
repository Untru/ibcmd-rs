//! Deterministic indexes over canonical metadata graphs.

use std::collections::{BTreeMap, btree_map::Entry};
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use crate::diagnostic::ObjectPath;
use crate::identity::ObjectUuid;
use crate::model::CanonicalConfiguration;

/// Stable address of a generated type within declared object/type order.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GeneratedTypeAddress {
    object_index: usize,
    generated_type_index: usize,
}

impl GeneratedTypeAddress {
    /// Returns the owning object's declared index.
    pub const fn object_index(self) -> usize {
        self.object_index
    }

    /// Returns the generated type's declared index within its object.
    pub const fn generated_type_index(self) -> usize {
        self.generated_type_index
    }
}

/// Stable address in the global object/generated-type UUID namespace.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GraphNodeAddress {
    /// Canonical object at the declared configuration index.
    Object {
        /// Declared object index.
        object_index: usize,
    },
    /// Generated type nested under an object.
    GeneratedType(GeneratedTypeAddress),
}

impl GraphNodeAddress {
    /// Returns the owning or direct object index.
    pub const fn object_index(self) -> usize {
        match self {
            Self::Object { object_index } => object_index,
            Self::GeneratedType(address) => address.object_index,
        }
    }
}

/// Failure to create deterministic graph indexes without overwriting a node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GraphIndexError {
    /// Two graph nodes claimed the same global UUID.
    DuplicateUuid {
        /// Conflicting UUID.
        uuid: ObjectUuid,
        /// Address retained by the first declaration.
        first: GraphNodeAddress,
        /// Address of the conflicting declaration.
        duplicate: GraphNodeAddress,
    },
    /// Two objects claimed the same exact logical path.
    DuplicateLogicalPath {
        /// Conflicting typed logical path.
        path: ObjectPath,
        /// First declared object index.
        first_object_index: usize,
        /// Conflicting object index.
        duplicate_object_index: usize,
    },
}

impl Display for GraphIndexError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateUuid {
                uuid,
                first,
                duplicate,
            } => write!(
                formatter,
                "duplicate graph UUID {uuid} at {duplicate:?}; first declared at {first:?}"
            ),
            Self::DuplicateLogicalPath {
                path,
                first_object_index,
                duplicate_object_index,
            } => write!(
                formatter,
                "duplicate logical path {path} at object {duplicate_object_index}; first declared at object {first_object_index}"
            ),
        }
    }
}

impl Error for GraphIndexError {}

/// Immutable deterministic indexes over a canonical configuration.
///
/// UUIDs use one namespace across objects and generated types. Construction
/// fails at the first collision instead of silently replacing a prior entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphIndex {
    object_by_uuid: BTreeMap<ObjectUuid, usize>,
    object_by_path: BTreeMap<ObjectPath, usize>,
    generated_type_by_uuid: BTreeMap<ObjectUuid, GeneratedTypeAddress>,
    node_by_uuid: BTreeMap<ObjectUuid, GraphNodeAddress>,
}

impl GraphIndex {
    /// Builds complete indexes or rejects duplicate UUID/path declarations.
    pub fn new(configuration: &CanonicalConfiguration) -> Result<Self, GraphIndexError> {
        let mut index = Self {
            object_by_uuid: BTreeMap::new(),
            object_by_path: BTreeMap::new(),
            generated_type_by_uuid: BTreeMap::new(),
            node_by_uuid: BTreeMap::new(),
        };

        for (object_index, object) in configuration.objects().iter().enumerate() {
            let uuid = object.identity().uuid();
            let address = GraphNodeAddress::Object { object_index };
            insert_uuid(&mut index.node_by_uuid, uuid, address)?;
            index.object_by_uuid.insert(uuid, object_index);

            match index.object_by_path.entry(object.identity().path().clone()) {
                Entry::Vacant(slot) => {
                    slot.insert(object_index);
                }
                Entry::Occupied(slot) => {
                    return Err(GraphIndexError::DuplicateLogicalPath {
                        path: slot.key().clone(),
                        first_object_index: *slot.get(),
                        duplicate_object_index: object_index,
                    });
                }
            }

            for (generated_type_index, generated_type) in
                object.generated_types().iter().enumerate()
            {
                let generated_address = GeneratedTypeAddress {
                    object_index,
                    generated_type_index,
                };
                insert_uuid(
                    &mut index.node_by_uuid,
                    generated_type.uuid(),
                    GraphNodeAddress::GeneratedType(generated_address),
                )?;
                index
                    .generated_type_by_uuid
                    .insert(generated_type.uuid(), generated_address);
            }
        }
        Ok(index)
    }

    /// Looks up an object by exact UUID.
    pub fn object_index_by_uuid(&self, uuid: ObjectUuid) -> Option<usize> {
        self.object_by_uuid.get(&uuid).copied()
    }

    /// Looks up an object by exact typed logical path.
    pub fn object_index_by_path(&self, path: &ObjectPath) -> Option<usize> {
        self.object_by_path.get(path).copied()
    }

    /// Looks up a generated type by exact UUID.
    pub fn generated_type_address(&self, uuid: ObjectUuid) -> Option<GeneratedTypeAddress> {
        self.generated_type_by_uuid.get(&uuid).copied()
    }

    /// Looks up either an object or generated type in the global UUID namespace.
    pub fn node_address(&self, uuid: ObjectUuid) -> Option<GraphNodeAddress> {
        self.node_by_uuid.get(&uuid).copied()
    }

    /// Returns whether an object UUID exists; generated types do not satisfy ownership.
    pub fn contains_object(&self, uuid: ObjectUuid) -> bool {
        self.object_by_uuid.contains_key(&uuid)
    }

    /// Returns whether an object or generated type can satisfy a semantic reference.
    pub fn contains_reference_target(&self, uuid: ObjectUuid) -> bool {
        self.node_by_uuid.contains_key(&uuid)
    }

    /// Returns the number of indexed objects.
    pub fn object_count(&self) -> usize {
        self.object_by_uuid.len()
    }

    /// Returns the number of indexed generated types.
    pub fn generated_type_count(&self) -> usize {
        self.generated_type_by_uuid.len()
    }
}

fn insert_uuid(
    index: &mut BTreeMap<ObjectUuid, GraphNodeAddress>,
    uuid: ObjectUuid,
    address: GraphNodeAddress,
) -> Result<(), GraphIndexError> {
    match index.entry(uuid) {
        Entry::Vacant(slot) => {
            slot.insert(address);
            Ok(())
        }
        Entry::Occupied(slot) => Err(GraphIndexError::DuplicateUuid {
            uuid,
            first: *slot.get(),
            duplicate: address,
        }),
    }
}

#[cfg(test)]
mod tests {
    use crate::artifact::ProfileId;
    use crate::diagnostic::{PathSegment, PropertyPath};
    use crate::identity::LogicalIdentity;
    use crate::model::{
        CanonicalObject, CanonicalObjectParts, GeneratedType, GeneratedTypeKind, MetadataKind,
    };
    use crate::provenance::{CanonicalAnchor, SourceProvenance};

    use super::*;

    fn uuid(suffix: u32) -> ObjectUuid {
        ObjectUuid::parse(&format!("00000000-0000-0000-0000-{suffix:012x}")).unwrap()
    }

    fn object(id: u32, path_name: &str, generated: &[u32]) -> CanonicalObject {
        let path = ObjectPath::new(vec![PathSegment::name(path_name).unwrap()]).unwrap();
        let provenance = SourceProvenance::new(
            ProfileId::parse("profile:test").unwrap(),
            CanonicalAnchor::new(path.clone(), PropertyPath::root()),
        );
        let mut parts = CanonicalObjectParts::new(
            LogicalIdentity::new(uuid(id), path),
            MetadataKind::new("Catalog").unwrap(),
            provenance,
        );
        parts.generated_types = generated
            .iter()
            .map(|id| GeneratedType::new(uuid(*id), GeneratedTypeKind::new("ObjectType").unwrap()))
            .collect();
        CanonicalObject::new(parts).unwrap()
    }

    #[test]
    fn valid_indexes_cover_objects_paths_and_generated_types() {
        let first = object(1, "first", &[101]);
        let second = object(2, "second", &[102]);
        let second_path = second.identity().path().clone();
        let configuration = CanonicalConfiguration::new(vec![first, second]).unwrap();
        let index = GraphIndex::new(&configuration).unwrap();
        assert_eq!(index.object_index_by_uuid(uuid(2)), Some(1));
        assert_eq!(index.object_index_by_path(&second_path), Some(1));
        assert_eq!(
            index.generated_type_address(uuid(101)),
            Some(GeneratedTypeAddress {
                object_index: 0,
                generated_type_index: 0
            })
        );
        assert_eq!(index.object_count(), 2);
        assert_eq!(index.generated_type_count(), 2);
    }

    #[test]
    fn duplicate_object_generated_and_cross_kind_uuids_never_overwrite() {
        let duplicate_object =
            CanonicalConfiguration::new(vec![object(1, "a", &[]), object(1, "b", &[])]).unwrap();
        assert!(matches!(
            GraphIndex::new(&duplicate_object),
            Err(GraphIndexError::DuplicateUuid { .. })
        ));

        let duplicate_generated =
            CanonicalConfiguration::new(vec![object(1, "a", &[9, 9])]).unwrap();
        assert!(matches!(
            GraphIndex::new(&duplicate_generated),
            Err(GraphIndexError::DuplicateUuid { .. })
        ));

        let cross_kind = CanonicalConfiguration::new(vec![object(1, "a", &[1])]).unwrap();
        assert!(matches!(
            GraphIndex::new(&cross_kind),
            Err(GraphIndexError::DuplicateUuid { .. })
        ));
    }

    #[test]
    fn duplicate_logical_path_is_rejected_without_last_wins() {
        let configuration =
            CanonicalConfiguration::new(vec![object(1, "same", &[]), object(2, "same", &[])])
                .unwrap();
        assert!(matches!(
            GraphIndex::new(&configuration),
            Err(GraphIndexError::DuplicateLogicalPath {
                first_object_index: 0,
                duplicate_object_index: 1,
                ..
            })
        ));
    }
}
