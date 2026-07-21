//! Strict, deterministic standalone conversion profiles.

use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::marker::PhantomData;
use std::str::FromStr;

use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use crate::artifact::{DbmsKind, ParseIdentifierError, ProfileId, StorageProfileId};
use crate::version::{CompatibilityMode, ContainerRevision, PlatformBuild, XmlDialect};

/// The only raw profile schema version supported by this implementation.
pub const PROFILE_SCHEMA_VERSION: u32 = 1;

/// An open capability identifier.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CapabilityId(ProfileId);

impl CapabilityId {
    /// Validates a capability identifier before retaining it.
    pub fn new(input: &str) -> Result<Self, ParseIdentifierError> {
        ProfileId::new(input).map(Self)
    }

    /// Parses an open capability identifier.
    pub fn parse(input: &str) -> Result<Self, ParseIdentifierError> {
        Self::new(input)
    }

    /// Returns the exact capability identifier.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl Display for CapabilityId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for CapabilityId {
    type Err = ParseIdentifierError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Self::new(input)
    }
}

impl Serialize for CapabilityId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for CapabilityId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(CapabilityIdVisitor)
    }
}

struct CapabilityIdVisitor;

impl Visitor<'_> for CapabilityIdVisitor {
    type Value = CapabilityId;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("a bounded capability identifier")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        CapabilityId::parse(value).map_err(E::custom)
    }
}

/// Verification status explicitly declared by a profile.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileStatus {
    /// The profile may change as evidence develops.
    Experimental,
    /// The profile is backed by reproducible evidence.
    Verified,
}

/// Explicit support state for an open capability identifier.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityState {
    /// The capability is available for the profile.
    Supported,
    /// The capability is explicitly unavailable and overrides inherited support.
    Unsupported,
}

/// Strict raw JSON representation of one profile declaration.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawProfile {
    /// Raw schema version. Resolution accepts only [`PROFILE_SCHEMA_VERSION`].
    pub schema_version: u32,
    /// Globally unique profile identifier.
    pub id: ProfileId,
    /// Optional single parent.
    pub extends: Option<ProfileId>,
    /// Optional declaration inherited from the parent when absent.
    pub status: Option<ProfileStatus>,
    /// Independent platform build coordinate.
    pub platform_build: Option<PlatformBuild>,
    /// Independent XML dialect coordinate.
    pub xml_dialect: Option<XmlDialect>,
    /// Independent compatibility-mode coordinate.
    pub compatibility_mode: Option<CompatibilityMode>,
    /// Independent logical storage-profile coordinate.
    pub storage_profile: Option<StorageProfileId>,
    /// Independent physical container coordinate.
    pub container_revision: Option<ContainerRevision>,
    /// Independent DBMS coordinate.
    pub dbms: Option<DbmsKind>,
    /// Open observed fingerprints, merged by key.
    #[serde(default, deserialize_with = "deserialize_unique_map")]
    pub fingerprints: BTreeMap<String, String>,
    /// Open profile constants, merged by key.
    #[serde(default, deserialize_with = "deserialize_unique_map")]
    pub constants: BTreeMap<String, String>,
    /// Evidence references, sorted and deduplicated during resolution.
    #[serde(default)]
    pub evidence: Vec<String>,
    /// Capability delta. Child entries explicitly override parent entries.
    #[serde(default, deserialize_with = "deserialize_unique_map")]
    pub capabilities: BTreeMap<CapabilityId, CapabilityState>,
}

fn deserialize_unique_map<'de, D, K, V>(deserializer: D) -> Result<BTreeMap<K, V>, D::Error>
where
    D: Deserializer<'de>,
    K: Deserialize<'de> + Display + Ord,
    V: Deserialize<'de>,
{
    deserializer.deserialize_map(UniqueMapVisitor(PhantomData))
}

struct UniqueMapVisitor<K, V>(PhantomData<fn() -> (K, V)>);

