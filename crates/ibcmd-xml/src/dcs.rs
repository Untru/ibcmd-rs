//! Evidence-gated serialization boundary for canonical DCS settings.
//!
//! The bundled EDT corpus currently proves the Form `ListSettings` delegation
//! boundary and the structural existence of the two typed settings features.
//! It does not yet prove the XML QName, TypeId, default-emission, or ordering
//! rules needed to emit either physical context. This module therefore exposes
//! one shared fail-closed preflight for standalone settings and Form
//! `ListSettings`; no caller can obtain an emission permit while those exact
//! decisions remain pending.

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use ibcmd_core::artifact::ProfileId;
use ibcmd_core::dcs::DcsSettingsEnvelope;
use ibcmd_core::diagnostic::{Diagnostic, DiagnosticCode, Severity};
use ibcmd_schema::{
    EvidenceStatus, FeatureSemanticKey, FeatureSemanticsCorpus, FormListSettingsNullValue,
    WriterPolicy, WriterRuleCorpus, WriterRuleKey, bundled_feature_semantics, bundled_writer_rules,
};

/// EDT release against which the DCS writer boundary was inspected.
pub const DCS_WRITER_EVIDENCE_RELEASE: &str = "2025.2.3+30";
/// Stable diagnostic emitted before XML generation when an exact rule is pending.
pub const DCS_WRITER_EVIDENCE_PENDING_CODE: &str = "dcs.writer-evidence-pending";
/// Stable diagnostic emitted when the embedded corpus cannot prove a claimed rule.
pub const DCS_WRITER_EVIDENCE_INVALID_CODE: &str = "dcs.writer-evidence-invalid";

const DCS_SETTINGS_NAMESPACE: &str = "http://g5.1c.ru/v8/dt/data-composition-system/settings";
const DCS_SETTINGS_CLASSIFIER: &str = "DataCompositionSettings";

/// One XML decision that must be evidence-backed before DCS bytes are emitted.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DcsWriterDecision {
    /// QName of a standalone settings document.
    StandaloneDocumentQName,
    /// QName of the Form feature wrapper which receives delegated settings.
    FormListSettingsWrapperQName,
    /// Exact serialized TypeId for the settings value.
    SettingsTypeId,
    /// QName of `itemsUserSettingID`.
    ItemsUserSettingIdQName,
    /// Position of `itemsUserSettingID`.
    ItemsUserSettingIdOrder,
    /// Default/absence emission policy of `itemsUserSettingID`.
    ItemsUserSettingIdDefaultEmission,
    /// QName of `itemsViewMode`.
    ItemsViewModeQName,
    /// Position of `itemsViewMode`.
    ItemsViewModeOrder,
    /// Default/absence emission policy of `itemsViewMode`.
    ItemsViewModeDefaultEmission,
    /// Placement of retained opaque XML relative to typed settings children.
    OpaqueExtensionPlacement,
    /// EDT delegation from Form `ListSettings` into the DCS serializer.
    FormListSettingsDelegate,
}

impl DcsWriterDecision {
    /// Returns the stable schema key used in diagnostics and tests.
    pub const fn schema_key(self) -> &'static str {
        match self {
            Self::StandaloneDocumentQName => "dcs.settings.document.qname",
            Self::FormListSettingsWrapperQName => "form.DynamicListExtInfo.listSettings.qname",
            Self::SettingsTypeId => "dcs.DataCompositionSettings.type-id",
            Self::ItemsUserSettingIdQName => "dcs.DataCompositionSettings.itemsUserSettingID.qname",
            Self::ItemsUserSettingIdOrder => "dcs.DataCompositionSettings.itemsUserSettingID.order",
            Self::ItemsUserSettingIdDefaultEmission => {
                "dcs.DataCompositionSettings.itemsUserSettingID.emit-default"
            }
            Self::ItemsViewModeQName => "dcs.DataCompositionSettings.itemsViewMode.qname",
            Self::ItemsViewModeOrder => "dcs.DataCompositionSettings.itemsViewMode.order",
            Self::ItemsViewModeDefaultEmission => {
                "dcs.DataCompositionSettings.itemsViewMode.emit-default"
            }
            Self::OpaqueExtensionPlacement => {
                "dcs.DataCompositionSettings.opaque-extension.placement"
            }
            Self::FormListSettingsDelegate => "form.DynamicListExtInfo.listSettings.delegate",
        }
    }
}

/// Confirmation state of one DCS writer decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DcsWriterEvidenceStatus {
    /// The exact runtime decision is present in the bundled verified corpus.
    Verified,
    /// Evidence is incomplete and must not become a runtime fallback.
    Pending,
}

