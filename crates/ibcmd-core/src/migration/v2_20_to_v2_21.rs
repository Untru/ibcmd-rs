//! Verified zero-loss XCF 2.20 to 2.21 migration edge.
//!
//! The canonical semantics of the evidenced Constant and Catalog slices do
//! not change on this edge. Version-specific lexical work (the root version
//! and Catalog palette namespace) belongs to the target XML adapter. This
//! step proves that the model is inside that bounded family cohort before the
//! adapter is allowed to encode it for 2.21.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::adapter::AdapterOutcome;
use crate::artifact::ProfileId;
use crate::diagnostic::{
    Diagnostic, DiagnosticBuildError, DiagnosticCode, DiagnosticReport, ObjectPath, PathSegment,
    PropertyPath, Severity,
};
use crate::identity::ObjectUuid;
use crate::model::CanonicalObject;
use crate::profile::CapabilityId;

use super::compatibility::{
    CompatibilityRequirement, CompatibilityRequirements, analyze_compatibility,
};
use super::graph::{MigrationDirection, MigrationEdge, MigrationVerification};
use super::step::{
    MigrationAnalysis, MigrationAnalyzeRequest, MigrationApplyOutcome, MigrationApplyRequest,
    MigrationStep, MigrationStepDescriptor, MigrationStepId, MigrationStepOutput,
    ProfileConstraint,
};

/// Stable identifier of the verified upgrade edge.
pub const STEP_ID: &str = "migration:xcf-2.20-to-2.21";
/// Exact source profile accepted by the edge.
pub const SOURCE_PROFILE: &str = "xml-2.20";
/// Exact target profile emitted by the edge.
pub const TARGET_PROFILE: &str = "xml-2.21";

/// Capability proving the strict Constant family codec is available.
pub const CONSTANT_CAPABILITY: &str = "xcf:metadata-constant";
/// Capability proving the strict Catalog family codec is available.
pub const CATALOG_CAPABILITY: &str = "xcf:metadata-catalog";
/// Capability introduced by the evidenced 2.21 palette namespace delta.
pub const PALETTE_CAPABILITY: &str = "xcf:palette-namespace";
/// Capability introduced by the evidenced 2.21 configuration property.
pub const INTERFACE_MODE_CAPABILITY: &str = "xcf:use-in-interface-compatibility-mode";

/// Stable diagnostic for a model object outside the verified family cohort.
pub const UNKNOWN_DELTA_CODE: &str = "migration.xcf-2-20-to-2-21.unknown-delta";
/// Stable diagnostic for model provenance that does not belong to XCF 2.20.
pub const SOURCE_PROVENANCE_CODE: &str = "migration.xcf-2-20-to-2-21.source-provenance";

const ROOT_FAMILIES: &[&str] = &["Catalog", "Constant"];
const CATALOG_CHILD_FAMILIES: &[&str] = &["Attribute", "Command", "TabularSection"];

/// Zero-loss, evidence-bounded migration implementation.
#[derive(Clone, Debug)]
pub struct V2_20ToV2_21 {
    requirements: CompatibilityRequirements,
}

impl V2_20ToV2_21 {
    /// Builds the immutable hard-coded contract.
    pub fn new() -> Self {
        let constant = capability(CONSTANT_CAPABILITY);
        let catalog = capability(CATALOG_CAPABILITY);
        let palette = capability(PALETTE_CAPABILITY);
        let interface_mode = capability(INTERFACE_MODE_CAPABILITY);
        let source = ProfileConstraint::with_capabilities(
            profile(SOURCE_PROFILE),
            vec![constant.clone(), catalog.clone()],
        )
        .expect("built-in 2.20 capabilities are unique and bounded");
        let target = ProfileConstraint::with_capabilities(
            profile(TARGET_PROFILE),
            vec![
                constant.clone(),
                catalog.clone(),
                palette.clone(),
                interface_mode.clone(),
            ],
        )
        .expect("built-in 2.21 capabilities are unique and bounded");
        let descriptor = MigrationStepDescriptor::new(
            MigrationStepId::parse(STEP_ID).expect("built-in migration id is valid"),
            source,
            target,
            vec![
                constant.clone(),
                catalog.clone(),
                palette.clone(),
                interface_mode.clone(),
            ],
            Vec::new(),
        )
        .expect("built-in migration descriptor is valid");
        let requirements = CompatibilityRequirements::new(
            descriptor,
            vec![
                CompatibilityRequirement::global(constant),
                CompatibilityRequirement::global(catalog),
                CompatibilityRequirement::global(palette),
                CompatibilityRequirement::global(interface_mode),
            ],
        )
        .expect("built-in migration requirements cover every capability exactly once");
        Self { requirements }
    }

