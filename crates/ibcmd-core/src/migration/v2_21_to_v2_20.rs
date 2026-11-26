//! Guarded XCF 2.21 to 2.20 downgrade edge.
//!
//! The evidenced Constant and Catalog cohort is semantically portable. The
//! 2.21 Catalog palette namespace is a lexical target-adapter concern. The
//! 2.21-only managed-form `UseInInterfaceCompatibilityMode=Any` property is
//! different: it is retained under `warn`, rejected by default, and removed
//! only after an explicit `drop` policy has produced path-addressed evidence.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::adapter::AdapterOutcome;
use crate::artifact::ProfileId;
use crate::diagnostic::{
    CodecLossDeclaration, CodecLossPermission, Diagnostic, DiagnosticBuildError, DiagnosticCode,
    DiagnosticReport, LossDisposition, LossPolicy, LossPolicyError, ObjectPath, PathSegment,
    PropertyPath, Severity, evaluate_loss,
};
use crate::identity::ObjectUuid;
use crate::model::{CanonicalConfiguration, CanonicalObject, CanonicalObjectParts};
use crate::profile::CapabilityId;
use crate::value::CanonicalValueKind;

use super::compatibility::{
    CompatibilityRequirement, CompatibilityRequirements, analyze_compatibility,
};
use super::graph::{MigrationDirection, MigrationEdge, MigrationVerification};
use super::step::{
    MAX_MIGRATION_APPLIED_LOSSES, MigrationAnalysis, MigrationAnalyzeRequest,
    MigrationApplyOutcome, MigrationApplyRequest, MigrationStep, MigrationStepDescriptor,
    MigrationStepId, MigrationStepOutput, ProfileConstraint,
};

/// Stable identifier of the verified downgrade edge.
pub const STEP_ID: &str = "migration:xcf-2.21-to-2.20";
/// Exact source profile accepted by the edge.
pub const SOURCE_PROFILE: &str = "xml-2.21";
/// Exact target profile emitted by the edge.
pub const TARGET_PROFILE: &str = "xml-2.20";

/// Capability proving the strict Constant family codec is available.
pub const CONSTANT_CAPABILITY: &str = "xcf:metadata-constant";
/// Capability proving the strict Catalog family codec is available.
pub const CATALOG_CAPABILITY: &str = "xcf:metadata-catalog";
/// Capability whose lexical Catalog delta is handled by the XML adapter.
pub const PALETTE_CAPABILITY: &str = "xcf:palette-namespace";
/// Capability owning the 2.21-only managed-form property.
pub const INTERFACE_MODE_CAPABILITY: &str = "xcf:use-in-interface-compatibility-mode";

/// Declared droppable loss for the evidenced 2.21-only form property.
pub const INTERFACE_MODE_LOSS_CODE: &str =
    "migration.xcf-2-21-to-2-20.use-in-interface-compatibility-mode";
/// Stable diagnostic for a model object or property outside the verified cohort.
pub const UNKNOWN_DELTA_CODE: &str = "migration.xcf-2-21-to-2-20.unknown-delta";
/// Stable diagnostic for an unevidenced value of the known 2.21-only property.
pub const UNSUPPORTED_INTERFACE_MODE_VALUE_CODE: &str =
    "migration.xcf-2-21-to-2-20.unsupported-interface-mode-value";
/// Stable diagnostic for model provenance that does not belong to XCF 2.21.
pub const SOURCE_PROVENANCE_CODE: &str = "migration.xcf-2-21-to-2-20.source-provenance";
/// Stable diagnostic emitted before retaining more loss evidence than one step allows.
pub const LOSS_LIMIT_CODE: &str = "migration.xcf-2-21-to-2-20.loss-limit";

const INTERFACE_MODE_PROPERTY: &str = "UseInInterfaceCompatibilityMode";
const ROOT_FAMILIES: &[&str] = &["Catalog", "Constant"];
const CATALOG_CHILD_FAMILIES: &[&str] = &["Attribute", "Command", "TabularSection"];

/// Evidence-bounded downgrade implementation with an explicit loss contract.
#[derive(Clone, Debug)]
pub struct V2_21ToV2_20 {
    requirements: CompatibilityRequirements,
}

