//! Standalone, versioned schema knowledge derived from public XML behaviour and
//! locally inspected EDT model/export metadata.
//!
//! This crate embeds declarative data only. It neither links to nor starts EDT,
//! Java, OSGi, platform executables, or native libraries.

#![forbid(unsafe_code)]

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use serde::{Deserialize, Serialize};

/// Embedded EDT-derived model inventory.
pub const BUNDLED_MODEL_INVENTORY_JSON: &str =
    include_str!("../data/edt-2025.2.3-model-inventory.json");

/// Embedded EDT EPackage classifier and feature identifiers.
pub const BUNDLED_PACKAGE_FEATURES_JSON: &str =
    include_str!("../data/edt-2025.2.3-package-features.json");

/// Embedded EDT Xcore-derived feature semantics for the first form vertical slice.
pub const BUNDLED_FEATURE_SEMANTICS_JSON: &str =
    include_str!("../data/edt-2025.2.3-feature-semantics.json");

/// Embedded, verified writer behaviour rules.
pub const BUNDLED_WRITER_RULES_JSON: &str = include_str!("../data/edt-2025.2.3-writer-rules.json");

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CorpusSource {
    pub product: String,
    pub release: String,
    pub derivation: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InventorySummary {
    pub bundles: usize,
    pub model_types: usize,
    pub importers: usize,
    pub exporters: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleInventory {
    pub symbolic_name: String,
    pub version: Option<String>,
    pub model_types: Vec<String>,
    pub importers: Vec<String>,
    pub exporters: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInventory {
    pub schema_version: u32,
    pub source: CorpusSource,
    pub summary: InventorySummary,
    pub bundles: Vec<BundleInventory>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageFeatureSummary {
    pub packages: usize,
    pub classifiers: usize,
    pub features: usize,
    pub operations: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageFeatureCorpus {
    pub schema_version: u32,
    pub source: CorpusSource,
    pub summary: PackageFeatureSummary,
    pub packages: Vec<ModelPackage>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelPackage {
    pub bundle: String,
    pub package_class: String,
    pub name: Option<String>,
    pub namespace_uri: Option<String>,
    pub namespace_prefix: Option<String>,
    pub classifiers: Vec<ModelClassifier>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelClassifier {
    pub token: String,
    pub id: i32,
    pub feature_count: Option<i32>,
    pub operation_count: Option<i32>,
    pub features: Vec<ModelMember>,
    pub operations: Vec<ModelMember>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelMember {
    pub token: String,
    pub id: i32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriterRuleCorpus {
    pub schema_version: u32,
    pub source: CorpusSource,
    pub rules: Vec<WriterRule>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriterRule {
    pub id: String,
    pub source_class: String,
    pub model_type: String,
    pub feature: String,
    pub operations: Vec<String>,
    pub conditions: Vec<String>,
    pub delegate: Option<String>,
    pub evidence: RuleEvidence,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleEvidence {
    pub kind: String,
    pub status: String,
    pub note: String,
}

/// A stable semantic identity for an Xcore feature.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureSemanticKey {
    pub namespace_uri: String,
    pub classifier: String,
    pub feature: String,
}

/// Whether a corpus statement has been confirmed by evidence or is still incomplete.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EvidenceStatus {
    Pending,
    Verified,
}

/// Provenance and confirmation state for one group of feature facts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureEvidence {
    pub status: EvidenceStatus,
    pub kind: String,
    #[serde(default)]
    pub sources: Vec<String>,
    pub note: Option<String>,
}

/// The Xcore declaration kind of a feature.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FeatureKind {
    Attribute,
    Reference,
    Containment,
}

/// Xcore feature modifiers preserved by the semantics corpus.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum XcoreFeatureQualifier {
    Container,
    Transient,
    Unsettable,
    Unique,
}

/// The kind of an Xcore classifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FeatureClassifierKind {
    Class,
    Interface,
    Enum,
    Datatype,
}

/// A value whose availability has independent evidence.
///
/// `Known { value: None }` records a verified absence, whereas `Pending` records that the
/// importer has not yet established whether the value exists.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum EvidenceValue<T> {
    Pending,
    Known { value: Option<T> },
}

/// XML writer behaviour associated with a feature.
///
/// Any field may be unknown while its evidence is pending. All fields are required once the
/// behaviour is verified.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XmlFeatureBehavior {
    #[serde(rename = "qname")]
    pub qname: Option<String>,
    pub order: Option<u32>,
    pub emit_default: Option<bool>,
    pub version_gate: EvidenceValue<String>,
    pub delegate: EvidenceValue<String>,
    pub evidence: FeatureEvidence,
}

/// Semantics for one Xcore feature.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureSemantics {
    pub name: String,
    pub kind: FeatureKind,
    pub model_type: String,
    pub lower_bound: u32,
    /// `None` means an unbounded upper bound.
    pub upper_bound: Option<u32>,
    /// The value explicitly declared by the model, rather than an inferred language default.
    pub default_value: Option<String>,
    pub qualifiers: Vec<XcoreFeatureQualifier>,
    pub model_evidence: FeatureEvidence,
    pub xml: XmlFeatureBehavior,
}

/// A classifier and its Xcore feature declarations.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureSemanticsClassifier {
    pub name: String,
    pub kind: FeatureClassifierKind,
    pub features: Vec<FeatureSemantics>,
}

/// One Xcore resource and the package it declares.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureSemanticsPackage {
    pub bundle: String,
    pub resource: String,
    pub package_name: String,
    pub namespace_uri: String,
    pub classifiers: Vec<FeatureSemanticsClassifier>,
}

