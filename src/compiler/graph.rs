//! Native storage inventory projected from bootstrap identities.

use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use ibcmd_core::artifact::ProfileId;
use ibcmd_core::identity::ObjectUuid;
use ibcmd_core::storage::{
    MAX_STORAGE_PATCH_ENTRIES, StorageBuildError, StorageKey, StoragePatch, StoragePatchOutcome,
};

use super::identity::BootstrapIdentities;

/// Maximum accepted byte length of a native object-entry suffix.
pub const MAX_STORAGE_SUFFIX_BYTES: usize = 16;

/// A proven service entry in the configuration storage inventory.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SpecialEntryKind {
    /// Entry that points to the Configuration metadata body.
    Root,
    /// Entry that selects the native container compatibility layout.
    Version,
    /// Complete FileName-to-generation UUID map.
    Versions,
}

impl SpecialEntryKind {
    /// Returns the exact reserved native storage key.
    pub const fn key(self) -> &'static str {
        match self {
            Self::Root => "root",
            Self::Version => "version",
            Self::Versions => "versions",
        }
    }
}

/// A validated suffix such as `.0`, `.a`, `.10`, or `.1c`.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StorageSuffix(Box<str>);

impl StorageSuffix {
    /// Validates an exact lower-hex native suffix.
    pub fn new(value: &str) -> Result<Self, BootstrapGraphError> {
        let bytes = value.as_bytes();
        if bytes.len() < 2
            || bytes.len() > MAX_STORAGE_SUFFIX_BYTES
            || bytes[0] != b'.'
            || !bytes[1..]
                .iter()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
        {
            return Err(BootstrapGraphError::InvalidSuffix(value.to_owned()));
        }
        Ok(Self(value.into()))
    }

    /// Returns the exact suffix text including the leading dot.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Additional physical entries emitted for one canonical object.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjectStorageRoute {
    object_uuid: ObjectUuid,
    suffixes: Vec<StorageSuffix>,
}

impl ObjectStorageRoute {
    /// Creates an input-order-independent route and rejects duplicate suffixes.
    pub fn new(
        object_uuid: ObjectUuid,
        mut suffixes: Vec<StorageSuffix>,
    ) -> Result<Self, BootstrapGraphError> {
        if suffixes.len() > MAX_STORAGE_PATCH_ENTRIES {
            return Err(BootstrapGraphError::TooManySuffixes {
                object: object_uuid,
                maximum: MAX_STORAGE_PATCH_ENTRIES,
                actual: suffixes.len(),
            });
        }
        suffixes.sort();
        for pair in suffixes.windows(2) {
            if pair[0] == pair[1] {
                return Err(BootstrapGraphError::DuplicateSuffix {
                    object: object_uuid,
                    suffix: pair[0].as_str().to_owned(),
                });
            }
        }
        Ok(Self {
            object_uuid,
            suffixes,
        })
    }

    /// Returns the routed object UUID.
    pub const fn object_uuid(&self) -> ObjectUuid {
        self.object_uuid
    }

    /// Returns extra suffixes in canonical lexical order.
    pub fn suffixes(&self) -> &[StorageSuffix] {
        &self.suffixes
    }
}

/// Semantic owner of one expected native storage key.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BootstrapStorageOwner {
    /// Reserved service entry.
    Special(SpecialEntryKind),
    /// Primary or suffixed entry owned by a canonical metadata object.
    Object {
        /// Owning metadata UUID.
        uuid: ObjectUuid,
        /// `None` for the primary metadata body, otherwise an exact suffix.
        suffix: Option<StorageSuffix>,
    },
}

/// One resolved entry in the exact bootstrap storage inventory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootstrapStorageIdentity {
    key: StorageKey,
    owner: BootstrapStorageOwner,
}

impl BootstrapStorageIdentity {
    /// Returns the exact native logical key.
    pub const fn key(&self) -> &StorageKey {
        &self.key
    }

