//! Path-scoped compatibility analysis for migration steps.

use crate::diagnostic::{
    Diagnostic, DiagnosticCode, DiagnosticReport, ObjectPath, PropertyPath, Severity,
};
use crate::profile::{CapabilityId, CapabilityState};

use super::step::{MigrationAnalysis, MigrationAnalyzeRequest, MigrationStepDescriptor};

/// Stable code for a source profile that does not satisfy a step constraint.
pub const SOURCE_CONSTRAINT_MISMATCH_CODE: &str = "migration.source-constraint-mismatch";
/// Stable code for a target profile that does not satisfy a step constraint.
pub const TARGET_CONSTRAINT_MISMATCH_CODE: &str = "migration.target-constraint-mismatch";
/// Stable code for a capability explicitly unsupported by the target profile.
pub const CAPABILITY_UNSUPPORTED_CODE: &str = "migration.capability-unsupported";
/// Stable code for a capability not declared by the target profile.
pub const CAPABILITY_UNDECLARED_CODE: &str = "migration.capability-undeclared";

/// One exact capability requirement attached to a concrete model path.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CompatibilityRequirement {
    capability: CapabilityId,
    object_path: ObjectPath,
    property_path: PropertyPath,
}

impl CompatibilityRequirement {
    /// Creates a requirement whose paths have already passed core bounds.
    pub const fn new(
        capability: CapabilityId,
        object_path: ObjectPath,
        property_path: PropertyPath,
    ) -> Self {
        Self {
            capability,
            object_path,
            property_path,
        }
    }

    /// Returns the exact open capability identifier.
    pub const fn capability(&self) -> &CapabilityId {
        &self.capability
    }

    /// Returns the concrete incompatible object path.
    pub const fn object_path(&self) -> &ObjectPath {
        &self.object_path
    }

    /// Returns the concrete incompatible property path.
    pub const fn property_path(&self) -> &PropertyPath {
        &self.property_path
    }
}

/// Evaluates exact route constraints and all path-scoped target requirements.
///
/// Every incompatible requirement produces its own stable diagnostic. The
/// analyzed configuration is only available through an immutable borrow in
/// [`MigrationAnalyzeRequest`].
pub fn analyze_compatibility(
    descriptor: &MigrationStepDescriptor,
    request: MigrationAnalyzeRequest<'_>,
    requirements: &[CompatibilityRequirement],
) -> MigrationAnalysis {
    let mut diagnostics = Vec::new();
    let source_id = &request.source_profile().id;
    let target_id = &request.target_profile().id;

    if !descriptor.source().matches(request.source_profile()) {
        diagnostics.push(route_diagnostic(
            SOURCE_CONSTRAINT_MISMATCH_CODE,
            "source",
            descriptor,
            source_id,
            target_id,
        ));
    }
    if !descriptor.target().matches(request.target_profile()) {
        diagnostics.push(route_diagnostic(
            TARGET_CONSTRAINT_MISMATCH_CODE,
            "target",
            descriptor,
            source_id,
            target_id,
        ));
    }

    for requirement in requirements {
        match request
            .target_profile()
            .capabilities
            .get(requirement.capability())
            .map(|state| state.value)
        {
            Some(CapabilityState::Supported) => {}
            Some(CapabilityState::Unsupported) => diagnostics.push(capability_diagnostic(
                CAPABILITY_UNSUPPORTED_CODE,
                "is explicitly unsupported",
                descriptor,
                request,
                requirement,
            )),
            None => diagnostics.push(capability_diagnostic(
                CAPABILITY_UNDECLARED_CODE,
                "is not declared",
                descriptor,
                request,
                requirement,
            )),
        }
    }

    MigrationAnalysis::new(DiagnosticReport::from_diagnostics(diagnostics))
}

fn route_diagnostic(
    code: &'static str,
    endpoint: &'static str,
    descriptor: &MigrationStepDescriptor,
    source_profile: &crate::artifact::ProfileId,
    target_profile: &crate::artifact::ProfileId,
) -> Diagnostic {
    let message = format!(
        "{endpoint} profile does not satisfy migration step `{}` constraint",
        descriptor.id()
    );
    Diagnostic::new(
        DiagnosticCode::parse(code).expect("built-in migration diagnostic code is valid"),
        Severity::Error,
        ObjectPath::root(),
        PropertyPath::root(),
        &message,
    )
    .expect("bounded profile and migration identifiers fit diagnostic text")
    .with_profiles(Some(source_profile.clone()), Some(target_profile.clone()))
    .with_recovery_hint("select profiles satisfying the exact migration step constraints")
    .expect("static recovery hint fits diagnostic bounds")
    .with_context("endpoint", endpoint)
    .expect("static context fits diagnostic bounds")
    .with_context("migration_step", descriptor.id().as_str())
    .expect("bounded migration identifier fits diagnostic context")
}

