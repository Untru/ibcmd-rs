//! Bounded deterministic reports for planned and executed migration routes.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use serde::de::{IgnoredAny, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, de};

use crate::artifact::ProfileId;
use crate::diagnostic::{
    CodecLossDeclaration, CodecLossPermission, Diagnostic, DiagnosticCode, DiagnosticReport,
    LossDisposition, LossDispositionKind, LossPolicy, Severity,
};

use super::graph::MAX_MIGRATION_GRAPH_STEPS;
use super::step::MigrationStepId;

/// Maximum steps retained by one route report.
pub const MAX_MIGRATION_REPORT_STEPS: usize = MAX_MIGRATION_GRAPH_STEPS;
/// Maximum diagnostics retained for all phases of one step.
pub const MAX_MIGRATION_STEP_DIAGNOSTICS: usize = 4_096;
/// Maximum diagnostics retained by one composed migration report.
pub const MAX_MIGRATION_REPORT_DIAGNOSTICS: usize = 16_384;
/// Maximum actual losses retained for one step.
pub const MAX_MIGRATION_STEP_LOSS_EVIDENCE: usize = 1_024;
/// Maximum actual losses retained by one composed migration report.
pub const MAX_MIGRATION_REPORT_LOSS_EVIDENCE: usize = 4_096;
/// Current stable JSON schema version for migration execution reports.
pub const MIGRATION_REPORT_SCHEMA_VERSION: u32 = 1;
pub(crate) const APPLY_MISSING_VALUE_DIAGNOSTIC_CODE: &str = "migration.apply-missing-value";

/// Explicit terminal outcome of one migration operation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationOperationOutcome {
    /// Every planned step completed and validated successfully.
    Success,
    /// Execution stopped fail-closed without returning a migrated model.
    Failure,
}

/// Stable stage at which one route step failed.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationFailureStage {
    /// Immutable compatibility analysis.
    Analyze,
    /// Step application or its guarded value contract.
    Apply,
    /// Validation of actual loss declarations, diagnostics, and permissions.
    LossContract,
    /// Canonical validation of the candidate model.
    ResultValidation,
}

/// Stable machine-readable reason for a terminal step failure.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationFailureCode {
    /// Analysis could not construct its bounded diagnostics.
    AnalysisBuildFailed,
    /// Analysis emitted at least one blocking diagnostic.
    AnalysisBlocked,
    /// Apply emitted at least one blocking diagnostic.
    ApplyBlocked,
    /// Apply returned no value and no blocking diagnostic.
    ApplyMissingValue,
    /// An actual loss was absent from the step descriptor.
    UndeclaredAppliedLoss,
    /// An actual loss was absent from apply diagnostics.
    LossDiagnosticMissing,
    /// A declared loss warning had no actual disposition.
    LossDispositionMissing,
    /// The descriptor did not permit the actual disposition.
    LossPermissionDenied,
    /// The candidate canonical model was invalid.
    ResultValidationFailed,
}

impl MigrationFailureCode {
    const fn stage(self) -> MigrationFailureStage {
        match self {
            Self::AnalysisBuildFailed | Self::AnalysisBlocked => MigrationFailureStage::Analyze,
            Self::ApplyBlocked | Self::ApplyMissingValue => MigrationFailureStage::Apply,
            Self::UndeclaredAppliedLoss
            | Self::LossDiagnosticMissing
            | Self::LossDispositionMissing
            | Self::LossPermissionDenied => MigrationFailureStage::LossContract,
            Self::ResultValidationFailed => MigrationFailureStage::ResultValidation,
        }
    }
}

/// Explicit status of one attempted route step.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationStepStatus {
    /// Analyze, apply, and result validation all completed.
    Success,
    /// This step terminated the operation.
    Failure,
}

/// Failure attached to one failed step report.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MigrationStepFailure {
    stage: MigrationFailureStage,
    code: MigrationFailureCode,
}

impl<'de> Deserialize<'de> for MigrationStepFailure {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawFailure {
            stage: MigrationFailureStage,
            code: MigrationFailureCode,
        }

        let raw = RawFailure::deserialize(deserializer)?;
        Self::new(raw.stage, raw.code).map_err(de::Error::custom)
    }
}

impl MigrationStepFailure {
    /// Creates a stage/code pair only when both describe the same failure class.
    pub fn new(
        stage: MigrationFailureStage,
        code: MigrationFailureCode,
    ) -> Result<Self, MigrationReportError> {
        if code.stage() != stage {
            return Err(MigrationReportError::FailureStageCodeMismatch { stage, code });
        }
        Ok(Self { stage, code })
    }

    /// Returns the stable failure stage.
    pub const fn stage(&self) -> MigrationFailureStage {
        self.stage
    }

    /// Returns the stable failure code.
    pub const fn code(&self) -> MigrationFailureCode {
        self.code
    }
}

/// Terminal failure coordinates within the complete planned route.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MigrationTerminalFailure {
    stage: MigrationFailureStage,
    code: MigrationFailureCode,
    step_index: usize,
    completed_step_count: usize,
}

impl<'de> Deserialize<'de> for MigrationTerminalFailure {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawTerminalFailure {
            stage: MigrationFailureStage,
            code: MigrationFailureCode,
            step_index: usize,
            completed_step_count: usize,
        }

        let raw = RawTerminalFailure::deserialize(deserializer)?;
        Self::from_parts(
            raw.stage,
            raw.code,
            raw.step_index,
            raw.completed_step_count,
        )
        .map_err(de::Error::custom)
    }
}

impl MigrationTerminalFailure {
    /// Creates a checked terminal stage, reason, and zero-based route index.
    pub fn new(
        stage: MigrationFailureStage,
        code: MigrationFailureCode,
        step_index: usize,
    ) -> Result<Self, MigrationReportError> {
        Self::from_parts(stage, code, step_index, step_index)
    }

    fn from_parts(
        stage: MigrationFailureStage,
        code: MigrationFailureCode,
        step_index: usize,
        completed_step_count: usize,
    ) -> Result<Self, MigrationReportError> {
        MigrationStepFailure::new(stage, code)?;
        if completed_step_count != step_index {
            return Err(MigrationReportError::TerminalCompletedStepCountMismatch {
                step_index,
                completed_step_count,
            });
        }
        Ok(Self {
            stage,
            code,
            step_index,
            completed_step_count,
        })
    }

    /// Returns the terminal failure stage.
    pub const fn stage(&self) -> MigrationFailureStage {
        self.stage
    }

    /// Returns the terminal stable failure reason.
    pub const fn code(&self) -> MigrationFailureCode {
        self.code
    }

    /// Returns the zero-based index of the failed route step.
    pub const fn step_index(&self) -> usize {
        self.step_index
    }

    /// Returns how many route steps completed before this terminal step.
    pub const fn completed_step_count(&self) -> usize {
        self.completed_step_count
    }
}

/// Execution phase that produced actual loss evidence.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationLossPhase {
    /// Step application actually applied the declared loss.
    Apply,
}

/// Serializable observable form of an evaluated loss disposition.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationLossDisposition {
    /// The data was retained reversibly and execution continued with a warning.
    ContinueWithWarning,
    /// The data was explicitly and permissibly dropped.
    DroppedExplicitly,
}

impl From<LossDispositionKind> for MigrationLossDisposition {
    fn from(value: LossDispositionKind) -> Self {
        match value {
            LossDispositionKind::ContinueWithWarning => Self::ContinueWithWarning,
            LossDispositionKind::DroppedExplicitly => Self::DroppedExplicitly,
        }
    }
}

/// Machine-readable evidence for one exact evaluated loss at one path.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MigrationLossEvidence {
    phase: MigrationLossPhase,
    requested_policy: LossPolicy,
    actual_disposition: MigrationLossDisposition,
    declaration: CodecLossDeclaration,
    diagnostic: Diagnostic,
}

impl MigrationLossEvidence {
    pub(crate) fn from_disposition(
        phase: MigrationLossPhase,
        requested_policy: LossPolicy,
        disposition: &LossDisposition,
        declaration: &CodecLossDeclaration,
        source_profile: &ProfileId,
        target_profile: &ProfileId,
    ) -> Result<Self, MigrationReportError> {
        Self::from_parts(
            phase,
            requested_policy,
            disposition.kind().into(),
            declaration.clone(),
            disposition
                .diagnostic()
                .clone()
                .with_profiles(Some(source_profile.clone()), Some(target_profile.clone())),
        )
    }

