//! Bounded path-scoped compatibility analysis for migration steps.

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use crate::diagnostic::{
    Diagnostic, DiagnosticBuildError, DiagnosticCode, DiagnosticReport, ObjectPath, PathSegment,
    PropertyPath, Severity,
};
use crate::model::{CanonicalConfiguration, CanonicalObject};
use crate::profile::{CapabilityId, CapabilityState};
use crate::value::{CanonicalField, CanonicalValue, CanonicalValueKind};

use super::step::{MigrationAnalysis, MigrationAnalyzeRequest, MigrationStepDescriptor};

/// Maximum path-scoped requirements accepted by one migration step.
pub const MAX_COMPATIBILITY_REQUIREMENTS: usize = 16_384;

/// Stable code for a source profile that does not satisfy a step constraint.
pub const SOURCE_CONSTRAINT_MISMATCH_CODE: &str = "migration.source-constraint-mismatch";
/// Stable code for a target profile that does not satisfy a step constraint.
pub const TARGET_CONSTRAINT_MISMATCH_CODE: &str = "migration.target-constraint-mismatch";
/// Stable code for a capability explicitly unsupported by the target profile.
pub const CAPABILITY_UNSUPPORTED_CODE: &str = "migration.capability-unsupported";
/// Stable code for a capability not declared by the target profile.
pub const CAPABILITY_UNDECLARED_CODE: &str = "migration.capability-undeclared";
/// Stable blocking code for a requirement path absent from the analyzed model.
pub const REQUIREMENT_PATH_UNKNOWN_CODE: &str = "migration.requirement-path-unknown";

/// One exact capability requirement attached to a concrete model path.
///
/// Root object and property paths form an explicit global scope. A non-root
/// object path must resolve to a canonical object, and its property path must
/// resolve within that object before capability support is evaluated.
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

    /// Creates an explicit configuration-wide capability requirement.
    pub const fn global(capability: CapabilityId) -> Self {
        Self::new(capability, ObjectPath::root(), PropertyPath::root())
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

/// Failure to bind a bounded requirement set to one exact step descriptor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompatibilityContractError {
    /// The caller supplied more requirements than one analysis may retain.
    TooManyRequirements {
        /// Maximum accepted requirements.
        maximum: usize,
        /// Actual supplied requirements.
        actual: usize,
    },
    /// The exact same capability and path requirement was supplied twice.
    DuplicateRequirement {
        /// Duplicate capability.
        capability: CapabilityId,
        /// Duplicate object path.
        object_path: ObjectPath,
        /// Duplicate property path.
        property_path: PropertyPath,
    },
    /// A requirement names a capability absent from the bound descriptor.
    CapabilityNotTouched {
        /// Undeclared requirement capability.
        capability: CapabilityId,
    },
    /// A touched capability has no explicit concrete or global path coverage.
    MissingCapabilityCoverage {
        /// Touched capability without coverage.
        capability: CapabilityId,
    },
}

impl Display for CompatibilityContractError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyRequirements { maximum, actual } => write!(
                formatter,
                "compatibility requirements exceed {maximum} items (actual {actual})"
            ),
            Self::DuplicateRequirement {
                capability,
                object_path,
                property_path,
            } => write!(
                formatter,
                "duplicate compatibility requirement `{capability}` at {object_path} {property_path}"
            ),
            Self::CapabilityNotTouched { capability } => write!(
                formatter,
                "compatibility requirement capability `{capability}` is absent from touched capabilities"
            ),
            Self::MissingCapabilityCoverage { capability } => write!(
                formatter,
                "touched capability `{capability}` has no explicit compatibility path coverage"
            ),
        }
    }
}

impl Error for CompatibilityContractError {}

/// Bounded requirements owned together with their exact step descriptor.
///
/// Owning the descriptor prevents a validated set from being silently reused
/// with a different route or touched-capability declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompatibilityRequirements {
    descriptor: MigrationStepDescriptor,
    requirements: Vec<CompatibilityRequirement>,
}

