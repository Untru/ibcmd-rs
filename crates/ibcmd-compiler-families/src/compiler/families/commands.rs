//! Standalone native codecs for common commands, command groups and pictures.

use std::collections::{BTreeMap, BTreeSet};
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
    NativeMetadataHeader, NativeValue, exact_list, exact_token, inflate_and_parse, inline_list,
    localized, metadata_header, parse_bool_token, parse_localized, parse_metadata_header,
    raw_deflate, required_text, required_token, serialize, styled_list, styled_list_with_tail,
    text, token,
};

const SUPPORTED_STORAGE_PROFILE: &str = "storage:mssql-config-configsave";
const COMMAND_VALUE_UUID: &str = "078a6af8-d22c-4248-9c33-7e90075a3d2c";

const COMMON_COMMAND_PROPERTIES: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "Group",
    "Representation",
    "ToolTip",
    "PictureReference",
    "PictureLoadTransparent",
    "IncludeHelpInContents",
    "CommandParameterType",
    "ParameterUseMode",
    "ModifiesData",
    "OnMainServerUnavalableBehavior",
];
const COMMAND_GROUP_PROPERTIES: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "Representation",
    "ToolTip",
    "PictureReference",
    "PictureLoadTransparent",
    "Category",
];
const COMMON_PICTURE_PROPERTIES: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "AvailabilityForChoice",
    "AvailabilityForAppearance",
];

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CommandMetadataFamily {
    CommonCommand,
    CommandGroup,
    CommonPicture,
}