    fn from_parts(
        phase: MigrationLossPhase,
        requested_policy: LossPolicy,
        actual_disposition: MigrationLossDisposition,
        declaration: CodecLossDeclaration,
        diagnostic: Diagnostic,
    ) -> Result<Self, MigrationReportError> {
        if diagnostic.code() != declaration.code() {
            return Err(MigrationReportError::LossCodeMismatch {
                diagnostic_code: diagnostic.code().clone(),
                declared_code: declaration.code().clone(),
            });
        }
        if diagnostic.severity() != Severity::Warning {
            return Err(MigrationReportError::LossDiagnosticNotWarning {
                code: diagnostic.code().clone(),
                actual: diagnostic.severity(),
            });
        }
        if diagnostic.source_profile().is_none() || diagnostic.target_profile().is_none() {
            return Err(MigrationReportError::LossProfilesMissing {
                code: diagnostic.code().clone(),
            });
        }
        let policy_matches = matches!(
            (requested_policy, actual_disposition),
            (
                LossPolicy::Warn,
                MigrationLossDisposition::ContinueWithWarning
            ) | (
                LossPolicy::DropExplicitly,
                MigrationLossDisposition::DroppedExplicitly
            )
        );
        if !policy_matches {
            return Err(MigrationReportError::LossPolicyMismatch {
                code: diagnostic.code().clone(),
                requested: requested_policy,
                actual: actual_disposition,
            });
        }
        if actual_disposition == MigrationLossDisposition::DroppedExplicitly
            && declaration.permission() != CodecLossPermission::DropAllowed
        {
            return Err(MigrationReportError::LossPermissionMismatch {
                code: diagnostic.code().clone(),
                permission: declaration.permission(),
                actual: actual_disposition,
            });
        }
        Ok(Self {
            phase,
            requested_policy,
            actual_disposition,
            declaration,
            diagnostic,
        })
    }

    /// Returns the phase that produced this actual loss.
    pub const fn phase(&self) -> MigrationLossPhase {
        self.phase
    }

    /// Returns the caller-requested loss policy.
    pub const fn requested_policy(&self) -> LossPolicy {
        self.requested_policy
    }

    /// Returns the actual evaluated disposition.
    pub const fn actual_disposition(&self) -> MigrationLossDisposition {
        self.actual_disposition
    }

    /// Returns the exact codec declaration that authorized the disposition.
    pub const fn declaration(&self) -> &CodecLossDeclaration {
        &self.declaration
    }

    /// Returns the normalized path-addressed warning diagnostic.
    pub const fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }
}

impl<'de> Deserialize<'de> for MigrationLossEvidence {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawEvidence {
            phase: MigrationLossPhase,
            requested_policy: LossPolicy,
            actual_disposition: MigrationLossDisposition,
            declaration: CodecLossDeclaration,
            diagnostic: Diagnostic,
        }

        let raw = RawEvidence::deserialize(deserializer)?;
        Self::from_parts(
            raw.phase,
            raw.requested_policy,
            raw.actual_disposition,
            raw.declaration,
            raw.diagnostic,
        )
        .map_err(de::Error::custom)
    }
}

/// Diagnostics and actual losses retained for one attempted route step.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MigrationStepReport {
    step_id: MigrationStepId,
    source_profile: ProfileId,
    target_profile: ProfileId,
    status: MigrationStepStatus,
    failure: Option<MigrationStepFailure>,
    analysis_diagnostics: DiagnosticReport,
    apply_diagnostics: DiagnosticReport,
    validation_diagnostics: DiagnosticReport,
    losses: Vec<MigrationLossEvidence>,
}

impl MigrationStepReport {
    /// Builds one bounded successful step report.
    #[allow(clippy::too_many_arguments)]
    pub fn successful(
        step_id: MigrationStepId,
        source_profile: ProfileId,
        target_profile: ProfileId,
        analysis_diagnostics: DiagnosticReport,
        apply_diagnostics: DiagnosticReport,
        validation_diagnostics: DiagnosticReport,
        losses: Vec<MigrationLossEvidence>,
    ) -> Result<Self, MigrationReportError> {
        Self::from_parts(
            step_id,
            source_profile,
            target_profile,
            MigrationStepStatus::Success,
            None,
            analysis_diagnostics,
            apply_diagnostics,
            validation_diagnostics,
            losses,
        )
    }

