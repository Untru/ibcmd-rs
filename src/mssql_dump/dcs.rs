use super::*;
use ibcmd_core::dcs::{
    DcsBuildError, DcsConditionalAppearance, DcsFilter, DcsOrder, DcsOutputParameters,
    DcsSelection, DcsSettingsBuilder, DcsSettingsEnvelope,
};
use ibcmd_core::diagnostic::{DiagnosticCode, PathSegment, PropertyPath, Severity};
use ibcmd_core::opaque::OpaqueFacets;
use ibcmd_core::provenance::{CanonicalAnchor, SourceProvenance};
use ibcmd_core::value::{CanonicalText, EnumToken};
use ibcmd_xml::{
    DcsChildParseOutcome, DcsInlineSettingsFragment, DcsInnerSchemaError, DcsSchemaTemplateError,
    DcsSettingsChildrenError, DcsSettingsChildrenParts, DcsSettingsDocumentAnalysisError,
    analyze_dcs_schema_template_documents_with_references, analyze_dcs_settings_document,
    emit_dcs_area_template_source_fragment, emit_dcs_inner_schema_source_document,
    emit_dcs_query_union_link_source_document, emit_dcs_settings_children_parts,
    parse_dcs_area_template_storage_document_with_references,
    parse_dcs_inner_schema_storage_document_with_references,
    parse_dcs_query_union_link_storage_document_with_references, rewrite_dcs_settings_children,
};

const DCS_SCHEMA_NS: &[u8] = b"http://v8.1c.ru/8.1/data-composition-system/schema";
const DCS_COMMON_NS: &[u8] = b"http://v8.1c.ru/8.1/data-composition-system/common";
const DCS_CORE_NS: &[u8] = b"http://v8.1c.ru/8.1/data-composition-system/core";
const DCS_SETTINGS_NS: &[u8] = b"http://v8.1c.ru/8.1/data-composition-system/settings";
const DCS_AREA_TEMPLATE_NS: &[u8] = b"http://v8.1c.ru/8.1/data-composition-system/area-template";
const DATA_CORE_NS: &[u8] = b"http://v8.1c.ru/8.1/data/core";
const DATA_UI_NS: &[u8] = b"http://v8.1c.ru/8.1/data/ui";
const ENTERPRISE_NS: &[u8] = b"http://v8.1c.ru/8.1/data/enterprise";
const CURRENT_CONFIG_NS: &[u8] = b"http://v8.1c.ru/8.1/data/enterprise/current-config";
const STYLE_NS: &[u8] = b"http://v8.1c.ru/8.1/data/ui/style";
const SYS_NS: &[u8] = b"http://v8.1c.ru/8.1/data/ui/fonts/system";
const WEB_NS: &[u8] = b"http://v8.1c.ru/8.1/data/ui/colors/web";
const WIN_NS: &[u8] = b"http://v8.1c.ru/8.1/data/ui/colors/windows";
const XSI_NS: &[u8] = b"http://www.w3.org/2001/XMLSchema-instance";
const XS_NS: &[u8] = b"http://www.w3.org/2001/XMLSchema";
const DCS_AREA_TEMPLATE_URI: &str = "http://v8.1c.ru/8.1/data-composition-system/area-template";
const ENTERPRISE_URI: &str = "http://v8.1c.ru/8.1/data/enterprise";
const CURRENT_CONFIG_URI: &str = "http://v8.1c.ru/8.1/data/enterprise/current-config";
const ANY_IB_REF_TYPE_ID: &str = "280f5f0e-9c8a-49cc-bf6d-4d296cc17a63";
const CFG_PREFIX: &str = "cfg:";
const SETTINGS_ROOT_UI_NAMESPACES: &str = " xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\" xmlns:sys=\"http://v8.1c.ru/8.1/data/ui/fonts/system\" xmlns:web=\"http://v8.1c.ru/8.1/data/ui/colors/web\" xmlns:win=\"http://v8.1c.ru/8.1/data/ui/colors/windows\"";

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum CanonicalDcsSettingsContext {
    Standalone,
    FormListSettings,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum CanonicalDcsSettingsAdapterError {
    Value {
        field: &'static str,
        message: String,
    },
    Provenance(String),
    Build(DcsBuildError),
    Serialize(DcsSettingsChildrenError),
}

impl std::fmt::Display for CanonicalDcsSettingsAdapterError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Value { field, message } => {
                write!(formatter, "invalid canonical DCS {field}: {message}")
            }
            Self::Provenance(message) => {
                write!(formatter, "invalid canonical DCS provenance: {message}")
            }
            Self::Build(error) => write!(formatter, "{error}"),
            Self::Serialize(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for CanonicalDcsSettingsAdapterError {}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct CanonicalDcsSettingsInput<'a> {
    pub(super) selection: Option<&'a DcsSelection>,
    pub(super) filter: Option<&'a DcsFilter>,
    pub(super) order: Option<&'a DcsOrder>,
    pub(super) conditional_appearance: Option<&'a DcsConditionalAppearance>,
    pub(super) output_parameters: Option<&'a DcsOutputParameters>,
    pub(super) items_view_mode: Option<&'a str>,
    pub(super) items_user_setting_id: Option<&'a str>,
}

pub(super) fn emit_canonical_dcs_settings_children(
    context: CanonicalDcsSettingsContext,
    input: CanonicalDcsSettingsInput<'_>,
    source_profile: &ProfileId,
    target_profile: &ProfileId,
    prefix: &str,
    indent: &str,
    locator: &str,
) -> Result<String, CanonicalDcsSettingsAdapterError> {
    let parts = emit_canonical_dcs_settings_parts(
        context,
        input,
        source_profile,
        target_profile,
        prefix,
        indent,
        locator,
    )?;
    let mut output = parts.selection().unwrap_or_default().to_owned();
    output.push_str(parts.filter().unwrap_or_default());
    output.push_str(parts.order().unwrap_or_default());
    output.push_str(parts.conditional_appearance().unwrap_or_default());
    output.push_str(parts.tail());
    Ok(output)
}

pub(super) fn emit_canonical_dcs_settings_parts(
    context: CanonicalDcsSettingsContext,
    input: CanonicalDcsSettingsInput<'_>,
    source_profile: &ProfileId,
    target_profile: &ProfileId,
    prefix: &str,
    indent: &str,
    locator: &str,
) -> Result<DcsSettingsChildrenParts, CanonicalDcsSettingsAdapterError> {
    let items_view_mode = input
        .items_view_mode
        .map(EnumToken::new)
        .transpose()
        .map_err(|error| CanonicalDcsSettingsAdapterError::Value {
            field: "itemsViewMode",
            message: error.to_string(),
        })?;
    let items_user_setting_id = input
        .items_user_setting_id
        .map(CanonicalText::new)
        .transpose()
        .map_err(|error| CanonicalDcsSettingsAdapterError::Value {
            field: "itemsUserSettingID",
            message: error.to_string(),
        })?;
    let anchor = CanonicalAnchor::new(
        ObjectPath::new(vec![
            PathSegment::name("dcs_settings").expect("static DCS object path is valid"),
        ])
        .expect("static DCS object path is bounded"),
        PropertyPath::root(),
    );
    let provenance = SourceProvenance::with_locator(source_profile.clone(), anchor, locator)
        .map_err(|error| CanonicalDcsSettingsAdapterError::Provenance(error.to_string()))?;
    let settings = DcsSettingsBuilder::new(provenance)
        .selection(input.selection.cloned())
        .filter(input.filter.cloned())
        .order(input.order.cloned())
        .conditional_appearance(input.conditional_appearance.cloned())
        .output_parameters(input.output_parameters.cloned())
        .items_user_setting_id(items_user_setting_id)
        .items_view_mode(items_view_mode)
        .opaque_extensions(OpaqueFacets::new(Vec::new()).expect("empty opaque facets are valid"))
        .build()
        .map_err(CanonicalDcsSettingsAdapterError::Build)?;
    let envelope = match context {
        CanonicalDcsSettingsContext::Standalone => DcsSettingsEnvelope::settings(settings),
        CanonicalDcsSettingsContext::FormListSettings => {
            DcsSettingsEnvelope::list_settings(settings)
        }
    };
    emit_dcs_settings_children_parts(&envelope, target_profile, prefix, indent)
        .map_err(CanonicalDcsSettingsAdapterError::Serialize)
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum DcsTypeResolution {
    KeepId,
    Type { qname: String },
    TypeSet { qname: String },
}

pub(crate) type DcsTypeIndex = BTreeMap<String, DcsTypeResolution>;

/// Stage-typed reason one native three-document DCS schema template could not
/// be normalized into its native source XML.
///
/// The normalizer chains several independently fallible steps (envelope
/// analysis, primary-schema parse, per-variant settings canonicalization,
/// terminal AreaTemplate splice). Collapsing them into a bare `None` made the
/// dominant `cf export` failure class diagnostically invisible, so every step
/// now names itself. This carries observability only: each variant is produced
/// exactly where the previous code produced `None`, so what counts as success
/// is unchanged.
///
/// Diagnostics reuse the crate-wide vocabulary rather than a parallel one:
/// [`Self::diagnostic_code`] returns a validated [`DiagnosticCode`] under the
/// same `dcs.*` kebab namespace `ibcmd-xml` already publishes, and
/// [`Self::class`] maps each step onto [`MetadataSourceFailureClass`], the
/// failure taxonomy the metadata-source ledger already reports.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum DcsTemplateNormalizeError {
    /// The three-document envelope itself was not admitted.
    EnvelopeAnalysis(DcsSchemaTemplateError),
    /// A settings variant document was not valid UTF-8.
    SettingsDocumentNotUtf8 { index: usize },
    /// A settings variant did not canonicalize.
    SettingsCanonicalize {
        index: usize,
        cause: DcsSettingsCanonicalizeError,
    },
    /// A canonicalized settings variant was rejected by the inline-fragment
    /// analyzer that re-verifies it before it is spliced into the schema.
    SettingsFragmentParse {
        index: usize,
        cause: DcsInnerSchemaError,
    },
    /// The primary schema file matched neither admitted parser shape.
    PrimarySchemaParse {
        inner_schema: DcsInnerSchemaError,
        query_union_link: DcsInnerSchemaError,
    },
    /// The `DataSetObject` schema emitter rejected the parsed schema.
    InnerSchemaEmit(DcsInnerSchemaError),
    /// The query/union/link emitter rejected the parsed schema.
    QueryUnionLinkEmit(DcsInnerSchemaError),
    /// The terminal AreaTemplate parsed but its source fragment was rejected.
    AreaTemplateEmit(DcsInnerSchemaError),
    /// The emitted schema had no `settingsVariant` anchor to splice against.
    AreaTemplateAnchorMissing,
    /// The spliced document length overflowed the platform size boundary.
    AreaTemplateSizeOverflow,
}

impl DcsTemplateNormalizeError {
    /// Returns the stable diagnostic code for the failing step.
    pub(crate) const fn code(&self) -> &'static str {
        match self {
            Self::EnvelopeAnalysis(_) => "dcs.template-normalize.envelope-analysis",
            Self::SettingsDocumentNotUtf8 { .. } => "dcs.template-normalize.settings-not-utf8",
            Self::SettingsCanonicalize { cause, .. } => cause.code(),
            Self::SettingsFragmentParse { .. } => "dcs.template-normalize.settings-fragment-parse",
            Self::PrimarySchemaParse { .. } => "dcs.template-normalize.primary-schema-parse",
            Self::InnerSchemaEmit(_) => "dcs.template-normalize.inner-schema-emit",
            Self::QueryUnionLinkEmit(_) => "dcs.template-normalize.query-union-link-emit",
            Self::AreaTemplateEmit(_) => "dcs.template-normalize.area-template-emit",
            Self::AreaTemplateAnchorMissing => {
                "dcs.template-normalize.area-template-anchor-missing"
            }
            Self::AreaTemplateSizeOverflow => "dcs.template-normalize.area-template-size-overflow",
        }
    }

    /// Returns the validated diagnostic code for the failing step.
    pub(crate) fn diagnostic_code(&self) -> DiagnosticCode {
        DiagnosticCode::new(self.code()).expect("static DCS normalize codes are valid")
    }

    /// Classifies the failing step in the shared metadata-source taxonomy.
    pub(crate) const fn class(&self) -> MetadataSourceFailureClass {
        match self {
            Self::EnvelopeAnalysis(error) => match error {
                DcsSchemaTemplateError::InvalidEvidence(_) => MetadataSourceFailureClass::Invariant,
                DcsSchemaTemplateError::Malformed(_) => MetadataSourceFailureClass::Malformed,
                DcsSchemaTemplateError::UnsupportedSource(_) => {
                    MetadataSourceFailureClass::Unsupported
                }
            },
            Self::SettingsDocumentNotUtf8 { .. } => MetadataSourceFailureClass::Malformed,
            Self::SettingsCanonicalize { cause, .. } => cause.class(),
            Self::SettingsFragmentParse { cause, .. }
            | Self::InnerSchemaEmit(cause)
            | Self::QueryUnionLinkEmit(cause)
            | Self::AreaTemplateEmit(cause) => inner_schema_failure_class(cause),
            // Neither admitted parser recognized the shape: the source is
            // outside the evidenced cohort, not malformed.
            Self::PrimarySchemaParse { .. } => MetadataSourceFailureClass::Unsupported,
            Self::AreaTemplateAnchorMissing => MetadataSourceFailureClass::Invariant,
            Self::AreaTemplateSizeOverflow => MetadataSourceFailureClass::Invariant,
        }
    }

    /// Every normalize failure is fail-closed.
    pub(crate) const fn severity(&self) -> Severity {
        Severity::Error
    }
}

