//! Object-safe migration-step contracts and immutable requests.

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

use serde::de::Visitor;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use crate::adapter::AdapterOutcome;
use crate::artifact::{ParseIdentifierError, ProfileId};
use crate::diagnostic::{CodecLossDeclaration, DiagnosticCode, DiagnosticReport, LossPolicy};
use crate::model::CanonicalConfiguration;
use crate::profile::{CapabilityId, CapabilityState, EffectiveProfile};

/// Maximum capability constraints or declarations retained by one migration contract.
pub const MAX_MIGRATION_CAPABILITIES: usize = 1_024;
/// Maximum possible losses declared by one migration step.
pub const MAX_MIGRATION_LOSSES: usize = 1_024;

/// Stable open identifier for one migration step.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MigrationStepId(ProfileId);

impl MigrationStepId {
    /// Validates a migration-step identifier without consulting a closed registry.
    pub fn new(value: &str) -> Result<Self, ParseIdentifierError> {
        ProfileId::new(value).map(Self)
    }

    /// Parses a bounded migration-step identifier.
    pub fn parse(value: &str) -> Result<Self, ParseIdentifierError> {
        Self::new(value)
    }

    /// Returns the exact stable identifier.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl Display for MigrationStepId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for MigrationStepId {
    type Err = ParseIdentifierError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl Serialize for MigrationStepId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for MigrationStepId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(MigrationStepIdVisitor)
    }
}

struct MigrationStepIdVisitor;

impl Visitor<'_> for MigrationStepIdVisitor {
    type Value = MigrationStepId;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("a bounded open migration-step identifier")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        MigrationStepId::parse(value).map_err(E::custom)
    }
}

/// An exact profile plus capabilities that must be supported by that profile.
///
/// Exact matching deliberately prevents a migration planner from selecting a
/// nearest profile. Additional constraint forms can be introduced without
/// weakening this fail-closed baseline.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProfileConstraint {
    exact_profile: ProfileId,
    required_capabilities: Vec<CapabilityId>,
}

impl ProfileConstraint {
    /// Constrains a route endpoint to one exact profile.
    pub const fn exact(exact_profile: ProfileId) -> Self {
        Self {
            exact_profile,
            required_capabilities: Vec::new(),
        }
    }

    /// Constrains an exact profile and its independently declared capabilities.
    pub fn with_capabilities(
        exact_profile: ProfileId,
        required_capabilities: Vec<CapabilityId>,
    ) -> Result<Self, MigrationContractError> {
        Ok(Self {
            exact_profile,
            required_capabilities: canonical_capabilities(
                "profile constraint capabilities",
                required_capabilities,
            )?,
        })
    }

    /// Returns the mandatory exact profile identifier.
    pub const fn exact_profile(&self) -> &ProfileId {
        &self.exact_profile
    }

    /// Returns required capabilities in deterministic identifier order.
    pub fn required_capabilities(&self) -> &[CapabilityId] {
        &self.required_capabilities
    }

    /// Evaluates this constraint without profile approximation or inference.
    pub fn matches(&self, profile: &EffectiveProfile) -> bool {
        profile.id == self.exact_profile
            && self.required_capabilities.iter().all(|capability| {
                profile
                    .capabilities
                    .get(capability)
                    .is_some_and(|state| state.value == CapabilityState::Supported)
            })
    }
}

/// Failure to build a deterministic bounded migration descriptor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MigrationContractError {
    /// A capability collection exceeded its explicit bound.
    TooManyCapabilities {
        /// Logical descriptor field.
        field: &'static str,
        /// Maximum accepted declarations.
        maximum: usize,
        /// Actual declarations.
        actual: usize,
    },
    /// The same exact capability was declared more than once.
    DuplicateCapability {
        /// Logical descriptor field.
        field: &'static str,
        /// Duplicate capability.
        capability: CapabilityId,
    },
    /// Possible-loss declarations exceeded their explicit bound.
    TooManyLosses {
        /// Maximum accepted declarations.
        maximum: usize,
        /// Actual declarations.
        actual: usize,
    },
    /// The same exact possible-loss code was declared more than once.
    DuplicateLoss {
        /// Duplicate stable diagnostic code.
        code: DiagnosticCode,
    },
}