    /// Builds one bounded terminal failed-step report.
    #[allow(clippy::too_many_arguments)]
    pub fn failed(
        step_id: MigrationStepId,
        source_profile: ProfileId,
        target_profile: ProfileId,
        failure: MigrationStepFailure,
        analysis_diagnostics: DiagnosticReport,
        apply_diagnostics: DiagnosticReport,
        validation_diagnostics: DiagnosticReport,
        losses: Vec<MigrationLossEvidence>,
    ) -> Result<Self, MigrationReportError> {
        Self::from_parts(
            step_id,
            source_profile,
            target_profile,
            MigrationStepStatus::Failure,
            Some(failure),
            analysis_diagnostics,
            apply_diagnostics,
            validation_diagnostics,
            losses,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn from_parts(
        step_id: MigrationStepId,
        source_profile: ProfileId,
        target_profile: ProfileId,
        status: MigrationStepStatus,
        failure: Option<MigrationStepFailure>,
        analysis_diagnostics: DiagnosticReport,
        apply_diagnostics: DiagnosticReport,
        validation_diagnostics: DiagnosticReport,
        mut losses: Vec<MigrationLossEvidence>,
    ) -> Result<Self, MigrationReportError> {
        match (status, failure) {
            (MigrationStepStatus::Success, None) => {}
            (MigrationStepStatus::Failure, Some(value)) => {
                MigrationStepFailure::new(value.stage(), value.code())?;
            }
            _ => {
                return Err(MigrationReportError::StepOutcomeMismatch {
                    step_id,
                    status,
                    has_failure: failure.is_some(),
                });
            }
        }
        for report in [
            &analysis_diagnostics,
            &apply_diagnostics,
            &validation_diagnostics,
        ] {
            for diagnostic in report.diagnostics() {
                if diagnostic.source_profile() != Some(&source_profile)
                    || diagnostic.target_profile() != Some(&target_profile)
                {
                    return Err(MigrationReportError::DiagnosticProfileMismatch {
                        step_id,
                        code: diagnostic.code().clone(),
                    });
                }
            }
        }
        if status == MigrationStepStatus::Success {
            if failure.is_some() {
                return Err(MigrationReportError::StepOutcomeMismatch {
                    step_id,
                    status,
                    has_failure: true,
                });
            }
            {
                if analysis_diagnostics.has_errors()
                    || apply_diagnostics.has_errors()
                    || validation_diagnostics.has_errors()
                {
                    return Err(MigrationReportError::SuccessfulStepHasErrors { step_id });
                }
            }
        } else if let Some(value) = failure {
            let analysis_is_empty = analysis_diagnostics.diagnostics().is_empty();
            let apply_is_empty = apply_diagnostics.diagnostics().is_empty();
            let validation_is_empty = validation_diagnostics.diagnostics().is_empty();
            let loss_contract_shape = !analysis_diagnostics.has_errors()
                && !apply_diagnostics.has_errors()
                && validation_is_empty
                && losses.is_empty();
            let shape_is_valid = match value.code() {
                MigrationFailureCode::AnalysisBuildFailed => {
                    analysis_is_empty && apply_is_empty && validation_is_empty && losses.is_empty()
                }
                MigrationFailureCode::AnalysisBlocked => {
                    analysis_diagnostics.has_errors()
                        && apply_is_empty
                        && validation_is_empty
                        && losses.is_empty()
                }
                MigrationFailureCode::ApplyBlocked => {
                    !analysis_diagnostics.has_errors()
                        && apply_diagnostics.has_errors()
                        && validation_is_empty
                        && losses.is_empty()
                }
                MigrationFailureCode::ApplyMissingValue => {
                    !analysis_diagnostics.has_errors()
                        && missing_value_apply_diagnostics_are_valid(&apply_diagnostics)
                        && validation_is_empty
                        && losses.is_empty()
                }
                MigrationFailureCode::UndeclaredAppliedLoss
                | MigrationFailureCode::LossDiagnosticMissing
                | MigrationFailureCode::LossPermissionDenied => loss_contract_shape,
                MigrationFailureCode::LossDispositionMissing => {
                    loss_contract_shape
                        && apply_diagnostics
                            .diagnostics()
                            .iter()
                            .any(|diagnostic| diagnostic.severity() == Severity::Warning)
                }
                MigrationFailureCode::ResultValidationFailed => {
                    !analysis_diagnostics.has_errors()
                        && !apply_diagnostics.has_errors()
                        && validation_diagnostics.has_errors()
                }
            };
            if !shape_is_valid {
                return Err(MigrationReportError::FailurePhaseShapeMismatch {
                    step_id,
                    stage: value.stage(),
                });
            }
        } else {
            return Err(MigrationReportError::StepOutcomeMismatch {
                step_id,
                status,
                has_failure: false,
            });
        }
        let diagnostic_count = checked_diagnostic_count([
            &analysis_diagnostics,
            &apply_diagnostics,
            &validation_diagnostics,
        ])?;
        if diagnostic_count > MAX_MIGRATION_STEP_DIAGNOSTICS {
            return Err(MigrationReportError::TooManyStepDiagnostics {
                step_id,
                maximum: MAX_MIGRATION_STEP_DIAGNOSTICS,
                actual: diagnostic_count,
            });
        }
        if losses.len() > MAX_MIGRATION_STEP_LOSS_EVIDENCE {
            return Err(MigrationReportError::TooManyStepLosses {
                step_id,
                maximum: MAX_MIGRATION_STEP_LOSS_EVIDENCE,
                actual: losses.len(),
            });
        }
        for loss in &losses {
            if loss.diagnostic().source_profile() != Some(&source_profile)
                || loss.diagnostic().target_profile() != Some(&target_profile)
            {
                return Err(MigrationReportError::LossProfileMismatch {
                    step_id,
                    code: loss.diagnostic().code().clone(),
                });
            }
        }
        let mut matched_apply_diagnostics = vec![false; apply_diagnostics.diagnostics().len()];
        for loss in &losses {
            let Some((index, _)) =
                apply_diagnostics
                    .diagnostics()
                    .iter()
                    .enumerate()
                    .find(|(index, diagnostic)| {
                        !matched_apply_diagnostics[*index] && *diagnostic == loss.diagnostic()
                    })
            else {
                return Err(MigrationReportError::LossEvidenceDiagnosticMissing {
                    step_id,
                    code: loss.diagnostic().code().clone(),
                });
            };
            matched_apply_diagnostics[index] = true;
        }
        losses.sort_by(compare_loss_evidence);
        Ok(Self {
            step_id,
            source_profile,
            target_profile,
            status,
            failure,
            analysis_diagnostics,
            apply_diagnostics,
            validation_diagnostics,
            losses,
        })
    }

    /// Returns the stable attempted step identifier.
    pub const fn step_id(&self) -> &MigrationStepId {
        &self.step_id
    }

    /// Returns this step's exact source profile.
    pub const fn source_profile(&self) -> &ProfileId {
        &self.source_profile
    }

    /// Returns this step's exact target profile.
    pub const fn target_profile(&self) -> &ProfileId {
        &self.target_profile
    }

    /// Returns whether this step completed or terminated the operation.
    pub const fn status(&self) -> MigrationStepStatus {
        self.status
    }

    /// Returns the explicit failure stage and code for a failed step.
    pub const fn failure(&self) -> Option<&MigrationStepFailure> {
        self.failure.as_ref()
    }

    /// Returns immutable compatibility-analysis diagnostics.
    pub const fn analysis_diagnostics(&self) -> &DiagnosticReport {
        &self.analysis_diagnostics
    }

    /// Returns diagnostics emitted while applying the step.
    pub const fn apply_diagnostics(&self) -> &DiagnosticReport {
        &self.apply_diagnostics
    }

    /// Returns diagnostics emitted while validating the candidate result.
    pub const fn validation_diagnostics(&self) -> &DiagnosticReport {
        &self.validation_diagnostics
    }

    /// Returns actual loss evidence in deterministic path/code order.
    pub fn losses(&self) -> &[MigrationLossEvidence] {
        &self.losses
    }

    /// Returns all diagnostics retained by this step.
    pub fn diagnostic_count(&self) -> usize {
        self.analysis_diagnostics.diagnostics().len()
            + self.apply_diagnostics.diagnostics().len()
            + self.validation_diagnostics.diagnostics().len()
    }
}

impl<'de> Deserialize<'de> for MigrationStepReport {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawStepReport {
            step_id: MigrationStepId,
            source_profile: ProfileId,
            target_profile: ProfileId,
            status: MigrationStepStatus,
            failure: Option<MigrationStepFailure>,
            analysis_diagnostics: BoundedDiagnosticReport,
            apply_diagnostics: BoundedDiagnosticReport,
            validation_diagnostics: BoundedDiagnosticReport,
            losses: BoundedVec<MigrationLossEvidence, MAX_MIGRATION_STEP_LOSS_EVIDENCE>,
        }

        let raw = RawStepReport::deserialize(deserializer)?;
        Self::from_parts(
            raw.step_id,
            raw.source_profile,
            raw.target_profile,
            raw.status,
            raw.failure,
            raw.analysis_diagnostics.0,
            raw.apply_diagnostics.0,
            raw.validation_diagnostics.0,
            raw.losses.0,
        )
        .map_err(de::Error::custom)
    }
}

/// Stable composed report for one complete route or attempted route prefix.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MigrationReport {
    schema_version: u32,
    source_profile: ProfileId,
    target_profile: ProfileId,
    requested_policy: LossPolicy,
    outcome: MigrationOperationOutcome,
    terminal_failure: Option<MigrationTerminalFailure>,
    completed_step_count: usize,
    route: Vec<MigrationStepId>,
    steps: Vec<MigrationStepReport>,
}

impl MigrationReport {
    /// Builds a self-contained successful full-route report.
    pub fn successful(
        source_profile: ProfileId,
        target_profile: ProfileId,
        requested_policy: LossPolicy,
        route: Vec<MigrationStepId>,
        steps: Vec<MigrationStepReport>,
    ) -> Result<Self, MigrationReportError> {
        let completed_step_count = steps.len();
        Self::from_parts(
            MIGRATION_REPORT_SCHEMA_VERSION,
            source_profile,
            target_profile,
            requested_policy,
            MigrationOperationOutcome::Success,
            None,
            completed_step_count,
            route,
            steps,
        )
    }