impl V2_21ToV2_20 {
    /// Builds the immutable hard-coded route and loss contract.
    pub fn new() -> Self {
        let constant = capability(CONSTANT_CAPABILITY);
        let catalog = capability(CATALOG_CAPABILITY);
        let source = ProfileConstraint::with_capabilities(
            profile(SOURCE_PROFILE),
            vec![
                constant.clone(),
                catalog.clone(),
                capability(PALETTE_CAPABILITY),
                capability(INTERFACE_MODE_CAPABILITY),
            ],
        )
        .expect("built-in 2.21 capabilities are unique and bounded");
        let target = ProfileConstraint::with_capabilities(
            profile(TARGET_PROFILE),
            vec![constant.clone(), catalog.clone()],
        )
        .expect("built-in 2.20 capabilities are unique and bounded");
        let loss = CodecLossDeclaration::new(
            diagnostic_code(INTERFACE_MODE_LOSS_CODE),
            CodecLossPermission::DropAllowed,
            "XCF 2.20 cannot encode the 2.21 managed-form interface compatibility mode",
        )
        .expect("built-in loss declaration is bounded");
        let descriptor = MigrationStepDescriptor::new(
            MigrationStepId::parse(STEP_ID).expect("built-in migration id is valid"),
            source,
            target,
            vec![constant.clone(), catalog.clone()],
            vec![loss],
        )
        .expect("built-in migration descriptor is valid");
        let requirements = CompatibilityRequirements::new(
            descriptor,
            vec![
                CompatibilityRequirement::global(constant),
                CompatibilityRequirement::global(catalog),
            ],
        )
        .expect("built-in migration requirements cover every capability exactly once");
        Self { requirements }
    }

    /// Wraps this implementation as a verified directed downgrade edge.
    pub fn verified_edge() -> MigrationEdge {
        MigrationEdge::new(
            Arc::new(Self::new()),
            MigrationDirection::Downgrade,
            MigrationVerification::Verified,
        )
    }

    fn loss_declaration(&self) -> &CodecLossDeclaration {
        &self.requirements.descriptor().possible_losses()[0]
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
        let mut interface_mode_count = 0_usize;
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

            if object.kind().as_str() == "Form" {
                diagnostics.extend(analyze_form(request, object)?);
                interface_mode_count += usize::from(interface_mode_field(object).is_some());
            } else if !is_supported_metadata_object(object, &objects_by_uuid) {
                diagnostics.push(unknown_delta_diagnostic(
                    request,
                    object,
                    property_path("kind"),
                    "canonical object is outside the verified XCF 2.21 to 2.20 family cohort",
                )?);
            }
        }
        if interface_mode_count > MAX_MIGRATION_APPLIED_LOSSES {
            diagnostics.push(loss_limit_diagnostic(request, interface_mode_count)?);
        }
        Ok(diagnostics)
    }
}

impl Default for V2_21ToV2_20 {
    fn default() -> Self {
        Self::new()
    }
}

impl MigrationStep for V2_21ToV2_20 {
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
        let analysis = match analysis {
            Ok(analysis) if analysis.is_compatible() => analysis,
            Ok(analysis) => return AdapterOutcome::without_value(analysis.diagnostics().clone()),
            Err(error) => {
                return AdapterOutcome::without_value(DiagnosticReport::from_diagnostics(vec![
                    internal_diagnostic(request, &error),
                ]));
            }
        };
        debug_assert!(analysis.diagnostics().diagnostics().is_empty());

        let mut diagnostics = Vec::new();
        let mut losses = Vec::<LossDisposition>::new();
        for object in request.configuration().objects() {
            if interface_mode_field(object).is_none() {
                continue;
            }
            let diagnostic = interface_mode_loss_diagnostic(request, object);
            match evaluate_loss(
                request.loss_policy(),
                diagnostic,
                Some(self.loss_declaration()),
            ) {
                Ok(loss) => {
                    diagnostics.push(loss.diagnostic().clone());
                    losses.push(loss);
                }
                Err(LossPolicyError::Rejected { diagnostic }) => diagnostics.push(*diagnostic),
                Err(error) => {
                    diagnostics.push(loss_evaluation_diagnostic(request, object, &error));
                }
            }
        }

        let diagnostics = DiagnosticReport::from_diagnostics(diagnostics);
        if diagnostics.has_errors() {
            return AdapterOutcome::without_value(diagnostics);
        }