fn capability_diagnostic(
    code: &'static str,
    state: &'static str,
    descriptor: &MigrationStepDescriptor,
    request: MigrationAnalyzeRequest<'_>,
    requirement: &CompatibilityRequirement,
) -> Diagnostic {
    let message = format!(
        "target profile capability `{}` {state} for this path",
        requirement.capability()
    );
    Diagnostic::new(
        DiagnosticCode::parse(code).expect("built-in migration diagnostic code is valid"),
        Severity::Error,
        requirement.object_path().clone(),
        requirement.property_path().clone(),
        &message,
    )
    .expect("bounded capability identifier fits diagnostic text")
    .with_profiles(
        Some(request.source_profile().id.clone()),
        Some(request.target_profile().id.clone()),
    )
    .with_recovery_hint(
        "add evidence-backed target support or implement an explicit migration rule for this path",
    )
    .expect("static recovery hint fits diagnostic bounds")
    .with_context("capability", requirement.capability().as_str())
    .expect("bounded capability identifier fits diagnostic context")
    .with_context("migration_step", descriptor.id().as_str())
    .expect("bounded migration identifier fits diagnostic context")
}

#[cfg(test)]
mod tests {
    use crate::adapter::AdapterOutcome;
    use crate::artifact::ProfileId;
    use crate::diagnostic::{LossPolicy, PathSegment};
    use crate::identity::{LogicalIdentity, ObjectUuid};
    use crate::model::{
        CanonicalConfiguration, CanonicalObject, CanonicalObjectParts, MetadataKind,
    };
    use crate::profile::{
        EffectiveProfile, ProfileSourceKind, parse_profile_source, resolve_profiles,
    };
    use crate::provenance::{CanonicalAnchor, SourceProvenance};
    use crate::validate::validate_configuration;

    use super::super::step::{
        MigrationApplyOutcome, MigrationApplyRequest, MigrationStep, MigrationStepId,
        ProfileConstraint,
    };
    use super::*;

    fn profile(value: &str, capabilities: &str) -> EffectiveProfile {
        let json = format!(
            r#"{{"schema_version":1,"id":"{value}","status":"experimental","capabilities":{capabilities}}}"#
        );
        let document =
            parse_profile_source("migration-test.json", ProfileSourceKind::Bundled, &json).unwrap();
        resolve_profiles([document])
            .unwrap()
            .get(&ProfileId::parse(value).unwrap())
            .unwrap()
            .clone()
    }

    fn object_path(name: &str) -> ObjectPath {
        ObjectPath::new(vec![
            PathSegment::name("objects").unwrap(),
            PathSegment::name(name).unwrap(),
        ])
        .unwrap()
    }

    fn property_path(name: &str) -> PropertyPath {
        PropertyPath::new(vec![
            PathSegment::name("properties").unwrap(),
            PathSegment::name(name).unwrap(),
        ])
        .unwrap()
    }

    fn configuration() -> CanonicalConfiguration {
        let path = object_path("Catalog.Test");
        let identity = LogicalIdentity::new(
            ObjectUuid::parse("00000000-0000-0000-0000-000000000001").unwrap(),
            path.clone(),
        );
        let provenance = SourceProvenance::new(
            ProfileId::parse("profile:source").unwrap(),
            CanonicalAnchor::new(path, PropertyPath::root()),
        );
        let object = CanonicalObject::new(CanonicalObjectParts::new(
            identity,
            MetadataKind::new("Catalog").unwrap(),
            provenance,
        ))
        .unwrap();
        CanonicalConfiguration::new(vec![object]).unwrap()
    }

    struct TestStep {
        descriptor: MigrationStepDescriptor,
        requirements: Vec<CompatibilityRequirement>,
    }

    impl MigrationStep for TestStep {
        fn descriptor(&self) -> &MigrationStepDescriptor {
            &self.descriptor
        }

        fn analyze(&self, request: MigrationAnalyzeRequest<'_>) -> MigrationAnalysis {
            analyze_compatibility(&self.descriptor, request, &self.requirements)
        }

        fn apply(&self, request: MigrationApplyRequest<'_>) -> MigrationApplyOutcome {
            AdapterOutcome::success(request.configuration().clone())
        }
    }