    /// Returns the semantic contributor that owns the key.
    pub const fn owner(&self) -> &BootstrapStorageOwner {
        &self.owner
    }
}

/// Scope used while checking a compiled patch against the graph inventory.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InventoryScope {
    /// Every graph key including `versions` must be present.
    Complete,
    /// Every key except `versions` must be present.
    BeforeVersions,
}

/// Failure to construct or verify a bootstrap storage graph.
#[derive(Debug)]
pub enum BootstrapGraphError {
    /// A suffix does not use the exact native lower-hex grammar.
    InvalidSuffix(String),
    /// One object route declares a suffix more than once.
    DuplicateSuffix { object: ObjectUuid, suffix: String },
    /// One route alone exceeds the complete neutral patch entry bound.
    TooManySuffixes {
        object: ObjectUuid,
        maximum: usize,
        actual: usize,
    },
    /// More than one route was supplied for an object.
    DuplicateRoute { object: ObjectUuid },
    /// A route names an object absent from the validated graph.
    UnknownRouteObject { object: ObjectUuid },
    /// A route names a canonical descendant embedded in its owner's row.
    EmbeddedObjectRoute {
        object: ObjectUuid,
        owner: ObjectUuid,
    },
    /// A top-level canonical object has no explicit physical-row declaration.
    MissingObjectRoute { object: ObjectUuid },
    /// Two semantic contributors resolved to the same storage key.
    DuplicateStorageKey { key: String },
    /// The exact inventory exceeds the neutral patch entry bound.
    TooManyEntries { maximum: usize, actual: usize },
    /// A storage component rejected a derived key.
    Storage(StorageBuildError),
    /// The patch declares multipart entries, which the current versions layout
    /// does not model.
    MultipartUnsupported { key: String, part_count: u32 },
    /// The patch and graph inventory differ.
    InventoryMismatch {
        missing: Vec<String>,
        extra: Vec<String>,
    },
    /// An expected entry has no compiled payload.
    EntryBlocked { key: String },
    /// A required special-entry reference cannot be resolved.
    DanglingSpecialReference { key: String },
}

impl Display for BootstrapGraphError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSuffix(value) => write!(
                formatter,
                "invalid bootstrap storage suffix `{value}`; expected dot plus lowercase hexadecimal digits"
            ),
            Self::DuplicateSuffix { object, suffix } => {
                write!(
                    formatter,
                    "object {object} declares duplicate suffix {suffix}"
                )
            }
            Self::TooManySuffixes {
                object,
                maximum,
                actual,
            } => write!(
                formatter,
                "object {object} declares {actual} suffixes, exceeding the {maximum}-entry patch bound"
            ),
            Self::DuplicateRoute { object } => {
                write!(formatter, "object {object} has multiple storage routes")
            }
            Self::UnknownRouteObject { object } => {
                write!(
                    formatter,
                    "storage route references unknown object {object}"
                )
            }
            Self::EmbeddedObjectRoute { object, owner } => write!(
                formatter,
                "storage route references embedded object {object} owned by {owner}"
            ),
            Self::MissingObjectRoute { object } => {
                write!(formatter, "top-level object {object} has no storage route")
            }
            Self::DuplicateStorageKey { key } => {
                write!(
                    formatter,
                    "bootstrap contributors collide at storage key `{key}`"
                )
            }
            Self::TooManyEntries { maximum, actual } => write!(
                formatter,
                "bootstrap inventory exceeds {maximum} entries (actual {actual})"
            ),
            Self::Storage(source) => write!(formatter, "invalid bootstrap storage key: {source}"),
            Self::MultipartUnsupported { key, part_count } => write!(
                formatter,
                "bootstrap versions layout does not support {part_count}-part entry `{key}`"
            ),
            Self::InventoryMismatch { missing, extra } => write!(
                formatter,
                "bootstrap patch inventory mismatch: missing [{}], extra [{}]",
                missing.join(", "),
                extra.join(", ")
            ),
            Self::EntryBlocked { key } => {
                write!(formatter, "bootstrap entry `{key}` is not compiled")
            }
            Self::DanglingSpecialReference { key } => {
                write!(
                    formatter,
                    "bootstrap special entry references missing key `{key}`"
                )
            }
        }
    }
}