/// Renders a severity with the same snake_case tokens the persisted
/// diagnostic ledger serializes.
const fn severity_token(severity: Severity) -> &'static str {
    match severity {
        Severity::Info => "info",
        Severity::Warning => "warning",
        Severity::Error => "error",
    }
}

const fn inner_schema_failure_class(error: &DcsInnerSchemaError) -> MetadataSourceFailureClass {
    match error {
        DcsInnerSchemaError::InvalidEvidence(_) => MetadataSourceFailureClass::Invariant,
        DcsInnerSchemaError::Malformed(_) => MetadataSourceFailureClass::Malformed,
        DcsInnerSchemaError::UnsupportedSource(_) => MetadataSourceFailureClass::Unsupported,
        DcsInnerSchemaError::Build(_) => MetadataSourceFailureClass::Invariant,
    }
}

impl std::fmt::Display for DcsTemplateNormalizeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Header shape mirrors the persisted ledger entries: severity, stable
        // code, failure classification.
        write!(
            formatter,
            "[{} {} {}] ",
            severity_token(self.severity()),
            self.diagnostic_code(),
            self.class().as_str()
        )?;
        match self {
            Self::EnvelopeAnalysis(error) => {
                write!(formatter, "DCS template envelope was not admitted: {error}")
            }
            Self::SettingsDocumentNotUtf8 { index } => {
                write!(formatter, "settings variant {index} is not valid UTF-8 XML")
            }
            Self::SettingsCanonicalize { index, cause } => {
                write!(formatter, "settings variant {index}: {cause}")
            }
            Self::SettingsFragmentParse { index, cause } => write!(
                formatter,
                "canonicalized settings variant {index} was rejected by the inline fragment analyzer: {cause}"
            ),
            Self::PrimarySchemaParse {
                inner_schema,
                query_union_link,
            } => write!(
                formatter,
                "primary schema file matched no admitted parser (inner schema: {inner_schema}; query/union/link: {query_union_link})"
            ),
            Self::InnerSchemaEmit(error) => {
                write!(formatter, "inner schema source emit failed: {error}")
            }
            Self::QueryUnionLinkEmit(error) => {
                write!(formatter, "query/union/link source emit failed: {error}")
            }
            Self::AreaTemplateEmit(error) => write!(
                formatter,
                "terminal AreaTemplate source fragment emit failed: {error}"
            ),
            Self::AreaTemplateAnchorMissing => formatter.write_str(
                "emitted schema has no settingsVariant anchor to splice the AreaTemplate against",
            ),
            Self::AreaTemplateSizeOverflow => formatter
                .write_str("spliced AreaTemplate document exceeds the platform size boundary"),
        }
    }
}

impl std::error::Error for DcsTemplateNormalizeError {}

/// One typed direct child of a DCS `Settings` root.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum DcsSettingsChild {
    Selection,
    Filter,
    Order,
    ConditionalAppearance,
    OutputParameters,
}

impl DcsSettingsChild {
    /// Returns the source element name.
    const fn element(self) -> &'static str {
        match self {
            Self::Selection => "selection",
            Self::Filter => "filter",
            Self::Order => "order",
            Self::ConditionalAppearance => "conditionalAppearance",
            Self::OutputParameters => "outputParameters",
        }
    }

    /// Returns the stable diagnostic code for an unsupported shape here.
    const fn unsupported_code(self) -> &'static str {
        match self {
            Self::Selection => "dcs.settings-canonicalize.unsupported-child.selection",
            Self::Filter => "dcs.settings-canonicalize.unsupported-child.filter",
            Self::Order => "dcs.settings-canonicalize.unsupported-child.order",
            Self::ConditionalAppearance => {
                "dcs.settings-canonicalize.unsupported-child.conditional-appearance"
            }
            Self::OutputParameters => {
                "dcs.settings-canonicalize.unsupported-child.output-parameters"
            }
        }
    }
}

/// Stage-typed reason one DCS `Settings` document did not canonicalize.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum DcsSettingsCanonicalizeError {
    /// The shared settings analyzer rejected the document.
    Analysis(DcsSettingsDocumentAnalysisError),
    /// One direct `Settings` child parsed into an unsupported shape.
    UnsupportedChild {
        child: DcsSettingsChild,
        reason: &'static str,
    },
    /// The lexical settings rewriter could not reproduce the document.
    Writer,
    /// The canonical DCS IR could not be re-serialized into children.
    CanonicalChildren(CanonicalDcsSettingsAdapterError),
    /// The canonical children could not be spliced back into the document.
    ChildrenRewrite,
}

impl DcsSettingsCanonicalizeError {
    /// Returns the stable diagnostic code for the failing step.
    pub(crate) const fn code(&self) -> &'static str {
        match self {
            Self::Analysis(_) => "dcs.settings-canonicalize.analysis",
            Self::UnsupportedChild { child, .. } => child.unsupported_code(),
            Self::Writer => "dcs.settings-canonicalize.writer",
            Self::CanonicalChildren(_) => "dcs.settings-canonicalize.canonical-children",
            Self::ChildrenRewrite => "dcs.settings-canonicalize.children-rewrite",
        }
    }

    /// Classifies the failing step in the shared metadata-source taxonomy.
    pub(crate) const fn class(&self) -> MetadataSourceFailureClass {
        match self {
            Self::Analysis(error) => match error {
                DcsSettingsDocumentAnalysisError::Malformed(_) => {
                    MetadataSourceFailureClass::Malformed
                }
                DcsSettingsDocumentAnalysisError::UnsupportedSource { .. } => {
                    MetadataSourceFailureClass::Unsupported
                }
            },
            Self::UnsupportedChild { .. } | Self::Writer => MetadataSourceFailureClass::Unsupported,
            Self::CanonicalChildren(_) | Self::ChildrenRewrite => {
                MetadataSourceFailureClass::Invariant
            }
        }
    }
}

impl std::fmt::Display for DcsSettingsCanonicalizeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Analysis(error) => {
                write!(
                    formatter,
                    "settings analyzer rejected the document: {error}"
                )
            }
            Self::UnsupportedChild { child, reason } => write!(
                formatter,
                "direct settings child `{}` is unsupported: {reason}",
                child.element()
            ),
            Self::Writer => formatter
                .write_str("lexical settings writer could not reproduce the source document"),
            Self::CanonicalChildren(error) => write!(
                formatter,
                "canonical settings children could not be serialized: {error}"
            ),
            Self::ChildrenRewrite => formatter
                .write_str("canonical settings children could not be spliced into the document"),
        }
    }
}

impl std::error::Error for DcsSettingsCanonicalizeError {}

pub(crate) fn normalize_data_composition_schema_template_documents_with_profiles(
    documents: &[&[u8]],
    type_index: &DcsTypeIndex,
    object_refs: &BTreeMap<String, String>,
    source_profile: &ProfileId,
    target_profile: &ProfileId,
) -> std::result::Result<Vec<u8>, DcsTemplateNormalizeError> {
    // Same shape/convention as `reference_types` below, but sourced from
    // `object_refs` (already keyed by lowercase canonical uuid, the same
    // convention `data_composition_style_item_name` uses for conditional
    // appearance) rather than `type_index`, and filtered to StyleItem
    // objects specifically. Built up front: the envelope's own structural
    // validation independently re-parses the terminal AreaTemplate and
    // needs the same resolver to accept the custom-StyleItem coordinate.
    let style_reference_types = object_refs
        .iter()
        .filter_map(|(uuid, reference)| {
            reference
                .strip_prefix("StyleItem.")
                .map(|name| (uuid.clone(), name.to_owned()))
        })
        .collect();
    let envelope =
        analyze_dcs_schema_template_documents_with_references(documents, &style_reference_types)
            .map_err(DcsTemplateNormalizeError::EnvelopeAnalysis)?;
    let reference_types = type_index
        .iter()
        .filter_map(|(type_id, resolution)| match resolution {
            DcsTypeResolution::Type { qname } => qname
                .strip_prefix(CFG_PREFIX)
                .map(|name| (type_id.clone(), name.to_owned())),
            DcsTypeResolution::KeepId | DcsTypeResolution::TypeSet { .. } => None,
        })
        .collect();
    let schema = parse_dcs_inner_schema_storage_document_with_references(
        envelope.primary_schema_file(),
        source_profile.clone(),
        "mssql:dcs-schema-template/primary-schema-file",
        &reference_types,
    );
    let mut settings = Vec::with_capacity(envelope.settings().len());
    for (index, document) in envelope.settings().iter().enumerate() {
        let document = std::str::from_utf8(document)
            .map_err(|_| DcsTemplateNormalizeError::SettingsDocumentNotUtf8 { index })?;
        let canonical = canonicalize_data_composition_settings_document(
            document,
            object_refs,
            source_profile,
            target_profile,
        )
        .map_err(|cause| DcsTemplateNormalizeError::SettingsCanonicalize { index, cause })?;
        settings.push(
            DcsInlineSettingsFragment::parse(canonical).map_err(|cause| {
                DcsTemplateNormalizeError::SettingsFragmentParse { index, cause }
            })?,
        );
    }
    let mut source = match schema {
        Ok(schema) => emit_dcs_inner_schema_source_document(&schema, &settings)
            .map_err(DcsTemplateNormalizeError::InnerSchemaEmit)?,
        Err(inner_schema) => {
            let schema = parse_dcs_query_union_link_storage_document_with_references(
                envelope.primary_schema_file(),
                source_profile.clone(),
                "mssql:dcs-schema-template/query-union-link",
                &reference_types,
            )
            .map_err(|query_union_link| {
                DcsTemplateNormalizeError::PrimarySchemaParse {
                    inner_schema,
                    query_union_link,
                }
            })?;
            emit_dcs_query_union_link_source_document(&schema, &settings)
                .map_err(DcsTemplateNormalizeError::QueryUnionLinkEmit)?
        }
    };
    let terminal = envelope.terminal_schema_file();
    if let Ok(area) = parse_dcs_area_template_storage_document_with_references(
        terminal,
        source_profile.clone(),
        "mssql:dcs-schema-template/area-template",
        &style_reference_types,
    ) {
        let fragment = emit_dcs_area_template_source_fragment(&area)
            .map_err(DcsTemplateNormalizeError::AreaTemplateEmit)?;
        let variant = b"\r\n\t<settingsVariant>";
        let offset = source
            .windows(variant.len())
            .position(|window| window == variant)
            .ok_or(DcsTemplateNormalizeError::AreaTemplateAnchorMissing)?;
        let capacity = source
            .len()
            .checked_add(fragment.len())
            .and_then(|length| length.checked_add(2))
            .ok_or(DcsTemplateNormalizeError::AreaTemplateSizeOverflow)?;
        let mut with_area = Vec::with_capacity(capacity);
        with_area.extend_from_slice(&source[..offset]);
        with_area.extend_from_slice(b"\r\n");
        with_area.extend_from_slice(&fragment);
        with_area.extend_from_slice(&source[offset..]);
        source = with_area;
    }
    Ok(source)
}

