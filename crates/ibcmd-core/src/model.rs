//! Bounded canonical metadata graph model.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::marker::PhantomData;
use std::str::FromStr;

use serde::de::{IgnoredAny, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use crate::asset::AssetReference;
use crate::identity::{LogicalIdentity, ObjectUuid};
use crate::opaque::{OpaqueFacet, OpaqueFacets};
use crate::provenance::SourceProvenance;
use crate::value::{CanonicalField, CanonicalValue, CanonicalValueKind, FieldName};

/// Maximum encoded length of an open graph kind token.
pub const MAX_MODEL_KIND_BYTES: usize = 256;
/// Maximum ordered properties retained by one object.
pub const MAX_OBJECT_PROPERTIES: usize = 16_384;
/// Maximum ordered references retained by one object.
pub const MAX_OBJECT_REFERENCES: usize = 16_384;
/// Maximum ordered generated types retained by one object.
pub const MAX_GENERATED_TYPES: usize = 4_096;
/// Maximum ordered asset references retained by one object.
pub const MAX_OBJECT_ASSETS: usize = 16_384;
/// Maximum aggregate members retained by one canonical object.
pub const MAX_OBJECT_MEMBERS: usize = 262_144;
/// Maximum canonical objects retained by one configuration.
pub const MAX_CONFIGURATION_OBJECTS: usize = 65_536;
/// Maximum aggregate members across all configuration objects.
pub const MAX_CONFIGURATION_MEMBERS: usize = 262_144;
/// Maximum variable-sized retained bytes in one canonical object.
pub const MAX_OBJECT_RETAINED_BYTES: usize = 134_217_728;
/// Maximum variable-sized retained bytes in one canonical configuration.
pub const MAX_CONFIGURATION_RETAINED_BYTES: usize = 268_435_456;

/// Failure to construct or revalidate the bounded canonical graph model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModelBuildError {
    /// A required open token was empty.
    EmptyKind {
        /// Logical token field.
        field: &'static str,
    },
    /// An open token exceeded its encoded bound.
    KindTooLong {
        /// Logical token field.
        field: &'static str,
        /// Maximum accepted bytes.
        maximum: usize,
        /// Actual bytes.
        actual: usize,
    },
    /// An open token violated the stable ASCII grammar.
    InvalidKind {
        /// Logical token field.
        field: &'static str,
    },
    /// A bounded ordered collection contained too many items.
    TooManyItems {
        /// Logical collection field.
        field: &'static str,
        /// Maximum accepted items.
        maximum: usize,
        /// Actual items.
        actual: usize,
    },
    /// An object contained the same exact property name more than once.
    DuplicateProperty {
        /// Duplicate case-sensitive name.
        name: String,
    },
    /// Aggregate graph members exceeded their bound.
    TooManyMembers {
        /// Logical budget scope.
        scope: &'static str,
        /// Maximum accepted members.
        maximum: usize,
        /// Actual members.
        actual: usize,
    },
    /// Variable-sized retained data exceeded an aggregate budget.
    RetainedBytesExceeded {
        /// Logical budget scope.
        scope: &'static str,
        /// Maximum accepted retained bytes.
        maximum: usize,
        /// Actual retained bytes.
        actual: usize,
    },
    /// Aggregate count arithmetic overflowed.
    CountOverflow,
    /// Aggregate retained-byte arithmetic overflowed.
    RetainedByteCountOverflow,
}

impl Display for ModelBuildError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyKind { field } => write!(formatter, "{field} is empty"),
            Self::KindTooLong {
                field,
                maximum,
                actual,
            } => write!(
                formatter,
                "{field} exceeds {maximum} bytes (actual {actual})"
            ),
            Self::InvalidKind { field } => write!(
                formatter,
                "{field} must be a stable ASCII token using letters, digits, '.', '-', '_', or ':'"
            ),
            Self::TooManyItems {
                field,
                maximum,
                actual,
            } => write!(
                formatter,
                "{field} exceeds {maximum} items (actual {actual})"
            ),
            Self::DuplicateProperty { name } => {
                write!(
                    formatter,
                    "canonical object contains duplicate property `{name}`"
                )
            }
            Self::TooManyMembers {
                scope,
                maximum,
                actual,
            } => write!(
                formatter,
                "{scope} exceeds {maximum} graph members (actual {actual})"
            ),
            Self::RetainedBytesExceeded {
                scope,
                maximum,
                actual,
            } => write!(
                formatter,
                "{scope} exceeds retained-byte budget {maximum} (actual {actual})"
            ),
            Self::CountOverflow => formatter.write_str("canonical graph member count overflowed"),
            Self::RetainedByteCountOverflow => {
                formatter.write_str("canonical graph retained-byte count overflowed")
            }
        }
    }
}

