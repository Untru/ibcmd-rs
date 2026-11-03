//! Profile-gated base-free codec for managed `Form.xml` native bodies.
//!
//! The detailed typed XML model and the evidenced marker-50 formatter live in
//! `module_blob` while the legacy dump/export code is being decomposed.  This
//! module is the compiler boundary: it selects the platform cohort, applies
//! bounded payload decoding, and rejects every native layout other than the
//! evidenced marker-50 body with four typed trailing sections.

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use ibcmd_core::artifact::ProfileId;
use ibcmd_core::profile::EffectiveProfile;

use super::{BodyProfileError, SelectedBodyProfile};
use crate::compiler::families::native::{NativeError, inflate};
use ibcmd_metadata_blob::{
    MetadataSourceContext, ParsedFormBodyBlob, pack_form_body_blob_from_form_xml_base_free,
    parse_form_body_blob,
};

const LAYOUT_KEY: &str = "bootstrap.body.form.layout";
const LAYOUT: &str = "managed-form-marker50-v1-raw-deflate-utf8-bom";
const UTF8_BOM: &[u8; 3] = b"\xef\xbb\xbf";
const FORM_LAYOUT_MARKER: &str = "50";
const FORM_TRAILING_SECTIONS: usize = 4;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagedFormCodecProfile(SelectedBodyProfile);

impl ManagedFormCodecProfile {
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
pub struct ManagedFormBody {
    plain: Vec<u8>,
    parsed: ParsedFormBodyBlob,
}

impl ManagedFormBody {
    pub const fn parsed(&self) -> &ParsedFormBodyBlob {
        &self.parsed
    }

    pub fn plaintext(&self) -> &[u8] {
        &self.plain
    }

