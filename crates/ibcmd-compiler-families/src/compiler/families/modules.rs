//! Standalone native codec for `CommonModule` metadata.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use ibcmd_core::artifact::{ProfileId, StorageProfileId};
use ibcmd_core::identity::ObjectUuid;
use ibcmd_core::model::CanonicalObject;
use ibcmd_core::profile::EffectiveProfile;
use ibcmd_core::storage::{
    MultipartIdentity, StorageBuildError, StoragePatchBuildError, StoragePatchEntry,
    StoragePatchOutcome, StoragePatchTarget, StorageProvenance,
};
use ibcmd_core::validate::ValidatedConfiguration;
use ibcmd_core::value::{CanonicalField, CanonicalValue, CanonicalValueKind};
use ibcmd_core::version::PlatformBuild;

use super::super::CompileAxes;
use super::super::graph::BootstrapGraph;
use super::native::{
    NativeMetadataHeader, NativeValue, exact_list, exact_token, inflate_and_parse, metadata_header,
    parse_bool_token, parse_metadata_header, raw_deflate, serialize, styled_list, token,
};

const LAYOUT_KEY: &str = "bootstrap.metadata.common_module.layout";
const LAYOUT: &str = "common-module-v1-crlf-utf8-bom";
const SUPPORTED_STORAGE_PROFILE: &str = "storage:mssql-config-configsave";

const PROPERTY_SCHEMA: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "Global",
    "ClientManagedApplication",
    "Server",
    "ExternalConnection",
    "ClientOrdinaryApplication",
    "ServerCall",
    "Privileged",
    "ReturnValuesReuse",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommonModuleProfile {
    profile_id: ProfileId,
    platform_build: PlatformBuild,
    storage_profile: StorageProfileId,
}

impl CommonModuleProfile {
    pub fn from_effective(profile: &EffectiveProfile) -> Result<Self, CommonModuleProfileError> {
        let platform_build = profile
            .platform_build
            .as_ref()
            .map(|value| value.value.clone())
            .ok_or_else(|| CommonModuleProfileError::MissingCoordinate {
                profile: profile.id.clone(),
                coordinate: "platform_build",
            })?;
        let storage_profile = profile
            .storage_profile
            .as_ref()
            .map(|value| value.value.clone())
            .ok_or_else(|| CommonModuleProfileError::MissingCoordinate {
                profile: profile.id.clone(),
                coordinate: "storage_profile",
            })?;
        if storage_profile.as_str() != SUPPORTED_STORAGE_PROFILE {
            return Err(CommonModuleProfileError::UnsupportedCoordinate {
                profile: profile.id.clone(),
                coordinate: "storage_profile",
                value: storage_profile.to_string(),
            });
        }
        let layout = profile.constants.get(LAYOUT_KEY).ok_or_else(|| {
            CommonModuleProfileError::MissingConstant {
                profile: profile.id.clone(),
                key: LAYOUT_KEY,
            }
        })?;
        if layout.value != LAYOUT {
            return Err(CommonModuleProfileError::UnsupportedLayout {
                profile: profile.id.clone(),
                value: layout.value.clone(),
            });
        }
        Ok(Self {
            profile_id: profile.id.clone(),
            platform_build,
            storage_profile,
        })
    }

    pub const fn profile_id(&self) -> &ProfileId {
        &self.profile_id
    }

    #[cfg(test)]
    fn fixture() -> Self {
        Self {
            profile_id: ProfileId::parse("platform-8.3.27.1989").unwrap(),
            platform_build: PlatformBuild::parse("8.3.27.1989").unwrap(),
            storage_profile: StorageProfileId::parse(SUPPORTED_STORAGE_PROFILE).unwrap(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommonModuleProfileError {
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
        value: String,
    },
}

impl Display for CommonModuleProfileError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCoordinate {
                profile,
                coordinate,
            } => write!(
                formatter,
                "profile `{profile}` has no `{coordinate}` coordinate"
            ),
            Self::MissingConstant { profile, key } => {
                write!(formatter, "profile `{profile}` has no `{key}` constant")
            }
            Self::UnsupportedCoordinate {
                profile,
                coordinate,
                value,
            } => write!(
                formatter,
                "profile `{profile}` declares unsupported `{coordinate}` value `{value}`"
            ),
            Self::UnsupportedLayout { profile, value } => write!(
                formatter,
                "profile `{profile}` declares unsupported `{LAYOUT_KEY}={value}`"
            ),
        }
    }
}

