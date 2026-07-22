//! Transactional execution of validated deterministic migration plans.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use crate::artifact::ProfileId;
use crate::diagnostic::{
    Diagnostic, DiagnosticBuildError, DiagnosticCode, DiagnosticReport, LossDisposition,
    LossPolicy, ObjectPath, PropertyPath, Severity,
};
use crate::model::CanonicalConfiguration;
use crate::validate::validate_configuration;

use super::graph::{MigrationGraph, MigrationPlan};
use super::report::{
    MAX_MIGRATION_REPORT_DIAGNOSTICS, MAX_MIGRATION_STEP_DIAGNOSTICS, MigrationLossEvidence,
    MigrationLossPhase, MigrationReport, MigrationReportError, MigrationStepReport,
};
use super::step::{
    MigrationAnalyzeRequest, MigrationApplyRequest, MigrationStepDescriptor, MigrationStepId,
};

/// Stable diagnostic emitted when a step returns no value and no error.
pub const APPLY_MISSING_VALUE_CODE: &str = "migration.apply-missing-value";

/// Immutable input for one transactional migration execution.
#[derive(Clone, Copy, Debug)]
pub struct MigrationExecutionRequest<'a> {
    plan: &'a MigrationPlan,
    source: &'a CanonicalConfiguration,
    loss_policy: LossPolicy,
}

impl<'a> MigrationExecutionRequest<'a> {
    /// Creates an exact plan execution request over an immutable source model.
    pub const fn new(
        plan: &'a MigrationPlan,
        source: &'a CanonicalConfiguration,
        loss_policy: LossPolicy,
    ) -> Self {
        Self {
            plan,
            source,
            loss_policy,
        }
    }

    /// Returns the immutable deterministic plan.
    pub const fn plan(&self) -> &'a MigrationPlan {
        self.plan
    }

    /// Returns the source model that will never be mutated.
    pub const fn source(&self) -> &'a CanonicalConfiguration {
        self.source
    }

    /// Returns the caller-selected fail-closed loss policy.
    pub const fn loss_policy(&self) -> LossPolicy {
        self.loss_policy
    }
}

/// Successful independently owned model plus its complete migration report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationExecution {
    configuration: CanonicalConfiguration,
    report: MigrationReport,
}

impl MigrationExecution {
    /// Returns the validated independently owned result.
    pub const fn configuration(&self) -> &CanonicalConfiguration {
        &self.configuration
    }

    /// Returns the complete deterministic report.
    pub const fn report(&self) -> &MigrationReport {
        &self.report
    }

    /// Separates the validated result from its complete report.
    pub fn into_parts(self) -> (CanonicalConfiguration, MigrationReport) {
        (self.configuration, self.report)
    }
}

/// Stateless transactional executor bound to one immutable graph snapshot.
#[derive(Clone, Copy, Debug)]
pub struct MigrationExecutor<'a> {
    graph: &'a MigrationGraph,
}

impl<'a> MigrationExecutor<'a> {
    /// Binds execution to one validated graph snapshot.
    pub const fn new(graph: &'a MigrationGraph) -> Self {
        Self { graph }
    }