impl<'de, K, V> Visitor<'de> for UniqueMapVisitor<K, V>
where
    K: Deserialize<'de> + Display + Ord,
    V: Deserialize<'de>,
{
    type Value = BTreeMap<K, V>;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("an object with unique keys")
    }

    fn visit_map<A>(self, mut access: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = BTreeMap::new();
        while let Some((key, value)) = access.next_entry()? {
            match values.entry(key) {
                Entry::Vacant(entry) => {
                    entry.insert(value);
                }
                Entry::Occupied(entry) => {
                    return Err(de::Error::custom(format_args!(
                        "duplicate map key `{}`",
                        entry.key()
                    )));
                }
            }
        }
        Ok(values)
    }
}

/// Trust boundary of a named profile source.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileSourceKind {
    /// Profile shipped with the application or supplied as a trusted bundle.
    Bundled,
    /// Profile loaded from an external filesystem location.
    External,
}

/// Stable source identity retained in effective-profile provenance.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProfileSource {
    /// Caller-provided deterministic source name.
    pub name: String,
    /// Trust boundary of the source.
    pub kind: ProfileSourceKind,
}

/// One parsed profile coupled to its named source.
#[derive(Clone, Debug)]
pub struct ProfileDocument {
    /// Named source provenance.
    pub source: ProfileSource,
    /// Strict raw declaration.
    pub profile: RawProfile,
}

/// Parses one strict JSON profile without resolving or selecting it.
pub fn parse_profile_source(
    name: &str,
    kind: ProfileSourceKind,
    json: &str,
) -> Result<ProfileDocument, ProfileError> {
    let profile = serde_json::from_str(json).map_err(|error| ProfileError::InvalidJson {
        source: name.to_owned(),
        message: error.to_string(),
    })?;
    Ok(ProfileDocument {
        source: ProfileSource {
            name: name.to_owned(),
            kind,
        },
        profile,
    })
}

/// A value together with the profile that last declared it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Provenanced<T> {
    /// Effective value.
    pub value: T,
    /// Profile whose declaration produced the effective value.
    pub declared_by: ProfileId,
}

impl<T> Provenanced<T> {
    fn new(value: T, declared_by: &ProfileId) -> Self {
        Self {
            value,
            declared_by: declared_by.clone(),
        }
    }
}

/// Source entry ordered parent-first in an effective profile.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProfileSourceProvenance {
    /// Profile declared by the source.
    pub profile_id: ProfileId,
    /// Stable source identity.
    pub source: ProfileSource,
}

/// Fully resolved profile. No coordinate is inferred from another coordinate.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EffectiveProfile {
    /// Resolved profile identifier.
    pub id: ProfileId,
    /// Required effective status.
    pub status: Provenanced<ProfileStatus>,
    /// Independent effective coordinates.
    pub platform_build: Option<Provenanced<PlatformBuild>>,
    /// Independent effective coordinates.
    pub xml_dialect: Option<Provenanced<XmlDialect>>,
    /// Independent effective coordinates.
    pub compatibility_mode: Option<Provenanced<CompatibilityMode>>,
    /// Independent effective coordinates.
    pub storage_profile: Option<Provenanced<StorageProfileId>>,
    /// Independent effective coordinates.
    pub container_revision: Option<Provenanced<ContainerRevision>>,
    /// Independent effective coordinates.
    pub dbms: Option<Provenanced<DbmsKind>>,
    /// Effective fingerprints with per-entry provenance.
    pub fingerprints: BTreeMap<String, Provenanced<String>>,
    /// Effective constants with per-entry provenance.
    pub constants: BTreeMap<String, Provenanced<String>>,
    /// Deterministically sorted, deduplicated evidence with provenance.
    pub evidence: Vec<Provenanced<String>>,
    /// Effective capability states with per-entry provenance.
    pub capabilities: BTreeMap<CapabilityId, Provenanced<CapabilityState>>,
    /// Parent-first profile inheritance chain including this profile.
    pub inheritance_chain: Vec<ProfileId>,
    /// Parent-first source chain including this profile.
    pub source_chain: Vec<ProfileSourceProvenance>,
}