impl Error for ModelBuildError {}

fn validate_kind(field: &'static str, value: &str) -> Result<(), ModelBuildError> {
    if value.is_empty() {
        return Err(ModelBuildError::EmptyKind { field });
    }
    if value.len() > MAX_MODEL_KIND_BYTES {
        return Err(ModelBuildError::KindTooLong {
            field,
            maximum: MAX_MODEL_KIND_BYTES,
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
        return Err(ModelBuildError::InvalidKind { field });
    }
    Ok(())
}

struct ParseKindVisitor<T>(PhantomData<fn() -> T>);

impl<'de, T> Visitor<'de> for ParseKindVisitor<T>
where
    T: FromStr,
    T::Err: Display,
{
    type Value = T;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "an open stable kind token of at most {MAX_MODEL_KIND_BYTES} bytes"
        )
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        value.parse().map_err(E::custom)
    }
}

macro_rules! open_kind_type {
    ($(#[$metadata:meta])* $name:ident, $field:literal) => {
        $(#[$metadata])*
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(Box<str>);

        impl $name {
            /// Validates borrowed text before retaining it exactly.
            pub fn new(value: &str) -> Result<Self, ModelBuildError> {
                validate_kind($field, value)?;
                Ok(Self(value.into()))
            }

            /// Returns the exact open token.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl Display for $name {
            fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl FromStr for $name {
            type Err = ModelBuildError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::new(value)
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
                deserializer.deserialize_str(ParseKindVisitor::<Self>(PhantomData))
            }
        }
    };
}

open_kind_type! {
    /// Open metadata-object kind such as `Catalog` or a future family.
    MetadataKind, "metadata kind"
}

open_kind_type! {
    /// Open semantic role of an object reference.
    ReferenceKind, "reference kind"
}

open_kind_type! {
    /// Open generated-type role retained without a vendor registry.
    GeneratedTypeKind, "generated type kind"
}

/// A typed reference to an object or generated-type UUID.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectReference {
    kind: ReferenceKind,
    target: ObjectUuid,
}

impl ObjectReference {
    /// Creates a reference from validated parts.
    pub const fn new(kind: ReferenceKind, target: ObjectUuid) -> Self {
        Self { kind, target }
    }

    /// Returns its open semantic role.
    pub const fn kind(&self) -> &ReferenceKind {
        &self.kind
    }

    /// Returns its exact target UUID.
    pub const fn target(&self) -> ObjectUuid {
        self.target
    }
}

/// One generated semantic type owned by a canonical object.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedType {
    uuid: ObjectUuid,
    kind: GeneratedTypeKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    value_id: Option<ObjectUuid>,
}

impl GeneratedType {
    /// Creates a generated type from an exact TypeId UUID and open role.
    ///
    /// `ValueId` is optional because not every source family exposes one and
    /// older canonical documents predate its typed representation.
    pub const fn new(uuid: ObjectUuid, kind: GeneratedTypeKind) -> Self {
        Self {
            uuid,
            kind,
            value_id: None,
        }
    }

    /// Attaches the exact generated ValueId required by native layouts that
    /// distinguish type and value identities.
    pub const fn with_value_id(mut self, value_id: ObjectUuid) -> Self {
        self.value_id = Some(value_id);
        self
    }

    /// Returns the exact globally indexed UUID.
    pub const fn uuid(&self) -> ObjectUuid {
        self.uuid
    }

    /// Returns the exact open generated-type role.
    pub const fn kind(&self) -> &GeneratedTypeKind {
        &self.kind
    }