/// Counts that allow a corpus to detect truncation without reading EDT.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureSemanticsSummary {
    pub packages: usize,
    pub classifiers: usize,
    pub features: usize,
}

/// A standalone, versioned feature-semantics corpus.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureSemanticsCorpus {
    pub schema_version: u32,
    pub source: CorpusSource,
    pub summary: FeatureSemanticsSummary,
    pub packages: Vec<FeatureSemanticsPackage>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SchemaError {
    InvalidJson(String),
    UnsupportedSchemaVersion(u32),
    EmptyField(&'static str),
    DuplicateValue {
        field: &'static str,
        value: String,
    },
    SummaryMismatch {
        field: &'static str,
        expected: usize,
        actual: usize,
    },
    NonPortablePath(String),
    InvalidCardinality {
        lower: u32,
        upper: u32,
    },
    IncompleteVerifiedXmlBehavior {
        key: FeatureSemanticKey,
        field: &'static str,
    },
}

impl Display for SchemaError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson(message) => write!(formatter, "invalid schema JSON: {message}"),
            Self::UnsupportedSchemaVersion(version) => {
                write!(formatter, "unsupported schema version {version}")
            }
            Self::EmptyField(field) => write!(formatter, "{field} is empty"),
            Self::DuplicateValue { field, value } => {
                write!(formatter, "duplicate {field} `{value}`")
            }
            Self::SummaryMismatch {
                field,
                expected,
                actual,
            } => write!(
                formatter,
                "{field} summary mismatch: expected {expected}, actual {actual}"
            ),
            Self::NonPortablePath(value) => {
                write!(formatter, "corpus contains a non-portable path `{value}`")
            }
            Self::InvalidCardinality { lower, upper } => {
                write!(formatter, "invalid cardinality {lower}..{upper}")
            }
            Self::IncompleteVerifiedXmlBehavior { key, field } => write!(
                formatter,
                "verified XML behaviour for {} / {} / {} is missing {field}",
                key.namespace_uri, key.classifier, key.feature
            ),
        }
    }
}

impl Error for SchemaError {}

impl ModelInventory {
    pub fn parse(json: &str) -> Result<Self, SchemaError> {
        let inventory: Self = serde_json::from_str(json)
            .map_err(|error| SchemaError::InvalidJson(error.to_string()))?;
        inventory.validate()?;
        Ok(inventory)
    }

    pub fn validate(&self) -> Result<(), SchemaError> {
        validate_source(self.schema_version, &self.source)?;
        let mut bundle_names = BTreeSet::new();
        let mut model_types = 0usize;
        let mut importers = 0usize;
        let mut exporters = 0usize;
        for bundle in &self.bundles {
            validate_text("bundle symbolic name", &bundle.symbolic_name)?;
            if !bundle_names.insert(bundle.symbolic_name.as_str()) {
                return Err(SchemaError::DuplicateValue {
                    field: "bundle symbolic name",
                    value: bundle.symbolic_name.clone(),
                });
            }
            validate_unique_names("model type", &bundle.model_types)?;
            validate_unique_names("importer", &bundle.importers)?;
            validate_unique_names("exporter", &bundle.exporters)?;
            model_types += bundle.model_types.len();
            importers += bundle.importers.len();
            exporters += bundle.exporters.len();
        }
        validate_count("bundles", self.summary.bundles, self.bundles.len())?;
        validate_count("modelTypes", self.summary.model_types, model_types)?;
        validate_count("importers", self.summary.importers, importers)?;
        validate_count("exporters", self.summary.exporters, exporters)
    }

