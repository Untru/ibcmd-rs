//! Open metadata-family descriptors and deterministic codec registration.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

use serde::de::Visitor;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use crate::artifact::{ParseIdentifierError, ProfileId};
use crate::capability::{CapabilityEvaluation, PreservationLevel};
use crate::profile::CapabilityId;

/// Maximum codec descriptors retained by one family registry.
pub const MAX_FAMILY_CODECS: usize = 4_096;

/// A bounded open metadata-family identifier.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FamilyId(ProfileId);

impl FamilyId {
    /// Validates a family identifier without consulting a closed registry.
    pub fn new(value: &str) -> Result<Self, ParseIdentifierError> {
        ProfileId::new(value).map(Self)
    }

    /// Parses a bounded open family identifier.
    pub fn parse(value: &str) -> Result<Self, ParseIdentifierError> {
        Self::new(value)
    }

    /// Returns the exact family identifier.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl Display for FamilyId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for FamilyId {
    type Err = ParseIdentifierError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl Serialize for FamilyId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for FamilyId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(FamilyIdVisitor)
    }
}

struct FamilyIdVisitor;

impl Visitor<'_> for FamilyIdVisitor {
    type Value = FamilyId;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("a bounded open family identifier")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        FamilyId::parse(value).map_err(E::custom)
    }
}

/// Direction whose exact profile coordinates are carried by a query.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CodecDirection {
    /// Decode from one exact source profile.
    Decode,
    /// Encode from one exact source profile into a mandatory exact target.
    Encode,
}

#[derive(Clone, Copy, Debug)]
enum ProfileRoute<'a> {
    Decode {
        source: &'a ProfileId,
    },
    Encode {
        source: &'a ProfileId,
        target: &'a ProfileId,
    },
}

/// Exact-profile query passed to a family descriptor.
///
/// Private route fields and separate constructors make a target mandatory for
/// encode while preventing a decode query from inventing one.
#[derive(Clone, Copy, Debug)]
pub struct FamilyCapabilityQuery<'a> {
    route: ProfileRoute<'a>,
    capability: &'a CapabilityId,
    preservation: PreservationLevel,
    base_available: bool,
}

impl<'a> FamilyCapabilityQuery<'a> {
    /// Creates a query for decoding one exact source profile.
    pub const fn for_decode(
        source_profile: &'a ProfileId,
        capability: &'a CapabilityId,
        preservation: PreservationLevel,
        base_available: bool,
    ) -> Self {
        Self {
            route: ProfileRoute::Decode {
                source: source_profile,
            },
            capability,
            preservation,
            base_available,
        }
    }

    /// Creates a query for encoding between two explicit exact profiles.
    pub const fn for_encode(
        source_profile: &'a ProfileId,
        target_profile: &'a ProfileId,
        capability: &'a CapabilityId,
        preservation: PreservationLevel,
        base_available: bool,
    ) -> Self {
        Self {
            route: ProfileRoute::Encode {
                source: source_profile,
                target: target_profile,
            },
            capability,
            preservation,
            base_available,
        }
    }

    /// Returns whether this is a decode or encode query.
    pub const fn direction(&self) -> CodecDirection {
        match self.route {
            ProfileRoute::Decode { .. } => CodecDirection::Decode,
            ProfileRoute::Encode { .. } => CodecDirection::Encode,
        }
    }

    /// Returns the exact source profile without nearest-profile selection.
    pub const fn source_profile(&self) -> &'a ProfileId {
        match self.route {
            ProfileRoute::Decode { source } | ProfileRoute::Encode { source, .. } => source,
        }
    }

    /// Returns the mandatory target for encode and no target for decode.
    pub const fn target_profile(&self) -> Option<&'a ProfileId> {
        match self.route {
            ProfileRoute::Decode { .. } => None,
            ProfileRoute::Encode { target, .. } => Some(target),
        }
    }

    /// Returns the exact independently requested operation identifier.
    pub const fn capability(&self) -> &'a CapabilityId {
        self.capability
    }

    /// Returns the exact requested preservation requirement.
    pub const fn preservation(&self) -> PreservationLevel {
        self.preservation
    }

    /// Returns whether the caller has supplied a base artifact.
    pub const fn base_available(&self) -> bool {
        self.base_available
    }
}

/// Object-safe descriptor implemented by format-specific family codecs.
pub trait FamilyCodec: Send + Sync {
    /// Returns the exact open family identifier registered for this codec.
    fn family_id(&self) -> &FamilyId;

    /// Evaluates one exact route and independent operation declaration.
    fn query_capability(&self, query: &FamilyCapabilityQuery<'_>) -> CapabilityEvaluation;
}

/// Failure to extend a bounded deterministic codec registry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CodecRegistryError {
    /// The exact family identifier is already registered.
    DuplicateFamily {
        /// Duplicate identifier that was not replaced.
        family: FamilyId,
    },
    /// The registry reached its explicit descriptor bound.
    TooManyCodecs {
        /// Maximum accepted registrations.
        maximum: usize,
    },
}

impl Display for CodecRegistryError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateFamily { family } => {
                write!(formatter, "duplicate family codec registration `{family}`")
            }
            Self::TooManyCodecs { maximum } => {
                write!(formatter, "codec registry exceeds {maximum} families")
            }
        }
    }
}

impl Error for CodecRegistryError {}

/// Immutable-key registry with deterministic exact-family lookup and order.
pub struct CodecRegistry<C: FamilyCodec + ?Sized> {
    codecs: BTreeMap<FamilyId, Box<C>>,
}

impl<C: FamilyCodec + ?Sized> CodecRegistry<C> {
    /// Creates an empty registry.
    pub const fn new() -> Self {
        Self {
            codecs: BTreeMap::new(),
        }
    }