impl Error for CommonModuleProfileError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReturnValuesReuse {
    DontUse,
    DuringRequest,
    DuringSession,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommonModuleNativeIr {
    pub uuid: ObjectUuid,
    pub name: String,
    pub synonyms: Vec<(String, String)>,
    pub comment: String,
    pub global: bool,
    pub client_managed_application: bool,
    pub server: bool,
    pub external_connection: bool,
    pub client_ordinary_application: bool,
    pub server_call: bool,
    pub privileged: bool,
    pub return_values_reuse: ReturnValuesReuse,
}

pub fn compile_common_module_metadata(
    validated: &ValidatedConfiguration<'_>,
    graph: &BootstrapGraph,
    object_uuid: ObjectUuid,
    axes: &CompileAxes,
    profile: &CommonModuleProfile,
) -> Result<StoragePatchEntry, CommonModuleBuildError> {
    validate_coordinates(graph, axes, profile)?;
    let object_index = validated
        .graph()
        .object_index_by_uuid(object_uuid)
        .ok_or(CommonModuleBuildError::UnknownObject(object_uuid))?;
    let object = &validated.configuration().objects()[object_index];
    let ir = project_object(object)?;
    let route = graph
        .primary_object_entry(object_uuid)
        .ok_or(CommonModuleBuildError::MissingPrimaryRoute(object_uuid))?;
    let bytes = encode_common_module_blob(&ir, profile)?;
    let provenance = StorageProvenance::new(&format!(
        "bootstrap:{}:metadata:CommonModule",
        profile.profile_id
    ))?;
    Ok(StoragePatchEntry::new(
        StoragePatchTarget::new(route.key().clone(), MultipartIdentity::single(), provenance),
        StoragePatchOutcome::compiled(bytes)?,
    ))
}

pub fn encode_common_module_blob(
    value: &CommonModuleNativeIr,
    _profile: &CommonModuleProfile,
) -> Result<Vec<u8>, CommonModuleBuildError> {
    raw_deflate(&build_native(value)).map_err(native_error)
}

pub fn common_module_plaintext(
    value: &CommonModuleNativeIr,
    _profile: &CommonModuleProfile,
) -> Result<Vec<u8>, CommonModuleBuildError> {
    serialize(&build_native(value)).map_err(native_error)
}

pub fn decode_common_module_blob(
    blob: &[u8],
    _profile: &CommonModuleProfile,
) -> Result<CommonModuleNativeIr, CommonModuleBuildError> {
    let root = inflate_and_parse(blob).map_err(native_error)?;
    parse_native(&root)
}

fn build_native(value: &CommonModuleNativeIr) -> NativeValue {
    let reuse = match value.return_values_reuse {
        ReturnValuesReuse::DontUse => "0",
        ReturnValuesReuse::DuringRequest => "1",
        ReturnValuesReuse::DuringSession => "2",
    };
    styled_list(
        vec![
            token("1"),
            styled_list(
                vec![
                    token("12"),
                    metadata_header(&NativeMetadataHeader {
                        uuid: value.uuid,
                        name: value.name.clone(),
                        synonyms: value.synonyms.clone(),
                        comment: value.comment.clone(),
                    }),
                    bool_token(value.client_ordinary_application),
                    bool_token(value.server),
                    bool_token(value.external_connection),
                    bool_token(value.privileged),
                    bool_token(value.global),
                    bool_token(value.client_managed_application),
                    token(reuse),
                    bool_token(value.server_call),
                ],
                vec![1],
            ),
            token("0"),
        ],
        vec![1],
    )
}

fn parse_native(root: &NativeValue) -> Result<CommonModuleNativeIr, CommonModuleBuildError> {
    let root = exact_list(root, 3, "CommonModule root").map_err(native_error)?;
    exact_token(&root[0], "1", "CommonModule root marker").map_err(native_error)?;
    exact_token(&root[2], "0", "CommonModule root tail").map_err(native_error)?;
    let object = exact_list(&root[1], 10, "CommonModule object").map_err(native_error)?;
    exact_token(&object[0], "12", "CommonModule object marker").map_err(native_error)?;
    let header = parse_metadata_header(&object[1]).map_err(native_error)?;
    let reuse = match object[8].as_token() {
        Some("0") => ReturnValuesReuse::DontUse,
        Some("1") => ReturnValuesReuse::DuringRequest,
        Some("2") => ReturnValuesReuse::DuringSession,
        _ => {
            return Err(CommonModuleBuildError::Native(
                "invalid ReturnValuesReuse code".into(),
            ));
        }
    };
    Ok(CommonModuleNativeIr {
        uuid: header.uuid,
        name: header.name,
        synonyms: header.synonyms,
        comment: header.comment,
        client_ordinary_application: parse_bool_token(&object[2], "ClientOrdinaryApplication")
            .map_err(native_error)?,
        server: parse_bool_token(&object[3], "Server").map_err(native_error)?,
        external_connection: parse_bool_token(&object[4], "ExternalConnection")
            .map_err(native_error)?,
        privileged: parse_bool_token(&object[5], "Privileged").map_err(native_error)?,
        global: parse_bool_token(&object[6], "Global").map_err(native_error)?,
        client_managed_application: parse_bool_token(&object[7], "ClientManagedApplication")
            .map_err(native_error)?,
        return_values_reuse: reuse,
        server_call: parse_bool_token(&object[9], "ServerCall").map_err(native_error)?,
    })
}

fn project_object(
    object: &CanonicalObject,
) -> Result<CommonModuleNativeIr, CommonModuleBuildError> {
    if object.kind().as_str() != "CommonModule" {
        return invalid_model(object, "metadata kind is not CommonModule");
    }
    if object.owner().is_some()
        || !object.references().is_empty()
        || !object.generated_types().is_empty()
        || !object.assets().is_empty()
    {
        return invalid_model(
            object,
            "CommonModule ownership/reference inventory is not empty",
        );
    }
    require_property_schema(object, PROPERTY_SCHEMA)?;
    Ok(CommonModuleNativeIr {
        uuid: object.identity().uuid(),
        name: text_property(object, "Name")?.to_owned(),
        synonyms: localized_property(object, "Synonym", "lang")?,
        comment: text_property(object, "Comment")?.to_owned(),
        global: bool_property(object, "Global")?,
        client_managed_application: bool_property(object, "ClientManagedApplication")?,
        server: bool_property(object, "Server")?,
        external_connection: bool_property(object, "ExternalConnection")?,
        client_ordinary_application: bool_property(object, "ClientOrdinaryApplication")?,
        server_call: bool_property(object, "ServerCall")?,
        privileged: bool_property(object, "Privileged")?,
        return_values_reuse: match enum_property(object, "ReturnValuesReuse")? {
            "DontUse" => ReturnValuesReuse::DontUse,
            "DuringRequest" => ReturnValuesReuse::DuringRequest,
            "DuringSession" => ReturnValuesReuse::DuringSession,
            _ => return invalid_model(object, "ReturnValuesReuse has no evidenced native code"),
        },
    })
}

fn validate_coordinates(
    graph: &BootstrapGraph,
    axes: &CompileAxes,
    profile: &CommonModuleProfile,
) -> Result<(), CommonModuleBuildError> {
    if graph.profile_id() != profile.profile_id() {
        return Err(CommonModuleBuildError::ProfileMismatch {
            graph: graph.profile_id().clone(),
            codec: profile.profile_id.clone(),
        });
    }
    if axes.platform_build() != Some(&profile.platform_build) {
        return Err(CommonModuleBuildError::AxisMismatch("platform_build"));
    }
    if axes.storage_profile() != &profile.storage_profile {
        return Err(CommonModuleBuildError::AxisMismatch("storage_profile"));
    }
    if axes.compatibility_mode().is_some() || axes.container_revision().is_some() {
        return Err(CommonModuleBuildError::AxisMismatch(
            "unevidenced optional coordinate",
        ));
    }
    if !matches!(axes.xml_dialect().to_string().as_str(), "2.20" | "2.21") {
        return Err(CommonModuleBuildError::AxisMismatch("xml_dialect"));
    }
    Ok(())
}

fn require_property_schema(
    object: &CanonicalObject,
    expected: &[&str],
) -> Result<(), CommonModuleBuildError> {
    if object.properties().len() != expected.len()
        || object
            .properties()
            .iter()
            .zip(expected)
            .any(|(field, expected)| field.name().as_str() != *expected)
    {
        return invalid_model(object, "canonical property schema is not exact");
    }
    Ok(())
}

fn property<'a>(
    object: &'a CanonicalObject,
    name: &str,
) -> Result<&'a CanonicalValue, CommonModuleBuildError> {
    object
        .properties()
        .iter()
        .find(|field| field.name().as_str() == name)
        .map(CanonicalField::value)
        .ok_or(CommonModuleBuildError::InvalidModel {
            object: object.identity().uuid(),
            reason: "required typed property is missing",
        })
}