impl Error for BootstrapGraphError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Storage(source) => Some(source),
            _ => None,
        }
    }
}

impl From<StorageBuildError> for BootstrapGraphError {
    fn from(source: StorageBuildError) -> Self {
        Self::Storage(source)
    }
}

/// Deterministic native-entry inventory over a validated canonical graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootstrapGraph {
    profile_id: ProfileId,
    configuration_uuid: ObjectUuid,
    entries: Vec<BootstrapStorageIdentity>,
    generated_types: BTreeMap<ObjectUuid, ObjectUuid>,
}

impl BootstrapGraph {
    /// Returns the exact target profile used to resolve physical routes.
    pub const fn profile_id(&self) -> &ProfileId {
        &self.profile_id
    }

    /// Returns the Configuration metadata UUID referenced by `root`.
    pub const fn configuration_uuid(&self) -> ObjectUuid {
        self.configuration_uuid
    }

    /// Returns storage identities in exact lexical key order.
    pub fn entries(&self) -> &[BootstrapStorageIdentity] {
        &self.entries
    }

    /// Returns whether an exact logical key belongs to the inventory.
    pub fn contains_key(&self, key: &str) -> bool {
        self.entries
            .binary_search_by(|entry| entry.key.as_str().cmp(key))
            .is_ok()
    }

    /// Returns the canonical key sequence used by the `versions` compiler.
    pub fn inventory_keys(&self) -> impl ExactSizeIterator<Item = &StorageKey> {
        self.entries.iter().map(BootstrapStorageIdentity::key)
    }

    /// Resolves the exact unsuffixed physical row owned by a top-level object.
    ///
    /// Family compilers use this lookup instead of reconstructing storage keys
    /// from UUID text, so their targets cannot drift from the validated graph.
    pub fn primary_object_entry(&self, uuid: ObjectUuid) -> Option<&BootstrapStorageIdentity> {
        self.entries.iter().find(|entry| {
            matches!(
                entry.owner(),
                BootstrapStorageOwner::Object {
                    uuid: owner,
                    suffix: None,
                } if *owner == uuid
            )
        })
    }

    /// Resolves a generated type to its owning canonical object.
    pub fn generated_type_owner(&self, uuid: ObjectUuid) -> Option<ObjectUuid> {
        self.generated_types.get(&uuid).copied()
    }

    /// Verifies that all service references have exact storage targets.
    pub fn validate_special_references(&self) -> Result<(), BootstrapGraphError> {
        for key in [
            self.configuration_uuid.to_string(),
            SpecialEntryKind::Root.key().to_owned(),
            SpecialEntryKind::Version.key().to_owned(),
            SpecialEntryKind::Versions.key().to_owned(),
        ] {
            if !self.contains_key(&key) {
                return Err(BootstrapGraphError::DanglingSpecialReference { key });
            }
        }
        Ok(())
    }

    /// Checks exact key coverage, single-part identity and compiled outcomes.
    pub fn validate_patch_inventory(
        &self,
        patch: &StoragePatch,
        scope: InventoryScope,
    ) -> Result<(), BootstrapGraphError> {
        let expected = self
            .entries
            .iter()
            .filter(|entry| {
                scope == InventoryScope::Complete
                    || entry.key.as_str() != SpecialEntryKind::Versions.key()
            })
            .map(|entry| entry.key.as_str())
            .collect::<BTreeSet<_>>();
        let mut actual = BTreeSet::new();
        for entry in patch.entries() {
            let target = entry.target();
            if target.multipart().part_count() != 1 {
                return Err(BootstrapGraphError::MultipartUnsupported {
                    key: target.key().as_str().to_owned(),
                    part_count: target.multipart().part_count(),
                });
            }
            actual.insert(target.key().as_str());
        }

        if expected != actual {
            return Err(BootstrapGraphError::InventoryMismatch {
                missing: expected
                    .difference(&actual)
                    .map(|key| (*key).to_owned())
                    .collect(),
                extra: actual
                    .difference(&expected)
                    .map(|key| (*key).to_owned())
                    .collect(),
            });
        }
        for entry in patch.entries() {
            if !matches!(entry.outcome(), StoragePatchOutcome::Compiled(_)) {
                return Err(BootstrapGraphError::EntryBlocked {
                    key: entry.target().key().as_str().to_owned(),
                });
            }
        }
        Ok(())
    }
}