/// Deterministic resolved registry. It never performs profile selection.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProfileRegistry {
    profiles: BTreeMap<ProfileId, EffectiveProfile>,
}

impl ProfileRegistry {
    /// Returns all profiles in identifier order.
    pub fn profiles(&self) -> &BTreeMap<ProfileId, EffectiveProfile> {
        &self.profiles
    }

    /// Looks up one explicitly named profile.
    pub fn get(&self, id: &ProfileId) -> Option<&EffectiveProfile> {
        self.profiles.get(id)
    }
}

/// Stable profile parse or resolution failure.
#[derive(Debug, Eq, PartialEq)]
pub enum ProfileError {
    /// Strict JSON parsing failed.
    InvalidJson {
        /// Named source.
        source: String,
        /// Serde diagnostic.
        message: String,
    },
    /// A declaration uses an unsupported schema version.
    UnsupportedSchemaVersion {
        /// Profile identifier.
        id: ProfileId,
        /// Declared schema version.
        version: u32,
    },
    /// External profiles must explicitly declare experimental status.
    ExternalMustBeExperimental {
        /// Profile identifier.
        id: ProfileId,
        /// Named external source.
        source: String,
    },
    /// A verified effective profile cannot depend on untrusted external input.
    VerifiedDependsOnExternal {
        /// Effective profile identifier.
        id: ProfileId,
        /// Sorted, deduplicated external source names in its ancestry.
        external_sources: Vec<String>,
    },
    /// More than one source declared the same profile identifier.
    DuplicateProfile {
        /// Duplicate profile identifier.
        id: ProfileId,
        /// Deterministically first source.
        first_source: String,
        /// Deterministically second source.
        second_source: String,
    },
    /// A profile directly extends itself.
    SelfParent {
        /// Invalid profile identifier.
        id: ProfileId,
    },
    /// A declared parent is absent.
    MissingParent {
        /// Child profile identifier.
        id: ProfileId,
        /// Missing parent identifier.
        parent: ProfileId,
    },
    /// An inheritance cycle was found.
    Cycle {
        /// Stable closed cycle, for example `a, b, a`.
        chain: Vec<ProfileId>,
    },
    /// A root and its inheritance chain provide no status.
    MissingEffectiveStatus {
        /// Unresolvable profile identifier.
        id: ProfileId,
    },
}

impl Display for ProfileError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson { source, message } => {
                write!(formatter, "invalid profile JSON in `{source}`: {message}")
            }
            Self::UnsupportedSchemaVersion { id, version } => write!(
                formatter,
                "profile `{id}` declares unsupported schema version {version}"
            ),
            Self::ExternalMustBeExperimental { id, source } => write!(
                formatter,
                "external profile `{id}` from `{source}` must explicitly declare experimental status"
            ),
            Self::VerifiedDependsOnExternal {
                id,
                external_sources,
            } => write!(
                formatter,
                "verified profile `{id}` depends on external sources: {}",
                external_sources.join(", ")
            ),
            Self::DuplicateProfile {
                id,
                first_source,
                second_source,
            } => write!(
                formatter,
                "duplicate profile `{id}` in `{first_source}` and `{second_source}`"
            ),
            Self::SelfParent { id } => write!(formatter, "profile `{id}` extends itself"),
            Self::MissingParent { id, parent } => {
                write!(
                    formatter,
                    "profile `{id}` extends missing profile `{parent}`"
                )
            }
            Self::Cycle { chain } => {
                formatter.write_str("profile inheritance cycle: ")?;
                for (index, id) in chain.iter().enumerate() {
                    if index != 0 {
                        formatter.write_str(" -> ")?;
                    }
                    id.fmt(formatter)?;
                }
                Ok(())
            }
            Self::MissingEffectiveStatus { id } => {
                write!(formatter, "profile `{id}` has no effective status")
            }
        }
    }
}

impl Error for ProfileError {}

