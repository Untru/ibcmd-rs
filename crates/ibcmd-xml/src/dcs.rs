//! Evidence-gated serialization boundary for canonical DCS settings.
//!
//! The bundled EDT corpus proves both physical wrappers, the absence of a
//! settings TypeId, and the final two typed settings children. Opaque facets
//! remain deliberately non-emittable because EDT provides no lossless slot.

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use ibcmd_core::artifact::ProfileId;
use ibcmd_core::dcs::{DcsSelectedField, DcsSelectedItem, DcsSelection, DcsSettingsEnvelope};
use ibcmd_core::diagnostic::{Diagnostic, DiagnosticCode, Severity};
use ibcmd_core::value::CanonicalText;
use ibcmd_schema::{
    DcsListSettingsTailField, FormListSettingsNullValue, SchemaError, WriterPolicy,
    WriterRuleCorpus, WriterRuleKey, bundled_dcs_list_settings_tail_policy,
    bundled_dcs_selection_policy, bundled_dcs_settings_serialization_policy, bundled_writer_rules,
};
use quick_xml::escape::escape;

use crate::node::{AttributeKind, XmlElement, XmlNode};
use crate::reader::XmlReader;

const DCS_SETTINGS_NAMESPACE: &str = "http://v8.1c.ru/8.1/data-composition-system/settings";
const XSI_NAMESPACE: &str = "http://www.w3.org/2001/XMLSchema-instance";

/// EDT release against which the DCS writer boundary was inspected.
pub const DCS_WRITER_EVIDENCE_RELEASE: &str = "2025.2.3+30";
/// Legacy diagnostic identifier retained for source compatibility. The bounded
/// Stable diagnostic for a DCS writer decision that has no platform evidence.
pub const DCS_WRITER_EVIDENCE_PENDING_CODE: &str = "dcs.writer-evidence-pending";
/// Stable diagnostic emitted when the embedded corpus cannot prove a claimed rule.
pub const DCS_WRITER_EVIDENCE_INVALID_CODE: &str = "dcs.writer-evidence-invalid";
/// Stable diagnostic emitted when opaque DCS XML cannot be placed losslessly.
pub const DCS_OPAQUE_NO_LOSSLESS_PLACEMENT_CODE: &str =
    "dcs.opaque-extension-no-lossless-placement";
/// Stable diagnostic emitted when a canonical scalar is not XML 1.0 text.
pub const DCS_INVALID_XML_VALUE_CODE: &str = "dcs.invalid-xml-value";

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
    /// Root selection QName, item variants, order, and empty policy.
    RootSelectionPolicy,
    /// Whether Form `ListSettings` has a physical root-selection ingress.
    FormListSettingsSelectionIngress,
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
            Self::RootSelectionPolicy => "dcs.DataCompositionSettings.selection.policy",
            Self::FormListSettingsSelectionIngress => {
                "form.DynamicListExtInfo.listSettings.selection.ingress"
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
        status: DcsWriterEvidenceStatus::Verified,
        source: "dcs-writer-evidence:dcs.settings.document.qname",
    },
    DcsWriterEvidence {
        decision: DcsWriterDecision::FormListSettingsWrapperQName,
        status: DcsWriterEvidenceStatus::Verified,
        source: "dcs-writer-evidence:form.DynamicListExtInfo.listSettings.qname",
    },
    DcsWriterEvidence {
        decision: DcsWriterDecision::SettingsTypeId,
        status: DcsWriterEvidenceStatus::Verified,
        source: "dcs-writer-evidence:DataCompositionSettings/type-id-absent",
    },
    DcsWriterEvidence {
        decision: DcsWriterDecision::ItemsUserSettingIdQName,
        status: DcsWriterEvidenceStatus::Verified,
        source: "dcs-writer-evidence:DataCompositionSettings/itemsUserSettingID:qname",
    },
    DcsWriterEvidence {
        decision: DcsWriterDecision::ItemsUserSettingIdOrder,
        status: DcsWriterEvidenceStatus::Verified,
        source: "dcs-writer-evidence:DataCompositionSettings/verified-tail-order",
    },
    DcsWriterEvidence {
        decision: DcsWriterDecision::ItemsUserSettingIdDefaultEmission,
        status: DcsWriterEvidenceStatus::Verified,
        source: "dcs-writer-evidence:DataCompositionSettings/itemsUserSettingID",
    },
    DcsWriterEvidence {
        decision: DcsWriterDecision::ItemsViewModeQName,
        status: DcsWriterEvidenceStatus::Verified,
        source: "dcs-writer-evidence:DataCompositionSettings/itemsViewMode:qname",
    },
    DcsWriterEvidence {
        decision: DcsWriterDecision::ItemsViewModeOrder,
        status: DcsWriterEvidenceStatus::Verified,
        source: "dcs-writer-evidence:DataCompositionSettings/verified-tail-order",
    },
    DcsWriterEvidence {
        decision: DcsWriterDecision::ItemsViewModeDefaultEmission,
        status: DcsWriterEvidenceStatus::Verified,
        source: "dcs-writer-evidence:DataCompositionSettings/itemsViewMode",
    },
    DcsWriterEvidence {
        decision: DcsWriterDecision::RootSelectionPolicy,
        status: DcsWriterEvidenceStatus::Verified,
        source: "native-evidence:8.3.27.2214-xml-2.20-dcs-selection-auto/root-selection",
    },
    DcsWriterEvidence {
        decision: DcsWriterDecision::FormListSettingsSelectionIngress,
        status: DcsWriterEvidenceStatus::Pending,
        source: "no-platform-authenticated-form-list-settings-selection-cohort",
    },
    DcsWriterEvidence {
        decision: DcsWriterDecision::OpaqueExtensionPlacement,
        status: DcsWriterEvidenceStatus::Verified,
        source: "dcs-writer-evidence:opaque-extension:no-lossless-placement",
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

/// Proof that all typed decisions required by the shared DCS boundary resolved.
///
/// There is intentionally no public constructor; opaque settings never receive
/// this permit because their lossless physical placement is unsupported.
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

/// Narrow failure produced before any Form `ListSettings` tail bytes are returned.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DcsListSettingsTailError {
    Schema(SchemaError),
    InvalidFormat(&'static str),
    InvalidValue(&'static str),
}

impl Display for DcsListSettingsTailError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Schema(error) => write!(formatter, "{error}"),
            Self::InvalidFormat(reason) => {
                write!(formatter, "invalid ListSettings tail format: {reason}")
            }
            Self::InvalidValue(field) => {
                write!(formatter, "invalid ListSettings tail value for {field}")
            }
        }
    }
}

