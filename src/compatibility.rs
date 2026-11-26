//! Evidence-derived compatibility matrix for standalone conversion routes.
//!
//! The checked-in matrix is the only source used by the library and CLI. A
//! route can be `verified` only when it references at least one green Cargo
//! test. Unknown profiles and routes without an exact matrix record are always
//! reported as unsupported.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path};

use anyhow::{Context, Result, anyhow, bail};
use ibcmd_core::artifact::ProfileId;
use ibcmd_core::profile::ProfileRegistry;
use serde::{Deserialize, Serialize};

use crate::profile_registry::load_bundled_profile_registry;

/// Version of the compatibility-matrix JSON contract.
pub const COMPATIBILITY_SCHEMA_VERSION: u32 = 1;
/// Matrix embedded into every standalone binary.
pub const BUNDLED_COMPATIBILITY_MATRIX_JSON: &str = include_str!("../compatibility/matrix.json");
/// JSON Schema shipped alongside the matrix for external consumers.
pub const BUNDLED_COMPATIBILITY_SCHEMA_JSON: &str =
    include_str!("../compatibility/matrix.schema.json");

const MAX_EVIDENCE: usize = 4_096;
const MAX_ROUTES: usize = 4_096;
const MAX_ROUTE_EVIDENCE: usize = 64;

/// Complete evidence catalogue and exact route matrix.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CompatibilityMatrix {
    pub schema_version: u32,
    pub generated_from: String,
    pub evidence: Vec<CompatibilityEvidence>,
    pub routes: Vec<CompatibilityRoute>,
}

/// One repository-owned evidence item referenced by matrix routes.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CompatibilityEvidence {
    pub id: String,
    pub kind: EvidenceKind,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub case: Option<String>,
    pub state: EvidenceState,
}

/// Evidence classes understood by the standalone validator.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    CargoTest,
    Document,
}

/// Whether evidence is executable proof or supporting context.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceState {
    Green,
    Context,
}

/// One exact operation × artifact × profile × family compatibility record.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CompatibilityRoute {
    pub id: String,
    pub operation: String,
    pub source_artifact: String,
    pub target_artifact: String,
    pub source_profile: String,
    pub target_profile: String,
    pub family: String,
    pub support: RouteSupport,
    pub status: VerificationStatus,
    pub preservation: PreservationClaim,
    pub evidence: Vec<String>,
    pub detail: String,
}

/// Whether the exact route can currently execute.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RouteSupport {
    Supported,
    Unsupported,
}

/// Strength of the evidence behind one exact route.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    Experimental,
    Verified,
}

/// Strongest preservation behavior claimed by the exact route.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreservationClaim {
    None,
    Semantic,
}

/// Exact coordinates for a fail-closed compatibility lookup.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompatibilityQuery<'a> {
    pub operation: &'a str,
    pub source_artifact: &'a str,
    pub target_artifact: &'a str,
    pub source_profile: &'a str,
    pub target_profile: &'a str,
    pub family: &'a str,
}

/// Result of an exact compatibility lookup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompatibilityLookup<'a> {
    Match(&'a CompatibilityRoute),
    Unsupported(UnsupportedReason),
}

/// Stable reason for a fail-closed lookup result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnsupportedReason {
    UnknownSourceProfile,
    UnknownTargetProfile,
    NoExactRoute,
}

impl CompatibilityMatrix {
    /// Looks up only an exact route after proving both profiles are registered.
    pub fn lookup<'a>(
        &'a self,
        query: &CompatibilityQuery<'_>,
        profiles: &ProfileRegistry,
    ) -> CompatibilityLookup<'a> {
        if !known_profile(query.source_profile, profiles) {
            return CompatibilityLookup::Unsupported(UnsupportedReason::UnknownSourceProfile);
        }
        if !known_profile(query.target_profile, profiles) {
            return CompatibilityLookup::Unsupported(UnsupportedReason::UnknownTargetProfile);
        }

        self.routes
            .iter()
            .find(|route| {
                route.operation == query.operation
                    && route.source_artifact == query.source_artifact
                    && route.target_artifact == query.target_artifact
                    && route.source_profile == query.source_profile
                    && route.target_profile == query.target_profile
                    && route.family == query.family
            })
            .map(CompatibilityLookup::Match)
            .unwrap_or(CompatibilityLookup::Unsupported(
                UnsupportedReason::NoExactRoute,
            ))
    }
}

/// Parses strict matrix JSON. Semantic validation is intentionally separate so
/// tests and generators can validate a modified document against a registry.
pub fn parse_compatibility_matrix(json: &str) -> Result<CompatibilityMatrix> {
    serde_json::from_str(json).context("failed to parse compatibility matrix JSON")
}

/// Loads and validates the matrix compiled into the current binary.
pub fn current_compatibility_report() -> Result<CompatibilityMatrix> {
    let matrix = parse_compatibility_matrix(BUNDLED_COMPATIBILITY_MATRIX_JSON)?;
    let profiles = load_bundled_profile_registry()?;
    validate_compatibility_matrix(&matrix, &profiles)?;
    Ok(matrix)
}