/// Builds primary object entries, explicit suffixed entries and all three
/// reserved service entries. No suffix is inferred from an object family.
pub fn build_bootstrap_graph(
    identities: &BootstrapIdentities,
    profile_id: ProfileId,
    routes: Vec<ObjectStorageRoute>,
) -> Result<BootstrapGraph, BootstrapGraphError> {
    let mut routes_by_object = BTreeMap::new();
    for route in routes {
        let object = identities.object(route.object_uuid).ok_or(
            BootstrapGraphError::UnknownRouteObject {
                object: route.object_uuid,
            },
        )?;
        if let Some(owner) = object.owner() {
            return Err(BootstrapGraphError::EmbeddedObjectRoute {
                object: route.object_uuid,
                owner,
            });
        }
        match routes_by_object.entry(route.object_uuid) {
            Entry::Vacant(slot) => {
                slot.insert(route);
            }
            Entry::Occupied(_) => {
                return Err(BootstrapGraphError::DuplicateRoute {
                    object: route.object_uuid,
                });
            }
        }
    }

    let mut entries = BTreeMap::<String, BootstrapStorageIdentity>::new();
    for kind in [
        SpecialEntryKind::Root,
        SpecialEntryKind::Version,
        SpecialEntryKind::Versions,
    ] {
        insert_entry(
            &mut entries,
            kind.key().to_owned(),
            BootstrapStorageOwner::Special(kind),
        )?;
    }

    let mut generated_types = BTreeMap::new();
    for object in identities.objects() {
        let uuid = object.uuid();
        if object.owner().is_none() {
            let route = routes_by_object
                .get(&uuid)
                .ok_or(BootstrapGraphError::MissingObjectRoute { object: uuid })?;
            insert_entry(
                &mut entries,
                uuid.to_string(),
                BootstrapStorageOwner::Object { uuid, suffix: None },
            )?;
            for suffix in route.suffixes() {
                insert_entry(
                    &mut entries,
                    format!("{uuid}{}", suffix.as_str()),
                    BootstrapStorageOwner::Object {
                        uuid,
                        suffix: Some(suffix.clone()),
                    },
                )?;
            }
        }
        for generated_type in object.generated_types() {
            generated_types.insert(generated_type.uuid(), uuid);
        }
    }

    let graph = BootstrapGraph {
        profile_id,
        configuration_uuid: identities.configuration_uuid(),
        entries: entries.into_values().collect(),
        generated_types,
    };
    graph.validate_special_references()?;
    Ok(graph)
}

fn insert_entry(
    entries: &mut BTreeMap<String, BootstrapStorageIdentity>,
    key: String,
    owner: BootstrapStorageOwner,
) -> Result<(), BootstrapGraphError> {
    let storage_key = StorageKey::new(&key)?;
    if entries.contains_key(&key) {
        return Err(BootstrapGraphError::DuplicateStorageKey { key });
    }
    let actual = entries
        .len()
        .checked_add(1)
        .expect("BTreeMap entry count cannot overflow usize");
    if actual > MAX_STORAGE_PATCH_ENTRIES {
        return Err(BootstrapGraphError::TooManyEntries {
            maximum: MAX_STORAGE_PATCH_ENTRIES,
            actual,
        });
    }
    match entries.entry(key.clone()) {
        Entry::Vacant(slot) => {
            slot.insert(BootstrapStorageIdentity {
                key: storage_key,
                owner,
            });
            Ok(())
        }
        Entry::Occupied(_) => unreachable!("duplicate storage key was checked above"),
    }
}