impl Error for DcsListSettingsTailError {}

impl From<SchemaError> for DcsListSettingsTailError {
    fn from(error: SchemaError) -> Self {
        Self::Schema(error)
    }
}

/// Failure to emit the verified scalar children from canonical DCS settings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DcsSettingsChildrenError {
    /// The canonical envelope or its evidence gate rejected serialization.
    Serialization(DcsSerializationError),
    /// The caller-supplied lexical fragment context is invalid.
    Fragment(DcsListSettingsTailError),
}

/// Platform-evidenced typed children read from a DCS settings root.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DcsSettingsTypedChildren {
    selection: Option<DcsSelection>,
    items_view_mode: Option<String>,
    items_user_setting_id: Option<String>,
}

impl DcsSettingsTypedChildren {
    /// Returns the bounded direct root selection, if present.
    pub const fn selection(&self) -> Option<&DcsSelection> {
        self.selection.as_ref()
    }

    /// Returns the direct `itemsViewMode` value, if present.
    pub fn items_view_mode(&self) -> Option<&str> {
        self.items_view_mode.as_deref()
    }

    /// Returns the direct `itemsUserSettingID` value, if present.
    pub fn items_user_setting_id(&self) -> Option<&str> {
        self.items_user_setting_id.as_deref()
    }
}

impl Display for DcsSettingsChildrenError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Serialization(error) => write!(formatter, "{error}"),
            Self::Fragment(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for DcsSettingsChildrenError {}

impl From<DcsSerializationError> for DcsSettingsChildrenError {
    fn from(error: DcsSerializationError) -> Self {
        Self::Serialization(error)
    }
}

impl From<DcsListSettingsTailError> for DcsSettingsChildrenError {
    fn from(error: DcsListSettingsTailError) -> Self {
        Self::Fragment(error)
    }
}

/// Emits only the two verified final children of a caller-owned Form
/// `ListSettings` wrapper.
pub fn emit_form_list_settings_tail(
    items_view_mode: Option<&str>,
    items_user_setting_id: Option<&str>,
    prefix: &str,
    indent: &str,
) -> Result<String, DcsListSettingsTailError> {
    if !is_xml_prefix(prefix) {
        return Err(DcsListSettingsTailError::InvalidFormat(
            "prefix must be a bounded XML NCName",
        ));
    }
    if indent.len() > 64 || !indent.bytes().all(|byte| matches!(byte, b' ' | b'\t')) {
        return Err(DcsListSettingsTailError::InvalidFormat(
            "indent must contain at most 64 spaces or tabs",
        ));
    }
    for (field, value) in [
        ("itemsViewMode", items_view_mode),
        ("itemsUserSettingID", items_user_setting_id),
    ] {
        if value.is_some_and(|value| {
            value.len() > 4 * 1024 || value.chars().any(|character| !is_xml_1_0_char(character))
        }) {
            return Err(DcsListSettingsTailError::InvalidValue(field));
        }
    }

    let policy = bundled_dcs_list_settings_tail_policy()?;
    let mut output = String::new();
    for field in policy.tail_order() {
        let (qname, value, default) = match field {
            DcsListSettingsTailField::ItemsViewMode => (
                policy.items_view_mode_qname(),
                items_view_mode,
                policy.items_view_mode_default(),
            ),
            DcsListSettingsTailField::ItemsUserSettingId => (
                policy.items_user_setting_id_qname(),
                items_user_setting_id,
                policy.items_user_setting_id_default(),
            ),
        };
        let Some(value) = value.filter(|value| *value != default) else {
            continue;
        };
        let local_name = qname
            .rsplit_once('}')
            .map(|(_, local_name)| local_name)
            .ok_or(DcsListSettingsTailError::InvalidFormat(
                "verified QName is not expanded",
            ))?;
        output.push_str(indent);
        output.push('<');
        output.push_str(prefix);
        output.push(':');
        output.push_str(local_name);
        output.push('>');
        output.push_str(&escape(value));
        output.push_str("</");
        output.push_str(prefix);
        output.push(':');
        output.push_str(local_name);
        output.push_str(">\r\n");
    }
    Ok(output)
}

/// Emits the verified settings children from the shared canonical IR.
///
/// The physical wrapper remains caller-owned because standalone DCS and Form
/// embed the same semantics in different surrounding documents. Both callers
/// still cross the same evidence preflight and omission/order policy here.
pub fn emit_dcs_settings_children(
    envelope: &DcsSettingsEnvelope,
    target_profile: &ProfileId,
    prefix: &str,
    indent: &str,
) -> Result<String, DcsSettingsChildrenError> {
    let parts = emit_dcs_settings_children_parts(envelope, target_profile, prefix, indent)?;
    let mut output = parts.selection.unwrap_or_default();
    output.push_str(&parts.tail);
    Ok(output)
}

/// Atomic typed fragments whose positions are governed independently: root
/// selection precedes order/structure items, while scalar fields form the
/// verified final tail.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DcsSettingsChildrenParts {
    selection: Option<String>,
    tail: String,
}

impl DcsSettingsChildrenParts {
    pub fn selection(&self) -> Option<&str> {
        self.selection.as_deref()
    }
    pub fn tail(&self) -> &str {
        &self.tail
    }
}

/// Emits platform-authenticated settings fragments without choosing their
/// positions in a caller-owned surrounding document.
pub fn emit_dcs_settings_children_parts(
    envelope: &DcsSettingsEnvelope,
    target_profile: &ProfileId,
    prefix: &str,
    indent: &str,
) -> Result<DcsSettingsChildrenParts, DcsSettingsChildrenError> {
    preflight_dcs_settings_serialization(envelope, target_profile)?;
    let settings = envelope.as_settings();
    let selection = settings
        .selection()
        .map(|selection| emit_dcs_selection(selection, prefix, indent))
        .transpose()?;
    let tail = emit_form_list_settings_tail(
        settings.items_view_mode().map(|value| value.as_str()),
        settings.items_user_setting_id().map(|value| value.as_str()),
        prefix,
        indent,
    )
    .map_err(DcsSettingsChildrenError::from)?;
    Ok(DcsSettingsChildrenParts { selection, tail })
}