impl CompatibilityRequirements {
    /// Binds, validates, and deterministically orders one complete requirement set.
    pub fn new(
        descriptor: MigrationStepDescriptor,
        mut requirements: Vec<CompatibilityRequirement>,
    ) -> Result<Self, CompatibilityContractError> {
        if requirements.len() > MAX_COMPATIBILITY_REQUIREMENTS {
            return Err(CompatibilityContractError::TooManyRequirements {
                maximum: MAX_COMPATIBILITY_REQUIREMENTS,
                actual: requirements.len(),
            });
        }

        requirements.sort();
        if let Some(pair) = requirements.windows(2).find(|pair| pair[0] == pair[1]) {
            return Err(CompatibilityContractError::DuplicateRequirement {
                capability: pair[0].capability().clone(),
                object_path: pair[0].object_path().clone(),
                property_path: pair[0].property_path().clone(),
            });
        }

        for requirement in &requirements {
            if descriptor
                .touched_capabilities()
                .binary_search(requirement.capability())
                .is_err()
            {
                return Err(CompatibilityContractError::CapabilityNotTouched {
                    capability: requirement.capability().clone(),
                });
            }
        }

        for capability in descriptor.touched_capabilities() {
            if requirements
                .binary_search_by(|requirement| requirement.capability().cmp(capability))
                .is_err()
            {
                return Err(CompatibilityContractError::MissingCapabilityCoverage {
                    capability: capability.clone(),
                });
            }
        }

        Ok(Self {
            descriptor,
            requirements,
        })
    }

    /// Returns the exact descriptor permanently bound to this set.
    pub const fn descriptor(&self) -> &MigrationStepDescriptor {
        &self.descriptor
    }

    /// Returns requirements in deterministic capability/path order.
    pub fn requirements(&self) -> &[CompatibilityRequirement] {
        &self.requirements
    }

    /// Returns the number of path-scoped requirements.
    pub const fn len(&self) -> usize {
        self.requirements.len()
    }

    /// Returns whether the bound descriptor touches no capabilities.
    pub const fn is_empty(&self) -> bool {
        self.requirements.is_empty()
    }
}

/// Evaluates the bound route and every path-scoped target requirement.
///
/// Every incompatible path produces its own stable diagnostic. Unknown model
/// paths fail closed before capability evaluation. Diagnostic construction is
/// fallible rather than panicking on data supplied by profiles or codecs.
pub fn analyze_compatibility(
    requirements: &CompatibilityRequirements,
    request: MigrationAnalyzeRequest<'_>,
) -> Result<MigrationAnalysis, DiagnosticBuildError> {
    let descriptor = requirements.descriptor();
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
        )?);
    }
    if !descriptor.target().matches(request.target_profile()) {
        diagnostics.push(route_diagnostic(
            TARGET_CONSTRAINT_MISMATCH_CODE,
            "target",
            descriptor,
            source_id,
            target_id,
        )?);
    }

    for requirement in requirements.requirements() {
        if !requirement_path_exists(request.configuration(), requirement) {
            diagnostics.push(requirement_path_diagnostic(
                descriptor,
                request,
                requirement,
            )?);
            continue;
        }

        match request
            .target_profile()
            .capabilities
            .get(requirement.capability())
            .map(|state| state.value)
        {
            Some(CapabilityState::Supported) => {}
            Some(CapabilityState::Unsupported) => diagnostics.push(capability_diagnostic(
                CAPABILITY_UNSUPPORTED_CODE,
                "unsupported",
                descriptor,
                request,
                requirement,
            )?),
            None => diagnostics.push(capability_diagnostic(
                CAPABILITY_UNDECLARED_CODE,
                "undeclared",
                descriptor,
                request,
                requirement,
            )?),
        }
    }

    Ok(MigrationAnalysis::new(DiagnosticReport::from_diagnostics(
        diagnostics,
    )))
}

fn route_diagnostic(
    code: &'static str,
    endpoint: &'static str,
    descriptor: &MigrationStepDescriptor,
    source_profile: &crate::artifact::ProfileId,
    target_profile: &crate::artifact::ProfileId,
) -> Result<Diagnostic, DiagnosticBuildError> {
    Diagnostic::new(
        stable_code(code),
        Severity::Error,
        ObjectPath::root(),
        PropertyPath::root(),
        "profile does not satisfy the migration step constraint",
    )
    .map(|diagnostic| {
        diagnostic.with_profiles(Some(source_profile.clone()), Some(target_profile.clone()))
    })?
    .with_recovery_hint("select profiles satisfying the exact migration step constraints")?
    .with_context("endpoint", endpoint)?
    .with_context("migration_step", descriptor.id().as_str())
}

fn capability_diagnostic(
    code: &'static str,
    state: &'static str,
    descriptor: &MigrationStepDescriptor,
    request: MigrationAnalyzeRequest<'_>,
    requirement: &CompatibilityRequirement,
) -> Result<Diagnostic, DiagnosticBuildError> {
    Diagnostic::new(
        stable_code(code),
        Severity::Error,
        requirement.object_path().clone(),
        requirement.property_path().clone(),
        "target profile cannot satisfy the required capability at this path",
    )
    .map(|diagnostic| {
        diagnostic.with_profiles(
            Some(request.source_profile().id.clone()),
            Some(request.target_profile().id.clone()),
        )
    })?
    .with_recovery_hint(
        "add evidence-backed target support or implement an explicit migration rule for this path",
    )?
    .with_context("capability", requirement.capability().as_str())?
    .with_context("capability_state", state)?
    .with_context("migration_step", descriptor.id().as_str())
}

