//! Transactional execution of validated deterministic migration plans.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use crate::artifact::ProfileId;
use crate::diagnostic::{
    CodecLossPermission, Diagnostic, DiagnosticBuildError, DiagnosticCode, DiagnosticReport,
    LossDisposition, LossDispositionKind, LossPolicy, ObjectPath, PropertyPath, Severity,
};
use crate::model::CanonicalConfiguration;
use crate::validate::validate_configuration;

use super::graph::{MigrationGraph, MigrationPlan};
use super::report::{
    MAX_MIGRATION_REPORT_DIAGNOSTICS, MAX_MIGRATION_REPORT_LOSS_EVIDENCE,
    MAX_MIGRATION_STEP_DIAGNOSTICS, MAX_MIGRATION_STEP_LOSS_EVIDENCE, MigrationFailureCode,
    MigrationFailureStage, MigrationLossEvidence, MigrationLossPhase, MigrationReport,
    MigrationReportError, MigrationStepFailure, MigrationStepReport, MigrationTerminalFailure,
};
use super::step::{
    MigrationAnalyzeRequest, MigrationApplyRequest, MigrationStepDescriptor, MigrationStepId,
};

/// Stable diagnostic emitted when a step returns no value and no error.
pub const APPLY_MISSING_VALUE_CODE: &str = super::report::APPLY_MISSING_VALUE_DIAGNOSTIC_CODE;

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
        if let Err(diagnostics) = validate_configuration(request.source()) {
            ensure_diagnostic_bound(&diagnostics, None, MigrationPhase::InitialValidation)?;
            return Err(MigrationExecutionError::InvalidSource { diagnostics });
        }
        self.validate_plan(request.plan())?;

        let mut working = request.source().clone();
        let mut step_reports = Vec::with_capacity(request.plan().len());
        let mut budget = ExecutionReportBudget::default();

        for (index, step_id) in request.plan().step_ids().iter().enumerate() {
            budget.begin_step();
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

            let analysis = match registered
                .implementation()
                .analyze(MigrationAnalyzeRequest::new(
                    source_profile,
                    target_profile,
                    &working,
                )) {
                Ok(analysis) => analysis,
                Err(error) => {
                    step_reports.push(MigrationStepReport::failed(
                        step_id.clone(),
                        source_profile.id.clone(),
                        target_profile.id.clone(),
                        MigrationStepFailure::new(
                            MigrationFailureStage::Analyze,
                            MigrationFailureCode::AnalysisBuildFailed,
                        )?,
                        DiagnosticReport::new(),
                        DiagnosticReport::new(),
                        DiagnosticReport::new(),
                        Vec::new(),
                    )?);
                    let report = compose_failed_report(
                        request.plan(),
                        request.loss_policy(),
                        step_reports,
                        index,
                        MigrationFailureStage::Analyze,
                        MigrationFailureCode::AnalysisBuildFailed,
                    )?;
                    return Err(MigrationExecutionError::AnalysisBuildFailed {
                        step_id: step_id.clone(),
                        error,
                        report: Box::new(report),
                    });
                }
            };
            budget.observe_diagnostics(
                analysis.diagnostics().diagnostics().len(),
                step_id,
                MigrationPhase::Analyze,
            )?;
            let analysis_diagnostics = normalize_report(
                analysis.diagnostics(),
                &source_profile.id,
                &target_profile.id,
            );
            if analysis_diagnostics.has_errors() {
                let step_report = MigrationStepReport::failed(
                    step_id.clone(),
                    source_profile.id.clone(),
                    target_profile.id.clone(),
                    MigrationStepFailure::new(
                        MigrationFailureStage::Analyze,
                        MigrationFailureCode::AnalysisBlocked,
                    )?,
                    analysis_diagnostics,
                    DiagnosticReport::new(),
                    DiagnosticReport::new(),
                    Vec::new(),
                )?;
                step_reports.push(step_report);
                let report = compose_failed_report(
                    request.plan(),
                    request.loss_policy(),
                    step_reports,
                    index,
                    MigrationFailureStage::Analyze,
                    MigrationFailureCode::AnalysisBlocked,
                )?;
                return Err(MigrationExecutionError::AnalysisFailed {
                    step_id: step_id.clone(),
                    report: Box::new(report),
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
            budget.observe_diagnostics(
                raw_apply_diagnostics.diagnostics().len(),
                step_id,
                MigrationPhase::Apply,
            )?;
            let mut apply_diagnostics = normalize_report(
                &raw_apply_diagnostics,
                &source_profile.id,
                &target_profile.id,
            );
            if apply_diagnostics.has_errors() {
                let step_report = MigrationStepReport::failed(
                    step_id.clone(),
                    source_profile.id.clone(),
                    target_profile.id.clone(),
                    MigrationStepFailure::new(
                        MigrationFailureStage::Apply,
                        MigrationFailureCode::ApplyBlocked,
                    )?,
                    analysis_diagnostics,
                    apply_diagnostics,
                    DiagnosticReport::new(),
                    Vec::new(),
                )?;
                step_reports.push(step_report);
                let report = compose_failed_report(
                    request.plan(),
                    request.loss_policy(),
                    step_reports,
                    index,
                    MigrationFailureStage::Apply,
                    MigrationFailureCode::ApplyBlocked,
                )?;
                return Err(MigrationExecutionError::ApplyFailed {
                    step_id: step_id.clone(),
                    report: Box::new(report),
                });
            }

            let Some(output) = output else {
                if budget.try_observe_one_diagnostic() {
                    let diagnostic =
                        missing_value_diagnostic(step_id, &source_profile.id, &target_profile.id)?;
                    apply_diagnostics.push(diagnostic);
                }
                let step_report = MigrationStepReport::failed(
                    step_id.clone(),
                    source_profile.id.clone(),
                    target_profile.id.clone(),
                    MigrationStepFailure::new(
                        MigrationFailureStage::Apply,
                        MigrationFailureCode::ApplyMissingValue,
                    )?,
                    analysis_diagnostics,
                    apply_diagnostics,
                    DiagnosticReport::new(),
                    Vec::new(),
                )?;
                step_reports.push(step_report);
                let report = compose_failed_report(
                    request.plan(),
                    request.loss_policy(),
                    step_reports,
                    index,
                    MigrationFailureStage::Apply,
                    MigrationFailureCode::ApplyMissingValue,
                )?;
                return Err(MigrationExecutionError::ApplyMissingValue {
                    step_id: step_id.clone(),
                    report: Box::new(report),
                });
            };

            let (candidate, losses) = output.into_parts();
            budget.observe_losses(losses.len(), step_id)?;
            let loss_evidence = match validate_actual_losses(
                registered.descriptor(),
                &losses,
                &raw_apply_diagnostics,
                request.loss_policy(),
                &source_profile.id,
                &target_profile.id,
            ) {
                Ok(evidence) => evidence,
                Err(violation) => {
                    let failure_code = violation.failure_code();
                    step_reports.push(MigrationStepReport::failed(
                        step_id.clone(),
                        source_profile.id.clone(),
                        target_profile.id.clone(),
                        MigrationStepFailure::new(
                            MigrationFailureStage::LossContract,
                            failure_code,
                        )?,
                        analysis_diagnostics,
                        apply_diagnostics,
                        DiagnosticReport::new(),
                        Vec::new(),
                    )?);
                    let report = compose_failed_report(
                        request.plan(),
                        request.loss_policy(),
                        step_reports,
                        index,
                        MigrationFailureStage::LossContract,
                        failure_code,
                    )?;
                    return Err(violation.into_execution_error(step_id.clone(), report));
                }
            };

            let validation_diagnostics = match validate_configuration(&candidate) {
                Ok(_) => DiagnosticReport::new(),
                Err(diagnostics) => {
                    budget.observe_diagnostics(
                        diagnostics.diagnostics().len(),
                        step_id,
                        MigrationPhase::ResultValidation,
                    )?;
                    normalize_report(&diagnostics, &source_profile.id, &target_profile.id)
                }
            };
            let validation_failed = validation_diagnostics.has_errors();
            let step_report = if validation_failed {
                MigrationStepReport::failed(
                    step_id.clone(),
                    source_profile.id.clone(),
                    target_profile.id.clone(),
                    MigrationStepFailure::new(
                        MigrationFailureStage::ResultValidation,
                        MigrationFailureCode::ResultValidationFailed,
                    )?,
                    analysis_diagnostics,
                    apply_diagnostics,
                    validation_diagnostics,
                    loss_evidence,
                )?
            } else {
                MigrationStepReport::successful(
                    step_id.clone(),
                    source_profile.id.clone(),
                    target_profile.id.clone(),
                    analysis_diagnostics,
                    apply_diagnostics,
                    validation_diagnostics,
                    loss_evidence,
                )?
            };
            step_reports.push(step_report);
            if validation_failed {
                let report = compose_failed_report(
                    request.plan(),
                    request.loss_policy(),
                    step_reports,
                    index,
                    MigrationFailureStage::ResultValidation,
                    MigrationFailureCode::ResultValidationFailed,
                )?;
                return Err(MigrationExecutionError::ResultValidationFailed {
                    step_id: step_id.clone(),
                    report: Box::new(report),
                });
            }

            working = candidate;
        }

        let report =
            compose_successful_report(request.plan(), request.loss_policy(), step_reports)?;
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
        report: Box<MigrationReport>,
    },
    /// Analysis emitted a blocking diagnostic; apply was not called.
    AnalysisFailed {
        step_id: MigrationStepId,
        report: Box<MigrationReport>,
    },
    /// Apply emitted a blocking diagnostic and returned no accepted value.
    ApplyFailed {
        step_id: MigrationStepId,
        report: Box<MigrationReport>,
    },
    /// Apply violated its guarded value contract without an explanatory error.
    ApplyMissingValue {
        step_id: MigrationStepId,
        report: Box<MigrationReport>,
    },
    /// A candidate result failed canonical graph validation.
    ResultValidationFailed {
        step_id: MigrationStepId,
        report: Box<MigrationReport>,
    },
    /// A step returned an evaluated loss absent from its descriptor.
    UndeclaredAppliedLoss {
        step_id: MigrationStepId,
        code: DiagnosticCode,
        report: Box<MigrationReport>,
    },
    /// Actual loss evidence was absent from apply diagnostics.
    LossDiagnosticMissing {
        step_id: MigrationStepId,
        code: DiagnosticCode,
        report: Box<MigrationReport>,
    },
    /// A declared loss warning had no corresponding actual disposition.
    LossDispositionMissing {
        step_id: MigrationStepId,
        code: DiagnosticCode,
        report: Box<MigrationReport>,
    },
    /// The descriptor did not authorize the actual loss disposition.
    LossPermissionDenied {
        step_id: MigrationStepId,
        code: DiagnosticCode,
        report: Box<MigrationReport>,
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
            Self::LossPermissionDenied { .. } => "migration.execute-loss-permission-denied",
            Self::DiagnosticLimitExceeded { .. } => "migration.execute-diagnostic-limit",
            Self::DiagnosticBuildFailed { .. } => "migration.execute-diagnostic-build-failed",
            Self::InvalidBuiltInDiagnosticCode { .. } => {
                "migration.execute-invalid-built-in-diagnostic-code"
            }
            Self::Report(error) => error.code(),
        }
    }

    /// Returns a partial deterministic report when execution reached a step failure.
    pub fn report(&self) -> Option<&MigrationReport> {
        match self {
            Self::AnalysisBuildFailed { report, .. }
            | Self::AnalysisFailed { report, .. }
            | Self::ApplyFailed { report, .. }
            | Self::ApplyMissingValue { report, .. }
            | Self::ResultValidationFailed { report, .. }
            | Self::UndeclaredAppliedLoss { report, .. }
            | Self::LossDiagnosticMissing { report, .. }
            | Self::LossDispositionMissing { report, .. }
            | Self::LossPermissionDenied { report, .. } => Some(report.as_ref()),
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
            Self::AnalysisBuildFailed { step_id, error, .. } => write!(
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
            Self::UndeclaredAppliedLoss { step_id, code, .. } => write!(
                formatter,
                "migration step `{step_id}` applied undeclared loss `{code}`"
            ),
            Self::LossDiagnosticMissing { step_id, code, .. } => write!(
                formatter,
                "migration step `{step_id}` loss `{code}` is absent from apply diagnostics"
            ),
            Self::LossDispositionMissing { step_id, code, .. } => write!(
                formatter,
                "migration step `{step_id}` diagnostic `{code}` has no actual loss disposition"
            ),
            Self::LossPermissionDenied { step_id, code, .. } => write!(
                formatter,
                "migration step `{step_id}` loss `{code}` disposition is not permitted by its descriptor"
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

#[derive(Default)]
struct ExecutionReportBudget {
    report_diagnostics: usize,
    report_losses: usize,
    step_diagnostics: usize,
    step_losses: usize,
}

impl ExecutionReportBudget {
    fn begin_step(&mut self) {
        self.step_diagnostics = 0;
        self.step_losses = 0;
    }

    fn observe_diagnostics(
        &mut self,
        incoming: usize,
        step_id: &MigrationStepId,
        phase: MigrationPhase,
    ) -> Result<(), MigrationExecutionError> {
        let step_actual =
            self.step_diagnostics
                .checked_add(incoming)
                .ok_or(MigrationExecutionError::Report(
                    MigrationReportError::ReportCountOverflow {
                        field: "diagnostics",
                    },
                ))?;
        if step_actual > MAX_MIGRATION_STEP_DIAGNOSTICS {
            return Err(MigrationExecutionError::DiagnosticLimitExceeded {
                step_id: Some(step_id.clone()),
                phase,
                maximum: MAX_MIGRATION_STEP_DIAGNOSTICS,
                actual: step_actual,
            });
        }
        let report_actual = self.report_diagnostics.checked_add(incoming).ok_or(
            MigrationExecutionError::Report(MigrationReportError::ReportCountOverflow {
                field: "diagnostics",
            }),
        )?;
        if report_actual > MAX_MIGRATION_REPORT_DIAGNOSTICS {
            return Err(MigrationExecutionError::DiagnosticLimitExceeded {
                step_id: Some(step_id.clone()),
                phase,
                maximum: MAX_MIGRATION_REPORT_DIAGNOSTICS,
                actual: report_actual,
            });
        }
        self.step_diagnostics = step_actual;
        self.report_diagnostics = report_actual;
        Ok(())
    }

    fn try_observe_one_diagnostic(&mut self) -> bool {
        if self.step_diagnostics >= MAX_MIGRATION_STEP_DIAGNOSTICS
            || self.report_diagnostics >= MAX_MIGRATION_REPORT_DIAGNOSTICS
        {
            return false;
        }
        self.step_diagnostics += 1;
        self.report_diagnostics += 1;
        true
    }

    fn observe_losses(
        &mut self,
        incoming: usize,
        step_id: &MigrationStepId,
    ) -> Result<(), MigrationExecutionError> {
        let step_actual =
            self.step_losses
                .checked_add(incoming)
                .ok_or(MigrationExecutionError::Report(
                    MigrationReportError::ReportCountOverflow { field: "losses" },
                ))?;
        if step_actual > MAX_MIGRATION_STEP_LOSS_EVIDENCE {
            return Err(MigrationExecutionError::Report(
                MigrationReportError::TooManyStepLosses {
                    step_id: step_id.clone(),
                    maximum: MAX_MIGRATION_STEP_LOSS_EVIDENCE,
                    actual: step_actual,
                },
            ));
        }
        let report_actual =
            self.report_losses
                .checked_add(incoming)
                .ok_or(MigrationExecutionError::Report(
                    MigrationReportError::ReportCountOverflow { field: "losses" },
                ))?;
        if report_actual > MAX_MIGRATION_REPORT_LOSS_EVIDENCE {
            return Err(MigrationExecutionError::Report(
                MigrationReportError::TooManyReportLosses {
                    maximum: MAX_MIGRATION_REPORT_LOSS_EVIDENCE,
                    actual: report_actual,
                },
            ));
        }
        self.step_losses = step_actual;
        self.report_losses = report_actual;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum LossContractViolation {
    Undeclared(DiagnosticCode),
    DiagnosticMissing(DiagnosticCode),
    DispositionMissing(DiagnosticCode),
    PermissionDenied(DiagnosticCode),
}

impl LossContractViolation {
    const fn failure_code(&self) -> MigrationFailureCode {
        match self {
            Self::Undeclared(_) => MigrationFailureCode::UndeclaredAppliedLoss,
            Self::DiagnosticMissing(_) => MigrationFailureCode::LossDiagnosticMissing,
            Self::DispositionMissing(_) => MigrationFailureCode::LossDispositionMissing,
            Self::PermissionDenied(_) => MigrationFailureCode::LossPermissionDenied,
        }
    }

    fn into_execution_error(
        self,
        step_id: MigrationStepId,
        report: MigrationReport,
    ) -> MigrationExecutionError {
        match self {
            Self::Undeclared(code) => MigrationExecutionError::UndeclaredAppliedLoss {
                step_id,
                code,
                report: Box::new(report),
            },
            Self::DiagnosticMissing(code) => MigrationExecutionError::LossDiagnosticMissing {
                step_id,
                code,
                report: Box::new(report),
            },
            Self::DispositionMissing(code) => MigrationExecutionError::LossDispositionMissing {
                step_id,
                code,
                report: Box::new(report),
            },
            Self::PermissionDenied(code) => MigrationExecutionError::LossPermissionDenied {
                step_id,
                code,
                report: Box::new(report),
            },
        }
    }
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
    descriptor: &MigrationStepDescriptor,
    losses: &[LossDisposition],
    apply_diagnostics: &DiagnosticReport,
    requested_policy: LossPolicy,
    source_profile: &ProfileId,
    target_profile: &ProfileId,
) -> Result<Vec<MigrationLossEvidence>, LossContractViolation> {
    let mut matched_diagnostics = vec![false; apply_diagnostics.diagnostics().len()];
    let mut evidence = Vec::with_capacity(losses.len());

    for loss in losses {
        let declaration = descriptor
            .possible_losses()
            .binary_search_by(|declaration| declaration.code().cmp(loss.diagnostic().code()))
            .ok()
            .and_then(|index| descriptor.possible_losses().get(index))
            .ok_or_else(|| LossContractViolation::Undeclared(loss.diagnostic().code().clone()))?;
        let disposition_is_permitted = match loss.kind() {
            LossDispositionKind::ContinueWithWarning => requested_policy == LossPolicy::Warn,
            LossDispositionKind::DroppedExplicitly => {
                requested_policy == LossPolicy::DropExplicitly
                    && declaration.permission() == CodecLossPermission::DropAllowed
            }
        };
        if !disposition_is_permitted {
            return Err(LossContractViolation::PermissionDenied(
                loss.diagnostic().code().clone(),
            ));
        }
        let diagnostic_index = apply_diagnostics
            .diagnostics()
            .iter()
            .enumerate()
            .find(|(index, diagnostic)| {
                !matched_diagnostics[*index] && *diagnostic == loss.diagnostic()
            })
            .map(|(index, _)| index)
            .ok_or_else(|| {
                LossContractViolation::DiagnosticMissing(loss.diagnostic().code().clone())
            })?;
        matched_diagnostics[diagnostic_index] = true;
        let loss_code = loss.diagnostic().code().clone();
        evidence.push(
            MigrationLossEvidence::from_disposition(
                MigrationLossPhase::Apply,
                requested_policy,
                loss,
                declaration,
                source_profile,
                target_profile,
            )
            .map_err(|_| LossContractViolation::PermissionDenied(loss_code))?,
        );
    }

    for (index, diagnostic) in apply_diagnostics.diagnostics().iter().enumerate() {
        let declared = descriptor
            .possible_losses()
            .binary_search_by(|declaration| declaration.code().cmp(diagnostic.code()))
            .is_ok();
        if declared && diagnostic.severity() == Severity::Warning && !matched_diagnostics[index] {
            return Err(LossContractViolation::DispositionMissing(
                diagnostic.code().clone(),
            ));
        }
    }

    Ok(evidence)
}

fn compose_successful_report(
    plan: &MigrationPlan,
    requested_policy: LossPolicy,
    steps: Vec<MigrationStepReport>,
) -> Result<MigrationReport, MigrationReportError> {
    MigrationReport::successful(
        plan.source_profile().clone(),
        plan.target_profile().clone(),
        requested_policy,
        plan.step_ids().to_vec(),
        steps,
    )
}

fn compose_failed_report(
    plan: &MigrationPlan,
    requested_policy: LossPolicy,
    steps: Vec<MigrationStepReport>,
    step_index: usize,
    stage: MigrationFailureStage,
    code: MigrationFailureCode,
) -> Result<MigrationReport, MigrationReportError> {
    MigrationReport::failed(
        plan.source_profile().clone(),
        plan.target_profile().clone(),
        requested_policy,
        plan.step_ids().to_vec(),
        steps,
        MigrationTerminalFailure::new(stage, code, step_index)?,
    )
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

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
        UndeclaredLoss,
        MissingLossDiagnostic,
        MissingLossDisposition,
        ForgedDropPermission,
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
                Behavior::UndeclaredLoss => {
                    let declaration = CodecLossDeclaration::new(
                        DiagnosticCode::parse("migration.test-loss").unwrap(),
                        CodecLossPermission::WarnOnly,
                        "external declaration",
                    )
                    .unwrap();
                    let loss = evaluate_loss(
                        request.loss_policy(),
                        diagnostic(
                            declaration.code().as_str(),
                            Severity::Error,
                            &request.source_profile().id,
                            &request.target_profile().id,
                        ),
                        Some(&declaration),
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
                Behavior::MissingLossDiagnostic => {
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
                    AdapterOutcome::new(
                        MigrationStepOutput::new(request.configuration().clone(), vec![loss])
                            .unwrap(),
                        DiagnosticReport::new(),
                    )
                }
                Behavior::MissingLossDisposition => AdapterOutcome::new(
                    MigrationStepOutput::new(request.configuration().clone(), Vec::new()).unwrap(),
                    diagnostic_report(
                        self.descriptor.possible_losses()[0].code().as_str(),
                        Severity::Warning,
                        &request.source_profile().id,
                        &request.target_profile().id,
                    ),
                ),
                Behavior::ForgedDropPermission => {
                    let descriptor_declaration = &self.descriptor.possible_losses()[0];
                    let external_declaration = CodecLossDeclaration::new(
                        descriptor_declaration.code().clone(),
                        CodecLossPermission::DropAllowed,
                        "untrusted external declaration",
                    )
                    .unwrap();
                    let loss = evaluate_loss(
                        request.loss_policy(),
                        diagnostic(
                            external_declaration.code().as_str(),
                            Severity::Error,
                            &request.source_profile().id,
                            &request.target_profile().id,
                        ),
                        Some(&external_declaration),
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

    struct MutableDescriptorStep {
        registered: MigrationStepDescriptor,
        changed: MigrationStepDescriptor,
        changed_enabled: Arc<AtomicBool>,
        descriptor_calls: Arc<AtomicUsize>,
        analyze_calls: Arc<AtomicUsize>,
        apply_calls: Arc<AtomicUsize>,
    }

    impl MigrationStep for MutableDescriptorStep {
        fn descriptor(&self) -> &MigrationStepDescriptor {
            self.descriptor_calls.fetch_add(1, Ordering::SeqCst);
            if self.changed_enabled.load(Ordering::SeqCst) {
                &self.changed
            } else {
                &self.registered
            }
        }

        fn analyze(
            &self,
            _request: MigrationAnalyzeRequest<'_>,
        ) -> Result<MigrationAnalysis, DiagnosticBuildError> {
            self.analyze_calls.fetch_add(1, Ordering::SeqCst);
            Ok(MigrationAnalysis::new(DiagnosticReport::new()))
        }

        fn apply(&self, request: MigrationApplyRequest<'_>) -> MigrationApplyOutcome {
            self.apply_calls.fetch_add(1, Ordering::SeqCst);
            AdapterOutcome::success(
                MigrationStepOutput::new(request.configuration().clone(), Vec::new()).unwrap(),
            )
        }
    }

    struct DiagnosticFloodStep {
        descriptor: MigrationStepDescriptor,
        analysis_count: usize,
        apply_calls: Arc<AtomicUsize>,
    }

    impl MigrationStep for DiagnosticFloodStep {
        fn descriptor(&self) -> &MigrationStepDescriptor {
            &self.descriptor
        }

        fn analyze(
            &self,
            request: MigrationAnalyzeRequest<'_>,
        ) -> Result<MigrationAnalysis, DiagnosticBuildError> {
            let item = diagnostic(
                "migration.test-budget",
                Severity::Info,
                &request.source_profile().id,
                &request.target_profile().id,
            );
            Ok(MigrationAnalysis::new(DiagnosticReport::from_diagnostics(
                vec![item; self.analysis_count],
            )))
        }

        fn apply(&self, request: MigrationApplyRequest<'_>) -> MigrationApplyOutcome {
            self.apply_calls.fetch_add(1, Ordering::SeqCst);
            AdapterOutcome::success(
                MigrationStepOutput::new(request.configuration().clone(), Vec::new()).unwrap(),
            )
        }
    }

    struct MissingValueDiagnosticsStep {
        descriptor: MigrationStepDescriptor,
        apply_diagnostic_count: usize,
        apply_calls: Arc<AtomicUsize>,
    }

    impl MigrationStep for MissingValueDiagnosticsStep {
        fn descriptor(&self) -> &MigrationStepDescriptor {
            &self.descriptor
        }

        fn analyze(
            &self,
            _request: MigrationAnalyzeRequest<'_>,
        ) -> Result<MigrationAnalysis, DiagnosticBuildError> {
            Ok(MigrationAnalysis::new(DiagnosticReport::new()))
        }

        fn apply(&self, request: MigrationApplyRequest<'_>) -> MigrationApplyOutcome {
            self.apply_calls.fetch_add(1, Ordering::SeqCst);
            let item = diagnostic(
                "migration.test-missing-max",
                Severity::Info,
                &request.source_profile().id,
                &request.target_profile().id,
            );
            AdapterOutcome::without_value(DiagnosticReport::from_diagnostics(vec![
                item;
                self.apply_diagnostic_count
            ]))
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
        let possible_losses = match behavior {
            Behavior::EvaluatedLoss | Behavior::MissingLossDiagnostic => vec![
                CodecLossDeclaration::new(
                    DiagnosticCode::parse("migration.test-loss").unwrap(),
                    CodecLossPermission::DropAllowed,
                    "test loss",
                )
                .unwrap(),
            ],
            Behavior::MissingLossDisposition | Behavior::ForgedDropPermission => vec![
                CodecLossDeclaration::new(
                    DiagnosticCode::parse("migration.test-loss").unwrap(),
                    CodecLossPermission::WarnOnly,
                    "test loss",
                )
                .unwrap(),
            ],
            Behavior::Success
            | Behavior::FatalAnalyze
            | Behavior::FatalApply
            | Behavior::MissingValue
            | Behavior::InvalidResult
            | Behavior::UndeclaredLoss => Vec::new(),
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
    fn exact_step_diagnostic_limit_still_returns_a_missing_value_report() {
        let profiles = registry(&["profile:a", "profile:c"]);
        let apply_calls = Arc::new(AtomicUsize::new(0));
        let implementation: Arc<dyn MigrationStep> = Arc::new(MissingValueDiagnosticsStep {
            descriptor: MigrationStepDescriptor::new(
                step_id_for_test("migration:a-c-missing-max"),
                ProfileConstraint::exact(ProfileId::parse("profile:a").unwrap()),
                ProfileConstraint::exact(ProfileId::parse("profile:c").unwrap()),
                Vec::new(),
                Vec::new(),
            )
            .unwrap(),
            apply_diagnostic_count: MAX_MIGRATION_STEP_DIAGNOSTICS,
            apply_calls: Arc::clone(&apply_calls),
        });
        let source = configuration();
        let snapshot = source.clone();

        let error = execute_route(
            &profiles,
            vec![edge(implementation)],
            &source,
            LossPolicy::Error,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            MigrationExecutionError::ApplyMissingValue { .. }
        ));
        let report = error.report().unwrap();
        let apply_diagnostics = report.steps()[0].apply_diagnostics().diagnostics();
        assert_eq!(source, snapshot);
        assert_eq!(apply_calls.load(Ordering::SeqCst), 1);
        assert!(!report.is_complete());
        assert_eq!(report.completed_step_count(), 0);
        assert_eq!(apply_diagnostics.len(), MAX_MIGRATION_STEP_DIAGNOSTICS);
        assert!(apply_diagnostics.iter().all(|diagnostic| {
            diagnostic.code().as_str() == "migration.test-missing-max"
                && diagnostic.severity() == Severity::Info
        }));
        assert_eq!(
            report.steps()[0].failure().unwrap().code(),
            MigrationFailureCode::ApplyMissingValue
        );
        assert_eq!(
            report.terminal_failure().unwrap().code(),
            MigrationFailureCode::ApplyMissingValue
        );
        let json = serde_json::to_string(report).unwrap();
        let decoded = serde_json::from_str::<MigrationReport>(&json).unwrap();
        assert_eq!(&decoded, report);
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

    #[test]
    fn descriptor_permission_cannot_be_bypassed_by_an_external_declaration() {
        let profiles = registry(&["profile:a", "profile:c"]);
        let (step, _, apply_calls) = test_step(
            "migration:a-c-forged-drop",
            "profile:a",
            "profile:c",
            Behavior::ForgedDropPermission,
        );
        let source = configuration();
        let snapshot = source.clone();

        let error = execute_route(
            &profiles,
            vec![edge(step)],
            &source,
            LossPolicy::DropExplicitly,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            MigrationExecutionError::LossPermissionDenied { .. }
        ));
        let report = error.report().unwrap();
        assert_eq!(source, snapshot);
        assert_eq!(apply_calls.load(Ordering::SeqCst), 1);
        assert_eq!(report.requested_policy(), LossPolicy::DropExplicitly);
        assert_eq!(
            report.outcome(),
            super::super::report::MigrationOperationOutcome::Failure
        );
        assert!(!report.is_complete());
        assert_eq!(report.completed_step_count(), 0);
        let terminal = report.terminal_failure().unwrap();
        assert_eq!(terminal.stage(), MigrationFailureStage::LossContract);
        assert_eq!(terminal.code(), MigrationFailureCode::LossPermissionDenied);
        assert_eq!(terminal.step_index(), 0);
        assert_eq!(terminal.completed_step_count(), 0);
        let failed = &report.steps()[0];
        assert_eq!(
            failed.status(),
            super::super::report::MigrationStepStatus::Failure
        );
        assert_eq!(
            failed.failure().unwrap().code(),
            MigrationFailureCode::LossPermissionDenied
        );
        assert_eq!(failed.apply_diagnostics().diagnostics().len(), 1);
        assert_eq!(
            failed.apply_diagnostics().diagnostics()[0].source_profile(),
            Some(&ProfileId::parse("profile:a").unwrap())
        );
        assert!(failed.losses().is_empty());
    }

    #[test]
    fn every_loss_contract_failure_retains_the_failing_apply_diagnostics() {
        let cases = [
            (
                Behavior::UndeclaredLoss,
                LossPolicy::Warn,
                "migration.execute-undeclared-applied-loss",
                MigrationFailureCode::UndeclaredAppliedLoss,
                1,
            ),
            (
                Behavior::MissingLossDiagnostic,
                LossPolicy::Warn,
                "migration.execute-loss-diagnostic-missing",
                MigrationFailureCode::LossDiagnosticMissing,
                0,
            ),
            (
                Behavior::MissingLossDisposition,
                LossPolicy::Warn,
                "migration.execute-loss-disposition-missing",
                MigrationFailureCode::LossDispositionMissing,
                1,
            ),
        ];

        for (behavior, policy, expected_error_code, expected_failure_code, diagnostic_count) in
            cases
        {
            let profiles = registry(&["profile:a", "profile:c"]);
            let (step, _, _) =
                test_step("migration:a-c-contract", "profile:a", "profile:c", behavior);
            let source = configuration();
            let snapshot = source.clone();

            let error = execute_route(&profiles, vec![edge(step)], &source, policy).unwrap_err();

            assert_eq!(error.code(), expected_error_code);
            let report = error.report().unwrap();
            assert_eq!(source, snapshot);
            assert_eq!(report.completed_step_count(), 0);
            assert!(!report.is_complete());
            assert_eq!(report.steps().len(), 1);
            assert_eq!(
                report.terminal_failure().unwrap().stage(),
                MigrationFailureStage::LossContract
            );
            assert_eq!(
                report.terminal_failure().unwrap().code(),
                expected_failure_code
            );
            let failed = &report.steps()[0];
            assert_eq!(failed.failure().unwrap().code(), expected_failure_code);
            assert_eq!(
                failed.apply_diagnostics().diagnostics().len(),
                diagnostic_count
            );
            for diagnostic in failed.apply_diagnostics().diagnostics() {
                assert_eq!(
                    diagnostic.source_profile(),
                    Some(&ProfileId::parse("profile:a").unwrap())
                );
                assert_eq!(
                    diagnostic.target_profile(),
                    Some(&ProfileId::parse("profile:c").unwrap())
                );
            }
        }
    }

    #[test]
    fn loss_contract_failure_preserves_all_successful_prefix_reports() {
        let profiles = registry(&["profile:a", "profile:b", "profile:c"]);
        let (first, _, _) = test_step("migration:a-b", "profile:a", "profile:b", Behavior::Success);
        let (second, _, _) = test_step(
            "migration:b-c-contract",
            "profile:b",
            "profile:c",
            Behavior::UndeclaredLoss,
        );
        let source = configuration();

        let error = execute_route(
            &profiles,
            vec![edge(second), edge(first)],
            &source,
            LossPolicy::Warn,
        )
        .unwrap_err();
        let report = error.report().unwrap();

        assert_eq!(report.completed_step_count(), 1);
        assert_eq!(report.steps().len(), 2);
        assert_eq!(
            report.steps()[0].status(),
            super::super::report::MigrationStepStatus::Success
        );
        assert_eq!(
            report.steps()[1].failure().unwrap().code(),
            MigrationFailureCode::UndeclaredAppliedLoss
        );
        assert_eq!(report.terminal_failure().unwrap().step_index(), 1);
    }

    #[test]
    fn invalid_source_wins_before_a_changed_plan_descriptor_is_inspected() {
        let profiles = registry(&["profile:a", "profile:c"]);
        let changed_enabled = Arc::new(AtomicBool::new(false));
        let descriptor_calls = Arc::new(AtomicUsize::new(0));
        let analyze_calls = Arc::new(AtomicUsize::new(0));
        let apply_calls = Arc::new(AtomicUsize::new(0));
        let implementation: Arc<dyn MigrationStep> = Arc::new(MutableDescriptorStep {
            registered: MigrationStepDescriptor::new(
                step_id_for_test("migration:a-c-mutable"),
                ProfileConstraint::exact(ProfileId::parse("profile:a").unwrap()),
                ProfileConstraint::exact(ProfileId::parse("profile:c").unwrap()),
                Vec::new(),
                Vec::new(),
            )
            .unwrap(),
            changed: MigrationStepDescriptor::new(
                step_id_for_test("migration:a-c-mutable"),
                ProfileConstraint::exact(ProfileId::parse("profile:c").unwrap()),
                ProfileConstraint::exact(ProfileId::parse("profile:a").unwrap()),
                Vec::new(),
                Vec::new(),
            )
            .unwrap(),
            changed_enabled: Arc::clone(&changed_enabled),
            descriptor_calls: Arc::clone(&descriptor_calls),
            analyze_calls: Arc::clone(&analyze_calls),
            apply_calls: Arc::clone(&apply_calls),
        });
        let graph = MigrationGraph::new(&profiles, vec![edge(implementation)]).unwrap();
        let plan = graph
            .plan(
                profiles
                    .get(&ProfileId::parse("profile:a").unwrap())
                    .unwrap(),
                profiles
                    .get(&ProfileId::parse("profile:c").unwrap())
                    .unwrap(),
            )
            .unwrap();
        let calls_before_execute = descriptor_calls.load(Ordering::SeqCst);
        changed_enabled.store(true, Ordering::SeqCst);
        let object = configuration().objects()[0].clone();
        let source = CanonicalConfiguration::new(vec![object.clone(), object]).unwrap();
        let snapshot = source.clone();

        let error = MigrationExecutor::new(&graph)
            .execute(MigrationExecutionRequest::new(
                &plan,
                &source,
                LossPolicy::Error,
            ))
            .unwrap_err();

        assert!(matches!(
            error,
            MigrationExecutionError::InvalidSource { .. }
        ));
        assert_eq!(
            descriptor_calls.load(Ordering::SeqCst),
            calls_before_execute
        );
        assert_eq!(analyze_calls.load(Ordering::SeqCst), 0);
        assert_eq!(apply_calls.load(Ordering::SeqCst), 0);
        assert_eq!(source, snapshot);
    }

    #[test]
    fn streaming_report_budget_accepts_exact_limits_and_rejects_plus_one() {
        let step_id = step_id_for_test("migration:budget-test");
        let mut budget = ExecutionReportBudget::default();
        budget.begin_step();
        assert!(
            budget
                .observe_diagnostics(
                    MAX_MIGRATION_STEP_DIAGNOSTICS,
                    &step_id,
                    MigrationPhase::Analyze,
                )
                .is_ok()
        );
        assert!(matches!(
            budget.observe_diagnostics(1, &step_id, MigrationPhase::Apply),
            Err(MigrationExecutionError::DiagnosticLimitExceeded {
                maximum: MAX_MIGRATION_STEP_DIAGNOSTICS,
                actual,
                ..
            }) if actual == MAX_MIGRATION_STEP_DIAGNOSTICS + 1
        ));

        let mut report_budget = ExecutionReportBudget {
            report_diagnostics: MAX_MIGRATION_REPORT_DIAGNOSTICS - 1,
            ..ExecutionReportBudget::default()
        };
        report_budget.begin_step();
        assert!(
            report_budget
                .observe_diagnostics(1, &step_id, MigrationPhase::Analyze)
                .is_ok()
        );
        report_budget.begin_step();
        assert!(matches!(
            report_budget.observe_diagnostics(1, &step_id, MigrationPhase::Analyze),
            Err(MigrationExecutionError::DiagnosticLimitExceeded {
                maximum: MAX_MIGRATION_REPORT_DIAGNOSTICS,
                actual,
                ..
            }) if actual == MAX_MIGRATION_REPORT_DIAGNOSTICS + 1
        ));

        let mut loss_budget = ExecutionReportBudget {
            report_losses: MAX_MIGRATION_REPORT_LOSS_EVIDENCE - 1,
            ..ExecutionReportBudget::default()
        };
        loss_budget.begin_step();
        assert!(loss_budget.observe_losses(1, &step_id).is_ok());
        loss_budget.begin_step();
        assert!(matches!(
            loss_budget.observe_losses(1, &step_id),
            Err(MigrationExecutionError::Report(
                MigrationReportError::TooManyReportLosses { maximum, actual }
            )) if maximum == MAX_MIGRATION_REPORT_LOSS_EVIDENCE
                && actual == MAX_MIGRATION_REPORT_LOSS_EVIDENCE + 1
        ));

        let mut step_loss_budget = ExecutionReportBudget::default();
        step_loss_budget.begin_step();
        assert!(
            step_loss_budget
                .observe_losses(MAX_MIGRATION_STEP_LOSS_EVIDENCE, &step_id)
                .is_ok()
        );
        assert!(matches!(
            step_loss_budget.observe_losses(1, &step_id),
            Err(MigrationExecutionError::Report(
                MigrationReportError::TooManyStepLosses {
                    maximum,
                    actual,
                    ..
                }
            )) if maximum == MAX_MIGRATION_STEP_LOSS_EVIDENCE
                && actual == MAX_MIGRATION_STEP_LOSS_EVIDENCE + 1
        ));
    }

    #[test]
    fn cumulative_diagnostic_overflow_is_rejected_before_the_next_apply() {
        let profile_names = [
            "profile:a",
            "profile:b",
            "profile:c",
            "profile:d",
            "profile:e",
            "profile:f",
        ];
        let profiles = registry(&profile_names);
        let mut edges = Vec::new();
        let mut apply_calls = Vec::new();
        for index in 0..5 {
            let calls = Arc::new(AtomicUsize::new(0));
            let implementation: Arc<dyn MigrationStep> = Arc::new(DiagnosticFloodStep {
                descriptor: MigrationStepDescriptor::new(
                    step_id_for_test(&format!("migration:budget-{index}")),
                    ProfileConstraint::exact(ProfileId::parse(profile_names[index]).unwrap()),
                    ProfileConstraint::exact(ProfileId::parse(profile_names[index + 1]).unwrap()),
                    Vec::new(),
                    Vec::new(),
                )
                .unwrap(),
                analysis_count: if index < 4 {
                    MAX_MIGRATION_STEP_DIAGNOSTICS
                } else {
                    1
                },
                apply_calls: Arc::clone(&calls),
            });
            apply_calls.push(calls);
            edges.push(edge(implementation));
        }
        let graph = MigrationGraph::new(&profiles, edges).unwrap();
        let plan = graph
            .plan(
                profiles
                    .get(&ProfileId::parse("profile:a").unwrap())
                    .unwrap(),
                profiles
                    .get(&ProfileId::parse("profile:f").unwrap())
                    .unwrap(),
            )
            .unwrap();
        let source = configuration();

        let error = MigrationExecutor::new(&graph)
            .execute(MigrationExecutionRequest::new(
                &plan,
                &source,
                LossPolicy::Error,
            ))
            .unwrap_err();

        assert!(matches!(
            error,
            MigrationExecutionError::DiagnosticLimitExceeded {
                phase: MigrationPhase::Analyze,
                maximum: MAX_MIGRATION_REPORT_DIAGNOSTICS,
                actual,
                ..
            } if actual == MAX_MIGRATION_REPORT_DIAGNOSTICS + 1
        ));
        for calls in &apply_calls[..4] {
            assert_eq!(calls.load(Ordering::SeqCst), 1);
        }
        assert_eq!(apply_calls[4].load(Ordering::SeqCst), 0);
    }

    #[test]
    fn exact_report_diagnostic_limit_still_returns_missing_value_and_stops_route() {
        let profile_names = [
            "profile:a",
            "profile:b",
            "profile:c",
            "profile:d",
            "profile:e",
            "profile:f",
            "profile:g",
        ];
        let profiles = registry(&profile_names);
        let mut edges = Vec::new();
        for index in 0..4 {
            let implementation: Arc<dyn MigrationStep> = Arc::new(DiagnosticFloodStep {
                descriptor: MigrationStepDescriptor::new(
                    step_id_for_test(&format!("migration:full-budget-{index}")),
                    ProfileConstraint::exact(ProfileId::parse(profile_names[index]).unwrap()),
                    ProfileConstraint::exact(ProfileId::parse(profile_names[index + 1]).unwrap()),
                    Vec::new(),
                    Vec::new(),
                )
                .unwrap(),
                analysis_count: MAX_MIGRATION_STEP_DIAGNOSTICS,
                apply_calls: Arc::new(AtomicUsize::new(0)),
            });
            edges.push(edge(implementation));
        }
        let (missing, _, missing_apply_calls) = test_step(
            "migration:e-f-missing",
            "profile:e",
            "profile:f",
            Behavior::MissingValue,
        );
        let (next, next_analyze_calls, next_apply_calls) = test_step(
            "migration:f-g-next",
            "profile:f",
            "profile:g",
            Behavior::Success,
        );
        edges.push(edge(next));
        edges.push(edge(missing));
        let graph = MigrationGraph::new(&profiles, edges).unwrap();
        let plan = graph
            .plan(
                profiles
                    .get(&ProfileId::parse("profile:a").unwrap())
                    .unwrap(),
                profiles
                    .get(&ProfileId::parse("profile:g").unwrap())
                    .unwrap(),
            )
            .unwrap();
        let source = configuration();
        let snapshot = source.clone();

        let error = MigrationExecutor::new(&graph)
            .execute(MigrationExecutionRequest::new(
                &plan,
                &source,
                LossPolicy::Error,
            ))
            .unwrap_err();

        assert!(matches!(
            error,
            MigrationExecutionError::ApplyMissingValue { .. }
        ));
        let report = error.report().unwrap();
        assert_eq!(source, snapshot);
        assert_eq!(missing_apply_calls.load(Ordering::SeqCst), 1);
        assert_eq!(next_analyze_calls.load(Ordering::SeqCst), 0);
        assert_eq!(next_apply_calls.load(Ordering::SeqCst), 0);
        assert!(!report.is_complete());
        assert_eq!(report.completed_step_count(), 4);
        assert_eq!(report.steps().len(), 5);
        assert_eq!(report.terminal_failure().unwrap().step_index(), 4);
        assert_eq!(
            report.terminal_failure().unwrap().code(),
            MigrationFailureCode::ApplyMissingValue
        );
        assert!(
            report.steps()[4]
                .apply_diagnostics()
                .diagnostics()
                .is_empty()
        );
        assert_eq!(
            report
                .steps()
                .iter()
                .map(MigrationStepReport::diagnostic_count)
                .sum::<usize>(),
            MAX_MIGRATION_REPORT_DIAGNOSTICS
        );
    }

    fn step_id_for_test(value: &str) -> MigrationStepId {
        MigrationStepId::parse(value).unwrap()
    }
}