    pub fn bundle(&self, symbolic_name: &str) -> Option<&BundleInventory> {
        self.bundles
            .iter()
            .find(|bundle| bundle.symbolic_name == symbolic_name)
    }
}

impl PackageFeatureCorpus {
    pub fn parse(json: &str) -> Result<Self, SchemaError> {
        let corpus: Self = serde_json::from_str(json)
            .map_err(|error| SchemaError::InvalidJson(error.to_string()))?;
        corpus.validate()?;
        Ok(corpus)
    }

    pub fn validate(&self) -> Result<(), SchemaError> {
        validate_source(self.schema_version, &self.source)?;
        let mut package_names = BTreeSet::new();
        let mut classifier_count = 0usize;
        let mut feature_count = 0usize;
        let mut operation_count = 0usize;
        for package in &self.packages {
            validate_text("model package bundle", &package.bundle)?;
            validate_text("model package class", &package.package_class)?;
            if !package_names.insert(package.package_class.as_str()) {
                return Err(SchemaError::DuplicateValue {
                    field: "model package class",
                    value: package.package_class.clone(),
                });
            }
            let mut classifier_tokens = BTreeSet::new();
            for classifier in &package.classifiers {
                validate_text("model classifier token", &classifier.token)?;
                if !classifier_tokens.insert(classifier.token.as_str()) {
                    return Err(SchemaError::DuplicateValue {
                        field: "model classifier token",
                        value: classifier.token.clone(),
                    });
                }
                validate_members("model feature", &classifier.features)?;
                validate_members("model operation", &classifier.operations)?;
                classifier_count += 1;
                feature_count += classifier.features.len();
                operation_count += classifier.operations.len();
            }
        }
        validate_count("packages", self.summary.packages, self.packages.len())?;
        validate_count("classifiers", self.summary.classifiers, classifier_count)?;
        validate_count("features", self.summary.features, feature_count)?;
        validate_count("operations", self.summary.operations, operation_count)
    }

    pub fn package(&self, package_class: &str) -> Option<&ModelPackage> {
        self.packages
            .iter()
            .find(|package| package.package_class == package_class)
    }
}

impl WriterRuleCorpus {
    pub fn parse(json: &str) -> Result<Self, SchemaError> {
        let corpus: Self = serde_json::from_str(json)
            .map_err(|error| SchemaError::InvalidJson(error.to_string()))?;
        corpus.validate()?;
        Ok(corpus)
    }

    pub fn validate(&self) -> Result<(), SchemaError> {
        validate_source(self.schema_version, &self.source)?;
        let mut identifiers = BTreeSet::new();
        for rule in &self.rules {
            for (field, value) in [
                ("writer rule id", rule.id.as_str()),
                ("source class", rule.source_class.as_str()),
                ("model type", rule.model_type.as_str()),
                ("feature", rule.feature.as_str()),
                ("evidence kind", rule.evidence.kind.as_str()),
                ("evidence status", rule.evidence.status.as_str()),
            ] {
                validate_text(field, value)?;
            }
            if !identifiers.insert(rule.id.as_str()) {
                return Err(SchemaError::DuplicateValue {
                    field: "writer rule id",
                    value: rule.id.clone(),
                });
            }
            if rule.operations.is_empty() {
                return Err(SchemaError::EmptyField("writer rule operations"));
            }
            validate_unique_names("writer operation", &rule.operations)?;
        }
        Ok(())
    }

    pub fn rule(&self, id: &str) -> Option<&WriterRule> {
        self.rules.iter().find(|rule| rule.id == id)
    }

    pub fn rules_for_class<'a>(
        &'a self,
        source_class: &'a str,
    ) -> impl Iterator<Item = &'a WriterRule> + 'a {
        self.rules
            .iter()
            .filter(move |rule| rule.source_class == source_class)
    }
}

impl FeatureSemanticsCorpus {
    pub fn parse(json: &str) -> Result<Self, SchemaError> {
        let corpus: Self = serde_json::from_str(json)
            .map_err(|error| SchemaError::InvalidJson(error.to_string()))?;
        corpus.validate()?;
        Ok(corpus)
    }

