//! Deterministic profile detection from independent, explicit observations.

use std::collections::{BTreeMap, btree_map::Entry};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

use serde::{Serialize, Serializer};

use crate::artifact::{ParseIdentifierError, ProfileId};
use crate::profile::{EffectiveProfile, ProfileRegistry};
use crate::version::{PlatformBuild, XmlDialect};

/// Maximum number of fingerprint observations accepted by one detection pass.
pub const MAX_OBSERVED_FINGERPRINTS: usize = 64;
/// Maximum encoded length of one fingerprint value.
pub const MAX_FINGERPRINT_VALUE_BYTES: usize = 256;

/// A bounded, open fingerprint key.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FingerprintKey(ProfileId);

impl FingerprintKey {
    /// Validates a fingerprint key before retaining it.
    pub fn new(input: &str) -> Result<Self, ParseIdentifierError> {
        ProfileId::new(input).map(Self)
    }

    /// Parses a fingerprint key.
    pub fn parse(input: &str) -> Result<Self, ParseIdentifierError> {
        Self::new(input)
    }

    /// Returns the exact validated key.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl Display for FingerprintKey {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for FingerprintKey {
    type Err = ParseIdentifierError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Self::new(input)
    }
}

impl Serialize for FingerprintKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

/// Bounded observations supplied to profile detection.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DetectionObservations {
    platform_build: Option<PlatformBuild>,
    xml_dialect: Option<XmlDialect>,
    fingerprints: BTreeMap<FingerprintKey, String>,
}

impl DetectionObservations {
    /// Validates and normalizes independent observations.
    ///
    /// Fingerprints are sorted by key. Duplicate keys are rejected rather than
    /// overwritten, and all strings are validated before being copied.
    pub fn try_new<I, K, V>(
        platform_build: Option<PlatformBuild>,
        xml_dialect: Option<XmlDialect>,
        fingerprints: I,
    ) -> Result<Self, ObservationError>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: AsRef<str>,
    {
        let mut normalized = BTreeMap::new();
        for (raw_key, raw_value) in fingerprints {
            let key = FingerprintKey::new(raw_key.as_ref())
                .map_err(ObservationError::InvalidFingerprintKey)?;
            if let Entry::Occupied(entry) = normalized.entry(key.clone()) {
                return Err(ObservationError::DuplicateFingerprint {
                    key: entry.key().clone(),
                });
            }
            if normalized.len() == MAX_OBSERVED_FINGERPRINTS {
                return Err(ObservationError::TooManyFingerprints {
                    maximum: MAX_OBSERVED_FINGERPRINTS,
                });
            }

            let value = raw_value.as_ref();
            if value.is_empty() {
                return Err(ObservationError::EmptyFingerprintValue { key });
            }
            if value.len() > MAX_FINGERPRINT_VALUE_BYTES {
                return Err(ObservationError::FingerprintValueTooLong {
                    key,
                    maximum: MAX_FINGERPRINT_VALUE_BYTES,
                });
            }
            if value.chars().any(char::is_control) {
                return Err(ObservationError::InvalidFingerprintValue { key });
            }
            normalized.insert(key, value.to_owned());
        }

        Ok(Self {
            platform_build,
            xml_dialect,
            fingerprints: normalized,
        })
    }

    /// Returns the exact observed platform build, when supplied.
    pub fn platform_build(&self) -> Option<&PlatformBuild> {
        self.platform_build.as_ref()
    }

    /// Returns the exact observed XML dialect, when supplied.
    pub fn xml_dialect(&self) -> Option<&XmlDialect> {
        self.xml_dialect.as_ref()
    }

    /// Returns normalized fingerprint observations in key order.
    pub fn fingerprints(&self) -> &BTreeMap<FingerprintKey, String> {
        &self.fingerprints
    }

    /// Returns whether no observation was supplied.
    pub fn is_empty(&self) -> bool {
        self.platform_build.is_none() && self.xml_dialect.is_none() && self.fingerprints.is_empty()
    }
}

/// Failure to construct bounded detection observations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ObservationError {
    /// A fingerprint key violates the shared open-identifier grammar.
    InvalidFingerprintKey(ParseIdentifierError),
    /// The same fingerprint key was supplied more than once.
    DuplicateFingerprint {
        /// Duplicate key.
        key: FingerprintKey,
    },
    /// The fingerprint count exceeds the public resource bound.
    TooManyFingerprints {
        /// Maximum accepted count.
        maximum: usize,
    },
    /// A fingerprint value is empty.
    EmptyFingerprintValue {
        /// Affected key.
        key: FingerprintKey,
    },
    /// A fingerprint value exceeds the public resource bound.
    FingerprintValueTooLong {
        /// Affected key.
        key: FingerprintKey,
        /// Maximum accepted encoded length.
        maximum: usize,
    },
    /// A fingerprint value contains a control character.
    InvalidFingerprintValue {
        /// Affected key.
        key: FingerprintKey,
    },
}