    /// Returns the independently sourced generated ValueId, when declared.
    pub const fn value_id(&self) -> Option<ObjectUuid> {
        self.value_id
    }
}

/// Validated constructor input for an immutable [`CanonicalObject`].
///
/// Collections are retained in their supplied order. Making this staging
/// value mutable does not mutate a built object and cannot bypass `build`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalObjectParts {
    /// Exact UUID and stable logical path.
    pub identity: LogicalIdentity,
    /// Open metadata family.
    pub kind: MetadataKind,
    /// Optional owning object UUID.
    pub owner: Option<ObjectUuid>,
    /// Ordered typed properties.
    pub properties: Vec<CanonicalField>,
    /// Ordered typed references.
    pub references: Vec<ObjectReference>,
    /// Ordered generated types.
    pub generated_types: Vec<GeneratedType>,
    /// Ordered content-addressed asset references.
    pub assets: Vec<AssetReference>,
    /// Ordered anchored opaque facets.
    pub opaque_facets: OpaqueFacets,
    /// Exact source evidence, excluded from semantic equality.
    pub provenance: SourceProvenance,
}

impl CanonicalObjectParts {
    /// Creates minimal object parts with empty optional collections.
    pub fn new(
        identity: LogicalIdentity,
        kind: MetadataKind,
        provenance: SourceProvenance,
    ) -> Self {
        Self {
            identity,
            kind,
            owner: None,
            properties: Vec::new(),
            references: Vec::new(),
            generated_types: Vec::new(),
            assets: Vec::new(),
            opaque_facets: OpaqueFacets::default(),
            provenance,
        }
    }
}

/// One immutable canonical metadata object with ordered semantic collections.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CanonicalObject {
    identity: LogicalIdentity,
    kind: MetadataKind,
    owner: Option<ObjectUuid>,
    properties: Vec<CanonicalField>,
    references: Vec<ObjectReference>,
    generated_types: Vec<GeneratedType>,
    assets: Vec<AssetReference>,
    opaque_facets: OpaqueFacets,
    provenance: SourceProvenance,
}

impl CanonicalObject {
    /// Validates all count, duplicate-property, and aggregate-byte invariants.
    pub fn new(parts: CanonicalObjectParts) -> Result<Self, ModelBuildError> {
        validate_item_count(
            "object properties",
            parts.properties.len(),
            MAX_OBJECT_PROPERTIES,
        )?;
        validate_item_count(
            "object references",
            parts.references.len(),
            MAX_OBJECT_REFERENCES,
        )?;
        validate_item_count(
            "object generated types",
            parts.generated_types.len(),
            MAX_GENERATED_TYPES,
        )?;
        validate_item_count("object assets", parts.assets.len(), MAX_OBJECT_ASSETS)?;
        validate_unique_properties(&parts.properties)?;

        let object = Self {
            identity: parts.identity,
            kind: parts.kind,
            owner: parts.owner,
            properties: parts.properties,
            references: parts.references,
            generated_types: parts.generated_types,
            assets: parts.assets,
            opaque_facets: parts.opaque_facets,
            provenance: parts.provenance,
        };
        enforce_member_budget(
            "canonical object",
            object.member_count()?,
            MAX_OBJECT_MEMBERS,
        )?;
        let retained = measure_object_retained_bytes(&object)?;
        enforce_retained_budget("canonical object", retained, MAX_OBJECT_RETAINED_BYTES)?;
        Ok(object)
    }

    /// Returns exact logical identity.
    pub const fn identity(&self) -> &LogicalIdentity {
        &self.identity
    }

    /// Returns the open metadata kind.
    pub const fn kind(&self) -> &MetadataKind {
        &self.kind
    }

    /// Returns the optional exact owner UUID.
    pub const fn owner(&self) -> Option<ObjectUuid> {
        self.owner
    }

    /// Returns typed properties in declared order.
    pub fn properties(&self) -> &[CanonicalField] {
        &self.properties
    }

    /// Returns typed references in declared order.
    pub fn references(&self) -> &[ObjectReference] {
        &self.references
    }

    /// Returns generated types in declared order.
    pub fn generated_types(&self) -> &[GeneratedType] {
        &self.generated_types
    }

