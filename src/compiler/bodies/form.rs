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
use crate::module_blob::{
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

    #[cfg(test)]
    fn fixture() -> Self {
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
    decode_strict(blob)
}

/// Transitional reader for historic synthetic fixtures that did not carry a
/// UTF-8 BOM. New profile-selected compilation and decoding remain strict.
pub(crate) fn decode_compatible_managed_form(
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
    use std::collections::BTreeMap;

    use super::*;
    use crate::compiler::families::native::deflate_bytes;

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
    fn marker_50_form_compiles_deterministically_without_base_blob() {
        let profile = ManagedFormCodecProfile::fixture();
        let module = b"&AtClient\r\nProcedure Run(Command)\r\nEndProcedure";
        let first = compile_managed_form(&profile, SIMPLE_FORM, Some(module), None).unwrap();
        let second = compile_managed_form(&profile, SIMPLE_FORM, Some(module), None).unwrap();
        assert_eq!(first, second);

        let decoded = decode_managed_form(&profile, &first).unwrap();
        assert_eq!(decoded.module_text(), std::str::from_utf8(module).unwrap());
        assert!(decoded.plaintext().starts_with(UTF8_BOM));
        assert_eq!(decoded.parsed().trailing_fields, FORM_TRAILING_SECTIONS);
        assert!(decoded.parsed().layout.starts_with("{50,"));
        assert!(decoded.parsed().layout.contains("\"Main\""));
        assert!(decoded.parsed().layout.contains("\"Run\""));

        let extracted = crate::mssql_dump::extract_form_body_xml(&first, &BTreeMap::new())
            .expect("marker-50 body must remain readable by the export adapter");
        for expected in [
            "<Width>480</Width>",
            "<Height>320</Height>",
            "<Command name=\"Do\" id=\"1\">",
            "<UsualGroup name=\"Main\" id=\"10\">",
            "<Button name=\"Run\" id=\"11\">",
            "<CommandName>Form.Command.Do</CommandName>",
            "<Attribute name=\"Description\" id=\"2\">",
            "<Parameter name=\"Key\">",
            "<InputField name=\"Description\" id=\"12\">",
            "<DataPath>Description</DataPath>",
        ] {
            assert!(extracted.contains(expected), "{expected}: {extracted}");
        }
    }

    #[test]
    fn unsupported_source_layout_is_a_hard_blocker() {
        let profile = ManagedFormCodecProfile::fixture();
        let unsupported = br#"<Form xmlns="http://v8.1c.ru/8.3/xcf/logform"><ChildItems><LabelDecoration name="Future" id="1"/></ChildItems></Form>"#;
        let error = compile_managed_form(&profile, unsupported, None, None).unwrap_err();
        assert!(error.to_string().contains("unsupported base-free element"));
    }

    #[test]
    fn representative_container_field_table_and_addition_matrix_roundtrips() {
        let profile = ManagedFormCodecProfile::fixture();
        let xml = br#"<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<ChildItems>
		<CommandBar name="Actions" id="1"><ChildItems><Button name="Action" id="2"/></ChildItems></CommandBar>
		<Pages name="Tabs" id="3"><ChildItems><Page name="General" id="4"><ChildItems>
			<CheckBoxField name="Enabled" id="5"/>
			<TextDocumentField name="Notes" id="6"/>
			<PictureDecoration name="Logo" id="7"/>
		</ChildItems></Page></ChildItems></Pages>
		<Table name="Rows" id="8"><ChildItems>
			<LabelField name="RowLabel" id="9"/>
			<InputField name="RowValue" id="10"/>
		</ChildItems></Table>
		<SearchStringAddition name="Search" id="11"/>
		<ViewStatusAddition name="Status" id="12"/>
		<SearchControlAddition name="SearchControl" id="13"/>
	</ChildItems>
</Form>"#;

        let blob = compile_managed_form(&profile, xml, None, None).unwrap();
        let extracted = crate::mssql_dump::extract_form_body_xml(&blob, &BTreeMap::new())
            .expect("representative marker-50 matrix must be exportable");
        for tag in [
            "CommandBar",
            "Button",
            "Pages",
            "Page",
            "CheckBoxField",
            "TextDocumentField",
            "PictureDecoration",
            "Table",
            "LabelField",
            "InputField",
            "SearchStringAddition",
            "ViewStatusAddition",
            "SearchControlAddition",
        ] {
            assert!(
                extracted.contains(&format!("<{tag} ")),
                "missing {tag}: {extracted}"
            );
        }
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