fn emit_dcs_selection(
    selection: &DcsSelection,
    prefix: &str,
    indent: &str,
) -> Result<String, DcsSettingsChildrenError> {
    if !is_xml_prefix(prefix)
        || indent.len() > 64
        || !indent.bytes().all(|byte| matches!(byte, b' ' | b'\t'))
    {
        return Err(DcsListSettingsTailError::InvalidFormat(
            "selection prefix or indent is invalid",
        )
        .into());
    }
    let policy = bundled_dcs_selection_policy().map_err(DcsListSettingsTailError::from)?;
    if !policy.precedes_order_and_structure_items()
        || !policy.empty_selection_is_unsupported()
        || selection.items().is_empty()
    {
        return Err(DcsListSettingsTailError::InvalidFormat(
            "selection placement or empty-emission policy is unsupported",
        )
        .into());
    }
    let selection_local = expanded_local_name(policy.selection_qname())?;
    let item_local = expanded_local_name(policy.item_qname())?;
    let field_local = expanded_local_name(policy.field_qname())?;
    let field_type = expanded_local_name(policy.field_type_qname())?;
    let auto_type = expanded_local_name(policy.auto_type_qname())?;
    let nested = format!("{indent}\t");
    let field_indent = format!("{nested}\t");
    let mut output = format!("{indent}<{prefix}:{selection_local}>\r\n");
    for item in selection.items() {
        match item {
            DcsSelectedItem::Field(field) => {
                output.push_str(&format!(
                    "{nested}<{prefix}:{item_local} xsi:type=\"{prefix}:{field_type}\">\r\n"
                ));
                output.push_str(&format!(
                    "{field_indent}<{prefix}:{field_local}>{}</{prefix}:{field_local}>\r\n",
                    escape(field.field().as_str())
                ));
                output.push_str(&format!("{nested}</{prefix}:{item_local}>\r\n"));
            }
            DcsSelectedItem::Auto => output.push_str(&format!(
                "{nested}<{prefix}:{item_local} xsi:type=\"{prefix}:{auto_type}\"/>\r\n"
            )),
        }
    }
    output.push_str(&format!("{indent}</{prefix}:{selection_local}>\r\n"));
    Ok(output)
}

fn expanded_local_name(expanded: &str) -> Result<&str, DcsListSettingsTailError> {
    expanded.rsplit_once('}').map(|(_, local)| local).ok_or(
        DcsListSettingsTailError::InvalidFormat("verified selection QName is not expanded"),
    )
}

/// Reads the verified typed children from a standalone DCS `Settings`
/// document.
///
/// Unknown siblings and unsupported selections remain with their owning codec.
/// Duplicate scalars, scalar attributes, complex scalar content, or an inexact
/// scalar namespace fail closed.
pub fn parse_dcs_settings_children(document: &str) -> Option<DcsSettingsTypedChildren> {
    let document = XmlReader::from_slice(document.as_bytes()).ok()?;
    let root = document.root();
    if root.name().local() != "Settings"
        || !xml_element_uses_namespace(root, root, DCS_SETTINGS_NAMESPACE)
    {
        return None;
    }
    let mut children = DcsSettingsTypedChildren::default();
    let mut selection_candidate = None;
    let mut selection_is_ambiguous = false;
    for node in root.children() {
        let XmlNode::Element(element) = node else {
            continue;
        };
        if element.name().local() == "selection" {
            if selection_candidate.replace(element).is_some() {
                selection_is_ambiguous = true;
            }
            continue;
        }
        let target = match element.name().local() {
            "itemsViewMode" => &mut children.items_view_mode,
            "itemsUserSettingID" => &mut children.items_user_setting_id,
            _ => continue,
        };
        if !xml_element_uses_namespace(element, root, DCS_SETTINGS_NAMESPACE)
            || target.is_some()
            || element
                .attributes()
                .iter()
                .any(|attribute| matches!(attribute.kind(), AttributeKind::Ordinary(_)))
        {
            return None;
        }
        let mut value = String::new();
        for child in element.children() {
            match child {
                XmlNode::Text(text) => value.push_str(text.value()),
                XmlNode::CData(text) => value.push_str(text.value()),
                _ => return None,
            }
        }
        *target = Some(value);
    }
    if !selection_is_ambiguous && let Some(element) = selection_candidate {
        children.selection = parse_dcs_selection(element, root);
    }
    Some(children)
}

fn parse_dcs_selection(element: &XmlElement, root: &XmlElement) -> Option<DcsSelection> {
    let policy = bundled_dcs_selection_policy().ok()?;
    if !xml_element_uses_namespace(element, root, policy.namespace_uri())
        || element
            .attributes()
            .iter()
            .any(|attribute| matches!(attribute.kind(), AttributeKind::Ordinary(_)))
    {
        return None;
    }
    let mut items = Vec::new();
    for node in element.children() {
        let item = match node {
            XmlNode::Text(text) if text.value().trim().is_empty() => continue,
            XmlNode::CData(text) if text.value().trim().is_empty() => continue,
            XmlNode::Element(item) => item,
            _ => return None,
        };
        if item.name().local() != "item"
            || !xml_element_uses_namespace(item, root, policy.namespace_uri())
        {
            return None;
        }
        let item_type = resolved_xsi_type(item, root)?;
        if item_type == policy.field_type_qname() {
            let mut field = None;
            for child in item.children() {
                let field_element = match child {
                    XmlNode::Text(text) if text.value().trim().is_empty() => continue,
                    XmlNode::CData(text) if text.value().trim().is_empty() => continue,
                    XmlNode::Element(field_element) => field_element,
                    _ => return None,
                };
                if field.is_some()
                    || field_element.name().local() != "field"
                    || !xml_element_uses_namespace(field_element, root, policy.namespace_uri())
                    || field_element
                        .attributes()
                        .iter()
                        .any(|attribute| matches!(attribute.kind(), AttributeKind::Ordinary(_)))
                {
                    return None;
                }
                let value = simple_element_text(field_element)?;
                field = Some(DcsSelectedField::new(CanonicalText::new(&value).ok()?).ok()?);
            }
            items.push(DcsSelectedItem::Field(field?));
        } else if item_type == policy.auto_type_qname() {
            if item.children().iter().any(|child| match child {
                XmlNode::Text(text) => !text.value().trim().is_empty(),
                XmlNode::CData(text) => !text.value().trim().is_empty(),
                _ => true,
            }) {
                return None;
            }
            items.push(DcsSelectedItem::Auto);
        } else {
            return None;
        }
    }
    DcsSelection::new(items).ok()
}

fn simple_element_text(element: &XmlElement) -> Option<String> {
    let mut value = String::new();
    for child in element.children() {
        match child {
            XmlNode::Text(text) => value.push_str(text.value()),
            XmlNode::CData(text) => value.push_str(text.value()),
            _ => return None,
        }
    }
    (!value.is_empty()).then_some(value)
}