    /// Returns asset references in declared order.
    pub fn assets(&self) -> &[AssetReference] {
        &self.assets
    }

    /// Returns opaque facets in declared order.
    pub const fn opaque_facets(&self) -> &OpaqueFacets {
        &self.opaque_facets
    }

    /// Returns exact source provenance.
    pub const fn provenance(&self) -> &SourceProvenance {
        &self.provenance
    }

    /// Returns variable-sized bytes retained by this object.
    pub fn retained_byte_len(&self) -> usize {
        measure_object_retained_bytes(self)
            .expect("private canonical object invariants remain valid")
    }

    pub(crate) fn member_count(&self) -> Result<usize, ModelBuildError> {
        let mut count = 1_usize;
        for property in &self.properties {
            count = add_property_members(count, property)?;
        }
        [
            self.references.len(),
            self.generated_types.len(),
            self.assets.len(),
            self.opaque_facets.len(),
        ]
        .into_iter()
        .try_fold(count, checked_add_members)
    }
}

fn validate_item_count(
    field: &'static str,
    actual: usize,
    maximum: usize,
) -> Result<(), ModelBuildError> {
    if actual > maximum {
        return Err(ModelBuildError::TooManyItems {
            field,
            maximum,
            actual,
        });
    }
    Ok(())
}

fn validate_unique_properties(properties: &[CanonicalField]) -> Result<(), ModelBuildError> {
    let mut names = BTreeSet::<&FieldName>::new();
    for property in properties {
        if !names.insert(property.name()) {
            return Err(ModelBuildError::DuplicateProperty {
                name: property.name().as_str().to_owned(),
            });
        }
    }
    Ok(())
}

fn checked_add_members(current: usize, additional: usize) -> Result<usize, ModelBuildError> {
    current
        .checked_add(additional)
        .ok_or(ModelBuildError::CountOverflow)
}

fn enforce_member_budget(
    scope: &'static str,
    actual: usize,
    maximum: usize,
) -> Result<(), ModelBuildError> {
    if actual > maximum {
        return Err(ModelBuildError::TooManyMembers {
            scope,
            maximum,
            actual,
        });
    }
    Ok(())
}

fn add_property_members(
    current: usize,
    property: &CanonicalField,
) -> Result<usize, ModelBuildError> {
    let with_property = checked_add_members(current, 1)?;
    checked_add_members(
        with_property,
        canonical_value_member_count(property.value())?,
    )
}

fn canonical_value_member_count(value: &CanonicalValue) -> Result<usize, ModelBuildError> {
    let mut count = 1_usize;
    match value.kind() {
        CanonicalValueKind::Record(fields) => {
            for field in fields {
                count = checked_add_members(count, 1)?;
                count = checked_add_members(count, canonical_value_member_count(field.value())?)?;
            }
        }
        CanonicalValueKind::Sequence(values) => {
            for child in values {
                count = checked_add_members(count, canonical_value_member_count(child)?)?;
            }
        }
        CanonicalValueKind::Null
        | CanonicalValueKind::Bool(_)
        | CanonicalValueKind::Integer(_)
        | CanonicalValueKind::Decimal(_)
        | CanonicalValueKind::Text(_)
        | CanonicalValueKind::EnumToken(_)
        | CanonicalValueKind::Reference(_)
        | CanonicalValueKind::Binary(_)
        | CanonicalValueKind::AssetReference(_) => {}
    }
    Ok(count)
}

fn checked_add_retained(current: usize, additional: usize) -> Result<usize, ModelBuildError> {
    current
        .checked_add(additional)
        .ok_or(ModelBuildError::RetainedByteCountOverflow)
}

fn enforce_retained_budget(
    scope: &'static str,
    actual: usize,
    maximum: usize,
) -> Result<(), ModelBuildError> {
    if actual > maximum {
        return Err(ModelBuildError::RetainedBytesExceeded {
            scope,
            maximum,
            actual,
        });
    }
    Ok(())
}