fn requirement_path_diagnostic(
    descriptor: &MigrationStepDescriptor,
    request: MigrationAnalyzeRequest<'_>,
    requirement: &CompatibilityRequirement,
) -> Result<Diagnostic, DiagnosticBuildError> {
    Diagnostic::new(
        stable_code(REQUIREMENT_PATH_UNKNOWN_CODE),
        Severity::Error,
        requirement.object_path().clone(),
        requirement.property_path().clone(),
        "compatibility requirement path does not exist in the analyzed canonical model",
    )
    .map(|diagnostic| {
        diagnostic.with_profiles(
            Some(request.source_profile().id.clone()),
            Some(request.target_profile().id.clone()),
        )
    })?
    .with_recovery_hint("fix the migration requirement or decode the missing model path first")?
    .with_context("capability", requirement.capability().as_str())?
    .with_context("migration_step", descriptor.id().as_str())
}

fn stable_code(value: &'static str) -> DiagnosticCode {
    DiagnosticCode::parse(value).expect("built-in migration diagnostic code is valid")
}

fn requirement_path_exists(
    configuration: &CanonicalConfiguration,
    requirement: &CompatibilityRequirement,
) -> bool {
    if requirement.object_path().segments().is_empty() {
        return requirement.property_path().segments().is_empty();
    }

    configuration
        .objects()
        .iter()
        .find(|object| object.identity().path() == requirement.object_path())
        .is_some_and(|object| object_property_path_exists(object, requirement.property_path()))
}

fn object_property_path_exists(object: &CanonicalObject, path: &PropertyPath) -> bool {
    let Some((head, tail)) = path.segments().split_first() else {
        return true;
    };
    let Some(name) = head.as_name() else {
        return false;
    };

    match name {
        "identity" => named_leaf_path_exists(tail, &["uuid", "path"]),
        "kind" | "owner" | "provenance" => tail.is_empty(),
        "properties" => canonical_fields_path_exists(object.properties(), tail),
        "references" => {
            indexed_struct_path_exists(object.references().len(), tail, &["kind", "target"])
        }
        "generated_types" => {
            indexed_struct_path_exists(object.generated_types().len(), tail, &["uuid", "kind"])
        }
        "assets" => indexed_struct_path_exists(
            object.assets().len(),
            tail,
            &["sha256", "byte_len", "media_kind"],
        ),
        "opaque_facets" => indexed_struct_path_exists(
            object.opaque_facets().len(),
            tail,
            &["provenance", "placement", "bytes", "media_kind"],
        ),
        _ => false,
    }
}

fn named_leaf_path_exists(segments: &[PathSegment], names: &[&str]) -> bool {
    match segments {
        [] => true,
        [segment] => segment.as_name().is_some_and(|name| names.contains(&name)),
        _ => false,
    }
}

fn indexed_struct_path_exists(
    length: usize,
    segments: &[PathSegment],
    field_names: &[&str],
) -> bool {
    let Some((selection, tail)) = segments.split_first() else {
        return true;
    };
    let Some(index) = selection.as_index().map(|value| value as usize) else {
        return false;
    };
    index < length && named_leaf_path_exists(tail, field_names)
}

fn canonical_fields_path_exists(fields: &[CanonicalField], segments: &[PathSegment]) -> bool {
    let Some((selection, tail)) = segments.split_first() else {
        return true;
    };
    let field = if let Some(name) = selection.as_name() {
        fields.iter().find(|field| field.name().as_str() == name)
    } else if let Some(index) = selection.as_index() {
        fields.get(index as usize)
    } else {
        None
    };
    field.is_some_and(|field| canonical_value_path_exists(field.value(), tail))
}