fn text_property<'a>(
    object: &'a CanonicalObject,
    name: &str,
) -> Result<&'a str, CommonModuleBuildError> {
    match property(object, name)?.kind() {
        CanonicalValueKind::Text(value) => Ok(value.as_str()),
        _ => invalid_model(object, "typed property is not text"),
    }
}

fn bool_property(object: &CanonicalObject, name: &str) -> Result<bool, CommonModuleBuildError> {
    match property(object, name)?.kind() {
        CanonicalValueKind::Bool(value) => Ok(value),
        _ => invalid_model(object, "typed property is not boolean"),
    }
}

fn enum_property<'a>(
    object: &'a CanonicalObject,
    name: &str,
) -> Result<&'a str, CommonModuleBuildError> {
    match property(object, name)?.kind() {
        CanonicalValueKind::EnumToken(value) => Ok(value.as_str()),
        _ => invalid_model(object, "typed property is not an enum token"),
    }
}

fn localized_property(
    object: &CanonicalObject,
    name: &str,
    language_field: &str,
) -> Result<Vec<(String, String)>, CommonModuleBuildError> {
    let values =
        property(object, name)?
            .as_sequence()
            .ok_or(CommonModuleBuildError::InvalidModel {
                object: object.identity().uuid(),
                reason: "localized property is not a sequence",
            })?;
    let mut output = Vec::with_capacity(values.len());
    let mut languages = BTreeSet::new();
    for value in values {
        let fields = value
            .as_record()
            .ok_or(CommonModuleBuildError::InvalidModel {
                object: object.identity().uuid(),
                reason: "localized item is not a record",
            })?;
        if fields.len() != 2
            || fields[0].name().as_str() != language_field
            || fields[1].name().as_str() != "content"
        {
            return invalid_model(object, "localized item schema is not exact");
        }
        let language = match fields[0].value().kind() {
            CanonicalValueKind::Text(value) => value.as_str(),
            _ => return invalid_model(object, "localized language is not text"),
        };
        let content = match fields[1].value().kind() {
            CanonicalValueKind::Text(value) => value.as_str(),
            _ => return invalid_model(object, "localized content is not text"),
        };
        if !languages.insert(language) {
            return invalid_model(object, "localized language is duplicated");
        }
        output.push((language.to_owned(), content.to_owned()));
    }
    Ok(output)
}