/// Validates determinism, exact profile references, and evidence-derived
/// verification status without requiring a source checkout at runtime.
pub fn validate_compatibility_matrix(
    matrix: &CompatibilityMatrix,
    profiles: &ProfileRegistry,
) -> Result<()> {
    if matrix.schema_version != COMPATIBILITY_SCHEMA_VERSION {
        bail!(
            "unsupported compatibility schema version {} (expected {})",
            matrix.schema_version,
            COMPATIBILITY_SCHEMA_VERSION
        );
    }
    if matrix.generated_from != "repository_evidence" {
        bail!("compatibility matrix must declare generated_from=repository_evidence");
    }
    if matrix.evidence.len() > MAX_EVIDENCE {
        bail!(
            "compatibility evidence count {} exceeds limit {MAX_EVIDENCE}",
            matrix.evidence.len()
        );
    }
    if matrix.routes.len() > MAX_ROUTES {
        bail!(
            "compatibility route count {} exceeds limit {MAX_ROUTES}",
            matrix.routes.len()
        );
    }

    require_sorted_unique(
        matrix.evidence.iter().map(|item| item.id.as_str()),
        "evidence IDs",
    )?;
    require_sorted_unique(
        matrix.routes.iter().map(|route| route.id.as_str()),
        "route IDs",
    )?;

    let mut evidence_by_id = BTreeMap::new();
    for item in &matrix.evidence {
        validate_open_id(&item.id, "evidence ID")?;
        validate_evidence_path(&item.path)?;
        match (item.kind, item.state, item.case.as_deref()) {
            (EvidenceKind::CargoTest, EvidenceState::Green, Some(case)) => {
                validate_rust_identifier(case)?;
                if !item.path.starts_with("tests/") || !item.path.ends_with(".rs") {
                    bail!(
                        "green Cargo test evidence `{}` must reference tests/*.rs",
                        item.id
                    );
                }
            }
            (EvidenceKind::Document, EvidenceState::Context, None) => {
                if !item.path.starts_with("docs/") || !item.path.ends_with(".md") {
                    bail!(
                        "document evidence `{}` must reference a docs/*.md file",
                        item.id
                    );
                }
            }
            _ => bail!(
                "evidence `{}` has an invalid kind/state/case combination",
                item.id
            ),
        }
        evidence_by_id.insert(item.id.as_str(), item);
    }

    let mut referenced_evidence = BTreeSet::new();
    let mut coordinates = BTreeSet::new();
    for route in &matrix.routes {
        validate_open_id(&route.id, "route ID")?;
        for (label, value) in [
            ("operation", route.operation.as_str()),
            ("source artifact", route.source_artifact.as_str()),
            ("target artifact", route.target_artifact.as_str()),
            ("family", route.family.as_str()),
        ] {
            validate_open_id(value, label)?;
        }
        require_known_profile(&route.source_profile, "source", profiles)?;
        require_known_profile(&route.target_profile, "target", profiles)?;
        if route.detail.trim().is_empty() {
            bail!("compatibility route `{}` has an empty detail", route.id);
        }
        if route.evidence.len() > MAX_ROUTE_EVIDENCE {
            bail!(
                "compatibility route `{}` references more than {MAX_ROUTE_EVIDENCE} evidence items",
                route.id
            );
        }
        require_sorted_unique(
            route.evidence.iter().map(String::as_str),
            &format!("evidence references for route `{}`", route.id),
        )?;

        let coordinate = (
            route.operation.as_str(),
            route.source_artifact.as_str(),
            route.target_artifact.as_str(),
            route.source_profile.as_str(),
            route.target_profile.as_str(),
            route.family.as_str(),
        );
        if !coordinates.insert(coordinate) {
            bail!(
                "duplicate compatibility coordinates at route `{}`",
                route.id
            );
        }

        let mut has_green_test = false;
        for evidence_id in &route.evidence {
            let item = evidence_by_id.get(evidence_id.as_str()).ok_or_else(|| {
                anyhow!(
                    "compatibility route `{}` references unknown evidence `{evidence_id}`",
                    route.id
                )
            })?;
            referenced_evidence.insert(evidence_id.as_str());
            has_green_test |=
                item.kind == EvidenceKind::CargoTest && item.state == EvidenceState::Green;
        }

        let derived_status = if route.support == RouteSupport::Supported && has_green_test {
            VerificationStatus::Verified
        } else {
            VerificationStatus::Experimental
        };
        if route.status != derived_status {
            bail!(
                "compatibility route `{}` claims {:?}, but its evidence derives {:?}",
                route.id,
                route.status,
                derived_status
            );
        }
        if route.support == RouteSupport::Unsupported
            && route.preservation != PreservationClaim::None
        {
            bail!(
                "unsupported compatibility route `{}` cannot claim preservation",
                route.id
            );
        }
    }

    if referenced_evidence.len() != matrix.evidence.len() {
        let unused = matrix
            .evidence
            .iter()
            .filter(|item| !referenced_evidence.contains(item.id.as_str()))
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>();
        bail!("compatibility matrix contains unreferenced evidence: {unused:?}");
    }

    Ok(())
}

