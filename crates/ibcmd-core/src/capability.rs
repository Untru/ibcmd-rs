//! Independent codec capabilities and preservation guarantees.

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use serde::de::{IgnoredAny, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, de};

use crate::profile::CapabilityId;

/// Maximum capability declarations retained by one codec descriptor.
pub const MAX_CAPABILITY_DECLARATIONS: usize = 1_024;

/// Stable identifier for structural inspection.
pub const CAPABILITY_INSPECT: &str = "inspect";
/// Stable identifier for lossless storage repacking.
pub const CAPABILITY_REPACK: &str = "repack";
/// Stable identifier for export into a semantic representation.
pub const CAPABILITY_EXPORT: &str = "export";
/// Stable identifier for applying changes over an existing base artifact.
pub const CAPABILITY_OVERLAY: &str = "overlay";
/// Stable identifier for creating an artifact without a base.
pub const CAPABILITY_BOOTSTRAP: &str = "bootstrap";
/// Stable identifier for conversion between exact profiles.
pub const CAPABILITY_CONVERT: &str = "convert";

fn stable_capability(value: &'static str) -> CapabilityId {
    CapabilityId::parse(value).expect("built-in capability identifiers are valid")
}

/// Returns the stable inspection capability identifier.
pub fn inspect_capability() -> CapabilityId {
    stable_capability(CAPABILITY_INSPECT)
}

/// Returns the stable repack capability identifier.
pub fn repack_capability() -> CapabilityId {
    stable_capability(CAPABILITY_REPACK)
}

/// Returns the stable export capability identifier.
pub fn export_capability() -> CapabilityId {
    stable_capability(CAPABILITY_EXPORT)
}

/// Returns the stable overlay capability identifier.
pub fn overlay_capability() -> CapabilityId {
    stable_capability(CAPABILITY_OVERLAY)
}

/// Returns the stable base-free bootstrap capability identifier.
pub fn bootstrap_capability() -> CapabilityId {
    stable_capability(CAPABILITY_BOOTSTRAP)
}

/// Returns the stable cross-profile conversion capability identifier.
pub fn convert_capability() -> CapabilityId {
    stable_capability(CAPABILITY_CONVERT)
}

/// How a codec implements one independent capability.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImplementationLevel {
    /// The operation is explicitly unavailable.
    Unsupported,
    /// The operation is implemented only when an existing base is supplied.
    NeedsBase,
    /// The operation is implemented without a base artifact.
    Compiled,
}

impl ImplementationLevel {
    /// Returns whether this implementation requires a caller-supplied base.
    pub const fn requires_base_blob(self) -> bool {
        matches!(self, Self::NeedsBase)
    }
}

/// Strongest preservation guarantee made by a capability declaration.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreservationLevel {
    /// No semantic or byte-identity guarantee is made.
    None,
    /// Canonical meaning is preserved.
    Semantic,
    /// Exact source bytes are preserved.
    ByteExact,
}

impl PreservationLevel {
    /// Returns whether this level satisfies the exact requested minimum.
    pub const fn satisfies(self, requested: Self) -> bool {
        self as u8 >= requested as u8
    }
}

/// Failure to build a bounded, internally consistent capability set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CapabilityBuildError {
    /// Too many declarations were supplied.
    TooManyDeclarations {
        /// Maximum accepted declarations.
        maximum: usize,
        /// Actual declarations, when known.
        actual: usize,
    },
    /// The same exact open identifier was declared more than once.
    DuplicateCapability {
        /// Duplicate identifier.
        capability: CapabilityId,
    },
    /// An unsupported capability claimed a preservation guarantee.
    UnsupportedPreservation {
        /// Invalid capability identifier.
        capability: CapabilityId,
        /// Invalid non-empty guarantee.
        preservation: PreservationLevel,
    },
}

impl Display for CapabilityBuildError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyDeclarations { maximum, actual } => write!(
                formatter,
                "capability set exceeds {maximum} declarations (actual {actual})"
            ),
            Self::DuplicateCapability { capability } => {
                write!(formatter, "duplicate capability declaration `{capability}`")
            }
            Self::UnsupportedPreservation {
                capability,
                preservation,
            } => write!(
                formatter,
                "unsupported capability `{capability}` cannot claim {preservation:?} preservation"
            ),
        }
    }
}