    /// Executes analyze/apply/validate step-by-step on a private owned clone.
    pub fn execute(
        &self,
        request: MigrationExecutionRequest<'_>,
    ) -> Result<MigrationExecution, MigrationExecutionError> {
        self.validate_plan(request.plan())?;
        if let Err(diagnostics) = validate_configuration(request.source()) {
            ensure_diagnostic_bound(&diagnostics, None, MigrationPhase::InitialValidation)?;
            return Err(MigrationExecutionError::InvalidSource { diagnostics });
        }

        let mut working = request.source().clone();
        let mut step_reports = Vec::with_capacity(request.plan().len());

        for (index, step_id) in request.plan().step_ids().iter().enumerate() {
            let source_id = &request.plan().route_profiles()[index];
            let target_id = &request.plan().route_profiles()[index + 1];
            let source_profile = self.graph.profile(source_id).ok_or_else(|| {
                MigrationExecutionError::UnknownRouteProfile {
                    profile: source_id.clone(),
                }
            })?;
            let target_profile = self.graph.profile(target_id).ok_or_else(|| {
                MigrationExecutionError::UnknownRouteProfile {
                    profile: target_id.clone(),
                }
            })?;
            let registered = self.graph.registered_step(step_id).ok_or_else(|| {
                MigrationExecutionError::UnknownRouteStep {
                    step_id: step_id.clone(),
                }
            })?;
            ensure_contract_unchanged(
                step_id,
                registered.descriptor(),
                registered.implementation(),
            )?;

            let analysis = registered
                .implementation()
                .analyze(MigrationAnalyzeRequest::new(
                    source_profile,
                    target_profile,
                    &working,
                ))
                .map_err(|error| MigrationExecutionError::AnalysisBuildFailed {
                    step_id: step_id.clone(),
                    error,
                })?;
            ensure_diagnostic_bound(
                analysis.diagnostics(),
                Some(step_id),
                MigrationPhase::Analyze,
            )?;
            let analysis_diagnostics = normalize_report(
                analysis.diagnostics(),
                &source_profile.id,
                &target_profile.id,
            );
            if analysis_diagnostics.has_errors() {
                let step_report = MigrationStepReport::new(
                    step_id.clone(),
                    source_profile.id.clone(),
                    target_profile.id.clone(),
                    analysis_diagnostics,
                    DiagnosticReport::new(),
                    DiagnosticReport::new(),
                    Vec::new(),
                )?;
                step_reports.push(step_report);
                let report = compose_report(request.plan(), step_reports)?;
                return Err(MigrationExecutionError::AnalysisFailed {
                    step_id: step_id.clone(),
                    report,
                });
            }

            ensure_contract_unchanged(
                step_id,
                registered.descriptor(),
                registered.implementation(),
            )?;
            let outcome = registered
                .implementation()
                .apply(MigrationApplyRequest::new(
                    source_profile,
                    target_profile,
                    &working,
                    request.loss_policy(),
                ));
            ensure_contract_unchanged(
                step_id,
                registered.descriptor(),
                registered.implementation(),
            )?;
            let (output, raw_apply_diagnostics) = outcome.into_parts();
            ensure_diagnostic_bound(&raw_apply_diagnostics, Some(step_id), MigrationPhase::Apply)?;
            let mut apply_diagnostics = normalize_report(
                &raw_apply_diagnostics,
                &source_profile.id,
                &target_profile.id,
            );
            if apply_diagnostics.has_errors() {
                let step_report = MigrationStepReport::new(
                    step_id.clone(),
                    source_profile.id.clone(),
                    target_profile.id.clone(),
                    analysis_diagnostics,
                    apply_diagnostics,
                    DiagnosticReport::new(),
                    Vec::new(),
                )?;
                step_reports.push(step_report);
                let report = compose_report(request.plan(), step_reports)?;
                return Err(MigrationExecutionError::ApplyFailed {
                    step_id: step_id.clone(),
                    report,
                });
            }

            let Some(output) = output else {
                let diagnostic =
                    missing_value_diagnostic(step_id, &source_profile.id, &target_profile.id)?;
                apply_diagnostics.push(diagnostic);
                let step_report = MigrationStepReport::new(
                    step_id.clone(),
                    source_profile.id.clone(),
                    target_profile.id.clone(),
                    analysis_diagnostics,
                    apply_diagnostics,
                    DiagnosticReport::new(),
                    Vec::new(),
                )?;
                step_reports.push(step_report);
                let report = compose_report(request.plan(), step_reports)?;
                return Err(MigrationExecutionError::ApplyMissingValue {
                    step_id: step_id.clone(),
                    report,
                });
            };

            let (candidate, losses) = output.into_parts();
            let loss_evidence = validate_actual_losses(
                step_id,
                registered.descriptor(),
                &losses,
                &raw_apply_diagnostics,
                request.loss_policy(),
                &source_profile.id,
                &target_profile.id,
            )?;

            let validation_diagnostics = match validate_configuration(&candidate) {
                Ok(_) => DiagnosticReport::new(),
                Err(diagnostics) => {
                    ensure_diagnostic_bound(
                        &diagnostics,
                        Some(step_id),
                        MigrationPhase::ResultValidation,
                    )?;
                    normalize_report(&diagnostics, &source_profile.id, &target_profile.id)
                }
            };
            let validation_failed = validation_diagnostics.has_errors();
            let step_report = MigrationStepReport::new(
                step_id.clone(),
                source_profile.id.clone(),
                target_profile.id.clone(),
                analysis_diagnostics,
                apply_diagnostics,
                validation_diagnostics,
                loss_evidence,
            )?;
            step_reports.push(step_report);
            if validation_failed {
                let report = compose_report(request.plan(), step_reports)?;
                return Err(MigrationExecutionError::ResultValidationFailed {
                    step_id: step_id.clone(),
                    report,
                });
            }

            working = candidate;
        }

        let report = compose_report(request.plan(), step_reports)?;
        Ok(MigrationExecution {
            configuration: working,
            report,
        })
    }