impl Display for ObservationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFingerprintKey(error) => {
                write!(formatter, "invalid fingerprint key: {error}")
            }
            Self::DuplicateFingerprint { key } => {
                write!(formatter, "duplicate fingerprint observation `{key}`")
            }
            Self::TooManyFingerprints { maximum } => {
                write!(formatter, "fingerprint observation count exceeds {maximum}")
            }
            Self::EmptyFingerprintValue { key } => {
                write!(formatter, "fingerprint observation `{key}` is empty")
            }
            Self::FingerprintValueTooLong { key, maximum } => write!(
                formatter,
                "fingerprint observation `{key}` exceeds {maximum} bytes"
            ),
            Self::InvalidFingerprintValue { key } => write!(
                formatter,
                "fingerprint observation `{key}` contains a control character"
            ),
        }
    }
}

impl Error for ObservationError {}

/// One observation matched to the declaration that supplied its value.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MatchedObservation {
    /// Exact platform build match.
    PlatformBuild {
        /// Matched value.
        value: PlatformBuild,
        /// Profile that declared the effective value.
        declared_by: ProfileId,
    },
    /// Exact XML dialect match.
    XmlDialect {
        /// Matched value.
        value: XmlDialect,
        /// Profile that declared the effective value.
        declared_by: ProfileId,
    },
    /// Exact fingerprint match.
    Fingerprint {
        /// Matched fingerprint key.
        key: FingerprintKey,
        /// Matched fingerprint value.
        value: String,
        /// Profile that declared the effective value.
        declared_by: ProfileId,
    },
}

/// One deterministic detection candidate with full effective provenance.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DetectionCandidate {
    /// Fully resolved profile and its scalar/map/source provenance.
    pub profile: EffectiveProfile,
    /// Matched observations in platform, XML, then fingerprint-key order.
    pub matched: Vec<MatchedObservation>,
}

impl DetectionCandidate {
    /// Returns the candidate profile identifier.
    pub fn id(&self) -> &ProfileId {
        &self.profile.id
    }
}

/// Why no candidate was selected.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UnknownReason {
    /// Detection was asked to infer from no evidence, which is forbidden.
    NoObservations,
    /// No single profile explicitly matched every supplied observation.
    NoMatchingProfile,
}

/// Deterministic result of matching observations against a resolved registry.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum DetectionResult {
    /// Exactly one profile explicitly matched every observation.
    Exact {
        /// Unique candidate.
        candidate: Box<DetectionCandidate>,
    },
    /// More than one profile explicitly matched every observation.
    Ambiguous {
        /// Normalized input retained for diagnostics.
        observations: DetectionObservations,
        /// Candidates sorted by profile identifier.
        candidates: Vec<DetectionCandidate>,
    },
    /// No profile can be selected.
    Unknown {
        /// Normalized input retained for diagnostics.
        observations: DetectionObservations,
        /// Stable reason.
        reason: UnknownReason,
    },
}

/// Matches only exact, explicitly declared effective values.
///
/// The matcher never maps one version axis to another and never chooses a
/// nearest version. Contradictory observations therefore produce `Unknown`.
pub fn detect_profiles(
    registry: &ProfileRegistry,
    observations: &DetectionObservations,
) -> DetectionResult {
    if observations.is_empty() {
        return DetectionResult::Unknown {
            observations: observations.clone(),
            reason: UnknownReason::NoObservations,
        };
    }

    let mut candidates = Vec::new();
    for profile in registry.profiles().values() {
        if let Some(matched) = match_profile(profile, observations) {
            candidates.push(DetectionCandidate {
                profile: profile.clone(),
                matched,
            });
        }
    }

    match candidates.len() {
        0 => DetectionResult::Unknown {
            observations: observations.clone(),
            reason: UnknownReason::NoMatchingProfile,
        },
        1 => DetectionResult::Exact {
            candidate: Box::new(candidates.pop().expect("one candidate")),
        },
        _ => DetectionResult::Ambiguous {
            observations: observations.clone(),
            candidates,
        },
    }
}