impl Error for CapabilityBuildError {}

/// One immutable declaration for an exact, independent capability identifier.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CapabilityDeclaration {
    capability: CapabilityId,
    implementation: ImplementationLevel,
    preservation: PreservationLevel,
}

impl CapabilityDeclaration {
    /// Validates and creates a declaration without inferring other capabilities.
    pub fn new(
        capability: CapabilityId,
        implementation: ImplementationLevel,
        preservation: PreservationLevel,
    ) -> Result<Self, CapabilityBuildError> {
        if implementation == ImplementationLevel::Unsupported
            && preservation != PreservationLevel::None
        {
            return Err(CapabilityBuildError::UnsupportedPreservation {
                capability,
                preservation,
            });
        }
        Ok(Self {
            capability,
            implementation,
            preservation,
        })
    }

    /// Creates an explicit unsupported declaration.
    pub fn unsupported(capability: CapabilityId) -> Self {
        Self {
            capability,
            implementation: ImplementationLevel::Unsupported,
            preservation: PreservationLevel::None,
        }
    }

    /// Returns the exact open capability identifier.
    pub const fn capability(&self) -> &CapabilityId {
        &self.capability
    }

    /// Returns the independently declared implementation level.
    pub const fn implementation(&self) -> ImplementationLevel {
        self.implementation
    }

    /// Returns the strongest declared preservation guarantee.
    pub const fn preservation(&self) -> PreservationLevel {
        self.preservation
    }

    /// Returns whether this declaration requires a base artifact.
    pub const fn requires_base_blob(&self) -> bool {
        self.implementation.requires_base_blob()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCapabilityDeclaration {
    capability: CapabilityId,
    implementation: ImplementationLevel,
    preservation: PreservationLevel,
}

impl<'de> Deserialize<'de> for CapabilityDeclaration {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawCapabilityDeclaration::deserialize(deserializer)?;
        Self::new(raw.capability, raw.implementation, raw.preservation).map_err(de::Error::custom)
    }
}

/// Result of evaluating one exact declaration against caller requirements.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapabilityEvaluation {
    /// The exact declaration satisfies preservation and base requirements.
    Available {
        /// Implementation used for the operation.
        implementation: ImplementationLevel,
        /// Strongest guarantee made by the codec.
        preservation: PreservationLevel,
    },
    /// No declaration exists for the exact identifier.
    Undeclared,
    /// The exact capability is explicitly unsupported.
    Unsupported,
    /// The exact capability requires a base, but none was supplied.
    BaseRequired,
    /// The implementation cannot satisfy the requested preservation level.
    InsufficientPreservation {
        /// Strongest available guarantee.
        available: PreservationLevel,
        /// Exact caller requirement.
        requested: PreservationLevel,
    },
}

impl CapabilityEvaluation {
    /// Returns whether the operation may proceed.
    pub const fn is_available(self) -> bool {
        matches!(self, Self::Available { .. })
    }
}

/// Bounded declarations sorted by exact capability identifier.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct CapabilitySet {
    declarations: Vec<CapabilityDeclaration>,
}

impl CapabilitySet {
    /// Validates, sorts, and retains declarations without last-wins behavior.
    pub fn new(mut declarations: Vec<CapabilityDeclaration>) -> Result<Self, CapabilityBuildError> {
        if declarations.len() > MAX_CAPABILITY_DECLARATIONS {
            return Err(CapabilityBuildError::TooManyDeclarations {
                maximum: MAX_CAPABILITY_DECLARATIONS,
                actual: declarations.len(),
            });
        }
        declarations.sort_by(|left, right| left.capability.cmp(&right.capability));
        if let Some(pair) = declarations
            .windows(2)
            .find(|pair| pair[0].capability == pair[1].capability)
        {
            return Err(CapabilityBuildError::DuplicateCapability {
                capability: pair[0].capability.clone(),
            });
        }
        Ok(Self { declarations })
    }

