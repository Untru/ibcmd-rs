//! Profile-gated base-free codec for SpreadsheetDocument (MOXCEL/MXL) bodies.

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use ibcmd_core::artifact::ProfileId;
use ibcmd_core::profile::EffectiveProfile;

use super::{BodyProfileError, SelectedBodyProfile};
use crate::compiler::families::native::{
    NativeError, exact_list, exact_token, inflate, parse, parse_without_bom, required_list,
    required_token,
};
use crate::module_blob::{
    MetadataSourceContext, SpreadsheetNumberFormatHint,
    pack_moxel_spreadsheet_blob_from_xml_with_source_and_hint,
};

const LAYOUT_KEY: &str = "bootstrap.body.mxl.layout";
const LAYOUT: &str = "moxel-v8-raw-deflate-v1";
const MOXCEL_HEADER: &[u8] = b"MOXCEL\0\x08\0\x01\0\x0c\0";
const UTF8_BOM: &[u8] = b"\xef\xbb\xbf";
const ROOT_PREFIX: &[u8] = b"{8,";
const MAX_COLUMNS: usize = 1_000_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MxlCodecProfile(SelectedBodyProfile);

impl MxlCodecProfile {
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MxlBody {
    plain: String,
    body_start: usize,
    declared_columns: usize,
    native_fields: usize,
}

impl MxlBody {
    pub fn plaintext(&self) -> &[u8] {
        self.plain.as_bytes()
    }

    pub fn native_body_text(&self) -> &str {
        &self.plain[self.body_start..]
    }

    pub const fn declared_columns(&self) -> usize {
        self.declared_columns
    }

    pub const fn native_fields(&self) -> usize {
        self.native_fields
    }
}

pub fn compile_mxl(
    profile: &MxlCodecProfile,
    xml: &[u8],
    source: Option<&MetadataSourceContext>,
    number_format_hint: Option<&SpreadsheetNumberFormatHint>,
) -> Result<Vec<u8>, MxlCodecError> {
    let _ = profile;
    compile_evidenced_mxl(xml, source, number_format_hint)
}

pub(crate) fn compile_evidenced_mxl(
    xml: &[u8],
    source: Option<&MetadataSourceContext>,
    number_format_hint: Option<&SpreadsheetNumberFormatHint>,
) -> Result<Vec<u8>, MxlCodecError> {
    let packed =
        pack_moxel_spreadsheet_blob_from_xml_with_source_and_hint(xml, source, number_format_hint)
            .map_err(|error| MxlCodecError::Source(error.to_string()))?;
    decode_strict(&packed.blob)?;
    Ok(packed.blob)
}

pub fn decode_mxl(profile: &MxlCodecProfile, blob: &[u8]) -> Result<MxlBody, MxlCodecError> {
    let _ = profile;
    decode_strict(blob)
}

/// Bounded reader for retained native rows and older unit fixtures. The
/// standalone profile decoder above additionally requires the exact evidenced
/// binary header and canonical trailer.
pub(crate) fn decode_compatible_mxl(blob: &[u8]) -> Result<MxlBody, MxlCodecError> {
    decode(blob, false)
}

fn decode_strict(blob: &[u8]) -> Result<MxlBody, MxlCodecError> {
    decode(blob, true)
}

pub(crate) fn decode_evidenced_mxl(blob: &[u8]) -> Result<MxlBody, MxlCodecError> {
    decode_strict(blob)
}

fn decode(blob: &[u8], strict: bool) -> Result<MxlBody, MxlCodecError> {
    let plain = inflate(blob)?;
    if !plain.starts_with(b"MOXCEL") {
        return Err(MxlCodecError::UnsupportedLayout(
            "missing MOXCEL signature".to_string(),
        ));
    }

    let body_start = if strict {
        if !plain.starts_with(MOXCEL_HEADER) {
            return Err(MxlCodecError::UnsupportedLayout(
                "unknown MOXCEL binary header".to_string(),
            ));
        }
        let bom_start = MOXCEL_HEADER.len();
        if plain.get(bom_start..bom_start + UTF8_BOM.len()) != Some(UTF8_BOM) {
            return Err(MxlCodecError::UnsupportedLayout(
                "MOXCEL native body has no UTF-8 BOM".to_string(),
            ));
        }
        bom_start + UTF8_BOM.len()
    } else {
        find_bytes(&plain, ROOT_PREFIX).ok_or_else(|| {
            MxlCodecError::UnsupportedLayout("MOXCEL marker-8 root is absent".to_string())
        })?
    };

    let native = if strict {
        parse(&plain[MOXCEL_HEADER.len()..])?
    } else {
        parse_without_bom(&plain[body_start..])?
    };
    let fields = required_list(&native, "MOXCEL root")?;
    if fields.len() < 8 {
        return Err(MxlCodecError::InvalidShape(
            "MOXCEL marker-8 root is too short".to_string(),
        ));
    }
    exact_token(&fields[0], "8", "MOXCEL root marker")?;
    exact_token(&fields[1], "1", "MOXCEL root version")?;
    let declared_columns = required_token(&fields[2], "MOXCEL column count")?
        .parse::<usize>()
        .map_err(|_| MxlCodecError::InvalidShape("invalid MOXCEL column count".to_string()))?
        .checked_add(1)
        .ok_or_else(|| MxlCodecError::InvalidShape("MOXCEL column count overflow".to_string()))?;
    if declared_columns > MAX_COLUMNS {
        return Err(MxlCodecError::LimitExceeded("MOXCEL column count"));
    }
    let language = required_list(&fields[3], "MOXCEL language descriptor")?;
    if language.len() != 8 {
        return Err(MxlCodecError::InvalidShape(
            "MOXCEL language descriptor has an unknown layout".to_string(),
        ));
    }
    if strict {
        exact_token(
            &fields[fields.len() - 2],
            "2",
            "MOXCEL trailing version marker",
        )?;
        let tail = exact_list(&fields[fields.len() - 1], 2, "MOXCEL trailing descriptor")?;
        exact_token(&tail[0], "0", "MOXCEL trailing descriptor marker")?;
        exact_token(&tail[1], "1", "MOXCEL trailing descriptor version")?;
    }

    let plain = String::from_utf8(plain)
        .map_err(|_| MxlCodecError::InvalidShape("MOXCEL body is not UTF-8".to_string()))?;
    Ok(MxlBody {
        plain,
        body_start,
        declared_columns,
        native_fields: fields.len(),
    })
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MxlCodecError {
    Profile(BodyProfileError),
    Native(String),
    Source(String),
    UnsupportedLayout(String),
    InvalidShape(String),
    LimitExceeded(&'static str),
}

impl Display for MxlCodecError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Profile(source) => Display::fmt(source, formatter),
            Self::Native(reason) => write!(formatter, "native MXL codec rejected data: {reason}"),
            Self::Source(reason) => write!(formatter, "MXL source cannot be compiled: {reason}"),
            Self::UnsupportedLayout(reason) => {
                write!(formatter, "unsupported MXL body layout: {reason}")
            }
            Self::InvalidShape(reason) => write!(formatter, "invalid MXL body: {reason}"),
            Self::LimitExceeded(field) => write!(formatter, "{field} exceeds the standalone limit"),
        }
    }
}