/// Resolves named documents into a deterministic registry.
pub fn resolve_profiles<I>(documents: I) -> Result<ProfileRegistry, ProfileError>
where
    I: IntoIterator<Item = ProfileDocument>,
{
    let mut documents = documents.into_iter().collect::<Vec<_>>();
    documents.sort_by(|left, right| {
        left.profile
            .id
            .cmp(&right.profile.id)
            .then_with(|| left.source.kind.cmp(&right.source.kind))
            .then_with(|| left.source.name.cmp(&right.source.name))
    });

    let mut declarations = BTreeMap::<ProfileId, ProfileDocument>::new();
    for document in documents {
        let id = document.profile.id.clone();
        if document.profile.schema_version != PROFILE_SCHEMA_VERSION {
            return Err(ProfileError::UnsupportedSchemaVersion {
                id,
                version: document.profile.schema_version,
            });
        }
        if document.source.kind == ProfileSourceKind::External
            && document.profile.status != Some(ProfileStatus::Experimental)
        {
            return Err(ProfileError::ExternalMustBeExperimental {
                id,
                source: document.source.name,
            });
        }
        if document.profile.extends.as_ref() == Some(&id) {
            return Err(ProfileError::SelfParent { id });
        }
        match declarations.entry(id.clone()) {
            Entry::Vacant(entry) => {
                entry.insert(document);
            }
            Entry::Occupied(entry) => {
                return Err(ProfileError::DuplicateProfile {
                    id,
                    first_source: entry.get().source.name.clone(),
                    second_source: document.source.name,
                });
            }
        }
    }

    let mut resolver = Resolver {
        declarations: &declarations,
        states: BTreeMap::new(),
        resolved: BTreeMap::new(),
        stack: Vec::new(),
    };
    for id in declarations.keys() {
        resolver.resolve(id)?;
    }
    Ok(ProfileRegistry {
        profiles: resolver.resolved,
    })
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum VisitState {
    Visiting,
    Resolved,
}

struct Resolver<'a> {
    declarations: &'a BTreeMap<ProfileId, ProfileDocument>,
    states: BTreeMap<ProfileId, VisitState>,
    resolved: BTreeMap<ProfileId, EffectiveProfile>,
    stack: Vec<ProfileId>,
}

impl Resolver<'_> {
    fn resolve(&mut self, id: &ProfileId) -> Result<(), ProfileError> {
        match self.states.get(id) {
            Some(VisitState::Resolved) => return Ok(()),
            Some(VisitState::Visiting) => {
                let start = self.stack.iter().position(|item| item == id).unwrap_or(0);
                let mut chain = self.stack[start..].to_vec();
                chain.push(id.clone());
                return Err(ProfileError::Cycle { chain });
            }
            None => {}
        }

        self.states.insert(id.clone(), VisitState::Visiting);
        self.stack.push(id.clone());
        let document = self
            .declarations
            .get(id)
            .expect("registered profile")
            .clone();
        let parent = if let Some(parent_id) = &document.profile.extends {
            if !self.declarations.contains_key(parent_id) {
                return Err(ProfileError::MissingParent {
                    id: id.clone(),
                    parent: parent_id.clone(),
                });
            }
            self.resolve(parent_id)?;
            self.resolved.get(parent_id).cloned()
        } else {
            None
        };

        let effective = build_effective(&document, parent.as_ref())?;
        let popped = self.stack.pop();
        debug_assert_eq!(popped.as_ref(), Some(id));
        self.states.insert(id.clone(), VisitState::Resolved);
        self.resolved.insert(id.clone(), effective);
        Ok(())
    }
}