impl Display for MigrationContractError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyCapabilities {
                field,
                maximum,
                actual,
            } => write!(
                formatter,
                "{field} exceeds {maximum} capabilities (actual {actual})"
            ),
            Self::DuplicateCapability { field, capability } => {
                write!(formatter, "duplicate {field} capability `{capability}`")
            }
            Self::TooManyLosses { maximum, actual } => write!(
                formatter,
                "migration descriptor exceeds {maximum} possible losses (actual {actual})"
            ),
            Self::DuplicateLoss { code } => {
                write!(formatter, "duplicate possible migration loss `{code}`")
            }
        }
    }
}

impl Error for MigrationContractError {}

fn canonical_capabilities(
    field: &'static str,
    mut capabilities: Vec<CapabilityId>,
) -> Result<Vec<CapabilityId>, MigrationContractError> {
    if capabilities.len() > MAX_MIGRATION_CAPABILITIES {
        return Err(MigrationContractError::TooManyCapabilities {
            field,
            maximum: MAX_MIGRATION_CAPABILITIES,
            actual: capabilities.len(),
        });
    }
    capabilities.sort();
    if let Some(pair) = capabilities.windows(2).find(|pair| pair[0] == pair[1]) {
        return Err(MigrationContractError::DuplicateCapability {
            field,
            capability: pair[0].clone(),
        });
    }
    Ok(capabilities)
}

fn canonical_losses(
    mut losses: Vec<CodecLossDeclaration>,
) -> Result<Vec<CodecLossDeclaration>, MigrationContractError> {
    if losses.len() > MAX_MIGRATION_LOSSES {
        return Err(MigrationContractError::TooManyLosses {
            maximum: MAX_MIGRATION_LOSSES,
            actual: losses.len(),
        });
    }
    losses.sort_by(|left, right| left.code().cmp(right.code()));
    if let Some(pair) = losses
        .windows(2)
        .find(|pair| pair[0].code() == pair[1].code())
    {
        return Err(MigrationContractError::DuplicateLoss {
            code: pair[0].code().clone(),
        });
    }
    Ok(losses)
}

/// Immutable declaration of one directed migration edge.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MigrationStepDescriptor {
    id: MigrationStepId,
    source: ProfileConstraint,
    target: ProfileConstraint,
    touched_capabilities: Vec<CapabilityId>,
    possible_losses: Vec<CodecLossDeclaration>,
}

impl MigrationStepDescriptor {
    /// Validates, sorts, and retains all migration declarations.
    pub fn new(
        id: MigrationStepId,
        source: ProfileConstraint,
        target: ProfileConstraint,
        touched_capabilities: Vec<CapabilityId>,
        possible_losses: Vec<CodecLossDeclaration>,
    ) -> Result<Self, MigrationContractError> {
        Ok(Self {
            id,
            source,
            target,
            touched_capabilities: canonical_capabilities(
                "touched capabilities",
                touched_capabilities,
            )?,
            possible_losses: canonical_losses(possible_losses)?,
        })
    }

    /// Returns the exact stable step identifier.
    pub const fn id(&self) -> &MigrationStepId {
        &self.id
    }

    /// Returns the source constraint for this directed edge.
    pub const fn source(&self) -> &ProfileConstraint {
        &self.source
    }

    /// Returns the target constraint for this directed edge.
    pub const fn target(&self) -> &ProfileConstraint {
        &self.target
    }

    /// Returns touched capabilities in deterministic identifier order.
    pub fn touched_capabilities(&self) -> &[CapabilityId] {
        &self.touched_capabilities
    }

    /// Returns declared possible losses in deterministic code order.
    pub fn possible_losses(&self) -> &[CodecLossDeclaration] {
        &self.possible_losses
    }
}

