//! Base-free native codecs for compact metadata families.
//!
//! Implemented vertical slices are `Language`, `SessionParameter`,
//! `FunctionalOption`, and `FunctionalOptionsParameter`. Other BOOT-003
//! families remain explicit profile-selection failures until their complete
//! native layouts and required UUID/type inputs are represented.

use std::collections::{BTreeMap, BTreeSet};
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
const FUNCTIONAL_OPTION_LAYOUT_KEY: &str = "bootstrap.metadata.functional_option.layout";
const FUNCTIONAL_OPTION_LAYOUT: &str = "functional-option-v1-crlf-no-bom";
const FUNCTIONAL_OPTIONS_PARAMETER_LAYOUT_KEY: &str =
    "bootstrap.metadata.functional_options_parameter.layout";
const FUNCTIONAL_OPTIONS_PARAMETER_LAYOUT: &str = "functional-options-parameter-v1-crlf-no-bom";
const SESSION_PARAMETER_LAYOUT_KEY: &str = "bootstrap.metadata.session_parameter.layout";
const SESSION_PARAMETER_LAYOUT: &str = "session-parameter-v1-crlf-no-bom";
const SUPPORTED_STORAGE_PROFILE: &str = "storage:mssql-config-configsave";
const NIL_UUID: &str = "00000000-0000-0000-0000-000000000000";
const DESIGN_TIME_REFERENCE_CLASS_UUID: &str = "157fa490-4ce9-11d4-9415-008048da11f9";
const FUNCTIONAL_OPTION_CONTENT_CLASS_UUID: &str = "3ea29ea5-66f6-4e3b-8595-d8940db766a2";
const FIXED_ARRAY_TYPE_UUID: &str = "4500381b-db30-4a10-9db4-990038032acf";
const FIXED_MAP_TYPE_UUID: &str = "220455ea-6c85-4513-996f-bbe79ed07774";
const FIXED_STRUCTURE_TYPE_UUID: &str = "3ee983d7-ace7-40f9-bb7e-2e916fcddd56";
const UUID_TYPE_UUID: &str = "fc01b5df-97fe-449b-83d4-218a090e681e";
const VALUE_STORAGE_TYPE_UUID: &str = "e199ca70-93cf-46ce-a54b-6edc88c3a296";

const MAX_LANGUAGE_CODE_BYTES: usize = 256;
const MAX_SIMPLE_METADATA_PLAIN_BYTES: usize = MAX_CANONICAL_RETAINED_BYTES + 4 * 1_048_576;
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
    SessionParameterV1,
    FunctionalOptionV1,
    FunctionalOptionsParameterV1,
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
            SimpleFamily::SessionParameter => (
                SESSION_PARAMETER_LAYOUT_KEY,
                SESSION_PARAMETER_LAYOUT,
                SimpleLayout::SessionParameterV1,
            ),
            SimpleFamily::FunctionalOption => (
                FUNCTIONAL_OPTION_LAYOUT_KEY,
                FUNCTIONAL_OPTION_LAYOUT,
                SimpleLayout::FunctionalOptionV1,
            ),
            SimpleFamily::FunctionalOptionsParameter => (
                FUNCTIONAL_OPTIONS_PARAMETER_LAYOUT_KEY,
                FUNCTIONAL_OPTIONS_PARAMETER_LAYOUT,
                SimpleLayout::FunctionalOptionsParameterV1,
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

    #[cfg(test)]
    fn functional_options_parameter_fixture(profile_id: &str) -> Self {
        Self {
            profile_id: ProfileId::parse(profile_id).unwrap(),
            platform_build: PlatformBuild::parse("8.3.27.1989").unwrap(),
            storage_profile: StorageProfileId::parse(SUPPORTED_STORAGE_PROFILE).unwrap(),
            family: SimpleFamily::FunctionalOptionsParameter,
            layout: SimpleLayout::FunctionalOptionsParameterV1,
        }
    }

    #[cfg(test)]
    fn functional_option_fixture(profile_id: &str) -> Self {
        Self {
            profile_id: ProfileId::parse(profile_id).unwrap(),
            platform_build: PlatformBuild::parse("8.3.27.1989").unwrap(),
            storage_profile: StorageProfileId::parse(SUPPORTED_STORAGE_PROFILE).unwrap(),
            family: SimpleFamily::FunctionalOption,
            layout: SimpleLayout::FunctionalOptionV1,
        }
    }

    #[cfg(test)]
    fn session_parameter_fixture(profile_id: &str) -> Self {
        Self {
            profile_id: ProfileId::parse(profile_id).unwrap(),
            platform_build: PlatformBuild::parse("8.3.27.1989").unwrap(),
            storage_profile: StorageProfileId::parse(SUPPORTED_STORAGE_PROFILE).unwrap(),
            family: SimpleFamily::SessionParameter,
            layout: SimpleLayout::SessionParameterV1,
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

/// Complete base-free native IR for a `FunctionalOption` row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionalOptionNativeIr {
    pub uuid: ObjectUuid,
    pub name: String,
    pub synonyms: Vec<NativeLocalizedString>,
    pub comment: String,
    pub location: Option<ObjectUuid>,
    pub privileged_get_mode: bool,
    pub content: Vec<ObjectUuid>,
}

/// Complete base-free native IR for a `FunctionalOptionsParameter` row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionalOptionsParameterNativeIr {
    pub uuid: ObjectUuid,
    pub name: String,
    pub synonyms: Vec<NativeLocalizedString>,
    pub comment: String,
    pub uses: Vec<ObjectUuid>,
}

/// One exact native `SessionParameter` type-pattern item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionParameterTypeNativeIr {
    Boolean,
    String {
        length: u32,
    },
    Number {
        digits: u32,
        fraction_digits: u32,
        nonnegative: bool,
    },
    DateTime,
    Reference(ObjectUuid),
}

/// Complete base-free native IR for a `SessionParameter` row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionParameterNativeIr {
    pub uuid: ObjectUuid,
    pub name: String,
    pub synonyms: Vec<NativeLocalizedString>,
    pub comment: String,
    pub types: Vec<SessionParameterTypeNativeIr>,
}

impl FunctionalOptionNativeIr {
    /// Renders XCF using caller-supplied readable names for every native UUID.
    pub fn to_xml(
        &self,
        profile: &ProfileId,
        references: &BTreeMap<ObjectUuid, String>,
    ) -> Result<Vec<u8>, SimpleMetadataBuildError> {
        let version = xml_profile_version(profile)
            .ok_or_else(|| SimpleMetadataBuildError::InvalidXmlProfile(profile.clone()))?;
        let location = self
            .location
            .map(|uuid| {
                references
                    .get(&uuid)
                    .cloned()
                    .ok_or(SimpleMetadataBuildError::MissingReadableReference(uuid))
            })
            .transpose()?;
        let mut content = Vec::with_capacity(self.content.len());
        for uuid in &self.content {
            content.push(
                references
                    .get(uuid)
                    .cloned()
                    .ok_or(SimpleMetadataBuildError::MissingReadableReference(*uuid))?,
            );
        }
        let mut xml = String::new();
        xml.push('\u{feff}');
        xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n");
        write!(
            &mut xml,
            "<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" version=\"{version}\">\r\n\t<FunctionalOption uuid=\"{}\">\r\n\t\t<Properties>\r\n",
            self.uuid
        )
        .expect("writing to String cannot fail");
        write_xml_text_element(&mut xml, "\t\t\t", "Name", &self.name);
        write_synonym_xml(&mut xml, &self.synonyms);
        write_xml_text_element(&mut xml, "\t\t\t", "Comment", &self.comment);
        if let Some(location) = location {
            write_xml_text_element(&mut xml, "\t\t\t", "Location", &location);
        }
        xml.push_str("\t\t\t<PrivilegedGetMode>");
        xml.push_str(if self.privileged_get_mode {
            "true"
        } else {
            "false"
        });
        xml.push_str("</PrivilegedGetMode>\r\n");
        if content.is_empty() {
            xml.push_str("\t\t\t<Content/>\r\n");
        } else {
            xml.push_str("\t\t\t<Content>\r\n");
            for reference in content {
                xml.push_str("\t\t\t\t<xr:Object>");
                push_xml_text(&mut xml, &reference);
                xml.push_str("</xr:Object>\r\n");
            }
            xml.push_str("\t\t\t</Content>\r\n");
        }
        xml.push_str("\t\t</Properties>\r\n\t</FunctionalOption>\r\n</MetaDataObject>");
        Ok(xml.into_bytes())
    }
}

impl FunctionalOptionsParameterNativeIr {
    /// Renders XCF using caller-supplied readable names for every native UUID.
    pub fn to_xml(
        &self,
        profile: &ProfileId,
        references: &BTreeMap<ObjectUuid, String>,
    ) -> Result<Vec<u8>, SimpleMetadataBuildError> {
        let version = xml_profile_version(profile)
            .ok_or_else(|| SimpleMetadataBuildError::InvalidXmlProfile(profile.clone()))?;
        let mut resolved = Vec::with_capacity(self.uses.len());
        for uuid in &self.uses {
            resolved.push(
                references
                    .get(uuid)
                    .cloned()
                    .ok_or(SimpleMetadataBuildError::MissingReadableReference(*uuid))?,
            );
        }
        let mut xml = String::new();
        xml.push('\u{feff}');
        xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n");
        write!(
            &mut xml,
            "<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"{version}\">\r\n\t<FunctionalOptionsParameter uuid=\"{}\">\r\n\t\t<Properties>\r\n",
            self.uuid
        )
        .expect("writing to String cannot fail");
        write_xml_text_element(&mut xml, "\t\t\t", "Name", &self.name);
        write_synonym_xml(&mut xml, &self.synonyms);
        write_xml_text_element(&mut xml, "\t\t\t", "Comment", &self.comment);
        if resolved.is_empty() {
            xml.push_str("\t\t\t<Use/>\r\n");
        } else {
            xml.push_str("\t\t\t<Use>\r\n");
            for reference in resolved {
                xml.push_str("\t\t\t\t<xr:Item xsi:type=\"xr:MDObjectRef\">");
                push_xml_text(&mut xml, &reference);
                xml.push_str("</xr:Item>\r\n");
            }
            xml.push_str("\t\t\t</Use>\r\n");
        }
        xml.push_str("\t\t</Properties>\r\n\t</FunctionalOptionsParameter>\r\n</MetaDataObject>");
        Ok(xml.into_bytes())
    }
}

