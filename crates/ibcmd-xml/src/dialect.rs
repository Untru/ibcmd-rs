//! Profile-backed, open XCF dialect detection and description.
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use ibcmd_core::artifact::ProfileId;
use ibcmd_core::profile::{
    EffectiveProfile, ProfileRegistry, ProfileSourceKind, parse_profile_source, resolve_profiles,
};
use ibcmd_core::version::XmlDialect;

use crate::{AttributeKind, XmlDocument, XmlElement, XmlNode};

const MAX_DESCRIPTORS: usize = 256;
const MAX_FEATURES: usize = 256;
const MAX_ROOTS: usize = 256;
const MAX_MATCHERS: usize = 256;
const MAX_FIELD_BYTES: usize = 1_024;
const MAX_EVIDENCE: usize = 256;
const MAX_DEPTH: usize = 128;
const MAX_NODES: usize = 65_536;
const MAX_OPEN_ID_BYTES: usize = 64;
const XML_NAMESPACE: &str = "http://www.w3.org/XML/1998/namespace";
const XMLNS_NAMESPACE: &str = "http://www.w3.org/2000/xmlns/";
const MD_CLASSES: &str = "http://v8.1c.ru/8.3/MDClasses";
const EXTERNAL_PROPERTIES: &str = "http://v8.1c.ru/8.3/xcf/extrnprops";
const BASELINE_ROOTS: &[(&str, &str)] = &[
    ("MetaDataObject", MD_CLASSES),
    ("CommonAttribute", MD_CLASSES),
    ("DefinedType", MD_CLASSES),
    ("Form", "http://v8.1c.ru/8.3/xcf/logform"),
    ("Help", EXTERNAL_PROPERTIES),
    ("GraphicalSchema", EXTERNAL_PROPERTIES),
    ("Form", EXTERNAL_PROPERTIES),
    ("CommandInterface", EXTERNAL_PROPERTIES),
    ("ExchangePlanContent", EXTERNAL_PROPERTIES),
    ("ExtPicture", EXTERNAL_PROPERTIES),
    ("JobSchedule", EXTERNAL_PROPERTIES),
    ("Style", EXTERNAL_PROPERTIES),
    ("HomePageWorkArea", EXTERNAL_PROPERTIES),
    ("AccumulationRegisterAggregates", EXTERNAL_PROPERTIES),
    ("StandaloneContent", EXTERNAL_PROPERTIES),
    ("GraphicalSchema", "http://v8.1c.ru/8.3/xcf/gsrc"),
    ("GraphicalSchema", "http://v8.1c.ru/8.3/xcf/scheme"),
    ("PredefinedData", "http://v8.1c.ru/8.3/xcf/predef"),
    ("Rights", "http://v8.1c.ru/8.2/roles"),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParseDialectIdError {
    Empty,
    TooLong,
    InvalidCharacter,
}
impl Display for ParseDialectIdError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("dialect identifier is empty"),
            Self::TooLong => write!(f, "dialect identifier exceeds {MAX_OPEN_ID_BYTES} bytes"),
            Self::InvalidCharacter => {
                f.write_str("dialect identifier contains an invalid character")
            }
        }
    }
}
impl Error for ParseDialectIdError {}