/// Immutable input for migration analysis.
#[derive(Clone, Copy, Debug)]
pub struct MigrationAnalyzeRequest<'a> {
    source_profile: &'a EffectiveProfile,
    target_profile: &'a EffectiveProfile,
    configuration: &'a CanonicalConfiguration,
}

impl<'a> MigrationAnalyzeRequest<'a> {
    /// Creates an analysis request over a borrowed immutable model.
    pub const fn new(
        source_profile: &'a EffectiveProfile,
        target_profile: &'a EffectiveProfile,
        configuration: &'a CanonicalConfiguration,
    ) -> Self {
        Self {
            source_profile,
            target_profile,
            configuration,
        }
    }

    /// Returns the exact effective source profile.
    pub const fn source_profile(&self) -> &'a EffectiveProfile {
        self.source_profile
    }

    /// Returns the exact effective target profile.
    pub const fn target_profile(&self) -> &'a EffectiveProfile {
        self.target_profile
    }

    /// Returns the borrowed immutable canonical model.
    pub const fn configuration(&self) -> &'a CanonicalConfiguration {
        self.configuration
    }
}

/// Immutable input for applying one already selected migration step.
#[derive(Clone, Copy, Debug)]
pub struct MigrationApplyRequest<'a> {
    source_profile: &'a EffectiveProfile,
    target_profile: &'a EffectiveProfile,
    configuration: &'a CanonicalConfiguration,
    loss_policy: LossPolicy,
}

impl<'a> MigrationApplyRequest<'a> {
    /// Creates an apply request that must produce a separate configuration.
    pub const fn new(
        source_profile: &'a EffectiveProfile,
        target_profile: &'a EffectiveProfile,
        configuration: &'a CanonicalConfiguration,
        loss_policy: LossPolicy,
    ) -> Self {
        Self {
            source_profile,
            target_profile,
            configuration,
            loss_policy,
        }
    }

    /// Returns the exact effective source profile.
    pub const fn source_profile(&self) -> &'a EffectiveProfile {
        self.source_profile
    }

    /// Returns the exact effective target profile.
    pub const fn target_profile(&self) -> &'a EffectiveProfile {
        self.target_profile
    }

    /// Returns the borrowed source model; implementations return a new model.
    pub const fn configuration(&self) -> &'a CanonicalConfiguration {
        self.configuration
    }

    /// Returns the caller-selected fail-closed loss policy.
    pub const fn loss_policy(&self) -> LossPolicy {
        self.loss_policy
    }
}

/// Complete diagnostics produced without mutating the analyzed model.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct MigrationAnalysis {
    diagnostics: DiagnosticReport,
}

impl MigrationAnalysis {
    /// Creates an analysis from a complete canonical report.
    pub const fn new(diagnostics: DiagnosticReport) -> Self {
        Self { diagnostics }
    }

    /// Returns the complete deterministic report.
    pub const fn diagnostics(&self) -> &DiagnosticReport {
        &self.diagnostics
    }

    /// Returns whether analysis found no blocking diagnostic.
    pub fn is_compatible(&self) -> bool {
        !self.diagnostics.has_errors()
    }
}

/// Guarded result of applying one migration step to a new model.
pub type MigrationApplyOutcome = AdapterOutcome<CanonicalConfiguration>;

/// Object-safe contract for one explicit directed migration edge.
///
/// `analyze` can only borrow an immutable model. `apply` also receives a
/// borrowed model and must return a distinct owned configuration, leaving
/// transaction orchestration and graph planning to later layers.
pub trait MigrationStep: Send + Sync {
    /// Returns immutable route, capability, and possible-loss declarations.
    fn descriptor(&self) -> &MigrationStepDescriptor;

