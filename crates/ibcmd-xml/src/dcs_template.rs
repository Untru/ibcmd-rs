//! Evidence-bounded DCS schema-template envelope analysis and Settings binding.
//!
//! This module owns the platform-authenticated document roles, the positional
//! association of external `Settings` documents with direct root
//! `settingsVariant` nodes, and the compile entry points; the source-to-storage
//! spelling itself lives in [`crate::dcs_storage`].

use ibcmd_core::artifact::ProfileId;
use ibcmd_schema::{
    DcsSchemaTemplateEnvelopeDocumentRole, bundled_dcs_schema_template_envelope_policy,
};
use quick_xml::NsReader;
use quick_xml::events::Event;
use quick_xml::name::ResolveResult;
use std::collections::BTreeMap;
use std::fmt::{self, Display, Formatter};

use crate::dcs_storage::{DcsStorageTypeResolver, compile_dcs_schema_storage_documents};
use crate::{
    DcsSettingsDocumentAnalysisError, analyze_dcs_settings_document,
    parse_dcs_area_template_storage_document_with_references,
};

const UTF8_BOM: &[u8] = b"\xef\xbb\xbf";
const DCS_AREA_TEMPLATE_NAMESPACE_URI: &str =
    "http://v8.1c.ru/8.1/data-composition-system/area-template";
const MAX_XML_DEPTH: usize = 256;
const MAX_XML_EVENTS: usize = 1_000_000;
const EMPTY_SETTINGS_DOCUMENT: &str = "<Settings xmlns=\"http://v8.1c.ru/8.1/data-composition-system/settings\" xmlns:dcscor=\"http://v8.1c.ru/8.1/data-composition-system/core\" xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\" xmlns:sys=\"http://v8.1c.ru/8.1/data/ui/fonts/system\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" xmlns:web=\"http://v8.1c.ru/8.1/data/ui/colors/web\" xmlns:win=\"http://v8.1c.ru/8.1/data/ui/colors/windows\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"/>";
const SETTINGS_DOCUMENT_OPEN: &str = "<Settings xmlns=\"http://v8.1c.ru/8.1/data-composition-system/settings\" xmlns:dcsset=\"http://v8.1c.ru/8.1/data-composition-system/settings\" xmlns:dcscor=\"http://v8.1c.ru/8.1/data-composition-system/core\" xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\" xmlns:sys=\"http://v8.1c.ru/8.1/data/ui/fonts/system\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" xmlns:web=\"http://v8.1c.ru/8.1/data/ui/colors/web\" xmlns:win=\"http://v8.1c.ru/8.1/data/ui/colors/windows\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DcsSchemaTemplateDocuments<'a> {
    primary_schema_file: &'a [u8],
    settings: Vec<&'a [u8]>,
    terminal_schema_file: &'a [u8],
    terminal_carries_templates: bool,
}

impl<'a> DcsSchemaTemplateDocuments<'a> {
    pub const fn primary_schema_file(&self) -> &'a [u8] {
        self.primary_schema_file
    }

    pub fn settings(&self) -> &[&'a [u8]] {
        &self.settings
    }

    pub const fn terminal_schema_file(&self) -> &'a [u8] {
        self.terminal_schema_file
    }

    /// Whether the terminal document holds area templates that neither the
    /// empty shape nor the typed `AreaTemplate` coordinate accounts for, and
    /// so has to be transliterated from its own bytes.
    ///
    /// The overwhelming majority of envelopes carry an empty terminal
    /// document and answer `false`, which is what keeps the fragment rewriter
    /// -- and its preconditions -- off every template that never needed it.
    pub const fn terminal_carries_templates(&self) -> bool {
        self.terminal_carries_templates
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DcsSchemaTemplateError {
    InvalidEvidence(String),
    Malformed(String),
    UnsupportedSource(&'static str),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DetachedDcsSchemaTemplateSource {
    schema_without_settings: String,
    settings_documents: Vec<String>,
}

/// Owned physical XML documents produced from the bounded source compiler
/// cohort. Binary framing and compression deliberately remain outside the XML
/// layer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DcsSchemaTemplateOwnedDocuments {
    primary_schema_file: Vec<u8>,
    settings: Vec<Vec<u8>>,
    terminal_schema_file: Vec<u8>,
}

impl DcsSchemaTemplateOwnedDocuments {
    pub(crate) fn from_parts(
        primary_schema_file: Vec<u8>,
        settings: Vec<Vec<u8>>,
        terminal_schema_file: Vec<u8>,
    ) -> Self {
        Self {
            primary_schema_file,
            settings,
            terminal_schema_file,
        }
    }

    pub fn primary_schema_file(&self) -> &[u8] {
        &self.primary_schema_file
    }

    pub fn settings(&self) -> &[Vec<u8>] {
        &self.settings
    }

    pub fn terminal_schema_file(&self) -> &[u8] {
        &self.terminal_schema_file
    }

    pub fn into_documents(self) -> Vec<Vec<u8>> {
        let mut documents = Vec::with_capacity(self.settings.len() + 2);
        documents.push(self.primary_schema_file);
        documents.extend(self.settings);
        documents.push(self.terminal_schema_file);
        documents
    }
}

impl DetachedDcsSchemaTemplateSource {
    pub fn schema_without_settings(&self) -> &str {
        &self.schema_without_settings
    }

    pub fn settings_documents(&self) -> &[String] {
        &self.settings_documents
    }
}

impl Display for DcsSchemaTemplateError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEvidence(reason) => {
                write!(formatter, "invalid DCS envelope evidence: {reason}")
            }
            Self::Malformed(reason) => write!(formatter, "malformed DCS template XML: {reason}"),
            Self::UnsupportedSource(reason) => {
                write!(formatter, "unsupported DCS template source: {reason}")
            }
        }
    }
}

impl std::error::Error for DcsSchemaTemplateError {}

/// Validates decoder-resolved native documents and assigns their evidenced
/// roles without scanning the plaintext for XML declarations.
pub fn analyze_dcs_schema_template_documents<'a>(
    documents: &[&'a [u8]],
) -> Result<DcsSchemaTemplateDocuments<'a>, DcsSchemaTemplateError> {
    analyze_dcs_schema_template_documents_with_references(documents, &BTreeMap::new())
}