    fn validate_plan(&self, plan: &MigrationPlan) -> Result<(), MigrationExecutionError> {
        let expected_profiles = plan.step_ids().len().checked_add(1).ok_or(
            MigrationExecutionError::InvalidRouteLength {
                profiles: plan.route_profiles().len(),
                steps: plan.step_ids().len(),
            },
        )?;
        if plan.route_profiles().len() != expected_profiles {
            return Err(MigrationExecutionError::InvalidRouteLength {
                profiles: plan.route_profiles().len(),
                steps: plan.step_ids().len(),
            });
        }
        if plan.route_profiles().first() != Some(plan.source_profile())
            || plan.route_profiles().last() != Some(plan.target_profile())
        {
            return Err(MigrationExecutionError::RouteEndpointMismatch {
                source_profile: plan.source_profile().clone(),
                target_profile: plan.target_profile().clone(),
            });
        }

        let mut visited_profiles = BTreeSet::new();
        for profile in plan.route_profiles() {
            if !visited_profiles.insert(profile) {
                return Err(MigrationExecutionError::RouteCycle {
                    profile: profile.clone(),
                });
            }
            if self.graph.profile(profile).is_none() {
                return Err(MigrationExecutionError::UnknownRouteProfile {
                    profile: profile.clone(),
                });
            }
        }

        let mut visited_steps = BTreeSet::new();
        for (index, step_id) in plan.step_ids().iter().enumerate() {
            if !visited_steps.insert(step_id) {
                return Err(MigrationExecutionError::RepeatedRouteStep {
                    step_id: step_id.clone(),
                });
            }
            let registered = self.graph.registered_step(step_id).ok_or_else(|| {
                MigrationExecutionError::UnknownRouteStep {
                    step_id: step_id.clone(),
                }
            })?;
            if registered.is_unverified_downgrade() {
                return Err(MigrationExecutionError::UnverifiedDowngrade {
                    step_id: step_id.clone(),
                });
            }
            let descriptor = registered.descriptor();
            let source_id = &plan.route_profiles()[index];
            let target_id = &plan.route_profiles()[index + 1];
            if descriptor.source().exact_profile() != source_id
                || descriptor.target().exact_profile() != target_id
            {
                return Err(MigrationExecutionError::RouteStepMismatch {
                    step_id: step_id.clone(),
                    expected_source: source_id.clone(),
                    expected_target: target_id.clone(),
                });
            }
            let source = self.graph.profile(source_id).ok_or_else(|| {
                MigrationExecutionError::UnknownRouteProfile {
                    profile: source_id.clone(),
                }
            })?;
            let target = self.graph.profile(target_id).ok_or_else(|| {
                MigrationExecutionError::UnknownRouteProfile {
                    profile: target_id.clone(),
                }
            })?;
            if !descriptor.source().matches(source) || !descriptor.target().matches(target) {
                return Err(MigrationExecutionError::RouteConstraintMismatch {
                    step_id: step_id.clone(),
                });
            }
            ensure_contract_unchanged(step_id, descriptor, registered.implementation())?;
        }
        Ok(())
    }
}

/// Stable typed failure that never contains a successful migrated model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MigrationExecutionError {
    /// The initial canonical graph is invalid.
    InvalidSource { diagnostics: DiagnosticReport },
    /// Route profile and step counts are inconsistent.
    InvalidRouteLength { profiles: usize, steps: usize },
    /// Route endpoints differ from the plan endpoints.
    RouteEndpointMismatch {
        source_profile: ProfileId,
        target_profile: ProfileId,
    },
    /// A prepared route repeats a profile.
    RouteCycle { profile: ProfileId },
    /// A prepared route repeats a step.
    RepeatedRouteStep { step_id: MigrationStepId },
    /// A route names a profile absent from the graph snapshot.
    UnknownRouteProfile { profile: ProfileId },
    /// A route names a step absent from the graph snapshot.
    UnknownRouteStep { step_id: MigrationStepId },
    /// A route step does not connect the adjacent exact profiles.
    RouteStepMismatch {
        step_id: MigrationStepId,
        expected_source: ProfileId,
        expected_target: ProfileId,
    },
    /// A route endpoint no longer satisfies the snapshotted constraints.
    RouteConstraintMismatch { step_id: MigrationStepId },
    /// A blocked unverified downgrade appeared in a prepared route.
    UnverifiedDowngrade { step_id: MigrationStepId },
    /// A mutable implementation changed its descriptor after registration.
    StepContractChanged { step_id: MigrationStepId },
    /// Analysis could not construct bounded diagnostics.
    AnalysisBuildFailed {
        step_id: MigrationStepId,
        error: DiagnosticBuildError,
    },
    /// Analysis emitted a blocking diagnostic; apply was not called.
    AnalysisFailed {
        step_id: MigrationStepId,
        report: MigrationReport,
    },
    /// Apply emitted a blocking diagnostic and returned no accepted value.
    ApplyFailed {
        step_id: MigrationStepId,
        report: MigrationReport,
    },
    /// Apply violated its guarded value contract without an explanatory error.
    ApplyMissingValue {
        step_id: MigrationStepId,
        report: MigrationReport,
    },
    /// A candidate result failed canonical graph validation.
    ResultValidationFailed {
        step_id: MigrationStepId,
        report: MigrationReport,
    },
    /// A step returned an evaluated loss absent from its descriptor.
    UndeclaredAppliedLoss {
        step_id: MigrationStepId,
        code: DiagnosticCode,
    },
    /// Actual loss evidence was absent from apply diagnostics.
    LossDiagnosticMissing {
        step_id: MigrationStepId,
        code: DiagnosticCode,
    },
    /// A declared loss warning had no corresponding actual disposition.
    LossDispositionMissing {
        step_id: MigrationStepId,
        code: DiagnosticCode,
    },
    /// A step or initial validation report exceeded execution bounds.
    DiagnosticLimitExceeded {
        step_id: Option<MigrationStepId>,
        phase: MigrationPhase,
        maximum: usize,
        actual: usize,
    },
    /// Internal construction of a bounded diagnostic failed safely.
    DiagnosticBuildFailed {
        step_id: MigrationStepId,
        error: DiagnosticBuildError,
    },
    /// A built-in stable diagnostic code was invalid.
    InvalidBuiltInDiagnosticCode { code: &'static str },
    /// Composed report invariants or bounds failed.
    Report(MigrationReportError),
}

