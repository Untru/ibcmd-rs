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
    parse_dcs_query_union_link_storage_document_with_references,
    rewrite_dcs_primary_schema_storage_document, rewrite_dcs_settings_children,
    rewrite_dcs_terminal_area_template_storage_fragment,
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
const MANAGED_APPLICATION_CORE_NS: &[u8] = b"http://v8.1c.ru/8.2/managed-application/core";
const MANAGED_APPLICATION_LOGFORM_NS: &[u8] = b"http://v8.1c.ru/8.2/managed-application/logform";
const XCF_READABLE_NS: &[u8] = b"http://v8.1c.ru/8.3/xcf/readable";
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
    /// The primary schema file matched no admitted shape: neither closed
    /// typed cohort recognized it, and the general storage-to-source
    /// transliteration could not account for its bytes either.
    PrimarySchemaParse {
        inner_schema: DcsInnerSchemaError,
        query_union_link: DcsInnerSchemaError,
        transliterate: DcsInnerSchemaError,
    },
    /// The `DataSetObject` schema emitter rejected the parsed schema.
    InnerSchemaEmit(DcsInnerSchemaError),
    /// The query/union/link emitter rejected the parsed schema.
    QueryUnionLinkEmit(DcsInnerSchemaError),
    /// The terminal AreaTemplate parsed but its source fragment was rejected.
    AreaTemplateEmit(DcsInnerSchemaError),
    /// The typed AreaTemplate coordinate refused the terminal document and the
    /// storage-to-source fragment transliteration could not account for its
    /// bytes either.
    TerminalFragmentRewrite(DcsInnerSchemaError),
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
            Self::TerminalFragmentRewrite(_) => "dcs.template-normalize.terminal-fragment-rewrite",
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
            | Self::AreaTemplateEmit(cause)
            | Self::TerminalFragmentRewrite(cause) => inner_schema_failure_class(cause),
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
                transliterate,
            } => write!(
                formatter,
                "primary schema file matched no admitted parser (inner schema: {inner_schema}; query/union/link: {query_union_link}; transliterate: {transliterate})"
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
            Self::TerminalFragmentRewrite(error) => write!(
                formatter,
                "no typed coordinate described the terminal SchemaFile and the \
                 storage-to-source fragment transliteration could not account for its \
                 bytes: {error}"
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
    /// The typed cohort refused the document and the general
    /// storage-to-source transliteration could not account for its bytes
    /// either.
    Transliterate,
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
            Self::Transliterate => "dcs.settings-canonicalize.transliterate",
        }
    }

    /// Whether this rejection says only "no enumerated shape describes this
    /// document", as opposed to "these bytes are not a settings document" or
    /// "an invariant of our own serializer broke".
    ///
    /// Only the former may be handed to the transliteration: a malformed
    /// document has nothing to transliterate, and an invariant break must
    /// stay loud rather than be silently routed around.
    const fn is_cohort_refusal(&self) -> bool {
        match self {
            Self::Analysis(DcsSettingsDocumentAnalysisError::UnsupportedSource { .. })
            | Self::UnsupportedChild { .. } => true,
            Self::Analysis(DcsSettingsDocumentAnalysisError::Malformed(_))
            | Self::Writer
            | Self::CanonicalChildren(_)
            | Self::ChildrenRewrite
            | Self::Transliterate => false,
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
            Self::UnsupportedChild { .. } | Self::Writer | Self::Transliterate => {
                MetadataSourceFailureClass::Unsupported
            }
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
            Self::Transliterate => formatter.write_str(
                "no typed cohort described the document and the storage-to-source \
                 transliteration could not account for its bytes",
            ),
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
    // The reference-family half of the same index. `data_composition_type_id_xml`
    // already spells these as `<v8:TypeSet>` on the typed path; the
    // transliteration had no map to put them in at all, so every
    // `DocumentRef`/`CatalogRef`/`ExchangePlanRef` uuid fell through to
    // "no configuration type-index resolution" and failed closed on a name
    // the index in fact carried. `AnyIBRef` is a protocol identifier that
    // sits outside the generated-type index, exactly as that function's own
    // special case records.
    let mut type_set_types: BTreeMap<String, String> = type_index
        .iter()
        .filter_map(|(type_id, resolution)| match resolution {
            DcsTypeResolution::TypeSet { qname } => qname
                .strip_prefix(CFG_PREFIX)
                .map(|name| (type_id.clone(), name.to_owned())),
            DcsTypeResolution::KeepId | DcsTypeResolution::Type { .. } => None,
        })
        .collect();
    type_set_types
        .entry(ANY_IB_REF_TYPE_ID.to_owned())
        .or_insert_with(|| "AnyIBRef".to_owned());
    // The type ids the configuration index deliberately resolves to no
    // semantic name. The platform writes those straight back as
    // `<v8:TypeId>`; a type id in none of the three maps is not resolvable at
    // all and must fail closed rather than be spelled any way.
    let opaque_type_ids = type_index
        .iter()
        .filter_map(|(type_id, resolution)| match resolution {
            DcsTypeResolution::KeepId => Some(type_id.clone()),
            DcsTypeResolution::Type { .. } | DcsTypeResolution::TypeSet { .. } => None,
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
        // A typed fragment is re-analyzed against the full cohort; a
        // transliterated one only against the structural contract, because
        // the cohort question is precisely the one it was produced to route
        // around. See `DcsInlineSettingsFragment::parse_transliterated`.
        let fragment = match canonical {
            CanonicalDcsSettingsDocument::Typed(xml) => DcsInlineSettingsFragment::parse(xml),
            CanonicalDcsSettingsDocument::Transliterated(xml) => {
                DcsInlineSettingsFragment::parse_transliterated(xml)
            }
        };
        settings.push(
            fragment.map_err(|cause| DcsTemplateNormalizeError::SettingsFragmentParse {
                index,
                cause,
            })?,
        );
    }
    let mut source = match schema {
        Ok(schema) => emit_dcs_inner_schema_source_document(&schema, &settings)
            .map_err(DcsTemplateNormalizeError::InnerSchemaEmit)?,
        Err(inner_schema) => {
            match parse_dcs_query_union_link_storage_document_with_references(
                envelope.primary_schema_file(),
                source_profile.clone(),
                "mssql:dcs-schema-template/query-union-link",
                &reference_types,
            ) {
                Ok(schema) => emit_dcs_query_union_link_source_document(&schema, &settings)
                    .map_err(DcsTemplateNormalizeError::QueryUnionLinkEmit)?,
                // Neither closed typed cohort describes this schema. Fall
                // through to the general storage-to-source transliteration,
                // which reproduces the platform's own source spelling from
                // the stored document's own bytes instead of from an
                // enumerated shape.
                Err(query_union_link) => rewrite_dcs_primary_schema_storage_document(
                    envelope.primary_schema_file(),
                    &reference_types,
                    &type_set_types,
                    &opaque_type_ids,
                    &style_reference_types,
                    &settings,
                )
                .map_err(|transliterate| {
                    DcsTemplateNormalizeError::PrimarySchemaParse {
                        inner_schema,
                        query_union_link,
                        transliterate,
                    }
                })?,
            }
        }
    };
    let terminal = envelope.terminal_schema_file();
    // The typed coordinate still decides every terminal document it describes.
    // What changes after it refuses is only that the document is transliterated
    // from its own bytes instead of being dropped: an empty terminal
    // contributes no fragment, and a template-carrying one that cannot be
    // spelled fails the template closed rather than silently exporting a
    // schema with its area templates missing.
    let fragment = match parse_dcs_area_template_storage_document_with_references(
        terminal,
        source_profile.clone(),
        "mssql:dcs-schema-template/area-template",
        &style_reference_types,
    ) {
        Ok(area) => Some(
            emit_dcs_area_template_source_fragment(&area)
                .map_err(DcsTemplateNormalizeError::AreaTemplateEmit)?,
        ),
        Err(_) if envelope.terminal_carries_templates() => {
            rewrite_dcs_terminal_area_template_storage_fragment(
                terminal,
                &reference_types,
                &type_set_types,
                &opaque_type_ids,
                &style_reference_types,
            )
            .map_err(DcsTemplateNormalizeError::TerminalFragmentRewrite)?
        }
        Err(_) => None,
    };
    if let Some(fragment) = fragment {
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
    data_composition_object_ref_name(text, object_refs, "StyleItem.")
}

/// The name a stored `0:<uuid>` reference denotes, under one object kind.
fn data_composition_object_ref_name(
    text: &str,
    object_refs: &BTreeMap<String, String>,
    kind: &str,
) -> Option<String> {
    let uuid = serialized_data_composition_color_ref_uuid(text)?;
    let reference = object_refs.get(&uuid).or_else(|| {
        let source_uuid = text.trim().strip_prefix("0:")?;
        object_refs.get(source_uuid)
    })?;
    let name = reference.strip_prefix(kind)?;
    (!name.is_empty()).then(|| name.to_string())
}

/// Canonicalizes one standalone `Settings` document into its inline source
/// fragment.
///
/// The typed cohort is tried first and, when it describes the document, still
/// decides every one of its five children. What changes here is only what
/// happens after it refuses: instead of failing the whole template, the
/// document is transliterated from its own bytes by the same lexical writer
/// that already renders the parts no typed child owns. That writer is the
/// settings-side twin of
/// [`ibcmd_xml::rewrite_dcs_primary_schema_storage_document`] -- it re-spells
/// the storage document in the source direction (root rename, prefix mapping,
/// `dNpM` renumbering, style-item resolution) and fails closed on anything it
/// cannot account for, rather than emitting a guess.
fn canonicalize_data_composition_settings_document(
    document: &str,
    object_refs: &BTreeMap<String, String>,
    source_profile: &ProfileId,
    target_profile: &ProfileId,
) -> std::result::Result<CanonicalDcsSettingsDocument, DcsSettingsCanonicalizeError> {
    match canonicalize_typed_data_composition_settings_document(
        document,
        object_refs,
        source_profile,
        target_profile,
    ) {
        Ok(canonical) => Ok(CanonicalDcsSettingsDocument::Typed(canonical)),
        // A cohort refusal is not evidence that the document cannot be
        // spelled -- only that no enumerated shape describes it. Everything
        // else (a malformed document, a writer that could not reproduce the
        // bytes, an invariant break in the canonical serializer) stays fatal.
        Err(typed) if typed.is_cohort_refusal() => {
            transliterate_data_composition_settings_document(document, object_refs)
                .map(CanonicalDcsSettingsDocument::Transliterated)
                .ok_or(DcsSettingsCanonicalizeError::Transliterate)
        }
        Err(typed) => Err(typed),
    }
}

/// One canonicalized `Settings` document, tagged with which of the two paths
/// produced it.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum CanonicalDcsSettingsDocument {
    /// Emitted by the typed cohort.
    Typed(String),
    /// Transliterated from the storage document's own bytes.
    Transliterated(String),
}

impl CanonicalDcsSettingsDocument {
    /// The rendered fragment, whichever path produced it. Production code
    /// always needs the path too, so it matches on the variant instead; this
    /// exists for assertions that only care about the bytes.
    #[cfg(test)]
    pub(crate) fn as_str(&self) -> &str {
        match self {
            Self::Typed(xml) | Self::Transliterated(xml) => xml,
        }
    }
}

/// Renders one standalone `Settings` document into its inline source fragment
/// with the lexical writer alone, with no typed child re-emission.
fn transliterate_data_composition_settings_document(
    document: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    let mut writer = DataCompositionXmlWriter::new(object_refs);
    writer.write_document(document, DataCompositionDocumentMode::Settings)?;
    let settings = writer
        .output
        .trim_start_matches(['\r', '\n', '\t'])
        .to_string();
    Some(indent_data_composition_settings(&settings))
}

fn canonicalize_typed_data_composition_settings_document(
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

/// The storage documents a packed form body carries under its dynamic list
/// `ListSettings`, paired with the inline element each becomes.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FormListSettingsChildKind {
    Filter,
    Order,
    ConditionalAppearance,
    /// The grouping document, stored under the `Group` property. Its rendered
    /// root is only an intermediate: the inline position nests it, see
    /// [`transliterate_form_list_settings_group_items_document`].
    GroupItems,
    /// The data-parameter values, stored under the `DataParameters` property.
    DataParameters,
}

impl FormListSettingsChildKind {
    /// The element name the platform gives this child inline in a decompiled
    /// Form.xml.
    ///
    /// Its storage spelling differs only in the initial letter (`Filter`,
    /// `Order`, `ConditionalAppearance`, `GroupItems`), so pinning the rendered
    /// name pins the storage root the writer accepted as well.
    const fn source_root_local(self) -> &'static str {
        match self {
            Self::Filter => "filter",
            Self::Order => "order",
            Self::ConditionalAppearance => "conditionalAppearance",
            Self::GroupItems => "groupItems",
            Self::DataParameters => "dataParameters",
        }
    }
}

/// What one `ListSettings` child storage document re-spells to.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum FormListSettingsChildTransliteration {
    /// The storage document is an empty element. It carries no content, and
    /// the platform's own export writes nothing for it -- the same physical
    /// state, and the same treatment, the `ConditionalAppearance` property
    /// already has: property present, element omitted.
    Empty,
    /// The inline source fragment, indented for its position.
    Fragment(String),
}

/// Re-spells one `ListSettings` child storage document as the inline source
/// fragment the platform's own export writes for it, indented for its
/// position under `<ListSettings>`.
///
/// This is the form-body twin of the standalone-`Settings` transliteration:
/// the storage and source documents carry the same content and differ only in
/// how they spell it -- the root's initial letter, prefixes for namespaces the
/// `Form.xml` root already declares, and generated prefixes numbered by depth
/// in the target document. So the fragment is produced from the platform's own
/// bytes by a lexical writer rather than by teaching a typed representation
/// one more shape.
///
/// Returns `None` -- leaving the caller's existing refusal in place -- for
/// anything the bytes do not account for: a document that is not well-formed,
/// a root that is not the expected settings-namespace element, a namespace
/// outside the evidenced `ListSettings` vocabulary, more than one generated
/// prefix on a single element, and an unresolvable QName in character data.
pub(crate) fn transliterate_form_list_settings_child_document(
    bytes: &[u8],
    kind: FormListSettingsChildKind,
    object_refs: &BTreeMap<String, String>,
    indent: &str,
) -> Option<FormListSettingsChildTransliteration> {
    if bytes.len() > MAX_FORM_LIST_SETTINGS_CHILD_STORAGE_BYTES {
        return None;
    }
    let document = std::str::from_utf8(bytes).ok()?;
    let document = document.strip_prefix('\u{feff}').unwrap_or(document);
    let mut writer = DataCompositionXmlWriter::new_for_mode(
        object_refs,
        DataCompositionDocumentMode::FormListSettingsChild,
    );
    writer.write_document(document, DataCompositionDocumentMode::FormListSettingsChild)?;
    let fragment = writer.output.trim_start_matches(['\r', '\n', '\t', ' ']);
    // The writer renames the root from the mode alone; this pins that the
    // document it renamed was the one this caller asked for, so a `Filter`
    // payload can never be spliced in as an `order` element.
    let expected_open = format!("<dcsset:{}", kind.source_root_local());
    if !fragment.starts_with(&expected_open)
        || !fragment[expected_open.len()..].starts_with([' ', '>', '/', '\t', '\r', '\n'])
    {
        return None;
    }
    if fragment == format!("{expected_open}/>") {
        return Some(FormListSettingsChildTransliteration::Empty);
    }
    Some(FormListSettingsChildTransliteration::Fragment(
        indent_form_list_settings_child_fragment(fragment, indent),
    ))
}