fn measure_object_retained_bytes(object: &CanonicalObject) -> Result<usize, ModelBuildError> {
    let mut retained = object.identity.retained_byte_len();
    retained = checked_add_retained(retained, object.kind.as_str().len())?;
    retained = checked_add_retained(retained, object.owner.map_or(0, |_| 16))?;
    for property in &object.properties {
        retained = checked_add_retained(retained, property.name().as_str().len())?;
        retained = checked_add_retained(retained, property.value().retained_byte_len())?;
    }
    for reference in &object.references {
        retained = checked_add_retained(retained, reference.kind.as_str().len() + 16)?;
    }
    for generated_type in &object.generated_types {
        retained = checked_add_retained(retained, generated_type.kind.as_str().len() + 16)?;
    }
    for asset in &object.assets {
        retained = checked_add_retained(retained, 40 + asset.media_kind().as_str().len())?;
    }
    for facet in object.opaque_facets.as_slice() {
        retained = checked_add_retained(retained, measure_opaque_facet(facet)?)?;
    }
    checked_add_retained(retained, object.provenance.retained_byte_len())
}

fn measure_opaque_facet(facet: &OpaqueFacet) -> Result<usize, ModelBuildError> {
    let bytes = usize::try_from(facet.byte_len())
        .map_err(|_| ModelBuildError::RetainedByteCountOverflow)?;
    let retained = checked_add_retained(bytes, 32 + facet.media_kind().as_str().len())?;
    let retained = checked_add_retained(retained, facet.placement().kind().as_str().len())?;
    checked_add_retained(retained, facet.provenance().retained_byte_len())
}

struct BoundedVec<T, const MAXIMUM: usize> {
    values: Vec<T>,
}

struct BoundedVecVisitor<T, const MAXIMUM: usize> {
    field: &'static str,
    marker: PhantomData<fn() -> T>,
}

impl<'de, T, const MAXIMUM: usize> Visitor<'de> for BoundedVecVisitor<T, MAXIMUM>
where
    T: Deserialize<'de>,
{
    type Value = BoundedVec<T, MAXIMUM>;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "a bounded ordered {} collection of at most {MAXIMUM} items",
            self.field
        )
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::with_capacity(sequence.size_hint().unwrap_or_default().min(MAXIMUM));
        while values.len() < MAXIMUM {
            let Some(value) = sequence.next_element::<T>()? else {
                return Ok(BoundedVec { values });
            };
            values.push(value);
        }
        if sequence.next_element::<IgnoredAny>()?.is_some() {
            return Err(de::Error::custom(ModelBuildError::TooManyItems {
                field: self.field,
                maximum: MAXIMUM,
                actual: MAXIMUM + 1,
            }));
        }
        Ok(BoundedVec { values })
    }
}

impl<'de, T, const MAXIMUM: usize> Deserialize<'de> for BoundedVec<T, MAXIMUM>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(BoundedVecVisitor {
            field: "model",
            marker: PhantomData,
        })
    }
}

#[derive(Debug)]
struct BoundedProperties<const MAXIMUM_MEMBERS: usize>(Vec<CanonicalField>);

struct BoundedPropertiesVisitor<const MAXIMUM_MEMBERS: usize>;

impl<'de, const MAXIMUM_MEMBERS: usize> Visitor<'de> for BoundedPropertiesVisitor<MAXIMUM_MEMBERS> {
    type Value = BoundedProperties<MAXIMUM_MEMBERS>;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "at most {MAX_OBJECT_PROPERTIES} unique ordered canonical properties within the object byte and {MAXIMUM_MEMBERS}-member budgets"
        )
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut properties = Vec::with_capacity(
            sequence
                .size_hint()
                .unwrap_or_default()
                .min(MAX_OBJECT_PROPERTIES),
        );
        let mut names = BTreeSet::new();
        let mut members = 1_usize;
        let mut retained = 0_usize;
        while properties.len() < MAX_OBJECT_PROPERTIES {
            let Some(property) = sequence.next_element::<CanonicalField>()? else {
                return Ok(BoundedProperties(properties));
            };
            if !names.insert(property.name().clone()) {
                return Err(de::Error::custom(ModelBuildError::DuplicateProperty {
                    name: property.name().as_str().to_owned(),
                }));
            }
            retained = checked_add_retained(retained, property.name().as_str().len())
                .and_then(|value| checked_add_retained(value, property.value().retained_byte_len()))
                .map_err(de::Error::custom)?;
            enforce_retained_budget("object properties", retained, MAX_OBJECT_RETAINED_BYTES)
                .map_err(de::Error::custom)?;
            members = add_property_members(members, &property).map_err(de::Error::custom)?;
            enforce_member_budget("canonical object", members, MAXIMUM_MEMBERS)
                .map_err(de::Error::custom)?;
            properties.push(property);
        }
        if sequence.next_element::<IgnoredAny>()?.is_some() {
            return Err(de::Error::custom(ModelBuildError::TooManyItems {
                field: "object properties",
                maximum: MAX_OBJECT_PROPERTIES,
                actual: MAX_OBJECT_PROPERTIES + 1,
            }));
        }
        Ok(BoundedProperties(properties))
    }
}