/// Auditable status row for one DCS writer decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DcsWriterEvidence {
    /// Decision governed by this row.
    pub decision: DcsWriterDecision,
    /// Current evidence status.
    pub status: DcsWriterEvidenceStatus,
    /// Stable corpus coordinate or evidence note.
    pub source: &'static str,
}

/// Current evidence map. Pending entries are deliberately explicit so their
/// absence can never be interpreted as permission to infer a value.
pub const DCS_WRITER_EVIDENCE: &[DcsWriterEvidence] = &[
    DcsWriterEvidence {
        decision: DcsWriterDecision::StandaloneDocumentQName,
        status: DcsWriterEvidenceStatus::Pending,
        source: "standalone DCS document writer inspection pending",
    },
    DcsWriterEvidence {
        decision: DcsWriterDecision::FormListSettingsWrapperQName,
        status: DcsWriterEvidenceStatus::Pending,
        source: "Form ListSettings wrapper QName inspection pending",
    },
    DcsWriterEvidence {
        decision: DcsWriterDecision::SettingsTypeId,
        status: DcsWriterEvidenceStatus::Pending,
        source: "DCS settings TypeId inspection pending",
    },
    DcsWriterEvidence {
        decision: DcsWriterDecision::ItemsUserSettingIdQName,
        status: DcsWriterEvidenceStatus::Pending,
        source: "feature-semantics:DataCompositionSettings/itemsUserSettingID:qname",
    },
    DcsWriterEvidence {
        decision: DcsWriterDecision::ItemsUserSettingIdOrder,
        status: DcsWriterEvidenceStatus::Pending,
        source: "feature-semantics:DataCompositionSettings/itemsUserSettingID:order",
    },
    DcsWriterEvidence {
        decision: DcsWriterDecision::ItemsUserSettingIdDefaultEmission,
        status: DcsWriterEvidenceStatus::Pending,
        source: "feature-semantics:DataCompositionSettings/itemsUserSettingID:emitDefault",
    },
    DcsWriterEvidence {
        decision: DcsWriterDecision::ItemsViewModeQName,
        status: DcsWriterEvidenceStatus::Pending,
        source: "feature-semantics:DataCompositionSettings/itemsViewMode:qname",
    },
    DcsWriterEvidence {
        decision: DcsWriterDecision::ItemsViewModeOrder,
        status: DcsWriterEvidenceStatus::Pending,
        source: "feature-semantics:DataCompositionSettings/itemsViewMode:order",
    },
    DcsWriterEvidence {
        decision: DcsWriterDecision::ItemsViewModeDefaultEmission,
        status: DcsWriterEvidenceStatus::Pending,
        source: "feature-semantics:DataCompositionSettings/itemsViewMode:emitDefault",
    },
    DcsWriterEvidence {
        decision: DcsWriterDecision::OpaqueExtensionPlacement,
        status: DcsWriterEvidenceStatus::Pending,
        source: "DCS opaque-to-typed child placement inspection pending",
    },
    DcsWriterEvidence {
        decision: DcsWriterDecision::FormListSettingsDelegate,
        status: DcsWriterEvidenceStatus::Verified,
        source: "writer-rules:form.dynamic-list.list-settings",
    },
];

/// Fail-closed DCS writer failure with a stable machine-readable diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DcsSerializationError {
    diagnostic: Box<Diagnostic>,
    missing_decisions: Vec<DcsWriterDecision>,
}

impl DcsSerializationError {
    /// Returns the structured diagnostic.
    pub fn diagnostic(&self) -> &Diagnostic {
        self.diagnostic.as_ref()
    }

    /// Returns missing schema decisions in deterministic key order.
    pub fn missing_decisions(&self) -> &[DcsWriterDecision] {
        &self.missing_decisions
    }
}

impl Display for DcsSerializationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: {}",
            self.diagnostic.code(),
            self.diagnostic.message()
        )
    }
}

impl Error for DcsSerializationError {}

/// Proof that all decisions required by the shared DCS boundary were resolved.
///
/// There is intentionally no public constructor. With the current corpus this
/// permit cannot be obtained because QName/TypeId/order rules are pending.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DcsSerializationPermit {
    target_profile: ProfileId,
    form_list_settings: bool,
}

impl DcsSerializationPermit {
    /// Returns the exact target profile checked by preflight.
    pub fn target_profile(&self) -> &ProfileId {
        &self.target_profile
    }

    /// Returns whether the physical context is Form `ListSettings`.
    pub const fn is_form_list_settings(&self) -> bool {
        self.form_list_settings
    }
}