    /// Builds a self-contained failed report including its terminal step.
    #[allow(clippy::too_many_arguments)]
    pub fn failed(
        source_profile: ProfileId,
        target_profile: ProfileId,
        requested_policy: LossPolicy,
        route: Vec<MigrationStepId>,
        steps: Vec<MigrationStepReport>,
        terminal_failure: MigrationTerminalFailure,
    ) -> Result<Self, MigrationReportError> {
        Self::from_parts(
            MIGRATION_REPORT_SCHEMA_VERSION,
            source_profile,
            target_profile,
            requested_policy,
            MigrationOperationOutcome::Failure,
            Some(terminal_failure),
            terminal_failure.step_index(),
            route,
            steps,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn from_parts(
        schema_version: u32,
        source_profile: ProfileId,
        target_profile: ProfileId,
        requested_policy: LossPolicy,
        outcome: MigrationOperationOutcome,
        terminal_failure: Option<MigrationTerminalFailure>,
        completed_step_count: usize,
        route: Vec<MigrationStepId>,
        steps: Vec<MigrationStepReport>,
    ) -> Result<Self, MigrationReportError> {
        if schema_version != MIGRATION_REPORT_SCHEMA_VERSION {
            return Err(MigrationReportError::UnsupportedSchemaVersion {
                expected: MIGRATION_REPORT_SCHEMA_VERSION,
                actual: schema_version,
            });
        }
        if route.len() > MAX_MIGRATION_REPORT_STEPS {
            return Err(MigrationReportError::TooManyRouteSteps {
                maximum: MAX_MIGRATION_REPORT_STEPS,
                actual: route.len(),
            });
        }
        if steps.len() > route.len() {
            return Err(MigrationReportError::TooManyStepReports {
                maximum: route.len(),
                actual: steps.len(),
            });
        }
        let mut seen = BTreeSet::new();
        for step_id in &route {
            if !seen.insert(step_id) {
                return Err(MigrationReportError::DuplicateRouteStep {
                    step_id: step_id.clone(),
                });
            }
        }
        if route.is_empty() && source_profile != target_profile {
            return Err(MigrationReportError::EmptyRouteProfileMismatch {
                source_profile,
                target_profile,
            });
        }

        let mut expected_source = &source_profile;
        for (index, step) in steps.iter().enumerate() {
            if route.get(index) != Some(step.step_id()) {
                return Err(MigrationReportError::StepOrderMismatch {
                    index,
                    expected: route.get(index).cloned(),
                    actual: step.step_id().clone(),
                });
            }
            if step.source_profile() != expected_source {
                return Err(MigrationReportError::StepContinuityMismatch {
                    step_id: step.step_id().clone(),
                    expected_source: expected_source.clone(),
                    actual_source: step.source_profile().clone(),
                });
            }
            for loss in step.losses() {
                if loss.requested_policy() != requested_policy {
                    return Err(MigrationReportError::LossRequestedPolicyMismatch {
                        step_id: step.step_id().clone(),
                        code: loss.diagnostic().code().clone(),
                        report_policy: requested_policy,
                        evidence_policy: loss.requested_policy(),
                    });
                }
            }
            expected_source = step.target_profile();
        }
        if steps.len() == route.len() && expected_source != &target_profile {
            return Err(MigrationReportError::CompleteRouteTargetMismatch {
                expected_target: target_profile,
                actual_target: expected_source.clone(),
            });
        }

        let diagnostic_count = steps.iter().try_fold(0_usize, |total, step| {
            total.checked_add(step.diagnostic_count()).ok_or(
                MigrationReportError::ReportCountOverflow {
                    field: "diagnostics",
                },
            )
        })?;
        if diagnostic_count > MAX_MIGRATION_REPORT_DIAGNOSTICS {
            return Err(MigrationReportError::TooManyReportDiagnostics {
                maximum: MAX_MIGRATION_REPORT_DIAGNOSTICS,
                actual: diagnostic_count,
            });
        }
        let loss_count = steps.iter().try_fold(0_usize, |total, step| {
            total
                .checked_add(step.losses().len())
                .ok_or(MigrationReportError::ReportCountOverflow { field: "losses" })
        })?;
        if loss_count > MAX_MIGRATION_REPORT_LOSS_EVIDENCE {
            return Err(MigrationReportError::TooManyReportLosses {
                maximum: MAX_MIGRATION_REPORT_LOSS_EVIDENCE,
                actual: loss_count,
            });
        }

        match (outcome, terminal_failure) {
            (MigrationOperationOutcome::Success, None) => {
                if completed_step_count != route.len() || steps.len() != route.len() {
                    return Err(MigrationReportError::CompletedStepCountMismatch {
                        outcome,
                        expected: route.len(),
                        actual: completed_step_count,
                    });
                }
                if let Some(step) = steps
                    .iter()
                    .find(|step| step.status() != MigrationStepStatus::Success)
                {
                    return Err(MigrationReportError::UnexpectedStepStatus {
                        step_id: step.step_id().clone(),
                        expected: MigrationStepStatus::Success,
                        actual: step.status(),
                    });
                }
            }
            (MigrationOperationOutcome::Failure, Some(terminal)) => {
                MigrationTerminalFailure::new(
                    terminal.stage(),
                    terminal.code(),
                    terminal.step_index(),
                )?;
                if terminal.completed_step_count() != completed_step_count {
                    return Err(MigrationReportError::TerminalCompletedStepCountMismatch {
                        step_index: terminal.step_index(),
                        completed_step_count: terminal.completed_step_count(),
                    });
                }
                if terminal.step_index() >= route.len() {
                    return Err(MigrationReportError::FailureStepOutOfRange {
                        step_index: terminal.step_index(),
                        route_steps: route.len(),
                    });
                }
                if completed_step_count != terminal.step_index() {
                    return Err(MigrationReportError::CompletedStepCountMismatch {
                        outcome,
                        expected: terminal.step_index(),
                        actual: completed_step_count,
                    });
                }
                let expected_reports = terminal.step_index().checked_add(1).ok_or(
                    MigrationReportError::ReportCountOverflow {
                        field: "failed step reports",
                    },
                )?;
                if steps.len() != expected_reports {
                    return Err(MigrationReportError::FailedReportLengthMismatch {
                        expected: expected_reports,
                        actual: steps.len(),
                    });
                }
                for step in &steps[..terminal.step_index()] {
                    if step.status() != MigrationStepStatus::Success {
                        return Err(MigrationReportError::UnexpectedStepStatus {
                            step_id: step.step_id().clone(),
                            expected: MigrationStepStatus::Success,
                            actual: step.status(),
                        });
                    }
                }
                let failed_step = &steps[terminal.step_index()];
                if failed_step.status() != MigrationStepStatus::Failure {
                    return Err(MigrationReportError::UnexpectedStepStatus {
                        step_id: failed_step.step_id().clone(),
                        expected: MigrationStepStatus::Failure,
                        actual: failed_step.status(),
                    });
                }
                let Some(step_failure) = failed_step.failure() else {
                    return Err(MigrationReportError::StepFailureMismatch {
                        step_id: failed_step.step_id().clone(),
                    });
                };
                if step_failure.stage() != terminal.stage()
                    || step_failure.code() != terminal.code()
                {
                    return Err(MigrationReportError::StepFailureMismatch {
                        step_id: failed_step.step_id().clone(),
                    });
                }
            }
            _ => {
                return Err(MigrationReportError::OperationOutcomeMismatch {
                    outcome,
                    has_terminal_failure: terminal_failure.is_some(),
                });
            }
        }

        Ok(Self {
            schema_version,
            source_profile,
            target_profile,
            requested_policy,
            outcome,
            terminal_failure,
            completed_step_count,
            route,
            steps,
        })
    }

    /// Returns the stable JSON report schema version.
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the exact overall source profile.
    pub const fn source_profile(&self) -> &ProfileId {
        &self.source_profile
    }

    /// Returns the exact overall target profile.
    pub const fn target_profile(&self) -> &ProfileId {
        &self.target_profile
    }

    /// Returns the operation-wide requested loss policy.
    pub const fn requested_policy(&self) -> LossPolicy {
        self.requested_policy
    }

    /// Returns the explicit terminal operation outcome.
    pub const fn outcome(&self) -> MigrationOperationOutcome {
        self.outcome
    }

    /// Returns terminal failure coordinates for a failed operation.
    pub const fn terminal_failure(&self) -> Option<&MigrationTerminalFailure> {
        self.terminal_failure.as_ref()
    }

    /// Returns the number of fully successful steps before termination.
    pub const fn completed_step_count(&self) -> usize {
        self.completed_step_count
    }

    /// Returns all planned step identifiers in order.
    pub fn route(&self) -> &[MigrationStepId] {
        &self.route
    }

    /// Returns the attempted route prefix with per-step evidence.
    pub fn steps(&self) -> &[MigrationStepReport] {
        &self.steps
    }

    /// Returns true only for a successful fully reported route.
    pub fn is_complete(&self) -> bool {
        self.outcome == MigrationOperationOutcome::Success
            && self.completed_step_count == self.route.len()
            && self.steps.len() == self.route.len()
    }
}

impl<'de> Deserialize<'de> for MigrationReport {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawReport {
            schema_version: u32,
            source_profile: ProfileId,
            target_profile: ProfileId,
            requested_policy: LossPolicy,
            outcome: MigrationOperationOutcome,
            terminal_failure: Option<MigrationTerminalFailure>,
            completed_step_count: usize,
            route: BoundedVec<MigrationStepId, MAX_MIGRATION_REPORT_STEPS>,
            steps: BoundedStepReports,
        }

        let raw = RawReport::deserialize(deserializer)?;
        Self::from_parts(
            raw.schema_version,
            raw.source_profile,
            raw.target_profile,
            raw.requested_policy,
            raw.outcome,
            raw.terminal_failure,
            raw.completed_step_count,
            raw.route.0,
            raw.steps.0,
        )
        .map_err(de::Error::custom)
    }
}

/// Stable typed failure to construct or deserialize a migration report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MigrationReportError {
    /// The JSON schema is not this reader's exact supported revision.
    UnsupportedSchemaVersion { expected: u32, actual: u32 },
    /// Planned route exceeds its explicit bound.
    TooManyRouteSteps { maximum: usize, actual: usize },
    /// Attempted step reports exceed the planned route prefix.
    TooManyStepReports { maximum: usize, actual: usize },
    /// One step retained too many diagnostics.
    TooManyStepDiagnostics {
        step_id: MigrationStepId,
        maximum: usize,
        actual: usize,
    },
    /// The composed report retained too many diagnostics.
    TooManyReportDiagnostics { maximum: usize, actual: usize },
    /// One step retained too much actual loss evidence.
    TooManyStepLosses {
        step_id: MigrationStepId,
        maximum: usize,
        actual: usize,
    },
    /// The composed report retained too much actual loss evidence.
    TooManyReportLosses { maximum: usize, actual: usize },
    /// A checked report count overflowed.
    ReportCountOverflow { field: &'static str },
    /// A stable failure code was paired with a different stage.
    FailureStageCodeMismatch {
        stage: MigrationFailureStage,
        code: MigrationFailureCode,
    },
    /// Step status and optional failure details contradicted one another.
    StepOutcomeMismatch {
        step_id: MigrationStepId,
        status: MigrationStepStatus,
        has_failure: bool,
    },
    /// A per-step diagnostic omitted or contradicted exact route profiles.
    DiagnosticProfileMismatch {
        step_id: MigrationStepId,
        code: DiagnosticCode,
    },
    /// A successful step retained a blocking diagnostic.
    SuccessfulStepHasErrors { step_id: MigrationStepId },
    /// Diagnostics/losses were present in a phase that never ran.
    FailurePhaseShapeMismatch {
        step_id: MigrationStepId,
        stage: MigrationFailureStage,
    },
    /// Loss evidence had no identical warning in apply diagnostics.
    LossEvidenceDiagnosticMissing {
        step_id: MigrationStepId,
        code: DiagnosticCode,
    },
    /// A route repeated a step identifier.
    DuplicateRouteStep { step_id: MigrationStepId },
    /// An empty route named different endpoints.
    EmptyRouteProfileMismatch {
        source_profile: ProfileId,
        target_profile: ProfileId,
    },
    /// A step report did not match its route position.
    StepOrderMismatch {
        index: usize,
        expected: Option<MigrationStepId>,
        actual: MigrationStepId,
    },
    /// Consecutive step reports did not share one exact intermediate profile.
    StepContinuityMismatch {
        step_id: MigrationStepId,
        expected_source: ProfileId,
        actual_source: ProfileId,
    },
    /// A complete report ended at a profile other than its target.
    CompleteRouteTargetMismatch {
        expected_target: ProfileId,
        actual_target: ProfileId,
    },
    /// Operation outcome and terminal failure presence contradicted one another.
    OperationOutcomeMismatch {
        outcome: MigrationOperationOutcome,
        has_terminal_failure: bool,
    },
    /// Successful or failed reports declared the wrong completed-step count.
    CompletedStepCountMismatch {
        outcome: MigrationOperationOutcome,
        expected: usize,
        actual: usize,
    },
    /// Terminal metadata disagreed with the failed step index.
    TerminalCompletedStepCountMismatch {
        step_index: usize,
        completed_step_count: usize,
    },
    /// Terminal failure index was outside the planned route.
    FailureStepOutOfRange {
        step_index: usize,
        route_steps: usize,
    },
    /// A failed report did not contain exactly the successful prefix and failed step.
    FailedReportLengthMismatch { expected: usize, actual: usize },
    /// A successful prefix or failed terminal step had the wrong status.
    UnexpectedStepStatus {
        step_id: MigrationStepId,
        expected: MigrationStepStatus,
        actual: MigrationStepStatus,
    },
    /// Terminal and per-step failure details differed.
    StepFailureMismatch { step_id: MigrationStepId },
    /// Loss diagnostic and declaration codes differed.
    LossCodeMismatch {
        diagnostic_code: DiagnosticCode,
        declared_code: DiagnosticCode,
    },
    /// Evaluated loss evidence did not contain a normalized warning.
    LossDiagnosticNotWarning {
        code: DiagnosticCode,
        actual: Severity,
    },
    /// Requested and actual loss dispositions contradicted each other.
    LossPolicyMismatch {
        code: DiagnosticCode,
        requested: LossPolicy,
        actual: MigrationLossDisposition,
    },
    /// Explicit drop was not permitted by the retained descriptor declaration.
    LossPermissionMismatch {
        code: DiagnosticCode,
        permission: CodecLossPermission,
        actual: MigrationLossDisposition,
    },
    /// Per-loss and operation-wide requested policies differed.
    LossRequestedPolicyMismatch {
        step_id: MigrationStepId,
        code: DiagnosticCode,
        report_policy: LossPolicy,
        evidence_policy: LossPolicy,
    },
    /// Loss evidence omitted this step's exact profiles.
    LossProfileMismatch {
        step_id: MigrationStepId,
        code: DiagnosticCode,
    },
    /// Standalone loss evidence omitted an exact source or target profile.
    LossProfilesMissing { code: DiagnosticCode },
}

impl MigrationReportError {
    /// Returns a stable machine-readable error code.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::UnsupportedSchemaVersion { .. } => "migration.report-unsupported-schema-version",
            Self::TooManyRouteSteps { .. } => "migration.report-too-many-route-steps",
            Self::TooManyStepReports { .. } => "migration.report-too-many-step-reports",
            Self::TooManyStepDiagnostics { .. } => "migration.report-too-many-step-diagnostics",
            Self::TooManyReportDiagnostics { .. } => "migration.report-too-many-diagnostics",
            Self::TooManyStepLosses { .. } => "migration.report-too-many-step-losses",
            Self::TooManyReportLosses { .. } => "migration.report-too-many-losses",
            Self::ReportCountOverflow { .. } => "migration.report-count-overflow",
            Self::FailureStageCodeMismatch { .. } => "migration.report-failure-stage-code-mismatch",
            Self::StepOutcomeMismatch { .. } => "migration.report-step-outcome-mismatch",
            Self::DiagnosticProfileMismatch { .. } => {
                "migration.report-diagnostic-profile-mismatch"
            }
            Self::SuccessfulStepHasErrors { .. } => "migration.report-successful-step-has-errors",
            Self::FailurePhaseShapeMismatch { .. } => {
                "migration.report-failure-phase-shape-mismatch"
            }
            Self::LossEvidenceDiagnosticMissing { .. } => {
                "migration.report-loss-evidence-diagnostic-missing"
            }
            Self::DuplicateRouteStep { .. } => "migration.report-duplicate-route-step",
            Self::EmptyRouteProfileMismatch { .. } => {
                "migration.report-empty-route-profile-mismatch"
            }
            Self::StepOrderMismatch { .. } => "migration.report-step-order-mismatch",
            Self::StepContinuityMismatch { .. } => "migration.report-step-continuity-mismatch",
            Self::CompleteRouteTargetMismatch { .. } => "migration.report-complete-target-mismatch",
            Self::OperationOutcomeMismatch { .. } => "migration.report-operation-outcome-mismatch",
            Self::CompletedStepCountMismatch { .. } => {
                "migration.report-completed-step-count-mismatch"
            }
            Self::TerminalCompletedStepCountMismatch { .. } => {
                "migration.report-terminal-completed-step-count-mismatch"
            }
            Self::FailureStepOutOfRange { .. } => "migration.report-failure-step-out-of-range",
            Self::FailedReportLengthMismatch { .. } => "migration.report-failed-length-mismatch",
            Self::UnexpectedStepStatus { .. } => "migration.report-unexpected-step-status",
            Self::StepFailureMismatch { .. } => "migration.report-step-failure-mismatch",
            Self::LossCodeMismatch { .. } => "migration.report-loss-code-mismatch",
            Self::LossDiagnosticNotWarning { .. } => "migration.report-loss-diagnostic-not-warning",
            Self::LossPolicyMismatch { .. } => "migration.report-loss-policy-mismatch",
            Self::LossPermissionMismatch { .. } => "migration.report-loss-permission-mismatch",
            Self::LossRequestedPolicyMismatch { .. } => {
                "migration.report-loss-requested-policy-mismatch"
            }
            Self::LossProfileMismatch { .. } => "migration.report-loss-profile-mismatch",
            Self::LossProfilesMissing { .. } => "migration.report-loss-profiles-missing",
        }
    }
}

