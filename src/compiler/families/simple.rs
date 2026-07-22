//! Base-free native codecs for compact metadata families.
//!
//! The first implemented vertical slice is `Language`.  Other BOOT-003
//! families remain explicit profile-selection failures until their complete
//! native layouts and required UUID/reference inputs are represented.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::{self, Display, Formatter, Write as _};
use std::io::{self, Read, Write};

use flate2::Compression;
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use ibcmd_core::artifact::{ProfileId, StorageProfileId};
use ibcmd_core::identity::ObjectUuid;
use ibcmd_core::model::CanonicalObject;
use ibcmd_core::profile::EffectiveProfile;
use ibcmd_core::storage::{
    MultipartIdentity, StorageBuildError, StoragePatchBuildError, StoragePatchEntry,
    StoragePatchOutcome, StoragePatchTarget, StorageProvenance,
};
use ibcmd_core::validate::ValidatedConfiguration;
use ibcmd_core::value::{
    CanonicalField, CanonicalValue, CanonicalValueKind, MAX_CANONICAL_COLLECTION_ITEMS,
    MAX_CANONICAL_RETAINED_BYTES, MAX_CANONICAL_TEXT_BYTES,
};
use ibcmd_core::version::PlatformBuild;

use super::super::CompileAxes;
use super::super::graph::BootstrapGraph;

const LANGUAGE_LAYOUT_KEY: &str = "bootstrap.metadata.language.layout";
const LANGUAGE_LAYOUT: &str = "language-v1-crlf-no-bom";
const SUPPORTED_STORAGE_PROFILE: &str = "storage:mssql-config-configsave";
const NIL_UUID: &str = "00000000-0000-0000-0000-000000000000";

const MAX_LANGUAGE_CODE_BYTES: usize = 256;
const MAX_LANGUAGE_PLAIN_BYTES: usize = MAX_CANONICAL_RETAINED_BYTES + 4 * 1_048_576;
const MAX_NATIVE_DEPTH: usize = 8;
const MAX_NATIVE_NODES: usize = 100_000;

/// BOOT-003 metadata families.  Each family selects its own layout constant,
/// allowing a future platform profile to evolve one codec without reopening
/// unrelated families.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SimpleFamily {
    Constant,
    Language,
    SessionParameter,
    DefinedType,
    FunctionalOption,
    FunctionalOptionsParameter,
}

impl SimpleFamily {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Constant => "Constant",
            Self::Language => "Language",
            Self::SessionParameter => "SessionParameter",
            Self::DefinedType => "DefinedType",
            Self::FunctionalOption => "FunctionalOption",
            Self::FunctionalOptionsParameter => "FunctionalOptionsParameter",
        }
    }

    fn from_kind(kind: &str) -> Option<Self> {
        match kind {
            "Constant" => Some(Self::Constant),
            "Language" => Some(Self::Language),
            "SessionParameter" => Some(Self::SessionParameter),
            "DefinedType" => Some(Self::DefinedType),
            "FunctionalOption" => Some(Self::FunctionalOption),
            "FunctionalOptionsParameter" => Some(Self::FunctionalOptionsParameter),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SimpleLayout {
    LanguageV1,
}

/// Exact independent target coordinates and one family-specific layout.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimpleMetadataProfile {
    profile_id: ProfileId,
    platform_build: PlatformBuild,
    storage_profile: StorageProfileId,
    family: SimpleFamily,
    layout: SimpleLayout,
}

impl SimpleMetadataProfile {
    /// Selects one family without deriving platform or storage coordinates.
    pub fn from_effective_for_family(
        profile: &EffectiveProfile,
        family: SimpleFamily,
    ) -> Result<Self, SimpleMetadataProfileError> {
        let platform_build = profile
            .platform_build
            .as_ref()
            .map(|value| value.value.clone())
            .ok_or_else(|| SimpleMetadataProfileError::MissingCoordinate {
                profile: profile.id.clone(),
                coordinate: "platform_build",
            })?;
        let storage_profile = profile
            .storage_profile
            .as_ref()
            .map(|value| value.value.clone())
            .ok_or_else(|| SimpleMetadataProfileError::MissingCoordinate {
                profile: profile.id.clone(),
                coordinate: "storage_profile",
            })?;
        if storage_profile.as_str() != SUPPORTED_STORAGE_PROFILE {
            return Err(SimpleMetadataProfileError::UnsupportedCoordinate {
                profile: profile.id.clone(),
                coordinate: "storage_profile",
                value: storage_profile.to_string(),
            });
        }

        let (key, expected, layout) = match family {
            SimpleFamily::Language => (
                LANGUAGE_LAYOUT_KEY,
                LANGUAGE_LAYOUT,
                SimpleLayout::LanguageV1,
            ),
            _ => {
                return Err(SimpleMetadataProfileError::FamilyNotImplemented {
                    profile: profile.id.clone(),
                    family,
                });
            }
        };
        let value = profile.constants.get(key).ok_or_else(|| {
            SimpleMetadataProfileError::MissingConstant {
                profile: profile.id.clone(),
                key,
            }
        })?;
        if value.value != expected {
            return Err(SimpleMetadataProfileError::UnsupportedLayout {
                profile: profile.id.clone(),
                family,
                key,
                value: value.value.clone(),
            });
        }
        Ok(Self {
            profile_id: profile.id.clone(),
            platform_build,
            storage_profile,
            family,
            layout,
        })
    }

    pub const fn profile_id(&self) -> &ProfileId {
        &self.profile_id
    }

    pub const fn family(&self) -> SimpleFamily {
        self.family
    }

    #[cfg(test)]
    fn language_fixture(profile_id: &str) -> Self {
        Self {
            profile_id: ProfileId::parse(profile_id).unwrap(),
            platform_build: PlatformBuild::parse("8.3.27.1989").unwrap(),
            storage_profile: StorageProfileId::parse(SUPPORTED_STORAGE_PROFILE).unwrap(),
            family: SimpleFamily::Language,
            layout: SimpleLayout::LanguageV1,
        }
    }
}

/// Failure to select a family-specific layout from a target profile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SimpleMetadataProfileError {
    MissingCoordinate {
        profile: ProfileId,
        coordinate: &'static str,
    },
    MissingConstant {
        profile: ProfileId,
        key: &'static str,
    },
    UnsupportedCoordinate {
        profile: ProfileId,
        coordinate: &'static str,
        value: String,
    },
    UnsupportedLayout {
        profile: ProfileId,
        family: SimpleFamily,
        key: &'static str,
        value: String,
    },
    FamilyNotImplemented {
        profile: ProfileId,
        family: SimpleFamily,
    },
}