/// Re-spells the `Group` storage document as the nested `StructureItemGroup`
/// chain the platform writes inline under `<ListSettings>`.
///
/// Storage keeps the grouping as one flat `<GroupItems>` list; the inline
/// position spells the same content as a right-nested chain, one
/// `StructureItemGroup` per stored item, each carrying that one item in its own
/// `groupItems` and the next group beside it:
///
/// ```text
/// <dcsset:item xsi:type="dcsset:StructureItemGroup">
///     <dcsset:groupItems> <item 1> </dcsset:groupItems>
///     <dcsset:item xsi:type="dcsset:StructureItemGroup">
///         <dcsset:groupItems> <item 2> </dcsset:groupItems>
///         ...
/// ```
///
/// Evidence: UT 11.5.27.75. 254 forms store a `Group` document; 220 of them
/// store the empty `<GroupItems/>` and their native `<ListSettings>` carries no
/// `dcsset:item`, and the 17 forms that store items are exactly the 17 native
/// `<ListSettings>` blocks that carry one. The chain above is the shape of all
/// of them, the deepest being the six-item
/// `Catalogs/ВидыЦен/Forms/ФормаНастройкиРасписанияАвтообновленияЦен`.
///
/// Everything below the stored items is re-spelled by the same lexical writer
/// the other three children use, so the item vocabulary needs no typed model;
/// only the nesting is composed here. Returns `None`, writing nothing, for a
/// document that writer does not account for or a rendered root that is not the
/// bare `<dcsset:groupItems>` this composition splices.
pub(crate) fn transliterate_form_list_settings_group_items_document(
    bytes: &[u8],
    object_refs: &BTreeMap<String, String>,
    indent: &str,
) -> Option<FormListSettingsChildTransliteration> {
    let flat = match transliterate_form_list_settings_child_document(
        bytes,
        FormListSettingsChildKind::GroupItems,
        object_refs,
        "",
    )? {
        FormListSettingsChildTransliteration::Empty => {
            return Some(FormListSettingsChildTransliteration::Empty);
        }
        FormListSettingsChildTransliteration::Fragment(fragment) => fragment,
    };
    let inner = flat
        .strip_prefix("<dcsset:groupItems>")?
        .strip_suffix("\r\n")?
        .strip_suffix("</dcsset:groupItems>")?;
    let runs = data_composition_group_item_runs(inner)?;
    if runs.is_empty() {
        return Some(FormListSettingsChildTransliteration::Empty);
    }
    let mut output = String::new();
    let mut pad = indent.to_string();
    for run in &runs {
        output.push_str(pad.as_str());
        output.push_str("<dcsset:item xsi:type=\"dcsset:StructureItemGroup\">\r\n");
        output.push_str(&format!("{pad}\t<dcsset:groupItems>\r\n"));
        output.push_str(&indent_form_list_settings_child_fragment(
            run,
            &format!("{pad}\t"),
        ));
        output.push_str(&format!("{pad}\t</dcsset:groupItems>\r\n"));
        pad.push('\t');
    }
    for _ in &runs {
        pad.pop();
        output.push_str(&format!("{pad}</dcsset:item>\r\n"));
    }
    Some(FormListSettingsChildTransliteration::Fragment(output))
}

/// The top-level `<dcsset:item>` elements of a rendered `groupItems` body, each
/// with the indentation of its own line so the chain can re-indent them
/// uniformly.
///
/// Refuses a body that carries anything besides those elements and the
/// whitespace between them, so no stored content can be dropped silently.
fn data_composition_group_item_runs(inner: &str) -> Option<Vec<&str>> {
    const OPEN: &str = "<dcsset:item";
    const CLOSE: &str = "</dcsset:item>";
    let whitespace = ['\r', '\n', '\t', ' '];
    let mut runs = Vec::new();
    let mut rest = inner;
    loop {
        let Some(open) = rest.find(OPEN) else {
            return rest.trim_matches(whitespace).is_empty().then_some(runs);
        };
        if !rest[..open].trim_matches(whitespace).is_empty() {
            return None;
        }
        let line_start = rest[..open].rfind('\n').map_or(0, |index| index + 1);
        let mut depth = 0usize;
        let mut cursor = open;
        let end = loop {
            let next_open = rest[cursor..].find(OPEN).map(|at| cursor + at);
            let next_close = rest[cursor..].find(CLOSE).map(|at| cursor + at);
            match (next_open, next_close) {
                (Some(next_open), close) if close.is_none_or(|close| next_open < close) => {
                    let tag_end = rest[next_open..].find('>').map(|at| next_open + at + 1)?;
                    if !rest[..tag_end].ends_with("/>") {
                        depth += 1;
                    } else if depth == 0 {
                        break tag_end;
                    }
                    cursor = tag_end;
                }
                (_, Some(next_close)) => {
                    depth = depth.checked_sub(1)?;
                    let tag_end = next_close + CLOSE.len();
                    if depth == 0 {
                        break tag_end;
                    }
                    cursor = tag_end;
                }
                _ => return None,
            }
        };
        runs.push(&rest[line_start..end]);
        rest = &rest[end..];
    }
}

/// Bounds one `ListSettings` child storage document. The largest such
/// document in the UT corpus is far below this; the ceiling exists so a
/// malformed length cannot drive an unbounded rewrite.
const MAX_FORM_LIST_SETTINGS_CHILD_STORAGE_BYTES: usize = 4 * 1024 * 1024;

/// Shifts a rendered `ListSettings` child to the depth its inline position
/// sits at, on the same rule the standalone settings splice uses: only
/// pretty-printing whitespace moves, never a line break inside character data.
fn indent_form_list_settings_child_fragment(fragment: &str, indent: &str) -> String {
    let literal = data_composition_character_data_runs(fragment);
    let mut indented = String::with_capacity(fragment.len() + indent.len() * 8);
    let mut offset = 0usize;
    for line in fragment.split_inclusive('\n') {
        if !data_composition_offset_continues_character_data(&literal, offset) {
            indented.push_str(indent);
        }
        indented.push_str(line);
        offset += line.len();
    }
    indented.push_str("\r\n");
    indented
}

/// Re-spells the Form-wide `Attributes/ConditionalAppearance` storage
/// document as the inline source fragment the platform's own export writes
/// for it.
///
/// This is the same lexical rewrite the `ListSettings` children get, applied
/// one position higher: the storage document wraps its `conditionalAppearance`
/// in a `Settings` root, while the inline position is a bare
/// `<ConditionalAppearance>` under `<Attributes>` whose children are the
/// settings-namespace contents of that child. So the whole document is handed
/// to the writer in the `ListSettings`-child mode -- the vocabulary, the
/// prefix rules and the style-item resolution are identical -- and the two
/// spellings that differ are corrected afterwards: the `Settings` wrapper is
/// dropped and the child is renamed to the wrapper element the policy names.
///
/// The mode's depth-numbered `dNpM` minting cannot fire here, because it fires
/// only for a namespace the `Form` root does not declare, and none of the 126
/// UT payloads declares one anywhere -- root or below. A payload that did
/// would be spelled at the `ListSettings` depth rather than this one, so
/// instead of trusting that, the whole rewrite is refused when the writer
/// mints anything at all.
///
/// Returns `None`, leaving the caller's typed refusal in place, for anything
/// the bytes do not account for.
pub(crate) fn transliterate_form_attributes_conditional_appearance_document(
    bytes: &[u8],
    object_refs: &BTreeMap<String, String>,
    indent: &str,
) -> Option<String> {
    if bytes.len() > MAX_FORM_LIST_SETTINGS_CHILD_STORAGE_BYTES {
        return None;
    }
    let document = std::str::from_utf8(bytes).ok()?;
    let document = document.strip_prefix('\u{feff}').unwrap_or(document);
    let mut writer = DataCompositionXmlWriter::new_for_mode(
        object_refs,
        DataCompositionDocumentMode::FormListSettingsChild,
    );
    writer.write_document(document, DataCompositionDocumentMode::FormListSettingsChild)?;
    let fragment = writer.output.trim_start_matches(['\r', '\n', '\t', ' ']);
    if fragment.contains(" xmlns:") {
        return None;
    }
    let wrapper = form_attributes_conditional_appearance_wrapper_local()?;
    let body = fragment
        .strip_prefix("<dcsset:settings>")?
        .strip_suffix("</dcsset:settings>")?;
    // The storage document puts its one child on its own line, one tab in.
    let body = body.strip_prefix("\r\n")?.strip_suffix("\r\n")?;
    let body = body
        .strip_prefix("\t<dcsset:conditionalAppearance")?
        .strip_suffix("</dcsset:conditionalAppearance>")?;
    // Nothing may separate the child's own close tag from the root's, so a
    // second top-level child cannot be swallowed into the rename.
    if !body.starts_with(['>', ' ', '\t', '\r', '\n']) {
        return None;
    }
    let renamed = format!("<{wrapper}{body}</{wrapper}>");
    Some(reindent_form_attributes_conditional_appearance_fragment(
        &renamed, indent,
    ))
}

/// The inline element name the Form-wide conditional appearance is spelled
/// with, taken from the same policy the typed writer emits against.
fn form_attributes_conditional_appearance_wrapper_local() -> Option<String> {
    let policy = ibcmd_schema::bundled_dcs_form_attributes_conditional_appearance_policy().ok()?;
    let qname = policy.wrapper_qname();
    let local = qname.rsplit_once('}').map(|(_, local)| local)?;
    (!local.is_empty() && !local.contains(':')).then(|| local.to_string())
}

