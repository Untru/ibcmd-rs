//! Bounded deterministic reports for planned and executed migration routes.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use serde::de::{IgnoredAny, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, de};

use crate::artifact::ProfileId;
use crate::diagnostic::{
    CodecLossDeclaration, Diagnostic, DiagnosticCode, DiagnosticReport, LossDisposition,
    LossDispositionKind, LossPolicy, Severity,
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
    analysis_diagnostics: DiagnosticReport,
    apply_diagnostics: DiagnosticReport,
    validation_diagnostics: DiagnosticReport,
    losses: Vec<MigrationLossEvidence>,
}

impl MigrationStepReport {
    /// Builds one bounded deterministic step report.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        step_id: MigrationStepId,
        source_profile: ProfileId,
        target_profile: ProfileId,
        analysis_diagnostics: DiagnosticReport,
        apply_diagnostics: DiagnosticReport,
        validation_diagnostics: DiagnosticReport,
        mut losses: Vec<MigrationLossEvidence>,
    ) -> Result<Self, MigrationReportError> {
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
        losses.sort_by(compare_loss_evidence);
        Ok(Self {
            step_id,
            source_profile,
            target_profile,
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
            analysis_diagnostics: BoundedDiagnosticReport,
            apply_diagnostics: BoundedDiagnosticReport,
            validation_diagnostics: BoundedDiagnosticReport,
            losses: BoundedVec<MigrationLossEvidence, MAX_MIGRATION_STEP_LOSS_EVIDENCE>,
        }

        let raw = RawStepReport::deserialize(deserializer)?;
        Self::new(
            raw.step_id,
            raw.source_profile,
            raw.target_profile,
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
    source_profile: ProfileId,
    target_profile: ProfileId,
    route: Vec<MigrationStepId>,
    steps: Vec<MigrationStepReport>,
}

impl MigrationReport {
    /// Validates bounds, route order, and profile continuity.
    pub fn new(
        source_profile: ProfileId,
        target_profile: ProfileId,
        route: Vec<MigrationStepId>,
        steps: Vec<MigrationStepReport>,
    ) -> Result<Self, MigrationReportError> {
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

        Ok(Self {
            source_profile,
            target_profile,
            route,
            steps,
        })
    }

    /// Returns the exact overall source profile.
    pub const fn source_profile(&self) -> &ProfileId {
        &self.source_profile
    }

    /// Returns the exact overall target profile.
    pub const fn target_profile(&self) -> &ProfileId {
        &self.target_profile
    }

    /// Returns all planned step identifiers in order.
    pub fn route(&self) -> &[MigrationStepId] {
        &self.route
    }

    /// Returns the attempted route prefix with per-step evidence.
    pub fn steps(&self) -> &[MigrationStepReport] {
        &self.steps
    }

    /// Returns whether every planned route step has a report.
    pub fn is_complete(&self) -> bool {
        self.steps.len() == self.route.len()
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
            source_profile: ProfileId,
            target_profile: ProfileId,
            route: BoundedVec<MigrationStepId, MAX_MIGRATION_REPORT_STEPS>,
            steps: BoundedStepReports,
        }

        let raw = RawReport::deserialize(deserializer)?;
        Self::new(
            raw.source_profile,
            raw.target_profile,
            raw.route.0,
            raw.steps.0,
        )
        .map_err(de::Error::custom)
    }
}

/// Stable typed failure to construct or deserialize a migration report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MigrationReportError {
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
            Self::TooManyRouteSteps { .. } => "migration.report-too-many-route-steps",
            Self::TooManyStepReports { .. } => "migration.report-too-many-step-reports",
            Self::TooManyStepDiagnostics { .. } => "migration.report-too-many-step-diagnostics",
            Self::TooManyReportDiagnostics { .. } => "migration.report-too-many-diagnostics",
            Self::TooManyStepLosses { .. } => "migration.report-too-many-step-losses",
            Self::TooManyReportLosses { .. } => "migration.report-too-many-losses",
            Self::ReportCountOverflow { .. } => "migration.report-count-overflow",
            Self::DuplicateRouteStep { .. } => "migration.report-duplicate-route-step",
            Self::EmptyRouteProfileMismatch { .. } => {
                "migration.report-empty-route-profile-mismatch"
            }
            Self::StepOrderMismatch { .. } => "migration.report-step-order-mismatch",
            Self::StepContinuityMismatch { .. } => "migration.report-step-continuity-mismatch",
            Self::CompleteRouteTargetMismatch { .. } => "migration.report-complete-target-mismatch",
            Self::LossCodeMismatch { .. } => "migration.report-loss-code-mismatch",
            Self::LossDiagnosticNotWarning { .. } => "migration.report-loss-diagnostic-not-warning",
            Self::LossPolicyMismatch { .. } => "migration.report-loss-policy-mismatch",
            Self::LossProfileMismatch { .. } => "migration.report-loss-profile-mismatch",
            Self::LossProfilesMissing { .. } => "migration.report-loss-profiles-missing",
        }
    }
}

impl Display for MigrationReportError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
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
    use crate::diagnostic::DiagnosticReport;

    use super::*;

    fn profile(value: &str) -> ProfileId {
        ProfileId::parse(value).unwrap()
    }

    fn step_id(value: &str) -> MigrationStepId {
        MigrationStepId::parse(value).unwrap()
    }

    #[test]
    fn report_json_is_stable_golden_and_roundtrips() {
        let step = MigrationStepReport::new(
            step_id("migration:a-b"),
            profile("profile:a"),
            profile("profile:b"),
            DiagnosticReport::new(),
            DiagnosticReport::new(),
            DiagnosticReport::new(),
            Vec::new(),
        )
        .unwrap();
        let report = MigrationReport::new(
            profile("profile:a"),
            profile("profile:b"),
            vec![step_id("migration:a-b")],
            vec![step],
        )
        .unwrap();

        let json = serde_json::to_string(&report).unwrap();
        assert_eq!(
            json,
            r#"{"source_profile":"profile:a","target_profile":"profile:b","route":["migration:a-b"],"steps":[{"step_id":"migration:a-b","source_profile":"profile:a","target_profile":"profile:b","analysis_diagnostics":{"diagnostics":[]},"apply_diagnostics":{"diagnostics":[]},"validation_diagnostics":{"diagnostics":[]},"losses":[]}]}"#
        );
        assert_eq!(
            serde_json::from_str::<MigrationReport>(&json).unwrap(),
            report
        );
    }

    #[test]
    fn report_rejects_discontinuous_or_duplicate_routes() {
        let duplicate = MigrationReport::new(
            profile("profile:a"),
            profile("profile:b"),
            vec![step_id("migration:a-b"), step_id("migration:a-b")],
            Vec::new(),
        );
        assert!(matches!(
            duplicate,
            Err(MigrationReportError::DuplicateRouteStep { .. })
        ));

        let wrong_step = MigrationStepReport::new(
            step_id("migration:b-c"),
            profile("profile:x"),
            profile("profile:c"),
            DiagnosticReport::new(),
            DiagnosticReport::new(),
            DiagnosticReport::new(),
            Vec::new(),
        )
        .unwrap();
        let discontinuous = MigrationReport::new(
            profile("profile:a"),
            profile("profile:c"),
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
        let report = MigrationReport::new(
            profile("profile:a"),
            profile("profile:b"),
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