impl Display for SimpleMetadataProfileError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCoordinate {
                profile,
                coordinate,
            } => write!(
                formatter,
                "profile `{profile}` has no independent `{coordinate}` coordinate"
            ),
            Self::MissingConstant { profile, key } => {
                write!(
                    formatter,
                    "profile `{profile}` has no required `{key}` constant"
                )
            }
            Self::UnsupportedCoordinate {
                profile,
                coordinate,
                value,
            } => write!(
                formatter,
                "profile `{profile}` declares unsupported `{coordinate}` value `{value}`"
            ),
            Self::UnsupportedLayout {
                profile,
                family,
                key,
                value,
            } => write!(
                formatter,
                "profile `{profile}` declares unsupported {} layout `{key}={value}`",
                family.as_str()
            ),
            Self::FamilyNotImplemented { profile, family } => write!(
                formatter,
                "profile `{profile}` cannot select {} because its base-free codec is not implemented",
                family.as_str()
            ),
        }
    }
}

impl Error for SimpleMetadataProfileError {}

/// One native localized string in storage order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeLocalizedString {
    pub language: String,
    pub content: String,
}

/// Complete base-free native IR for a `Language` row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LanguageNativeIr {
    pub uuid: ObjectUuid,
    pub name: String,
    pub synonyms: Vec<NativeLocalizedString>,
    pub comment: String,
    pub language_code: String,
}

impl LanguageNativeIr {
    /// Renders a minimal standalone XCF document for a caller-selected dialect.
    pub fn to_xml(&self, profile: &ProfileId) -> Result<Vec<u8>, SimpleMetadataBuildError> {
        let version = xml_profile_version(profile)
            .ok_or_else(|| SimpleMetadataBuildError::InvalidXmlProfile(profile.clone()))?;
        let mut xml = String::new();
        xml.push('\u{feff}');
        xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n");
        write!(
            &mut xml,
            "<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" version=\"{version}\">\r\n\t<Language uuid=\"{}\">\r\n\t\t<Properties>\r\n",
            self.uuid
        )
        .expect("writing to String cannot fail");
        write_xml_text_element(&mut xml, "\t\t\t", "Name", &self.name);
        if self.synonyms.is_empty() {
            xml.push_str("\t\t\t<Synonym/>\r\n");
        } else {
            xml.push_str("\t\t\t<Synonym>\r\n");
            for synonym in &self.synonyms {
                xml.push_str("\t\t\t\t<v8:item>\r\n");
                write_xml_text_element(&mut xml, "\t\t\t\t\t", "v8:lang", &synonym.language);
                write_xml_text_element(&mut xml, "\t\t\t\t\t", "v8:content", &synonym.content);
                xml.push_str("\t\t\t\t</v8:item>\r\n");
            }
            xml.push_str("\t\t\t</Synonym>\r\n");
        }
        write_xml_text_element(&mut xml, "\t\t\t", "Comment", &self.comment);
        write_xml_text_element(&mut xml, "\t\t\t", "LanguageCode", &self.language_code);
        xml.push_str("\t\t</Properties>\r\n\t</Language>\r\n</MetaDataObject>");
        Ok(xml.into_bytes())
    }
}

/// Failure to compile or decode one compact native metadata row.
#[derive(Debug)]
pub enum SimpleMetadataBuildError {
    Profile(SimpleMetadataProfileError),
    ProfileMismatch {
        graph: ProfileId,
        simple: ProfileId,
    },
    AxisMismatch {
        axis: &'static str,
        expected: String,
        actual: String,
    },
    UnknownObject(ObjectUuid),
    MissingPrimaryRoute(ObjectUuid),
    UnsupportedFamily(SimpleFamily),
    InvalidModel {
        object: ObjectUuid,
        reason: &'static str,
    },
    InvalidXmlProfile(ProfileId),
    Native(String),
    PlainPayloadTooLarge {
        maximum: usize,
        actual: usize,
    },
    Deflate(io::Error),
    Inflate(io::Error),
    Storage(StorageBuildError),
    Patch(StoragePatchBuildError),
}