impl MigrationExecutionError {
    /// Returns a stable machine-readable error code.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidSource { .. } => "migration.execute-invalid-source",
            Self::InvalidRouteLength { .. } => "migration.execute-invalid-route-length",
            Self::RouteEndpointMismatch { .. } => "migration.execute-route-endpoint-mismatch",
            Self::RouteCycle { .. } => "migration.execute-route-cycle",
            Self::RepeatedRouteStep { .. } => "migration.execute-repeated-route-step",
            Self::UnknownRouteProfile { .. } => "migration.execute-unknown-route-profile",
            Self::UnknownRouteStep { .. } => "migration.execute-unknown-route-step",
            Self::RouteStepMismatch { .. } => "migration.execute-route-step-mismatch",
            Self::RouteConstraintMismatch { .. } => "migration.execute-route-constraint-mismatch",
            Self::UnverifiedDowngrade { .. } => "migration.execute-unverified-downgrade",
            Self::StepContractChanged { .. } => "migration.execute-step-contract-changed",
            Self::AnalysisBuildFailed { .. } => "migration.execute-analysis-build-failed",
            Self::AnalysisFailed { .. } => "migration.execute-analysis-failed",
            Self::ApplyFailed { .. } => "migration.execute-apply-failed",
            Self::ApplyMissingValue { .. } => "migration.execute-apply-missing-value",
            Self::ResultValidationFailed { .. } => "migration.execute-result-validation-failed",
            Self::UndeclaredAppliedLoss { .. } => "migration.execute-undeclared-applied-loss",
            Self::LossDiagnosticMissing { .. } => "migration.execute-loss-diagnostic-missing",
            Self::LossDispositionMissing { .. } => "migration.execute-loss-disposition-missing",
            Self::DiagnosticLimitExceeded { .. } => "migration.execute-diagnostic-limit",
            Self::DiagnosticBuildFailed { .. } => "migration.execute-diagnostic-build-failed",
            Self::InvalidBuiltInDiagnosticCode { .. } => {
                "migration.execute-invalid-built-in-diagnostic-code"
            }
            Self::Report(error) => error.code(),
        }
    }

    /// Returns a partial deterministic report when execution reached a step failure.
    pub const fn report(&self) -> Option<&MigrationReport> {
        match self {
            Self::AnalysisFailed { report, .. }
            | Self::ApplyFailed { report, .. }
            | Self::ApplyMissingValue { report, .. }
            | Self::ResultValidationFailed { report, .. } => Some(report),
            _ => None,
        }
    }
}

impl Display for MigrationExecutionError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSource { .. } => formatter.write_str("source configuration is invalid"),
            Self::InvalidRouteLength { profiles, steps } => write!(
                formatter,
                "migration route has {profiles} profiles for {steps} steps"
            ),
            Self::RouteEndpointMismatch {
                source_profile,
                target_profile,
            } => write!(
                formatter,
                "migration route does not connect plan endpoints `{source_profile}` and `{target_profile}`"
            ),
            Self::RouteCycle { profile } => {
                write!(formatter, "migration route repeats profile `{profile}`")
            }
            Self::RepeatedRouteStep { step_id } => {
                write!(formatter, "migration route repeats step `{step_id}`")
            }
            Self::UnknownRouteProfile { profile } => {
                write!(
                    formatter,
                    "migration route contains unknown profile `{profile}`"
                )
            }
            Self::UnknownRouteStep { step_id } => {
                write!(
                    formatter,
                    "migration route contains unknown step `{step_id}`"
                )
            }
            Self::RouteStepMismatch {
                step_id,
                expected_source,
                expected_target,
            } => write!(
                formatter,
                "migration step `{step_id}` does not connect `{expected_source}` to `{expected_target}`"
            ),
            Self::RouteConstraintMismatch { step_id } => write!(
                formatter,
                "migration step `{step_id}` route constraints are not satisfied"
            ),
            Self::UnverifiedDowngrade { step_id } => write!(
                formatter,
                "migration route contains unverified downgrade step `{step_id}`"
            ),
            Self::StepContractChanged { step_id } => write!(
                formatter,
                "migration step `{step_id}` changed its descriptor after registration"
            ),
            Self::AnalysisBuildFailed { step_id, error } => write!(
                formatter,
                "migration step `{step_id}` analysis could not build diagnostics: {error}"
            ),
            Self::AnalysisFailed { step_id, .. } => {
                write!(formatter, "migration step `{step_id}` analysis failed")
            }
            Self::ApplyFailed { step_id, .. } => {
                write!(formatter, "migration step `{step_id}` apply failed")
            }
            Self::ApplyMissingValue { step_id, .. } => write!(
                formatter,
                "migration step `{step_id}` apply returned no value and no error"
            ),
            Self::ResultValidationFailed { step_id, .. } => write!(
                formatter,
                "migration step `{step_id}` produced an invalid canonical model"
            ),
            Self::UndeclaredAppliedLoss { step_id, code } => write!(
                formatter,
                "migration step `{step_id}` applied undeclared loss `{code}`"
            ),
            Self::LossDiagnosticMissing { step_id, code } => write!(
                formatter,
                "migration step `{step_id}` loss `{code}` is absent from apply diagnostics"
            ),
            Self::LossDispositionMissing { step_id, code } => write!(
                formatter,
                "migration step `{step_id}` diagnostic `{code}` has no actual loss disposition"
            ),
            Self::DiagnosticLimitExceeded {
                step_id,
                phase,
                maximum,
                actual,
            } => write!(
                formatter,
                "migration {:?} diagnostics for {:?} exceed {maximum} items (actual {actual})",
                phase,
                step_id.as_ref().map(MigrationStepId::as_str)
            ),
            Self::DiagnosticBuildFailed { step_id, error } => write!(
                formatter,
                "migration step `{step_id}` could not build an executor diagnostic: {error}"
            ),
            Self::InvalidBuiltInDiagnosticCode { code } => {
                write!(
                    formatter,
                    "built-in migration diagnostic code `{code}` is invalid"
                )
            }
            Self::Report(error) => Display::fmt(error, formatter),
        }
    }
}