impl SessionParameterNativeIr {
    /// Renders XCF, resolving non-built-in native TypeIds through the
    /// caller-supplied `TypeId -> cfg:*` map.
    pub fn to_xml(
        &self,
        profile: &ProfileId,
        type_names: &BTreeMap<ObjectUuid, String>,
    ) -> Result<Vec<u8>, SimpleMetadataBuildError> {
        let version = xml_profile_version(profile)
            .ok_or_else(|| SimpleMetadataBuildError::InvalidXmlProfile(profile.clone()))?;
        if self.types.is_empty() {
            return Err(native("SessionParameter Type pattern is empty"));
        }

        let mut scalar_names = Vec::with_capacity(self.types.len());
        let mut unique_names = BTreeSet::new();
        let mut string_qualifiers = None;
        let mut number_qualifiers = None;
        let mut date_qualifiers = false;
        for item in &self.types {
            let name = match item {
                SessionParameterTypeNativeIr::Boolean => "xs:boolean".to_owned(),
                SessionParameterTypeNativeIr::String { length } => {
                    if string_qualifiers.replace(*length).is_some() {
                        return Err(native("duplicate String type in SessionParameter pattern"));
                    }
                    "xs:string".to_owned()
                }
                SessionParameterTypeNativeIr::Number {
                    digits,
                    fraction_digits,
                    nonnegative,
                } => {
                    if fraction_digits > digits {
                        return Err(native("Number FractionDigits exceeds Digits"));
                    }
                    if number_qualifiers
                        .replace((*digits, *fraction_digits, *nonnegative))
                        .is_some()
                    {
                        return Err(native("duplicate Number type in SessionParameter pattern"));
                    }
                    "xs:decimal".to_owned()
                }
                SessionParameterTypeNativeIr::DateTime => {
                    if date_qualifiers {
                        return Err(native(
                            "duplicate DateTime type in SessionParameter pattern",
                        ));
                    }
                    date_qualifiers = true;
                    "xs:dateTime".to_owned()
                }
                SessionParameterTypeNativeIr::Reference(type_id) => {
                    if let Some(name) = builtin_type_name(*type_id) {
                        name.to_owned()
                    } else {
                        let name = type_names
                            .get(type_id)
                            .ok_or(SimpleMetadataBuildError::MissingReadableReference(*type_id))?;
                        if !supported_cfg_type_name(name) {
                            return Err(native(
                                "readable generated type is not an exact cfg:* name",
                            ));
                        }
                        name.clone()
                    }
                }
            };
            if !unique_names.insert(name.clone()) {
                return Err(native("duplicate SessionParameter Type scalar"));
            }
            scalar_names.push(name);
        }

        let mut xml = String::new();
        xml.push('\u{feff}');
        xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n");
        write!(
            &mut xml,
            "<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:cfg=\"http://v8.1c.ru/8.1/data/enterprise/current-config\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" version=\"{version}\">\r\n\t<SessionParameter uuid=\"{}\">\r\n\t\t<Properties>\r\n",
            self.uuid
        )
        .expect("writing to String cannot fail");
        write_xml_text_element(&mut xml, "\t\t\t", "Name", &self.name);
        write_synonym_xml(&mut xml, &self.synonyms);
        write_xml_text_element(&mut xml, "\t\t\t", "Comment", &self.comment);
        xml.push_str("\t\t\t<Type>\r\n");
        for name in scalar_names {
            write_xml_text_element(&mut xml, "\t\t\t\t", "v8:Type", &name);
        }
        if let Some(length) = string_qualifiers {
            xml.push_str("\t\t\t\t<v8:StringQualifiers>\r\n");
            write_xml_text_element(&mut xml, "\t\t\t\t\t", "v8:Length", &length.to_string());
            write_xml_text_element(&mut xml, "\t\t\t\t\t", "v8:AllowedLength", "Variable");
            xml.push_str("\t\t\t\t</v8:StringQualifiers>\r\n");
        }
        if let Some((digits, fraction_digits, nonnegative)) = number_qualifiers {
            xml.push_str("\t\t\t\t<v8:NumberQualifiers>\r\n");
            write_xml_text_element(&mut xml, "\t\t\t\t\t", "v8:Digits", &digits.to_string());
            write_xml_text_element(
                &mut xml,
                "\t\t\t\t\t",
                "v8:FractionDigits",
                &fraction_digits.to_string(),
            );
            write_xml_text_element(
                &mut xml,
                "\t\t\t\t\t",
                "v8:AllowedSign",
                if nonnegative { "Nonnegative" } else { "Any" },
            );
            xml.push_str("\t\t\t\t</v8:NumberQualifiers>\r\n");
        }
        if date_qualifiers {
            xml.push_str("\t\t\t\t<v8:DateQualifiers>\r\n");
            write_xml_text_element(&mut xml, "\t\t\t\t\t", "v8:DateFractions", "DateTime");
            xml.push_str("\t\t\t\t</v8:DateQualifiers>\r\n");
        }
        xml.push_str(
            "\t\t\t</Type>\r\n\t\t</Properties>\r\n\t</SessionParameter>\r\n</MetaDataObject>",
        );
        Ok(xml.into_bytes())
    }
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
    MissingReadableReference(ObjectUuid),
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
                    "object {object} is not compilable simple metadata: {reason}"
                )
            }
            Self::InvalidXmlProfile(profile) => {
                write!(
                    formatter,
                    "unsupported simple metadata XML profile `{profile}`"
                )
            }
            Self::MissingReadableReference(uuid) => write!(
                formatter,
                "no readable XCF reference was supplied for native object UUID {uuid}"
            ),
            Self::Native(reason) => {
                write!(formatter, "invalid native simple metadata row: {reason}")
            }
            Self::PlainPayloadTooLarge { maximum, actual } => write!(
                formatter,
                "native simple metadata plaintext has {actual} bytes, exceeding the {maximum}-byte bound"
            ),
            Self::Deflate(source) => {
                write!(
                    formatter,
                    "failed to raw-deflate simple metadata row: {source}"
                )
            }
            Self::Inflate(source) => {
                write!(formatter, "failed to inflate simple metadata row: {source}")
            }
            Self::Storage(source) => {
                write!(
                    formatter,
                    "invalid simple metadata storage target: {source}"
                )
            }
            Self::Patch(source) => {
                write!(
                    formatter,
                    "invalid simple metadata storage payload: {source}"
                )
            }
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
        (SimpleFamily::SessionParameter, SimpleLayout::SessionParameterV1) => {
            let projection = project_session_parameter(validated, object)?;
            serialize_session_parameter(&projection)
        }
        (SimpleFamily::FunctionalOption, SimpleLayout::FunctionalOptionV1) => {
            let projection = project_functional_option(validated, object)?;
            serialize_functional_option(&projection)
        }
        (SimpleFamily::FunctionalOptionsParameter, SimpleLayout::FunctionalOptionsParameterV1) => {
            let projection = project_functional_options_parameter(validated, object)?;
            serialize_functional_options_parameter(&projection)
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

/// Strictly decodes a raw-DEFLATE `SessionParameter` primary row.
pub fn decode_session_parameter_blob(
    blob: &[u8],
    profile: &SimpleMetadataProfile,
) -> Result<SessionParameterNativeIr, SimpleMetadataBuildError> {
    if profile.family != SimpleFamily::SessionParameter
        || profile.layout != SimpleLayout::SessionParameterV1
    {
        return Err(SimpleMetadataBuildError::UnsupportedFamily(profile.family));
    }
    let plain = inflate_bounded(blob)?;
    parse_session_parameter(&plain)
}

/// Strictly decodes a raw-DEFLATE `FunctionalOption` primary row.
pub fn decode_functional_option_blob(
    blob: &[u8],
    profile: &SimpleMetadataProfile,
) -> Result<FunctionalOptionNativeIr, SimpleMetadataBuildError> {
    if profile.family != SimpleFamily::FunctionalOption
        || profile.layout != SimpleLayout::FunctionalOptionV1
    {
        return Err(SimpleMetadataBuildError::UnsupportedFamily(profile.family));
    }
    let plain = inflate_bounded(blob)?;
    parse_functional_option(&plain)
}

/// Strictly decodes a raw-DEFLATE `FunctionalOptionsParameter` primary row.
pub fn decode_functional_options_parameter_blob(
    blob: &[u8],
    profile: &SimpleMetadataProfile,
) -> Result<FunctionalOptionsParameterNativeIr, SimpleMetadataBuildError> {
    if profile.family != SimpleFamily::FunctionalOptionsParameter
        || profile.layout != SimpleLayout::FunctionalOptionsParameterV1
    {
        return Err(SimpleMetadataBuildError::UnsupportedFamily(profile.family));
    }
    let plain = inflate_bounded(blob)?;
    parse_functional_options_parameter(&plain)
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

fn project_session_parameter(
    validated: &ValidatedConfiguration<'_>,
    object: &CanonicalObject,
) -> Result<SessionParameterNativeIr, SimpleMetadataBuildError> {
    let uuid = object.identity().uuid();
    match object.provenance().source_profile().as_str() {
        "xml-2.20" | "xml-2.21" => {}
        _ => return invalid_model(uuid, "source profile is not xml-2.20 or xml-2.21"),
    }
    if object.owner().is_some() {
        return invalid_model(uuid, "SessionParameter must be top-level");
    }
    if !object.references().is_empty()
        || !object.generated_types().is_empty()
        || !object.assets().is_empty()
    {
        return invalid_model(
            uuid,
            "SessionParameter cannot own canonical references, generated types, or assets",
        );
    }
    if validated
        .configuration()
        .objects()
        .iter()
        .any(|candidate| candidate.owner() == Some(uuid))
    {
        return invalid_model(uuid, "SessionParameter cannot own child objects");
    }
    let expected = ["Name", "Synonym", "Comment", "Type"];
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
    let synonyms = synonym_property(object, "Synonym")?;
    let comment = text_property(object, "Comment")?.to_owned();
    let values =
        property(object, "Type")?
            .as_sequence()
            .ok_or(SimpleMetadataBuildError::InvalidModel {
                object: uuid,
                reason: "Type is not a sequence",
            })?;
    if values.is_empty() {
        return invalid_model(uuid, "Type pattern is empty");
    }

    let generated_types = generated_type_reference_index(validated, uuid)?;
    let mut seen_scalars = BTreeSet::new();
    let mut seen_type_ids = BTreeSet::new();
    let mut types = Vec::with_capacity(values.len());
    for value in values {
        let fields = value
            .as_record()
            .ok_or(SimpleMetadataBuildError::InvalidModel {
                object: uuid,
                reason: "Type item is not a record",
            })?;
        let kind = canonical_enum_field(fields.first(), uuid, "Type kind is not an enum token")?;
        let native = match kind {
            "Boolean" => {
                require_record_schema(fields, &["kind"], uuid)?;
                SessionParameterTypeNativeIr::Boolean
            }
            "String" => {
                require_record_schema(fields, &["kind", "length", "allowed_length"], uuid)?;
                let length = canonical_u32_field(&fields[1], uuid)?;
                let allowed = canonical_enum_field(
                    Some(&fields[2]),
                    uuid,
                    "String allowed_length is not an enum token",
                )?;
                if allowed != "Variable" {
                    return invalid_model(
                        uuid,
                        "Fixed String has no evidenced SessionParameter native mapping",
                    );
                }
                SessionParameterTypeNativeIr::String { length }
            }
            "Number" => {
                require_record_schema(
                    fields,
                    &["kind", "digits", "fraction_digits", "allowed_sign"],
                    uuid,
                )?;
                let digits = canonical_u32_field(&fields[1], uuid)?;
                let fraction_digits = canonical_u32_field(&fields[2], uuid)?;
                if fraction_digits > digits {
                    return invalid_model(uuid, "Number FractionDigits exceeds Digits");
                }
                let allowed = canonical_enum_field(
                    Some(&fields[3]),
                    uuid,
                    "Number allowed_sign is not an enum token",
                )?;
                let nonnegative = match allowed {
                    "Any" => false,
                    "Nonnegative" => true,
                    _ => return invalid_model(uuid, "Number allowed_sign is unsupported"),
                };
                SessionParameterTypeNativeIr::Number {
                    digits,
                    fraction_digits,
                    nonnegative,
                }
            }
            "DateTime" => {
                require_record_schema(fields, &["kind", "date_fractions"], uuid)?;
                if canonical_enum_field(
                    Some(&fields[1]),
                    uuid,
                    "DateTime date_fractions is not an enum token",
                )? != "DateTime"
                {
                    return invalid_model(
                        uuid,
                        "Date or Time-only type has no evidenced native mapping",
                    );
                }
                SessionParameterTypeNativeIr::DateTime
            }
            "Reference" => {
                require_record_schema(fields, &["kind", "reference"], uuid)?;
                let readable = canonical_text(fields[1].value(), uuid)?;
                let type_id = builtin_type_uuid(readable)
                    .or_else(|| generated_types.get(readable).copied())
                    .ok_or(SimpleMetadataBuildError::InvalidModel {
                        object: uuid,
                        reason: "Type contains an unresolved readable generated type",
                    })?;
                if !seen_type_ids.insert(type_id) {
                    return invalid_model(uuid, "Type resolves more than once to the same TypeId");
                }
                SessionParameterTypeNativeIr::Reference(type_id)
            }
            _ => return invalid_model(uuid, "Type item kind is unsupported"),
        };
        if kind != "Reference" && !seen_scalars.insert(kind) {
            return invalid_model(uuid, "Type contains a duplicate primitive scalar");
        }
        types.push(native);
    }
    Ok(SessionParameterNativeIr {
        uuid,
        name,
        synonyms,
        comment,
        types,
    })
}

fn project_functional_option(
    validated: &ValidatedConfiguration<'_>,
    object: &CanonicalObject,
) -> Result<FunctionalOptionNativeIr, SimpleMetadataBuildError> {
    let uuid = object.identity().uuid();
    match object.provenance().source_profile().as_str() {
        "xml-2.20" | "xml-2.21" => {}
        _ => {
            return invalid_model(uuid, "source profile is not xml-2.20 or xml-2.21");
        }
    }
    if object.owner().is_some() {
        return invalid_model(uuid, "FunctionalOption must be top-level");
    }
    if !object.references().is_empty()
        || !object.generated_types().is_empty()
        || !object.assets().is_empty()
    {
        return invalid_model(
            uuid,
            "FunctionalOption cannot own canonical references, generated types, or assets",
        );
    }
    if validated
        .configuration()
        .objects()
        .iter()
        .any(|candidate| candidate.owner() == Some(uuid))
    {
        return invalid_model(uuid, "FunctionalOption cannot own child objects");
    }
    let expected = [
        "Name",
        "Synonym",
        "Comment",
        "Location",
        "PrivilegedGetMode",
        "Content",
    ];
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
    let synonyms = synonym_property(object, "Synonym")?;
    let references = readable_reference_index(validated, uuid)?;
    let location = match property(object, "Location")?.kind() {
        CanonicalValueKind::Null => None,
        CanonicalValueKind::Text(value) if !value.as_str().is_empty() => {
            Some(references.get(value.as_str()).copied().ok_or(
                SimpleMetadataBuildError::InvalidModel {
                    object: uuid,
                    reason: "Location contains an unresolved readable reference",
                },
            )?)
        }
        _ => return invalid_model(uuid, "Location is not null or non-empty text"),
    };
    let privileged_get_mode = match property(object, "PrivilegedGetMode")?.kind() {
        CanonicalValueKind::Bool(value) => value,
        _ => return invalid_model(uuid, "PrivilegedGetMode is not boolean"),
    };
    let content_values = property(object, "Content")?.as_sequence().ok_or(
        SimpleMetadataBuildError::InvalidModel {
            object: uuid,
            reason: "Content is not a sequence",
        },
    )?;
    let mut seen_names = BTreeSet::new();
    let mut seen_uuids = BTreeSet::new();
    let mut content = Vec::with_capacity(content_values.len());
    for value in content_values {
        let readable = canonical_text(value, uuid)?;
        if readable.is_empty() || !seen_names.insert(readable) {
            return invalid_model(uuid, "Content contains an empty or duplicate reference");
        }
        let target =
            references
                .get(readable)
                .copied()
                .ok_or(SimpleMetadataBuildError::InvalidModel {
                    object: uuid,
                    reason: "Content contains an unresolved readable reference",
                })?;
        if !seen_uuids.insert(target) {
            return invalid_model(uuid, "Content resolves more than once to the same object");
        }
        content.push(target);
    }
    Ok(FunctionalOptionNativeIr {
        uuid,
        name,
        synonyms,
        comment,
        location,
        privileged_get_mode,
        content,
    })
}

fn project_functional_options_parameter(
    validated: &ValidatedConfiguration<'_>,
    object: &CanonicalObject,
) -> Result<FunctionalOptionsParameterNativeIr, SimpleMetadataBuildError> {
    let uuid = object.identity().uuid();
    match object.provenance().source_profile().as_str() {
        "xml-2.20" | "xml-2.21" => {}
        _ => {
            return invalid_model(uuid, "source profile is not xml-2.20 or xml-2.21");
        }
    }
    if object.owner().is_some() {
        return invalid_model(uuid, "FunctionalOptionsParameter must be top-level");
    }
    if !object.references().is_empty()
        || !object.generated_types().is_empty()
        || !object.assets().is_empty()
    {
        return invalid_model(
            uuid,
            "FunctionalOptionsParameter cannot own canonical references, generated types, or assets",
        );
    }
    if validated
        .configuration()
        .objects()
        .iter()
        .any(|candidate| candidate.owner() == Some(uuid))
    {
        return invalid_model(uuid, "FunctionalOptionsParameter cannot own child objects");
    }
    let expected = ["Name", "Synonym", "Comment", "Use"];
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
    let synonyms = synonym_property(object, "Synonym")?;
    let use_values =
        property(object, "Use")?
            .as_sequence()
            .ok_or(SimpleMetadataBuildError::InvalidModel {
                object: uuid,
                reason: "Use is not a sequence",
            })?;
    let references = readable_reference_index(validated, uuid)?;
    let mut seen_names = BTreeSet::new();
    let mut seen_uuids = BTreeSet::new();
    let mut uses = Vec::with_capacity(use_values.len());
    for value in use_values {
        let readable = canonical_text(value, uuid)?;
        if readable.is_empty() || !seen_names.insert(readable) {
            return invalid_model(uuid, "Use contains an empty or duplicate reference");
        }
        let target =
            references
                .get(readable)
                .copied()
                .ok_or(SimpleMetadataBuildError::InvalidModel {
                    object: uuid,
                    reason: "Use contains an unresolved readable reference",
                })?;
        if !seen_uuids.insert(target) {
            return invalid_model(uuid, "Use resolves more than once to the same object");
        }
        uses.push(target);
    }
    Ok(FunctionalOptionsParameterNativeIr {
        uuid,
        name,
        synonyms,
        comment,
        uses,
    })
}

fn readable_reference_index(
    validated: &ValidatedConfiguration<'_>,
    compiling: ObjectUuid,
) -> Result<BTreeMap<String, ObjectUuid>, SimpleMetadataBuildError> {
    let mut cache = BTreeMap::<usize, Option<String>>::new();
    let mut visiting = BTreeSet::new();
    let mut references = BTreeMap::new();
    for index in 0..validated.configuration().objects().len() {
        let Some(reference) =
            readable_reference_for_object(validated, index, &mut cache, &mut visiting)
        else {
            continue;
        };
        let uuid = validated.configuration().objects()[index].identity().uuid();
        if references.insert(reference, uuid).is_some() {
            return invalid_model(compiling, "readable metadata reference is ambiguous");
        }
    }
    Ok(references)
}

fn generated_type_reference_index(
    validated: &ValidatedConfiguration<'_>,
    compiling: ObjectUuid,
) -> Result<BTreeMap<String, ObjectUuid>, SimpleMetadataBuildError> {
    let mut references = BTreeMap::new();
    for object in validated.configuration().objects() {
        if object.generated_types().is_empty() {
            continue;
        }
        let Some(name) = object
            .properties()
            .iter()
            .find(|field| field.name().as_str() == "Name")
            .and_then(|field| match field.value().kind() {
                CanonicalValueKind::Text(value)
                    if !value.as_str().is_empty()
                        && !value.as_str().contains('.')
                        && !value.as_str().chars().any(char::is_whitespace) =>
                {
                    Some(value.as_str())
                }
                _ => None,
            })
        else {
            continue;
        };
        for generated_type in object.generated_types() {
            if builtin_type_name(generated_type.uuid()).is_some() {
                return invalid_model(
                    compiling,
                    "generated TypeId collides with an evidenced built-in TypeId",
                );
            }
            let readable = format!(
                "cfg:{}{}.{}",
                object.kind().as_str(),
                generated_type.kind().as_str(),
                name
            );
            if references.insert(readable, generated_type.uuid()).is_some() {
                return invalid_model(compiling, "readable generated type name is ambiguous");
            }
        }
    }
    Ok(references)
}

fn readable_reference_for_object(
    validated: &ValidatedConfiguration<'_>,
    index: usize,
    cache: &mut BTreeMap<usize, Option<String>>,
    visiting: &mut BTreeSet<usize>,
) -> Option<String> {
    if let Some(cached) = cache.get(&index) {
        return cached.clone();
    }
    if !visiting.insert(index) {
        return None;
    }
    let object = validated.configuration().objects().get(index)?;
    let name = object
        .properties()
        .iter()
        .find(|field| field.name().as_str() == "Name")
        .and_then(|field| match field.value().kind() {
            CanonicalValueKind::Text(value)
                if !value.as_str().is_empty() && !value.as_str().contains('.') =>
            {
                Some(value.as_str())
            }
            _ => None,
        });
    let reference = name.and_then(|name| {
        let own = format!("{}.{}", object.kind().as_str(), name);
        match object.owner() {
            None => Some(own),
            Some(owner) => {
                let owner_index = validated.graph().object_index_by_uuid(owner)?;
                readable_reference_for_object(validated, owner_index, cache, visiting)
                    .map(|parent| format!("{parent}.{own}"))
            }
        }
    });
    visiting.remove(&index);
    cache.insert(index, reference.clone());
    reference
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
        _ => invalid_model(object, "typed value is not text"),
    }
}

fn require_record_schema(
    fields: &[CanonicalField],
    expected: &[&str],
    object: ObjectUuid,
) -> Result<(), SimpleMetadataBuildError> {
    if fields.len() != expected.len()
        || fields
            .iter()
            .zip(expected)
            .any(|(field, expected)| field.name().as_str() != *expected)
    {
        invalid_model(object, "Type item schema is not exact")
    } else {
        Ok(())
    }
}

fn canonical_enum_field<'a>(
    field: Option<&'a CanonicalField>,
    object: ObjectUuid,
    reason: &'static str,
) -> Result<&'a str, SimpleMetadataBuildError> {
    match field.map(CanonicalField::value).map(CanonicalValue::kind) {
        Some(CanonicalValueKind::EnumToken(value)) => Ok(value.as_str()),
        _ => invalid_model(object, reason),
    }
}