impl Display for SimpleMetadataBuildError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Profile(source) => {
                write!(formatter, "unsupported simple metadata profile: {source}")
            }
            Self::ProfileMismatch { graph, simple } => write!(
                formatter,
                "bootstrap graph profile `{graph}` differs from simple metadata profile `{simple}`"
            ),
            Self::AxisMismatch {
                axis,
                expected,
                actual,
            } => write!(
                formatter,
                "simple metadata `{axis}` axis mismatch: expected `{expected}`, got `{actual}`"
            ),
            Self::UnknownObject(uuid) => write!(formatter, "validated graph has no object {uuid}"),
            Self::MissingPrimaryRoute(uuid) => {
                write!(
                    formatter,
                    "bootstrap graph has no primary row for object {uuid}"
                )
            }
            Self::UnsupportedFamily(family) => {
                write!(
                    formatter,
                    "{} has no base-free simple codec",
                    family.as_str()
                )
            }
            Self::InvalidModel { object, reason } => {
                write!(
                    formatter,
                    "object {object} is not a compilable Language: {reason}"
                )
            }
            Self::InvalidXmlProfile(profile) => {
                write!(formatter, "unsupported Language XML profile `{profile}`")
            }
            Self::Native(reason) => write!(formatter, "invalid native Language row: {reason}"),
            Self::PlainPayloadTooLarge { maximum, actual } => write!(
                formatter,
                "native Language plaintext has {actual} bytes, exceeding the {maximum}-byte bound"
            ),
            Self::Deflate(source) => {
                write!(formatter, "failed to raw-deflate Language row: {source}")
            }
            Self::Inflate(source) => write!(formatter, "failed to inflate Language row: {source}"),
            Self::Storage(source) => write!(formatter, "invalid Language storage target: {source}"),
            Self::Patch(source) => write!(formatter, "invalid Language storage payload: {source}"),
        }
    }
}

impl Error for SimpleMetadataBuildError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Profile(source) => Some(source),
            Self::Deflate(source) | Self::Inflate(source) => Some(source),
            Self::Storage(source) => Some(source),
            Self::Patch(source) => Some(source),
            _ => None,
        }
    }
}

impl From<SimpleMetadataProfileError> for SimpleMetadataBuildError {
    fn from(source: SimpleMetadataProfileError) -> Self {
        Self::Profile(source)
    }
}

impl From<StorageBuildError> for SimpleMetadataBuildError {
    fn from(source: StorageBuildError) -> Self {
        Self::Storage(source)
    }
}

impl From<StoragePatchBuildError> for SimpleMetadataBuildError {
    fn from(source: StoragePatchBuildError) -> Self {
        Self::Patch(source)
    }
}

/// Compiles one validated compact metadata object into its exact primary row.
pub fn compile_simple_metadata(
    validated: &ValidatedConfiguration<'_>,
    graph: &BootstrapGraph,
    object_uuid: ObjectUuid,
    axes: &CompileAxes,
    profile: &SimpleMetadataProfile,
) -> Result<StoragePatchEntry, SimpleMetadataBuildError> {
    validate_coordinates(graph, axes, profile)?;
    let object_index = validated
        .graph()
        .object_index_by_uuid(object_uuid)
        .ok_or(SimpleMetadataBuildError::UnknownObject(object_uuid))?;
    let object = &validated.configuration().objects()[object_index];
    let family = SimpleFamily::from_kind(object.kind().as_str()).ok_or(
        SimpleMetadataBuildError::InvalidModel {
            object: object_uuid,
            reason: "metadata kind is outside BOOT-003",
        },
    )?;
    if family != profile.family {
        return Err(SimpleMetadataBuildError::AxisMismatch {
            axis: "family",
            expected: profile.family.as_str().to_owned(),
            actual: family.as_str().to_owned(),
        });
    }
    let expected_source_profile = format!("xml-{}", axes.xml_dialect());
    if object.provenance().source_profile().as_str() != expected_source_profile {
        return Err(SimpleMetadataBuildError::AxisMismatch {
            axis: "xml_dialect",
            expected: object.provenance().source_profile().to_string(),
            actual: axes.xml_dialect().to_string(),
        });
    }
    let route = graph
        .primary_object_entry(object_uuid)
        .ok_or(SimpleMetadataBuildError::MissingPrimaryRoute(object_uuid))?;

    let plaintext = match (family, profile.layout) {
        (SimpleFamily::Language, SimpleLayout::LanguageV1) => {
            let projection = project_language(validated, object)?;
            serialize_language(&projection)
        }
        (family, _) => return Err(SimpleMetadataBuildError::UnsupportedFamily(family)),
    };
    let bytes = raw_deflate(&plaintext)?;
    let provenance = StorageProvenance::new(&format!(
        "bootstrap:{}:metadata:{}",
        profile.profile_id,
        family.as_str()
    ))?;
    Ok(StoragePatchEntry::new(
        StoragePatchTarget::new(route.key().clone(), MultipartIdentity::single(), provenance),
        StoragePatchOutcome::compiled(bytes)?,
    ))
}

/// Strictly decodes a raw-DEFLATE `Language` primary row into native IR.
pub fn decode_language_blob(
    blob: &[u8],
    profile: &SimpleMetadataProfile,
) -> Result<LanguageNativeIr, SimpleMetadataBuildError> {
    if profile.family != SimpleFamily::Language || profile.layout != SimpleLayout::LanguageV1 {
        return Err(SimpleMetadataBuildError::UnsupportedFamily(profile.family));
    }
    let plain = inflate_bounded(blob)?;
    parse_language(&plain)
}