fn match_profile(
    profile: &EffectiveProfile,
    observations: &DetectionObservations,
) -> Option<Vec<MatchedObservation>> {
    let mut matched = Vec::new();

    if let Some(observed) = observations.platform_build() {
        let declared = profile.platform_build.as_ref()?;
        if &declared.value != observed {
            return None;
        }
        matched.push(MatchedObservation::PlatformBuild {
            value: observed.clone(),
            declared_by: declared.declared_by.clone(),
        });
    }
    if let Some(observed) = observations.xml_dialect() {
        let declared = profile.xml_dialect.as_ref()?;
        if &declared.value != observed {
            return None;
        }
        matched.push(MatchedObservation::XmlDialect {
            value: observed.clone(),
            declared_by: declared.declared_by.clone(),
        });
    }
    for (key, observed) in observations.fingerprints() {
        let declared = profile.fingerprints.get(key.as_str())?;
        if &declared.value != observed {
            return None;
        }
        matched.push(MatchedObservation::Fingerprint {
            key: key.clone(),
            value: observed.clone(),
            declared_by: declared.declared_by.clone(),
        });
    }

    Some(matched)
}

/// Typed exact-target preflight failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RequireExactError {
    /// More than one candidate matched.
    Ambiguous {
        /// Candidate identifiers in deterministic order.
        candidate_ids: Vec<ProfileId>,
    },
    /// No candidate matched.
    Unknown {
        /// Stable unknown reason.
        reason: UnknownReason,
    },
}

impl Display for RequireExactError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ambiguous { candidate_ids } => {
                formatter.write_str("target profile is ambiguous: ")?;
                for (index, id) in candidate_ids.iter().enumerate() {
                    if index != 0 {
                        formatter.write_str(", ")?;
                    }
                    id.fmt(formatter)?;
                }
                Ok(())
            }
            Self::Unknown { reason } => write!(formatter, "target profile is unknown: {reason:?}"),
        }
    }
}

impl Error for RequireExactError {}