fn validate_open_id(value: &str) -> Result<(), ParseDialectIdError> {
    if value.is_empty() {
        return Err(ParseDialectIdError::Empty);
    }
    if value.len() > MAX_OPEN_ID_BYTES {
        return Err(ParseDialectIdError::TooLong);
    }
    if !value
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'-' | b'.'))
    {
        return Err(ParseDialectIdError::InvalidCharacter);
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DialectFeature(Box<str>);
impl DialectFeature {
    pub fn parse(value: &str) -> Result<Self, ParseDialectIdError> {
        validate_open_id(value)?;
        Ok(Self(value.into()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn palette_namespace() -> Self {
        Self::parse("palette_namespace").expect("known feature")
    }
    pub fn use_in_interface_compatibility_mode() -> Self {
        Self::parse("use_in_interface_compatibility_mode").expect("known feature")
    }
}
impl Display for DialectFeature {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FeatureAvailability {
    Unknown,
    Supported,
    Unsupported,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuleProvenance {
    CommonBaseline,
    Profile(ProfileId),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialectRule<T> {
    value: T,
    provenance: RuleProvenance,
}
impl<T> DialectRule<T> {
    pub fn value(&self) -> &T {
        &self.value
    }
    pub fn provenance(&self) -> &RuleProvenance {
        &self.provenance
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum XmlEncoding {
    Utf8,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DialectLexicalPolicy {
    Preserve,
    Normalized,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BomRule {
    Preserve,
    Optional,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LineEndingRule {
    Preserve,
    Lf,
    CrLf,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PropertyOrderRule(Box<str>);
impl PropertyOrderRule {
    pub fn parse(value: &str) -> Result<Self, ParseDialectIdError> {
        validate_open_id(value)?;
        Ok(Self(value.into()))
    }
    pub fn source() -> Self {
        Self::parse("source").expect("known order")
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl Display for PropertyOrderRule {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LexicalRules {
    encoding: DialectRule<XmlEncoding>,
    policy: DialectRule<DialectLexicalPolicy>,
    bom: DialectRule<BomRule>,
    line_endings: DialectRule<LineEndingRule>,
}
impl LexicalRules {
    pub fn encoding(&self) -> &DialectRule<XmlEncoding> {
        &self.encoding
    }
    pub fn policy(&self) -> &DialectRule<DialectLexicalPolicy> {
        &self.policy
    }
    pub fn bom(&self) -> &DialectRule<BomRule> {
        &self.bom
    }
    pub fn line_endings(&self) -> &DialectRule<LineEndingRule> {
        &self.line_endings
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RootSignature {
    local: String,
    namespace: String,
    provenance: RuleProvenance,
}
impl RootSignature {
    pub fn local(&self) -> &str {
        &self.local
    }
    pub fn namespace(&self) -> &str {
        &self.namespace
    }
    pub fn provenance(&self) -> &RuleProvenance {
        &self.provenance
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NamespaceMatcher {
    feature: DialectFeature,
    uri: String,
    provenance: ProfileId,
}
impl NamespaceMatcher {
    pub fn feature(&self) -> &DialectFeature {
        &self.feature
    }
    pub fn uri(&self) -> &str {
        &self.uri
    }
    pub fn provenance(&self) -> &ProfileId {
        &self.provenance
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ElementMatcher {
    feature: DialectFeature,
    local: String,
    namespace: Option<String>,
    provenance: ProfileId,
}
impl ElementMatcher {
    pub fn feature(&self) -> &DialectFeature {
        &self.feature
    }
    pub fn local(&self) -> &str {
        &self.local
    }
    pub fn namespace(&self) -> Option<&str> {
        self.namespace.as_deref()
    }
    pub fn provenance(&self) -> &ProfileId {
        &self.provenance
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FeatureDeclaration {
    availability: FeatureAvailability,
    provenance: ProfileId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialectDescriptor {
    profile_id: ProfileId,
    dialect: XmlDialect,
    dialect_declared_by: ProfileId,
    roots: Vec<RootSignature>,
    features: BTreeMap<DialectFeature, FeatureDeclaration>,
    namespace_matchers: Vec<NamespaceMatcher>,
    element_matchers: Vec<ElementMatcher>,
    lexical: LexicalRules,
    order: DialectRule<PropertyOrderRule>,
}
impl DialectDescriptor {
    pub fn profile_id(&self) -> &ProfileId {
        &self.profile_id
    }
    pub fn dialect(&self) -> &XmlDialect {
        &self.dialect
    }
    pub fn dialect_declared_by(&self) -> &ProfileId {
        &self.dialect_declared_by
    }
    pub fn roots(&self) -> &[RootSignature] {
        &self.roots
    }
    pub fn lexical_rules(&self) -> &LexicalRules {
        &self.lexical
    }
    pub fn root_child_order(&self) -> &DialectRule<PropertyOrderRule> {
        &self.order
    }
    pub fn feature(&self, f: &DialectFeature) -> FeatureAvailability {
        self.features
            .get(f)
            .map_or(FeatureAvailability::Unknown, |x| x.availability)
    }
    pub fn feature_provenance(&self, f: &DialectFeature) -> Option<&ProfileId> {
        self.features.get(f).map(|x| &x.provenance)
    }
    pub fn namespace_matchers(&self) -> &[NamespaceMatcher] {
        &self.namespace_matchers
    }
    pub fn element_matchers(&self) -> &[ElementMatcher] {
        &self.element_matchers
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NamespaceEvidence {
    prefix: Option<String>,
    uri: String,
}
impl NamespaceEvidence {
    pub fn prefix(&self) -> Option<&str> {
        self.prefix.as_deref()
    }
    pub fn uri(&self) -> &str {
        &self.uri
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialectEvidence {
    root_local: String,
    root_namespace: Option<String>,
    version: Option<String>,
    namespaces: Vec<NamespaceEvidence>,
    features: BTreeSet<DialectFeature>,
}
impl DialectEvidence {
    pub fn root_local(&self) -> &str {
        &self.root_local
    }
    pub fn root_namespace(&self) -> Option<&str> {
        self.root_namespace.as_deref()
    }
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }
    pub fn namespaces(&self) -> &[NamespaceEvidence] {
        &self.namespaces
    }
    pub fn features(&self) -> &BTreeSet<DialectFeature> {
        &self.features
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DetectionCandidate {
    profile_id: ProfileId,
    dialect: XmlDialect,
    reasons: Vec<String>,
}
impl DetectionCandidate {
    pub fn profile_id(&self) -> &ProfileId {
        &self.profile_id
    }
    pub fn dialect(&self) -> &XmlDialect {
        &self.dialect
    }
    pub fn reasons(&self) -> &[String] {
        &self.reasons
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DialectDetection {
    Exact {
        candidate: DetectionCandidate,
        evidence: DialectEvidence,
    },
    Ambiguous {
        candidates: Vec<DetectionCandidate>,
        evidence: DialectEvidence,
    },
    Unknown {
        evidence: DialectEvidence,
    },
}
impl DialectDetection {
    pub fn evidence(&self) -> &DialectEvidence {
        match self {
            Self::Exact { evidence, .. }
            | Self::Ambiguous { evidence, .. }
            | Self::Unknown { evidence } => evidence,
        }
    }
}

#[derive(Debug)]
pub enum DialectError {
    Profile(String),
    MixedAxes {
        profile: ProfileId,
    },
    DuplicateDialect {
        dialect: XmlDialect,
        first: ProfileId,
        second: ProfileId,
    },
    MissingField {
        profile: ProfileId,
        key: &'static str,
    },
    VersionMismatch {
        profile: ProfileId,
        dialect: XmlDialect,
        fingerprint: String,
    },
    UnknownDescriptorKey {
        profile: ProfileId,
        key: String,
    },
    InvalidField {
        profile: ProfileId,
        key: String,
        value: String,
    },
    DuplicateRoot {
        profile: ProfileId,
        local: String,
        namespace: String,
    },
    DuplicateMatcher {
        profile: ProfileId,
        value: String,
    },
    MatcherWithoutFeatureDeclaration {
        profile: ProfileId,
        feature: DialectFeature,
    },
    TooManyDescriptors,
    TooManyFeatures {
        profile: ProfileId,
    },
    TooManyRoots {
        profile: ProfileId,
    },
    TooManyMatchers {
        profile: ProfileId,
    },
    EvidenceLimit,
    DuplicateVersion,
    DuplicateNamespace {
        element: String,
        prefix: Option<String>,
    },
    InvalidNamespace {
        element: String,
        prefix: Option<String>,
        uri: String,
    },
    UnboundPrefix {
        element: String,
        prefix: String,
    },
    InvalidVersion(String),
}
impl Display for DialectError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Profile(x) => write!(f, "profile error: {x}"),
            Self::MixedAxes { profile } => write!(
                f,
                "profile `{profile}` mixes XML dialect and platform build"
            ),
            Self::DuplicateDialect {
                dialect,
                first,
                second,
            } => write!(
                f,
                "duplicate dialect `{dialect}` in `{first}` and `{second}`"
            ),
            Self::MissingField { profile, key } => {
                write!(f, "profile `{profile}` is missing `{key}`")
            }
            Self::VersionMismatch {
                profile,
                dialect,
                fingerprint,
            } => write!(
                f,
                "profile `{profile}` dialect `{dialect}` disagrees with version fingerprint `{fingerprint}`"
            ),
            Self::UnknownDescriptorKey { profile, key } => write!(
                f,
                "profile `{profile}` declares unknown dialect key `{key}`"
            ),
            Self::InvalidField {
                profile,
                key,
                value,
            } => write!(f, "profile `{profile}` has invalid `{key}` value `{value}`"),
            Self::DuplicateRoot {
                profile,
                local,
                namespace,
            } => write!(
                f,
                "profile `{profile}` duplicates root signature `{local}|{namespace}`"
            ),
            Self::DuplicateMatcher { profile, value } => {
                write!(
                    f,
                    "profile `{profile}` duplicates evidence matcher `{value}`"
                )
            }
            Self::MatcherWithoutFeatureDeclaration { profile, feature } => write!(
                f,
                "profile `{profile}` matcher `{feature}` lacks a feature declaration"
            ),
            Self::TooManyDescriptors => f.write_str("too many dialect descriptors"),
            Self::TooManyFeatures { profile } => {
                write!(f, "profile `{profile}` declares too many dialect features")
            }
            Self::TooManyRoots { profile } => {
                write!(f, "profile `{profile}` declares too many root signatures")
            }
            Self::TooManyMatchers { profile } => {
                write!(f, "profile `{profile}` declares too many evidence matchers")
            }
            Self::EvidenceLimit => f.write_str("XML dialect evidence exceeds bounds"),
            Self::DuplicateVersion => f.write_str("duplicate root version evidence"),
            Self::DuplicateNamespace { element, prefix } => write!(
                f,
                "element `{element}` duplicates namespace declaration for `{}`",
                prefix.as_deref().unwrap_or("default")
            ),
            Self::InvalidNamespace {
                element,
                prefix,
                uri,
            } => write!(
                f,
                "element `{element}` has invalid namespace declaration `{}` = `{uri}`",
                prefix.as_deref().unwrap_or("default")
            ),
            Self::UnboundPrefix { element, prefix } => {
                write!(f, "element `{element}` uses unbound prefix `{prefix}`")
            }
            Self::InvalidVersion(v) => write!(f, "invalid root version evidence `{v}`"),
        }
    }
}
impl Error for DialectError {}

#[derive(Clone, Debug)]
pub struct DialectRegistry {
    descriptors: BTreeMap<XmlDialect, DialectDescriptor>,
}
impl DialectRegistry {
    pub fn from_profiles(registry: &ProfileRegistry) -> Result<Self, DialectError> {
        let mut out = BTreeMap::new();
        for p in registry.profiles().values() {
            let Some(axis) = &p.xml_dialect else { continue };
            if p.platform_build.is_some() {
                return Err(DialectError::MixedAxes {
                    profile: p.id.clone(),
                });
            }
            if out.len() >= MAX_DESCRIPTORS {
                return Err(DialectError::TooManyDescriptors);
            }
            let d = parse_descriptor(p)?;
            if let Some(old) = out.insert(axis.value.clone(), d) {
                return Err(DialectError::DuplicateDialect {
                    dialect: axis.value.clone(),
                    first: old.profile_id,
                    second: p.id.clone(),
                });
            }
        }
        Ok(Self { descriptors: out })
    }
    pub fn descriptors(&self) -> &BTreeMap<XmlDialect, DialectDescriptor> {
        &self.descriptors
    }
    pub fn get(&self, d: &XmlDialect) -> Option<&DialectDescriptor> {
        self.descriptors.get(d)
    }
    pub fn detect(&self, doc: &XmlDocument) -> Result<DialectDetection, DialectError> {
        let evidence = extract(doc, self, MAX_NODES)?;
        if let Some(version) = evidence.version() {
            let axis = XmlDialect::parse(version)
                .map_err(|_| DialectError::InvalidVersion(version.into()))?;
            if !self.descriptors.contains_key(&axis) {
                return Ok(DialectDetection::Unknown { evidence });
            }
        }
        let root_set = self.root_candidates(&evidence);
        if root_set.is_empty() {
            return Ok(DialectDetection::Unknown { evidence });
        }
        let mut candidates = BTreeMap::<XmlDialect, DetectionCandidate>::new();
        let mut groups = Vec::<BTreeSet<XmlDialect>>::new();
        if let Some(version) = evidence.version() {
            let axis = XmlDialect::parse(version).expect("validated");
            let d = &self.descriptors[&axis];
            add_candidate(&mut candidates, d, "root version");
            groups.push(BTreeSet::from([axis]));
        }
        for feature in &evidence.features {
            let matches = self
                .descriptors
                .values()
                .filter(|d| d.feature(feature) == FeatureAvailability::Supported)
                .map(|d| d.dialect.clone())
                .collect::<BTreeSet<_>>();
            for axis in &matches {
                let d = &self.descriptors[axis];
                add_candidate(&mut candidates, d, &format!("feature {feature}"));
            }
            if matches.is_empty() {
                return Ok(DialectDetection::Unknown { evidence });
            }
            groups.push(matches);
        }
        let final_set = if groups.is_empty() {
            for axis in &root_set {
                add_candidate(&mut candidates, &self.descriptors[axis], "root signature");
            }
            root_set
        } else {
            let mut intersection = groups[0].clone();
            let mut union = groups[0].clone();
            for group in &groups[1..] {
                intersection = intersection.intersection(group).cloned().collect();
                union.extend(group.iter().cloned());
            }
            let groups_conflict = intersection.is_empty();
            let strong = if groups_conflict { union } else { intersection };
            let compatible = strong
                .intersection(&root_set)
                .cloned()
                .collect::<BTreeSet<_>>();
            if compatible.is_empty() {
                for axis in &root_set {
                    add_candidate(
                        &mut candidates,
                        &self.descriptors[axis],
                        "conflicting root signature",
                    );
                }
                strong.union(&root_set).cloned().collect()
            } else if groups_conflict {
                strong
            } else {
                compatible
            }
        };
        candidates.retain(|axis, _| final_set.contains(axis));
        let values = candidates.into_values().collect::<Vec<_>>();
        Ok(match values.len() {
            0 => DialectDetection::Unknown { evidence },
            1 => DialectDetection::Exact {
                candidate: values.into_iter().next().expect("one"),
                evidence,
            },
            _ => DialectDetection::Ambiguous {
                candidates: values,
                evidence,
            },
        })
    }
    fn root_candidates(&self, e: &DialectEvidence) -> BTreeSet<XmlDialect> {
        self.descriptors
            .values()
            .filter(|d| {
                d.roots.iter().any(|r| {
                    r.local == e.root_local && Some(r.namespace.as_str()) == e.root_namespace()
                })
            })
            .map(|d| d.dialect.clone())
            .collect()
    }
}
fn add_candidate(
    map: &mut BTreeMap<XmlDialect, DetectionCandidate>,
    d: &DialectDescriptor,
    reason: &str,
) {
    map.entry(d.dialect.clone())
        .and_modify(|x| {
            if !x.reasons.iter().any(|r| r == reason) {
                x.reasons.push(reason.into())
            }
        })
        .or_insert_with(|| DetectionCandidate {
            profile_id: d.profile_id.clone(),
            dialect: d.dialect.clone(),
            reasons: vec![reason.into()],
        });
}

fn common_rule<T>(value: T) -> DialectRule<T> {
    DialectRule {
        value,
        provenance: RuleProvenance::CommonBaseline,
    }
}
fn override_rule<T: Clone>(
    p: &EffectiveProfile,
    key: &str,
    baseline: T,
    parse: fn(&str) -> Option<T>,
) -> Result<DialectRule<T>, DialectError> {
    match p.constants.get(key) {
        None => Ok(common_rule(baseline)),
        Some(x) => {
            if x.value.len() > MAX_FIELD_BYTES {
                return Err(invalid(p, key, &x.value));
            }
            let Some(value) = parse(&x.value) else {
                return Err(invalid(p, key, &x.value));
            };
            Ok(DialectRule {
                value,
                provenance: RuleProvenance::Profile(x.declared_by.clone()),
            })
        }
    }
}
fn invalid(p: &EffectiveProfile, key: &str, value: &str) -> DialectError {
    DialectError::InvalidField {
        profile: p.id.clone(),
        key: key.into(),
        value: value.into(),
    }
}
fn availability(v: &str) -> Option<FeatureAvailability> {
    match v {
        "supported" => Some(FeatureAvailability::Supported),
        "unsupported" => Some(FeatureAvailability::Unsupported),
        _ => None,
    }
}
fn encoding(v: &str) -> Option<XmlEncoding> {
    (v == "utf-8").then_some(XmlEncoding::Utf8)
}
fn policy(v: &str) -> Option<DialectLexicalPolicy> {
    match v {
        "preserve" => Some(DialectLexicalPolicy::Preserve),
        "normalized" => Some(DialectLexicalPolicy::Normalized),
        _ => None,
    }
}
fn bom(v: &str) -> Option<BomRule> {
    match v {
        "preserve" => Some(BomRule::Preserve),
        "optional" => Some(BomRule::Optional),
        _ => None,
    }
}
fn endings(v: &str) -> Option<LineEndingRule> {
    match v {
        "preserve" => Some(LineEndingRule::Preserve),
        "lf" => Some(LineEndingRule::Lf),
        "crlf" => Some(LineEndingRule::CrLf),
        _ => None,
    }
}
fn order(v: &str) -> Option<PropertyOrderRule> {
    PropertyOrderRule::parse(v).ok()
}

fn parse_descriptor(p: &EffectiveProfile) -> Result<DialectDescriptor, DialectError> {
    let axis = p.xml_dialect.as_ref().expect("filtered");
    let version = p
        .fingerprints
        .get("xcf.version")
        .ok_or_else(|| DialectError::MissingField {
            profile: p.id.clone(),
            key: "xcf.version",
        })?;
    if version.value != axis.value.to_string() {
        return Err(DialectError::VersionMismatch {
            profile: p.id.clone(),
            dialect: axis.value.clone(),
            fingerprint: version.value.clone(),
        });
    }
    let mut roots = BASELINE_ROOTS
        .iter()
        .map(|(local, namespace)| RootSignature {
            local: (*local).into(),
            namespace: (*namespace).into(),
            provenance: RuleProvenance::CommonBaseline,
        })
        .collect::<Vec<_>>();
    if p.constants
        .keys()
        .filter(|key| key.starts_with("xcf.feature."))
        .take(MAX_FEATURES + 1)
        .count()
        > MAX_FEATURES
    {
        return Err(DialectError::TooManyFeatures {
            profile: p.id.clone(),
        });
    }
    let mut features = BTreeMap::new();
    for (key, value) in &p.constants {
        if let Some(id) = key.strip_prefix("xcf.feature.") {
            let feature = DialectFeature::parse(id).map_err(|_| invalid(p, key, &value.value))?;
            let Some(state) = availability(&value.value) else {
                return Err(invalid(p, key, &value.value));
            };
            features.insert(
                feature,
                FeatureDeclaration {
                    availability: state,
                    provenance: value.declared_by.clone(),
                },
            );
        } else if key.starts_with("xcf.")
            && !matches!(
                key.as_str(),
                "xcf.lexical.encoding"
                    | "xcf.lexical.policy"
                    | "xcf.lexical.bom"
                    | "xcf.lexical.line_endings"
                    | "xcf.order.root_children"
            )
        {
            return Err(DialectError::UnknownDescriptorKey {
                profile: p.id.clone(),
                key: key.clone(),
            });
        }
    }
    let matcher_count = p
        .fingerprints
        .keys()
        .filter(|key| key.starts_with("xcf.namespace.") || key.starts_with("xcf.element."))
        .take(MAX_MATCHERS + 1)
        .count();
    if matcher_count > MAX_MATCHERS {
        return Err(DialectError::TooManyMatchers {
            profile: p.id.clone(),
        });
    }
    let mut namespace_matchers = vec![];
    let mut element_matchers = vec![];
    for (key, value) in &p.fingerprints {
        if key == "xcf.version" {
            continue;
        } else if key == "xcf.root" {
            if value
                .value
                .split(';')
                .take(MAX_ROOTS - roots.len() + 1)
                .count()
                > MAX_ROOTS - roots.len()
            {
                return Err(DialectError::TooManyRoots {
                    profile: p.id.clone(),
                });
            }
            for raw in value.value.split(';') {
                let Some((local, namespace)) = raw.split_once('|') else {
                    return Err(invalid(p, key, raw));
                };
                validate_root(p, key, local, namespace)?;
                if roots
                    .iter()
                    .any(|r| r.local == local && r.namespace == namespace)
                {
                    return Err(DialectError::DuplicateRoot {
                        profile: p.id.clone(),
                        local: local.into(),
                        namespace: namespace.into(),
                    });
                }
                if roots.len() >= MAX_ROOTS {
                    return Err(DialectError::TooManyRoots {
                        profile: p.id.clone(),
                    });
                }
                roots.push(RootSignature {
                    local: local.into(),
                    namespace: namespace.into(),
                    provenance: RuleProvenance::Profile(value.declared_by.clone()),
                });
            }
        } else if let Some(id) = key.strip_prefix("xcf.namespace.") {
            let feature = DialectFeature::parse(id).map_err(|_| invalid(p, key, &value.value))?;
            validate_uri(p, key, &value.value)?;
            namespace_matchers.push(NamespaceMatcher {
                feature,
                uri: value.value.clone(),
                provenance: value.declared_by.clone(),
            });
        } else if let Some(id) = key.strip_prefix("xcf.element.") {
            let feature = DialectFeature::parse(id).map_err(|_| invalid(p, key, &value.value))?;
            let Some((local, namespace)) = value.value.split_once('|') else {
                return Err(invalid(p, key, &value.value));
            };
            validate_local(p, key, local)?;
            if !namespace.is_empty() {
                validate_uri(p, key, namespace)?
            }
            element_matchers.push(ElementMatcher {
                feature,
                local: local.into(),
                namespace: (!namespace.is_empty()).then(|| namespace.into()),
                provenance: value.declared_by.clone(),
            });
        } else if key.starts_with("xcf.") {
            return Err(DialectError::UnknownDescriptorKey {
                profile: p.id.clone(),
                key: key.clone(),
            });
        }
    }
    namespace_matchers.sort_by(|a, b| a.feature.cmp(&b.feature).then(a.uri.cmp(&b.uri)));
    element_matchers.sort_by(|a, b| {
        a.feature
            .cmp(&b.feature)
            .then(a.local.cmp(&b.local))
            .then(a.namespace.cmp(&b.namespace))
    });
    let mut matcher_values = BTreeSet::new();
    for matcher in &namespace_matchers {
        let value = format!("namespace|{}", matcher.uri);
        if !matcher_values.insert(value.clone()) {
            return Err(DialectError::DuplicateMatcher {
                profile: p.id.clone(),
                value,
            });
        }
    }
    for matcher in &element_matchers {
        let value = format!(
            "element|{}|{}",
            matcher.local,
            matcher.namespace.as_deref().unwrap_or("")
        );
        if !matcher_values.insert(value.clone()) {
            return Err(DialectError::DuplicateMatcher {
                profile: p.id.clone(),
                value,
            });
        }
    }
    for feature in namespace_matchers
        .iter()
        .map(|m| &m.feature)
        .chain(element_matchers.iter().map(|m| &m.feature))
    {
        if !features.contains_key(feature) {
            return Err(DialectError::MatcherWithoutFeatureDeclaration {
                profile: p.id.clone(),
                feature: feature.clone(),
            });
        }
    }
    Ok(DialectDescriptor {
        profile_id: p.id.clone(),
        dialect: axis.value.clone(),
        dialect_declared_by: axis.declared_by.clone(),
        roots,
        features,
        namespace_matchers,
        element_matchers,
        lexical: LexicalRules {
            encoding: override_rule(p, "xcf.lexical.encoding", XmlEncoding::Utf8, encoding)?,
            policy: override_rule(
                p,
                "xcf.lexical.policy",
                DialectLexicalPolicy::Preserve,
                policy,
            )?,
            bom: override_rule(p, "xcf.lexical.bom", BomRule::Preserve, bom)?,
            line_endings: override_rule(
                p,
                "xcf.lexical.line_endings",
                LineEndingRule::Preserve,
                endings,
            )?,
        },
        order: override_rule(
            p,
            "xcf.order.root_children",
            PropertyOrderRule::source(),
            order,
        )?,
    })
}
fn validate_local(p: &EffectiveProfile, key: &str, local: &str) -> Result<(), DialectError> {
    if local.len() > MAX_FIELD_BYTES || local.contains(':') || crate::QName::new(local).is_err() {
        return Err(invalid(p, key, local));
    }
    Ok(())
}
fn validate_uri(p: &EffectiveProfile, key: &str, uri: &str) -> Result<(), DialectError> {
    if uri.is_empty() || uri.len() > MAX_FIELD_BYTES || uri.chars().any(char::is_control) {
        return Err(invalid(p, key, uri));
    }
    Ok(())
}
fn validate_root(
    p: &EffectiveProfile,
    key: &str,
    local: &str,
    namespace: &str,
) -> Result<(), DialectError> {
    validate_local(p, key, local)?;
    validate_uri(p, key, namespace)
}

#[derive(Default)]
struct Matchers {
    namespaces: BTreeMap<DialectFeature, BTreeSet<String>>,
    elements: BTreeMap<DialectFeature, BTreeSet<(String, Option<String>)>>,
}
fn extract(
    doc: &XmlDocument,
    registry: &DialectRegistry,
    node_limit: usize,
) -> Result<DialectEvidence, DialectError> {
    let root = doc.root();
    validate_qname_bounds(root.name())?;
    let mut version = None;
    for attribute in root.attributes() {
        if matches!(attribute.kind(),AttributeKind::Ordinary(name) if name.prefix().is_none()&&name.local()=="version")
        {
            if version.is_some() {
                return Err(DialectError::DuplicateVersion);
            }
            if attribute.value().len() > MAX_FIELD_BYTES {
                return Err(DialectError::EvidenceLimit);
            }
            version = Some(attribute.value().to_owned());
        }
    }
    let base = BTreeMap::from([(Some("xml".to_owned()), XML_NAMESPACE.to_owned())]);
    let (root_scope, root_declarations) = scope(root, &base)?;
    let root_namespace = effective_namespace(root, &root_scope);
    let recognized = registry
        .descriptors
        .values()
        .filter(|d| {
            d.roots.iter().any(|r| {
                r.local == root.name().local()
                    && Some(r.namespace.as_str()) == root_namespace.as_deref()
            })
        })
        .collect::<Vec<_>>();
    let mut matchers = Matchers::default();
    for d in recognized {
        for m in &d.namespace_matchers {
            matchers
                .namespaces
                .entry(m.feature.clone())
                .or_default()
                .insert(m.uri.clone());
        }
        for m in &d.element_matchers {
            matchers
                .elements
                .entry(m.feature.clone())
                .or_default()
                .insert((m.local.clone(), m.namespace.clone()));
        }
    }
    let mut evidence = DialectEvidence {
        root_local: root.name().local().into(),
        root_namespace,
        version,
        namespaces: vec![],
        features: BTreeSet::new(),
    };
    let mut visited = 0;
    walk(
        root,
        &root_scope,
        &root_declarations,
        &matchers,
        0,
        node_limit,
        &mut visited,
        &mut evidence,
    )?;
    evidence
        .namespaces
        .sort_by(|a, b| a.prefix.cmp(&b.prefix).then(a.uri.cmp(&b.uri)));
    Ok(evidence)
}
type NamespaceScope = BTreeMap<Option<String>, String>;
fn validate_qname_bounds(name: &crate::QName) -> Result<(), DialectError> {
    if name.raw().len() > MAX_FIELD_BYTES
        || name.local().len() > MAX_FIELD_BYTES
        || name
            .prefix()
            .is_some_and(|prefix| prefix.len() > MAX_FIELD_BYTES)
    {
        return Err(DialectError::EvidenceLimit);
    }
    Ok(())
}
fn scope(
    element: &XmlElement,
    parent: &NamespaceScope,
) -> Result<(NamespaceScope, Vec<NamespaceEvidence>), DialectError> {
    validate_qname_bounds(element.name())?;
    for attribute in element.attributes() {
        match attribute.kind() {
            AttributeKind::Ordinary(name) => validate_qname_bounds(name)?,
            AttributeKind::Namespace(prefix) => {
                if prefix
                    .as_deref()
                    .is_some_and(|prefix| prefix.len() > MAX_FIELD_BYTES)
                    || attribute.value().len() > MAX_FIELD_BYTES
                {
                    return Err(DialectError::EvidenceLimit);
                }
            }
        }
    }
    if parent.len() > MAX_EVIDENCE {
        return Err(DialectError::EvidenceLimit);
    }
    let mut result = parent.clone();
    let mut seen = BTreeSet::new();
    let mut declarations = vec![];
    for a in element.attributes() {
        if let AttributeKind::Namespace(prefix) = a.kind() {
            if declarations.len() >= MAX_EVIDENCE {
                return Err(DialectError::EvidenceLimit);
            }
            if !seen.insert(prefix.clone()) {
                return Err(DialectError::DuplicateNamespace {
                    element: element.name().raw().into(),
                    prefix: prefix.clone(),
                });
            }
            if prefix
                .as_deref()
                .is_some_and(|p| p.contains(':') || crate::QName::new(p).is_err())
                || a.value().chars().any(char::is_control)
                || (prefix.is_some() && a.value().is_empty())
            {
                return Err(DialectError::InvalidNamespace {
                    element: element.name().raw().into(),
                    prefix: prefix.clone(),
                    uri: a.value().into(),
                });
            }
            if prefix.as_deref() == Some("xmlns")
                || a.value() == XMLNS_NAMESPACE
                || (prefix.as_deref() == Some("xml") && a.value() != XML_NAMESPACE)
                || (prefix.as_deref() != Some("xml") && a.value() == XML_NAMESPACE)
            {
                return Err(DialectError::InvalidNamespace {
                    element: element.name().raw().into(),
                    prefix: prefix.clone(),
                    uri: a.value().into(),
                });
            }
            if a.value().is_empty() {
                result.remove(prefix);
            } else {
                if !result.contains_key(prefix) && result.len() >= MAX_EVIDENCE {
                    return Err(DialectError::EvidenceLimit);
                }
                result.insert(prefix.clone(), a.value().into());
            }
            declarations.push(NamespaceEvidence {
                prefix: prefix.clone(),
                uri: a.value().into(),
            });
        }
    }
    validate_bound_names(element, &result)?;
    Ok((result, declarations))
}
fn validate_bound_names(element: &XmlElement, scope: &NamespaceScope) -> Result<(), DialectError> {
    if let Some(prefix) = element.name().prefix() {
        if prefix == "xmlns" {
            return Err(DialectError::InvalidNamespace {
                element: element.name().raw().into(),
                prefix: Some(prefix.into()),
                uri: String::new(),
            });
        }
        if !scope.contains_key(&Some(prefix.into())) {
            return Err(DialectError::UnboundPrefix {
                element: element.name().raw().into(),
                prefix: prefix.into(),
            });
        }
    }
    for attribute in element.attributes() {
        if let AttributeKind::Ordinary(name) = attribute.kind() {
            if name.raw() == "xmlns" || name.prefix() == Some("xmlns") {
                if attribute.value().len() > MAX_FIELD_BYTES {
                    return Err(DialectError::EvidenceLimit);
                }
                return Err(DialectError::InvalidNamespace {
                    element: element.name().raw().into(),
                    prefix: name.prefix().map(str::to_owned),
                    uri: attribute.value().into(),
                });
            }
            if let Some(prefix) = name.prefix()
                && !scope.contains_key(&Some(prefix.into()))
            {
                return Err(DialectError::UnboundPrefix {
                    element: element.name().raw().into(),
                    prefix: prefix.into(),
                });
            }
        }
    }
    Ok(())
}
fn effective_namespace(element: &XmlElement, scope: &NamespaceScope) -> Option<String> {
    scope
        .get(&element.name().prefix().map(str::to_owned))
        .cloned()
}
#[allow(clippy::too_many_arguments)]
fn walk(
    element: &XmlElement,
    current_scope: &NamespaceScope,
    declarations: &[NamespaceEvidence],
    matchers: &Matchers,
    depth: usize,
    node_limit: usize,
    visited: &mut usize,
    evidence: &mut DialectEvidence,
) -> Result<(), DialectError> {
    *visited = visited.checked_add(1).ok_or(DialectError::EvidenceLimit)?;
    if depth > MAX_DEPTH || *visited > node_limit {
        return Err(DialectError::EvidenceLimit);
    }
    for declaration in declarations {
        if evidence.namespaces.len() >= MAX_EVIDENCE {
            return Err(DialectError::EvidenceLimit);
        }
        evidence.namespaces.push(declaration.clone());
        for (feature, uris) in &matchers.namespaces {
            if uris.contains(&declaration.uri) {
                record_feature(evidence, feature)?;
            }
        }
    }
    let effective = effective_namespace(element, current_scope);
    if element.name().prefix().is_none() || effective.is_some() {
        for (feature, elements) in &matchers.elements {
            if elements.contains(&(element.name().local().into(), effective.clone())) {
                record_feature(evidence, feature)?;
            }
        }
    }
    for node in element.children() {
        if let XmlNode::Element(child) = node {
            let (child_scope, child_declarations) = scope(child, current_scope)?;
            walk(
                child,
                &child_scope,
                &child_declarations,
                matchers,
                depth + 1,
                node_limit,
                visited,
                evidence,
            )?
        }
    }
    Ok(())
}
fn record_feature(
    evidence: &mut DialectEvidence,
    feature: &DialectFeature,
) -> Result<(), DialectError> {
    record_feature_with_limit(evidence, feature, MAX_EVIDENCE)
}
fn record_feature_with_limit(
    evidence: &mut DialectEvidence,
    feature: &DialectFeature,
    maximum: usize,
) -> Result<(), DialectError> {
    if !evidence.features.contains(feature) && evidence.features.len() >= maximum {
        return Err(DialectError::EvidenceLimit);
    }
    evidence.features.insert(feature.clone());
    Ok(())
}

fn bundled_profiles() -> Result<ProfileRegistry, DialectError> {
    let docs = [
        ("2.17", include_str!("../../../profiles/xml/2.17.json")),
        ("2.20", include_str!("../../../profiles/xml/2.20.json")),
        ("2.21", include_str!("../../../profiles/xml/2.21.json")),
    ]
    .into_iter()
    .map(|(name, json)| {
        parse_profile_source(name, ProfileSourceKind::Bundled, json)
            .map_err(|e| DialectError::Profile(e.to_string()))
    })
    .collect::<Result<Vec<_>, _>>()?;
    resolve_profiles(docs).map_err(|e| DialectError::Profile(e.to_string()))
}
pub fn bundled_dialect_registry() -> Result<DialectRegistry, DialectError> {
    DialectRegistry::from_profiles(&bundled_profiles()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Attribute, QName, XmlReader};
    fn detect(xml: &str) -> DialectDetection {
        bundled_dialect_registry()
            .unwrap()
            .detect(&XmlReader::from_slice(xml.as_bytes()).unwrap())
            .unwrap()
    }
    fn external(items: &[(&str, &str)]) -> Result<DialectRegistry, DialectError> {
        let docs = items
            .iter()
            .map(|(name, json)| {
                parse_profile_source(name, ProfileSourceKind::External, json).unwrap()
            })
            .collect::<Vec<_>>();
        DialectRegistry::from_profiles(&resolve_profiles(docs).unwrap())
    }
    #[test]
    fn descriptors_have_open_features_unknown_and_provenance() {
        let r = bundled_dialect_registry().unwrap();
        assert_eq!(r.descriptors.len(), 3);
        assert!(
            bundled_profiles()
                .unwrap()
                .profiles()
                .values()
                .all(|p| p.platform_build.is_none())
        );
        let a = r.get(&XmlDialect::parse("2.20").unwrap()).unwrap();
        let b = r.get(&XmlDialect::parse("2.21").unwrap()).unwrap();
        let palette = DialectFeature::palette_namespace();
        assert_eq!(a.feature(&palette), FeatureAvailability::Unknown);
        assert_eq!(a.feature_provenance(&palette), None);
        assert_eq!(b.feature(&palette), FeatureAvailability::Supported);
        assert_eq!(b.feature_provenance(&palette).unwrap().as_str(), "xml-2.21");
        assert_eq!(a.lexical_rules().bom().value(), &BomRule::Preserve);
        assert_eq!(
            a.lexical_rules().bom().provenance(),
            &RuleProvenance::CommonBaseline
        );
        assert_eq!(a.root_child_order().value().as_str(), "source");
    }
    #[test]
    fn exact_versions_include_217() {
        for version in ["2.17", "2.20", "2.21"] {
            let xml = format!(
                "<MetaDataObject xmlns='http://v8.1c.ru/8.3/MDClasses' version='{version}'/>"
            );
            assert!(
                matches!(detect(&xml),DialectDetection::Exact{candidate,..}if candidate.dialect().to_string()==version)
            );
        }
    }
    #[test]
    fn repository_xcf_roots_are_recognized_but_dumpinfo_is_not_conflated() {
        for &(local, namespace) in BASELINE_ROOTS {
            let xml = format!("<{local} xmlns='{namespace}' version='2.20'/>");
            assert!(
                matches!(detect(&xml),DialectDetection::Exact{candidate,..}if candidate.dialect().to_string()=="2.20"),
                "{local}|{namespace}"
            );
        }
        assert!(matches!(
            detect("<ConfigDumpInfo xmlns='http://v8.1c.ru/8.3/xcf/dumpinfo' version='2.0'/>"),
            DialectDetection::Unknown { .. }
        ));
    }
    #[test]
    fn common_root_without_version_is_sorted_ambiguity() {
        let DialectDetection::Ambiguous { candidates, .. } =
            detect("<MetaDataObject xmlns='http://v8.1c.ru/8.3/MDClasses'/>")
        else {
            panic!()
        };
        assert_eq!(
            candidates
                .iter()
                .map(|x| x.dialect().to_string())
                .collect::<Vec<_>>(),
            ["2.17", "2.20", "2.21"]
        );
    }
    #[test]
    fn conflicting_version_and_dynamic_221_evidence_is_ambiguous() {
        for body in ["<MetaDataObject xmlns='http://v8.1c.ru/8.3/MDClasses' xmlns:p='http://v8.1c.ru/8.1/data/ui/colors/palette' version='2.20'/>".to_owned(),"<MetaDataObject xmlns='http://v8.1c.ru/8.3/MDClasses' version='2.20'><UseInInterfaceCompatibilityMode/></MetaDataObject>".into()]{let DialectDetection::Ambiguous{candidates,..}=detect(&body)else{panic!()};assert_eq!(candidates.iter().map(|x|x.dialect().to_string()).collect::<Vec<_>>(),["2.20","2.21"]);}
    }
    #[test]
    fn unknown_version_and_unrecognized_root_are_unknown() {
        assert!(matches!(
            detect("<MetaDataObject version='2.99'/>"),
            DialectDetection::Unknown { .. }
        ));
        assert!(matches!(
            detect("<junk xmlns='urn:unrelated'><UseInInterfaceCompatibilityMode/></junk>"),
            DialectDetection::Unknown { .. }
        ));
        assert!(matches!(
            detect("<junk xmlns:p='http://v8.1c.ru/8.1/data/ui/colors/palette'/>"),
            DialectDetection::Unknown { .. }
        ));
        assert!(matches!(
            detect("<junk version='2.21'/>"),
            DialectDetection::Unknown { .. }
        ));
        assert!(matches!(
            detect(
                "<MetaDataObject xmlns='http://v8.1c.ru/8.3/MDClasses' xmlns:q='urn:other'><q:UseInInterfaceCompatibilityMode/></MetaDataObject>"
            ),
            DialectDetection::Ambiguous { .. }
        ));
    }
    #[test]
    fn namespace_scopes_prefixes_and_duplicates_are_strict() {
        let x = detect(
            "<m:MetaDataObject xmlns:m='http://v8.1c.ru/8.3/MDClasses' xmlns:q='urn:future' version='2.20' q:version='2.21'/>",
        );
        assert!(
            matches!(x,DialectDetection::Exact{ref candidate,..}if candidate.dialect().to_string()=="2.20")
        );
        assert!(
            x.evidence()
                .namespaces()
                .iter()
                .any(|n| n.uri() == "urn:future")
        );
        assert!(XmlReader::from_slice(b"<x version='2.20' version='2.21'/>").is_err());
        let duplicate_version = XmlDocument::new(XmlElement::with_parts(
            QName::new("MetaDataObject").unwrap(),
            vec![
                Attribute::namespace(None, "http://v8.1c.ru/8.3/MDClasses"),
                Attribute::ordinary(QName::new("version").unwrap(), "2.20"),
                Attribute::ordinary(QName::new("version").unwrap(), "2.21"),
            ],
            vec![],
        ));
        assert!(matches!(
            bundled_dialect_registry()
                .unwrap()
                .detect(&duplicate_version),
            Err(DialectError::DuplicateVersion)
        ));
        let malformed = XmlReader::from_slice(
            b"<MetaDataObject xmlns='http://v8.1c.ru/8.3/MDClasses' version='future'/>",
        )
        .unwrap();
        assert!(matches!(
            bundled_dialect_registry().unwrap().detect(&malformed),
            Err(DialectError::InvalidVersion(_))
        ));
        let duplicate = XmlDocument::new(XmlElement::with_parts(
            QName::new("x").unwrap(),
            vec![
                Attribute::namespace(Some("p".into()), "a"),
                Attribute::namespace(Some("p".into()), "b"),
            ],
            vec![],
        ));
        assert!(matches!(
            bundled_dialect_registry().unwrap().detect(&duplicate),
            Err(DialectError::DuplicateNamespace { .. })
        ));
        let child = XmlElement::with_parts(
            QName::new("child").unwrap(),
            vec![
                Attribute::namespace(None, "a"),
                Attribute::namespace(None, "b"),
            ],
            vec![],
        );
        let nested = XmlDocument::new(XmlElement::with_parts(
            QName::new("x").unwrap(),
            vec![],
            vec![XmlNode::Element(child)],
        ));
        assert!(matches!(
            bundled_dialect_registry().unwrap().detect(&nested),
            Err(DialectError::DuplicateNamespace { .. })
        ));
        let invalid = XmlDocument::new(XmlElement::with_parts(
            QName::new("x").unwrap(),
            vec![Attribute::namespace(Some("bad:name".into()), "urn:x")],
            vec![],
        ));
        assert!(matches!(
            bundled_dialect_registry().unwrap().detect(&invalid),
            Err(DialectError::InvalidNamespace { .. })
        ));
    }
    #[test]
    fn namespace_reserved_bindings_and_unbound_prefixes_fail_closed() {
        assert!(matches!(
            detect(
                "<MetaDataObject xmlns='http://v8.1c.ru/8.3/MDClasses' xml:lang='en' version='2.20'/>"
            ),
            DialectDetection::Exact { .. }
        ));
        for attribute in [
            Attribute::namespace(Some("xml".into()), "urn:wrong"),
            Attribute::namespace(Some("p".into()), XML_NAMESPACE),
            Attribute::namespace(None, XML_NAMESPACE),
            Attribute::namespace(Some("xmlns".into()), "urn:x"),
            Attribute::namespace(Some("p".into()), XMLNS_NAMESPACE),
        ] {
            let doc = XmlDocument::new(XmlElement::with_parts(
                QName::new("x").unwrap(),
                vec![attribute],
                vec![],
            ));
            assert!(matches!(
                bundled_dialect_registry().unwrap().detect(&doc),
                Err(DialectError::InvalidNamespace { .. })
            ));
        }
        let root = XmlDocument::new(XmlElement::with_parts(
            QName::new("p:x").unwrap(),
            vec![],
            vec![],
        ));
        assert!(matches!(
            bundled_dialect_registry().unwrap().detect(&root),
            Err(DialectError::UnboundPrefix { .. })
        ));
        let child = XmlElement::with_parts(QName::new("p:child").unwrap(), vec![], vec![]);
        let doc = XmlDocument::new(XmlElement::with_parts(
            QName::new("MetaDataObject").unwrap(),
            vec![Attribute::namespace(None, MD_CLASSES)],
            vec![XmlNode::Element(child)],
        ));
        assert!(matches!(
            bundled_dialect_registry().unwrap().detect(&doc),
            Err(DialectError::UnboundPrefix { .. })
        ));
        let doc = XmlDocument::new(XmlElement::with_parts(
            QName::new("MetaDataObject").unwrap(),
            vec![
                Attribute::namespace(None, MD_CLASSES),
                Attribute::ordinary(QName::new("p:a").unwrap(), "x"),
            ],
            vec![],
        ));
        assert!(matches!(
            bundled_dialect_registry().unwrap().detect(&doc),
            Err(DialectError::UnboundPrefix { .. })
        ));
        let control_uri = XmlDocument::new(XmlElement::with_parts(
            QName::new("x").unwrap(),
            vec![Attribute::namespace(Some("p".into()), "urn:\u{1}")],
            vec![],
        ));
        assert!(matches!(
            bundled_dialect_registry().unwrap().detect(&control_uri),
            Err(DialectError::InvalidNamespace { .. })
        ));
    }
    #[test]
    fn oversized_namespace_names_fail_before_owned_errors() {
        let oversized_prefix = "p".repeat(MAX_FIELD_BYTES + 1);
        let declaration = XmlDocument::new(XmlElement::with_parts(
            QName::new("x").unwrap(),
            vec![Attribute::namespace(
                Some(oversized_prefix.clone()),
                "urn:x",
            )],
            vec![],
        ));
        assert!(matches!(
            bundled_dialect_registry().unwrap().detect(&declaration),
            Err(DialectError::EvidenceLimit)
        ));

        let unbound_element = XmlDocument::new(XmlElement::with_parts(
            QName::new(format!("{oversized_prefix}:x")).unwrap(),
            vec![],
            vec![],
        ));
        assert!(matches!(
            bundled_dialect_registry().unwrap().detect(&unbound_element),
            Err(DialectError::EvidenceLimit)
        ));

        let unbound_attribute = XmlDocument::new(XmlElement::with_parts(
            QName::new("x").unwrap(),
            vec![Attribute::ordinary(
                QName::new(format!("{oversized_prefix}:a")).unwrap(),
                "value",
            )],
            vec![],
        ));
        assert!(matches!(
            bundled_dialect_registry()
                .unwrap()
                .detect(&unbound_attribute),
            Err(DialectError::EvidenceLimit)
        ));
    }
    #[test]
    fn duplicate_version_is_rejected_without_retaining_later_values() {
        let mut attributes = vec![
            Attribute::namespace(None, MD_CLASSES),
            Attribute::ordinary(QName::new("version").unwrap(), "2.20"),
            Attribute::ordinary(
                QName::new("version").unwrap(),
                "x".repeat(MAX_FIELD_BYTES + 1),
            ),
        ];
        attributes.extend(
            (0..MAX_EVIDENCE * 4)
                .map(|_| Attribute::ordinary(QName::new("version").unwrap(), "2.21")),
        );
        attributes.push(Attribute::ordinary(
            QName::new(format!("{}:a", "p".repeat(MAX_FIELD_BYTES + 1))).unwrap(),
            "value",
        ));
        let duplicates = XmlDocument::new(XmlElement::with_parts(
            QName::new("MetaDataObject").unwrap(),
            attributes,
            vec![],
        ));
        assert!(matches!(
            bundled_dialect_registry().unwrap().detect(&duplicates),
            Err(DialectError::DuplicateVersion)
        ));

        let oversized_first = XmlDocument::new(XmlElement::with_parts(
            QName::new("MetaDataObject").unwrap(),
            vec![Attribute::ordinary(
                QName::new("version").unwrap(),
                "x".repeat(MAX_FIELD_BYTES + 1),
            )],
            vec![],
        ));
        assert!(matches!(
            bundled_dialect_registry().unwrap().detect(&oversized_first),
            Err(DialectError::EvidenceLimit)
        ));
    }
    #[test]
    fn future_profile_declares_dynamic_element_and_namespace_features() {
        let future = r#"{"schema_version":1,"id":"future","status":"experimental","xml_dialect":"3.7","fingerprints":{"xcf.version":"3.7","xcf.namespace.future_palette":"urn:future:palette","xcf.element.future_toggle":"FutureToggle|urn:future:elements"},"constants":{"xcf.feature.future_palette":"supported","xcf.feature.future_toggle":"supported"}}"#;
        let r = external(&[("future", future)]).unwrap();
        let d = r.get(&XmlDialect::parse("3.7").unwrap()).unwrap();
        assert_eq!(
            d.feature(&DialectFeature::parse("future_toggle").unwrap()),
            FeatureAvailability::Supported
        );
        let xml=XmlReader::from_slice(b"<MetaDataObject xmlns='http://v8.1c.ru/8.3/MDClasses' xmlns:p='urn:future:palette'><f:FutureToggle xmlns:f='urn:future:elements'/></MetaDataObject>").unwrap();
        let DialectDetection::Exact {
            candidate,
            evidence,
        } = r.detect(&xml).unwrap()
        else {
            panic!()
        };
        assert_eq!(candidate.dialect().to_string(), "3.7");
        assert_eq!(
            evidence
                .features()
                .iter()
                .map(DialectFeature::as_str)
                .collect::<Vec<_>>(),
            ["future_palette", "future_toggle"]
        );
    }
    #[test]
    fn invalid_external_profiles_fail_closed() {
        let mixed = r#"{"schema_version":1,"id":"mixed","status":"experimental","xml_dialect":"3.0","platform_build":"9.0","fingerprints":{"xcf.version":"3.0"}}"#;
        assert!(matches!(
            external(&[("mixed", mixed)]),
            Err(DialectError::MixedAxes { .. })
        ));
        let mismatch = r#"{"schema_version":1,"id":"bad","status":"experimental","xml_dialect":"3.0","fingerprints":{"xcf.version":"3.1"}}"#;
        assert!(matches!(
            external(&[("bad", mismatch)]),
            Err(DialectError::VersionMismatch { .. })
        ));
        let lexical = r#"{"schema_version":1,"id":"bad","status":"experimental","xml_dialect":"3.0","fingerprints":{"xcf.version":"3.0"},"constants":{"xcf.lexical.policy":"banana"}}"#;
        assert!(matches!(
            external(&[("bad", lexical)]),
            Err(DialectError::InvalidField { .. })
        ));
        let matcher = r#"{"schema_version":1,"id":"bad","status":"experimental","xml_dialect":"3.0","fingerprints":{"xcf.version":"3.0","xcf.element.toggle":"Toggle|urn:x"}}"#;
        assert!(matches!(
            external(&[("bad", matcher)]),
            Err(DialectError::MatcherWithoutFeatureDeclaration { .. })
        ));
        let unknown = r#"{"schema_version":1,"id":"bad","status":"experimental","xml_dialect":"3.0","fingerprints":{"xcf.version":"3.0","xcf.typo.x":"y"}}"#;
        assert!(matches!(
            external(&[("bad", unknown)]),
            Err(DialectError::UnknownDescriptorKey { .. })
        ));
        let duplicate = r#"{"schema_version":1,"id":"bad","status":"experimental","xml_dialect":"3.0","fingerprints":{"xcf.version":"3.0","xcf.namespace.a":"urn:same","xcf.namespace.b":"urn:same"},"constants":{"xcf.feature.a":"supported","xcf.feature.b":"supported"}}"#;
        assert!(matches!(
            external(&[("bad", duplicate)]),
            Err(DialectError::DuplicateMatcher { .. })
        ));
    }
    #[test]
    fn duplicate_dialects_and_shuffled_input_are_deterministic() {
        let a = r#"{"schema_version":1,"id":"a","status":"experimental","xml_dialect":"4.0","fingerprints":{"xcf.version":"4.0"}}"#;
        let b = r#"{"schema_version":1,"id":"b","status":"experimental","xml_dialect":"4.0","fingerprints":{"xcf.version":"4.0"}}"#;
        let one = external(&[("b", b), ("a", a)]).unwrap_err().to_string();
        let two = external(&[("a", a), ("b", b)]).unwrap_err().to_string();
        assert_eq!(one, two);
        let c = r#"{"schema_version":1,"id":"c","status":"experimental","xml_dialect":"4.1","fingerprints":{"xcf.version":"4.1"}}"#;
        let d = r#"{"schema_version":1,"id":"d","status":"experimental","xml_dialect":"4.2","fingerprints":{"xcf.version":"4.2"}}"#;
        assert_eq!(
            external(&[("d", d), ("c", c)])
                .unwrap()
                .descriptors()
                .keys()
                .collect::<Vec<_>>(),
            external(&[("c", c), ("d", d)])
                .unwrap()
                .descriptors()
                .keys()
                .collect::<Vec<_>>()
        );
    }
    #[test]
    fn compatible_evidence_intersects_and_conflicts_union() {
        let a = r#"{"schema_version":1,"id":"a","status":"experimental","xml_dialect":"5.0","fingerprints":{"xcf.version":"5.0","xcf.namespace.shared":"urn:shared","xcf.namespace.alpha":"urn:alpha"},"constants":{"xcf.feature.shared":"supported","xcf.feature.alpha":"supported"}}"#;
        let b = r#"{"schema_version":1,"id":"b","status":"experimental","xml_dialect":"5.1","fingerprints":{"xcf.version":"5.1","xcf.namespace.shared":"urn:shared","xcf.namespace.alpha":"urn:alpha","xcf.element.beta":"Beta|http://v8.1c.ru/8.3/MDClasses"},"constants":{"xcf.feature.shared":"supported","xcf.feature.alpha":"supported","xcf.feature.beta":"supported"}}"#;
        let c = r#"{"schema_version":1,"id":"c","status":"experimental","xml_dialect":"5.2","fingerprints":{"xcf.version":"5.2","xcf.element.beta":"Beta|http://v8.1c.ru/8.3/MDClasses"},"constants":{"xcf.feature.beta":"supported"}}"#;
        let r = external(&[("c", c), ("a", a), ("b", b)]).unwrap();
        let one=XmlReader::from_slice(b"<MetaDataObject xmlns='http://v8.1c.ru/8.3/MDClasses' xmlns:s='urn:shared' version='5.0'/>").unwrap();
        assert!(
            matches!(r.detect(&one).unwrap(),DialectDetection::Exact{candidate,..}if candidate.dialect().to_string()=="5.0")
        );
        let two=XmlReader::from_slice(b"<MetaDataObject xmlns='http://v8.1c.ru/8.3/MDClasses' xmlns:a='urn:alpha'><Beta/></MetaDataObject>").unwrap();
        assert!(
            matches!(r.detect(&two).unwrap(),DialectDetection::Exact{candidate,..}if candidate.dialect().to_string()=="5.1")
        );
        let d = r#"{"schema_version":1,"id":"d","status":"experimental","xml_dialect":"6.0","fingerprints":{"xcf.version":"6.0","xcf.root":"Future|urn:future"}}"#;
        let e = r#"{"schema_version":1,"id":"e","status":"experimental","xml_dialect":"6.1","fingerprints":{"xcf.version":"6.1"}}"#;
        let r = external(&[("d", d), ("e", e)]).unwrap();
        let xml = XmlReader::from_slice(b"<Future xmlns='urn:future' version='6.1'/>").unwrap();
        let DialectDetection::Ambiguous { candidates, .. } = r.detect(&xml).unwrap() else {
            panic!()
        };
        assert_eq!(
            candidates
                .iter()
                .map(|x| x.dialect().to_string())
                .collect::<Vec<_>>(),
            ["6.0", "6.1"]
        );
    }
    #[test]
    fn inherited_matcher_can_be_explicitly_disabled() {
        let parent = r#"{"schema_version":1,"id":"parent","status":"experimental","xml_dialect":"7.0","fingerprints":{"xcf.version":"7.0","xcf.namespace.toggle":"urn:toggle"},"constants":{"xcf.feature.toggle":"supported"}}"#;
        let child = r#"{"schema_version":1,"id":"child","extends":"parent","status":"experimental","xml_dialect":"7.1","fingerprints":{"xcf.version":"7.1"},"constants":{"xcf.feature.toggle":"unsupported"}}"#;
        let r = external(&[("child", child), ("parent", parent)]).unwrap();
        let feature = DialectFeature::parse("toggle").unwrap();
        assert_eq!(
            r.get(&XmlDialect::parse("7.1").unwrap())
                .unwrap()
                .feature(&feature),
            FeatureAvailability::Unsupported
        );
        let xml = XmlReader::from_slice(
            b"<MetaDataObject xmlns='http://v8.1c.ru/8.3/MDClasses' xmlns:t='urn:toggle'/>",
        )
        .unwrap();
        assert!(
            matches!(r.detect(&xml).unwrap(),DialectDetection::Exact{candidate,..}if candidate.dialect().to_string()=="7.0")
        );
    }
    #[test]
    fn evidence_with_no_supporting_descriptor_is_unknown() {
        let standalone = r#"{"schema_version":1,"id":"standalone","status":"experimental","xml_dialect":"8.0","fingerprints":{"xcf.version":"8.0","xcf.namespace.toggle":"urn:toggle"},"constants":{"xcf.feature.toggle":"unsupported"}}"#;
        let r = external(&[("standalone", standalone)]).unwrap();
        let xml = XmlReader::from_slice(
            b"<MetaDataObject xmlns='http://v8.1c.ru/8.3/MDClasses' xmlns:t='urn:toggle'/>",
        )
        .unwrap();
        assert!(matches!(
            r.detect(&xml).unwrap(),
            DialectDetection::Unknown { .. }
        ));
        let parent = r#"{"schema_version":1,"id":"vocabulary","status":"experimental","fingerprints":{"xcf.namespace.toggle":"urn:toggle"},"constants":{"xcf.feature.toggle":"supported"}}"#;
        let child = r#"{"schema_version":1,"id":"child","extends":"vocabulary","status":"experimental","xml_dialect":"8.1","fingerprints":{"xcf.version":"8.1"},"constants":{"xcf.feature.toggle":"unsupported"}}"#;
        let r = external(&[("child", child), ("parent", parent)]).unwrap();
        assert!(matches!(
            r.detect(&xml).unwrap(),
            DialectDetection::Unknown { .. }
        ));
    }
    #[test]
    fn evidence_and_open_ids_are_bounded() {
        assert!(DialectFeature::parse(&"x".repeat(MAX_OPEN_ID_BYTES + 1)).is_err());
        let long = format!("<{} />", "x".repeat(MAX_FIELD_BYTES + 1));
        let doc = XmlReader::from_slice(long.as_bytes()).unwrap();
        assert!(matches!(
            bundled_dialect_registry().unwrap().detect(&doc),
            Err(DialectError::EvidenceLimit)
        ));
        let doc = XmlReader::from_slice(b"<x><a/><a/></x>").unwrap();
        assert!(matches!(
            extract(&doc, &bundled_dialect_registry().unwrap(), 2),
            Err(DialectError::EvidenceLimit)
        ));
        let mut evidence = DialectEvidence {
            root_local: "x".into(),
            root_namespace: None,
            version: None,
            namespaces: vec![],
            features: BTreeSet::new(),
        };
        record_feature_with_limit(&mut evidence, &DialectFeature::parse("a").unwrap(), 1).unwrap();
        assert!(matches!(
            record_feature_with_limit(&mut evidence, &DialectFeature::parse("b").unwrap(), 1),
            Err(DialectError::EvidenceLimit)
        ));
        assert_eq!(evidence.features.len(), 1);
        assert!(
            !evidence
                .features
                .contains(&DialectFeature::parse("b").unwrap())
        );
        let feature_constants = (0..=MAX_FEATURES)
            .map(|i| format!("\"xcf.feature.f{i}\":\"supported\""))
            .collect::<Vec<_>>()
            .join(",");
        let profile = format!(
            r#"{{"schema_version":1,"id":"many","status":"experimental","xml_dialect":"9.0","fingerprints":{{"xcf.version":"9.0"}},"constants":{{{feature_constants}}}}}"#
        );
        assert!(matches!(
            external(&[("many", profile.as_str())]),
            Err(DialectError::TooManyFeatures { .. })
        ));
        let extra = MAX_ROOTS - BASELINE_ROOTS.len() + 1;
        let root_values = (0..extra)
            .map(|i| format!("R{i}|urn:r:{i}"))
            .collect::<Vec<_>>()
            .join(";");
        let profile = format!(
            r#"{{"schema_version":1,"id":"roots","status":"experimental","xml_dialect":"9.1","fingerprints":{{"xcf.version":"9.1","xcf.root":"{root_values}"}}}}"#
        );
        assert!(matches!(
            external(&[("roots", profile.as_str())]),
            Err(DialectError::TooManyRoots { .. })
        ));
        let matcher_features = MAX_MATCHERS / 2 + 1;
        let matcher_fingerprints = (0..matcher_features)
            .flat_map(|i| {
                [
                    format!("\"xcf.namespace.m{i}\":\"urn:m:{i}\""),
                    format!("\"xcf.element.m{i}\":\"E{i}|urn:e:{i}\""),
                ]
            })
            .collect::<Vec<_>>()
            .join(",");
        let matcher_constants = (0..matcher_features)
            .map(|i| format!("\"xcf.feature.m{i}\":\"supported\""))
            .collect::<Vec<_>>()
            .join(",");
        let profile = format!(
            r#"{{"schema_version":1,"id":"matchers","status":"experimental","xml_dialect":"9.2","fingerprints":{{"xcf.version":"9.2",{matcher_fingerprints}}},"constants":{{{matcher_constants}}}}}"#
        );
        assert!(matches!(
            external(&[("matchers", profile.as_str())]),
            Err(DialectError::TooManyMatchers { .. })
        ));
        let attributes = (0..=MAX_EVIDENCE)
            .map(|i| Attribute::namespace(Some(format!("p{i}")), format!("urn:{i}")))
            .collect();
        let doc = XmlDocument::new(XmlElement::with_parts(
            QName::new("x").unwrap(),
            attributes,
            vec![],
        ));
        assert!(matches!(
            bundled_dialect_registry().unwrap().detect(&doc),
            Err(DialectError::EvidenceLimit)
        ));
    }
}