        let configuration = if request.loss_policy() == LossPolicy::DropExplicitly {
            drop_interface_mode_properties(request.configuration())
        } else {
            request.configuration().clone()
        };
        AdapterOutcome::new(
            MigrationStepOutput::new(configuration, losses)
                .expect("bounded input cannot exceed the per-step loss limit"),
            diagnostics,
        )
    }
}

fn is_supported_metadata_object(
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

fn analyze_form(
    request: MigrationAnalyzeRequest<'_>,
    object: &CanonicalObject,
) -> Result<Vec<Diagnostic>, DiagnosticBuildError> {
    let mut diagnostics = Vec::new();
    if object.owner().is_some()
        || !object.references().is_empty()
        || !object.generated_types().is_empty()
        || !object.assets().is_empty()
        || !object.opaque_facets().as_slice().is_empty()
    {
        diagnostics.push(unknown_delta_diagnostic(
            request,
            object,
            property_path("structure"),
            "managed-form downgrade fixture has an unevidenced relationship, asset, or opaque facet",
        )?);
    }

    let mut has_name = false;
    for field in object.properties() {
        match field.name().as_str() {
            "Name" => {
                has_name = true;
                if !matches!(field.value().kind(), CanonicalValueKind::Text(_)) {
                    diagnostics.push(unknown_delta_diagnostic(
                        request,
                        object,
                        property_path("Name"),
                        "managed-form Name is outside the evidenced text projection",
                    )?);
                }
            }
            INTERFACE_MODE_PROPERTY => {
                if !matches!(
                    field.value().kind(),
                    CanonicalValueKind::EnumToken(value) if value.as_str() == "Any"
                ) {
                    diagnostics.push(interface_mode_value_diagnostic(request, object)?);
                }
            }
            other => diagnostics.push(
                unknown_delta_diagnostic(
                    request,
                    object,
                    property_path("properties"),
                    "managed-form property is outside the bounded downgrade projection",
                )?
                .with_context("property_name", other)?,
            ),
        }
    }
    if !has_name {
        diagnostics.push(unknown_delta_diagnostic(
            request,
            object,
            property_path("Name"),
            "managed-form downgrade projection requires a typed Name",
        )?);
    }
    Ok(diagnostics)
}

fn interface_mode_field(object: &CanonicalObject) -> Option<usize> {
    (object.kind().as_str() == "Form").then(|| {
        object
            .properties()
            .iter()
            .position(|field| field.name().as_str() == INTERFACE_MODE_PROPERTY)
    })?
}

fn drop_interface_mode_properties(
    configuration: &CanonicalConfiguration,
) -> CanonicalConfiguration {
    let objects = configuration
        .objects()
        .iter()
        .map(|object| {
            if interface_mode_field(object).is_none() {
                return object.clone();
            }
            let mut parts = copy_object_parts(object);
            parts
                .properties
                .retain(|field| field.name().as_str() != INTERFACE_MODE_PROPERTY);
            CanonicalObject::new(parts)
                .expect("removing one property preserves canonical object invariants")
        })
        .collect();
    CanonicalConfiguration::new(objects)
        .expect("removing properties preserves canonical configuration bounds")
}

fn copy_object_parts(object: &CanonicalObject) -> CanonicalObjectParts {
    CanonicalObjectParts {
        identity: object.identity().clone(),
        kind: object.kind().clone(),
        owner: object.owner(),
        properties: object.properties().to_vec(),
        references: object.references().to_vec(),
        generated_types: object.generated_types().to_vec(),
        assets: object.assets().to_vec(),
        opaque_facets: object.opaque_facets().clone(),
        provenance: object.provenance().clone(),
    }
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
        "canonical object does not carry exact XCF 2.21 source provenance",
    )
    .map(|diagnostic| with_profiles(diagnostic, request))?
    .with_recovery_hint("decode the object with the exact xml-2.21 adapter before migration")?
    .with_context("migration_step", STEP_ID)?
    .with_context(
        "object_source_profile",
        object.provenance().source_profile().as_str(),
    )
}