impl CommandMetadataFamily {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CommonCommand => "CommonCommand",
            Self::CommandGroup => "CommandGroup",
            Self::CommonPicture => "CommonPicture",
        }
    }

    const fn layout_key(self) -> &'static str {
        match self {
            Self::CommonCommand => "bootstrap.metadata.common_command.layout",
            Self::CommandGroup => "bootstrap.metadata.command_group.layout",
            Self::CommonPicture => "bootstrap.metadata.common_picture.layout",
        }
    }

    const fn layout(self) -> &'static str {
        match self {
            Self::CommonCommand => "common-command-v1-crlf-utf8-bom",
            Self::CommandGroup => "command-group-v1-crlf-utf8-bom",
            Self::CommonPicture => "common-picture-v1-crlf-utf8-bom",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandMetadataProfile {
    profile_id: ProfileId,
    platform_build: PlatformBuild,
    storage_profile: StorageProfileId,
    family: CommandMetadataFamily,
}

impl CommandMetadataProfile {
    pub fn from_effective_for_family(
        profile: &EffectiveProfile,
        family: CommandMetadataFamily,
    ) -> Result<Self, CommandMetadataProfileError> {
        let platform_build = profile
            .platform_build
            .as_ref()
            .map(|value| value.value.clone())
            .ok_or_else(|| CommandMetadataProfileError::MissingCoordinate {
                profile: profile.id.clone(),
                coordinate: "platform_build",
            })?;
        let storage_profile = profile
            .storage_profile
            .as_ref()
            .map(|value| value.value.clone())
            .ok_or_else(|| CommandMetadataProfileError::MissingCoordinate {
                profile: profile.id.clone(),
                coordinate: "storage_profile",
            })?;
        if storage_profile.as_str() != SUPPORTED_STORAGE_PROFILE {
            return Err(CommandMetadataProfileError::UnsupportedCoordinate {
                profile: profile.id.clone(),
                coordinate: "storage_profile",
                value: storage_profile.to_string(),
            });
        }
        let value = profile.constants.get(family.layout_key()).ok_or_else(|| {
            CommandMetadataProfileError::MissingConstant {
                profile: profile.id.clone(),
                key: family.layout_key(),
            }
        })?;
        if value.value != family.layout() {
            return Err(CommandMetadataProfileError::UnsupportedLayout {
                profile: profile.id.clone(),
                family,
                value: value.value.clone(),
            });
        }
        Ok(Self {
            profile_id: profile.id.clone(),
            platform_build,
            storage_profile,
            family,
        })
    }

    pub const fn family(&self) -> CommandMetadataFamily {
        self.family
    }

    #[cfg(test)]
    fn fixture(family: CommandMetadataFamily) -> Self {
        Self {
            profile_id: ProfileId::parse("platform-8.3.27.1989").unwrap(),
            platform_build: PlatformBuild::parse("8.3.27.1989").unwrap(),
            storage_profile: StorageProfileId::parse(SUPPORTED_STORAGE_PROFILE).unwrap(),
            family,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandMetadataProfileError {
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
        family: CommandMetadataFamily,
        value: String,
    },
}

impl Display for CommandMetadataProfileError {
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
            Self::UnsupportedLayout {
                profile,
                family,
                value,
            } => write!(
                formatter,
                "profile `{profile}` declares unsupported `{}` layout `{value}`",
                family.as_str()
            ),
        }
    }
}

impl Error for CommandMetadataProfileError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativePicture {
    Empty,
    Code {
        code: i32,
        load_transparent: bool,
    },
    Uuid {
        uuid: ObjectUuid,
        load_transparent: bool,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandParameterType {
    Empty,
    DefinedType(ObjectUuid),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommonCommandNativeIr {
    pub uuid: ObjectUuid,
    pub name: String,
    pub synonyms: Vec<(String, String)>,
    pub comment: String,
    pub group: ObjectUuid,
    pub representation: u8,
    pub tooltip: Vec<(String, String)>,
    pub picture: NativePicture,
    pub include_help_in_contents: bool,
    pub command_parameter_type: CommandParameterType,
    pub parameter_use_mode: u8,
    pub modifies_data: bool,
    pub on_main_server_unavailable_behavior: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandGroupNativeIr {
    pub uuid: ObjectUuid,
    pub name: String,
    pub synonyms: Vec<(String, String)>,
    pub comment: String,
    pub representation: u8,
    pub tooltip: Vec<(String, String)>,
    pub picture: NativePicture,
    pub category: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommonPictureNativeIr {
    pub uuid: ObjectUuid,
    pub name: String,
    pub synonyms: Vec<(String, String)>,
    pub comment: String,
    pub availability_for_choice: bool,
    pub availability_for_appearance: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandMetadataNativeIr {
    CommonCommand(CommonCommandNativeIr),
    CommandGroup(CommandGroupNativeIr),
    CommonPicture(CommonPictureNativeIr),
}

pub fn compile_command_metadata(
    validated: &ValidatedConfiguration<'_>,
    graph: &BootstrapGraph,
    object_uuid: ObjectUuid,
    axes: &CompileAxes,
    profile: &CommandMetadataProfile,
) -> Result<StoragePatchEntry, CommandMetadataBuildError> {
    validate_coordinates(graph, axes, profile)?;
    let object_index = validated
        .graph()
        .object_index_by_uuid(object_uuid)
        .ok_or(CommandMetadataBuildError::UnknownObject(object_uuid))?;
    let object = &validated.configuration().objects()[object_index];
    let indexes = ReferenceIndexes::build(validated, object_uuid)?;
    let value = match profile.family {
        CommandMetadataFamily::CommonCommand => {
            CommandMetadataNativeIr::CommonCommand(project_common_command(object, &indexes)?)
        }
        CommandMetadataFamily::CommandGroup => {
            CommandMetadataNativeIr::CommandGroup(project_command_group(object, &indexes)?)
        }
        CommandMetadataFamily::CommonPicture => {
            CommandMetadataNativeIr::CommonPicture(project_common_picture(object)?)
        }
    };
    let route = graph
        .primary_object_entry(object_uuid)
        .ok_or(CommandMetadataBuildError::MissingPrimaryRoute(object_uuid))?;
    let bytes = encode_command_metadata_blob(&value, profile)?;
    let provenance = StorageProvenance::new(&format!(
        "bootstrap:{}:metadata:{}",
        profile.profile_id,
        profile.family.as_str()
    ))?;
    Ok(StoragePatchEntry::new(
        StoragePatchTarget::new(route.key().clone(), MultipartIdentity::single(), provenance),
        StoragePatchOutcome::compiled(bytes)?,
    ))
}

pub fn encode_command_metadata_blob(
    value: &CommandMetadataNativeIr,
    profile: &CommandMetadataProfile,
) -> Result<Vec<u8>, CommandMetadataBuildError> {
    ensure_family(value, profile.family)?;
    raw_deflate(&build_native(value)).map_err(native_error)
}

pub fn command_metadata_plaintext(
    value: &CommandMetadataNativeIr,
    profile: &CommandMetadataProfile,
) -> Result<Vec<u8>, CommandMetadataBuildError> {
    ensure_family(value, profile.family)?;
    serialize(&build_native(value)).map_err(native_error)
}

pub fn decode_command_metadata_blob(
    blob: &[u8],
    profile: &CommandMetadataProfile,
) -> Result<CommandMetadataNativeIr, CommandMetadataBuildError> {
    let root = inflate_and_parse(blob).map_err(native_error)?;
    match profile.family {
        CommandMetadataFamily::CommonCommand => {
            parse_common_command(&root).map(CommandMetadataNativeIr::CommonCommand)
        }
        CommandMetadataFamily::CommandGroup => {
            parse_command_group(&root).map(CommandMetadataNativeIr::CommandGroup)
        }
        CommandMetadataFamily::CommonPicture => {
            parse_common_picture(&root).map(CommandMetadataNativeIr::CommonPicture)
        }
    }
}

fn ensure_family(
    value: &CommandMetadataNativeIr,
    family: CommandMetadataFamily,
) -> Result<(), CommandMetadataBuildError> {
    let actual = match value {
        CommandMetadataNativeIr::CommonCommand(_) => CommandMetadataFamily::CommonCommand,
        CommandMetadataNativeIr::CommandGroup(_) => CommandMetadataFamily::CommandGroup,
        CommandMetadataNativeIr::CommonPicture(_) => CommandMetadataFamily::CommonPicture,
    };
    if actual != family {
        return Err(CommandMetadataBuildError::FamilyMismatch {
            expected: family,
            actual,
        });
    }
    Ok(())
}

fn build_native(value: &CommandMetadataNativeIr) -> NativeValue {
    match value {
        CommandMetadataNativeIr::CommonCommand(value) => build_common_command(value),
        CommandMetadataNativeIr::CommandGroup(value) => build_command_group(value),
        CommandMetadataNativeIr::CommonPicture(value) => build_common_picture(value),
    }
}

fn build_common_command(value: &CommonCommandNativeIr) -> NativeValue {
    let parameter = match value.command_parameter_type {
        CommandParameterType::Empty => inline_list(vec![text("Pattern")]),
        CommandParameterType::DefinedType(uuid) => inline_list(vec![
            text("Pattern"),
            inline_list(vec![text("#"), token(uuid.to_string())]),
        ]),
    };
    let details = styled_list(
        vec![
            token("9"),
            picture(&value.picture),
            token(value.representation.to_string()),
            localized(&value.tooltip),
            token("1"),
            inline_list(vec![token("0"), token("0"), token("0")]),
            bool_token(value.include_help_in_contents),
            inline_list(vec![token("1"), token(value.group.to_string())]),
            parameter,
            metadata_header(&header(
                value.uuid,
                &value.name,
                &value.synonyms,
                &value.comment,
            )),
            bool_token(value.modifies_data),
            token(value.parameter_use_mode.to_string()),
            token(value.on_main_server_unavailable_behavior.to_string()),
        ],
        vec![1, 3, 5, 7, 8, 9],
    );
    let collection = styled_list_with_tail(
        vec![
            token("1"),
            inline_list(vec![
                token("2"),
                token(value.uuid.to_string()),
                token(COMMAND_VALUE_UUID),
            ]),
            details,
        ],
        vec![1, 2],
    );
    styled_list(
        vec![
            token("1"),
            styled_list_with_tail(vec![token("2"), collection], vec![1]),
            token("0"),
        ],
        vec![1],
    )
}

fn build_command_group(value: &CommandGroupNativeIr) -> NativeValue {
    let object = styled_list_with_tail(
        vec![
            token("3"),
            picture(&value.picture),
            token(value.category.to_string()),
            token(value.representation.to_string()),
            localized(&value.tooltip),
            inline_list(vec![token("0")]),
            metadata_header(&header(
                value.uuid,
                &value.name,
                &value.synonyms,
                &value.comment,
            )),
        ],
        vec![1, 4, 5, 6],
    );
    styled_list(vec![token("1"), object, token("0")], vec![1])
}

fn build_common_picture(value: &CommonPictureNativeIr) -> NativeValue {
    let object = styled_list(
        vec![
            token("4"),
            metadata_header(&header(
                value.uuid,
                &value.name,
                &value.synonyms,
                &value.comment,
            )),
            bool_token(value.availability_for_choice),
            bool_token(value.availability_for_appearance),
        ],
        vec![1],
    );
    styled_list(vec![token("1"), object, token("0")], vec![1])
}

fn picture(value: &NativePicture) -> NativeValue {
    match value {
        NativePicture::Empty => styled_list(
            vec![
                token("4"),
                token("0"),
                inline_list(vec![token("0")]),
                text(""),
                token("-1"),
                token("-1"),
                token("1"),
                token("0"),
                text(""),
            ],
            vec![2],
        ),
        NativePicture::Code {
            code,
            load_transparent,
        } => styled_list(
            vec![
                token("4"),
                token("1"),
                inline_list(vec![token(code.to_string())]),
                text(""),
                token("-1"),
                token("-1"),
                bool_token(*load_transparent),
                token("0"),
                text(""),
            ],
            vec![2],
        ),
        NativePicture::Uuid {
            uuid,
            load_transparent,
        } => styled_list(
            vec![
                token("4"),
                token("1"),
                inline_list(vec![token("0"), token(uuid.to_string())]),
                text(""),
                token("-1"),
                token("-1"),
                bool_token(*load_transparent),
                token("0"),
                text(""),
            ],
            vec![2],
        ),
    }
}

fn header(
    uuid: ObjectUuid,
    name: &str,
    synonyms: &[(String, String)],
    comment: &str,
) -> NativeMetadataHeader {
    NativeMetadataHeader {
        uuid,
        name: name.to_owned(),
        synonyms: synonyms.to_vec(),
        comment: comment.to_owned(),
    }
}

fn parse_common_command(
    root: &NativeValue,
) -> Result<CommonCommandNativeIr, CommandMetadataBuildError> {
    let root = root_wrapper(root, "2", "CommonCommand")?;
    let collection = exact_list(&root[1], 3, "CommonCommand collection").map_err(native_error)?;
    exact_token(&collection[0], "1", "CommonCommand collection marker").map_err(native_error)?;
    let identity = exact_list(&collection[1], 3, "CommonCommand identity").map_err(native_error)?;
    exact_token(&identity[0], "2", "CommonCommand identity marker").map_err(native_error)?;
    exact_token(&identity[2], COMMAND_VALUE_UUID, "CommonCommand value UUID")
        .map_err(native_error)?;
    let identity_uuid = parse_uuid(&identity[1], "CommonCommand identity UUID")?;
    let details = exact_list(&collection[2], 13, "CommonCommand details").map_err(native_error)?;
    exact_token(&details[0], "9", "CommonCommand details marker").map_err(native_error)?;
    exact_token(&details[4], "1", "CommonCommand reserved flag").map_err(native_error)?;
    let reserved =
        exact_list(&details[5], 3, "CommonCommand reserved tuple").map_err(native_error)?;
    for value in reserved {
        exact_token(value, "0", "CommonCommand reserved tuple value").map_err(native_error)?;
    }
    let group = exact_list(&details[7], 2, "CommonCommand group").map_err(native_error)?;
    exact_token(&group[0], "1", "CommonCommand group marker").map_err(native_error)?;
    let parameter = parse_parameter(&details[8])?;
    let header = parse_metadata_header(&details[9]).map_err(native_error)?;
    if header.uuid != identity_uuid {
        return Err(CommandMetadataBuildError::Native(
            "CommonCommand identity UUID differs from header".into(),
        ));
    }
    Ok(CommonCommandNativeIr {
        uuid: header.uuid,
        name: header.name,
        synonyms: header.synonyms,
        comment: header.comment,
        picture: parse_picture(&details[1])?,
        representation: parse_u8(&details[2], 0..=3, "CommonCommand representation")?,
        tooltip: parse_localized(&details[3]).map_err(native_error)?,
        include_help_in_contents: parse_bool_token(&details[6], "IncludeHelpInContents")
            .map_err(native_error)?,
        group: parse_uuid(&group[1], "CommonCommand group UUID")?,
        command_parameter_type: parameter,
        modifies_data: parse_bool_token(&details[10], "ModifiesData").map_err(native_error)?,
        parameter_use_mode: parse_u8(&details[11], 0..=1, "ParameterUseMode")?,
        on_main_server_unavailable_behavior: parse_u8(
            &details[12],
            0..=0,
            "OnMainServerUnavalableBehavior",
        )?,
    })
}

fn parse_command_group(
    root: &NativeValue,
) -> Result<CommandGroupNativeIr, CommandMetadataBuildError> {
    let object = root_wrapper(root, "3", "CommandGroup")?;
    if object.len() != 7 {
        return Err(CommandMetadataBuildError::Native(
            "CommandGroup object field count is invalid".into(),
        ));
    }
    let reserved =
        exact_list(&object[5], 1, "CommandGroup reserved tuple").map_err(native_error)?;
    exact_token(&reserved[0], "0", "CommandGroup reserved tuple marker").map_err(native_error)?;
    let header = parse_metadata_header(&object[6]).map_err(native_error)?;
    Ok(CommandGroupNativeIr {
        uuid: header.uuid,
        name: header.name,
        synonyms: header.synonyms,
        comment: header.comment,
        picture: parse_picture(&object[1])?,
        category: parse_u8(&object[2], 1..=8, "CommandGroup category")?,
        representation: parse_u8(&object[3], 0..=3, "CommandGroup representation")?,
        tooltip: parse_localized(&object[4]).map_err(native_error)?,
    })
}

fn parse_common_picture(
    root: &NativeValue,
) -> Result<CommonPictureNativeIr, CommandMetadataBuildError> {
    let object = root_wrapper(root, "4", "CommonPicture")?;
    if object.len() != 4 {
        return Err(CommandMetadataBuildError::Native(
            "CommonPicture object field count is invalid".into(),
        ));
    }
    let header = parse_metadata_header(&object[1]).map_err(native_error)?;
    Ok(CommonPictureNativeIr {
        uuid: header.uuid,
        name: header.name,
        synonyms: header.synonyms,
        comment: header.comment,
        availability_for_choice: parse_bool_token(&object[2], "AvailabilityForChoice")
            .map_err(native_error)?,
        availability_for_appearance: parse_bool_token(&object[3], "AvailabilityForAppearance")
            .map_err(native_error)?,
    })
}

fn root_wrapper<'a>(
    root: &'a NativeValue,
    marker: &str,
    field: &'static str,
) -> Result<&'a [NativeValue], CommandMetadataBuildError> {
    let root = exact_list(root, 3, field).map_err(native_error)?;
    exact_token(&root[0], "1", "metadata root marker").map_err(native_error)?;
    exact_token(&root[2], "0", "metadata root tail").map_err(native_error)?;
    let wrapper = root[1]
        .as_list()
        .ok_or_else(|| CommandMetadataBuildError::Native(format!("{field} is not a list")))?;
    if wrapper.is_empty() {
        return Err(CommandMetadataBuildError::Native(format!(
            "{field} container is empty"
        )));
    }
    exact_token(&wrapper[0], marker, "metadata wrapper marker").map_err(native_error)?;
    Ok(wrapper)
}

fn parse_picture(value: &NativeValue) -> Result<NativePicture, CommandMetadataBuildError> {
    let fields = exact_list(value, 9, "picture descriptor").map_err(native_error)?;
    exact_token(&fields[0], "4", "picture descriptor marker").map_err(native_error)?;
    if !required_text(&fields[3], "picture name")
        .map_err(native_error)?
        .is_empty()
    {
        return Err(CommandMetadataBuildError::Native(
            "picture name is not empty".into(),
        ));
    }
    exact_token(&fields[4], "-1", "picture width").map_err(native_error)?;
    exact_token(&fields[5], "-1", "picture height").map_err(native_error)?;
    exact_token(&fields[7], "0", "picture reserved flag").map_err(native_error)?;
    if !required_text(&fields[8], "picture tail")
        .map_err(native_error)?
        .is_empty()
    {
        return Err(CommandMetadataBuildError::Native(
            "picture tail is not empty".into(),
        ));
    }
    let payload = fields[2]
        .as_list()
        .ok_or_else(|| CommandMetadataBuildError::Native("picture payload is not a list".into()))?;
    match required_token(&fields[1], "picture presence").map_err(native_error)? {
        "0" => {
            if payload.len() != 1 || payload[0].as_token() != Some("0") {
                return Err(CommandMetadataBuildError::Native(
                    "empty picture payload is invalid".into(),
                ));
            }
            if !parse_bool_token(&fields[6], "empty picture transparent flag")
                .map_err(native_error)?
            {
                return Err(CommandMetadataBuildError::Native(
                    "empty picture transparent flag is not 1".into(),
                ));
            }
            Ok(NativePicture::Empty)
        }
        "1" => {
            let load_transparent =
                parse_bool_token(&fields[6], "picture transparent flag").map_err(native_error)?;
            if payload.len() == 1 {
                let code = required_token(&payload[0], "standard picture code")
                    .map_err(native_error)?
                    .parse::<i32>()
                    .map_err(|_| {
                        CommandMetadataBuildError::Native("standard picture code is invalid".into())
                    })?;
                Ok(NativePicture::Code {
                    code,
                    load_transparent,
                })
            } else if payload.len() == 2 && payload[0].as_token() == Some("0") {
                Ok(NativePicture::Uuid {
                    uuid: parse_uuid(&payload[1], "picture UUID")?,
                    load_transparent,
                })
            } else {
                Err(CommandMetadataBuildError::Native(
                    "picture payload shape is invalid".into(),
                ))
            }
        }
        _ => Err(CommandMetadataBuildError::Native(
            "picture presence flag is invalid".into(),
        )),
    }
}

fn parse_parameter(value: &NativeValue) -> Result<CommandParameterType, CommandMetadataBuildError> {
    let fields = value.as_list().ok_or_else(|| {
        CommandMetadataBuildError::Native("parameter pattern is not a list".into())
    })?;
    if fields.is_empty()
        || required_text(&fields[0], "parameter pattern marker").map_err(native_error)? != "Pattern"
    {
        return Err(CommandMetadataBuildError::Native(
            "parameter pattern marker is invalid".into(),
        ));
    }
    match fields.len() {
        1 => Ok(CommandParameterType::Empty),
        2 => {
            let reference =
                exact_list(&fields[1], 2, "parameter type reference").map_err(native_error)?;
            if required_text(&reference[0], "parameter type marker").map_err(native_error)? != "#" {
                return Err(CommandMetadataBuildError::Native(
                    "parameter type marker is invalid".into(),
                ));
            }
            Ok(CommandParameterType::DefinedType(parse_uuid(
                &reference[1],
                "parameter TypeId",
            )?))
        }
        _ => Err(CommandMetadataBuildError::Native(
            "parameter type inventory is unsupported".into(),
        )),
    }
}

fn parse_uuid(
    value: &NativeValue,
    field: &'static str,
) -> Result<ObjectUuid, CommandMetadataBuildError> {
    let value = required_token(value, field).map_err(native_error)?;
    ObjectUuid::parse(value)
        .map_err(|_| CommandMetadataBuildError::Native(format!("{field} is not a UUID")))
}

fn parse_u8(
    value: &NativeValue,
    accepted: std::ops::RangeInclusive<u8>,
    field: &'static str,
) -> Result<u8, CommandMetadataBuildError> {
    let value = required_token(value, field)
        .map_err(native_error)?
        .parse::<u8>()
        .map_err(|_| CommandMetadataBuildError::Native(format!("{field} is not u8")))?;
    if !accepted.contains(&value) {
        return Err(CommandMetadataBuildError::Native(format!(
            "{field} has unsupported code"
        )));
    }
    Ok(value)
}

fn project_common_command(
    object: &CanonicalObject,
    indexes: &ReferenceIndexes,
) -> Result<CommonCommandNativeIr, CommandMetadataBuildError> {
    require_family_and_schema(
        object,
        CommandMetadataFamily::CommonCommand,
        COMMON_COMMAND_PROPERTIES,
    )?;
    Ok(CommonCommandNativeIr {
        uuid: object.identity().uuid(),
        name: text_property(object, "Name")?.to_owned(),
        synonyms: localized_property(object, "Synonym", "lang")?,
        comment: text_property(object, "Comment")?.to_owned(),
        group: indexes.command_group(object, text_property(object, "Group")?)?,
        representation: representation_code(object, "Representation")?,
        tooltip: localized_property(object, "ToolTip", "language")?,
        picture: project_picture(object, indexes, false)?,
        include_help_in_contents: bool_property(object, "IncludeHelpInContents")?,
        command_parameter_type: {
            let reference = text_property(object, "CommandParameterType")?;
            if reference.is_empty() {
                CommandParameterType::Empty
            } else {
                CommandParameterType::DefinedType(indexes.defined_type(object, reference)?)
            }
        },
        parameter_use_mode: match enum_property(object, "ParameterUseMode")? {
            "Single" => 0,
            "Multiple" => 1,
            _ => return invalid_model(object, "ParameterUseMode has no evidenced native code"),
        },
        modifies_data: bool_property(object, "ModifiesData")?,
        on_main_server_unavailable_behavior: match enum_property(
            object,
            "OnMainServerUnavalableBehavior",
        )? {
            "Auto" => 0,
            _ => return invalid_model(object, "main-server behavior has no evidenced native code"),
        },
    })
}

fn project_command_group(
    object: &CanonicalObject,
    indexes: &ReferenceIndexes,
) -> Result<CommandGroupNativeIr, CommandMetadataBuildError> {
    require_family_and_schema(
        object,
        CommandMetadataFamily::CommandGroup,
        COMMAND_GROUP_PROPERTIES,
    )?;
    Ok(CommandGroupNativeIr {
        uuid: object.identity().uuid(),
        name: text_property(object, "Name")?.to_owned(),
        synonyms: localized_property(object, "Synonym", "lang")?,
        comment: text_property(object, "Comment")?.to_owned(),
        representation: representation_code(object, "Representation")?,
        tooltip: localized_property(object, "ToolTip", "language")?,
        picture: project_picture(object, indexes, true)?,
        category: match enum_property(object, "Category")? {
            "NavigationPanel" => 1,
            "FormNavigationPanel" => 2,
            "ActionsPanel" => 4,
            "FormCommandBar" => 8,
            _ => return invalid_model(object, "Category has no evidenced native code"),
        },
    })
}

fn project_common_picture(
    object: &CanonicalObject,
) -> Result<CommonPictureNativeIr, CommandMetadataBuildError> {
    require_family_and_schema(
        object,
        CommandMetadataFamily::CommonPicture,
        COMMON_PICTURE_PROPERTIES,
    )?;
    Ok(CommonPictureNativeIr {
        uuid: object.identity().uuid(),
        name: text_property(object, "Name")?.to_owned(),
        synonyms: localized_property(object, "Synonym", "lang")?,
        comment: text_property(object, "Comment")?.to_owned(),
        availability_for_choice: bool_property(object, "AvailabilityForChoice")?,
        availability_for_appearance: bool_property(object, "AvailabilityForAppearance")?,
    })
}

fn project_picture(
    object: &CanonicalObject,
    indexes: &ReferenceIndexes,
    command_group: bool,
) -> Result<NativePicture, CommandMetadataBuildError> {
    let reference = text_property(object, "PictureReference")?;
    if reference.is_empty() {
        return Ok(NativePicture::Empty);
    }
    let load_transparent = bool_property(object, "PictureLoadTransparent")?;
    if let Some(code) = standard_picture_code(reference, command_group) {
        return Ok(NativePicture::Code {
            code,
            load_transparent,
        });
    }
    if let Some(uuid) = standard_picture_uuid(reference, command_group) {
        return Ok(NativePicture::Uuid {
            uuid,
            load_transparent,
        });
    }
    Ok(NativePicture::Uuid {
        uuid: indexes.common_picture(object, reference)?,
        load_transparent,
    })
}

fn representation_code(
    object: &CanonicalObject,
    name: &str,
) -> Result<u8, CommandMetadataBuildError> {
    match enum_property(object, name)? {
        "Text" => Ok(0),
        "Picture" => Ok(1),
        "PictureAndText" => Ok(2),
        "Auto" => Ok(3),
        _ => invalid_model(object, "Representation has no evidenced native code"),
    }
}

struct ReferenceIndexes {
    command_groups: BTreeMap<String, ObjectUuid>,
    common_pictures: BTreeMap<String, ObjectUuid>,
    defined_types: BTreeMap<String, ObjectUuid>,
}

impl ReferenceIndexes {
    fn build(
        validated: &ValidatedConfiguration<'_>,
        compiling: ObjectUuid,
    ) -> Result<Self, CommandMetadataBuildError> {
        let mut command_groups = BTreeMap::new();
        let mut common_pictures = BTreeMap::new();
        let mut defined_types = BTreeMap::new();
        for object in validated.configuration().objects() {
            if object.owner().is_some() {
                continue;
            }
            let Some(name) = optional_text_property(object, "Name") else {
                continue;
            };
            if name.is_empty() || name.contains('.') {
                continue;
            }
            match object.kind().as_str() {
                "CommandGroup" => insert_reference(
                    &mut command_groups,
                    format!("CommandGroup.{name}"),
                    object.identity().uuid(),
                    compiling,
                )?,
                "CommonPicture" => insert_reference(
                    &mut common_pictures,
                    format!("CommonPicture.{name}"),
                    object.identity().uuid(),
                    compiling,
                )?,
                "DefinedType" => {
                    let mut matching = object
                        .generated_types()
                        .iter()
                        .filter(|generated| generated.kind().as_str() == "DefinedType");
                    let generated =
                        matching
                            .next()
                            .ok_or(CommandMetadataBuildError::InvalidModel {
                                object: compiling,
                                reason: "DefinedType has no generated TypeId",
                            })?;
                    if matching.next().is_some() {
                        return Err(CommandMetadataBuildError::InvalidModel {
                            object: compiling,
                            reason: "DefinedType has ambiguous generated TypeId",
                        });
                    }
                    insert_reference(
                        &mut defined_types,
                        format!("cfg:DefinedType.{name}"),
                        generated.uuid(),
                        compiling,
                    )?;
                }
                _ => {}
            }
        }
        Ok(Self {
            command_groups,
            common_pictures,
            defined_types,
        })
    }

    fn command_group(
        &self,
        object: &CanonicalObject,
        reference: &str,
    ) -> Result<ObjectUuid, CommandMetadataBuildError> {
        builtin_command_group(reference)
            .or_else(|| self.command_groups.get(reference).copied())
            .ok_or(CommandMetadataBuildError::InvalidModel {
                object: object.identity().uuid(),
                reason: "CommandGroup reference is unresolved",
            })
    }

    fn common_picture(
        &self,
        object: &CanonicalObject,
        reference: &str,
    ) -> Result<ObjectUuid, CommandMetadataBuildError> {
        if !reference.starts_with("CommonPicture.") {
            return invalid_model(
                object,
                "Picture reference is neither mapped StdPicture nor CommonPicture",
            );
        }
        self.common_pictures.get(reference).copied().ok_or(
            CommandMetadataBuildError::InvalidModel {
                object: object.identity().uuid(),
                reason: "CommonPicture reference is unresolved",
            },
        )
    }

    fn defined_type(
        &self,
        object: &CanonicalObject,
        reference: &str,
    ) -> Result<ObjectUuid, CommandMetadataBuildError> {
        self.defined_types
            .get(reference)
            .copied()
            .ok_or(CommandMetadataBuildError::InvalidModel {
                object: object.identity().uuid(),
                reason: "CommandParameterType is not a resolved DefinedType",
            })
    }
}

fn insert_reference(
    values: &mut BTreeMap<String, ObjectUuid>,
    reference: String,
    uuid: ObjectUuid,
    compiling: ObjectUuid,
) -> Result<(), CommandMetadataBuildError> {
    if let Some(existing) = values.insert(reference, uuid)
        && existing != uuid
    {
        return Err(CommandMetadataBuildError::InvalidModel {
            object: compiling,
            reason: "readable metadata reference is ambiguous",
        });
    }
    Ok(())
}

fn builtin_command_group(reference: &str) -> Option<ObjectUuid> {
    let value = match reference {
        "NavigationPanelOrdinary" => "77ea1b8f-dd79-4717-9dba-5628e7f348cf",
        "NavigationPanelSeeAlso" => "bc80566a-86a5-4e87-acd4-872239385a2e",
        "NavigationPanelImportant" => "1af6d528-0b86-4fba-ab95-bd7475db03ba",
        "ActionsPanelCreate" => "4f499c31-050b-47c5-aa84-d0366c0a0da8",
        "ActionsPanelReports" => "5b360bff-01a1-49b6-93d2-26e7e8e3a038",
        "ActionsPanelTools" => "aabb34e1-98c1-4bd0-bf7f-243f95437b44",
        "FormCommandBarCreateBasedOn" => "dc2ade0f-383e-4c78-85f2-c0dabc0e2dc0",
        "FormCommandBarImportant" => "cb50f5c0-8013-4262-93a2-f0db379d6b6b",
        "FormNavigationPanelGoTo" => "eacad741-96b9-4b3a-bf79-dde9ecead1a1",
        "FormNavigationPanelSeeAlso" => "8ab1540c-0bfa-4fa6-a1e1-5d5069efc7d8",
        "FormNavigationPanelImportant" => "dc11a6be-de1f-4b64-a7a5-9b17bf4ec9f2",
        _ => return None,
    };
    Some(ObjectUuid::parse(value).expect("evidenced command-group UUID is canonical"))
}

fn standard_picture_code(reference: &str, command_group: bool) -> Option<i32> {
    if command_group {
        return (reference == "StdPicture.Print").then_some(-13);
    }
    match reference {
        "StdPicture.InputFieldClear" => Some(-2),
        "StdPicture.MoveUp" => Some(-3),
        "StdPicture.MoveDown" => Some(-4),
        "StdPicture.InputFieldOpen" => Some(-7),
        "StdPicture.MoveRight" => Some(-9),
        "StdPicture.CheckAll" => Some(-10),
        "StdPicture.UncheckAll" => Some(-11),
        _ => None,
    }
}

fn standard_picture_uuid(reference: &str, command_group: bool) -> Option<ObjectUuid> {
    let value = if command_group {
        match reference {
            "StdPicture.InformationRegister" => "5b87ad1b-d8cc-43c1-b5c4-dc43613c518c",
            _ => return None,
        }
    } else {
        match reference {
            "StdPicture.Information" => "4b54770b-d069-4c0e-9b17-5cc2a01134d9",
            "StdPicture.SaveFile" => "818ab7d0-4654-4542-bd5e-fd9d1352b5a1",
            "StdPicture.User" => "6ff3ddbd-56e3-4ddf-a5bf-048c1e2dfb2f",
            "StdPicture.LoadReportSettings" => "283ecabd-aaed-41d1-ad46-6cca91c29120",
            "StdPicture.Change" => "97b2cc97-d5c6-45fb-9824-9d6d73db21fe",
            "StdPicture.Task" => "37cf7cc0-abad-4385-b597-6fd2d8dc085a",
            "StdPicture.ChooseValue" => "2f130057-bb2a-4e22-bba5-e108fac26940",
            "StdPicture.DataHistory" => "e8a49985-fef7-45a9-b6bb-ddd2b9028172",
            "StdPicture.BusinessProcessObject" => "a24cff7f-a1a5-4403-af82-a7b31852cde9",
            "StdPicture.CloneObject" => "f6532868-30b9-44ab-803c-78f0f0b06b02",
            "StdPicture.CloneListItem" => "448d6f55-d885-496c-870d-d1bd78374745",
            "StdPicture.EventLog" => "723765ab-0b92-4745-a621-1ba0f77c92c9",
            "StdPicture.EventLogByUser" => "4fddea39-5129-4b4c-83fe-4e443cd61940",
            "StdPicture.Find" => "ffab30f1-da11-44b5-b34c-24da22badcf4",
            "StdPicture.CreateInitialImage" => "4d2570b5-205f-413c-b4cc-b2097f61684f",
            "StdPicture.GenerateReport" => "0ce78048-0196-4f80-a781-9829cdb7f43e",
            "StdPicture.MarkToDelete" => "18492a87-2fe4-44af-b218-304897fed020",
            "StdPicture.Post" => "20ebc47b-f4d9-439c-acd3-fdc624fbac2a",
            "StdPicture.Reread" => "8f29e0e2-d5e6-41e8-a34d-9a0288156322",
            "StdPicture.Report" => "db817ee1-fd28-4e7f-bb4a-53686b2b153c",
            "StdPicture.ScheduledJob" => "1970a480-9b38-405e-9d9e-8209f3fad5f1",
            "StdPicture.SetDateInterval" => "58174855-39be-462e-8723-cb2d95182146",
            "StdPicture.SetTime" => "55ef0776-5ee4-4daf-9a9b-70d63643ab8d",
            "StdPicture.Refresh" => "fc4f29e0-d168-4fe0-8e64-e982fabf2595",
            "StdPicture.SortListAsc" => "91022b99-b610-48ad-954e-a297848081ce",
            "StdPicture.SortListDesc" => "1fa32fdb-a180-418f-a6eb-db7516b7a30b",
            "StdPicture.UndoPosting" => "8ca4ea33-603d-4992-8a41-c7924b5bd40b",
            "StdPicture.Write" => "894cf65b-4109-4533-a1d7-c87b1fcc80a3",
            "StdPicture.WriteAndClose" => "e6fc55a0-3d58-4b15-bdd3-717453929598",
            "StdPicture.Delete" => "08a45a70-c221-4339-b3b1-9f11cb22147d",
            _ => return None,
        }
    };
    Some(ObjectUuid::parse(value).expect("evidenced standard-picture UUID is canonical"))
}

fn require_family_and_schema(
    object: &CanonicalObject,
    family: CommandMetadataFamily,
    schema: &[&str],
) -> Result<(), CommandMetadataBuildError> {
    if object.kind().as_str() != family.as_str() {
        return invalid_model(object, "metadata family differs from selected codec");
    }
    if object.owner().is_some()
        || !object.references().is_empty()
        || !object.generated_types().is_empty()
        || !object.assets().is_empty()
    {
        return invalid_model(object, "ownership/reference inventory is not empty");
    }
    if object.properties().len() != schema.len()
        || object
            .properties()
            .iter()
            .zip(schema)
            .any(|(field, expected)| field.name().as_str() != *expected)
    {
        return invalid_model(object, "canonical property schema is not exact");
    }
    Ok(())
}

fn property<'a>(
    object: &'a CanonicalObject,
    name: &str,
) -> Result<&'a CanonicalValue, CommandMetadataBuildError> {
    object
        .properties()
        .iter()
        .find(|field| field.name().as_str() == name)
        .map(CanonicalField::value)
        .ok_or(CommandMetadataBuildError::InvalidModel {
            object: object.identity().uuid(),
            reason: "required typed property is missing",
        })
}

fn text_property<'a>(
    object: &'a CanonicalObject,
    name: &str,
) -> Result<&'a str, CommandMetadataBuildError> {
    match property(object, name)?.kind() {
        CanonicalValueKind::Text(value) => Ok(value.as_str()),
        _ => invalid_model(object, "typed property is not text"),
    }
}

fn optional_text_property<'a>(object: &'a CanonicalObject, name: &str) -> Option<&'a str> {
    object
        .properties()
        .iter()
        .find(|field| field.name().as_str() == name)
        .and_then(|field| match field.value().kind() {
            CanonicalValueKind::Text(value) => Some(value.as_str()),
            _ => None,
        })
}

