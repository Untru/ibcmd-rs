//! Bootstrap-readiness inventory and fail-closed build preflight.
//!
//! The bundled catalog deliberately keeps coarse [`SourceKind`] routes
//! conservative. Detailed legacy routing evidence is diagnostic only until a
//! family codec can map every source contributor to exact storage targets.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use ibcmd_core::storage::{StoragePatch, StoragePatchPreflightError};
use ibcmd_xml::source_tree::{SourceKind, SourceTree};
use serde::{Deserialize, Serialize};

const BOOTSTRAP_SCHEMA_VERSION: u32 = 1;
pub const MAX_BOOTSTRAP_MANIFEST_BYTES: usize = 1024 * 1024;
const MAX_LEGACY_INVENTORY_RECORDS: usize = 1_024;
const MAX_EXPECTED_ENTRIES: usize = 128;
const MAX_BASE_READS: usize = 64;
const MAX_TEXT_BYTES: usize = 1_024;

const REQUIRED_LEGACY_INVENTORY_IDS: &[&str] = &[
    "owner_metadata",
    "common_module_metadata",
    "versions",
    "module_body",
    "form_module_contributor",
    "raw_deflated_body",
    "command_interface_raw",
    "command_interface_readable",
    "additional_indexes_mapped",
    "additional_indexes_unmapped",
    "predefined_data",
    "business_process_flowchart",
    "form_body",
    "role_rights",
    "style_body_bypass",
    "scheduled_job_schedule_bypass",
    "binary_template_bypass",
    "ext_picture_bypass",
    "help_and_html_bypass",
    "exchange_plan_content_bypass",
    "spreadsheet_document_bypass",
    "picture_asset_contributor",
    "help_asset_contributor",
];

const BUNDLED_MANIFEST_JSON: &str = include_str!("../../compatibility/bootstrap.json");

/// Manifest representation of the canonical source-tree classifier.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BootstrapSourceKind {
    ConfigurationRoot,
    MetadataXml,
    Module,
    Form,
    Template,
    Binary,
    OtherXml,
    Other,
}

impl BootstrapSourceKind {
    pub const ALL: [Self; 8] = [
        Self::ConfigurationRoot,
        Self::MetadataXml,
        Self::Module,
        Self::Form,
        Self::Template,
        Self::Binary,
        Self::OtherXml,
        Self::Other,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ConfigurationRoot => "configuration_root",
            Self::MetadataXml => "metadata_xml",
            Self::Module => "module",
            Self::Form => "form",
            Self::Template => "template",
            Self::Binary => "binary",
            Self::OtherXml => "other_xml",
            Self::Other => "other",
        }
    }

    /// Exhaustive conversion keeps readiness coupled to the canonical enum.
    /// Adding a new canonical variant must update this match and the manifest.
    pub const fn from_canonical(kind: SourceKind) -> Self {
        match kind {
            SourceKind::ConfigurationRoot => Self::ConfigurationRoot,
            SourceKind::MetadataXml => Self::MetadataXml,
            SourceKind::Module => Self::Module,
            SourceKind::Form => Self::Form,
            SourceKind::Template => Self::Template,
            SourceKind::Binary => Self::Binary,
            SourceKind::OtherXml => Self::OtherXml,
            SourceKind::Other => Self::Other,
        }
    }
}

impl Display for BootstrapSourceKind {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Coarse or observed readiness outcome.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BootstrapOutcome {
    Compiled,
    NeedsBase,
    Unsupported,
}

/// How a legacy route consults native storage.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BootstrapBaseReadMode {
    Required,
    OptionalHint,
}

/// Explicit inventory record for one native-storage read.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BootstrapBaseRead {
    pub mode: BootstrapBaseReadMode,
    pub key: String,
    pub purpose: String,
}

/// Conservative gate rule for one canonical [`SourceKind`].
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BootstrapRoute {
    pub source_kind: BootstrapSourceKind,
    pub expected_entries: Vec<String>,
    pub codec: String,
    pub outcome: BootstrapOutcome,
    pub blocker: Option<String>,
    pub base_reads: Vec<BootstrapBaseRead>,
}

/// Relationship between a source artifact and native storage entries.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LegacyInventoryRelationship {
    Emits,
    ContributesTo,
}