    pub fn validate(&self) -> Result<(), SchemaError> {
        validate_source(self.schema_version, &self.source)?;
        let mut keys = BTreeSet::new();
        let mut package_names = BTreeSet::new();
        let mut classifier_count = 0usize;
        let mut feature_count = 0usize;
        for package in &self.packages {
            validate_text("feature semantics bundle", &package.bundle)?;
            validate_portable_pathlike("feature semantics resource", &package.resource)?;
            validate_text("feature semantics package", &package.package_name)?;
            validate_uri("feature semantics namespace URI", &package.namespace_uri)?;
            if !package_names.insert(package.namespace_uri.as_str()) {
                return Err(SchemaError::DuplicateValue {
                    field: "feature semantics namespace URI",
                    value: package.namespace_uri.clone(),
                });
            }
            let mut classifiers = BTreeSet::new();
            for classifier in &package.classifiers {
                validate_text("feature semantics classifier", &classifier.name)?;
                if !classifiers.insert(classifier.name.as_str()) {
                    return Err(SchemaError::DuplicateValue {
                        field: "feature semantics classifier",
                        value: classifier.name.clone(),
                    });
                }
                classifier_count += 1;
                for semantics in &classifier.features {
                    let key = FeatureSemanticKey {
                        namespace_uri: package.namespace_uri.clone(),
                        classifier: classifier.name.clone(),
                        feature: semantics.name.clone(),
                    };
                    validate_feature_semantic_key(&key)?;
                    if !keys.insert(key.clone()) {
                        return Err(SchemaError::DuplicateValue {
                            field: "feature semantic key",
                            value: format!(
                                "{} / {} / {}",
                                key.namespace_uri, key.classifier, key.feature
                            ),
                        });
                    }
                    validate_text("feature model type", &semantics.model_type)?;
                    let mut qualifiers = BTreeSet::new();
                    for qualifier in &semantics.qualifiers {
                        if !qualifiers.insert(*qualifier) {
                            return Err(SchemaError::DuplicateValue {
                                field: "feature qualifier",
                                value: format!("{qualifier:?}").to_ascii_lowercase(),
                            });
                        }
                    }
                    if let Some(upper) = semantics.upper_bound
                        && semantics.lower_bound > upper
                    {
                        return Err(SchemaError::InvalidCardinality {
                            lower: semantics.lower_bound,
                            upper,
                        });
                    }
                    validate_feature_evidence("feature model evidence", &semantics.model_evidence)?;
                    validate_xml_feature_behavior(
                        &key,
                        &semantics.xml,
                        semantics.xml.evidence.status,
                    )?;
                    feature_count += 1;
                }
            }
        }
        validate_count("packages", self.summary.packages, self.packages.len())?;
        validate_count("classifiers", self.summary.classifiers, classifier_count)?;
        validate_count("features", self.summary.features, feature_count)
    }

    pub fn feature(&self, key: &FeatureSemanticKey) -> Option<&FeatureSemantics> {
        self.packages
            .iter()
            .find(|package| package.namespace_uri == key.namespace_uri)
            .and_then(|package| {
                package
                    .classifiers
                    .iter()
                    .find(|classifier| classifier.name == key.classifier)
            })
            .and_then(|classifier| {
                classifier
                    .features
                    .iter()
                    .find(|semantics| semantics.name == key.feature)
            })
    }
}

pub fn bundled_model_inventory() -> Result<ModelInventory, SchemaError> {
    ModelInventory::parse(BUNDLED_MODEL_INVENTORY_JSON)
}

pub fn bundled_package_features() -> Result<PackageFeatureCorpus, SchemaError> {
    PackageFeatureCorpus::parse(BUNDLED_PACKAGE_FEATURES_JSON)
}

pub fn bundled_feature_semantics() -> Result<FeatureSemanticsCorpus, SchemaError> {
    FeatureSemanticsCorpus::parse(BUNDLED_FEATURE_SEMANTICS_JSON)
}

pub fn bundled_writer_rules() -> Result<WriterRuleCorpus, SchemaError> {
    WriterRuleCorpus::parse(BUNDLED_WRITER_RULES_JSON)
}

fn validate_source(schema_version: u32, source: &CorpusSource) -> Result<(), SchemaError> {
    if schema_version != 1 {
        return Err(SchemaError::UnsupportedSchemaVersion(schema_version));
    }
    for (field, value) in [
        ("source product", source.product.as_str()),
        ("source release", source.release.as_str()),
        ("source derivation", source.derivation.as_str()),
    ] {
        validate_text(field, value)?;
    }
    Ok(())
}