/// Runs the single schema/evidence boundary shared by standalone settings and
/// Form `ListSettings`.
///
/// The function verifies same-profile opaque emission before consulting writer
/// evidence. It never returns a permit while any QName, TypeId, ordering,
/// default-emission, or placement decision is pending.
pub fn preflight_dcs_settings_serialization(
    envelope: &DcsSettingsEnvelope,
    target_profile: &ProfileId,
) -> Result<DcsSerializationPermit, DcsSerializationError> {
    let settings = envelope.as_settings();

    for facet in settings.opaque_extensions().as_slice() {
        facet
            .emit_permit(target_profile)
            .map_err(|error| DcsSerializationError {
                diagnostic: Box::new(error.into_diagnostic()),
                missing_decisions: Vec::new(),
            })?;
    }

    let writer_rules = bundled_writer_rules()
        .map_err(|error| invalid_evidence(envelope, target_profile, &error))?;
    let feature_semantics = bundled_feature_semantics()
        .map_err(|error| invalid_evidence(envelope, target_profile, &error))?;

    if matches!(envelope, DcsSettingsEnvelope::ListSettings(_)) {
        verify_form_list_settings_delegate(envelope, target_profile, &writer_rules)?;
    }

    let mut missing = Vec::new();
    match envelope {
        DcsSettingsEnvelope::Settings(_) => {
            missing.push(DcsWriterDecision::StandaloneDocumentQName);
        }
        DcsSettingsEnvelope::ListSettings(_) => {
            missing.push(DcsWriterDecision::FormListSettingsWrapperQName);
        }
    }
    missing.push(DcsWriterDecision::SettingsTypeId);
    collect_feature_evidence(
        &feature_semantics,
        "itemsUserSettingID",
        DcsWriterDecision::ItemsUserSettingIdQName,
        DcsWriterDecision::ItemsUserSettingIdOrder,
        DcsWriterDecision::ItemsUserSettingIdDefaultEmission,
        &mut missing,
    )
    .map_err(|error| invalid_evidence(envelope, target_profile, &error))?;
    collect_feature_evidence(
        &feature_semantics,
        "itemsViewMode",
        DcsWriterDecision::ItemsViewModeQName,
        DcsWriterDecision::ItemsViewModeOrder,
        DcsWriterDecision::ItemsViewModeDefaultEmission,
        &mut missing,
    )
    .map_err(|error| invalid_evidence(envelope, target_profile, &error))?;
    if !settings.opaque_extensions().is_empty() {
        missing.push(DcsWriterDecision::OpaqueExtensionPlacement);
    }
    missing.sort_unstable();
    missing.dedup();

    if !missing.is_empty() {
        return Err(pending_evidence(envelope, target_profile, missing));
    }

    Ok(DcsSerializationPermit {
        target_profile: target_profile.clone(),
        form_list_settings: matches!(envelope, DcsSettingsEnvelope::ListSettings(_)),
    })
}

fn verify_form_list_settings_delegate(
    envelope: &DcsSettingsEnvelope,
    target_profile: &ProfileId,
    writer_rules: &WriterRuleCorpus,
) -> Result<(), DcsSerializationError> {
    let rule = writer_rules
        .exact_rule(WriterRuleKey {
            source_release: DCS_WRITER_EVIDENCE_RELEASE,
            model_type: "DynamicListExtInfo",
            feature: "listSettings",
        })
        .map_err(|error| invalid_evidence(envelope, target_profile, &error))?;
    match rule.policy.as_ref() {
        Some(WriterPolicy::FormListSettings {
            null_value: FormListSettingsNullValue::Omit,
            delegate,
        }) if delegate == "DcsV8Serializer.writeSettings"
            && rule.delegate.as_deref() == Some(delegate.as_str()) =>
        {
            Ok(())
        }
        _ => Err(invalid_evidence(
            envelope,
            target_profile,
            &"verified Form ListSettings rule has no exact DCS delegation policy",
        )),
    }
}

fn collect_feature_evidence(
    corpus: &FeatureSemanticsCorpus,
    feature_name: &str,
    qname_decision: DcsWriterDecision,
    order_decision: DcsWriterDecision,
    default_decision: DcsWriterDecision,
    missing: &mut Vec<DcsWriterDecision>,
) -> Result<(), String> {
    let key = FeatureSemanticKey {
        namespace_uri: DCS_SETTINGS_NAMESPACE.to_owned(),
        classifier: DCS_SETTINGS_CLASSIFIER.to_owned(),
        feature: feature_name.to_owned(),
    };
    let feature = corpus
        .feature(&key)
        .ok_or_else(|| format!("feature semantics are missing for {feature_name}"))?;

    if feature.xml.evidence.status != EvidenceStatus::Verified || feature.xml.qname.is_none() {
        missing.push(qname_decision);
    }
    if feature.xml.evidence.status != EvidenceStatus::Verified || feature.xml.order.is_none() {
        missing.push(order_decision);
    }
    if feature.xml.evidence.status != EvidenceStatus::Verified || feature.xml.emit_default.is_none()
    {
        missing.push(default_decision);
    }
    Ok(())
}