fn canonical_u32_field(
    field: &CanonicalField,
    object: ObjectUuid,
) -> Result<u32, SimpleMetadataBuildError> {
    match field.value().kind() {
        CanonicalValueKind::Integer(value) => {
            value
                .as_str()
                .parse::<u32>()
                .map_err(|_| SimpleMetadataBuildError::InvalidModel {
                    object,
                    reason: "Type qualifier is not u32",
                })
        }
        _ => invalid_model(object, "Type qualifier is not an integer"),
    }
}

fn serialize_language(value: &LanguageNativeIr) -> Vec<u8> {
    let mut plaintext = String::new();
    plaintext.push_str("{1,\r\n{0,\r\n");
    push_native_header(
        &mut plaintext,
        value.uuid,
        &value.name,
        &value.synonyms,
        &value.comment,
    );
    plaintext.push(',');
    push_1c_string(&mut plaintext, &value.language_code);
    plaintext.push_str("},0}");
    plaintext.into_bytes()
}

fn serialize_session_parameter(value: &SessionParameterNativeIr) -> Vec<u8> {
    let mut plaintext = String::new();
    plaintext.push_str("{1,\r\n{1,\r\n{2,\r\n");
    push_native_header(
        &mut plaintext,
        value.uuid,
        &value.name,
        &value.synonyms,
        &value.comment,
    );
    plaintext.push_str(",\r\n{\"Pattern\"");
    for item in &value.types {
        plaintext.push_str(",\r\n");
        match item {
            SessionParameterTypeNativeIr::Boolean => plaintext.push_str("{\"B\"}"),
            SessionParameterTypeNativeIr::String { length: 0 } => plaintext.push_str("{\"S\"}"),
            SessionParameterTypeNativeIr::String { length } => {
                write!(&mut plaintext, "{{\"S\",{length},1}}")
                    .expect("writing to String cannot fail");
            }
            SessionParameterTypeNativeIr::Number {
                digits,
                fraction_digits,
                nonnegative,
            } => {
                write!(
                    &mut plaintext,
                    "{{\"N\",{digits},{fraction_digits},{}}}",
                    if *nonnegative { 1 } else { 0 }
                )
                .expect("writing to String cannot fail");
            }
            SessionParameterTypeNativeIr::DateTime => plaintext.push_str("{\"D\"}"),
            SessionParameterTypeNativeIr::Reference(type_id) => {
                plaintext.push_str("{\"#\",");
                plaintext.push_str(&type_id.to_string());
                plaintext.push('}');
            }
        }
    }
    plaintext.push_str("\r\n}\r\n}\r\n},0}");
    plaintext.into_bytes()
}