pub(crate) fn data_composition_type_id_xml(
    type_id: &str,
    type_index: &DcsTypeIndex,
    current_config_prefix: &str,
    declare_current_config_namespace: bool,
    characteristic_type_set_as_type: bool,
) -> Option<String> {
    let (element, qname) = if type_id.eq_ignore_ascii_case(ANY_IB_REF_TYPE_ID) {
        ("TypeSet", "cfg:AnyIBRef")
    } else {
        match type_index.get(&type_id.to_ascii_lowercase())? {
            DcsTypeResolution::KeepId => return None,
            DcsTypeResolution::Type { qname } => ("Type", qname.as_str()),
            DcsTypeResolution::TypeSet { qname } => {
                let reference = qname.strip_prefix(CFG_PREFIX)?;
                let element = if characteristic_type_set_as_type
                    && reference.starts_with("Characteristic.")
                {
                    "Type"
                } else {
                    "TypeSet"
                };
                (element, qname.as_str())
            }
        }
    };
    let reference = qname.strip_prefix(CFG_PREFIX)?;
    let namespace = declare_current_config_namespace
        .then(|| format!(r#" xmlns:{current_config_prefix}="{CURRENT_CONFIG_URI}""#))
        .unwrap_or_default();
    Some(format!(
        "<v8:{element}{namespace}>{current_config_prefix}:{}</v8:{element}>",
        escape_xml_text(reference)
    ))
}

pub(super) fn extract_ws_definition_xml(inflated: &[u8]) -> Option<Vec<u8>> {
    let xml_start = find_bytes(inflated, b"<?xml")?;
    let xml = &inflated[xml_start..];
    let mut content = Vec::with_capacity(3 + xml.len());
    content.extend_from_slice(b"\xEF\xBB\xBF");
    content.extend_from_slice(xml);
    Some(content)
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn data_composition_xsi_type_is(
    reader: &NsReader<&[u8]>,
    event: &quick_xml::events::BytesStart<'_>,
    expected_namespace: &[u8],
    expected_local: &[u8],
) -> Option<bool> {
    for attribute in event.attributes().with_checks(false) {
        let attribute = attribute.ok()?;
        let (namespace, local) = reader.resolve_attribute(attribute.key);
        if namespace_ref(&namespace) != Some(XSI_NS) || local.as_ref() != b"type" {
            continue;
        }
        let value = attribute.decode_and_unescape_value(reader.decoder()).ok()?;
        let (namespace, local) = reader.resolve(quick_xml::name::QName(value.as_bytes()), false);
        return Some(
            namespace_ref(&namespace) == Some(expected_namespace)
                && local.as_ref() == expected_local,
        );
    }
    Some(false)
}

fn serialized_data_composition_color_ref_uuid(text: &str) -> Option<String> {
    let uuid = text.trim().strip_prefix("0:")?;
    if uuid.len() != 36 {
        return None;
    }
    let canonical = uuid::Uuid::parse_str(uuid).ok()?.hyphenated().to_string();
    canonical.eq_ignore_ascii_case(uuid).then_some(canonical)
}

fn data_composition_style_item_name(
    text: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    let uuid = serialized_data_composition_color_ref_uuid(text)?;
    let reference = object_refs.get(&uuid).or_else(|| {
        let source_uuid = text.trim().strip_prefix("0:")?;
        object_refs.get(source_uuid)
    })?;
    let name = reference.strip_prefix("StyleItem.")?;
    (!name.is_empty()).then(|| name.to_string())
}

fn canonicalize_data_composition_settings_document(
    document: &str,
    object_refs: &BTreeMap<String, String>,
    source_profile: &ProfileId,
    target_profile: &ProfileId,
) -> std::result::Result<String, DcsSettingsCanonicalizeError> {
    let analysis =
        analyze_dcs_settings_document(document).map_err(DcsSettingsCanonicalizeError::Analysis)?;
    let children = analysis.typed();
    // Same five children in the same order the previous single boolean chain
    // tested; only the rejection is now named.
    if let DcsChildParseOutcome::Unsupported(reason) = children.selection_outcome() {
        return Err(DcsSettingsCanonicalizeError::UnsupportedChild {
            child: DcsSettingsChild::Selection,
            reason,
        });
    }
    if let DcsChildParseOutcome::Unsupported(reason) = children.filter() {
        return Err(DcsSettingsCanonicalizeError::UnsupportedChild {
            child: DcsSettingsChild::Filter,
            reason,
        });
    }
    if let DcsChildParseOutcome::Unsupported(reason) = children.order() {
        return Err(DcsSettingsCanonicalizeError::UnsupportedChild {
            child: DcsSettingsChild::Order,
            reason,
        });
    }
    if let DcsChildParseOutcome::Unsupported(reason) = children.conditional_appearance() {
        return Err(DcsSettingsCanonicalizeError::UnsupportedChild {
            child: DcsSettingsChild::ConditionalAppearance,
            reason,
        });
    }
    if let DcsChildParseOutcome::Unsupported(reason) = children.output_parameters() {
        return Err(DcsSettingsCanonicalizeError::UnsupportedChild {
            child: DcsSettingsChild::OutputParameters,
            reason,
        });
    }
    let mut writer = DataCompositionXmlWriter::new(object_refs);
    writer
        .write_document(document, DataCompositionDocumentMode::Settings)
        .ok_or(DcsSettingsCanonicalizeError::Writer)?;
    let parts = emit_canonical_dcs_settings_parts(
        CanonicalDcsSettingsContext::Standalone,
        CanonicalDcsSettingsInput {
            selection: children.selection(),
            filter: match children.filter() {
                DcsChildParseOutcome::Typed(filter) => Some(filter),
                DcsChildParseOutcome::Absent => None,
                DcsChildParseOutcome::Unsupported(_) => unreachable!("checked above"),
            },
            order: match children.order() {
                DcsChildParseOutcome::Typed(order) => Some(order),
                DcsChildParseOutcome::Absent => None,
                DcsChildParseOutcome::Unsupported(_) => unreachable!("checked above"),
            },
            conditional_appearance: match children.conditional_appearance() {
                DcsChildParseOutcome::Typed(value) => Some(value),
                DcsChildParseOutcome::Absent => None,
                DcsChildParseOutcome::Unsupported(_) => unreachable!("checked above"),
            },
            output_parameters: match children.output_parameters() {
                DcsChildParseOutcome::Typed(value) => Some(value),
                DcsChildParseOutcome::Absent => None,
                DcsChildParseOutcome::Unsupported(_) => unreachable!("checked above"),
            },
            items_view_mode: children.items_view_mode(),
            items_user_setting_id: children.items_user_setting_id(),
        },
        source_profile,
        target_profile,
        "dcsset",
        "\t",
        "mssql:dcs/Settings",
    )
    .map_err(DcsSettingsCanonicalizeError::CanonicalChildren)?;
    rewrite_dcs_settings_children(&mut writer.output, &children, &parts)
        .ok_or(DcsSettingsCanonicalizeError::ChildrenRewrite)?;
    let settings = writer
        .output
        .trim_start_matches(['\r', '\n', '\t'])
        .to_string();
    Ok(indent_data_composition_settings(&settings))
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct CanonicalFormServerStateFragment {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) xml: String,
}

pub(crate) fn canonicalize_form_server_state_conditional_appearances(
    document: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<Vec<CanonicalFormServerStateFragment>> {
    let mut reader = NsReader::from_str(document);
    reader.config_mut().trim_text(false);
    let mode = DataCompositionDocumentMode::FormServerStateFragment;
    let mut writer = DataCompositionXmlWriter::new(object_refs);
    let mut capture_depth = 0usize;
    let mut capture_start = None::<usize>;
    let mut captures = Vec::new();
    loop {
        let event_start = usize::try_from(reader.buffer_position()).ok()?;
        let event = reader.read_event().ok()?;
        let event_end = usize::try_from(reader.buffer_position()).ok()?;
        match event {
            Event::Start(event) => {
                let (namespace, local) = reader.resolve_element(event.name());
                if capture_depth == 0
                    && !(namespace_ref(&namespace) == Some(DCS_SETTINGS_NS)
                        && local.as_ref() == b"conditionalAppearance")
                {
                    continue;
                }
                if capture_depth == 0 {
                    capture_start = Some(event_start);
                }
                let written_start = writer.write_start_tag(
                    &reader,
                    &event,
                    namespace_ref(&namespace),
                    local.as_ref(),
                    false,
                    &mode,
                )?;
                writer.element_stack.push(data_composition_element_frame(
                    &reader,
                    &event,
                    namespace_ref(&namespace),
                    local.as_ref(),
                    written_start,
                )?);
                capture_depth = capture_depth.checked_add(1)?;
            }
            Event::Empty(event) => {
                let (namespace, local) = reader.resolve_element(event.name());
                let starts_capture = capture_depth == 0
                    && namespace_ref(&namespace) == Some(DCS_SETTINGS_NS)
                    && local.as_ref() == b"conditionalAppearance";
                if capture_depth > 0 || starts_capture {
                    writer.write_start_tag(
                        &reader,
                        &event,
                        namespace_ref(&namespace),
                        local.as_ref(),
                        true,
                        &mode,
                    )?;
                    if starts_capture {
                        captures.push(CanonicalFormServerStateFragment {
                            start: event_start,
                            end: event_end,
                            xml: std::mem::take(&mut writer.output),
                        });
                    }
                }
            }
            Event::End(event) => {
                if capture_depth == 0 {
                    continue;
                }
                let (namespace, local) = reader.resolve_element(event.name());
                let frame = writer.element_stack.pop()?;
                if frame.namespace.as_deref() != namespace_ref(&namespace)
                    || frame.local.as_slice() != local.as_ref()
                {
                    return None;
                }
                writer.output.push_str("</");
                writer.output.push_str(&frame.rendered_name);
                writer.output.push('>');
                capture_depth = capture_depth.checked_sub(1)?;
                if capture_depth == 0 {
                    captures.push(CanonicalFormServerStateFragment {
                        start: capture_start.take()?,
                        end: event_end,
                        xml: std::mem::take(&mut writer.output),
                    });
                }
            }
            Event::Text(event) if capture_depth > 0 => {
                writer.write_text(&reader, &event, &mode)?;
            }
            Event::CData(event) if capture_depth > 0 => {
                writer.output.push_str("<![CDATA[");
                writer
                    .output
                    .push_str(std::str::from_utf8(event.as_ref()).ok()?);
                writer.output.push_str("]]>");
            }
            Event::Comment(event) if capture_depth > 0 => {
                writer.output.push_str("<!--");
                writer
                    .output
                    .push_str(std::str::from_utf8(event.as_ref()).ok()?);
                writer.output.push_str("-->");
            }
            Event::GeneralRef(event) if capture_depth > 0 => {
                writer.output.push('&');
                writer
                    .output
                    .push_str(std::str::from_utf8(event.as_ref()).ok()?);
                writer.output.push(';');
            }
            Event::Eof => break,
            _ => {}
        }
    }
    (capture_depth == 0 && capture_start.is_none() && writer.element_stack.is_empty())
        .then_some(captures)
}

fn indent_data_composition_settings(settings: &str) -> String {
    let mut indented = String::from("\r\n");
    for line in settings.split_inclusive('\n') {
        indented.push_str("\t\t");
        indented.push_str(line);
    }
    indented
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum DataCompositionDocumentMode {
    Settings,
    FormServerStateFragment,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct DcsDynamicNamespace {
    prefix: String,
    uri: String,
}

#[derive(Debug)]
struct DcsElementFrame {
    namespace: Option<Vec<u8>>,
    local: Vec<u8>,
    rendered_name: String,
    xsi_type_local: Option<String>,
    dynamic_namespaces: Vec<DcsDynamicNamespace>,
    is_data_ui_color_value: bool,
    output_namespace_offset: usize,
}

#[derive(Debug, Clone)]
struct DcsExpandedQName {
    namespace: Option<Vec<u8>>,
    local: String,
}

#[derive(Debug)]
struct DcsRenderedQName {
    value: String,
    declaration: Option<(String, String)>,
}

struct DcsWrittenStart {
    rendered_name: String,
    dynamic_namespaces: Vec<DcsDynamicNamespace>,
    output_namespace_offset: usize,
}

struct DataCompositionXmlWriter<'a> {
    output: String,
    skip_depth: usize,
    element_stack: Vec<DcsElementFrame>,
    object_refs: &'a BTreeMap<String, String>,
}

impl<'a> DataCompositionXmlWriter<'a> {
    fn new(object_refs: &'a BTreeMap<String, String>) -> Self {
        Self {
            output: String::new(),
            skip_depth: 0,
            element_stack: Vec::new(),
            object_refs,
        }
    }

    fn write_document(&mut self, document: &str, mode: DataCompositionDocumentMode) -> Option<()> {
        let mut reader = NsReader::from_str(document);
        reader.config_mut().trim_text(false);
        loop {
            match reader.read_event().ok()? {
                Event::Start(event) => {
                    let (namespace, local) = reader.resolve_element(event.name());
                    let local = local.as_ref();
                    if self.skip_depth == 0 {
                        let written_start = self.write_start_tag(
                            &reader,
                            &event,
                            namespace_ref(&namespace),
                            local,
                            false,
                            &mode,
                        )?;
                        self.element_stack.push(data_composition_element_frame(
                            &reader,
                            &event,
                            namespace_ref(&namespace),
                            local,
                            written_start,
                        )?);
                    }
                }
                Event::Empty(event) => {
                    let (namespace, local) = reader.resolve_element(event.name());
                    if self.skip_depth == 0 {
                        self.write_start_tag(
                            &reader,
                            &event,
                            namespace_ref(&namespace),
                            local.as_ref(),
                            true,
                            &mode,
                        )?;
                    }
                }
                Event::End(event) => {
                    let (namespace, local) = reader.resolve_element(event.name());
                    let local = local.as_ref();
                    let frame = self.element_stack.pop()?;
                    if frame.namespace.as_deref() != namespace_ref(&namespace)
                        || frame.local.as_slice() != local
                    {
                        return None;
                    }
                    self.output.push_str("</");
                    self.output.push_str(&frame.rendered_name);
                    self.output.push('>');
                }
                Event::Text(event) => {
                    if self.skip_depth == 0 {
                        self.write_text(&reader, &event, &mode)?;
                    }
                }
                Event::CData(event) => {
                    if self.skip_depth == 0 {
                        self.output.push_str("<![CDATA[");
                        self.output
                            .push_str(std::str::from_utf8(event.as_ref()).ok()?);
                        self.output.push_str("]]>");
                    }
                }
                Event::Comment(event) => {
                    if self.skip_depth == 0 {
                        self.output.push_str("<!--");
                        self.output
                            .push_str(std::str::from_utf8(event.as_ref()).ok()?);
                        self.output.push_str("-->");
                    }
                }
                Event::GeneralRef(event) => {
                    if self.skip_depth == 0 {
                        self.output.push('&');
                        self.output
                            .push_str(std::str::from_utf8(event.as_ref()).ok()?);
                        self.output.push(';');
                    }
                }
                Event::Decl(_) => {}
                Event::Eof => break,
                _ => {}
            }
        }
        self.element_stack.is_empty().then_some(())
    }

    fn write_start_tag(
        &mut self,
        reader: &NsReader<&[u8]>,
        event: &quick_xml::events::BytesStart<'_>,
        namespace: Option<&[u8]>,
        local: &[u8],
        empty: bool,
        mode: &DataCompositionDocumentMode,
    ) -> Option<DcsWrittenStart> {
        let is_settings_root = matches!(mode, DataCompositionDocumentMode::Settings)
            && namespace == Some(DCS_SETTINGS_NS)
            && local == b"Settings";
        let is_inline_settings_root = local == b"settings"
            && self.element_stack.last().is_some_and(|parent| {
                (namespace == Some(DCS_SETTINGS_NS)
                    && parent.namespace.as_deref() == Some(DCS_SCHEMA_NS)
                    && parent.local.as_slice() == b"settingsVariant")
                    || (namespace == Some(DCS_SCHEMA_NS)
                        && parent.namespace.as_deref() == Some(DCS_SCHEMA_NS)
                        && parent.local.as_slice() == b"nestedSchema")
            });
        let mut rendered_attributes = Vec::<(String, String)>::new();
        let mut dynamic_namespaces = Vec::<DcsDynamicNamespace>::new();
        let is_data_ui_picture_value = namespace == Some(DCS_CORE_NS)
            && local == b"value"
            && data_composition_xsi_type_is(reader, event, DATA_UI_NS, b"Picture")?;
        if namespace == Some(DATA_CORE_NS)
            && matches!(local, b"Type" | b"TypeSet")
            && event_declares_namespace(event, CURRENT_CONFIG_NS)
        {
            self.push_dynamic_namespace(
                &mut dynamic_namespaces,
                self.current_config_prefix(),
                CURRENT_CONFIG_URI.to_string(),
            )?;
        }
        for attribute in event.attributes().with_checks(false) {
            let attribute = attribute.ok()?;
            if is_xmlns_attribute(attribute.key.as_ref()) {
                continue;
            }
            let (attr_namespace, attr_local) = reader.resolve_attribute(attribute.key);
            let rendered_attr_name = self.render_data_composition_node_name(
                attribute.key.as_ref(),
                namespace_ref(&attr_namespace),
                attr_local.as_ref(),
                true,
                *mode,
                &dynamic_namespaces,
            )?;
            if let Some((prefix, uri)) = rendered_attr_name.declaration {
                self.push_dynamic_namespace(&mut dynamic_namespaces, prefix, uri)?;
            }
            let attr_name = rendered_attr_name.value;
            let value = attribute
                .decode_and_unescape_value(reader.decoder())
                .ok()?
                .into_owned();
            let value = if attr_name == "ref" && is_data_ui_picture_value {
                canonical_data_composition_picture_ref(reader, &value).unwrap_or(value)
            } else {
                value
            };
            let rendered = if attr_name == "xsi:type" {
                let rendered = self.render_xsi_type(
                    reader,
                    &value,
                    namespace,
                    local,
                    *mode,
                    &dynamic_namespaces,
                );
                if value.contains(':') {
                    Some(rendered?)
                } else {
                    rendered
                }
            } else {
                None
            };
            let value = if let Some(rendered) = rendered {
                if let Some((prefix, uri)) = rendered.declaration {
                    self.push_dynamic_namespace(&mut dynamic_namespaces, prefix, uri)?;
                }
                rendered.value
            } else {
                canonical_data_composition_attr_value(&attr_name, &value, namespace)
            };
            rendered_attributes.push((attr_name, value));
        }
        let name = if is_settings_root {
            "dcsset:settings".to_string()
        } else {
            let rendered_name = self.render_data_composition_node_name(
                event.name().as_ref(),
                namespace,
                local,
                false,
                *mode,
                &dynamic_namespaces,
            )?;
            if let Some((prefix, uri)) = rendered_name.declaration {
                self.push_dynamic_namespace(&mut dynamic_namespaces, prefix, uri)?;
            }
            rendered_name.value
        };
        self.output.push('<');
        self.output.push_str(&name);
        let output_namespace_offset = self.output.len();
        if is_settings_root || is_inline_settings_root {
            self.output.push_str(SETTINGS_ROOT_UI_NAMESPACES);
        }
        for namespace in &dynamic_namespaces {
            self.output.push_str(" xmlns:");
            self.output.push_str(&namespace.prefix);
            self.output.push_str("=\"");
            self.output.push_str(&namespace.uri);
            self.output.push('"');
        }
        for (attr_name, value) in rendered_attributes {
            self.output.push(' ');
            self.output.push_str(&attr_name);
            self.output.push_str("=\"");
            self.output.push_str(&escape_xml_text(&value));
            self.output.push('"');
        }
        if empty {
            self.output.push_str("/>");
        } else {
            self.output.push('>');
        }
        Some(DcsWrittenStart {
            rendered_name: name,
            dynamic_namespaces,
            output_namespace_offset,
        })
    }

    fn write_text(
        &mut self,
        reader: &NsReader<&[u8]>,
        event: &quick_xml::events::BytesText<'_>,
        mode: &DataCompositionDocumentMode,
    ) -> Option<()> {
        let text = std::str::from_utf8(event.as_ref()).ok()?;
        let is_qname_text = self.element_stack.last().is_some_and(|frame| {
            frame.namespace.as_deref() == Some(DATA_CORE_NS)
                && matches!(frame.local.as_slice(), b"Type" | b"TypeSet")
        });
        if is_qname_text {
            let value = text.trim();
            if !value.is_empty()
                && let Some(rendered) = self.render_lexical_qname(
                    reader,
                    value,
                    Some(DATA_CORE_NS),
                    self.element_stack.last()?.local.as_slice(),
                )
            {
                if let Some((prefix, _)) = &rendered.declaration
                    && !self
                        .element_stack
                        .last()?
                        .dynamic_namespaces
                        .iter()
                        .any(|namespace| &namespace.prefix == prefix)
                {
                    return None;
                }
                let value_start = text.find(value)?;
                self.output.push_str(&text[..value_start]);
                self.output.push_str(&escape_xml_text(&rendered.value));
                self.output.push_str(&text[value_start + value.len()..]);
                return Some(());
            }
        }
        if self
            .element_stack
            .last()
            .is_some_and(|frame| frame.is_data_ui_color_value)
        {
            let value = text.trim();
            let value_start = text.find(value)?;
            let in_area_template = self.element_stack.iter().any(|frame| {
                frame.namespace.as_deref() == Some(DCS_AREA_TEMPLATE_NS)
                    && frame.local.as_slice() == b"appearance"
            });
            let in_schema_appearance = self.element_stack.iter().any(|frame| {
                frame.namespace.as_deref() == Some(DCS_SCHEMA_NS)
                    && frame.local.as_slice() == b"appearance"
            });
            let resolved_style_name = data_composition_style_item_name(text, self.object_refs);
            let qualified_value = value
                .contains(':')
                .then(|| resolve_data_composition_qname(reader, value))
                .flatten()
                .and_then(|expanded| {
                    let namespace = expanded.namespace?;
                    Some((namespace, expanded.local))
                });
            let qualified_schema_style = in_schema_appearance
                && qualified_value
                    .as_ref()
                    .is_some_and(|(namespace, _)| namespace.as_slice() == STYLE_NS);
            let qualified_canonical_value = (in_area_template || qualified_schema_style)
                .then_some(qualified_value.clone())
                .flatten();
            let qualified_form_value = (*mode
                == DataCompositionDocumentMode::FormServerStateFragment)
                .then_some(qualified_value)
                .flatten()
                .filter(|(namespace, _)| canonical_form_data_ui_value_prefix(namespace).is_some());
            if let Some((namespace, local)) = resolved_style_name
                .map(|name| (STYLE_NS.to_vec(), name))
                .or(qualified_canonical_value)
                .or(qualified_form_value)
            {
                let prefix = if in_area_template {
                    Some(self.scope_prefix(8))
                } else if qualified_schema_style {
                    Some(self.schema_style_prefix())
                } else if *mode == DataCompositionDocumentMode::FormServerStateFragment {
                    canonical_form_data_ui_value_prefix(&namespace).map(str::to_string)
                } else {
                    None
                };
                if let Some(prefix) = &prefix
                    && *mode != DataCompositionDocumentMode::FormServerStateFragment
                {
                    let output_namespace_offset =
                        self.element_stack.last()?.output_namespace_offset;
                    let namespace = std::str::from_utf8(&namespace).ok()?;
                    self.output.insert_str(
                        output_namespace_offset,
                        &format!(" xmlns:{prefix}=\"{}\"", escape_xml_text(namespace)),
                    );
                }
                self.output.push_str(&text[..value_start]);
                self.output.push_str(prefix.as_deref().unwrap_or("style"));
                self.output.push(':');
                self.output.push_str(&escape_xml_text(&local));
                self.output.push_str(&text[value_start + value.len()..]);
                return Some(());
            }
        }
        self.output.push_str(text);
        Some(())
    }

    fn render_data_composition_node_name(
        &self,
        lexical_name: &[u8],
        namespace: Option<&[u8]>,
        local: &[u8],
        is_attribute: bool,
        mode: DataCompositionDocumentMode,
        local_namespaces: &[DcsDynamicNamespace],
    ) -> Option<DcsRenderedQName> {
        if namespace.is_none() && lexical_name.contains(&b':') {
            return None;
        }
        let canonical = if is_attribute {
            canonical_data_composition_attr_name(namespace, local)
        } else {
            canonical_data_composition_name(namespace, local)
        };
        if let Some(value) = canonical {
            return Some(DcsRenderedQName {
                value,
                declaration: None,
            });
        }
        self.render_dynamic_qname(
            lexical_name,
            DcsExpandedQName {
                namespace: Some(namespace?.to_vec()),
                local: std::str::from_utf8(local).ok()?.to_string(),
            },
            mode,
            local_namespaces,
        )
    }

    fn render_dynamic_qname(
        &self,
        lexical_name: &[u8],
        expanded: DcsExpandedQName,
        mode: DataCompositionDocumentMode,
        local_namespaces: &[DcsDynamicNamespace],
    ) -> Option<DcsRenderedQName> {
        let uri = std::str::from_utf8(expanded.namespace.as_deref()?).ok()?;
        if let Some(prefix) = self.output_prefix_for_namespace(uri, local_namespaces) {
            return Some(DcsRenderedQName {
                value: format!("{prefix}:{}", expanded.local),
                declaration: None,
            });
        }
        let lexical_name = std::str::from_utf8(lexical_name).ok()?;
        let (input_prefix, lexical_local) = lexical_name.split_once(':')?;
        if lexical_local != expanded.local {
            return None;
        }
        let output_prefix = data_composition_output_scope_prefix(input_prefix, mode)?;
        if let Some(existing_uri) =
            self.dynamic_namespace_uri_for_prefix(&output_prefix, local_namespaces)
        {
            if existing_uri != uri {
                return None;
            }
            return Some(DcsRenderedQName {
                value: format!("{output_prefix}:{}", expanded.local),
                declaration: None,
            });
        }
        Some(DcsRenderedQName {
            value: format!("{output_prefix}:{}", expanded.local),
            declaration: Some((output_prefix, uri.to_string())),
        })
    }

    fn output_prefix_for_namespace(
        &self,
        uri: &str,
        local_namespaces: &[DcsDynamicNamespace],
    ) -> Option<String> {
        local_namespaces
            .iter()
            .rev()
            .chain(
                self.element_stack
                    .iter()
                    .rev()
                    .flat_map(|frame| frame.dynamic_namespaces.iter().rev()),
            )
            .find(|namespace| namespace.uri == uri)
            .map(|namespace| namespace.prefix.clone())
            .or_else(|| {
                globally_declared_data_composition_prefix(uri.as_bytes()).map(str::to_string)
            })
    }

    fn dynamic_namespace_uri_for_prefix<'b>(
        &'b self,
        prefix: &str,
        local_namespaces: &'b [DcsDynamicNamespace],
    ) -> Option<&'b str> {
        local_namespaces
            .iter()
            .rev()
            .chain(
                self.element_stack
                    .iter()
                    .rev()
                    .flat_map(|frame| frame.dynamic_namespaces.iter().rev()),
            )
            .find(|namespace| namespace.prefix == prefix)
            .map(|namespace| namespace.uri.as_str())
    }

    fn push_dynamic_namespace(
        &self,
        namespaces: &mut Vec<DcsDynamicNamespace>,
        prefix: String,
        uri: String,
    ) -> Option<()> {
        if reserved_data_composition_namespace_uri(&prefix)
            .is_some_and(|reserved_uri| reserved_uri.as_bytes() != uri.as_bytes())
        {
            return None;
        }
        if self
            .element_stack
            .iter()
            .rev()
            .flat_map(|frame| frame.dynamic_namespaces.iter().rev())
            .find(|namespace| namespace.prefix == prefix)
            .is_some_and(|namespace| namespace.uri != uri)
        {
            return None;
        }
        if let Some(existing) = namespaces
            .iter()
            .find(|namespace| namespace.prefix == prefix)
        {
            return (existing.uri == uri).then_some(());
        }
        namespaces.push(DcsDynamicNamespace { prefix, uri });
        Some(())
    }

    fn render_lexical_qname(
        &self,
        reader: &NsReader<&[u8]>,
        value: &str,
        element_namespace: Option<&[u8]>,
        element_local: &[u8],
    ) -> Option<DcsRenderedQName> {
        let mut expanded = resolve_data_composition_qname(reader, value)?;
        if !value.contains(':')
            && matches!(
                element_namespace,
                Some(DCS_CORE_NS | DCS_SETTINGS_NS | DATA_CORE_NS)
            )
        {
            expanded.namespace = element_namespace.map(<[u8]>::to_vec);
        }
        self.render_expanded_qname(expanded, element_local)
    }

    fn render_xsi_type(
        &self,
        reader: &NsReader<&[u8]>,
        value: &str,
        element_namespace: Option<&[u8]>,
        element_local: &[u8],
        mode: DataCompositionDocumentMode,
        local_namespaces: &[DcsDynamicNamespace],
    ) -> Option<DcsRenderedQName> {
        if value.contains(':') {
            let expanded = resolve_data_composition_qname(reader, value)?;
            if let Some(rendered) = self.render_expanded_qname(expanded.clone(), element_local) {
                return Some(rendered);
            }
            return self.render_dynamic_qname(value.as_bytes(), expanded, mode, local_namespaces);
        }
        let namespace = if element_namespace == Some(DCS_AREA_TEMPLATE_NS) {
            Some(DCS_AREA_TEMPLATE_NS)
        } else if is_data_core_xsi_type(value) {
            Some(DATA_CORE_NS)
        } else if value == "Field" {
            Some(DCS_CORE_NS)
        } else if is_dcs_settings_xsi_type(value) {
            Some(DCS_SETTINGS_NS)
        } else if matches!(element_namespace, Some(DCS_CORE_NS | DCS_SETTINGS_NS)) {
            element_namespace
        } else {
            return self.render_lexical_qname(reader, value, element_namespace, element_local);
        };
        self.render_expanded_qname(
            DcsExpandedQName {
                namespace: namespace.map(<[u8]>::to_vec),
                local: value.to_string(),
            },
            element_local,
        )
    }

    fn render_expanded_qname(
        &self,
        expanded: DcsExpandedQName,
        element_local: &[u8],
    ) -> Option<DcsRenderedQName> {
        let namespace = expanded.namespace.as_deref();
        if namespace == Some(DCS_AREA_TEMPLATE_NS) {
            let prefix = "dcsat".to_string();
            let declaration = (!self.element_stack.iter().any(|frame| {
                frame
                    .dynamic_namespaces
                    .iter()
                    .any(|namespace| namespace.prefix == prefix)
            }))
            .then(|| (prefix.clone(), DCS_AREA_TEMPLATE_URI.to_string()));
            return Some(DcsRenderedQName {
                value: format!("{prefix}:{}", expanded.local),
                declaration,
            });
        }
        let fixed_prefix = match namespace {
            None | Some(DCS_SCHEMA_NS) => Some(None),
            Some(DCS_COMMON_NS) => Some(Some("dcscom")),
            Some(DCS_CORE_NS) => Some(Some("dcscor")),
            Some(DCS_SETTINGS_NS) => Some(Some("dcsset")),
            Some(DATA_CORE_NS) => Some(Some("v8")),
            Some(DATA_UI_NS) => Some(Some("v8ui")),
            Some(STYLE_NS) => Some(Some("style")),
            Some(SYS_NS) => Some(Some("sys")),
            Some(WEB_NS) => Some(Some("web")),
            Some(WIN_NS) => Some(Some("win")),
            Some(XSI_NS) => Some(Some("xsi")),
            Some(XS_NS) => Some(Some("xs")),
            _ => None,
        };
        if let Some(prefix) = fixed_prefix {
            let value = prefix
                .map(|prefix| format!("{prefix}:{}", expanded.local))
                .unwrap_or(expanded.local);
            return Some(DcsRenderedQName {
                value,
                declaration: None,
            });
        }
        let (prefix, uri) = match namespace {
            Some(CURRENT_CONFIG_NS) => {
                (self.current_config_prefix(), CURRENT_CONFIG_URI.to_string())
            }
            Some(ENTERPRISE_NS) => (
                self.enterprise_prefix(element_local),
                ENTERPRISE_URI.to_string(),
            ),
            _ => return None,
        };
        Some(DcsRenderedQName {
            value: format!("{prefix}:{}", expanded.local),
            declaration: Some((prefix, uri)),
        })
    }

    fn current_config_prefix(&self) -> String {
        let base = if self.has_parameter_ancestor() {
            4
        } else if self.has_data_set_item_field_ancestor() {
            6
        } else if self.element_stack.iter().any(|frame| {
            frame.local.as_slice() == b"item"
                && frame.xsi_type_local.as_deref() == Some("DataSetObject")
        }) {
            6
        } else {
            5
        };
        self.scope_prefix(base)
    }

    fn schema_style_prefix(&self) -> String {
        let base = if self.has_parameter_ancestor() {
            5
        } else if self.has_data_set_item_field_ancestor()
            || self.element_stack.iter().any(|frame| {
                frame.local.as_slice() == b"item"
                    && frame.xsi_type_local.as_deref() == Some("DataSetObject")
            })
        {
            7
        } else {
            6
        };
        self.scope_prefix(base)
    }

    fn enterprise_prefix(&self, element_local: &[u8]) -> String {
        let base = if element_local != b"mode" {
            5
        } else if self.has_parameter_ancestor() {
            7
        } else {
            8
        };
        self.scope_prefix(base)
    }

    fn has_parameter_ancestor(&self) -> bool {
        self.element_stack.iter().any(|frame| {
            frame.namespace.as_deref() == Some(DCS_SCHEMA_NS)
                && matches!(frame.local.as_slice(), b"parameter" | b"calculatedField")
        })
    }

    fn has_data_set_item_field_ancestor(&self) -> bool {
        self.element_stack.windows(3).any(|frames| {
            frames
                .iter()
                .all(|frame| frame.namespace.as_deref() == Some(DCS_SCHEMA_NS))
                && frames[0].local.as_slice() == b"dataSet"
                && frames[1].local.as_slice() == b"item"
                && frames[2].local.as_slice() == b"field"
        })
    }

    fn scope_prefix(&self, base: usize) -> String {
        let nested_schema_depth = self
            .element_stack
            .iter()
            .filter(|frame| {
                frame.namespace.as_deref() == Some(DCS_SCHEMA_NS)
                    && frame.local.as_slice() == b"nestedSchema"
            })
            .count();
        format!("d{}p1", base + 2 * nested_schema_depth)
    }
}

fn data_composition_element_frame(
    reader: &NsReader<&[u8]>,
    event: &quick_xml::events::BytesStart<'_>,
    namespace: Option<&[u8]>,
    local: &[u8],
    written_start: DcsWrittenStart,
) -> Option<DcsElementFrame> {
    let mut xsi_type_local = None;
    let mut is_data_ui_color_value = false;
    for attribute in event.attributes().with_checks(false) {
        let attribute = attribute.ok()?;
        let (attr_namespace, attr_local) = reader.resolve_attribute(attribute.key);
        if namespace_ref(&attr_namespace) == Some(XSI_NS) && attr_local.as_ref() == b"type" {
            let value = attribute.decode_and_unescape_value(reader.decoder()).ok()?;
            let expanded = resolve_data_composition_qname(reader, &value)?;
            is_data_ui_color_value = namespace == Some(DCS_CORE_NS)
                && local == b"value"
                && expanded.namespace.as_deref() == Some(DATA_UI_NS)
                && expanded.local == "Color";
            xsi_type_local = Some(
                value
                    .rsplit_once(':')
                    .map(|(_, local)| local)
                    .unwrap_or(value.as_ref())
                    .to_string(),
            );
            break;
        }
    }
    Some(DcsElementFrame {
        namespace: namespace.map(<[u8]>::to_vec),
        local: local.to_vec(),
        rendered_name: written_start.rendered_name,
        xsi_type_local,
        dynamic_namespaces: written_start.dynamic_namespaces,
        is_data_ui_color_value,
        output_namespace_offset: written_start.output_namespace_offset,
    })
}

fn resolve_data_composition_qname(
    reader: &NsReader<&[u8]>,
    value: &str,
) -> Option<DcsExpandedQName> {
    let (namespace, local) = reader.resolve(quick_xml::name::QName(value.as_bytes()), false);
    let namespace = match namespace {
        ResolveResult::Bound(namespace) => Some(namespace.0.to_vec()),
        ResolveResult::Unbound if value.contains(':') => return None,
        ResolveResult::Unbound => None,
        ResolveResult::Unknown(_) => return None,
    };
    Some(DcsExpandedQName {
        namespace,
        local: std::str::from_utf8(local.as_ref()).ok()?.to_string(),
    })
}

fn event_declares_namespace(event: &quick_xml::events::BytesStart<'_>, namespace: &[u8]) -> bool {
    event
        .attributes()
        .with_checks(false)
        .flatten()
        .any(|attribute| {
            is_xmlns_attribute(attribute.key.as_ref()) && attribute.value.as_ref() == namespace
        })
}

fn data_composition_output_scope_prefix(
    input_prefix: &str,
    mode: DataCompositionDocumentMode,
) -> Option<String> {
    if mode == DataCompositionDocumentMode::Settings
        && let Some(number) = input_prefix
            .strip_prefix('d')
            .and_then(|value| value.strip_suffix("p1"))
            .filter(|value| !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()))
        && let Ok(number) = number.parse::<usize>()
        && input_prefix == format!("d{number}p1")
    {
        return Some(format!("d{}p1", number.checked_add(2)?));
    }
    Some(input_prefix.to_string())
}

fn globally_declared_data_composition_prefix(namespace: &[u8]) -> Option<&'static str> {
    match namespace {
        DCS_COMMON_NS => Some("dcscom"),
        DCS_CORE_NS => Some("dcscor"),
        DCS_SETTINGS_NS => Some("dcsset"),
        DATA_CORE_NS => Some("v8"),
        DATA_UI_NS => Some("v8ui"),
        XSI_NS => Some("xsi"),
        XS_NS => Some("xs"),
        _ => None,
    }
}

fn canonical_form_data_ui_value_prefix(namespace: &[u8]) -> Option<&'static str> {
    match namespace {
        DATA_UI_NS => Some("v8ui"),
        STYLE_NS => Some("style"),
        SYS_NS => Some("sys"),
        WEB_NS => Some("web"),
        WIN_NS => Some("win"),
        _ => None,
    }
}