    pub fn module_text(&self) -> &str {
        &self.parsed.module_text
    }
}

pub fn compile_managed_form(
    profile: &ManagedFormCodecProfile,
    form_xml: &[u8],
    module_text: Option<&[u8]>,
    source: Option<&MetadataSourceContext>,
) -> Result<Vec<u8>, ManagedFormCodecError> {
    let _ = profile;
    let packed = pack_form_body_blob_from_form_xml_base_free(form_xml, module_text, source)
        .map_err(|error| ManagedFormCodecError::Source(error.to_string()))?;
    // Validate our own output through the same strict boundary used for
    // storage input. This keeps new formatter branches fail-closed.
    decode_strict(&packed.blob)?;
    Ok(packed.blob)
}

pub fn decode_managed_form(
    profile: &ManagedFormCodecProfile,
    blob: &[u8],
) -> Result<ManagedFormBody, ManagedFormCodecError> {
    let _ = profile;
    decode_managed_form_layout(blob)
}

/// Decode the existing reader layout, including its legacy upgrade behavior.
/// Success is not platform, SQL schema or write admission.
pub fn decode_managed_form_layout(blob: &[u8]) -> Result<ManagedFormBody, ManagedFormCodecError> {
    decode_strict(blob)
}

/// Validate the original managed Form layout before any legacy reader upgrade.
pub fn validate_original_managed_form_layout(
    blob: &[u8],
) -> Result<ManagedFormBody, ManagedFormCodecError> {
    original_layout(blob).map(|(body, _)| body)
}

fn original_layout(
    blob: &[u8],
) -> Result<(ManagedFormBody, std::ops::Range<usize>), ManagedFormCodecError> {
    let plain = inflate(blob)?;
    if !plain.starts_with(UTF8_BOM) {
        return Err(ManagedFormCodecError::MissingBom);
    }
    let text = std::str::from_utf8(&plain)
        .map_err(|error| ManagedFormCodecError::NativeBody(error.to_string()))?;
    let (parsed, range) = ibcmd_form_body::parse_original_form_body_plain(text)
        .map_err(|error| ManagedFormCodecError::NativeBody(error.to_string()))?;
    validate_layout(&parsed)?;
    // The field scanner includes whitespace; only the quoted token may change.
    let field = &text[range.clone()];
    let start = range.start + field.len() - field.trim_start().len();
    let end = range.end - (field.len() - field.trim_end().len());
    Ok((ManagedFormBody { plain, parsed }, start..end))
}

/// Require exact original UTF-8 bytes outside the quoted module token.
/// Both containers must satisfy the original admitted layout; this grants no write authority.
pub fn verify_module_only_edit(old: &[u8], new: &[u8]) -> Result<(), ManagedFormCodecError> {
    let (old, a) = original_layout(old)?;
    let (new, b) = original_layout(new)?;
    if old.plain[..a.start] != new.plain[..b.start] || old.plain[a.end..] != new.plain[b.end..] {
        return Err(ManagedFormCodecError::InvalidShape(
            "Form edit changed bytes outside the quoted module".to_owned(),
        ));
    }
    Ok(())
}

/// Decode the original native V4/marker50 module-only envelope. This grammar
/// witness admits no Form.xml rewrite, platform build or SQL write authority.
/// Five trailing blocks are retained as opaque bytes; both final scalars are zero.
/// The base-free four-tail compiler/profile APIs keep their existing contract.
pub fn decode_native_managed_form_module_layout(
    blob: &[u8],
) -> Result<ManagedFormBody, ManagedFormCodecError> {
    native_module_layout(blob).map(|(body, _)| body)
}

fn native_module_layout(
    blob: &[u8],
) -> Result<(ManagedFormBody, std::ops::Range<usize>), ManagedFormCodecError> {
    use crate::compiler::families::native::parse;

    let plain = inflate(blob)?;
    // One existing bounded lexical parser checks BOM, depth, quoting and full
    // input consumption. Never serialize this tree back into storage.
    let native = parse(&plain)?;
    let fields = native.as_list().ok_or_else(|| {
        ManagedFormCodecError::InvalidShape("native Form envelope is not a list".into())
    })?;
    if fields.len() != 10
        || fields[0].as_token() != Some("4")
        || fields[1]
            .as_list()
            .and_then(|v| v.first())
            .and_then(|v| v.as_token())
            != Some("50")
        || fields[2].as_text().is_none()
        || fields[3..8].iter().any(|v| v.as_list().is_none())
        || fields[8].as_token() != Some("0")
        || fields[9].as_token() != Some("0")
    {
        return Err(ManagedFormCodecError::InvalidShape(
            "native module edit requires V4/marker50, five blocks and two zero scalars".into(),
        ));
    }
    let text = std::str::from_utf8(&plain)
        .map_err(|error| ManagedFormCodecError::NativeBody(error.to_string()))?;
    let (parsed, range) = ibcmd_form_body::parse_original_form_body_plain(text)
        .map_err(|error| ManagedFormCodecError::NativeBody(error.to_string()))?;
    // Original container only: no legacy layout upgrade or module reformatting.
    if parsed.revision != ibcmd_form_body::FormBodyRevision::V4
        || parsed.trailing_fields != 7
        || parsed.trailing.len() != 7
    {
        return Err(ManagedFormCodecError::InvalidShape(
            "native Form parsers disagree".into(),
        ));
    }
    // The field scanner trims space/tab/CR/LF, but admitted form-feed can
    // remain around the quoted token. Protect those neighboring bytes too.
    let field = &text[range.clone()];
    let start = range.start + field.len() - field.trim_start().len();
    let end = range.end - (field.len() - field.trim_end().len());
    Ok((ManagedFormBody { plain, parsed }, start..end))
}

/// Preserve every original plaintext byte outside the quoted module token.
/// Opaque native sections are copied, never interpreted or rebuilt by this API.
pub fn verify_native_form_module_only_edit(
    old: &[u8],
    new: &[u8],
) -> Result<(), ManagedFormCodecError> {
    let (old, a) = native_module_layout(old)?;
    let (new, b) = native_module_layout(new)?;
    if old.plain[..a.start] != new.plain[..b.start] || old.plain[a.end..] != new.plain[b.end..] {
        return Err(ManagedFormCodecError::InvalidShape(
            "native Form edit changed bytes outside the quoted module".into(),
        ));
    }
    Ok(())
}

/// Transitional reader for historic synthetic fixtures that did not carry a
/// UTF-8 BOM. New profile-selected compilation and decoding remain strict.
#[doc(hidden)]
pub fn decode_compatible_managed_form(
    blob: &[u8],
) -> Result<ManagedFormBody, ManagedFormCodecError> {
    decode(blob, false, false)
}

fn decode_strict(blob: &[u8]) -> Result<ManagedFormBody, ManagedFormCodecError> {
    decode(blob, true, true)
}

fn decode(
    blob: &[u8],
    require_bom: bool,
    require_evidenced_layout: bool,
) -> Result<ManagedFormBody, ManagedFormCodecError> {
    // `inflate` applies the shared compressed/plain payload bounds before the
    // mature Form parser sees the body.
    let plain = inflate(blob)?;
    if require_bom && !plain.starts_with(UTF8_BOM) {
        return Err(ManagedFormCodecError::MissingBom);
    }
    let parsed = parse_form_body_blob(blob)
        .map_err(|error| ManagedFormCodecError::NativeBody(error.to_string()))?;
    if require_evidenced_layout {
        validate_layout(&parsed)?;
    }
    Ok(ManagedFormBody { plain, parsed })
}

fn validate_layout(parsed: &ParsedFormBodyBlob) -> Result<(), ManagedFormCodecError> {
    let marker = parsed
        .layout
        .trim()
        .strip_prefix('{')
        .and_then(|value| value.split([',', '}']).next())
        .map(str::trim);
    if marker != Some(FORM_LAYOUT_MARKER) {
        return Err(ManagedFormCodecError::UnsupportedLayout(
            marker.unwrap_or("<absent>").to_owned(),
        ));
    }
    if parsed.trailing_fields != FORM_TRAILING_SECTIONS
        || parsed.trailing.len() != FORM_TRAILING_SECTIONS
    {
        return Err(ManagedFormCodecError::InvalidShape(format!(
            "expected {FORM_TRAILING_SECTIONS} trailing sections, got {}",
            parsed.trailing_fields
        )));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ManagedFormCodecError {
    Profile(BodyProfileError),
    Native(String),
    Source(String),
    NativeBody(String),
    MissingBom,
    UnsupportedLayout(String),
    InvalidShape(String),
}

impl Display for ManagedFormCodecError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Profile(source) => Display::fmt(source, formatter),
            Self::Native(reason) => write!(formatter, "native Form payload was rejected: {reason}"),
            Self::Source(reason) => write!(formatter, "Form source cannot be compiled: {reason}"),
            Self::NativeBody(reason) => write!(formatter, "native Form body is invalid: {reason}"),
            Self::MissingBom => write!(formatter, "native Form body has no UTF-8 BOM"),
            Self::UnsupportedLayout(marker) => write!(
                formatter,
                "native Form layout marker `{marker}` is outside the evidenced marker-50 cohort"
            ),
            Self::InvalidShape(reason) => {
                write!(formatter, "native Form body is invalid: {reason}")
            }
        }
    }
}