impl Display for MigrationReportError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSchemaVersion { expected, actual } => write!(
                formatter,
                "migration report schema version {actual} is unsupported (expected {expected})"
            ),
            Self::TooManyRouteSteps { maximum, actual } => write!(
                formatter,
                "migration report exceeds {maximum} route steps (actual {actual})"
            ),
            Self::TooManyStepReports { maximum, actual } => write!(
                formatter,
                "migration report exceeds its {maximum}-step route (actual {actual})"
            ),
            Self::TooManyStepDiagnostics {
                step_id,
                maximum,
                actual,
            } => write!(
                formatter,
                "migration step `{step_id}` report exceeds {maximum} diagnostics (actual {actual})"
            ),
            Self::TooManyReportDiagnostics { maximum, actual } => write!(
                formatter,
                "migration report exceeds {maximum} diagnostics (actual {actual})"
            ),
            Self::TooManyStepLosses {
                step_id,
                maximum,
                actual,
            } => write!(
                formatter,
                "migration step `{step_id}` report exceeds {maximum} losses (actual {actual})"
            ),
            Self::TooManyReportLosses { maximum, actual } => write!(
                formatter,
                "migration report exceeds {maximum} losses (actual {actual})"
            ),
            Self::ReportCountOverflow { field } => {
                write!(formatter, "migration report {field} count overflowed")
            }
            Self::FailureStageCodeMismatch { stage, code } => write!(
                formatter,
                "migration failure code {code:?} does not belong to stage {stage:?}"
            ),
            Self::StepOutcomeMismatch {
                step_id,
                status,
                has_failure,
            } => write!(
                formatter,
                "migration step `{step_id}` status {status:?} contradicts failure presence {has_failure}"
            ),
            Self::DiagnosticProfileMismatch { step_id, code } => write!(
                formatter,
                "diagnostic `{code}` does not carry exact profiles for migration step `{step_id}`"
            ),
            Self::SuccessfulStepHasErrors { step_id } => write!(
                formatter,
                "successful migration step `{step_id}` contains a blocking diagnostic"
            ),
            Self::FailurePhaseShapeMismatch { step_id, stage } => write!(
                formatter,
                "migration step `{step_id}` contains data after terminal stage {stage:?}"
            ),
            Self::LossEvidenceDiagnosticMissing { step_id, code } => write!(
                formatter,
                "migration step `{step_id}` loss evidence `{code}` is absent from apply diagnostics"
            ),
            Self::DuplicateRouteStep { step_id } => {
                write!(formatter, "migration report repeats route step `{step_id}`")
            }
            Self::EmptyRouteProfileMismatch {
                source_profile,
                target_profile,
            } => write!(
                formatter,
                "empty migration route cannot connect `{source_profile}` to `{target_profile}`"
            ),
            Self::StepOrderMismatch {
                index,
                expected,
                actual,
            } => write!(
                formatter,
                "migration report step {index} expected {:?}, found `{actual}`",
                expected.as_ref().map(MigrationStepId::as_str)
            ),
            Self::StepContinuityMismatch {
                step_id,
                expected_source,
                actual_source,
            } => write!(
                formatter,
                "migration step `{step_id}` expected source `{expected_source}`, found `{actual_source}`"
            ),
            Self::CompleteRouteTargetMismatch {
                expected_target,
                actual_target,
            } => write!(
                formatter,
                "complete migration report expected target `{expected_target}`, found `{actual_target}`"
            ),
            Self::OperationOutcomeMismatch {
                outcome,
                has_terminal_failure,
            } => write!(
                formatter,
                "migration outcome {outcome:?} contradicts terminal failure presence {has_terminal_failure}"
            ),
            Self::CompletedStepCountMismatch {
                outcome,
                expected,
                actual,
            } => write!(
                formatter,
                "migration outcome {outcome:?} expects {expected} completed steps, found {actual}"
            ),
            Self::TerminalCompletedStepCountMismatch {
                step_index,
                completed_step_count,
            } => write!(
                formatter,
                "migration terminal step index {step_index} contradicts completed step count {completed_step_count}"
            ),
            Self::FailureStepOutOfRange {
                step_index,
                route_steps,
            } => write!(
                formatter,
                "migration failure step index {step_index} is outside {route_steps}-step route"
            ),
            Self::FailedReportLengthMismatch { expected, actual } => write!(
                formatter,
                "failed migration report expects {expected} step reports, found {actual}"
            ),
            Self::UnexpectedStepStatus {
                step_id,
                expected,
                actual,
            } => write!(
                formatter,
                "migration step `{step_id}` expected status {expected:?}, found {actual:?}"
            ),
            Self::StepFailureMismatch { step_id } => write!(
                formatter,
                "migration step `{step_id}` failure differs from terminal failure"
            ),
            Self::LossCodeMismatch {
                diagnostic_code,
                declared_code,
            } => write!(
                formatter,
                "loss diagnostic `{diagnostic_code}` does not match declaration `{declared_code}`"
            ),
            Self::LossDiagnosticNotWarning { code, actual } => write!(
                formatter,
                "evaluated loss `{code}` must be a warning, found {actual:?}"
            ),
            Self::LossPolicyMismatch {
                code,
                requested,
                actual,
            } => write!(
                formatter,
                "evaluated loss `{code}` disposition {actual:?} contradicts requested policy {requested:?}"
            ),
            Self::LossPermissionMismatch {
                code,
                permission,
                actual,
            } => write!(
                formatter,
                "evaluated loss `{code}` disposition {actual:?} is not permitted by {permission:?}"
            ),
            Self::LossRequestedPolicyMismatch {
                step_id,
                code,
                report_policy,
                evidence_policy,
            } => write!(
                formatter,
                "migration step `{step_id}` loss `{code}` policy {evidence_policy:?} differs from report policy {report_policy:?}"
            ),
            Self::LossProfileMismatch { step_id, code } => write!(
                formatter,
                "loss `{code}` does not carry exact profiles for migration step `{step_id}`"
            ),
            Self::LossProfilesMissing { code } => {
                write!(
                    formatter,
                    "loss `{code}` omits an exact source or target profile"
                )
            }
        }
    }
}