fn bool_token(value: bool) -> NativeValue {
    token(if value { "1" } else { "0" })
}

fn invalid_model<T>(
    object: &CanonicalObject,
    reason: &'static str,
) -> Result<T, CommonModuleBuildError> {
    Err(CommonModuleBuildError::InvalidModel {
        object: object.identity().uuid(),
        reason,
    })
}

fn native_error(error: impl Display) -> CommonModuleBuildError {
    CommonModuleBuildError::Native(error.to_string())
}

#[derive(Debug)]
pub enum CommonModuleBuildError {
    Profile(CommonModuleProfileError),
    ProfileMismatch {
        graph: ProfileId,
        codec: ProfileId,
    },
    AxisMismatch(&'static str),
    UnknownObject(ObjectUuid),
    MissingPrimaryRoute(ObjectUuid),
    InvalidModel {
        object: ObjectUuid,
        reason: &'static str,
    },
    Native(String),
    Storage(StorageBuildError),
    Patch(StoragePatchBuildError),
}

impl Display for CommonModuleBuildError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Profile(source) => Display::fmt(source, formatter),
            Self::ProfileMismatch { graph, codec } => {
                write!(
                    formatter,
                    "graph profile `{graph}` differs from codec `{codec}`"
                )
            }
            Self::AxisMismatch(axis) => write!(formatter, "CommonModule `{axis}` axis mismatch"),
            Self::UnknownObject(uuid) => write!(formatter, "validated graph has no object {uuid}"),
            Self::MissingPrimaryRoute(uuid) => {
                write!(formatter, "bootstrap graph has no primary row for {uuid}")
            }
            Self::InvalidModel { object, reason } => {
                write!(
                    formatter,
                    "CommonModule {object} is not compilable: {reason}"
                )
            }
            Self::Native(reason) => write!(formatter, "invalid CommonModule native row: {reason}"),
            Self::Storage(source) => Display::fmt(source, formatter),
            Self::Patch(source) => Display::fmt(source, formatter),
        }
    }
}