    /// Returns declarations in deterministic identifier order.
    pub fn declarations(&self) -> &[CapabilityDeclaration] {
        &self.declarations
    }

    /// Returns the declaration for an exact identifier.
    pub fn get(&self, capability: &CapabilityId) -> Option<&CapabilityDeclaration> {
        self.declarations
            .binary_search_by(|declaration| declaration.capability.cmp(capability))
            .ok()
            .map(|index| &self.declarations[index])
    }

    /// Evaluates only the exact requested capability, preservation, and base state.
    pub fn evaluate(
        &self,
        capability: &CapabilityId,
        requested: PreservationLevel,
        base_available: bool,
    ) -> CapabilityEvaluation {
        let Some(declaration) = self.get(capability) else {
            return CapabilityEvaluation::Undeclared;
        };
        match declaration.implementation {
            ImplementationLevel::Unsupported => CapabilityEvaluation::Unsupported,
            ImplementationLevel::NeedsBase if !base_available => CapabilityEvaluation::BaseRequired,
            ImplementationLevel::NeedsBase | ImplementationLevel::Compiled => {
                if declaration.preservation.satisfies(requested) {
                    CapabilityEvaluation::Available {
                        implementation: declaration.implementation,
                        preservation: declaration.preservation,
                    }
                } else {
                    CapabilityEvaluation::InsufficientPreservation {
                        available: declaration.preservation,
                        requested,
                    }
                }
            }
        }
    }

    /// Returns the number of exact declarations.
    pub const fn len(&self) -> usize {
        self.declarations.len()
    }

    /// Returns whether no capabilities are declared.
    pub const fn is_empty(&self) -> bool {
        self.declarations.is_empty()
    }
}

struct BoundedDeclarations(Vec<CapabilityDeclaration>);

struct BoundedDeclarationsVisitor;

impl<'de> Visitor<'de> for BoundedDeclarationsVisitor {
    type Value = BoundedDeclarations;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "at most {MAX_CAPABILITY_DECLARATIONS} capability declarations"
        )
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut declarations = Vec::with_capacity(
            sequence
                .size_hint()
                .unwrap_or_default()
                .min(MAX_CAPABILITY_DECLARATIONS),
        );
        while declarations.len() < MAX_CAPABILITY_DECLARATIONS {
            let Some(declaration) = sequence.next_element::<CapabilityDeclaration>()? else {
                return Ok(BoundedDeclarations(declarations));
            };
            declarations.push(declaration);
        }
        if sequence.next_element::<IgnoredAny>()?.is_some() {
            return Err(de::Error::custom(
                CapabilityBuildError::TooManyDeclarations {
                    maximum: MAX_CAPABILITY_DECLARATIONS,
                    actual: MAX_CAPABILITY_DECLARATIONS + 1,
                },
            ));
        }
        Ok(BoundedDeclarations(declarations))
    }
}