fn validate_coordinates(
    graph: &BootstrapGraph,
    axes: &CompileAxes,
    profile: &SimpleMetadataProfile,
) -> Result<(), SimpleMetadataBuildError> {
    if graph.profile_id() != profile.profile_id() {
        return Err(SimpleMetadataBuildError::ProfileMismatch {
            graph: graph.profile_id().clone(),
            simple: profile.profile_id().clone(),
        });
    }
    let actual_platform = axes
        .platform_build()
        .map(ToString::to_string)
        .unwrap_or_else(|| "<missing>".to_owned());
    if axes.platform_build() != Some(&profile.platform_build) {
        return Err(SimpleMetadataBuildError::AxisMismatch {
            axis: "platform_build",
            expected: profile.platform_build.to_string(),
            actual: actual_platform,
        });
    }
    if axes.storage_profile() != &profile.storage_profile {
        return Err(SimpleMetadataBuildError::AxisMismatch {
            axis: "storage_profile",
            expected: profile.storage_profile.to_string(),
            actual: axes.storage_profile().to_string(),
        });
    }
    if axes.compatibility_mode().is_some() {
        return Err(SimpleMetadataBuildError::AxisMismatch {
            axis: "compatibility_mode",
            expected: "<unspecified for evidenced layout>".to_owned(),
            actual: axes.compatibility_mode().unwrap().to_string(),
        });
    }
    if axes.container_revision().is_some() {
        return Err(SimpleMetadataBuildError::AxisMismatch {
            axis: "container_revision",
            expected: "<unspecified for evidenced layout>".to_owned(),
            actual: axes.container_revision().unwrap().to_string(),
        });
    }
    if !matches!(axes.xml_dialect().to_string().as_str(), "2.20" | "2.21") {
        return Err(SimpleMetadataBuildError::AxisMismatch {
            axis: "xml_dialect",
            expected: "2.20 or 2.21".to_owned(),
            actual: axes.xml_dialect().to_string(),
        });
    }
    Ok(())
}

fn project_language(
    validated: &ValidatedConfiguration<'_>,
    object: &CanonicalObject,
) -> Result<LanguageNativeIr, SimpleMetadataBuildError> {
    let uuid = object.identity().uuid();
    match object.provenance().source_profile().as_str() {
        "xml-2.20" | "xml-2.21" => {}
        _ => {
            return Err(SimpleMetadataBuildError::InvalidModel {
                object: uuid,
                reason: "source profile is not xml-2.20 or xml-2.21",
            });
        }
    }
    if object.owner().is_some() {
        return invalid_model(uuid, "Language must be top-level");
    }
    if !object.references().is_empty()
        || !object.generated_types().is_empty()
        || !object.assets().is_empty()
    {
        return invalid_model(
            uuid,
            "Language cannot own references, generated types, or assets",
        );
    }
    if validated
        .configuration()
        .objects()
        .iter()
        .any(|candidate| candidate.owner() == Some(uuid))
    {
        return invalid_model(uuid, "Language cannot own child objects");
    }
    let expected = ["Name", "Synonym", "Comment", "LanguageCode"];
    if object.properties().len() != expected.len()
        || object
            .properties()
            .iter()
            .zip(expected)
            .any(|(field, expected)| field.name().as_str() != expected)
    {
        return invalid_model(uuid, "typed property schema is not exact");
    }
    let name = text_property(object, "Name")?.to_owned();
    if name.is_empty() {
        return invalid_model(uuid, "Name must not be empty");
    }
    let comment = text_property(object, "Comment")?.to_owned();
    let language_code = text_property(object, "LanguageCode")?.to_owned();
    if language_code.is_empty() || language_code.len() > MAX_LANGUAGE_CODE_BYTES {
        return invalid_model(uuid, "LanguageCode is empty or exceeds its bound");
    }
    let synonyms = synonym_property(object, "Synonym")?;
    Ok(LanguageNativeIr {
        uuid,
        name,
        synonyms,
        comment,
        language_code,
    })
}

fn invalid_model<T>(
    object: ObjectUuid,
    reason: &'static str,
) -> Result<T, SimpleMetadataBuildError> {
    Err(SimpleMetadataBuildError::InvalidModel { object, reason })
}

fn property<'a>(
    object: &'a CanonicalObject,
    name: &str,
) -> Result<&'a CanonicalValue, SimpleMetadataBuildError> {
    object
        .properties()
        .iter()
        .find(|field| field.name().as_str() == name)
        .map(CanonicalField::value)
        .ok_or(SimpleMetadataBuildError::InvalidModel {
            object: object.identity().uuid(),
            reason: "required typed property is missing",
        })
}

fn text_property<'a>(
    object: &'a CanonicalObject,
    name: &str,
) -> Result<&'a str, SimpleMetadataBuildError> {
    match property(object, name)?.kind() {
        CanonicalValueKind::Text(value) => Ok(value.as_str()),
        _ => invalid_model(object.identity().uuid(), "typed property is not text"),
    }
}

fn synonym_property(
    object: &CanonicalObject,
    name: &str,
) -> Result<Vec<NativeLocalizedString>, SimpleMetadataBuildError> {
    let values =
        property(object, name)?
            .as_sequence()
            .ok_or(SimpleMetadataBuildError::InvalidModel {
                object: object.identity().uuid(),
                reason: "Synonym is not a sequence",
            })?;
    let mut languages = BTreeSet::new();
    let mut result = Vec::with_capacity(values.len());
    for value in values {
        let fields = value
            .as_record()
            .ok_or(SimpleMetadataBuildError::InvalidModel {
                object: object.identity().uuid(),
                reason: "Synonym item is not a record",
            })?;
        if fields.len() != 2
            || fields[0].name().as_str() != "lang"
            || fields[1].name().as_str() != "content"
        {
            return invalid_model(object.identity().uuid(), "Synonym item schema is not exact");
        }
        let language = canonical_text(fields[0].value(), object.identity().uuid())?.to_owned();
        let content = canonical_text(fields[1].value(), object.identity().uuid())?.to_owned();
        if language.is_empty() || language.len() > MAX_LANGUAGE_CODE_BYTES {
            return invalid_model(
                object.identity().uuid(),
                "Synonym language is empty or exceeds its bound",
            );
        }
        if !languages.insert(language.clone()) {
            return invalid_model(object.identity().uuid(), "duplicate Synonym language");
        }
        result.push(NativeLocalizedString { language, content });
    }
    Ok(result)
}

