//! Profile-gated codecs for data-composition template bodies.

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::ops::Range;

use ibcmd_core::artifact::ProfileId;
use ibcmd_core::profile::EffectiveProfile;
use ibcmd_xml::{
    DcsChildParseOutcome, DcsSchemaTemplateError, DcsSettingsDocumentAnalysisError,
    analyze_dcs_schema_template_documents, analyze_dcs_settings_document,
    compile_dcs_schema_template_source_documents,
};
use quick_xml::NsReader;
use quick_xml::events::Event;
use quick_xml::name::ResolveResult;

use super::{BodyProfileError, SelectedBodyProfile};
use crate::compiler::families::native::{NativeError, deflate_bytes, inflate};

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
    let _ = profile;
    compile_evidenced_dcs(kind, xml)
}

pub(crate) fn compile_evidenced_dcs(
    kind: DcsTemplateKind,
    xml: &[u8],
) -> Result<Vec<u8>, DcsCodecError> {
    let plain = match kind {
        DcsTemplateKind::Appearance => {
            validate_xml_document(xml, "AppearanceTemplate", Some(APPEARANCE_NS))?;
            xml.to_vec()
        }
        DcsTemplateKind::Schema => compile_schema_plain(xml)?,
    };
    let blob = deflate_bytes(&plain)?;
    decode_strict(kind, &blob)?;
    Ok(blob)
}