/// Validates decoder-resolved native documents and assigns their evidenced
/// roles exactly like [`analyze_dcs_schema_template_documents`], but also
/// resolves a custom-`StyleItem` style-color reference's storage uuid via
/// `reference_types` when validating the terminal AreaTemplate (see
/// [`crate::parse_dcs_area_template_storage_document_with_references`]).
/// Without a matching entry, that one coordinate fails closed exactly as
/// the plain function does; every other coordinate is unaffected.
pub fn analyze_dcs_schema_template_documents_with_references<'a>(
    documents: &[&'a [u8]],
    reference_types: &BTreeMap<String, String>,
) -> Result<DcsSchemaTemplateDocuments<'a>, DcsSchemaTemplateError> {
    let policy = bundled_dcs_schema_template_envelope_policy()
        .map_err(|error| DcsSchemaTemplateError::InvalidEvidence(error.to_string()))?;
    let settings_count =
        documents
            .len()
            .checked_sub(2)
            .ok_or(DcsSchemaTemplateError::UnsupportedSource(
                "native DCS envelope must contain primary and terminal SchemaFile documents",
            ))?;
    if !policy.supports_framed_settings_variant_count(settings_count) {
        return Err(DcsSchemaTemplateError::UnsupportedSource(
            "native DCS envelope settings count is below the framed minimum",
        ));
    }
    let mut terminal_carries_templates = false;

    for (index, document) in documents.iter().enumerate() {
        if policy.documents_require_utf8_bom() && !document.starts_with(UTF8_BOM) {
            return Err(DcsSchemaTemplateError::Malformed(
                "native DCS XML document has no UTF-8 BOM".to_string(),
            ));
        }
        match policy.document_role(settings_count, index) {
            Some(DcsSchemaTemplateEnvelopeDocumentRole::PrimarySchemaFile) => {
                inspect_schema_file(document, false, &policy)?;
            }
            Some(DcsSchemaTemplateEnvelopeDocumentRole::Settings) => {
                // The envelope owns framing, not the settings cohort. A
                // document the typed cohort does not describe is still a
                // well-formed Settings document in the evidenced slot, and
                // the per-variant canonicalization -- which can fall back to
                // transliterating it from its own bytes -- is what decides
                // whether it can be spelled in the source direction. Only a
                // malformed document is refused here, where nothing
                // downstream could recover it.
                if let Err(error) =
                    analyze_dcs_settings_document(std::str::from_utf8(document).map_err(|_| {
                        DcsSchemaTemplateError::Malformed(
                            "native Settings document is not UTF-8".to_string(),
                        )
                    })?)
                    && matches!(error, DcsSettingsDocumentAnalysisError::Malformed(_))
                {
                    return Err(map_settings_error(error));
                }
            }
            Some(DcsSchemaTemplateEnvelopeDocumentRole::TerminalSchemaFile) => {
                terminal_carries_templates = inspect_schema_file(document, true, &policy).is_err()
                    && parse_dcs_area_template_storage_document_with_references(
                        document,
                        ProfileId::parse("provider:mssql-legacy").map_err(|error| {
                            DcsSchemaTemplateError::InvalidEvidence(error.to_string())
                        })?,
                        "dcs-envelope:terminal-area-template",
                        reference_types,
                    )
                    .is_err();
                if terminal_carries_templates {
                    // As with the Settings role, the envelope owns framing and
                    // not the template cohort. A terminal document the typed
                    // AreaTemplate coordinate does not describe is still a
                    // well-formed SchemaFile in the evidenced slot, and the
                    // storage-to-source fragment rewriter -- which reproduces
                    // it from its own bytes -- is what decides whether it can
                    // be spelled at all. Refused here only when it is not that
                    // shape, where nothing downstream could recover it.
                    inspect_terminal_template_schema_file(document, &policy)?;
                }
            }
            None => {
                return Err(DcsSchemaTemplateError::UnsupportedSource(
                    "native DCS document role is outside the attested envelope",
                ));
            }
        }
    }

    Ok(DcsSchemaTemplateDocuments {
        primary_schema_file: documents[0],
        settings: documents[1..documents.len() - 1].to_vec(),
        terminal_schema_file: documents[documents.len() - 1],
        terminal_carries_templates,
    })
}

/// Inserts already-rendered inline Settings blocks into direct root
/// `settingsVariant` elements positionally. The result is re-analyzed before
/// it is returned, so nested, duplicate, or foreign lookalikes cannot bind.
pub fn bind_dcs_settings_to_source_variants(
    source_schema: &str,
    settings_blocks: &[String],
) -> Result<String, DcsSchemaTemplateError> {
    let policy = bundled_dcs_schema_template_envelope_policy()
        .map_err(|error| DcsSchemaTemplateError::InvalidEvidence(error.to_string()))?;
    if !policy.supports_framed_settings_variant_count(settings_blocks.len()) {
        return Err(DcsSchemaTemplateError::UnsupportedSource(
            "source settingsVariant count is below the framed minimum",
        ));
    }
    let variants = direct_variant_closing_offsets(source_schema, false, &policy)?;
    if variants.len() != settings_blocks.len() {
        return Err(DcsSchemaTemplateError::UnsupportedSource(
            "external Settings document count does not match direct settingsVariant count",
        ));
    }

    let mut output = source_schema.to_owned();
    for (closing, settings) in variants.iter().zip(settings_blocks).rev() {
        let insertion = output[..*closing]
            .trim_end_matches(['\r', '\n', '\t', ' '])
            .len();
        output.insert_str(insertion, settings);
    }
    let rebound = direct_variant_closing_offsets(&output, true, &policy)?;
    if rebound.len() != settings_blocks.len() {
        return Err(DcsSchemaTemplateError::Malformed(
            "materialized Settings bindings are not direct variant children".to_string(),
        ));
    }
    Ok(output)
}

