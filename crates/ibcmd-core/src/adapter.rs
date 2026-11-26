//! Format-independent adapter requests, guarded outcomes, and encode permits.

use crate::artifact::ProfileId;
use crate::capability::PreservationLevel;
use crate::diagnostic::DiagnosticReport;
use crate::profile::CapabilityId;

/// Borrowed input and exact requirements for one decode operation.
#[derive(Clone, Copy, Debug)]
pub struct DecodeRequest<'a, I: ?Sized> {
    source_profile: &'a ProfileId,
    input: &'a I,
    capability: &'a CapabilityId,
    preservation: PreservationLevel,
}

impl<'a, I: ?Sized> DecodeRequest<'a, I> {
    /// Creates a platform-independent decode request.
    pub const fn new(
        source_profile: &'a ProfileId,
        input: &'a I,
        capability: &'a CapabilityId,
        preservation: PreservationLevel,
    ) -> Self {
        Self {
            source_profile,
            input,
            capability,
            preservation,
        }
    }

    /// Returns the exact source profile without detection or approximation.
    pub const fn source_profile(&self) -> &'a ProfileId {
        self.source_profile
    }

    /// Returns the borrowed adapter input.
    pub const fn input(&self) -> &'a I {
        self.input
    }

    /// Returns the exact independently requested capability.
    pub const fn capability(&self) -> &'a CapabilityId {
        self.capability
    }

    /// Returns the exact requested preservation level.
    pub const fn preservation(&self) -> PreservationLevel {
        self.preservation
    }
}

/// Borrowed input, optional base, and exact route for one encode operation.
#[derive(Clone, Copy, Debug)]
pub struct EncodeRequest<'a, I: ?Sized, B: ?Sized> {
    source_profile: &'a ProfileId,
    target_profile: &'a ProfileId,
    input: &'a I,
    base: Option<&'a B>,
    capability: &'a CapabilityId,
    preservation: PreservationLevel,
}

impl<'a, I: ?Sized, B: ?Sized> EncodeRequest<'a, I, B> {
    /// Creates an encode request with explicit source and target profiles.
    pub const fn new(
        source_profile: &'a ProfileId,
        target_profile: &'a ProfileId,
        input: &'a I,
        base: Option<&'a B>,
        capability: &'a CapabilityId,
        preservation: PreservationLevel,
    ) -> Self {
        Self {
            source_profile,
            target_profile,
            input,
            base,
            capability,
            preservation,
        }
    }

    /// Returns the exact source profile.
    pub const fn source_profile(&self) -> &'a ProfileId {
        self.source_profile
    }

    /// Returns the exact mandatory target profile.
    pub const fn target_profile(&self) -> &'a ProfileId {
        self.target_profile
    }

    /// Returns the borrowed validated or adapter-specific input.
    pub const fn input(&self) -> &'a I {
        self.input
    }

    /// Returns the optional borrowed base artifact.
    pub const fn base(&self) -> Option<&'a B> {
        self.base
    }

    /// Returns whether a base artifact was explicitly supplied.
    pub const fn base_available(&self) -> bool {
        self.base.is_some()
    }

    /// Returns the exact independently requested capability.
    pub const fn capability(&self) -> &'a CapabilityId {
        self.capability
    }

    /// Returns the exact requested preservation level.
    pub const fn preservation(&self) -> PreservationLevel {
        self.preservation
    }
}

/// A value paired with canonical diagnostics under a fail-closed invariant.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterOutcome<T> {
    value: Option<T>,
    diagnostics: DiagnosticReport,
}

impl<T> AdapterOutcome<T> {
    /// Retains the value only when the complete report has no error diagnostic.
    pub fn new(value: T, diagnostics: DiagnosticReport) -> Self {
        let value = (!diagnostics.has_errors()).then_some(value);
        Self { value, diagnostics }
    }

    /// Creates an outcome without a value while retaining every diagnostic.
    pub const fn without_value(diagnostics: DiagnosticReport) -> Self {
        Self {
            value: None,
            diagnostics,
        }
    }

    /// Creates a successful outcome with an empty report.
    pub fn success(value: T) -> Self {
        Self::new(value, DiagnosticReport::new())
    }

    /// Returns the value only when no fatal diagnostic accompanied it.
    pub const fn value(&self) -> Option<&T> {
        self.value.as_ref()
    }

    /// Returns whether this outcome contains an accepted value.
    pub const fn has_value(&self) -> bool {
        self.value.is_some()
    }