    /// Analyzes compatibility without mutating the source configuration.
    fn analyze(&self, request: MigrationAnalyzeRequest<'_>) -> MigrationAnalysis;

    /// Applies this step and returns a guarded independently owned model.
    fn apply(&self, request: MigrationApplyRequest<'_>) -> MigrationApplyOutcome;
}

#[cfg(test)]
mod tests {
    use crate::diagnostic::{CodecLossPermission, DiagnosticCode};
    use crate::profile::{ProfileSourceKind, parse_profile_source, resolve_profiles};

    use super::*;

    fn profile(value: &str, capabilities: &str) -> EffectiveProfile {
        let json = format!(
            r#"{{"schema_version":1,"id":"{value}","status":"experimental","capabilities":{capabilities}}}"#
        );
        let document =
            parse_profile_source("test-profile.json", ProfileSourceKind::Bundled, &json).unwrap();
        resolve_profiles([document])
            .unwrap()
            .get(&ProfileId::parse(value).unwrap())
            .unwrap()
            .clone()
    }

    #[test]
    fn constraints_and_descriptor_are_exact_sorted_and_bounded() {
        let capability_a = CapabilityId::parse("feature:a").unwrap();
        let capability_z = CapabilityId::parse("feature:z").unwrap();
        let source_constraint = ProfileConstraint::with_capabilities(
            ProfileId::parse("profile:source").unwrap(),
            vec![capability_z.clone(), capability_a.clone()],
        )
        .unwrap();
        let source = profile(
            "profile:source",
            r#"{"feature:a":"supported","feature:z":"supported"}"#,
        );
        assert!(source_constraint.matches(&source));
        assert_eq!(
            source_constraint.required_capabilities(),
            [capability_a.clone(), capability_z.clone()]
        );

        let loss_z = CodecLossDeclaration::new(
            DiagnosticCode::parse("migration.loss-z").unwrap(),
            CodecLossPermission::DropAllowed,
            "loss z",
        )
        .unwrap();
        let loss_a = CodecLossDeclaration::new(
            DiagnosticCode::parse("migration.loss-a").unwrap(),
            CodecLossPermission::WarnOnly,
            "loss a",
        )
        .unwrap();
        let descriptor = MigrationStepDescriptor::new(
            MigrationStepId::parse("migration:source-to-target").unwrap(),
            source_constraint,
            ProfileConstraint::exact(ProfileId::parse("profile:target").unwrap()),
            vec![capability_z, capability_a],
            vec![loss_z, loss_a],
        )
        .unwrap();
        assert_eq!(descriptor.touched_capabilities()[0].as_str(), "feature:a");
        assert_eq!(
            descriptor.possible_losses()[0].code().as_str(),
            "migration.loss-a"
        );
    }

    #[test]
    fn duplicate_contract_declarations_are_rejected() {
        let capability = CapabilityId::parse("feature:duplicate").unwrap();
        assert!(matches!(
            ProfileConstraint::with_capabilities(
                ProfileId::parse("profile:source").unwrap(),
                vec![capability.clone(), capability.clone()],
            ),
            Err(MigrationContractError::DuplicateCapability { .. })
        ));

        let loss = CodecLossDeclaration::new(
            DiagnosticCode::parse("migration.duplicate-loss").unwrap(),
            CodecLossPermission::DropAllowed,
            "duplicate test",
        )
        .unwrap();
        assert!(matches!(
            MigrationStepDescriptor::new(
                MigrationStepId::parse("migration:duplicate-test").unwrap(),
                ProfileConstraint::exact(ProfileId::parse("profile:source").unwrap()),
                ProfileConstraint::exact(ProfileId::parse("profile:target").unwrap()),
                Vec::new(),
                vec![loss.clone(), loss],
            ),
            Err(MigrationContractError::DuplicateLoss { .. })
        ));
    }

    #[test]
    fn step_identifier_is_open_and_string_serialized() {
        let id = MigrationStepId::parse("migration:future-edge").unwrap();
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"migration:future-edge\"");
        assert_eq!(serde_json::from_str::<MigrationStepId>(&json).unwrap(), id);
    }
}