impl Error for MigrationExecutionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::AnalysisBuildFailed { error, .. } | Self::DiagnosticBuildFailed { error, .. } => {
                Some(error)
            }
            Self::Report(error) => Some(error),
            _ => None,
        }
    }
}

impl From<MigrationReportError> for MigrationExecutionError {
    fn from(value: MigrationReportError) -> Self {
        Self::Report(value)
    }
}

/// Execution phase named by a bounded diagnostic-limit failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MigrationPhase {
    /// Validation before the first route step.
    InitialValidation,
    /// Immutable compatibility analysis.
    Analyze,
    /// Step application.
    Apply,
    /// Candidate validation before committing the local working copy.
    ResultValidation,
}

fn ensure_contract_unchanged(
    step_id: &MigrationStepId,
    snapshot: &MigrationStepDescriptor,
    implementation: &std::sync::Arc<dyn super::step::MigrationStep>,
) -> Result<(), MigrationExecutionError> {
    if implementation.descriptor() != snapshot {
        return Err(MigrationExecutionError::StepContractChanged {
            step_id: step_id.clone(),
        });
    }
    Ok(())
}

fn normalize_report(
    report: &DiagnosticReport,
    source_profile: &ProfileId,
    target_profile: &ProfileId,
) -> DiagnosticReport {
    DiagnosticReport::from_diagnostics(
        report
            .diagnostics()
            .iter()
            .cloned()
            .map(|diagnostic| {
                diagnostic.with_profiles(Some(source_profile.clone()), Some(target_profile.clone()))
            })
            .collect(),
    )
}

fn ensure_diagnostic_bound(
    report: &DiagnosticReport,
    step_id: Option<&MigrationStepId>,
    phase: MigrationPhase,
) -> Result<(), MigrationExecutionError> {
    let maximum = if matches!(phase, MigrationPhase::InitialValidation) {
        MAX_MIGRATION_REPORT_DIAGNOSTICS
    } else {
        MAX_MIGRATION_STEP_DIAGNOSTICS
    };
    if report.diagnostics().len() > maximum {
        return Err(MigrationExecutionError::DiagnosticLimitExceeded {
            step_id: step_id.cloned(),
            phase,
            maximum,
            actual: report.diagnostics().len(),
        });
    }
    Ok(())
}

fn missing_value_diagnostic(
    step_id: &MigrationStepId,
    source_profile: &ProfileId,
    target_profile: &ProfileId,
) -> Result<Diagnostic, MigrationExecutionError> {
    let code = DiagnosticCode::parse(APPLY_MISSING_VALUE_CODE).map_err(|_| {
        MigrationExecutionError::InvalidBuiltInDiagnosticCode {
            code: APPLY_MISSING_VALUE_CODE,
        }
    })?;
    Diagnostic::new(
        code,
        Severity::Error,
        ObjectPath::root(),
        PropertyPath::root(),
        "migration step returned no value and no blocking diagnostic",
    )
    .map(|diagnostic| {
        diagnostic.with_profiles(Some(source_profile.clone()), Some(target_profile.clone()))
    })
    .and_then(|diagnostic| diagnostic.with_context("migration_step", step_id.as_str()))
    .map_err(|error| MigrationExecutionError::DiagnosticBuildFailed {
        step_id: step_id.clone(),
        error,
    })
}