impl<'de, const MAXIMUM_MEMBERS: usize> Deserialize<'de> for BoundedProperties<MAXIMUM_MEMBERS> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(BoundedPropertiesVisitor::<MAXIMUM_MEMBERS>)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCanonicalObject {
    identity: LogicalIdentity,
    kind: MetadataKind,
    owner: Option<ObjectUuid>,
    properties: BoundedProperties<MAX_OBJECT_MEMBERS>,
    references: BoundedVec<ObjectReference, MAX_OBJECT_REFERENCES>,
    generated_types: BoundedVec<GeneratedType, MAX_GENERATED_TYPES>,
    assets: BoundedVec<AssetReference, MAX_OBJECT_ASSETS>,
    opaque_facets: OpaqueFacets,
    provenance: SourceProvenance,
}

impl<'de> Deserialize<'de> for CanonicalObject {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawCanonicalObject::deserialize(deserializer)?;
        Self::new(CanonicalObjectParts {
            identity: raw.identity,
            kind: raw.kind,
            owner: raw.owner,
            properties: raw.properties.0,
            references: raw.references.values,
            generated_types: raw.generated_types.values,
            assets: raw.assets.values,
            opaque_facets: raw.opaque_facets,
            provenance: raw.provenance,
        })
        .map_err(de::Error::custom)
    }
}

/// An ordered canonical configuration before graph validation.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct CanonicalConfiguration {
    objects: Vec<CanonicalObject>,
}

impl CanonicalConfiguration {
    /// Validates global count and aggregate retained-byte bounds without reordering.
    pub fn new(objects: Vec<CanonicalObject>) -> Result<Self, ModelBuildError> {
        validate_item_count(
            "configuration objects",
            objects.len(),
            MAX_CONFIGURATION_OBJECTS,
        )?;
        validate_configuration_budgets(&objects)?;
        Ok(Self { objects })
    }

    /// Returns objects in exact declared source order.
    pub fn objects(&self) -> &[CanonicalObject] {
        &self.objects
    }

    /// Returns the number of canonical objects.
    pub const fn len(&self) -> usize {
        self.objects.len()
    }

    /// Returns whether the configuration contains no objects.
    pub const fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }

    /// Consumes the configuration without changing object order.
    pub fn into_objects(self) -> Vec<CanonicalObject> {
        self.objects
    }
}

fn validate_configuration_budgets(objects: &[CanonicalObject]) -> Result<(), ModelBuildError> {
    let mut members = 0_usize;
    let mut retained = 0_usize;
    for object in objects {
        members = members
            .checked_add(object.member_count()?)
            .ok_or(ModelBuildError::CountOverflow)?;
        if members > MAX_CONFIGURATION_MEMBERS {
            return Err(ModelBuildError::TooManyMembers {
                scope: "canonical configuration",
                maximum: MAX_CONFIGURATION_MEMBERS,
                actual: members,
            });
        }
        retained = checked_add_retained(retained, object.retained_byte_len())?;
        enforce_retained_budget(
            "canonical configuration",
            retained,
            MAX_CONFIGURATION_RETAINED_BYTES,
        )?;
    }
    Ok(())
}

struct BoundedObjects(Vec<CanonicalObject>);

struct BoundedObjectsVisitor;