fn reserved_data_composition_namespace_uri(prefix: &str) -> Option<&'static str> {
    let namespace = match prefix {
        "dcscom" => DCS_COMMON_NS,
        "dcscor" => DCS_CORE_NS,
        "dcsset" => DCS_SETTINGS_NS,
        "dcsat" => DCS_AREA_TEMPLATE_NS,
        "v8" => DATA_CORE_NS,
        "v8ui" => DATA_UI_NS,
        "style" => STYLE_NS,
        "sys" => SYS_NS,
        "web" => WEB_NS,
        "win" => WIN_NS,
        "xsi" => XSI_NS,
        "xs" => XS_NS,
        _ => return None,
    };
    std::str::from_utf8(namespace).ok()
}

fn namespace_ref<'a>(namespace: &'a ResolveResult<'a>) -> Option<&'a [u8]> {
    match namespace {
        ResolveResult::Bound(namespace) => Some(namespace.0),
        _ => None,
    }
}

fn is_xmlns_attribute(name: &[u8]) -> bool {
    name == b"xmlns" || name.starts_with(b"xmlns:")
}

fn canonical_data_composition_name(namespace: Option<&[u8]>, local: &[u8]) -> Option<String> {
    let local = std::str::from_utf8(local).ok()?;
    match namespace {
        Some(DCS_SCHEMA_NS) => Some(local.to_string()),
        Some(DCS_COMMON_NS) => Some(format!("dcscom:{local}")),
        Some(DCS_CORE_NS) => Some(format!("dcscor:{local}")),
        Some(DCS_SETTINGS_NS) => Some(format!("dcsset:{local}")),
        Some(DCS_AREA_TEMPLATE_NS) => Some(format!("dcsat:{local}")),
        Some(DATA_CORE_NS) => Some(format!("v8:{local}")),
        Some(DATA_UI_NS) => Some(format!("v8ui:{local}")),
        Some(STYLE_NS) => Some(format!("style:{local}")),
        Some(SYS_NS) => Some(format!("sys:{local}")),
        Some(WEB_NS) => Some(format!("web:{local}")),
        Some(WIN_NS) => Some(format!("win:{local}")),
        Some(XSI_NS) => Some(format!("xsi:{local}")),
        Some(XS_NS) => Some(format!("xs:{local}")),
        Some(_) => None,
        None => Some(local.to_string()),
    }
}