    /// Wraps this implementation as a verified directed upgrade edge.
    pub fn verified_edge() -> MigrationEdge {
        MigrationEdge::new(
            Arc::new(Self::new()),
            MigrationDirection::Upgrade,
            MigrationVerification::Verified,
        )
    }

    fn analyze_model(
        &self,
        request: MigrationAnalyzeRequest<'_>,
    ) -> Result<Vec<Diagnostic>, DiagnosticBuildError> {
        let objects_by_uuid = request
            .configuration()
            .objects()
            .iter()
            .map(|object| (object.identity().uuid(), object))
            .collect::<BTreeMap<_, _>>();
        let mut diagnostics = Vec::new();
        for object in request.configuration().objects() {
            let provenance_matches = object.provenance().source_profile().as_str()
                == SOURCE_PROFILE
                && object
                    .opaque_facets()
                    .as_slice()
                    .iter()
                    .all(|facet| facet.source_profile().as_str() == SOURCE_PROFILE);
            if !provenance_matches {
                diagnostics.push(source_provenance_diagnostic(request, object)?);
            }
            if !is_supported_object(object, &objects_by_uuid) {
                diagnostics.push(unknown_delta_diagnostic(request, object)?);
            }
        }
        Ok(diagnostics)
    }
}

impl Default for V2_20ToV2_21 {
    fn default() -> Self {
        Self::new()
    }
}

impl MigrationStep for V2_20ToV2_21 {
    fn descriptor(&self) -> &MigrationStepDescriptor {
        self.requirements.descriptor()
    }

    fn analyze(
        &self,
        request: MigrationAnalyzeRequest<'_>,
    ) -> Result<MigrationAnalysis, DiagnosticBuildError> {
        let compatibility = analyze_compatibility(&self.requirements, request)?;
        let mut diagnostics = compatibility.diagnostics().diagnostics().to_vec();
        diagnostics.extend(self.analyze_model(request)?);
        Ok(MigrationAnalysis::new(DiagnosticReport::from_diagnostics(
            diagnostics,
        )))
    }

    fn apply(&self, request: MigrationApplyRequest<'_>) -> MigrationApplyOutcome {
        let analysis = self.analyze(MigrationAnalyzeRequest::new(
            request.source_profile(),
            request.target_profile(),
            request.configuration(),
        ));
        match analysis {
            Ok(analysis) if analysis.is_compatible() => AdapterOutcome::success(
                MigrationStepOutput::new(request.configuration().clone(), Vec::new())
                    .expect("zero-loss migration output is bounded"),
            ),
            Ok(analysis) => AdapterOutcome::without_value(analysis.diagnostics().clone()),
            Err(error) => AdapterOutcome::without_value(DiagnosticReport::from_diagnostics(vec![
                internal_diagnostic(request, &error),
            ])),
        }
    }
}

fn is_supported_object(
    object: &CanonicalObject,
    objects: &BTreeMap<ObjectUuid, &CanonicalObject>,
) -> bool {
    if object.owner().is_none() {
        return ROOT_FAMILIES.contains(&object.kind().as_str());
    }
    if !CATALOG_CHILD_FAMILIES.contains(&object.kind().as_str()) {
        return false;
    }

    let mut owner = object.owner();
    let mut visited = BTreeSet::new();
    while let Some(owner_uuid) = owner {
        if !visited.insert(owner_uuid) {
            return false;
        }
        let Some(parent) = objects.get(&owner_uuid).copied() else {
            return false;
        };
        if parent.owner().is_none() {
            return parent.kind().as_str() == "Catalog";
        }
        if !CATALOG_CHILD_FAMILIES.contains(&parent.kind().as_str()) {
            return false;
        }
        owner = parent.owner();
    }
    false
}

