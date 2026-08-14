//! Evidence-bounded DCS schema-template envelope analysis and Settings binding.
//!
//! The schema subtree remains source-owned. This module owns only the
//! platform-authenticated document roles and the positional association of
//! external `Settings` documents with direct root `settingsVariant` nodes.

use ibcmd_core::artifact::ProfileId;
use ibcmd_schema::{
    DcsSchemaTemplateEnvelopeDocumentRole, bundled_dcs_schema_template_envelope_policy,
};
use quick_xml::NsReader;
use quick_xml::events::Event;
use quick_xml::name::ResolveResult;
use std::collections::BTreeMap;
use std::fmt::{self, Display, Formatter};

use crate::{
    DcsSettingsDocumentAnalysisError, analyze_dcs_settings_document,
    emit_dcs_area_template_storage_document_with_references,
    parse_dcs_area_template_source_document,
    parse_dcs_area_template_storage_document_with_references,
};

const UTF8_BOM: &[u8] = b"\xef\xbb\xbf";
const XML_DECLARATION: &[u8] = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n";
const SCHEMA_FILE_OPEN: &str = "<SchemaFile xmlns=\"\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">";
const MAX_XML_DEPTH: usize = 256;
const MAX_XML_EVENTS: usize = 1_000_000;
const EMPTY_SETTINGS_DOCUMENT: &str = "<Settings xmlns=\"http://v8.1c.ru/8.1/data-composition-system/settings\" xmlns:dcscor=\"http://v8.1c.ru/8.1/data-composition-system/core\" xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\" xmlns:sys=\"http://v8.1c.ru/8.1/data/ui/fonts/system\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" xmlns:web=\"http://v8.1c.ru/8.1/data/ui/colors/web\" xmlns:win=\"http://v8.1c.ru/8.1/data/ui/colors/windows\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"/>";
const SETTINGS_DOCUMENT_OPEN: &str = "<Settings xmlns=\"http://v8.1c.ru/8.1/data-composition-system/settings\" xmlns:dcsset=\"http://v8.1c.ru/8.1/data-composition-system/settings\" xmlns:dcscor=\"http://v8.1c.ru/8.1/data-composition-system/core\" xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\" xmlns:sys=\"http://v8.1c.ru/8.1/data/ui/fonts/system\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" xmlns:web=\"http://v8.1c.ru/8.1/data/ui/colors/web\" xmlns:win=\"http://v8.1c.ru/8.1/data/ui/colors/windows\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DcsSchemaTemplateDocuments<'a> {
    primary_schema_file: &'a [u8],
    settings: Vec<&'a [u8]>,
    terminal_schema_file: &'a [u8],
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
    if !policy.supports_attested_settings_variant_count(settings_count) {
        return Err(DcsSchemaTemplateError::UnsupportedSource(
            "native DCS envelope settings count is outside the attested range",
        ));
    }

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
                analyze_dcs_settings_document(std::str::from_utf8(document).map_err(|_| {
                    DcsSchemaTemplateError::Malformed(
                        "native Settings document is not UTF-8".to_string(),
                    )
                })?)
                .map_err(map_settings_error)?;
            }
            Some(DcsSchemaTemplateEnvelopeDocumentRole::TerminalSchemaFile) => {
                if inspect_schema_file(document, true, &policy).is_err() {
                    parse_dcs_area_template_storage_document_with_references(
                        document,
                        ProfileId::parse("provider:mssql-legacy").map_err(|error| {
                            DcsSchemaTemplateError::InvalidEvidence(error.to_string())
                        })?,
                        "dcs-envelope:terminal-area-template",
                        reference_types,
                    )
                    .map_err(|_| {
                        DcsSchemaTemplateError::UnsupportedSource(
                            "terminal SchemaFile is neither empty nor the evidenced AreaTemplate",
                        )
                    })?;
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
    if !policy.supports_attested_settings_variant_count(settings_blocks.len()) {
        return Err(DcsSchemaTemplateError::UnsupportedSource(
            "source settingsVariant count is outside the attested range",
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
    if !policy.supports_attested_settings_variant_count(captures.len()) {
        return Err(DcsSchemaTemplateError::UnsupportedSource(
            "source settingsVariant count is outside the attested range",
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

/// Converts the attested source wrapper into native XML document roles. The
/// schema subtree stays source-owned: this operation only detaches Settings,
/// applies the evidenced root-case/wrapper mapping, and creates the evidenced
/// empty terminal SchemaFile.
pub fn compile_dcs_schema_template_source_documents(
    source: &[u8],
) -> Result<DcsSchemaTemplateOwnedDocuments, DcsSchemaTemplateError> {
    compile_dcs_schema_template_source_documents_with_references(source, &BTreeMap::new())
}

/// Compiles the attested source wrapper into native XML document roles
/// exactly like [`compile_dcs_schema_template_source_documents`], but also
/// resolves a custom-`StyleItem` style-color reference's semantic name back
/// to its configuration-local storage uuid via `reference_types` when the
/// terminal AreaTemplate's `back_color_style_reference` is in that form
/// (see [`crate::emit_dcs_area_template_storage_document_with_references`]).
/// The standard `Named` style-reference form and every other coordinate are
/// unaffected: an empty map behaves identically to the plain function.
pub fn compile_dcs_schema_template_source_documents_with_references(
    source: &[u8],
    reference_types: &BTreeMap<String, String>,
) -> Result<DcsSchemaTemplateOwnedDocuments, DcsSchemaTemplateError> {
    let policy = bundled_dcs_schema_template_envelope_policy()
        .map_err(|error| DcsSchemaTemplateError::InvalidEvidence(error.to_string()))?;
    let text = std::str::from_utf8(source)
        .map_err(|_| DcsSchemaTemplateError::Malformed("DCS source is not UTF-8".to_string()))?;
    let source_body = strip_source_document_shell(text)?;
    if !source_body.starts_with("<DataCompositionSchema") {
        return Err(DcsSchemaTemplateError::UnsupportedSource(
            "prefixed or indirect DataCompositionSchema roots are outside the attested compiler cohort",
        ));
    }
    let source_profile = ProfileId::parse("source:designer-xml-2.20")
        .map_err(|error| DcsSchemaTemplateError::InvalidEvidence(error.to_string()))?;
    let area = parse_dcs_area_template_source_document(
        source_body.as_bytes(),
        source_profile,
        "dcs-template:source-area-template",
    )
    .map_err(|_| {
        DcsSchemaTemplateError::UnsupportedSource(
            "source AreaTemplate is outside the evidenced coordinate",
        )
    })?;
    let schema_without_area = if area.is_some() {
        remove_direct_area_template(source_body, policy.schema_namespace_uri())?
    } else {
        source_body.to_string()
    };
    let detached = detach_dcs_settings_from_source_variants(&schema_without_area)?;
    let mut native_schema = detached.schema_without_settings;
    rename_attested_source_schema_root(&mut native_schema)?;

    let primary = xml_document(&format!(
        "{SCHEMA_FILE_OPEN}\r\n{}\r\n</SchemaFile>",
        native_schema.trim_end()
    ));
    let settings = detached
        .settings_documents
        .iter()
        .map(|document| xml_document(document))
        .collect::<Vec<_>>();
    let terminal = match area {
        Some(area) => {
            emit_dcs_area_template_storage_document_with_references(&area, reference_types)
                .map_err(|_| {
                    DcsSchemaTemplateError::UnsupportedSource(
                        "source AreaTemplate is outside the evidenced storage coordinate",
                    )
                })?
        }
        None => xml_document(&format!(
            "{SCHEMA_FILE_OPEN}\r\n\t<dataCompositionSchema xmlns=\"{}\"/>\r\n</SchemaFile>",
            policy.schema_namespace_uri()
        )),
    };

    let borrowed = std::iter::once(primary.as_slice())
        .chain(settings.iter().map(Vec::as_slice))
        .chain(std::iter::once(terminal.as_slice()))
        .collect::<Vec<_>>();
    analyze_dcs_schema_template_documents_with_references(&borrowed, reference_types)?;
    Ok(DcsSchemaTemplateOwnedDocuments {
        primary_schema_file: primary,
        settings,
        terminal_schema_file: terminal,
    })
}

fn remove_direct_area_template(
    source: &str,
    schema_namespace: &str,
) -> Result<String, DcsSchemaTemplateError> {
    let mut reader = NsReader::from_str(source);
    reader.config_mut().trim_text(false);
    let mut depth = 0usize;
    let mut active = None::<usize>;
    let mut capture = None::<(usize, usize)>;
    loop {
        let event = reader
            .read_event()
            .map_err(|error| DcsSchemaTemplateError::Malformed(error.to_string()))?;
        let end = usize::try_from(reader.buffer_position())
            .map_err(|_| DcsSchemaTemplateError::Malformed("XML offset overflow".to_string()))?;
        match event {
            Event::Start(event) => {
                let (namespace, local) = reader.resolve_element(event.name());
                let namespace = namespace_bytes(&namespace)?;
                if depth == 1
                    && namespace == Some(schema_namespace.as_bytes())
                    && local.as_ref() == b"template"
                {
                    if active.is_some() || capture.is_some() {
                        return Err(DcsSchemaTemplateError::UnsupportedSource(
                            "more than one direct AreaTemplate is unsupported",
                        ));
                    }
                    active = Some(source_event_start(source, end)?);
                }
                depth += 1;
            }
            Event::Empty(event) => {
                let (namespace, local) = reader.resolve_element(event.name());
                if depth == 1
                    && namespace_bytes(&namespace)? == Some(schema_namespace.as_bytes())
                    && local.as_ref() == b"template"
                {
                    return Err(DcsSchemaTemplateError::UnsupportedSource(
                        "empty AreaTemplate is outside the evidenced coordinate",
                    ));
                }
            }
            Event::End(event) => {
                depth = depth.checked_sub(1).ok_or_else(|| {
                    DcsSchemaTemplateError::Malformed("XML depth underflow".to_string())
                })?;
                let (namespace, local) = reader.resolve_element(event.name());
                if depth == 1
                    && namespace_bytes(&namespace)? == Some(schema_namespace.as_bytes())
                    && local.as_ref() == b"template"
                {
                    let start = active.take().ok_or_else(|| {
                        DcsSchemaTemplateError::Malformed(
                            "AreaTemplate closing element has no opener".to_string(),
                        )
                    })?;
                    capture = Some((expand_indented_line_start(source, start), end));
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    let (start, end) = capture.ok_or(DcsSchemaTemplateError::UnsupportedSource(
        "direct AreaTemplate is absent",
    ))?;
    let mut output = source.to_string();
    output.replace_range(start..end, "");
    Ok(output)
}

fn expand_indented_line_start(source: &str, start: usize) -> usize {
    let line_start = source[..start].rfind('\n').map_or(0, |offset| offset + 1);
    if source[line_start..start]
        .bytes()
        .all(|byte| matches!(byte, b' ' | b'\t'))
    {
        line_start.saturating_sub(if source[..line_start].ends_with("\r\n") {
            2
        } else {
            1
        })
    } else {
        start
    }
}

fn strip_source_document_shell(source: &str) -> Result<&str, DcsSchemaTemplateError> {
    let mut body = source.trim_start_matches('\u{feff}').trim_start();
    if let Some(after_decl) = body.strip_prefix("<?xml") {
        let end = after_decl.find("?>").ok_or_else(|| {
            DcsSchemaTemplateError::Malformed("DCS XML declaration is not closed".to_string())
        })?;
        body = after_decl[end + 2..].trim_start_matches(['\r', '\n', ' ', '\t']);
    }
    Ok(body)
}

fn rename_attested_source_schema_root(xml: &mut String) -> Result<(), DcsSchemaTemplateError> {
    const SOURCE_ROOT: &str = "DataCompositionSchema";
    const SOURCE_CLOSE: &str = "</DataCompositionSchema>";
    let opening = xml
        .strip_prefix('<')
        .is_some_and(|body| body.starts_with(SOURCE_ROOT));
    let trimmed_len = xml.trim_end_matches(['\r', '\n', '\t', ' ']).len();
    let closing = trimmed_len.checked_sub(SOURCE_CLOSE.len());
    if !opening
        || closing.is_none()
        || xml.get(closing.unwrap_or_default()..trimmed_len) != Some(SOURCE_CLOSE)
    {
        return Err(DcsSchemaTemplateError::UnsupportedSource(
            "source DataCompositionSchema root spelling is outside the attested compiler cohort",
        ));
    }
    xml.replace_range(1..1 + SOURCE_ROOT.len(), "dataCompositionSchema");
    let closing = closing.expect("closing offset is checked");
    xml.replace_range(
        closing + 2..closing + 2 + SOURCE_ROOT.len(),
        "dataCompositionSchema",
    );
    Ok(())
}

fn xml_document(body: &str) -> Vec<u8> {
    let mut document = Vec::with_capacity(UTF8_BOM.len() + XML_DECLARATION.len() + body.len());
    document.extend_from_slice(UTF8_BOM);
    document.extend_from_slice(XML_DECLARATION);
    document.extend_from_slice(body.as_bytes());
    document
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
        assert_eq!(documents.terminal_schema_file(), expected_area);
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
        assert_eq!(documents.terminal_schema_file(), expected_area);
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

    #[test]
    fn authentic_one_variant_documents_are_role_checked_and_terminal_must_be_empty() {
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

        let mut terminal = documents[2].to_vec();
        let schema_start = terminal
            .windows(b"<dataCompositionSchema".len())
            .position(|window| window == b"<dataCompositionSchema")
            .unwrap();
        let empty_end = terminal[schema_start..]
            .windows(2)
            .position(|window| window == b"/>")
            .map(|offset| schema_start + offset)
            .unwrap();
        terminal.splice(
            empty_end..empty_end + 2,
            b"><future/></dataCompositionSchema>".iter().copied(),
        );
        let invalid = [documents[0], documents[1], terminal.as_slice()];
        assert!(matches!(
            analyze_dcs_schema_template_documents(&invalid),
            Err(DcsSchemaTemplateError::UnsupportedSource(_))
        ));
    }
}