fn canonical_data_composition_attr_name(namespace: Option<&[u8]>, local: &[u8]) -> Option<String> {
    let local = std::str::from_utf8(local).ok()?;
    match namespace {
        Some(XSI_NS) => Some(format!("xsi:{local}")),
        Some(XS_NS) => Some(format!("xs:{local}")),
        Some(DATA_CORE_NS) => Some(format!("v8:{local}")),
        Some(DATA_UI_NS) => Some(format!("v8ui:{local}")),
        Some(DCS_CORE_NS) => Some(format!("dcscor:{local}")),
        Some(DCS_SETTINGS_NS) => Some(format!("dcsset:{local}")),
        Some(DCS_COMMON_NS) => Some(format!("dcscom:{local}")),
        Some(DCS_AREA_TEMPLATE_NS) => Some(format!("dcsat:{local}")),
        Some(_) => None,
        None => Some(local.to_string()),
    }
}

fn canonical_data_composition_attr_value(
    attr_name: &str,
    value: &str,
    element_namespace: Option<&[u8]>,
) -> String {
    if attr_name != "xsi:type" {
        return value.to_string();
    }
    let suffix = value
        .rsplit_once(':')
        .map(|(_, suffix)| suffix)
        .unwrap_or(value);
    match suffix {
        "LocalStringType" => "v8:LocalStringType".to_string(),
        "Field" => "dcscor:Field".to_string(),
        _ if is_data_core_xsi_type(suffix) => format!("v8:{suffix}"),
        _ if is_dcs_settings_xsi_type(suffix) => format!("dcsset:{suffix}"),
        _ if element_namespace == Some(DCS_CORE_NS) && !value.contains(':') => {
            format!("dcscor:{value}")
        }
        _ if element_namespace == Some(DCS_SETTINGS_NS) && !value.contains(':') => {
            format!("dcsset:{value}")
        }
        _ if element_namespace == Some(DCS_AREA_TEMPLATE_NS) && !value.contains(':') => {
            format!("dcsat:{value}")
        }
        _ => value.to_string(),
    }
}