/// Diagnostic evidence for a concrete legacy packer or staging route.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyInventoryRecord {
    pub id: String,
    pub source_kinds: Vec<BootstrapSourceKind>,
    pub selector: String,
    pub relationship: LegacyInventoryRelationship,
    pub expected_entries: Vec<String>,
    pub codec: String,
    pub codec_capability: BootstrapOutcome,
    pub outcome: BootstrapOutcome,
    pub blocker: Option<String>,
    pub base_reads: Vec<BootstrapBaseRead>,
}

/// Versioned bundled bootstrap catalog.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BootstrapManifest {
    pub schema_version: u32,
    pub routes: Vec<BootstrapRoute>,
    pub legacy_inventory: Vec<LegacyInventoryRecord>,
}

impl BootstrapManifest {
    /// Parses and fully validates a manifest before it can be assessed.
    pub fn from_json(json: &str) -> Result<Self, BootstrapManifestError> {
        if json.len() > MAX_BOOTSTRAP_MANIFEST_BYTES {
            return Err(BootstrapManifestError::ManifestTooLarge {
                maximum: MAX_BOOTSTRAP_MANIFEST_BYTES,
                actual: json.len(),
            });
        }
        let manifest: Self =
            serde_json::from_str(json).map_err(BootstrapManifestError::InvalidJson)?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Validates schema, exact canonical coverage, and declaration invariants.
    pub fn validate(&self) -> Result<(), BootstrapManifestError> {
        if self.schema_version != BOOTSTRAP_SCHEMA_VERSION {
            return Err(BootstrapManifestError::UnsupportedSchemaVersion {
                expected: BOOTSTRAP_SCHEMA_VERSION,
                actual: self.schema_version,
            });
        }

        let mut routes_by_kind = BTreeMap::new();
        for route in &self.routes {
            if routes_by_kind.insert(route.source_kind, route).is_some() {
                return Err(BootstrapManifestError::DuplicateSourceKind(
                    route.source_kind,
                ));
            }
            validate_declaration(
                &format!("route {}", route.source_kind),
                &route.expected_entries,
                &route.codec,
                route.outcome,
                route.blocker.as_deref(),
                &route.base_reads,
            )?;
        }
        for kind in BootstrapSourceKind::ALL {
            if !routes_by_kind.contains_key(&kind) {
                return Err(BootstrapManifestError::MissingSourceKind(kind));
            }
        }
        if self.routes.len() != BootstrapSourceKind::ALL.len() {
            return Err(BootstrapManifestError::InvalidDeclaration {
                location: "routes".to_string(),
                message: format!(
                    "expected exactly {} canonical routes, found {}",
                    BootstrapSourceKind::ALL.len(),
                    self.routes.len()
                ),
            });
        }

        if self.legacy_inventory.is_empty()
            || self.legacy_inventory.len() > MAX_LEGACY_INVENTORY_RECORDS
        {
            return Err(BootstrapManifestError::InvalidDeclaration {
                location: "legacy_inventory".to_string(),
                message: format!(
                    "must contain 1..={MAX_LEGACY_INVENTORY_RECORDS} diagnostic records"
                ),
            });
        }
        let mut inventory_ids = BTreeSet::new();
        let mut inventory_kinds = BTreeSet::new();
        for record in &self.legacy_inventory {
            validate_identifier(&record.id, "legacy inventory id")?;
            if !inventory_ids.insert(record.id.as_str()) {
                return Err(BootstrapManifestError::DuplicateInventoryId(
                    record.id.clone(),
                ));
            }
            if record.source_kinds.is_empty()
                || record.source_kinds.len() > BootstrapSourceKind::ALL.len()
            {
                return Err(BootstrapManifestError::InvalidDeclaration {
                    location: format!("legacy inventory {} source_kinds", record.id),
                    message: format!(
                        "must contain 1..={} canonical source kinds",
                        BootstrapSourceKind::ALL.len()
                    ),
                });
            }
            validate_text(
                &record.selector,
                &format!("legacy inventory {} selector", record.id),
            )?;
            let mut record_kinds = BTreeSet::new();
            for kind in &record.source_kinds {
                if !record_kinds.insert(*kind) {
                    return Err(BootstrapManifestError::InvalidDeclaration {
                        location: format!("legacy inventory {} source_kinds", record.id),
                        message: format!("duplicate source kind {kind}"),
                    });
                }
                inventory_kinds.insert(*kind);
            }
            validate_declaration(
                &format!("legacy inventory {}", record.id),
                &record.expected_entries,
                &record.codec,
                record.outcome,
                record.blocker.as_deref(),
                &record.base_reads,
            )?;
            validate_codec_capability(record)?;
        }
        for kind in BootstrapSourceKind::ALL {
            if !inventory_kinds.contains(&kind) {
                return Err(BootstrapManifestError::MissingInventorySourceKind(kind));
            }
        }
        for required in REQUIRED_LEGACY_INVENTORY_IDS {
            if !inventory_ids.contains(required) {
                return Err(BootstrapManifestError::MissingRequiredInventoryId(
                    (*required).to_string(),
                ));
            }
        }
        Ok(())
    }

    /// Returns the single validated coarse route for a canonical source kind.
    pub fn route(&self, kind: BootstrapSourceKind) -> Option<&BootstrapRoute> {
        self.routes.iter().find(|route| route.source_kind == kind)
    }

    /// Resolves every canonical source entry in exact [`SourceTree`] order.
    pub fn assess(
        &self,
        source_tree: &SourceTree,
    ) -> Result<BootstrapAssessment, BootstrapManifestError> {
        self.validate()?;
        let routes = self
            .routes
            .iter()
            .map(|route| (route.source_kind, route))
            .collect::<BTreeMap<_, _>>();
        let mut sources = Vec::with_capacity(source_tree.entries().len());
        for (source_index, source) in source_tree.entries().iter().enumerate() {
            let kind = BootstrapSourceKind::from_canonical(source.kind());
            let route = routes
                .get(&kind)
                .copied()
                .ok_or(BootstrapManifestError::MissingSourceKind(kind))?;
            sources.push(BootstrapSourceAssessment {
                source_index,
                source_kind: kind,
                outcome: route.outcome,
            });
        }
        Ok(BootstrapAssessment { sources })
    }
}

/// Loads the repository-bundled, schema-validated readiness catalog.
pub fn bundled_manifest() -> Result<BootstrapManifest, BootstrapManifestError> {
    BootstrapManifest::from_json(BUNDLED_MANIFEST_JSON)
}

/// Ordered readiness decision for one scanned source artifact.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BootstrapSourceAssessment {
    pub source_index: usize,
    pub source_kind: BootstrapSourceKind,
    pub outcome: BootstrapOutcome,
}