#[cfg(test)]
mod tests {
    use ibcmd_core::artifact::ProfileId;
    use ibcmd_core::diagnostic::{ObjectPath, PathSegment, PropertyPath};
    use ibcmd_core::identity::LogicalIdentity;
    use ibcmd_core::model::{
        CanonicalConfiguration, CanonicalObject, CanonicalObjectParts, GeneratedType,
        GeneratedTypeKind, MetadataKind,
    };
    use ibcmd_core::provenance::{CanonicalAnchor, SourceProvenance};
    use ibcmd_core::storage::{
        MultipartIdentity, StoragePatchBuildError, StoragePatchEntry, StoragePatchTarget,
        StorageProvenance,
    };
    use ibcmd_core::validate::validate_configuration;

    use crate::compiler::identity::collect_bootstrap_identities;

    use super::*;

    fn uuid(value: u32) -> ObjectUuid {
        ObjectUuid::parse(&format!("00000000-0000-4000-8000-{value:012x}")).unwrap()
    }

    fn object(value: u32, kind: &str, owner: Option<u32>, generated: &[u32]) -> CanonicalObject {
        let path =
            ObjectPath::new(vec![PathSegment::name(&format!("item-{value}")).unwrap()]).unwrap();
        let provenance = SourceProvenance::new(
            ProfileId::parse("profile:test").unwrap(),
            CanonicalAnchor::new(path.clone(), PropertyPath::root()),
        );
        let mut parts = CanonicalObjectParts::new(
            LogicalIdentity::new(uuid(value), path),
            MetadataKind::new(kind).unwrap(),
            provenance,
        );
        parts.owner = owner.map(uuid);
        parts.generated_types = generated
            .iter()
            .map(|value| GeneratedType::new(uuid(*value), GeneratedTypeKind::new("type").unwrap()))
            .collect();
        CanonicalObject::new(parts).unwrap()
    }

    fn identities() -> BootstrapIdentities {
        let configuration = CanonicalConfiguration::new(vec![
            object(2, "Catalog", None, &[102]),
            object(3, "Attribute", Some(2), &[103]),
            object(1, "Configuration", None, &[]),
        ])
        .unwrap();
        let validated = validate_configuration(&configuration).unwrap();
        collect_bootstrap_identities(&validated).unwrap()
    }

    fn graph(catalog_suffixes: Vec<StorageSuffix>) -> BootstrapGraph {
        build_bootstrap_graph(
            &identities(),
            ProfileId::parse("platform-test").unwrap(),
            vec![
                ObjectStorageRoute::new(uuid(1), Vec::new()).unwrap(),
                ObjectStorageRoute::new(uuid(2), catalog_suffixes).unwrap(),
            ],
        )
        .unwrap()
    }

    fn compiled(key: &str) -> Result<StoragePatchEntry, StoragePatchBuildError> {
        Ok(StoragePatchEntry::new(
            StoragePatchTarget::new(
                StorageKey::new(key)?,
                MultipartIdentity::single(),
                StorageProvenance::new("fixture:bootstrap-graph")?,
            ),
            StoragePatchOutcome::compiled(key.as_bytes().to_vec())?,
        ))
    }

    #[test]
    fn graph_inventory_is_key_sorted_and_contains_special_references() {
        let graph = graph(vec![
            StorageSuffix::new(".a").unwrap(),
            StorageSuffix::new(".0").unwrap(),
        ]);
        let keys = graph
            .inventory_keys()
            .map(StorageKey::as_str)
            .collect::<Vec<_>>();

        let mut sorted = keys.clone();
        sorted.sort_unstable();
        assert_eq!(keys, sorted);
        assert!(graph.contains_key(&uuid(1).to_string()));
        assert_eq!(graph.profile_id().as_str(), "platform-test");
        assert!(graph.contains_key(&format!("{}.0", uuid(2))));
        assert!(!graph.contains_key(&uuid(3).to_string()));
        assert_eq!(graph.generated_type_owner(uuid(102)), Some(uuid(2)));
        assert_eq!(graph.generated_type_owner(uuid(103)), Some(uuid(3)));
        graph.validate_special_references().unwrap();
    }