fn resolved_xsi_type(element: &XmlElement, root: &XmlElement) -> Option<String> {
    let mut value = None;
    for attribute in element.attributes() {
        let AttributeKind::Ordinary(name) = attribute.kind() else {
            continue;
        };
        if name.local() == "type"
            && namespace_for_prefix(element, root, name.prefix()) == Some(XSI_NAMESPACE)
        {
            if value.is_some() {
                return None;
            }
            value = Some(attribute.value());
        } else {
            return None;
        }
    }
    let value = value?;
    let (prefix, local) = value
        .split_once(':')
        .map_or((None, value), |(prefix, local)| (Some(prefix), local));
    if local.is_empty() || (prefix.is_some() && value.matches(':').count() != 1) {
        return None;
    }
    let namespace = namespace_for_prefix(element, root, prefix)?;
    Some(format!("{{{namespace}}}{local}"))
}

/// Replaces the verified typed children in a canonical DCS settings fragment
/// with already evidence-gated serializations.
///
/// This keeps QName recognition and child placement in the XML layer instead
/// of leaking them into a physical provider adapter.
pub fn rewrite_dcs_settings_children(
    xml: &mut String,
    children: &DcsSettingsTypedChildren,
    serialized_selection: Option<&str>,
    serialized_tail: &str,
) -> Option<()> {
    if children.selection.is_some() {
        remove_first_canonical_dcs_child(xml, "selection")?;
        let selection = serialized_selection?;
        let root_start = xml.find("<dcsset:settings")?;
        let root_open_end = xml[root_start..].find('>')?.checked_add(root_start + 1)?;
        let insertion = if xml[root_open_end..].starts_with("\r\n") {
            root_open_end + 2
        } else {
            root_open_end
        };
        xml.insert_str(insertion, selection);
    } else if serialized_selection.is_some() {
        return None;
    }
    for (remove, local) in [
        (children.items_view_mode.is_some(), "itemsViewMode"),
        (
            children.items_user_setting_id.is_some(),
            "itemsUserSettingID",
        ),
    ] {
        if remove {
            remove_unique_canonical_dcs_child(xml, local)?;
        }
    }
    if children.items_view_mode.is_none() && children.items_user_setting_id.is_none() {
        return serialized_tail.is_empty().then_some(());
    }
    let closing = "</dcsset:settings>";
    let closing_offset = xml.rfind(closing)?;
    let insertion = xml[..closing_offset]
        .trim_end_matches(['\r', '\n', '\t', ' '])
        .len();
    xml.replace_range(insertion..closing_offset, "");
    let insertion_text = if serialized_tail.is_empty() {
        "\r\n".to_string()
    } else {
        format!("\r\n{serialized_tail}")
    };
    xml.insert_str(insertion, &insertion_text);
    Some(())
}

fn remove_first_canonical_dcs_child(xml: &mut String, local: &str) -> Option<()> {
    let open = format!("<dcsset:{local}>");
    let empty = format!("<dcsset:{local}/>");
    let close = format!("</dcsset:{local}>");
    let (start, end) = if let Some(start) = xml.find(&open) {
        let content_start = start.checked_add(open.len())?;
        let relative_end = xml[content_start..].find(&close)?;
        let end = content_start
            .checked_add(relative_end)?
            .checked_add(close.len())?;
        (start, end)
    } else {
        let start = xml.find(&empty)?;
        (start, start.checked_add(empty.len())?)
    };
    let line_start = xml[..start].rfind('\n').map_or(0, |offset| offset + 1);
    let start = if xml[line_start..start]
        .bytes()
        .all(|byte| matches!(byte, b' ' | b'\t'))
    {
        line_start
    } else {
        start
    };
    let end = if xml[end..].starts_with("\r\n") {
        end + 2
    } else if xml[end..].starts_with('\n') {
        end + 1
    } else {
        end
    };
    xml.replace_range(start..end, "");
    Some(())
}

fn xml_element_uses_namespace(element: &XmlElement, root: &XmlElement, uri: &str) -> bool {
    namespace_for_prefix(element, root, element.name().prefix()) == Some(uri)
}

fn namespace_for_prefix<'a>(
    element: &'a XmlElement,
    root: &'a XmlElement,
    prefix: Option<&str>,
) -> Option<&'a str> {
    fn declaration<'a>(element: &'a XmlElement, prefix: Option<&str>) -> Option<&'a str> {
        element.attributes().iter().find_map(|attribute| {
            let AttributeKind::Namespace(declared_prefix) = attribute.kind() else {
                return None;
            };
            (declared_prefix.as_deref() == prefix).then_some(attribute.value())
        })
    }
    declaration(element, prefix).or_else(|| {
        if std::ptr::eq(element, root) {
            None
        } else {
            declaration(root, prefix)
        }
    })
}

fn remove_unique_canonical_dcs_child(xml: &mut String, local: &str) -> Option<()> {
    let open = format!("<dcsset:{local}>");
    let empty = format!("<dcsset:{local}/>");
    let close = format!("</dcsset:{local}>");
    let (start, end) = if let Some(start) = xml.find(&open) {
        let content_start = start.checked_add(open.len())?;
        let relative_end = xml[content_start..].find(&close)?;
        let end = content_start
            .checked_add(relative_end)?
            .checked_add(close.len())?;
        (start, end)
    } else {
        let start = xml.find(&empty)?;
        (start, start.checked_add(empty.len())?)
    };
    if xml[end..].contains(&open) || xml[end..].contains(&empty) {
        return None;
    }
    xml.replace_range(start..end, "");
    Some(())
}

fn is_xml_prefix(prefix: &str) -> bool {
    if prefix.is_empty()
        || prefix.len() > 64
        || prefix.eq_ignore_ascii_case("xml")
        || prefix.eq_ignore_ascii_case("xmlns")
    {
        return false;
    }
    let mut characters = prefix.chars();
    characters.next().is_some_and(is_ncname_start_char) && characters.all(is_ncname_char)
}

fn is_ncname_start_char(character: char) -> bool {
    matches!(
        character,
        'A'..='Z'
            | '_'
            | 'a'..='z'
            | '\u{c0}'..='\u{d6}'
            | '\u{d8}'..='\u{f6}'
            | '\u{f8}'..='\u{2ff}'
            | '\u{370}'..='\u{37d}'
            | '\u{37f}'..='\u{1fff}'
            | '\u{200c}'..='\u{200d}'
            | '\u{2070}'..='\u{218f}'
            | '\u{2c00}'..='\u{2fef}'
            | '\u{3001}'..='\u{d7ff}'
            | '\u{f900}'..='\u{fdcf}'
            | '\u{fdf0}'..='\u{fffd}'
            | '\u{10000}'..='\u{effff}'
    )
}