fn serialize_functional_option(value: &FunctionalOptionNativeIr) -> Vec<u8> {
    let mut plaintext = String::new();
    plaintext.push_str("{1,\r\n{2,\r\n");
    push_native_header(
        &mut plaintext,
        value.uuid,
        &value.name,
        &value.synonyms,
        &value.comment,
    );
    plaintext.push(',');
    plaintext.push_str(
        &value
            .location
            .map(|uuid| uuid.to_string())
            .unwrap_or_else(|| NIL_UUID.to_owned()),
    );
    plaintext.push_str(",\r\n");
    if value.content.is_empty() {
        plaintext.push_str("{0,0}");
    } else {
        write!(&mut plaintext, "{{0,{}", value.content.len())
            .expect("writing to String cannot fail");
        for uuid in &value.content {
            plaintext.push_str(",\r\n{\"#\",");
            plaintext.push_str(FUNCTIONAL_OPTION_CONTENT_CLASS_UUID);
            plaintext.push_str(",\r\n{0,\r\n");
            push_design_time_reference(&mut plaintext, *uuid);
            plaintext.push_str("\r\n}\r\n}");
        }
        plaintext.push_str("\r\n}");
    }
    plaintext.push(',');
    plaintext.push(if value.privileged_get_mode { '1' } else { '0' });
    plaintext.push_str("},0}");
    plaintext.into_bytes()
}

fn serialize_functional_options_parameter(value: &FunctionalOptionsParameterNativeIr) -> Vec<u8> {
    let mut plaintext = String::new();
    plaintext.push_str("{1,\r\n{0,\r\n");
    push_native_header(
        &mut plaintext,
        value.uuid,
        &value.name,
        &value.synonyms,
        &value.comment,
    );
    plaintext.push_str(",\r\n");
    if value.uses.is_empty() {
        plaintext.push_str("{0}");
    } else {
        write!(&mut plaintext, "{{0,{}", value.uses.len()).expect("writing to String cannot fail");
        for uuid in &value.uses {
            plaintext.push_str(",\r\n");
            push_design_time_reference(&mut plaintext, *uuid);
        }
        plaintext.push_str("\r\n}");
    }
    plaintext.push_str("\r\n},0}");
    plaintext.into_bytes()
}

fn push_design_time_reference(output: &mut String, uuid: ObjectUuid) {
    output.push_str("{\"#\",");
    output.push_str(DESIGN_TIME_REFERENCE_CLASS_UUID);
    output.push_str(",\r\n{1,");
    output.push_str(&uuid.to_string());
    output.push_str("}\r\n}");
}