fn canonical_text(
    value: &CanonicalValue,
    object: ObjectUuid,
) -> Result<&str, SimpleMetadataBuildError> {
    match value.kind() {
        CanonicalValueKind::Text(value) => Ok(value.as_str()),
        _ => invalid_model(object, "Synonym field is not text"),
    }
}

fn serialize_language(value: &LanguageNativeIr) -> Vec<u8> {
    let mut plaintext = String::new();
    plaintext.push_str("{1,\r\n{0,\r\n{3,\r\n{1,0,");
    plaintext.push_str(&value.uuid.to_string());
    plaintext.push_str("},");
    push_1c_string(&mut plaintext, &value.name);
    plaintext.push(',');
    write!(&mut plaintext, "{{{}", value.synonyms.len()).expect("writing to String cannot fail");
    for synonym in &value.synonyms {
        plaintext.push(',');
        push_1c_string(&mut plaintext, &synonym.language);
        plaintext.push(',');
        push_1c_string(&mut plaintext, &synonym.content);
    }
    plaintext.push_str("},");
    push_1c_string(&mut plaintext, &value.comment);
    plaintext.push_str(",0,0,");
    plaintext.push_str(NIL_UUID);
    plaintext.push_str(",0},");
    push_1c_string(&mut plaintext, &value.language_code);
    plaintext.push_str("},0}");
    plaintext.into_bytes()
}

fn push_1c_string(output: &mut String, value: &str) {
    output.push('"');
    for character in value.chars() {
        if character == '"' {
            output.push('"');
        }
        output.push(character);
    }
    output.push('"');
}

fn raw_deflate(plaintext: &[u8]) -> Result<Vec<u8>, SimpleMetadataBuildError> {
    if plaintext.len() > MAX_LANGUAGE_PLAIN_BYTES {
        return Err(SimpleMetadataBuildError::PlainPayloadTooLarge {
            maximum: MAX_LANGUAGE_PLAIN_BYTES,
            actual: plaintext.len(),
        });
    }
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::best());
    encoder
        .write_all(plaintext)
        .map_err(SimpleMetadataBuildError::Deflate)?;
    encoder.finish().map_err(SimpleMetadataBuildError::Deflate)
}

fn inflate_bounded(blob: &[u8]) -> Result<Vec<u8>, SimpleMetadataBuildError> {
    let limit = MAX_LANGUAGE_PLAIN_BYTES
        .checked_add(1)
        .expect("Language plaintext bound is below usize::MAX");
    let mut decoder = DeflateDecoder::new(blob).take(limit as u64);
    let mut plain = Vec::new();
    decoder
        .read_to_end(&mut plain)
        .map_err(SimpleMetadataBuildError::Inflate)?;
    if plain.len() > MAX_LANGUAGE_PLAIN_BYTES {
        return Err(SimpleMetadataBuildError::PlainPayloadTooLarge {
            maximum: MAX_LANGUAGE_PLAIN_BYTES,
            actual: plain.len(),
        });
    }
    Ok(plain)
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum NativeValue {
    Token(String),
    Text(String),
    List(Vec<NativeValue>),
}

struct NativeParser<'a> {
    input: &'a [u8],
    offset: usize,
    nodes: usize,
}

impl<'a> NativeParser<'a> {
    fn new(input: &'a [u8]) -> Self {
        Self {
            input,
            offset: 0,
            nodes: 0,
        }
    }

    fn parse(mut self) -> Result<NativeValue, SimpleMetadataBuildError> {
        if self.input.starts_with(b"\xef\xbb\xbf") {
            return Err(native("unexpected BOM for language-v1-crlf-no-bom"));
        }
        let value = self.value(0)?;
        self.whitespace();
        if self.offset != self.input.len() {
            return Err(native("trailing bytes after root value"));
        }
        Ok(value)
    }

    fn value(&mut self, depth: usize) -> Result<NativeValue, SimpleMetadataBuildError> {
        if depth > MAX_NATIVE_DEPTH {
            return Err(native("native value exceeds nesting bound"));
        }
        self.nodes = self
            .nodes
            .checked_add(1)
            .ok_or_else(|| native("native node count overflow"))?;
        if self.nodes > MAX_NATIVE_NODES {
            return Err(native("native value exceeds node bound"));
        }
        self.whitespace();
        match self.input.get(self.offset) {
            Some(b'{') => self.list(depth),
            Some(b'"') => self.text(),
            Some(_) => self.token(),
            None => Err(native("unexpected end of input")),
        }
    }

    fn list(&mut self, depth: usize) -> Result<NativeValue, SimpleMetadataBuildError> {
        self.offset += 1;
        self.whitespace();
        let mut values = Vec::new();
        if self.input.get(self.offset) == Some(&b'}') {
            self.offset += 1;
            return Ok(NativeValue::List(values));
        }
        loop {
            values.push(self.value(depth + 1)?);
            self.whitespace();
            match self.input.get(self.offset) {
                Some(b',') => {
                    self.offset += 1;
                    self.whitespace();
                    if self.input.get(self.offset) == Some(&b'}') {
                        return Err(native("trailing comma in native list"));
                    }
                }
                Some(b'}') => {
                    self.offset += 1;
                    return Ok(NativeValue::List(values));
                }
                _ => return Err(native("expected comma or closing brace")),
            }
        }
    }

    fn text(&mut self) -> Result<NativeValue, SimpleMetadataBuildError> {
        self.offset += 1;
        let mut output = Vec::new();
        while let Some(byte) = self.input.get(self.offset).copied() {
            if byte == b'"' {
                if self.input.get(self.offset + 1) == Some(&b'"') {
                    output.push(b'"');
                    self.offset += 2;
                } else {
                    self.offset += 1;
                    return String::from_utf8(output)
                        .map(NativeValue::Text)
                        .map_err(|_| native("quoted field is not UTF-8"));
                }
            } else {
                output.push(byte);
                self.offset += 1;
            }
        }
        Err(native("unterminated quoted field"))
    }