fn source_provenance_diagnostic(
    request: MigrationAnalyzeRequest<'_>,
    object: &CanonicalObject,
) -> Result<Diagnostic, DiagnosticBuildError> {
    Diagnostic::new(
        diagnostic_code(SOURCE_PROVENANCE_CODE),
        Severity::Error,
        object.identity().path().clone(),
        property_path("provenance"),
        "canonical object does not carry exact XCF 2.20 source provenance",
    )
    .map(|diagnostic| with_profiles(diagnostic, request))?
    .with_recovery_hint("decode the object with the exact xml-2.20 adapter before migration")?
    .with_context("migration_step", STEP_ID)?
    .with_context(
        "object_source_profile",
        object.provenance().source_profile().as_str(),
    )
}

fn unknown_delta_diagnostic(
    request: MigrationAnalyzeRequest<'_>,
    object: &CanonicalObject,
) -> Result<Diagnostic, DiagnosticBuildError> {
    Diagnostic::new(
        diagnostic_code(UNKNOWN_DELTA_CODE),
        Severity::Error,
        object.identity().path().clone(),
        property_path("kind"),
        "canonical object is outside the verified XCF 2.20 to 2.21 family cohort",
    )
    .map(|diagnostic| with_profiles(diagnostic, request))?
    .with_recovery_hint(
        "add an evidence-backed family codec and migration fixture before extending this edge",
    )?
    .with_context("migration_step", STEP_ID)?
    .with_context("metadata_kind", object.kind().as_str())
}

fn internal_diagnostic(
    request: MigrationApplyRequest<'_>,
    error: &DiagnosticBuildError,
) -> Diagnostic {
    let reason = error.to_string();
    Diagnostic::new(
        diagnostic_code("migration.xcf-2-20-to-2-21.diagnostic-build-failed"),
        Severity::Error,
        ObjectPath::root(),
        PropertyPath::root(),
        "migration analysis could not build its bounded diagnostics",
    )
    .map(|diagnostic| {
        diagnostic.with_profiles(
            Some(request.source_profile().id.clone()),
            Some(request.target_profile().id.clone()),
        )
    })
    .and_then(|diagnostic| diagnostic.with_context("reason", &reason))
    .expect("built-in fallback diagnostic is bounded")
}

fn with_profiles(diagnostic: Diagnostic, request: MigrationAnalyzeRequest<'_>) -> Diagnostic {
    diagnostic.with_profiles(
        Some(request.source_profile().id.clone()),
        Some(request.target_profile().id.clone()),
    )
}

fn property_path(name: &str) -> PropertyPath {
    PropertyPath::new(vec![
        PathSegment::name(name).expect("built-in property path is valid"),
    ])
    .expect("built-in property path is bounded")
}

fn profile(value: &str) -> ProfileId {
    ProfileId::parse(value).expect("built-in profile id is valid")
}

fn capability(value: &str) -> CapabilityId {
    CapabilityId::parse(value).expect("built-in capability id is valid")
}

fn diagnostic_code(value: &str) -> DiagnosticCode {
    DiagnosticCode::parse(value).expect("built-in diagnostic code is valid")
}

#[cfg(test)]
mod tests {
    use crate::diagnostic::{LossPolicy, ObjectPath, PropertyPath};
    use crate::identity::{LogicalIdentity, ObjectUuid};
    use crate::model::{CanonicalConfiguration, CanonicalObjectParts, MetadataKind};
    use crate::profile::{
        EffectiveProfile, ProfileSourceKind, parse_profile_source, resolve_profiles,
    };
    use crate::provenance::{CanonicalAnchor, SourceProvenance};

    use super::*;