impl BootstrapSourceAssessment {
    pub const fn route_id(&self) -> &'static str {
        self.source_kind.as_str()
    }

    pub fn source_path<'a>(&self, source_tree: &'a SourceTree) -> Option<&'a str> {
        source_tree
            .entries()
            .get(self.source_index)
            .map(|source| source.path().as_str())
    }

    pub fn blocker_reason<'a>(&self, manifest: &'a BootstrapManifest) -> Option<&'a str> {
        manifest
            .route(self.source_kind)
            .and_then(|route| route.blocker.as_deref())
    }
}

/// Ordered assessment of a canonical source tree.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BootstrapAssessment {
    sources: Vec<BootstrapSourceAssessment>,
}

impl BootstrapAssessment {
    pub fn sources(&self) -> &[BootstrapSourceAssessment] {
        &self.sources
    }

    pub fn is_ready(&self) -> bool {
        self.sources
            .iter()
            .all(|source| source.outcome == BootstrapOutcome::Compiled)
    }

    fn blockers(&self) -> Vec<BootstrapReadinessBlocker> {
        self.sources
            .iter()
            .filter(|source| source.outcome != BootstrapOutcome::Compiled)
            .map(|source| BootstrapReadinessBlocker {
                source_index: source.source_index,
                source_kind: source.source_kind,
                outcome: source.outcome,
            })
            .collect()
    }
}

/// One manifest-declared blocker, kept in exact source-tree order.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BootstrapReadinessBlocker {
    pub source_index: usize,
    pub source_kind: BootstrapSourceKind,
    pub outcome: BootstrapOutcome,
}

impl BootstrapReadinessBlocker {
    pub const fn route_id(&self) -> &'static str {
        self.source_kind.as_str()
    }

    pub fn source_path<'a>(&self, source_tree: &'a SourceTree) -> Option<&'a str> {
        source_tree
            .entries()
            .get(self.source_index)
            .map(|source| source.path().as_str())
    }

    pub fn reason<'a>(&self, manifest: &'a BootstrapManifest) -> Option<&'a str> {
        manifest
            .route(self.source_kind)
            .and_then(|route| route.blocker.as_deref())
    }
}

/// Complete blocker set produced by conservative readiness assessment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BootstrapReadinessError {
    blockers: Vec<BootstrapReadinessBlocker>,
}