fn canonical_value_path_exists(value: &CanonicalValue, segments: &[PathSegment]) -> bool {
    let Some((selection, tail)) = segments.split_first() else {
        return true;
    };

    match value.kind() {
        CanonicalValueKind::Record(fields) => canonical_fields_path_exists(fields, segments),
        CanonicalValueKind::Sequence(values) => selection
            .as_index()
            .and_then(|index| values.get(index as usize))
            .is_some_and(|value| canonical_value_path_exists(value, tail)),
        CanonicalValueKind::Null
        | CanonicalValueKind::Bool(_)
        | CanonicalValueKind::Integer(_)
        | CanonicalValueKind::Decimal(_)
        | CanonicalValueKind::Text(_)
        | CanonicalValueKind::EnumToken(_)
        | CanonicalValueKind::Reference(_)
        | CanonicalValueKind::Binary(_)
        | CanonicalValueKind::AssetReference(_) => false,
    }
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
    use crate::value::{CanonicalField, CanonicalValue};

    use super::super::step::{
        MigrationApplyOutcome, MigrationApplyRequest, MigrationStep, MigrationStepId,
        MigrationStepOutput, ProfileConstraint,
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

    fn canonical_object(name: &str, suffix: u32, properties: &[&str]) -> CanonicalObject {
        let path = object_path(name);
        let identity = LogicalIdentity::new(
            ObjectUuid::parse(&format!("00000000-0000-0000-0000-{suffix:012x}")).unwrap(),
            path.clone(),
        );
        let provenance = SourceProvenance::new(
            ProfileId::parse("profile:source").unwrap(),
            CanonicalAnchor::new(path, PropertyPath::root()),
        );
        let mut parts =
            CanonicalObjectParts::new(identity, MetadataKind::new("Catalog").unwrap(), provenance);
        parts.properties = properties
            .iter()
            .map(|name| CanonicalField::named(name, CanonicalValue::boolean(true)).unwrap())
            .collect();
        CanonicalObject::new(parts).unwrap()
    }

    fn configuration() -> CanonicalConfiguration {
        CanonicalConfiguration::new(vec![
            canonical_object("Catalog.Test", 1, &["KnownFeature", "FutureFeature"]),
            canonical_object("Catalog.Second", 2, &["KnownFeature"]),
        ])
        .unwrap()
    }

    fn descriptor(touched: &[&str]) -> MigrationStepDescriptor {
        MigrationStepDescriptor::new(
            MigrationStepId::parse("migration:test-edge").unwrap(),
            ProfileConstraint::exact(ProfileId::parse("profile:source").unwrap()),
            ProfileConstraint::exact(ProfileId::parse("profile:target").unwrap()),
            touched
                .iter()
                .map(|capability| CapabilityId::parse(capability).unwrap())
                .collect(),
            Vec::new(),
        )
        .unwrap()
    }

    struct TestStep {
        requirements: CompatibilityRequirements,
    }

    impl MigrationStep for TestStep {
        fn descriptor(&self) -> &MigrationStepDescriptor {
            self.requirements.descriptor()
        }

        fn analyze(
            &self,
            request: MigrationAnalyzeRequest<'_>,
        ) -> Result<MigrationAnalysis, DiagnosticBuildError> {
            analyze_compatibility(&self.requirements, request)
        }

        fn apply(&self, request: MigrationApplyRequest<'_>) -> MigrationApplyOutcome {
            AdapterOutcome::success(
                MigrationStepOutput::new(request.configuration().clone(), Vec::new()).unwrap(),
            )
        }
    }

    fn step(
        touched: &[&str],
        requirements: Vec<CompatibilityRequirement>,
    ) -> Result<TestStep, CompatibilityContractError> {
        Ok(TestStep {
            requirements: CompatibilityRequirements::new(descriptor(touched), requirements)?,
        })
    }

    #[test]
    fn two_paths_for_one_unsupported_capability_produce_two_diagnostics() {
        let source = profile("profile:source", r#"{"feature:known":"supported"}"#);
        let target = profile("profile:target", r#"{"feature:known":"unsupported"}"#);
        let first_object = object_path("Catalog.Test");
        let second_object = object_path("Catalog.Second");
        let property = property_path("KnownFeature");
        let step = step(
            &["feature:known"],
            vec![
                CompatibilityRequirement::new(
                    CapabilityId::parse("feature:known").unwrap(),
                    first_object.clone(),
                    property.clone(),
                ),
                CompatibilityRequirement::new(
                    CapabilityId::parse("feature:known").unwrap(),
                    second_object.clone(),
                    property.clone(),
                ),
            ],
        )
        .unwrap();
        let configuration = configuration();

        let analysis = step
            .analyze(MigrationAnalyzeRequest::new(
                &source,
                &target,
                &configuration,
            ))
            .unwrap();

        assert!(!analysis.is_compatible());
        assert_eq!(analysis.diagnostics().diagnostics().len(), 2);
        assert!(
            analysis
                .diagnostics()
                .diagnostics()
                .iter()
                .all(|diagnostic| {
                    diagnostic.code().as_str() == CAPABILITY_UNSUPPORTED_CODE
                        && diagnostic.property_path() == &property
                })
        );
        let diagnosed_paths = analysis
            .diagnostics()
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.object_path())
            .collect::<Vec<_>>();
        assert!(diagnosed_paths.contains(&&first_object));
        assert!(diagnosed_paths.contains(&&second_object));
    }

    #[test]
    fn touched_capability_without_coverage_is_a_contract_error() {
        let result = step(
            &["feature:known", "feature:future"],
            vec![CompatibilityRequirement::global(
                CapabilityId::parse("feature:known").unwrap(),
            )],
        );
        assert!(matches!(
            result,
            Err(CompatibilityContractError::MissingCapabilityCoverage { capability })
                if capability == CapabilityId::parse("feature:future").unwrap()
        ));
    }

    #[test]
    fn requirement_capability_absent_from_descriptor_is_a_contract_error() {
        let result = step(
            &["feature:known"],
            vec![CompatibilityRequirement::global(
                CapabilityId::parse("feature:future").unwrap(),
            )],
        );
        assert!(matches!(
            result,
            Err(CompatibilityContractError::CapabilityNotTouched { capability })
                if capability == CapabilityId::parse("feature:future").unwrap()
        ));
    }

    #[test]
    fn requirement_limit_plus_one_fails_before_analysis() {
        let capability = CapabilityId::parse("feature:known").unwrap();
        let requirements =
            vec![CompatibilityRequirement::global(capability); MAX_COMPATIBILITY_REQUIREMENTS + 1];
        let result = CompatibilityRequirements::new(descriptor(&["feature:known"]), requirements);
        assert!(matches!(
            result,
            Err(CompatibilityContractError::TooManyRequirements {
                maximum: MAX_COMPATIBILITY_REQUIREMENTS,
                actual,
            }) if actual == MAX_COMPATIBILITY_REQUIREMENTS + 1
        ));
    }

    #[test]
    fn unknown_model_path_is_a_blocking_stable_diagnostic() {
        let source = profile("profile:source", r#"{"feature:known":"supported"}"#);
        let target = profile("profile:target", r#"{"feature:known":"supported"}"#);
        let unknown_object = object_path("Catalog.Missing");
        let unknown_property = property_path("MissingFeature");
        let step = step(
            &["feature:known"],
            vec![CompatibilityRequirement::new(
                CapabilityId::parse("feature:known").unwrap(),
                unknown_object.clone(),
                unknown_property.clone(),
            )],
        )
        .unwrap();
        let configuration = configuration();

        let analysis = step
            .analyze(MigrationAnalyzeRequest::new(
                &source,
                &target,
                &configuration,
            ))
            .unwrap();

        assert!(!analysis.is_compatible());
        assert_eq!(analysis.diagnostics().diagnostics().len(), 1);
        let diagnostic = &analysis.diagnostics().diagnostics()[0];
        assert_eq!(diagnostic.code().as_str(), REQUIREMENT_PATH_UNKNOWN_CODE);
        assert_eq!(diagnostic.object_path(), &unknown_object);
        assert_eq!(diagnostic.property_path(), &unknown_property);
    }

    #[test]
    fn analyze_cannot_change_model_or_semantic_digest() {
        fn assert_object_safe(_: &dyn MigrationStep) {}

        let source = profile("profile:source", r#"{"feature:known":"supported"}"#);
        let target = profile("profile:target", r#"{"feature:known":"supported"}"#);
        let step = step(
            &["feature:known"],
            vec![CompatibilityRequirement::new(
                CapabilityId::parse("feature:known").unwrap(),
                object_path("Catalog.Test"),
                property_path("KnownFeature"),
            )],
        )
        .unwrap();
        assert_object_safe(&step);
        let configuration = configuration();
        let snapshot = configuration.clone();
        let digest_before = validate_configuration(&configuration)
            .unwrap()
            .semantic_digest();

        let analysis = step
            .analyze(MigrationAnalyzeRequest::new(
                &source,
                &target,
                &configuration,
            ))
            .unwrap();

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
        assert_eq!(
            applied.value().map(MigrationStepOutput::configuration),
            Some(&snapshot)
        );
        assert_eq!(configuration, snapshot);
    }

    #[test]
    fn route_constraint_failures_are_stable_and_fail_closed() {
        let source = profile("profile:other-source", r#"{}"#);
        let target = profile("profile:other-target", r#"{}"#);
        let step = step(&[], Vec::new()).unwrap();
        let configuration = configuration();

        let analysis = step
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