    fn token(&mut self) -> Result<NativeValue, SimpleMetadataBuildError> {
        let start = self.offset;
        while let Some(byte) = self.input.get(self.offset) {
            if matches!(byte, b',' | b'}') {
                break;
            }
            self.offset += 1;
        }
        let token = std::str::from_utf8(&self.input[start..self.offset])
            .map_err(|_| native("token is not UTF-8"))?
            .trim();
        if token.is_empty() {
            return Err(native("empty native token"));
        }
        Ok(NativeValue::Token(token.to_owned()))
    }

    fn whitespace(&mut self) {
        while self
            .input
            .get(self.offset)
            .is_some_and(|byte| byte.is_ascii_whitespace())
        {
            self.offset += 1;
        }
    }
}

fn parse_language(plain: &[u8]) -> Result<LanguageNativeIr, SimpleMetadataBuildError> {
    let root = NativeParser::new(plain).parse()?;
    let root = exact_list(&root, 3, "root")?;
    exact_token(&root[0], "1", "root discriminator")?;
    exact_token(&root[2], "0", "root tail")?;
    let object = exact_list(&root[1], 3, "Language object")?;
    exact_token(&object[0], "0", "Language discriminator")?;
    let header = exact_list(&object[1], 9, "metadata header")?;
    exact_token(&header[0], "3", "metadata header discriminator")?;
    let identity = exact_list(&header[1], 3, "metadata identity")?;
    exact_token(&identity[0], "1", "identity discriminator")?;
    exact_token(&identity[1], "0", "identity tail")?;
    let uuid_text = token(&identity[2], "object UUID")?;
    let uuid = ObjectUuid::parse(uuid_text).map_err(|_| native("invalid object UUID"))?;
    if uuid.to_string() != uuid_text {
        return Err(native("object UUID is not canonical lowercase text"));
    }
    let name = text(&header[2], "Name")?.to_owned();
    let synonyms = parse_synonyms(&header[3])?;
    let comment = text(&header[4], "Comment")?.to_owned();
    exact_token(&header[5], "0", "header flag 1")?;
    exact_token(&header[6], "0", "header flag 2")?;
    exact_token(&header[7], NIL_UUID, "header nil UUID")?;
    exact_token(&header[8], "0", "header tail")?;
    let language_code = text(&object[2], "LanguageCode")?.to_owned();
    validate_native_text(&name, "Name")?;
    validate_native_text(&comment, "Comment")?;
    validate_native_text(&language_code, "LanguageCode")?;
    if name.is_empty() {
        return Err(native("Name must not be empty"));
    }
    if language_code.is_empty() || language_code.len() > MAX_LANGUAGE_CODE_BYTES {
        return Err(native("LanguageCode is empty or exceeds its bound"));
    }
    Ok(LanguageNativeIr {
        uuid,
        name,
        synonyms,
        comment,
        language_code,
    })
}

fn parse_synonyms(
    value: &NativeValue,
) -> Result<Vec<NativeLocalizedString>, SimpleMetadataBuildError> {
    let fields = list(value, "Synonym")?;
    let count_text = fields
        .first()
        .ok_or_else(|| native("Synonym count is missing"))
        .and_then(|value| token(value, "Synonym count"))?;
    let count = count_text
        .parse::<usize>()
        .ok()
        .filter(|count| count.to_string() == count_text)
        .ok_or_else(|| native("Synonym count is not canonical decimal"))?;
    if count > MAX_CANONICAL_COLLECTION_ITEMS {
        return Err(native("Synonym count exceeds canonical bound"));
    }
    let expected = count
        .checked_mul(2)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| native("Synonym field count overflow"))?;
    if fields.len() != expected {
        return Err(native("Synonym count does not match fields"));
    }
    let mut languages = BTreeSet::new();
    let mut result = Vec::with_capacity(count);
    for pair in fields[1..].chunks_exact(2) {
        let language = text(&pair[0], "Synonym language")?.to_owned();
        let content = text(&pair[1], "Synonym content")?.to_owned();
        validate_native_text(&language, "Synonym language")?;
        validate_native_text(&content, "Synonym content")?;
        if language.is_empty() || language.len() > MAX_LANGUAGE_CODE_BYTES {
            return Err(native("Synonym language is empty or exceeds its bound"));
        }
        if !languages.insert(language.clone()) {
            return Err(native("duplicate Synonym language"));
        }
        result.push(NativeLocalizedString { language, content });
    }
    Ok(result)
}

fn validate_native_text(value: &str, field: &'static str) -> Result<(), SimpleMetadataBuildError> {
    if value.len() > MAX_CANONICAL_TEXT_BYTES {
        Err(native(&format!("{field} exceeds canonical text bound")))
    } else {
        Ok(())
    }
}

fn exact_list<'a>(
    value: &'a NativeValue,
    length: usize,
    field: &'static str,
) -> Result<&'a [NativeValue], SimpleMetadataBuildError> {
    let values = list(value, field)?;
    if values.len() == length {
        Ok(values)
    } else {
        Err(native(&format!("{field} has unexpected field count")))
    }
}

fn list<'a>(
    value: &'a NativeValue,
    field: &'static str,
) -> Result<&'a [NativeValue], SimpleMetadataBuildError> {
    match value {
        NativeValue::List(values) => Ok(values),
        _ => Err(native(&format!("{field} is not a list"))),
    }
}