impl BootstrapReadinessError {
    pub fn blockers(&self) -> &[BootstrapReadinessBlocker] {
        &self.blockers
    }
}

impl Display for BootstrapReadinessError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "bootstrap readiness found {} blocking source artifacts",
            self.blockers.len()
        )
    }
}

impl Error for BootstrapReadinessError {}

/// Distinct manifest, readiness, and neutral patch failures for the build gate.
#[derive(Debug)]
pub enum BootstrapBuildError {
    Manifest(BootstrapManifestError),
    MissingConfigurationRoot { actual: usize },
    Readiness(BootstrapReadinessError),
    Patch(StoragePatchPreflightError),
}

impl Display for BootstrapBuildError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Manifest(source) => write!(formatter, "invalid bootstrap manifest: {source}"),
            Self::MissingConfigurationRoot { actual } => write!(
                formatter,
                "bootstrap source tree must contain exactly one ConfigurationRoot, found {actual}"
            ),
            Self::Readiness(source) => Display::fmt(source, formatter),
            Self::Patch(source) => write!(formatter, "bootstrap patch is blocked: {source}"),
        }
    }
}

impl Error for BootstrapBuildError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Manifest(source) => Some(source),
            Self::MissingConfigurationRoot { .. } => None,
            Self::Readiness(source) => Some(source),
            Self::Patch(source) => Some(source),
        }
    }
}

/// Blocks bootstrap until both source readiness and the neutral patch pass.
///
/// Patch outcome inspection is deliberately delegated to
/// [`StoragePatch::preflight`]; this layer never duplicates blocker rules.
pub fn preflight_bootstrap_build(
    manifest: &BootstrapManifest,
    source_tree: &SourceTree,
    patch: &StoragePatch,
) -> Result<BootstrapAssessment, BootstrapBuildError> {
    manifest.validate().map_err(BootstrapBuildError::Manifest)?;
    let configuration_roots = source_tree
        .entries()
        .iter()
        .filter(|source| source.kind() == SourceKind::ConfigurationRoot)
        .count();
    if configuration_roots != 1 {
        return Err(BootstrapBuildError::MissingConfigurationRoot {
            actual: configuration_roots,
        });
    }
    let assessment = manifest
        .assess(source_tree)
        .map_err(BootstrapBuildError::Manifest)?;
    let blockers = assessment.blockers();
    if !blockers.is_empty() {
        return Err(BootstrapBuildError::Readiness(BootstrapReadinessError {
            blockers,
        }));
    }
    patch.preflight().map_err(BootstrapBuildError::Patch)?;
    Ok(assessment)
}

/// Schema or route-catalog failure. It is intentionally distinct from patch
/// preflight so callers cannot mistake missing coverage for a codec blocker.
#[derive(Debug)]
pub enum BootstrapManifestError {
    ManifestTooLarge { maximum: usize, actual: usize },
    InvalidJson(serde_json::Error),
    UnsupportedSchemaVersion { expected: u32, actual: u32 },
    DuplicateSourceKind(BootstrapSourceKind),
    MissingSourceKind(BootstrapSourceKind),
    DuplicateInventoryId(String),
    MissingInventorySourceKind(BootstrapSourceKind),
    MissingRequiredInventoryId(String),
    InvalidDeclaration { location: String, message: String },
}

impl Display for BootstrapManifestError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::ManifestTooLarge { maximum, actual } => write!(
                formatter,
                "manifest is {actual} bytes, exceeding the {maximum}-byte limit"
            ),
            Self::InvalidJson(source) => write!(formatter, "invalid JSON: {source}"),
            Self::UnsupportedSchemaVersion { expected, actual } => write!(
                formatter,
                "unsupported schema_version {actual}; expected {expected}"
            ),
            Self::DuplicateSourceKind(kind) => {
                write!(formatter, "duplicate canonical route for {kind}")
            }
            Self::MissingSourceKind(kind) => {
                write!(formatter, "missing canonical route for {kind}")
            }
            Self::DuplicateInventoryId(id) => {
                write!(formatter, "duplicate legacy inventory id `{id}`")
            }
            Self::MissingInventorySourceKind(kind) => {
                write!(formatter, "legacy inventory does not cover {kind}")
            }
            Self::MissingRequiredInventoryId(id) => {
                write!(
                    formatter,
                    "legacy inventory is missing required route `{id}`"
                )
            }
            Self::InvalidDeclaration { location, message } => {
                write!(formatter, "invalid {location}: {message}")
            }
        }
    }
}