fn validate_feature_semantic_key(key: &FeatureSemanticKey) -> Result<(), SchemaError> {
    validate_uri("feature semantic namespace URI", &key.namespace_uri)?;
    validate_text("feature semantic classifier", &key.classifier)?;
    validate_text("feature semantic feature", &key.feature)
}

fn validate_feature_evidence(
    field: &'static str,
    evidence: &FeatureEvidence,
) -> Result<(), SchemaError> {
    validate_text("feature evidence kind", &evidence.kind)?;
    if evidence.status == EvidenceStatus::Verified && evidence.sources.is_empty() {
        return Err(SchemaError::EmptyField("verified feature evidence sources"));
    }
    let mut sources = BTreeSet::new();
    for source in &evidence.sources {
        validate_portable_pathlike("feature evidence source", source)?;
        if !sources.insert(source.as_str()) {
            return Err(SchemaError::DuplicateValue {
                field: "feature evidence source",
                value: source.clone(),
            });
        }
    }
    if let Some(note) = &evidence.note {
        validate_text(field, note)?;
    }
    Ok(())
}

fn validate_xml_feature_behavior(
    key: &FeatureSemanticKey,
    behavior: &XmlFeatureBehavior,
    status: EvidenceStatus,
) -> Result<(), SchemaError> {
    validate_feature_evidence("feature XML evidence", &behavior.evidence)?;
    if let Some(qname) = &behavior.qname {
        validate_text("XML QName", qname)?;
    }
    validate_evidence_value("XML version gate", &behavior.version_gate)?;
    validate_evidence_value("XML delegate", &behavior.delegate)?;
    if status == EvidenceStatus::Verified {
        if behavior.qname.is_none() {
            return Err(SchemaError::IncompleteVerifiedXmlBehavior {
                key: key.clone(),
                field: "QName",
            });
        }
        if behavior.order.is_none() {
            return Err(SchemaError::IncompleteVerifiedXmlBehavior {
                key: key.clone(),
                field: "order",
            });
        }
        if behavior.emit_default.is_none() {
            return Err(SchemaError::IncompleteVerifiedXmlBehavior {
                key: key.clone(),
                field: "default emission",
            });
        }
        if matches!(behavior.version_gate, EvidenceValue::Pending) {
            return Err(SchemaError::IncompleteVerifiedXmlBehavior {
                key: key.clone(),
                field: "version gate",
            });
        }
        if matches!(behavior.delegate, EvidenceValue::Pending) {
            return Err(SchemaError::IncompleteVerifiedXmlBehavior {
                key: key.clone(),
                field: "delegate",
            });
        }
    }
    Ok(())
}

fn validate_evidence_value(
    field: &'static str,
    evidence: &EvidenceValue<String>,
) -> Result<(), SchemaError> {
    if let EvidenceValue::Known { value: Some(value) } = evidence {
        validate_text(field, value)?;
    }
    Ok(())
}

fn validate_text(field: &'static str, value: &str) -> Result<(), SchemaError> {
    if value.trim().is_empty() {
        return Err(SchemaError::EmptyField(field));
    }
    let lower = value.to_ascii_lowercase();
    let drive_rooted = value.as_bytes().get(1) == Some(&b':')
        && matches!(value.as_bytes().get(2), Some(b'/') | Some(b'\\'));
    if drive_rooted
        || lower.starts_with("file:")
        || value.starts_with("\\\\")
        || value.starts_with("//")
        || value.starts_with('/')
        || lower.contains("program files")
        || lower.contains("users\\")
    {
        return Err(SchemaError::NonPortablePath(value.to_owned()));
    }
    Ok(())
}

fn validate_portable_pathlike(field: &'static str, value: &str) -> Result<(), SchemaError> {
    validate_text(field, value)
}

fn validate_uri(field: &'static str, value: &str) -> Result<(), SchemaError> {
    validate_text(field, value)?;
    let Some((scheme, _)) = value.split_once(':') else {
        return Err(SchemaError::InvalidJson(format!("{field} is not a URI")));
    };
    if scheme.is_empty()
        || !scheme.chars().enumerate().all(|(index, character)| {
            character.is_ascii_alphabetic()
                || (index > 0
                    && (character.is_ascii_digit() || matches!(character, '+' | '-' | '.')))
        })
    {
        return Err(SchemaError::InvalidJson(format!("{field} is not a URI")));
    }
    Ok(())
}