impl<'de> Visitor<'de> for BoundedObjectsVisitor {
    type Value = BoundedObjects;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "at most {MAX_CONFIGURATION_OBJECTS} ordered canonical objects within graph budgets"
        )
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut objects = Vec::with_capacity(
            sequence
                .size_hint()
                .unwrap_or_default()
                .min(MAX_CONFIGURATION_OBJECTS),
        );
        let mut members = 0_usize;
        let mut retained = 0_usize;
        while objects.len() < MAX_CONFIGURATION_OBJECTS {
            let Some(object) = sequence.next_element::<CanonicalObject>()? else {
                return Ok(BoundedObjects(objects));
            };
            members = members
                .checked_add(object.member_count().map_err(de::Error::custom)?)
                .ok_or_else(|| de::Error::custom(ModelBuildError::CountOverflow))?;
            if members > MAX_CONFIGURATION_MEMBERS {
                return Err(de::Error::custom(ModelBuildError::TooManyMembers {
                    scope: "canonical configuration",
                    maximum: MAX_CONFIGURATION_MEMBERS,
                    actual: members,
                }));
            }
            retained = checked_add_retained(retained, object.retained_byte_len())
                .map_err(de::Error::custom)?;
            enforce_retained_budget(
                "canonical configuration",
                retained,
                MAX_CONFIGURATION_RETAINED_BYTES,
            )
            .map_err(de::Error::custom)?;
            objects.push(object);
        }
        if sequence.next_element::<IgnoredAny>()?.is_some() {
            return Err(de::Error::custom(ModelBuildError::TooManyItems {
                field: "configuration objects",
                maximum: MAX_CONFIGURATION_OBJECTS,
                actual: MAX_CONFIGURATION_OBJECTS + 1,
            }));
        }
        Ok(BoundedObjects(objects))
    }
}