fn unknown_delta_diagnostic(
    request: MigrationAnalyzeRequest<'_>,
    object: &CanonicalObject,
    property_path: PropertyPath,
    message: &str,
) -> Result<Diagnostic, DiagnosticBuildError> {
    Diagnostic::new(
        diagnostic_code(UNKNOWN_DELTA_CODE),
        Severity::Error,
        object.identity().path().clone(),
        property_path,
        message,
    )
    .map(|diagnostic| with_profiles(diagnostic, request))?
    .with_recovery_hint(
        "add an evidence-backed family codec and downgrade fixture before extending this edge",
    )?
    .with_context("migration_step", STEP_ID)?
    .with_context("metadata_kind", object.kind().as_str())
}

fn interface_mode_value_diagnostic(
    request: MigrationAnalyzeRequest<'_>,
    object: &CanonicalObject,
) -> Result<Diagnostic, DiagnosticBuildError> {
    Diagnostic::new(
        diagnostic_code(UNSUPPORTED_INTERFACE_MODE_VALUE_CODE),
        Severity::Error,
        object.identity().path().clone(),
        property_path(INTERFACE_MODE_PROPERTY),
        "managed-form interface compatibility mode is not the evidenced value Any",
    )
    .map(|diagnostic| with_profiles(diagnostic, request))?
    .with_recovery_hint("retain the source profile until this exact mode has downgrade evidence")?
    .with_context("migration_step", STEP_ID)
}

fn loss_limit_diagnostic(
    request: MigrationAnalyzeRequest<'_>,
    actual: usize,
) -> Result<Diagnostic, DiagnosticBuildError> {
    Diagnostic::new(
        diagnostic_code(LOSS_LIMIT_CODE),
        Severity::Error,
        ObjectPath::root(),
        PropertyPath::root(),
        "downgrade loss evidence exceeds the bounded per-step limit",
    )
    .map(|diagnostic| with_profiles(diagnostic, request))?
    .with_recovery_hint("split the bounded conversion into smaller independently reported inputs")?
    .with_context("migration_step", STEP_ID)?
    .with_context("maximum", &MAX_MIGRATION_APPLIED_LOSSES.to_string())?
    .with_context("actual", &actual.to_string())
}

fn interface_mode_loss_diagnostic(
    request: MigrationApplyRequest<'_>,
    object: &CanonicalObject,
) -> Diagnostic {
    Diagnostic::new(
        diagnostic_code(INTERFACE_MODE_LOSS_CODE),
        Severity::Error,
        object.identity().path().clone(),
        property_path(INTERFACE_MODE_PROPERTY),
        "XCF 2.20 cannot represent UseInInterfaceCompatibilityMode=Any",
    )
    .map(|diagnostic| {
        diagnostic.with_profiles(
            Some(request.source_profile().id.clone()),
            Some(request.target_profile().id.clone()),
        )
    })
    .and_then(|diagnostic| {
        diagnostic.with_recovery_hint(
            "keep the 2.21 profile, use warn to retain the value, or explicitly allow drop",
        )
    })
    .and_then(|diagnostic| diagnostic.with_context("migration_step", STEP_ID))
    .and_then(|diagnostic| diagnostic.with_context("source_value", "Any"))
    .expect("built-in loss diagnostic is bounded")
}

fn loss_evaluation_diagnostic(
    request: MigrationApplyRequest<'_>,
    object: &CanonicalObject,
    error: &LossPolicyError,
) -> Diagnostic {
    Diagnostic::new(
        diagnostic_code("migration.xcf-2-21-to-2-20.loss-evaluation-failed"),
        Severity::Error,
        object.identity().path().clone(),
        property_path(INTERFACE_MODE_PROPERTY),
        "built-in downgrade loss policy evaluation failed",
    )
    .map(|diagnostic| {
        diagnostic.with_profiles(
            Some(request.source_profile().id.clone()),
            Some(request.target_profile().id.clone()),
        )
    })
    .and_then(|diagnostic| diagnostic.with_context("reason", &error.to_string()))
    .expect("built-in fallback diagnostic is bounded")
}