fn is_ncname_char(character: char) -> bool {
    is_ncname_start_char(character)
        || matches!(
            character,
            '-' | '.' | '0'..='9' | '\u{b7}' | '\u{300}'..='\u{36f}' | '\u{203f}'..='\u{2040}'
        )
}

fn is_xml_1_0_char(character: char) -> bool {
    matches!(
        character,
        '\u{9}' | '\u{a}' | '\u{d}' | '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}'
    )
}

/// Runs the single schema/evidence boundary shared by standalone settings and
/// Form `ListSettings`.
///
/// The function never returns a permit for opaque settings. EDT rejects and
/// discards unknown `readSettings` children, so emitting retained facets would
/// fabricate a placement and lose source semantics.
pub fn preflight_dcs_settings_serialization(
    envelope: &DcsSettingsEnvelope,
    target_profile: &ProfileId,
) -> Result<DcsSerializationPermit, DcsSerializationError> {
    let settings = envelope.as_settings();

    if !settings.opaque_extensions().is_empty() {
        return Err(unsupported_opaque_placement(envelope, target_profile));
    }

    let writer_rules = bundled_writer_rules()
        .map_err(|error| invalid_evidence(envelope, target_profile, &error))?;
    bundled_dcs_settings_serialization_policy()
        .map_err(|error| invalid_evidence(envelope, target_profile, &error))?;
    if settings.selection().is_some() {
        bundled_dcs_selection_policy()
            .map_err(|error| invalid_evidence(envelope, target_profile, &error))?;
    }

    if matches!(envelope, DcsSettingsEnvelope::ListSettings(_)) {
        if settings.selection().is_some() {
            return Err(unsupported_form_selection(envelope, target_profile));
        }
        verify_form_list_settings_delegate(envelope, target_profile, &writer_rules)?;
    }

    Ok(DcsSerializationPermit {
        target_profile: target_profile.clone(),
        form_list_settings: matches!(envelope, DcsSettingsEnvelope::ListSettings(_)),
    })
}

/// Emits the complete bounded standalone or Form `ListSettings` envelope.
///
/// This deliberately has no extension hook: a nonempty opaque facet is a
/// stable unsupported decision, not an invitation to choose a guessed slot.
pub fn emit_dcs_settings_envelope(
    envelope: &DcsSettingsEnvelope,
    target_profile: &ProfileId,
) -> Result<String, DcsSerializationError> {
    preflight_dcs_settings_serialization(envelope, target_profile)?;
    let wrappers = bundled_dcs_settings_serialization_policy()
        .map_err(|error| invalid_evidence(envelope, target_profile, &error))?;
    let tail = bundled_dcs_list_settings_tail_policy()
        .map_err(|error| invalid_evidence(envelope, target_profile, &error))?;
    let (wrapper_qname, form_wrapper) = match envelope {
        DcsSettingsEnvelope::Settings(_) => (wrappers.standalone_document_qname(), false),
        DcsSettingsEnvelope::ListSettings(_) => (wrappers.form_list_settings_qname(), true),
    };
    let (wrapper_namespace, wrapper_name) = expanded_qname(wrapper_qname).ok_or_else(|| {
        invalid_evidence(envelope, target_profile, &"invalid verified wrapper QName")
    })?;
    let settings = envelope.as_settings();
    if settings.selection().is_some() {
        return Err(invalid_evidence(
            envelope,
            target_profile,
            &"complete envelope selection requires caller-owned namespace prefix and placement",
        ));
    }
    for (field, value) in [
        (
            "itemsViewMode",
            settings.items_view_mode().map(|value| value.as_str()),
        ),
        (
            "itemsUserSettingID",
            settings.items_user_setting_id().map(|value| value.as_str()),
        ),
    ] {
        if let Some(value) = value
            && value.chars().any(|character| !is_xml_1_0_char(character))
        {
            return Err(invalid_xml_value(envelope, target_profile, field));
        }
    }
    let emits_view_mode = settings
        .items_view_mode()
        .is_some_and(|value| value.as_str() != tail.items_view_mode_default());
    let emits_user_setting_id = settings
        .items_user_setting_id()
        .is_some_and(|value| value.as_str() != tail.items_user_setting_id_default());
    if !emits_view_mode && !emits_user_setting_id {
        return Ok(format!(
            "<{wrapper_name} xmlns=\"{wrapper_namespace}\"/>\r\n"
        ));
    }
    let mut output = format!("<{wrapper_name} xmlns=\"{wrapper_namespace}\">\r\n");
    for field in tail.tail_order() {
        let (qname, value, default) = match field {
            DcsListSettingsTailField::ItemsViewMode => (
                tail.items_view_mode_qname(),
                settings.items_view_mode().map(|value| value.as_str()),
                tail.items_view_mode_default(),
            ),
            DcsListSettingsTailField::ItemsUserSettingId => (
                tail.items_user_setting_id_qname(),
                settings.items_user_setting_id().map(|value| value.as_str()),
                tail.items_user_setting_id_default(),
            ),
        };
        let Some(value) = value.filter(|value| *value != default) else {
            continue;
        };
        let (namespace, local_name) = expanded_qname(qname).ok_or_else(|| {
            invalid_evidence(
                envelope,
                target_profile,
                &"invalid verified settings child QName",
            )
        })?;
        output.push_str("\t<");
        output.push_str(local_name);
        if form_wrapper {
            output.push_str(" xmlns=\"");
            output.push_str(namespace);
            output.push('"');
        }
        output.push('>');
        output.push_str(&escape(value));
        output.push_str("</");
        output.push_str(local_name);
        output.push_str(">\r\n");
    }
    output.push_str("</");
    output.push_str(wrapper_name);
    output.push_str(">\r\n");
    Ok(output)
}