#[allow(clippy::too_many_arguments)]
fn validate_actual_losses(
    step_id: &MigrationStepId,
    descriptor: &MigrationStepDescriptor,
    losses: &[LossDisposition],
    apply_diagnostics: &DiagnosticReport,
    requested_policy: LossPolicy,
    source_profile: &ProfileId,
    target_profile: &ProfileId,
) -> Result<Vec<MigrationLossEvidence>, MigrationExecutionError> {
    let mut matched_diagnostics = vec![false; apply_diagnostics.diagnostics().len()];
    let mut evidence = Vec::with_capacity(losses.len());

    for loss in losses {
        let declaration = descriptor
            .possible_losses()
            .binary_search_by(|declaration| declaration.code().cmp(loss.diagnostic().code()))
            .ok()
            .and_then(|index| descriptor.possible_losses().get(index))
            .ok_or_else(|| MigrationExecutionError::UndeclaredAppliedLoss {
                step_id: step_id.clone(),
                code: loss.diagnostic().code().clone(),
            })?;
        let diagnostic_index = apply_diagnostics
            .diagnostics()
            .iter()
            .enumerate()
            .find(|(index, diagnostic)| {
                !matched_diagnostics[*index] && *diagnostic == loss.diagnostic()
            })
            .map(|(index, _)| index)
            .ok_or_else(|| MigrationExecutionError::LossDiagnosticMissing {
                step_id: step_id.clone(),
                code: loss.diagnostic().code().clone(),
            })?;
        matched_diagnostics[diagnostic_index] = true;
        evidence.push(MigrationLossEvidence::from_disposition(
            MigrationLossPhase::Apply,
            requested_policy,
            loss,
            declaration,
            source_profile,
            target_profile,
        )?);
    }

    for (index, diagnostic) in apply_diagnostics.diagnostics().iter().enumerate() {
        let declared = descriptor
            .possible_losses()
            .binary_search_by(|declaration| declaration.code().cmp(diagnostic.code()))
            .is_ok();
        if declared && diagnostic.severity() == Severity::Warning && !matched_diagnostics[index] {
            return Err(MigrationExecutionError::LossDispositionMissing {
                step_id: step_id.clone(),
                code: diagnostic.code().clone(),
            });
        }
    }

    Ok(evidence)
}