fn push_native_header(
    output: &mut String,
    uuid: ObjectUuid,
    name: &str,
    synonyms: &[NativeLocalizedString],
    comment: &str,
) {
    output.push_str("{3,\r\n{1,0,");
    output.push_str(&uuid.to_string());
    output.push_str("},");
    push_1c_string(output, name);
    output.push(',');
    write!(output, "{{{}", synonyms.len()).expect("writing to String cannot fail");
    for synonym in synonyms {
        output.push(',');
        push_1c_string(output, &synonym.language);
        output.push(',');
        push_1c_string(output, &synonym.content);
    }
    output.push_str("},");
    push_1c_string(output, comment);
    output.push_str(",0,0,");
    output.push_str(NIL_UUID);
    output.push_str(",0}");
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
    if plaintext.len() > MAX_SIMPLE_METADATA_PLAIN_BYTES {
        return Err(SimpleMetadataBuildError::PlainPayloadTooLarge {
            maximum: MAX_SIMPLE_METADATA_PLAIN_BYTES,
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
    let limit = MAX_SIMPLE_METADATA_PLAIN_BYTES
        .checked_add(1)
        .expect("simple metadata plaintext bound is below usize::MAX");
    let mut decoder = DeflateDecoder::new(blob).take(limit as u64);
    let mut plain = Vec::new();
    decoder
        .read_to_end(&mut plain)
        .map_err(SimpleMetadataBuildError::Inflate)?;
    if plain.len() > MAX_SIMPLE_METADATA_PLAIN_BYTES {
        return Err(SimpleMetadataBuildError::PlainPayloadTooLarge {
            maximum: MAX_SIMPLE_METADATA_PLAIN_BYTES,
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
            return Err(native("unexpected BOM for simple metadata no-BOM layout"));
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
    let header = parse_native_header(&object[1])?;
    let language_code = text(&object[2], "LanguageCode")?.to_owned();
    validate_native_text(&language_code, "LanguageCode")?;
    if language_code.is_empty() || language_code.len() > MAX_LANGUAGE_CODE_BYTES {
        return Err(native("LanguageCode is empty or exceeds its bound"));
    }
    Ok(LanguageNativeIr {
        uuid: header.uuid,
        name: header.name,
        synonyms: header.synonyms,
        comment: header.comment,
        language_code,
    })
}

fn parse_session_parameter(
    plain: &[u8],
) -> Result<SessionParameterNativeIr, SimpleMetadataBuildError> {
    let root = NativeParser::new(plain).parse()?;
    let root = exact_list(&root, 3, "root")?;
    exact_token(&root[0], "1", "root discriminator")?;
    exact_token(&root[2], "0", "root tail")?;
    let wrapper = exact_list(&root[1], 2, "SessionParameter wrapper")?;
    exact_token(&wrapper[0], "1", "SessionParameter wrapper discriminator")?;
    let object = exact_list(&wrapper[1], 3, "SessionParameter object")?;
    exact_token(&object[0], "2", "SessionParameter discriminator")?;
    let header = parse_native_header(&object[1])?;
    let types = parse_native_type_pattern(&object[2])?;
    Ok(SessionParameterNativeIr {
        uuid: header.uuid,
        name: header.name,
        synonyms: header.synonyms,
        comment: header.comment,
        types,
    })
}

fn parse_native_type_pattern(
    value: &NativeValue,
) -> Result<Vec<SessionParameterTypeNativeIr>, SimpleMetadataBuildError> {
    let fields = list(value, "Type pattern")?;
    if fields.len() < 2 {
        return Err(native("Type pattern is empty"));
    }
    if text(&fields[0], "Type pattern marker")? != "Pattern" {
        return Err(native("Type pattern marker is not Pattern"));
    }
    if fields.len() - 1 > MAX_CANONICAL_COLLECTION_ITEMS {
        return Err(native("Type pattern exceeds canonical collection bound"));
    }
    let mut scalar_tags = BTreeSet::new();
    let mut type_ids = BTreeSet::new();
    let mut types = Vec::with_capacity(fields.len() - 1);
    for value in &fields[1..] {
        let item = list(value, "Type pattern item")?;
        let tag = item
            .first()
            .ok_or_else(|| native("Type pattern item is empty"))
            .and_then(|value| text(value, "Type pattern item tag"))?;
        let parsed = match tag {
            "B" if item.len() == 1 => SessionParameterTypeNativeIr::Boolean,
            "S" if item.len() == 1 => SessionParameterTypeNativeIr::String { length: 0 },
            "S" if item.len() == 3 => {
                let length = canonical_u32_token(&item[1], "String length")?;
                if length == 0 {
                    return Err(native(
                        "zero-length String must use the compact native shape",
                    ));
                }
                exact_token(&item[2], "1", "String Variable flag")?;
                SessionParameterTypeNativeIr::String { length }
            }
            "N" if item.len() == 4 => {
                let digits = canonical_u32_token(&item[1], "Number digits")?;
                let fraction_digits = canonical_u32_token(&item[2], "Number fraction digits")?;
                if fraction_digits > digits {
                    return Err(native("Number FractionDigits exceeds Digits"));
                }
                let nonnegative = match token(&item[3], "Number sign flag")? {
                    "0" => false,
                    "1" => true,
                    _ => return Err(native("Number sign flag is not 0 or 1")),
                };
                SessionParameterTypeNativeIr::Number {
                    digits,
                    fraction_digits,
                    nonnegative,
                }
            }
            "D" if item.len() == 1 => SessionParameterTypeNativeIr::DateTime,
            "#" if item.len() == 2 => {
                let type_id = canonical_uuid_token(&item[1], "TypeId")?;
                if type_id.to_string() == NIL_UUID || !type_ids.insert(type_id) {
                    return Err(native("TypeId is nil or duplicated"));
                }
                types.push(SessionParameterTypeNativeIr::Reference(type_id));
                continue;
            }
            "B" | "S" | "N" | "D" | "#" => {
                return Err(native(
                    "Type pattern item has unexpected field count or flag",
                ));
            }
            _ => return Err(native("unknown SessionParameter Type pattern item")),
        };
        if !scalar_tags.insert(tag) {
            return Err(native(
                "duplicate primitive in SessionParameter Type pattern",
            ));
        }
        types.push(parsed);
    }
    Ok(types)
}

fn parse_functional_option(
    plain: &[u8],
) -> Result<FunctionalOptionNativeIr, SimpleMetadataBuildError> {
    let root = NativeParser::new(plain).parse()?;
    let root = exact_list(&root, 3, "root")?;
    exact_token(&root[0], "1", "root discriminator")?;
    exact_token(&root[2], "0", "root tail")?;
    let object = exact_list(&root[1], 5, "FunctionalOption object")?;
    exact_token(&object[0], "2", "FunctionalOption discriminator")?;
    let header = parse_native_header(&object[1])?;
    let location_text = token(&object[2], "Location UUID")?;
    let location = if location_text == NIL_UUID {
        None
    } else {
        Some(canonical_uuid_token(&object[2], "Location UUID")?)
    };
    let content = parse_native_functional_option_content(&object[3])?;
    let privileged_get_mode = match token(&object[4], "PrivilegedGetMode")? {
        "0" => false,
        "1" => true,
        _ => return Err(native("PrivilegedGetMode is not 0 or 1")),
    };
    Ok(FunctionalOptionNativeIr {
        uuid: header.uuid,
        name: header.name,
        synonyms: header.synonyms,
        comment: header.comment,
        location,
        privileged_get_mode,
        content,
    })
}

fn parse_functional_options_parameter(
    plain: &[u8],
) -> Result<FunctionalOptionsParameterNativeIr, SimpleMetadataBuildError> {
    let root = NativeParser::new(plain).parse()?;
    let root = exact_list(&root, 3, "root")?;
    exact_token(&root[0], "1", "root discriminator")?;
    exact_token(&root[2], "0", "root tail")?;
    let object = exact_list(&root[1], 3, "FunctionalOptionsParameter object")?;
    exact_token(&object[0], "0", "FunctionalOptionsParameter discriminator")?;
    let header = parse_native_header(&object[1])?;
    let uses = parse_native_use_references(&object[2])?;
    Ok(FunctionalOptionsParameterNativeIr {
        uuid: header.uuid,
        name: header.name,
        synonyms: header.synonyms,
        comment: header.comment,
        uses,
    })
}

struct NativeHeaderIr {
    uuid: ObjectUuid,
    name: String,
    synonyms: Vec<NativeLocalizedString>,
    comment: String,
}

fn parse_native_header(value: &NativeValue) -> Result<NativeHeaderIr, SimpleMetadataBuildError> {
    let header = exact_list(value, 9, "metadata header")?;
    exact_token(&header[0], "3", "metadata header discriminator")?;
    let identity = exact_list(&header[1], 3, "metadata identity")?;
    exact_token(&identity[0], "1", "identity discriminator")?;
    exact_token(&identity[1], "0", "identity tail")?;
    let uuid = canonical_uuid_token(&identity[2], "object UUID")?;
    let name = text(&header[2], "Name")?.to_owned();
    let synonyms = parse_synonyms(&header[3])?;
    let comment = text(&header[4], "Comment")?.to_owned();
    exact_token(&header[5], "0", "header flag 1")?;
    exact_token(&header[6], "0", "header flag 2")?;
    exact_token(&header[7], NIL_UUID, "header nil UUID")?;
    exact_token(&header[8], "0", "header tail")?;
    validate_native_text(&name, "Name")?;
    validate_native_text(&comment, "Comment")?;
    if name.is_empty() {
        return Err(native("Name must not be empty"));
    }
    Ok(NativeHeaderIr {
        uuid,
        name,
        synonyms,
        comment,
    })
}

fn parse_native_use_references(
    value: &NativeValue,
) -> Result<Vec<ObjectUuid>, SimpleMetadataBuildError> {
    let fields = list(value, "Use")?;
    if fields.len() == 1 {
        exact_token(&fields[0], "0", "empty Use discriminator")?;
        return Ok(Vec::new());
    }
    exact_token(&fields[0], "0", "Use discriminator")?;
    let count_text = token(&fields[1], "Use count")?;
    let count = count_text
        .parse::<usize>()
        .ok()
        .filter(|count| count.to_string() == count_text)
        .ok_or_else(|| native("Use count is not canonical decimal"))?;
    if count > MAX_CANONICAL_COLLECTION_ITEMS || fields.len() != count + 2 {
        return Err(native(
            "Use count is out of bounds or does not match fields",
        ));
    }
    let mut unique = BTreeSet::new();
    let mut uses = Vec::with_capacity(count);
    for value in &fields[2..] {
        let uuid = parse_design_time_reference(value)?;
        if !unique.insert(uuid) {
            return Err(native("Use target UUID is nil or duplicated"));
        }
        uses.push(uuid);
    }
    Ok(uses)
}

fn parse_native_functional_option_content(
    value: &NativeValue,
) -> Result<Vec<ObjectUuid>, SimpleMetadataBuildError> {
    let fields = list(value, "Content")?;
    if fields.len() < 2 {
        return Err(native("Content discriminator or count is missing"));
    }
    exact_token(&fields[0], "0", "Content discriminator")?;
    let count_text = token(&fields[1], "Content count")?;
    let count = count_text
        .parse::<usize>()
        .ok()
        .filter(|count| count.to_string() == count_text)
        .ok_or_else(|| native("Content count is not canonical decimal"))?;
    if count > MAX_CANONICAL_COLLECTION_ITEMS || fields.len() != count + 2 {
        return Err(native(
            "Content count is out of bounds or does not match fields",
        ));
    }
    let mut unique = BTreeSet::new();
    let mut content = Vec::with_capacity(count);
    for value in &fields[2..] {
        let outer = exact_list(value, 3, "Content reference")?;
        if text(&outer[0], "Content reference marker")? != "#" {
            return Err(native("Content reference marker is not #"));
        }
        exact_token(
            &outer[1],
            FUNCTIONAL_OPTION_CONTENT_CLASS_UUID,
            "Content reference class",
        )?;
        let payload = exact_list(&outer[2], 2, "Content reference payload")?;
        exact_token(&payload[0], "0", "Content payload discriminator")?;
        let uuid = parse_design_time_reference(&payload[1])?;
        if !unique.insert(uuid) {
            return Err(native("Content target UUID is duplicated"));
        }
        content.push(uuid);
    }
    Ok(content)
}

fn parse_design_time_reference(
    value: &NativeValue,
) -> Result<ObjectUuid, SimpleMetadataBuildError> {
    let reference = exact_list(value, 3, "design-time reference")?;
    if text(&reference[0], "design-time reference marker")? != "#" {
        return Err(native("design-time reference marker is not #"));
    }
    exact_token(
        &reference[1],
        DESIGN_TIME_REFERENCE_CLASS_UUID,
        "design-time reference class",
    )?;
    let target = exact_list(&reference[2], 2, "design-time reference target")?;
    exact_token(&target[0], "1", "design-time reference discriminator")?;
    let uuid = canonical_uuid_token(&target[1], "design-time target UUID")?;
    if uuid.to_string() == NIL_UUID {
        return Err(native("design-time target UUID is nil"));
    }
    Ok(uuid)
}

fn canonical_uuid_token(
    value: &NativeValue,
    field: &'static str,
) -> Result<ObjectUuid, SimpleMetadataBuildError> {
    let value = token(value, field)?;
    let uuid = ObjectUuid::parse(value).map_err(|_| native(&format!("invalid {field}")))?;
    if uuid.to_string() != value {
        return Err(native(&format!("{field} is not canonical lowercase text")));
    }
    Ok(uuid)
}

fn canonical_u32_token(
    value: &NativeValue,
    field: &'static str,
) -> Result<u32, SimpleMetadataBuildError> {
    let value = token(value, field)?;
    value
        .parse::<u32>()
        .ok()
        .filter(|parsed| parsed.to_string() == value)
        .ok_or_else(|| native(&format!("{field} is not canonical u32")))
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

fn builtin_type_uuid(name: &str) -> Option<ObjectUuid> {
    let value = match name {
        "v8:FixedArray" => FIXED_ARRAY_TYPE_UUID,
        "v8:FixedMap" => FIXED_MAP_TYPE_UUID,
        "v8:FixedStructure" => FIXED_STRUCTURE_TYPE_UUID,
        "v8:UUID" => UUID_TYPE_UUID,
        "v8:ValueStorage" => VALUE_STORAGE_TYPE_UUID,
        _ => return None,
    };
    Some(ObjectUuid::parse(value).expect("evidenced built-in TypeId UUID is stable"))
}

fn builtin_type_name(type_id: ObjectUuid) -> Option<&'static str> {
    match type_id.to_string().as_str() {
        FIXED_ARRAY_TYPE_UUID => Some("v8:FixedArray"),
        FIXED_MAP_TYPE_UUID => Some("v8:FixedMap"),
        FIXED_STRUCTURE_TYPE_UUID => Some("v8:FixedStructure"),
        UUID_TYPE_UUID => Some("v8:UUID"),
        VALUE_STORAGE_TYPE_UUID => Some("v8:ValueStorage"),
        _ => None,
    }
}

fn supported_cfg_type_name(value: &str) -> bool {
    let Some(tail) = value.strip_prefix("cfg:") else {
        return false;
    };
    let Some((kind, name)) = tail.split_once('.') else {
        return false;
    };
    !kind.is_empty()
        && !name.is_empty()
        && !tail.chars().any(char::is_whitespace)
        && !name.contains('.')
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

fn write_synonym_xml(output: &mut String, synonyms: &[NativeLocalizedString]) {
    if synonyms.is_empty() {
        output.push_str("\t\t\t<Synonym/>\r\n");
        return;
    }
    output.push_str("\t\t\t<Synonym>\r\n");
    for synonym in synonyms {
        output.push_str("\t\t\t\t<v8:item>\r\n");
        write_xml_text_element(output, "\t\t\t\t\t", "v8:lang", &synonym.language);
        write_xml_text_element(output, "\t\t\t\t\t", "v8:content", &synonym.content);
        output.push_str("\t\t\t\t</v8:item>\r\n");
    }
    output.push_str("\t\t\t</Synonym>\r\n");
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
        CanonicalConfiguration, CanonicalObject, CanonicalObjectParts, GeneratedType,
        GeneratedTypeKind, MetadataKind,
    };
    use ibcmd_core::profile::{ProfileSourceKind, parse_profile_source, resolve_profiles};
    use ibcmd_core::provenance::{CanonicalAnchor, SourceProvenance};
    use ibcmd_core::storage::StoragePatchOutcome;
    use ibcmd_core::validate::validate_configuration;
    use ibcmd_core::value::{CanonicalField, CanonicalText, CanonicalValue};
    use ibcmd_core::version::XmlDialect;
    use ibcmd_xml::{XmlReader, bundled_metadata_registry};

    use crate::compiler::graph::{ObjectStorageRoute, build_bootstrap_graph};
    use crate::compiler::identity::collect_bootstrap_identities;

    use super::*;

    const UUID: &str = "11111111-1111-4111-8111-111111111111";
    const CONFIGURATION_UUID: &str = "22222222-2222-4222-8222-222222222222";
    const FUNCTIONAL_OPTIONS_PARAMETER_UUID: &str = "33333333-3333-4333-8333-333333333333";
    const CATALOG_UUID: &str = "44444444-4444-4444-8444-444444444444";
    const FUNCTIONAL_OPTION_UUID: &str = "55555555-5555-4555-8555-555555555555";
    const CONSTANT_UUID: &str = "66666666-6666-4666-8666-666666666666";
    const SESSION_PARAMETER_UUID: &str = "77777777-7777-4777-8777-777777777777";
    const SECOND_CATALOG_UUID: &str = "88888888-8888-4888-8888-888888888888";
    const FIRST_CATALOG_REF_TYPE_ID: &str = "3997c341-4065-4af6-813f-99750a01052b";
    const SECOND_CATALOG_REF_TYPE_ID: &str = "c54edff0-c3a1-44d7-9707-1fe05700b055";

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

    fn functional_options_parameter_xml(version: &str, reference: &str) -> Vec<u8> {
        format!(
            "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"{version}\">\r\n\
\t<FunctionalOptionsParameter uuid=\"{FUNCTIONAL_OPTIONS_PARAMETER_UUID}\">\r\n\
\t\t<Properties>\r\n\
\t\t\t<Name>UseFeatureFor</Name>\r\n\
\t\t\t<Synonym><v8:item><v8:lang>en</v8:lang><v8:content>Use feature for</v8:content></v8:item></Synonym>\r\n\
\t\t\t<Comment/>\r\n\
\t\t\t<Use><xr:Item xsi:type=\"xr:MDObjectRef\">{reference}</xr:Item></Use>\r\n\
\t\t</Properties>\r\n\
\t</FunctionalOptionsParameter>\r\n\
</MetaDataObject>"
        )
        .into_bytes()
    }

    fn functional_option_xml(version: &str, location: &str, content: &str) -> Vec<u8> {
        format!(
            "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" version=\"{version}\">\r\n\
\t<FunctionalOption uuid=\"{FUNCTIONAL_OPTION_UUID}\">\r\n\
\t\t<Properties>\r\n\
\t\t\t<Name>UseFeature</Name>\r\n\
\t\t\t<Synonym><v8:item><v8:lang>en</v8:lang><v8:content>Use feature</v8:content></v8:item></Synonym>\r\n\
\t\t\t<Comment/>\r\n\
\t\t\t<Location>{location}</Location>\r\n\
\t\t\t<PrivilegedGetMode>true</PrivilegedGetMode>\r\n\
\t\t\t<Content><xr:Object>{content}</xr:Object></Content>\r\n\
\t\t</Properties>\r\n\
\t</FunctionalOption>\r\n\
</MetaDataObject>"
        )
        .into_bytes()
    }

    fn session_parameter_xml(version: &str, type_body: &str) -> Vec<u8> {
        format!(
            "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:cfg=\"http://v8.1c.ru/8.1/data/enterprise/current-config\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" version=\"{version}\">\r\n\
\t<SessionParameter uuid=\"{SESSION_PARAMETER_UUID}\">\r\n\
\t\t<Properties>\r\n\
\t\t\t<Name>AuthorizedUser</Name>\r\n\
\t\t\t<Synonym><v8:item><v8:lang>en</v8:lang><v8:content>Authorized user</v8:content></v8:item></Synonym>\r\n\
\t\t\t<Comment/>\r\n\
\t\t\t<Type>{type_body}</Type>\r\n\
\t\t</Properties>\r\n\
\t</SessionParameter>\r\n\
</MetaDataObject>"
        )
        .into_bytes()
    }

    fn simple_object_parts(
        version: &str,
        uuid: &str,
        kind: &str,
        name: &str,
    ) -> CanonicalObjectParts {
        let path = ObjectPath::new(vec![
            PathSegment::name(&format!(
                "{}-{}",
                kind.to_ascii_lowercase(),
                name.to_ascii_lowercase()
            ))
            .unwrap(),
        ])
        .unwrap();
        let provenance = SourceProvenance::new(
            ProfileId::parse(&format!("xml-{version}")).unwrap(),
            CanonicalAnchor::new(path.clone(), PropertyPath::root()),
        );
        let mut parts = CanonicalObjectParts::new(
            LogicalIdentity::new(ObjectUuid::parse(uuid).unwrap(), path),
            MetadataKind::new(kind).unwrap(),
            provenance,
        );
        parts.properties.push(
            CanonicalField::named(
                "Name",
                CanonicalValue::text(CanonicalText::new(name).unwrap()),
            )
            .unwrap(),
        );
        parts
    }

    fn simple_object(version: &str, uuid: &str, kind: &str, name: &str) -> CanonicalObject {
        CanonicalObject::new(simple_object_parts(version, uuid, kind, name)).unwrap()
    }

    fn simple_object_with_generated_type(
        version: &str,
        uuid: &str,
        kind: &str,
        name: &str,
        type_id: &str,
        type_kind: &str,
    ) -> CanonicalObject {
        let mut parts = simple_object_parts(version, uuid, kind, name);
        parts.generated_types.push(GeneratedType::new(
            ObjectUuid::parse(type_id).unwrap(),
            GeneratedTypeKind::new(type_kind).unwrap(),
        ));
        CanonicalObject::new(parts).unwrap()
    }

    fn functional_options_parameter_configuration(version: &str) -> CanonicalConfiguration {
        functional_options_parameter_configuration_with_reference(version, "Catalog.Products")
    }

    fn functional_options_parameter_configuration_with_reference(
        version: &str,
        reference: &str,
    ) -> CanonicalConfiguration {
        let document =
            XmlReader::from_slice(&functional_options_parameter_xml(version, reference)).unwrap();
        let parameter = bundled_metadata_registry()
            .decode(
                &FamilyId::parse("FunctionalOptionsParameter").unwrap(),
                &document,
                ProfileId::parse(&format!("xml-{version}")).unwrap(),
                ObjectPath::root(),
            )
            .unwrap()
            .root()
            .clone();
        CanonicalConfiguration::new(vec![
            simple_object(version, CONFIGURATION_UUID, "Configuration", "Fixture"),
            simple_object(version, CATALOG_UUID, "Catalog", "Products"),
            parameter,
        ])
        .unwrap()
    }

    fn functional_option_configuration(
        version: &str,
        location: &str,
        content: &str,
    ) -> CanonicalConfiguration {
        let document =
            XmlReader::from_slice(&functional_option_xml(version, location, content)).unwrap();
        let option = bundled_metadata_registry()
            .decode(
                &FamilyId::parse("FunctionalOption").unwrap(),
                &document,
                ProfileId::parse(&format!("xml-{version}")).unwrap(),
                ObjectPath::root(),
            )
            .unwrap()
            .root()
            .clone();
        CanonicalConfiguration::new(vec![
            simple_object(version, CONFIGURATION_UUID, "Configuration", "Fixture"),
            simple_object(version, CONSTANT_UUID, "Constant", "FeatureState"),
            simple_object(version, CATALOG_UUID, "Catalog", "Products"),
            option,
        ])
        .unwrap()
    }

    fn session_parameter_configuration(version: &str, type_body: &str) -> CanonicalConfiguration {
        let document = XmlReader::from_slice(&session_parameter_xml(version, type_body)).unwrap();
        let parameter = bundled_metadata_registry()
            .decode(
                &FamilyId::parse("SessionParameter").unwrap(),
                &document,
                ProfileId::parse(&format!("xml-{version}")).unwrap(),
                ObjectPath::root(),
            )
            .unwrap()
            .root()
            .clone();
        CanonicalConfiguration::new(vec![
            simple_object(version, CONFIGURATION_UUID, "Configuration", "Fixture"),
            simple_object_with_generated_type(
                version,
                CATALOG_UUID,
                "Catalog",
                "ExternalUsers",
                FIRST_CATALOG_REF_TYPE_ID,
                "Ref",
            ),
            simple_object_with_generated_type(
                version,
                SECOND_CATALOG_UUID,
                "Catalog",
                "Users",
                SECOND_CATALOG_REF_TYPE_ID,
                "Ref",
            ),
            parameter,
        ])
        .unwrap()
    }

    fn functional_options_parameter_graph<'a>(
        validated: &ValidatedConfiguration<'a>,
    ) -> (BootstrapGraph, SimpleMetadataProfile) {
        let identities = collect_bootstrap_identities(validated).unwrap();
        let graph = build_bootstrap_graph(
            &identities,
            ProfileId::parse("platform-test").unwrap(),
            [
                CONFIGURATION_UUID,
                CATALOG_UUID,
                FUNCTIONAL_OPTIONS_PARAMETER_UUID,
            ]
            .into_iter()
            .map(|uuid| {
                ObjectStorageRoute::new(ObjectUuid::parse(uuid).unwrap(), Vec::new()).unwrap()
            })
            .collect(),
        )
        .unwrap();
        (
            graph,
            SimpleMetadataProfile::functional_options_parameter_fixture("platform-test"),
        )
    }

    fn functional_option_graph<'a>(
        validated: &ValidatedConfiguration<'a>,
    ) -> (BootstrapGraph, SimpleMetadataProfile) {
        let identities = collect_bootstrap_identities(validated).unwrap();
        let graph = build_bootstrap_graph(
            &identities,
            ProfileId::parse("platform-test").unwrap(),
            [
                CONFIGURATION_UUID,
                CONSTANT_UUID,
                CATALOG_UUID,
                FUNCTIONAL_OPTION_UUID,
            ]
            .into_iter()
            .map(|uuid| {
                ObjectStorageRoute::new(ObjectUuid::parse(uuid).unwrap(), Vec::new()).unwrap()
            })
            .collect(),
        )
        .unwrap();
        (
            graph,
            SimpleMetadataProfile::functional_option_fixture("platform-test"),
        )
    }

    fn session_parameter_graph<'a>(
        validated: &ValidatedConfiguration<'a>,
    ) -> (BootstrapGraph, SimpleMetadataProfile) {
        let identities = collect_bootstrap_identities(validated).unwrap();
        let graph = build_bootstrap_graph(
            &identities,
            ProfileId::parse("platform-test").unwrap(),
            [
                CONFIGURATION_UUID,
                CATALOG_UUID,
                SECOND_CATALOG_UUID,
                SESSION_PARAMETER_UUID,
            ]
            .into_iter()
            .map(|uuid| {
                ObjectStorageRoute::new(ObjectUuid::parse(uuid).unwrap(), Vec::new()).unwrap()
            })
            .collect(),
        )
        .unwrap();
        (
            graph,
            SimpleMetadataProfile::session_parameter_fixture("platform-test"),
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
                "constants": {{
                    "{LANGUAGE_LAYOUT_KEY}": "{LANGUAGE_LAYOUT}",
                    "{SESSION_PARAMETER_LAYOUT_KEY}": "{SESSION_PARAMETER_LAYOUT}",
                    "{FUNCTIONAL_OPTION_LAYOUT_KEY}": "{FUNCTIONAL_OPTION_LAYOUT}",
                    "{FUNCTIONAL_OPTIONS_PARAMETER_LAYOUT_KEY}": "{FUNCTIONAL_OPTIONS_PARAMETER_LAYOUT}"
                }}
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
        assert_eq!(
            SimpleMetadataProfile::from_effective_for_family(
                effective,
                SimpleFamily::SessionParameter
            )
            .unwrap()
            .family(),
            SimpleFamily::SessionParameter
        );
        assert_eq!(
            SimpleMetadataProfile::from_effective_for_family(
                effective,
                SimpleFamily::FunctionalOptionsParameter
            )
            .unwrap()
            .family(),
            SimpleFamily::FunctionalOptionsParameter
        );
        assert_eq!(
            SimpleMetadataProfile::from_effective_for_family(
                effective,
                SimpleFamily::FunctionalOption
            )
            .unwrap()
            .family(),
            SimpleFamily::FunctionalOption
        );
    }

    #[test]
    fn bundled_8_3_27_profile_selects_only_committed_simple_layouts() {
        let registry = crate::profile_registry::load_bundled_profile_registry().unwrap();
        let effective = registry
            .get(&ProfileId::parse("platform-8.3.27.1989").unwrap())
            .unwrap();
        for family in [
            SimpleFamily::Language,
            SimpleFamily::SessionParameter,
            SimpleFamily::FunctionalOption,
            SimpleFamily::FunctionalOptionsParameter,
        ] {
            assert_eq!(
                SimpleMetadataProfile::from_effective_for_family(effective, family)
                    .unwrap()
                    .family(),
                family
            );
        }
        assert!(matches!(
            SimpleMetadataProfile::from_effective_for_family(effective, SimpleFamily::DefinedType),
            Err(SimpleMetadataProfileError::FamilyNotImplemented { .. })
        ));
    }

    #[test]
    fn session_parameter_roundtrips_generated_type_ids_without_a_base() {
        let type_body = "<v8:Type>cfg:CatalogRef.ExternalUsers</v8:Type><v8:Type>cfg:CatalogRef.Users</v8:Type>";
        for version in ["2.20", "2.21"] {
            let configuration = session_parameter_configuration(version, type_body);
            let validated = validate_configuration(&configuration).unwrap();
            let (graph, profile) = session_parameter_graph(&validated);
            let uuid = ObjectUuid::parse(SESSION_PARAMETER_UUID).unwrap();
            let first = compile_simple_metadata(&validated, &graph, uuid, &axes(version), &profile)
                .unwrap();
            let second =
                compile_simple_metadata(&validated, &graph, uuid, &axes(version), &profile)
                    .unwrap();
            assert_eq!(first, second);
            let plain =
                inflate_bounded(first.outcome().compiled_payload().unwrap().bytes()).unwrap();
            assert_eq!(
                plain,
                format!(
                    "{{1,\r\n{{1,\r\n{{2,\r\n{{3,\r\n{{1,0,{SESSION_PARAMETER_UUID}}},\"AuthorizedUser\",{{1,\"en\",\"Authorized user\"}},\"\",0,0,{NIL_UUID},0}},\r\n{{\"Pattern\",\r\n{{\"#\",{FIRST_CATALOG_REF_TYPE_ID}}},\r\n{{\"#\",{SECOND_CATALOG_REF_TYPE_ID}}}\r\n}}\r\n}}\r\n}},0}}"
                )
                .as_bytes()
            );
            let ir = decode_session_parameter_blob(
                first.outcome().compiled_payload().unwrap().bytes(),
                &profile,
            )
            .unwrap();
            assert_eq!(
                ir.types,
                [
                    SessionParameterTypeNativeIr::Reference(
                        ObjectUuid::parse(FIRST_CATALOG_REF_TYPE_ID).unwrap()
                    ),
                    SessionParameterTypeNativeIr::Reference(
                        ObjectUuid::parse(SECOND_CATALOG_REF_TYPE_ID).unwrap()
                    ),
                ]
            );
            let type_names = BTreeMap::from([
                (
                    ObjectUuid::parse(FIRST_CATALOG_REF_TYPE_ID).unwrap(),
                    "cfg:CatalogRef.ExternalUsers".to_owned(),
                ),
                (
                    ObjectUuid::parse(SECOND_CATALOG_REF_TYPE_ID).unwrap(),
                    "cfg:CatalogRef.Users".to_owned(),
                ),
            ]);
            let xml = ir
                .to_xml(
                    &ProfileId::parse(&format!("xml-{version}")).unwrap(),
                    &type_names,
                )
                .unwrap();
            let document = XmlReader::from_slice(&xml).unwrap();
            bundled_metadata_registry()
                .decode(
                    &FamilyId::parse("SessionParameter").unwrap(),
                    &document,
                    ProfileId::parse(&format!("xml-{version}")).unwrap(),
                    ObjectPath::root(),
                )
                .unwrap();
        }
    }

    #[test]
    fn session_parameter_builtin_and_variable_string_use_evidenced_pattern_items() {
        let type_body = "<v8:Type>v8:FixedArray</v8:Type><v8:Type>xs:string</v8:Type><v8:StringQualifiers><v8:Length>1</v8:Length><v8:AllowedLength>Variable</v8:AllowedLength></v8:StringQualifiers>";
        let configuration = session_parameter_configuration("2.20", type_body);
        let validated = validate_configuration(&configuration).unwrap();
        let (graph, profile) = session_parameter_graph(&validated);
        let entry = compile_simple_metadata(
            &validated,
            &graph,
            ObjectUuid::parse(SESSION_PARAMETER_UUID).unwrap(),
            &axes("2.20"),
            &profile,
        )
        .unwrap();
        let plain = inflate_bounded(entry.outcome().compiled_payload().unwrap().bytes()).unwrap();
        assert!(String::from_utf8(plain).unwrap().contains(&format!(
            "{{\"#\",{FIXED_ARRAY_TYPE_UUID}}},\r\n{{\"S\",1,1}}"
        )));
        let ir = decode_session_parameter_blob(
            entry.outcome().compiled_payload().unwrap().bytes(),
            &profile,
        )
        .unwrap();
        let xml = ir
            .to_xml(&ProfileId::parse("xml-2.21").unwrap(), &BTreeMap::new())
            .unwrap();
        let document = XmlReader::from_slice(&xml).unwrap();
        bundled_metadata_registry()
            .decode(
                &FamilyId::parse("SessionParameter").unwrap(),
                &document,
                ProfileId::parse("xml-2.21").unwrap(),
                ObjectPath::root(),
            )
            .unwrap();
    }

    #[test]
    fn session_parameter_does_not_guess_unresolved_generated_types() {
        let configuration =
            session_parameter_configuration("2.20", "<v8:Type>cfg:CatalogRef.Missing</v8:Type>");
        let validated = validate_configuration(&configuration).unwrap();
        let (graph, profile) = session_parameter_graph(&validated);
        assert!(matches!(
            compile_simple_metadata(
                &validated,
                &graph,
                ObjectUuid::parse(SESSION_PARAMETER_UUID).unwrap(),
                &axes("2.20"),
                &profile,
            ),
            Err(SimpleMetadataBuildError::InvalidModel {
                reason: "Type contains an unresolved readable generated type",
                ..
            })
        ));
    }

    #[test]
    fn session_parameter_decodes_observed_row_and_rejects_unknown_pattern_items() {
        let observed = format!(
            "{{1,\r\n{{1,\r\n{{2,\r\n{{3,\r\n{{1,0,5efc4bc4-b711-4620-8d2e-9d947c6cc141}},\"АвторизованныйПользователь\",\r\n{{1,\"ru\",\"Авторизованный пользователь\"}},\"\",0,0,{NIL_UUID},0}},\r\n{{\"Pattern\",\r\n{{\"#\",{FIRST_CATALOG_REF_TYPE_ID}}},\r\n{{\"#\",{SECOND_CATALOG_REF_TYPE_ID}}}\r\n}}\r\n}}\r\n}},0}}"
        );
        let ir = parse_session_parameter(observed.as_bytes()).unwrap();
        assert_eq!(ir.types.len(), 2);
        let unknown = observed.replace(
            &format!("{{\"#\",{FIRST_CATALOG_REF_TYPE_ID}}}"),
            "{\"Future\"}",
        );
        assert!(matches!(
            parse_session_parameter(unknown.as_bytes()),
            Err(SimpleMetadataBuildError::Native(_))
        ));
        let nil = observed.replace(FIRST_CATALOG_REF_TYPE_ID, NIL_UUID);
        assert!(matches!(
            parse_session_parameter(nil.as_bytes()),
            Err(SimpleMetadataBuildError::Native(_))
        ));
    }

    #[test]
    fn functional_option_roundtrips_references_without_a_base() {
        for version in ["2.20", "2.21"] {
            let configuration = functional_option_configuration(
                version,
                "Constant.FeatureState",
                "Catalog.Products",
            );
            let validated = validate_configuration(&configuration).unwrap();
            let (graph, profile) = functional_option_graph(&validated);
            let uuid = ObjectUuid::parse(FUNCTIONAL_OPTION_UUID).unwrap();
            let first = compile_simple_metadata(&validated, &graph, uuid, &axes(version), &profile)
                .unwrap();
            let second =
                compile_simple_metadata(&validated, &graph, uuid, &axes(version), &profile)
                    .unwrap();
            assert_eq!(first, second);
            let ir = decode_functional_option_blob(
                first.outcome().compiled_payload().unwrap().bytes(),
                &profile,
            )
            .unwrap();
            assert_eq!(ir.location, Some(ObjectUuid::parse(CONSTANT_UUID).unwrap()));
            assert_eq!(ir.content, [ObjectUuid::parse(CATALOG_UUID).unwrap()]);
            assert!(ir.privileged_get_mode);
            let references = BTreeMap::from([
                (
                    ObjectUuid::parse(CONSTANT_UUID).unwrap(),
                    "Constant.FeatureState".to_owned(),
                ),
                (
                    ObjectUuid::parse(CATALOG_UUID).unwrap(),
                    "Catalog.Products".to_owned(),
                ),
            ]);
            let xml = ir
                .to_xml(
                    &ProfileId::parse(&format!("xml-{version}")).unwrap(),
                    &references,
                )
                .unwrap();
            let document = XmlReader::from_slice(&xml).unwrap();
            bundled_metadata_registry()
                .decode(
                    &FamilyId::parse("FunctionalOption").unwrap(),
                    &document,
                    ProfileId::parse(&format!("xml-{version}")).unwrap(),
                    ObjectPath::root(),
                )
                .unwrap();
        }
    }

    #[test]
    fn functional_option_plaintext_matches_evidenced_layout() {
        let configuration =
            functional_option_configuration("2.20", "Constant.FeatureState", "Catalog.Products");
        let validated = validate_configuration(&configuration).unwrap();
        let (graph, profile) = functional_option_graph(&validated);
        let entry = compile_simple_metadata(
            &validated,
            &graph,
            ObjectUuid::parse(FUNCTIONAL_OPTION_UUID).unwrap(),
            &axes("2.20"),
            &profile,
        )
        .unwrap();
        let plain = inflate_bounded(entry.outcome().compiled_payload().unwrap().bytes()).unwrap();
        assert_eq!(
            plain,
            format!(
                "{{1,\r\n{{2,\r\n{{3,\r\n{{1,0,{FUNCTIONAL_OPTION_UUID}}},\"UseFeature\",{{1,\"en\",\"Use feature\"}},\"\",0,0,{NIL_UUID},0}},{CONSTANT_UUID},\r\n{{0,1,\r\n{{\"#\",{FUNCTIONAL_OPTION_CONTENT_CLASS_UUID},\r\n{{0,\r\n{{\"#\",{DESIGN_TIME_REFERENCE_CLASS_UUID},\r\n{{1,{CATALOG_UUID}}}\r\n}}\r\n}}\r\n}}\r\n}},1}},0}}"
            )
            .as_bytes()
        );
    }

    #[test]
    fn functional_option_does_not_guess_unresolved_references() {
        let configuration =
            functional_option_configuration("2.20", "Constant.Missing", "Catalog.Products");
        let validated = validate_configuration(&configuration).unwrap();
        let (graph, profile) = functional_option_graph(&validated);
        assert!(matches!(
            compile_simple_metadata(
                &validated,
                &graph,
                ObjectUuid::parse(FUNCTIONAL_OPTION_UUID).unwrap(),
                &axes("2.20"),
                &profile,
            ),
            Err(SimpleMetadataBuildError::InvalidModel {
                reason: "Location contains an unresolved readable reference",
                ..
            })
        ));
    }

    #[test]
    fn functional_option_decodes_the_observed_empty_content_row_shape() {
        let plain = format!(
            "{{1,\r\n{{2,\r\n{{3,\r\n{{1,0,46e0522b-b88b-43b2-9067-a5359e90994b}},\"АктуальнаяДатаОбновленияПовторноИспользуемыхЗначенийМРО\",\r\n{{1,\"ru\",\"Актуальная дата обновления повторно используемых значений МРО\"}},\"\",0,0,{NIL_UUID},0}},94e08dcf-3885-4a73-82ab-cb291f1cf95f,\r\n{{0,0}},1}},0}}"
        );
        let ir = parse_functional_option(plain.as_bytes()).unwrap();
        assert_eq!(
            ir.uuid,
            ObjectUuid::parse("46e0522b-b88b-43b2-9067-a5359e90994b").unwrap()
        );
        assert_eq!(
            ir.location,
            Some(ObjectUuid::parse("94e08dcf-3885-4a73-82ab-cb291f1cf95f").unwrap())
        );
        assert!(ir.content.is_empty());
        assert!(ir.privileged_get_mode);
    }

    #[test]
    fn functional_options_parameter_roundtrips_references_without_a_base() {
        for version in ["2.20", "2.21"] {
            let configuration = functional_options_parameter_configuration(version);
            let validated = validate_configuration(&configuration).unwrap();
            let (graph, profile) = functional_options_parameter_graph(&validated);
            let uuid = ObjectUuid::parse(FUNCTIONAL_OPTIONS_PARAMETER_UUID).unwrap();
            let entry = compile_simple_metadata(&validated, &graph, uuid, &axes(version), &profile)
                .unwrap();
            assert_eq!(
                entry.target().key().as_str(),
                FUNCTIONAL_OPTIONS_PARAMETER_UUID
            );
            let payload = entry.outcome().compiled_payload().unwrap();
            let ir = decode_functional_options_parameter_blob(payload.bytes(), &profile).unwrap();
            assert_eq!(ir.uses, [ObjectUuid::parse(CATALOG_UUID).unwrap()]);
            let references = BTreeMap::from([(
                ObjectUuid::parse(CATALOG_UUID).unwrap(),
                "Catalog.Products".to_owned(),
            )]);
            let xml = ir
                .to_xml(
                    &ProfileId::parse(&format!("xml-{version}")).unwrap(),
                    &references,
                )
                .unwrap();
            let document = XmlReader::from_slice(&xml).unwrap();
            bundled_metadata_registry()
                .decode(
                    &FamilyId::parse("FunctionalOptionsParameter").unwrap(),
                    &document,
                    ProfileId::parse(&format!("xml-{version}")).unwrap(),
                    ObjectPath::root(),
                )
                .unwrap();
        }
    }

    #[test]
    fn functional_options_parameter_plaintext_matches_evidenced_golden() {
        let configuration = functional_options_parameter_configuration("2.20");
        let validated = validate_configuration(&configuration).unwrap();
        let (graph, profile) = functional_options_parameter_graph(&validated);
        let entry = compile_simple_metadata(
            &validated,
            &graph,
            ObjectUuid::parse(FUNCTIONAL_OPTIONS_PARAMETER_UUID).unwrap(),
            &axes("2.20"),
            &profile,
        )
        .unwrap();
        let plain = inflate_bounded(entry.outcome().compiled_payload().unwrap().bytes()).unwrap();
        assert_eq!(
            plain,
            format!(
                "{{1,\r\n{{0,\r\n{{3,\r\n{{1,0,{FUNCTIONAL_OPTIONS_PARAMETER_UUID}}},\"UseFeatureFor\",{{1,\"en\",\"Use feature for\"}},\"\",0,0,{NIL_UUID},0}},\r\n{{0,1,\r\n{{\"#\",{DESIGN_TIME_REFERENCE_CLASS_UUID},\r\n{{1,{CATALOG_UUID}}}\r\n}}\r\n}}\r\n}},0}}"
            )
            .as_bytes()
        );
    }

    #[test]
    fn functional_options_parameter_does_not_guess_unresolved_references() {
        let configuration =
            functional_options_parameter_configuration_with_reference("2.20", "Catalog.Missing");
        let validated = validate_configuration(&configuration).unwrap();
        let (graph, profile) = functional_options_parameter_graph(&validated);
        assert!(matches!(
            compile_simple_metadata(
                &validated,
                &graph,
                ObjectUuid::parse(FUNCTIONAL_OPTIONS_PARAMETER_UUID).unwrap(),
                &axes("2.20"),
                &profile,
            ),
            Err(SimpleMetadataBuildError::InvalidModel {
                reason: "Use contains an unresolved readable reference",
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
    fn functional_options_parameter_rejects_an_unknown_native_reference_class() {
        let configuration = functional_options_parameter_configuration("2.20");
        let validated = validate_configuration(&configuration).unwrap();
        let (graph, profile) = functional_options_parameter_graph(&validated);
        let entry = compile_simple_metadata(
            &validated,
            &graph,
            ObjectUuid::parse(FUNCTIONAL_OPTIONS_PARAMETER_UUID).unwrap(),
            &axes("2.20"),
            &profile,
        )
        .unwrap();
        let plain = inflate_bounded(entry.outcome().compiled_payload().unwrap().bytes()).unwrap();
        let malformed = String::from_utf8(plain).unwrap().replace(
            DESIGN_TIME_REFERENCE_CLASS_UUID,
            "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
        );
        let malformed = raw_deflate(malformed.as_bytes()).unwrap();
        assert!(matches!(
            decode_functional_options_parameter_blob(&malformed, &profile),
            Err(SimpleMetadataBuildError::Native(_))
        ));
    }

    #[test]
    fn functional_option_rejects_an_unknown_native_content_class() {
        let configuration =
            functional_option_configuration("2.20", "Constant.FeatureState", "Catalog.Products");
        let validated = validate_configuration(&configuration).unwrap();
        let (graph, profile) = functional_option_graph(&validated);
        let entry = compile_simple_metadata(
            &validated,
            &graph,
            ObjectUuid::parse(FUNCTIONAL_OPTION_UUID).unwrap(),
            &axes("2.20"),
            &profile,
        )
        .unwrap();
        let plain = inflate_bounded(entry.outcome().compiled_payload().unwrap().bytes()).unwrap();
        let malformed = String::from_utf8(plain).unwrap().replace(
            FUNCTIONAL_OPTION_CONTENT_CLASS_UUID,
            "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
        );
        let malformed = raw_deflate(malformed.as_bytes()).unwrap();
        assert!(matches!(
            decode_functional_option_blob(&malformed, &profile),
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