impl Error for BootstrapManifestError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidJson(source) => Some(source),
            _ => None,
        }
    }
}

fn validate_identifier(value: &str, location: &str) -> Result<(), BootstrapManifestError> {
    validate_text(value, location)?;
    if !value.bytes().all(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
    }) {
        return Err(BootstrapManifestError::InvalidDeclaration {
            location: location.to_string(),
            message: "must contain only lowercase ASCII letters, digits, '-' or '_'".to_string(),
        });
    }
    Ok(())
}

fn validate_text(value: &str, location: &str) -> Result<(), BootstrapManifestError> {
    if value.trim().is_empty() || value.len() > MAX_TEXT_BYTES {
        return Err(BootstrapManifestError::InvalidDeclaration {
            location: location.to_string(),
            message: format!("must contain 1..={MAX_TEXT_BYTES} non-whitespace bytes"),
        });
    }
    Ok(())
}

fn validate_declaration(
    location: &str,
    expected_entries: &[String],
    codec: &str,
    outcome: BootstrapOutcome,
    blocker: Option<&str>,
    base_reads: &[BootstrapBaseRead],
) -> Result<(), BootstrapManifestError> {
    if expected_entries.is_empty() || expected_entries.len() > MAX_EXPECTED_ENTRIES {
        return Err(BootstrapManifestError::InvalidDeclaration {
            location: format!("{location} expected_entries"),
            message: format!("must contain 1..={MAX_EXPECTED_ENTRIES} entries"),
        });
    }
    let mut unique_entries = BTreeSet::new();
    for entry in expected_entries {
        validate_text(entry, &format!("{location} expected entry"))?;
        if !unique_entries.insert(entry.as_str()) {
            return Err(BootstrapManifestError::InvalidDeclaration {
                location: format!("{location} expected_entries"),
                message: format!("duplicate entry `{entry}`"),
            });
        }
    }
    validate_text(codec, &format!("{location} codec"))?;
    if base_reads.len() > MAX_BASE_READS {
        return Err(BootstrapManifestError::InvalidDeclaration {
            location: format!("{location} base_reads"),
            message: format!("exceeds {MAX_BASE_READS} records"),
        });
    }
    for read in base_reads {
        validate_text(&read.key, &format!("{location} base read key"))?;
        validate_text(&read.purpose, &format!("{location} base read purpose"))?;
        if read.mode == BootstrapBaseReadMode::OptionalHint
            && outcome != BootstrapOutcome::Unsupported
        {
            return Err(BootstrapManifestError::InvalidDeclaration {
                location: format!("{location} base_reads"),
                message: "optional_hint is allowed only for Unsupported declarations".to_string(),
            });
        }
    }

    match outcome {
        BootstrapOutcome::Compiled => {
            if blocker.is_some() {
                return Err(BootstrapManifestError::InvalidDeclaration {
                    location: format!("{location} blocker"),
                    message: "Compiled declarations cannot have a blocker".to_string(),
                });
            }
            if !base_reads.is_empty() {
                return Err(BootstrapManifestError::InvalidDeclaration {
                    location: format!("{location} base_reads"),
                    message: "Compiled declarations cannot read a base".to_string(),
                });
            }
        }
        BootstrapOutcome::NeedsBase => {
            validate_text(
                blocker.ok_or_else(|| BootstrapManifestError::InvalidDeclaration {
                    location: format!("{location} blocker"),
                    message: "NeedsBase declarations require a blocker".to_string(),
                })?,
                &format!("{location} blocker"),
            )?;
            if !base_reads
                .iter()
                .any(|read| read.mode == BootstrapBaseReadMode::Required)
            {
                return Err(BootstrapManifestError::InvalidDeclaration {
                    location: format!("{location} base_reads"),
                    message: "NeedsBase declarations require at least one required read"
                        .to_string(),
                });
            }
        }
        BootstrapOutcome::Unsupported => validate_text(
            blocker.ok_or_else(|| BootstrapManifestError::InvalidDeclaration {
                location: format!("{location} blocker"),
                message: "Unsupported declarations require a blocker".to_string(),
            })?,
            &format!("{location} blocker"),
        )?,
    }
    Ok(())
}