fn expanded_qname(value: &str) -> Option<(&str, &str)> {
    let (namespace, local_name) = value.strip_prefix('{')?.split_once('}')?;
    (!namespace.is_empty() && !local_name.is_empty()).then_some((namespace, local_name))
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

fn unsupported_opaque_placement(
    envelope: &DcsSettingsEnvelope,
    target_profile: &ProfileId,
) -> DcsSerializationError {
    let settings = envelope.as_settings();
    let diagnostic = Diagnostic::new(
        DiagnosticCode::new(DCS_OPAQUE_NO_LOSSLESS_PLACEMENT_CODE)
            .expect("static DCS diagnostic code is valid"),
        Severity::Error,
        settings.provenance().anchor().object_path().clone(),
        settings.provenance().anchor().property_path().clone(),
        "DCS opaque extensions have no EDT-verified lossless XML placement",
    )
    .expect("static DCS diagnostic is bounded")
    .with_profiles(
        Some(settings.provenance().source_profile().clone()),
        Some(target_profile.clone()),
    )
    .with_context("schema.release", DCS_WRITER_EVIDENCE_RELEASE)
    .expect("static context is bounded")
    .with_context(
        "schema.unsupported-key",
        DcsWriterDecision::OpaqueExtensionPlacement.schema_key(),
    )
    .expect("static unsupported decision fits diagnostic context");
    DcsSerializationError {
        diagnostic: Box::new(diagnostic),
        missing_decisions: vec![DcsWriterDecision::OpaqueExtensionPlacement],
    }
}

fn unsupported_form_selection(
    envelope: &DcsSettingsEnvelope,
    target_profile: &ProfileId,
) -> DcsSerializationError {
    let settings = envelope.as_settings();
    let diagnostic = Diagnostic::new(
        DiagnosticCode::new(DCS_WRITER_EVIDENCE_PENDING_CODE)
            .expect("static DCS diagnostic code is valid"),
        Severity::Error,
        settings.provenance().anchor().object_path().clone(),
        settings.provenance().anchor().property_path().clone(),
        "Form ListSettings has no platform-authenticated root-selection ingress",
    )
    .expect("static DCS diagnostic is bounded")
    .with_profiles(
        Some(settings.provenance().source_profile().clone()),
        Some(target_profile.clone()),
    )
    .with_context("schema.release", DCS_WRITER_EVIDENCE_RELEASE)
    .expect("static context is bounded")
    .with_context(
        "schema.unsupported-key",
        DcsWriterDecision::FormListSettingsSelectionIngress.schema_key(),
    )
    .expect("static unsupported decision fits diagnostic context");
    DcsSerializationError {
        diagnostic: Box::new(diagnostic),
        missing_decisions: vec![DcsWriterDecision::FormListSettingsSelectionIngress],
    }
}

fn invalid_xml_value(
    envelope: &DcsSettingsEnvelope,
    target_profile: &ProfileId,
    field: &'static str,
) -> DcsSerializationError {
    let settings = envelope.as_settings();
    let diagnostic = Diagnostic::new(
        DiagnosticCode::new(DCS_INVALID_XML_VALUE_CODE)
            .expect("static DCS diagnostic code is valid"),
        Severity::Error,
        settings.provenance().anchor().object_path().clone(),
        settings.provenance().anchor().property_path().clone(),
        "DCS settings contain a character forbidden by XML 1.0",
    )
    .expect("static DCS diagnostic is bounded")
    .with_profiles(
        Some(settings.provenance().source_profile().clone()),
        Some(target_profile.clone()),
    )
    .with_context("dcs.field", field)
    .expect("static DCS field name fits diagnostic context");
    DcsSerializationError {
        diagnostic: Box::new(diagnostic),
        missing_decisions: Vec::new(),
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
            None,
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

    fn empty_envelope(list_settings: bool) -> DcsSettingsEnvelope {
        let settings = ibcmd_core::dcs::DcsSettings::new(
            None,
            None,
            Some(EnumToken::new("QuickAccess").unwrap()),
            OpaqueFacets::new(Vec::new()).unwrap(),
            provenance("platform:8.3.24", "settings"),
        )
        .unwrap();
        if list_settings {
            DcsSettingsEnvelope::list_settings(settings)
        } else {
            DcsSettingsEnvelope::settings(settings)
        }
    }

    fn invalid_text_envelope(list_settings: bool, invalid: char) -> DcsSettingsEnvelope {
        let settings = ibcmd_core::dcs::DcsSettings::new(
            None,
            Some(CanonicalText::new(&format!("bad{invalid}value")).unwrap()),
            None,
            OpaqueFacets::new(Vec::new()).unwrap(),
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
        assert_eq!(DCS_WRITER_EVIDENCE.len(), 13);
        assert!(
            DCS_WRITER_EVIDENCE
                .iter()
                .all(|entry| !entry.decision.schema_key().is_empty() && !entry.source.is_empty())
        );
        assert_eq!(
            DCS_WRITER_EVIDENCE
                .iter()
                .filter(|entry| entry.status == DcsWriterEvidenceStatus::Verified)
                .map(|entry| entry.decision)
                .collect::<Vec<_>>(),
            vec![
                DcsWriterDecision::StandaloneDocumentQName,
                DcsWriterDecision::FormListSettingsWrapperQName,
                DcsWriterDecision::SettingsTypeId,
                DcsWriterDecision::ItemsUserSettingIdQName,
                DcsWriterDecision::ItemsUserSettingIdOrder,
                DcsWriterDecision::ItemsUserSettingIdDefaultEmission,
                DcsWriterDecision::ItemsViewModeQName,
                DcsWriterDecision::ItemsViewModeOrder,
                DcsWriterDecision::ItemsViewModeDefaultEmission,
                DcsWriterDecision::RootSelectionPolicy,
                DcsWriterDecision::OpaqueExtensionPlacement,
                DcsWriterDecision::FormListSettingsDelegate,
            ]
        );
    }

    #[test]
    fn known_settings_preflight_yields_context_exact_permits() {
        let target = ProfileId::parse("platform:8.3.24").unwrap();
        let standalone =
            preflight_dcs_settings_serialization(&envelope(false, None), &target).unwrap();
        let form = preflight_dcs_settings_serialization(&envelope(true, None), &target).unwrap();
        assert!(!standalone.is_form_list_settings());
        assert!(form.is_form_list_settings());
        assert_eq!(standalone.target_profile(), &target);
        assert_eq!(form.target_profile(), &target);
    }

    #[test]
    fn aggregate_evidence_matrix_exposes_form_selection_gap_and_negative_opaque_source() {
        assert_eq!(
            DCS_WRITER_EVIDENCE
                .iter()
                .filter(|entry| entry.status == DcsWriterEvidenceStatus::Pending)
                .map(|entry| entry.decision)
                .collect::<Vec<_>>(),
            [DcsWriterDecision::FormListSettingsSelectionIngress]
        );
        let opaque = DCS_WRITER_EVIDENCE
            .iter()
            .find(|entry| entry.decision == DcsWriterDecision::OpaqueExtensionPlacement)
            .unwrap();
        assert_eq!(opaque.status, DcsWriterEvidenceStatus::Verified);
        assert!(opaque.source.contains("no-lossless-placement"));
    }

    #[test]
    fn form_list_settings_tail_uses_verified_order_escaping_and_omission() {
        let emitted =
            emit_form_list_settings_tail(Some("Compact<&"), Some("id<&"), "dcsset", "\t\t")
                .unwrap();
        assert_eq!(
            emitted,
            concat!(
                "\t\t<dcsset:itemsViewMode>Compact&lt;&amp;</dcsset:itemsViewMode>\r\n",
                "\t\t<dcsset:itemsUserSettingID>id&lt;&amp;</dcsset:itemsUserSettingID>\r\n",
            )
        );
        assert_eq!(
            emit_form_list_settings_tail(None, None, "dcsset", "\t").unwrap(),
            ""
        );
        assert_eq!(
            emit_form_list_settings_tail(Some("QuickAccess"), Some(""), "dcsset", "\t").unwrap(),
            ""
        );
        assert!(
            emit_form_list_settings_tail(Some("Compact\nMode"), None, "dcsset", "")
                .unwrap()
                .contains("Compact\nMode")
        );
        for prefix in ["bad:prefix", "1bad", "xml", "XML", "xmlns", "XmlNs"] {
            assert!(
                emit_form_list_settings_tail(Some("Compact"), None, prefix, "").is_err(),
                "{prefix}"
            );
        }
        assert!(emit_form_list_settings_tail(Some("Compact"), None, "параметр", "").is_ok());
        for forbidden in ['\0', '\u{1f}', '\u{fffe}', '\u{ffff}'] {
            let value = format!("Compact{forbidden}");
            assert!(
                emit_form_list_settings_tail(Some(&value), None, "dcsset", "").is_err(),
                "U+{:04X}",
                u32::from(forbidden)
            );
        }
        assert!(
            emit_form_list_settings_tail(Some(&"x".repeat(4 * 1024 + 1)), None, "dcsset", "")
                .is_err()
        );
    }

    #[test]
    fn canonical_settings_children_are_identical_across_physical_contexts() {
        let target = ProfileId::parse("xml-2.20").unwrap();
        let standalone =
            emit_dcs_settings_children(&envelope(false, None), &target, "dcsset", "\t").unwrap();
        let form =
            emit_dcs_settings_children(&envelope(true, None), &target, "dcsset", "\t").unwrap();
        assert_eq!(standalone, form);
        assert_eq!(
            standalone,
            "\t<dcsset:itemsUserSettingID>main-settings</dcsset:itemsUserSettingID>\r\n"
        );
    }

    #[test]
    fn settings_scalar_parse_and_rewrite_are_owned_by_the_xml_layer() {
        let document = concat!(
            "<s:Settings xmlns:s=\"http://v8.1c.ru/8.1/data-composition-system/settings\">",
            "<s:order/>",
            "<s:itemsUserSettingID>id&lt;&amp;</s:itemsUserSettingID>",
            "<s:itemsViewMode>Compact</s:itemsViewMode>",
            "</s:Settings>"
        );
        let children = parse_dcs_settings_children(document).unwrap();
        assert_eq!(children.items_view_mode(), Some("Compact"));
        assert_eq!(children.items_user_setting_id(), Some("id<&"));

        let mut canonical = concat!(
            "<dcsset:settings>\r\n",
            "\t<dcsset:itemsUserSettingID>stale</dcsset:itemsUserSettingID>\r\n",
            "\t<dcsset:itemsViewMode>stale</dcsset:itemsViewMode>\r\n",
            "</dcsset:settings>"
        )
        .to_owned();
        rewrite_dcs_settings_children(
            &mut canonical,
            &children,
            None,
            concat!(
                "\t<dcsset:itemsViewMode>Compact</dcsset:itemsViewMode>\r\n",
                "\t<dcsset:itemsUserSettingID>id&lt;&amp;</dcsset:itemsUserSettingID>\r\n"
            ),
        )
        .unwrap();
        assert_eq!(canonical.matches("itemsViewMode").count(), 2);
        assert_eq!(canonical.matches("itemsUserSettingID").count(), 2);
        assert!(canonical.find("Compact").unwrap() < canonical.find("id&lt;&amp;").unwrap());
    }

    #[test]
    fn platform_selection_field_and_auto_parse_and_emit_atomically() {
        let document = concat!(
            "<Settings xmlns=\"http://v8.1c.ru/8.1/data-composition-system/settings\" ",
            "xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">",
            "<selection>",
            "<item xsi:type=\"SelectedItemField\"><field>Name</field></item>",
            "<item xsi:type=\"SelectedItemAuto\"/>",
            "<item xsi:type=\"SelectedItemField\"><field>A&lt;&amp;</field></item>",
            "</selection><order/>",
            "</Settings>"
        );
        let parsed = parse_dcs_settings_children(document).unwrap();
        let items = parsed.selection().unwrap().items();
        let [
            DcsSelectedItem::Field(name),
            DcsSelectedItem::Auto,
            DcsSelectedItem::Field(escaped),
        ] = items
        else {
            panic!("unexpected selected-item sequence: {items:?}");
        };
        assert_eq!(name.field().as_str(), "Name");
        assert_eq!(escaped.field().as_str(), "A<&");

        let target = ProfileId::parse("platform:8.3.27").unwrap();
        let settings = ibcmd_core::dcs::DcsSettings::new(
            parsed.selection().cloned(),
            None,
            None,
            OpaqueFacets::new(Vec::new()).unwrap(),
            provenance("platform:8.3.27", "selection"),
        )
        .unwrap();
        let envelope = DcsSettingsEnvelope::settings(settings);
        let parts = emit_dcs_settings_children_parts(&envelope, &target, "dcsset", "\t").unwrap();
        assert_eq!(
            parts.selection().unwrap(),
            concat!(
                "\t<dcsset:selection>\r\n",
                "\t\t<dcsset:item xsi:type=\"dcsset:SelectedItemField\">\r\n",
                "\t\t\t<dcsset:field>Name</dcsset:field>\r\n",
                "\t\t</dcsset:item>\r\n",
                "\t\t<dcsset:item xsi:type=\"dcsset:SelectedItemAuto\"/>\r\n",
                "\t\t<dcsset:item xsi:type=\"dcsset:SelectedItemField\">\r\n",
                "\t\t\t<dcsset:field>A&lt;&amp;</dcsset:field>\r\n",
                "\t\t</dcsset:item>\r\n",
                "\t</dcsset:selection>\r\n"
            )
        );
        assert!(parts.tail().is_empty());
        assert!(emit_dcs_settings_envelope(&envelope, &target).is_err());

        let form = DcsSettingsEnvelope::list_settings(envelope.as_settings().clone());
        let error = emit_dcs_settings_children_parts(&form, &target, "dcsset", "\t").unwrap_err();
        let DcsSettingsChildrenError::Serialization(error) = error else {
            panic!("Form selection must fail at the evidence boundary");
        };
        assert_eq!(
            error.diagnostic().code().as_str(),
            DCS_WRITER_EVIDENCE_PENDING_CODE
        );
        assert_eq!(
            error.missing_decisions(),
            [DcsWriterDecision::FormListSettingsSelectionIngress]
        );
    }

    #[test]
    fn unsupported_selection_shapes_remain_outside_typed_ownership() {
        for selection in [
            "<selection/>",
            "<selection><item xsi:type=\"SelectedItemFolder\"/></selection>",
            "<selection><item xsi:type=\"SelectedItemAuto\"><use>false</use></item></selection>",
            "<selection><item xsi:type=\"SelectedItemField\"><field/></item></selection>",
            "<selection><item extra=\"1\" xsi:type=\"SelectedItemAuto\"/></selection>",
            "<selection><item xsi:type=\"SelectedItemField\"><field>Name</field><field>Other</field></item></selection>",
        ] {
            let document = format!(
                "<Settings xmlns=\"{DCS_SETTINGS_NAMESPACE}\" xmlns:xsi=\"{XSI_NAMESPACE}\">{selection}</Settings>"
            );
            let parsed = parse_dcs_settings_children(&document).unwrap();
            assert!(
                parsed.selection().is_none(),
                "typed unsupported selection: {selection}"
            );
        }

        let duplicate = format!(
            "<Settings xmlns=\"{DCS_SETTINGS_NAMESPACE}\" xmlns:xsi=\"{XSI_NAMESPACE}\"><selection><item xsi:type=\"SelectedItemAuto\"/></selection><selection><item xsi:type=\"SelectedItemAuto\"/></selection></Settings>"
        );
        assert!(
            parse_dcs_settings_children(&duplicate)
                .unwrap()
                .selection()
                .is_none()
        );

        let wrong_namespace = format!(
            "<Settings xmlns=\"{DCS_SETTINGS_NAMESPACE}\" xmlns:xsi=\"{XSI_NAMESPACE}\"><bad:selection xmlns:bad=\"urn:bad\"><bad:item xsi:type=\"bad:SelectedItemAuto\"/></bad:selection></Settings>"
        );
        assert!(
            parse_dcs_settings_children(&wrong_namespace)
                .unwrap()
                .selection()
                .is_none()
        );
    }

    #[test]
    fn opaque_settings_fail_closed_with_a_stable_unsupported_decision() {
        let target = ProfileId::parse("platform:8.3.24").unwrap();
        let error = preflight_dcs_settings_serialization(
            &envelope(false, Some("platform:8.3.24")),
            &target,
        )
        .unwrap_err();
        assert_eq!(
            error.diagnostic().code().as_str(),
            DCS_OPAQUE_NO_LOSSLESS_PLACEMENT_CODE
        );
        assert_eq!(
            error.missing_decisions(),
            [DcsWriterDecision::OpaqueExtensionPlacement]
        );
        assert_eq!(
            error
                .diagnostic()
                .context()
                .get("schema.unsupported-key")
                .unwrap(),
            DcsWriterDecision::OpaqueExtensionPlacement.schema_key()
        );
        assert_eq!(
            emit_dcs_settings_envelope(&envelope(false, Some("platform:8.3.24")), &target)
                .unwrap_err()
                .diagnostic()
                .code()
                .as_str(),
            DCS_OPAQUE_NO_LOSSLESS_PLACEMENT_CODE
        );
    }

    #[test]
    fn opaque_settings_never_fall_back_to_cross_profile_replay() {
        let target = ProfileId::parse("platform:8.3.25").unwrap();
        let error =
            preflight_dcs_settings_serialization(&envelope(true, Some("platform:8.3.24")), &target)
                .unwrap_err();
        assert_eq!(
            error.diagnostic().code().as_str(),
            DCS_OPAQUE_NO_LOSSLESS_PLACEMENT_CODE
        );
        assert_eq!(
            error.missing_decisions(),
            [DcsWriterDecision::OpaqueExtensionPlacement]
        );
    }

    #[test]
    fn exact_standalone_and_form_envelopes_have_no_xsi_type() {
        let target = ProfileId::parse("platform:8.3.24").unwrap();
        let standalone = emit_dcs_settings_envelope(&envelope(false, None), &target).unwrap();
        let form = emit_dcs_settings_envelope(&envelope(true, None), &target).unwrap();
        assert_eq!(
            standalone,
            concat!(
                "<Settings xmlns=\"http://v8.1c.ru/8.1/data-composition-system/settings\">\r\n",
                "\t<itemsUserSettingID>main-settings</itemsUserSettingID>\r\n",
                "</Settings>\r\n",
            )
        );
        assert_eq!(
            form,
            concat!(
                "<ListSettings xmlns=\"http://v8.1c.ru/8.3/xcf/logform\">\r\n",
                "\t<itemsUserSettingID xmlns=\"http://v8.1c.ru/8.1/data-composition-system/settings\">main-settings</itemsUserSettingID>\r\n",
                "</ListSettings>\r\n",
            )
        );
        assert!(!standalone.contains("xsi:type"));
        assert!(!form.contains("xsi:type"));
    }

    #[test]
    fn default_settings_use_the_verified_empty_element_shape_in_both_contexts() {
        let target = ProfileId::parse("platform:8.3.24").unwrap();
        assert_eq!(
            emit_dcs_settings_envelope(&empty_envelope(false), &target).unwrap(),
            "<Settings xmlns=\"http://v8.1c.ru/8.1/data-composition-system/settings\"/>\r\n"
        );
        assert_eq!(
            emit_dcs_settings_envelope(&empty_envelope(true), &target).unwrap(),
            "<ListSettings xmlns=\"http://v8.1c.ru/8.3/xcf/logform\"/>\r\n"
        );
    }

    #[test]
    fn invalid_xml_text_is_rejected_before_any_envelope_is_returned() {
        let target = ProfileId::parse("platform:8.3.24").unwrap();
        for list_settings in [false, true] {
            for invalid in ['\0', '\u{1f}', '\u{fffe}', '\u{ffff}'] {
                let error = emit_dcs_settings_envelope(
                    &invalid_text_envelope(list_settings, invalid),
                    &target,
                )
                .unwrap_err();
                assert_eq!(
                    error.diagnostic().code().as_str(),
                    DCS_INVALID_XML_VALUE_CODE
                );
                assert_eq!(
                    error.diagnostic().context().get("dcs.field").unwrap(),
                    "itemsUserSettingID"
                );
                assert!(error.missing_decisions().is_empty());
            }
        }
    }
}
