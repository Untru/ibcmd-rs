//! Profile-gated base-free codec for `Ext/CommandInterface.xml` native bodies.

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use ibcmd_core::artifact::ProfileId;
use ibcmd_core::identity::ObjectUuid;
use ibcmd_core::profile::EffectiveProfile;

use super::{BodyProfileError, SelectedBodyProfile};
use crate::compiler::families::native::{
    NativeError, NativeValue, exact_list, exact_token, inflate_and_parse, platform_list,
    raw_deflate, required_list, required_text, required_token, serialize, text, token,
};

const LAYOUT_KEY: &str = "bootstrap.body.command_interface.layout";
const LAYOUT: &str = "command-interface-v7-sections-v1-raw-deflate-utf8-bom";
const NIL_UUID: &str = "00000000-0000-0000-0000-000000000000";
const MAX_SECTION_ITEMS: usize = 100_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandInterfaceCodecProfile(SelectedBodyProfile);

impl CommandInterfaceCodecProfile {
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

/// The six sections of a stored command interface, in stored order.
///
/// The record is `{7,<visibility>,<placement>,<order>,<subsystem order>,<group
/// order>,<subsystem visibility>}`; each section is `0` when empty and
/// `1,<count>,<items>` otherwise -- the inverse of the exporter's
/// `parse_command_interface_sectioned_fields`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CommandInterfaceModel {
    pub commands_visibility: Vec<CommandVisibility>,
    pub commands_placement: Vec<CommandPlacement>,
    pub commands_order: Vec<CommandOrder>,
    /// The declared subsystem order. The nil uuid holds a slot that names no
    /// subsystem; the platform exports it as a self-closed `<Subsystem/>`.
    pub subsystems_order: Vec<ObjectUuid>,
    pub groups_order: Vec<ObjectUuid>,
    pub subsystems_visibility: Vec<SubsystemVisibility>,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CommandReference {
    /// The explicit `{0}` sentinel observed in order/visibility sections.
    Empty,
    Resolved {
        kind: u32,
        uuid: ObjectUuid,
    },
}

impl CommandReference {
    pub const fn resolved(kind: u32, uuid: ObjectUuid) -> Self {
        Self::Resolved { kind, uuid }
    }
}

/// One per-role override of an adjustable visibility, in stored order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VisibilityValue {
    pub role: ObjectUuid,
    pub value: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandVisibility {
    pub command: CommandReference,
    pub common: bool,
    pub values: Vec<VisibilityValue>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubsystemVisibility {
    pub subsystem: ObjectUuid,
    pub common: bool,
    pub values: Vec<VisibilityValue>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandPlacementMode {
    Auto,
    Manual,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandPlacement {
    pub command: CommandReference,
    pub command_group: ObjectUuid,
    pub placement: CommandPlacementMode,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandOrder {
    pub command_group: ObjectUuid,
    pub command: CommandReference,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandInterfaceBody {
    native: NativeValue,
    model: CommandInterfaceModel,
}

impl CommandInterfaceBody {
    pub const fn model(&self) -> &CommandInterfaceModel {
        &self.model
    }

    pub fn plaintext(&self) -> Result<Vec<u8>, CommandInterfaceCodecError> {
        serialize(&self.native).map_err(Into::into)
    }
}

pub fn compile_command_interface(
    profile: &CommandInterfaceCodecProfile,
    model: &CommandInterfaceModel,
) -> Result<Vec<u8>, CommandInterfaceCodecError> {
    let _ = profile;
    compile_evidenced_command_interface(model)
}

pub(crate) fn compile_evidenced_command_interface(
    model: &CommandInterfaceModel,
) -> Result<Vec<u8>, CommandInterfaceCodecError> {
    validate_model(model)?;
    raw_deflate(&native_from_model(model)).map_err(Into::into)
}

/// The plain text the platform stores for `model`, BOM included.
pub(crate) fn command_interface_plaintext(
    model: &CommandInterfaceModel,
) -> Result<Vec<u8>, CommandInterfaceCodecError> {
    validate_model(model)?;
    serialize(&native_from_model(model)).map_err(Into::into)
}

pub fn decode_command_interface(
    profile: &CommandInterfaceCodecProfile,
    blob: &[u8],
) -> Result<CommandInterfaceBody, CommandInterfaceCodecError> {
    let _ = profile;
    let native = inflate_and_parse(blob)?;
    body_from_native(native)
}

fn body_from_native(
    native: NativeValue,
) -> Result<CommandInterfaceBody, CommandInterfaceCodecError> {
    let model = model_from_native(&native)?;
    validate_model(&model)?;
    Ok(CommandInterfaceBody { native, model })
}

fn native_from_model(model: &CommandInterfaceModel) -> NativeValue {
    let mut fields = vec![token("7")];
    push_section(&mut fields, &model.commands_visibility, |entry| {
        vec![
            native_command_reference(&entry.command),
            native_visibility(entry.common, &entry.values),
        ]
    });
    push_section(&mut fields, &model.commands_placement, |entry| {
        vec![
            native_command_reference(&entry.command),
            token(entry.command_group.to_string()),
            token(match entry.placement {
                CommandPlacementMode::Auto => "0",
                CommandPlacementMode::Manual => "1",
            }),
        ]
    });
    push_section(&mut fields, &model.commands_order, |entry| {
        vec![
            token(entry.command_group.to_string()),
            native_command_reference(&entry.command),
        ]
    });
    push_section(&mut fields, &model.subsystems_order, |uuid| {
        vec![token(uuid.to_string())]
    });
    push_section(&mut fields, &model.groups_order, |uuid| {
        vec![token(uuid.to_string())]
    });
    push_section(&mut fields, &model.subsystems_visibility, |entry| {
        vec![
            token(entry.subsystem.to_string()),
            native_visibility(entry.common, &entry.values),
        ]
    });
    platform_list(fields)
}

fn push_section<T>(
    fields: &mut Vec<NativeValue>,
    values: &[T],
    mut encode: impl FnMut(&T) -> Vec<NativeValue>,
) {
    if values.is_empty() {
        fields.push(token("0"));
        return;
    }
    fields.push(token("1"));
    fields.push(token(values.len().to_string()));
    for value in values {
        fields.extend(encode(value));
    }
}

fn native_command_reference(reference: &CommandReference) -> NativeValue {
    match reference {
        CommandReference::Empty => platform_list(vec![token("0")]),
        CommandReference::Resolved { kind, uuid } => {
            platform_list(vec![token(kind.to_string()), token(uuid.to_string())])
        }
    }
}

/// `{"B",0|1}`.
pub(crate) fn native_boolean(value: bool) -> NativeValue {
    platform_list(vec![text("B"), token(if value { "1" } else { "0" })])
}

/// The adjustable visibility atom a command, a subsystem and a home-page item
/// share: `{0,{0,{"B",<common>},<count>,<role uuid>,{"B",<value>},…}}`, the
/// per-role overrides in stored order -- the inverse of the exporter's
/// `parse_command_interface_adjustable_visibility`.
pub(crate) fn native_visibility(common: bool, values: &[VisibilityValue]) -> NativeValue {
    let mut inner = Vec::with_capacity(3 + values.len() * 2);
    inner.push(token("0"));
    inner.push(native_boolean(common));
    inner.push(token(values.len().to_string()));
    for value in values {
        inner.push(token(value.role.to_string()));
        inner.push(native_boolean(value.value));
    }
    platform_list(vec![token("0"), platform_list(inner)])
}

fn model_from_native(
    native: &NativeValue,
) -> Result<CommandInterfaceModel, CommandInterfaceCodecError> {
    let fields = required_list(native, "CommandInterface root")?;
    if fields.is_empty() {
        return Err(CommandInterfaceCodecError::InvalidShape(
            "CommandInterface root is empty",
        ));
    }
    exact_token(&fields[0], "7", "CommandInterface root marker")?;
    let mut index = 1usize;

    let visibility_count = section_count(fields, &mut index, "visibility")?;
    let mut commands_visibility = Vec::with_capacity(visibility_count);
    for _ in 0..visibility_count {
        let command = parse_command_reference(field(fields, index, "visibility command")?, true)?;
        index += 1;
        let (common, values) = parse_visibility(field(fields, index, "visibility common")?)?;
        index += 1;
        commands_visibility.push(CommandVisibility {
            command,
            common,
            values,
        });
    }

    let placement_count = section_count(fields, &mut index, "placement")?;
    let mut commands_placement = Vec::with_capacity(placement_count);
    for _ in 0..placement_count {
        let command = parse_command_reference(field(fields, index, "placement command")?, true)?;
        index += 1;
        let command_group = parse_uuid_allow_nil(
            field(fields, index, "placement command group")?,
            "placement command group",
        )?;
        index += 1;
        let placement =
            match required_token(field(fields, index, "placement mode")?, "placement mode")? {
                "0" => CommandPlacementMode::Auto,
                "1" => CommandPlacementMode::Manual,
                _ => return Err(CommandInterfaceCodecError::InvalidShape("placement mode")),
            };
        index += 1;
        commands_placement.push(CommandPlacement {
            command,
            command_group,
            placement,
        });
    }

    let order_count = section_count(fields, &mut index, "command order")?;
    let mut commands_order = Vec::with_capacity(order_count);
    for _ in 0..order_count {
        let command_group = parse_uuid_allow_nil(
            field(fields, index, "order command group")?,
            "order command group",
        )?;
        index += 1;
        let command = parse_command_reference(field(fields, index, "ordered command")?, true)?;
        index += 1;
        commands_order.push(CommandOrder {
            command_group,
            command,
        });
    }

    let subsystem_count = section_count(fields, &mut index, "subsystem order")?;
    let mut subsystems_order = Vec::with_capacity(subsystem_count);
    for _ in 0..subsystem_count {
        subsystems_order.push(parse_uuid_allow_nil(
            field(fields, index, "ordered subsystem")?,
            "ordered subsystem",
        )?);
        index += 1;
    }

    let group_count = section_count(fields, &mut index, "group order")?;
    let mut groups_order = Vec::with_capacity(group_count);
    for _ in 0..group_count {
        groups_order.push(parse_uuid(
            field(fields, index, "ordered group")?,
            "ordered group",
        )?);
        index += 1;
    }

    let subsystem_visibility_count = section_count(fields, &mut index, "subsystem visibility")?;
    let mut subsystems_visibility = Vec::with_capacity(subsystem_visibility_count);
    for _ in 0..subsystem_visibility_count {
        let subsystem = parse_uuid(
            field(fields, index, "visible subsystem")?,
            "visible subsystem",
        )?;
        index += 1;
        let (common, values) = parse_visibility(field(fields, index, "subsystem visibility")?)?;
        index += 1;
        subsystems_visibility.push(SubsystemVisibility {
            subsystem,
            common,
            values,
        });
    }

    if index != fields.len() {
        return Err(CommandInterfaceCodecError::InvalidShape(
            "CommandInterface has an unsupported tail",
        ));
    }

    Ok(CommandInterfaceModel {
        commands_visibility,
        commands_placement,
        commands_order,
        subsystems_order,
        groups_order,
        subsystems_visibility,
    })
}

fn field<'a>(
    fields: &'a [NativeValue],
    index: usize,
    name: &'static str,
) -> Result<&'a NativeValue, CommandInterfaceCodecError> {
    fields
        .get(index)
        .ok_or(CommandInterfaceCodecError::InvalidShape(name))
}

fn section_count(
    fields: &[NativeValue],
    index: &mut usize,
    name: &'static str,
) -> Result<usize, CommandInterfaceCodecError> {
    let marker = required_token(field(fields, *index, name)?, name)?;
    *index += 1;
    match marker {
        "0" => Ok(0),
        "1" => {
            let count = required_token(field(fields, *index, name)?, name)?
                .parse::<usize>()
                .map_err(|_| CommandInterfaceCodecError::InvalidShape(name))?;
            *index += 1;
            if count > MAX_SECTION_ITEMS {
                return Err(CommandInterfaceCodecError::LimitExceeded(name));
            }
            Ok(count)
        }
        _ => Err(CommandInterfaceCodecError::InvalidShape(name)),
    }
}

fn parse_command_reference(
    value: &NativeValue,
    allow_empty: bool,
) -> Result<CommandReference, CommandInterfaceCodecError> {
    let fields = required_list(value, "command reference")?;
    if allow_empty && fields.len() == 1 {
        exact_token(&fields[0], "0", "empty command reference")?;
        return Ok(CommandReference::Empty);
    }
    if fields.len() != 2 {
        return Err(CommandInterfaceCodecError::InvalidShape(
            "command reference arity",
        ));
    }
    let kind = required_token(&fields[0], "command kind")?
        .parse::<u32>()
        .map_err(|_| CommandInterfaceCodecError::InvalidShape("command kind"))?;
    let uuid = parse_uuid(&fields[1], "command UUID")?;
    Ok(CommandReference::Resolved { kind, uuid })
}

fn parse_uuid(
    value: &NativeValue,
    field: &'static str,
) -> Result<ObjectUuid, CommandInterfaceCodecError> {
    if required_token(value, field)? == NIL_UUID {
        return Err(CommandInterfaceCodecError::InvalidShape(field));
    }
    parse_uuid_allow_nil(value, field)
}

fn parse_uuid_allow_nil(
    value: &NativeValue,
    field: &'static str,
) -> Result<ObjectUuid, CommandInterfaceCodecError> {
    let value = required_token(value, field)?;
    let uuid = ObjectUuid::parse(value)
        .map_err(|_| CommandInterfaceCodecError::InvalidUuid(value.to_owned()))?;
    Ok(uuid)
}

fn parse_boolean(value: &NativeValue) -> Result<bool, CommandInterfaceCodecError> {
    let boolean = exact_list(value, 2, "command visibility boolean")?;
    if required_text(&boolean[0], "command visibility type")? != "B" {
        return Err(CommandInterfaceCodecError::InvalidShape(
            "command visibility type",
        ));
    }
    match required_token(&boolean[1], "command visibility value")? {
        "0" => Ok(false),
        "1" => Ok(true),
        _ => Err(CommandInterfaceCodecError::InvalidShape(
            "command visibility value",
        )),
    }
}

fn parse_visibility(
    value: &NativeValue,
) -> Result<(bool, Vec<VisibilityValue>), CommandInterfaceCodecError> {
    let outer = exact_list(value, 2, "command visibility wrapper")?;
    exact_token(&outer[0], "0", "command visibility wrapper marker")?;
    let inner = required_list(&outer[1], "command visibility payload")?;
    if inner.len() < 3 {
        return Err(CommandInterfaceCodecError::InvalidShape(
            "command visibility payload",
        ));
    }
    exact_token(&inner[0], "0", "command visibility payload marker")?;
    let common = parse_boolean(&inner[1])?;
    let count = required_token(&inner[2], "command visibility override count")?
        .parse::<usize>()
        .map_err(|_| {
            CommandInterfaceCodecError::InvalidShape("command visibility override count")
        })?;
    if count > MAX_SECTION_ITEMS || inner.len() != 3 + count * 2 {
        return Err(CommandInterfaceCodecError::InvalidShape(
            "command visibility override count",
        ));
    }
    let mut values = Vec::with_capacity(count);
    for pair in inner[3..].chunks_exact(2) {
        values.push(VisibilityValue {
            role: parse_uuid(&pair[0], "command visibility role")?,
            value: parse_boolean(&pair[1])?,
        });
    }
    Ok((common, values))
}

/// Bounds every section. Repeated entries are not an error: the platform
/// stores them -- ERP УХ `Subsystems/Склад/Subsystems/КонтрольКачестваТоваров`
/// orders two documents twice each, and two `Subsystems/ПодсистемыУХ/…` rows
/// place one command five times -- and exports every one.
fn validate_model(model: &CommandInterfaceModel) -> Result<(), CommandInterfaceCodecError> {
    for (name, count) in [
        ("visibility", model.commands_visibility.len()),
        ("placement", model.commands_placement.len()),
        ("command order", model.commands_order.len()),
        ("subsystem order", model.subsystems_order.len()),
        ("group order", model.groups_order.len()),
        ("subsystem visibility", model.subsystems_visibility.len()),
    ] {
        if count > MAX_SECTION_ITEMS {
            return Err(CommandInterfaceCodecError::LimitExceeded(name));
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandInterfaceCodecError {
    Profile(BodyProfileError),
    Native(String),
    InvalidShape(&'static str),
    InvalidModel(&'static str),
    InvalidUuid(String),
    LimitExceeded(&'static str),
}

impl Display for CommandInterfaceCodecError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Profile(source) => Display::fmt(source, formatter),
            Self::Native(reason) => {
                write!(
                    formatter,
                    "native CommandInterface codec rejected data: {reason}"
                )
            }
            Self::InvalidShape(field) => {
                write!(formatter, "invalid CommandInterface body: {field}")
            }
            Self::InvalidModel(field) => {
                write!(formatter, "invalid CommandInterface model: {field}")
            }
            Self::InvalidUuid(value) => {
                write!(formatter, "invalid CommandInterface UUID `{value}`")
            }
            Self::LimitExceeded(section) => write!(
                formatter,
                "CommandInterface {section} exceeds the standalone item limit"
            ),
        }
    }
}

impl Error for CommandInterfaceCodecError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Profile(source) => Some(source),
            _ => None,
        }
    }
}

impl From<BodyProfileError> for CommandInterfaceCodecError {
    fn from(source: BodyProfileError) -> Self {
        Self::Profile(source)
    }
}

impl From<NativeError> for CommandInterfaceCodecError {
    fn from(source: NativeError) -> Self {
        Self::Native(source.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::families::native::deflate_bytes;

    const COMMAND_A: &str = "c322dc4a-30f3-4d23-9824-4a01c496408d";
    const COMMAND_B: &str = "6e5307ea-8fbf-4177-bcc8-f163a6ce1b40";
    const GROUP: &str = "1af6d528-0b86-4fba-ab95-bd7475db03ba";
    const SUBSYSTEM: &str = "57f6e29d-0261-4e6d-b689-8e27eef0855a";
    const ROLE: &str = "6e0ab1d5-108a-4cbb-92d4-aebb5ecd3af5";

    fn uuid(value: &str) -> ObjectUuid {
        ObjectUuid::parse(value).unwrap()
    }

    fn fixture_model() -> CommandInterfaceModel {
        let first = CommandReference::resolved(0, uuid(COMMAND_A));
        let second = CommandReference::resolved(0, uuid(COMMAND_B));
        CommandInterfaceModel {
            commands_visibility: vec![
                CommandVisibility {
                    command: first.clone(),
                    common: false,
                    values: vec![VisibilityValue {
                        role: uuid(ROLE),
                        value: true,
                    }],
                },
                CommandVisibility {
                    command: second.clone(),
                    common: true,
                    values: Vec::new(),
                },
            ],
            commands_placement: vec![CommandPlacement {
                command: first.clone(),
                command_group: uuid(GROUP),
                placement: CommandPlacementMode::Auto,
            }],
            commands_order: vec![CommandOrder {
                command_group: uuid(GROUP),
                command: second,
            }],
            subsystems_order: vec![uuid(SUBSYSTEM)],
            groups_order: vec![uuid(GROUP)],
            subsystems_visibility: vec![SubsystemVisibility {
                subsystem: uuid(SUBSYSTEM),
                common: false,
                values: Vec::new(),
            }],
        }
    }

    #[test]
    fn all_six_sections_roundtrip_without_a_base_blob() {
        let profile = CommandInterfaceCodecProfile::fixture();
        let model = fixture_model();
        let first = compile_command_interface(&profile, &model).unwrap();
        let decoded = decode_command_interface(&profile, &first).unwrap();
        assert_eq!(decoded.model(), &model);
        let second = compile_command_interface(&profile, decoded.model()).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn writes_the_platform_layout() {
        let model = CommandInterfaceModel {
            commands_visibility: vec![CommandVisibility {
                command: CommandReference::resolved(0, uuid(COMMAND_A)),
                common: false,
                values: vec![VisibilityValue {
                    role: uuid(ROLE),
                    value: true,
                }],
            }],
            ..CommandInterfaceModel::default()
        };
        let plain = command_interface_plaintext(&model).unwrap();
        assert_eq!(
            String::from_utf8(plain).unwrap(),
            format!(
                "\u{feff}{{7,1,1,\r\n{{0,{COMMAND_A}}},\r\n{{0,\r\n{{0,\r\n{{\"B\",0}},1,{ROLE},\r\n{{\"B\",1}}\r\n}}\r\n}},0,0,0,0,0}}"
            )
        );
    }

    #[test]
    fn nil_command_group_is_preserved_in_placement_and_order() {
        let profile = CommandInterfaceCodecProfile::fixture();
        let plain = format!(
            "\u{feff}{{7,0,1,1,{{0,{COMMAND_A}}},{NIL_UUID},1,1,1,{NIL_UUID},{{0,{COMMAND_A}}},0,0,0}}"
        );
        let blob = deflate_bytes(plain.as_bytes()).unwrap();

        let decoded = decode_command_interface(&profile, &blob).unwrap();

        assert_eq!(
            decoded.model().commands_placement[0]
                .command_group
                .to_string(),
            NIL_UUID
        );
        assert_eq!(
            decoded.model().commands_order[0].command_group.to_string(),
            NIL_UUID
        );
        assert_eq!(decoded.plaintext().unwrap(), plain.as_bytes());
    }

    #[test]
    fn missing_bom_is_rejected() {
        let profile = CommandInterfaceCodecProfile::fixture();
        let no_bom = format!("{{7,1,1,{{0,{COMMAND_A}}},{{0,{{0,{{\"B\",1}},0}}}},0,0,0,0,0}}");
        let blob = deflate_bytes(no_bom.as_bytes()).unwrap();
        assert!(decode_command_interface(&profile, &blob).is_err());
    }
}
