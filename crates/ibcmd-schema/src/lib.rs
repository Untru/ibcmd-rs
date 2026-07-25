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

/// Embedded EDT Xcore-derived feature semantics for every packaged model resource.
pub const BUNDLED_FEATURE_SEMANTICS_JSON: &str =
    include_str!("../data/edt-2025.2.3-feature-semantics.json");

/// Embedded exhaustive canonical-model implementation coverage.
pub const BUNDLED_CANONICAL_COVERAGE_JSON: &str =
    include_str!("../data/edt-2025.2.3-canonical-coverage.json");

/// Embedded, provider-derived metadata and produced-type feature order.
pub const BUNDLED_METADATA_ORDER_JSON: &str =
    include_str!("../data/edt-2025.2.3-metadata-order.json");

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
    #[serde(default)]
    pub policy: Option<WriterPolicy>,
    pub evidence: RuleEvidence,
}

/// A structured subset of verified writer behaviour.  Free-form operations remain useful
/// provenance, but production writers must consume this typed policy instead of parsing prose.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum WriterPolicy {
    FormChoiceList {
        #[serde(rename = "itemOrder")]
        item_order: Vec<FormChoiceListItemPart>,
        #[serde(rename = "emptyCollection")]
        empty_collection: FormChoiceListEmptyCollection,
    },
    FormListSettings {
        #[serde(rename = "nullValue")]
        null_value: FormListSettingsNullValue,
        delegate: String,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FormChoiceListItemPart {
    Presentation,
    CheckState,
    Value,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FormChoiceListEmptyCollection {
    WriteWrapperWhenWriteDefault,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FormListSettingsNullValue {
    Omit,
}

/// Exact identity used by a writer-rule consumer.  The release is deliberately part of the
/// key: silently reusing evidence obtained from a different EDT release is forbidden.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WriterRuleKey<'a> {
    pub source_release: &'a str,
    pub model_type: &'a str,
    pub feature: &'a str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WriterRuleLookupError {
    SourceReleaseMismatch {
        requested: String,
        available: String,
    },
    Missing {
        model_type: String,
        feature: String,
    },
    Ambiguous {
        model_type: String,
        feature: String,
    },
    Unverified {
        id: String,
        status: String,
    },
    MissingTypedPolicy {
        id: String,
    },
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
    Derived,
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

/// How one EDT feature is preserved by the canonical model.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CoverageStatus {
    Typed,
    OpaqueLossless,
    Unsupported,
    PlatformOnly,
}

/// One explicit EDT feature to canonical-model mapping.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalCoverageEntry {
    pub key: FeatureSemanticKey,
    pub family: String,
    pub status: CoverageStatus,
    pub canonical_type: Option<String>,
    pub canonical_field: Option<String>,
    pub opaque_placement: Option<String>,
    pub diagnostic_code: Option<String>,
    pub evidence: FeatureEvidence,
}

/// Derived coverage totals for completeness and reporting.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalCoverageSummary {
    pub entries: usize,
    pub typed: usize,
    pub opaque_lossless: usize,
    pub unsupported: usize,
    pub platform_only: usize,
}

/// Complete coverage mapping for one EDT-derived feature corpus.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalCoverageCorpus {
    pub schema_version: u32,
    pub source: CorpusSource,
    pub summary: CanonicalCoverageSummary,
    pub entries: Vec<CanonicalCoverageEntry>,
}

/// EDT writer-provider section whose feature order was observed.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MetadataOrderSection {
    InternalInfo,
    Properties,
    ChildObjects,
    ProducedTypes,
}

/// Version condition attached to a provider order record.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MetadataOrderVersionPredicate {
    Always,
    #[serde(rename = "greaterThan(V8_3_14)")]
    GreaterThanV8_3_14,
    #[serde(rename = "notGreaterThan(V8_3_14)")]
    NotGreaterThanV8_3_14,
}