fn validate_codec_capability(record: &LegacyInventoryRecord) -> Result<(), BootstrapManifestError> {
    if record.codec_capability == BootstrapOutcome::NeedsBase
        && !record
            .base_reads
            .iter()
            .any(|read| read.mode == BootstrapBaseReadMode::Required)
    {
        return Err(BootstrapManifestError::InvalidDeclaration {
            location: format!("legacy inventory {} base_reads", record.id),
            message: "a NeedsBase codec capability requires at least one required read".to_string(),
        });
    }
    if record.codec_capability == BootstrapOutcome::NeedsBase
        && record.outcome == BootstrapOutcome::Compiled
    {
        return Err(BootstrapManifestError::InvalidDeclaration {
            location: format!("legacy inventory {} outcome", record.id),
            message: "an enforceable Compiled outcome cannot exceed a NeedsBase codec capability"
                .to_string(),
        });
    }
    if record.codec_capability == BootstrapOutcome::Unsupported
        && record.outcome != BootstrapOutcome::Unsupported
    {
        return Err(BootstrapManifestError::InvalidDeclaration {
            location: format!("legacy inventory {} outcome", record.id),
            message: "an unsupported codec capability requires an Unsupported enforceable outcome"
                .to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use ibcmd_core::storage::{
        MultipartIdentity, StorageKey, StoragePatchEntry, StoragePatchOutcome, StoragePatchTarget,
        StorageProvenance,
    };
    use ibcmd_xml::source_tree::{SourceEntry, SourcePath};

    use super::*;

    fn module_tree(paths: &[&str]) -> SourceTree {
        SourceTree::new(
            paths
                .iter()
                .map(|path| {
                    SourceEntry::from_bytes(
                        SourcePath::new(path).unwrap(),
                        b"Procedure Run()\nEndProcedure".to_vec(),
                    )
                    .unwrap()
                })
                .collect(),
        )
        .unwrap()
    }

    fn full_tree(module_paths: &[&str]) -> SourceTree {
        let mut entries = vec![
            SourceEntry::from_bytes(
                SourcePath::new("Configuration.xml").unwrap(),
                br#"<MetaDataObject><Configuration uuid="aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa"/></MetaDataObject>"#
                    .to_vec(),
            )
            .unwrap(),
        ];
        entries.extend(module_paths.iter().map(|path| {
            SourceEntry::from_bytes(
                SourcePath::new(path).unwrap(),
                b"Procedure Run()\nEndProcedure".to_vec(),
            )
            .unwrap()
        }));
        SourceTree::new(entries).unwrap()
    }

    fn all_compiled_manifest() -> BootstrapManifest {
        let mut manifest = bundled_manifest().unwrap();
        for route in &mut manifest.routes {
            route.outcome = BootstrapOutcome::Compiled;
            route.blocker = None;
            route.base_reads.clear();
        }
        manifest.validate().unwrap();
        manifest
    }

    fn target(key: &str) -> StoragePatchTarget {
        StoragePatchTarget::new(
            StorageKey::new(key).unwrap(),
            MultipartIdentity::single(),
            StorageProvenance::new(&format!("source/{key}")).unwrap(),
        )
    }

    #[test]
    fn bundled_manifest_is_strict_complete_and_conservative() {
        let manifest = bundled_manifest().unwrap();

        assert_eq!(manifest.schema_version, BOOTSTRAP_SCHEMA_VERSION);
        assert_eq!(manifest.routes.len(), BootstrapSourceKind::ALL.len());
        assert!(!manifest.legacy_inventory.is_empty());
        for kind in BootstrapSourceKind::ALL {
            assert_eq!(
                manifest
                    .routes
                    .iter()
                    .filter(|route| route.source_kind == kind)
                    .count(),
                1,
                "{kind}"
            );
        }
        assert!(
            manifest
                .routes
                .iter()
                .all(|route| route.outcome != BootstrapOutcome::Compiled)
        );
    }

    #[test]
    fn canonical_source_kind_mapping_is_exhaustive() {
        let expected = [
            (
                SourceKind::ConfigurationRoot,
                BootstrapSourceKind::ConfigurationRoot,
            ),
            (SourceKind::MetadataXml, BootstrapSourceKind::MetadataXml),
            (SourceKind::Module, BootstrapSourceKind::Module),
            (SourceKind::Form, BootstrapSourceKind::Form),
            (SourceKind::Template, BootstrapSourceKind::Template),
            (SourceKind::Binary, BootstrapSourceKind::Binary),
            (SourceKind::OtherXml, BootstrapSourceKind::OtherXml),
            (SourceKind::Other, BootstrapSourceKind::Other),
        ];
        assert_eq!(expected.len(), BootstrapSourceKind::ALL.len());
        for (canonical, manifest) in expected {
            assert_eq!(BootstrapSourceKind::from_canonical(canonical), manifest);
        }
    }

    #[test]
    fn assessment_preserves_canonical_source_tree_order() {
        let tree = module_tree(&["Zeta.bsl", "Alpha.bsl"]);
        let assessment = bundled_manifest().unwrap().assess(&tree).unwrap();

        assert_eq!(
            assessment
                .sources()
                .iter()
                .map(|source| source.source_path(&tree).unwrap())
                .collect::<Vec<_>>(),
            ["Alpha.bsl", "Zeta.bsl"]
        );
        assert!(assessment.sources().iter().all(|source| {
            source.source_kind == BootstrapSourceKind::Module
                && source.outcome == BootstrapOutcome::Unsupported
        }));
    }

    #[test]
    fn form_module_contributor_cannot_open_the_coarse_module_gate() {
        let tree = full_tree(&["CommonForms/Main/Ext/Form/Module.bsl"]);
        let patch = StoragePatch::new(Vec::new()).unwrap();
        let error = preflight_bootstrap_build(&bundled_manifest().unwrap(), &tree, &patch)
            .expect_err("bundled Module route must remain fail-closed");

        let BootstrapBuildError::Readiness(error) = error else {
            panic!("expected readiness blocker");
        };
        let blocker = error
            .blockers()
            .iter()
            .find(|blocker| blocker.source_kind == BootstrapSourceKind::Module)
            .unwrap();
        assert_eq!(blocker.outcome, BootstrapOutcome::Unsupported);
        assert_eq!(
            blocker.source_path(&tree),
            Some("CommonForms/Main/Ext/Form/Module.bsl")
        );
        let manifest = bundled_manifest().unwrap();
        assert!(
            blocker
                .reason(&manifest)
                .unwrap()
                .contains("owner/suffix resolution")
        );
    }

    #[test]
    fn missing_duplicate_and_unknown_routes_fail_closed() {
        let mut missing = bundled_manifest().unwrap();
        missing
            .routes
            .retain(|route| route.source_kind != BootstrapSourceKind::Other);
        assert!(matches!(
            missing.validate(),
            Err(BootstrapManifestError::MissingSourceKind(
                BootstrapSourceKind::Other
            ))
        ));

        let mut duplicate = bundled_manifest().unwrap();
        duplicate.routes.push(duplicate.routes[0].clone());
        assert!(matches!(
            duplicate.validate(),
            Err(BootstrapManifestError::DuplicateSourceKind(_))
        ));

        let unknown = r#"{
          "schema_version": 1,
          "routes": [{
            "source_kind": "future_source_kind",
            "expected_entries": ["future"],
            "codec": "future",
            "outcome": "unsupported",
            "blocker": "future source kind",
            "base_reads": []
          }],
          "legacy_inventory": []
        }"#;
        assert!(matches!(
            BootstrapManifest::from_json(unknown),
            Err(BootstrapManifestError::InvalidJson(_))
        ));
    }

    #[test]
    fn declaration_invariants_reject_fail_open_catalog_data() {
        let mut compiled_with_read = bundled_manifest().unwrap();
        let route = compiled_with_read
            .routes
            .iter_mut()
            .find(|route| route.source_kind == BootstrapSourceKind::MetadataXml)
            .unwrap();
        route.outcome = BootstrapOutcome::Compiled;
        route.blocker = None;
        assert!(matches!(
            compiled_with_read.validate(),
            Err(BootstrapManifestError::InvalidDeclaration { .. })
        ));

        let mut needs_base_without_required = bundled_manifest().unwrap();
        let route = needs_base_without_required
            .routes
            .iter_mut()
            .find(|route| route.source_kind == BootstrapSourceKind::Form)
            .unwrap();
        route.base_reads.clear();
        assert!(matches!(
            needs_base_without_required.validate(),
            Err(BootstrapManifestError::InvalidDeclaration { .. })
        ));

        let mut optional_compiled = bundled_manifest().unwrap();
        let route = optional_compiled
            .routes
            .iter_mut()
            .find(|route| route.source_kind == BootstrapSourceKind::Template)
            .unwrap();
        route.outcome = BootstrapOutcome::Compiled;
        route.blocker = None;
        assert!(matches!(
            optional_compiled.validate(),
            Err(BootstrapManifestError::InvalidDeclaration { .. })
        ));
    }

    #[test]
    fn needs_base_capability_requires_a_required_inventory_read() {
        let mut manifest = bundled_manifest().unwrap();
        let record = manifest
            .legacy_inventory
            .iter_mut()
            .find(|record| record.codec_capability == BootstrapOutcome::NeedsBase)
            .unwrap();
        record.outcome = BootstrapOutcome::Unsupported;
        record.base_reads.clear();

        let error = manifest.validate().unwrap_err();
        assert!(
            error
                .to_string()
                .contains("NeedsBase codec capability requires at least one required read")
        );
    }

    #[test]
    fn mandatory_legacy_inventory_routes_cannot_be_omitted() {
        let mut manifest = bundled_manifest().unwrap();
        manifest
            .legacy_inventory
            .retain(|record| record.id != "spreadsheet_document_bypass");

        assert!(matches!(
            manifest.validate(),
            Err(BootstrapManifestError::MissingRequiredInventoryId(id))
                if id == "spreadsheet_document_bypass"
        ));
    }

    #[test]
    fn strict_json_rejects_unknown_manifest_fields() {
        let json = r#"{
          "schema_version": 1,
          "routes": [],
          "legacy_inventory": [],
          "future_field": true
        }"#;
        assert!(matches!(
            BootstrapManifest::from_json(json),
            Err(BootstrapManifestError::InvalidJson(_))
        ));
    }

    #[test]
    fn oversized_manifest_is_rejected_before_json_parsing() {
        let json = " ".repeat(MAX_BOOTSTRAP_MANIFEST_BYTES + 1);
        assert!(matches!(
            BootstrapManifest::from_json(&json),
            Err(BootstrapManifestError::ManifestTooLarge {
                maximum: MAX_BOOTSTRAP_MANIFEST_BYTES,
                actual
            }) if actual == MAX_BOOTSTRAP_MANIFEST_BYTES + 1
        ));
    }

    #[test]
    fn build_gate_wraps_typed_patch_preflight_without_reclassifying_it() {
        let manifest = all_compiled_manifest();
        let tree = full_tree(&["CommonModules/Tools/Ext/Module.bsl"]);
        let patch = StoragePatch::new(vec![
            StoragePatchEntry::new(
                target("first"),
                StoragePatchOutcome::needs_base(
                    StorageKey::new("base-first").unwrap(),
                    "first needs its base",
                )
                .unwrap(),
            ),
            StoragePatchEntry::new(
                target("second"),
                StoragePatchOutcome::unsupported("second is unsupported").unwrap(),
            ),
        ])
        .unwrap();

        let error = preflight_bootstrap_build(&manifest, &tree, &patch).unwrap_err();
        let BootstrapBuildError::Patch(error) = error else {
            panic!("expected wrapped StoragePatchPreflightError");
        };
        assert_eq!(error.blockers().len(), 2);
        assert_eq!(error.blockers()[0].target().key().as_str(), "first");
        assert_eq!(error.blockers()[1].target().key().as_str(), "second");
    }

    #[test]
    fn all_compiled_synthetic_manifest_and_patch_pass_the_gate() {
        let manifest = all_compiled_manifest();
        let tree = full_tree(&["CommonModules/Tools/Ext/Module.bsl"]);
        let patch = StoragePatch::new(vec![StoragePatchEntry::new(
            target("module.0"),
            StoragePatchOutcome::compiled(b"payload".to_vec()).unwrap(),
        )])
        .unwrap();

        let assessment = preflight_bootstrap_build(&manifest, &tree, &patch).unwrap();
        assert!(assessment.is_ready());
        assert_eq!(assessment.sources().len(), 2);
    }

    #[test]
    fn full_build_gate_rejects_empty_and_partial_source_trees() {
        let manifest = all_compiled_manifest();
        let patch = StoragePatch::new(Vec::new()).unwrap();
        let empty = SourceTree::new(Vec::new()).unwrap();
        assert!(matches!(
            preflight_bootstrap_build(&manifest, &empty, &patch),
            Err(BootstrapBuildError::MissingConfigurationRoot { actual: 0 })
        ));

        let partial = module_tree(&["CommonModules/Tools/Ext/Module.bsl"]);
        assert!(matches!(
            preflight_bootstrap_build(&manifest, &partial, &patch),
            Err(BootstrapBuildError::MissingConfigurationRoot { actual: 0 })
        ));
    }
}