/// Detaches exactly one direct inline Settings child from every direct root
/// settingsVariant. Prefix spelling is deliberately bounded to the attested
/// source form; namespace/depth decide ownership before any byte range moves.
pub fn detach_dcs_settings_from_source_variants(
    source_schema: &str,
) -> Result<DetachedDcsSchemaTemplateSource, DcsSchemaTemplateError> {
    let policy = bundled_dcs_schema_template_envelope_policy()
        .map_err(|error| DcsSchemaTemplateError::InvalidEvidence(error.to_string()))?;
    let captures = direct_inline_settings_ranges(source_schema, &policy)?;
    if !policy.supports_framed_settings_variant_count(captures.len()) {
        return Err(DcsSchemaTemplateError::UnsupportedSource(
            "source settingsVariant count is below the framed minimum",
        ));
    }
    let mut settings_documents = Vec::with_capacity(captures.len());
    for capture in &captures {
        let opening = &source_schema[capture.start..capture.content_start];
        if !opening.starts_with("<dcsset:settings") {
            return Err(DcsSchemaTemplateError::UnsupportedSource(
                "inline Settings prefix spelling is outside the attested compiler cohort",
            ));
        }
        if capture.content_start == capture.content_end {
            settings_documents.push(EMPTY_SETTINGS_DOCUMENT.to_string());
        } else {
            settings_documents.push(format!(
                "{SETTINGS_DOCUMENT_OPEN}{}</Settings>",
                &source_schema[capture.content_start..capture.content_end]
            ));
        }
    }
    let mut schema_without_settings = source_schema.to_owned();
    for capture in captures.iter().rev() {
        schema_without_settings.replace_range(capture.start..capture.end, "");
    }
    Ok(DetachedDcsSchemaTemplateSource {
        schema_without_settings,
        settings_documents,
    })
}

/// Compiles a source `DataCompositionSchema` document into the native XML
/// document roles: the primary `SchemaFile`, one `Settings` document per root
/// `settingsVariant`, and the terminal `SchemaFile` carrying the area
/// templates. See [`crate::dcs_storage`] for the platform writer rules.
///
/// Configuration types stay spelled by name: without a configuration to
/// resolve them against, that is the one storage spelling this has.
pub fn compile_dcs_schema_template_source_documents(
    source: &[u8],
) -> Result<DcsSchemaTemplateOwnedDocuments, DcsSchemaTemplateError> {
    compile_dcs_schema_template_source_documents_with_references(source, &BTreeMap::new())
}

/// Compiles exactly like [`compile_dcs_schema_template_source_documents`],
/// but also resolves a custom-`StyleItem` style reference's semantic name
/// back to its configuration-local storage uuid via `reference_types`
/// (uuid -> name, searched by value). An empty map behaves identically to the
/// plain function.
pub fn compile_dcs_schema_template_source_documents_with_references(
    source: &[u8],
    reference_types: &BTreeMap<String, String>,
) -> Result<DcsSchemaTemplateOwnedDocuments, DcsSchemaTemplateError> {
    let unresolved = |_: &str| None::<String>;
    compile_dcs_schema_template_source_documents_with_resolvers(
        source,
        reference_types,
        &unresolved,
    )
}