/// Requires a unique exact candidate before an encode/write operation.
pub fn require_exact_target(
    result: &DetectionResult,
) -> Result<&DetectionCandidate, RequireExactError> {
    match result {
        DetectionResult::Exact { candidate } => Ok(candidate.as_ref()),
        DetectionResult::Ambiguous { candidates, .. } => Err(RequireExactError::Ambiguous {
            candidate_ids: candidates
                .iter()
                .map(|candidate| candidate.id().clone())
                .collect(),
        }),
        DetectionResult::Unknown { reason, .. } => {
            Err(RequireExactError::Unknown { reason: *reason })
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::profile::{
        ProfileDocument, ProfileSourceKind, parse_profile_source, resolve_profiles,
    };

    use super::*;

    fn document(name: &str, json: &str) -> ProfileDocument {
        parse_profile_source(name, ProfileSourceKind::Bundled, json).unwrap()
    }

    fn observations(
        platform: Option<&str>,
        xml: Option<&str>,
        fingerprints: &[(&str, &str)],
    ) -> DetectionObservations {
        DetectionObservations::try_new(
            platform.map(|value| value.parse().unwrap()),
            xml.map(|value| value.parse().unwrap()),
            fingerprints.iter().copied(),
        )
        .unwrap()
    }

    #[test]
    fn xml_fingerprint_never_infers_a_platform_profile() {
        let xml = document(
            "xml.json",
            r#"{"schema_version":1,"id":"xml-2.20","status":"experimental","xml_dialect":"2.20","fingerprints":{"xcf.version":"2.20"}}"#,
        );
        let platform = document(
            "platform.json",
            r#"{"schema_version":1,"id":"platform-8.3.24","status":"experimental","platform_build":"8.3.24.1"}"#,
        );
        let registry = resolve_profiles([platform, xml]).unwrap();
        let result = detect_profiles(
            &registry,
            &observations(None, None, &[("xcf.version", "2.20")]),
        );
        let candidate = require_exact_target(&result).unwrap();

        assert_eq!(candidate.id().as_str(), "xml-2.20");
        assert!(candidate.profile.platform_build.is_none());
        assert_eq!(candidate.matched.len(), 1);
    }

    #[test]
    fn exact_platform_observation_selects_only_the_platform_profile() {
        let platform = document(
            "platform.json",
            r#"{"schema_version":1,"id":"platform","status":"experimental","platform_build":"8.3.27.1989"}"#,
        );
        let xml = document(
            "xml.json",
            r#"{"schema_version":1,"id":"xml","status":"experimental","xml_dialect":"2.21"}"#,
        );
        let registry = resolve_profiles([xml, platform]).unwrap();
        let result = detect_profiles(&registry, &observations(Some("8.3.27.1989"), None, &[]));
        assert_eq!(
            require_exact_target(&result).unwrap().id().as_str(),
            "platform"
        );
    }

    #[test]
    fn shared_fingerprint_is_ambiguous_and_sorted() {
        let z = document(
            "z.json",
            r#"{"schema_version":1,"id":"z-profile","status":"experimental","fingerprints":{"shared":"same"}}"#,
        );
        let a = document(
            "a.json",
            r#"{"schema_version":1,"id":"a-profile","status":"experimental","fingerprints":{"shared":"same"}}"#,
        );
        let registry = resolve_profiles([z, a]).unwrap();
        let result = detect_profiles(&registry, &observations(None, None, &[("shared", "same")]));

        let DetectionResult::Ambiguous { candidates, .. } = &result else {
            panic!("expected ambiguous result");
        };
        assert_eq!(
            candidates
                .iter()
                .map(|value| value.id().as_str())
                .collect::<Vec<_>>(),
            ["a-profile", "z-profile"]
        );
        assert!(matches!(
            require_exact_target(&result),
            Err(RequireExactError::Ambiguous { .. })
        ));
    }

    #[test]
    fn empty_unmatched_and_contradictory_observations_are_unknown() {
        let platform = document(
            "platform.json",
            r#"{"schema_version":1,"id":"platform","status":"experimental","platform_build":"8.3.27.1989"}"#,
        );
        let xml = document(
            "xml.json",
            r#"{"schema_version":1,"id":"xml","status":"experimental","xml_dialect":"2.21"}"#,
        );
        let registry = resolve_profiles([platform, xml]).unwrap();

        let empty = detect_profiles(&registry, &observations(None, None, &[]));
        assert!(matches!(
            empty,
            DetectionResult::Unknown {
                reason: UnknownReason::NoObservations,
                ..
            }
        ));
        let unmatched = detect_profiles(&registry, &observations(None, None, &[("missing", "x")]));
        assert!(matches!(
            unmatched,
            DetectionResult::Unknown {
                reason: UnknownReason::NoMatchingProfile,
                ..
            }
        ));
        let contradictory = detect_profiles(
            &registry,
            &observations(Some("8.3.27.1989"), Some("2.21"), &[]),
        );
        assert!(matches!(
            require_exact_target(&contradictory),
            Err(RequireExactError::Unknown {
                reason: UnknownReason::NoMatchingProfile
            })
        ));
    }

    #[test]
    fn future_external_profile_can_match_exactly() {
        let future = parse_profile_source(
            "external/future.json",
            ProfileSourceKind::External,
            r#"{"schema_version":1,"id":"future","status":"experimental","platform_build":"9.1.0.42","fingerprints":{"future.key":"future-value"}}"#,
        )
        .unwrap();
        let registry = resolve_profiles([future]).unwrap();
        let result = detect_profiles(
            &registry,
            &observations(Some("9.1.0.42"), None, &[("future.key", "future-value")]),
        );
        let candidate = require_exact_target(&result).unwrap();
        assert_eq!(candidate.id().as_str(), "future");
        assert_eq!(candidate.matched.len(), 2);
    }

    #[test]
    fn detection_is_deterministic_for_shuffled_registry_input() {
        let a_json = r#"{"schema_version":1,"id":"a","status":"experimental","fingerprints":{"key":"value"}}"#;
        let b_json = r#"{"schema_version":1,"id":"b","status":"experimental","fingerprints":{"key":"value"}}"#;
        let first =
            resolve_profiles([document("a.json", a_json), document("b.json", b_json)]).unwrap();
        let second =
            resolve_profiles([document("b.json", b_json), document("a.json", a_json)]).unwrap();
        let input = observations(None, None, &[("key", "value")]);
        assert_eq!(
            detect_profiles(&first, &input),
            detect_profiles(&second, &input)
        );
    }

    #[test]
    fn observation_constructor_rejects_duplicates_and_resource_excess() {
        assert!(matches!(
            DetectionObservations::try_new(None, None, [("same", "one"), ("same", "two")]),
            Err(ObservationError::DuplicateFingerprint { .. })
        ));

        let too_many = (0..=MAX_OBSERVED_FINGERPRINTS)
            .map(|index| (format!("key-{index}"), "value".to_owned()))
            .collect::<Vec<_>>();
        assert!(matches!(
            DetectionObservations::try_new(
                None,
                None,
                too_many
                    .iter()
                    .map(|(key, value)| (key.as_str(), value.as_str())),
            ),
            Err(ObservationError::TooManyFingerprints { .. })
        ));

        let too_long = "v".repeat(MAX_FINGERPRINT_VALUE_BYTES + 1);
        assert!(matches!(
            DetectionObservations::try_new(None, None, [("key", too_long.as_str())]),
            Err(ObservationError::FingerprintValueTooLong { .. })
        ));
    }
}