fn canonical_data_composition_picture_ref(reader: &NsReader<&[u8]>, value: &str) -> Option<String> {
    if !value.contains(':') {
        return None;
    }
    let expanded = resolve_data_composition_qname(reader, value)?;
    (expanded.namespace.as_deref() == Some(DATA_UI_NS) && !expanded.local.is_empty())
        .then(|| format!("v8ui:{}", expanded.local))
}

fn is_data_core_xsi_type(value: &str) -> bool {
    matches!(value, "StandardPeriod" | "StandardPeriodVariant")
}

fn is_dcs_settings_xsi_type(value: &str) -> bool {
    matches!(
        value,
        "DataCompositionAttributesPlacement"
            | "DataCompositionChartLegendPlacement"
            | "DataCompositionFixation"
            | "DataCompositionGroupFieldsPlacement"
            | "DataCompositionGroupPlacement"
            | "DataCompositionGroupTemplateType"
            | "DataCompositionGroupUseVariant"
            | "DataCompositionPictureOutputType"
            | "DataCompositionResourcesAutoPosition"
            | "DataCompositionResourcesPlacement"
            | "DataCompositionTextOutputType"
            | "FilterItemComparison"
            | "FilterItemGroup"
            | "GroupItemAuto"
            | "GroupItemField"
            | "OrderItemAuto"
            | "OrderItemField"
            | "SelectedItemAuto"
            | "SelectedItemField"
            | "SelectedItemFolder"
            | "SettingsParameterValue"
            | "StructureItemChart"
            | "StructureItemGroup"
            | "StructureItemNestedObject"
            | "StructureItemTable"
            | "UserFieldCase"
            | "UserFieldExpression"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use ibcmd_xml::parse_dcs_settings_children;
    use sha2::{Digest, Sha256};

    fn decode_base64_fixture(encoded: &str) -> Vec<u8> {
        let mut output = Vec::new();
        let mut quartet = [0u8; 4];
        let mut length = 0usize;
        for byte in encoded.bytes().filter(|byte| !byte.is_ascii_whitespace()) {
            quartet[length] = match byte {
                b'A'..=b'Z' => byte - b'A',
                b'a'..=b'z' => byte - b'a' + 26,
                b'0'..=b'9' => byte - b'0' + 52,
                b'+' => 62,
                b'/' => 63,
                b'=' => 64,
                _ => panic!("invalid fixture base64 byte {byte}"),
            };
            length += 1;
            if length == 4 {
                output.push((quartet[0] << 2) | (quartet[1] >> 4));
                if quartet[2] != 64 {
                    output.push((quartet[1] << 4) | (quartet[2] >> 2));
                }
                if quartet[3] != 64 {
                    output.push((quartet[2] << 6) | quartet[3]);
                }
                length = 0;
            }
        }
        assert_eq!(length, 0, "fixture base64 must contain complete quartets");
        output
    }

    /// Every normalize step must name itself with a code that is valid in the
    /// shared `ibcmd-core` diagnostic vocabulary, and no two steps may share a
    /// code -- the whole point of the typed reason is that a `cf export`
    /// failure ledger can be grouped by step.
    #[test]
    fn every_normalize_step_carries_a_distinct_valid_diagnostic_code() {
        let inner = |reason: &str| DcsInnerSchemaError::Malformed(reason.to_owned());
        let errors = vec![
            DcsTemplateNormalizeError::EnvelopeAnalysis(DcsSchemaTemplateError::Malformed(
                "probe".to_owned(),
            )),
            DcsTemplateNormalizeError::SettingsDocumentNotUtf8 { index: 0 },
            DcsTemplateNormalizeError::SettingsFragmentParse {
                index: 0,
                cause: inner("probe"),
            },
            DcsTemplateNormalizeError::PrimarySchemaParse {
                inner_schema: inner("probe"),
                query_union_link: inner("probe"),
            },
            DcsTemplateNormalizeError::InnerSchemaEmit(inner("probe")),
            DcsTemplateNormalizeError::QueryUnionLinkEmit(inner("probe")),
            DcsTemplateNormalizeError::AreaTemplateEmit(inner("probe")),
            DcsTemplateNormalizeError::AreaTemplateAnchorMissing,
            DcsTemplateNormalizeError::AreaTemplateSizeOverflow,
            DcsTemplateNormalizeError::SettingsCanonicalize {
                index: 0,
                cause: DcsSettingsCanonicalizeError::Analysis(
                    DcsSettingsDocumentAnalysisError::UnsupportedSource {
                        reason: "probe",
                        direct_ordinal: None,
                    },
                ),
            },
            DcsTemplateNormalizeError::SettingsCanonicalize {
                index: 0,
                cause: DcsSettingsCanonicalizeError::Writer,
            },
            DcsTemplateNormalizeError::SettingsCanonicalize {
                index: 0,
                cause: DcsSettingsCanonicalizeError::CanonicalChildren(
                    CanonicalDcsSettingsAdapterError::Provenance("probe".to_owned()),
                ),
            },
            DcsTemplateNormalizeError::SettingsCanonicalize {
                index: 0,
                cause: DcsSettingsCanonicalizeError::ChildrenRewrite,
            },
        ];
        let children = [
            DcsSettingsChild::Selection,
            DcsSettingsChild::Filter,
            DcsSettingsChild::Order,
            DcsSettingsChild::ConditionalAppearance,
            DcsSettingsChild::OutputParameters,
        ];
        let errors = errors
            .into_iter()
            .chain(children.into_iter().map(|child| {
                DcsTemplateNormalizeError::SettingsCanonicalize {
                    index: 0,
                    cause: DcsSettingsCanonicalizeError::UnsupportedChild {
                        child,
                        reason: "probe",
                    },
                }
            }))
            .collect::<Vec<_>>();

        let mut codes = BTreeSet::new();
        for error in &errors {
            let code = error.code();
            assert_eq!(
                error.diagnostic_code().as_str(),
                code,
                "step code must round-trip through the shared vocabulary: {code}"
            );
            assert!(codes.insert(code), "duplicate normalize step code: {code}");
            assert_eq!(error.severity(), Severity::Error, "{code} must fail closed");
            let rendered = error.to_string();
            assert!(
                rendered.starts_with(&format!("[error {code} {}] ", error.class().as_str())),
                "rendered reason must lead with severity, code and class: {rendered}"
            );
        }
        assert_eq!(codes.len(), errors.len());
    }

    /// A settings child whose shape has no ownership rule must be reported
    /// against the exact child, not as an anonymous normalization failure.
    #[test]
    fn unsupported_settings_child_names_the_child() {
        let settings = concat!(
            "<Settings xmlns=\"http://v8.1c.ru/8.1/data-composition-system/settings\"",
            " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">",
            "<order><probe/></order>",
            "</Settings>"
        );
        let error = canonicalize_data_composition_settings_document(
            settings,
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .expect_err("an unparsable order child must fail closed");
        assert!(
            matches!(
                error,
                DcsSettingsCanonicalizeError::UnsupportedChild {
                    child: DcsSettingsChild::Order,
                    ..
                }
            ),
            "expected an unsupported `order` child, got: {error}"
        );
    }

    #[test]
    fn standalone_settings_tail_uses_the_shared_canonical_scalar_path() {
        let settings = concat!(
            "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>",
            "<Settings xmlns=\"http://v8.1c.ru/8.1/data-composition-system/settings\">",
            "<itemsUserSettingID>id&lt;&amp;</itemsUserSettingID>",
            "<itemsViewMode>Compact</itemsViewMode>",
            "</Settings>"
        );
        let children = parse_dcs_settings_children(settings).unwrap();
        assert_eq!(children.items_view_mode(), Some("Compact"));
        assert_eq!(children.items_user_setting_id(), Some("id<&"));
        let source_profile = ProfileId::parse("provider:mssql-legacy").unwrap();
        let target_profile = ProfileId::parse("xml-2.20").unwrap();
        let canonical_settings = canonicalize_data_composition_settings_document(
            settings,
            &BTreeMap::new(),
            &source_profile,
            &target_profile,
        )
        .unwrap();
        assert!(canonical_settings.contains("<dcsset:itemsViewMode>Compact"));

        let view = canonical_settings
            .find("<dcsset:itemsViewMode>Compact</dcsset:itemsViewMode>")
            .unwrap();
        let id = canonical_settings
            .find("<dcsset:itemsUserSettingID>id&lt;&amp;</dcsset:itemsUserSettingID>")
            .unwrap();
        assert!(view < id);
        assert_eq!(
            canonical_settings.matches("<dcsset:itemsViewMode>").count(),
            1
        );
        assert_eq!(
            canonical_settings
                .matches("<dcsset:itemsUserSettingID>")
                .count(),
            1
        );
    }

    #[test]
    fn platform_8_3_27_xml_2_20_dcs_body_exports_byte_exact() {
        let packed = include_bytes!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/dcs-core/raw/\
             f4db0f6c-34f4-4449-995d-6265516e5fa8.0.deflate"
        );
        let expected = include_bytes!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/dcs-core/native/Reports/\
             DcsCorpus/Templates/MainSchema/Ext/Template.xml"
        );

        let body = crate::compiler::bodies::dcs::decode_compatible_dcs(
            crate::compiler::bodies::dcs::DcsTemplateKind::Schema,
            packed,
        )
        .expect("platform-attested DCS body must decode");
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .expect("platform-attested DCS body must be exportable through the live codec");

        assert_eq!(actual, expected);
    }

    #[test]
    fn platform_data_parameters_source_owned_body_exports_byte_exact() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-data-parameters-source-owned/raw/",
            "f4db0f6c-34f4-4449-995d-6265516e5fa8.0.deflate.b64"
        )));
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-data-parameters-source-owned/native-template.xml.b64"
        )));

        let body = crate::compiler::bodies::dcs::decode_compatible_dcs(
            crate::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .expect("platform-attested source-owned DCS body must decode");
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .expect(
            "platform-attested source-owned DCS body must be exportable through the live codec",
        );

        assert_eq!(actual, expected);
    }

    #[test]
    fn platform_multi_variant_envelope_materializes_settings_positionally_byte_exact() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-multi-variant-envelope/raw/",
            "f4db0f6c-34f4-4449-995d-6265516e5fa8.0.deflate.b64"
        )));
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-multi-variant-envelope/native-template.xml.b64"
        )));

        let body = crate::compiler::bodies::dcs::decode_compatible_dcs(
            crate::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .expect("platform-attested multi-variant body must decode");
        let documents = body.documents();
        let source_profile = ProfileId::parse("provider:mssql-legacy").unwrap();
        let target_profile = ProfileId::parse("xml-2.20").unwrap();
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &documents,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &source_profile,
            &target_profile,
        )
        .expect("platform-attested multi-variant DCS body must be exportable");

        assert_eq!(actual, expected);
    }

    #[test]
    fn platform_type_id_reference_body_exports_byte_exact_through_common_codec() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-typeid-reference/raw-packed.bin.b64"
        )));
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-typeid-reference/native-template.xml.b64"
        )));
        let mut type_index = BTreeMap::new();
        type_index.insert(
            "488c0ffa-ef24-480c-a420-3bd2736317f9".to_owned(),
            DcsTypeResolution::Type {
                qname: "cfg:CatalogRef.FilterProbe".to_owned(),
            },
        );
        let body = crate::compiler::bodies::dcs::decode_compatible_dcs(
            crate::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .unwrap();
        let documents = body.documents();
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &documents,
            &type_index,
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn platform_query_union_link_body_exports_byte_exact_through_common_codec() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-query-union-link/raw-packed.bin.b64"
        )));
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-query-union-link/native-template.xml.b64"
        )));
        let body = crate::compiler::bodies::dcs::decode_compatible_dcs(
            crate::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .unwrap();
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .unwrap();
        assert_eq!(actual, expected);
    }

    /// Gate test for the query-union-link fallback's `reference_types.is_empty()`
    /// bug (DCS-LEGACY-REMOVAL-01 phase-1 finding, candidate a): the inner-schema
    /// typed parser rejects any DataSetQuery/DataSetUnion/dataSetLink shape (it
    /// only admits DataSetObject), and this fixture's own type_index -- the
    /// same evidenced `CatalogRef.FilterProbe` construction
    /// `platform_type_id_reference_body_exports_byte_exact_through_common_codec`
    /// already proves, built the same way the real route's
    /// `build_metadata_type_indexes_from_texts` would produce it -- is
    /// non-empty. Before the fix this combination (inner-schema parse
    /// fails + `reference_types` non-empty) skipped the query-union-link
    /// fallback entirely and hard failed; the fix removes the accidental
    /// gate so the fallback is always attempted on inner-schema-parse
    /// failure, independent of `reference_types`. This reuses the base
    /// `dcs-query-union-link` corpus's own genuine bytes (its shape is
    /// admitted by the query-union-link parser) with an injected type_index
    /// entry the corpus itself does not need, isolating the gate-condition
    /// fix from the query-union-link parser's own unrelated fixed-shape
    /// strictness -- see the module doc comment on this test for why the
    /// evidenced `dcs-query-union-link-typeid` corpus cannot be used here.
    #[test]
    fn platform_query_union_link_exports_byte_exact_with_non_empty_type_index() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-query-union-link/raw-packed.bin.b64"
        )));
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-query-union-link/native-template.xml.b64"
        )));
        let body = crate::compiler::bodies::dcs::decode_compatible_dcs(
            crate::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .unwrap();
        // A type_index entry this corpus's own content never references:
        // before the fix, its mere non-emptiness alone was enough to skip
        // the query-union-link fallback and hard fail; after the fix,
        // export must still succeed byte-exactly because the fallback is
        // attempted regardless of what (or whether) `reference_types`
        // resolves.
        let mut type_index = BTreeMap::new();
        type_index.insert(
            "488c0ffa-ef24-480c-a420-3bd2736317f9".to_owned(),
            DcsTypeResolution::Type {
                qname: "cfg:CatalogRef.FilterProbe".to_owned(),
            },
        );
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &type_index,
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .expect("query-union-link fallback must still be attempted with a non-empty type_index");
        assert_eq!(actual, expected);
    }

    /// `dcs-query-union-link-typeid` (evidenced fixture, immutable manifest,
    /// DCS-QUERY-SECOND-FIELD-01): `QueryRows` carries a second, typed field
    /// (`Owner`) transplanting the exact evidenced current-config TypeId
    /// construction `dcs-typeid-reference`'s DataSetObject field already
    /// proved. `DcsQueryUnionLinkPolicy::query_children()`/`parse_query`
    /// (ibcmd-schema/ibcmd-xml) now admit exactly two evidenced
    /// `DataSetQuery` shapes, selected by child count: the original
    /// single-field cohort, or this cohort's five-child shape. With a
    /// real, non-empty `type_index` (built the same way the live route
    /// would), the full common codec now exports this corpus byte-exact.
    #[test]
    fn platform_query_union_link_typeid_body_exports_byte_exact_through_common_codec() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-query-union-link-typeid/raw-packed.bin.b64"
        )));
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-query-union-link-typeid/native-template.xml.b64"
        )));
        // manifest.json: retained.packed_body.sha256 / retained.native_template.sha256
        assert_eq!(
            format!("{:x}", Sha256::digest(&packed)),
            "f51f756a382994936bcf748a62d665cf9a53cfc05153e75401d3df6e8b4ee3ca"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&expected)),
            "5ce6f74897ce0428f2898fcf54ed542dcd23af6aafdbd0f331a0fde1e18bb3be"
        );
        let body = crate::compiler::bodies::dcs::decode_compatible_dcs(
            crate::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .unwrap();
        // The same evidenced TypeId -> qname construction
        // `platform_type_id_reference_body_exports_byte_exact_through_common_codec`
        // already proves, built the same way the real route's
        // `build_metadata_type_indexes_from_texts` would produce it.
        let mut type_index = BTreeMap::new();
        type_index.insert(
            "488c0ffa-ef24-480c-a420-3bd2736317f9".to_owned(),
            DcsTypeResolution::Type {
                qname: "cfg:CatalogRef.FilterProbe".to_owned(),
            },
        );
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &type_index,
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .expect("two-field DataSetQuery must now export through the common codec");
        assert_eq!(actual, expected);
    }

    /// Regression negative: a third field (or any other cardinality outside
    /// the two evidenced `DataSetQuery` shapes) still fails closed with a
    /// typed bail, not a silent skip or a guessed admission -- admitting
    /// exactly two evidenced child-lists by count does not loosen the
    /// parser into accepting an arbitrary N fields.
    #[test]
    fn query_union_link_third_field_fails_closed() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-query-union-link-typeid/raw-packed.bin.b64"
        )));
        let body = crate::compiler::bodies::dcs::decode_compatible_dcs(
            crate::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .unwrap();
        let documents = body.documents();
        let primary = std::str::from_utf8(documents[0]).unwrap();
        // Duplicate the evidenced typed `Owner` field so `QueryRows` carries
        // three fields (name, field, field, field, dataSource, query --
        // six children), a cardinality neither evidenced list admits.
        // Insert the duplicate right after the existing typed field's own
        // closing `</field>` (located via its unique TypeId uuid).
        let type_id_marker = "488c0ffa-ef24-480c-a420-3bd2736317f9</TypeId>";
        let type_id_at = primary.find(type_id_marker).expect("Owner TypeId marker");
        let insertion = type_id_at
            + primary[type_id_at..]
                .find("</field>")
                .expect("Owner field closing tag")
            + "</field>".len();
        let extra_field = "\r\n\t\t\t<field xsi:type=\"DataSetFieldField\"><dataPath>Owner</dataPath><field>Owner</field><valueType><TypeId xmlns=\"http://v8.1c.ru/8.1/data/core\">488c0ffa-ef24-480c-a420-3bd2736317f9</TypeId></valueType></field>";
        let three_field_primary = format!(
            "{}{}{}",
            &primary[..insertion],
            extra_field,
            &primary[insertion..]
        );
        let terminal = std::str::from_utf8(documents[2]).unwrap();
        let settings = std::str::from_utf8(documents[1]).unwrap();
        let three_field_documents: [&[u8]; 3] = [
            three_field_primary.as_bytes(),
            settings.as_bytes(),
            terminal.as_bytes(),
        ];

        let mut type_index = BTreeMap::new();
        type_index.insert(
            "488c0ffa-ef24-480c-a420-3bd2736317f9".to_owned(),
            DcsTypeResolution::Type {
                qname: "cfg:CatalogRef.FilterProbe".to_owned(),
            },
        );
        assert!(
            normalize_data_composition_schema_template_documents_with_profiles(
                &three_field_documents,
                &type_index,
                &BTreeMap::new(),
                &ProfileId::parse("provider:mssql-legacy").unwrap(),
                &ProfileId::parse("xml-2.20").unwrap(),
            )
            .is_err(),
            "a three-field DataSetQuery must fail closed, not be silently admitted"
        );
    }

    /// Regression negative: a primary schema outside BOTH the inner-schema
    /// parser's admitted shape (not DataSetObject) AND the query-union-link
    /// parser's admitted shape (not exactly dataSource+query+union+link+variant)
    /// must still fail closed with a typed bail, not a silent skip -- the
    /// fix only removes the accidental `reference_types.is_empty()` gate on
    /// the fallback *attempt*, it does not loosen either parser's own
    /// admitted-shape strictness.
    #[test]
    fn schema_outside_both_inner_schema_and_query_union_link_parsers_fails_closed() {
        let primary = concat!(
            "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
            "<SchemaFile xmlns=\"\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\r\n",
            "\t<dataCompositionSchema xmlns=\"http://v8.1c.ru/8.1/data-composition-system/schema\">\r\n",
            "\t\t<dataSource><name>Source1</name><dataSourceType>Local</dataSourceType></dataSource>\r\n",
            "\t\t<settingsVariant><name xmlns=\"http://v8.1c.ru/8.1/data-composition-system/settings\">Default</name></settingsVariant>\r\n",
            "\t</dataCompositionSchema>\r\n",
            "</SchemaFile>"
        );
        let settings = concat!(
            "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
            "<Settings xmlns=\"http://v8.1c.ru/8.1/data-composition-system/settings\"/>"
        );
        let terminal = concat!(
            "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
            "<SchemaFile xmlns=\"\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\r\n",
            "\t<dataCompositionSchema xmlns=\"http://v8.1c.ru/8.1/data-composition-system/schema\"/>\r\n",
            "</SchemaFile>"
        );
        let documents: [&[u8]; 3] = [primary.as_bytes(), settings.as_bytes(), terminal.as_bytes()];

        let mut type_index = BTreeMap::new();
        type_index.insert(
            "488c0ffa-ef24-480c-a420-3bd2736317f9".to_owned(),
            DcsTypeResolution::Type {
                qname: "cfg:CatalogRef.FilterProbe".to_owned(),
            },
        );
        let rejection = normalize_data_composition_schema_template_documents_with_profiles(
            &documents,
            &type_index,
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .expect_err(
            "a schema admitted by neither parser must still fail closed with a non-empty type_index",
        );
        // The rejection must be named, not anonymous. This probe's settings
        // variant is rejected by the inline fragment analyzer before the
        // primary-schema branch is reached -- the same step order the previous
        // `Option`-based chain used, now visible in the reason itself.
        assert_eq!(
            rejection.code(),
            "dcs.template-normalize.settings-fragment-parse",
            "the rejection must name the step that refused the source: {rejection}"
        );
    }

    #[test]
    fn platform_link_parameter_exports_byte_exact_through_common_codec() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-link-parameter/raw-packed.bin.b64"
        )));
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-link-parameter/native-template.xml.b64"
        )));
        // manifest.json: retained.packed_body.sha256 / retained.native_template.sha256
        assert_eq!(
            format!("{:x}", Sha256::digest(&packed)),
            "5211a2ac9fa02d3351686f48963445e0062e2d174327fcaf47026a6f11a6b9ae"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&expected)),
            "381e86721884c63c9f99dcde21f1cd78cca07b4644714bf635e954b1f59fc698"
        );
        let body = crate::compiler::bodies::dcs::decode_compatible_dcs(
            crate::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .unwrap();
        // Proves the existing route already transparently threads the new
        // optional `dataSetLink` fields through the shared codec: no
        // per-field wiring was added here, this call is unchanged from the
        // base `dcs-query-union-link` corpus's own equivalent test.
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn platform_link_expressions_exports_byte_exact_through_common_codec() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-link-expressions/raw-packed.bin.b64"
        )));
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-link-expressions/native-template.xml.b64"
        )));
        // manifest.json: retained.packed_body.sha256 / retained.native_template.sha256
        assert_eq!(
            format!("{:x}", Sha256::digest(&packed)),
            "c78d5cbf882eec93cec27480f200f1dad7b0d98c5938e8c2d115c4c1f4b46ce3"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&expected)),
            "e80cc9492ab93cabff9799fb14e7e4c6fafff0d96129acba19ba53d4aa4faf54"
        );
        let body = crate::compiler::bodies::dcs::decode_compatible_dcs(
            crate::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .unwrap();
        // Same transparency proof as the link-parameter cohort, for the
        // fuller six-plus-three-field state (including the platform's own
        // canonical reordering of the three newest fields).
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn platform_style_free_area_template_exports_byte_exact_through_common_codec() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-template/raw-packed.bin.b64"
        )));
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-template/native-template.xml.b64"
        )));
        let body = crate::compiler::bodies::dcs::decode_compatible_dcs(
            crate::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .unwrap();
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn platform_area_appearance_web_color_exports_byte_exact_through_common_codec() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-appearance-web-color/raw-packed.bin.b64"
        )));
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-appearance-web-color/native-template.xml.b64"
        )));
        let body = crate::compiler::bodies::dcs::decode_compatible_dcs(
            crate::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .unwrap();
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn platform_multi_cell_appearance_exports_byte_exact_through_common_codec() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-multi-cell-appearance/raw-packed.bin.b64"
        )));
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-multi-cell-appearance/native-template.xml.b64"
        )));
        let body = crate::compiler::bodies::dcs::decode_compatible_dcs(
            crate::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .unwrap();
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn platform_area_style_color_reference_exports_byte_exact_through_common_codec() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-style-color-reference/raw-packed.bin.b64"
        )));
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-style-color-reference/native-template.xml.b64"
        )));
        // manifest.json: retained.packed_body.sha256 / retained.native_template.sha256
        assert_eq!(
            format!("{:x}", Sha256::digest(&packed)),
            "1e6c10a050235b9ecd42b1b7bdcdb3df5b148bde0c42cb442ba7bd16722cdf9b"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&expected)),
            "4269ac193b76bb88ecaaf65a5b4ef9ed12a31cdcf1d36d8ac429de68cf10f970"
        );
        let body = crate::compiler::bodies::dcs::decode_compatible_dcs(
            crate::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .unwrap();
        // The standard/built-in style-reference form needs no resolver
        // (`object_refs` empty, exactly like every other pre-existing
        // corpus): the named lexical spelling is authenticated on both
        // directions without any configuration-object lookup.
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn platform_area_style_item_uuid_exports_byte_exact_through_common_codec() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-style-item-uuid/raw-packed.bin.b64"
        )));
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-style-item-uuid/native-template.xml.b64"
        )));
        // manifest.json: retained.packed_body.sha256 / retained.native_template.sha256
        assert_eq!(
            format!("{:x}", Sha256::digest(&packed)),
            "680d04d34a12c54be75ac69a5c20ff82d2136736e11998b070c77d7abbbe3235"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&expected)),
            "98f1857d3424198275cc35834a6635c28623568aae8d01a95cb5e220f91b818f"
        );
        let body = crate::compiler::bodies::dcs::decode_compatible_dcs(
            crate::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .unwrap();
        // The custom StyleItem form's raw uuid storage wire form requires a
        // resolver: `object_refs` here stands in for what a real dump
        // session's own object-reference scan would already have found
        // (see `data_composition_style_item_name`'s own established
        // "StyleItem.<Name>" convention, reused verbatim, not invented).
        let mut object_refs = BTreeMap::new();
        object_refs.insert(
            "4a9d8536-ff59-4a90-a1cf-646d241dc53c".to_string(),
            "StyleItem.CorpusAccent".to_string(),
        );
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &BTreeMap::new(),
            &object_refs,
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .unwrap();
        assert_eq!(actual, expected);

        // Without the resolver entry, decode must fail closed instead of
        // silently dropping the AreaTemplate appearance or emitting a
        // fabricated value.
        let without_resolver = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        );
        assert_ne!(without_resolver.ok(), Some(expected));
    }

    #[test]
    fn platform_parameter_scalar_types_exports_byte_exact_through_common_codec() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-parameter-scalar-types/raw-packed.bin.b64"
        )));
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-parameter-scalar-types/native-template.xml.b64"
        )));
        let body = crate::compiler::bodies::dcs::decode_compatible_dcs(
            crate::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .unwrap();
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn platform_output_parameters_exports_byte_exact_through_common_codec() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-output-parameters/raw-packed.bin.b64"
        )));
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-output-parameters/native-template.xml.b64"
        )));
        // manifest.json: rounds.packed_body_sha256 / rounds.native_template_sha256
        assert_eq!(
            format!("{:x}", Sha256::digest(&packed)),
            "1bab2e8e93c491f33d094473d8456e7877c1673d45656cca9d4729ea40c82fd7"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&expected)),
            "bc27a20de1bb75a83b3727ac457db04791cdd092c64e2e31e5b58ebecf296ddb"
        );

        let body = crate::compiler::bodies::dcs::decode_compatible_dcs(
            crate::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .unwrap();
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .unwrap();

        // `canonicalize_data_composition_settings_document` now routes
        // outputParameters through the same shared `ibcmd-xml` codec
        // (`DcsSettingsBuilder::output_parameters`, `emit_dcs_settings_children_parts`,
        // `rewrite_dcs_settings_children`) that selection/filter/order/
        // conditionalAppearance already used, the same way the terminal
        // AreaTemplate side table achieves byte-accuracy for TextColor/
        // Details. The storage "Title" -> source "Заголовок" lexical
        // canonicalization this codec already performed is now actually
        // exercised on this path, so the export is byte-exact.
        assert_eq!(actual, expected);
    }

    #[test]
    fn unknown_settings_children_never_reach_the_generic_normalizer() {
        // `outputParameters` is intentionally not in this list: this work
        // package admits it as a recognized typed element (evidence:
        // dcs-output-parameters), so an occurrence with no items no longer
        // qualifies as "unowned" -- the dedicated cohort-shape fail-closed
        // cases for it are covered by the ibcmd-xml unit tests instead
        // (`output_parameters_rejects_*` in crates/ibcmd-xml/src/dcs.rs).
        for unknown in [
            "<futureProbe/>",
            "<probe:futureProbe xmlns:probe=\"urn:ibcmd-rs:dcs-probe\"/>",
        ] {
            let settings = format!(
                "<Settings xmlns=\"{}\" xmlns:xsi=\"{}\">{unknown}</Settings>",
                std::str::from_utf8(DCS_SETTINGS_NS).unwrap(),
                std::str::from_utf8(XSI_NS).unwrap()
            );
            let source_profile = ProfileId::parse("provider:mssql-legacy").unwrap();
            let target_profile = ProfileId::parse("xml-2.20").unwrap();
            assert!(
                canonicalize_data_composition_settings_document(
                    &settings,
                    &BTreeMap::new(),
                    &source_profile,
                    &target_profile,
                )
                .is_err(),
                "unknown child reached generic normalization: {unknown}"
            );
        }
    }

    #[test]
    fn platform_8_3_27_xml_2_20_root_auto_selection_exports_byte_exact() {
        let packed = include_bytes!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/dcs-selection-auto/raw/\
             f4db0f6c-34f4-4449-995d-6265516e5fa8.0.deflate"
        );
        let expected = include_bytes!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/dcs-selection-auto/native/Reports/\
             DcsCorpus/Templates/MainSchema/Ext/Template.xml"
        );

        let body = crate::compiler::bodies::dcs::decode_compatible_dcs(
            crate::compiler::bodies::dcs::DcsTemplateKind::Schema,
            packed,
        )
        .expect("platform-attested root Auto selection body must decode");
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .expect("platform-attested root Auto selection must be exportable through the live codec");

        assert_eq!(actual, expected);
    }
}