    fn profiles() -> (EffectiveProfile, EffectiveProfile) {
        let source = format!(
            r#"{{"schema_version":1,"id":"{SOURCE_PROFILE}","status":"experimental","capabilities":{{"{CONSTANT_CAPABILITY}":"supported","{CATALOG_CAPABILITY}":"supported","{PALETTE_CAPABILITY}":"unsupported","{INTERFACE_MODE_CAPABILITY}":"unsupported"}}}}"#
        );
        let target = format!(
            r#"{{"schema_version":1,"id":"{TARGET_PROFILE}","status":"experimental","capabilities":{{"{CONSTANT_CAPABILITY}":"supported","{CATALOG_CAPABILITY}":"supported","{PALETTE_CAPABILITY}":"supported","{INTERFACE_MODE_CAPABILITY}":"supported"}}}}"#
        );
        let registry = resolve_profiles([
            parse_profile_source("source.json", ProfileSourceKind::Bundled, &source).unwrap(),
            parse_profile_source("target.json", ProfileSourceKind::Bundled, &target).unwrap(),
        ])
        .unwrap();
        (
            registry.get(&profile(SOURCE_PROFILE)).unwrap().clone(),
            registry.get(&profile(TARGET_PROFILE)).unwrap().clone(),
        )
    }

    fn object(uuid: &str, kind: &str, owner: Option<&str>) -> CanonicalObject {
        let uuid = ObjectUuid::parse(uuid).unwrap();
        let path_text = format!("{kind}.{uuid}");
        let path = ObjectPath::new(vec![PathSegment::name(&path_text).unwrap()]).unwrap();
        let mut parts = CanonicalObjectParts::new(
            LogicalIdentity::new(uuid, path.clone()),
            MetadataKind::new(kind).unwrap(),
            SourceProvenance::new(
                profile(SOURCE_PROFILE),
                CanonicalAnchor::new(path, PropertyPath::root()),
            ),
        );
        parts.owner = owner.map(|value| ObjectUuid::parse(value).unwrap());
        CanonicalObject::new(parts).unwrap()
    }

    #[test]
    fn evidenced_constant_and_catalog_tree_are_zero_loss() {
        let catalog_uuid = "11111111-1111-4111-8111-111111111111";
        let configuration = CanonicalConfiguration::new(vec![
            object(catalog_uuid, "Catalog", None),
            object(
                "22222222-2222-4222-8222-222222222222",
                "Attribute",
                Some(catalog_uuid),
            ),
            object("33333333-3333-4333-8333-333333333333", "Constant", None),
        ])
        .unwrap();
        let (source, target) = profiles();
        let step = V2_20ToV2_21::new();

        let analysis = step
            .analyze(MigrationAnalyzeRequest::new(
                &source,
                &target,
                &configuration,
            ))
            .unwrap();
        assert!(analysis.is_compatible());
        assert!(analysis.diagnostics().diagnostics().is_empty());

        let output = step.apply(MigrationApplyRequest::new(
            &source,
            &target,
            &configuration,
            LossPolicy::Error,
        ));
        assert_eq!(output.value().unwrap().configuration(), &configuration);
        assert!(output.value().unwrap().losses().is_empty());
    }

    #[test]
    fn unknown_family_and_wrong_provenance_block_with_stable_paths() {
        let mut future = object("44444444-4444-4444-8444-444444444444", "FutureFamily", None);
        let parts = CanonicalObjectParts::new(
            future.identity().clone(),
            future.kind().clone(),
            SourceProvenance::new(
                profile(TARGET_PROFILE),
                CanonicalAnchor::new(future.identity().path().clone(), PropertyPath::root()),
            ),
        );
        future = CanonicalObject::new(parts).unwrap();
        let configuration = CanonicalConfiguration::new(vec![future]).unwrap();
        let (source, target) = profiles();
        let analysis = V2_20ToV2_21::new()
            .analyze(MigrationAnalyzeRequest::new(
                &source,
                &target,
                &configuration,
            ))
            .unwrap();

        let codes = analysis
            .diagnostics()
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.code().as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            codes,
            BTreeSet::from([SOURCE_PROVENANCE_CODE, UNKNOWN_DELTA_CODE])
        );
    }

    #[test]
    fn edge_is_explicitly_verified_upgrade() {
        let edge = V2_20ToV2_21::verified_edge();
        assert_eq!(edge.direction(), MigrationDirection::Upgrade);
        assert_eq!(edge.verification(), MigrationVerification::Verified);
        assert_eq!(edge.step().descriptor().id().as_str(), STEP_ID);
        assert_eq!(edge.step().descriptor().possible_losses(), &[]);
    }
}