impl Error for MxlCodecError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Profile(source) => Some(source),
            _ => None,
        }
    }
}

impl From<BodyProfileError> for MxlCodecError {
    fn from(source: BodyProfileError) -> Self {
        Self::Profile(source)
    }
}

impl From<NativeError> for MxlCodecError {
    fn from(source: NativeError) -> Self {
        Self::Native(source.to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::compiler::families::native::deflate_bytes;

    const SIMPLE_SPREADSHEET: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<document xmlns="http://v8.1c.ru/8.2/data/spreadsheet" xmlns:v8="http://v8.1c.ru/8.1/data/core">
	<columns><size>3</size></columns>
	<rowsItem><index>0</index><row>
		<c><c><f>0</f><tl><v8:item><v8:lang>en</v8:lang><v8:content>Hello</v8:content></v8:item></tl></c></c>
		<c><i>2</i><c><f>0</f><parameter>Name</parameter></c></c>
	</row></rowsItem>
</document>"#;

    #[test]
    fn spreadsheet_compiles_deterministically_and_roundtrips_semantically() {
        let profile = MxlCodecProfile::fixture();
        let first = compile_mxl(&profile, SIMPLE_SPREADSHEET, None, None).unwrap();
        let second = compile_mxl(&profile, SIMPLE_SPREADSHEET, None, None).unwrap();
        assert_eq!(first, second);

        let decoded = decode_mxl(&profile, &first).unwrap();
        assert!(decoded.plaintext().starts_with(MOXCEL_HEADER));
        assert!(decoded.native_body_text().starts_with("{8,"));
        assert_eq!(decoded.declared_columns(), 3);
        assert!(decoded.native_fields() >= 8);

        let xml = crate::mssql_dump::extract_moxel_spreadsheet_xml(&first, &BTreeMap::new())
            .expect("evidenced MOXCEL body must remain exportable");
        assert!(xml.contains("<v8:content>Hello</v8:content>"));
        assert!(xml.contains("<parameter>Name</parameter>"));
    }

    #[test]
    fn unknown_header_and_root_are_hard_blockers() {
        let profile = MxlCodecProfile::fixture();
        let unknown_header = deflate_bytes(
            b"MOXCEL\0\x09\0\x01\0\x0c\0\xef\xbb\xbf{8,1,0,{0,0,0,0,0,0,0,0},{0},{0},2,{0,1}}",
        )
        .unwrap();
        assert!(matches!(
            decode_mxl(&profile, &unknown_header),
            Err(MxlCodecError::UnsupportedLayout(_))
        ));

        let unknown_root = deflate_bytes(
            b"MOXCEL\0\x08\0\x01\0\x0c\0\xef\xbb\xbf{9,1,0,{0,0,0,0,0,0,0,0},{0},{0},2,{0,1}}",
        )
        .unwrap();
        assert!(decode_mxl(&profile, &unknown_root).is_err());
    }
}