    #[test]
    fn route_order_does_not_change_inventory() {
        assert_eq!(
            graph(vec![
                StorageSuffix::new(".a").unwrap(),
                StorageSuffix::new(".0").unwrap(),
            ]),
            graph(vec![
                StorageSuffix::new(".0").unwrap(),
                StorageSuffix::new(".a").unwrap(),
            ])
        );
    }

    #[test]
    fn suffix_and_route_ambiguity_fail_closed() {
        assert!(StorageSuffix::new("0").is_err());
        assert!(StorageSuffix::new(".A").is_err());
        assert!(matches!(
            ObjectStorageRoute::new(
                uuid(2),
                vec![
                    StorageSuffix::new(".0").unwrap(),
                    StorageSuffix::new(".0").unwrap()
                ]
            ),
            Err(BootstrapGraphError::DuplicateSuffix { .. })
        ));
        let unknown = ObjectStorageRoute::new(uuid(9), vec![]).unwrap();
        assert!(matches!(
            build_bootstrap_graph(
                &identities(),
                ProfileId::parse("platform-test").unwrap(),
                vec![unknown],
            ),
            Err(BootstrapGraphError::UnknownRouteObject { object }) if object == uuid(9)
        ));
        let embedded = ObjectStorageRoute::new(uuid(3), vec![]).unwrap();
        assert!(matches!(
            build_bootstrap_graph(
                &identities(),
                ProfileId::parse("platform-test").unwrap(),
                vec![
                    ObjectStorageRoute::new(uuid(1), Vec::new()).unwrap(),
                    ObjectStorageRoute::new(uuid(2), Vec::new()).unwrap(),
                    embedded,
                ],
            ),
            Err(BootstrapGraphError::EmbeddedObjectRoute { object, owner })
                if object == uuid(3) && owner == uuid(2)
        ));
        assert!(matches!(
            build_bootstrap_graph(
                &identities(),
                ProfileId::parse("platform-test").unwrap(),
                vec![ObjectStorageRoute::new(uuid(1), Vec::new()).unwrap()],
            ),
            Err(BootstrapGraphError::MissingObjectRoute { object }) if object == uuid(2)
        ));
    }

    #[test]
    fn patch_inventory_requires_exact_compiled_single_part_coverage() {
        let graph = graph(Vec::new());
        let before_keys = graph
            .inventory_keys()
            .filter(|key| key.as_str() != "versions")
            .map(StorageKey::as_str)
            .collect::<Vec<_>>();
        let patch = StoragePatch::new(
            before_keys
                .iter()
                .map(|key| compiled(key).unwrap())
                .collect(),
        )
        .unwrap();
        graph
            .validate_patch_inventory(&patch, InventoryScope::BeforeVersions)
            .unwrap();
        assert!(matches!(
            graph.validate_patch_inventory(&patch, InventoryScope::Complete),
            Err(BootstrapGraphError::InventoryMismatch { .. })
        ));

        let mut blocked = patch.into_entries();
        let key = before_keys[0];
        blocked[0] = StoragePatchEntry::new(
            StoragePatchTarget::new(
                StorageKey::new(key).unwrap(),
                MultipartIdentity::single(),
                StorageProvenance::new("fixture:bootstrap-graph").unwrap(),
            ),
            StoragePatchOutcome::unsupported("fixture blocker").unwrap(),
        );
        let blocked = StoragePatch::new(blocked).unwrap();
        assert!(matches!(
            graph.validate_patch_inventory(&blocked, InventoryScope::BeforeVersions),
            Err(BootstrapGraphError::EntryBlocked { .. })
        ));
    }
}