    /// Registers one codec without replacing an exact existing family.
    pub fn register(&mut self, codec: Box<C>) -> Result<(), CodecRegistryError> {
        let family = codec.family_id().clone();
        if self.codecs.contains_key(&family) {
            return Err(CodecRegistryError::DuplicateFamily { family });
        }
        if self.codecs.len() == MAX_FAMILY_CODECS {
            return Err(CodecRegistryError::TooManyCodecs {
                maximum: MAX_FAMILY_CODECS,
            });
        }
        self.codecs.insert(family, codec);
        Ok(())
    }

    /// Returns the codec registered for one exact family identifier.
    pub fn get(&self, family: &FamilyId) -> Option<&C> {
        self.codecs.get(family).map(Box::as_ref)
    }

    /// Returns codecs in deterministic family-identifier order.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&FamilyId, &C)> {
        self.codecs
            .iter()
            .map(|(family, codec)| (family, codec.as_ref()))
    }

    /// Returns the number of exact family registrations.
    pub fn len(&self) -> usize {
        self.codecs.len()
    }

    /// Returns whether no family codecs are registered.
    pub fn is_empty(&self) -> bool {
        self.codecs.is_empty()
    }
}

impl<C: FamilyCodec + ?Sized> Default for CodecRegistry<C> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use crate::capability::{
        CapabilityDeclaration, CapabilitySet, ImplementationLevel, bootstrap_capability,
        inspect_capability,
    };

    use super::*;

    struct MockCodec {
        family: FamilyId,
        capabilities: CapabilitySet,
        source: ProfileId,
        target: Option<ProfileId>,
    }

    impl FamilyCodec for MockCodec {
        fn family_id(&self) -> &FamilyId {
            &self.family
        }

        fn query_capability(&self, query: &FamilyCapabilityQuery<'_>) -> CapabilityEvaluation {
            if query.source_profile() != &self.source
                || query.target_profile() != self.target.as_ref()
            {
                return CapabilityEvaluation::Undeclared;
            }
            self.capabilities.evaluate(
                query.capability(),
                query.preservation(),
                query.base_available(),
            )
        }
    }

    fn profile(value: &str) -> ProfileId {
        ProfileId::parse(value).unwrap()
    }

    fn family(value: &str) -> FamilyId {
        FamilyId::parse(value).unwrap()
    }

    fn codec(family_id: &str, source: &str, target: Option<&str>, available: bool) -> MockCodec {
        let capability = if available {
            CapabilityDeclaration::new(
                inspect_capability(),
                ImplementationLevel::Compiled,
                PreservationLevel::Semantic,
            )
            .unwrap()
        } else {
            CapabilityDeclaration::unsupported(inspect_capability())
        };
        MockCodec {
            family: family(family_id),
            capabilities: CapabilitySet::new(vec![capability]).unwrap(),
            source: profile(source),
            target: target.map(profile),
        }
    }

    #[test]
    fn encode_query_keeps_exact_source_and_mandatory_target_separate() {
        let source = profile("profile:source");
        let target = profile("profile:target");
        let operation = bootstrap_capability();
        let query = FamilyCapabilityQuery::for_encode(
            &source,
            &target,
            &operation,
            PreservationLevel::Semantic,
            false,
        );
        assert_eq!(query.direction(), CodecDirection::Encode);
        assert_eq!(query.source_profile(), &source);
        assert_eq!(query.target_profile(), Some(&target));
        assert_ne!(query.source_profile(), query.target_profile().unwrap());

        let decode =
            FamilyCapabilityQuery::for_decode(&source, &operation, PreservationLevel::None, false);
        assert_eq!(decode.direction(), CodecDirection::Decode);
        assert_eq!(decode.target_profile(), None);
    }

    #[test]
    fn trait_object_registry_is_sorted_and_uses_exact_lookup() {
        let mut registry = CodecRegistry::<dyn FamilyCodec>::new();
        registry
            .register(Box::new(codec("zeta", "profile:z", None, true)))
            .unwrap();
        registry
            .register(Box::new(codec("alpha", "profile:a", None, true)))
            .unwrap();
        registry
            .register(Box::new(codec("middle", "profile:m", None, true)))
            .unwrap();

        let ordered = registry
            .iter()
            .map(|(family, _)| family.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ordered, ["alpha", "middle", "zeta"]);
        assert!(registry.get(&family("alpha")).is_some());
        assert!(registry.get(&family("unknown")).is_none());
    }

    #[test]
    fn duplicate_registration_is_rejected_without_overwrite() {
        let mut registry = CodecRegistry::<dyn FamilyCodec>::new();
        registry
            .register(Box::new(codec("catalog", "profile:source", None, true)))
            .unwrap();
        let error = registry
            .register(Box::new(codec("catalog", "profile:source", None, false)))
            .unwrap_err();
        assert!(matches!(
            error,
            CodecRegistryError::DuplicateFamily { family }
                if family == FamilyId::parse("catalog").unwrap()
        ));
        assert_eq!(registry.len(), 1);

        let source = profile("profile:source");
        let operation = inspect_capability();
        let query = FamilyCapabilityQuery::for_decode(
            &source,
            &operation,
            PreservationLevel::Semantic,
            false,
        );
        assert!(
            registry
                .get(&family("catalog"))
                .unwrap()
                .query_capability(&query)
                .is_available()
        );
    }

    #[test]
    fn family_id_is_open_bounded_and_string_serialized() {
        let future = family("future:family");
        let json = serde_json::to_string(&future).unwrap();
        assert_eq!(json, "\"future:family\"");
        assert_eq!(serde_json::from_str::<FamilyId>(&json).unwrap(), future);
        assert!(FamilyId::new(&"x".repeat(crate::artifact::MAX_IDENTIFIER_BYTES + 1)).is_err());
    }
}