fn build_effective(
    document: &ProfileDocument,
    parent: Option<&EffectiveProfile>,
) -> Result<EffectiveProfile, ProfileError> {
    let raw = &document.profile;
    let id = &raw.id;
    let status = raw
        .status
        .map(|value| Provenanced::new(value, id))
        .or_else(|| parent.map(|value| value.status.clone()))
        .ok_or_else(|| ProfileError::MissingEffectiveStatus { id: id.clone() })?;

    let mut inheritance_chain = parent
        .map(|value| value.inheritance_chain.clone())
        .unwrap_or_default();
    inheritance_chain.push(id.clone());
    let mut source_chain = parent
        .map(|value| value.source_chain.clone())
        .unwrap_or_default();
    source_chain.push(ProfileSourceProvenance {
        profile_id: id.clone(),
        source: document.source.clone(),
    });

    let effective = EffectiveProfile {
        id: id.clone(),
        status,
        platform_build: merge_scalar(
            parent.and_then(|value| value.platform_build.as_ref()),
            raw.platform_build.as_ref(),
            id,
        ),
        xml_dialect: merge_scalar(
            parent.and_then(|value| value.xml_dialect.as_ref()),
            raw.xml_dialect.as_ref(),
            id,
        ),
        compatibility_mode: merge_scalar(
            parent.and_then(|value| value.compatibility_mode.as_ref()),
            raw.compatibility_mode.as_ref(),
            id,
        ),
        storage_profile: merge_scalar(
            parent.and_then(|value| value.storage_profile.as_ref()),
            raw.storage_profile.as_ref(),
            id,
        ),
        container_revision: merge_scalar(
            parent.and_then(|value| value.container_revision.as_ref()),
            raw.container_revision.as_ref(),
            id,
        ),
        dbms: merge_scalar(
            parent.and_then(|value| value.dbms.as_ref()),
            raw.dbms.as_ref(),
            id,
        ),
        fingerprints: merge_map(
            parent.map(|value| &value.fingerprints),
            &raw.fingerprints,
            id,
        ),
        constants: merge_map(parent.map(|value| &value.constants), &raw.constants, id),
        evidence: merge_evidence(
            parent.map(|value| value.evidence.as_slice()),
            &raw.evidence,
            id,
        ),
        capabilities: merge_map(
            parent.map(|value| &value.capabilities),
            &raw.capabilities,
            id,
        ),
        inheritance_chain,
        source_chain,
    };

    if effective.status.value == ProfileStatus::Verified {
        let external_sources = effective
            .source_chain
            .iter()
            .filter(|entry| entry.source.kind == ProfileSourceKind::External)
            .map(|entry| entry.source.name.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        if !external_sources.is_empty() {
            return Err(ProfileError::VerifiedDependsOnExternal {
                id: id.clone(),
                external_sources,
            });
        }
    }

    Ok(effective)
}

fn merge_scalar<T: Clone>(
    parent: Option<&Provenanced<T>>,
    child: Option<&T>,
    child_id: &ProfileId,
) -> Option<Provenanced<T>> {
    child
        .map(|value| Provenanced::new(value.clone(), child_id))
        .or_else(|| parent.cloned())
}

fn merge_map<K, V>(
    parent: Option<&BTreeMap<K, Provenanced<V>>>,
    child: &BTreeMap<K, V>,
    child_id: &ProfileId,
) -> BTreeMap<K, Provenanced<V>>
where
    K: Clone + Ord,
    V: Clone,
{
    let mut merged = parent.cloned().unwrap_or_default();
    for (key, value) in child {
        merged.insert(key.clone(), Provenanced::new(value.clone(), child_id));
    }
    merged
}

fn merge_evidence(
    parent: Option<&[Provenanced<String>]>,
    child: &[String],
    child_id: &ProfileId,
) -> Vec<Provenanced<String>> {
    let mut merged = BTreeMap::<String, Provenanced<String>>::new();
    for value in parent.into_iter().flatten() {
        merged.insert(value.value.clone(), value.clone());
    }
    for value in child {
        merged
            .entry(value.clone())
            .or_insert_with(|| Provenanced::new(value.clone(), child_id));
    }
    merged.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bundled(name: &str, json: &str) -> ProfileDocument {
        parse_profile_source(name, ProfileSourceKind::Bundled, json).unwrap()
    }

    fn experimental_json(id: &str) -> String {
        format!(r#"{{"schema_version":1,"id":"{id}","status":"experimental"}}"#)
    }

    #[test]
    fn parent_merge_tracks_provenance_and_deduplicates_evidence() {
        let parent = bundled(
            "parent.json",
            r#"{
                "schema_version": 1,
                "id": "base",
                "status": "verified",
                "platform_build": "8.3.27.1989",
                "storage_profile": "storage:mssql",
                "fingerprints": {"layout": "parent"},
                "constants": {"inherited": "yes", "overridden": "parent"},
                "evidence": ["z-evidence", "shared"],
                "capabilities": {"inspect": "supported", "write": "supported"}
            }"#,
        );
        let child = bundled(
            "child.json",
            r#"{
                "schema_version": 1,
                "id": "child",
                "extends": "base",
                "xml_dialect": "2.21",
                "constants": {"overridden": "child"},
                "evidence": ["a-evidence", "shared"],
                "capabilities": {"write": "unsupported"}
            }"#,
        );

        let registry = resolve_profiles([child, parent]).unwrap();
        let child_id = ProfileId::parse("child").unwrap();
        let base_id = ProfileId::parse("base").unwrap();
        let effective = registry.get(&child_id).unwrap();

        assert_eq!(effective.status.declared_by, base_id);
        assert_eq!(
            effective.platform_build.as_ref().unwrap().declared_by,
            base_id
        );
        assert_eq!(
            effective.xml_dialect.as_ref().unwrap().declared_by,
            child_id
        );
        assert_eq!(
            effective.constants["inherited"].declared_by,
            ProfileId::parse("base").unwrap()
        );
        assert_eq!(effective.constants["overridden"].value, "child");
        assert_eq!(effective.constants["overridden"].declared_by, child_id);
        assert_eq!(
            effective.capabilities[&CapabilityId::parse("write").unwrap()].value,
            CapabilityState::Unsupported
        );
        assert_eq!(
            effective.capabilities[&CapabilityId::parse("write").unwrap()].declared_by,
            child_id
        );
        assert_eq!(
            effective
                .evidence
                .iter()
                .map(|value| value.value.as_str())
                .collect::<Vec<_>>(),
            ["a-evidence", "shared", "z-evidence"]
        );
        assert_eq!(effective.evidence[1].declared_by, base_id);
        assert_eq!(effective.inheritance_chain, [base_id, child_id]);
        assert_eq!(effective.source_chain[0].source.name, "parent.json");
        assert_eq!(effective.source_chain[1].source.name, "child.json");
    }

    #[test]
    fn unsupported_capability_explicitly_overrides_supported_parent() {
        let parent = bundled(
            "a.json",
            r#"{"schema_version":1,"id":"a","status":"verified","capabilities":{"convert":"supported"}}"#,
        );
        let child = bundled(
            "b.json",
            r#"{"schema_version":1,"id":"b","extends":"a","capabilities":{"convert":"unsupported"}}"#,
        );
        let registry = resolve_profiles([parent, child]).unwrap();
        let effective = registry.get(&ProfileId::parse("b").unwrap()).unwrap();
        let capability = &effective.capabilities[&CapabilityId::parse("convert").unwrap()];
        assert_eq!(capability.value, CapabilityState::Unsupported);
        assert_eq!(capability.declared_by, ProfileId::parse("b").unwrap());
    }

    #[test]
    fn rejects_self_parent_missing_parent_and_stable_cycle() {
        let self_parent = bundled(
            "self.json",
            r#"{"schema_version":1,"id":"self","extends":"self","status":"experimental"}"#,
        );
        assert!(matches!(
            resolve_profiles([self_parent]),
            Err(ProfileError::SelfParent { .. })
        ));

        let missing = bundled(
            "missing.json",
            r#"{"schema_version":1,"id":"child","extends":"absent","status":"experimental"}"#,
        );
        assert!(matches!(
            resolve_profiles([missing]),
            Err(ProfileError::MissingParent { .. })
        ));

        let a = bundled(
            "a.json",
            r#"{"schema_version":1,"id":"a","extends":"b","status":"experimental"}"#,
        );
        let b = bundled(
            "b.json",
            r#"{"schema_version":1,"id":"b","extends":"a","status":"experimental"}"#,
        );
        let error = resolve_profiles([b, a]).unwrap_err();
        assert_eq!(error.to_string(), "profile inheritance cycle: a -> b -> a");
    }

    #[test]
    fn rejects_duplicate_ids_and_missing_effective_status() {
        let first = bundled("z.json", &experimental_json("duplicate"));
        let second = bundled("a.json", &experimental_json("duplicate"));
        let error = resolve_profiles([first, second]).unwrap_err();
        assert_eq!(
            error.to_string(),
            "duplicate profile `duplicate` in `a.json` and `z.json`"
        );

        let no_status = bundled("root.json", r#"{"schema_version":1,"id":"root"}"#);
        assert!(matches!(
            resolve_profiles([no_status]),
            Err(ProfileError::MissingEffectiveStatus { .. })
        ));
    }

    #[test]
    fn strict_json_rejects_duplicate_fields_keys_and_unknown_fields() {
        for json in [
            r#"{"schema_version":1,"id":"a","id":"b","status":"experimental"}"#,
            r#"{"schema_version":1,"id":"a","status":"experimental","unknown":true}"#,
            r#"{"schema_version":1,"id":"a","status":"experimental","capabilities":{"inspect":"supported","inspect":"unsupported"}}"#,
            r#"{"schema_version":1,"id":"a","status":"experimental","constants":{"same":"one","same":"two"}}"#,
        ] {
            assert!(
                parse_profile_source("invalid.json", ProfileSourceKind::Bundled, json).is_err(),
                "{json}"
            );
        }
    }

    #[test]
    fn shuffled_input_produces_identical_effective_registry() {
        let a_json = experimental_json("a");
        let b_json = r#"{"schema_version":1,"id":"b","extends":"a","constants":{"k":"v"}}"#;
        let c_json = experimental_json("c");
        let first = resolve_profiles([
            bundled("a.json", &a_json),
            bundled("b.json", b_json),
            bundled("c.json", &c_json),
        ])
        .unwrap();
        let second = resolve_profiles([
            bundled("c.json", &c_json),
            bundled("b.json", b_json),
            bundled("a.json", &a_json),
        ])
        .unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn future_coordinates_and_unknown_ids_load_without_inference() {
        let profile = bundled(
            "future.json",
            r#"{
                "schema_version": 1,
                "id": "profile:future",
                "status": "experimental",
                "platform_build": "9.1.0.42",
                "xml_dialect": "3.0",
                "compatibility_mode": "Version9_1_Future",
                "storage_profile": "storage:future",
                "container_revision": "Format42",
                "dbms": "future-dbms",
                "capabilities": {"future:capability": "supported"}
            }"#,
        );
        let registry = resolve_profiles([profile]).unwrap();
        let effective = registry
            .get(&ProfileId::parse("profile:future").unwrap())
            .unwrap();
        assert_eq!(
            effective.platform_build.as_ref().unwrap().value.to_string(),
            "9.1.0.42"
        );
        assert_eq!(
            effective.xml_dialect.as_ref().unwrap().value.to_string(),
            "3.0"
        );
        assert_eq!(
            effective.dbms.as_ref().unwrap().value.as_str(),
            "future-dbms"
        );
        assert!(
            effective
                .capabilities
                .contains_key(&CapabilityId::parse("future:capability").unwrap())
        );
    }

    #[test]
    fn profile_dotted_coordinates_use_exact_u32_component_bounds() {
        let accepted = bundled(
            "bounds.json",
            r#"{
                "schema_version": 1,
                "id": "bounds",
                "status": "experimental",
                "platform_build": "4294967295.0",
                "xml_dialect": "0.4294967295"
            }"#,
        );
        let registry = resolve_profiles([accepted]).unwrap();
        let effective = registry.get(&ProfileId::parse("bounds").unwrap()).unwrap();
        assert_eq!(
            effective.platform_build.as_ref().unwrap().value.to_string(),
            "4294967295.0"
        );
        assert_eq!(
            effective.xml_dialect.as_ref().unwrap().value.to_string(),
            "0.4294967295"
        );

        for json in [
            r#"{"schema_version":1,"id":"too-large-platform","status":"experimental","platform_build":"4294967296.0"}"#,
            r#"{"schema_version":1,"id":"too-large-dialect","status":"experimental","xml_dialect":"0.4294967296"}"#,
        ] {
            assert!(
                parse_profile_source("bounds.json", ProfileSourceKind::Bundled, json).is_err(),
                "{json}"
            );
        }
    }

    #[test]
    fn external_profile_must_explicitly_be_experimental() {
        for json in [
            r#"{"schema_version":1,"id":"verified","status":"verified"}"#,
            r#"{"schema_version":1,"id":"implicit"}"#,
        ] {
            let document =
                parse_profile_source("external.json", ProfileSourceKind::External, json).unwrap();
            assert!(matches!(
                resolve_profiles([document]),
                Err(ProfileError::ExternalMustBeExperimental { .. })
            ));
        }
    }

    #[test]
    fn verified_profiles_cannot_depend_on_external_ancestry() {
        let external_parent = parse_profile_source(
            "z-external-parent.json",
            ProfileSourceKind::External,
            r#"{"schema_version":1,"id":"external-parent","status":"experimental"}"#,
        )
        .unwrap();
        let external_middle = parse_profile_source(
            "a-external-middle.json",
            ProfileSourceKind::External,
            r#"{"schema_version":1,"id":"external-middle","extends":"external-parent","status":"experimental"}"#,
        )
        .unwrap();
        let verified_child = bundled(
            "bundled-child.json",
            r#"{"schema_version":1,"id":"verified-child","extends":"external-middle","status":"verified"}"#,
        );

        assert_eq!(
            resolve_profiles([external_parent, external_middle, verified_child]).unwrap_err(),
            ProfileError::VerifiedDependsOnExternal {
                id: ProfileId::parse("verified-child").unwrap(),
                external_sources: vec![
                    "a-external-middle.json".to_owned(),
                    "z-external-parent.json".to_owned(),
                ],
            }
        );
    }

    #[test]
    fn experimental_bundled_child_can_depend_on_external_parent() {
        let external_parent = parse_profile_source(
            "external-parent.json",
            ProfileSourceKind::External,
            r#"{"schema_version":1,"id":"external-parent","status":"experimental"}"#,
        )
        .unwrap();
        let bundled_child = bundled(
            "bundled-child.json",
            r#"{"schema_version":1,"id":"bundled-child","extends":"external-parent","status":"experimental"}"#,
        );

        let registry = resolve_profiles([bundled_child, external_parent]).unwrap();
        let effective = registry
            .get(&ProfileId::parse("bundled-child").unwrap())
            .unwrap();
        assert_eq!(effective.status.value, ProfileStatus::Experimental);
        assert_eq!(
            effective.status.declared_by,
            ProfileId::parse("bundled-child").unwrap()
        );
    }

    #[test]
    fn external_child_of_verified_bundle_stays_experimental() {
        let bundled_parent = bundled(
            "bundled-parent.json",
            r#"{"schema_version":1,"id":"bundled-parent","status":"verified"}"#,
        );
        let external_child = parse_profile_source(
            "external-child.json",
            ProfileSourceKind::External,
            r#"{"schema_version":1,"id":"external-child","extends":"bundled-parent","status":"experimental"}"#,
        )
        .unwrap();

        let registry = resolve_profiles([external_child, bundled_parent]).unwrap();
        let effective = registry
            .get(&ProfileId::parse("external-child").unwrap())
            .unwrap();
        assert_eq!(effective.status.value, ProfileStatus::Experimental);
        assert_eq!(
            effective.status.declared_by,
            ProfileId::parse("external-child").unwrap()
        );
        assert_eq!(
            effective.source_chain[1].source.kind,
            ProfileSourceKind::External
        );
    }
}
