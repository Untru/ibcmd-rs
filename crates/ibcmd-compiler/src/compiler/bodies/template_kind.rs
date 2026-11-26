//! The kind of a template and the error of its codec: what reads a template
//! kind without its body codec (`compiler::external_owned`) names it here,
//! not through `template`, which compiles the bodies through the DCS and MXL
//! codecs. `template` re-exports both.

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use super::BodyProfileError;
use crate::compiler::families::native::NativeError;

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