fn bool_property(object: &CanonicalObject, name: &str) -> Result<bool, CommandMetadataBuildError> {
    match property(object, name)?.kind() {
        CanonicalValueKind::Bool(value) => Ok(value),
        _ => invalid_model(object, "typed property is not boolean"),
    }
}

fn enum_property<'a>(
    object: &'a CanonicalObject,
    name: &str,
) -> Result<&'a str, CommandMetadataBuildError> {
    match property(object, name)?.kind() {
        CanonicalValueKind::EnumToken(value) => Ok(value.as_str()),
        _ => invalid_model(object, "typed property is not an enum token"),
    }
}

fn localized_property(
    object: &CanonicalObject,
    name: &str,
    language_field: &str,
) -> Result<Vec<(String, String)>, CommandMetadataBuildError> {
    let values =
        property(object, name)?
            .as_sequence()
            .ok_or(CommandMetadataBuildError::InvalidModel {
                object: object.identity().uuid(),
                reason: "localized property is not a sequence",
            })?;
    let mut output = Vec::with_capacity(values.len());
    let mut languages = BTreeSet::new();
    for value in values {
        let fields = value
            .as_record()
            .ok_or(CommandMetadataBuildError::InvalidModel {
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

fn validate_coordinates(
    graph: &BootstrapGraph,
    axes: &CompileAxes,
    profile: &CommandMetadataProfile,
) -> Result<(), CommandMetadataBuildError> {
    if graph.profile_id() != &profile.profile_id {
        return Err(CommandMetadataBuildError::ProfileMismatch {
            graph: graph.profile_id().clone(),
            codec: profile.profile_id.clone(),
        });
    }
    if axes.platform_build() != Some(&profile.platform_build) {
        return Err(CommandMetadataBuildError::AxisMismatch("platform_build"));
    }
    if axes.storage_profile() != &profile.storage_profile {
        return Err(CommandMetadataBuildError::AxisMismatch("storage_profile"));
    }
    if axes.compatibility_mode().is_some() || axes.container_revision().is_some() {
        return Err(CommandMetadataBuildError::AxisMismatch(
            "unevidenced optional coordinate",
        ));
    }
    if !matches!(axes.xml_dialect().to_string().as_str(), "2.20" | "2.21") {
        return Err(CommandMetadataBuildError::AxisMismatch("xml_dialect"));
    }
    Ok(())
}

fn bool_token(value: bool) -> NativeValue {
    token(if value { "1" } else { "0" })
}

fn invalid_model<T>(
    object: &CanonicalObject,
    reason: &'static str,
) -> Result<T, CommandMetadataBuildError> {
    Err(CommandMetadataBuildError::InvalidModel {
        object: object.identity().uuid(),
        reason,
    })
}

fn native_error(error: impl Display) -> CommandMetadataBuildError {
    CommandMetadataBuildError::Native(error.to_string())
}

#[derive(Debug)]
pub enum CommandMetadataBuildError {
    Profile(CommandMetadataProfileError),
    ProfileMismatch {
        graph: ProfileId,
        codec: ProfileId,
    },
    AxisMismatch(&'static str),
    FamilyMismatch {
        expected: CommandMetadataFamily,
        actual: CommandMetadataFamily,
    },
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

impl Display for CommandMetadataBuildError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Profile(source) => Display::fmt(source, formatter),
            Self::ProfileMismatch { graph, codec } => {
                write!(
                    formatter,
                    "graph profile `{graph}` differs from codec `{codec}`"
                )
            }
            Self::AxisMismatch(axis) => {
                write!(formatter, "command metadata `{axis}` axis mismatch")
            }
            Self::FamilyMismatch { expected, actual } => write!(
                formatter,
                "selected {} codec cannot encode {} IR",
                expected.as_str(),
                actual.as_str()
            ),
            Self::UnknownObject(uuid) => write!(formatter, "validated graph has no object {uuid}"),
            Self::MissingPrimaryRoute(uuid) => {
                write!(formatter, "bootstrap graph has no primary row for {uuid}")
            }
            Self::InvalidModel { object, reason } => {
                write!(
                    formatter,
                    "command metadata {object} is not compilable: {reason}"
                )
            }
            Self::Native(reason) => {
                write!(formatter, "invalid command metadata native row: {reason}")
            }
            Self::Storage(source) => Display::fmt(source, formatter),
            Self::Patch(source) => Display::fmt(source, formatter),
        }
    }
}

impl Error for CommandMetadataBuildError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Profile(source) => Some(source),
            Self::Storage(source) => Some(source),
            Self::Patch(source) => Some(source),
            _ => None,
        }
    }
}