pub fn decode_dcs(
    profile: &DcsCodecProfile,
    kind: DcsTemplateKind,
    blob: &[u8],
) -> Result<DcsBody, DcsCodecError> {
    let _ = profile;
    decode_strict(kind, blob)
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

fn decode_strict(kind: DcsTemplateKind, blob: &[u8]) -> Result<DcsBody, DcsCodecError> {
    let plain = inflate(blob)?;
    match kind {
        DcsTemplateKind::Schema => decode_schema_plain(plain),
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

fn compile_schema_plain(xml: &[u8]) -> Result<Vec<u8>, DcsCodecError> {
    validate_xml_document(xml, "DataCompositionSchema", Some(SCHEMA_NS))?;

    let documents =
        compile_dcs_schema_template_source_documents(xml).map_err(map_template_error)?;
    for settings_document in documents.settings() {
        let settings_document = std::str::from_utf8(settings_document).map_err(|_| {
            DcsCodecError::InvalidXml("native Settings document is not UTF-8".to_string())
        })?;
        let settings_analysis =
            analyze_dcs_settings_document(settings_document).map_err(|error| match error {
                DcsSettingsDocumentAnalysisError::Malformed(error) => {
                    DcsCodecError::InvalidXml(error.to_string())
                }
                DcsSettingsDocumentAnalysisError::UnsupportedSource { reason, .. } => {
                    DcsCodecError::UnsupportedSource(reason)
                }
            })?;
        let typed_settings = settings_analysis.typed();
        if matches!(
            typed_settings.selection_outcome(),
            DcsChildParseOutcome::Unsupported(_)
        ) {
            return Err(DcsCodecError::UnsupportedSource(
                "DCS selection is outside the platform-authenticated compiler cohort",
            ));
        }
        if matches!(
            typed_settings.filter(),
            DcsChildParseOutcome::Unsupported(_)
        ) {
            return Err(DcsCodecError::UnsupportedSource(
                "DCS filter is outside the platform-authenticated compiler cohort",
            ));
        }
        if matches!(typed_settings.order(), DcsChildParseOutcome::Unsupported(_)) {
            return Err(DcsCodecError::UnsupportedSource(
                "DCS order is outside the platform-authenticated compiler cohort",
            ));
        }
        if matches!(
            typed_settings.conditional_appearance(),
            DcsChildParseOutcome::Unsupported(_)
        ) {
            return Err(DcsCodecError::UnsupportedSource(
                "DCS conditional appearance is outside the platform-authenticated compiler cohort",
            ));
        }
    }
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
    Ok(plain)
}

fn decode_schema_plain(plain: Vec<u8>) -> Result<DcsBody, DcsCodecError> {
    let body = decode_schema_plain_framed(plain)?;
    analyze_dcs_schema_template_documents(&body.documents()).map_err(map_template_error)?;
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

    const SIMPLE_SCHEMA: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<DataCompositionSchema xmlns="http://v8.1c.ru/8.1/data-composition-system/schema" xmlns:dcsset="http://v8.1c.ru/8.1/data-composition-system/settings" xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
	<dataSource><name>Source1</name><dataSourceType>Local</dataSourceType></dataSource>
	<settingsVariant><dcsset:name>Main</dcsset:name><dcsset:presentation xsi:type="xs:string">Main</dcsset:presentation><dcsset:settings/></settingsVariant>
</DataCompositionSchema>"#;
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
        let first = compile_dcs(&profile, DcsTemplateKind::Schema, SIMPLE_SCHEMA).unwrap();
        let second = compile_dcs(&profile, DcsTemplateKind::Schema, SIMPLE_SCHEMA).unwrap();
        assert_eq!(first, second);

        let decoded = decode_dcs(&profile, DcsTemplateKind::Schema, &first).unwrap();
        assert_eq!(decoded.layout(), DcsBodyLayout::NativeThreeDocument);
        assert_eq!(decoded.document_count(), 3);
        let exported = crate::mssql_dump::normalize_data_composition_schema_template_xml(
            decoded.plaintext(),
            &BTreeMap::new(),
            &BTreeMap::new(),
        )
        .expect("native DCS body must remain exportable");
        let exported = String::from_utf8(exported).unwrap();
        assert!(exported.contains("<DataCompositionSchema "));
        assert!(exported.contains("<name>Source1</name>"));
        assert!(exported.contains("<dataSourceType>Local</dataSourceType>"));
    }

    #[test]
    fn common_document_builder_preserves_platform_accepted_compiler_body() {
        let source = include_bytes!(concat!(
            "../../../tests/fixtures/native-evidence/8.3.27.2214/dcs-core/native/",
            "Reports/DcsCorpus/Templates/MainSchema/Ext/Template.xml"
        ));
        let packed =
            compile_dcs(&DcsCodecProfile::fixture(), DcsTemplateKind::Schema, source).unwrap();
        let decoded = decode_dcs(
            &DcsCodecProfile::fixture(),
            DcsTemplateKind::Schema,
            &packed,
        )
        .unwrap();
        assert_eq!(decoded.plaintext().len(), 4_734);
        assert_eq!(
            format!("{:x}", Sha256::digest(decoded.plaintext())),
            "928de9e6a9fbcfe89530e5d02fe8f08c0efe491c392b671fa61c4c36d48ec81a"
        );
        assert_eq!(packed.len(), 1_026);
        assert_eq!(
            format!("{:x}", Sha256::digest(&packed)),
            "5b8f758dc3d64e56b744b7554148245b0bf1f3023ce5aa81df63bcd730058ca8"
        );
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

    #[test]
    fn schema_decoder_rejects_more_than_two_unattested_settings_documents() {
        let settings = xml_document(EMPTY_SETTINGS);
        let plain = synthetic_schema_plain(&[settings.clone(), settings.clone(), settings]);
        assert!(matches!(
            decode_schema_plain(plain),
            Err(DcsCodecError::UnsupportedSource(reason))
                if reason == "native DCS envelope settings count is outside the attested range"
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

    #[test]
    fn schema_decoder_rejects_zero_settings_count() {
        let mut plain = synthetic_schema_plain(&[xml_document(EMPTY_SETTINGS)]);
        plain[4..8].copy_from_slice(&0u32.to_le_bytes());

        assert!(matches!(
            decode_schema_plain(plain),
            Err(DcsCodecError::UnsupportedLayout(reason))
                if reason == "DCS schema has no settings documents"
        ));
    }

    #[test]
    fn schema_decoder_rejects_huge_settings_count_before_document_iteration() {
        let mut plain = vec![0u8; DCS_HEADER_BYTES];
        plain[4..8].copy_from_slice(&u32::MAX.to_le_bytes());

        assert!(matches!(
            decode_schema_plain(plain),
            Err(DcsCodecError::UnsupportedLayout(reason))
                if reason == "DCS schema header is truncated"
        ));
    }

    #[test]
    fn schema_decoder_rejects_zero_stored_document_length() {
        let mut plain = synthetic_schema_plain(&[xml_document(EMPTY_SETTINGS)]);
        plain[8..16].copy_from_slice(&0u64.to_le_bytes());

        assert!(matches!(
            decode_schema_plain(plain),
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
            decode_schema_plain(plain),
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
            decode_schema_plain(plain),
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
            decode_schema_plain(plain),
            Err(DcsCodecError::InvalidXml(_))
        ));
    }

    #[test]
    fn non_empty_settings_survive_semantic_round_trip() {
        let profile = DcsCodecProfile::fixture();
        let source = br#"<?xml version="1.0" encoding="UTF-8"?>
<DataCompositionSchema xmlns="http://v8.1c.ru/8.1/data-composition-system/schema" xmlns:dcscor="http://v8.1c.ru/8.1/data-composition-system/core" xmlns:dcsset="http://v8.1c.ru/8.1/data-composition-system/settings" xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
	<settingsVariant><dcsset:name>Main</dcsset:name><dcsset:presentation xsi:type="xs:string">Main</dcsset:presentation><dcsset:settings><dcsset:selection><dcsset:item xsi:type="dcsset:SelectedItemField"><dcsset:field>SortKey</dcsset:field></dcsset:item></dcsset:selection><dcsset:filter><dcsset:item xsi:type="dcsset:FilterItemComparison"><dcsset:left xsi:type="dcscor:Field">SortKey</dcsset:left><dcsset:comparisonType>Equal</dcsset:comparisonType><dcsset:right xsi:type="xs:string">A</dcsset:right></dcsset:item></dcsset:filter></dcsset:settings></settingsVariant>
</DataCompositionSchema>"#;

        let blob = compile_dcs(&profile, DcsTemplateKind::Schema, source).unwrap();
        let decoded = decode_dcs(&profile, DcsTemplateKind::Schema, &blob).unwrap();
        let exported = crate::mssql_dump::normalize_data_composition_schema_template_xml(
            decoded.plaintext(),
            &BTreeMap::new(),
            &BTreeMap::new(),
        )
        .expect("settings document must remain exportable");
        let exported = String::from_utf8(exported).unwrap();
        assert!(exported.contains("<dcsset:settings"));
        assert!(exported.contains("<dcsset:comparisonType>Equal</dcsset:comparisonType>"));
        assert!(exported.contains("<dcsset:right xsi:type=\"xs:string\">A</dcsset:right>"));
        assert_eq!(exported.matches("<settingsVariant>").count(), 1);
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
        let exported = crate::mssql_dump::normalize_data_composition_schema_template_xml(
            decoded.plaintext(),
            &BTreeMap::new(),
            &BTreeMap::new(),
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
        let blob = compile_dcs(&profile, DcsTemplateKind::Schema, &source).unwrap();
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
    fn schema_compiler_rejects_every_unowned_settings_child() {
        for unknown in [
            "<dcsset:outputParameters/>",
            "<dcsset:futureProbe/>",
            "<probe:futureProbe xmlns:probe=\"urn:ibcmd-rs:dcs-probe\"/>",
        ] {
            let source = String::from_utf8(SIMPLE_SCHEMA.to_vec()).unwrap().replace(
                "<dcsset:settings/>",
                &format!("<dcsset:settings>{unknown}</dcsset:settings>"),
            );
            assert!(matches!(
                compile_dcs(
                    &DcsCodecProfile::fixture(),
                    DcsTemplateKind::Schema,
                    source.as_bytes()
                ),
                Err(DcsCodecError::UnsupportedSource(_))
            ));
        }
    }

    #[test]
    fn schema_compiler_rejects_filter_outside_platform_authenticated_cohort() {
        let profile = DcsCodecProfile::fixture();
        let source = br#"<?xml version="1.0" encoding="UTF-8"?>
<DataCompositionSchema xmlns="http://v8.1c.ru/8.1/data-composition-system/schema" xmlns:dcsset="http://v8.1c.ru/8.1/data-composition-system/settings" xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
	<settingsVariant><dcsset:name>Main</dcsset:name><dcsset:presentation xsi:type="xs:string">Main</dcsset:presentation><dcsset:settings><dcsset:filter><dcsset:viewMode>Normal</dcsset:viewMode></dcsset:filter></dcsset:settings></settingsVariant>
</DataCompositionSchema>"#;

        assert!(matches!(
            compile_dcs(&profile, DcsTemplateKind::Schema, source),
            Err(DcsCodecError::UnsupportedSource(reason))
                if reason == "DCS filter is outside the platform-authenticated compiler cohort"
        ));
    }

    #[test]
    fn appearance_is_exact_direct_xml_and_unknown_schema_layout_is_blocked() {
        let profile = DcsCodecProfile::fixture();
        let blob = compile_dcs(&profile, DcsTemplateKind::Appearance, SIMPLE_APPEARANCE).unwrap();
        let decoded = decode_dcs(&profile, DcsTemplateKind::Appearance, &blob).unwrap();
        assert_eq!(decoded.layout(), DcsBodyLayout::DirectXml);
        assert_eq!(decoded.plaintext(), SIMPLE_APPEARANCE);

        let legacy_direct = deflate_bytes(SIMPLE_SCHEMA).unwrap();
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
