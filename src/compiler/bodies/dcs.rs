//! Profile-gated codecs for data-composition template bodies.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::ops::Range;

use ibcmd_core::artifact::ProfileId;
use ibcmd_core::profile::EffectiveProfile;
use ibcmd_xml::{
    DcsSchemaTemplateError, DcsStorageTypeResolver, DcsStorageTypeSpelling,
    analyze_dcs_schema_template_documents_with_references, compile_dcs_schema_storage_documents,
};
use quick_xml::NsReader;
use quick_xml::events::Event;
use quick_xml::name::ResolveResult;

use super::{BodyProfileError, SelectedBodyProfile};
use crate::compiler::families::native::{NativeError, deflate_bytes, inflate};
use crate::mssql_dump::{DcsTypeIndex, DcsTypeResolution};

const LAYOUT_KEY: &str = "bootstrap.body.dcs.layout";
const LAYOUT: &str = "dcs-schema-three-document-v1";
const UTF8_BOM: &[u8] = b"\xef\xbb\xbf";
const SCHEMA_NS: &[u8] = b"http://v8.1c.ru/8.1/data-composition-system/schema";
#[cfg(test)]
const SETTINGS_NS: &[u8] = b"http://v8.1c.ru/8.1/data-composition-system/settings";
const APPEARANCE_NS: &[u8] = b"http://v8.1c.ru/8.1/data-composition-system/appearance-template";
const MIN_DCS_HEADER_BYTES: usize = 24;
#[cfg(test)]
const DCS_HEADER_BYTES: usize = MIN_DCS_HEADER_BYTES;
const MAX_XML_DEPTH: usize = 256;
const MAX_XML_NODES: usize = 1_000_000;

#[cfg(test)]
const SCHEMA_FILE_OPEN: &str = "<SchemaFile xmlns=\"\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">";
#[cfg(test)]
const EMPTY_SETTINGS: &str = "<Settings xmlns=\"http://v8.1c.ru/8.1/data-composition-system/settings\" xmlns:dcscor=\"http://v8.1c.ru/8.1/data-composition-system/core\" xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\" xmlns:sys=\"http://v8.1c.ru/8.1/data/ui/fonts/system\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" xmlns:web=\"http://v8.1c.ru/8.1/data/ui/colors/web\" xmlns:win=\"http://v8.1c.ru/8.1/data/ui/colors/windows\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"/>";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DcsCodecProfile(SelectedBodyProfile);

impl DcsCodecProfile {
    pub fn from_effective(profile: &EffectiveProfile) -> Result<Self, BodyProfileError> {
        SelectedBodyProfile::from_effective(profile, LAYOUT_KEY, LAYOUT).map(Self)
    }

    pub const fn profile_id(&self) -> &ProfileId {
        self.0.profile_id()
    }