fn pending_evidence(
    envelope: &DcsSettingsEnvelope,
    target_profile: &ProfileId,
    missing_decisions: Vec<DcsWriterDecision>,
) -> DcsSerializationError {
    let settings = envelope.as_settings();
    let missing_keys = missing_decisions
        .iter()
        .map(|decision| decision.schema_key())
        .collect::<Vec<_>>()
        .join(",");
    let diagnostic = Diagnostic::new(
        DiagnosticCode::new(DCS_WRITER_EVIDENCE_PENDING_CODE)
            .expect("static DCS diagnostic code is valid"),
        Severity::Error,
        settings.provenance().anchor().object_path().clone(),
        settings.provenance().anchor().property_path().clone(),
        "DCS XML serialization requires exact writer evidence that is still pending",
    )
    .expect("static DCS diagnostic is bounded")
    .with_profiles(
        Some(settings.provenance().source_profile().clone()),
        Some(target_profile.clone()),
    )
    .with_context("schema.release", DCS_WRITER_EVIDENCE_RELEASE)
    .expect("static context is bounded")
    .with_context("schema.missing-keys", &missing_keys)
    .expect("bounded DCS decision set fits diagnostic context");
    DcsSerializationError {
        diagnostic: Box::new(diagnostic),
        missing_decisions,
    }
}