    fn step(requirements: Vec<CompatibilityRequirement>) -> TestStep {
        TestStep {
            descriptor: MigrationStepDescriptor::new(
                MigrationStepId::parse("migration:test-edge").unwrap(),
                ProfileConstraint::exact(ProfileId::parse("profile:source").unwrap()),
                ProfileConstraint::exact(ProfileId::parse("profile:target").unwrap()),
                vec![
                    CapabilityId::parse("feature:known").unwrap(),
                    CapabilityId::parse("feature:future").unwrap(),
                ],
                Vec::new(),
            )
            .unwrap(),
            requirements,
        }
    }

    #[test]
    fn every_incompatible_path_has_a_stable_diagnostic() {
        let source = profile(
            "profile:source",
            r#"{"feature:known":"supported","feature:future":"supported"}"#,
        );
        let target = profile("profile:target", r#"{"feature:known":"unsupported"}"#);
        let unsupported_object = object_path("Catalog.Test");
        let unsupported_property = property_path("KnownFeature");
        let undeclared_object = object_path("Catalog.Future");
        let undeclared_property = property_path("FutureFeature");
        let step = step(vec![
            CompatibilityRequirement::new(
                CapabilityId::parse("feature:known").unwrap(),
                unsupported_object.clone(),
                unsupported_property.clone(),
            ),
            CompatibilityRequirement::new(
                CapabilityId::parse("feature:future").unwrap(),
                undeclared_object.clone(),
                undeclared_property.clone(),
            ),
        ]);
        let configuration = configuration();

        let analysis = step.analyze(MigrationAnalyzeRequest::new(
            &source,
            &target,
            &configuration,
        ));

        assert!(!analysis.is_compatible());
        assert_eq!(analysis.diagnostics().diagnostics().len(), 2);
        let unsupported = analysis
            .diagnostics()
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code().as_str() == CAPABILITY_UNSUPPORTED_CODE)
            .unwrap();
        assert_eq!(unsupported.object_path(), &unsupported_object);
        assert_eq!(unsupported.property_path(), &unsupported_property);
        assert_eq!(unsupported.source_profile(), Some(&source.id));
        assert_eq!(unsupported.target_profile(), Some(&target.id));

        let undeclared = analysis
            .diagnostics()
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code().as_str() == CAPABILITY_UNDECLARED_CODE)
            .unwrap();
        assert_eq!(undeclared.object_path(), &undeclared_object);
        assert_eq!(undeclared.property_path(), &undeclared_property);
        assert_eq!(
            undeclared.context().get("capability").map(String::as_str),
            Some("feature:future")
        );
    }

    #[test]
    fn analyze_cannot_change_model_or_semantic_digest() {
        fn assert_object_safe(_: &dyn MigrationStep) {}

        let source = profile(
            "profile:source",
            r#"{"feature:known":"supported","feature:future":"supported"}"#,
        );
        let target = profile(
            "profile:target",
            r#"{"feature:known":"supported","feature:future":"supported"}"#,
        );
        let step = step(vec![CompatibilityRequirement::new(
            CapabilityId::parse("feature:known").unwrap(),
            object_path("Catalog.Test"),
            property_path("KnownFeature"),
        )]);
        assert_object_safe(&step);
        let configuration = configuration();
        let snapshot = configuration.clone();
        let digest_before = validate_configuration(&configuration)
            .unwrap()
            .semantic_digest();

        let analysis = step.analyze(MigrationAnalyzeRequest::new(
            &source,
            &target,
            &configuration,
        ));

        assert!(analysis.is_compatible());
        assert_eq!(configuration, snapshot);
        assert_eq!(
            validate_configuration(&configuration)
                .unwrap()
                .semantic_digest(),
            digest_before
        );

        let applied = step.apply(MigrationApplyRequest::new(
            &source,
            &target,
            &configuration,
            LossPolicy::Error,
        ));
        assert_eq!(applied.value(), Some(&snapshot));
        assert_eq!(configuration, snapshot);
    }

    #[test]
    fn route_constraint_failures_are_stable_and_fail_closed() {
        let source = profile("profile:other-source", r#"{}"#);
        let target = profile("profile:other-target", r#"{}"#);
        let step = step(Vec::new());
        let configuration = configuration();

        let analysis = step.analyze(MigrationAnalyzeRequest::new(
            &source,
            &target,
            &configuration,
        ));

        let codes = analysis
            .diagnostics()
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.code().as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            codes,
            [
                SOURCE_CONSTRAINT_MISMATCH_CODE,
                TARGET_CONSTRAINT_MISMATCH_CODE
            ]
        );
    }
}