impl Error for MigrationReportError {}

fn compare_loss_evidence(
    left: &MigrationLossEvidence,
    right: &MigrationLossEvidence,
) -> std::cmp::Ordering {
    left.diagnostic()
        .code()
        .cmp(right.diagnostic().code())
        .then_with(|| {
            left.diagnostic()
                .object_path()
                .cmp(right.diagnostic().object_path())
        })
        .then_with(|| {
            left.diagnostic()
                .property_path()
                .cmp(right.diagnostic().property_path())
        })
        .then_with(|| {
            left.diagnostic()
                .source_profile()
                .cmp(&right.diagnostic().source_profile())
        })
        .then_with(|| {
            left.diagnostic()
                .target_profile()
                .cmp(&right.diagnostic().target_profile())
        })
        .then_with(|| {
            loss_policy_rank(left.requested_policy).cmp(&loss_policy_rank(right.requested_policy))
        })
        .then_with(|| {
            loss_disposition_rank(left.actual_disposition)
                .cmp(&loss_disposition_rank(right.actual_disposition))
        })
        .then_with(|| {
            loss_permission_rank(left.declaration.permission())
                .cmp(&loss_permission_rank(right.declaration.permission()))
        })
        .then_with(|| left.declaration.reason().cmp(right.declaration.reason()))
        .then_with(|| {
            severity_rank(left.diagnostic().severity())
                .cmp(&severity_rank(right.diagnostic().severity()))
        })
        .then_with(|| {
            left.diagnostic()
                .message()
                .cmp(right.diagnostic().message())
        })
        .then_with(|| {
            left.diagnostic()
                .recovery_hint()
                .cmp(&right.diagnostic().recovery_hint())
        })
        .then_with(|| {
            left.diagnostic()
                .context()
                .cmp(right.diagnostic().context())
        })
}

