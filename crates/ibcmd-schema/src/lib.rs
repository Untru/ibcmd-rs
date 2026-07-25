//! Standalone, versioned schema knowledge derived from public XML behaviour and
//! locally inspected EDT model/export metadata.
//!
//! This crate embeds declarative data only. It neither links to nor starts EDT,
//! Java, OSGi, platform executables, or native libraries.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::marker::PhantomData;
use std::sync::OnceLock;

use serde::de::{Error as DeError, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Embedded EDT-derived model inventory.
pub const BUNDLED_MODEL_INVENTORY_JSON: &str =
    include_str!("../data/edt-2025.2.3-model-inventory.json");

/// Embedded EDT EPackage classifier and feature identifiers.
pub const BUNDLED_PACKAGE_FEATURES_JSON: &str =
    include_str!("../data/edt-2025.2.3-package-features.json");

/// Embedded EDT Xcore-derived feature semantics for every packaged model resource.
pub const BUNDLED_FEATURE_SEMANTICS_JSON: &str =
    include_str!("../data/edt-2025.2.3-feature-semantics.json");

/// Runtime projection containing only the model fact required by the verified
/// Form `ListSettings` tail policy.
///
/// Keeping this projection separate prevents the complete EDT research corpus
/// from becoming product-binary payload. Schema tests prove that the projection
/// is structurally identical to the corresponding parsed feature.
const BUNDLED_DCS_LIST_SETTINGS_FEATURE_SEMANTICS_JSON: &str =
    include_str!("../data/edt-2025.2.3-dcs-list-settings-feature-semantics.json");

/// Embedded exhaustive canonical-model implementation coverage.
pub const BUNDLED_CANONICAL_COVERAGE_JSON: &str =
    include_str!("../data/edt-2025.2.3-canonical-coverage.json");

/// Embedded, provider-derived metadata and produced-type feature order.
pub const BUNDLED_METADATA_ORDER_JSON: &str =
    include_str!("../data/edt-2025.2.3-metadata-order.json");

/// Embedded, verified writer behaviour rules.
pub const BUNDLED_WRITER_RULES_JSON: &str = include_str!("../data/edt-2025.2.3-writer-rules.json");

/// Embedded, exact EDT writer evidence for the bounded DCS settings tail.
pub const BUNDLED_DCS_WRITER_EVIDENCE_JSON: &str =
    include_str!("../data/edt-2025.2.3-dcs-writer-evidence.json");

/// Embedded, exact EDT and live native-export evidence for the bounded
/// `InputFieldExtInfo.choiceParameters` writer.
pub const BUNDLED_FORM_CHOICE_PARAMETERS_WRITER_EVIDENCE_JSON: &str =
    include_str!("../data/edt-2025.2.3-form-choice-parameters-writer-evidence.json");
const BUNDLED_FORM_CHOICE_PARAMETERS_LIVE_FIXTURE_JSON: &str =
    include_str!("../../../tests/fixtures/form_choice_parameters_slot27_live.json");

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

const MAX_DCS_WRITER_EVIDENCE_JSON_BYTES: usize = 32 * 1024;
const MAX_DCS_WRITER_EVIDENCE_TEXT_BYTES: usize = 4 * 1024;
const MAX_DCS_WRITER_EVIDENCE_FACTS: usize = 16;
const MAX_DCS_WRITER_EVIDENCE_MISSING_KEYS: usize = 8;
const MAX_DCS_WRITER_EVIDENCE_SOURCES: usize = 8;
const DCS_SETTINGS_MODEL_NAMESPACE: &str = "http://g5.1c.ru/v8/dt/data-composition-system/settings";
const DCS_SETTINGS_CLASSIFIER: &str = "DataCompositionSettings";

/// Verified field identity for the only schema-driven Form `ListSettings` tail.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DcsListSettingsTailField {
    ItemsViewMode,
    ItemsUserSettingId,
}

/// Exact verified policy for the two final Form `ListSettings` children.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DcsListSettingsTailPolicy {
    namespace_uri: String,
    tail_order: [DcsListSettingsTailField; 2],
    items_view_mode_qname: String,
    items_view_mode_default: String,
    items_user_setting_id_qname: String,
    items_user_setting_id_default: String,
}

impl DcsListSettingsTailPolicy {
    pub fn namespace_uri(&self) -> &str {
        &self.namespace_uri
    }

    pub const fn tail_order(&self) -> &[DcsListSettingsTailField; 2] {
        &self.tail_order
    }

    pub fn items_view_mode_qname(&self) -> &str {
        &self.items_view_mode_qname
    }

    pub fn items_view_mode_default(&self) -> &str {
        &self.items_view_mode_default
    }

    pub fn items_user_setting_id_qname(&self) -> &str {
        &self.items_user_setting_id_qname
    }

    pub fn items_user_setting_id_default(&self) -> &str {
        &self.items_user_setting_id_default
    }
}