impl<'de> Deserialize<'de> for BoundedDeclarations {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(BoundedDeclarationsVisitor)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCapabilitySet {
    declarations: BoundedDeclarations,
}

impl<'de> Deserialize<'de> for CapabilitySet {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawCapabilitySet::deserialize(deserializer)?;
        Self::new(raw.declarations.0).map_err(de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    fn declaration(
        capability: CapabilityId,
        implementation: ImplementationLevel,
        preservation: PreservationLevel,
    ) -> CapabilityDeclaration {
        CapabilityDeclaration::new(capability, implementation, preservation).unwrap()
    }

    #[test]
    fn stable_capabilities_are_distinct_independent_and_open() {
        let known = [
            inspect_capability(),
            repack_capability(),
            export_capability(),
            overlay_capability(),
            bootstrap_capability(),
            convert_capability(),
        ];
        assert_eq!(known.iter().collect::<BTreeSet<_>>().len(), known.len());

        let future = CapabilityId::parse("future:operation").unwrap();
        let set = CapabilitySet::new(vec![
            declaration(
                inspect_capability(),
                ImplementationLevel::Compiled,
                PreservationLevel::Semantic,
            ),
            declaration(
                future.clone(),
                ImplementationLevel::Compiled,
                PreservationLevel::Semantic,
            ),
        ])
        .unwrap();
        assert!(
            set.evaluate(&future, PreservationLevel::Semantic, false)
                .is_available()
        );
        assert!(
            set.evaluate(&inspect_capability(), PreservationLevel::Semantic, false)
                .is_available()
        );
        for capability in known
            .into_iter()
            .filter(|value| value != &inspect_capability())
        {
            assert_eq!(
                set.evaluate(&capability, PreservationLevel::None, true),
                CapabilityEvaluation::Undeclared
            );
        }
    }

    #[test]
    fn evaluation_checks_base_and_preservation_exactly() {
        let needs_base = repack_capability();
        let semantic = export_capability();
        let unsupported = bootstrap_capability();
        let set = CapabilitySet::new(vec![
            declaration(
                needs_base.clone(),
                ImplementationLevel::NeedsBase,
                PreservationLevel::ByteExact,
            ),
            declaration(
                semantic.clone(),
                ImplementationLevel::Compiled,
                PreservationLevel::Semantic,
            ),
            CapabilityDeclaration::unsupported(unsupported.clone()),
        ])
        .unwrap();

        assert_eq!(
            set.evaluate(&needs_base, PreservationLevel::Semantic, false),
            CapabilityEvaluation::BaseRequired
        );
        assert!(
            set.evaluate(&needs_base, PreservationLevel::Semantic, true)
                .is_available()
        );
        assert_eq!(
            set.evaluate(&semantic, PreservationLevel::ByteExact, true),
            CapabilityEvaluation::InsufficientPreservation {
                available: PreservationLevel::Semantic,
                requested: PreservationLevel::ByteExact,
            }
        );
        assert_eq!(
            set.evaluate(&unsupported, PreservationLevel::None, true),
            CapabilityEvaluation::Unsupported
        );
    }

    #[test]
    fn duplicate_and_invalid_declarations_fail_without_last_wins() {
        let first = declaration(
            inspect_capability(),
            ImplementationLevel::Compiled,
            PreservationLevel::Semantic,
        );
        let second = declaration(
            inspect_capability(),
            ImplementationLevel::NeedsBase,
            PreservationLevel::ByteExact,
        );
        assert!(matches!(
            CapabilitySet::new(vec![first.clone(), second]),
            Err(CapabilityBuildError::DuplicateCapability { capability })
                if capability == inspect_capability()
        ));
        assert!(
            CapabilityDeclaration::new(
                export_capability(),
                ImplementationLevel::Unsupported,
                PreservationLevel::Semantic,
            )
            .is_err()
        );

        let json = format!(
            "{{\"declarations\":[{},{}]}}",
            serde_json::to_string(&first).unwrap(),
            serde_json::to_string(&first).unwrap()
        );
        assert!(serde_json::from_str::<CapabilitySet>(&json).is_err());
    }

    #[test]
    fn public_serde_is_deterministic_and_streaming_bounded() {
        let set = CapabilitySet::new(vec![
            declaration(
                repack_capability(),
                ImplementationLevel::NeedsBase,
                PreservationLevel::ByteExact,
            ),
            declaration(
                inspect_capability(),
                ImplementationLevel::Compiled,
                PreservationLevel::None,
            ),
        ])
        .unwrap();
        let json = serde_json::to_string(&set).unwrap();
        assert!(json.find(CAPABILITY_INSPECT).unwrap() < json.find(CAPABILITY_REPACK).unwrap());
        assert_eq!(serde_json::from_str::<CapabilitySet>(&json).unwrap(), set);

        let declarations = (0..=MAX_CAPABILITY_DECLARATIONS)
            .map(|index| {
                format!(
                    "{{\"capability\":\"future:{index}\",\"implementation\":\"compiled\",\"preservation\":\"none\"}}"
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let oversized = format!("{{\"declarations\":[{declarations}]}}");
        assert!(serde_json::from_str::<CapabilitySet>(&oversized).is_err());
    }
}