const fn loss_policy_rank(policy: LossPolicy) -> u8 {
    match policy {
        LossPolicy::Error => 0,
        LossPolicy::Warn => 1,
        LossPolicy::DropExplicitly => 2,
    }
}

const fn loss_disposition_rank(disposition: MigrationLossDisposition) -> u8 {
    match disposition {
        MigrationLossDisposition::ContinueWithWarning => 0,
        MigrationLossDisposition::DroppedExplicitly => 1,
    }
}

const fn loss_permission_rank(permission: crate::diagnostic::CodecLossPermission) -> u8 {
    match permission {
        crate::diagnostic::CodecLossPermission::WarnOnly => 0,
        crate::diagnostic::CodecLossPermission::DropAllowed => 1,
    }
}

const fn severity_rank(severity: Severity) -> u8 {
    match severity {
        Severity::Error => 0,
        Severity::Warning => 1,
        Severity::Info => 2,
    }
}

fn checked_diagnostic_count<const N: usize>(
    reports: [&DiagnosticReport; N],
) -> Result<usize, MigrationReportError> {
    reports.iter().try_fold(0_usize, |total, report| {
        total.checked_add(report.diagnostics().len()).ok_or(
            MigrationReportError::ReportCountOverflow {
                field: "diagnostics",
            },
        )
    })
}

fn missing_value_apply_diagnostics_are_valid(report: &DiagnosticReport) -> bool {
    let mut errors = report
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.severity() == Severity::Error);
    match errors.next() {
        None => true,
        Some(diagnostic) => {
            diagnostic.code().as_str() == APPLY_MISSING_VALUE_DIAGNOSTIC_CODE
                && errors.next().is_none()
        }
    }
}

struct BoundedVec<T, const MAXIMUM: usize>(Vec<T>);

impl<'de, T, const MAXIMUM: usize> Deserialize<'de> for BoundedVec<T, MAXIMUM>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(BoundedVecVisitor::<T, MAXIMUM>(std::marker::PhantomData))
    }
}

struct BoundedVecVisitor<T, const MAXIMUM: usize>(std::marker::PhantomData<fn() -> T>);

impl<'de, T, const MAXIMUM: usize> Visitor<'de> for BoundedVecVisitor<T, MAXIMUM>
where
    T: Deserialize<'de>,
{
    type Value = BoundedVec<T, MAXIMUM>;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "a sequence containing at most {MAXIMUM} items")
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::with_capacity(sequence.size_hint().unwrap_or(0).min(MAXIMUM));
        while values.len() < MAXIMUM {
            let Some(value) = sequence.next_element()? else {
                return Ok(BoundedVec(values));
            };
            values.push(value);
        }
        if sequence.next_element::<IgnoredAny>()?.is_some() {
            return Err(de::Error::custom(format_args!(
                "sequence contains more than {MAXIMUM} items"
            )));
        }
        Ok(BoundedVec(values))
    }
}

struct BoundedDiagnosticReport(DiagnosticReport);

impl<'de> Deserialize<'de> for BoundedDiagnosticReport {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawDiagnostics {
            diagnostics: BoundedVec<Diagnostic, MAX_MIGRATION_STEP_DIAGNOSTICS>,
        }

        let raw = RawDiagnostics::deserialize(deserializer)?;
        Ok(Self(DiagnosticReport::from_diagnostics(raw.diagnostics.0)))
    }
}

struct BoundedStepReports(Vec<MigrationStepReport>);

impl<'de> Deserialize<'de> for BoundedStepReports {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(BoundedStepReportsVisitor)
    }
}

struct BoundedStepReportsVisitor;

impl<'de> Visitor<'de> for BoundedStepReportsVisitor {
    type Value = BoundedStepReports;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "at most {MAX_MIGRATION_REPORT_STEPS} bounded migration step reports"
        )
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::with_capacity(
            sequence
                .size_hint()
                .unwrap_or(0)
                .min(MAX_MIGRATION_REPORT_STEPS),
        );
        let mut diagnostic_count = 0_usize;
        let mut loss_count = 0_usize;
        while values.len() < MAX_MIGRATION_REPORT_STEPS {
            let Some(value) = sequence.next_element::<MigrationStepReport>()? else {
                return Ok(BoundedStepReports(values));
            };
            diagnostic_count = diagnostic_count
                .checked_add(value.diagnostic_count())
                .ok_or_else(|| de::Error::custom("migration report diagnostic count overflowed"))?;
            if diagnostic_count > MAX_MIGRATION_REPORT_DIAGNOSTICS {
                return Err(de::Error::custom(format_args!(
                    "migration report contains more than {MAX_MIGRATION_REPORT_DIAGNOSTICS} diagnostics"
                )));
            }
            loss_count = loss_count
                .checked_add(value.losses().len())
                .ok_or_else(|| de::Error::custom("migration report loss count overflowed"))?;
            if loss_count > MAX_MIGRATION_REPORT_LOSS_EVIDENCE {
                return Err(de::Error::custom(format_args!(
                    "migration report contains more than {MAX_MIGRATION_REPORT_LOSS_EVIDENCE} losses"
                )));
            }
            values.push(value);
        }
        if sequence.next_element::<IgnoredAny>()?.is_some() {
            return Err(de::Error::custom(format_args!(
                "migration report contains more than {MAX_MIGRATION_REPORT_STEPS} step reports"
            )));
        }
        Ok(BoundedStepReports(values))
    }
}

#[cfg(test)]
mod tests {
    use crate::artifact::ProfileId;
    use crate::diagnostic::{
        Diagnostic, DiagnosticCode, DiagnosticReport, ObjectPath, PropertyPath, Severity,
    };

    use super::*;

    fn profile(value: &str) -> ProfileId {
        ProfileId::parse(value).unwrap()
    }

    fn step_id(value: &str) -> MigrationStepId {
        MigrationStepId::parse(value).unwrap()
    }

    fn successful_step(id: &str, source: &str, target: &str) -> MigrationStepReport {
        MigrationStepReport::successful(
            step_id(id),
            profile(source),
            profile(target),
            DiagnosticReport::new(),
            DiagnosticReport::new(),
            DiagnosticReport::new(),
            Vec::new(),
        )
        .unwrap()
    }

    fn analysis_build_failed_step(id: &str, source: &str, target: &str) -> MigrationStepReport {
        MigrationStepReport::failed(
            step_id(id),
            profile(source),
            profile(target),
            MigrationStepFailure::new(
                MigrationFailureStage::Analyze,
                MigrationFailureCode::AnalysisBuildFailed,
            )
            .unwrap(),
            DiagnosticReport::new(),
            DiagnosticReport::new(),
            DiagnosticReport::new(),
            Vec::new(),
        )
        .unwrap()
    }

    fn diagnostic_report(
        code: &str,
        severity: Severity,
        source: &str,
        target: &str,
    ) -> DiagnosticReport {
        let diagnostic = Diagnostic::new(
            DiagnosticCode::parse(code).unwrap(),
            severity,
            ObjectPath::root(),
            PropertyPath::root(),
            "migration report invariant test diagnostic",
        )
        .unwrap()
        .with_profiles(Some(profile(source)), Some(profile(target)));
        DiagnosticReport::from_diagnostics(vec![diagnostic])
    }