impl Error for CommonModuleBuildError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Profile(source) => Some(source),
            Self::Storage(source) => Some(source),
            Self::Patch(source) => Some(source),
            _ => None,
        }
    }
}

impl From<CommonModuleProfileError> for CommonModuleBuildError {
    fn from(source: CommonModuleProfileError) -> Self {
        Self::Profile(source)
    }
}

impl From<StorageBuildError> for CommonModuleBuildError {
    fn from(source: StorageBuildError) -> Self {
        Self::Storage(source)
    }
}

impl From<StoragePatchBuildError> for CommonModuleBuildError {
    fn from(source: StoragePatchBuildError) -> Self {
        Self::Patch(source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> CommonModuleNativeIr {
        CommonModuleNativeIr {
            uuid: ObjectUuid::parse("db63155a-3df1-44a7-aabc-dd906b877c3f").unwrap(),
            name: "БТС".to_owned(),
            synonyms: vec![("ru".to_owned(), "БТС".to_owned())],
            comment: String::new(),
            global: false,
            client_managed_application: false,
            server: true,
            external_connection: true,
            client_ordinary_application: true,
            server_call: false,
            privileged: false,
            return_values_reuse: ReturnValuesReuse::DontUse,
        }
    }

    #[test]
    fn representative_plaintext_is_byte_exact_and_roundtrips() {
        let profile = CommonModuleProfile::fixture();
        let value = fixture();
        let expected = concat!(
            "\u{feff}{1,\r\n{12,\r\n{3,\r\n{1,0,",
            "db63155a-3df1-44a7-aabc-dd906b877c3f},\"БТС\",\r\n",
            "{1,\"ru\",\"БТС\"},\"\",0,0,",
            "00000000-0000-0000-0000-000000000000,0},1,1,1,0,0,0,0,0},0}"
        );
        assert_eq!(
            common_module_plaintext(&value, &profile).unwrap(),
            expected.as_bytes()
        );
        let blob = encode_common_module_blob(&value, &profile).unwrap();
        assert_eq!(decode_common_module_blob(&blob, &profile).unwrap(), value);
    }
}