/// Moves a rendered Form-wide conditional appearance from the depth its
/// storage document spells it at to the depth its inline position sits at.
///
/// The storage document indents the appearance one level deeper than the
/// inline form does, since it carries a `Settings` root the inline form has
/// no counterpart for. Only pretty-printing whitespace moves: a line that
/// continues character data is left exactly as the platform stored it.
fn reindent_form_attributes_conditional_appearance_fragment(
    fragment: &str,
    indent: &str,
) -> String {
    let literal = data_composition_character_data_runs(fragment);
    let mut output = String::with_capacity(fragment.len() + indent.len() * 8);
    let mut offset = 0usize;
    for line in fragment.split_inclusive('\n') {
        if data_composition_offset_continues_character_data(&literal, offset) {
            output.push_str(line);
        } else {
            output.push_str(indent);
            output.push_str(line.strip_prefix('\t').unwrap_or(line));
        }
        offset += line.len();
    }
    output.push_str("\r\n");
    output
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
    let mut writer = DataCompositionXmlWriter::new_for_mode(object_refs, mode);
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

/// Shifts a rendered settings document to the two-tab depth its inline
/// source position sits at.
///
/// Only the document's pretty-printing whitespace moves. A line break inside
/// character data -- a multi-line query, expression or presentation string --
/// is part of the value the platform stored, so indenting it would change the
/// value rather than the layout, and the platform's own export shows it does
/// not.
fn indent_data_composition_settings(settings: &str) -> String {
    let literal = data_composition_character_data_runs(settings);
    let mut indented = String::from("\r\n");
    let mut offset = 0usize;
    for line in settings.split_inclusive('\n') {
        if !data_composition_offset_continues_character_data(&literal, offset) {
            indented.push_str("\t\t");
        }
        indented.push_str(line);
        offset += line.len();
    }
    indented
}

/// Byte ranges of the text runs that carry character data rather than
/// pretty-printing whitespace.
///
/// A text run is everything between a `>` that closes a tag and the `<` that
/// opens the next one. The scan is quote-aware so a `>` inside an attribute
/// value cannot end a tag early.
fn data_composition_character_data_runs(xml: &str) -> Vec<(usize, usize)> {
    let bytes = xml.as_bytes();
    let mut runs = Vec::new();
    let mut quote: Option<u8> = None;
    let mut in_tag = false;
    let mut run_start = 0usize;
    for (offset, byte) in bytes.iter().copied().enumerate() {
        if in_tag {
            match (quote, byte) {
                (Some(open), byte) if byte == open => quote = None,
                (Some(_), _) => {}
                (None, b'"' | b'\'') => quote = Some(byte),
                (None, b'>') => {
                    in_tag = false;
                    run_start = offset + 1;
                }
                (None, _) => {}
            }
        } else if byte == b'<' {
            in_tag = true;
            if run_start < offset && !xml[run_start..offset].trim().is_empty() {
                runs.push((run_start, offset));
            }
        }
    }
    if !in_tag && run_start < xml.len() && !xml[run_start..].trim().is_empty() {
        runs.push((run_start, xml.len()));
    }
    runs
}

/// Whether a line starting at `offset` continues a character-data run rather
/// than starting one. The run's own first line is still preceded by markup,
/// so only offsets strictly inside a run are continuations.
fn data_composition_offset_continues_character_data(
    runs: &[(usize, usize)],
    offset: usize,
) -> bool {
    runs.iter()
        .any(|(start, end)| offset > *start && offset < *end)
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum DataCompositionDocumentMode {
    Settings,
    FormServerStateFragment,
    /// One packed-form `ListSettings` child storage document -- `Filter`,
    /// `Order` or `ConditionalAppearance` -- re-spelled for its inline
    /// position inside a decompiled `Form.xml`.
    ///
    /// The distinguishing fact is where the fragment lands. A standalone
    /// `Settings` document carries its own root, so the writer declares the
    /// four UI namespaces there and mints `dNpM` prefixes against the
    /// settings document's own depth. A `ListSettings` child is spliced under
    /// `Form/Attributes/Attribute/Settings/ListSettings`, whose root already
    /// declares every namespace the platform's own export uses -- proven by
    /// the `Form` element of all 1 672 native UT forms that carry a
    /// `ListSettings` -- so the same content is spelled with the globally
    /// declared prefixes and generated prefixes count depth from the Form
    /// root, not from the fragment.
    FormListSettingsChild,
}

impl DataCompositionDocumentMode {
    /// Whether QNames resolve against the prefixes a decompiled `Form.xml`
    /// declares on its own root element rather than against declarations the
    /// writer emits itself.
    const fn uses_form_root_prefixes(self) -> bool {
        matches!(
            self,
            Self::FormServerStateFragment | Self::FormListSettingsChild
        )
    }
}

/// 1-based depth of a `ListSettings` child element inside `Form.xml`:
/// `Form`(1) / `Attributes`(2) / `Attribute`(3) / `Settings`(4) /
/// `ListSettings`(5), so the fragment's own root sits at 6.
///
/// The platform numbers a generated `dNp1` prefix by exactly this depth. Two
/// native UT captures pin both ends of the range: `DocumentJournals/
/// Взаимодействия/Forms/ФормаСписка` declares `xmlns:d8p1` on a
/// `filter/item/right` (6 + 2) and `DocumentJournals/ЧекиККМ/Forms/
/// ФормаСписка` declares `xmlns:d10p1` on a
/// `conditionalAppearance/item/filter/item/right` (6 + 4).
const FORM_LIST_SETTINGS_CHILD_ROOT_DEPTH: usize = 6;

#[derive(Debug, Clone, Eq, PartialEq)]
struct DcsDynamicNamespace {
    prefix: String,
    uri: String,
}

/// What to do with a `Start`/`End` pair that turns out, once its matching
/// `End` is reached, to have written no content in between (no text, no
/// child elements): the native platform does not always leave such an
/// element as an open/close pair with nothing inside.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum DcsEmptyElementAction {
    /// Omit the element -- both its opening and closing tags -- entirely.
    ///
    /// Evidenced on real ERP УХ 3.2.12.6 bytes: a `settingsVariant`'s
    /// `dcsset:outputParameters` is present in *storage* as an empty
    /// placeholder even when unset (unlike its sibling optional children
    /// `selection`/`filter`/`order`/`conditionalAppearance`, which storage
    /// omits outright when unset), but the platform's own decompiled
    /// source XML never carries an empty `<dcsset:outputParameters/>` --
    /// the element is missing altogether, matching doctrine point 6
    /// (default is not absence) applied in the other direction: an
    /// evidenced default omission must not be rendered as an explicit
    /// empty marker.
    OmitIfEmpty,
}

#[derive(Debug)]
struct DcsElementFrame {
    namespace: Option<Vec<u8>>,
    local: Vec<u8>,
    rendered_name: String,
    xsi_type_local: Option<String>,
    dynamic_namespaces: Vec<DcsDynamicNamespace>,
    is_data_ui_color_value: bool,
    /// The element's `xsi:type` is `{data/core}Type` or `{data/core}TypeSet`,
    /// which makes its character data a QName. A settings `right` value spells
    /// a type this way, where the element's own name says nothing about it.
    is_data_core_type_value: bool,
    output_namespace_offset: usize,
    /// See [`DcsEmptyElementAction`]. `None` for the vast majority of
    /// elements, which always keep whatever content (or lack of it) the
    /// storage document itself carried.
    empty_element_action: Option<DcsEmptyElementAction>,
    /// Whether a child this element carried in storage was dropped on the way
    /// out -- an `outputParameters` placeholder that `OmitIfEmpty` removed.
    ///
    /// What is left inside is then the storage pretty-printer's own
    /// indentation, which indents nothing any more. The platform reads the
    /// same storage into an object model where the element simply has no
    /// children and writes it self-closed; over the `Templates/*/Ext/
    /// Template.xml` trees of ERP УХ 3.2.12.6, 1С:УТ 11.5.27.75, БСП
    /// demo/base 3.1.12.297 and Документооборот КОРП 3.0.21.3 not one
    /// element is written as an open/close pair whose interior is only
    /// indentation. The pairs those trees do carry -- 171 `v8:content`,
    /// 3 `dcsset:name`, 1 `dcsset:title` -- hold character data that happens
    /// to be spaces or tabs, which is a value, not indentation, and none of
    /// them lost a child. So the rule is the dropped child, not the element
    /// name: `dcsset:settings`, the inline `settings` of a template
    /// envelope and a `dcsset:item` structure node all self-close by it.
    dropped_child: bool,
    /// Output offset right before this element's opening `<` was written.
    start_tag_begin_offset: usize,
    /// Output offset right after this element's opening tag's own closing
    /// `>` was written (i.e., before any content).
    start_tag_end_offset: usize,
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
    /// The document mode this writer was built for.
    ///
    /// The mode is also threaded through the write calls, which is how the
    /// existing paths read it; the field exists so the QName resolvers that
    /// sit below those calls can see it without re-threading a parameter
    /// through every caller.
    mode: DataCompositionDocumentMode,
}

impl<'a> DataCompositionXmlWriter<'a> {
    fn new(object_refs: &'a BTreeMap<String, String>) -> Self {
        Self::new_for_mode(object_refs, DataCompositionDocumentMode::Settings)
    }

    fn new_for_mode(
        object_refs: &'a BTreeMap<String, String>,
        mode: DataCompositionDocumentMode,
    ) -> Self {
        Self {
            output: String::new(),
            skip_depth: 0,
            element_stack: Vec::new(),
            object_refs,
            mode,
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
                        let start_tag_begin_offset = self.output.len();
                        let written_start = self.write_start_tag(
                            &reader,
                            &event,
                            namespace_ref(&namespace),
                            local,
                            false,
                            &mode,
                        )?;
                        let start_tag_end_offset = self.output.len();
                        let empty_element_action = if event_has_ordinary_attributes(&event)? {
                            None
                        } else {
                            dcs_empty_element_action(&written_start.rendered_name)
                        };
                        let mut frame = data_composition_element_frame(
                            &reader,
                            &event,
                            namespace_ref(&namespace),
                            local,
                            written_start,
                        )?;
                        frame.empty_element_action = empty_element_action;
                        frame.start_tag_begin_offset = start_tag_begin_offset;
                        frame.start_tag_end_offset = start_tag_end_offset;
                        self.element_stack.push(frame);
                    }
                }
                Event::Empty(event) => {
                    let (namespace, local) = reader.resolve_element(event.name());
                    if self.skip_depth == 0 {
                        // The platform's own decompiled source XML omits an
                        // empty `outputParameters` placeholder entirely
                        // rather than spelling it as a self-closed element;
                        // see `DcsEmptyElementAction::OmitIfEmpty`. Storage
                        // carries this shape whenever the element's source
                        // form was already self-closed instead of an
                        // open/close pair with nothing in between.
                        let omit = namespace_ref(&namespace) == Some(DCS_SETTINGS_NS)
                            && local.as_ref() == b"outputParameters"
                            && !event_has_ordinary_attributes(&event)?;
                        if omit {
                            // The immediately preceding `Event::Text` already
                            // wrote the indentation that led up to this now-
                            // omitted element; trimmed too, so no orphaned
                            // blank line is left for whatever follows.
                            let trimmed_len =
                                self.output.trim_end_matches(['\r', '\n', '\t', ' ']).len();
                            self.output.truncate(trimmed_len);
                            if let Some(parent) = self.element_stack.last_mut() {
                                parent.dropped_child = true;
                            }
                        } else {
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
                    // Whitespace-only, not strictly zero-length: the storage
                    // document's own pretty-printing writes indentation text
                    // nodes as siblings around a now-omitted child (or inside
                    // a childless element), and the platform's own source
                    // XML does not preserve that indentation once there is
                    // nothing left for it to indent.
                    let interior_is_blank = self.output[frame.start_tag_end_offset..]
                        .bytes()
                        .all(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n'));
                    let is_empty = frame.empty_element_action.is_some() && interior_is_blank;
                    match frame.empty_element_action {
                        Some(DcsEmptyElementAction::OmitIfEmpty) if is_empty => {
                            // As in the `Event::Empty` arm above, the
                            // indentation leading up to this element's own
                            // opening tag is trimmed too.
                            self.output.truncate(frame.start_tag_begin_offset);
                            let trimmed_len =
                                self.output.trim_end_matches(['\r', '\n', '\t', ' ']).len();
                            self.output.truncate(trimmed_len);
                            if let Some(parent) = self.element_stack.last_mut() {
                                parent.dropped_child = true;
                            }
                        }
                        _ if frame.dropped_child && interior_is_blank => {
                            // Every child this element carried was dropped and
                            // what is left inside is the indentation that led
                            // up to them -- see `DcsElementFrame::
                            // dropped_child`. Drop that interior and rewrite
                            // the already-written opening tag's trailing `>`
                            // into `/>` instead of appending a closing tag.
                            self.output.truncate(frame.start_tag_end_offset);
                            self.output.truncate(self.output.len() - 1);
                            self.output.push_str("/>");
                        }
                        _ => {
                            self.output.push_str("</");
                            self.output.push_str(&frame.rendered_name);
                            self.output.push('>');
                        }
                    }
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
        let is_form_list_settings_child_root = *mode
            == DataCompositionDocumentMode::FormListSettingsChild
            && self.element_stack.is_empty();
        let mut rendered_attributes = Vec::<(String, String)>::new();
        let mut dynamic_namespaces = Vec::<DcsDynamicNamespace>::new();
        if *mode == DataCompositionDocumentMode::FormListSettingsChild {
            // A generated prefix in a `ListSettings` child is declared at the
            // point of use and numbered by the element's depth in `Form.xml`,
            // not carried down from a document root. Mint it here, before any
            // attribute or character-data QName is rendered against it, so
            // both resolve through the frame's own declaration. More than one
            // undeclarable namespace on a single element has no evidenced
            // numbering, so it is refused rather than guessed.
            let mut minted = None::<String>;
            for attribute in event.attributes().with_checks(false) {
                let attribute = attribute.ok()?;
                if !is_xmlns_attribute(attribute.key.as_ref()) {
                    continue;
                }
                let uri = attribute
                    .decode_and_unescape_value(reader.decoder())
                    .ok()?
                    .into_owned();
                if uri.is_empty()
                    || form_root_declared_data_composition_prefix(uri.as_bytes()).is_some()
                {
                    continue;
                }
                if minted.replace(uri.clone()).is_some() {
                    return None;
                }
                self.push_dynamic_namespace(
                    &mut dynamic_namespaces,
                    self.form_list_settings_child_scope_prefix(),
                    uri,
                )?;
            }
        }
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
                canonical_data_composition_picture_ref(reader, &value)
                    // A picture the configuration owns is stored by its
                    // metadata uuid and published by its name under the
                    // globally declared `v8ui` prefix, exactly as a stored
                    // style-item reference is published under `style`. The
                    // uuid form is not a spelling the platform ever writes:
                    // `InformationRegisters/ЗначенияЭлементовКонструктора
                    // ВидовПродукцииИС/Templates/МакетКонструктораВидов
                    // ПродукцииИС` stores `ref="0:3febcafd-…"` and the
                    // platform exports `ref="v8ui:УспешнаяПроверка
                    // НаличияГосИС"`.
                    .or_else(|| {
                        data_composition_object_ref_name(&value, self.object_refs, "CommonPicture.")
                            .map(|name| format!("v8ui:{name}"))
                    })
                    .unwrap_or(value)
            } else if attr_name == "ref"
                && matches!(
                    *mode,
                    DataCompositionDocumentMode::FormListSettingsChild
                        | DataCompositionDocumentMode::Settings
                )
                && value.trim_start().starts_with("0:")
            {
                // A style-item reference the platform stored by metadata
                // UUID. Its inline spelling is the style item's own name
                // under the globally declared `style` prefix -- proven by
                // `CommonForms/МашиночитаемыеДоверенности`, whose
                // `ref="0:4a6c2c50-..."` is exported as
                // `ref="style:ЗачеркнутыйШрифтБЭД"`, and by the four UT
                // `ЭлементовКонструктораВидовПродукцииИС` templates, whose
                // standalone `Settings` documents carry the same storage
                // spelling and the same `style:ВажнаяНадписьШрифт` export.
                // Both positions declare `style` at their root, so the
                // prefix is in scope wherever the value lands. A UUID that
                // resolves to no style item is refused rather than passed
                // through, since the raw form is not a spelling the platform
                // ever writes.
                format!(
                    "style:{}",
                    data_composition_style_item_name(&value, self.object_refs)?
                )
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
        } else if is_form_list_settings_child_root {
            // The storage document names its root in the platform's storage
            // spelling (`Filter`, `Order`, `ConditionalAppearance`); the
            // inline source position names the same element in the settings
            // namespace with a lower-case initial (`dcsset:filter`,
            // `dcsset:order`, `dcsset:conditionalAppearance`). Everything
            // below the root is already spelled the source way in storage.
            if namespace != Some(DCS_SETTINGS_NS) {
                return None;
            }
            format!(
                "dcsset:{}",
                form_list_settings_child_source_local(std::str::from_utf8(local).ok()?)?
            )
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
            (frame.namespace.as_deref() == Some(DATA_CORE_NS)
                && matches!(frame.local.as_slice(), b"Type" | b"TypeSet"))
                // A `ListSettings` child spells a type as a settings element
                // carrying `xsi:type="v8:Type"`, so the element's own name is
                // not what makes its body a QName.
                || (*mode == DataCompositionDocumentMode::FormListSettingsChild
                    && frame.is_data_core_type_value)
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
            // A `v8:Type`/`v8:TypeSet` body is a QName. In a fragment spliced
            // into a document this writer does not own the root of, letting an
            // unresolved one through verbatim would emit a prefix nothing
            // declares, so it is refused instead.
            if *mode == DataCompositionDocumentMode::FormListSettingsChild
                && text.trim().contains(':')
            {
                return None;
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
            let qualified_form_value = mode
                .uses_form_root_prefixes()
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
                } else if mode.uses_form_root_prefixes() {
                    canonical_form_data_ui_value_prefix(&namespace).map(str::to_string)
                } else {
                    None
                };
                if let Some(prefix) = &prefix
                    && !mode.uses_form_root_prefixes()
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
            if *mode == DataCompositionDocumentMode::FormListSettingsChild && value.contains(':') {
                return None;
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
        if mode == DataCompositionDocumentMode::FormListSettingsChild {
            // An unprefixed attribute name carries no namespace and needs
            // none; every other name in this mode resolves through the Form
            // root's own declarations or through a prefix minted at the
            // element that declared it.
            if is_attribute && namespace.is_none() {
                return Some(DcsRenderedQName {
                    value: std::str::from_utf8(local).ok()?.to_string(),
                    declaration: None,
                });
            }
            return self.render_form_list_settings_child_qname(
                DcsExpandedQName {
                    namespace: Some(namespace?.to_vec()),
                    local: std::str::from_utf8(local).ok()?.to_string(),
                },
                local_namespaces,
            );
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

    /// Resolves one QName for a `ListSettings` child fragment.
    ///
    /// Exactly two sources are admitted: a prefix the decompiled `Form.xml`
    /// declares on its own root, and a prefix this writer minted at the
    /// element that declared the namespace. Anything else is refused rather
    /// than spelled with a prefix nothing declares -- a fragment is spliced
    /// into a document whose root the writer does not control, so an
    /// unresolvable namespace has no place to be declared.
    fn render_form_list_settings_child_qname(
        &self,
        expanded: DcsExpandedQName,
        local_namespaces: &[DcsDynamicNamespace],
    ) -> Option<DcsRenderedQName> {
        let namespace = expanded.namespace.as_deref()?;
        if let Some(prefix) = form_root_declared_data_composition_prefix(namespace) {
            return Some(DcsRenderedQName {
                value: format!("{prefix}:{}", expanded.local),
                declaration: None,
            });
        }
        let uri = std::str::from_utf8(namespace).ok()?;
        let prefix = self.dynamic_namespace_prefix_for_uri(uri, local_namespaces)?;
        Some(DcsRenderedQName {
            value: format!("{prefix}:{}", expanded.local),
            declaration: None,
        })
    }

    fn dynamic_namespace_prefix_for_uri(
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
    }

    fn render_expanded_qname(
        &self,
        expanded: DcsExpandedQName,
        element_local: &[u8],
    ) -> Option<DcsRenderedQName> {
        if self.mode == DataCompositionDocumentMode::FormListSettingsChild {
            return self.render_form_list_settings_child_qname(expanded, &[]);
        }
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

    /// The generated prefix for a namespace declared at the element currently
    /// being written, numbered by that element's depth in `Form.xml`.
    fn form_list_settings_child_scope_prefix(&self) -> String {
        format!(
            "d{}p1",
            FORM_LIST_SETTINGS_CHILD_ROOT_DEPTH + self.element_stack.len()
        )
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
    let mut is_data_core_type_value = false;
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
            is_data_core_type_value = expanded.namespace.as_deref() == Some(DATA_CORE_NS)
                && matches!(expanded.local.as_str(), "Type" | "TypeSet");
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
        is_data_core_type_value,
        output_namespace_offset: written_start.output_namespace_offset,
        // Set by `write_document`'s `Event::Start` arm, the only caller
        // that needs this; every other caller leaves it at the neutral
        // default (never collapsed on close).
        empty_element_action: None,
        dropped_child: false,
        start_tag_begin_offset: 0,
        start_tag_end_offset: 0,
    })
}

/// Whether this element's `Start`/`End` pair, if it turns out to hold no
/// content once its matching `End` is reached, should be omitted outright
/// instead of left as an empty open/close pair -- see
/// [`DcsEmptyElementAction`]. Keyed off the already-rendered name so it
/// shares the exact same identity the platform's own `dcsset`/`dcscor`
/// prefix resolution already settled on, rather than re-deriving it from
/// the source spelling.
fn dcs_empty_element_action(rendered_name: &str) -> Option<DcsEmptyElementAction> {
    match rendered_name {
        "dcsset:outputParameters" => Some(DcsEmptyElementAction::OmitIfEmpty),
        _ => None,
    }
}

/// Whether this start event carries any attribute other than a namespace
/// declaration. `OmitIfEmpty` applies only to the evidenced attribute-free
/// shape; an element carrying an attribute is left exactly as storage
/// spelled it.
fn event_has_ordinary_attributes(event: &quick_xml::events::BytesStart<'_>) -> Option<bool> {
    for attribute in event.attributes().with_checks(false) {
        let attribute = attribute.ok()?;
        if !is_xmlns_attribute(attribute.key.as_ref()) {
            return Some(true);
        }
    }
    Some(false)
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

/// The prefix a decompiled `Form.xml` declares on its own root element for
/// `namespace`, for the namespaces a `ListSettings` child can carry.
///
/// The set is exactly the vocabulary the platform's own export uses inside
/// `ListSettings`: surveying all 1 672 UT forms that carry one yields element
/// prefixes `dcsset`/`dcscor`/`v8`, `xsi:type` prefixes adding `v8ui`/`xs`,
/// and character-data QName prefixes `style`/`web` (plus generated ones,
/// which are minted at their point of use). Namespaces outside it -- the
/// schema, common, area-template, current-config and enterprise namespaces
/// among them -- are deliberately absent: no `ListSettings` in the corpus
/// carries one, so admitting them would be a guess about a spelling nothing
/// proves, and the writer refuses instead.
fn form_root_declared_data_composition_prefix(namespace: &[u8]) -> Option<&'static str> {
    matches!(
        namespace,
        DCS_CORE_NS
            | DCS_SETTINGS_NS
            | DATA_CORE_NS
            | DATA_UI_NS
            | STYLE_NS
            | SYS_NS
            | WEB_NS
            | WIN_NS
            | XSI_NS
            | XS_NS
    )
    .then(|| form_root_declared_prefix(namespace))
    .flatten()
}

/// The prefix a decompiled `Form.xml` declares on its own root element for
/// `namespace`, over the whole root declaration set rather than one position's
/// slice of it.
///
/// All 5 201 native UT 11.5.27.75 forms declare exactly the same seventeen
/// namespaces on `<Form>` -- one distinct declaration set across the corpus,
/// no variation at all -- so this is the entire table and not a sample. The
/// seventeenth is the default declaration, the logform namespace, which
/// carries no prefix and therefore no entry here.
///
/// [`form_root_declared_data_composition_prefix`] is this table restricted to
/// the vocabulary a `ListSettings` child is observed to carry, so the two can
/// never disagree about a namespace both admit.
fn form_root_declared_prefix(namespace: &[u8]) -> Option<&'static str> {
    match namespace {
        MANAGED_APPLICATION_CORE_NS => Some("app"),
        CURRENT_CONFIG_NS => Some("cfg"),
        DCS_CORE_NS => Some("dcscor"),
        DCS_SCHEMA_NS => Some("dcssch"),
        DCS_SETTINGS_NS => Some("dcsset"),
        ENTERPRISE_NS => Some("ent"),
        MANAGED_APPLICATION_LOGFORM_NS => Some("lf"),
        STYLE_NS => Some("style"),
        SYS_NS => Some("sys"),
        DATA_CORE_NS => Some("v8"),
        DATA_UI_NS => Some("v8ui"),
        WEB_NS => Some("web"),
        WIN_NS => Some("win"),
        XCF_READABLE_NS => Some("xr"),
        XS_NS => Some("xs"),
        XSI_NS => Some("xsi"),
        _ => None,
    }
}

/// Depth of a dynamic list's `<Settings>` element in a decompiled `Form.xml`:
/// `Form`(1) / `Attributes`(2) / `Attribute`(3) / `Settings`(4).
///
/// The `ServerState` blob's own root is spelled at exactly that element, so a
/// generated prefix minted inside the blob counts its depth from here. This is
/// the same chain [`FORM_LIST_SETTINGS_CHILD_ROOT_DEPTH`] already measures --
/// that one continues through `ListSettings`(5) to the child root(6) -- and is
/// derived from it rather than restated, so the two cannot drift apart.
const FORM_SERVER_STATE_ROOT_DEPTH: usize = FORM_LIST_SETTINGS_CHILD_ROOT_DEPTH - 2;

/// One namespace declaration in scope while a `ServerState` blob is re-spelled.
#[derive(Debug, Clone)]
struct FormServerStateNamespace {
    /// The prefix the blob spells the namespace with; empty for a default
    /// declaration.
    prefix: String,
    uri: String,
    /// The prefix the output keeps the declaration under, when it keeps it at
    /// all. `None` means the `Form` root already declares this namespace, so
    /// the declaration is dropped and every QName bound to it renders against
    /// the root's own prefix.
    minted: Option<String>,
}

/// One open element while a `ServerState` blob is re-spelled.
#[derive(Debug)]
struct FormServerStateFrame {
    namespaces: Vec<FormServerStateNamespace>,
    /// Expanded name of the element, which together with its `xsi:type` is
    /// what says whether its character data is a QName.
    namespace: String,
    local: String,
    xsi_type: Option<(String, String)>,
    /// The name the blob's own start tag spells, which its end tag must repeat.
    blob_name: String,
    /// The name its end tag is written with.
    close_name: String,
}

/// Re-spells the generated namespace prefixes of a dynamic list's decoded
/// `ServerState` blob for the position the blob's content is inlined at.
///
/// The blob is a standalone XML document, so the platform serializes it with
/// its own declarations: every namespace it needs is declared at the element
/// that first uses it, under a prefix numbered `d<depth>p<index>` by that
/// element's depth *in the blob*. The inline position is `Form.xml`, whose
/// root already declares seventeen namespaces (see
/// [`form_root_declared_prefix`]), so the platform's own export writes the
/// same content differently: a declaration whose namespace the root already
/// carries is dropped and its QNames re-spelled with the root's prefix, and
/// one the root does not carry keeps a generated prefix renumbered by the
/// element's depth in `Form.xml`.
///
/// Both halves are pinned by the corpus. The 657 distinct `ServerState` blobs
/// of UT 11.5.27.75 carry 32 332 prefixed declarations over seven namespaces --
/// 26 640 schema, 4 370 core, 657 each of `xs` and `xsi`, 6 settings, one ui,
/// one `http://v8.1c.ru/8.2/data/types` -- and exactly the last of those is
/// outside the `Form` root's set. The platform writes that one as `d6p1`, and
/// the element it declares it on sits at `Form`/`Attributes`/`Attribute`/
/// `Settings`/`Parameter`/`value`, depth six. Its blob-side neighbour, a core
/// declaration written before it and dropped here, does not consume the index:
/// the index counts the declarations actually written, exactly as the storage
/// side numbers the ones it writes.
///
/// Character data is re-spelled on the same rule wherever it is a QName, which
/// is where the element is `{data/core}Type`/`TypeSet` or carries an
/// `xsi:type` naming one. All 510 such bodies in the corpus are covered: 508
/// already spell a root-declared prefix and are returned unchanged, and the
/// two that spell none resolve through the default declaration their own
/// element carries.
///
/// Default declarations are left exactly as the blob wrote them: they bind
/// element names this function does not rename, and the caller's own rewrite
/// owns that spelling.
///
/// Returns `None`, changing nothing, for any blob this does not fully account
/// for -- a prefix declared nowhere in scope, a namespace the root does not
/// declare appearing where no prefix can be minted, a markup construct outside
/// elements and character data, or a start tag this cannot re-render
/// byte-for-byte from its own parse.
pub(crate) fn respell_form_server_state_namespaces(
    root_open_tag: &str,
    inner: &str,
) -> Option<String> {
    let root = scan_form_server_state_start_tag(root_open_tag)?;
    let mut root_namespaces = Vec::new();
    for (name, value, _quote) in &root.attributes {
        let Some(prefix) = form_server_state_declared_prefix(name) else {
            continue;
        };
        if !value.is_empty() && form_root_declared_prefix(value.as_bytes()).is_none() {
            // The blob's root declares a namespace the `Form` root does not,
            // and the blob root is not written at all in the inline position,
            // so there is nowhere to mint a replacement for it.
            return None;
        }
        root_namespaces.push(FormServerStateNamespace {
            prefix: prefix.to_owned(),
            uri: (*value).to_owned(),
            minted: None,
        });
    }
    let mut stack = vec![FormServerStateFrame {
        namespaces: root_namespaces,
        namespace: String::new(),
        local: String::new(),
        xsi_type: None,
        blob_name: root.name.to_owned(),
        close_name: String::new(),
    }];
    let mut output = String::with_capacity(inner.len());
    let mut cursor = 0usize;
    while cursor < inner.len() {
        let Some(relative) = inner[cursor..].find('<') else {
            output.push_str(&inner[cursor..]);
            break;
        };
        let start = cursor + relative;
        output.push_str(&form_server_state_rendered_text(
            &stack,
            &inner[cursor..start],
        )?);
        let end = form_server_state_tag_end(inner, start)?;
        let tag = &inner[start..end];
        if tag.starts_with("</") {
            if stack.len() < 2 {
                return None;
            }
            let frame = stack.pop()?;
            if tag.strip_prefix("</")?.strip_suffix('>')?.trim() != frame.blob_name {
                return None;
            }
            output.push_str("</");
            output.push_str(&frame.close_name);
            output.push('>');
        } else if tag.starts_with("<!") || tag.starts_with("<?") {
            return None;
        } else {
            output.push_str(&form_server_state_rendered_start_tag(&mut stack, tag)?);
        }
        cursor = end;
    }
    (stack.len() == 1).then_some(output)
}

/// The prefix an `xmlns` attribute declares: the empty string for the default
/// declaration, the part after the colon for a prefixed one, `None` for an
/// attribute that declares nothing.
fn form_server_state_declared_prefix(name: &str) -> Option<&str> {
    if name == "xmlns" {
        return Some("");
    }
    name.strip_prefix("xmlns:")
        .filter(|prefix| !prefix.is_empty())
}

/// The namespace a prefix is bound to at this point of the walk. The empty
/// prefix falls back to no namespace when nothing declares a default.
fn form_server_state_namespace_for_prefix(
    stack: &[FormServerStateFrame],
    prefix: &str,
) -> Option<String> {
    stack
        .iter()
        .rev()
        .flat_map(|frame| frame.namespaces.iter().rev())
        .find(|namespace| namespace.prefix == prefix)
        .map(|namespace| namespace.uri.clone())
        .or_else(|| prefix.is_empty().then(String::new))
}

/// The prefix the output spells a namespace with: the `Form` root's own where
/// the root declares it, otherwise the one minted in scope for it.
fn form_server_state_rendered_prefix(stack: &[FormServerStateFrame], uri: &str) -> Option<String> {
    if uri.is_empty() {
        return Some(String::new());
    }
    if let Some(prefix) = form_root_declared_prefix(uri.as_bytes()) {
        return Some(prefix.to_owned());
    }
    stack
        .iter()
        .rev()
        .flat_map(|frame| frame.namespaces.iter().rev())
        .find(|namespace| namespace.uri == uri && namespace.minted.is_some())
        .and_then(|namespace| namespace.minted.clone())
}

/// Re-spells one QName -- an element or attribute name, an `xsi:type`, or a
/// QName-valued body -- from the blob's namespace context into the `Form`
/// document's.
fn form_server_state_render_qname(stack: &[FormServerStateFrame], value: &str) -> Option<String> {
    let (prefix, local) = value.split_once(':').unwrap_or(("", value));
    if local.is_empty() || local.contains(':') {
        return None;
    }
    let uri = form_server_state_namespace_for_prefix(stack, prefix)?;
    let rendered = form_server_state_rendered_prefix(stack, &uri)?;
    Some(if rendered.is_empty() {
        local.to_owned()
    } else {
        format!("{rendered}:{local}")
    })
}

/// The expanded name a QName denotes, without re-spelling it.
fn form_server_state_expand_qname(
    stack: &[FormServerStateFrame],
    value: &str,
) -> Option<(String, String)> {
    let (prefix, local) = value.split_once(':').unwrap_or(("", value));
    if local.is_empty() || local.contains(':') {
        return None;
    }
    Some((
        form_server_state_namespace_for_prefix(stack, prefix)?,
        local.to_owned(),
    ))
}

/// Character data, re-spelled where the enclosing element makes it a QName.
///
/// A `{data/core}Type`/`TypeSet` element, and any element whose `xsi:type`
/// names one, carries a type QName as its body; everything else is copied
/// exactly as the blob wrote it.
fn form_server_state_rendered_text(stack: &[FormServerStateFrame], text: &str) -> Option<String> {
    let is_qname = stack.last().is_some_and(|frame| {
        let names_a_type = |namespace: &str, local: &str| {
            namespace.as_bytes() == DATA_CORE_NS && matches!(local, "Type" | "TypeSet")
        };
        names_a_type(&frame.namespace, &frame.local)
            || frame
                .xsi_type
                .as_ref()
                .is_some_and(|(namespace, local)| names_a_type(namespace, local))
    });
    let value = text.trim();
    if !is_qname || value.is_empty() {
        return Some(text.to_owned());
    }
    let rendered = form_server_state_render_qname(stack, value)?;
    let value_start = text.find(value)?;
    Some(format!(
        "{}{rendered}{}",
        &text[..value_start],
        &text[value_start + value.len()..]
    ))
}

/// Re-spells one start tag and pushes its frame, popping it again when the tag
/// is self-closing.
fn form_server_state_rendered_start_tag(
    stack: &mut Vec<FormServerStateFrame>,
    tag: &str,
) -> Option<String> {
    let parsed = scan_form_server_state_start_tag(tag)?;
    // The tag is re-rendered from this parse, so a tag the parse cannot
    // reproduce byte-for-byte is refused rather than silently reformatted.
    if form_server_state_start_tag_text(
        parsed.name,
        parsed
            .attributes
            .iter()
            .map(|(name, value, quote)| ((*name).to_owned(), (*value).to_owned(), *quote)),
        parsed.empty,
    ) != tag
    {
        return None;
    }
    let form_depth = FORM_SERVER_STATE_ROOT_DEPTH.checked_add(stack.len())?;
    let mut namespaces = Vec::new();
    let mut minted = 0usize;
    for (name, value, _quote) in &parsed.attributes {
        let Some(prefix) = form_server_state_declared_prefix(name) else {
            continue;
        };
        let kept = if prefix.is_empty() || value.is_empty() {
            // A default declaration binds element names this pass does not
            // rename, so it is carried through untouched.
            None
        } else if form_root_declared_prefix(value.as_bytes()).is_some() {
            None
        } else {
            minted = minted.checked_add(1)?;
            Some(format!("d{form_depth}p{minted}"))
        };
        namespaces.push(FormServerStateNamespace {
            prefix: prefix.to_owned(),
            uri: (*value).to_owned(),
            minted: kept,
        });
    }
    stack.push(FormServerStateFrame {
        namespaces,
        namespace: String::new(),
        local: String::new(),
        xsi_type: None,
        blob_name: parsed.name.to_owned(),
        close_name: String::new(),
    });
    let rendered_name = if parsed.name.contains(':') {
        form_server_state_render_qname(stack, parsed.name)?
    } else {
        parsed.name.to_owned()
    };
    let (namespace, local) = form_server_state_expand_qname(stack, parsed.name)?;
    let mut attributes = Vec::new();
    let mut xsi_type = None;
    for (name, value, quote) in &parsed.attributes {
        if let Some(prefix) = form_server_state_declared_prefix(name) {
            let declaration = stack
                .last()?
                .namespaces
                .iter()
                .find(|namespace| namespace.prefix == prefix)?;
            match (&declaration.minted, prefix.is_empty()) {
                (Some(minted), _) => {
                    attributes.push((format!("xmlns:{minted}"), (*value).to_owned(), *quote));
                }
                (None, true) => {
                    attributes.push(((*name).to_owned(), (*value).to_owned(), *quote));
                }
                (None, false) => {}
            }
            continue;
        }
        let rendered_attribute = if name.contains(':') {
            form_server_state_render_qname(stack, name)?
        } else {
            (*name).to_owned()
        };
        let names_xsi_type = form_server_state_expand_qname(stack, name)
            .is_some_and(|(namespace, local)| namespace.as_bytes() == XSI_NS && local == "type");
        let rendered_value = if names_xsi_type {
            xsi_type = form_server_state_expand_qname(stack, value);
            form_server_state_render_qname(stack, value)?
        } else {
            (*value).to_owned()
        };
        attributes.push((rendered_attribute, rendered_value, *quote));
    }
    let frame = stack.last_mut()?;
    frame.namespace = namespace;
    frame.local = local;
    frame.xsi_type = xsi_type;
    frame.close_name = rendered_name.clone();
    let text = form_server_state_start_tag_text(&rendered_name, attributes, parsed.empty);
    if parsed.empty {
        stack.pop();
    }
    Some(text)
}

/// Writes a start tag back out from a name, its attributes and whether it is
/// self-closing.
fn form_server_state_start_tag_text(
    name: &str,
    attributes: impl IntoIterator<Item = (String, String, char)>,
    empty: bool,
) -> String {
    let mut text = String::from("<");
    text.push_str(name);
    for (attribute, value, quote) in attributes {
        text.push(' ');
        text.push_str(&attribute);
        text.push('=');
        text.push(quote);
        text.push_str(&value);
        text.push(quote);
    }
    text.push_str(if empty { "/>" } else { ">" });
    text
}

/// One parsed start tag of a `ServerState` blob.
struct FormServerStateStartTag<'a> {
    name: &'a str,
    /// Attribute name, raw (still escaped) value and the quote it was written
    /// with, in document order.
    attributes: Vec<(&'a str, &'a str, char)>,
    empty: bool,
}

fn scan_form_server_state_start_tag(tag: &str) -> Option<FormServerStateStartTag<'_>> {
    let body = tag.strip_prefix('<')?.strip_suffix('>')?;
    let (body, empty) = match body.strip_suffix('/') {
        Some(rest) => (rest, true),
        None => (body, false),
    };
    let name_end = body
        .find(|character: char| character.is_ascii_whitespace())
        .unwrap_or(body.len());
    let name = &body[..name_end];
    if name.is_empty() || name.starts_with('/') {
        return None;
    }
    let mut rest = &body[name_end..];
    let mut attributes = Vec::new();
    loop {
        let trimmed = rest.trim_start();
        if trimmed.is_empty() {
            break;
        }
        let equals = trimmed.find('=')?;
        let attribute = trimmed[..equals].trim_end();
        if attribute.is_empty() || attribute.contains(char::is_whitespace) {
            return None;
        }
        let after = trimmed[equals + 1..].trim_start();
        let quote = after.chars().next()?;
        if quote != '"' && quote != '\'' {
            return None;
        }
        let value_end = after[1..].find(quote)? + 1;
        attributes.push((attribute, &after[1..value_end], quote));
        rest = &after[value_end + 1..];
    }
    Some(FormServerStateStartTag {
        name,
        attributes,
        empty,
    })
}

/// End offset of the tag that starts at `start`, quote-aware so a `>` inside
/// an attribute value cannot close it early.
fn form_server_state_tag_end(text: &str, start: usize) -> Option<usize> {
    let mut quote: Option<u8> = None;
    for (offset, byte) in text.as_bytes().get(start..)?.iter().copied().enumerate() {
        match (quote, byte) {
            (Some(open), byte) if byte == open => quote = None,
            (Some(_), _) => {}
            (None, byte @ (b'"' | b'\'')) => quote = Some(byte),
            (None, b'>') => return Some(start + offset + 1),
            (None, _) => {}
        }
    }
    None
}

/// The inline source name of a `ListSettings` child root, given the local name
/// its storage document uses.
///
/// Four of the five children differ from their storage spelling in the initial
/// character only. `DataParameterValues` is the exception: storage names the
/// document by the collection it holds, the inline position names the settings
/// member the collection fills. Evidence: UT 11.5.27.75, the 3 native
/// `<ListSettings>` blocks carrying a `dcsset:dataParameters` all store a
/// document rooted `<DataParameterValues>` in the settings namespace, and no
/// other storage root in the corpus maps across a name change.
fn form_list_settings_child_source_local(local: &str) -> Option<String> {
    if local == "DataParameterValues" {
        return Some("dataParameters".to_string());
    }
    lower_camel_data_composition_local(local)
}

/// Lowercases the first character of a storage-spelled element local name.
///
/// The three `ListSettings` child roots are the only names this is applied
/// to, and each differs from its inline source spelling in exactly that
/// character: `Filter`/`filter`, `Order`/`order`,
/// `ConditionalAppearance`/`conditionalAppearance`.
fn lower_camel_data_composition_local(local: &str) -> Option<String> {
    let mut chars = local.chars();
    let first = chars.next()?;
    if !first.is_ascii_uppercase() {
        return None;
    }
    Some(format!("{}{}", first.to_ascii_lowercase(), chars.as_str()))
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
                transliterate: inner("probe"),
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
        let error = canonicalize_typed_data_composition_settings_document(
            settings,
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .expect_err("an unparsable order child must be refused by the typed step");
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
        // Contract change, named rather than hidden: the typed refusal above
        // is unchanged and still names the child, but it is no longer the
        // last word. The document is then accounted for from its own bytes,
        // exactly as the primary schema is.
        let canonical = canonicalize_data_composition_settings_document(
            settings,
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .expect("a typed refusal hands the document to the transliteration");
        assert!(
            matches!(canonical, CanonicalDcsSettingsDocument::Transliterated(_)),
            "the typed cohort must not claim a document it refused"
        );
        assert!(
            canonical
                .as_str()
                .contains("<dcsset:order><dcsset:probe/></dcsset:order>"),
            "the refused child must survive the rewrite verbatim: {}",
            canonical.as_str()
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
        // This shape is inside the typed cohort, so it must still take the
        // typed path -- the transliteration is a fallback, not a takeover.
        assert!(
            matches!(canonical_settings, CanonicalDcsSettingsDocument::Typed(_)),
            "a cohort shape must not reach the transliteration"
        );
        let canonical_settings = canonical_settings.as_str();
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

    /// A real production `DataSetQuery` schema -- the shape no closed typed
    /// cohort describes and that the general storage-to-source
    /// transliteration exists for.
    ///
    /// Provenance (`manifest.json` in the fixture directory): storage element
    /// `20db535c-d9a7-4a81-98b5-06295e8f518d.0` of 1C:Trade Management
    /// 11.5.27.75's `1cv8.cf`, packed body sha256
    /// `5df7cb6cb94efb1c7ef3e0ea1405553bd2cdf2895cc35ceacc31e7bf9130319f`;
    /// the expectation is the platform's own
    /// `DataProcessors/УправлениеВыгрузкамиВБидзаар/Templates/УсловияОтбораНоменклатуры/Ext/Template.xml`
    /// from an `ibcmd config export` capture with 1C:Enterprise 8.3.27.2214,
    /// sha256 `1d04899ee7fe61cddaa3efb51892d91fcea94db481db34e3ecaf0a2bfa3fc6b7`.
    #[test]
    fn platform_query_data_set_body_exports_byte_exact() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-ut-query-data-set/raw-packed.bin.b64"
        )));
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-ut-query-data-set/native-template.xml.b64"
        )));
        assert_eq!(
            format!("{:x}", Sha256::digest(&packed)),
            "5df7cb6cb94efb1c7ef3e0ea1405553bd2cdf2895cc35ceacc31e7bf9130319f"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&expected)),
            "1d04899ee7fe61cddaa3efb51892d91fcea94db481db34e3ecaf0a2bfa3fc6b7"
        );

        let body = crate::compiler::bodies::dcs::decode_compatible_dcs(
            crate::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .expect("platform-attested DCS body must decode");
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .expect("a production DataSetQuery schema must export through the live codec");

        assert_eq!(actual, expected);
    }

    /// The empty `<dcsset:settings .../>` variant: its inline fragment used to
    /// be spliced as `.../ xmlns:dcsset="..."...>`, which is not well-formed,
    /// so our own analyzer rejected our own output.
    ///
    /// Provenance (`manifest.json` in the fixture directory): storage element
    /// `15f6a89d-53a1-47f7-967e-973c5966caf8.0` of 1C:Trade Management
    /// 11.5.27.75's `1cv8.cf`, packed body sha256
    /// `a7e61a3156c7a2fa3302f7f77277d7f1a64abda10eb0519292bf4a1829a6f169`;
    /// the expectation is the platform's own
    /// `Reports/РезультатыТестирования/Templates/Макет/Ext/Template.xml`
    /// from an `ibcmd config export` capture with 1C:Enterprise 8.3.27.2214,
    /// sha256 `c1b1771520a0c8153e3c2f8d8381fa90f02c78338dc9e936eaef68caf2dffe67`.
    #[test]
    fn platform_empty_inline_settings_body_exports_byte_exact() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-ut-empty-inline-settings/raw-packed.bin.b64"
        )));
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-ut-empty-inline-settings/native-template.xml.b64"
        )));
        assert_eq!(
            format!("{:x}", Sha256::digest(&packed)),
            "a7e61a3156c7a2fa3302f7f77277d7f1a64abda10eb0519292bf4a1829a6f169"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&expected)),
            "c1b1771520a0c8153e3c2f8d8381fa90f02c78338dc9e936eaef68caf2dffe67"
        );

        let body = crate::compiler::bodies::dcs::decode_compatible_dcs(
            crate::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .expect("platform-attested DCS body must decode");
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .expect("an empty settings variant must export through the live codec");

        assert_eq!(actual, expected);
    }

    /// The terminal `SchemaFile` carrying a real report's area template, with
    /// its table cell's appearance held in the envelope's side table and
    /// selected by ordinal.
    ///
    /// Storage writes `<dcsat:appIndex>0</dcsat:appIndex>` inside the cell and
    /// a `<appearance xmlns="...area-template" xsi:type="TableCellAppearance">`
    /// child of `SchemaFile` itself; the source document writes that child's
    /// items inline as `<dcsat:appearance>` -- no ordinal, no discriminator,
    /// and no declaration, because `dcsat` is already bound by the
    /// `AreaTemplate` element the fragment sits under. This is the join and
    /// the inherited-prefix rule in their simplest form.
    ///
    /// Provenance (`manifest.json` in the fixture directory): storage element
    /// `2bf9c338-d31e-467e-8281-d1d6a3a6b2e2.0` of 1C:Trade Management
    /// 11.5.27.75's `1cv8.cf`, packed body sha256
    /// `f26cb51838fbdd3d1ab60381ce694d7cecb61e034793bd4c11aaf48dd149b734`;
    /// the expectation is the platform's own
    /// `Reports/ДлительностьОтложенногоОбновления/Templates/ОсновнаяСхемаКомпоновкиДанных/Ext/Template.xml`
    /// from an `ibcmd config export` capture with 1C:Enterprise 8.3.27.2214,
    /// sha256 `8cb4cddc8ecc053d74cf349ba3d4f78fe2d6f080ea966a9a3488184b2a2360a2`.
    #[test]
    fn platform_terminal_area_template_appearance_index_exports_byte_exact() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-ut-area-template-appearance-index/raw-packed.bin.b64"
        )));
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-ut-area-template-appearance-index/native-template.xml.b64"
        )));
        assert_eq!(
            format!("{:x}", Sha256::digest(&packed)),
            "f26cb51838fbdd3d1ab60381ce694d7cecb61e034793bd4c11aaf48dd149b734"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&expected)),
            "8cb4cddc8ecc053d74cf349ba3d4f78fe2d6f080ea966a9a3488184b2a2360a2"
        );

        let body = crate::compiler::bodies::dcs::decode_compatible_dcs(
            crate::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .expect("platform-attested DCS body must decode");
        let terminal = body.documents()[body.documents().len() - 1];
        let terminal = std::str::from_utf8(terminal).expect("the terminal document is UTF-8");
        assert!(
            terminal.contains("<dcsat:appIndex>0</dcsat:appIndex>")
                && terminal.contains(
                    "<appearance xmlns=\"http://v8.1c.ru/8.1/data-composition-system/\
                     area-template\" xsi:type=\"TableCellAppearance\">"
                ),
            "the fixture is the out-of-line appearance shape this exercises"
        );

        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .expect("a template-carrying terminal must export through the live codec");

        assert_eq!(actual, expected);
        let actual = String::from_utf8(actual).expect("the source document is UTF-8");
        assert!(
            !actual.contains("appIndex") && actual.contains("<dcsat:appearance>"),
            "the ordinal is resolved away and the appearance written inline"
        );
    }

    /// The same join where the appearance carries typed values, so both prefix
    /// rules the source direction applies inside it are visible at once.
    ///
    /// The storage `<value xmlns:d4p1="http://v8.1c.ru/8.1/data/ui" ...>` binds
    /// a generated prefix to a namespace the source root does declare, so the
    /// declaration is dropped and the value moves onto `v8ui`; a generated
    /// prefix bound to one the root does *not* declare is renumbered by the
    /// element's depth in the target document, not the storage one. Two
    /// templates select two different side-table entries by ordinal, and the
    /// schema's `fieldTemplate`/`groupTemplate` children ride along in
    /// document order.
    ///
    /// Provenance (`manifest.json` in the fixture directory): storage element
    /// `cc43c5a3-5e72-4f37-9e1a-016c70f94d85.0` of 1C:Trade Management
    /// 11.5.27.75's `1cv8.cf`, packed body sha256
    /// `fc95f8e0bb82f09f15a741c2e5bd2a6755448a3c7abe17b9815ebe0d7c76bdd2`;
    /// the expectation is the platform's own
    /// `InformationRegisters/МестаПримененияЭлементовКонструктораВидовПродукцииИС/Templates/МакетНастроекМестПрименения/Ext/Template.xml`
    /// from an `ibcmd config export` capture with 1C:Enterprise 8.3.27.2214,
    /// sha256 `d2f420275b235890de29152125610f97bc995114e98c62a834bd7e54201efbde`.
    #[test]
    fn platform_terminal_area_template_style_prefixes_export_byte_exact() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-ut-area-template-style-prefixes/raw-packed.bin.b64"
        )));
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-ut-area-template-style-prefixes/native-template.xml.b64"
        )));
        assert_eq!(
            format!("{:x}", Sha256::digest(&packed)),
            "fc95f8e0bb82f09f15a741c2e5bd2a6755448a3c7abe17b9815ebe0d7c76bdd2"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&expected)),
            "d2f420275b235890de29152125610f97bc995114e98c62a834bd7e54201efbde"
        );

        let body = crate::compiler::bodies::dcs::decode_compatible_dcs(
            crate::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .expect("platform-attested DCS body must decode");
        // The style item this configuration's Settings document names by uuid.
        let object_refs = BTreeMap::from([(
            "fa2a9ef2-00a1-44f4-a82c-6c7288dd62dc".to_string(),
            "StyleItem.ВажнаяНадписьШрифт".to_string(),
        )]);
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &BTreeMap::new(),
            &object_refs,
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .expect("two ordinals into the side table must export through the live codec");

        assert_eq!(actual, expected);
        let rendered = String::from_utf8(actual).expect("the source document is UTF-8");
        assert!(
            rendered.contains("<dcscor:value xsi:type=\"v8ui:VerticalAlign\">Bottom"),
            "a generated prefix bound to a root-declared namespace collapses \
             onto the root's own and loses its declaration"
        );
        assert!(
            rendered
                .contains("<dcscor:value xsi:type=\"dcscor:DataCompositionTextPlacementType\">"),
            "an unprefixed xsi:type picks up the prefix of the default \
             namespace it resolved through"
        );
        assert_eq!(
            rendered.matches("<dcsat:appearance>").count(),
            2,
            "both ordinals resolved into their own inline appearance"
        );
    }

    /// Everything about the side table that the platform's own bytes do not
    /// account for is refused rather than guessed.
    ///
    /// The join is by ordinal and nothing else, so an ordinal that names no
    /// entry has no appearance to inline; an entry no ordinal names would be
    /// dropped, since the source document has no side table to put it in, and
    /// dropping stored content is exactly the silent wrongness this direction
    /// exists to avoid; and an entry whose discriminator is not the evidenced
    /// `TableCellAppearance` is a shape whose inline spelling nothing here has
    /// seen. All three keep the fragment out of the export entirely.
    #[test]
    fn terminal_side_table_shapes_without_evidence_fail_closed() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-ut-area-template-appearance-index/raw-packed.bin.b64"
        )));
        let body = crate::compiler::bodies::dcs::decode_compatible_dcs(
            crate::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .expect("platform-attested DCS body must decode");
        let documents = body.documents();
        let terminal = std::str::from_utf8(documents[documents.len() - 1])
            .expect("the terminal document is UTF-8");

        for (what, mutated) in [
            (
                "an ordinal past the end of the side table",
                terminal.replace(
                    "<dcsat:appIndex>0</dcsat:appIndex>",
                    "<dcsat:appIndex>1</dcsat:appIndex>",
                ),
            ),
            (
                "a side-table entry no table cell selects",
                terminal.replace("\t\t\t\t\t\t<dcsat:appIndex>0</dcsat:appIndex>\r\n", ""),
            ),
            (
                "a discriminator beyond TableCellAppearance",
                terminal.replace(
                    "xsi:type=\"TableCellAppearance\"",
                    "xsi:type=\"FutureAppearance\"",
                ),
            ),
        ] {
            assert_ne!(mutated, terminal, "{what}: the mutation must bite");
            let mut mutated_documents = documents.clone();
            let last = mutated_documents.len() - 1;
            mutated_documents[last] = mutated.as_bytes();
            let error = normalize_data_composition_schema_template_documents_with_profiles(
                &mutated_documents,
                &BTreeMap::new(),
                &BTreeMap::new(),
                &ProfileId::parse("provider:mssql-legacy").unwrap(),
                &ProfileId::parse("xml-2.20").unwrap(),
            )
            .err()
            .unwrap_or_else(|| panic!("{what} must not export"));
            assert_eq!(
                error.class(),
                MetadataSourceFailureClass::Unsupported,
                "{what}: the refusal is `no evidence describes this`"
            );
        }
    }

    /// A `{data/ui}Font`'s `ref` attribute naming a configuration `StyleItem`
    /// by storage uuid, in both places this configuration writes one: inside
    /// the terminal document's inlined appearance and inside a standalone
    /// `Settings` document's own.
    ///
    /// The platform pre-declares `xmlns:style` on the very element carrying
    /// the reference in the first position and inherits the settings root's
    /// declaration in the second, and writes `ref="style:<Name>"` in both.
    /// A uuid the object-reference index cannot name is refused rather than
    /// written in its `0:<uuid>` storage spelling, which the platform never
    /// produces.
    ///
    /// Provenance (`manifest.json` in the fixture directory): storage element
    /// `e0ef648e-3e13-4aba-bd7f-206fbb42572d.0` of 1C:Trade Management
    /// 11.5.27.75's `1cv8.cf`, packed body sha256
    /// `31b6cd7d91e8314f2f3614ba4daa94b1007d092459eb78dbcd0f344984679084`;
    /// the expectation is the platform's own
    /// `InformationRegisters/ДоступныеЗначенияЭлементовКонструктораВидовПродукцииИС/Templates/МакетНастроекДоступныхЗначений/Ext/Template.xml`
    /// from an `ibcmd config export` capture with 1C:Enterprise 8.3.27.2214,
    /// sha256 `0d1894d8624dd00fd566afcc37d6816cf71dd223f4124f1cbcff97502c200082`.
    #[test]
    fn platform_terminal_area_template_style_item_ref_exports_byte_exact() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-ut-area-template-style-item-ref/raw-packed.bin.b64"
        )));
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-ut-area-template-style-item-ref/native-template.xml.b64"
        )));
        assert_eq!(
            format!("{:x}", Sha256::digest(&packed)),
            "31b6cd7d91e8314f2f3614ba4daa94b1007d092459eb78dbcd0f344984679084"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&expected)),
            "0d1894d8624dd00fd566afcc37d6816cf71dd223f4124f1cbcff97502c200082"
        );

        let body = crate::compiler::bodies::dcs::decode_compatible_dcs(
            crate::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .expect("platform-attested DCS body must decode");
        // The two style items this template's documents name by uuid: one on
        // a `Font`'s `ref` attribute, one as a `Color` value's character data.
        let object_refs = BTreeMap::from([
            (
                "fa2a9ef2-00a1-44f4-a82c-6c7288dd62dc".to_string(),
                "StyleItem.ВажнаяНадписьШрифт".to_string(),
            ),
            (
                "fe63db47-3ece-4518-b6a2-0b6499f60a61".to_string(),
                "StyleItem.ТекстЗапрещеннойЯчейкиЦвет".to_string(),
            ),
        ]);
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &BTreeMap::new(),
            &object_refs,
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .expect("a named style item must export through the live codec");

        assert_eq!(actual, expected);
        let rendered = String::from_utf8(actual).expect("the source document is UTF-8");
        assert_eq!(
            rendered.matches("ref=\"style:ВажнаяНадписьШрифт\"").count(),
            2,
            "both the inlined appearance and the Settings document name it"
        );
        assert!(
            !rendered.contains("0:fa2a9ef2"),
            "the storage uuid spelling never reaches the source document"
        );

        // Without the name, both positions refuse rather than fall back to the
        // storage spelling.
        let error = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .expect_err("an unnamed style item has no source spelling");
        assert_eq!(error.class(), MetadataSourceFailureClass::Unsupported);
    }

    /// Four external Settings documents in one envelope -- twice the count the
    /// clean-room corpus could exhibit, and the shape the attested-range gate
    /// refused outright. The header declares the count and the framing formula
    /// is uniform in it; this pins that reading it beats enumerating it, on
    /// the platform's own bytes.
    ///
    /// Provenance (`manifest.json` in the fixture directory): storage element
    /// `542a53f5-56c5-4ca9-90d1-d689532c69fd.0` of 1C:Trade Management
    /// 11.5.27.75's `1cv8.cf`, packed body sha256
    /// `90db75a46813f9f36e8abcc32499cf8bf5591af8b7c09770a11456569428e11f`;
    /// the expectation is the platform's own
    /// `Reports/ПродлениеСрокаДействияЭлектронныхПодписей/Templates/Макет/Ext/Template.xml`
    /// from an `ibcmd config export` capture with 1C:Enterprise 8.3.27.2214,
    /// sha256 `afaa6a8bc76bb34a3d5bf4650c60b0a7e740625fd690ec8da039675578eecbc5`.
    #[test]
    fn platform_multi_variant_settings_body_exports_byte_exact() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-ut-multi-variant-settings/raw-packed.bin.b64"
        )));
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-ut-multi-variant-settings/native-template.xml.b64"
        )));
        assert_eq!(
            format!("{:x}", Sha256::digest(&packed)),
            "90db75a46813f9f36e8abcc32499cf8bf5591af8b7c09770a11456569428e11f"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&expected)),
            "afaa6a8bc76bb34a3d5bf4650c60b0a7e740625fd690ec8da039675578eecbc5"
        );

        let body = crate::compiler::bodies::dcs::decode_compatible_dcs(
            crate::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .expect("platform-attested DCS body must decode");
        assert_eq!(
            body.documents().len(),
            6,
            "the header declares four Settings documents between the two SchemaFiles"
        );
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .expect("a four-variant envelope must export through the live codec");

        assert_eq!(actual, expected);
    }

    /// `AnyIBRef` is a protocol identifier outside the generated-type index,
    /// so it reaches the transliteration through the same unconditional seed
    /// `data_composition_type_id_xml` applies on the typed path -- and it is
    /// spelled `<v8:TypeSet>`, not `<v8:Type>`.
    ///
    /// Provenance (`manifest.json` in the fixture directory): storage element
    /// `cd6770d5-cd3c-47af-bc46-c865993adf63.0` of 1C:Trade Management
    /// 11.5.27.75's `1cv8.cf`, packed body sha256
    /// `4b70aeb7032569c77a2508fc932e0d27c03704191193f4e946922001f3adc9e4`;
    /// the expectation is the platform's own
    /// `Reports/МестаИспользованияСсылок/Templates/ОсновнаяСхемаКомпоновкиДанных/Ext/Template.xml`
    /// from an `ibcmd config export` capture with 1C:Enterprise 8.3.27.2214,
    /// sha256 `98d644bc92ee7fb5b100277478f90d075ab22bea2b358e6e2e86bbb34c07d6f9`.
    #[test]
    fn platform_any_ib_ref_type_set_body_exports_byte_exact() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-ut-any-ib-ref-type-set/raw-packed.bin.b64"
        )));
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-ut-any-ib-ref-type-set/native-template.xml.b64"
        )));
        assert_eq!(
            format!("{:x}", Sha256::digest(&packed)),
            "4b70aeb7032569c77a2508fc932e0d27c03704191193f4e946922001f3adc9e4"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&expected)),
            "98d644bc92ee7fb5b100277478f90d075ab22bea2b358e6e2e86bbb34c07d6f9"
        );

        let body = crate::compiler::bodies::dcs::decode_compatible_dcs(
            crate::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .expect("platform-attested DCS body must decode");
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .expect("an AnyIBRef valueType must export through the live codec");

        assert_eq!(actual, expected);
        assert!(
            String::from_utf8_lossy(&actual).contains(":AnyIBRef</v8:TypeSet>"),
            "a reference family is spelled as a TypeSet"
        );
    }

    /// The index-driven half of the same gap: `DocumentRef` is a
    /// `DcsTypeResolution::TypeSet` entry the type index does carry, which the
    /// transliteration had no map to read it from and so refused by name.
    ///
    /// Provenance (`manifest.json` in the fixture directory): storage element
    /// `3c305f89-e453-46c7-987b-f01dc964efdb.0` of 1C:Trade Management
    /// 11.5.27.75's `1cv8.cf`, packed body sha256
    /// `d0e430b3ab056b79ca70bd3c810bf0a82c006fb2d637bc41d9217a6b15f9b5fe`;
    /// the expectation is the platform's own
    /// `Reports/ДвиженияДокумента/Templates/ПустаяСхемаКомпоновкиДанных/Ext/Template.xml`
    /// from an `ibcmd config export` capture with 1C:Enterprise 8.3.27.2214,
    /// sha256 `4dcc90d8002faeabc911551b2ce388148feaab71b1d3283cd0deb01b85b31944`.
    #[test]
    fn platform_document_ref_type_set_body_exports_byte_exact() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-ut-document-ref-type-set/raw-packed.bin.b64"
        )));
        let expected = decode_base64_fixture(include_str!(concat!(
            "../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-ut-document-ref-type-set/native-template.xml.b64"
        )));
        assert_eq!(
            format!("{:x}", Sha256::digest(&packed)),
            "d0e430b3ab056b79ca70bd3c810bf0a82c006fb2d637bc41d9217a6b15f9b5fe"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&expected)),
            "4dcc90d8002faeabc911551b2ce388148feaab71b1d3283cd0deb01b85b31944"
        );

        let mut type_index = DcsTypeIndex::new();
        type_index.insert(
            "38bfd075-3e63-4aaa-a93e-94521380d579".to_owned(),
            DcsTypeResolution::TypeSet {
                qname: "cfg:DocumentRef".to_owned(),
            },
        );
        let body = crate::compiler::bodies::dcs::decode_compatible_dcs(
            crate::compiler::bodies::dcs::DcsTemplateKind::Schema,
            &packed,
        )
        .expect("platform-attested DCS body must decode");
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &type_index,
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .expect("a DocumentRef valueType must export through the live codec");

        assert_eq!(actual, expected);

        // Without the index entry the uuid resolves to nothing at all, and
        // that must still fail closed rather than be spelled either way.
        let refused = normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            &DcsTypeIndex::new(),
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        );
        assert!(
            refused.is_err(),
            "an unresolvable TypeId must not be guessed as either spelling"
        );
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

    /// A third field is outside both closed typed cohorts (which admit exactly
    /// two evidenced `DataSetQuery` child-lists, by count), so it reaches the
    /// general storage-to-source transliteration instead -- and is exported,
    /// not skipped. What the typed parsers refuse is still refused *by them*;
    /// the document is then accounted for from its own bytes. The
    /// transliterated result must reproduce the document itself: same
    /// `DataSetQuery`, all three fields, in order.
    #[test]
    fn query_union_link_third_field_transliterates_instead_of_failing_closed() {
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
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &three_field_documents,
            &type_index,
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .expect("a three-field DataSetQuery must transliterate, not be dropped");
        let actual = String::from_utf8(actual).expect("source XML is UTF-8");
        // The corpus carries the top-level `dataSet` query plus the one nested
        // in the `DataSetUnion` item, and both survive the rewrite.
        assert_eq!(actual.matches("xsi:type=\"DataSetQuery\"").count(), 2);
        assert_eq!(actual.matches("<dataPath>Owner</dataPath>").count(), 2);
        assert!(
            actual.contains("<v8:Type xmlns:d5p1=\"http://v8.1c.ru/8.1/data/enterprise/current-config\">d5p1:CatalogRef.FilterProbe</v8:Type>"),
            "the resolved TypeId keeps its evidenced source spelling: {actual}"
        );
    }

    /// Fail-closed floor for the transliteration: a primary schema carrying an
    /// element the source root declares no prefix for cannot be spelled in the
    /// source direction at all, so it must still be refused by name rather
    /// than exported with an invented prefix.
    #[test]
    fn primary_schema_with_undeclarable_namespace_still_fails_closed() {
        let primary = concat!(
            "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
            "<SchemaFile xmlns=\"\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\r\n",
            "\t<dataCompositionSchema xmlns=\"http://v8.1c.ru/8.1/data-composition-system/schema\">\r\n",
            "\t\t<dataSource><name>Source1</name><dataSourceType>Local</dataSourceType></dataSource>\r\n",
            "\t\t<probe xmlns=\"urn:example:not-a-dcs-namespace\">x</probe>\r\n",
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
        let rejection = normalize_data_composition_schema_template_documents_with_profiles(
            &documents,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .expect_err("an undeclarable namespace must fail closed");
        assert_eq!(
            rejection.code(),
            "dcs.template-normalize.primary-schema-parse",
            "the rejection must name the step that refused the source: {rejection}"
        );
    }

    /// Builds a three-document envelope around one primary schema body, so a
    /// probe only has to spell the part it is about.
    fn probe_documents(schema_body: &str) -> [String; 3] {
        [
            format!(
                concat!(
                    "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
                    "<SchemaFile xmlns=\"\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\"",
                    " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\r\n",
                    "\t<dataCompositionSchema",
                    " xmlns=\"http://v8.1c.ru/8.1/data-composition-system/schema\">\r\n",
                    "{}\r\n",
                    "\t\t<settingsVariant><name",
                    " xmlns=\"http://v8.1c.ru/8.1/data-composition-system/settings\"",
                    ">Default</name></settingsVariant>\r\n",
                    "\t</dataCompositionSchema>\r\n",
                    "</SchemaFile>"
                ),
                schema_body
            ),
            concat!(
                "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
                "<Settings xmlns=\"http://v8.1c.ru/8.1/data-composition-system/settings\"/>"
            )
            .to_owned(),
            concat!(
                "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
                "<SchemaFile xmlns=\"\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\"",
                " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\r\n",
                "\t<dataCompositionSchema",
                " xmlns=\"http://v8.1c.ru/8.1/data-composition-system/schema\"/>\r\n",
                "</SchemaFile>"
            )
            .to_owned(),
        ]
    }

    /// The three point-of-use spellings the transliteration takes from the
    /// storage bytes rather than from a cohort, each evidenced by the
    /// platform's own export of 1C:Trade Management 11.5.27.75:
    ///
    ///  * a generated `dNpM` prefix names the *storage* document's depth and
    ///    is reminted against the target depth (`d4p2` -> `d3p1`);
    ///  * a prefix the platform spelled itself (`sys`) is not depth-derived
    ///    and travels through verbatim, in the very same element;
    ///  * a `dcscor:value` typed `{data/ui}Color` carries a QName in its
    ///    character data, which moves onto the reminted prefix instead of
    ///    keeping the storage one.
    #[test]
    fn transliteration_remints_generated_prefixes_and_keeps_platform_spelled_ones() {
        let documents = probe_documents(concat!(
            "\t\t<appearance>\r\n",
            "\t\t\t<item xmlns=\"http://v8.1c.ru/8.1/data-composition-system/core\">\r\n",
            "\t\t\t\t<parameter>ЦветТекста</parameter>\r\n",
            "\t\t\t\t<value xmlns:d4p1=\"http://v8.1c.ru/8.1/data/ui\"",
            " xmlns:d4p2=\"http://v8.1c.ru/8.1/data/ui/style\"",
            " xsi:type=\"d4p1:Color\">d4p2:SpecialTextColor</value>\r\n",
            "\t\t\t\t<value xmlns:d4p1=\"http://v8.1c.ru/8.1/data/ui\"",
            " xmlns:sys=\"http://v8.1c.ru/8.1/data/ui/fonts/system\"",
            " xsi:type=\"d4p1:Font\" ref=\"sys:DefaultGUIFont\"/>\r\n",
            "\t\t\t</item>\r\n",
            "\t\t</appearance>"
        ));
        let borrowed: [&[u8]; 3] = [
            documents[0].as_bytes(),
            documents[1].as_bytes(),
            documents[2].as_bytes(),
        ];
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &borrowed,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .expect("an appearance probe must transliterate");
        let actual = String::from_utf8(actual).expect("source XML is UTF-8");
        assert!(
            actual.contains(concat!(
                "<dcscor:value xmlns:d4p1=\"http://v8.1c.ru/8.1/data/ui/style\"",
                " xsi:type=\"v8ui:Color\">d4p1:SpecialTextColor</dcscor:value>"
            )),
            "the generated prefix must be reminted in both the declaration and the value: {actual}"
        );
        assert!(
            actual.contains(concat!(
                "<dcscor:value xmlns:sys=\"http://v8.1c.ru/8.1/data/ui/fonts/system\"",
                " xsi:type=\"v8ui:Font\" ref=\"sys:DefaultGUIFont\"/>"
            )),
            "a platform-spelled prefix must travel through verbatim: {actual}"
        );
    }

    /// A `0:<uuid>` colour reference is a `StyleItem` the source document
    /// spells as a QName, so the style namespace is declared at the point of
    /// use. An unresolvable uuid has no name to write and fails closed
    /// instead of being emitted as the raw storage reference.
    #[test]
    fn transliteration_resolves_style_item_colour_references_or_fails_closed() {
        let documents = probe_documents(concat!(
            "\t\t<appearance>\r\n",
            "\t\t\t<item xmlns=\"http://v8.1c.ru/8.1/data-composition-system/core\">\r\n",
            "\t\t\t\t<value xmlns:d4p1=\"http://v8.1c.ru/8.1/data/ui\"",
            " xsi:type=\"d4p1:Color\">0:283ce432-3553-4de9-94a2-ca9a590437f5</value>\r\n",
            "\t\t\t</item>\r\n",
            "\t\t</appearance>"
        ));
        let borrowed: [&[u8]; 3] = [
            documents[0].as_bytes(),
            documents[1].as_bytes(),
            documents[2].as_bytes(),
        ];
        let rejection = normalize_data_composition_schema_template_documents_with_profiles(
            &borrowed,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .expect_err("an unresolvable style reference must fail closed");
        assert_eq!(
            rejection.code(),
            "dcs.template-normalize.primary-schema-parse",
            "the rejection must name the step that refused the source: {rejection}"
        );

        let mut object_refs = BTreeMap::new();
        object_refs.insert(
            "283ce432-3553-4de9-94a2-ca9a590437f5".to_owned(),
            "StyleItem.ПросроченныеДанныеЦвет".to_owned(),
        );
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &borrowed,
            &BTreeMap::new(),
            &object_refs,
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .expect("a resolvable style reference must transliterate");
        let actual = String::from_utf8(actual).expect("source XML is UTF-8");
        assert!(
            actual.contains(concat!(
                "<dcscor:value xmlns:d4p1=\"http://v8.1c.ru/8.1/data/ui/style\"",
                " xsi:type=\"v8ui:Color\">d4p1:ПросроченныеДанныеЦвет</dcscor:value>"
            )),
            "the style reference must be spelled as a QName at the point of use: {actual}"
        );
    }

    /// Storage's schema puts every literal `Type` before every `TypeId`, so a
    /// `valueType` mixing the two arrives grouped while the platform writes it
    /// interleaved. The interleaving is recovered rather than refused: a
    /// configuration type whose uuid sits below the builtin's evidenced sort
    /// interval is written ahead of it.
    #[test]
    fn transliteration_interleaves_a_mixed_value_type_at_the_evidenced_position() {
        let documents = mixed_value_type_probe("3a87ef2a-9de1-4d34-9e5f-3c8cdf53b3ab");
        let borrowed: [&[u8]; 3] = [
            documents[0].as_bytes(),
            documents[1].as_bytes(),
            documents[2].as_bytes(),
        ];
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &borrowed,
            &mixed_value_type_index("3a87ef2a-9de1-4d34-9e5f-3c8cdf53b3ab"),
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .expect("a mixed valueType whose order is evidenced must transliterate");
        let actual = String::from_utf8(actual).expect("source XML is UTF-8");
        let reference = actual
            .find("d4p1:CatalogRef.Probe")
            .expect("the reference type must be spelled: {actual}");
        let builtin = actual
            .find("<v8:Type>xs:string</v8:Type>")
            .expect("the builtin must be spelled: {actual}");
        assert!(
            reference < builtin,
            "a type uuid below the builtin's interval is written ahead of it: {actual}"
        );
    }

    /// Fail-closed floor for the same rule: a configuration type whose uuid
    /// falls strictly inside the builtin's evidenced interval decides nothing,
    /// so the source order is refused rather than guessed.
    #[test]
    fn transliteration_refuses_a_mixed_value_type_inside_the_evidenced_interval() {
        let documents = mixed_value_type_probe("9bd43cde-a83d-11e7-7088-f45c898df8f7");
        let borrowed: [&[u8]; 3] = [
            documents[0].as_bytes(),
            documents[1].as_bytes(),
            documents[2].as_bytes(),
        ];
        let rejection = normalize_data_composition_schema_template_documents_with_profiles(
            &borrowed,
            &mixed_value_type_index("9bd43cde-a83d-11e7-7088-f45c898df8f7"),
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .expect_err("a type uuid inside the interval has no evidenced position");
        assert_eq!(
            rejection.code(),
            "dcs.template-normalize.primary-schema-parse",
            "the rejection must name the step that refused the source: {rejection}"
        );
    }

    fn mixed_value_type_probe(type_id: &str) -> [String; 3] {
        probe_documents(&format!(
            concat!(
                "\t\t<calculatedField>\r\n",
                "\t\t\t<dataPath>Probe</dataPath>\r\n",
                "\t\t\t<valueType>\r\n",
                "\t\t\t\t<Type xmlns=\"http://v8.1c.ru/8.1/data/core\">xs:string</Type>\r\n",
                "\t\t\t\t<TypeId xmlns=\"http://v8.1c.ru/8.1/data/core\">{}</TypeId>\r\n",
                "\t\t\t</valueType>\r\n",
                "\t\t</calculatedField>"
            ),
            type_id
        ))
    }

    fn mixed_value_type_index(type_id: &str) -> DcsTypeIndex {
        let mut type_index = DcsTypeIndex::new();
        type_index.insert(
            type_id.to_owned(),
            DcsTypeResolution::Type {
                qname: "cfg:CatalogRef.Probe".to_owned(),
            },
        );
        type_index
    }

    /// A primary schema outside BOTH the inner-schema parser's admitted shape
    /// (not DataSetObject) AND the query-union-link parser's admitted shape
    /// (not exactly dataSource+query+union+link+variant) is neither dropped
    /// nor guessed at: it reaches the general storage-to-source
    /// transliteration, which accounts for it from its own bytes. Neither
    /// typed parser's admitted-shape strictness is loosened; what changes is
    /// only what happens after both of them have refused.
    ///
    /// The empty `<dcsset:settings/>` variant this probe carries is also the
    /// shape whose inline fragment used to be spliced as
    /// `.../ xmlns:dcsset="..."...>` -- not well-formed, and therefore
    /// rejected by our own analyzer before the primary schema was ever
    /// reached.
    #[test]
    fn schema_outside_both_typed_parsers_transliterates_with_its_empty_settings() {
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
        let actual = normalize_data_composition_schema_template_documents_with_profiles(
            &documents,
            &type_index,
            &BTreeMap::new(),
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .expect("a schema admitted by neither typed parser must still transliterate");
        let actual = String::from_utf8(actual).expect("source XML is UTF-8");
        assert!(
            actual.starts_with(
                "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<DataCompositionSchema xmlns="
            ),
            "the source root replaces the SchemaFile wrapper: {actual}"
        );
        assert!(
            actual.contains("\r\n\t<dataSource><name>Source1</name>"),
            "storage indentation loses exactly the wrapper level: {actual}"
        );
        assert!(
            actual.contains("<dcsset:name>Default</dcsset:name>"),
            "the settings-namespace child moves onto the source prefix: {actual}"
        );
        assert!(
            actual.contains("<dcsset:settings"),
            "the empty settings variant is inlined, not dropped: {actual}"
        );
        assert!(
            actual.ends_with("</DataCompositionSchema>"),
            "the wrapper close is replaced too: {actual}"
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

        // Without the resolver entry the export must fail closed instead of
        // silently dropping the AreaTemplate appearance or emitting the raw
        // `0:<uuid>` storage spelling, which the platform never writes.
        //
        // This is where the resolver gate lives now. The envelope admits this
        // terminal document either way -- its frame is decidable without
        // naming the style item, which is what lets real configurations'
        // area templates be transliterated at all -- so the typed coordinate
        // refuses for want of the name and the fragment transliteration,
        // reached next, refuses for exactly the same reason rather than
        // guessing one. See
        // `crate::compiler::bodies::dcs::tests::area_style_item_uuid_strict_decode_admits_the_frame_without_a_resolver`.
        for resolver in [
            BTreeMap::new(),
            BTreeMap::from([(
                "00000000-0000-0000-0000-000000000000".to_string(),
                "StyleItem.SomeOtherStyleItem".to_string(),
            )]),
        ] {
            let error = normalize_data_composition_schema_template_documents_with_profiles(
                &body.documents(),
                &BTreeMap::new(),
                &resolver,
                &ProfileId::parse("provider:mssql-legacy").unwrap(),
                &ProfileId::parse("xml-2.20").unwrap(),
            )
            .expect_err("an unresolvable style reference has no source spelling");
            assert_eq!(
                error.class(),
                MetadataSourceFailureClass::Unsupported,
                "the refusal is `no evidence names this`, not a broken invariant"
            );
            assert!(
                format!("{error}").contains("StyleItem resolution"),
                "the refusal names the missing resolution: {error}"
            );
        }
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

    /// Contract change, named rather than hidden: a direct `Settings` child
    /// no typed cohort owns used to fail the whole template. It is now
    /// transliterated instead -- the same move the primary schema made -- so
    /// what this test pins is that it never reaches *typed* normalization,
    /// and that the transliteration reproduces the child from its own bytes
    /// rather than dropping, renaming or inventing anything.
    ///
    /// A child in a namespace the source root cannot declare still fails
    /// closed; that floor is
    /// `settings_with_undeclarable_namespace_still_fails_closed` below.
    ///
    /// `outputParameters` is intentionally not in this list: it is a
    /// recognized typed element (evidence: dcs-output-parameters), so an
    /// occurrence with no items does not qualify as "unowned" -- its
    /// cohort-shape fail-closed cases live in the ibcmd-xml unit tests
    /// (`output_parameters_rejects_*` in crates/ibcmd-xml/src/dcs.rs).
    #[test]
    fn unknown_settings_children_transliterate_instead_of_failing_closed() {
        for (unknown, expected) in [
            ("<futureProbe/>", "<dcsset:futureProbe/>"),
            (
                "<probe:futureProbe xmlns:probe=\"http://v8.1c.ru/8.1/data-composition-system/settings\"/>",
                "<dcsset:futureProbe/>",
            ),
        ] {
            let settings = format!(
                "<Settings xmlns=\"{}\" xmlns:xsi=\"{}\">{unknown}</Settings>",
                std::str::from_utf8(DCS_SETTINGS_NS).unwrap(),
                std::str::from_utf8(XSI_NS).unwrap()
            );
            let source_profile = ProfileId::parse("provider:mssql-legacy").unwrap();
            let target_profile = ProfileId::parse("xml-2.20").unwrap();
            let canonical = canonicalize_data_composition_settings_document(
                &settings,
                &BTreeMap::new(),
                &source_profile,
                &target_profile,
            )
            .expect("an unowned child must transliterate, not fail the template");
            assert!(
                matches!(canonical, CanonicalDcsSettingsDocument::Transliterated(_)),
                "an unowned child must not be claimed by the typed cohort: {unknown}"
            );
            assert!(
                canonical.as_str().contains(expected),
                "the unowned child must survive the rewrite verbatim: {unknown} -> {}",
                canonical.as_str()
            );
        }
    }

    /// Fail-closed floor for the settings transliteration: only a *cohort*
    /// refusal reaches it. A document that is not a settings document at all
    /// is refused by the analyzer and stays refused -- there is nothing to
    /// transliterate, and routing it onward would turn a malformed input into
    /// invented output.
    #[test]
    fn malformed_settings_never_reach_the_transliteration() {
        for probe in [
            // Root is not the settings-namespace `Settings` element.
            "<Probe xmlns=\"http://v8.1c.ru/8.1/data-composition-system/settings\"/>".to_owned(),
            format!(
                "<Settings xmlns=\"urn:ibcmd-rs:not-the-settings-namespace\" xmlns:xsi=\"{}\"/>",
                std::str::from_utf8(XSI_NS).unwrap()
            ),
            // Not well-formed at all.
            format!(
                "<Settings xmlns=\"{}\"><order></Settings>",
                std::str::from_utf8(DCS_SETTINGS_NS).unwrap()
            ),
        ] {
            let rejection = canonicalize_data_composition_settings_document(
                &probe,
                &BTreeMap::new(),
                &ProfileId::parse("provider:mssql-legacy").unwrap(),
                &ProfileId::parse("xml-2.20").unwrap(),
            )
            .expect_err("a malformed settings document must fail closed");
            assert_eq!(
                rejection.code(),
                "dcs.settings-canonicalize.analysis",
                "the rejection must name the analyzer, not the transliteration: {rejection}"
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

    /// The root tag every decoded `ServerState` blob of UT 11.5.27.75 carries;
    /// all 657 are byte-identical in it.
    const SERVER_STATE_ROOT: &str = concat!(
        "<UniversalListServerOnlyState xmlns=\"\" ",
        "xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" ",
        "xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">"
    );

    /// A namespace the `Form` root already declares loses its blob-local
    /// declaration and every QName bound to it takes the root's prefix, at
    /// whatever depth the blob happened to declare it.
    ///
    /// The blob spells the same content at two depths -- `d3p1` on a
    /// `dcssch:title`, `d4p1` on a `dcssch:presentation` -- and the platform
    /// writes both with `v8`.
    #[test]
    fn a_root_declared_namespace_drops_its_generated_declaration_at_any_depth() {
        let inner = concat!(
            "\n\t<Field xmlns:dcssch=\"http://v8.1c.ru/8.1/data-composition-system/schema\" ",
            "xsi:type=\"dcssch:DataSetFieldField\">\n",
            "\t\t<dcssch:title xmlns:d3p1=\"http://v8.1c.ru/8.1/data/core\" ",
            "xsi:type=\"d3p1:LocalStringType\">\n",
            "\t\t\t<d3p1:item><d3p1:lang>ru</d3p1:lang></d3p1:item>\n",
            "\t\t</dcssch:title>\n",
            "\t\t<dcssch:availableValue>\n",
            "\t\t\t<dcssch:presentation xmlns:d4p1=\"http://v8.1c.ru/8.1/data/core\" ",
            "xsi:type=\"d4p1:LocalStringType\">\n",
            "\t\t\t\t<d4p1:item><d4p1:lang>ru</d4p1:lang></d4p1:item>\n",
            "\t\t\t</dcssch:presentation>\n",
            "\t\t</dcssch:availableValue>\n",
            "\t</Field>\n"
        );
        let expected = concat!(
            "\n\t<Field xsi:type=\"dcssch:DataSetFieldField\">\n",
            "\t\t<dcssch:title xsi:type=\"v8:LocalStringType\">\n",
            "\t\t\t<v8:item><v8:lang>ru</v8:lang></v8:item>\n",
            "\t\t</dcssch:title>\n",
            "\t\t<dcssch:availableValue>\n",
            "\t\t\t<dcssch:presentation xsi:type=\"v8:LocalStringType\">\n",
            "\t\t\t\t<v8:item><v8:lang>ru</v8:lang></v8:item>\n",
            "\t\t\t</dcssch:presentation>\n",
            "\t\t</dcssch:availableValue>\n",
            "\t</Field>\n"
        );
        assert_eq!(
            respell_form_server_state_namespaces(SERVER_STATE_ROOT, inner).as_deref(),
            Some(expected)
        );
    }

    /// The one namespace in the corpus the `Form` root does not declare keeps
    /// a generated prefix, renumbered from the blob's own depth to the depth
    /// its element sits at in `Form.xml`, and the QName in the body follows it.
    ///
    /// This is `Catalogs/КлючиРеестраДокументов/Forms/ФормаСписка` verbatim:
    /// the platform writes `d6p1` where the blob wrote `d3p2`, and the sibling
    /// core declaration -- written first, and dropped -- does not consume the
    /// index.
    #[test]
    fn the_one_namespace_the_form_root_lacks_is_renumbered_by_form_depth() {
        let inner = concat!(
            "\n\t<Parameter xmlns:dcssch=\"http://v8.1c.ru/8.1/data-composition-system/schema\" ",
            "xsi:type=\"dcssch:Parameter\">\n",
            "\t\t<dcssch:valueType>\n",
            "\t\t\t<Type xmlns=\"http://v8.1c.ru/8.1/data/core\">Type</Type>\n",
            "\t\t</dcssch:valueType>\n",
            "\t\t<dcssch:value xmlns:d3p1=\"http://v8.1c.ru/8.1/data/core\" ",
            "xmlns:d3p2=\"http://v8.1c.ru/8.2/data/types\" ",
            "xsi:type=\"d3p1:Type\">d3p2:Undefined</dcssch:value>\n",
            "\t</Parameter>\n"
        );
        let expected = concat!(
            "\n\t<Parameter xsi:type=\"dcssch:Parameter\">\n",
            "\t\t<dcssch:valueType>\n",
            "\t\t\t<Type xmlns=\"http://v8.1c.ru/8.1/data/core\">v8:Type</Type>\n",
            "\t\t</dcssch:valueType>\n",
            "\t\t<dcssch:value xmlns:d6p1=\"http://v8.1c.ru/8.2/data/types\" ",
            "xsi:type=\"v8:Type\">d6p1:Undefined</dcssch:value>\n",
            "\t</Parameter>\n"
        );
        assert_eq!(
            respell_form_server_state_namespaces(SERVER_STATE_ROOT, inner).as_deref(),
            Some(expected)
        );
    }

    /// A `valueType` body that already spells a prefix the blob root declares
    /// and the `Form` root declares too comes back untouched: 508 of the 510
    /// type bodies in the corpus are of this shape, and a rule that moved them
    /// would break output that is already exact.
    #[test]
    fn a_type_body_already_spelled_with_a_shared_prefix_is_left_alone() {
        let inner = concat!(
            "\n\t<Field xmlns:dcssch=\"http://v8.1c.ru/8.1/data-composition-system/schema\" ",
            "xsi:type=\"dcssch:DataSetFieldField\">\n",
            "\t\t<dcssch:valueType>\n",
            "\t\t\t<Type xmlns=\"http://v8.1c.ru/8.1/data/core\">xs:string</Type>\n",
            "\t\t\t<StringQualifiers xmlns=\"http://v8.1c.ru/8.1/data/core\">\n",
            "\t\t\t\t<Length>50</Length>\n",
            "\t\t\t</StringQualifiers>\n",
            "\t\t</dcssch:valueType>\n",
            "\t</Field>\n"
        );
        let expected = inner.replace(
            " xmlns:dcssch=\"http://v8.1c.ru/8.1/data-composition-system/schema\"",
            "",
        );
        assert_eq!(
            respell_form_server_state_namespaces(SERVER_STATE_ROOT, inner).as_deref(),
            Some(expected.as_str())
        );
    }

    /// A `dcssch:value` whose `xsi:type` names a type other than
    /// `{data/core}Type` carries a value, not a QName, and is not re-spelled.
    #[test]
    fn a_value_whose_xsi_type_is_not_a_type_keeps_its_character_data() {
        let inner = concat!(
            "\n\t<Parameter xmlns:dcssch=\"http://v8.1c.ru/8.1/data-composition-system/schema\" ",
            "xsi:type=\"dcssch:Parameter\">\n",
            "\t\t<dcssch:value xmlns:d3p1=\"http://v8.1c.ru/8.1/data/core\" ",
            "xsi:type=\"d3p1:UUID\">00000000-0000-0000-0000-000000000000</dcssch:value>\n",
            "\t</Parameter>\n"
        );
        let expected = concat!(
            "\n\t<Parameter xsi:type=\"dcssch:Parameter\">\n",
            "\t\t<dcssch:value xsi:type=\"v8:UUID\">",
            "00000000-0000-0000-0000-000000000000</dcssch:value>\n",
            "\t</Parameter>\n"
        );
        assert_eq!(
            respell_form_server_state_namespaces(SERVER_STATE_ROOT, inner).as_deref(),
            Some(expected)
        );
    }

    /// Fail-closed floor: a prefix nothing in scope declares has no spelling,
    /// so the whole blob is left exactly as it was rather than rewritten
    /// around the hole.
    #[test]
    fn an_undeclared_prefix_leaves_the_blob_untouched() {
        let inner = "\n\t<Field xsi:type=\"nowhere:DataSetFieldField\"/>\n";
        assert_eq!(
            respell_form_server_state_namespaces(SERVER_STATE_ROOT, inner),
            None
        );
    }

    /// Fail-closed floor for the other direction: a namespace the `Form` root
    /// does not declare, declared on the blob's own root, has nowhere to be
    /// minted, because the inline position writes no root element at all.
    #[test]
    fn a_root_declaration_outside_the_form_roots_set_is_refused() {
        let root = concat!(
            "<UniversalListServerOnlyState xmlns=\"\" ",
            "xmlns:d1p1=\"http://v8.1c.ru/8.2/data/types\">"
        );
        assert_eq!(
            respell_form_server_state_namespaces(root, "\n\t<Field/>\n"),
            None
        );
    }
}