fn validate_unique_names(field: &'static str, values: &[String]) -> Result<(), SchemaError> {
    let mut unique = BTreeSet::new();
    for value in values {
        validate_text(field, value)?;
        if !unique.insert(value.as_str()) {
            return Err(SchemaError::DuplicateValue {
                field,
                value: value.clone(),
            });
        }
    }
    Ok(())
}

fn validate_members(field: &'static str, values: &[ModelMember]) -> Result<(), SchemaError> {
    let mut tokens = BTreeSet::new();
    for value in values {
        validate_text(field, &value.token)?;
        if !tokens.insert(value.token.as_str()) {
            return Err(SchemaError::DuplicateValue {
                field,
                value: value.token.clone(),
            });
        }
    }
    Ok(())
}

fn validate_count(field: &'static str, expected: usize, actual: usize) -> Result<(), SchemaError> {
    if expected != actual {
        return Err(SchemaError::SummaryMismatch {
            field,
            expected,
            actual,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_inventory_is_complete_and_portable() {
        let inventory = bundled_model_inventory().unwrap();
        assert_eq!(inventory.source.release, "2025.2.3+30");
        assert_eq!(inventory.summary.bundles, 76);
        assert_eq!(inventory.summary.model_types, 3_902);
        assert_eq!(inventory.summary.importers, 229);
        assert_eq!(inventory.summary.exporters, 265);
        assert!(
            inventory
                .bundle("com._1c.g5.v8.dt.form.export.xml")
                .unwrap()
                .exporters
                .iter()
                .any(|name| name.ends_with("FormChoiceListDesTimeValueWriter"))
        );
    }

    #[test]
    fn bundled_writer_rules_are_verified_and_queryable() {
        let corpus = bundled_writer_rules().unwrap();
        assert_eq!(corpus.rules.len(), 3);
        let choice = corpus.rule("form.choice-list.design-time-value").unwrap();
        assert_eq!(choice.evidence.status, "verified");
        assert!(
            choice
                .conditions
                .iter()
                .any(|condition| condition.contains("8.5.1"))
        );
    }

    #[test]
    fn bundled_package_features_include_real_form_model_fields() {
        let corpus = bundled_package_features().unwrap();
        assert!(corpus.summary.packages > 50);
        assert!(corpus.summary.classifiers > 1_000);
        assert!(corpus.summary.features > 5_000);
        let package = corpus
            .package("com._1c.g5.v8.dt.form.model.FormPackage")
            .unwrap();
        let form = package
            .classifiers
            .iter()
            .find(|classifier| classifier.token == "FORM")
            .unwrap();
        assert_eq!(form.feature_count, Some(65));
        assert!(
            form.features
                .iter()
                .any(|feature| feature.token == "SHOW_TITLE851" && feature.id == 47)
        );
    }

    #[test]
    fn bundled_feature_semantics_cover_representative_form_features() {
        let corpus = bundled_feature_semantics().unwrap();
        assert_eq!(corpus.source.release, "2025.2.3+30");
        assert_eq!(corpus.summary.packages, 1);
        assert_eq!(corpus.summary.classifiers, 257);
        assert_eq!(corpus.summary.features, 919);

        let key = |classifier: &str, feature: &str| FeatureSemanticKey {
            namespace_uri: "http://g5.1c.ru/v8/dt/form".to_owned(),
            classifier: classifier.to_owned(),
            feature: feature.to_owned(),
        };

        let attributes = corpus.feature(&key("Form", "attributes")).unwrap();
        assert_eq!(attributes.kind, FeatureKind::Containment);
        assert_eq!(attributes.model_type, "FormAttribute");
        assert_eq!((attributes.lower_bound, attributes.upper_bound), (0, None));

        let base_form = corpus.feature(&key("Form", "baseForm")).unwrap();
        assert_eq!(base_form.kind, FeatureKind::Reference);
        assert_eq!((base_form.lower_bound, base_form.upper_bound), (0, Some(1)));
        assert_eq!(base_form.qualifiers, vec![XcoreFeatureQualifier::Transient]);

        let segments = corpus
            .feature(&key("AbstractDataPath", "segments"))
            .unwrap();
        assert_eq!(segments.kind, FeatureKind::Attribute);
        assert_eq!(segments.model_type, "String");
        assert_eq!((segments.lower_bound, segments.upper_bound), (1, None));

        let image_scale = corpus
            .feature(&key("ImageFieldExtInfo", "imageScale"))
            .unwrap();
        assert_eq!(image_scale.default_value.as_deref(), Some("100"));
        assert_eq!(image_scale.xml.evidence.status, EvidenceStatus::Pending);
    }

    #[test]
    fn validation_rejects_machine_specific_paths() {
        let mut inventory = bundled_model_inventory().unwrap();
        inventory.bundles[0].model_types[0] = r"C:\Program Files\1C\secret".to_owned();
        assert!(matches!(
            inventory.validate(),
            Err(SchemaError::NonPortablePath(_))
        ));
    }

    fn feature_semantics_fixture() -> FeatureSemanticsCorpus {
        FeatureSemanticsCorpus {
            schema_version: 1,
            source: CorpusSource {
                product: "1C:EDT".to_owned(),
                release: "2025.2.3+30".to_owned(),
                derivation: "local Xcore inventory".to_owned(),
            },
            summary: FeatureSemanticsSummary {
                packages: 1,
                classifiers: 1,
                features: 1,
            },
            packages: vec![FeatureSemanticsPackage {
                bundle: "com._1c.g5.v8.dt.form.model".to_owned(),
                resource: "model/form.xcore".to_owned(),
                package_name: "form".to_owned(),
                namespace_uri: "http://v8.1c.ru/8.3/xcf/logform".to_owned(),
                classifiers: vec![FeatureSemanticsClassifier {
                    name: "Form".to_owned(),
                    kind: FeatureClassifierKind::Class,
                    features: vec![FeatureSemantics {
                        name: "baseForm".to_owned(),
                        kind: FeatureKind::Reference,
                        model_type: "Form".to_owned(),
                        lower_bound: 0,
                        upper_bound: Some(1),
                        default_value: None,
                        qualifiers: vec![XcoreFeatureQualifier::Transient],
                        model_evidence: FeatureEvidence {
                            status: EvidenceStatus::Verified,
                            kind: "xcore".to_owned(),
                            sources: vec!["model/form.xcore".to_owned()],
                            note: None,
                        },
                        xml: XmlFeatureBehavior {
                            qname: Some("form:baseForm".to_owned()),
                            order: Some(12),
                            emit_default: Some(false),
                            version_gate: EvidenceValue::Known {
                                value: Some("8.3".to_owned()),
                            },
                            delegate: EvidenceValue::Known {
                                value: Some("FormWriter".to_owned()),
                            },
                            evidence: FeatureEvidence {
                                status: EvidenceStatus::Verified,
                                kind: "writer-inspection".to_owned(),
                                sources: vec!["FormWriter".to_owned()],
                                note: None,
                            },
                        },
                    }],
                }],
            }],
        }
    }

    #[test]
    fn feature_semantics_reject_duplicate_semantic_keys() {
        let mut corpus = feature_semantics_fixture();
        let duplicate = corpus.packages[0].classifiers[0].features[0].clone();
        corpus.packages[0].classifiers[0].features.push(duplicate);
        corpus.summary.features = 2;
        let json = serde_json::to_string(&corpus).unwrap();
        assert!(matches!(
            FeatureSemanticsCorpus::parse(&json),
            Err(SchemaError::DuplicateValue {
                field: "feature semantic key",
                ..
            })
        ));
    }

    #[test]
    fn feature_semantics_reject_invalid_cardinality() {
        let mut corpus = feature_semantics_fixture();
        let feature = &mut corpus.packages[0].classifiers[0].features[0];
        feature.lower_bound = 2;
        feature.upper_bound = Some(1);
        let json = serde_json::to_string(&corpus).unwrap();
        assert!(matches!(
            FeatureSemanticsCorpus::parse(&json),
            Err(SchemaError::InvalidCardinality { lower: 2, upper: 1 })
        ));
    }

    #[test]
    fn feature_semantics_reject_incomplete_verified_xml_behavior() {
        let mut corpus = feature_semantics_fixture();
        corpus.packages[0].classifiers[0].features[0]
            .xml
            .emit_default = None;
        let json = serde_json::to_string(&corpus).unwrap();
        assert!(matches!(
            FeatureSemanticsCorpus::parse(&json),
            Err(SchemaError::IncompleteVerifiedXmlBehavior {
                field: "default emission",
                ..
            })
        ));
    }

    #[test]
    fn feature_semantics_distinguish_verified_absence_from_pending_optional_xml_facts() {
        let mut corpus = feature_semantics_fixture();
        {
            let xml = &mut corpus.packages[0].classifiers[0].features[0].xml;
            xml.version_gate = EvidenceValue::Known { value: None };
            xml.delegate = EvidenceValue::Known { value: None };
        }
        assert!(corpus.validate().is_ok());

        {
            let xml = &mut corpus.packages[0].classifiers[0].features[0].xml;
            xml.version_gate = EvidenceValue::Pending;
            xml.delegate = EvidenceValue::Pending;
        }
        assert!(matches!(
            corpus.validate(),
            Err(SchemaError::IncompleteVerifiedXmlBehavior {
                field: "version gate",
                ..
            })
        ));
        corpus.packages[0].classifiers[0].features[0]
            .xml
            .evidence
            .status = EvidenceStatus::Pending;
        assert!(corpus.validate().is_ok());
    }

    #[test]
    fn feature_semantics_allow_unknown_pending_xml_behavior() {
        let mut corpus = feature_semantics_fixture();
        let xml = &mut corpus.packages[0].classifiers[0].features[0].xml;
        xml.qname = None;
        xml.order = None;
        xml.emit_default = None;
        xml.version_gate = EvidenceValue::Pending;
        xml.delegate = EvidenceValue::Pending;
        xml.evidence.status = EvidenceStatus::Pending;
        assert!(corpus.validate().is_ok());
    }

    #[test]
    fn feature_semantics_key_uses_namespace_uri_not_package_name() {
        let mut corpus = feature_semantics_fixture();
        let mut second = corpus.packages[0].clone();
        second.namespace_uri = "http://v8.1c.ru/8.3/xcf/other-form".to_owned();
        second.resource = "model/other-form.xcore".to_owned();
        corpus.packages.push(second);
        corpus.summary.packages = 2;
        corpus.summary.classifiers = 2;
        corpus.summary.features = 2;
        assert!(corpus.validate().is_ok());
        assert!(
            corpus
                .feature(&FeatureSemanticKey {
                    namespace_uri: "http://v8.1c.ru/8.3/xcf/other-form".to_owned(),
                    classifier: "Form".to_owned(),
                    feature: "baseForm".to_owned(),
                })
                .is_some()
        );
    }

    #[test]
    fn feature_semantics_reject_unknown_classifier_kind_and_unportable_sources() {
        let corpus = feature_semantics_fixture();
        let mut json = serde_json::to_value(&corpus).unwrap();
        json["packages"][0]["classifiers"][0]["kind"] = serde_json::json!("unknown");
        assert!(matches!(
            FeatureSemanticsCorpus::parse(&serde_json::to_string(&json).unwrap()),
            Err(SchemaError::InvalidJson(_))
        ));

        let mut corpus = feature_semantics_fixture();
        corpus.packages[0].resource = r"C:/EDT/model/form.xcore".to_owned();
        assert!(matches!(
            corpus.validate(),
            Err(SchemaError::NonPortablePath(_))
        ));
        corpus.packages[0].resource = "file:///C:/EDT/model/form.xcore".to_owned();
        assert!(matches!(
            corpus.validate(),
            Err(SchemaError::NonPortablePath(_))
        ));
        corpus.packages[0].resource = "model/form.xcore".to_owned();
        corpus.packages[0].classifiers[0].features[0]
            .model_evidence
            .sources = vec![r"\\server\share\form.xcore".to_owned()];
        assert!(matches!(
            corpus.validate(),
            Err(SchemaError::NonPortablePath(_))
        ));
    }

    #[test]
    fn verified_feature_evidence_requires_a_source() {
        let mut corpus = feature_semantics_fixture();
        corpus.packages[0].classifiers[0].features[0]
            .model_evidence
            .sources
            .clear();
        assert!(matches!(
            corpus.validate(),
            Err(SchemaError::EmptyField("verified feature evidence sources"))
        ));
    }

    #[test]
    fn feature_semantics_use_importer_camel_case_and_preserve_base_form_qualifier() {
        let corpus = feature_semantics_fixture();
        let json = serde_json::to_value(&corpus).unwrap();
        let feature = &json["packages"][0]["classifiers"][0]["features"][0];
        assert_eq!(feature["name"], "baseForm");
        assert_eq!(feature["qualifiers"], serde_json::json!(["transient"]));
        assert_eq!(feature["xml"]["qname"], "form:baseForm");
        assert_eq!(feature["xml"]["emitDefault"], false);
        assert_eq!(
            feature["xml"]["versionGate"],
            serde_json::json!({"status": "known", "value": "8.3"})
        );
        assert!(feature["xml"].get("qName").is_none());
    }
}