fn exact_token(
    value: &NativeValue,
    expected: &str,
    field: &'static str,
) -> Result<(), SimpleMetadataBuildError> {
    if token(value, field)? == expected {
        Ok(())
    } else {
        Err(native(&format!("{field} has an unsupported value")))
    }
}

fn token<'a>(
    value: &'a NativeValue,
    field: &'static str,
) -> Result<&'a str, SimpleMetadataBuildError> {
    match value {
        NativeValue::Token(value) => Ok(value),
        _ => Err(native(&format!("{field} is not a token"))),
    }
}

fn text<'a>(
    value: &'a NativeValue,
    field: &'static str,
) -> Result<&'a str, SimpleMetadataBuildError> {
    match value {
        NativeValue::Text(value) => Ok(value),
        _ => Err(native(&format!("{field} is not quoted text"))),
    }
}

fn native(reason: &str) -> SimpleMetadataBuildError {
    SimpleMetadataBuildError::Native(reason.to_owned())
}

fn xml_profile_version(profile: &ProfileId) -> Option<&'static str> {
    match profile.as_str() {
        "xml-2.20" => Some("2.20"),
        "xml-2.21" => Some("2.21"),
        _ => None,
    }
}

fn write_xml_text_element(output: &mut String, indent: &str, name: &str, value: &str) {
    output.push_str(indent);
    output.push('<');
    output.push_str(name);
    if value.is_empty() {
        output.push_str("/>\r\n");
        return;
    }
    output.push('>');
    push_xml_text(output, value);
    output.push_str("</");
    output.push_str(name);
    output.push_str(">\r\n");
}

fn push_xml_text(output: &mut String, value: &str) {
    for character in value.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            _ => output.push(character),
        }
    }
}

#[cfg(test)]
mod tests {
    use ibcmd_core::artifact::ProfileId;
    use ibcmd_core::diagnostic::{ObjectPath, PathSegment, PropertyPath};
    use ibcmd_core::family::FamilyId;
    use ibcmd_core::identity::LogicalIdentity;
    use ibcmd_core::model::{
        CanonicalConfiguration, CanonicalObject, CanonicalObjectParts, MetadataKind,
    };
    use ibcmd_core::profile::{ProfileSourceKind, parse_profile_source, resolve_profiles};
    use ibcmd_core::provenance::{CanonicalAnchor, SourceProvenance};
    use ibcmd_core::storage::StoragePatchOutcome;
    use ibcmd_core::validate::validate_configuration;
    use ibcmd_core::version::XmlDialect;
    use ibcmd_xml::{XmlReader, bundled_metadata_registry};

    use crate::compiler::graph::{ObjectStorageRoute, build_bootstrap_graph};
    use crate::compiler::identity::collect_bootstrap_identities;

    use super::*;

    const UUID: &str = "11111111-1111-4111-8111-111111111111";
    const CONFIGURATION_UUID: &str = "22222222-2222-4222-8222-222222222222";

    fn xml(version: &str) -> Vec<u8> {
        format!(
            "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" version=\"{version}\">\r\n\
\t<Language uuid=\"{UUID}\">\r\n\
\t\t<Properties>\r\n\
\t\t\t<Name>English &amp; More</Name>\r\n\
\t\t\t<Synonym><v8:item><v8:lang>en</v8:lang><v8:content>English \"main\"</v8:content></v8:item></Synonym>\r\n\
\t\t\t<Comment>Primary</Comment>\r\n\
\t\t\t<LanguageCode>en</LanguageCode>\r\n\
\t\t</Properties>\r\n\
\t</Language>\r\n\
</MetaDataObject>"
        )
        .into_bytes()
    }

    fn decoded(version: &str) -> CanonicalConfiguration {
        let document = XmlReader::from_slice(&xml(version)).unwrap();
        let envelope = bundled_metadata_registry()
            .decode(
                &FamilyId::parse("Language").unwrap(),
                &document,
                ProfileId::parse(&format!("xml-{version}")).unwrap(),
                ObjectPath::root(),
            )
            .unwrap();
        let path = ObjectPath::new(vec![PathSegment::name("configuration").unwrap()]).unwrap();
        let provenance = SourceProvenance::new(
            ProfileId::parse(&format!("xml-{version}")).unwrap(),
            CanonicalAnchor::new(path.clone(), PropertyPath::root()),
        );
        let configuration = CanonicalObject::new(CanonicalObjectParts::new(
            LogicalIdentity::new(ObjectUuid::parse(CONFIGURATION_UUID).unwrap(), path),
            MetadataKind::new("Configuration").unwrap(),
            provenance,
        ))
        .unwrap();
        CanonicalConfiguration::new(vec![configuration, envelope.root().clone()]).unwrap()
    }

    fn axes(version: &str) -> CompileAxes {
        CompileAxes::new(
            XmlDialect::parse(version).unwrap(),
            Some(PlatformBuild::parse("8.3.27.1989").unwrap()),
            None,
            StorageProfileId::parse(SUPPORTED_STORAGE_PROFILE).unwrap(),
            None,
        )
    }