/// Explicit provider fallback; it is not an XML default or emission rule.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum MetadataOrderFallback {
    #[serde(rename = "eClass.getEAllReferences() when ORDER_MAP has no key")]
    AllReferencesWhenUnmapped,
    #[serde(
        rename = "ListBuilder(eClass, defaultPropertyFilter).build() when propertiesOrderMap has no key"
    )]
    DefaultPropertyFilterWhenUnmapped,
    #[serde(
        rename = "eClass.getEStructuralFeature(\"producedTypes\") when present, otherwise empty list"
    )]
    ProducedTypesWhenPresent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MetadataOrderOperationKind {
    Cursor,
    Next,
    Emit,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataOrderOperation {
    pub operation: MetadataOrderOperationKind,
    pub feature: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataOrderRecord {
    pub provider: String,
    pub classifier: String,
    pub section: MetadataOrderSection,
    pub ordered_features: Vec<String>,
    #[serde(default)]
    pub order_operations: Vec<MetadataOrderOperation>,
    pub version_predicate: MetadataOrderVersionPredicate,
    pub fallback: MetadataOrderFallback,
    pub evidence: FeatureEvidence,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataOrderSummary {
    pub bundle: String,
    pub verified_records: usize,
    pub rejected_records: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataOrderCorpus {
    pub schema_version: u32,
    pub source: CorpusSource,
    pub summary: MetadataOrderSummary,
    pub records: Vec<MetadataOrderRecord>,
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
    InvalidCoverageEntry {
        key: FeatureSemanticKey,
        reason: &'static str,
    },
    CoverageMismatch {
        kind: &'static str,
        key: FeatureSemanticKey,
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
            Self::InvalidCoverageEntry { key, reason } => write!(
                formatter,
                "invalid canonical coverage for {} / {} / {}: {reason}",
                key.namespace_uri, key.classifier, key.feature
            ),
            Self::CoverageMismatch { kind, key } => write!(
                formatter,
                "{kind} canonical coverage key {} / {} / {}",
                key.namespace_uri, key.classifier, key.feature
            ),
        }
    }
}

impl Error for SchemaError {}

impl Display for WriterRuleLookupError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourceReleaseMismatch {
                requested,
                available,
            } => write!(
                formatter,
                "writer rule source release mismatch: requested `{requested}`, available `{available}`"
            ),
            Self::Missing {
                model_type,
                feature,
            } => write!(
                formatter,
                "writer rule is missing for `{model_type}` / `{feature}`"
            ),
            Self::Ambiguous {
                model_type,
                feature,
            } => write!(
                formatter,
                "writer rule is ambiguous for `{model_type}` / `{feature}`"
            ),
            Self::Unverified { id, status } => {
                write!(
                    formatter,
                    "writer rule `{id}` has unverified status `{status}`"
                )
            }
            Self::MissingTypedPolicy { id } => {
                write!(formatter, "writer rule `{id}` has no typed policy")
            }
        }
    }
}