/// Strict, bounded representation of the committed EDT DCS writer evidence.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DcsWriterEvidenceCorpus {
    schema_version: u32,
    source: DcsWriterEvidenceSource,
    verified_facts: Vec<DcsWriterEvidenceFact>,
    missing_keys: Vec<DcsWriterEvidenceMissingKey>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsWriterEvidenceSource {
    product: String,
    release: String,
    derivation: String,
    input_contract: String,
    invocation: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsWriterEvidenceFact {
    key: String,
    value: DcsWriterEvidenceValue,
    evidence: DcsWriterEvidenceProof,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(untagged)]
enum DcsWriterEvidenceValue {
    Text(String),
    TailOrder(Vec<String>),
    EnumNotDefault(DcsEnumNotDefaultEvidence),
    StringNotDefault(DcsStringNotDefaultEvidence),
    DefaultValue(DcsDefaultValueEvidence),
    FormDelegate(DcsFormDelegateEvidence),
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsEnumNotDefaultEvidence {
    qname: String,
    default_model_constant: String,
    writer: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsStringNotDefaultEvidence {
    qname: String,
    default_string: String,
    writer: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsDefaultValueEvidence {
    predicate: String,
    operations: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsFormDelegateEvidence {
    delegate: String,
    qname_source: String,
    null_branch: DcsNullBranchEvidence,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsNullBranchEvidence {
    from_offset: u32,
    target_offset: u32,
    target_opcode: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsWriterEvidenceProof {
    kind: String,
    status: EvidenceStatus,
    sources: Vec<String>,
    note: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DcsWriterEvidenceMissingKey {
    key: String,
    status: String,
    reason: String,
}

/// A structured subset of verified writer behaviour.  Free-form operations remain useful
/// provenance, but production writers must consume this typed policy instead of parsing prose.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum WriterPolicy {
    FormChoiceList {
        #[serde(rename = "itemOrder")]
        item_order: Vec<FormChoiceListItemPart>,
        #[serde(rename = "emptyCollection")]
        empty_collection: FormChoiceListEmptyCollection,
        #[serde(rename = "emptyStringValue")]
        empty_string_value: FormChoiceListEmptyStringValue,
    },
    FormListSettings {
        #[serde(rename = "nullValue")]
        null_value: FormListSettingsNullValue,
        delegate: String,
    },
    FormChoiceParameters {
        #[serde(rename = "ownerQName")]
        owner_qname: String,
        #[serde(rename = "ownerPredecessorQName")]
        owner_predecessor_qname: String,
        #[serde(rename = "ownerSuccessorQName")]
        owner_successor_qname: String,
        #[serde(rename = "emptyCollection")]
        empty_collection: FormChoiceParametersEmptyCollection,
        item: FormChoiceParameterItemPolicy,
        #[serde(rename = "fixedArray")]
        fixed_array: FormChoiceParameterFixedArrayPolicy,
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
pub enum FormChoiceListEmptyStringValue {
    SelfClosing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FormListSettingsNullValue {
    Omit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FormChoiceParametersEmptyCollection {
    OmitWhenWriteDefaultFalse,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FormChoiceParameterValuePart {
    Presentation,
    Value,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FormChoiceParameterItemPolicy {
    #[serde(rename = "itemQName")]
    pub item_qname: String,
    #[serde(rename = "nameAttributeQName")]
    pub name_attribute_qname: String,
    #[serde(rename = "valueQName")]
    pub value_qname: String,
    pub value_xsi_type: String,
    pub value_order: Vec<FormChoiceParameterValuePart>,
    #[serde(rename = "presentationQName")]
    pub presentation_qname: String,
    #[serde(rename = "scalarValueQName")]
    pub scalar_value_qname: String,
    pub boolean_xsi_type: String,
    pub design_time_ref_xsi_type: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FormChoiceParameterFixedArrayPolicy {
    pub xsi_type: String,
    #[serde(rename = "itemQName")]
    pub item_qname: String,
    pub item_xsi_type: String,
    pub item_order: Vec<FormChoiceParameterValuePart>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FormChoiceParametersWriterEvidence {
    schema_version: u32,
    source: FormChoiceParametersEvidenceSource,
    scope: FormChoiceParametersEvidenceScope,
    verified_facts: FormChoiceParametersVerifiedFacts,
    missing_keys: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FormChoiceParametersEvidenceSource {
    product: String,
    release: String,
    root_identity: FormChoiceParametersEvidenceRootIdentity,
    validated_bundles: Vec<FormChoiceParametersEvidenceBundle>,
    derivation: String,
    invocation: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FormChoiceParametersEvidenceRootIdentity {
    leaf: String,
    product_version: String,
    build_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FormChoiceParametersEvidenceBundle {
    symbolic_name: String,
    version: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FormChoiceParametersEvidenceScope {
    disposition: String,
    production_emission: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FormChoiceParametersVerifiedFacts {
    model: FormChoiceParametersModelFact,
    owner_order: FormChoiceParametersOwnerOrderFact,
    writer: FormChoiceParametersWriterFact,
    live_slot27: FormChoiceParametersLiveFact,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FormChoiceParametersModelFact {
    model_type: String,
    feature: String,
    lower_bound: u32,
    upper_bound: i32,
    #[serde(rename = "ownerQName")]
    owner_qname: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FormChoiceParametersOwnerOrderFact {
    #[serde(rename = "predecessorQName")]
    predecessor_qname: String,
    #[serde(rename = "featureQName")]
    feature_qname: String,
    #[serde(rename = "successorQName")]
    successor_qname: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FormChoiceParametersWriterFact {
    delegate: String,
    empty_collection: FormChoiceParametersEmptyCollection,
    item: FormChoiceParameterItemPolicy,
    fixed_array: FormChoiceParameterFixedArrayPolicy,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FormChoiceParametersLiveFact {
    fixture: String,
    fixture_sha256: String,
    raw_row: String,
    raw_source: String,
    raw_source_sha256: String,
    raw_slot: usize,
    native_source: String,
    native_source_sha256: String,
    item_names_in_order: Vec<String>,
    value_kinds_in_order: Vec<String>,
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
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
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
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
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
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CoverageStatus {
    Typed,
    OpaqueLossless,
    Unsupported,
    PlatformOnly,
}

/// Canonical implementation family used for deterministic coverage reporting.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CanonicalCoverageFamily {
    Metadata,
    Forms,
    Dcs,
    Mxl,
    Common,
    Other,
}

/// One explicit EDT feature to canonical-model mapping.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CanonicalCoverageEntry {
    pub key: FeatureSemanticKey,
    pub family: CanonicalCoverageFamily,
    pub status: CoverageStatus,
    pub canonical_type: Option<String>,
    pub canonical_field: Option<String>,
    pub opaque_placement: Option<String>,
    pub diagnostic_code: Option<String>,
    pub evidence: FeatureEvidence,
}

/// Derived coverage totals for completeness and reporting.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CanonicalCoverageSummary {
    pub entries: usize,
    pub typed: usize,
    pub opaque_lossless: usize,
    pub unsupported: usize,
    pub platform_only: usize,
}

/// Status totals for one canonical implementation family.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CanonicalCoverageFamilyAggregate {
    pub family: CanonicalCoverageFamily,
    pub entries: usize,
    pub typed: usize,
    pub opaque_lossless: usize,
    pub unsupported: usize,
    pub platform_only: usize,
}

/// Reusable migration work grouped without object, feature, UUID, or file names.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CanonicalMigrationBacklogEntry {
    pub rule: String,
    pub family: CanonicalCoverageFamily,
    pub package: String,
    pub classifier_kind: FeatureClassifierKind,
    pub feature_kind: FeatureKind,
    pub features: usize,
}

/// Complete coverage mapping for one EDT-derived feature corpus.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CanonicalCoverageCorpus {
    pub schema_version: u32,
    pub source: CorpusSource,
    pub summary: CanonicalCoverageSummary,
    pub family_aggregates: Vec<CanonicalCoverageFamilyAggregate>,
    pub migration_backlog: Vec<CanonicalMigrationBacklogEntry>,
    pub entries: Vec<CanonicalCoverageEntry>,
}

const MAX_CANONICAL_COVERAGE_JSON_BYTES: usize = 4 * 1024 * 1024;
const MAX_CANONICAL_COVERAGE_STRING_BYTES: usize = 4 * 1024;
const MAX_CANONICAL_COVERAGE_ENTRIES: usize = 5_000;
const MAX_CANONICAL_COVERAGE_FAMILY_AGGREGATES: usize = 6;
const MAX_CANONICAL_COVERAGE_BACKLOG_ENTRIES: usize = 256;
const MAX_CANONICAL_COVERAGE_EVIDENCE_SOURCES: usize = 16;

struct BoundedText<const MAX: usize>;

impl<'de, const MAX: usize> Deserialize<'de> for BoundedText<MAX> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct TextVisitor<const MAX: usize>;

        impl<const MAX: usize> Visitor<'_> for TextVisitor<MAX> {
            type Value = BoundedText<MAX>;

            fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
                write!(formatter, "a string of at most {MAX} UTF-8 bytes")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                if value.len() > MAX {
                    return Err(E::custom(format!(
                        "canonical coverage string exceeds {MAX} UTF-8 bytes"
                    )));
                }
                Ok(BoundedText)
            }

            fn visit_borrowed_str<E>(self, value: &'_ str) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                self.visit_str(value)
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                self.visit_str(&value)
            }
        }

        deserializer.deserialize_string(TextVisitor::<MAX>)
    }
}

struct BoundedVec<T, const MAX: usize>(PhantomData<T>);

impl<T, const MAX: usize> Default for BoundedVec<T, MAX> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<'de, T, const MAX: usize> Deserialize<'de> for BoundedVec<T, MAX>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct VecVisitor<T, const MAX: usize>(PhantomData<T>);

        impl<'de, T, const MAX: usize> Visitor<'de> for VecVisitor<T, MAX>
        where
            T: Deserialize<'de>,
        {
            type Value = BoundedVec<T, MAX>;

            fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
                write!(formatter, "an array of at most {MAX} elements")
            }

            fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                if sequence.size_hint().is_some_and(|size| size > MAX) {
                    return Err(A::Error::custom(format!(
                        "canonical coverage array exceeds {MAX} elements"
                    )));
                }
                let mut count = 0usize;
                while sequence.next_element::<T>()?.is_some() {
                    count += 1;
                    if count > MAX {
                        return Err(A::Error::custom(format!(
                            "canonical coverage array exceeds {MAX} elements"
                        )));
                    }
                }
                Ok(BoundedVec(PhantomData))
            }
        }

        deserializer.deserialize_seq(VecVisitor::<T, MAX>(PhantomData))
    }
}

#[allow(dead_code)]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CanonicalCoveragePreflight {
    schema_version: u32,
    source: CoverageSourcePreflight,
    summary: CoverageSummaryPreflight,
    family_aggregates:
        BoundedVec<CoverageFamilyAggregatePreflight, MAX_CANONICAL_COVERAGE_FAMILY_AGGREGATES>,
    migration_backlog: BoundedVec<CoverageBacklogPreflight, MAX_CANONICAL_COVERAGE_BACKLOG_ENTRIES>,
    entries: BoundedVec<CoverageEntryPreflight, MAX_CANONICAL_COVERAGE_ENTRIES>,
}

#[allow(dead_code)]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CoverageSourcePreflight {
    product: BoundedText<MAX_CANONICAL_COVERAGE_STRING_BYTES>,
    release: BoundedText<MAX_CANONICAL_COVERAGE_STRING_BYTES>,
    derivation: BoundedText<MAX_CANONICAL_COVERAGE_STRING_BYTES>,
}

#[allow(dead_code)]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CoverageSummaryPreflight {
    entries: usize,
    typed: usize,
    opaque_lossless: usize,
    unsupported: usize,
    platform_only: usize,
}

#[allow(dead_code)]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CoverageFamilyAggregatePreflight {
    family: CanonicalCoverageFamily,
    entries: usize,
    typed: usize,
    opaque_lossless: usize,
    unsupported: usize,
    platform_only: usize,
}

#[allow(dead_code)]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CoverageBacklogPreflight {
    rule: BoundedText<MAX_CANONICAL_COVERAGE_STRING_BYTES>,
    family: CanonicalCoverageFamily,
    package: BoundedText<MAX_CANONICAL_COVERAGE_STRING_BYTES>,
    classifier_kind: FeatureClassifierKind,
    feature_kind: FeatureKind,
    features: usize,
}

#[allow(dead_code)]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CoverageEntryPreflight {
    key: CoverageKeyPreflight,
    family: CanonicalCoverageFamily,
    status: CoverageStatus,
    canonical_type: Option<BoundedText<MAX_CANONICAL_COVERAGE_STRING_BYTES>>,
    canonical_field: Option<BoundedText<MAX_CANONICAL_COVERAGE_STRING_BYTES>>,
    opaque_placement: Option<BoundedText<MAX_CANONICAL_COVERAGE_STRING_BYTES>>,
    diagnostic_code: Option<BoundedText<MAX_CANONICAL_COVERAGE_STRING_BYTES>>,
    evidence: CoverageEvidencePreflight,
}

#[allow(dead_code)]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CoverageKeyPreflight {
    namespace_uri: BoundedText<MAX_CANONICAL_COVERAGE_STRING_BYTES>,
    classifier: BoundedText<MAX_CANONICAL_COVERAGE_STRING_BYTES>,
    feature: BoundedText<MAX_CANONICAL_COVERAGE_STRING_BYTES>,
}

#[allow(dead_code)]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CoverageEvidencePreflight {
    status: EvidenceStatus,
    kind: BoundedText<MAX_CANONICAL_COVERAGE_STRING_BYTES>,
    #[serde(default)]
    sources: BoundedVec<
        BoundedText<MAX_CANONICAL_COVERAGE_STRING_BYTES>,
        MAX_CANONICAL_COVERAGE_EVIDENCE_SOURCES,
    >,
    note: Option<BoundedText<MAX_CANONICAL_COVERAGE_STRING_BYTES>>,
}

fn preflight_canonical_coverage_json(json: &str) -> Result<(), SchemaError> {
    enforce_canonical_coverage_json_size(json)?;
    serde_json::from_str::<CanonicalCoveragePreflight>(json)
        .map(|_| ())
        .map_err(|error| SchemaError::InvalidJson(error.to_string()))
}

fn enforce_canonical_coverage_json_size(json: &str) -> Result<(), SchemaError> {
    if json.len() > MAX_CANONICAL_COVERAGE_JSON_BYTES {
        return Err(SchemaError::InvalidJson(format!(
            "canonical coverage JSON exceeds {MAX_CANONICAL_COVERAGE_JSON_BYTES} UTF-8 bytes"
        )));
    }
    Ok(())
}

const CANONICAL_COVERAGE_FAMILIES: [CanonicalCoverageFamily; 6] = [
    CanonicalCoverageFamily::Metadata,
    CanonicalCoverageFamily::Forms,
    CanonicalCoverageFamily::Dcs,
    CanonicalCoverageFamily::Mxl,
    CanonicalCoverageFamily::Common,
    CanonicalCoverageFamily::Other,
];

fn canonical_coverage_family(
    package: &str,
    classifier_kind: FeatureClassifierKind,
) -> Option<CanonicalCoverageFamily> {
    use CanonicalCoverageFamily::{Dcs, Forms, Other};
    use FeatureClassifierKind::{Class, Interface};

    let routed = match package {
        "com._1c.g5.v8.dt.form.layout.model.calculation.context"
        | "com._1c.g5.v8.dt.form.layout.model.description"
        | "com._1c.g5.v8.dt.form.layout.model.generation.context"
        | "com._1c.g5.v8.dt.form.layout.model.transformation.context"
        | "com._1c.g5.v8.dt.form.mapping.model"
        | "com._1c.g5.v8.dt.form.model"
            if matches!(classifier_kind, Class | Interface) =>
        {
            Forms
        }
        "com._1c.g5.v8.dt.dcs.expressions.model"
        | "com._1c.g5.v8.dt.dcs.model.appearancetemplate"
        | "com._1c.g5.v8.dt.dcs.model.areaTemplate"
        | "com._1c.g5.v8.dt.dcs.model.common"
        | "com._1c.g5.v8.dt.dcs.model.core"
        | "com._1c.g5.v8.dt.dcs.model.dbcopies"
        | "com._1c.g5.v8.dt.dcs.model.schema"
        | "com._1c.g5.v8.dt.dcs.model.settings"
        | "com._1c.g5.v8.dt.ql.dcs.model"
            if classifier_kind == Class =>
        {
            Dcs
        }
        "com._1c.g5.v8.dt.debug.model.core" if classifier_kind == Class => Other,
        "com._1c.g5.v8.dt.mcore"
        | "com._1c.g5.v8.dt.scc.model"
        | "com._1c.g5.v8.dt.supply.settings.model"
            if matches!(classifier_kind, Class | Interface) =>
        {
            Other
        }
        "com._1c.g5.v8.dt.aggregates.model"
        | "com._1c.g5.v8.dt.bp.scheme.model"
        | "com._1c.g5.v8.dt.bsl.model"
        | "com._1c.g5.v8.dt.cai.model"
        | "com._1c.g5.v8.dt.chart.model"
        | "com._1c.g5.v8.dt.chart.model.timescale"
        | "com._1c.g5.v8.dt.cmi.model"
        | "com._1c.g5.v8.dt.cmi.model.deriveddata"
        | "com._1c.g5.v8.dt.compare.model"
        | "com._1c.g5.v8.dt.debug.model.area"
        | "com._1c.g5.v8.dt.debug.model.attach"
        | "com._1c.g5.v8.dt.debug.model.base.data"
        | "com._1c.g5.v8.dt.debug.model.breakpoints"
        | "com._1c.g5.v8.dt.debug.model.bsl.exceptions"
        | "com._1c.g5.v8.dt.debug.model.calculations"
        | "com._1c.g5.v8.dt.debug.model.dbgui.commands"
        | "com._1c.g5.v8.dt.debug.model.foreground.data"
        | "com._1c.g5.v8.dt.debug.model.measure"
        | "com._1c.g5.v8.dt.debug.model.rdbg.request.response"
        | "com._1c.g5.v8.dt.debug.model.rte.filter"
        | "com._1c.g5.v8.dt.debug.model.rte.info"
        | "com._1c.g5.v8.dt.debug.model.virtual"
        | "com._1c.g5.v8.dt.dendrogram.model"
        | "com._1c.g5.v8.dt.ganttchart.model"
        | "com._1c.g5.v8.dt.geographicalschema.model"
        | "com._1c.g5.v8.dt.hpwa.model"
        | "com._1c.g5.v8.dt.lcore.model"
        | "com._1c.g5.v8.dt.planner.model"
        | "com._1c.g5.v8.dt.platform.model"
        | "com._1c.g5.v8.dt.platform.services.model"
        | "com._1c.g5.v8.dt.ql.model"
        | "com._1c.g5.v8.dt.right.ql.model"
        | "com._1c.g5.v8.dt.right.templates.model"
        | "com._1c.g5.v8.dt.rights.model"
        | "com._1c.g5.v8.dt.schedule.model"
        | "com._1c.g5.v8.dt.style.model"
        | "com._1c.g5.v8.dt.v8help.model"
        | "com._1c.g5.v8.dt.ws.wsdefinitions.model"
        | "com._1c.g5.v8.dt.xdto.model"
        | "com._1c.g5.v8.dt.xdto.type.model"
            if classifier_kind == Class =>
        {
            Other
        }
        _ => return None,
    };
    Some(routed)
}

fn recompute_family_aggregates(
    entries: &[CanonicalCoverageEntry],
) -> Vec<CanonicalCoverageFamilyAggregate> {
    let mut counts = BTreeMap::<CanonicalCoverageFamily, CanonicalCoverageFamilyAggregate>::new();
    for family in CANONICAL_COVERAGE_FAMILIES {
        counts.insert(
            family,
            CanonicalCoverageFamilyAggregate {
                family,
                entries: 0,
                typed: 0,
                opaque_lossless: 0,
                unsupported: 0,
                platform_only: 0,
            },
        );
    }
    for entry in entries {
        let aggregate = counts
            .get_mut(&entry.family)
            .expect("all canonical coverage families are initialized");
        aggregate.entries += 1;
        match entry.status {
            CoverageStatus::Typed => aggregate.typed += 1,
            CoverageStatus::OpaqueLossless => aggregate.opaque_lossless += 1,
            CoverageStatus::Unsupported => aggregate.unsupported += 1,
            CoverageStatus::PlatformOnly => aggregate.platform_only += 1,
        }
    }
    counts.into_values().collect()
}

fn recompute_migration_backlog(
    coverage: &CanonicalCoverageCorpus,
    features: &FeatureSemanticsCorpus,
) -> Result<Vec<CanonicalMigrationBacklogEntry>, SchemaError> {
    let coverage_by_key = coverage
        .entries
        .iter()
        .map(|entry| (entry.key.clone(), entry))
        .collect::<BTreeMap<_, _>>();
    let mut groups = BTreeMap::<
        (
            CanonicalCoverageFamily,
            String,
            FeatureClassifierKind,
            FeatureKind,
        ),
        usize,
    >::new();

    for package in &features.packages {
        for classifier in &package.classifiers {
            if classifier.features.is_empty() {
                continue;
            }
            let family = canonical_coverage_family(&package.package_name, classifier.kind)
                .ok_or_else(|| SchemaError::UnknownCoverageRoute {
                    package: package.package_name.clone(),
                    classifier_kind: classifier.kind,
                })?;
            for feature in &classifier.features {
                let key = FeatureSemanticKey {
                    namespace_uri: package.namespace_uri.clone(),
                    classifier: classifier.name.clone(),
                    feature: feature.name.clone(),
                };
                let entry = coverage_by_key
                    .get(&key)
                    .expect("exact full join is checked before backlog recomputation");
                if entry.family != family {
                    return Err(SchemaError::InvalidCoverageEntry {
                        key,
                        reason: "coverage family does not match canonical package/classifier route",
                    });
                }
                if entry.status == CoverageStatus::Unsupported
                    && entry.diagnostic_code.as_deref() == Some("schema.unmapped")
                {
                    *groups
                        .entry((
                            family,
                            package.package_name.clone(),
                            classifier.kind,
                            feature.kind,
                        ))
                        .or_default() += 1;
                }
            }
        }
    }

    Ok(groups
        .into_iter()
        .map(
            |((family, package, classifier_kind, feature_kind), features)| {
                CanonicalMigrationBacklogEntry {
                    rule: "unsupported/schema.unmapped".to_owned(),
                    family,
                    package,
                    classifier_kind,
                    feature_kind,
                    features,
                }
            },
        )
        .collect())
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
    UnknownCoverageRoute {
        package: String,
        classifier_kind: FeatureClassifierKind,
    },
    CoverageDerivedDataMismatch(&'static str),
    InvalidDcsWriterEvidence(String),
    InvalidFormChoiceParametersWriterEvidence(String),
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
            Self::UnknownCoverageRoute {
                package,
                classifier_kind,
            } => write!(
                formatter,
                "canonical coverage has no route for package `{package}` / classifier kind `{classifier_kind:?}`"
            ),
            Self::CoverageDerivedDataMismatch(field) => {
                write!(
                    formatter,
                    "canonical coverage {field} does not match recomputation"
                )
            }
            Self::InvalidDcsWriterEvidence(reason) => {
                write!(formatter, "invalid DCS writer evidence: {reason}")
            }
            Self::InvalidFormChoiceParametersWriterEvidence(reason) => {
                write!(
                    formatter,
                    "invalid Form choice-parameters writer evidence: {reason}"
                )
            }
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

impl DcsWriterEvidenceCorpus {
    pub fn parse(json: &str) -> Result<Self, SchemaError> {
        if json.len() > MAX_DCS_WRITER_EVIDENCE_JSON_BYTES {
            return Err(SchemaError::InvalidDcsWriterEvidence(format!(
                "JSON exceeds {MAX_DCS_WRITER_EVIDENCE_JSON_BYTES} UTF-8 bytes"
            )));
        }
        let corpus: Self = serde_json::from_str(json)
            .map_err(|error| SchemaError::InvalidJson(error.to_string()))?;
        corpus.validate()?;
        Ok(corpus)
    }

    pub fn validate(&self) -> Result<(), SchemaError> {
        if self.schema_version != 1 {
            return Err(SchemaError::UnsupportedSchemaVersion(self.schema_version));
        }
        if self.source.product != "1C:EDT" || self.source.release != "2025.2.3+30" {
            return Err(invalid_dcs_writer_evidence(
                "source product or release does not match the verified evidence",
            ));
        }
        for (field, value) in [
            ("source product", self.source.product.as_str()),
            ("source release", self.source.release.as_str()),
            ("source derivation", self.source.derivation.as_str()),
            ("source input contract", self.source.input_contract.as_str()),
            ("source invocation", self.source.invocation.as_str()),
        ] {
            validate_dcs_writer_evidence_text(field, value)?;
        }
        if self.verified_facts.len() > MAX_DCS_WRITER_EVIDENCE_FACTS {
            return Err(invalid_dcs_writer_evidence(format!(
                "verified facts exceed {MAX_DCS_WRITER_EVIDENCE_FACTS}"
            )));
        }
        if self.missing_keys.len() > MAX_DCS_WRITER_EVIDENCE_MISSING_KEYS {
            return Err(invalid_dcs_writer_evidence(format!(
                "missing keys exceed {MAX_DCS_WRITER_EVIDENCE_MISSING_KEYS}"
            )));
        }

        let expected_fact_keys = BTreeSet::from([
            "dcs.DataCompositionSettings.namespace",
            "dcs.DataCompositionSettings.verified-tail-order",
            "dcs.DataCompositionSettings.itemsViewMode",
            "dcs.DataCompositionSettings.itemsUserSettingID",
            "dcs.DataCompositionSettings.default-value",
            "form.DynamicListExtInfo.listSettings.delegate",
        ]);
        let mut fact_keys = BTreeSet::new();
        for fact in &self.verified_facts {
            validate_dcs_writer_evidence_text("verified fact key", &fact.key)?;
            if !fact_keys.insert(fact.key.as_str()) {
                return Err(invalid_dcs_writer_evidence(format!(
                    "duplicate verified fact `{}`",
                    fact.key
                )));
            }
            if fact.evidence.status != EvidenceStatus::Verified
                || fact.evidence.kind != "javap-v-exact-method-control-flow-constant-pool"
            {
                return Err(invalid_dcs_writer_evidence(format!(
                    "fact `{}` is not backed by the exact verified extractor",
                    fact.key
                )));
            }
            if fact.evidence.sources.is_empty()
                || fact.evidence.sources.len() > MAX_DCS_WRITER_EVIDENCE_SOURCES
            {
                return Err(invalid_dcs_writer_evidence(format!(
                    "fact `{}` has an invalid evidence source count",
                    fact.key
                )));
            }
            validate_dcs_writer_evidence_text("evidence kind", &fact.evidence.kind)?;
            validate_dcs_writer_evidence_text("evidence note", &fact.evidence.note)?;
            for source in &fact.evidence.sources {
                validate_dcs_writer_evidence_text("evidence source", source)?;
            }
        }
        if fact_keys != expected_fact_keys {
            return Err(invalid_dcs_writer_evidence(
                "verified fact keys differ from the exact supported evidence set",
            ));
        }

        let expected_missing_keys = BTreeSet::from([
            "dcs.settings.document.qname",
            "form.DynamicListExtInfo.listSettings.qname",
            "dcs.DataCompositionSettings.type-id",
            "dcs.DataCompositionSettings.opaque-extension.placement",
        ]);
        let mut missing_keys = BTreeSet::new();
        for missing in &self.missing_keys {
            for (field, value) in [
                ("missing key", missing.key.as_str()),
                ("missing key status", missing.status.as_str()),
                ("missing key reason", missing.reason.as_str()),
            ] {
                validate_dcs_writer_evidence_text(field, value)?;
            }
            if missing.status != "not-proven-by-this-extractor"
                || !missing_keys.insert(missing.key.as_str())
            {
                return Err(invalid_dcs_writer_evidence(
                    "missing evidence keys are duplicate or have an unexpected status",
                ));
            }
        }
        if missing_keys != expected_missing_keys {
            return Err(invalid_dcs_writer_evidence(
                "missing evidence keys differ from the exact four blocked facts",
            ));
        }

        self.verified_form_list_settings_tail_evidence().map(|_| ())
    }

    pub fn form_list_settings_tail_policy(
        &self,
        feature_semantics: &FeatureSemanticsCorpus,
    ) -> Result<DcsListSettingsTailPolicy, SchemaError> {
        feature_semantics.validate()?;
        if feature_semantics.source.release != self.source.release {
            return Err(invalid_dcs_writer_evidence(
                "writer evidence and feature semantics releases differ",
            ));
        }
        let view_feature = feature_semantics
            .feature(&FeatureSemanticKey {
                namespace_uri: DCS_SETTINGS_MODEL_NAMESPACE.to_owned(),
                classifier: DCS_SETTINGS_CLASSIFIER.to_owned(),
                feature: "itemsViewMode".to_owned(),
            })
            .ok_or_else(|| {
                invalid_dcs_writer_evidence(
                    "verified itemsViewMode feature semantics are unavailable",
                )
            })?;
        if view_feature.model_evidence.status != EvidenceStatus::Verified {
            return Err(invalid_dcs_writer_evidence(
                "itemsViewMode model default is not verified",
            ));
        }

        let (namespace, view, user_id) = self.verified_form_list_settings_tail_evidence()?;
        let model_default = view_feature.default_value.as_deref().ok_or_else(|| {
            invalid_dcs_writer_evidence("verified itemsViewMode model default is absent")
        })?;
        if (view.default_model_constant.as_str(), model_default) != ("QUICK_ACCESS", "QuickAccess")
        {
            return Err(invalid_dcs_writer_evidence(format!(
                "itemsViewMode exact default join requires writer `QUICK_ACCESS` and model `QuickAccess`, got writer `{}` and model `{model_default}`",
                view.default_model_constant
            )));
        }

        Ok(DcsListSettingsTailPolicy {
            namespace_uri: namespace.to_owned(),
            tail_order: [
                DcsListSettingsTailField::ItemsViewMode,
                DcsListSettingsTailField::ItemsUserSettingId,
            ],
            items_view_mode_qname: view.qname.clone(),
            items_view_mode_default: model_default.to_owned(),
            items_user_setting_id_qname: user_id.qname.clone(),
            items_user_setting_id_default: user_id.default_string.clone(),
        })
    }

    fn verified_form_list_settings_tail_evidence(
        &self,
    ) -> Result<
        (
            &str,
            &DcsEnumNotDefaultEvidence,
            &DcsStringNotDefaultEvidence,
        ),
        SchemaError,
    > {
        let namespace = match self.fact_value("dcs.DataCompositionSettings.namespace")? {
            DcsWriterEvidenceValue::Text(value)
                if value == "http://v8.1c.ru/8.1/data-composition-system/settings" =>
            {
                value.as_str()
            }
            _ => {
                return Err(invalid_dcs_writer_evidence(
                    "DataCompositionSettings namespace drifted",
                ));
            }
        };
        match self.fact_value("dcs.DataCompositionSettings.verified-tail-order")? {
            DcsWriterEvidenceValue::TailOrder(order)
                if order == &["itemsViewMode", "itemsUserSettingID"] => {}
            _ => {
                return Err(invalid_dcs_writer_evidence(
                    "verified settings tail order drifted",
                ));
            }
        }
        let view = match self.fact_value("dcs.DataCompositionSettings.itemsViewMode")? {
            DcsWriterEvidenceValue::EnumNotDefault(value)
                if value.qname
                    == "{http://v8.1c.ru/8.1/data-composition-system/settings}itemsViewMode"
                    && value.default_model_constant == "QUICK_ACCESS"
                    && value.writer == "V8XmlSerializer.writeEnumNotDefault" =>
            {
                value
            }
            _ => {
                return Err(invalid_dcs_writer_evidence(
                    "itemsViewMode writer policy drifted",
                ));
            }
        };
        let user_id = match self.fact_value("dcs.DataCompositionSettings.itemsUserSettingID")? {
            DcsWriterEvidenceValue::StringNotDefault(value)
                if value.qname
                    == "{http://v8.1c.ru/8.1/data-composition-system/settings}itemsUserSettingID"
                    && value.default_string.is_empty()
                    && value.writer == "V8XmlSerializer.writeStringNotDefault" =>
            {
                value
            }
            _ => {
                return Err(invalid_dcs_writer_evidence(
                    "itemsUserSettingID writer policy drifted",
                ));
            }
        };
        match self.fact_value("dcs.DataCompositionSettings.default-value")? {
            DcsWriterEvidenceValue::DefaultValue(value)
                if value.predicate == "DcsDefaultValueUtil.isDefaultValue"
                    && value.operations
                        == [
                            "V8XmlSerializer.writeEmptyElement",
                            "DcsV8Serializer.writeSettingsNamespace",
                        ] => {}
            _ => {
                return Err(invalid_dcs_writer_evidence(
                    "settings default-value policy drifted",
                ));
            }
        }
        match self.fact_value("form.DynamicListExtInfo.listSettings.delegate")? {
            DcsWriterEvidenceValue::FormDelegate(value)
                if value.delegate == "DcsV8Serializer.writeSettings"
                    && value.qname_source == "IQNameProvider.getElementQName"
                    && value.null_branch.from_offset == 48
                    && value.null_branch.target_offset == 106
                    && value.null_branch.target_opcode == "return" => {}
            _ => {
                return Err(invalid_dcs_writer_evidence(
                    "Form ListSettings delegate or null omission policy drifted",
                ));
            }
        }
        Ok((namespace, view, user_id))
    }

    fn fact_value(&self, key: &str) -> Result<&DcsWriterEvidenceValue, SchemaError> {
        self.verified_facts
            .iter()
            .find(|fact| fact.key == key)
            .map(|fact| &fact.value)
            .ok_or_else(|| invalid_dcs_writer_evidence(format!("missing verified fact `{key}`")))
    }
}

fn invalid_dcs_writer_evidence(reason: impl Into<String>) -> SchemaError {
    SchemaError::InvalidDcsWriterEvidence(reason.into())
}

fn validate_dcs_writer_evidence_text(field: &'static str, value: &str) -> Result<(), SchemaError> {
    if value.is_empty() {
        return Err(invalid_dcs_writer_evidence(format!("{field} is empty")));
    }
    if value.len() > MAX_DCS_WRITER_EVIDENCE_TEXT_BYTES {
        return Err(invalid_dcs_writer_evidence(format!(
            "{field} exceeds {MAX_DCS_WRITER_EVIDENCE_TEXT_BYTES} UTF-8 bytes"
        )));
    }
    if value.chars().any(char::is_control) {
        return Err(invalid_dcs_writer_evidence(format!(
            "{field} contains a control character"
        )));
    }
    Ok(())
}

fn exact_form_choice_parameters_policy() -> WriterPolicy {
    WriterPolicy::FormChoiceParameters {
        owner_qname: "{http://v8.1c.ru/8.3/xcf/logform}ChoiceParameters".to_owned(),
        owner_predecessor_qname: "{http://v8.1c.ru/8.3/xcf/logform}ChoiceParameterLinks".to_owned(),
        owner_successor_qname: "{http://v8.1c.ru/8.3/xcf/logform}AvailableTypes".to_owned(),
        empty_collection: FormChoiceParametersEmptyCollection::OmitWhenWriteDefaultFalse,
        item: FormChoiceParameterItemPolicy {
            item_qname: "{http://v8.1c.ru/8.2/managed-application/core}item".to_owned(),
            name_attribute_qname: "{}name".to_owned(),
            value_qname: "{http://v8.1c.ru/8.2/managed-application/core}value".to_owned(),
            value_xsi_type: "FormChoiceListDesTimeValue".to_owned(),
            value_order: vec![
                FormChoiceParameterValuePart::Presentation,
                FormChoiceParameterValuePart::Value,
            ],
            presentation_qname: "{http://v8.1c.ru/8.3/xcf/logform}Presentation".to_owned(),
            scalar_value_qname: "{http://v8.1c.ru/8.3/xcf/logform}Value".to_owned(),
            boolean_xsi_type: "xs:boolean".to_owned(),
            design_time_ref_xsi_type: "xr:DesignTimeRef".to_owned(),
        },
        fixed_array: FormChoiceParameterFixedArrayPolicy {
            xsi_type: "v8:FixedArray".to_owned(),
            item_qname: "{http://v8.1c.ru/8.1/data/core}Value".to_owned(),
            item_xsi_type: "FormChoiceListDesTimeValue".to_owned(),
            item_order: vec![
                FormChoiceParameterValuePart::Presentation,
                FormChoiceParameterValuePart::Value,
            ],
        },
    }
}

impl FormChoiceParametersWriterEvidence {
    pub fn parse(json: &str) -> Result<Self, SchemaError> {
        let evidence: Self = serde_json::from_str(json)
            .map_err(|error| SchemaError::InvalidJson(error.to_string()))?;
        evidence.validate()?;
        Ok(evidence)
    }

    pub fn validate(&self) -> Result<(), SchemaError> {
        let invalid = |reason: &str| {
            SchemaError::InvalidFormChoiceParametersWriterEvidence(reason.to_owned())
        };
        if self.schema_version != 1
            || self.source.product != "1C:EDT"
            || self.source.release != "2025.2.3+30"
            || self.source.root_identity.leaf != "1c-edt-2025.2.3+30-x86_64"
            || self.source.root_identity.product_version != "2025.2.3"
            || self.source.root_identity.build_id != "2025.2.3.30"
        {
            return Err(invalid("exact release identity differs"));
        }
        let expected_bundles = [
            ("com._1c.g5.v8.dt.export.xml", "13.0.100.v202602241426"),
            ("com._1c.g5.v8.dt.form.export.xml", "10.1.0.v202602241426"),
            ("com._1c.g5.v8.dt.form.model", "14.0.0.v202602241426"),
            ("com._1c.g5.v8.dt.mcore", "8.6.0.v202602241426"),
        ];
        let actual_bundles = self
            .source
            .validated_bundles
            .iter()
            .map(|bundle| (bundle.symbolic_name.as_str(), bundle.version.as_str()))
            .collect::<Vec<_>>();
        if actual_bundles != expected_bundles {
            return Err(invalid("validated bundle set differs"));
        }
        if self.source.derivation.trim().is_empty()
            || self.source.invocation
                != "tools/report-edt-form-choice-parameters-writer-evidence.ps1"
            || self.scope.disposition != "production-emission-evidence"
            || !self.scope.production_emission
            || !self.missing_keys.is_empty()
        {
            return Err(invalid("production evidence envelope differs"));
        }
        let fixture_sha256 = format!(
            "{:x}",
            Sha256::digest(BUNDLED_FORM_CHOICE_PARAMETERS_LIVE_FIXTURE_JSON.as_bytes())
        );
        if self.verified_facts.live_slot27.fixture_sha256 != fixture_sha256 {
            return Err(invalid(
                "committed live fixture bytes do not match the bound SHA-256",
            ));
        }
        let facts = &self.verified_facts;
        if facts.model.model_type != "InputFieldExtInfo"
            || facts.model.feature != "choiceParameters"
            || facts.model.lower_bound != 0
            || facts.model.upper_bound != -1
            || facts.owner_order.feature_qname != facts.model.owner_qname
            || facts.writer.delegate != "com._1c.g5.v8.dt.export.xml.writer.ChoiceParameterWriter"
            || facts.live_slot27.fixture != "tests/fixtures/form_choice_parameters_slot27_live.json"
            || facts.live_slot27.fixture_sha256
                != "05e4ef14ae7e3de0b2cc7d1b46e042be6ec70df629c57355036c5c7e58148bf7"
            || facts.live_slot27.raw_row != "34accda9-6211-4bc3-be8d-e42a24260653.0"
            || facts.live_slot27.raw_source
                != "candidate_dump/Config_inflated/34accda9-6211-4bc3-be8d-e42a24260653.0__part0.txt"
            || facts.live_slot27.raw_source_sha256
                != "77a99cffaa0b5c81ccccafa3a5fa01dec56342b49d1cce2e56f97f28b62785b1"
            || facts.live_slot27.raw_slot != 27
            || facts.live_slot27.native_source
                != "DataProcessors/УправлениеПродажамиНаOzon/Forms/НастройкиИнтеграции/Ext/Form.xml"
            || facts.live_slot27.native_source_sha256
                != "30cf0689522d6b74408da77426a178df282361f36d3787c0cfaf456c85cb8b03"
            || facts.live_slot27.item_names_in_order
                != [
                    "Отбор.Статус",
                    "Отбор.ХозяйственнаяОперация",
                    "Отбор.ПометкаУдаления",
                ]
            || facts.live_slot27.value_kinds_in_order != ["U", "FixedArray", "B"]
        {
            return Err(invalid(
                "verified model, writer, or live slot-27 facts differ",
            ));
        }
        let expected = exact_form_choice_parameters_policy();
        let WriterPolicy::FormChoiceParameters {
            owner_qname,
            owner_predecessor_qname,
            owner_successor_qname,
            empty_collection,
            item,
            fixed_array,
        } = expected
        else {
            unreachable!()
        };
        if facts.model.owner_qname != owner_qname
            || facts.owner_order.predecessor_qname != owner_predecessor_qname
            || facts.owner_order.successor_qname != owner_successor_qname
            || facts.writer.empty_collection != empty_collection
            || facts.writer.item != item
            || facts.writer.fixed_array != fixed_array
        {
            return Err(invalid(
                "verified QName, hierarchy, order, or fixed-array facts differ",
            ));
        }
        Ok(())
    }

    fn policy(&self) -> WriterPolicy {
        WriterPolicy::FormChoiceParameters {
            owner_qname: self.verified_facts.model.owner_qname.clone(),
            owner_predecessor_qname: self.verified_facts.owner_order.predecessor_qname.clone(),
            owner_successor_qname: self.verified_facts.owner_order.successor_qname.clone(),
            empty_collection: self.verified_facts.writer.empty_collection,
            item: self.verified_facts.writer.item.clone(),
            fixed_array: self.verified_facts.writer.fixed_array.clone(),
        }
    }
}

pub fn bind_form_choice_parameters_writer_evidence(
    json: &str,
    corpus: &WriterRuleCorpus,
) -> Result<(), SchemaError> {
    let evidence = FormChoiceParametersWriterEvidence::parse(json)?;
    let rule = corpus
        .rules
        .iter()
        .find(|rule| rule.model_type == "InputFieldExtInfo" && rule.feature == "choiceParameters")
        .ok_or_else(|| {
            SchemaError::InvalidFormChoiceParametersWriterEvidence(
                "matching writer rule is absent".to_owned(),
            )
        })?;
    if rule.id != "form.input-field-ext-info.choice-parameters"
        || rule.source_class != "com._1c.g5.v8.dt.export.xml.writer.ChoiceParameterWriter"
        || rule.delegate.as_deref()
            != Some("com._1c.g5.v8.dt.export.xml.writer.ChoiceParameterWriter")
        || rule.evidence.status != "verified"
        || rule.policy.as_ref() != Some(&evidence.policy())
    {
        return Err(SchemaError::InvalidFormChoiceParametersWriterEvidence(
            "writer rule and exact evidence are not cross-bound".to_owned(),
        ));
    }
    Ok(())
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
                    policy @ WriterPolicy::FormChoiceParameters { .. } => {
                        if policy != &exact_form_choice_parameters_policy()
                            || rule.id != "form.input-field-ext-info.choice-parameters"
                            || rule.model_type != "InputFieldExtInfo"
                            || rule.feature != "choiceParameters"
                            || rule.source_class
                                != "com._1c.g5.v8.dt.export.xml.writer.ChoiceParameterWriter"
                            || rule.delegate.as_deref()
                                != Some("com._1c.g5.v8.dt.export.xml.writer.ChoiceParameterWriter")
                        {
                            return Err(SchemaError::InvalidFormChoiceParametersWriterEvidence(
                                "dedicated writer policy identity or exact facts differ".to_owned(),
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
        preflight_canonical_coverage_json(json)?;
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
        )?;
        if self.family_aggregates != recompute_family_aggregates(&self.entries) {
            return Err(SchemaError::CoverageDerivedDataMismatch(
                "family aggregates",
            ));
        }

        let mut previous_backlog_key = None;
        for item in &self.migration_backlog {
            validate_text("canonical migration rule", &item.rule)?;
            validate_text("canonical migration package", &item.package)?;
            if item.features == 0 {
                return Err(SchemaError::CoverageDerivedDataMismatch(
                    "migration backlog",
                ));
            }
            let key = (
                item.family,
                item.package.as_str(),
                item.classifier_kind,
                item.feature_kind,
            );
            if previous_backlog_key
                .as_ref()
                .is_some_and(|previous| previous >= &key)
            {
                return Err(SchemaError::CoverageDerivedDataMismatch(
                    "migration backlog order",
                ));
            }
            previous_backlog_key = Some(key);
        }
        Ok(())
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
        if self.migration_backlog != recompute_migration_backlog(self, features)? {
            return Err(SchemaError::CoverageDerivedDataMismatch(
                "migration backlog",
            ));
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

fn bundled_dcs_list_settings_feature_semantics() -> Result<FeatureSemanticsCorpus, SchemaError> {
    FeatureSemanticsCorpus::parse(BUNDLED_DCS_LIST_SETTINGS_FEATURE_SEMANTICS_JSON)
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
    let corpus = WriterRuleCorpus::parse(BUNDLED_WRITER_RULES_JSON)?;
    bind_form_choice_parameters_writer_evidence(
        BUNDLED_FORM_CHOICE_PARAMETERS_WRITER_EVIDENCE_JSON,
        &corpus,
    )?;
    Ok(corpus)
}

pub fn bundled_form_choice_parameters_writer_evidence()
-> Result<FormChoiceParametersWriterEvidence, SchemaError> {
    FormChoiceParametersWriterEvidence::parse(BUNDLED_FORM_CHOICE_PARAMETERS_WRITER_EVIDENCE_JSON)
}

pub fn bundled_dcs_writer_evidence() -> Result<DcsWriterEvidenceCorpus, SchemaError> {
    DcsWriterEvidenceCorpus::parse(BUNDLED_DCS_WRITER_EVIDENCE_JSON)
}

pub fn bundled_dcs_list_settings_tail_policy() -> Result<DcsListSettingsTailPolicy, SchemaError> {
    static POLICY: OnceLock<Result<DcsListSettingsTailPolicy, SchemaError>> = OnceLock::new();
    POLICY
        .get_or_init(|| {
            let evidence = bundled_dcs_writer_evidence()?;
            let feature_semantics = bundled_dcs_list_settings_feature_semantics()?;
            evidence.form_list_settings_tail_policy(&feature_semantics)
        })
        .clone()
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

    const FORM_CHOICE_LIST_STRING_WRITER_EVIDENCE_JSON: &str =
        include_str!("../data/edt-2025.2.3-form-choice-list-string-writer-evidence.json");

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct FormChoiceListStringEvidenceReport {
        schema_version: u32,
        source: FormChoiceListStringEvidenceSource,
        verified_facts: Vec<FormChoiceListStringEvidenceFact>,
        missing_keys: Vec<serde_json::Value>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct FormChoiceListStringEvidenceSource {
        product: String,
        release: String,
        root_identity: FormChoiceListStringEvidenceRootIdentity,
        validated_bundles: Vec<FormChoiceListStringEvidenceBundle>,
        derivation: String,
        input_contract: String,
        invocation: String,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct FormChoiceListStringEvidenceRootIdentity {
        leaf: String,
        product_version: String,
        build_id: String,
        product: String,
        application: String,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct FormChoiceListStringEvidenceBundle {
        symbolic_name: String,
        version: String,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct FormChoiceListStringEvidenceFact {
        key: String,
        value: FormChoiceListStringEvidenceValue,
        evidence: FormChoiceListStringEvidenceProof,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct FormChoiceListStringEvidenceValue {
        model_value_type: String,
        empty_predicate: String,
        element: String,
        xsi_type: String,
        emission: FormChoiceListEmptyStringValue,
        delegate_chain: Vec<String>,
        branch: FormChoiceListStringEvidenceBranch,
        method_envelopes: Vec<FormChoiceListStringEvidenceMethodEnvelope>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct FormChoiceListStringEvidenceBranch {
        string_type_offset: u32,
        empty_predicate_offset: u32,
        non_empty_target_offset: u32,
        empty_element_offset: u32,
        xsi_type_attribute_offset: u32,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct FormChoiceListStringEvidenceMethodEnvelope {
        method: String,
        descriptor: String,
        instruction_count: usize,
        first_offset: u32,
        last_offset: u32,
        branch_graph: Vec<String>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct FormChoiceListStringEvidenceProof {
        kind: String,
        status: String,
        sources: Vec<String>,
        note: String,
    }

    fn parse_exact_form_choice_list_string_evidence(
        json: &str,
    ) -> Result<FormChoiceListStringEvidenceReport, String> {
        let report: FormChoiceListStringEvidenceReport =
            serde_json::from_str(json).map_err(|error| error.to_string())?;
        if report.schema_version != 1
            || report.source.product != "1C:EDT"
            || report.source.release != "2025.2.3+30"
            || !report.missing_keys.is_empty()
            || report.verified_facts.len() != 1
            || report.verified_facts[0].key != "form.FormChoiceListDesTimeValue.value.empty-string"
        {
            return Err("evidence does not have the exact verified fact envelope".to_owned());
        }
        Ok(report)
    }

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
        assert_eq!(corpus.rules.len(), 4);
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
                empty_string_value: FormChoiceListEmptyStringValue::SelfClosing,
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

        let choice_parameters = corpus
            .exact_rule(WriterRuleKey {
                source_release: "2025.2.3+30",
                model_type: "InputFieldExtInfo",
                feature: "choiceParameters",
            })
            .unwrap();
        assert_eq!(
            choice_parameters.policy,
            Some(exact_form_choice_parameters_policy())
        );
    }

    #[test]
    fn form_choice_parameters_policy_is_strictly_cross_bound_to_production_evidence() {
        let evidence =
            bundled_form_choice_parameters_writer_evidence().expect("strict production evidence");
        assert!(evidence.scope.production_emission);
        assert!(evidence.missing_keys.is_empty());
        let corpus = WriterRuleCorpus::parse(BUNDLED_WRITER_RULES_JSON).unwrap();
        bind_form_choice_parameters_writer_evidence(
            BUNDLED_FORM_CHOICE_PARAMETERS_WRITER_EVIDENCE_JSON,
            &corpus,
        )
        .unwrap();

        let raw: serde_json::Value =
            serde_json::from_str(BUNDLED_FORM_CHOICE_PARAMETERS_WRITER_EVIDENCE_JSON).unwrap();
        let mut unknown = raw.clone();
        unknown["verifiedFacts"]["writer"]["item"]["unexpected"] = serde_json::json!(true);
        assert!(
            FormChoiceParametersWriterEvidence::parse(&serde_json::to_string(&unknown).unwrap())
                .is_err()
        );
        let mut missing = raw.clone();
        missing["missingKeys"] = serde_json::json!(["form.choiceParameters.qname"]);
        assert!(
            FormChoiceParametersWriterEvidence::parse(&serde_json::to_string(&missing).unwrap())
                .is_err()
        );
        let mut wrong_successor = raw;
        wrong_successor["verifiedFacts"]["ownerOrder"]["successorQName"] =
            serde_json::json!("{http://v8.1c.ru/8.3/xcf/logform}Wrong");
        assert!(
            FormChoiceParametersWriterEvidence::parse(
                &serde_json::to_string(&wrong_successor).unwrap()
            )
            .is_err()
        );
        for pointer in [
            "/verifiedFacts/liveSlot27/fixtureSha256",
            "/verifiedFacts/liveSlot27/rawSourceSha256",
            "/verifiedFacts/liveSlot27/nativeSourceSha256",
        ] {
            let mut wrong_hash: serde_json::Value =
                serde_json::from_str(BUNDLED_FORM_CHOICE_PARAMETERS_WRITER_EVIDENCE_JSON).unwrap();
            *wrong_hash.pointer_mut(pointer).unwrap() = serde_json::json!(
                "0000000000000000000000000000000000000000000000000000000000000000"
            );
            assert!(
                FormChoiceParametersWriterEvidence::parse(
                    &serde_json::to_string(&wrong_hash).unwrap()
                )
                .is_err(),
                "evidence mutation {pointer} must fail closed"
            );
        }

        let mut wrong_policy = corpus;
        let rule = wrong_policy
            .rules
            .iter_mut()
            .find(|rule| rule.feature == "choiceParameters")
            .unwrap();
        let Some(WriterPolicy::FormChoiceParameters {
            owner_successor_qname,
            ..
        }) = rule.policy.as_mut()
        else {
            panic!("dedicated choice-parameters policy");
        };
        *owner_successor_qname = "{http://v8.1c.ru/8.3/xcf/logform}Wrong".to_owned();
        assert!(
            bind_form_choice_parameters_writer_evidence(
                BUNDLED_FORM_CHOICE_PARAMETERS_WRITER_EVIDENCE_JSON,
                &wrong_policy,
            )
            .is_err()
        );

        let mut missing_policy = WriterRuleCorpus::parse(BUNDLED_WRITER_RULES_JSON).unwrap();
        missing_policy
            .rules
            .iter_mut()
            .find(|rule| rule.feature == "choiceParameters")
            .unwrap()
            .policy = None;
        assert!(
            bind_form_choice_parameters_writer_evidence(
                BUNDLED_FORM_CHOICE_PARAMETERS_WRITER_EVIDENCE_JSON,
                &missing_policy,
            )
            .is_err()
        );

        let mut unknown_policy_field: serde_json::Value =
            serde_json::from_str(BUNDLED_WRITER_RULES_JSON).unwrap();
        let policy = unknown_policy_field["rules"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|rule| rule["feature"] == "choiceParameters")
            .unwrap()
            .get_mut("policy")
            .unwrap();
        policy["unexpected"] = serde_json::json!(true);
        assert!(
            WriterRuleCorpus::parse(&serde_json::to_string(&unknown_policy_field).unwrap())
                .is_err()
        );

        let original_rules: serde_json::Value =
            serde_json::from_str(BUNDLED_WRITER_RULES_JSON).unwrap();
        let rule_index = original_rules["rules"]
            .as_array()
            .unwrap()
            .iter()
            .position(|rule| rule["feature"] == "choiceParameters")
            .unwrap();
        let policy_mutations = [
            (
                "/ownerQName",
                serde_json::json!("{urn:wrong}ChoiceParameters"),
            ),
            (
                "/ownerPredecessorQName",
                serde_json::json!("{urn:wrong}ChoiceParameterLinks"),
            ),
            (
                "/ownerSuccessorQName",
                serde_json::json!("{urn:wrong}AvailableTypes"),
            ),
            (
                "/emptyCollection",
                serde_json::json!("write-wrapper-when-write-default"),
            ),
            ("/item/itemQName", serde_json::json!("{urn:wrong}item")),
            (
                "/item/nameAttributeQName",
                serde_json::json!("{urn:wrong}name"),
            ),
            ("/item/valueQName", serde_json::json!("{urn:wrong}value")),
            ("/item/valueXsiType", serde_json::json!("Wrong")),
            (
                "/item/valueOrder",
                serde_json::json!(["value", "presentation"]),
            ),
            (
                "/item/presentationQName",
                serde_json::json!("{urn:wrong}Presentation"),
            ),
            (
                "/item/scalarValueQName",
                serde_json::json!("{urn:wrong}Value"),
            ),
            ("/item/booleanXsiType", serde_json::json!("xs:string")),
            ("/item/designTimeRefXsiType", serde_json::json!("xs:string")),
            ("/fixedArray/xsiType", serde_json::json!("v8:Array")),
            (
                "/fixedArray/itemQName",
                serde_json::json!("{urn:wrong}Value"),
            ),
            ("/fixedArray/itemXsiType", serde_json::json!("Wrong")),
            (
                "/fixedArray/itemOrder",
                serde_json::json!(["value", "presentation"]),
            ),
        ];
        for (relative_pointer, replacement) in policy_mutations {
            let mut mutated = original_rules.clone();
            let pointer = format!("/rules/{rule_index}/policy{relative_pointer}");
            *mutated
                .pointer_mut(&pointer)
                .unwrap_or_else(|| panic!("policy mutation pointer {pointer}")) = replacement;
            assert!(
                WriterRuleCorpus::parse(&serde_json::to_string(&mutated).unwrap()).is_err(),
                "policy mutation {relative_pointer} must fail closed"
            );
        }
    }

    #[test]
    fn form_choice_list_empty_string_policy_matches_exact_research_evidence() {
        let report = parse_exact_form_choice_list_string_evidence(
            FORM_CHOICE_LIST_STRING_WRITER_EVIDENCE_JSON,
        )
        .expect("strict Form choice-list string evidence");
        assert_eq!(report.schema_version, 1);
        assert_eq!(report.source.product, "1C:EDT");
        assert_eq!(report.source.release, "2025.2.3+30");
        assert_eq!(
            report.source.root_identity.leaf,
            "1c-edt-2025.2.3+30-x86_64"
        );
        assert_eq!(report.source.root_identity.product_version, "2025.2.3");
        assert_eq!(report.source.root_identity.build_id, "2025.2.3.30");
        assert_eq!(
            report.source.root_identity.product,
            "com._1c.g5.v8.dt.product.application.rcp"
        );
        assert_eq!(
            report.source.root_identity.application,
            "org.eclipse.ui.ide.workbench"
        );
        assert_eq!(report.source.validated_bundles.len(), 2);
        assert_eq!(
            (
                report.source.validated_bundles[0].symbolic_name.as_str(),
                report.source.validated_bundles[0].version.as_str(),
            ),
            ("com._1c.g5.v8.dt.form.export.xml", "10.1.0.v202602241426",)
        );
        assert_eq!(
            (
                report.source.validated_bundles[1].symbolic_name.as_str(),
                report.source.validated_bundles[1].version.as_str(),
            ),
            ("com._1c.g5.v8.dt.export.xml", "13.0.100.v202602241426",)
        );
        assert!(!report.source.derivation.trim().is_empty());
        assert!(!report.source.input_contract.trim().is_empty());
        assert!(!report.source.invocation.trim().is_empty());
        assert!(report.missing_keys.is_empty());
        assert_eq!(report.verified_facts.len(), 1);

        let fact = &report.verified_facts[0];
        assert_eq!(
            fact.key,
            "form.FormChoiceListDesTimeValue.value.empty-string"
        );
        assert_eq!(fact.value.model_value_type, "mcore:StringValue");
        assert_eq!(fact.value.empty_predicate, "Strings.isNullOrEmpty");
        assert_eq!(fact.value.element, "feature QName");
        assert_eq!(fact.value.xsi_type, "xs:string");
        assert_eq!(
            fact.value.delegate_chain,
            [
                "FormChoiceListDesTimeValueWriter.write",
                "FormSmartFeatureWriter.write",
                "FormValueWriter.writeValue",
                "ValueWriter.writeValue",
                "ExportXmlStreamWriter.writeEmptyElement",
                "XMLStreamWriter.writeEmptyElement",
            ]
        );
        assert_eq!(fact.value.branch.string_type_offset, 144);
        assert_eq!(fact.value.branch.empty_predicate_offset, 163);
        assert_eq!(fact.value.branch.non_empty_target_offset, 187);
        assert_eq!(fact.value.branch.empty_element_offset, 171);
        assert_eq!(fact.value.branch.xsi_type_attribute_offset, 181);
        assert_eq!(fact.value.method_envelopes.len(), 6);
        let feature_descriptor = "(Lcom/_1c/g5/v8/dt/export/xml/writer/ExportXmlStreamWriter;Lorg/eclipse/emf/ecore/EObject;Lorg/eclipse/emf/ecore/EStructuralFeature;ZLcom/_1c/g5/v8/dt/export/xml/IExportContext;)V";
        let value_descriptor = "(Lcom/_1c/g5/v8/dt/export/xml/writer/ExportXmlStreamWriter;Ljava/lang/Object;Ljavax/xml/namespace/QName;ZLorg/eclipse/emf/ecore/EStructuralFeature;Lcom/_1c/g5/v8/dt/export/xml/IExportContext;)V";
        let expected_envelopes = [
            (
                "FormChoiceListDesTimeValueWriter.write",
                feature_descriptor,
                108,
                253,
                8,
            ),
            (
                "FormSmartFeatureWriter.write",
                feature_descriptor,
                90,
                209,
                11,
            ),
            (
                "FormSmartFeatureWriter.fillSpecialClassifierWriters",
                "()Lcom/google/common/collect/ImmutableMap;",
                165,
                360,
                0,
            ),
            ("FormValueWriter.writeValue", value_descriptor, 125, 314, 14),
            ("ValueWriter.writeValue", value_descriptor, 567, 1345, 64),
            (
                "ExportXmlStreamWriter.writeEmptyElement",
                "(Ljavax/xml/namespace/QName;)V",
                21,
                42,
                1,
            ),
        ];
        for (envelope, (method, descriptor, count, last_offset, branch_count)) in
            fact.value.method_envelopes.iter().zip(expected_envelopes)
        {
            assert_eq!(envelope.method, method);
            assert_eq!(envelope.descriptor, descriptor);
            assert_eq!(envelope.instruction_count, count);
            assert_eq!(envelope.first_offset, 0);
            assert_eq!(envelope.last_offset, last_offset);
            assert_eq!(envelope.branch_graph.len(), branch_count);
        }
        assert_eq!(
            fact.evidence.kind,
            "javap-v-exact-method-control-flow-constant-pool"
        );
        assert_eq!(fact.evidence.status, "verified");
        assert_eq!(fact.evidence.sources.len(), 7);
        assert!(!fact.evidence.note.trim().is_empty());
        assert!(fact.evidence.sources.iter().all(|source| {
            source == "tools/report-edt-form-choice-list-string-writer-evidence.ps1"
                || source.starts_with("edt-derived://2025.2.3+30/")
        }));

        let corpus = bundled_writer_rules().unwrap();
        let policy = corpus
            .exact_rule(WriterRuleKey {
                source_release: &report.source.release,
                model_type: "FormChoiceList",
                feature: "values",
            })
            .unwrap()
            .policy
            .as_ref()
            .expect("verified choice-list writer policy");
        let WriterPolicy::FormChoiceList {
            empty_string_value, ..
        } = policy
        else {
            panic!("unexpected choice-list writer policy kind");
        };
        assert_eq!(*empty_string_value, fact.value.emission);

        let raw: serde_json::Value =
            serde_json::from_str(FORM_CHOICE_LIST_STRING_WRITER_EVIDENCE_JSON).unwrap();
        let mut extra_field = raw.clone();
        extra_field["unexpected"] = serde_json::json!(true);
        assert!(
            parse_exact_form_choice_list_string_evidence(
                &serde_json::to_string(&extra_field).unwrap()
            )
            .is_err()
        );

        let mut missing_emission = raw.clone();
        missing_emission["verifiedFacts"][0]["value"]
            .as_object_mut()
            .unwrap()
            .remove("emission");
        assert!(
            parse_exact_form_choice_list_string_evidence(
                &serde_json::to_string(&missing_emission).unwrap()
            )
            .is_err()
        );

        let mut other_emission = raw.clone();
        other_emission["verifiedFacts"][0]["value"]["emission"] = serde_json::json!("paired");
        assert!(
            parse_exact_form_choice_list_string_evidence(
                &serde_json::to_string(&other_emission).unwrap()
            )
            .is_err()
        );

        let mut extra_fact = raw;
        let duplicate = extra_fact["verifiedFacts"][0].clone();
        extra_fact["verifiedFacts"]
            .as_array_mut()
            .unwrap()
            .push(duplicate);
        assert!(
            parse_exact_form_choice_list_string_evidence(
                &serde_json::to_string(&extra_fact).unwrap()
            )
            .is_err()
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

        let raw: serde_json::Value =
            serde_json::from_str(BUNDLED_WRITER_RULES_JSON).expect("bundled writer rules JSON");
        let choice_index = raw["rules"]
            .as_array()
            .and_then(|rules| {
                rules
                    .iter()
                    .position(|rule| rule["id"] == "form.choice-list.design-time-value")
            })
            .expect("choice-list writer rule");

        let mut missing_empty_string = raw.clone();
        missing_empty_string["rules"][choice_index]["policy"]
            .as_object_mut()
            .expect("choice-list writer policy")
            .remove("emptyStringValue");
        assert!(
            WriterRuleCorpus::parse(
                &serde_json::to_string(&missing_empty_string).expect("mutated JSON")
            )
            .is_err()
        );

        let mut unsupported_empty_string = raw;
        unsupported_empty_string["rules"][choice_index]["policy"]["emptyStringValue"] =
            serde_json::json!("paired");
        assert!(
            WriterRuleCorpus::parse(
                &serde_json::to_string(&unsupported_empty_string).expect("mutated JSON")
            )
            .is_err()
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
                .filter(|entry| {
                    serde_json::to_value(entry.family).unwrap() == serde_json::json!(family)
                })
                .count()
        };
        assert_eq!(family_count("metadata"), 0);
        assert_eq!(family_count("forms"), 2_314);
        assert_eq!(family_count("dcs"), 511);
        assert_eq!(family_count("mxl"), 0);
        assert_eq!(family_count("common"), 0);
        assert_eq!(family_count("other"), 2_141);
        assert_eq!(
            corpus
                .family_aggregates
                .iter()
                .map(|aggregate| aggregate.entries)
                .sum::<usize>(),
            4_966
        );
        assert_eq!(
            corpus
                .migration_backlog
                .iter()
                .map(|item| item.features)
                .sum::<usize>(),
            4_964
        );
        assert!(
            corpus
                .migration_backlog
                .iter()
                .all(|item| item.rule == "unsupported/schema.unmapped")
        );
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

    #[test]
    fn bundled_dcs_writer_evidence_exposes_only_the_verified_tail() {
        let corpus = bundled_dcs_writer_evidence().unwrap();
        let feature_semantics = bundled_dcs_list_settings_feature_semantics().unwrap();
        let policy = corpus
            .form_list_settings_tail_policy(&feature_semantics)
            .unwrap();
        assert_eq!(
            policy.namespace_uri(),
            "http://v8.1c.ru/8.1/data-composition-system/settings"
        );
        assert_eq!(
            policy.tail_order(),
            &[
                DcsListSettingsTailField::ItemsViewMode,
                DcsListSettingsTailField::ItemsUserSettingId,
            ]
        );
        assert_eq!(policy.items_view_mode_default(), "QuickAccess");
        assert_eq!(policy.items_user_setting_id_default(), "");
        assert_eq!(corpus.missing_keys.len(), 4);
    }

    #[test]
    fn bundled_dcs_runtime_feature_slice_matches_full_research_corpus() {
        let runtime = bundled_dcs_list_settings_feature_semantics().unwrap();
        let full = bundled_feature_semantics().unwrap();
        let runtime_raw = serde_json::from_str::<serde_json::Value>(
            BUNDLED_DCS_LIST_SETTINGS_FEATURE_SEMANTICS_JSON,
        )
        .unwrap();
        let typed_runtime_raw = serde_json::to_value(&runtime).unwrap();
        assert_eq!(runtime_raw, typed_runtime_raw);
        let is_exact_typed_projection =
            |raw: &serde_json::Value, parsed: &FeatureSemanticsCorpus| {
                raw == &serde_json::to_value(parsed).unwrap()
            };

        let mut root_extra = runtime_raw.clone();
        root_extra["unexpected"] = serde_json::json!("payload");
        let root_extra_parsed =
            FeatureSemanticsCorpus::parse(&serde_json::to_string(&root_extra).unwrap()).unwrap();
        assert!(!is_exact_typed_projection(&root_extra, &root_extra_parsed));

        let mut nested_extra = runtime_raw.clone();
        nested_extra["packages"][0]["classifiers"][0]["features"][0]["unexpected"] =
            serde_json::json!("payload");
        let nested_extra_parsed =
            FeatureSemanticsCorpus::parse(&serde_json::to_string(&nested_extra).unwrap()).unwrap();
        assert!(!is_exact_typed_projection(
            &nested_extra,
            &nested_extra_parsed
        ));

        assert_eq!(runtime.schema_version, 1);
        assert_eq!(runtime.source.product, "1C:EDT");
        assert_eq!(runtime.source.release, "2025.2.3+30");
        assert_eq!(runtime.source.product, full.source.product);
        assert_eq!(runtime.source.release, full.source.release);
        assert_eq!(
            runtime.source.derivation,
            "deterministic runtime projection of the verified Xcore feature semantics corpus"
        );
        assert_eq!(
            runtime.summary,
            FeatureSemanticsSummary {
                packages: 1,
                classifiers: 1,
                features: 1,
            }
        );
        assert_eq!(runtime.packages.len(), 1);
        let package = &runtime.packages[0];
        assert_eq!(package.bundle, "com._1c.g5.v8.dt.dcs.model");
        assert_eq!(package.resource, "model/settings.xcore");
        assert_eq!(package.package_name, "com._1c.g5.v8.dt.dcs.model.settings");
        assert_eq!(package.namespace_uri, DCS_SETTINGS_MODEL_NAMESPACE);
        assert_eq!(package.classifiers.len(), 1);
        let classifier = &package.classifiers[0];
        assert_eq!(classifier.name, DCS_SETTINGS_CLASSIFIER);
        assert_eq!(classifier.kind, FeatureClassifierKind::Class);
        assert_eq!(classifier.features.len(), 1);
        let feature = &classifier.features[0];
        assert_eq!(feature.name, "itemsViewMode");

        let key = FeatureSemanticKey {
            namespace_uri: DCS_SETTINGS_MODEL_NAMESPACE.to_owned(),
            classifier: DCS_SETTINGS_CLASSIFIER.to_owned(),
            feature: "itemsViewMode".to_owned(),
        };
        assert_eq!(Some(feature), full.feature(&key));

        for marker in [
            b"ibcmd.exe".as_slice(),
            b"1cv8.exe",
            b"1cv8c.exe",
            b"\\1cv8\\",
            b"/1cv8/",
            b".jar",
            b"org.eclipse",
            b"JNI_CreateJavaVM",
            b"JNIEnv",
            b"JavaVM",
            b"OSGi",
        ] {
            assert!(
                !BUNDLED_DCS_LIST_SETTINGS_FEATURE_SEMANTICS_JSON
                    .as_bytes()
                    .windows(marker.len())
                    .any(|window| window == marker),
                "runtime DCS feature-semantics slice contains forbidden payload marker `{}`",
                String::from_utf8_lossy(marker)
            );
        }
    }

    #[test]
    fn dcs_tail_model_default_other_fails_closed() {
        let corpus = bundled_dcs_writer_evidence().unwrap();
        let mut feature_semantics = bundled_feature_semantics().unwrap();
        let feature = feature_semantics
            .packages
            .iter_mut()
            .find(|package| package.namespace_uri == DCS_SETTINGS_MODEL_NAMESPACE)
            .and_then(|package| {
                package
                    .classifiers
                    .iter_mut()
                    .find(|classifier| classifier.name == DCS_SETTINGS_CLASSIFIER)
            })
            .and_then(|classifier| {
                classifier
                    .features
                    .iter_mut()
                    .find(|feature| feature.name == "itemsViewMode")
            })
            .unwrap();
        feature.default_value = Some("Other".to_owned());

        assert!(matches!(
            corpus.form_list_settings_tail_policy(&feature_semantics),
            Err(SchemaError::InvalidDcsWriterEvidence(message))
                if message.contains("exact default join")
        ));
    }

    #[test]
    fn dcs_tail_writer_constant_other_fails_closed() {
        let mut writer_evidence =
            serde_json::from_str::<serde_json::Value>(BUNDLED_DCS_WRITER_EVIDENCE_JSON).unwrap();
        writer_evidence["verifiedFacts"][2]["value"]["defaultModelConstant"] =
            serde_json::json!("OTHER");

        assert!(matches!(
            DcsWriterEvidenceCorpus::parse(
                &serde_json::to_string(&writer_evidence).unwrap()
            ),
            Err(SchemaError::InvalidDcsWriterEvidence(message))
                if message.contains("itemsViewMode writer policy drifted")
        ));
    }

    #[test]
    fn dcs_writer_evidence_parser_is_bounded_and_fails_closed_on_drift() {
        let oversized = " ".repeat(MAX_DCS_WRITER_EVIDENCE_JSON_BYTES + 1);
        assert!(matches!(
            DcsWriterEvidenceCorpus::parse(&oversized),
            Err(SchemaError::InvalidDcsWriterEvidence(message))
                if message.contains("JSON exceeds")
        ));

        let mut unknown =
            serde_json::from_str::<serde_json::Value>(BUNDLED_DCS_WRITER_EVIDENCE_JSON).unwrap();
        unknown["forged"] = serde_json::json!(true);
        assert!(matches!(
            DcsWriterEvidenceCorpus::parse(&serde_json::to_string(&unknown).unwrap()),
            Err(SchemaError::InvalidJson(message)) if message.contains("unknown field")
        ));

        let mut duplicate =
            serde_json::from_str::<serde_json::Value>(BUNDLED_DCS_WRITER_EVIDENCE_JSON).unwrap();
        let duplicate_fact = duplicate["verifiedFacts"][0].clone();
        duplicate["verifiedFacts"]
            .as_array_mut()
            .unwrap()
            .push(duplicate_fact);
        assert!(matches!(
            DcsWriterEvidenceCorpus::parse(&serde_json::to_string(&duplicate).unwrap()),
            Err(SchemaError::InvalidDcsWriterEvidence(message))
                if message.contains("duplicate verified fact")
        ));
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
            family_aggregates: vec![
                CanonicalCoverageFamilyAggregate {
                    family: CanonicalCoverageFamily::Metadata,
                    entries: 0,
                    typed: 0,
                    opaque_lossless: 0,
                    unsupported: 0,
                    platform_only: 0,
                },
                CanonicalCoverageFamilyAggregate {
                    family: CanonicalCoverageFamily::Forms,
                    entries: 1,
                    typed: 1,
                    opaque_lossless: 0,
                    unsupported: 0,
                    platform_only: 0,
                },
                CanonicalCoverageFamilyAggregate {
                    family: CanonicalCoverageFamily::Dcs,
                    entries: 0,
                    typed: 0,
                    opaque_lossless: 0,
                    unsupported: 0,
                    platform_only: 0,
                },
                CanonicalCoverageFamilyAggregate {
                    family: CanonicalCoverageFamily::Mxl,
                    entries: 0,
                    typed: 0,
                    opaque_lossless: 0,
                    unsupported: 0,
                    platform_only: 0,
                },
                CanonicalCoverageFamilyAggregate {
                    family: CanonicalCoverageFamily::Common,
                    entries: 0,
                    typed: 0,
                    opaque_lossless: 0,
                    unsupported: 0,
                    platform_only: 0,
                },
                CanonicalCoverageFamilyAggregate {
                    family: CanonicalCoverageFamily::Other,
                    entries: 0,
                    typed: 0,
                    opaque_lossless: 0,
                    unsupported: 0,
                    platform_only: 0,
                },
            ],
            migration_backlog: vec![],
            entries: vec![CanonicalCoverageEntry {
                key: FeatureSemanticKey {
                    namespace_uri: "http://g5.1c.ru/v8/dt/form".to_owned(),
                    classifier: "Form".to_owned(),
                    feature: "baseForm".to_owned(),
                },
                family: CanonicalCoverageFamily::Forms,
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
    fn canonical_coverage_public_parse_enforces_exact_byte_and_string_limits() {
        let mut json = serde_json::to_string(&canonical_coverage_fixture()).unwrap();
        json.push_str(&" ".repeat(MAX_CANONICAL_COVERAGE_JSON_BYTES - json.len()));
        assert_eq!(json.len(), MAX_CANONICAL_COVERAGE_JSON_BYTES);
        assert!(CanonicalCoverageCorpus::parse(&json).is_ok());

        json.push(' ');
        assert!(matches!(
            CanonicalCoverageCorpus::parse(&json),
            Err(SchemaError::InvalidJson(message))
                if message.contains("exceeds 4194304 UTF-8 bytes")
        ));

        let mut value = serde_json::to_value(canonical_coverage_fixture()).unwrap();
        value["source"]["derivation"] =
            serde_json::Value::String("x".repeat(MAX_CANONICAL_COVERAGE_STRING_BYTES));
        assert!(CanonicalCoverageCorpus::parse(&serde_json::to_string(&value).unwrap()).is_ok());

        value["source"]["derivation"] =
            serde_json::Value::String("x".repeat(MAX_CANONICAL_COVERAGE_STRING_BYTES + 1));
        assert!(matches!(
            CanonicalCoverageCorpus::parse(&serde_json::to_string(&value).unwrap()),
            Err(SchemaError::InvalidJson(message))
                if message.contains("exceeds 4096 UTF-8 bytes")
        ));
    }

    #[test]
    fn canonical_coverage_public_parse_enforces_exact_vector_limits() {
        let mut evidence_limit = canonical_coverage_fixture();
        evidence_limit.entries[0].evidence.sources = (0..MAX_CANONICAL_COVERAGE_EVIDENCE_SOURCES)
            .map(|index| format!("evidence/source-{index}"))
            .collect();
        assert!(
            CanonicalCoverageCorpus::parse(&serde_json::to_string(&evidence_limit).unwrap())
                .is_ok()
        );
        evidence_limit.entries[0]
            .evidence
            .sources
            .push("evidence/overflow".to_owned());
        assert!(matches!(
            CanonicalCoverageCorpus::parse(&serde_json::to_string(&evidence_limit).unwrap()),
            Err(SchemaError::InvalidJson(message))
                if message.contains("exceeds 16 elements")
        ));

        let mut family_limit = canonical_coverage_fixture();
        assert!(
            CanonicalCoverageCorpus::parse(&serde_json::to_string(&family_limit).unwrap()).is_ok()
        );
        family_limit
            .family_aggregates
            .push(family_limit.family_aggregates[0].clone());
        assert!(matches!(
            CanonicalCoverageCorpus::parse(&serde_json::to_string(&family_limit).unwrap()),
            Err(SchemaError::InvalidJson(message))
                if message.contains("exceeds 6 elements")
        ));

        let mut backlog_limit = canonical_coverage_fixture();
        backlog_limit.migration_backlog = (0..MAX_CANONICAL_COVERAGE_BACKLOG_ENTRIES)
            .map(|index| CanonicalMigrationBacklogEntry {
                rule: "unsupported/schema.unmapped".to_owned(),
                family: CanonicalCoverageFamily::Metadata,
                package: format!("package.{index:03}"),
                classifier_kind: FeatureClassifierKind::Class,
                feature_kind: FeatureKind::Attribute,
                features: 1,
            })
            .collect();
        assert!(
            CanonicalCoverageCorpus::parse(&serde_json::to_string(&backlog_limit).unwrap()).is_ok()
        );
        backlog_limit
            .migration_backlog
            .push(CanonicalMigrationBacklogEntry {
                rule: "unsupported/schema.unmapped".to_owned(),
                family: CanonicalCoverageFamily::Metadata,
                package: "package.overflow".to_owned(),
                classifier_kind: FeatureClassifierKind::Class,
                feature_kind: FeatureKind::Attribute,
                features: 1,
            });
        assert!(matches!(
            CanonicalCoverageCorpus::parse(&serde_json::to_string(&backlog_limit).unwrap()),
            Err(SchemaError::InvalidJson(message))
                if message.contains("exceeds 256 elements")
        ));

        let template = canonical_coverage_fixture().entries.remove(0);
        let mut entry_limit = canonical_coverage_fixture();
        entry_limit.entries = (0..MAX_CANONICAL_COVERAGE_ENTRIES)
            .map(|index| {
                let mut entry = template.clone();
                entry.key.feature = format!("feature{index:04}");
                entry
            })
            .collect();
        entry_limit.summary.entries = MAX_CANONICAL_COVERAGE_ENTRIES;
        entry_limit.summary.typed = MAX_CANONICAL_COVERAGE_ENTRIES;
        entry_limit.family_aggregates = recompute_family_aggregates(&entry_limit.entries);
        assert!(
            CanonicalCoverageCorpus::parse(&serde_json::to_string(&entry_limit).unwrap()).is_ok()
        );
        let mut overflow = template;
        overflow.key.feature = "featureOverflow".to_owned();
        entry_limit.entries.push(overflow);
        entry_limit.summary.entries += 1;
        entry_limit.summary.typed += 1;
        entry_limit.family_aggregates = recompute_family_aggregates(&entry_limit.entries);
        assert!(matches!(
            CanonicalCoverageCorpus::parse(&serde_json::to_string(&entry_limit).unwrap()),
            Err(SchemaError::InvalidJson(message))
                if message.contains("exceeds 5000 elements")
        ));
    }

    #[test]
    fn canonical_coverage_public_parse_rejects_forged_and_duplicate_fields() {
        for field in ["unexpected", "uuid", "objectName"] {
            let mut value = serde_json::to_value(canonical_coverage_fixture()).unwrap();
            value["entries"][0]
                .as_object_mut()
                .unwrap()
                .insert(field.to_owned(), serde_json::json!("forged"));
            assert!(matches!(
                CanonicalCoverageCorpus::parse(&serde_json::to_string(&value).unwrap()),
                Err(SchemaError::InvalidJson(message)) if message.contains("unknown field")
            ));
        }

        let json = serde_json::to_string(&canonical_coverage_fixture()).unwrap();
        let duplicate_root = json.replacen(
            "\"schemaVersion\":1",
            "\"schemaVersion\":1,\"schemaVersion\":1",
            1,
        );
        assert!(matches!(
            CanonicalCoverageCorpus::parse(&duplicate_root),
            Err(SchemaError::InvalidJson(message)) if message.contains("duplicate field")
        ));
        let duplicate_key = json.replacen(
            "\"feature\":\"baseForm\"",
            "\"feature\":\"baseForm\",\"feature\":\"baseForm\"",
            1,
        );
        assert!(matches!(
            CanonicalCoverageCorpus::parse(&duplicate_key),
            Err(SchemaError::InvalidJson(message)) if message.contains("duplicate field")
        ));
    }

    #[test]
    fn canonical_coverage_public_parse_rejects_duplicate_key_map_entries() {
        let mut corpus = canonical_coverage_fixture();
        corpus.entries.push(corpus.entries[0].clone());
        corpus.summary.entries = 2;
        corpus.summary.typed = 2;
        corpus.family_aggregates = recompute_family_aggregates(&corpus.entries);
        assert!(matches!(
            CanonicalCoverageCorpus::parse(&serde_json::to_string(&corpus).unwrap()),
            Err(SchemaError::DuplicateValue {
                field: "canonical coverage key",
                ..
            })
        ));
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
        corpus.family_aggregates = recompute_family_aggregates(&corpus.entries);
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
        opaque.family_aggregates = recompute_family_aggregates(&opaque.entries);
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
        unsupported.family_aggregates = recompute_family_aggregates(&unsupported.entries);
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
        platform_only.family_aggregates = recompute_family_aggregates(&platform_only.entries);
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
        missing.family_aggregates = recompute_family_aggregates(&missing.entries);
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
        stale.family_aggregates = recompute_family_aggregates(&stale.entries);
        assert!(matches!(
            stale.validate_against(&features),
            Err(SchemaError::CoverageMismatch { kind: "stale", .. })
        ));
    }

    #[test]
    fn canonical_coverage_rejects_drifted_aggregates_and_backlog() {
        let features = bundled_feature_semantics().unwrap();
        let mut aggregate_drift = bundled_canonical_coverage().unwrap();
        aggregate_drift.family_aggregates[1].entries += 1;
        assert!(matches!(
            aggregate_drift.validate(),
            Err(SchemaError::CoverageDerivedDataMismatch(
                "family aggregates"
            ))
        ));

        let mut backlog_drift = bundled_canonical_coverage().unwrap();
        backlog_drift.migration_backlog[0].features += 1;
        assert!(matches!(
            backlog_drift.validate_against(&features),
            Err(SchemaError::CoverageDerivedDataMismatch(
                "migration backlog"
            ))
        ));
    }

    #[test]
    fn canonical_coverage_unknown_package_classifier_route_fails_closed() {
        let mut features = feature_semantics_fixture();
        features.packages[0].package_name = "unknown.package".to_owned();
        features.packages[0].namespace_uri = "http://g5.1c.ru/v8/dt/form".to_owned();
        features.packages[0].classifiers[0].name = "Form".to_owned();
        features.packages[0].classifiers[0].features[0].name = "baseForm".to_owned();

        assert!(matches!(
            canonical_coverage_fixture().validate_against(&features),
            Err(SchemaError::UnknownCoverageRoute { package, .. })
                if package == "unknown.package"
        ));
    }
}