    /// Returns the complete canonical diagnostic report.
    pub const fn diagnostics(&self) -> &DiagnosticReport {
        &self.diagnostics
    }

    /// Converts the guarded outcome into a conventional result.
    pub fn into_result(self) -> Result<T, DiagnosticReport> {
        self.value.ok_or(self.diagnostics)
    }

    /// Separates the guarded optional value and preserved report.
    pub fn into_parts(self) -> (Option<T>, DiagnosticReport) {
        (self.value, self.diagnostics)
    }
}

/// Guarded decode result.
pub type DecodeOutcome<T> = AdapterOutcome<T>;
/// Guarded encode result.
pub type EncodeOutcome<T> = AdapterOutcome<T>;

/// Result of a complete encode analysis performed before writer startup.
///
/// Its state is private. Fatal diagnostics always discard the supplied plan,
/// and only a retained plan can be converted into a [`WritePermit`].
#[derive(Debug, Eq, PartialEq)]
pub struct EncodePreflight<P> {
    plan: Option<P>,
    diagnostics: DiagnosticReport,
}

impl<P> EncodePreflight<P> {
    /// Applies the fatal-diagnostic guard to a complete optional plan.
    ///
    /// Supplying `None` represents an adapter that could not produce a plan.
    /// Any report containing an error discards `plan` unconditionally.
    pub fn checked(plan: Option<P>, diagnostics: DiagnosticReport) -> Self {
        let plan = if diagnostics.has_errors() { None } else { plan };
        Self { plan, diagnostics }
    }

    /// Returns whether this preflight can issue a one-shot writer permit.
    pub const fn is_ready(&self) -> bool {
        self.plan.is_some()
    }

    /// Returns all preflight diagnostics, including blocking errors.
    pub const fn diagnostics(&self) -> &DiagnosticReport {
        &self.diagnostics
    }

    /// Consumes a ready preflight into the only token accepted by a writer.
    pub fn into_permit(self) -> Result<WritePermit<P>, DiagnosticReport> {
        match self.plan {
            Some(plan) => Ok(WritePermit {
                plan,
                diagnostics: self.diagnostics,
            }),
            None => Err(self.diagnostics),
        }
    }
}

/// One-shot proof that complete encode preflight succeeded without errors.
///
/// There is deliberately no public constructor. Writers consume this value,
/// so one permit cannot start a writer twice.
#[derive(Debug, Eq, PartialEq)]
pub struct WritePermit<P> {
    plan: P,
    diagnostics: DiagnosticReport,
}

impl<P> WritePermit<P> {
    /// Returns non-fatal diagnostics retained from preflight.
    pub const fn diagnostics(&self) -> &DiagnosticReport {
        &self.diagnostics
    }

    /// Consumes the permit into its checked plan and non-fatal diagnostics.
    pub fn into_parts(self) -> (P, DiagnosticReport) {
        (self.plan, self.diagnostics)
    }
}

/// Adapter contract for decoding one borrowed platform-independent input.
pub trait DecodeAdapter<I: ?Sized> {
    /// Successful decoded representation.
    type Output;

    /// Decodes under the exact request and guarded diagnostic invariant.
    fn decode(&self, request: DecodeRequest<'_, I>) -> DecodeOutcome<Self::Output>;
}

/// Adapter contract that makes full preflight mandatory before writer startup.
pub trait EncodeAdapter<I: ?Sized, B: ?Sized> {
    /// Adapter-private complete write plan.
    type Plan;
    /// Successful encoded representation.
    type Output;