    fn graph_and_profile<'a>(
        validated: &ValidatedConfiguration<'a>,
    ) -> (BootstrapGraph, SimpleMetadataProfile) {
        let identities = collect_bootstrap_identities(validated).unwrap();
        let uuid = ObjectUuid::parse(UUID).unwrap();
        let configuration_uuid = ObjectUuid::parse(CONFIGURATION_UUID).unwrap();
        let graph = build_bootstrap_graph(
            &identities,
            ProfileId::parse("platform-test").unwrap(),
            vec![
                ObjectStorageRoute::new(configuration_uuid, Vec::new()).unwrap(),
                ObjectStorageRoute::new(uuid, Vec::new()).unwrap(),
            ],
        )
        .unwrap();
        (
            graph,
            SimpleMetadataProfile::language_fixture("platform-test"),
        )
    }

    #[test]
    fn language_xml_to_blob_to_ir_to_xml_is_base_free_for_both_dialects() {
        for version in ["2.20", "2.21"] {
            let configuration = decoded(version);
            let validated = validate_configuration(&configuration).unwrap();
            let (graph, profile) = graph_and_profile(&validated);
            let uuid = ObjectUuid::parse(UUID).unwrap();
            let first = compile_simple_metadata(&validated, &graph, uuid, &axes(version), &profile)
                .unwrap();
            let second =
                compile_simple_metadata(&validated, &graph, uuid, &axes(version), &profile)
                    .unwrap();
            assert_eq!(first, second);
            assert_eq!(first.target().key().as_str(), UUID);
            let payload = first.outcome().compiled_payload().unwrap();
            let ir = decode_language_blob(payload.bytes(), &profile).unwrap();
            assert_eq!(ir.uuid, uuid);
            assert_eq!(ir.name, "English & More");
            assert_eq!(ir.synonyms[0].content, "English \"main\"");
            let roundtrip_xml = ir
                .to_xml(&ProfileId::parse(&format!("xml-{version}")).unwrap())
                .unwrap();
            let roundtrip = XmlReader::from_slice(&roundtrip_xml).unwrap();
            let envelope = bundled_metadata_registry()
                .decode(
                    &FamilyId::parse("Language").unwrap(),
                    &roundtrip,
                    ProfileId::parse(&format!("xml-{version}")).unwrap(),
                    ObjectPath::root(),
                )
                .unwrap();
            assert_eq!(envelope.root().identity().uuid(), uuid);
        }
    }

    #[test]
    fn plaintext_matches_evidenced_golden_and_escapes_quotes() {
        let configuration = decoded("2.20");
        let validated = validate_configuration(&configuration).unwrap();
        let (graph, profile) = graph_and_profile(&validated);
        let entry = compile_simple_metadata(
            &validated,
            &graph,
            ObjectUuid::parse(UUID).unwrap(),
            &axes("2.20"),
            &profile,
        )
        .unwrap();
        let plain = inflate_bounded(entry.outcome().compiled_payload().unwrap().bytes()).unwrap();
        assert_eq!(
            plain,
            format!(
                "{{1,\r\n{{0,\r\n{{3,\r\n{{1,0,{UUID}}},\"English & More\",{{1,\"en\",\"English \"\"main\"\"\"}},\"Primary\",0,0,{NIL_UUID},0}},\"en\"}},0}}"
            )
            .as_bytes()
        );
    }

    #[test]
    fn profile_selection_is_family_local_and_requires_explicit_axes() {
        let json = format!(
            r#"{{
                "schema_version": 1,
                "id": "platform-test",
                "status": "experimental",
                "platform_build": "8.3.27.1989",
                "storage_profile": "{SUPPORTED_STORAGE_PROFILE}",
                "constants": {{"{LANGUAGE_LAYOUT_KEY}": "{LANGUAGE_LAYOUT}"}}
            }}"#
        );
        let source =
            parse_profile_source("simple.json", ProfileSourceKind::Bundled, &json).unwrap();
        let registry = resolve_profiles([source]).unwrap();
        let effective = registry
            .get(&ProfileId::parse("platform-test").unwrap())
            .unwrap();
        assert_eq!(
            SimpleMetadataProfile::from_effective_for_family(effective, SimpleFamily::Language)
                .unwrap()
                .family(),
            SimpleFamily::Language
        );
        assert!(matches!(
            SimpleMetadataProfile::from_effective_for_family(effective, SimpleFamily::Constant),
            Err(SimpleMetadataProfileError::FamilyNotImplemented {
                family: SimpleFamily::Constant,
                ..
            })
        ));
    }

    #[test]
    fn wrong_graph_profile_and_future_layout_fail_closed() {
        let configuration = decoded("2.20");
        let validated = validate_configuration(&configuration).unwrap();
        let (graph, _) = graph_and_profile(&validated);
        let profile = SimpleMetadataProfile::language_fixture("platform-other");
        assert!(matches!(
            compile_simple_metadata(
                &validated,
                &graph,
                ObjectUuid::parse(UUID).unwrap(),
                &axes("2.20"),
                &profile
            ),
            Err(SimpleMetadataBuildError::ProfileMismatch { .. })
        ));
    }

    #[test]
    fn malformed_native_layout_is_rejected_instead_of_guessed() {
        let profile = SimpleMetadataProfile::language_fixture("platform-test");
        let malformed = raw_deflate(
            format!(
                "{{1,{{0,{{3,{{1,0,{UUID}}},\"English\",{{0}},\"\",0,0,{NIL_UUID},0}},\"en\",\"future\"}},0}}"
            )
            .as_bytes(),
        )
        .unwrap();
        assert!(matches!(
            decode_language_blob(&malformed, &profile),
            Err(SimpleMetadataBuildError::Native(_))
        ));
    }

    #[test]
    fn coarse_prepacked_outcome_is_not_used() {
        let configuration = decoded("2.20");
        let validated = validate_configuration(&configuration).unwrap();
        let (graph, profile) = graph_and_profile(&validated);
        let entry = compile_simple_metadata(
            &validated,
            &graph,
            ObjectUuid::parse(UUID).unwrap(),
            &axes("2.20"),
            &profile,
        )
        .unwrap();
        assert!(matches!(entry.outcome(), StoragePatchOutcome::Compiled(_)));
        assert!(entry.target().provenance().as_str().contains(":Language"));
    }
}