    #[cfg(test)]
    pub(crate) fn fixture() -> Self {
        Self(SelectedBodyProfile::fixture("platform-8.3.27.1989"))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DcsTemplateKind {
    Schema,
    Appearance,
}

impl Display for DcsTemplateKind {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Schema => "DataCompositionSchema",
            Self::Appearance => "DataCompositionAppearanceTemplate",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DcsBodyLayout {
    NativeThreeDocument,
    DirectXml,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DcsBody {
    kind: DcsTemplateKind,
    layout: DcsBodyLayout,
    plain: Vec<u8>,
    document_count: usize,
    document_ranges: Vec<Range<usize>>,
}

impl DcsBody {
    pub const fn kind(&self) -> DcsTemplateKind {
        self.kind
    }

    pub const fn layout(&self) -> DcsBodyLayout {
        self.layout
    }

    pub fn plaintext(&self) -> &[u8] {
        &self.plain
    }

    pub const fn document_count(&self) -> usize {
        self.document_count
    }

    /// Exact XML document slices resolved by the binary framing decoder.
    ///
    /// Consumers must use these ranges instead of scanning the plaintext for
    /// XML declarations: declarations may legally occur in comments or text.
    pub fn documents(&self) -> Vec<&[u8]> {
        self.document_ranges
            .iter()
            .map(|range| &self.plain[range.clone()])
            .collect()
    }
}

pub fn compile_dcs(
    profile: &DcsCodecProfile,
    kind: DcsTemplateKind,
    xml: &[u8],
) -> Result<Vec<u8>, DcsCodecError> {
    compile_dcs_with_references(profile, kind, xml, &BTreeMap::new())
}

/// Compiles exactly like [`compile_dcs`], but also resolves a custom
/// `StyleItem` area-appearance style reference's semantic name back to its
/// configuration-local storage uuid via `style_reference_types` -- the same
/// uuid-to-name map shape/convention as the TypeId-reference resolver (see
/// [`ibcmd_xml::parse_dcs_inner_schema_storage_document_with_references`]),
/// keyed the opposite direction here (searched by value) because this is
/// the compile, not the decode, direction. Building this map from source
/// tree data (`StyleItems/<Name>.xml`'s own `uuid` attribute) is the
/// caller's job; this codec never resolves a uuid by string heuristics,
/// only by exact lookup in the supplied map. Without a matching entry, a
/// custom-StyleItem reference fails closed rather than fabricating or
/// guessing a uuid; the standard `Named` form never needs a resolver and is
/// unaffected either way. Entries in the map that this coordinate never
/// looks up (any uuid/name pair unrelated to the one style reference this
/// document contains) are silently ignored, matching the TypeId
/// precedent's own lookup-only (never exhaustively validated) semantics.
pub fn compile_dcs_with_references(
    profile: &DcsCodecProfile,
    kind: DcsTemplateKind,
    xml: &[u8],
    style_reference_types: &BTreeMap<String, String>,
) -> Result<Vec<u8>, DcsCodecError> {
    let _ = profile;
    compile_evidenced_dcs_with_references(kind, xml, style_reference_types)
}

pub(crate) fn compile_evidenced_dcs_with_references(
    kind: DcsTemplateKind,
    xml: &[u8],
    style_reference_types: &BTreeMap<String, String>,
) -> Result<Vec<u8>, DcsCodecError> {
    let unresolved = |_: &str| None::<String>;
    compile_evidenced_dcs_with_resolvers(kind, xml, style_reference_types, &unresolved)
}

/// Compiles like [`compile_evidenced_dcs_with_references`] and resolves a
/// schema's configuration types to their storage `TypeId` through `types`,
/// the way the current platform writes them; a type it cannot resolve is
/// stored by name, which the exporter reads back the same.
///
/// A schema body is accepted only when the exporter's own reader, given the
/// compiled body, gives back the source document: the round trip is checked
/// here rather than assumed.
pub(crate) fn compile_evidenced_dcs_with_resolvers(
    kind: DcsTemplateKind,
    xml: &[u8],
    style_reference_types: &BTreeMap<String, String>,
    types: &dyn DcsStorageTypeResolver,
) -> Result<Vec<u8>, DcsCodecError> {
    match kind {
        DcsTemplateKind::Appearance => {
            validate_xml_document(xml, "AppearanceTemplate", Some(APPEARANCE_NS))?;
            let blob = deflate_bytes(xml)?;
            decode_strict_with_references(kind, &blob, style_reference_types)?;
            Ok(blob)
        }
        DcsTemplateKind::Schema => {
            let (plain, type_index, object_refs) =
                compile_schema_plain(xml, style_reference_types, types)?;
            let blob = deflate_bytes(&plain)?;
            let body = decode_strict_with_references(kind, &blob, style_reference_types)?;
            verify_schema_round_trip(xml, &body, &type_index, &object_refs)?;
            Ok(blob)
        }
    }
}

/// The type index and object references the exporter needs to read one
/// compiled schema body back: exactly the entries the writer resolved.
type SchemaReadBackIndexes = (DcsTypeIndex, BTreeMap<String, String>);

/// Proves the compiled body reads back as the source: the exporter's own
/// normalizer, given the body and the resolutions the writer made, has to
/// reproduce the source document after its BOM and XML declaration.
fn verify_schema_round_trip(
    source: &[u8],
    body: &DcsBody,
    type_index: &DcsTypeIndex,
    object_refs: &BTreeMap<String, String>,
) -> Result<(), DcsCodecError> {
    let exported =
        crate::mssql_dump::normalize_data_composition_schema_template_documents_with_profiles(
            &body.documents(),
            type_index,
            object_refs,
            &ProfileId::parse("provider:mssql-legacy")
                .map_err(|error| DcsCodecError::RoundTrip(error.to_string()))?,
            &ProfileId::parse("xml-2.20")
                .map_err(|error| DcsCodecError::RoundTrip(error.to_string()))?,
        )
        .map_err(|error| {
            DcsCodecError::RoundTrip(format!("the compiled body does not export: {error}"))
        })?;
    let expected = document_without_shell(source);
    let actual = document_without_shell(&exported);
    if expected == actual {
        return Ok(());
    }
    let at = expected
        .iter()
        .zip(actual)
        .position(|(left, right)| left != right)
        .unwrap_or_else(|| expected.len().min(actual.len()));
    Err(DcsCodecError::RoundTrip(format!(
        "the compiled body exports back differently at byte {at} of {}",
        expected.len()
    )))
}

/// A document after an optional UTF-8 BOM and XML declaration: the export
/// always writes both, so only the document itself is the source's to keep.
fn document_without_shell(document: &[u8]) -> &[u8] {
    let mut body = document.strip_prefix(UTF8_BOM).unwrap_or(document);
    if body.starts_with(b"<?xml")
        && let Some(end) = body.windows(2).position(|window| window == b"?>")
    {
        body = &body[end + 2..];
        while let Some((first, rest)) = body.split_first()
            && matches!(first, b'\r' | b'\n' | b' ' | b'\t')
        {
            body = rest;
        }
    }
    body
}

pub fn decode_dcs(
    profile: &DcsCodecProfile,
    kind: DcsTemplateKind,
    blob: &[u8],
) -> Result<DcsBody, DcsCodecError> {
    decode_dcs_with_references(profile, kind, blob, &BTreeMap::new())
}

/// Decodes exactly like [`decode_dcs`], but also resolves a custom
/// `StyleItem` area-appearance style reference's storage uuid back to its
/// semantic name via `style_reference_types` (uuid-to-name, the same
/// convention as [`compile_dcs_with_references`]'s resolver, searched
/// forward here since this is the decode direction). Without a matching
/// entry, that one coordinate fails closed exactly as [`decode_dcs`] does;
/// every other coordinate is unaffected.
pub fn decode_dcs_with_references(
    profile: &DcsCodecProfile,
    kind: DcsTemplateKind,
    blob: &[u8],
    style_reference_types: &BTreeMap<String, String>,
) -> Result<DcsBody, DcsCodecError> {
    let _ = profile;
    decode_strict_with_references(kind, blob, style_reference_types)
}

/// Bounded compatibility reader. Historical staging emitted a direct source
/// XML stream for schemas; retained platform rows use the evidenced
/// three-document header. Both remain readable, but only the latter is emitted
/// and accepted by the strict profile codec.
pub(crate) fn decode_compatible_dcs(
    kind: DcsTemplateKind,
    blob: &[u8],
) -> Result<DcsBody, DcsCodecError> {
    let plain = inflate(blob)?;
    match kind {
        DcsTemplateKind::Appearance => decode_appearance_plain(plain),
        DcsTemplateKind::Schema => match decode_schema_plain_framed(plain.clone()) {
            Ok(body) => Ok(body),
            Err(_) => {
                validate_xml_document(&plain, "DataCompositionSchema", Some(SCHEMA_NS))?;
                Ok(DcsBody {
                    kind,
                    layout: DcsBodyLayout::DirectXml,
                    document_ranges: vec![0..plain.len()],
                    plain,
                    document_count: 1,
                })
            }
        },
    }
}

fn decode_strict_with_references(
    kind: DcsTemplateKind,
    blob: &[u8],
    style_reference_types: &BTreeMap<String, String>,
) -> Result<DcsBody, DcsCodecError> {
    let plain = inflate(blob)?;
    match kind {
        DcsTemplateKind::Schema => {
            decode_schema_plain_with_references(plain, style_reference_types)
        }
        DcsTemplateKind::Appearance => decode_appearance_plain(plain),
    }
}

fn decode_appearance_plain(plain: Vec<u8>) -> Result<DcsBody, DcsCodecError> {
    validate_xml_document(&plain, "AppearanceTemplate", Some(APPEARANCE_NS))?;
    let document_len = plain.len();
    Ok(DcsBody {
        kind: DcsTemplateKind::Appearance,
        layout: DcsBodyLayout::DirectXml,
        plain,
        document_count: 1,
        document_ranges: vec![0..document_len],
    })
}

/// Writes the framed plaintext of a schema body, returning with it the
/// indexes the exporter needs to read that body back.
///
/// Which settings shapes the body may carry is not a cohort question any
/// more: the writer spells every settings child the way the platform does,
/// and the caller's round trip decides whether the result is the source.
fn compile_schema_plain(
    xml: &[u8],
    style_reference_types: &BTreeMap<String, String>,
    types: &dyn DcsStorageTypeResolver,
) -> Result<(Vec<u8>, DcsTypeIndex, BTreeMap<String, String>), DcsCodecError> {
    validate_xml_document(xml, "DataCompositionSchema", Some(SCHEMA_NS))?;

    let style_items = style_reference_types
        .iter()
        .map(|(uuid, name)| (name.clone(), uuid.clone()))
        .collect::<BTreeMap<_, _>>();
    let compiled = compile_dcs_schema_storage_documents(xml, &style_items, types)
        .map_err(map_template_error)?;
    let (type_index, object_refs) = read_back_indexes(&compiled);
    let documents = compiled.into_documents();
    let borrowed = std::iter::once(documents.primary_schema_file())
        .chain(documents.settings().iter().map(Vec::as_slice))
        .chain(std::iter::once(documents.terminal_schema_file()))
        .collect::<Vec<_>>();
    analyze_dcs_schema_template_documents_with_references(&borrowed, style_reference_types)
        .map_err(map_template_error)?;
    let first = documents.primary_schema_file();
    let settings = documents.settings();
    let third = documents.terminal_schema_file();

    let settings_count = u32::try_from(settings.len())
        .map_err(|_| DcsCodecError::LimitExceeded("DCS settings document count"))?;
    let stored_documents = std::iter::once(first).chain(settings.iter().map(Vec::as_slice));
    let lengths = stored_documents
        .clone()
        .map(|document| {
            u64::try_from(document.len())
                .map_err(|_| DcsCodecError::LimitExceeded("DCS XML document"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let header_len = 8usize
        .checked_add(
            lengths
                .len()
                .checked_mul(8)
                .ok_or(DcsCodecError::LimitExceeded("DCS header"))?,
        )
        .ok_or(DcsCodecError::LimitExceeded("DCS header"))?;
    let capacity = settings
        .iter()
        .fold(header_len.checked_add(first.len()), |total, document| {
            total.and_then(|value| value.checked_add(document.len()))
        })
        .and_then(|value| value.checked_add(third.len()))
        .ok_or(DcsCodecError::LimitExceeded("DCS body"))?;
    let mut plain = Vec::with_capacity(capacity);
    plain.extend_from_slice(&0u32.to_le_bytes());
    plain.extend_from_slice(&settings_count.to_le_bytes());
    for length in lengths {
        plain.extend_from_slice(&length.to_le_bytes());
    }
    plain.extend_from_slice(first);
    for document in settings {
        plain.extend_from_slice(document);
    }
    plain.extend_from_slice(third);
    Ok((plain, type_index, object_refs))
}

/// The exporter's view of the references one compiled body carries: every
/// `TypeId` the writer put into storage under the spelling the source gives
/// it, and every configuration style item it stored by uuid.
fn read_back_indexes(compiled: &ibcmd_xml::DcsStorageDocuments) -> SchemaReadBackIndexes {
    let type_index = compiled
        .type_ids()
        .iter()
        .map(|(uuid, spelling)| {
            let resolution = match spelling {
                DcsStorageTypeSpelling::Type(name) => DcsTypeResolution::Type {
                    qname: format!("cfg:{name}"),
                },
                DcsStorageTypeSpelling::TypeSet(name) => DcsTypeResolution::TypeSet {
                    qname: format!("cfg:{name}"),
                },
                DcsStorageTypeSpelling::Kept => DcsTypeResolution::KeepId,
            };
            (uuid.clone(), resolution)
        })
        .collect();
    let object_refs = compiled
        .style_items()
        .iter()
        .map(|(uuid, name)| (uuid.clone(), format!("StyleItem.{name}")))
        .collect();
    (type_index, object_refs)
}

fn decode_schema_plain_with_references(
    plain: Vec<u8>,
    style_reference_types: &BTreeMap<String, String>,
) -> Result<DcsBody, DcsCodecError> {
    let body = decode_schema_plain_framed(plain)?;
    analyze_dcs_schema_template_documents_with_references(&body.documents(), style_reference_types)
        .map_err(map_template_error)?;
    Ok(body)
}

fn decode_schema_plain_framed(plain: Vec<u8>) -> Result<DcsBody, DcsCodecError> {
    if plain.len() < MIN_DCS_HEADER_BYTES {
        return Err(DcsCodecError::UnsupportedLayout(
            "DCS schema header is truncated".to_string(),
        ));
    }
    if read_u32(&plain, 0)? != 0 {
        return Err(DcsCodecError::UnsupportedLayout(
            "unknown DCS schema header version".to_string(),
        ));
    }
    let settings_count = usize::try_from(read_u32(&plain, 4)?)
        .map_err(|_| DcsCodecError::LimitExceeded("DCS settings document count"))?;
    if settings_count == 0 {
        return Err(DcsCodecError::UnsupportedLayout(
            "DCS schema has no settings documents".to_string(),
        ));
    }
    let stored_length_count = settings_count
        .checked_add(1)
        .ok_or(DcsCodecError::LimitExceeded("DCS document count"))?;
    let header_len = stored_length_count
        .checked_mul(std::mem::size_of::<u64>())
        .and_then(|lengths| 8usize.checked_add(lengths))
        .ok_or(DcsCodecError::LimitExceeded("DCS schema header"))?;
    if header_len >= plain.len() {
        return Err(DcsCodecError::UnsupportedLayout(
            "DCS schema header is truncated".to_string(),
        ));
    }

    let document_count = settings_count
        .checked_add(2)
        .ok_or(DcsCodecError::LimitExceeded("DCS document count"))?;
    let mut document_start = header_len;
    let mut document_ranges = Vec::with_capacity(document_count);
    for index in 0..stored_length_count {
        let length_offset = index
            .checked_mul(std::mem::size_of::<u64>())
            .and_then(|offset| 8usize.checked_add(offset))
            .ok_or(DcsCodecError::LimitExceeded("DCS document length offset"))?;
        let length = read_len(&plain, length_offset, "DCS XML document")?;
        let document_end = document_start
            .checked_add(length)
            .ok_or(DcsCodecError::LimitExceeded("DCS document offset"))?;
        if length == 0 || document_end > plain.len() {
            return Err(DcsCodecError::UnsupportedLayout(
                "DCS schema document lengths are invalid".to_string(),
            ));
        }
        document_ranges.push(document_start..document_end);
        document_start = document_end;
    }
    if document_start >= plain.len() {
        return Err(DcsCodecError::UnsupportedLayout(
            "DCS schema document lengths are invalid".to_string(),
        ));
    }
    document_ranges.push(document_start..plain.len());
    Ok(DcsBody {
        kind: DcsTemplateKind::Schema,
        layout: DcsBodyLayout::NativeThreeDocument,
        plain,
        document_count,
        document_ranges,
    })
}

fn map_template_error(error: DcsSchemaTemplateError) -> DcsCodecError {
    match error {
        DcsSchemaTemplateError::InvalidEvidence(reason) => DcsCodecError::UnsupportedLayout(reason),
        DcsSchemaTemplateError::Malformed(reason) => DcsCodecError::InvalidXml(reason),
        DcsSchemaTemplateError::UnsupportedSource(reason) => {
            DcsCodecError::UnsupportedSource(reason)
        }
    }
}

fn read_u32(input: &[u8], offset: usize) -> Result<u32, DcsCodecError> {
    let bytes: [u8; 4] = input
        .get(offset..offset + 4)
        .ok_or_else(|| DcsCodecError::UnsupportedLayout("truncated DCS header".to_string()))?
        .try_into()
        .expect("slice length is checked");
    Ok(u32::from_le_bytes(bytes))
}

fn read_len(input: &[u8], offset: usize, field: &'static str) -> Result<usize, DcsCodecError> {
    let bytes: [u8; 8] = input
        .get(offset..offset + 8)
        .ok_or_else(|| DcsCodecError::UnsupportedLayout("truncated DCS header".to_string()))?
        .try_into()
        .expect("slice length is checked");
    usize::try_from(u64::from_le_bytes(bytes)).map_err(|_| DcsCodecError::LimitExceeded(field))
}

#[cfg(test)]
fn xml_document(body: &str) -> Vec<u8> {
    let mut document = Vec::with_capacity(UTF8_BOM.len() + 45 + body.len());
    document.extend_from_slice(UTF8_BOM);
    document.extend_from_slice(b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n");
    document.extend_from_slice(body.as_bytes());
    document
}

#[derive(Default)]
struct XmlInspection {
    has_inline_area_template: bool,
}

fn validate_xml_document(
    xml: &[u8],
    expected_root: &'static str,
    expected_namespace: Option<&[u8]>,
) -> Result<XmlInspection, DcsCodecError> {
    let mut reader = NsReader::from_reader(xml);
    reader.config_mut().trim_text(false);
    let mut inspection = XmlInspection::default();
    let mut depth = 0usize;
    let mut nodes = 0usize;
    let mut root_seen = false;

    loop {
        let event = reader
            .read_event()
            .map_err(|error| DcsCodecError::InvalidXml(error.to_string()))?;
        match event {
            Event::Start(event) => {
                let (namespace, local) = reader.resolve_element(event.name());
                let has_inline_area_template = event
                    .attributes()
                    .flatten()
                    .any(|attribute| attribute.value.as_ref().ends_with(b":AreaTemplate"));
                inspect_xml_element(
                    &namespace,
                    local.as_ref(),
                    has_inline_area_template,
                    depth,
                    expected_root,
                    expected_namespace,
                    &mut root_seen,
                    &mut inspection,
                    &mut nodes,
                )?;
                depth = depth
                    .checked_add(1)
                    .ok_or(DcsCodecError::LimitExceeded("DCS XML depth"))?;
                if depth > MAX_XML_DEPTH {
                    return Err(DcsCodecError::LimitExceeded("DCS XML depth"));
                }
            }
            Event::Empty(event) => {
                let (namespace, local) = reader.resolve_element(event.name());
                let has_inline_area_template = event
                    .attributes()
                    .flatten()
                    .any(|attribute| attribute.value.as_ref().ends_with(b":AreaTemplate"));
                inspect_xml_element(
                    &namespace,
                    local.as_ref(),
                    has_inline_area_template,
                    depth,
                    expected_root,
                    expected_namespace,
                    &mut root_seen,
                    &mut inspection,
                    &mut nodes,
                )?;
            }
            Event::End(_) => {
                depth = depth.checked_sub(1).ok_or_else(|| {
                    DcsCodecError::InvalidXml("DCS XML closes above its root".to_string())
                })?;
            }
            Event::Text(event)
                if depth == 0 && !event.as_ref().iter().all(u8::is_ascii_whitespace) =>
            {
                return Err(DcsCodecError::InvalidXml(
                    "DCS XML has text outside its root".to_string(),
                ));
            }
            Event::DocType(_) => {
                return Err(DcsCodecError::InvalidXml(
                    "DCS XML document types are not supported".to_string(),
                ));
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if !root_seen || depth != 0 {
        return Err(DcsCodecError::InvalidXml(
            "DCS XML root is absent or unclosed".to_string(),
        ));
    }
    Ok(inspection)
}

pub(crate) fn validate_raw_xml_root(
    xml: &[u8],
    expected_root: &'static str,
    expected_namespace: Option<&[u8]>,
) -> Result<(), DcsCodecError> {
    validate_xml_document(xml, expected_root, expected_namespace).map(|_| ())
}

#[allow(clippy::too_many_arguments)]
fn inspect_xml_element(
    namespace: &ResolveResult<'_>,
    local: &[u8],
    has_inline_area_template: bool,
    depth: usize,
    expected_root: &'static str,
    expected_namespace: Option<&[u8]>,
    root_seen: &mut bool,
    inspection: &mut XmlInspection,
    nodes: &mut usize,
) -> Result<(), DcsCodecError> {
    *nodes = nodes
        .checked_add(1)
        .ok_or(DcsCodecError::LimitExceeded("DCS XML nodes"))?;
    if *nodes > MAX_XML_NODES {
        return Err(DcsCodecError::LimitExceeded("DCS XML nodes"));
    }
    let namespace = match namespace {
        ResolveResult::Bound(namespace) => Some(namespace.0),
        ResolveResult::Unbound => None,
        ResolveResult::Unknown(_) => {
            return Err(DcsCodecError::InvalidXml(
                "DCS XML uses an unresolved namespace prefix".to_string(),
            ));
        }
    };
    if depth == 0 {
        if *root_seen {
            return Err(DcsCodecError::InvalidXml(
                "DCS XML contains multiple roots".to_string(),
            ));
        }
        *root_seen = true;
        if local != expected_root.as_bytes() || namespace != expected_namespace {
            return Err(DcsCodecError::UnsupportedLayout(format!(
                "expected {{{}}}{expected_root} root",
                expected_namespace
                    .and_then(|value| std::str::from_utf8(value).ok())
                    .unwrap_or("")
            )));
        }
    }
    inspection.has_inline_area_template |= local == b"template" && has_inline_area_template;
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DcsCodecError {
    Profile(BodyProfileError),
    Native(String),
    InvalidXml(String),
    UnsupportedLayout(String),
    UnsupportedSource(&'static str),
    /// The compiled body does not read back as the source it was compiled
    /// from, so it is refused rather than staged.
    RoundTrip(String),
    LimitExceeded(&'static str),
}

impl Display for DcsCodecError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Profile(source) => Display::fmt(source, formatter),
            Self::Native(reason) => write!(formatter, "native DCS codec rejected data: {reason}"),
            Self::InvalidXml(reason) => write!(formatter, "invalid DCS XML: {reason}"),
            Self::UnsupportedLayout(reason) => {
                write!(formatter, "unsupported DCS body layout: {reason}")
            }
            Self::UnsupportedSource(reason) => {
                write!(
                    formatter,
                    "DCS source cannot be compiled base-free: {reason}"
                )
            }
            Self::RoundTrip(reason) => {
                write!(
                    formatter,
                    "DCS source cannot be compiled base-free: {reason}"
                )
            }
            Self::LimitExceeded(field) => write!(formatter, "{field} exceeds the standalone limit"),
        }
    }
}

impl Error for DcsCodecError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Profile(source) => Some(source),
            _ => None,
        }
    }
}

impl From<BodyProfileError> for DcsCodecError {
    fn from(source: BodyProfileError) -> Self {
        Self::Profile(source)
    }
}

impl From<NativeError> for DcsCodecError {
    fn from(source: NativeError) -> Self {
        Self::Native(source.to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::compiler::families::native::deflate_bytes;
    use ibcmd_xml::{
        analyze_dcs_settings_document, compile_dcs_schema_template_source_documents,
        compile_dcs_schema_template_source_documents_with_references,
    };
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

    /// The namespace set the export declares on every source root.
    const SOURCE_ROOT_OPEN: &str = "<DataCompositionSchema xmlns=\"http://v8.1c.ru/8.1/data-composition-system/schema\" xmlns:dcscom=\"http://v8.1c.ru/8.1/data-composition-system/common\" xmlns:dcscor=\"http://v8.1c.ru/8.1/data-composition-system/core\" xmlns:dcsset=\"http://v8.1c.ru/8.1/data-composition-system/settings\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">";
    /// The declarations an inline `dcsset:settings` carries in the source.
    const INLINE_SETTINGS_NAMESPACES: &str = " xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\" xmlns:sys=\"http://v8.1c.ru/8.1/data/ui/fonts/system\" xmlns:web=\"http://v8.1c.ru/8.1/data/ui/colors/web\" xmlns:win=\"http://v8.1c.ru/8.1/data/ui/colors/windows\"";

    /// A source document the way the export writes one: BOM, declaration,
    /// the root namespace set, `\r\n` line breaks. `body` holds the root's
    /// children one per `\n`-separated line, indented with tabs; `{settings}`
    /// stands for an inline settings element's namespace declarations.
    fn canonical_source(body: &str) -> Vec<u8> {
        let mut text = String::from("\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n");
        text.push_str(SOURCE_ROOT_OPEN);
        for line in body
            .replace("{settings}", INLINE_SETTINGS_NAMESPACES)
            .lines()
        {
            text.push_str("\r\n");
            text.push_str(line);
        }
        text.push_str("\r\n</DataCompositionSchema>");
        text.into_bytes()
    }

    fn simple_schema() -> Vec<u8> {
        canonical_source(SIMPLE_SCHEMA_BODY)
    }

    const SIMPLE_SCHEMA_BODY: &str = "\t<dataSource>\n\t\t<name>Source1</name>\n\t\t<dataSourceType>Local</dataSourceType>\n\t</dataSource>\n\t<settingsVariant>\n\t\t<dcsset:name>Main</dcsset:name>\n\t\t<dcsset:presentation xsi:type=\"xs:string\">Main</dcsset:presentation>\n\t\t<dcsset:settings{settings}/>\n\t</settingsVariant>";

    /// The platform's XML load stored some of the lab cohorts' area
    /// appearance parameters under their English names (`Details`,
    /// `TextColor`, `BackColor`) while its export -- and so the source --
    /// spells them in Russian; the corpus-sized configurations (БСП, ERP УХ)
    /// store the Russian spelling their source carries. The source cannot say
    /// which spelling a load chose, so the writer keeps the source's, and the
    /// rest of each body is the platform's byte for byte.
    fn with_source_parameter_spelling(document: &[u8]) -> Vec<u8> {
        let mut out = document.to_vec();
        for (stored, source) in [
            (
                "<parameter>Details</parameter>",
                "<parameter>Расшифровка</parameter>",
            ),
            (
                "<parameter>TextColor</parameter>",
                "<parameter>ЦветТекста</parameter>",
            ),
            (
                "<parameter>BackColor</parameter>",
                "<parameter>ЦветФона</parameter>",
            ),
        ] {
            let (stored, source) = (stored.as_bytes(), source.as_bytes());
            let mut replaced = Vec::with_capacity(out.len());
            let mut index = 0;
            while index < out.len() {
                if out[index..].starts_with(stored) {
                    replaced.extend_from_slice(source);
                    index += stored.len();
                } else {
                    replaced.push(out[index]);
                    index += 1;
                }
            }
            out = replaced;
        }
        out
    }

    fn export_body(
        blob: &[u8],
        type_index: &DcsTypeIndex,
        object_refs: &BTreeMap<String, String>,
    ) -> Vec<u8> {
        let decoded = decode_compatible_dcs(DcsTemplateKind::Schema, blob).unwrap();
        crate::mssql_dump::normalize_data_composition_schema_template_documents_with_profiles(
            &decoded.documents(),
            type_index,
            object_refs,
            &ProfileId::parse("provider:mssql-legacy").unwrap(),
            &ProfileId::parse("xml-2.20").unwrap(),
        )
        .unwrap()
    }

    const SIMPLE_APPEARANCE: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<AppearanceTemplate xmlns="http://v8.1c.ru/8.1/data-composition-system/appearance-template"/>"#;

    fn synthetic_schema_document() -> Vec<u8> {
        xml_document(&format!(
            "{SCHEMA_FILE_OPEN}\r\n\t<dataCompositionSchema xmlns=\"{}\"/>\r\n</SchemaFile>",
            std::str::from_utf8(SCHEMA_NS).unwrap()
        ))
    }

    fn synthetic_schema_plain(settings: &[Vec<u8>]) -> Vec<u8> {
        assert!(!settings.is_empty());
        let schema = synthetic_schema_document();
        let trailing_schema = schema.clone();
        let mut documents = Vec::with_capacity(settings.len() + 2);
        documents.push(schema);
        documents.extend(settings.iter().cloned());
        documents.push(trailing_schema);

        let mut plain = Vec::new();
        plain.extend_from_slice(&0u32.to_le_bytes());
        plain.extend_from_slice(&(settings.len() as u32).to_le_bytes());
        for document in &documents[..documents.len() - 1] {
            plain.extend_from_slice(&(document.len() as u64).to_le_bytes());
        }
        for document in documents {
            plain.extend_from_slice(&document);
        }
        plain
    }

    #[test]
    fn schema_compiles_to_evidenced_three_document_container() {
        let profile = DcsCodecProfile::fixture();
        let source = simple_schema();
        let first = compile_dcs(&profile, DcsTemplateKind::Schema, &source).unwrap();
        let second = compile_dcs(&profile, DcsTemplateKind::Schema, &source).unwrap();
        assert_eq!(first, second);

        let decoded = decode_dcs(&profile, DcsTemplateKind::Schema, &first).unwrap();
        assert_eq!(decoded.layout(), DcsBodyLayout::NativeThreeDocument);
        assert_eq!(decoded.document_count(), 3);
        assert_eq!(
            decoded.documents()[0],
            "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<SchemaFile xmlns=\"\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\r\n\t<dataCompositionSchema xmlns=\"http://v8.1c.ru/8.1/data-composition-system/schema\">\r\n\t\t<dataSource>\r\n\t\t\t<name>Source1</name>\r\n\t\t\t<dataSourceType>Local</dataSourceType>\r\n\t\t</dataSource>\r\n\t\t<settingsVariant>\r\n\t\t\t<name xmlns=\"http://v8.1c.ru/8.1/data-composition-system/settings\">Main</name>\r\n\t\t\t<presentation xmlns=\"http://v8.1c.ru/8.1/data-composition-system/settings\" xsi:type=\"xs:string\">Main</presentation>\r\n\t\t</settingsVariant>\r\n\t</dataCompositionSchema>\r\n</SchemaFile>"
                .as_bytes()
        );
        assert_eq!(
            export_body(&first, &BTreeMap::new(), &BTreeMap::new()),
            source
        );
    }

    #[test]
    fn common_document_builder_preserves_platform_accepted_compiler_body() {
        let source = include_bytes!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/dcs-core/native/",
            "Reports/DcsCorpus/Templates/MainSchema/Ext/Template.xml"
        ));
        // manifest.json template.raw_entry.unpacked: the platform's own
        // genuine storage bytes for this exact template UUID -- a strictly
        // stronger ground truth than the pre-DCS-COMPILE-NAMESPACE-MIN-01
        // pin below, which only asserted "the compiler's own candidate was
        // VM-accepted" (manifest's own words: "semantically canonical even
        // though its internal namespace spelling is not byte-identical to
        // the platform storage body"). The compile direction now routes
        // the primary/settings documents through the evidenced
        // point-of-use namespace minimization, so the candidate is now
        // byte-identical to genuine platform storage too, not just
        // VM-accepted.
        let genuine_unpacked = include_bytes!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/dcs-core/raw/",
            "f4db0f6c-34f4-4449-995d-6265516e5fa8.0.bin"
        ));
        assert_eq!(genuine_unpacked.len(), 4_458);
        assert_eq!(
            format!("{:x}", Sha256::digest(genuine_unpacked)),
            "39790f6f4ff59a5487396eb435a12e4c1a74418c2b3750286dadac8cd40f4510"
        );
        let packed =
            compile_dcs(&DcsCodecProfile::fixture(), DcsTemplateKind::Schema, source).unwrap();
        let decoded = decode_dcs(
            &DcsCodecProfile::fixture(),
            DcsTemplateKind::Schema,
            &packed,
        )
        .unwrap();
        assert_eq!(decoded.plaintext(), genuine_unpacked);
    }

    #[test]
    fn schema_decoder_accepts_multiple_settings_documents() {
        let settings = xml_document(EMPTY_SETTINGS);
        let plain = synthetic_schema_plain(&[settings.clone(), settings]);
        let blob = deflate_bytes(&plain).unwrap();
        let decoded = decode_compatible_dcs(DcsTemplateKind::Schema, &blob).unwrap();
        assert_eq!(decoded.layout(), DcsBodyLayout::NativeThreeDocument);
        assert_eq!(decoded.document_count(), 4);
        assert_eq!(decoded.plaintext(), plain);
    }

    /// Contract change, named rather than hidden: this used to pin that a
    /// third Settings document is refused for not being enumerated. The
    /// header declares the count and the framing formula the evidence proves
    /// is uniform in it, so a longer envelope now decodes -- while the two
    /// floors that actually protect the bytes stay where they were.
    #[test]
    fn schema_decoder_admits_a_header_declared_count_beyond_the_attested_shapes() {
        let settings = xml_document(EMPTY_SETTINGS);
        let plain = synthetic_schema_plain(&[settings.clone(), settings.clone(), settings]);
        let decoded = decode_schema_plain_with_references(plain, &BTreeMap::new())
            .expect("a header-declared three-variant envelope frames consistently");
        assert_eq!(decoded.document_count(), 5);
    }

    #[test]
    fn schema_decoder_still_rejects_a_zero_settings_envelope() {
        let schema = synthetic_schema_document();
        let mut plain = Vec::new();
        plain.extend_from_slice(&0u32.to_le_bytes());
        plain.extend_from_slice(&0u32.to_le_bytes());
        plain.extend_from_slice(&(schema.len() as u64).to_le_bytes());
        plain.extend_from_slice(&schema);
        plain.extend_from_slice(&schema);
        assert!(matches!(
            decode_schema_plain_with_references(plain, &BTreeMap::new()),
            Err(DcsCodecError::UnsupportedLayout(reason))
                if reason == "DCS schema has no settings documents"
        ));
    }

    /// The count is only ever believed as far as the bytes bear it out: a
    /// header claiming one more document than the payload holds cannot frame,
    /// so it is refused rather than read past the end.
    #[test]
    fn schema_decoder_rejects_a_count_the_payload_cannot_frame() {
        let settings = xml_document(EMPTY_SETTINGS);
        let plain = synthetic_schema_plain(&[settings.clone(), settings]);
        let mut overstated = plain.clone();
        overstated[4..8].copy_from_slice(&9u32.to_le_bytes());
        assert!(matches!(
            decode_schema_plain_with_references(overstated, &BTreeMap::new()),
            Err(DcsCodecError::UnsupportedLayout(_))
        ));
    }

    #[test]
    fn platform_multi_variant_body_exposes_framed_document_roles_without_rescanning() {
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-multi-variant-envelope/raw/",
            "f4db0f6c-34f4-4449-995d-6265516e5fa8.0.deflate.b64"
        )));

        let decoded = decode_compatible_dcs(DcsTemplateKind::Schema, &packed)
            .expect("platform-attested multi-variant body must decode");
        let documents = decoded.documents();

        assert_eq!(decoded.document_count(), 4);
        assert_eq!(documents.len(), 4);
        assert_eq!(
            documents
                .iter()
                .map(|document| document.len())
                .collect::<Vec<_>>(),
            vec![3467, 1142, 826, 263]
        );
        assert!(
            documents[0]
                .windows(b"<SchemaFile".len())
                .any(|window| window == b"<SchemaFile")
        );
        assert!(
            documents[1]
                .windows(b"<Settings".len())
                .any(|window| window == b"<Settings")
        );
        assert!(
            documents[2]
                .windows(b"<Settings".len())
                .any(|window| window == b"<Settings")
        );
        assert!(
            documents[3]
                .windows(b"<SchemaFile".len())
                .any(|window| window == b"<SchemaFile")
        );
    }

    #[test]
    fn platform_multi_variant_source_compiles_and_materializes_both_settings() {
        let source = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-multi-variant-envelope/native-template.xml.b64"
        )));
        let compiled = compile_dcs(
            &DcsCodecProfile::fixture(),
            DcsTemplateKind::Schema,
            &source,
        )
        .expect("platform-attested two-variant source must compile");
        let decoded = decode_dcs(
            &DcsCodecProfile::fixture(),
            DcsTemplateKind::Schema,
            &compiled,
        )
        .unwrap();
        assert_eq!(decoded.documents().len(), 4);
        let exported =
            crate::mssql_dump::normalize_data_composition_schema_template_documents_with_profiles(
                &decoded.documents(),
                &BTreeMap::new(),
                &BTreeMap::new(),
                &ProfileId::parse("provider:mssql-legacy").unwrap(),
                &ProfileId::parse("xml-2.20").unwrap(),
            )
            .expect("compiled two-variant body must export through the shared binder");
        let exported = String::from_utf8(exported).unwrap();
        assert_eq!(exported.matches("<settingsVariant>").count(), 2);
        assert_eq!(exported.matches("<dcsset:settings").count(), 2);
        assert!(exported.contains("<dcsset:name>Main</dcsset:name>"));
        assert!(exported.contains("<dcsset:name>Secondary Secondary</dcsset:name>"));
    }

    #[test]
    fn platform_type_id_reference_source_compiles_and_exports_through_common_codec() {
        let source = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-typeid-reference/native-template.xml.b64"
        )));
        let compiled = compile_dcs(
            &DcsCodecProfile::fixture(),
            DcsTemplateKind::Schema,
            &source,
        )
        .expect("platform-attested current-config reference source must compile");
        let decoded = decode_dcs(
            &DcsCodecProfile::fixture(),
            DcsTemplateKind::Schema,
            &compiled,
        )
        .unwrap();
        let mut type_index = BTreeMap::new();
        type_index.insert(
            "488c0ffa-ef24-480c-a420-3bd2736317f9".to_owned(),
            crate::mssql_dump::DcsTypeResolution::Type {
                qname: "cfg:CatalogRef.FilterProbe".to_owned(),
            },
        );
        let exported =
            crate::mssql_dump::normalize_data_composition_schema_template_documents_with_profiles(
                &decoded.documents(),
                &type_index,
                &BTreeMap::new(),
                &ProfileId::parse("provider:mssql-legacy").unwrap(),
                &ProfileId::parse("xml-2.20").unwrap(),
            )
            .unwrap();
        let exported_text = std::str::from_utf8(&exported).unwrap();
        assert!(exported_text.contains("d5p1:CatalogRef.FilterProbe"));
        let recompiled = compile_dcs(
            &DcsCodecProfile::fixture(),
            DcsTemplateKind::Schema,
            &exported,
        )
        .unwrap();
        let redecoded = decode_dcs(
            &DcsCodecProfile::fixture(),
            DcsTemplateKind::Schema,
            &recompiled,
        )
        .unwrap();
        assert_eq!(redecoded.document_count(), decoded.document_count());
    }

    #[test]
    fn platform_query_union_link_source_compiles_and_exports_through_common_codec() {
        let source = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-query-union-link/native-template.xml.b64"
        )));
        let compiled = compile_dcs(
            &DcsCodecProfile::fixture(),
            DcsTemplateKind::Schema,
            &source,
        )
        .expect("platform-attested Query/Union/link source must compile");
        let decoded = decode_dcs(
            &DcsCodecProfile::fixture(),
            DcsTemplateKind::Schema,
            &compiled,
        )
        .unwrap();
        let exported =
            crate::mssql_dump::normalize_data_composition_schema_template_documents_with_profiles(
                &decoded.documents(),
                &BTreeMap::new(),
                &BTreeMap::new(),
                &ProfileId::parse("provider:mssql-legacy").unwrap(),
                &ProfileId::parse("xml-2.20").unwrap(),
            )
            .unwrap();
        assert!(
            std::str::from_utf8(&exported)
                .unwrap()
                .contains("DataSetUnion")
        );
        compile_dcs(
            &DcsCodecProfile::fixture(),
            DcsTemplateKind::Schema,
            &exported,
        )
        .unwrap();
    }

    /// DCS-QUERY-SECOND-FIELD-01: both directions for the second evidenced
    /// `DataSetQuery` shape.
    ///
    /// Whole-envelope byte equality between `compile_dcs(seed)` and
    /// `raw-unpacked.bin` is deliberately not asserted here: like every
    /// other query-union-link/link-* corpus, the compile direction's
    /// primary/settings documents stay blind passthrough (byte-range text
    /// splicing of the source, never routed through the typed
    /// inner-schema/query-union-link parsers), and `native-template.xml`'s
    /// `dataCompositionSchema`/`Settings` subtrees use a hoisted-namespace
    /// form while the genuine platform storage bytes for those same two
    /// documents use a minimized, point-of-use namespace form. This is the
    /// identical pre-existing primary/settings reindentation gap already
    /// documented as out of scope on
    /// `platform_area_style_item_uuid_compiles_base_free_with_resolver_full_cycle`
    /// -- confirmed here to be unrelated to the query-cohort widening
    /// itself: the terminal document (the only part of this envelope that
    /// is genuinely re-emitted rather than copied, not blind-passthrough
    /// copied) matches byte-for-byte below.
    ///
    /// Decode direction resolves the typed `Owner` field through
    /// `normalize_data_composition_schema_template_documents_with_profiles`'s
    /// `reference_types`. Re-exporting the compiled-then-decoded body is
    /// verified by content (the typed field's evidenced
    /// `d5p1:CatalogRef.FilterProbe` reference and its position after the
    /// untyped `SortKey` field) and by successful recompilation, not by
    /// byte-for-byte equality against `native-template.xml`: a direct diff
    /// showed the settings document's element structure is identical but
    /// its indentation differs by nesting depth -- the same pre-existing
    /// settings-document formatting gap (the shared settings serializer
    /// does not reproduce a source's own indentation), unrelated to the
    /// query-cohort widening under test. The genuine-bytes companion test
    /// `mssql_dump::dcs::tests::platform_query_union_link_typeid_body_exports_byte_exact_through_common_codec`
    /// independently proves byte-exact re-emission against
    /// `native-template.xml`, starting from the platform's own packed
    /// bytes rather than this compiler's blind-passthrough primary/settings
    /// documents.
    #[test]
    fn platform_query_union_link_typeid_compiles_and_decodes_byte_exact_both_directions() {
        let seed = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-query-union-link-typeid/native-template.xml.b64"
        )));
        let unpacked = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-query-union-link-typeid/raw-unpacked.bin.b64"
        )));
        // manifest.json: retained.native_template.sha256 / retained.unpacked_body.sha256
        assert_eq!(
            format!("{:x}", Sha256::digest(&seed)),
            "5ce6f74897ce0428f2898fcf54ed542dcd23af6aafdbd0f331a0fde1e18bb3be"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&unpacked)),
            "82a05801479c413d58377a6b2436b66c0e679ec94b211bbf3c8746cfd9469e44"
        );
        // document_topology (raw-unpacked.bin header): 24-byte header, primary
        // schema stored length 2001, sole settings document stored length 864;
        // the terminal document is the remaining bytes.
        let expected_terminal = unpacked[24 + 2001 + 864..].to_vec();
        assert_eq!(expected_terminal.len(), 263);

        let compiled = compile_dcs(&DcsCodecProfile::fixture(), DcsTemplateKind::Schema, &seed)
            .expect("dcs-query-union-link-typeid seed must compile base-free");
        let decoded = decode_dcs(
            &DcsCodecProfile::fixture(),
            DcsTemplateKind::Schema,
            &compiled,
        )
        .expect("compiled body must decode");
        assert_eq!(
            decoded.documents().last().copied(),
            Some(expected_terminal.as_slice())
        );

        let mut type_index = BTreeMap::new();
        type_index.insert(
            "488c0ffa-ef24-480c-a420-3bd2736317f9".to_owned(),
            crate::mssql_dump::DcsTypeResolution::Type {
                qname: "cfg:CatalogRef.FilterProbe".to_owned(),
            },
        );
        let exported =
            crate::mssql_dump::normalize_data_composition_schema_template_documents_with_profiles(
                &decoded.documents(),
                &type_index,
                &BTreeMap::new(),
                &ProfileId::parse("provider:mssql-legacy").unwrap(),
                &ProfileId::parse("xml-2.20").unwrap(),
            )
            .expect("decoded typed-field body must re-emit through the common codec");
        let exported_text = std::str::from_utf8(&exported).unwrap();
        assert!(
            exported_text.contains("d5p1:CatalogRef.FilterProbe"),
            "typed Owner field must resolve through the evidenced reference mechanism"
        );
        let sort_key_field_at = exported_text.find("<field>SortKey</field>").unwrap();
        let owner_field_at = exported_text.find("<field>Owner</field>").unwrap();
        assert!(
            sort_key_field_at < owner_field_at,
            "untyped SortKey field must precede the typed Owner field"
        );

        // Round trip: the re-exported source must itself compile base-free
        // (mirrors platform_query_union_link_source_compiles_and_exports_through_common_codec's
        // own closing assertion for the base, single-field corpus).
        compile_dcs(
            &DcsCodecProfile::fixture(),
            DcsTemplateKind::Schema,
            &exported,
        )
        .expect("re-exported typed-field source must recompile base-free");
    }

    #[test]
    fn platform_link_parameter_source_compiles_and_exports_through_common_codec() {
        let source = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-link-parameter/native-template.xml.b64"
        )));
        // manifest.json: retained.native_template.sha256
        assert_eq!(
            format!("{:x}", Sha256::digest(&source)),
            "381e86721884c63c9f99dcde21f1cd78cca07b4644714bf635e954b1f59fc698"
        );
        let compiled = compile_dcs(
            &DcsCodecProfile::fixture(),
            DcsTemplateKind::Schema,
            &source,
        )
        .expect("platform-attested link-parameter source must compile");
        let decoded = decode_dcs(
            &DcsCodecProfile::fixture(),
            DcsTemplateKind::Schema,
            &compiled,
        )
        .unwrap();
        let exported =
            crate::mssql_dump::normalize_data_composition_schema_template_documents_with_profiles(
                &decoded.documents(),
                &BTreeMap::new(),
                &BTreeMap::new(),
                &ProfileId::parse("provider:mssql-legacy").unwrap(),
                &ProfileId::parse("xml-2.20").unwrap(),
            )
            .unwrap();
        let exported_text = std::str::from_utf8(&exported).unwrap();
        assert!(exported_text.contains("<parameter>LinkParam</parameter>"));
        assert!(exported_text.contains("<parameterListAllowed>true</parameterListAllowed>"));
        compile_dcs(
            &DcsCodecProfile::fixture(),
            DcsTemplateKind::Schema,
            &exported,
        )
        .unwrap();
    }

    #[test]
    fn platform_link_expressions_source_compiles_and_exports_through_common_codec() {
        let source = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-link-expressions/native-template.xml.b64"
        )));
        // manifest.json: retained.native_template.sha256
        assert_eq!(
            format!("{:x}", Sha256::digest(&source)),
            "e80cc9492ab93cabff9799fb14e7e4c6fafff0d96129acba19ba53d4aa4faf54"
        );
        let compiled = compile_dcs(
            &DcsCodecProfile::fixture(),
            DcsTemplateKind::Schema,
            &source,
        )
        .expect("platform-attested link-expressions source must compile");
        let decoded = decode_dcs(
            &DcsCodecProfile::fixture(),
            DcsTemplateKind::Schema,
            &compiled,
        )
        .unwrap();
        let exported =
            crate::mssql_dump::normalize_data_composition_schema_template_documents_with_profiles(
                &decoded.documents(),
                &BTreeMap::new(),
                &BTreeMap::new(),
                &ProfileId::parse("provider:mssql-legacy").unwrap(),
                &ProfileId::parse("xml-2.20").unwrap(),
            )
            .unwrap();
        let exported_text = std::str::from_utf8(&exported).unwrap();
        assert!(exported_text.contains("<parameter>LinkParam</parameter>"));
        assert!(exported_text.contains("<parameterListAllowed>true</parameterListAllowed>"));
        assert!(
            exported_text
                .contains("<linkConditionExpression>SortKey &gt; 0</linkConditionExpression>")
        );
        assert!(exported_text.contains("<startExpression>SortKey</startExpression>"));
        // Non-default (EDT defaultValue "true") retained verbatim.
        assert!(exported_text.contains("<required>false</required>"));
        let condition_at = exported_text.find("linkConditionExpression").unwrap();
        let start_at = exported_text.find("startExpression").unwrap();
        let required_at = exported_text.find("<required>").unwrap();
        assert!(
            condition_at < start_at && start_at < required_at,
            "canonical order must be linkConditionExpression, startExpression, required"
        );
        let recompiled = compile_dcs(
            &DcsCodecProfile::fixture(),
            DcsTemplateKind::Schema,
            &exported,
        )
        .unwrap();
        let redecoded = decode_dcs(
            &DcsCodecProfile::fixture(),
            DcsTemplateKind::Schema,
            &recompiled,
        )
        .unwrap();
        assert_eq!(redecoded.document_count(), decoded.document_count());
    }

    #[test]
    fn schema_decoder_rejects_zero_settings_count() {
        let mut plain = synthetic_schema_plain(&[xml_document(EMPTY_SETTINGS)]);
        plain[4..8].copy_from_slice(&0u32.to_le_bytes());

        assert!(matches!(
            decode_schema_plain_with_references(plain, &BTreeMap::new()),
            Err(DcsCodecError::UnsupportedLayout(reason))
                if reason == "DCS schema has no settings documents"
        ));
    }

    #[test]
    fn schema_decoder_rejects_huge_settings_count_before_document_iteration() {
        let mut plain = vec![0u8; DCS_HEADER_BYTES];
        plain[4..8].copy_from_slice(&u32::MAX.to_le_bytes());

        assert!(matches!(
            decode_schema_plain_with_references(plain, &BTreeMap::new()),
            Err(DcsCodecError::UnsupportedLayout(reason))
                if reason == "DCS schema header is truncated"
        ));
    }

    #[test]
    fn schema_decoder_rejects_zero_stored_document_length() {
        let mut plain = synthetic_schema_plain(&[xml_document(EMPTY_SETTINGS)]);
        plain[8..16].copy_from_slice(&0u64.to_le_bytes());

        assert!(matches!(
            decode_schema_plain_with_references(plain, &BTreeMap::new()),
            Err(DcsCodecError::UnsupportedLayout(reason))
                if reason == "DCS schema document lengths are invalid"
        ));
    }

    #[test]
    fn schema_decoder_rejects_stored_document_length_past_payload() {
        let mut plain = synthetic_schema_plain(&[xml_document(EMPTY_SETTINGS)]);
        let past_payload = u64::try_from(plain.len()).unwrap();
        plain[8..16].copy_from_slice(&past_payload.to_le_bytes());

        assert!(matches!(
            decode_schema_plain_with_references(plain, &BTreeMap::new()),
            Err(DcsCodecError::UnsupportedLayout(reason))
                if reason == "DCS schema document lengths are invalid"
        ));
    }

    #[test]
    fn schema_decoder_rejects_missing_trailing_schema_document() {
        let mut plain = synthetic_schema_plain(&[xml_document(EMPTY_SETTINGS)]);
        let first_len = read_len(&plain, 8, "first document").unwrap();
        let settings_len = read_len(&plain, 16, "settings document").unwrap();
        plain.truncate(DCS_HEADER_BYTES + first_len + settings_len);

        assert!(matches!(
            decode_schema_plain_with_references(plain, &BTreeMap::new()),
            Err(DcsCodecError::UnsupportedLayout(reason))
                if reason == "DCS schema document lengths are invalid"
        ));
    }

    #[test]
    fn schema_decoder_rejects_malformed_second_settings_document() {
        let malformed = xml_document(&format!(
            "<Settings xmlns=\"{}\"><broken></Settings>",
            std::str::from_utf8(SETTINGS_NS).unwrap()
        ));
        let plain = synthetic_schema_plain(&[xml_document(EMPTY_SETTINGS), malformed]);

        assert!(matches!(
            decode_schema_plain_with_references(plain, &BTreeMap::new()),
            Err(DcsCodecError::InvalidXml(_))
        ));
    }

    #[test]
    fn non_empty_settings_survive_semantic_round_trip() {
        let profile = DcsCodecProfile::fixture();
        let source = canonical_source(
            "\t<settingsVariant>\n\t\t<dcsset:name>Main</dcsset:name>\n\t\t<dcsset:presentation xsi:type=\"xs:string\">Main</dcsset:presentation>\n\t\t<dcsset:settings{settings}>\n\t\t\t<dcsset:selection>\n\t\t\t\t<dcsset:item xsi:type=\"dcsset:SelectedItemField\">\n\t\t\t\t\t<dcsset:field>SortKey</dcsset:field>\n\t\t\t\t</dcsset:item>\n\t\t\t</dcsset:selection>\n\t\t\t<dcsset:filter>\n\t\t\t\t<dcsset:item xsi:type=\"dcsset:FilterItemComparison\">\n\t\t\t\t\t<dcsset:left xsi:type=\"dcscor:Field\">SortKey</dcsset:left>\n\t\t\t\t\t<dcsset:comparisonType>Equal</dcsset:comparisonType>\n\t\t\t\t\t<dcsset:right xsi:type=\"xs:string\">A</dcsset:right>\n\t\t\t\t</dcsset:item>\n\t\t\t</dcsset:filter>\n\t\t</dcsset:settings>\n\t</settingsVariant>",
        );

        let blob = compile_dcs(&profile, DcsTemplateKind::Schema, &source).unwrap();
        let decoded = decode_dcs(&profile, DcsTemplateKind::Schema, &blob).unwrap();
        let documents = decoded.documents();
        assert_eq!(documents.len(), 3);
        // Storage spells the settings namespace as the Settings document's
        // default, so the source's `dcsset:` prefix is gone from both the
        // element names and the `xsi:type` values.
        let settings_document = std::str::from_utf8(documents[1]).unwrap();
        assert!(settings_document.contains("<Settings "));
        assert!(settings_document.contains(
            "\r\n\t<selection>\r\n\t\t<item xsi:type=\"SelectedItemField\">\r\n\t\t\t<field>SortKey</field>"
        ));
        assert!(settings_document.contains("<comparisonType>Equal</comparisonType>"));
        assert!(settings_document.contains("<right xsi:type=\"xs:string\">A</right>"));
        assert!(!settings_document.contains("dcsset"));
        assert_eq!(
            export_body(&blob, &BTreeMap::new(), &BTreeMap::new()),
            source
        );
    }

    #[test]
    fn platform_data_parameters_source_owned_template_compiles_without_a_second_serializer() {
        let profile = DcsCodecProfile::fixture();
        let source = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-data-parameters-source-owned/native-template.xml.b64"
        )));
        let blob = compile_dcs(&profile, DcsTemplateKind::Schema, &source)
            .expect("platform-attested source-owned template must compile");
        let decoded = decode_dcs(&profile, DcsTemplateKind::Schema, &blob).unwrap();
        let exported =
            crate::mssql_dump::normalize_data_composition_schema_template_documents_with_profiles(
                &decoded.documents(),
                &BTreeMap::new(),
                &BTreeMap::new(),
                &ProfileId::parse("provider:mssql-legacy").unwrap(),
                &ProfileId::parse("xml-2.20").unwrap(),
            )
            .expect("compiled source-owned body must remain exportable");

        let documents = compile_dcs_schema_template_source_documents(&exported).unwrap();
        assert_eq!(documents.settings().len(), 1);
        let settings = std::str::from_utf8(&documents.settings()[0]).unwrap();
        let analysis = analyze_dcs_settings_document(settings).unwrap();
        assert_eq!(analysis.source_owned().len(), 2);
        assert!(settings.contains("<dcscor:parameter>Caption</dcscor:parameter>"));
        assert!(
            settings.contains("<dcscor:value xsi:type=\"xs:string\">Opaque probe</dcscor:value>")
        );
    }

    #[test]
    fn platform_style_free_area_template_compiles_to_exact_terminal_document() {
        let profile = DcsCodecProfile::fixture();
        let source = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-template/native-template.xml.b64"
        )));
        let expected_area = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-template/area-schema-file.xml.b64"
        )));
        let unpacked = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-template/raw-unpacked.bin.b64"
        )));
        assert_eq!(
            format!("{:x}", Sha256::digest(&unpacked)),
            "7047c6cfe75f1eb4241572bf350c273a6a1b8996c7350b75b557def2a6d8b7a6"
        );
        // Since DCS-COMPILE-NAMESPACE-MIN-01 routes the primary/settings
        // documents through the evidenced point-of-use namespace
        // minimization (the trivial dcs-core base schema this cohort's
        // primary shares with every other dcs-area-* corpus), the whole
        // compiled envelope now matches raw-unpacked.bin byte for byte.
        let blob = compile_dcs(&profile, DcsTemplateKind::Schema, &source).unwrap();
        assert_eq!(inflate(&blob).unwrap(), unpacked);
        let decoded = decode_dcs(&profile, DcsTemplateKind::Schema, &blob).unwrap();
        assert_eq!(
            decoded.documents().last().copied(),
            Some(expected_area.as_slice())
        );
        let exported =
            crate::mssql_dump::normalize_data_composition_schema_template_documents_with_profiles(
                &decoded.documents(),
                &BTreeMap::new(),
                &BTreeMap::new(),
                &ProfileId::parse("provider:mssql-legacy").unwrap(),
                &ProfileId::parse("xml-2.20").unwrap(),
            )
            .unwrap();
        let exported_text = std::str::from_utf8(&exported).unwrap();
        assert!(exported_text.contains("<name>AreaProbe</name>"));
        assert!(exported_text.contains("xsi:type=\"dcsat:AreaTemplate\""));
        let rebuilt = compile_dcs_schema_template_source_documents(&exported).unwrap();
        assert_eq!(rebuilt.terminal_schema_file(), expected_area);
    }

    #[test]
    fn platform_area_appearance_compiles_to_exact_app_index_side_table() {
        let profile = DcsCodecProfile::fixture();
        let source = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-template-appearance/native-template.xml.b64"
        )));
        let expected_area = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-template-appearance/area-schema-file.xml.b64"
        )));
        let unpacked = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-template-appearance/raw-unpacked.bin.b64"
        )));
        assert_eq!(
            format!("{:x}", Sha256::digest(&unpacked)),
            "0b1b8c051c0dffdb8546068733347900f310e7c921ca6ecc275bbeb87845af22"
        );
        // Since DCS-COMPILE-NAMESPACE-MIN-01 routes the primary/settings
        // documents through the evidenced point-of-use namespace
        // minimization, the whole compiled envelope now matches
        // raw-unpacked.bin byte for byte.
        let blob = compile_dcs(&profile, DcsTemplateKind::Schema, &source).unwrap();
        assert_eq!(inflate(&blob).unwrap(), unpacked);
        let decoded = decode_dcs(&profile, DcsTemplateKind::Schema, &blob).unwrap();
        if let Ok(path) = std::env::var("IBCMD_DCS_CANDIDATE_OUT") {
            std::fs::write(path, decoded.plaintext()).unwrap();
        }
        assert_eq!(
            decoded.documents().last().copied(),
            Some(expected_area.as_slice())
        );
        let exported =
            crate::mssql_dump::normalize_data_composition_schema_template_documents_with_profiles(
                &decoded.documents(),
                &BTreeMap::new(),
                &BTreeMap::new(),
                &ProfileId::parse("provider:mssql-legacy").unwrap(),
                &ProfileId::parse("xml-2.20").unwrap(),
            )
            .unwrap();
        let exported_text = std::str::from_utf8(&exported).unwrap();
        assert!(exported_text.contains("<dcscor:parameter>Расшифровка</dcscor:parameter>"));
        assert!(!exported_text.contains("appIndex"));
        let rebuilt = compile_dcs_schema_template_source_documents(&exported).unwrap();
        assert_eq!(rebuilt.terminal_schema_file(), expected_area);
    }

    #[test]
    fn platform_area_appearance_web_color_compiles_to_exact_side_table() {
        let profile = DcsCodecProfile::fixture();
        let source = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-appearance-web-color/native-template.xml.b64"
        )));
        let unpacked = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-appearance-web-color/raw-unpacked.bin.b64"
        )));
        assert_eq!(
            format!("{:x}", Sha256::digest(&source)),
            "7ca981cac18c0df2715d355eb6cf97665f80bd5a027dc67764b322b818a51a25"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&unpacked)),
            "2a78f4fd6218295397d6841d09e6652775f5b063397270e4729381c8ece79831"
        );
        // document_topology (manifest.json): header 24 bytes, one stored
        // length for the primary schema (3029) and one for the sole
        // settings document (1142); the terminal side-table SchemaFile is
        // the remaining bytes.
        let expected_area = unpacked[24 + 3029 + 1142..].to_vec();
        assert_eq!(expected_area.len(), 1614);
        assert_eq!(
            format!("{:x}", Sha256::digest(&expected_area)),
            "d8f30afc51eb97f8de38e339bf6d2aee0b1065d5a1333464e7a1d938ca93bfac"
        );

        // seed(source) XML -> body: compiling the evidenced native
        // document must reproduce the exact platform-observed terminal
        // side-table document, including the new color side-table bytes
        // (matching the scope b4bba2d's own equivalent test proves). Since
        // DCS-COMPILE-NAMESPACE-MIN-01 routes the primary/settings
        // documents through the evidenced point-of-use namespace
        // minimization, the whole compiled envelope now matches
        // raw-unpacked.bin byte for byte too.
        let blob = compile_dcs(&profile, DcsTemplateKind::Schema, &source).unwrap();
        assert_eq!(
            inflate(&blob).unwrap(),
            with_source_parameter_spelling(&unpacked)
        );
        let decoded = decode_dcs(&profile, DcsTemplateKind::Schema, &blob).unwrap();
        assert_eq!(
            decoded.documents().last().copied(),
            Some(with_source_parameter_spelling(&expected_area).as_slice())
        );

        // body -> XML: re-exporting the compiled body must reproduce the
        // evidenced color appearance verbatim.
        let exported =
            crate::mssql_dump::normalize_data_composition_schema_template_documents_with_profiles(
                &decoded.documents(),
                &BTreeMap::new(),
                &BTreeMap::new(),
                &ProfileId::parse("provider:mssql-legacy").unwrap(),
                &ProfileId::parse("xml-2.20").unwrap(),
            )
            .unwrap();
        let exported_text = std::str::from_utf8(&exported).unwrap();
        assert!(exported_text.contains("<dcscor:parameter>ЦветТекста</dcscor:parameter>"));
        assert!(exported_text.contains("d8p1:Red"));
        assert!(!exported_text.contains("appIndex"));
        let color_at = exported_text.find("ЦветТекста").unwrap();
        let details_at = exported_text.find("Расшифровка").unwrap();
        assert!(color_at < details_at, "color item must precede Расшифровка");

        // XML -> body: recompiling the exported source must reproduce the
        // exact terminal side-table bytes the platform emitted (the same
        // property `body -> XML -> body == raw-unpacked` pins for the
        // terminal document).
        let rebuilt = compile_dcs_schema_template_source_documents(&exported).unwrap();
        assert_eq!(
            rebuilt.terminal_schema_file(),
            with_source_parameter_spelling(&expected_area)
        );
    }

    #[test]
    fn platform_area_appearance_web_color_native_packed_body_exports_exact_native_template() {
        let profile = DcsCodecProfile::fixture();
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-appearance-web-color/raw-packed.bin.b64"
        )));
        let native_template = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-appearance-web-color/native-template.xml.b64"
        )));
        assert_eq!(
            format!("{:x}", Sha256::digest(&packed)),
            "5ba80e683598577102f59d71e55e781d7bc411574d3d6779db0888397292cab3"
        );

        // This decodes the platform's own deflate-compressed bytes
        // directly -- not anything ibcmd-rs compiled -- so the exact
        // re-export match below is independent of our own compiler
        // direction being correct.
        let decoded = decode_dcs(&profile, DcsTemplateKind::Schema, &packed)
            .expect("genuine platform-packed body must decode");
        let exported =
            crate::mssql_dump::normalize_data_composition_schema_template_documents_with_profiles(
                &decoded.documents(),
                &BTreeMap::new(),
                &BTreeMap::new(),
                &ProfileId::parse("provider:mssql-legacy").unwrap(),
                &ProfileId::parse("xml-2.20").unwrap(),
            )
            .expect("genuine platform side table must remain exportable");
        assert_eq!(exported, native_template);
    }

    #[test]
    fn area_appearance_web_color_seed_order_is_kept_as_written() {
        let profile = DcsCodecProfile::fixture();
        let seed = include_bytes!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-appearance-web-color/seed/Template.xml"
        ));
        // The seed is explicitly non-authoritative (manifest:
        // "hypothesis_only") and orders the appearance items
        // Расшифровка-then-ЦветТекста -- the reverse of what
        // native-template.xml (the platform-proven order) actually uses --
        // and spells the web colour through a `web` prefix of its own. The
        // writer neither reorders nor re-spells it: the body keeps what the
        // seed says, and the export gives the seed back byte for byte.
        let blob = compile_dcs(&profile, DcsTemplateKind::Schema, seed).unwrap();
        assert_eq!(
            export_body(&blob, &BTreeMap::new(), &BTreeMap::new()),
            seed.as_slice()
        );
    }

    #[test]
    fn platform_area_style_color_reference_compiles_to_exact_side_table() {
        let profile = DcsCodecProfile::fixture();
        let source = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-style-color-reference/native-template.xml.b64"
        )));
        let unpacked = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-style-color-reference/raw-unpacked.bin.b64"
        )));
        assert_eq!(
            format!("{:x}", Sha256::digest(&source)),
            "4269ac193b76bb88ecaaf65a5b4ef9ed12a31cdcf1d36d8ac429de68cf10f970"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&unpacked)),
            "1c18c7ed371e1fad4c56b8d9e48000455e752578e804bab19ffb9153293fbf72"
        );
        // document_topology (manifest.json): header 24 bytes, one stored
        // length for the primary schema (3029) and one for the sole
        // settings document (1142); the terminal side-table SchemaFile is
        // the remaining bytes.
        let expected_area = unpacked[24 + 3029 + 1142..].to_vec();
        assert_eq!(expected_area.len(), 1623);
        assert_eq!(
            format!("{:x}", Sha256::digest(&expected_area)),
            "c5879edc2a5776e5be6ae52f3843c2780dc141f74ffc221e7626bc442440cb08"
        );

        // seed(source) XML -> body: the standard/built-in style reference
        // needs no resolver on either direction, so the plain compiler
        // entry points must already reproduce the exact platform-observed
        // terminal side-table document byte for byte. Since
        // DCS-COMPILE-NAMESPACE-MIN-01 routes the primary/settings
        // documents through the evidenced point-of-use namespace
        // minimization (the trivial dcs-core base schema this cohort's
        // primary shares with every other dcs-area-* corpus), the whole
        // compiled envelope now matches raw-unpacked.bin byte for byte too.
        let blob = compile_dcs(&profile, DcsTemplateKind::Schema, &source).unwrap();
        assert_eq!(
            inflate(&blob).unwrap(),
            with_source_parameter_spelling(&unpacked)
        );
        let decoded = decode_dcs(&profile, DcsTemplateKind::Schema, &blob).unwrap();
        assert_eq!(
            decoded.documents().last().copied(),
            Some(with_source_parameter_spelling(&expected_area).as_slice())
        );

        // body -> XML: re-exporting the compiled body must reproduce the
        // evidenced style reference verbatim.
        let exported =
            crate::mssql_dump::normalize_data_composition_schema_template_documents_with_profiles(
                &decoded.documents(),
                &BTreeMap::new(),
                &BTreeMap::new(),
                &ProfileId::parse("provider:mssql-legacy").unwrap(),
                &ProfileId::parse("xml-2.20").unwrap(),
            )
            .unwrap();
        let exported_text = std::str::from_utf8(&exported).unwrap();
        assert!(exported_text.contains("<dcscor:parameter>ЦветФона</dcscor:parameter>"));
        assert!(exported_text.contains("d8p1:NegativeTextColor"));
        assert!(!exported_text.contains("appIndex"));
        let color_at = exported_text.find("ЦветФона").unwrap();
        let details_at = exported_text.find("Расшифровка").unwrap();
        assert!(
            color_at < details_at,
            "style-reference item must precede Расшифровка"
        );

        // XML -> body: recompiling the exported source must reproduce the
        // exact terminal side-table bytes the platform emitted (the same
        // property `body -> XML -> body == raw-unpacked` pins for the
        // terminal document).
        let rebuilt = compile_dcs_schema_template_source_documents(&exported).unwrap();
        assert_eq!(
            rebuilt.terminal_schema_file(),
            with_source_parameter_spelling(&expected_area)
        );
    }

    #[test]
    fn platform_area_style_color_reference_native_packed_body_exports_exact_native_template() {
        let profile = DcsCodecProfile::fixture();
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-style-color-reference/raw-packed.bin.b64"
        )));
        let native_template = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-style-color-reference/native-template.xml.b64"
        )));
        assert_eq!(
            format!("{:x}", Sha256::digest(&packed)),
            "1e6c10a050235b9ecd42b1b7bdcdb3df5b148bde0c42cb442ba7bd16722cdf9b"
        );

        // Decodes the platform's own deflate-compressed bytes directly --
        // not anything ibcmd-rs compiled -- so the exact re-export match
        // below is independent of our own compiler direction being
        // correct.
        let decoded = decode_dcs(&profile, DcsTemplateKind::Schema, &packed)
            .expect("genuine platform-packed body must decode");
        let exported =
            crate::mssql_dump::normalize_data_composition_schema_template_documents_with_profiles(
                &decoded.documents(),
                &BTreeMap::new(),
                &BTreeMap::new(),
                &ProfileId::parse("provider:mssql-legacy").unwrap(),
                &ProfileId::parse("xml-2.20").unwrap(),
            )
            .expect("genuine platform side table must remain exportable");
        assert_eq!(exported, native_template);
    }

    /// A custom-StyleItem reference is stored by the style item's uuid when
    /// the resolver names it, and by name otherwise: the platform stores a
    /// configuration style item as `0:<uuid>`, but a style name it cannot
    /// resolve is a QName the exporter reads back the same, so the body
    /// compiles either way and only the spelling differs.
    #[test]
    fn area_style_item_uuid_compile_direction_gates_resolver() {
        let profile = DcsCodecProfile::fixture();
        let source = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-style-item-uuid/native-template.xml.b64"
        )));
        let stored_by_uuid = |blob: &[u8]| {
            String::from_utf8_lossy(&inflate(blob).unwrap())
                .contains("0:4a9d8536-ff59-4a90-a1cf-646d241dc53c")
        };
        // Without a resolver, or with one that lacks this style item, the
        // reference stays spelled by name.
        let mut irrelevant_only = BTreeMap::new();
        irrelevant_only.insert(
            "00000000-0000-0000-0000-000000000000".to_string(),
            "SomeOtherStyleItem".to_string(),
        );
        for blob in [
            compile_dcs(&profile, DcsTemplateKind::Schema, &source).unwrap(),
            compile_dcs_with_references(
                &profile,
                DcsTemplateKind::Schema,
                &source,
                &BTreeMap::new(),
            )
            .unwrap(),
            compile_dcs_with_references(
                &profile,
                DcsTemplateKind::Schema,
                &source,
                &irrelevant_only,
            )
            .unwrap(),
        ] {
            assert!(!stored_by_uuid(&blob));
            assert_eq!(
                export_body(&blob, &BTreeMap::new(), &BTreeMap::new()),
                source
            );
        }
        // With the evidenced resolver entry the reference is stored by uuid,
        // and an extra, unrelated entry beside it is never looked up.
        let mut style_reference_types = BTreeMap::new();
        style_reference_types.insert(
            "4a9d8536-ff59-4a90-a1cf-646d241dc53c".to_string(),
            "CorpusAccent".to_string(),
        );
        let mut with_extra_entry = style_reference_types.clone();
        with_extra_entry.insert(
            "11111111-1111-1111-1111-111111111111".to_string(),
            "UnrelatedStyleItem".to_string(),
        );
        let mut object_refs = BTreeMap::new();
        object_refs.insert(
            "4a9d8536-ff59-4a90-a1cf-646d241dc53c".to_string(),
            "StyleItem.CorpusAccent".to_string(),
        );
        for references in [&style_reference_types, &with_extra_entry] {
            let blob =
                compile_dcs_with_references(&profile, DcsTemplateKind::Schema, &source, references)
                    .unwrap();
            assert!(stored_by_uuid(&blob));
            assert_eq!(export_body(&blob, &BTreeMap::new(), &object_refs), source);
        }
    }

    /// The resolver gate this pins moved one stage later.
    ///
    /// Decoding is framing plus role assignment, and this fixture's terminal
    /// document is a well-formed `SchemaFile` carrying a `dataCompositionSchema`
    /// and one area-template `appearance` whether or not a `StyleItem`
    /// resolver is on hand -- so the envelope, which now admits any terminal of
    /// that frame so real configurations' area templates can be transliterated,
    /// answers yes either way. What must still fail closed without the
    /// resolver is producing source bytes, and
    /// `crate::mssql_dump::dcs::tests::platform_area_style_item_uuid_exports_byte_exact_through_common_codec`
    /// is where that is now asserted: a `0:<uuid>` the object-reference index
    /// cannot name is refused rather than written in its storage spelling.
    #[test]
    fn area_style_item_uuid_strict_decode_admits_the_frame_without_a_resolver() {
        let profile = DcsCodecProfile::fixture();
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-style-item-uuid/raw-packed.bin.b64"
        )));
        assert_eq!(
            format!("{:x}", Sha256::digest(&packed)),
            "680d04d34a12c54be75ac69a5c20ff82d2136736e11998b070c77d7abbbe3235"
        );
        for resolver in [
            BTreeMap::new(),
            BTreeMap::from([(
                "00000000-0000-0000-0000-000000000000".to_string(),
                "SomeOtherStyleItem".to_string(),
            )]),
        ] {
            assert!(
                decode_dcs_with_references(&profile, DcsTemplateKind::Schema, &packed, &resolver)
                    .is_ok(),
                "the terminal frame is decidable without naming the style item"
            );
        }
        assert!(decode_dcs(&profile, DcsTemplateKind::Schema, &packed).is_ok());
        // With the evidenced resolver entry: decodes through the typed
        // AreaTemplate coordinate rather than the frame check.
        let mut style_reference_types = BTreeMap::new();
        style_reference_types.insert(
            "4a9d8536-ff59-4a90-a1cf-646d241dc53c".to_string(),
            "CorpusAccent".to_string(),
        );
        assert!(
            decode_dcs_with_references(
                &profile,
                DcsTemplateKind::Schema,
                &packed,
                &style_reference_types,
            )
            .is_ok()
        );
        // An extra, unrelated entry alongside the needed one is silently
        // ignored (never looked up), matching the TypeId precedent's
        // lookup-only semantics documented on `compile_dcs_with_references`.
        let mut with_extra_entry = style_reference_types.clone();
        with_extra_entry.insert(
            "11111111-1111-1111-1111-111111111111".to_string(),
            "UnrelatedStyleItem".to_string(),
        );
        assert!(
            decode_dcs_with_references(
                &profile,
                DcsTemplateKind::Schema,
                &packed,
                &with_extra_entry,
            )
            .is_ok()
        );
    }

    /// Gate test: the exact full cycle the fourth lab session's acceptance
    /// attempt could not run (see `docs/evidence/dcs-style-link-probes-2214-20260814.md`,
    /// A6 `NOT-COMPILABLE`) now works base-free with a resolver built from
    /// the same fact the platform's own `StyleItems/<Name>.xml` source
    /// object carries -- its `uuid` attribute -- exactly as the seed's
    /// `StyleItems-CorpusAccent.xml` does. Native `Template.xml` compiles
    /// to a terminal side-table document byte-identical to the one the
    /// platform actually emitted inside `raw-unpacked.bin` (manifest-pinned
    /// sha256, same `document_topology` slicing convention the sibling
    /// `platform_area_style_color_reference_compiles_to_exact_side_table`
    /// and `platform_area_appearance_web_color_compiles_to_exact_side_table`
    /// tests use), and decoding that same body and re-emitting through the
    /// shared codec reproduces the exact evidenced content again -- proving
    /// compile and decode agree, not just that each independently produces
    /// *something*.
    ///
    /// Whole-envelope byte equality against `raw-unpacked.bin` (primary
    /// schema + settings document, not just the terminal side table) IS
    /// asserted here as of DCS-COMPILE-NAMESPACE-MIN-01. The pre-existing
    /// settings/primary reindentation gap (A6-BODY-DIFF-01: candidate used
    /// a hoisted-namespace form -- `v8:Type`, `dcsset:field`, ... -- while
    /// genuine platform storage uses a minimized, point-of-use namespace
    /// form -- `Type xmlns="..."`, bare `field`, ...) is exactly what this
    /// corpus's real platform `config export` choked on ("Stream format
    /// error", zero-length re-exported Template.xml, while import/apply
    /// accepted the mismatched candidate without complaint). The compile
    /// direction now routes the primary document through the evidenced
    /// point-of-use minimization for this cohort's trivial dcs-core base
    /// schema (shared byte-for-byte with every other dcs-area-* corpus),
    /// and the settings document through the general evidenced
    /// `dcsset:`-prefix-stripping minimizer -- closing the gap this
    /// specific corpus's failed platform re-export flagged.
    #[test]
    fn platform_area_style_item_uuid_compiles_base_free_with_resolver_full_cycle() {
        let profile = DcsCodecProfile::fixture();
        let source = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-style-item-uuid/native-template.xml.b64"
        )));
        let unpacked = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-style-item-uuid/raw-unpacked.bin.b64"
        )));
        // manifest.json: retained.native_template.sha256 / retained.unpacked_body.sha256
        assert_eq!(
            format!("{:x}", Sha256::digest(&source)),
            "98f1857d3424198275cc35834a6635c28623568aae8d01a95cb5e220f91b818f"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&unpacked)),
            "611abdfc11a72f536ecc90d134f0d0c17b833a87a228cecd1275231701825ef5"
        );
        // document_topology (manifest.json): header 24 bytes, one stored
        // length for the primary schema (3029) and one for the sole
        // settings document (1142); the terminal side-table SchemaFile is
        // the remaining bytes.
        let expected_area = unpacked[24 + 3029 + 1142..].to_vec();
        assert_eq!(expected_area.len(), 1592);
        assert_eq!(
            format!("{:x}", Sha256::digest(&expected_area)),
            "44e4c0a63f776b90901dc60a1b6cd7545413e8813e2cef63abf39a36c7c0f923"
        );

        // The resolver map: uuid (from seed/StyleItems-CorpusAccent.xml's
        // own `uuid="..."` attribute, the same source tree fact a real
        // compiler caller would scan) -> semantic name (from that same
        // file's Properties/Name). Building this belongs to the caller, not
        // this codec -- this map is the test's stand-in for that.
        let mut style_reference_types = BTreeMap::new();
        style_reference_types.insert(
            "4a9d8536-ff59-4a90-a1cf-646d241dc53c".to_string(),
            "CorpusAccent".to_string(),
        );

        // source XML -> body: must reproduce the exact platform-observed
        // whole envelope (primary + settings + terminal side table),
        // including the custom-StyleItem uuid coordinate -- this is the
        // exact "Stream format error" fix.
        let blob = compile_dcs_with_references(
            &profile,
            DcsTemplateKind::Schema,
            &source,
            &style_reference_types,
        )
        .expect("custom-StyleItem source must compile base-free with the resolver");
        assert_eq!(
            inflate(&blob).unwrap(),
            with_source_parameter_spelling(&unpacked)
        );
        let decoded = decode_dcs_with_references(
            &profile,
            DcsTemplateKind::Schema,
            &blob,
            &style_reference_types,
        )
        .expect("compiled body must decode base-free with the same resolver");
        assert_eq!(
            decoded.documents().last().copied(),
            Some(with_source_parameter_spelling(&expected_area).as_slice())
        );

        // body -> XML: re-exporting the compiled body must reproduce the
        // evidenced custom-StyleItem reference verbatim. (Byte-exact
        // whole-document equality against native-template.xml is not
        // asserted here: this is the separate decode/export direction's
        // own settings-document reindentation gap in
        // `normalize_data_composition_schema_template_documents_with_profiles`,
        // untouched by DCS-COMPILE-NAMESPACE-MIN-01's compile-direction-only
        // scope; the genuine-bytes test below proves byte-exact
        // whole-document equality on that direction instead.)
        let object_refs = {
            let mut object_refs = BTreeMap::new();
            object_refs.insert(
                "4a9d8536-ff59-4a90-a1cf-646d241dc53c".to_string(),
                "StyleItem.CorpusAccent".to_string(),
            );
            object_refs
        };
        let exported =
            crate::mssql_dump::normalize_data_composition_schema_template_documents_with_profiles(
                &decoded.documents(),
                &BTreeMap::new(),
                &object_refs,
                &ProfileId::parse("provider:mssql-legacy").unwrap(),
                &ProfileId::parse("xml-2.20").unwrap(),
            )
            .expect("decoded body must remain exportable");
        let exported_text = std::str::from_utf8(&exported).unwrap();
        assert!(exported_text.contains("<dcscor:parameter>ЦветФона</dcscor:parameter>"));
        assert!(exported_text.contains("d8p1:CorpusAccent"));
        let color_at = exported_text.find("ЦветФона").unwrap();
        let details_at = exported_text.find("Расшифровка").unwrap();
        assert!(
            color_at < details_at,
            "style-reference item must precede Расшифровка"
        );

        // XML -> body: recompiling the exported source (with the same
        // resolver, since the exported source still carries the
        // name-lexical `d8p1:CorpusAccent` form that must resolve back to
        // the uuid for storage) must reproduce the exact terminal
        // side-table bytes the platform emitted.
        let rebuilt = compile_dcs_schema_template_source_documents_with_references(
            &exported,
            &style_reference_types,
        )
        .expect("re-exported custom-StyleItem source must recompile base-free");
        assert_eq!(
            rebuilt.terminal_schema_file(),
            with_source_parameter_spelling(&expected_area)
        );
    }

    /// Genuine-bytes companion to the gate test above: decodes the
    /// platform's own deflate-compressed bytes directly -- not anything
    /// ibcmd-rs compiled -- so the exact re-export match below is
    /// independent of our own compiler direction's primary/settings
    /// document emission. Mirrors
    /// `platform_area_style_color_reference_native_packed_body_exports_exact_native_template`
    /// and
    /// `platform_area_appearance_web_color_native_packed_body_exports_exact_native_template`,
    /// using the resolver-aware strict decode path this work package added
    /// since this coordinate's storage form needs one.
    #[test]
    fn platform_area_style_item_uuid_native_packed_body_exports_exact_native_template() {
        let profile = DcsCodecProfile::fixture();
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-style-item-uuid/raw-packed.bin.b64"
        )));
        let native_template = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-style-item-uuid/native-template.xml.b64"
        )));
        assert_eq!(
            format!("{:x}", Sha256::digest(&packed)),
            "680d04d34a12c54be75ac69a5c20ff82d2136736e11998b070c77d7abbbe3235"
        );

        let mut style_reference_types = BTreeMap::new();
        style_reference_types.insert(
            "4a9d8536-ff59-4a90-a1cf-646d241dc53c".to_string(),
            "CorpusAccent".to_string(),
        );
        let decoded = decode_dcs_with_references(
            &profile,
            DcsTemplateKind::Schema,
            &packed,
            &style_reference_types,
        )
        .expect("genuine platform-packed body must decode base-free with the resolver");
        let object_refs = {
            let mut object_refs = BTreeMap::new();
            object_refs.insert(
                "4a9d8536-ff59-4a90-a1cf-646d241dc53c".to_string(),
                "StyleItem.CorpusAccent".to_string(),
            );
            object_refs
        };
        let exported =
            crate::mssql_dump::normalize_data_composition_schema_template_documents_with_profiles(
                &decoded.documents(),
                &BTreeMap::new(),
                &object_refs,
                &ProfileId::parse("provider:mssql-legacy").unwrap(),
                &ProfileId::parse("xml-2.20").unwrap(),
            )
            .expect("genuine platform side table must remain exportable");
        assert_eq!(exported, native_template);
    }

    #[test]
    fn area_style_color_reference_seed_order_is_kept_as_written() {
        let profile = DcsCodecProfile::fixture();
        let seed = include_bytes!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-style-color-reference/seed/Template.xml"
        ));
        // The seed is explicitly non-authoritative (manifest:
        // "hypothesis_only") and orders the appearance items
        // Расшифровка-then-ЦветФона -- the reverse of what
        // native-template.xml (the platform-proven order) actually uses.
        // The writer keeps the seed's order; the export gives it back.
        let blob = compile_dcs(&profile, DcsTemplateKind::Schema, seed).unwrap();
        assert_eq!(
            export_body(&blob, &BTreeMap::new(), &BTreeMap::new()),
            seed.as_slice()
        );
    }

    #[test]
    fn platform_multi_cell_appearance_compiles_to_exact_side_table() {
        let profile = DcsCodecProfile::fixture();
        let source = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-multi-cell-appearance/native-template.xml.b64"
        )));
        let unpacked = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-multi-cell-appearance/raw-unpacked.bin.b64"
        )));
        assert_eq!(
            format!("{:x}", Sha256::digest(&source)),
            "a72d97cfe65a43326433ce1bcfe80f7adf6b6ecfa50074d5d092461db52080d0"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&unpacked)),
            "4092d3955a72ee99a7ef5f7ec5eaafcdc80fdead9fb9d8608af55e89c5f75106"
        );
        // document_topology (manifest.json): header 24 bytes, one stored
        // length for the primary schema (3029) and one for the sole
        // settings document (1142); the terminal side-table SchemaFile is
        // the remaining bytes.
        let expected_area = unpacked[24 + 3029 + 1142..].to_vec();
        assert_eq!(expected_area.len(), 1953);
        assert_eq!(
            format!("{:x}", Sha256::digest(&expected_area)),
            "2aed0a189339c894d16ab61c7a5fd89c08c30aec1a43e353085c8e6c20a10d90"
        );

        // seed(source) XML -> body: compiling the evidenced native document
        // must reproduce the exact platform-observed terminal side-table
        // document, including the shared appIndex=0 referenced by both
        // row-1 cells and the single `Details`-spelled side-table record.
        // Since DCS-COMPILE-NAMESPACE-MIN-01 routes the primary/settings
        // documents through the evidenced point-of-use namespace
        // minimization, the whole compiled envelope now matches
        // raw-unpacked.bin byte for byte too.
        let blob = compile_dcs(&profile, DcsTemplateKind::Schema, &source).unwrap();
        assert_eq!(
            inflate(&blob).unwrap(),
            with_source_parameter_spelling(&unpacked)
        );
        let decoded = decode_dcs(&profile, DcsTemplateKind::Schema, &blob).unwrap();
        assert_eq!(
            decoded.documents().last().copied(),
            Some(with_source_parameter_spelling(&expected_area).as_slice())
        );

        // body -> XML: re-exporting the compiled body must reproduce the
        // evidenced shared-row appearance verbatim.
        let exported =
            crate::mssql_dump::normalize_data_composition_schema_template_documents_with_profiles(
                &decoded.documents(),
                &BTreeMap::new(),
                &BTreeMap::new(),
                &ProfileId::parse("provider:mssql-legacy").unwrap(),
                &ProfileId::parse("xml-2.20").unwrap(),
            )
            .unwrap();
        let exported_text = std::str::from_utf8(&exported).unwrap();
        assert_eq!(
            exported_text.matches("<dcsat:tableCell>").count(),
            3,
            "two row-1 cells plus one row-2 cell"
        );
        assert_eq!(exported_text.matches("<dcsat:appearance>").count(), 2);
        assert!(!exported_text.contains("appIndex"));

        // XML -> body: recompiling the exported source must reproduce the
        // exact terminal side-table bytes the platform emitted (the same
        // property `body -> XML -> body == raw-unpacked` pins for the
        // terminal document).
        let rebuilt = compile_dcs_schema_template_source_documents(&exported).unwrap();
        assert_eq!(
            rebuilt.terminal_schema_file(),
            with_source_parameter_spelling(&expected_area)
        );
    }

    #[test]
    fn platform_multi_cell_appearance_native_packed_body_exports_exact_native_template() {
        let profile = DcsCodecProfile::fixture();
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-multi-cell-appearance/raw-packed.bin.b64"
        )));
        let native_template = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-multi-cell-appearance/native-template.xml.b64"
        )));
        assert_eq!(
            format!("{:x}", Sha256::digest(&packed)),
            "9e34a9b85eba342e1cc3eb0d6e62c6ae1d230a32696f11408cb94c375a1d5723"
        );

        // Decodes the platform's own deflate-compressed bytes directly --
        // not anything ibcmd-rs compiled -- so the exact re-export match
        // below is independent of our own compiler direction being
        // correct, and definitively answers the manifest's own open
        // question (shared vs. duplicated appIndex) from genuine bytes.
        let decoded = decode_dcs(&profile, DcsTemplateKind::Schema, &packed)
            .expect("genuine platform-packed body must decode");
        let exported =
            crate::mssql_dump::normalize_data_composition_schema_template_documents_with_profiles(
                &decoded.documents(),
                &BTreeMap::new(),
                &BTreeMap::new(),
                &ProfileId::parse("provider:mssql-legacy").unwrap(),
                &ProfileId::parse("xml-2.20").unwrap(),
            )
            .expect("genuine platform side table must remain exportable");
        assert_eq!(exported, native_template);
    }

    #[test]
    fn area_multi_cell_appearance_seed_order_is_rejected_fail_closed() {
        let profile = DcsCodecProfile::fixture();
        let seed = include_bytes!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-area-multi-cell-appearance/seed/Template.xml"
        ));
        // The seed is explicitly non-authoritative (manifest:
        // "hypothesis_only") and spells each row-1 cell
        // appearance-before-Field, the reverse of what native output and
        // storage always canonicalize to. The compiler must reject this
        // order, not silently accept or reorder it.
        assert!(matches!(
            compile_dcs(&profile, DcsTemplateKind::Schema, seed),
            Err(DcsCodecError::UnsupportedSource(_))
        ));
    }

    #[test]
    fn platform_output_parameters_native_packed_body_exports_exact_native_template() {
        let profile = DcsCodecProfile::fixture();
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-output-parameters/raw-packed.bin.b64"
        )));
        let native_template = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-output-parameters/native-template.xml.b64"
        )));
        // manifest.json: rounds.packed_body_sha256 / rounds.native_template_sha256
        assert_eq!(
            format!("{:x}", Sha256::digest(&packed)),
            "1bab2e8e93c491f33d094473d8456e7877c1673d45656cca9d4729ea40c82fd7"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&native_template)),
            "bc27a20de1bb75a83b3727ac457db04791cdd092c64e2e31e5b58ebecf296ddb"
        );

        // Decodes the platform's own deflate-compressed bytes directly --
        // not anything ibcmd-rs compiled -- so the exact re-export match
        // below is independent of our own compiler direction being
        // correct.
        let decoded = decode_dcs(&profile, DcsTemplateKind::Schema, &packed)
            .expect("genuine platform-packed body must decode");
        let exported =
            crate::mssql_dump::normalize_data_composition_schema_template_documents_with_profiles(
                &decoded.documents(),
                &BTreeMap::new(),
                &BTreeMap::new(),
                &ProfileId::parse("provider:mssql-legacy").unwrap(),
                &ProfileId::parse("xml-2.20").unwrap(),
            )
            .expect("genuine platform primary schema must remain exportable");

        // `canonicalize_data_composition_settings_document` now routes
        // outputParameters through the shared `ibcmd-xml` codec (same
        // mechanism the terminal AreaTemplate side table already used for
        // TextColor/Details), so the storage "Title" -> source "Заголовок"
        // canonicalization is exercised here and the export is byte-exact.
        assert_eq!(exported, native_template);
    }

    #[test]
    fn platform_parameter_scalar_types_native_packed_body_exports_exact_native_template() {
        let profile = DcsCodecProfile::fixture();
        let packed = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-parameter-scalar-types/raw-packed.bin.b64"
        )));
        let native_template = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-parameter-scalar-types/native-template.xml.b64"
        )));
        assert_eq!(
            format!("{:x}", Sha256::digest(&packed)),
            "e787dd364b36c76987e4c9d6640bf0f67f1775228954b9961b04ba1e474325aa"
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(&native_template)),
            "7f4f83f5e8adcb21b0e9e848726c3da19d9cbb94996a08e69a84784fc4c9f1e0"
        );

        // Decodes the platform's own deflate-compressed bytes directly --
        // not anything ibcmd-rs compiled -- so the exact re-export match
        // below is independent of our own compiler direction being
        // correct.
        let decoded = decode_dcs(&profile, DcsTemplateKind::Schema, &packed)
            .expect("genuine platform-packed body must decode");
        let exported =
            crate::mssql_dump::normalize_data_composition_schema_template_documents_with_profiles(
                &decoded.documents(),
                &BTreeMap::new(),
                &BTreeMap::new(),
                &ProfileId::parse("provider:mssql-legacy").unwrap(),
                &ProfileId::parse("xml-2.20").unwrap(),
            )
            .expect("genuine platform primary schema must remain exportable");
        assert_eq!(exported, native_template);
    }

    #[test]
    fn platform_output_parameters_source_compiles_and_round_trips() {
        let profile = DcsCodecProfile::fixture();
        let source = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-output-parameters/native-template.xml.b64"
        )));

        // seed(source) XML -> body: the settings document's compile
        // direction is blind passthrough (source bytes copied into the
        // compiled blob unchanged -- see
        // `schema_compiler_compile_direction_does_not_yet_gate_output_parameters_cohort`
        // above), so this must succeed even though the compile direction
        // does not deeply validate the outputParameters cohort.
        let blob = compile_dcs(&profile, DcsTemplateKind::Schema, &source).unwrap();
        let decoded = decode_dcs(&profile, DcsTemplateKind::Schema, &blob).unwrap();

        // body -> XML: re-exporting the compiled body must reproduce the
        // evidenced outputParameters item verbatim. Unlike the genuine
        // storage-bytes test above, there is no "Title" vs. "Заголовок" gap
        // to document here: the compiled blob embeds the *source*-spelled
        // settings-document bytes unchanged (blind passthrough), so the
        // typed decode/export path parses the source spelling directly and
        // there is nothing to canonicalize.
        let exported =
            crate::mssql_dump::normalize_data_composition_schema_template_documents_with_profiles(
                &decoded.documents(),
                &BTreeMap::new(),
                &BTreeMap::new(),
                &ProfileId::parse("provider:mssql-legacy").unwrap(),
                &ProfileId::parse("xml-2.20").unwrap(),
            )
            .unwrap();
        let exported_text = std::str::from_utf8(&exported).unwrap();
        assert!(exported_text.contains("<dcscor:parameter>Заголовок</dcscor:parameter>"));
        assert!(
            exported_text
                .contains("<dcscor:value xsi:type=\"xs:string\">Probe Title</dcscor:value>")
        );
        assert!(exported_text.contains("<dcscor:item xsi:type=\"dcsset:SettingsParameterValue\">"));
        // Placement: outputParameters immediately after order, immediately
        // before the terminal StructureItemGroup item.
        let order_close_at = exported_text.find("</dcsset:order>").unwrap();
        let output_parameters_at = exported_text.find("<dcsset:outputParameters>").unwrap();
        let structure_item_group_at = exported_text
            .find("<dcsset:item xsi:type=\"dcsset:StructureItemGroup\">")
            .unwrap();
        assert!(order_close_at < output_parameters_at);
        assert!(output_parameters_at < structure_item_group_at);

        // body -> XML -> body: recompiling the exported source and
        // re-decoding must reproduce the same outputParameters content.
        let recompiled_blob = compile_dcs(&profile, DcsTemplateKind::Schema, &exported).unwrap();
        let recompiled = decode_dcs(&profile, DcsTemplateKind::Schema, &recompiled_blob).unwrap();
        let reexported =
            crate::mssql_dump::normalize_data_composition_schema_template_documents_with_profiles(
                &recompiled.documents(),
                &BTreeMap::new(),
                &BTreeMap::new(),
                &ProfileId::parse("provider:mssql-legacy").unwrap(),
                &ProfileId::parse("xml-2.20").unwrap(),
            )
            .unwrap();
        let reexported_text = std::str::from_utf8(&reexported).unwrap();
        assert!(reexported_text.contains("<dcscor:parameter>Заголовок</dcscor:parameter>"));
        assert!(
            reexported_text
                .contains("<dcscor:value xsi:type=\"xs:string\">Probe Title</dcscor:value>")
        );
    }

    #[test]
    fn platform_parameter_scalar_types_source_compiles_and_round_trips() {
        let profile = DcsCodecProfile::fixture();
        let source = decode_base64_fixture(include_str!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-parameter-scalar-types/native-template.xml.b64"
        )));

        // seed(source) XML -> body: this corpus's own manifest observes the
        // native re-export is byte-identical to the submitted seed, so the
        // evidenced source IS native-template.xml here (no separate
        // "hypothesis vs. platform" gap to bridge).
        let blob = compile_dcs(&profile, DcsTemplateKind::Schema, &source).unwrap();
        let decoded = decode_dcs(&profile, DcsTemplateKind::Schema, &blob).unwrap();

        // body -> XML: re-exporting the compiled body must reproduce the
        // evidenced scalar parameters verbatim. (Byte-exact whole-document
        // equality against native-template.xml is not asserted here: the
        // primary schema's compile direction is a pre-existing, unrelated
        // blind-passthrough -- confirmed by dcs-core's own evidence, which
        // records its compiled body as "platform-valid... even though its
        // internal namespace spelling is not byte-identical to the
        // platform storage body" -- not a typed storage re-emitter like
        // AreaTemplate's. The genuine-bytes test above proves byte-exact
        // whole-document equality instead.)
        let exported =
            crate::mssql_dump::normalize_data_composition_schema_template_documents_with_profiles(
                &decoded.documents(),
                &BTreeMap::new(),
                &BTreeMap::new(),
                &ProfileId::parse("provider:mssql-legacy").unwrap(),
                &ProfileId::parse("xml-2.20").unwrap(),
            )
            .unwrap();
        let exported_text = std::str::from_utf8(&exported).unwrap();
        assert!(exported_text.contains("<name>Флаг</name>"));
        assert!(exported_text.contains("<v8:Type>xs:boolean</v8:Type>"));
        assert!(exported_text.contains("<value xsi:type=\"xs:boolean\">true</value>"));
        assert!(exported_text.contains("<name>Лимит</name>"));
        assert!(exported_text.contains("<v8:Digits>10</v8:Digits>"));
        assert!(exported_text.contains("<value xsi:type=\"xs:decimal\">100.5</value>"));
        assert!(exported_text.contains("<name>Период</name>"));
        assert!(exported_text.contains("<v8:Type>v8:StandardPeriod</v8:Type>"));
        assert!(
            exported_text.contains(
                "<v8:variant xsi:type=\"v8:StandardPeriodVariant\">LastMonth</v8:variant>"
            )
        );
        // Insertion order: Флаг, Лимит, Период, all after Caption.
        let caption_at = exported_text.find("<name>Caption</name>").unwrap();
        let flag_at = exported_text.find("<name>Флаг</name>").unwrap();
        let limit_at = exported_text.find("<name>Лимит</name>").unwrap();
        let period_at = exported_text.find("<name>Период</name>").unwrap();
        assert!(caption_at < flag_at && flag_at < limit_at && limit_at < period_at);

        // body -> XML -> body: recompiling the exported source and
        // re-decoding must reproduce the same typed scalar-parameter
        // values. Whole-document byte equality is not asserted here: the
        // recompile round trip goes through the same pre-existing,
        // unrelated settings-document reindentation gap documented on the
        // color and multi-cell-appearance slices' equivalent tests (one
        // `<dcsset:item xsi:type="dcsset:StructureItemGroup">` line loses
        // a tab of indentation on the second pass); the scalar-parameter
        // content itself is unaffected and checked explicitly instead.
        let recompiled_blob = compile_dcs(&profile, DcsTemplateKind::Schema, &exported).unwrap();
        let recompiled = decode_dcs(&profile, DcsTemplateKind::Schema, &recompiled_blob).unwrap();
        let reexported =
            crate::mssql_dump::normalize_data_composition_schema_template_documents_with_profiles(
                &recompiled.documents(),
                &BTreeMap::new(),
                &BTreeMap::new(),
                &ProfileId::parse("provider:mssql-legacy").unwrap(),
                &ProfileId::parse("xml-2.20").unwrap(),
            )
            .unwrap();
        let reexported_text = std::str::from_utf8(&reexported).unwrap();
        assert!(reexported_text.contains("<name>Флаг</name>"));
        assert!(reexported_text.contains("<value xsi:type=\"xs:boolean\">true</value>"));
        assert!(reexported_text.contains("<name>Лимит</name>"));
        assert!(reexported_text.contains("<value xsi:type=\"xs:decimal\">100.5</value>"));
        assert!(reexported_text.contains("<name>Период</name>"));
        assert!(
            reexported_text.contains(
                "<v8:variant xsi:type=\"v8:StandardPeriodVariant\">LastMonth</v8:variant>"
            )
        );
    }

    fn settings_variant_source(settings_children: &str) -> Vec<u8> {
        canonical_source(&format!(
            "\t<settingsVariant>\n\t\t<dcsset:name>Main</dcsset:name>\n\t\t<dcsset:presentation xsi:type=\"xs:string\">Main</dcsset:presentation>\n\t\t<dcsset:settings{{settings}}>\n{settings_children}\n\t\t</dcsset:settings>\n\t</settingsVariant>"
        ))
    }

    /// Which settings children a body may carry is no longer an enumerated
    /// cohort: a child compiles exactly when the exporter reads it back. One
    /// in the settings namespace the typed cohort does not know is still
    /// spelled the platform's way and comes back; one in a namespace the
    /// export cannot spell is refused by the round trip.
    #[test]
    fn schema_compiler_admits_an_unowned_settings_child_only_when_it_reads_back() {
        let profile = DcsCodecProfile::fixture();
        let known_namespace = settings_variant_source("\t\t\t<dcsset:futureProbe/>");
        let blob = compile_dcs(&profile, DcsTemplateKind::Schema, &known_namespace).unwrap();
        assert_eq!(
            export_body(&blob, &BTreeMap::new(), &BTreeMap::new()),
            known_namespace
        );

        let foreign_namespace = settings_variant_source(
            "\t\t\t<probe:futureProbe xmlns:probe=\"urn:ibcmd-rs:dcs-probe\"/>",
        );
        assert!(matches!(
            compile_dcs(&profile, DcsTemplateKind::Schema, &foreign_namespace),
            Err(DcsCodecError::RoundTrip(_))
        ));
    }

    /// An empty `outputParameters` is a placeholder storage may carry and the
    /// export never writes, so a source spelling one cannot come back the
    /// way it was written and is refused.
    #[test]
    fn schema_compiler_refuses_an_empty_output_parameters_the_export_drops() {
        let source = settings_variant_source("\t\t\t<dcsset:outputParameters/>");
        assert!(matches!(
            compile_dcs(
                &DcsCodecProfile::fixture(),
                DcsTemplateKind::Schema,
                &source
            ),
            Err(DcsCodecError::RoundTrip(_))
        ));
    }

    /// A filter the typed cohort does not describe used to be refused for
    /// that alone; it compiles now, because the body reads back as written.
    #[test]
    fn schema_compiler_compiles_a_filter_outside_the_typed_cohort() {
        let profile = DcsCodecProfile::fixture();
        let source = settings_variant_source(
            "\t\t\t<dcsset:filter>\n\t\t\t\t<dcsset:viewMode>Normal</dcsset:viewMode>\n\t\t\t</dcsset:filter>",
        );
        let blob = compile_dcs(&profile, DcsTemplateKind::Schema, &source).unwrap();
        assert_eq!(
            export_body(&blob, &BTreeMap::new(), &BTreeMap::new()),
            source
        );
    }

    #[test]
    fn appearance_is_exact_direct_xml_and_unknown_schema_layout_is_blocked() {
        let profile = DcsCodecProfile::fixture();
        let blob = compile_dcs(&profile, DcsTemplateKind::Appearance, SIMPLE_APPEARANCE).unwrap();
        let decoded = decode_dcs(&profile, DcsTemplateKind::Appearance, &blob).unwrap();
        assert_eq!(decoded.layout(), DcsBodyLayout::DirectXml);
        assert_eq!(decoded.plaintext(), SIMPLE_APPEARANCE);

        let legacy_direct = deflate_bytes(&simple_schema()).unwrap();
        assert!(decode_dcs(&profile, DcsTemplateKind::Schema, &legacy_direct).is_err());
        assert_eq!(
            decode_compatible_dcs(DcsTemplateKind::Schema, &legacy_direct)
                .unwrap()
                .layout(),
            DcsBodyLayout::DirectXml
        );
    }

    #[test]
    fn unsupported_settings_shape_and_inline_area_documents_fail_closed() {
        let profile = DcsCodecProfile::fixture();
        let settings = br#"<DataCompositionSchema xmlns="http://v8.1c.ru/8.1/data-composition-system/schema"><settingsVariant/></DataCompositionSchema>"#;
        assert!(matches!(
            compile_dcs(&profile, DcsTemplateKind::Schema, settings),
            Err(DcsCodecError::UnsupportedSource(_))
        ));

        let area = br#"<DataCompositionSchema xmlns="http://v8.1c.ru/8.1/data-composition-system/schema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:dcsat="http://v8.1c.ru/8.1/data-composition-system/area-template"><template xsi:type="dcsat:AreaTemplate"/></DataCompositionSchema>"#;
        assert!(matches!(
            compile_dcs(&profile, DcsTemplateKind::Schema, area),
            Err(DcsCodecError::UnsupportedSource(_))
        ));
    }
}
