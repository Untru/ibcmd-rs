//! Typed dispatch for base-free template bodies.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use ibcmd_core::artifact::ProfileId;
use ibcmd_core::profile::EffectiveProfile;

use super::dcs::{
    DcsBody, DcsCodecError, DcsCodecProfile, DcsTemplateKind, compile_evidenced_dcs,
    decode_compatible_dcs, validate_raw_xml_root,
};
use super::mxl::{
    MxlBody, MxlCodecError, MxlCodecProfile, compile_evidenced_mxl, decode_compatible_mxl,
    decode_evidenced_mxl,
};
use super::{BodyProfileError, SelectedBodyProfile};
use crate::compiler::families::native::{
    NativeError, deflate_bytes, exact_list, exact_token, inflate, parse_without_bom, required_list,
    required_text, required_token,
};
use crate::module_blob::{
    MetadataSourceContext, SpreadsheetNumberFormatHint, decode_base64_mime, encode_base64,
    pack_help_blob_from_parts,
};

const LAYOUT_KEY: &str = "bootstrap.body.template.layout";
const LAYOUT: &str = "template-kind-dispatch-v1";
const GRAPHICAL_SCHEMA_NS: &[u8] = b"http://v8.1c.ru/8.3/xcf/scheme";
const MAX_EMBEDDED_FILES: usize = 100_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemplateCodecProfile {
    selected: SelectedBodyProfile,
    _mxl: MxlCodecProfile,
    _dcs: DcsCodecProfile,
}

impl TemplateCodecProfile {
    pub fn from_effective(profile: &EffectiveProfile) -> Result<Self, BodyProfileError> {
        Ok(Self {
            selected: SelectedBodyProfile::from_effective(profile, LAYOUT_KEY, LAYOUT)?,
            _mxl: MxlCodecProfile::from_effective(profile)?,
            _dcs: DcsCodecProfile::from_effective(profile)?,
        })
    }

    pub const fn profile_id(&self) -> &ProfileId {
        self.selected.profile_id()
    }