impl From<CommandMetadataProfileError> for CommandMetadataBuildError {
    fn from(source: CommandMetadataProfileError) -> Self {
        Self::Profile(source)
    }
}

impl From<StorageBuildError> for CommandMetadataBuildError {
    fn from(source: StorageBuildError) -> Self {
        Self::Storage(source)
    }
}

impl From<StoragePatchBuildError> for CommandMetadataBuildError {
    fn from(source: StoragePatchBuildError) -> Self {
        Self::Patch(source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn representative_common_command_is_byte_exact_and_roundtrips() {
        let profile = CommandMetadataProfile::fixture(CommandMetadataFamily::CommonCommand);
        let value = CommandMetadataNativeIr::CommonCommand(CommonCommandNativeIr {
            uuid: ObjectUuid::parse("129f704b-cc63-4a4e-b6a7-9e53de4537a2").unwrap(),
            name: "ВсеФайлы".to_owned(),
            synonyms: vec![("ru".to_owned(), "Все файлы".to_owned())],
            comment: String::new(),
            group: ObjectUuid::parse("77ea1b8f-dd79-4717-9dba-5628e7f348cf").unwrap(),
            representation: 3,
            tooltip: Vec::new(),
            picture: NativePicture::Empty,
            include_help_in_contents: false,
            command_parameter_type: CommandParameterType::Empty,
            parameter_use_mode: 0,
            modifies_data: false,
            on_main_server_unavailable_behavior: 0,
        });
        let expected = concat!(
            "\u{feff}{1,\r\n{2,\r\n{1,\r\n",
            "{2,129f704b-cc63-4a4e-b6a7-9e53de4537a2,078a6af8-d22c-4248-9c33-7e90075a3d2c},\r\n",
            "{9,\r\n{4,0,\r\n{0},\"\",-1,-1,1,0,\"\"},3,\r\n{0},1,\r\n",
            "{0,0,0},0,\r\n{1,77ea1b8f-dd79-4717-9dba-5628e7f348cf},\r\n",
            "{\"Pattern\"},\r\n{3,\r\n{1,0,129f704b-cc63-4a4e-b6a7-9e53de4537a2},\"ВсеФайлы\",\r\n",
            "{1,\"ru\",\"Все файлы\"},\"\",0,0,00000000-0000-0000-0000-000000000000,0},0,0,0}\r\n",
            "}\r\n},0}"
        );
        assert_eq!(
            command_metadata_plaintext(&value, &profile).unwrap(),
            expected.as_bytes()
        );
        let blob = encode_command_metadata_blob(&value, &profile).unwrap();
        assert_eq!(
            decode_command_metadata_blob(&blob, &profile).unwrap(),
            value
        );
    }

    #[test]
    fn representative_command_group_is_byte_exact_and_roundtrips() {
        let profile = CommandMetadataProfile::fixture(CommandMetadataFamily::CommandGroup);
        let value = CommandMetadataNativeIr::CommandGroup(CommandGroupNativeIr {
            uuid: ObjectUuid::parse("ac39b903-0c60-417e-a50f-49ed375424f5").unwrap(),
            name: "Печать".to_owned(),
            synonyms: vec![("ru".to_owned(), "Печать".to_owned())],
            comment: "Печать".to_owned(),
            representation: 3,
            tooltip: vec![("ru".to_owned(), "Печать".to_owned())],
            picture: NativePicture::Code {
                code: -13,
                load_transparent: true,
            },
            category: 8,
        });
        let expected = concat!(
            "\u{feff}{1,\r\n{3,\r\n{4,1,\r\n{-13},\"\",-1,-1,1,0,\"\"},8,3,\r\n",
            "{1,\"ru\",\"Печать\"},\r\n{0},\r\n{3,\r\n",
            "{1,0,ac39b903-0c60-417e-a50f-49ed375424f5},\"Печать\",\r\n",
            "{1,\"ru\",\"Печать\"},\"Печать\",0,0,00000000-0000-0000-0000-000000000000,0}\r\n",
            "},0}"
        );
        assert_eq!(
            command_metadata_plaintext(&value, &profile).unwrap(),
            expected.as_bytes()
        );
        let blob = encode_command_metadata_blob(&value, &profile).unwrap();
        assert_eq!(
            decode_command_metadata_blob(&blob, &profile).unwrap(),
            value
        );
    }

    #[test]
    fn representative_common_picture_is_byte_exact_and_roundtrips() {
        let profile = CommandMetadataProfile::fixture(CommandMetadataFamily::CommonPicture);
        let value = CommandMetadataNativeIr::CommonPicture(CommonPictureNativeIr {
            uuid: ObjectUuid::parse("42e39ef7-268f-4053-b4e5-4bceab17d3e4").unwrap(),
            name: "Skype".to_owned(),
            synonyms: vec![("ru".to_owned(), "Skype".to_owned())],
            comment: String::new(),
            availability_for_choice: false,
            availability_for_appearance: false,
        });
        let expected = concat!(
            "\u{feff}{1,\r\n{4,\r\n",
            "{3,\r\n{1,0,42e39ef7-268f-4053-b4e5-4bceab17d3e4},\"Skype\",\r\n",
            "{1,\"ru\",\"Skype\"},\"\",0,0,00000000-0000-0000-0000-000000000000,0},0,0},0}"
        );
        assert_eq!(
            command_metadata_plaintext(&value, &profile).unwrap(),
            expected.as_bytes()
        );
        let blob = encode_command_metadata_blob(&value, &profile).unwrap();
        assert_eq!(
            decode_command_metadata_blob(&blob, &profile).unwrap(),
            value
        );
    }
}