fn internal_diagnostic(
    request: MigrationApplyRequest<'_>,
    error: &DiagnosticBuildError,
) -> Diagnostic {
    Diagnostic::new(
        diagnostic_code("migration.xcf-2-21-to-2-20.diagnostic-build-failed"),
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
    .and_then(|diagnostic| diagnostic.with_context("reason", &error.to_string()))
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
    use crate::diagnostic::{LossPolicy, ObjectPath, PathSegment, PropertyPath};
    use crate::identity::{LogicalIdentity, ObjectUuid};
    use crate::migration::executor::{MigrationExecutionRequest, MigrationExecutor};
    use crate::migration::graph::MigrationGraph;
    use crate::migration::report::MigrationLossDisposition;
    use crate::model::{CanonicalConfiguration, CanonicalObjectParts, MetadataKind};
    use crate::profile::{
        EffectiveProfile, ProfileRegistry, ProfileSourceKind, parse_profile_source,
        resolve_profiles,
    };
    use crate::provenance::{CanonicalAnchor, SourceProvenance};
    use crate::value::{CanonicalField, CanonicalText, CanonicalValue, EnumToken};

    use super::*;

    fn profiles() -> ProfileRegistry {
        let target = format!(
            r#"{{"schema_version":1,"id":"{TARGET_PROFILE}","status":"experimental","capabilities":{{"{CONSTANT_CAPABILITY}":"supported","{CATALOG_CAPABILITY}":"supported","{PALETTE_CAPABILITY}":"unsupported","{INTERFACE_MODE_CAPABILITY}":"unsupported"}}}}"#
        );
        let source = format!(
            r#"{{"schema_version":1,"id":"{SOURCE_PROFILE}","status":"experimental","capabilities":{{"{CONSTANT_CAPABILITY}":"supported","{CATALOG_CAPABILITY}":"supported","{PALETTE_CAPABILITY}":"supported","{INTERFACE_MODE_CAPABILITY}":"supported"}}}}"#
        );
        resolve_profiles([
            parse_profile_source("target.json", ProfileSourceKind::Bundled, &target).unwrap(),
            parse_profile_source("source.json", ProfileSourceKind::Bundled, &source).unwrap(),
        ])
        .unwrap()
    }

    fn endpoints(profiles: &ProfileRegistry) -> (&EffectiveProfile, &EffectiveProfile) {
        (
            profiles.get(&profile(SOURCE_PROFILE)).unwrap(),
            profiles.get(&profile(TARGET_PROFILE)).unwrap(),
        )
    }

    fn object(uuid: &str, kind: &str, owner: Option<&str>) -> CanonicalObject {
        let uuid = ObjectUuid::parse(uuid).unwrap();
        let path =
            ObjectPath::new(vec![PathSegment::name(&format!("{kind}.{uuid}")).unwrap()]).unwrap();
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

    fn form_with_uuid(uuid: &str, mode: &str) -> CanonicalObject {
        let mut object = object(uuid, "Form", None);
        let mut parts = copy_object_parts(&object);
        parts.properties = vec![
            CanonicalField::named(
                "Name",
                CanonicalValue::text(CanonicalText::new("Main").unwrap()),
            )
            .unwrap(),
            CanonicalField::named(
                INTERFACE_MODE_PROPERTY,
                CanonicalValue::enum_token(EnumToken::new(mode).unwrap()),
            )
            .unwrap(),
        ];
        object = CanonicalObject::new(parts).unwrap();
        object
    }

    fn form(mode: &str) -> CanonicalObject {
        form_with_uuid("44444444-4444-4444-8444-444444444444", mode)
    }

    fn execute(
        configuration: &CanonicalConfiguration,
        policy: LossPolicy,
    ) -> Result<
        crate::migration::executor::MigrationExecution,
        crate::migration::executor::MigrationExecutionError,
    > {
        let profiles = profiles();
        let (source, target) = endpoints(&profiles);
        let graph = MigrationGraph::new(&profiles, vec![V2_21ToV2_20::verified_edge()]).unwrap();
        let plan = graph.plan(source, target).unwrap();
        MigrationExecutor::new(&graph).execute(MigrationExecutionRequest::new(
            &plan,
            configuration,
            policy,
        ))
    }

    fn has_interface_mode(configuration: &CanonicalConfiguration) -> bool {
        configuration.objects().iter().any(|object| {
            object
                .properties()
                .iter()
                .any(|field| field.name().as_str() == INTERFACE_MODE_PROPERTY)
        })
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

        let execution = execute(&configuration, LossPolicy::Error).unwrap();
        assert_eq!(execution.configuration(), &configuration);
        assert!(execution.report().steps()[0].losses().is_empty());
    }

    #[test]
    fn default_error_blocks_the_exact_interface_mode_path() {
        let configuration = CanonicalConfiguration::new(vec![form("Any")]).unwrap();
        let snapshot = configuration.clone();
        let error = execute(&configuration, LossPolicy::Error).unwrap_err();
        let report = error.report().unwrap();

        assert_eq!(configuration, snapshot);
        assert_eq!(report.steps()[0].apply_diagnostics().diagnostics().len(), 1);
        let diagnostic = &report.steps()[0].apply_diagnostics().diagnostics()[0];
        assert_eq!(diagnostic.code().as_str(), INTERFACE_MODE_LOSS_CODE);
        assert_eq!(
            diagnostic.property_path(),
            &property_path(INTERFACE_MODE_PROPERTY)
        );
        assert_eq!(diagnostic.severity(), Severity::Error);
        assert!(report.steps()[0].losses().is_empty());
    }

    #[test]
    fn warn_retains_the_property_and_records_reversible_evidence() {
        let configuration = CanonicalConfiguration::new(vec![form("Any")]).unwrap();
        let execution = execute(&configuration, LossPolicy::Warn).unwrap();
        let evidence = &execution.report().steps()[0].losses()[0];

        assert!(has_interface_mode(execution.configuration()));
        assert_eq!(evidence.requested_policy(), LossPolicy::Warn);
        assert_eq!(
            evidence.actual_disposition(),
            MigrationLossDisposition::ContinueWithWarning
        );
        assert_eq!(
            evidence.diagnostic().code().as_str(),
            INTERFACE_MODE_LOSS_CODE
        );
        assert_eq!(
            evidence.diagnostic().property_path(),
            &property_path(INTERFACE_MODE_PROPERTY)
        );
    }

    #[test]
    fn explicit_drop_removes_only_the_property_and_records_it() {
        let configuration = CanonicalConfiguration::new(vec![form("Any")]).unwrap();
        let snapshot = configuration.clone();
        let execution = execute(&configuration, LossPolicy::DropExplicitly).unwrap();
        let evidence = &execution.report().steps()[0].losses()[0];

        assert_eq!(configuration, snapshot);
        assert!(!has_interface_mode(execution.configuration()));
        assert_eq!(evidence.requested_policy(), LossPolicy::DropExplicitly);
        assert_eq!(
            evidence.actual_disposition(),
            MigrationLossDisposition::DroppedExplicitly
        );
        assert_eq!(
            evidence.declaration().permission(),
            CodecLossPermission::DropAllowed
        );
    }

    #[test]
    fn unevidenced_interface_mode_value_is_never_droppable() {
        let configuration = CanonicalConfiguration::new(vec![form("Client")]).unwrap();
        let profiles = profiles();
        let (source, target) = endpoints(&profiles);
        let analysis = V2_21ToV2_20::new()
            .analyze(MigrationAnalyzeRequest::new(source, target, &configuration))
            .unwrap();

        assert_eq!(analysis.diagnostics().diagnostics().len(), 1);
        assert_eq!(
            analysis.diagnostics().diagnostics()[0].code().as_str(),
            UNSUPPORTED_INTERFACE_MODE_VALUE_CODE
        );
    }

    #[test]
    fn loss_count_above_the_step_bound_blocks_during_analysis() {
        let objects = (0..=MAX_MIGRATION_APPLIED_LOSSES)
            .map(|index| form_with_uuid(&format!("44444444-4444-4444-8444-{index:012x}"), "Any"))
            .collect();
        let configuration = CanonicalConfiguration::new(objects).unwrap();
        let profiles = profiles();
        let (source, target) = endpoints(&profiles);
        let analysis = V2_21ToV2_20::new()
            .analyze(MigrationAnalyzeRequest::new(source, target, &configuration))
            .unwrap();

        assert!(
            analysis
                .diagnostics()
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code().as_str() == LOSS_LIMIT_CODE)
        );
    }

    #[test]
    fn edge_is_explicitly_verified_downgrade() {
        let edge = V2_21ToV2_20::verified_edge();
        assert_eq!(edge.direction(), MigrationDirection::Downgrade);
        assert_eq!(edge.verification(), MigrationVerification::Verified);
        assert_eq!(edge.step().descriptor().id().as_str(), STEP_ID);
        assert_eq!(edge.step().descriptor().possible_losses().len(), 1);
        assert_eq!(
            edge.step().descriptor().possible_losses()[0]
                .code()
                .as_str(),
            INTERFACE_MODE_LOSS_CODE
        );
    }
}