impl Error for WriterRuleLookupError {}

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
        let mut exact_keys = BTreeSet::new();
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
            if !exact_keys.insert((rule.model_type.as_str(), rule.feature.as_str())) {
                return Err(SchemaError::DuplicateValue {
                    field: "writer rule model type/feature",
                    value: format!("{} / {}", rule.model_type, rule.feature),
                });
            }
            if rule.operations.is_empty() {
                return Err(SchemaError::EmptyField("writer rule operations"));
            }
            validate_unique_names("writer operation", &rule.operations)?;
            if let Some(policy) = &rule.policy {
                match policy {
                    WriterPolicy::FormChoiceList { item_order, .. } => {
                        if item_order.as_slice()
                            != [
                                FormChoiceListItemPart::Presentation,
                                FormChoiceListItemPart::CheckState,
                                FormChoiceListItemPart::Value,
                            ]
                        {
                            return Err(SchemaError::EmptyField(
                                "form choice-list verified item order",
                            ));
                        }
                    }
                    WriterPolicy::FormListSettings { delegate, .. } => {
                        validate_text("form list-settings delegate", delegate)?;
                        if rule.delegate.as_deref() != Some(delegate.as_str()) {
                            return Err(SchemaError::EmptyField(
                                "form list-settings matching delegate",
                            ));
                        }
                    }
                }
            }
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

    /// Returns one verified, structured rule for an exact corpus release/model/feature key.
    ///
    /// Every incomplete state is an error.  Callers must not fall back to a rule from another
    /// release or interpret the human-readable `operations`/`conditions` fields.
    pub fn exact_rule(&self, key: WriterRuleKey<'_>) -> Result<&WriterRule, WriterRuleLookupError> {
        if key.source_release != self.source.release {
            return Err(WriterRuleLookupError::SourceReleaseMismatch {
                requested: key.source_release.to_owned(),
                available: self.source.release.clone(),
            });
        }
        let mut matches = self
            .rules
            .iter()
            .filter(|rule| rule.model_type == key.model_type && rule.feature == key.feature);
        let Some(rule) = matches.next() else {
            return Err(WriterRuleLookupError::Missing {
                model_type: key.model_type.to_owned(),
                feature: key.feature.to_owned(),
            });
        };
        if matches.next().is_some() {
            return Err(WriterRuleLookupError::Ambiguous {
                model_type: key.model_type.to_owned(),
                feature: key.feature.to_owned(),
            });
        }
        if rule.evidence.status != "verified" {
            return Err(WriterRuleLookupError::Unverified {
                id: rule.id.clone(),
                status: rule.evidence.status.clone(),
            });
        }
        if rule.policy.is_none() {
            return Err(WriterRuleLookupError::MissingTypedPolicy {
                id: rule.id.clone(),
            });
        }
        Ok(rule)
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

impl CanonicalCoverageCorpus {
    pub fn parse(json: &str) -> Result<Self, SchemaError> {
        let corpus: Self = serde_json::from_str(json)
            .map_err(|error| SchemaError::InvalidJson(error.to_string()))?;
        corpus.validate()?;
        Ok(corpus)
    }

    pub fn validate(&self) -> Result<(), SchemaError> {
        validate_source(self.schema_version, &self.source)?;
        let mut keys = BTreeSet::new();
        let mut typed = 0usize;
        let mut opaque_lossless = 0usize;
        let mut unsupported = 0usize;
        let mut platform_only = 0usize;

        for entry in &self.entries {
            validate_feature_semantic_key(&entry.key)?;
            validate_text("canonical coverage family", &entry.family)?;
            validate_feature_evidence("canonical coverage evidence", &entry.evidence)?;
            if entry.evidence.status != EvidenceStatus::Verified {
                return Err(SchemaError::InvalidCoverageEntry {
                    key: entry.key.clone(),
                    reason: "coverage mapping requires verified evidence",
                });
            }
            if !keys.insert(entry.key.clone()) {
                return Err(SchemaError::DuplicateValue {
                    field: "canonical coverage key",
                    value: format!(
                        "{} / {} / {}",
                        entry.key.namespace_uri, entry.key.classifier, entry.key.feature
                    ),
                });
            }
            for (field, value) in [
                ("canonical coverage type", entry.canonical_type.as_deref()),
                ("canonical coverage field", entry.canonical_field.as_deref()),
                (
                    "canonical opaque placement",
                    entry.opaque_placement.as_deref(),
                ),
                (
                    "canonical diagnostic code",
                    entry.diagnostic_code.as_deref(),
                ),
            ] {
                if let Some(value) = value {
                    validate_text(field, value)?;
                }
            }
            match entry.status {
                CoverageStatus::Typed => {
                    if entry.canonical_type.is_none() || entry.canonical_field.is_none() {
                        return Err(SchemaError::InvalidCoverageEntry {
                            key: entry.key.clone(),
                            reason: "typed mapping requires canonical type and field",
                        });
                    }
                    if entry.opaque_placement.is_some() || entry.diagnostic_code.is_some() {
                        return Err(SchemaError::InvalidCoverageEntry {
                            key: entry.key.clone(),
                            reason: "typed mapping contains irrelevant status fields",
                        });
                    }
                    typed += 1;
                }
                CoverageStatus::OpaqueLossless => {
                    if entry.opaque_placement.is_none() {
                        return Err(SchemaError::InvalidCoverageEntry {
                            key: entry.key.clone(),
                            reason: "opaque-lossless mapping requires placement",
                        });
                    }
                    if entry.canonical_type.is_some()
                        || entry.canonical_field.is_some()
                        || entry.diagnostic_code.is_some()
                    {
                        return Err(SchemaError::InvalidCoverageEntry {
                            key: entry.key.clone(),
                            reason: "opaque-lossless mapping contains irrelevant status fields",
                        });
                    }
                    opaque_lossless += 1;
                }
                CoverageStatus::Unsupported => {
                    if entry.diagnostic_code.is_none() {
                        return Err(SchemaError::InvalidCoverageEntry {
                            key: entry.key.clone(),
                            reason: "unsupported mapping requires diagnostic code",
                        });
                    }
                    if entry.canonical_type.is_some()
                        || entry.canonical_field.is_some()
                        || entry.opaque_placement.is_some()
                    {
                        return Err(SchemaError::InvalidCoverageEntry {
                            key: entry.key.clone(),
                            reason: "unsupported mapping contains irrelevant status fields",
                        });
                    }
                    unsupported += 1;
                }
                CoverageStatus::PlatformOnly => {
                    if entry.evidence.note.is_none() {
                        return Err(SchemaError::InvalidCoverageEntry {
                            key: entry.key.clone(),
                            reason: "platform-only mapping requires an evidence note",
                        });
                    }
                    if entry.canonical_type.is_some()
                        || entry.canonical_field.is_some()
                        || entry.opaque_placement.is_some()
                        || entry.diagnostic_code.is_some()
                    {
                        return Err(SchemaError::InvalidCoverageEntry {
                            key: entry.key.clone(),
                            reason: "platform-only mapping contains irrelevant status fields",
                        });
                    }
                    platform_only += 1;
                }
            }
        }

        validate_count("coverage entries", self.summary.entries, self.entries.len())?;
        validate_count("typed coverage", self.summary.typed, typed)?;
        validate_count(
            "opaque-lossless coverage",
            self.summary.opaque_lossless,
            opaque_lossless,
        )?;
        validate_count(
            "unsupported coverage",
            self.summary.unsupported,
            unsupported,
        )?;
        validate_count(
            "platform-only coverage",
            self.summary.platform_only,
            platform_only,
        )
    }

    /// Proves that coverage and feature corpora form an exact full join.
    pub fn validate_against(&self, features: &FeatureSemanticsCorpus) -> Result<(), SchemaError> {
        self.validate()?;
        features.validate()?;

        let feature_keys = features
            .packages
            .iter()
            .flat_map(|package| {
                package.classifiers.iter().flat_map(move |classifier| {
                    classifier
                        .features
                        .iter()
                        .map(move |feature| FeatureSemanticKey {
                            namespace_uri: package.namespace_uri.clone(),
                            classifier: classifier.name.clone(),
                            feature: feature.name.clone(),
                        })
                })
            })
            .collect::<BTreeSet<_>>();
        let coverage_keys = self
            .entries
            .iter()
            .map(|entry| entry.key.clone())
            .collect::<BTreeSet<_>>();

        if let Some(key) = feature_keys.difference(&coverage_keys).next() {
            return Err(SchemaError::CoverageMismatch {
                kind: "unmapped",
                key: key.clone(),
            });
        }
        if let Some(key) = coverage_keys.difference(&feature_keys).next() {
            return Err(SchemaError::CoverageMismatch {
                kind: "stale",
                key: key.clone(),
            });
        }
        Ok(())
    }
}

impl MetadataOrderCorpus {
    pub fn parse(json: &str) -> Result<Self, SchemaError> {
        let corpus: Self = serde_json::from_str(json)
            .map_err(|error| SchemaError::InvalidJson(error.to_string()))?;
        corpus.validate()?;
        Ok(corpus)
    }

    pub fn validate(&self) -> Result<(), SchemaError> {
        validate_source(self.schema_version, &self.source)?;
        validate_text("metadata order bundle", &self.summary.bundle)?;
        let mut keys = BTreeSet::new();

        for record in &self.records {
            validate_text("metadata order provider", &record.provider)?;
            validate_text("metadata order classifier", &record.classifier)?;
            if record.ordered_features.is_empty() {
                return Err(SchemaError::EmptyField("metadata order ordered features"));
            }
            let mut features = BTreeSet::new();
            for feature in &record.ordered_features {
                validate_text("metadata order feature", feature)?;
                if !features.insert(feature.as_str()) {
                    return Err(SchemaError::DuplicateValue {
                        field: "metadata order feature",
                        value: feature.clone(),
                    });
                }
            }
            for operation in &record.order_operations {
                validate_text("metadata order operation feature", &operation.feature)?;
            }
            let operation_features = record
                .order_operations
                .iter()
                .map(|operation| operation.feature.as_str())
                .collect::<Vec<_>>();
            let ordered_features = record
                .ordered_features
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>();
            match record.section {
                MetadataOrderSection::Properties => {
                    if operation_features != ordered_features
                        || record.order_operations.first().is_none_or(|operation| {
                            operation.operation != MetadataOrderOperationKind::Cursor
                        })
                        || record.order_operations.iter().any(|operation| {
                            operation.operation == MetadataOrderOperationKind::Emit
                        })
                    {
                        return Err(SchemaError::InvalidJson(format!(
                            "invalid properties order operations for {}",
                            record.classifier
                        )));
                    }
                    if record.fallback != MetadataOrderFallback::DefaultPropertyFilterWhenUnmapped {
                        return Err(SchemaError::InvalidJson(format!(
                            "invalid properties fallback for {}",
                            record.classifier
                        )));
                    }
                }
                MetadataOrderSection::InternalInfo | MetadataOrderSection::ChildObjects => {
                    if operation_features != ordered_features
                        || record.order_operations.iter().any(|operation| {
                            operation.operation != MetadataOrderOperationKind::Emit
                        })
                    {
                        return Err(SchemaError::InvalidJson(format!(
                            "invalid {:?} order operations for {}",
                            record.section, record.classifier
                        )));
                    }
                    if record.fallback != MetadataOrderFallback::ProducedTypesWhenPresent {
                        return Err(SchemaError::InvalidJson(format!(
                            "invalid {:?} fallback for {}",
                            record.section, record.classifier
                        )));
                    }
                }
                MetadataOrderSection::ProducedTypes => {
                    if !record.order_operations.is_empty()
                        || record.fallback != MetadataOrderFallback::AllReferencesWhenUnmapped
                    {
                        return Err(SchemaError::InvalidJson(format!(
                            "invalid produced-types operations or fallback for {}",
                            record.classifier
                        )));
                    }
                }
            }
            validate_feature_evidence("metadata order evidence", &record.evidence)?;
            if record.evidence.status != EvidenceStatus::Verified {
                return Err(SchemaError::IncompleteVerifiedXmlBehavior {
                    key: FeatureSemanticKey {
                        namespace_uri: record.provider.clone(),
                        classifier: record.classifier.clone(),
                        feature: format!("{:?}", record.section),
                    },
                    field: "metadata order evidence",
                });
            }
            if !keys.insert((
                record.provider.as_str(),
                record.classifier.as_str(),
                record.section,
                record.version_predicate,
            )) {
                return Err(SchemaError::DuplicateValue {
                    field: "metadata order provider/classifier/section/version",
                    value: format!(
                        "{} / {} / {:?} / {:?}",
                        record.provider,
                        record.classifier,
                        record.section,
                        record.version_predicate
                    ),
                });
            }
        }

        validate_count(
            "metadata verified records",
            self.summary.verified_records,
            self.records.len(),
        )
    }

    pub fn order(
        &self,
        provider: &str,
        classifier: &str,
        section: MetadataOrderSection,
        version_predicate: MetadataOrderVersionPredicate,
    ) -> Option<&MetadataOrderRecord> {
        self.records.iter().find(|record| {
            record.provider == provider
                && record.classifier == classifier
                && record.section == section
                && record.version_predicate == version_predicate
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

pub fn bundled_canonical_coverage() -> Result<CanonicalCoverageCorpus, SchemaError> {
    let coverage = CanonicalCoverageCorpus::parse(BUNDLED_CANONICAL_COVERAGE_JSON)?;
    coverage.validate_against(&bundled_feature_semantics()?)?;
    Ok(coverage)
}

pub fn bundled_metadata_order() -> Result<MetadataOrderCorpus, SchemaError> {
    MetadataOrderCorpus::parse(BUNDLED_METADATA_ORDER_JSON)
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
        let choice = corpus
            .exact_rule(WriterRuleKey {
                source_release: "2025.2.3+30",
                model_type: "FormChoiceList",
                feature: "values",
            })
            .unwrap();
        assert_eq!(choice.evidence.status, "verified");
        assert_eq!(
            choice.policy,
            Some(WriterPolicy::FormChoiceList {
                item_order: vec![
                    FormChoiceListItemPart::Presentation,
                    FormChoiceListItemPart::CheckState,
                    FormChoiceListItemPart::Value,
                ],
                empty_collection: FormChoiceListEmptyCollection::WriteWrapperWhenWriteDefault,
            })
        );

        let settings = corpus
            .exact_rule(WriterRuleKey {
                source_release: "2025.2.3+30",
                model_type: "DynamicListExtInfo",
                feature: "listSettings",
            })
            .unwrap();
        assert_eq!(
            settings.policy,
            Some(WriterPolicy::FormListSettings {
                null_value: FormListSettingsNullValue::Omit,
                delegate: "DcsV8Serializer.writeSettings".to_owned(),
            })
        );
    }

    #[test]
    fn exact_writer_rule_lookup_fails_closed() {
        let corpus = bundled_writer_rules().unwrap();
        assert!(matches!(
            corpus.exact_rule(WriterRuleKey {
                source_release: "2026.1",
                model_type: "FormChoiceList",
                feature: "values",
            }),
            Err(WriterRuleLookupError::SourceReleaseMismatch { .. })
        ));
        assert!(matches!(
            corpus.exact_rule(WriterRuleKey {
                source_release: "2025.2.3+30",
                model_type: "FormChoiceList",
                feature: "unknown",
            }),
            Err(WriterRuleLookupError::Missing { .. })
        ));

        let mut pending = corpus.clone();
        pending
            .rules
            .iter_mut()
            .find(|rule| rule.id == "form.choice-list.design-time-value")
            .expect("fixture rule")
            .evidence
            .status = "pending".to_owned();
        assert!(matches!(
            pending.exact_rule(WriterRuleKey {
                source_release: "2025.2.3+30",
                model_type: "FormChoiceList",
                feature: "values",
            }),
            Err(WriterRuleLookupError::Unverified { .. })
        ));
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
    fn bundled_feature_semantics_cover_all_resources_and_representative_families() {
        let corpus = bundled_feature_semantics().unwrap();
        assert_eq!(corpus.source.release, "2025.2.3+30");
        assert_eq!(corpus.summary.packages, 63);
        assert_eq!(corpus.summary.classifiers, 1_820);
        assert_eq!(corpus.summary.features, 4_966);

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

        let dcs_enabled = corpus
            .feature(&FeatureSemanticKey {
                namespace_uri: "http://g5.1c.ru/v8/dt/data-composition-system/settings".to_owned(),
                classifier: "AvailableFieldUseRestriction".to_owned(),
                feature: "enabled".to_owned(),
            })
            .unwrap();
        assert_eq!(dcs_enabled.kind, FeatureKind::Attribute);
        assert_eq!(dcs_enabled.model_type, "boolean");

        let mcore_gap = corpus
            .feature(&FeatureSemanticKey {
                namespace_uri: "http://g5.1c.ru/v8/dt/mcore".to_owned(),
                classifier: "AbstractLine".to_owned(),
                feature: "gap".to_owned(),
            })
            .unwrap();
        assert_eq!(mcore_gap.model_type, "boolean");

        assert!(corpus.packages.iter().any(|package| {
            package.namespace_uri == "http://g5.1c.ru/v8/dt/binary"
                && package
                    .classifiers
                    .iter()
                    .any(|classifier| classifier.name == "BinaryData")
        }));
    }

    #[test]
    fn bundled_canonical_coverage_is_an_exact_full_join() {
        let corpus = bundled_canonical_coverage().unwrap();
        assert_eq!(corpus.source.release, "2025.2.3+30");
        assert_eq!(corpus.summary.entries, 4_966);
        assert_eq!(corpus.summary.typed, 2);
        assert_eq!(corpus.summary.opaque_lossless, 0);
        assert_eq!(corpus.summary.unsupported, 4_964);
        assert_eq!(corpus.summary.platform_only, 0);

        let family_count = |family: &str| {
            corpus
                .entries
                .iter()
                .filter(|entry| entry.family == family)
                .count()
        };
        assert_eq!(family_count("forms"), 2_314);
        assert_eq!(family_count("dcs"), 511);
        assert_eq!(family_count("common"), 8);
        assert_eq!(family_count("other"), 2_133);
        assert_eq!(
            corpus
                .entries
                .iter()
                .filter(|entry| entry.status == CoverageStatus::Typed)
                .map(|entry| (
                    entry.key.classifier.as_str(),
                    entry.key.feature.as_str(),
                    entry.canonical_field.as_deref().unwrap()
                ))
                .collect::<Vec<_>>(),
            [
                (
                    "DataCompositionSettings",
                    "itemsUserSettingID",
                    "items_user_setting_id"
                ),
                (
                    "DataCompositionSettings",
                    "itemsViewMode",
                    "items_view_mode"
                ),
            ]
        );
        assert!(corpus.entries.iter().all(|entry| {
            entry.status != CoverageStatus::Unsupported
                || entry.diagnostic_code.as_deref() == Some("schema.unmapped")
        }));
    }

    #[test]
    fn bundled_metadata_order_is_verified_and_queryable() {
        let corpus = bundled_metadata_order().unwrap();
        assert_eq!(corpus.source.release, "2025.2.3+30");
        assert_eq!(corpus.summary.verified_records, 60);
        assert_eq!(corpus.summary.rejected_records, 0);
        let catalog = corpus
            .order(
                "ProducedTypesOrderProvider",
                "CATALOG_TYPES",
                MetadataOrderSection::ProducedTypes,
                MetadataOrderVersionPredicate::Always,
            )
            .unwrap();
        assert_eq!(
            catalog.fallback,
            MetadataOrderFallback::AllReferencesWhenUnmapped
        );
        assert_eq!(
            catalog.ordered_features,
            [
                "BASIC_DB_OBJECT_TYPES__OBJECT_TYPE",
                "BASIC_DB_OBJECT_TYPES__REF_TYPE",
                "BASIC_DB_OBJECT_TYPES__SELECTION_TYPE",
                "BASIC_DB_OBJECT_TYPES__LIST_TYPE",
                "BASIC_DB_OBJECT_TYPES__MANAGER_TYPE",
            ]
        );
        assert!(
            corpus
                .records
                .iter()
                .all(|record| record.evidence.status == EvidenceStatus::Verified)
        );

        let configuration = corpus
            .order(
                "MetadataObjectFeatureOrderProvider",
                "CONFIGURATION",
                MetadataOrderSection::Properties,
                MetadataOrderVersionPredicate::GreaterThanV8_3_14,
            )
            .unwrap();
        assert_eq!(
            configuration.fallback,
            MetadataOrderFallback::DefaultPropertyFilterWhenUnmapped
        );
        assert_eq!(
            configuration.order_operations[0].operation,
            MetadataOrderOperationKind::Cursor
        );
        assert_eq!(
            configuration.order_operations[0].feature,
            "MD_OBJECT__COMMENT"
        );
        assert!(
            corpus
                .order(
                    "MetadataObjectFeatureOrderProvider",
                    "CONFIGURATION",
                    MetadataOrderSection::InternalInfo,
                    MetadataOrderVersionPredicate::Always,
                )
                .is_some()
        );
        assert!(
            corpus
                .order(
                    "MetadataObjectFeatureOrderProvider",
                    "DOCUMENT",
                    MetadataOrderSection::Properties,
                    MetadataOrderVersionPredicate::Always,
                )
                .is_some()
        );
    }

    #[test]
    fn metadata_order_rejects_duplicate_classifier_section_version() {
        let mut corpus = bundled_metadata_order().unwrap();
        corpus.records.push(corpus.records[0].clone());
        corpus.summary.verified_records += 1;
        assert!(matches!(
            corpus.validate(),
            Err(SchemaError::DuplicateValue {
                field: "metadata order provider/classifier/section/version",
                ..
            })
        ));
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

    fn canonical_coverage_fixture() -> CanonicalCoverageCorpus {
        CanonicalCoverageCorpus {
            schema_version: 1,
            source: CorpusSource {
                product: "ibcmd-rs".to_owned(),
                release: "2025.2.3+30".to_owned(),
                derivation: "canonical coverage bootstrap".to_owned(),
            },
            summary: CanonicalCoverageSummary {
                entries: 1,
                typed: 1,
                opaque_lossless: 0,
                unsupported: 0,
                platform_only: 0,
            },
            entries: vec![CanonicalCoverageEntry {
                key: FeatureSemanticKey {
                    namespace_uri: "http://g5.1c.ru/v8/dt/form".to_owned(),
                    classifier: "Form".to_owned(),
                    feature: "baseForm".to_owned(),
                },
                family: "forms".to_owned(),
                status: CoverageStatus::Typed,
                canonical_type: Some("CanonicalForm".to_owned()),
                canonical_field: Some("base_form".to_owned()),
                opaque_placement: None,
                diagnostic_code: None,
                evidence: FeatureEvidence {
                    status: EvidenceStatus::Verified,
                    kind: "code-inspection".to_owned(),
                    sources: vec!["crates/ibcmd-core/src/model.rs".to_owned()],
                    note: None,
                },
            }],
        }
    }

    #[test]
    fn canonical_coverage_requires_status_specific_contracts() {
        let mut corpus = canonical_coverage_fixture();
        corpus.entries[0].canonical_field = None;
        assert!(matches!(
            corpus.validate(),
            Err(SchemaError::InvalidCoverageEntry {
                reason: "typed mapping requires canonical type and field",
                ..
            })
        ));

        let entry = &mut corpus.entries[0];
        entry.status = CoverageStatus::OpaqueLossless;
        entry.canonical_type = None;
        entry.canonical_field = None;
        entry.opaque_placement = Some("property-slot".to_owned());
        corpus.summary.typed = 0;
        corpus.summary.opaque_lossless = 1;
        assert!(corpus.validate().is_ok());

        corpus.entries[0].evidence.status = EvidenceStatus::Pending;
        corpus.entries[0].evidence.sources.clear();
        assert!(matches!(
            corpus.validate(),
            Err(SchemaError::InvalidCoverageEntry {
                reason: "coverage mapping requires verified evidence",
                ..
            })
        ));
    }

    #[test]
    fn canonical_coverage_rejects_irrelevant_status_fields() {
        let mut typed = canonical_coverage_fixture();
        typed.entries[0].diagnostic_code = Some("unexpected".to_owned());
        assert!(matches!(
            typed.validate(),
            Err(SchemaError::InvalidCoverageEntry {
                reason: "typed mapping contains irrelevant status fields",
                ..
            })
        ));

        let mut opaque = canonical_coverage_fixture();
        opaque.entries[0].status = CoverageStatus::OpaqueLossless;
        opaque.entries[0].opaque_placement = Some("slot".to_owned());
        opaque.summary.typed = 0;
        opaque.summary.opaque_lossless = 1;
        assert!(matches!(
            opaque.validate(),
            Err(SchemaError::InvalidCoverageEntry {
                reason: "opaque-lossless mapping contains irrelevant status fields",
                ..
            })
        ));

        let mut unsupported = canonical_coverage_fixture();
        unsupported.entries[0].status = CoverageStatus::Unsupported;
        unsupported.entries[0].diagnostic_code = Some("schema.unsupported".to_owned());
        unsupported.summary.typed = 0;
        unsupported.summary.unsupported = 1;
        assert!(matches!(
            unsupported.validate(),
            Err(SchemaError::InvalidCoverageEntry {
                reason: "unsupported mapping contains irrelevant status fields",
                ..
            })
        ));

        let mut platform_only = canonical_coverage_fixture();
        platform_only.entries[0].status = CoverageStatus::PlatformOnly;
        platform_only.entries[0].evidence.note = Some("requires platform runtime".to_owned());
        platform_only.summary.typed = 0;
        platform_only.summary.platform_only = 1;
        assert!(matches!(
            platform_only.validate(),
            Err(SchemaError::InvalidCoverageEntry {
                reason: "platform-only mapping contains irrelevant status fields",
                ..
            })
        ));
    }

    #[test]
    fn canonical_coverage_full_join_rejects_unmapped_and_stale_keys() {
        let mut features = feature_semantics_fixture();
        features.packages[0].package_name = "com._1c.g5.v8.dt.form.model".to_owned();
        features.packages[0].namespace_uri = "http://g5.1c.ru/v8/dt/form".to_owned();
        features.packages[0].classifiers[0].name = "Form".to_owned();
        features.packages[0].classifiers[0].features[0].name = "baseForm".to_owned();

        let coverage = canonical_coverage_fixture();
        assert!(coverage.validate_against(&features).is_ok());

        let mut missing = coverage.clone();
        missing.entries.clear();
        missing.summary.entries = 0;
        missing.summary.typed = 0;
        assert!(matches!(
            missing.validate_against(&features),
            Err(SchemaError::CoverageMismatch {
                kind: "unmapped",
                ..
            })
        ));

        let mut stale = coverage;
        let mut stale_entry = stale.entries[0].clone();
        stale_entry.key.feature = "removedFeature".to_owned();
        stale.entries.push(stale_entry);
        stale.summary.entries = 2;
        stale.summary.typed = 2;
        assert!(matches!(
            stale.validate_against(&features),
            Err(SchemaError::CoverageMismatch { kind: "stale", .. })
        ));
    }
}