fn compose_report(
    plan: &MigrationPlan,
    steps: Vec<MigrationStepReport>,
) -> Result<MigrationReport, MigrationReportError> {
    MigrationReport::new(
        plan.source_profile().clone(),
        plan.target_profile().clone(),
        plan.step_ids().to_vec(),
        steps,
    )
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use crate::adapter::AdapterOutcome;
    use crate::diagnostic::{
        CodecLossDeclaration, CodecLossPermission, DiagnosticBuildError, DiagnosticCode,
        evaluate_loss,
    };
    use crate::identity::{LogicalIdentity, ObjectUuid};
    use crate::model::{
        CanonicalConfiguration, CanonicalObject, CanonicalObjectParts, MetadataKind,
    };
    use crate::profile::{
        ProfileRegistry, ProfileSourceKind, parse_profile_source, resolve_profiles,
    };
    use crate::provenance::{CanonicalAnchor, SourceProvenance};

    use super::super::graph::{MigrationDirection, MigrationEdge, MigrationVerification};
    use super::super::step::{
        MigrationAnalysis, MigrationApplyOutcome, MigrationStep, MigrationStepOutput,
        ProfileConstraint,
    };
    use super::*;

    #[derive(Clone, Copy)]
    enum Behavior {
        Success,
        FatalAnalyze,
        FatalApply,
        MissingValue,
        InvalidResult,
        EvaluatedLoss,
    }

    struct TestStep {
        descriptor: MigrationStepDescriptor,
        behavior: Behavior,
        analyze_calls: Arc<AtomicUsize>,
        apply_calls: Arc<AtomicUsize>,
    }

    impl MigrationStep for TestStep {
        fn descriptor(&self) -> &MigrationStepDescriptor {
            &self.descriptor
        }

        fn analyze(
            &self,
            request: MigrationAnalyzeRequest<'_>,
        ) -> Result<MigrationAnalysis, DiagnosticBuildError> {
            self.analyze_calls.fetch_add(1, Ordering::SeqCst);
            let diagnostics = if matches!(self.behavior, Behavior::FatalAnalyze) {
                diagnostic_report(
                    "migration.test-analysis-failed",
                    Severity::Error,
                    &request.source_profile().id,
                    &request.target_profile().id,
                )
            } else {
                DiagnosticReport::new()
            };
            Ok(MigrationAnalysis::new(diagnostics))
        }

        fn apply(&self, request: MigrationApplyRequest<'_>) -> MigrationApplyOutcome {
            self.apply_calls.fetch_add(1, Ordering::SeqCst);
            match self.behavior {
                Behavior::FatalApply => AdapterOutcome::without_value(diagnostic_report(
                    "migration.test-apply-failed",
                    Severity::Error,
                    &request.source_profile().id,
                    &request.target_profile().id,
                )),
                Behavior::MissingValue => AdapterOutcome::without_value(DiagnosticReport::new()),
                Behavior::InvalidResult => {
                    let object = request.configuration().objects()[0].clone();
                    let invalid =
                        CanonicalConfiguration::new(vec![object.clone(), object]).unwrap();
                    AdapterOutcome::success(MigrationStepOutput::new(invalid, Vec::new()).unwrap())
                }
                Behavior::EvaluatedLoss => {
                    let declaration = &self.descriptor.possible_losses()[0];
                    let loss = evaluate_loss(
                        request.loss_policy(),
                        diagnostic(
                            declaration.code().as_str(),
                            Severity::Error,
                            &request.source_profile().id,
                            &request.target_profile().id,
                        ),
                        Some(declaration),
                    )
                    .unwrap();
                    let diagnostics =
                        DiagnosticReport::from_diagnostics(vec![loss.diagnostic().clone()]);
                    AdapterOutcome::new(
                        MigrationStepOutput::new(request.configuration().clone(), vec![loss])
                            .unwrap(),
                        diagnostics,
                    )
                }
                Behavior::Success | Behavior::FatalAnalyze => AdapterOutcome::success(
                    MigrationStepOutput::new(request.configuration().clone(), Vec::new()).unwrap(),
                ),
            }
        }
    }

    fn registry(ids: &[&str]) -> ProfileRegistry {
        let documents = ids.iter().map(|id| {
            parse_profile_source(
                &format!("{id}.json"),
                ProfileSourceKind::Bundled,
                &format!(r#"{{"schema_version":1,"id":"{id}","status":"experimental"}}"#),
            )
            .unwrap()
        });
        resolve_profiles(documents).unwrap()
    }

    fn configuration() -> CanonicalConfiguration {
        let object_path = ObjectPath::root();
        let identity = LogicalIdentity::new(
            ObjectUuid::parse("00000000-0000-0000-0000-000000000001").unwrap(),
            object_path.clone(),
        );
        let provenance = SourceProvenance::new(
            ProfileId::parse("profile:a").unwrap(),
            CanonicalAnchor::new(object_path, PropertyPath::root()),
        );
        let object = CanonicalObject::new(CanonicalObjectParts::new(
            identity,
            MetadataKind::new("Catalog").unwrap(),
            provenance,
        ))
        .unwrap();
        CanonicalConfiguration::new(vec![object]).unwrap()
    }

    fn diagnostic(
        code: &str,
        severity: Severity,
        source: &ProfileId,
        target: &ProfileId,
    ) -> Diagnostic {
        Diagnostic::new(
            DiagnosticCode::parse(code).unwrap(),
            severity,
            ObjectPath::root(),
            PropertyPath::root(),
            "migration executor test diagnostic",
        )
        .unwrap()
        .with_profiles(Some(source.clone()), Some(target.clone()))
    }

    fn diagnostic_report(
        code: &str,
        severity: Severity,
        source: &ProfileId,
        target: &ProfileId,
    ) -> DiagnosticReport {
        DiagnosticReport::from_diagnostics(vec![diagnostic(code, severity, source, target)])
    }

    fn test_step(
        id: &str,
        source: &str,
        target: &str,
        behavior: Behavior,
    ) -> (Arc<dyn MigrationStep>, Arc<AtomicUsize>, Arc<AtomicUsize>) {
        let analyze_calls = Arc::new(AtomicUsize::new(0));
        let apply_calls = Arc::new(AtomicUsize::new(0));
        let possible_losses = if matches!(behavior, Behavior::EvaluatedLoss) {
            vec![
                CodecLossDeclaration::new(
                    DiagnosticCode::parse("migration.test-loss").unwrap(),
                    CodecLossPermission::DropAllowed,
                    "test loss",
                )
                .unwrap(),
            ]
        } else {
            Vec::new()
        };
        let implementation: Arc<dyn MigrationStep> = Arc::new(TestStep {
            descriptor: MigrationStepDescriptor::new(
                MigrationStepId::parse(id).unwrap(),
                ProfileConstraint::exact(ProfileId::parse(source).unwrap()),
                ProfileConstraint::exact(ProfileId::parse(target).unwrap()),
                Vec::new(),
                possible_losses,
            )
            .unwrap(),
            behavior,
            analyze_calls: Arc::clone(&analyze_calls),
            apply_calls: Arc::clone(&apply_calls),
        });
        (implementation, analyze_calls, apply_calls)
    }

    fn edge(step: Arc<dyn MigrationStep>) -> MigrationEdge {
        MigrationEdge::new(
            step,
            MigrationDirection::Lateral,
            MigrationVerification::Verified,
        )
    }

    fn execute_route(
        profiles: &ProfileRegistry,
        edges: Vec<MigrationEdge>,
        source: &CanonicalConfiguration,
        policy: LossPolicy,
    ) -> Result<MigrationExecution, MigrationExecutionError> {
        let graph = MigrationGraph::new(profiles, edges).unwrap();
        let source_profile = profiles
            .get(&ProfileId::parse("profile:a").unwrap())
            .unwrap();
        let target_profile = profiles
            .get(&ProfileId::parse("profile:c").unwrap())
            .unwrap();
        let plan = graph.plan(source_profile, target_profile).unwrap();
        MigrationExecutor::new(&graph)
            .execute(MigrationExecutionRequest::new(&plan, source, policy))
    }

    #[test]
    fn fatal_analysis_never_calls_apply_and_source_is_unchanged() {
        let profiles = registry(&["profile:a", "profile:c"]);
        let (step, analyze_calls, apply_calls) = test_step(
            "migration:a-c",
            "profile:a",
            "profile:c",
            Behavior::FatalAnalyze,
        );
        let source = configuration();
        let snapshot = source.clone();

        let error =
            execute_route(&profiles, vec![edge(step)], &source, LossPolicy::Error).unwrap_err();

        assert!(matches!(
            error,
            MigrationExecutionError::AnalysisFailed { .. }
        ));
        assert_eq!(analyze_calls.load(Ordering::SeqCst), 1);
        assert_eq!(apply_calls.load(Ordering::SeqCst), 0);
        assert_eq!(source, snapshot);
    }

    #[test]
    fn invalid_source_is_rejected_before_analysis_or_apply() {
        let profiles = registry(&["profile:a", "profile:c"]);
        let (step, analyze_calls, apply_calls) =
            test_step("migration:a-c", "profile:a", "profile:c", Behavior::Success);
        let object = configuration().objects()[0].clone();
        let source = CanonicalConfiguration::new(vec![object.clone(), object]).unwrap();
        let snapshot = source.clone();

        let error =
            execute_route(&profiles, vec![edge(step)], &source, LossPolicy::Error).unwrap_err();

        assert!(matches!(
            error,
            MigrationExecutionError::InvalidSource { .. }
        ));
        assert_eq!(analyze_calls.load(Ordering::SeqCst), 0);
        assert_eq!(apply_calls.load(Ordering::SeqCst), 0);
        assert_eq!(source, snapshot);
    }

    #[test]
    fn apply_failure_returns_no_value_and_source_is_unchanged() {
        let profiles = registry(&["profile:a", "profile:c"]);
        let (step, _, apply_calls) = test_step(
            "migration:a-c",
            "profile:a",
            "profile:c",
            Behavior::FatalApply,
        );
        let source = configuration();
        let snapshot = source.clone();

        let error =
            execute_route(&profiles, vec![edge(step)], &source, LossPolicy::Error).unwrap_err();

        assert!(matches!(error, MigrationExecutionError::ApplyFailed { .. }));
        assert_eq!(apply_calls.load(Ordering::SeqCst), 1);
        assert_eq!(source, snapshot);
    }

    #[test]
    fn invalid_candidate_returns_no_value_and_source_is_unchanged() {
        let profiles = registry(&["profile:a", "profile:c"]);
        let (step, _, _) = test_step(
            "migration:a-c",
            "profile:a",
            "profile:c",
            Behavior::InvalidResult,
        );
        let source = configuration();
        let snapshot = source.clone();

        let error =
            execute_route(&profiles, vec![edge(step)], &source, LossPolicy::Error).unwrap_err();

        assert!(matches!(
            error,
            MigrationExecutionError::ResultValidationFailed { .. }
        ));
        assert_eq!(source, snapshot);
    }

    #[test]
    fn missing_apply_value_becomes_a_stable_fatal_diagnostic() {
        let profiles = registry(&["profile:a", "profile:c"]);
        let (step, _, _) = test_step(
            "migration:a-c",
            "profile:a",
            "profile:c",
            Behavior::MissingValue,
        );
        let source = configuration();

        let error =
            execute_route(&profiles, vec![edge(step)], &source, LossPolicy::Error).unwrap_err();
        let report = error.report().unwrap();
        assert!(matches!(
            error,
            MigrationExecutionError::ApplyMissingValue { .. }
        ));
        assert_eq!(
            report.steps()[0].apply_diagnostics().diagnostics()[0]
                .code()
                .as_str(),
            APPLY_MISSING_VALUE_CODE
        );
    }

    #[test]
    fn successful_multi_step_route_is_ordered_and_transactional() {
        let profiles = registry(&["profile:a", "profile:b", "profile:c"]);
        let (first, _, first_apply) =
            test_step("migration:a-b", "profile:a", "profile:b", Behavior::Success);
        let (second, _, second_apply) =
            test_step("migration:b-c", "profile:b", "profile:c", Behavior::Success);
        let source = configuration();
        let snapshot = source.clone();

        let execution = execute_route(
            &profiles,
            vec![edge(second), edge(first)],
            &source,
            LossPolicy::Error,
        )
        .unwrap();

        assert_eq!(execution.configuration(), &snapshot);
        assert_eq!(source, snapshot);
        assert_eq!(first_apply.load(Ordering::SeqCst), 1);
        assert_eq!(second_apply.load(Ordering::SeqCst), 1);
        assert!(execution.report().is_complete());
        assert_eq!(
            execution
                .report()
                .route()
                .iter()
                .map(MigrationStepId::as_str)
                .collect::<Vec<_>>(),
            vec!["migration:a-b", "migration:b-c"]
        );
    }

    #[test]
    fn report_records_requested_and_actual_loss_policy() {
        let profiles = registry(&["profile:a", "profile:c"]);
        let (step, _, _) = test_step(
            "migration:a-c-loss",
            "profile:a",
            "profile:c",
            Behavior::EvaluatedLoss,
        );
        let source = configuration();

        let execution = execute_route(
            &profiles,
            vec![edge(step)],
            &source,
            LossPolicy::DropExplicitly,
        )
        .unwrap();
        let evidence = &execution.report().steps()[0].losses()[0];
        assert_eq!(evidence.requested_policy(), LossPolicy::DropExplicitly);
        assert_eq!(
            evidence.actual_disposition(),
            super::super::report::MigrationLossDisposition::DroppedExplicitly
        );
        assert_eq!(evidence.diagnostic().severity(), Severity::Warning);
        assert_eq!(
            evidence.declaration().code().as_str(),
            "migration.test-loss"
        );
    }
}
