//! Profile-gated codecs for data-composition template bodies.

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use ibcmd_core::artifact::ProfileId;
use ibcmd_core::profile::EffectiveProfile;
use quick_xml::NsReader;
use quick_xml::events::Event;
use quick_xml::name::ResolveResult;

use super::{BodyProfileError, SelectedBodyProfile};
use crate::compiler::families::native::{NativeError, deflate_bytes, inflate};

const LAYOUT_KEY: &str = "bootstrap.body.dcs.layout";
const LAYOUT: &str = "dcs-schema-three-document-v1";
const UTF8_BOM: &[u8] = b"\xef\xbb\xbf";
const SCHEMA_NS: &[u8] = b"http://v8.1c.ru/8.1/data-composition-system/schema";
const SETTINGS_NS: &[u8] = b"http://v8.1c.ru/8.1/data-composition-system/settings";
const APPEARANCE_NS: &[u8] = b"http://v8.1c.ru/8.1/data-composition-system/appearance-template";
const DCS_HEADER_BYTES: usize = 24;
const MAX_XML_DEPTH: usize = 256;
const MAX_XML_NODES: usize = 1_000_000;

const SCHEMA_FILE_OPEN: &str = "<SchemaFile xmlns=\"\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">";
const EMPTY_SETTINGS: &str = "<Settings xmlns=\"http://v8.1c.ru/8.1/data-composition-system/settings\" xmlns:dcscor=\"http://v8.1c.ru/8.1/data-composition-system/core\" xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\" xmlns:sys=\"http://v8.1c.ru/8.1/data/ui/fonts/system\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" xmlns:web=\"http://v8.1c.ru/8.1/data/ui/colors/web\" xmlns:win=\"http://v8.1c.ru/8.1/data/ui/colors/windows\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"/>";
const SETTINGS_OPEN: &str = "<Settings xmlns=\"http://v8.1c.ru/8.1/data-composition-system/settings\" xmlns:dcsset=\"http://v8.1c.ru/8.1/data-composition-system/settings\" xmlns:dcscor=\"http://v8.1c.ru/8.1/data-composition-system/core\" xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\" xmlns:sys=\"http://v8.1c.ru/8.1/data/ui/fonts/system\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" xmlns:web=\"http://v8.1c.ru/8.1/data/ui/colors/web\" xmlns:win=\"http://v8.1c.ru/8.1/data/ui/colors/windows\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">";

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
        DcsTemplateKind::Schema => match decode_schema_plain(plain.clone()) {
            Ok(body) => Ok(body),
            Err(_) => {
                validate_xml_document(&plain, "DataCompositionSchema", Some(SCHEMA_NS))?;
                Ok(DcsBody {
                    kind,
                    layout: DcsBodyLayout::DirectXml,
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
    Ok(DcsBody {
        kind: DcsTemplateKind::Appearance,
        layout: DcsBodyLayout::DirectXml,
        plain,
        document_count: 1,
    })
}

fn compile_schema_plain(xml: &[u8]) -> Result<Vec<u8>, DcsCodecError> {
    let inspection = validate_xml_document(xml, "DataCompositionSchema", Some(SCHEMA_NS))?;
    if inspection.settings_variant_count != 1 {
        return Err(DcsCodecError::UnsupportedSource(
            "exactly one settingsVariant is required by the evidenced three-document layout",
        ));
    }
    if inspection.has_inline_area_template {
        return Err(DcsCodecError::UnsupportedSource(
            "inline AreaTemplate requires the separately indexed native area-template document",
        ));
    }

    let (inner, settings) = source_schema_to_native_parts(xml)?;
    let first = xml_document(&format!("{SCHEMA_FILE_OPEN}\r\n{inner}\r\n</SchemaFile>"));
    let second = xml_document(&settings);
    let third = xml_document(&format!(
        "{SCHEMA_FILE_OPEN}\r\n\t<dataCompositionSchema xmlns=\"{}\"/>\r\n</SchemaFile>",
        std::str::from_utf8(SCHEMA_NS).expect("schema namespace is UTF-8")
    ));

    let first_len = u64::try_from(first.len())
        .map_err(|_| DcsCodecError::LimitExceeded("first DCS XML document"))?;
    let second_len = u64::try_from(second.len())
        .map_err(|_| DcsCodecError::LimitExceeded("second DCS XML document"))?;
    let capacity = DCS_HEADER_BYTES
        .checked_add(first.len())
        .and_then(|value| value.checked_add(second.len()))
        .and_then(|value| value.checked_add(third.len()))
        .ok_or(DcsCodecError::LimitExceeded("DCS body"))?;
    let mut plain = Vec::with_capacity(capacity);
    plain.extend_from_slice(&0u32.to_le_bytes());
    plain.extend_from_slice(&1u32.to_le_bytes());
    plain.extend_from_slice(&first_len.to_le_bytes());
    plain.extend_from_slice(&second_len.to_le_bytes());
    plain.extend_from_slice(&first);
    plain.extend_from_slice(&second);
    plain.extend_from_slice(&third);
    Ok(plain)
}

fn decode_schema_plain(plain: Vec<u8>) -> Result<DcsBody, DcsCodecError> {
    if plain.len() < DCS_HEADER_BYTES {
        return Err(DcsCodecError::UnsupportedLayout(
            "DCS schema header is truncated".to_string(),
        ));
    }
    if read_u32(&plain, 0)? != 0 || read_u32(&plain, 4)? != 1 {
        return Err(DcsCodecError::UnsupportedLayout(
            "unknown DCS schema header version".to_string(),
        ));
    }
    let first_len = read_len(&plain, 8, "first DCS XML document")?;
    let second_len = read_len(&plain, 16, "second DCS XML document")?;
    let second_start = DCS_HEADER_BYTES
        .checked_add(first_len)
        .ok_or(DcsCodecError::LimitExceeded("DCS document offset"))?;
    let third_start = second_start
        .checked_add(second_len)
        .ok_or(DcsCodecError::LimitExceeded("DCS document offset"))?;
    if first_len == 0 || second_len == 0 || third_start >= plain.len() {
        return Err(DcsCodecError::UnsupportedLayout(
            "DCS schema document lengths are invalid".to_string(),
        ));
    }
    let first = &plain[DCS_HEADER_BYTES..second_start];
    let second = &plain[second_start..third_start];
    let third = &plain[third_start..];
    for (index, document) in [first, second, third].iter().enumerate() {
        if !document.starts_with(UTF8_BOM) {
            return Err(DcsCodecError::UnsupportedLayout(format!(
                "DCS XML document {} has no UTF-8 BOM",
                index + 1
            )));
        }
    }
    validate_xml_document(first, "SchemaFile", None)?;
    validate_xml_document(second, "Settings", Some(SETTINGS_NS))?;
    validate_xml_document(third, "SchemaFile", None)?;
    if !contains_bytes(first, b"<dataCompositionSchema")
        || !contains_bytes(third, b"<dataCompositionSchema")
    {
        return Err(DcsCodecError::UnsupportedLayout(
            "DCS SchemaFile document has no dataCompositionSchema root".to_string(),
        ));
    }
    Ok(DcsBody {
        kind: DcsTemplateKind::Schema,
        layout: DcsBodyLayout::NativeThreeDocument,
        plain,
        document_count: 3,
    })
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

fn xml_document(body: &str) -> Vec<u8> {
    let mut document = Vec::with_capacity(UTF8_BOM.len() + 45 + body.len());
    document.extend_from_slice(UTF8_BOM);
    document.extend_from_slice(b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n");
    document.extend_from_slice(body.as_bytes());
    document
}

fn source_schema_to_native_parts(xml: &[u8]) -> Result<(String, String), DcsCodecError> {
    let text = std::str::from_utf8(xml)
        .map_err(|_| DcsCodecError::InvalidXml("DCS source is not UTF-8".to_string()))?;
    let mut body = text.trim_start_matches('\u{feff}').trim_start();
    if let Some(after_decl) = body.strip_prefix("<?xml") {
        let end = after_decl.find("?>").ok_or_else(|| {
            DcsCodecError::InvalidXml("DCS XML declaration is not closed".to_string())
        })?;
        body = after_decl[end + 2..].trim_start_matches(['\r', '\n', ' ', '\t']);
    }
    if !body.starts_with("<DataCompositionSchema") {
        return Err(DcsCodecError::UnsupportedSource(
            "prefixed or indirect DataCompositionSchema roots are not evidenced",
        ));
    }
    let mut body = body.to_owned();
    let settings = extract_settings_document(&mut body)?;
    body.replace_range(
        1..1 + "DataCompositionSchema".len(),
        "dataCompositionSchema",
    );
    if let Some(close) = body.rfind("</DataCompositionSchema>") {
        body.replace_range(
            close + 2..close + 2 + "DataCompositionSchema".len(),
            "dataCompositionSchema",
        );
    }
    Ok((body.trim_end().to_string(), settings))
}

fn extract_settings_document(body: &mut String) -> Result<String, DcsCodecError> {
    const OPEN: &str = "<dcsset:settings";
    let start = body.find(OPEN).ok_or(DcsCodecError::UnsupportedSource(
        "settingsVariant must contain one empty dcsset:settings element",
    ))?;
    if body[start + OPEN.len()..].contains(OPEN) {
        return Err(DcsCodecError::UnsupportedSource(
            "multiple dcsset:settings elements are not yet supported",
        ));
    }
    let bytes = body.as_bytes();
    let mut cursor = start + OPEN.len();
    let mut quote = None::<u8>;
    let end = loop {
        let byte = *bytes.get(cursor).ok_or_else(|| {
            DcsCodecError::InvalidXml("dcsset:settings start tag is not closed".to_string())
        })?;
        if quote == Some(byte) {
            quote = None;
        } else if quote.is_none() && matches!(byte, b'\'' | b'"') {
            quote = Some(byte);
        } else if quote.is_none() && byte == b'>' {
            break cursor;
        }
        cursor += 1;
    };
    if body[start..end].trim_end().ends_with('/') {
        body.replace_range(start..=end, "");
        return Ok(EMPTY_SETTINGS.to_string());
    }
    const CLOSE: &str = "</dcsset:settings>";
    let close = body[end + 1..]
        .find(CLOSE)
        .map(|relative| end + 1 + relative)
        .ok_or_else(|| {
            DcsCodecError::InvalidXml("dcsset:settings end tag is absent".to_string())
        })?;
    let content = body[end + 1..close].to_string();
    body.replace_range(start..close + CLOSE.len(), "");
    Ok(format!("{SETTINGS_OPEN}{content}</Settings>"))
}

#[derive(Default)]
struct XmlInspection {
    settings_variant_count: usize,
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
    if local == b"settingsVariant" {
        inspection.settings_variant_count = inspection
            .settings_variant_count
            .checked_add(1)
            .ok_or(DcsCodecError::LimitExceeded("DCS settingsVariant count"))?;
    }
    inspection.has_inline_area_template |= local == b"template" && has_inline_area_template;
    Ok(())
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
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

    const SIMPLE_SCHEMA: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<DataCompositionSchema xmlns="http://v8.1c.ru/8.1/data-composition-system/schema" xmlns:dcsset="http://v8.1c.ru/8.1/data-composition-system/settings" xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
	<dataSource><name>Source1</name><dataSourceType>Local</dataSourceType></dataSource>
	<settingsVariant><dcsset:name>Main</dcsset:name><dcsset:presentation xsi:type="xs:string">Main</dcsset:presentation><dcsset:settings/></settingsVariant>
</DataCompositionSchema>"#;
    const SIMPLE_APPEARANCE: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<AppearanceTemplate xmlns="http://v8.1c.ru/8.1/data-composition-system/appearance-template"/>"#;

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
    fn non_empty_settings_survive_semantic_round_trip() {
        let profile = DcsCodecProfile::fixture();
        let source = br#"<?xml version="1.0" encoding="UTF-8"?>
<DataCompositionSchema xmlns="http://v8.1c.ru/8.1/data-composition-system/schema" xmlns:dcsset="http://v8.1c.ru/8.1/data-composition-system/settings" xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
	<settingsVariant><dcsset:name>Main</dcsset:name><dcsset:presentation xsi:type="xs:string">Main</dcsset:presentation><dcsset:settings><dcsset:filter><dcsset:viewMode>Normal</dcsset:viewMode></dcsset:filter></dcsset:settings></settingsVariant>
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
        assert!(exported.contains("<dcsset:viewMode>Normal</dcsset:viewMode>"));
        assert_eq!(exported.matches("<settingsVariant>").count(), 1);
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