fn invalid_evidence(
    envelope: &DcsSettingsEnvelope,
    target_profile: &ProfileId,
    error: &dyn Display,
) -> DcsSerializationError {
    let settings = envelope.as_settings();
    let diagnostic = Diagnostic::new(
        DiagnosticCode::new(DCS_WRITER_EVIDENCE_INVALID_CODE)
            .expect("static DCS diagnostic code is valid"),
        Severity::Error,
        settings.provenance().anchor().object_path().clone(),
        settings.provenance().anchor().property_path().clone(),
        "bundled DCS writer evidence is invalid or unavailable",
    )
    .expect("static DCS diagnostic is bounded")
    .with_profiles(
        Some(settings.provenance().source_profile().clone()),
        Some(target_profile.clone()),
    )
    .with_context("schema.release", DCS_WRITER_EVIDENCE_RELEASE)
    .expect("static context is bounded")
    .with_context("schema.error", &error.to_string())
    .expect("schema errors are bounded by their corpus fields");
    DcsSerializationError {
        diagnostic: Box::new(diagnostic),
        missing_decisions: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use ibcmd_core::asset::MediaKind;
    use ibcmd_core::diagnostic::{ObjectPath, PathSegment, PropertyPath};
    use ibcmd_core::opaque::{OpaqueFacet, OpaqueFacets, OpaquePlacement};
    use ibcmd_core::provenance::{CanonicalAnchor, SourceProvenance};
    use ibcmd_core::value::{CanonicalText, EnumToken};

    use super::*;

    fn anchor(property: &str) -> CanonicalAnchor {
        CanonicalAnchor::new(
            ObjectPath::new(vec![PathSegment::name("dcs_settings").unwrap()]).unwrap(),
            PropertyPath::new(vec![PathSegment::name(property).unwrap()]).unwrap(),
        )
    }

    fn provenance(profile: &str, property: &str) -> SourceProvenance {
        SourceProvenance::with_locator(
            ProfileId::parse(profile).unwrap(),
            anchor(property),
            "fixture:dcs/settings.xml",
        )
        .unwrap()
    }

    fn envelope(list_settings: bool, opaque_profile: Option<&str>) -> DcsSettingsEnvelope {
        let opaque = opaque_profile
            .map(|profile| {
                OpaqueFacet::new(
                    provenance(profile, "extensions"),
                    OpaquePlacement::new("xml:child", 2).unwrap(),
                    b"<future xmlns=\"urn:future\" exact=\"yes\"/>".to_vec(),
                    MediaKind::new("application/xml").unwrap(),
                )
                .unwrap()
            })
            .into_iter()
            .collect();
        let settings = ibcmd_core::dcs::DcsSettings::new(
            Some(CanonicalText::new("main-settings").unwrap()),
            Some(EnumToken::new("QuickAccess").unwrap()),
            OpaqueFacets::new(opaque).unwrap(),
            provenance("platform:8.3.24", "settings"),
        )
        .unwrap();
        if list_settings {
            DcsSettingsEnvelope::list_settings(settings)
        } else {
            DcsSettingsEnvelope::settings(settings)
        }
    }

    #[test]
    fn every_dcs_writer_decision_is_explicitly_verified_or_pending() {
        assert_eq!(DCS_WRITER_EVIDENCE.len(), 11);
        assert_eq!(
            DCS_WRITER_EVIDENCE
                .iter()
                .filter(|entry| entry.status == DcsWriterEvidenceStatus::Verified)
                .map(|entry| entry.decision)
                .collect::<Vec<_>>(),
            vec![DcsWriterDecision::FormListSettingsDelegate]
        );
        assert!(
            DCS_WRITER_EVIDENCE
                .iter()
                .all(|entry| !entry.decision.schema_key().is_empty() && !entry.source.is_empty())
        );
    }

    #[test]
    fn standalone_and_form_use_one_boundary_and_report_exact_context_keys() {
        let target = ProfileId::parse("platform:8.3.24").unwrap();
        let standalone =
            preflight_dcs_settings_serialization(&envelope(false, None), &target).unwrap_err();
        let form =
            preflight_dcs_settings_serialization(&envelope(true, None), &target).unwrap_err();

        assert_eq!(
            standalone.diagnostic().code().as_str(),
            DCS_WRITER_EVIDENCE_PENDING_CODE
        );
        assert_eq!(
            form.diagnostic().code().as_str(),
            DCS_WRITER_EVIDENCE_PENDING_CODE
        );
        assert!(
            standalone
                .missing_decisions()
                .contains(&DcsWriterDecision::StandaloneDocumentQName)
        );
        assert!(
            form.missing_decisions()
                .contains(&DcsWriterDecision::FormListSettingsWrapperQName)
        );
        assert!(
            !form
                .missing_decisions()
                .contains(&DcsWriterDecision::FormListSettingsDelegate)
        );
        for common in [
            DcsWriterDecision::SettingsTypeId,
            DcsWriterDecision::ItemsUserSettingIdQName,
            DcsWriterDecision::ItemsUserSettingIdOrder,
            DcsWriterDecision::ItemsUserSettingIdDefaultEmission,
            DcsWriterDecision::ItemsViewModeQName,
            DcsWriterDecision::ItemsViewModeOrder,
            DcsWriterDecision::ItemsViewModeDefaultEmission,
        ] {
            assert!(standalone.missing_decisions().contains(&common));
            assert!(form.missing_decisions().contains(&common));
        }
    }

    #[test]
    fn same_profile_opaque_is_accepted_but_placement_stays_evidence_gated() {
        let target = ProfileId::parse("platform:8.3.24").unwrap();
        let error = preflight_dcs_settings_serialization(
            &envelope(false, Some("platform:8.3.24")),
            &target,
        )
        .unwrap_err();
        assert_eq!(
            error.diagnostic().code().as_str(),
            DCS_WRITER_EVIDENCE_PENDING_CODE
        );
        assert!(
            error
                .missing_decisions()
                .contains(&DcsWriterDecision::OpaqueExtensionPlacement)
        );
    }

    #[test]
    fn cross_profile_opaque_fails_before_schema_decisions_and_never_yields_a_permit() {
        let target = ProfileId::parse("platform:8.3.25").unwrap();
        let error =
            preflight_dcs_settings_serialization(&envelope(true, Some("platform:8.3.24")), &target)
                .unwrap_err();
        assert_eq!(
            error.diagnostic().code().as_str(),
            "opaque.cross-profile-emit-forbidden"
        );
        assert!(error.missing_decisions().is_empty());
    }

    #[test]
    fn pending_schema_keys_have_stable_sorted_diagnostic_context() {
        let target = ProfileId::parse("platform:8.3.24").unwrap();
        let error =
            preflight_dcs_settings_serialization(&envelope(false, None), &target).unwrap_err();
        let expected = error
            .missing_decisions()
            .iter()
            .map(|decision| decision.schema_key())
            .collect::<Vec<_>>()
            .join(",");
        assert_eq!(
            error
                .diagnostic()
                .context()
                .get("schema.missing-keys")
                .unwrap(),
            &expected
        );
        assert_eq!(
            error.diagnostic().context().get("schema.release").unwrap(),
            DCS_WRITER_EVIDENCE_RELEASE
        );
    }
}