/// Compiles like [`compile_dcs_schema_template_source_documents_with_references`]
/// and also resolves configuration types to their storage `TypeId` through
/// `types`, which is how the current platform stores them.
pub fn compile_dcs_schema_template_source_documents_with_resolvers(
    source: &[u8],
    reference_types: &BTreeMap<String, String>,
    types: &dyn DcsStorageTypeResolver,
) -> Result<DcsSchemaTemplateOwnedDocuments, DcsSchemaTemplateError> {
    let style_items = reference_types
        .iter()
        .map(|(uuid, name)| (name.clone(), uuid.clone()))
        .collect::<BTreeMap<_, _>>();
    let documents =
        compile_dcs_schema_storage_documents(source, &style_items, types)?.into_documents();
    let borrowed = std::iter::once(documents.primary_schema_file())
        .chain(documents.settings().iter().map(Vec::as_slice))
        .chain(std::iter::once(documents.terminal_schema_file()))
        .collect::<Vec<_>>();
    analyze_dcs_schema_template_documents_with_references(&borrowed, reference_types)?;
    Ok(documents)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct InlineSettingsRange {
    start: usize,
    content_start: usize,
    content_end: usize,
    end: usize,
}

fn direct_inline_settings_ranges(
    xml: &str,
    policy: &ibcmd_schema::DcsSchemaTemplateEnvelopePolicy,
) -> Result<Vec<InlineSettingsRange>, DcsSchemaTemplateError> {
    let mut reader = NsReader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut stack = Vec::<(Option<Vec<u8>>, Vec<u8>)>::new();
    let mut variant_item_counts = Vec::<usize>::new();
    let mut active = None::<(usize, usize)>;
    let mut captures = Vec::new();
    let mut events = 0usize;
    loop {
        let event = reader
            .read_event()
            .map_err(|error| DcsSchemaTemplateError::Malformed(error.to_string()))?;
        let event_end = usize::try_from(reader.buffer_position())
            .map_err(|_| DcsSchemaTemplateError::Malformed("XML offset overflow".to_string()))?;
        events = events.checked_add(1).ok_or_else(|| {
            DcsSchemaTemplateError::Malformed("source XML event count overflow".to_string())
        })?;
        if events > MAX_XML_EVENTS {
            return Err(DcsSchemaTemplateError::UnsupportedSource(
                "source XML event count exceeds the bounded envelope limit",
            ));
        }
        match event {
            Event::Start(event) => {
                let (namespace, local) = reader.resolve_element(event.name());
                let namespace = namespace_bytes(&namespace)?.map(<[u8]>::to_vec);
                if stack.is_empty()
                    && (namespace.as_deref() != Some(policy.schema_namespace_uri().as_bytes())
                        || local.as_ref() != b"DataCompositionSchema")
                {
                    return Err(DcsSchemaTemplateError::UnsupportedSource(
                        "source root must be schema DataCompositionSchema",
                    ));
                }
                let direct_variant = stack.len() == 1
                    && namespace.as_deref() == Some(policy.schema_namespace_uri().as_bytes())
                    && local.as_ref() == b"settingsVariant";
                if local.as_ref() == b"settingsVariant" && !direct_variant {
                    return Err(DcsSchemaTemplateError::UnsupportedSource(
                        "settingsVariant lookalikes outside the direct schema slot are unsupported",
                    ));
                }
                if direct_variant {
                    variant_item_counts.push(0);
                }
                let direct_settings = stack.len() == 2
                    && stack[1].0.as_deref() == Some(policy.schema_namespace_uri().as_bytes())
                    && stack[1].1.as_slice() == b"settingsVariant"
                    && namespace.as_deref() == Some(policy.settings_namespace_uri().as_bytes())
                    && local.as_ref() == b"settings";
                if local.as_ref() == b"settings" && !direct_settings {
                    return Err(DcsSchemaTemplateError::UnsupportedSource(
                        "Settings lookalikes outside a direct settingsVariant are unsupported",
                    ));
                }
                if direct_settings {
                    let count = variant_item_counts.last_mut().ok_or_else(|| {
                        DcsSchemaTemplateError::Malformed(
                            "variant state is inconsistent".to_string(),
                        )
                    })?;
                    *count += 1;
                    if *count > 1 || active.is_some() {
                        return Err(DcsSchemaTemplateError::UnsupportedSource(
                            "each direct settingsVariant must contain exactly one direct Settings child",
                        ));
                    }
                    let start = source_event_start(xml, event_end)?;
                    active = Some((start, event_end));
                }
                stack.push((namespace, local.as_ref().to_vec()));
            }
            Event::Empty(event) => {
                let (namespace, local) = reader.resolve_element(event.name());
                let namespace = namespace_bytes(&namespace)?;
                let direct_variant = stack.len() == 1
                    && namespace == Some(policy.schema_namespace_uri().as_bytes())
                    && local.as_ref() == b"settingsVariant";
                if local.as_ref() == b"settingsVariant" {
                    return Err(DcsSchemaTemplateError::UnsupportedSource(
                        if direct_variant {
                            "empty settingsVariant cannot bind an external Settings document"
                        } else {
                            "settingsVariant lookalikes outside the direct schema slot are unsupported"
                        },
                    ));
                }
                let direct_settings = stack.len() == 2
                    && stack[1].0.as_deref() == Some(policy.schema_namespace_uri().as_bytes())
                    && stack[1].1.as_slice() == b"settingsVariant"
                    && namespace == Some(policy.settings_namespace_uri().as_bytes())
                    && local.as_ref() == b"settings";
                if local.as_ref() == b"settings" && !direct_settings {
                    return Err(DcsSchemaTemplateError::UnsupportedSource(
                        "Settings lookalikes outside a direct settingsVariant are unsupported",
                    ));
                }
                if direct_settings {
                    let count = variant_item_counts.last_mut().ok_or_else(|| {
                        DcsSchemaTemplateError::Malformed(
                            "variant state is inconsistent".to_string(),
                        )
                    })?;
                    *count += 1;
                    if *count > 1 {
                        return Err(DcsSchemaTemplateError::UnsupportedSource(
                            "each direct settingsVariant must contain exactly one direct Settings child",
                        ));
                    }
                    let start = source_event_start(xml, event_end)?;
                    captures.push(InlineSettingsRange {
                        start,
                        content_start: event_end,
                        content_end: event_end,
                        end: event_end,
                    });
                }
            }
            Event::End(event) => {
                let (namespace, local) = reader.resolve_element(event.name());
                let namespace = namespace_bytes(&namespace)?;
                if stack.len() == 3
                    && namespace == Some(policy.settings_namespace_uri().as_bytes())
                    && local.as_ref() == b"settings"
                {
                    let (start, content_start) = active.take().ok_or_else(|| {
                        DcsSchemaTemplateError::Malformed(
                            "Settings capture state is absent".to_string(),
                        )
                    })?;
                    let content_end = xml[..event_end].rfind("</").ok_or_else(|| {
                        DcsSchemaTemplateError::Malformed(
                            "Settings closing tag offset is absent".to_string(),
                        )
                    })?;
                    captures.push(InlineSettingsRange {
                        start,
                        content_start,
                        content_end,
                        end: event_end,
                    });
                }
                if stack.len() == 2
                    && namespace == Some(policy.schema_namespace_uri().as_bytes())
                    && local.as_ref() == b"settingsVariant"
                    && variant_item_counts.pop() != Some(1)
                {
                    return Err(DcsSchemaTemplateError::UnsupportedSource(
                        "each direct settingsVariant must contain exactly one direct Settings child",
                    ));
                }
                let Some((open_namespace, open_local)) = stack.pop() else {
                    return Err(DcsSchemaTemplateError::Malformed(
                        "source closing element has no opener".to_string(),
                    ));
                };
                if open_namespace.as_deref() != namespace || open_local.as_slice() != local.as_ref()
                {
                    return Err(DcsSchemaTemplateError::Malformed(
                        "source element nesting is inconsistent".to_string(),
                    ));
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if !stack.is_empty() || !variant_item_counts.is_empty() || active.is_some() {
        return Err(DcsSchemaTemplateError::Malformed(
            "source schema XML is incomplete".to_string(),
        ));
    }
    Ok(captures)
}

fn source_event_start(xml: &str, event_end: usize) -> Result<usize, DcsSchemaTemplateError> {
    xml.get(..event_end)
        .and_then(|prefix| prefix.rfind('<'))
        .ok_or_else(|| {
            DcsSchemaTemplateError::Malformed("XML event start offset is absent".to_string())
        })
}

fn map_settings_error(error: DcsSettingsDocumentAnalysisError) -> DcsSchemaTemplateError {
    match error {
        DcsSettingsDocumentAnalysisError::Malformed(error) => {
            DcsSchemaTemplateError::Malformed(error.to_string())
        }
        DcsSettingsDocumentAnalysisError::UnsupportedSource { reason, .. } => {
            DcsSchemaTemplateError::UnsupportedSource(reason)
        }
    }
}

fn namespace_bytes<'a>(
    namespace: &'a ResolveResult<'a>,
) -> Result<Option<&'a [u8]>, DcsSchemaTemplateError> {
    match namespace {
        ResolveResult::Bound(namespace) => Ok(Some(namespace.0)),
        ResolveResult::Unbound => Ok(None),
        ResolveResult::Unknown(_) => Err(DcsSchemaTemplateError::Malformed(
            "XML uses an unresolved namespace prefix".to_string(),
        )),
    }
}

fn inspect_schema_file(
    document: &[u8],
    require_empty_schema: bool,
    policy: &ibcmd_schema::DcsSchemaTemplateEnvelopePolicy,
) -> Result<(), DcsSchemaTemplateError> {
    let mut reader = NsReader::from_reader(document);
    reader.config_mut().trim_text(false);
    let mut buffer = Vec::new();
    let mut stack = Vec::<(Option<Vec<u8>>, Vec<u8>)>::new();
    let mut schema_children = 0usize;
    let mut schema_descendants = 0usize;
    let mut events = 0usize;
    loop {
        let event = reader
            .read_event_into(&mut buffer)
            .map_err(|error| DcsSchemaTemplateError::Malformed(error.to_string()))?;
        events = events.checked_add(1).ok_or_else(|| {
            DcsSchemaTemplateError::Malformed("DCS XML event count overflow".to_string())
        })?;
        if events > MAX_XML_EVENTS {
            return Err(DcsSchemaTemplateError::UnsupportedSource(
                "DCS XML event count exceeds the bounded envelope limit",
            ));
        }
        match event {
            Event::Start(event) => {
                let (namespace, local) = reader.resolve_element(event.name());
                let namespace = namespace_bytes(&namespace)?.map(<[u8]>::to_vec);
                let depth = stack.len();
                validate_schema_file_element(depth, namespace.as_deref(), local.as_ref(), policy)?;
                if depth == 1 {
                    schema_children += 1;
                } else if depth > 1 {
                    schema_descendants += 1;
                }
                stack.push((namespace, local.as_ref().to_vec()));
                if stack.len() > MAX_XML_DEPTH {
                    return Err(DcsSchemaTemplateError::UnsupportedSource(
                        "DCS XML depth exceeds the bounded envelope limit",
                    ));
                }
            }
            Event::Empty(event) => {
                let (namespace, local) = reader.resolve_element(event.name());
                let namespace = namespace_bytes(&namespace)?;
                let depth = stack.len();
                validate_schema_file_element(depth, namespace, local.as_ref(), policy)?;
                if depth == 1 {
                    schema_children += 1;
                } else if depth > 1 {
                    schema_descendants += 1;
                }
            }
            Event::End(event) => {
                let (namespace, local) = reader.resolve_element(event.name());
                let namespace = namespace_bytes(&namespace)?;
                let Some((open_namespace, open_local)) = stack.pop() else {
                    return Err(DcsSchemaTemplateError::Malformed(
                        "DCS XML closing element has no opener".to_string(),
                    ));
                };
                if open_namespace.as_deref() != namespace || open_local.as_slice() != local.as_ref()
                {
                    return Err(DcsSchemaTemplateError::Malformed(
                        "DCS XML element nesting is inconsistent".to_string(),
                    ));
                }
            }
            Event::Text(text)
                if require_empty_schema
                    && stack.len() >= 2
                    && !text.as_ref().iter().all(u8::is_ascii_whitespace) =>
            {
                return Err(DcsSchemaTemplateError::UnsupportedSource(
                    "terminal native dataCompositionSchema must be empty",
                ));
            }
            Event::CData(_) if require_empty_schema && stack.len() >= 2 => {
                return Err(DcsSchemaTemplateError::UnsupportedSource(
                    "terminal native dataCompositionSchema must be empty",
                ));
            }
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    if !stack.is_empty() || schema_children != 1 {
        return Err(DcsSchemaTemplateError::Malformed(
            "SchemaFile must contain exactly one direct dataCompositionSchema".to_string(),
        ));
    }
    if require_empty_schema && schema_descendants != 0 {
        return Err(DcsSchemaTemplateError::UnsupportedSource(
            "terminal native dataCompositionSchema must be empty",
        ));
    }
    Ok(())
}

/// Accepts the physical shape a template-carrying terminal `SchemaFile` has:
/// one `dataCompositionSchema` followed by the area-template `appearance`
/// children its table cells select by ordinal.
///
/// This says nothing about what those elements contain -- that question
/// belongs to the fragment rewriter, which answers it from the bytes.
fn inspect_terminal_template_schema_file(
    document: &[u8],
    policy: &ibcmd_schema::DcsSchemaTemplateEnvelopePolicy,
) -> Result<(), DcsSchemaTemplateError> {
    let mut reader = NsReader::from_reader(document);
    reader.config_mut().trim_text(false);
    let mut buffer = Vec::new();
    let mut stack = Vec::<(Option<Vec<u8>>, Vec<u8>)>::new();
    let mut schema_children = 0usize;
    let mut events = 0usize;
    loop {
        let event = reader
            .read_event_into(&mut buffer)
            .map_err(|error| DcsSchemaTemplateError::Malformed(error.to_string()))?;
        events = events.checked_add(1).ok_or_else(|| {
            DcsSchemaTemplateError::Malformed("DCS XML event count overflow".to_string())
        })?;
        if events > MAX_XML_EVENTS {
            return Err(DcsSchemaTemplateError::UnsupportedSource(
                "DCS XML event count exceeds the bounded envelope limit",
            ));
        }
        let (start, name) = match &event {
            Event::Start(event) => (true, Some(event.name())),
            Event::Empty(event) => (false, Some(event.name())),
            Event::End(event) => {
                let (namespace, local) = reader.resolve_element(event.name());
                let namespace = namespace_bytes(&namespace)?;
                let Some((open_namespace, open_local)) = stack.pop() else {
                    return Err(DcsSchemaTemplateError::Malformed(
                        "DCS XML closing element has no opener".to_string(),
                    ));
                };
                if open_namespace.as_deref() != namespace || open_local.as_slice() != local.as_ref()
                {
                    return Err(DcsSchemaTemplateError::Malformed(
                        "DCS XML element nesting is inconsistent".to_string(),
                    ));
                }
                buffer.clear();
                continue;
            }
            Event::Eof => break,
            _ => {
                buffer.clear();
                continue;
            }
        };
        let Some(name) = name else {
            buffer.clear();
            continue;
        };
        let (namespace, local) = reader.resolve_element(name);
        let namespace = namespace_bytes(&namespace)?.map(<[u8]>::to_vec);
        let depth = stack.len();
        if depth == 0 {
            if namespace.is_some() || local.as_ref() != b"SchemaFile" {
                return Err(DcsSchemaTemplateError::UnsupportedSource(
                    "native DCS primary and terminal roots must be unqualified SchemaFile",
                ));
            }
        } else if depth == 1 {
            schema_children += 1;
            let expected: &[u8] = if schema_children == 1 {
                b"dataCompositionSchema"
            } else {
                b"appearance"
            };
            let expected_namespace = if schema_children == 1 {
                policy.schema_namespace_uri()
            } else {
                DCS_AREA_TEMPLATE_NAMESPACE_URI
            };
            if local.as_ref() != expected
                || namespace.as_deref() != Some(expected_namespace.as_bytes())
            {
                return Err(DcsSchemaTemplateError::UnsupportedSource(
                    "terminal SchemaFile carries neither a dataCompositionSchema nor its \
                     area-template appearance table",
                ));
            }
        }
        if start {
            stack.push((namespace, local.as_ref().to_vec()));
            if stack.len() > MAX_XML_DEPTH {
                return Err(DcsSchemaTemplateError::UnsupportedSource(
                    "DCS XML depth exceeds the bounded envelope limit",
                ));
            }
        }
    }
    if !stack.is_empty() || schema_children == 0 {
        return Err(DcsSchemaTemplateError::Malformed(
            "SchemaFile must contain exactly one direct dataCompositionSchema".to_string(),
        ));
    }
    Ok(())
}

fn validate_schema_file_element(
    depth: usize,
    namespace: Option<&[u8]>,
    local: &[u8],
    policy: &ibcmd_schema::DcsSchemaTemplateEnvelopePolicy,
) -> Result<(), DcsSchemaTemplateError> {
    if depth == 0 {
        if namespace.is_some() || local != b"SchemaFile" {
            return Err(DcsSchemaTemplateError::UnsupportedSource(
                "native DCS primary and terminal roots must be unqualified SchemaFile",
            ));
        }
    } else if depth == 1
        && (namespace != Some(policy.schema_namespace_uri().as_bytes())
            || local != b"dataCompositionSchema")
    {
        return Err(DcsSchemaTemplateError::UnsupportedSource(
            "SchemaFile direct child must be schema dataCompositionSchema",
        ));
    }
    Ok(())
}

fn direct_variant_closing_offsets(
    xml: &str,
    require_inline_settings: bool,
    policy: &ibcmd_schema::DcsSchemaTemplateEnvelopePolicy,
) -> Result<Vec<usize>, DcsSchemaTemplateError> {
    let mut reader = NsReader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut stack = Vec::<(Option<Vec<u8>>, Vec<u8>, usize)>::new();
    let mut offsets = Vec::new();
    let mut inline_counts = Vec::<usize>::new();
    let mut events = 0usize;
    loop {
        let event = reader
            .read_event()
            .map_err(|error| DcsSchemaTemplateError::Malformed(error.to_string()))?;
        events = events.checked_add(1).ok_or_else(|| {
            DcsSchemaTemplateError::Malformed("source XML event count overflow".to_string())
        })?;
        if events > MAX_XML_EVENTS {
            return Err(DcsSchemaTemplateError::UnsupportedSource(
                "source XML event count exceeds the bounded envelope limit",
            ));
        }
        match event {
            Event::Start(event) => {
                let (namespace, local) = reader.resolve_element(event.name());
                let namespace = namespace_bytes(&namespace)?.map(<[u8]>::to_vec);
                let position = usize::try_from(reader.buffer_position()).map_err(|_| {
                    DcsSchemaTemplateError::Malformed("XML offset overflow".to_string())
                })?;
                if stack.is_empty()
                    && (namespace.as_deref() != Some(policy.schema_namespace_uri().as_bytes())
                        || local.as_ref() != b"DataCompositionSchema")
                {
                    return Err(DcsSchemaTemplateError::UnsupportedSource(
                        "source root must be schema DataCompositionSchema",
                    ));
                }
                let is_direct_variant = stack.len() == 1
                    && namespace.as_deref() == Some(policy.schema_namespace_uri().as_bytes())
                    && local.as_ref() == b"settingsVariant";
                if local.as_ref() == b"settingsVariant" && !is_direct_variant {
                    return Err(DcsSchemaTemplateError::UnsupportedSource(
                        "settingsVariant lookalikes outside the direct schema slot are unsupported",
                    ));
                }
                let is_direct_inline_settings = stack.len() == 2
                    && stack[1].0.as_deref() == Some(policy.schema_namespace_uri().as_bytes())
                    && stack[1].1.as_slice() == b"settingsVariant"
                    && namespace.as_deref() == Some(policy.settings_namespace_uri().as_bytes())
                    && local.as_ref() == b"settings";
                if local.as_ref() == b"settings" && !is_direct_inline_settings {
                    return Err(DcsSchemaTemplateError::UnsupportedSource(
                        "Settings lookalikes outside a direct settingsVariant are unsupported",
                    ));
                }
                if is_direct_inline_settings {
                    let count = inline_counts.last_mut().ok_or_else(|| {
                        DcsSchemaTemplateError::Malformed(
                            "variant state is inconsistent".to_string(),
                        )
                    })?;
                    *count += 1;
                }
                if is_direct_variant {
                    inline_counts.push(0);
                }
                stack.push((namespace, local.as_ref().to_vec(), position));
                if stack.len() > MAX_XML_DEPTH {
                    return Err(DcsSchemaTemplateError::UnsupportedSource(
                        "source schema depth exceeds the bounded envelope limit",
                    ));
                }
            }
            Event::Empty(event) => {
                let (namespace, local) = reader.resolve_element(event.name());
                let namespace = namespace_bytes(&namespace)?;
                let is_direct_variant = stack.len() == 1
                    && namespace == Some(policy.schema_namespace_uri().as_bytes())
                    && local.as_ref() == b"settingsVariant";
                if local.as_ref() == b"settingsVariant" && !is_direct_variant {
                    return Err(DcsSchemaTemplateError::UnsupportedSource(
                        "settingsVariant lookalikes outside the direct schema slot are unsupported",
                    ));
                }
                if is_direct_variant {
                    return Err(DcsSchemaTemplateError::UnsupportedSource(
                        "empty settingsVariant cannot bind an external Settings document",
                    ));
                }
                let is_direct_inline_settings = stack.len() == 2
                    && stack[1].0.as_deref() == Some(policy.schema_namespace_uri().as_bytes())
                    && stack[1].1.as_slice() == b"settingsVariant"
                    && namespace == Some(policy.settings_namespace_uri().as_bytes())
                    && local.as_ref() == b"settings";
                if local.as_ref() == b"settings" && !is_direct_inline_settings {
                    return Err(DcsSchemaTemplateError::UnsupportedSource(
                        "Settings lookalikes outside a direct settingsVariant are unsupported",
                    ));
                }
                if is_direct_inline_settings {
                    let count = inline_counts.last_mut().ok_or_else(|| {
                        DcsSchemaTemplateError::Malformed(
                            "variant state is inconsistent".to_string(),
                        )
                    })?;
                    *count += 1;
                }
            }
            Event::End(event) => {
                let (namespace, local) = reader.resolve_element(event.name());
                let namespace = namespace_bytes(&namespace)?;
                if stack.len() == 2
                    && stack[0].0.as_deref() == Some(policy.schema_namespace_uri().as_bytes())
                    && stack[0].1.as_slice() == b"DataCompositionSchema"
                    && namespace == Some(policy.schema_namespace_uri().as_bytes())
                    && local.as_ref() == b"settingsVariant"
                {
                    let inline_count = inline_counts.pop().ok_or_else(|| {
                        DcsSchemaTemplateError::Malformed(
                            "variant state is inconsistent".to_string(),
                        )
                    })?;
                    if inline_count != usize::from(require_inline_settings) {
                        return Err(DcsSchemaTemplateError::UnsupportedSource(
                            "each direct settingsVariant must have the evidenced inline Settings cardinality",
                        ));
                    }
                    let position = usize::try_from(reader.buffer_position()).map_err(|_| {
                        DcsSchemaTemplateError::Malformed("XML offset overflow".to_string())
                    })?;
                    let event_name = event.name();
                    let lexical = std::str::from_utf8(event_name.as_ref()).map_err(|_| {
                        DcsSchemaTemplateError::Malformed(
                            "closing element name is not UTF-8".to_string(),
                        )
                    })?;
                    let end_tag = format!("</{lexical}>");
                    let search_end = position
                        .checked_add(end_tag.len())
                        .unwrap_or(xml.len())
                        .min(xml.len());
                    let closing = xml[..search_end].rfind(&end_tag).ok_or_else(|| {
                        DcsSchemaTemplateError::Malformed(
                            "settingsVariant closing tag is absent".to_string(),
                        )
                    })?;
                    offsets.push(closing);
                }
                let Some((open_namespace, open_local, _)) = stack.pop() else {
                    return Err(DcsSchemaTemplateError::Malformed(
                        "source closing element has no opener".to_string(),
                    ));
                };
                if open_namespace.as_deref() != namespace || open_local.as_slice() != local.as_ref()
                {
                    return Err(DcsSchemaTemplateError::Malformed(
                        "source element nesting is inconsistent".to_string(),
                    ));
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if !stack.is_empty() || !inline_counts.is_empty() {
        return Err(DcsSchemaTemplateError::Malformed(
            "source schema XML is incomplete".to_string(),
        ));
    }
    Ok(offsets)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(length, 0);
        output
    }

    #[test]
    fn platform_area_template_source_compiles_to_exact_native_documents() {
        let source = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-template/native-template.xml.b64"
        )));
        let expected_area = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-template/area-schema-file.xml.b64"
        )));
        let documents = compile_dcs_schema_template_source_documents(&source).unwrap();
        assert_eq!(documents.settings().len(), 1);
        assert_eq!(documents.terminal_schema_file(), expected_area);
        assert!(
            !std::str::from_utf8(documents.primary_schema_file())
                .unwrap()
                .contains("AreaProbe")
        );
    }

    #[test]
    fn platform_area_appearance_source_compiles_to_exact_side_table() {
        let source = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-template-appearance/native-template.xml.b64"
        )));
        let expected_area = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-template-appearance/area-schema-file.xml.b64"
        )));
        let documents = compile_dcs_schema_template_source_documents(&source).unwrap();
        assert_eq!(documents.settings().len(), 1);
        assert_eq!(documents.terminal_schema_file(), expected_area);
    }

    #[test]
    fn platform_area_appearance_web_color_source_compiles_to_exact_side_table() {
        let source = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-appearance-web-color/native-template.xml.b64"
        )));
        // The color cohort's manifest retains only the combined
        // `raw-unpacked` envelope; slice the terminal side-table document
        // from its length-prefixed header (magic + settings count + one
        // length per non-terminal document), matching the layout also
        // exercised in `src/compiler/bodies/dcs.rs`.
        let unpacked = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-appearance-web-color/raw-unpacked.bin.b64"
        )));
        let count = u32::from_le_bytes(unpacked[4..8].try_into().unwrap()) as usize;
        assert_eq!(count, 1);
        let first = u64::from_le_bytes(unpacked[8..16].try_into().unwrap()) as usize;
        let second = u64::from_le_bytes(unpacked[16..24].try_into().unwrap()) as usize;
        let expected_area = unpacked[24 + first + second..].to_vec();

        let documents = compile_dcs_schema_template_source_documents(&source).unwrap();
        assert_eq!(documents.settings().len(), 1);
        assert_eq!(
            documents.terminal_schema_file(),
            with_source_parameter_spelling(&expected_area)
        );
    }

    /// The platform's XML load stored two of these cohorts' appearance
    /// parameters under their English names (`Details`, `TextColor`) while
    /// its export -- and so the source -- spells them `Расшифровка` and
    /// `ЦветТекста`; the corpus-sized cohorts (БСП, ERP УХ) store the Russian
    /// spelling their source carries. The source cannot say which spelling a
    /// load chose, so the writer keeps the source's, and the rest of the side
    /// table is the platform's byte for byte.
    fn with_source_parameter_spelling(document: &[u8]) -> Vec<u8> {
        String::from_utf8(document.to_vec())
            .unwrap()
            .replace(
                "<parameter>Details</parameter>",
                "<parameter>Расшифровка</parameter>",
            )
            .replace(
                "<parameter>TextColor</parameter>",
                "<parameter>ЦветТекста</parameter>",
            )
            .into_bytes()
    }

    #[test]
    fn platform_multi_cell_appearance_source_compiles_to_exact_side_table() {
        let source = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-multi-cell-appearance/native-template.xml.b64"
        )));
        // Same manual slice pattern as the color cohort's equivalent test
        // above: this manifest also retains only the combined
        // `raw-unpacked` envelope.
        let unpacked = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-multi-cell-appearance/raw-unpacked.bin.b64"
        )));
        let count = u32::from_le_bytes(unpacked[4..8].try_into().unwrap()) as usize;
        assert_eq!(count, 1);
        let first = u64::from_le_bytes(unpacked[8..16].try_into().unwrap()) as usize;
        let second = u64::from_le_bytes(unpacked[16..24].try_into().unwrap()) as usize;
        let expected_area = unpacked[24 + first + second..].to_vec();

        let documents = compile_dcs_schema_template_source_documents(&source).unwrap();
        assert_eq!(documents.settings().len(), 1);
        assert_eq!(
            documents.terminal_schema_file(),
            with_source_parameter_spelling(&expected_area)
        );
    }

    #[test]
    fn binds_only_direct_variants_positionally() {
        let source = r#"<DataCompositionSchema xmlns="http://v8.1c.ru/8.1/data-composition-system/schema" xmlns:dcsset="http://v8.1c.ru/8.1/data-composition-system/settings"><settingsVariant><dcsset:name>A</dcsset:name></settingsVariant><settingsVariant><dcsset:name>B</dcsset:name></settingsVariant></DataCompositionSchema>"#;
        let blocks = vec![
            "<dcsset:settings><dcsset:selection/></dcsset:settings>".to_string(),
            "<dcsset:settings><dcsset:order/></dcsset:settings>".to_string(),
        ];
        let output = bind_dcs_settings_to_source_variants(source, &blocks).unwrap();
        assert!(output.find("selection").unwrap() < output.find("<dcsset:name>B").unwrap());
        assert!(output.find("<dcsset:name>B").unwrap() < output.find("order").unwrap());
    }

    #[test]
    fn rejects_nested_and_foreign_variant_lookalikes() {
        let nested = r#"<DataCompositionSchema xmlns="http://v8.1c.ru/8.1/data-composition-system/schema"><dataSet><settingsVariant/></dataSet></DataCompositionSchema>"#;
        assert!(bind_dcs_settings_to_source_variants(nested, &["<x/>".to_string()]).is_err());
        let foreign = r#"<DataCompositionSchema xmlns="http://v8.1c.ru/8.1/data-composition-system/schema" xmlns:f="urn:foreign"><f:settingsVariant/></DataCompositionSchema>"#;
        assert!(bind_dcs_settings_to_source_variants(foreign, &["<x/>".to_string()]).is_err());
    }

    #[test]
    fn detaches_direct_settings_without_matching_comment_or_nested_text() {
        let source = r#"<DataCompositionSchema xmlns="http://v8.1c.ru/8.1/data-composition-system/schema" xmlns:dcsset="http://v8.1c.ru/8.1/data-composition-system/settings"><!-- <dcsset:settings/> --><settingsVariant><dcsset:name>A</dcsset:name><dcsset:settings><dcsset:selection/></dcsset:settings></settingsVariant></DataCompositionSchema>"#;
        let detached = detach_dcs_settings_from_source_variants(source).unwrap();
        assert!(
            detached
                .schema_without_settings()
                .contains("<!-- <dcsset:settings/> -->")
        );
        assert!(
            !detached
                .schema_without_settings()
                .contains("<dcsset:selection/>")
        );
        assert_eq!(detached.settings_documents().len(), 1);
        assert!(detached.settings_documents()[0].contains("<dcsset:selection/>"));
    }

    #[test]
    fn platform_multi_variant_source_builds_owned_native_document_roles() {
        let source = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-multi-variant-envelope/native-template.xml.b64"
        )));
        let documents = compile_dcs_schema_template_source_documents(&source).unwrap();
        assert_eq!(documents.settings().len(), 2);
        assert!(documents.primary_schema_file().starts_with(UTF8_BOM));
        assert!(
            documents
                .primary_schema_file()
                .windows(b"<dataCompositionSchema".len())
                .any(|window| window == b"<dataCompositionSchema")
        );
        let borrowed = std::iter::once(documents.primary_schema_file())
            .chain(documents.settings().iter().map(Vec::as_slice))
            .chain(std::iter::once(documents.terminal_schema_file()))
            .collect::<Vec<_>>();
        assert_eq!(
            analyze_dcs_schema_template_documents(&borrowed)
                .unwrap()
                .settings()
                .len(),
            2
        );
    }

    #[test]
    fn source_document_constructor_rejects_unattested_root_spellings() {
        let prefixed = br#"<s:DataCompositionSchema xmlns:s="http://v8.1c.ru/8.1/data-composition-system/schema" xmlns:dcsset="http://v8.1c.ru/8.1/data-composition-system/settings"><s:settingsVariant><dcsset:settings/></s:settingsVariant></s:DataCompositionSchema>"#;
        assert!(matches!(
            compile_dcs_schema_template_source_documents(prefixed),
            Err(DcsSchemaTemplateError::UnsupportedSource(_))
        ));
    }

    /// The envelope owns the terminal document's *frame*, not its contents.
    ///
    /// This assertion used to read "and terminal must be empty": a terminal
    /// `dataCompositionSchema` with any child at all was refused here. Real
    /// configurations put a report's area templates in exactly that place, so
    /// the question "can these children be spelled in the source direction"
    /// moved to [`crate::rewrite_dcs_terminal_area_template_storage_fragment`],
    /// which answers it from the bytes and fails closed on its own. What the
    /// envelope still refuses is a terminal that is not this frame -- a
    /// `SchemaFile` direct child that is neither the schema element nor one of
    /// the area-template `appearance` elements its cells select.
    #[test]
    fn authentic_one_variant_documents_are_role_checked_and_terminal_frame_is_enforced() {
        let plain = include_bytes!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/dcs-core/raw/",
            "f4db0f6c-34f4-4449-995d-6265516e5fa8.0.bin"
        ));
        let first_len = u64::from_le_bytes(plain[8..16].try_into().unwrap()) as usize;
        let settings_len = u64::from_le_bytes(plain[16..24].try_into().unwrap()) as usize;
        let first_end = 24 + first_len;
        let settings_end = first_end + settings_len;
        let documents = [
            &plain[24..first_end],
            &plain[first_end..settings_end],
            &plain[settings_end..],
        ];
        let analysis = analyze_dcs_schema_template_documents(&documents).unwrap();
        assert_eq!(analysis.settings().len(), 1);

        let empty_end = |terminal: &[u8]| {
            let schema_start = terminal
                .windows(b"<dataCompositionSchema".len())
                .position(|window| window == b"<dataCompositionSchema")
                .unwrap();
            terminal[schema_start..]
                .windows(2)
                .position(|window| window == b"/>")
                .map(|offset| schema_start + offset)
                .unwrap()
        };

        // A child inside the terminal schema is now the rewriter's question.
        let mut carrying = documents[2].to_vec();
        let at = empty_end(&carrying);
        carrying.splice(
            at..at + 2,
            b"><future/></dataCompositionSchema>".iter().copied(),
        );
        let carrying = [documents[0], documents[1], carrying.as_slice()];
        analyze_dcs_schema_template_documents(&carrying)
            .expect("a template-carrying terminal is a framing question the envelope answers yes");

        // A `SchemaFile` child that is neither the schema element nor an
        // area-template appearance is still not this envelope.
        let mut foreign = documents[2].to_vec();
        let at = empty_end(&foreign);
        foreign.splice(
            at..at + 2,
            b"/><future xmlns=\"urn:future\"/>".iter().copied(),
        );
        let foreign = [documents[0], documents[1], foreign.as_slice()];
        assert!(matches!(
            analyze_dcs_schema_template_documents(&foreign),
            Err(DcsSchemaTemplateError::UnsupportedSource(_))
        ));
    }
}