impl Error for ManagedFormCodecError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Profile(source) => Some(source),
            _ => None,
        }
    }
}

impl From<BodyProfileError> for ManagedFormCodecError {
    fn from(source: BodyProfileError) -> Self {
        Self::Profile(source)
    }
}

impl From<NativeError> for ManagedFormCodecError {
    fn from(source: NativeError) -> Self {
        Self::Native(source.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::families::native::deflate_bytes;

    fn native_module_fixture() -> Vec<u8> {
        deflate_bytes(
            "\u{feff}{4,{50,0}, \"old\" ,{4,1,{7,0}},{0,0},{0,0},{0,0},{0,0},0,0}".as_bytes(),
        )
        .unwrap()
    }

    #[test]
    fn native_module_envelope_is_distinct_and_preserves_exact_raw_bytes() {
        let old = native_module_fixture();
        let decoded = decode_native_managed_form_module_layout(&old).unwrap();
        assert_eq!(decoded.module_text(), "old");
        assert_eq!(decoded.parsed().trailing_fields, 7);
        assert!(decode_managed_form(&ManagedFormCodecProfile::fixture(), &old).is_err());
        let changed = ibcmd_metadata_blob::pack_form_body_blob_from_module_text(
            &old,
            "новый \"quoted\"\r\n".as_bytes(),
        )
        .unwrap()
        .blob;
        verify_native_form_module_only_edit(&old, &changed).unwrap();
        assert_eq!(
            decode_native_managed_form_module_layout(&changed)
                .unwrap()
                .module_text(),
            "новый \"quoted\"\r\n"
        );
        let text = String::from_utf8(inflate(&changed).unwrap()).unwrap();
        let (_, token) = native_module_layout(&changed).unwrap();
        let mut before_quote = text.clone();
        before_quote.insert(token.start, ' ');
        let mut after_quote = text.clone();
        after_quote.insert(token.end, '\t');
        for altered in [
            before_quote,
            after_quote,
            text.replacen("{50,0}", "{50,1}", 1),
            text.replacen("{4,1,{7,0}}", "{4,1,{7,1}}", 1),
            text.replacen("{0,0}", "{0,1}", 1),
            format!("{text} "),
        ] {
            let packed = deflate_bytes(altered.as_bytes()).unwrap();
            assert!(decode_native_managed_form_module_layout(&packed).is_ok());
            assert!(verify_native_form_module_only_edit(&old, &packed).is_err());
        }
    }

    #[test]
    fn native_module_token_range_excludes_preserved_ascii_whitespace() {
        let prefix = "\u{feff}{4,{50,0},";
        let suffix = ",{4,1,{7,0}},{0,0},{0,0},{0,0},{0,0},0,0}";
        let module = "новый \"quoted\"\r\n";
        for (before, after) in [(" \t\r\n", ""), ("", "\r\n\t "), (" \t\r\n", "\r\n\t ")] {
            let expected_prefix = format!("{prefix}{before}");
            let expected_suffix = format!("{after}{suffix}");
            let plain = format!("{expected_prefix}\"old\"{expected_suffix}");
            let old = deflate_bytes(plain.as_bytes()).unwrap();
            let (old_body, old_token) = native_module_layout(&old).unwrap();
            // Independent fixture offsets distinguish the quoted token from
            // the containing comma-delimited field and its ASCII whitespace.
            assert_eq!(old_token, expected_prefix.len()..expected_prefix.len() + 5);
            assert_eq!(&old_body.plaintext()[old_token.clone()], b"\"old\"");
            let new =
                ibcmd_metadata_blob::pack_form_body_blob_from_module_text(&old, module.as_bytes())
                    .unwrap()
                    .blob;
            let (new_body, new_token) = native_module_layout(&new).unwrap();
            assert_eq!(new_body.module_text(), module);
            assert_eq!(
                &new_body.plaintext()[new_token.clone()],
                "\"новый \"\"quoted\"\"\r\n\"".as_bytes()
            );
            assert_eq!(
                &new_body.plaintext()[..new_token.start],
                expected_prefix.as_bytes()
            );
            assert_eq!(
                &new_body.plaintext()[new_token.end..],
                expected_suffix.as_bytes()
            );
            assert_eq!(
                &old_body.plaintext()[..old_token.start],
                &new_body.plaintext()[..new_token.start]
            );
            assert_eq!(
                &old_body.plaintext()[old_token.end..],
                &new_body.plaintext()[new_token.end..]
            );
            verify_native_form_module_only_edit(&old, &new).unwrap();

            for boundary in [new_token.start, new_token.end] {
                let mut altered = String::from_utf8(new_body.plaintext().to_vec()).unwrap();
                altered.insert(boundary, '\t');
                let altered = deflate_bytes(altered.as_bytes()).unwrap();
                // Neighbor whitespace remains grammatically valid but is
                // outside the only token the module edit may replace.
                assert!(decode_native_managed_form_module_layout(&altered).is_ok());
                assert_eq!(
                    verify_native_form_module_only_edit(&old, &altered),
                    Err(ManagedFormCodecError::InvalidShape(
                        "native Form edit changed bytes outside the quoted module".into()
                    ))
                );
            }
        }
    }

    #[test]
    fn native_module_ff_range_excludes_admitted_form_feed() {
        let prefix = "\u{feff}{4,{50,0},";
        let suffix = ",{4,1,{7,0}},{0,0},{0,0},{0,0},{0,0},0,0}";
        for (before, after) in [("\x0c", ""), ("", "\x0c"), ("\x0c", "\x0c")] {
            let expected_prefix = format!("{prefix}{before}");
            let expected_suffix = format!("{after}{suffix}");
            let plain = format!("{expected_prefix}\"old\"{expected_suffix}");
            let old = deflate_bytes(plain.as_bytes()).unwrap();
            assert_eq!(
                decode_native_managed_form_module_layout(&old)
                    .unwrap()
                    .module_text(),
                "old"
            );
            let (body, token) = native_module_layout(&old).unwrap();
            assert_eq!(token, expected_prefix.len()..expected_prefix.len() + 5);
            assert_eq!(&body.plaintext()[token.clone()], b"\"old\"");
            assert_eq!(&body.plaintext()[..token.start], expected_prefix.as_bytes());
            assert_eq!(&body.plaintext()[token.end..], expected_suffix.as_bytes());
        }
    }

    #[test]
    fn native_module_ff_neighbor_mutations_and_packer_loss_are_refused() {
        let prefix = "\u{feff}{4,{50,0},";
        let suffix = ",{4,1,{7,0}},{0,0},{0,0},{0,0},{0,0},0,0}";
        for (before, after) in [("\x0c", ""), ("", "\x0c"), ("\x0c", "\x0c")] {
            let expected_prefix = format!("{prefix}{before}");
            let expected_suffix = format!("{after}{suffix}");
            let plain = format!("{expected_prefix}\"old\"{expected_suffix}");
            let old = deflate_bytes(plain.as_bytes()).unwrap();
            assert!(decode_native_managed_form_module_layout(&old).is_ok());
            let preserved = format!("{expected_prefix}\"new\"{expected_suffix}");
            let new = deflate_bytes(preserved.as_bytes()).unwrap();
            verify_native_form_module_only_edit(&old, &new).unwrap();
            for boundary in [expected_prefix.len(), expected_prefix.len() + 5] {
                let mut altered = preserved.clone();
                altered.insert(boundary, '\x0c');
                let altered = deflate_bytes(altered.as_bytes()).unwrap();
                assert!(decode_native_managed_form_module_layout(&altered).is_ok());
                assert!(verify_native_form_module_only_edit(&old, &altered).is_err());
            }
            let packed = ibcmd_metadata_blob::pack_form_body_blob_from_module_text(&old, b"new")
                .unwrap()
                .blob;
            let packed_plain = inflate(&packed).unwrap();
            assert!(!packed_plain.contains(&b'\x0c'));
            assert!(decode_native_managed_form_module_layout(&packed).is_ok());
            assert!(verify_native_form_module_only_edit(&old, &packed).is_err());
        }
    }

    #[test]
    fn native_module_envelope_refuses_unmeasured_shapes_and_partial_input() {
        let old = native_module_fixture();
        let text = String::from_utf8(inflate(&old).unwrap()).unwrap();
        for altered in [
            text.replacen("{4,", "{3,", 1),
            text.replacen("{50,", "{49,", 1),
            text.replacen("{50,", "{42,", 1),
            text.replacen("{4,1,{7,0}}", "0", 1),
            text.replacen(",{0,0},0,0}", ",{0,0},1,0}", 1),
            text.replacen(",{0,0},0,0}", ",{0,0},0,0,0}", 1),
            format!("\u{feff}garbage{}", text.trim_start_matches('\u{feff}')),
            format!("{text}garbage"),
            text[..text.len() - 1].into(),
            text.trim_start_matches('\u{feff}').into(),
        ] {
            assert_ne!(altered, text);
            assert!(
                decode_native_managed_form_module_layout(
                    &deflate_bytes(altered.as_bytes()).unwrap()
                )
                .is_err()
            );
        }
        let abbreviated = compile_managed_form(
            &ManagedFormCodecProfile::fixture(),
            SIMPLE_FORM,
            Some(b"old"),
            None,
        )
        .unwrap();
        assert!(decode_managed_form(&ManagedFormCodecProfile::fixture(), &abbreviated).is_ok());
        assert!(decode_native_managed_form_module_layout(&abbreviated).is_err());
    }

    const SIMPLE_FORM: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:v8="http://v8.1c.ru/8.1/data/core" version="2.20">
	<Title><v8:item><v8:lang>en</v8:lang><v8:content>Standalone</v8:content></v8:item></Title>
	<Width>480</Width>
	<Height>320</Height>
	<AutoTitle>false</AutoTitle>
	<AutoCommandBar name="FormCommandBar" id="-1"/>
	<Commands>
		<Command name="Do" id="1"/>
	</Commands>
	<Attributes>
		<Attribute name="Description" id="2">
			<Type><v8:Type>xs:string</v8:Type></Type>
		</Attribute>
	</Attributes>
	<Parameters>
		<Parameter name="Key"><Type><v8:Type>xs:string</v8:Type></Type></Parameter>
	</Parameters>
	<ChildItems>
		<UsualGroup name="Main" id="10">
			<ChildItems>
				<Button name="Run" id="11"><CommandName>Form.Command.Do</CommandName></Button>
				<InputField name="Description" id="12"><DataPath>Description</DataPath></InputField>
			</ChildItems>
		</UsualGroup>
	</ChildItems>
</Form>"#;

    #[test]
    fn unsupported_source_layout_is_a_hard_blocker() {
        let profile = ManagedFormCodecProfile::fixture();
        let unsupported = br#"<Form xmlns="http://v8.1c.ru/8.3/xcf/logform"><ChildItems><LabelDecoration name="Future" id="1"/></ChildItems></Form>"#;
        let error = compile_managed_form(&profile, unsupported, None, None).unwrap_err();
        assert!(error.to_string().contains("unsupported base-free element"));
    }

    /// Measures — rather than assumes — what the base-free packer can do with a
    /// real platform form.
    ///
    /// The input is the platform's own `Ext/Form.xml` for
    /// `Catalogs/CorpusList/Forms/ListForm`, retained under
    /// `tests/fixtures/native-evidence/8.3.27.2214/dcs-form-list-settings-server-state`.
    /// It sits outside the evidenced base-free cohort, and the exact reasons are
    /// pinned here so the gap list is a checked fact rather than a note: when
    /// the packer grows a facet this test fails, and whoever grew it has to
    /// restate what is still missing.
    #[test]
    fn retained_native_form_is_refused_with_an_exact_gap_list() {
        let form_xml = decode_base64_fixture(include_str!(concat!(
            "../../../../../tests/fixtures/native-evidence/8.3.27.2214/",
            "dcs-form-list-settings-server-state/native-form.xml.b64"
        )));
        let profile = ManagedFormCodecProfile::fixture();
        let error = compile_managed_form(&profile, &form_xml, None, None)
            .expect_err("the retained native form is outside the base-free cohort");
        let rendered = error.to_string();
        assert!(
            rendered.contains("Form XML is outside the evidenced base-free marker-50 cohort"),
            "{rendered}"
        );
        for blocker in [
            "has a dynamic-list MainTable but no source resolver",
            "uses an unsupported decoration/font/border creation facet",
            "uses strict-schema properties without an evidenced creation layout",
        ] {
            assert!(
                rendered.contains(blocker),
                "missing `{blocker}` in {rendered}"
            );
        }
    }

    /// Minimal decoder for the retained base64 evidence files.
    fn decode_base64_fixture(input: &str) -> Vec<u8> {
        fn value(byte: u8) -> u32 {
            match byte {
                b'A'..=b'Z' => u32::from(byte - b'A'),
                b'a'..=b'z' => u32::from(byte - b'a') + 26,
                b'0'..=b'9' => u32::from(byte - b'0') + 52,
                b'+' => 62,
                b'/' => 63,
                _ => panic!("invalid base64 byte {byte}"),
            }
        }
        let bytes = input
            .as_bytes()
            .iter()
            .copied()
            .filter(|byte| !byte.is_ascii_whitespace())
            .collect::<Vec<_>>();
        assert!(
            bytes.len().is_multiple_of(4),
            "base64 length is not canonical"
        );
        let mut output = Vec::with_capacity(bytes.len() / 4 * 3);
        for chunk in bytes.chunks_exact(4) {
            let padding = chunk.iter().filter(|byte| **byte == b'=').count();
            let packed = (value(chunk[0]) << 18)
                | (value(chunk[1]) << 12)
                | (if padding > 1 { 0 } else { value(chunk[2]) } << 6)
                | (if padding > 0 { 0 } else { value(chunk[3]) });
            output.push((packed >> 16) as u8);
            if padding < 2 {
                output.push((packed >> 8) as u8);
            }
            if padding < 1 {
                output.push(packed as u8);
            }
        }
        output
    }

    #[test]
    fn original_layout_and_module_edit_witnesses_preserve_raw_bytes() {
        let profile = ManagedFormCodecProfile::fixture();
        let old = compile_managed_form(&profile, SIMPLE_FORM, Some(b"old"), None).unwrap();
        assert_eq!(
            decode_managed_form_layout(&old).unwrap(),
            decode_managed_form(&profile, &old).unwrap()
        );
        assert_eq!(
            validate_original_managed_form_layout(&old).unwrap(),
            decode_managed_form_layout(&old).unwrap()
        );
        let new = ibcmd_metadata_blob::pack_form_body_blob_from_module_text(&old, b"new\"text")
            .unwrap()
            .blob;
        verify_module_only_edit(&old, &new).unwrap();
        let (body, range) = original_layout(&new).unwrap();
        let text = std::str::from_utf8(body.plaintext()).unwrap();
        // Semantic IR equality must not hide whitespace edits outside the token.
        let mut trailing_space = text.to_owned();
        trailing_space.insert(text.rfind("{0}").unwrap(), ' ');
        for changed in [
            format!("{} {}", &text[..range.start], &text[range.start..]),
            text.replacen(",{50,", ", {50,", 1),
            trailing_space,
            format!("{} ", text),
        ] {
            let blob = deflate_bytes(changed.as_bytes()).unwrap();
            assert_eq!(
                decode_managed_form_layout(&blob).unwrap().parsed(),
                body.parsed()
            );
            assert!(verify_module_only_edit(&old, &blob).is_err());
        }
        let changed = text.replacen("{50,", "{59,", 1);
        assert!(
            verify_module_only_edit(&old, &deflate_bytes(changed.as_bytes()).unwrap()).is_err()
        );
    }

    #[test]
    fn module_packer_preserves_whitespace_around_quoted_token() {
        let profile = ManagedFormCodecProfile::fixture();
        let base = compile_managed_form(&profile, SIMPLE_FORM, Some(b"old"), None).unwrap();
        let (body, range) = original_layout(&base).unwrap();
        let text = std::str::from_utf8(body.plaintext()).unwrap();
        for (before, after) in [(" \t", ""), ("", "\r\n "), (" \t", "\r\n ")] {
            let spaced = format!(
                "{}{before}{}{after}{}",
                &text[..range.start],
                &text[range.clone()],
                &text[range.end..]
            );
            let old = deflate_bytes(spaced.as_bytes()).unwrap();
            let new = ibcmd_metadata_blob::pack_form_body_blob_from_module_text(&old, b"new\"text")
                .unwrap()
                .blob;
            verify_module_only_edit(&old, &new).unwrap();
            let (old_body, a) = original_layout(&old).unwrap();
            let (new_body, b) = original_layout(&new).unwrap();
            assert_eq!(
                &old_body.plaintext()[..a.start],
                &new_body.plaintext()[..b.start]
            );
            assert_eq!(
                &old_body.plaintext()[a.end..],
                &new_body.plaintext()[b.end..]
            );
            assert_eq!(new_body.module_text(), "new\"text");
            assert_eq!(old_body.parsed().layout, new_body.parsed().layout);
            assert_eq!(old_body.parsed().trailing, new_body.parsed().trailing);
        }
    }

    #[test]
    fn original_layout_refuses_root_42_before_reader_upgrade() {
        let plain = concat!(
            "\u{feff}{3,{42,0,0,0,0,1,0,0,00000000-0000-0000-0000-000000000000,1,{1,0},0,0,1,1,1,0,0,0,",
            "{1,9f2e5ddb-3492-4f5d-8f0d-416b8d1d5c5b,\"ПриСозданииНаСервере\",0,0},{0},1,",
            "{21,{-1,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,1,{0,{0,{\"B\",1},0}},9,\"ФормаКоманднаяПанель\",",
            "{1,0},{1,0},0,1,0,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{0,0,1},0,1,0,0,0,3,3},",
            "0,\"\",\"\",1,{21,{0},0,0,0,7,\"Navigator\",{1,0},{1,0},0,1,0,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{0,0,1},0,1,0,0,0,3,3},",
            "1,\"\",0,0,0,0,0,0,3,3,0,0,0},\"\",{0,0},{0,0},{0,0},{0,0},0,0}"
        );
        let blob = deflate_bytes(plain.as_bytes()).unwrap();
        let parsed = ibcmd_metadata_blob::parse_form_body_blob(&blob).unwrap();
        assert!(parsed.layout.starts_with("{50,"));
        assert!(matches!(validate_original_managed_form_layout(&blob),
            Err(ManagedFormCodecError::UnsupportedLayout(marker)) if marker == "42"));
    }

    #[test]
    fn strict_reader_rejects_no_bom_and_legacy_layout() {
        let profile = ManagedFormCodecProfile::fixture();
        let no_bom = deflate_bytes(br#"{4,{50,0},"",{0},{0},{0},{0}}"#).unwrap();
        assert!(matches!(
            decode_managed_form(&profile, &no_bom),
            Err(ManagedFormCodecError::MissingBom)
        ));

        let legacy = deflate_bytes(b"\xef\xbb\xbf{4,{59,0},\"\",{0},{0},{0},{0}}").unwrap();
        assert!(matches!(
            decode_managed_form(&profile, &legacy),
            Err(ManagedFormCodecError::UnsupportedLayout(marker)) if marker == "59"
        ));
    }
}
