//! Profile-gated base-free codec for SpreadsheetDocument (MOXCEL/MXL) bodies.

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use ibcmd_core::artifact::ProfileId;
use ibcmd_core::profile::EffectiveProfile;

use super::mxl_native::pack_moxel_spreadsheet_blob_from_xml_with_source_and_hint;
use super::{BodyProfileError, SelectedBodyProfile};
use crate::compiler::families::native::{
    NativeError, exact_token, inflate, outline, outline_without_bom, required_list, required_token,
};
use ibcmd_metadata_blob::{MetadataSourceContext, SpreadsheetNumberFormatHint};

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

    #[cfg(any(test, feature = "test-fixtures"))]
    #[doc(hidden)]
    pub fn fixture() -> Self {
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
#[doc(hidden)]
pub fn decode_compatible_mxl(blob: &[u8]) -> Result<MxlBody, MxlCodecError> {
    decode(blob, false)
}

/// Decode a retained `Config_inflated` MOXCEL payload.  It shares the exact
/// signature, framing and body-shape checks of the compatible blob decoder;
/// only the outer deflate layer has already been removed by the dump reader.
#[doc(hidden)]
pub fn decode_inflated_compatible_mxl(plain: &[u8]) -> Result<MxlBody, MxlCodecError> {
    decode_plain(plain.to_vec(), false, true)
}

fn decode_strict(blob: &[u8]) -> Result<MxlBody, MxlCodecError> {
    decode(blob, true)
}

pub(crate) fn decode_evidenced_mxl(blob: &[u8]) -> Result<MxlBody, MxlCodecError> {
    decode_strict(blob)
}

fn decode(blob: &[u8], strict: bool) -> Result<MxlBody, MxlCodecError> {
    let plain = inflate(blob)?;
    decode_plain(plain, strict, false)
}

fn decode_plain(
    plain: Vec<u8>,
    strict: bool,
    exact_inflated_framing: bool,
) -> Result<MxlBody, MxlCodecError> {
    if !plain.starts_with(b"MOXCEL") {
        return Err(MxlCodecError::UnsupportedLayout(
            "missing MOXCEL signature".to_string(),
        ));
    }

    let body_start = if strict || exact_inflated_framing {
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

    // Only the root's first four and last six fields are read below, so the
    // body is checked the way the full parse checks it without building the
    // tree of the rest.
    let native = if strict {
        outline(&plain[MOXCEL_HEADER.len()..], 4, 6)?
    } else {
        outline_without_bom(&plain[body_start..], 4, 6)?
    };
    let Some(field_count) = native.len else {
        return Err(NativeError::Shape {
            field: "MOXCEL root",
        }
        .into());
    };
    if field_count < 8 {
        return Err(MxlCodecError::InvalidShape(
            "MOXCEL marker-8 root is too short".to_string(),
        ));
    }
    let fields = &native.head;
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
    // `{current, default, 0, count, (id, code, description) * count, 0}`: the
    // record declares its own member count at position 3, so the shape check
    // has to read that declared count rather than assume a single language
    // (evidence: ERP UH `Web_Service`, two spreadsheets carry a two-language
    // record - `ru` and `en` - at 11 fields, `count=2`, still `count*3+5`).
    //
    // The older root revision `8` (`{8,1,8,…}`) stores the same record
    // without its closing `0` (`{"ru","ru",1,1,"ru","Русский","Русский"}`,
    // `count*3+4`). Two real configurations carry 87 such spreadsheets each;
    // read with the closing member absent, 75 of one's 87 come out
    // byte-identical to the platform's own dump. Revision `9` stores it the
    // same way: 21 and 24 spreadsheets of the same two configurations, 19 of
    // the first's 21 byte-identical read so. Revision `6` too: 49
    // spreadsheets of ЗУП 3.1.18, 20 of them byte-identical read so.
    let language = required_list(&fields[3], "MOXCEL language descriptor")?;
    let short_language_record = matches!(fields[2].as_token(), Some("6" | "8" | "9"));
    let language_shape_ok = language.len() >= 5
        && required_token(&language[3], "MOXCEL language count")
            .ok()
            .and_then(|token| token.parse::<usize>().ok())
            .is_some_and(|count| {
                count <= 64
                    && (language.len() == count * 3 + 5
                        || (short_language_record && language.len() == count * 3 + 4))
            });
    if !language_shape_ok {
        return Err(MxlCodecError::InvalidShape(
            "MOXCEL language descriptor has an unknown layout".to_string(),
        ));
    }
    if strict {
        // Every stored spreadsheet row of both corpora (14 046 ERP УХ, 61
        // БСП) opens `{8,1,12,`: the third member is a constant of the
        // format, not a column count.
        exact_token(&fields[2], "12", "MOXCEL root revision")?;
        // The platform closes every body with the same six scalars: all 14 046
        // ERP УХ and 61 БСП stored spreadsheet rows end `…,0,0,1,0,0,0}`.
        let tail = Some(native.tail.as_slice())
            .filter(|tail| tail.len() == 6)
            .ok_or_else(|| MxlCodecError::InvalidShape("MOXCEL root has no trailer".to_string()))?;
        for (field, expected) in tail.iter().zip(["0", "0", "1", "0", "0", "0"]) {
            exact_token(field, expected, "MOXCEL trailer")?;
        }
    }

    let plain = String::from_utf8(plain)
        .map_err(|_| MxlCodecError::InvalidShape("MOXCEL body is not UTF-8".to_string()))?;
    Ok(MxlBody {
        plain,
        body_start,
        declared_columns,
        native_fields: field_count,
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
    use std::io::Read;

    use super::*;
    use crate::compiler::families::native::deflate_bytes;

    const SIMPLE_SPREADSHEET: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<document xmlns="http://v8.1c.ru/8.2/data/spreadsheet" xmlns:v8="http://v8.1c.ru/8.1/data/core">
	<columns><size>3</size></columns>
	<rowsItem><index>0</index><row>
		<c><c><f>0</f><tl><v8:item><v8:lang>en</v8:lang><v8:content>Hello</v8:content></v8:item></tl></c></c>
		<c><i>2</i><c><f>0</f><parameter>Name</parameter></c></c>
	</row></rowsItem>
	<defaultFormatIndex>1</defaultFormatIndex>
	<format><width>72</width></format>
</document>"#;

    /// Root revisions `6`, `8` and `9` store the language record without its closing `0`;
    /// the same record under revision `12` still has to carry it.
    #[test]
    fn revisions_6_8_and_9_read_the_short_language_record() {
        let profile = MxlCodecProfile::fixture();
        let compiled = compile_mxl(&profile, SIMPLE_SPREADSHEET, None, None).unwrap();
        let plain = String::from_utf8(inflate(&compiled).unwrap()).unwrap();
        let root = plain.find("{8,1,12,").unwrap();
        let record_start = root + plain[root + 1..].find('{').unwrap() + 1;
        let record_end = record_start + plain[record_start..].find('}').unwrap();
        let record = &plain[record_start..record_end];
        assert!(record.ends_with(",0"), "{record}");
        let short = &record[..record.len() - 2];
        let short_12 = format!(
            "{}{}{}",
            &plain[..record_start],
            short,
            &plain[record_end..]
        );
        assert!(decode_inflated_compatible_mxl(short_12.as_bytes()).is_err());
        for revision in ["6", "8", "9"] {
            let short = short_12.replacen("{8,1,12,", &format!("{{8,1,{revision},"), 1);
            let decoded = decode_inflated_compatible_mxl(short.as_bytes()).unwrap();
            assert!(
                decoded
                    .native_body_text()
                    .starts_with(&format!("{{8,1,{revision},"))
            );
        }
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

    #[test]
    fn inflated_mxl_requires_exact_header_bom_and_complete_root() {
        let packed =
            compile_mxl(&MxlCodecProfile::fixture(), SIMPLE_SPREADSHEET, None, None).unwrap();
        let mut plain = Vec::new();
        flate2::read::DeflateDecoder::new(packed.as_slice())
            .read_to_end(&mut plain)
            .unwrap();
        assert!(decode_inflated_compatible_mxl(&plain).is_ok());

        let mut bad_header = plain.clone();
        bad_header[7] ^= 1;
        assert!(decode_inflated_compatible_mxl(&bad_header).is_err());

        let mut bad_bom = plain.clone();
        bad_bom[MOXCEL_HEADER.len()] = b'x';
        assert!(decode_inflated_compatible_mxl(&bad_bom).is_err());

        let mut trailing = plain;
        trailing.extend_from_slice(b"junk");
        assert!(decode_inflated_compatible_mxl(&trailing).is_err());
    }
}