/// Confirms that every referenced evidence file and Cargo test case still
/// exists in a repository checkout. CI runs the referenced tests normally;
/// this check prevents stale or invented green links from entering the matrix.
pub fn validate_repository_evidence(matrix: &CompatibilityMatrix, root: &Path) -> Result<()> {
    for item in &matrix.evidence {
        validate_evidence_path(&item.path)?;
        let path = root.join(&item.path);
        let contents = fs::read_to_string(&path).with_context(|| {
            format!(
                "failed to read compatibility evidence `{}` at {}",
                item.id,
                path.display()
            )
        })?;
        if item.kind == EvidenceKind::CargoTest {
            let case = item
                .case
                .as_deref()
                .ok_or_else(|| anyhow!("Cargo test evidence `{}` has no test case", item.id))?;
            let signature = format!("fn {case}(");
            let Some(signature_offset) = contents.find(&signature) else {
                bail!(
                    "green evidence `{}` references missing Cargo test `{case}`",
                    item.id
                );
            };
            let prefix = &contents[..signature_offset];
            let Some(test_attribute) = prefix.rfind("#[test]") else {
                bail!(
                    "green evidence `{}` case `{case}` is not marked #[test]",
                    item.id
                );
            };
            if prefix[test_attribute..].contains("fn ") {
                bail!(
                    "green evidence `{}` case `{case}` is not the test following #[test]",
                    item.id
                );
            }
        }
    }
    Ok(())
}

/// Writes the exact validated source-of-truth document emitted by the CLI.
pub fn write_compatibility_report(report: &CompatibilityMatrix, output: &Path) -> Result<()> {
    let json = serde_json::to_string_pretty(report)?;
    fs::write(output, json).with_context(|| format!("failed to write {}", output.display()))
}

fn known_profile(value: &str, profiles: &ProfileRegistry) -> bool {
    ProfileId::parse(value)
        .ok()
        .and_then(|id| profiles.get(&id))
        .is_some()
}

fn require_known_profile(value: &str, endpoint: &str, profiles: &ProfileRegistry) -> Result<()> {
    let id = ProfileId::parse(value)
        .with_context(|| format!("invalid {endpoint} profile `{value}` in compatibility matrix"))?;
    if profiles.get(&id).is_none() {
        bail!("unknown {endpoint} profile `{value}` in compatibility matrix");
    }
    Ok(())
}

fn require_sorted_unique<'a>(values: impl IntoIterator<Item = &'a str>, label: &str) -> Result<()> {
    let mut previous = None;
    for value in values {
        if let Some(previous) = previous
            && previous >= value
        {
            bail!("{label} must be strictly sorted and unique: `{previous}` then `{value}`");
        }
        previous = Some(value);
    }
    Ok(())
}

fn validate_open_id(value: &str, label: &str) -> Result<()> {
    ProfileId::parse(value).with_context(|| format!("invalid {label} `{value}`"))?;
    Ok(())
}

fn validate_rust_identifier(value: &str) -> Result<()> {
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        bail!("Cargo test case is empty");
    };
    if !(first.is_ascii_alphabetic() || first == b'_')
        || !bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        bail!("invalid Cargo test case `{value}`");
    }
    Ok(())
}

fn validate_evidence_path(value: &str) -> Result<()> {
    let path = Path::new(value);
    if value.contains('\\')
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        bail!("evidence path `{value}` must be a safe slash-separated relative path");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_matrix_is_strict_and_evidence_derived() {
        let matrix = current_compatibility_report().unwrap();
        assert!(matrix.routes.iter().any(|route| {
            route.operation == "convert" && route.status == VerificationStatus::Verified
        }));
        assert!(matrix.routes.iter().any(|route| {
            route.support == RouteSupport::Unsupported
                && route.status == VerificationStatus::Experimental
        }));
    }

    #[test]
    fn verified_status_cannot_survive_without_green_test_evidence() {
        let mut matrix = current_compatibility_report().unwrap();
        let route = matrix
            .routes
            .iter_mut()
            .find(|route| route.status == VerificationStatus::Verified)
            .unwrap();
        route.evidence.retain(|id| !id.starts_with("test-"));

        let profiles = load_bundled_profile_registry().unwrap();
        let error = validate_compatibility_matrix(&matrix, &profiles).unwrap_err();
        assert!(error.to_string().contains("evidence derives Experimental"));
    }

    #[test]
    fn unknown_profile_lookup_fails_closed() {
        let matrix = current_compatibility_report().unwrap();
        let profiles = load_bundled_profile_registry().unwrap();
        let query = CompatibilityQuery {
            operation: "convert",
            source_artifact: "xml_source_tree",
            target_artifact: "cf_archive",
            source_profile: "xml-99.99",
            target_profile: "platform-8.3.27.1989",
            family: "CommonModule",
        };

        assert_eq!(
            matrix.lookup(&query, &profiles),
            CompatibilityLookup::Unsupported(UnsupportedReason::UnknownSourceProfile)
        );
    }
}