    #[cfg(test)]
    fn fixture() -> Self {
        Self {
            selected: SelectedBodyProfile::fixture("platform-8.3.27.1989"),
            _mxl: MxlCodecProfile::fixture(),
            _dcs: DcsCodecProfile::fixture(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TemplateKind {
    AddIn,
    BinaryData,
    DataCompositionAppearanceTemplate,
    DataCompositionSchema,
    GraphicalSchema,
    HtmlDocument,
    SpreadsheetDocument,
    TextDocument,
}

impl TemplateKind {
    pub fn parse(value: &str) -> Result<Self, TemplateCodecError> {
        match value {
            "AddIn" => Ok(Self::AddIn),
            "BinaryData" => Ok(Self::BinaryData),
            "DataCompositionAppearanceTemplate" => Ok(Self::DataCompositionAppearanceTemplate),
            "DataCompositionSchema" => Ok(Self::DataCompositionSchema),
            "GraphicalSchema" => Ok(Self::GraphicalSchema),
            "HTMLDocument" => Ok(Self::HtmlDocument),
            "SpreadsheetDocument" => Ok(Self::SpreadsheetDocument),
            "TextDocument" => Ok(Self::TextDocument),
            _ => Err(TemplateCodecError::UnsupportedKind(value.to_owned())),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AddIn => "AddIn",
            Self::BinaryData => "BinaryData",
            Self::DataCompositionAppearanceTemplate => "DataCompositionAppearanceTemplate",
            Self::DataCompositionSchema => "DataCompositionSchema",
            Self::GraphicalSchema => "GraphicalSchema",
            Self::HtmlDocument => "HTMLDocument",
            Self::SpreadsheetDocument => "SpreadsheetDocument",
            Self::TextDocument => "TextDocument",
        }
    }

    pub const fn source_file(self) -> &'static str {
        match self {
            Self::AddIn | Self::BinaryData => "Template.bin",
            Self::TextDocument => "Template.txt",
            Self::DataCompositionAppearanceTemplate
            | Self::DataCompositionSchema
            | Self::GraphicalSchema
            | Self::HtmlDocument
            | Self::SpreadsheetDocument => "Template.xml",
        }
    }
}

impl Display for TemplateKind {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

pub enum TemplateSource<'a> {
    Bytes(&'a [u8]),
    Html {
        pages: &'a [(String, Vec<u8>)],
        files: &'a [(String, Vec<u8>)],
    },
    Spreadsheet {
        xml: &'a [u8],
        source: Option<&'a MetadataSourceContext>,
        number_format_hint: Option<&'a SpreadsheetNumberFormatHint>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemplateEmbeddedFile {
    name: String,
    content: Vec<u8>,
}

impl TemplateEmbeddedFile {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn content(&self) -> &[u8] {
        &self.content
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HtmlTemplateBody {
    pages: Vec<TemplateEmbeddedFile>,
    files: Vec<TemplateEmbeddedFile>,
}

impl HtmlTemplateBody {
    pub fn pages(&self) -> &[TemplateEmbeddedFile] {
        &self.pages
    }

    pub fn files(&self) -> &[TemplateEmbeddedFile] {
        &self.files
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TemplateBody {
    Raw { kind: TemplateKind, bytes: Vec<u8> },
    Binary { kind: TemplateKind, bytes: Vec<u8> },
    Html(HtmlTemplateBody),
    Spreadsheet(MxlBody),
    DataComposition(DcsBody),
}

impl TemplateBody {
    pub const fn kind(&self) -> TemplateKind {
        match self {
            Self::Raw { kind, .. } | Self::Binary { kind, .. } => *kind,
            Self::Html(_) => TemplateKind::HtmlDocument,
            Self::Spreadsheet(_) => TemplateKind::SpreadsheetDocument,
            Self::DataComposition(body) => match body.kind() {
                DcsTemplateKind::Schema => TemplateKind::DataCompositionSchema,
                DcsTemplateKind::Appearance => TemplateKind::DataCompositionAppearanceTemplate,
            },
        }
    }

    pub fn exact_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Raw { bytes, .. } | Self::Binary { bytes, .. } => Some(bytes),
            _ => None,
        }
    }

    pub const fn html(&self) -> Option<&HtmlTemplateBody> {
        match self {
            Self::Html(body) => Some(body),
            _ => None,
        }
    }

    pub const fn spreadsheet(&self) -> Option<&MxlBody> {
        match self {
            Self::Spreadsheet(body) => Some(body),
            _ => None,
        }
    }

    pub const fn data_composition(&self) -> Option<&DcsBody> {
        match self {
            Self::DataComposition(body) => Some(body),
            _ => None,
        }
    }
}

pub fn compile_template(
    profile: &TemplateCodecProfile,
    kind: TemplateKind,
    source: TemplateSource<'_>,
) -> Result<Vec<u8>, TemplateCodecError> {
    let _ = profile;
    compile_evidenced_template(kind, source)
}

pub(crate) fn compile_evidenced_template(
    kind: TemplateKind,
    source: TemplateSource<'_>,
) -> Result<Vec<u8>, TemplateCodecError> {
    let blob = match (kind, source) {
        (TemplateKind::DataCompositionSchema, TemplateSource::Bytes(xml)) => {
            compile_evidenced_dcs(DcsTemplateKind::Schema, xml)
                .map_err(|error| TemplateCodecError::Dcs(error.to_string()))?
        }
        (TemplateKind::DataCompositionAppearanceTemplate, TemplateSource::Bytes(xml)) => {
            compile_evidenced_dcs(DcsTemplateKind::Appearance, xml)
                .map_err(|error| TemplateCodecError::Dcs(error.to_string()))?
        }
        (TemplateKind::GraphicalSchema, TemplateSource::Bytes(xml)) => {
            validate_raw_xml_root(xml, "GraphicalSchema", Some(GRAPHICAL_SCHEMA_NS))
                .map_err(|error| TemplateCodecError::InvalidSource(error.to_string()))?;
            deflate_bytes(xml)?
        }
        (TemplateKind::TextDocument, TemplateSource::Bytes(bytes)) => deflate_bytes(bytes)?,
        (TemplateKind::AddIn | TemplateKind::BinaryData, TemplateSource::Bytes(bytes)) => {
            compile_binary_template(bytes)?
        }
        (TemplateKind::HtmlDocument, TemplateSource::Html { pages, files }) => {
            validate_html_parts(pages, files)?;
            pack_help_blob_from_parts(pages, files)
                .map_err(|error| TemplateCodecError::InvalidSource(error.to_string()))?
                .blob
        }
        (
            TemplateKind::SpreadsheetDocument,
            TemplateSource::Spreadsheet {
                xml,
                source,
                number_format_hint,
            },
        ) => compile_evidenced_mxl(xml, source, number_format_hint)
            .map_err(|error| TemplateCodecError::Mxl(error.to_string()))?,
        (kind, _) => return Err(TemplateCodecError::WrongSource(kind)),
    };
    decode_evidenced_template(kind, &blob)?;
    Ok(blob)
}

pub fn decode_template(
    profile: &TemplateCodecProfile,
    kind: TemplateKind,
    blob: &[u8],
) -> Result<TemplateBody, TemplateCodecError> {
    let _ = profile;
    decode_evidenced_template(kind, blob)
}

/// Reads both the evidenced profile layout and the bounded layouts emitted by
/// older ibcmd-rs builds. New bodies must still be written through
/// [`compile_template`], which always emits the selected profile layout.
pub fn decode_compatible_template(
    kind: TemplateKind,
    blob: &[u8],
) -> Result<TemplateBody, TemplateCodecError> {
    match kind {
        TemplateKind::DataCompositionSchema => decode_compatible_dcs(DcsTemplateKind::Schema, blob)
            .map(TemplateBody::DataComposition)
            .map_err(|error| TemplateCodecError::Dcs(error.to_string())),
        TemplateKind::DataCompositionAppearanceTemplate => {
            decode_compatible_dcs(DcsTemplateKind::Appearance, blob)
                .map(TemplateBody::DataComposition)
                .map_err(|error| TemplateCodecError::Dcs(error.to_string()))
        }
        TemplateKind::SpreadsheetDocument => decode_compatible_mxl(blob)
            .map(TemplateBody::Spreadsheet)
            .map_err(|error| TemplateCodecError::Mxl(error.to_string())),
        _ => decode_evidenced_template(kind, blob),
    }
}

fn decode_evidenced_template(
    kind: TemplateKind,
    blob: &[u8],
) -> Result<TemplateBody, TemplateCodecError> {
    match kind {
        TemplateKind::DataCompositionSchema => {
            let body = decode_compatible_dcs(DcsTemplateKind::Schema, blob)
                .map_err(|error| TemplateCodecError::Dcs(error.to_string()))?;
            if body.layout() != super::dcs::DcsBodyLayout::NativeThreeDocument {
                return Err(TemplateCodecError::UnsupportedLayout(
                    "DataCompositionSchema direct-XML compatibility body".to_string(),
                ));
            }
            Ok(TemplateBody::DataComposition(body))
        }
        TemplateKind::DataCompositionAppearanceTemplate => {
            decode_compatible_dcs(DcsTemplateKind::Appearance, blob)
                .map(TemplateBody::DataComposition)
                .map_err(|error| TemplateCodecError::Dcs(error.to_string()))
        }
        TemplateKind::GraphicalSchema => {
            let bytes = inflate(blob)?;
            validate_raw_xml_root(&bytes, "GraphicalSchema", Some(GRAPHICAL_SCHEMA_NS))
                .map_err(|error| TemplateCodecError::InvalidSource(error.to_string()))?;
            Ok(TemplateBody::Raw { kind, bytes })
        }
        TemplateKind::TextDocument => Ok(TemplateBody::Raw {
            kind,
            bytes: inflate(blob)?,
        }),
        TemplateKind::AddIn | TemplateKind::BinaryData => Ok(TemplateBody::Binary {
            kind,
            bytes: decode_binary_template(blob)?,
        }),
        TemplateKind::HtmlDocument => decode_html_template(blob).map(TemplateBody::Html),
        TemplateKind::SpreadsheetDocument => decode_evidenced_mxl(blob)
            .map(TemplateBody::Spreadsheet)
            .map_err(|error| TemplateCodecError::Mxl(error.to_string())),
    }
}

fn compile_binary_template(bytes: &[u8]) -> Result<Vec<u8>, TemplateCodecError> {
    let plain = format!("{{1,\r\n{{#base64:{}}}}}", encode_base64(bytes));
    deflate_bytes(plain.as_bytes()).map_err(Into::into)
}

fn decode_binary_template(blob: &[u8]) -> Result<Vec<u8>, TemplateCodecError> {
    let plain = inflate(blob)?;
    let native = parse_without_bom(&plain)?;
    let fields = exact_list(&native, 2, "binary Template root")?;
    exact_token(&fields[0], "1", "binary Template marker")?;
    decode_base64_value(&fields[1], "binary Template payload")
}

fn decode_html_template(blob: &[u8]) -> Result<HtmlTemplateBody, TemplateCodecError> {
    let plain = inflate(blob)?;
    let native = parse_without_bom(&plain)?;
    let fields = required_list(&native, "HTML Template root")?;
    exact_token(
        fields
            .first()
            .ok_or_else(|| TemplateCodecError::InvalidShape("empty HTML Template".to_string()))?,
        "5",
        "HTML Template marker",
    )?;
    let page_count = parse_count(fields.get(1), "HTML Template page count")?;
    let mut index = 2usize;
    let mut pages = Vec::with_capacity(page_count);
    for _ in 0..page_count {
        let name = required_text(
            fields
                .get(index)
                .ok_or_else(|| TemplateCodecError::InvalidShape("missing HTML page".to_string()))?,
            "HTML page name",
        )?
        .to_owned();
        index += 1;
        let content = decode_base64_value(
            fields.get(index).ok_or_else(|| {
                TemplateCodecError::InvalidShape("missing HTML page payload".to_string())
            })?,
            "HTML page payload",
        )?;
        index += 1;
        pages.push(TemplateEmbeddedFile { name, content });
    }
    let file_count = parse_count(fields.get(index), "HTML Template file count")?;
    index += 1;
    let mut files = Vec::with_capacity(file_count);
    for _ in 0..file_count {
        let name = required_text(
            fields.get(index).ok_or_else(|| {
                TemplateCodecError::InvalidShape("missing HTML file name".to_string())
            })?,
            "HTML file name",
        )?
        .to_owned();
        index += 1;
        exact_token(
            fields.get(index).ok_or_else(|| {
                TemplateCodecError::InvalidShape("missing HTML file marker".to_string())
            })?,
            "1",
            "HTML file marker",
        )?;
        index += 1;
        let content = decode_base64_value(
            fields.get(index).ok_or_else(|| {
                TemplateCodecError::InvalidShape("missing HTML file payload".to_string())
            })?,
            "HTML file payload",
        )?;
        index += 1;
        files.push(TemplateEmbeddedFile { name, content });
    }
    if index != fields.len() {
        return Err(TemplateCodecError::UnsupportedLayout(
            "HTML Template has an unknown tail".to_string(),
        ));
    }
    let page_parts = pages
        .iter()
        .map(|item| (item.name.clone(), item.content.clone()))
        .collect::<Vec<_>>();
    let file_parts = files
        .iter()
        .map(|item| (item.name.clone(), item.content.clone()))
        .collect::<Vec<_>>();
    validate_html_parts(&page_parts, &file_parts)?;
    Ok(HtmlTemplateBody { pages, files })
}

fn parse_count(
    value: Option<&crate::compiler::families::native::NativeValue>,
    field: &'static str,
) -> Result<usize, TemplateCodecError> {
    let count = required_token(
        value.ok_or_else(|| TemplateCodecError::InvalidShape(format!("missing {field}")))?,
        field,
    )?
    .parse::<usize>()
    .map_err(|_| TemplateCodecError::InvalidShape(format!("invalid {field}")))?;
    if count > MAX_EMBEDDED_FILES {
        return Err(TemplateCodecError::LimitExceeded(field));
    }
    Ok(count)
}

fn decode_base64_value(
    value: &crate::compiler::families::native::NativeValue,
    field: &'static str,
) -> Result<Vec<u8>, TemplateCodecError> {
    let wrapper = exact_list(value, 1, field)?;
    let token = required_token(&wrapper[0], field)?;
    let payload = token
        .strip_prefix("#base64:")
        .ok_or_else(|| TemplateCodecError::InvalidShape(format!("{field} wrapper is invalid")))?;
    decode_base64_mime(payload)
        .ok_or_else(|| TemplateCodecError::InvalidShape(format!("{field} is invalid base64")))
}

fn validate_html_parts(
    pages: &[(String, Vec<u8>)],
    files: &[(String, Vec<u8>)],
) -> Result<(), TemplateCodecError> {
    if pages.is_empty() {
        return Err(TemplateCodecError::InvalidSource(
            "HTML Template requires at least one page".to_string(),
        ));
    }
    for (label, values) in [("page", pages), ("file", files)] {
        if values.len() > MAX_EMBEDDED_FILES {
            return Err(TemplateCodecError::LimitExceeded("HTML embedded files"));
        }
        let mut names = BTreeSet::new();
        for (name, _) in values {
            if name.is_empty()
                || name == "."
                || name == ".."
                || name.chars().any(|value| matches!(value, '/' | '\\' | '\0'))
            {
                return Err(TemplateCodecError::InvalidSource(format!(
                    "unsafe HTML Template {label} name `{name}`"
                )));
            }
            if !names.insert(name) {
                return Err(TemplateCodecError::InvalidSource(format!(
                    "duplicate HTML Template {label} name `{name}`"
                )));
            }
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TemplateCodecError {
    Profile(BodyProfileError),
    Native(String),
    Dcs(String),
    Mxl(String),
    InvalidSource(String),
    InvalidShape(String),
    UnsupportedKind(String),
    UnsupportedLayout(String),
    WrongSource(TemplateKind),
    LimitExceeded(&'static str),
}

impl Display for TemplateCodecError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Profile(source) => Display::fmt(source, formatter),
            Self::Native(reason) => {
                write!(formatter, "native Template codec rejected data: {reason}")
            }
            Self::Dcs(reason) => write!(formatter, "DCS Template codec rejected data: {reason}"),
            Self::Mxl(reason) => write!(formatter, "MXL Template codec rejected data: {reason}"),
            Self::InvalidSource(reason) => {
                write!(formatter, "Template source is invalid: {reason}")
            }
            Self::InvalidShape(reason) => write!(formatter, "Template body is invalid: {reason}"),
            Self::UnsupportedKind(kind) => write!(formatter, "unsupported Template kind `{kind}`"),
            Self::UnsupportedLayout(reason) => {
                write!(formatter, "unsupported Template body layout: {reason}")
            }
            Self::WrongSource(kind) => {
                write!(
                    formatter,
                    "Template kind `{kind}` received the wrong source model"
                )
            }
            Self::LimitExceeded(field) => write!(formatter, "{field} exceeds the standalone limit"),
        }
    }
}

impl Error for TemplateCodecError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Profile(source) => Some(source),
            _ => None,
        }
    }
}

impl From<BodyProfileError> for TemplateCodecError {
    fn from(source: BodyProfileError) -> Self {
        Self::Profile(source)
    }
}

impl From<NativeError> for TemplateCodecError {
    fn from(source: NativeError) -> Self {
        Self::Native(source.to_string())
    }
}

impl From<DcsCodecError> for TemplateCodecError {
    fn from(source: DcsCodecError) -> Self {
        Self::Dcs(source.to_string())
    }
}

impl From<MxlCodecError> for TemplateCodecError {
    fn from(source: MxlCodecError) -> Self {
        Self::Mxl(source.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCHEMA: &[u8] = br#"<DataCompositionSchema xmlns="http://v8.1c.ru/8.1/data-composition-system/schema" xmlns:dcsset="http://v8.1c.ru/8.1/data-composition-system/settings" xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><dataSource><name>S</name><dataSourceType>Local</dataSourceType></dataSource><settingsVariant><dcsset:name>Main</dcsset:name><dcsset:presentation xsi:type="xs:string">Main</dcsset:presentation><dcsset:settings/></settingsVariant></DataCompositionSchema>"#;
    const APPEARANCE: &[u8] = br#"<AppearanceTemplate xmlns="http://v8.1c.ru/8.1/data-composition-system/appearance-template"/>"#;
    const GRAPHICAL: &[u8] = br#"<GraphicalSchema xmlns="http://v8.1c.ru/8.3/xcf/scheme" version="2.20"><Items/></GraphicalSchema>"#;
    const SPREADSHEET: &[u8] = br#"<document xmlns="http://v8.1c.ru/8.2/data/spreadsheet" xmlns:v8="http://v8.1c.ru/8.1/data/core"><columns><size>1</size></columns><rowsItem><index>0</index><row><c><c><f>0</f><tl><v8:item><v8:lang>en</v8:lang><v8:content>A</v8:content></v8:item></tl></c></c></row></rowsItem></document>"#;

    #[test]
    fn all_claimed_template_kinds_compile_and_decode() {
        let profile = TemplateCodecProfile::fixture();
        for (kind, bytes) in [
            (TemplateKind::TextDocument, b"text\0body".as_slice()),
            (TemplateKind::GraphicalSchema, GRAPHICAL),
            (TemplateKind::DataCompositionSchema, SCHEMA),
            (TemplateKind::DataCompositionAppearanceTemplate, APPEARANCE),
            (TemplateKind::BinaryData, b"PK\x03\x04".as_slice()),
            (TemplateKind::AddIn, b"addin\0payload".as_slice()),
        ] {
            let blob = compile_template(&profile, kind, TemplateSource::Bytes(bytes)).unwrap();
            let decoded = decode_template(&profile, kind, &blob).unwrap();
            assert_eq!(decoded.kind(), kind);
            if matches!(
                kind,
                TemplateKind::TextDocument
                    | TemplateKind::GraphicalSchema
                    | TemplateKind::BinaryData
                    | TemplateKind::AddIn
            ) {
                assert_eq!(decoded.exact_bytes(), Some(bytes));
            }
        }

        let pages = vec![("index".to_string(), b"<html>ok</html>".to_vec())];
        let files = vec![("logo.bin".to_string(), b"\0PNG\xff".to_vec())];
        let html = compile_template(
            &profile,
            TemplateKind::HtmlDocument,
            TemplateSource::Html {
                pages: &pages,
                files: &files,
            },
        )
        .unwrap();
        let html = decode_template(&profile, TemplateKind::HtmlDocument, &html).unwrap();
        assert_eq!(
            html.html().unwrap().pages()[0].content(),
            pages[0].1.as_slice()
        );
        assert_eq!(
            html.html().unwrap().files()[0].content(),
            files[0].1.as_slice()
        );

        let mxl = compile_template(
            &profile,
            TemplateKind::SpreadsheetDocument,
            TemplateSource::Spreadsheet {
                xml: SPREADSHEET,
                source: None,
                number_format_hint: None,
            },
        )
        .unwrap();
        let mxl = decode_template(&profile, TemplateKind::SpreadsheetDocument, &mxl).unwrap();
        assert_eq!(mxl.spreadsheet().unwrap().declared_columns(), 1);
    }

    #[test]
    fn binary_wrapper_is_evidenced_and_unknown_kinds_or_tails_block() {
        let profile = TemplateCodecProfile::fixture();
        let blob = compile_template(
            &profile,
            TemplateKind::BinaryData,
            TemplateSource::Bytes(b"binary"),
        )
        .unwrap();
        let plain = inflate(&blob).unwrap();
        assert!(plain.starts_with(b"{1,"));
        assert!(plain.windows(8).any(|window| window == b"{#base64"));

        assert!(matches!(
            TemplateKind::parse("FutureTemplate"),
            Err(TemplateCodecError::UnsupportedKind(kind)) if kind == "FutureTemplate"
        ));
        let unknown = deflate_bytes(b"{2,{#base64:YQ==}}").unwrap();
        assert!(decode_template(&profile, TemplateKind::BinaryData, &unknown).is_err());
    }
}