impl<'de> Deserialize<'de> for BoundedObjects {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(BoundedObjectsVisitor)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCanonicalConfiguration {
    objects: BoundedObjects,
}

impl<'de> Deserialize<'de> for CanonicalConfiguration {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawCanonicalConfiguration::deserialize(deserializer)?;
        Self::new(raw.objects.0).map_err(de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use crate::artifact::ProfileId;
    use crate::diagnostic::{ObjectPath, PathSegment, PropertyPath};
    use crate::identity::ObjectUuid;
    use crate::provenance::CanonicalAnchor;
    use crate::value::{CanonicalInteger, CanonicalValue};

    use super::*;

    fn uuid(value: &str) -> ObjectUuid {
        ObjectUuid::parse(value).unwrap()
    }

    fn identity(id: &str, name: &str) -> LogicalIdentity {
        LogicalIdentity::new(
            uuid(id),
            ObjectPath::new(vec![PathSegment::name(name).unwrap()]).unwrap(),
        )
    }

    fn provenance(id: &str, name: &str) -> SourceProvenance {
        SourceProvenance::new(
            ProfileId::parse("profile:test").unwrap(),
            CanonicalAnchor::new(identity(id, name).path().clone(), PropertyPath::root()),
        )
    }

    fn parts() -> CanonicalObjectParts {
        let id = "00000000-0000-0000-0000-000000000001";
        CanonicalObjectParts::new(
            identity(id, "one"),
            MetadataKind::new("Catalog").unwrap(),
            provenance(id, "one"),
        )
    }

    #[test]
    fn ordered_object_and_configuration_round_trip() {
        let mut parts = parts();
        parts.properties = vec![
            CanonicalField::named(
                "z",
                CanonicalValue::integer(CanonicalInteger::new("2").unwrap()),
            )
            .unwrap(),
            CanonicalField::named(
                "a",
                CanonicalValue::integer(CanonicalInteger::new("1").unwrap()),
            )
            .unwrap(),
        ];
        parts.references.push(ObjectReference::new(
            ReferenceKind::new("link").unwrap(),
            uuid("00000000-0000-0000-0000-000000000002"),
        ));
        let object = CanonicalObject::new(parts).unwrap();
        assert_eq!(object.properties()[0].name().as_str(), "z");
        let configuration = CanonicalConfiguration::new(vec![object]).unwrap();
        let json = serde_json::to_string(&configuration).unwrap();
        assert_eq!(
            serde_json::from_str::<CanonicalConfiguration>(&json).unwrap(),
            configuration
        );
    }

    #[test]
    fn generated_value_id_is_typed_and_old_json_remains_valid() {
        let type_id = uuid("11111111-1111-4111-8111-111111111111");
        let value_id = uuid("22222222-2222-4222-8222-222222222222");
        let legacy = GeneratedType::new(type_id, GeneratedTypeKind::new("Ref").unwrap());
        assert_eq!(legacy.value_id(), None);
        assert_eq!(
            serde_json::to_string(&legacy).unwrap(),
            r#"{"uuid":"11111111-1111-4111-8111-111111111111","kind":"Ref"}"#
        );
        let typed = legacy.with_value_id(value_id);
        assert_eq!(typed.value_id(), Some(value_id));
        assert_eq!(
            serde_json::from_str::<GeneratedType>(&serde_json::to_string(&typed).unwrap()).unwrap(),
            typed
        );
    }

    #[test]
    fn duplicate_properties_are_rejected_by_constructor_and_public_serde() {
        let mut duplicate = parts();
        duplicate.properties = vec![
            CanonicalField::named("same", CanonicalValue::null()).unwrap(),
            CanonicalField::named("same", CanonicalValue::boolean(true)).unwrap(),
        ];
        assert!(matches!(
            CanonicalObject::new(duplicate),
            Err(ModelBuildError::DuplicateProperty { name }) if name == "same"
        ));

        let valid = CanonicalObject::new(parts()).unwrap();
        let mut json = serde_json::to_value(valid).unwrap();
        json["properties"] = serde_json::json!([
            {"name":"same","value":{"null":null}},
            {"name":"same","value":{"bool":true}}
        ]);
        assert!(serde_json::from_value::<CanonicalObject>(json).is_err());
    }

    #[test]
    fn constructors_and_deserializers_enforce_kind_and_collection_bounds() {
        let oversized = "x".repeat(MAX_MODEL_KIND_BYTES + 1);
        assert!(MetadataKind::new(&oversized).is_err());
        assert!(serde_json::from_value::<MetadataKind>(serde_json::json!(oversized)).is_err());

        let target = uuid("00000000-0000-0000-0000-000000000002");
        let reference = ObjectReference::new(ReferenceKind::new("test").unwrap(), target);
        let json = serde_json::to_string(&vec![reference.clone(), reference]).unwrap();
        assert!(serde_json::from_str::<BoundedVec<ObjectReference, 1>>(&json).is_err());

        let mut oversized_object = parts();
        oversized_object.generated_types = (0..=MAX_GENERATED_TYPES)
            .map(|index| {
                let id = format!("00000000-0000-0000-0000-{index:012x}");
                GeneratedType::new(uuid(&id), GeneratedTypeKind::new("test").unwrap())
            })
            .collect();
        assert!(matches!(
            CanonicalObject::new(oversized_object),
            Err(ModelBuildError::TooManyItems {
                field: "object generated types",
                ..
            })
        ));
    }

    #[test]
    fn nested_zero_byte_values_count_toward_member_budgets() {
        let value = CanonicalValue::record(vec![
            CanonicalField::named(
                "nested",
                CanonicalValue::sequence(vec![
                    CanonicalValue::null(),
                    CanonicalValue::boolean(false),
                ])
                .unwrap(),
            )
            .unwrap(),
        ])
        .unwrap();
        assert_eq!(canonical_value_member_count(&value).unwrap(), 5);

        let property = CanonicalField::named("top", value).unwrap();
        assert_eq!(add_property_members(1, &property).unwrap(), 7);

        let mut object_parts = parts();
        object_parts.properties.push(property.clone());
        assert_eq!(
            CanonicalObject::new(object_parts).unwrap().member_count(),
            Ok(7)
        );

        let json = serde_json::to_string(&vec![property]).unwrap();
        let error = serde_json::from_str::<BoundedProperties<6>>(&json).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("canonical object exceeds 6 graph members (actual 7)")
        );
    }
}