    fn failed_report(
        stage: MigrationFailureStage,
        code: MigrationFailureCode,
        analysis_diagnostics: DiagnosticReport,
        apply_diagnostics: DiagnosticReport,
        validation_diagnostics: DiagnosticReport,
    ) -> MigrationReport {
        let failure = MigrationStepFailure::new(stage, code).unwrap();
        let step = MigrationStepReport::failed(
            step_id("migration:a-b"),
            profile("profile:a"),
            profile("profile:b"),
            failure,
            analysis_diagnostics,
            apply_diagnostics,
            validation_diagnostics,
            Vec::new(),
        )
        .unwrap();
        MigrationReport::failed(
            profile("profile:a"),
            profile("profile:b"),
            LossPolicy::Error,
            vec![step_id("migration:a-b")],
            vec![step],
            MigrationTerminalFailure::new(stage, code, 0).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn successful_report_json_is_stable_golden_and_roundtrips() {
        let report = MigrationReport::successful(
            profile("profile:a"),
            profile("profile:b"),
            LossPolicy::Error,
            vec![step_id("migration:a-b")],
            vec![successful_step("migration:a-b", "profile:a", "profile:b")],
        )
        .unwrap();

        let json = serde_json::to_string(&report).unwrap();
        assert_eq!(
            json,
            r#"{"schema_version":1,"source_profile":"profile:a","target_profile":"profile:b","requested_policy":"error","outcome":"success","terminal_failure":null,"completed_step_count":1,"route":["migration:a-b"],"steps":[{"step_id":"migration:a-b","source_profile":"profile:a","target_profile":"profile:b","status":"success","failure":null,"analysis_diagnostics":{"diagnostics":[]},"apply_diagnostics":{"diagnostics":[]},"validation_diagnostics":{"diagnostics":[]},"losses":[]}]}"#
        );
        assert!(report.is_complete());
        assert_eq!(
            serde_json::from_str::<MigrationReport>(&json).unwrap(),
            report
        );
    }

    #[test]
    fn failed_report_json_is_stable_golden_and_roundtrips() {
        let terminal = MigrationTerminalFailure::new(
            MigrationFailureStage::Analyze,
            MigrationFailureCode::AnalysisBuildFailed,
            0,
        )
        .unwrap();
        let report = MigrationReport::failed(
            profile("profile:a"),
            profile("profile:b"),
            LossPolicy::Warn,
            vec![step_id("migration:a-b")],
            vec![analysis_build_failed_step(
                "migration:a-b",
                "profile:a",
                "profile:b",
            )],
            terminal,
        )
        .unwrap();

        let json = serde_json::to_string(&report).unwrap();
        assert_eq!(
            json,
            r#"{"schema_version":1,"source_profile":"profile:a","target_profile":"profile:b","requested_policy":"warn","outcome":"failure","terminal_failure":{"stage":"analyze","code":"analysis_build_failed","step_index":0,"completed_step_count":0},"completed_step_count":0,"route":["migration:a-b"],"steps":[{"step_id":"migration:a-b","source_profile":"profile:a","target_profile":"profile:b","status":"failure","failure":{"stage":"analyze","code":"analysis_build_failed"},"analysis_diagnostics":{"diagnostics":[]},"apply_diagnostics":{"diagnostics":[]},"validation_diagnostics":{"diagnostics":[]},"losses":[]}]}"#
        );
        assert!(!report.is_complete());
        assert_eq!(report.completed_step_count(), 0);
        assert_eq!(
            serde_json::from_str::<MigrationReport>(&json).unwrap(),
            report
        );
    }

    #[test]
    fn report_json_rejects_unknown_schema_and_inconsistent_outcome_fields() {
        let report = MigrationReport::successful(
            profile("profile:a"),
            profile("profile:b"),
            LossPolicy::Error,
            vec![step_id("migration:a-b")],
            vec![successful_step("migration:a-b", "profile:a", "profile:b")],
        )
        .unwrap();
        let mut value = serde_json::to_value(report).unwrap();

        value["schema_version"] = serde_json::json!(2);
        assert!(serde_json::from_value::<MigrationReport>(value.clone()).is_err());

        value["schema_version"] = serde_json::json!(MIGRATION_REPORT_SCHEMA_VERSION);
        value["completed_step_count"] = serde_json::json!(0);
        assert!(serde_json::from_value::<MigrationReport>(value.clone()).is_err());

        value["completed_step_count"] = serde_json::json!(1);
        value["unknown"] = serde_json::json!(true);
        assert!(serde_json::from_value::<MigrationReport>(value).is_err());
    }

    #[test]
    fn report_json_rejects_blocked_codes_without_their_phase_error() {
        let analysis_blocked = failed_report(
            MigrationFailureStage::Analyze,
            MigrationFailureCode::AnalysisBlocked,
            diagnostic_report(
                "migration.test-analysis-blocked",
                Severity::Error,
                "profile:a",
                "profile:b",
            ),
            DiagnosticReport::new(),
            DiagnosticReport::new(),
        );
        let mut analysis_json = serde_json::to_value(analysis_blocked).unwrap();
        analysis_json["steps"][0]["analysis_diagnostics"]["diagnostics"] = serde_json::json!([]);
        assert!(serde_json::from_value::<MigrationReport>(analysis_json).is_err());

        let apply_blocked = failed_report(
            MigrationFailureStage::Apply,
            MigrationFailureCode::ApplyBlocked,
            DiagnosticReport::new(),
            diagnostic_report(
                "migration.test-apply-blocked",
                Severity::Error,
                "profile:a",
                "profile:b",
            ),
            DiagnosticReport::new(),
        );
        let mut apply_json = serde_json::to_value(apply_blocked).unwrap();
        apply_json["steps"][0]["apply_diagnostics"]["diagnostics"] = serde_json::json!([]);
        assert!(serde_json::from_value::<MigrationReport>(apply_json).is_err());
    }

    #[test]
    fn constructors_reject_code_specific_phase_contradictions() {
        let cases = [
            (
                MigrationFailureStage::Analyze,
                MigrationFailureCode::AnalysisBlocked,
                DiagnosticReport::new(),
                DiagnosticReport::new(),
                DiagnosticReport::new(),
            ),
            (
                MigrationFailureStage::Apply,
                MigrationFailureCode::ApplyBlocked,
                DiagnosticReport::new(),
                DiagnosticReport::new(),
                DiagnosticReport::new(),
            ),
            (
                MigrationFailureStage::Apply,
                MigrationFailureCode::ApplyMissingValue,
                DiagnosticReport::new(),
                diagnostic_report(
                    "migration.test-arbitrary-apply-error",
                    Severity::Error,
                    "profile:a",
                    "profile:b",
                ),
                DiagnosticReport::new(),
            ),
            (
                MigrationFailureStage::LossContract,
                MigrationFailureCode::LossDiagnosticMissing,
                DiagnosticReport::new(),
                diagnostic_report(
                    "migration.test-loss-contract-error",
                    Severity::Error,
                    "profile:a",
                    "profile:b",
                ),
                DiagnosticReport::new(),
            ),
            (
                MigrationFailureStage::ResultValidation,
                MigrationFailureCode::ResultValidationFailed,
                diagnostic_report(
                    "migration.test-early-error",
                    Severity::Error,
                    "profile:a",
                    "profile:b",
                ),
                DiagnosticReport::new(),
                diagnostic_report(
                    "migration.test-validation-error",
                    Severity::Error,
                    "profile:a",
                    "profile:b",
                ),
            ),
        ];

        for (stage, code, analysis, apply, validation) in cases {
            let result = MigrationStepReport::failed(
                step_id("migration:a-b"),
                profile("profile:a"),
                profile("profile:b"),
                MigrationStepFailure::new(stage, code).unwrap(),
                analysis,
                apply,
                validation,
                Vec::new(),
            );
            assert!(matches!(
                result,
                Err(MigrationReportError::FailurePhaseShapeMismatch { .. })
            ));
        }
    }

    #[test]
    fn report_rejects_discontinuous_or_duplicate_routes() {
        let duplicate = MigrationReport::successful(
            profile("profile:a"),
            profile("profile:b"),
            LossPolicy::Error,
            vec![step_id("migration:a-b"), step_id("migration:a-b")],
            Vec::new(),
        );
        assert!(matches!(
            duplicate,
            Err(MigrationReportError::DuplicateRouteStep { .. })
        ));

        let wrong_step = successful_step("migration:b-c", "profile:x", "profile:c");
        let discontinuous = MigrationReport::successful(
            profile("profile:a"),
            profile("profile:c"),
            LossPolicy::Error,
            vec![step_id("migration:b-c")],
            vec![wrong_step],
        );
        assert!(matches!(
            discontinuous,
            Err(MigrationReportError::StepContinuityMismatch { .. })
        ));
    }

    #[test]
    fn route_limit_plus_one_is_rejected_before_route_processing() {
        let repeated = step_id("migration:a-b");
        let report = MigrationReport::successful(
            profile("profile:a"),
            profile("profile:b"),
            LossPolicy::Error,
            vec![repeated; MAX_MIGRATION_REPORT_STEPS + 1],
            Vec::new(),
        );
        assert!(matches!(
            report,
            Err(MigrationReportError::TooManyRouteSteps {
                maximum: MAX_MIGRATION_REPORT_STEPS,
                actual,
            }) if actual == MAX_MIGRATION_REPORT_STEPS + 1
        ));
    }
}