    /// Analyzes the whole request without starting the writer.
    fn preflight(&self, request: EncodeRequest<'_, I, B>) -> EncodePreflight<Self::Plan>;

    /// Starts writing only after consuming a checked one-shot permit.
    fn write(&self, permit: WritePermit<Self::Plan>) -> EncodeOutcome<Self::Output>;
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use crate::capability::{export_capability, overlay_capability};
    use crate::diagnostic::{Diagnostic, DiagnosticCode, ObjectPath, PropertyPath, Severity};

    use super::*;

    fn profile(value: &str) -> ProfileId {
        ProfileId::parse(value).unwrap()
    }

    fn report(severity: Severity) -> DiagnosticReport {
        DiagnosticReport::from_diagnostics(vec![
            Diagnostic::new(
                DiagnosticCode::parse("adapter.test").unwrap(),
                severity,
                ObjectPath::root(),
                PropertyPath::root(),
                "adapter test diagnostic",
            )
            .unwrap(),
        ])
    }

    struct FatalDecoder;

    impl DecodeAdapter<[u8]> for FatalDecoder {
        type Output = usize;

        fn decode(&self, request: DecodeRequest<'_, [u8]>) -> DecodeOutcome<Self::Output> {
            AdapterOutcome::new(request.input().len(), report(Severity::Error))
        }
    }

    #[test]
    fn fatal_decoder_outcome_cannot_retain_a_value() {
        fn assert_object_safe(_: &dyn DecodeAdapter<[u8], Output = usize>) {}

        let source = profile("source:exact");
        let capability = export_capability();
        let input = [1_u8, 2, 3];
        let decoder = FatalDecoder;
        assert_object_safe(&decoder);
        let outcome = decoder.decode(DecodeRequest::new(
            &source,
            input.as_slice(),
            &capability,
            PreservationLevel::Semantic,
        ));
        assert!(!outcome.has_value());
        assert!(outcome.diagnostics().has_errors());
        assert!(outcome.into_result().is_err());
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct MockPlan(usize);

    struct MockEncoder {
        severity: Severity,
        writer_starts: Cell<usize>,
    }

    impl EncodeAdapter<[u8], [u8]> for MockEncoder {
        type Plan = MockPlan;
        type Output = usize;

        fn preflight(&self, request: EncodeRequest<'_, [u8], [u8]>) -> EncodePreflight<Self::Plan> {
            EncodePreflight::checked(Some(MockPlan(request.input().len())), report(self.severity))
        }

        fn write(&self, permit: WritePermit<Self::Plan>) -> EncodeOutcome<Self::Output> {
            self.writer_starts.set(self.writer_starts.get() + 1);
            let (plan, diagnostics) = permit.into_parts();
            AdapterOutcome::new(plan.0, diagnostics)
        }
    }

    fn encode_request<'a>(
        source: &'a ProfileId,
        target: &'a ProfileId,
        input: &'a [u8],
        base: Option<&'a [u8]>,
        capability: &'a CapabilityId,
    ) -> EncodeRequest<'a, [u8], [u8]> {
        EncodeRequest::new(
            source,
            target,
            input,
            base,
            capability,
            PreservationLevel::Semantic,
        )
    }

    #[test]
    fn fatal_preflight_cannot_issue_permit_or_start_writer() {
        let encoder = MockEncoder {
            severity: Severity::Error,
            writer_starts: Cell::new(0),
        };
        let source = profile("source:exact");
        let target = profile("target:exact");
        let capability = overlay_capability();
        let input = [1_u8];
        let preflight =
            encoder.preflight(encode_request(&source, &target, &input, None, &capability));
        assert!(!preflight.is_ready());
        let blocked = preflight.into_permit().unwrap_err();
        assert!(blocked.has_errors());
        assert_eq!(encoder.writer_starts.get(), 0);
    }

    #[test]
    fn warning_preflight_issues_one_permit_and_writer_starts_once() {
        fn assert_object_safe(_: &dyn EncodeAdapter<[u8], [u8], Plan = MockPlan, Output = usize>) {}

        let encoder = MockEncoder {
            severity: Severity::Warning,
            writer_starts: Cell::new(0),
        };
        let source = profile("source:exact");
        let target = profile("target:exact");
        let capability = overlay_capability();
        let input = [1_u8, 2];
        assert_object_safe(&encoder);
        let preflight =
            encoder.preflight(encode_request(&source, &target, &input, None, &capability));
        assert!(preflight.is_ready());
        let outcome = encoder.write(preflight.into_permit().unwrap());
        assert_eq!(outcome.value(), Some(&2));
        assert!(!outcome.diagnostics().has_errors());
        assert_eq!(encoder.writer_starts.get(), 1);
    }

    #[test]
    fn encode_request_keeps_source_target_and_base_explicit() {
        let source = profile("source:exact");
        let target = profile("target:exact");
        let capability = overlay_capability();
        let input = [1_u8];
        let base = [2_u8];
        let request = encode_request(&source, &target, &input, Some(&base), &capability);
        assert_eq!(request.source_profile(), &source);
        assert_eq!(request.target_profile(), &target);
        assert_ne!(request.source_profile(), request.target_profile());
        assert!(request.base_available());
        assert_eq!(request.base(), Some(base.as_slice()));
    }
}
