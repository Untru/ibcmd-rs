//! The export direction for the common objects and services: a stored row
//! -> the object's XML DOM, and what the row contributes to a name index.
//!
//! CommonModule, CommonPicture, CommonTemplate, CommonCommand, CommandGroup,
//! Role, XDTOPackage, StyleItem, Style, PaletteColor (8.5), WebService,
//! HTTPService, WSReference, IntegrationService, Bot. Each decoder walks the
//! layout of `common.rs` backwards and reads its code tables (a child module
//! sees the private ones); colours and fonts invert the form body encoder's
//! tables (`form_native`), which the load direction writes them with.

use anyhow::{Result, anyhow, bail};

use super::{
    BORDER_TYPE, COLOR_TYPE, COMMAND_GROUP_CATEGORIES, DATA_LOCK_CONTROL_MODES, FONT_TYPE,
    HTTP_METHOD_CODES, HTTP_METHODS, HTTP_URL_TEMPLATES, INTEGRATION_CHANNELS,
    MAIN_SERVER_UNAVAILABLE_BEHAVIORS, MESSAGE_DIRECTIONS, PARAMETER_USE_MODES, REPRESENTATIONS,
    RETURN_VALUES_REUSE, REUSE_SESSIONS, STD_PICTURE_CODES, TEMPLATE_TYPES, TRANSFER_DIRECTIONS,
    WEB_OPERATIONS, WEB_PARAMETERS, standard_namespace,
};
use crate::compiler::bodies::form_native::{
    PALETTE_COLORS_8_5_1, PLATFORM_STYLE_COLOR_CODES, PLATFORM_STYLE_FONT_CODES, WEB_COLOR_CODES,
    WINDOWS_COLOR_CODES,
};
use crate::metadata_model::brace::Brace;
use crate::metadata_model::export::values::{
    Header, bool_text, code_text, header, header_elements, localized_element, type_element,
};
use crate::metadata_model::export::{
    Build, ExportContext, GeneratedTypeName, NameIndex, ObjectNames, atom, el, item, leaf, list,
    number, short, string, xml_text,
};
use crate::metadata_model::xml::Element;

/// Decodes one stored row of a common kind into the object's element.
pub(crate) fn decode(kind: &str, row: &Brace, context: &ExportContext) -> Result<Element> {
    if kind == "ExternalDataSource" {
        return crate::metadata_model::external_data_source::decode(row);
    }
    let root = list(row)?;
    if atom(item(root, 0)?)? != "1" {
        bail!("not a descriptor row: {}", short(row));
    }
    let payload = item(root, 1)?;
    let names = &context.names;
    match kind {
        "CommonModule" => common_module(payload),
        "CommonPicture" => common_picture(payload),
        "CommonTemplate" => common_template(payload),
        "Role" => header_only("Role", payload, "6"),
        "Style" => header_only("Style", payload, "3"),
        "XDTOPackage" => xdto_package(payload),
        "PaletteColor" => palette_color(payload, names),
        "StyleItem" => style_item(payload, names),
        "CommandGroup" => command_group(payload, names),
        "CommonCommand" => common_command(payload, context),
        "Bot" => bot(payload, names),
        "WSReference" => ws_reference(payload),
        "IntegrationService" => integration_service(root),
        "HTTPService" => http_service(root),
        "WebSocketClient" => {
            crate::metadata_model::websocket_client::WebSocketClient::from_brace(row)?.to_xml()
        }
        "WebService" => web_service(root, names),
        other => bail!("{other} is not a common kind"),
    }
}

/// The payload's fields, its version checked.
fn record<'a>(payload: &'a Brace, version: &str, kind: &str) -> Result<&'a [Brace]> {
    let fields = list(payload)?;
    if atom(item(fields, 0)?)? != version {
        bail!("not a {kind} record: {}", short(payload));
    }
    Ok(fields)
}

/// `<Properties>` with the header's `Name`, `Synonym`, `Comment`.
fn header_properties(head: &Header) -> Result<Element> {
    let [name, synonym, comment] = header_elements(head)?;
    Ok(el("Properties").child(name).child(synonym).child(comment))
}

fn object(kind: &str, uuid: &str) -> Element {
    el(kind).attr("uuid", uuid)
}

fn flag(name: &str, node: &Brace) -> Result<Element> {
    Ok(leaf(name, bool_text(node)?))
}

fn coded(name: &str, node: &Brace, table: &[(&'static str, i64)]) -> Result<Element> {
    Ok(leaf(name, code_text(node, table)?))
}

fn text(name: &str, node: &Brace) -> Result<Element> {
    Ok(leaf(name, xml_text(string(node)?)))
}

/// `<xr:GeneratedType name=.. category="Manager">`.
fn manager_type(name: String, type_id: &Brace, value_id: &Brace) -> Result<Element> {
    Ok(el("xr:GeneratedType")
        .attr("name", name)
        .attr("category", "Manager")
        .child(leaf("xr:TypeId", atom(type_id)?))
        .child(leaf("xr:ValueId", atom(value_id)?)))
}

/// `{<class>,<n>,<item>...}` -> the items.
fn collection<'a>(node: &'a Brace, class: &str) -> Result<&'a [Brace]> {
    let fields = list(node)?;
    if atom(item(fields, 0)?)? != class {
        bail!("expected collection {class}: {}", short(node));
    }
    let count = number(item(fields, 1)?)? as usize;
    fields
        .get(2..2 + count)
        .ok_or_else(|| anyhow!("short collection {class}"))
}

/// The row's only child collection: `{1,<payload>,1,<collection>}`.
fn only_collection<'a>(root: &'a [Brace], class: &str) -> Result<&'a [Brace]> {
    if number(item(root, 2)?)? != 1 {
        bail!("expected one child collection");
    }
    collection(item(root, 3)?, class)
}

// ---------------------------------------------------------------------------
// Simple records.

/// `{12,<header>,ordinary,server,external,privileged,global,managed,reuse,
/// server call}`
fn common_module(payload: &Brace) -> Result<Element> {
    let fields = record(payload, "12", "CommonModule")?;
    let head = header(item(fields, 1)?)?;
    let properties = header_properties(&head)?
        .child(flag("Global", item(fields, 6)?)?)
        .child(flag("ClientManagedApplication", item(fields, 7)?)?)
        .child(flag("Server", item(fields, 3)?)?)
        .child(flag("ExternalConnection", item(fields, 4)?)?)
        .child(flag("ClientOrdinaryApplication", item(fields, 2)?)?)
        .child(flag("ServerCall", item(fields, 9)?)?)
        .child(flag("Privileged", item(fields, 5)?)?)
        .child(coded(
            "ReturnValuesReuse",
            item(fields, 8)?,
            RETURN_VALUES_REUSE,
        )?);
    Ok(object("CommonModule", &head.uuid).child(properties))
}

/// `{4,<header>,choice,appearance}`
fn common_picture(payload: &Brace) -> Result<Element> {
    let fields = record(payload, "4", "CommonPicture")?;
    let head = header(item(fields, 1)?)?;
    let properties = header_properties(&head)?
        .child(flag("AvailabilityForChoice", item(fields, 2)?)?)
        .child(flag("AvailabilityForAppearance", item(fields, 3)?)?);
    Ok(object("CommonPicture", &head.uuid).child(properties))
}

/// `{4,<header>,type}`
fn common_template(payload: &Brace) -> Result<Element> {
    let fields = record(payload, "4", "CommonTemplate")?;
    let head = header(item(fields, 1)?)?;
    let properties =
        header_properties(&head)?.child(coded("TemplateType", item(fields, 2)?, TEMPLATE_TYPES)?);
    Ok(object("CommonTemplate", &head.uuid).child(properties))
}

/// A role `{6,<header>,1,0,0}`, a style `{3,<header>}`: the header alone.
fn header_only(kind: &str, payload: &Brace, version: &str) -> Result<Element> {
    let fields = record(payload, version, kind)?;
    let head = header(item(fields, 1)?)?;
    Ok(object(kind, &head.uuid).child(header_properties(&head)?))
}

/// `{1,<header>,"Namespace"}`
fn xdto_package(payload: &Brace) -> Result<Element> {
    let fields = record(payload, "1", "XDTOPackage")?;
    let head = header(item(fields, 1)?)?;
    let properties = header_properties(&head)?.child(text("Namespace", item(fields, 2)?)?);
    Ok(object("XDTOPackage", &head.uuid).child(properties))
}

/// `{2,{"LocationURL",0},<header>,<manager type>,<manager value>}`
fn ws_reference(payload: &Brace) -> Result<Element> {
    let fields = record(payload, "2", "WSReference")?;
    let head = header(item(fields, 2)?)?;
    let location = item(list(item(fields, 1)?)?, 0)?;
    let internal = el("InternalInfo").child(manager_type(
        format!("WSReferenceManager.{}", head.name),
        item(fields, 3)?,
        item(fields, 4)?,
    )?);
    let properties = header_properties(&head)?.child(text("LocationURL", location)?);
    Ok(object("WSReference", &head.uuid)
        .child(internal)
        .child(properties))
}

// ---------------------------------------------------------------------------
// Colours, fonts, borders.

/// A colour tuple (`{3,<space>,{..}}`, 8.5 `{4,<space>,{..},<kind>}`) -> its
/// XML spelling: the inverse of `form_native::format_native_color`.
fn color_text(node: &Brace, names: &NameIndex) -> Result<String> {
    let fields = list(node)?;
    let space = atom(item(fields, 1)?)?;
    let value = list(item(fields, 2)?)?;
    let (version, kind) = (
        atom(item(fields, 0)?)?,
        fields.get(3).map(atom).transpose()?,
    );
    let code = |table: &[(&'static str, &'static str)]| -> Result<&'static str> {
        let code = atom(item(value, 0)?)?;
        table
            .iter()
            .find_map(|(name, candidate)| (*candidate == code).then_some(*name))
            .ok_or_else(|| anyhow!("unknown colour {}", short(node)))
    };
    if version == "4" && kind == Some("5") {
        let index = number(item(value, 0)?)?;
        let name = PALETTE_COLORS_8_5_1
            .iter()
            .find_map(|(name, candidate)| (i64::from(*candidate) == index).then_some(*name))
            .ok_or_else(|| anyhow!("unknown palette colour {}", short(node)))?;
        return Ok(format!("pal:{name}"));
    }
    if !(version == "3" || (version == "4" && kind == Some(space))) {
        bail!("unsupported colour {}", short(node));
    }
    Ok(match (space, value) {
        ("4", [code]) if atom(code)? == "-1" => "auto".to_string(),
        ("0", [code]) => {
            let bgr = number(code)?;
            format!(
                "#{:02X}{:02X}{:02X}",
                bgr & 0xff,
                (bgr >> 8) & 0xff,
                (bgr >> 16) & 0xff
            )
        }
        ("1", [_]) => format!("win:{}", code(WINDOWS_COLOR_CODES)?),
        ("2", [_]) => format!("web:{}", code(WEB_COLOR_CODES)?),
        ("3", [_]) => format!("style:{}", code(PLATFORM_STYLE_COLOR_CODES)?),
        ("3", [zero, uuid]) if atom(zero)? == "0" => {
            format!("style:{}", style_item_name(atom(uuid)?, names)?)
        }
        _ => bail!("unsupported colour {}", short(node)),
    })
}

/// A style item's name by uuid (`StyleItem.X` -> `X`).
fn style_item_name(uuid: &str, names: &NameIndex) -> Result<String> {
    let full = names
        .name(uuid)
        .ok_or_else(|| anyhow!("no name for style item {uuid}"))?;
    full.strip_prefix("StyleItem.")
        .map(str::to_string)
        .ok_or_else(|| anyhow!("{full} is not a style item"))
}

/// A font tuple (`{7,...}`, 8.5 `{8,...}`) -> the `<Value>` attributes in the
/// order the platform writes them: the inverse of
/// `form_native::format_native_font`.
fn font_attributes(node: &Brace, names: &NameIndex) -> Result<Vec<(&'static str, String)>> {
    let fields = list(node)?;
    if !matches!(atom(item(fields, 0)?)?, "7" | "8") {
        bail!("not a font: {}", short(node));
    }
    let kind = atom(item(fields, 1)?)?;
    let flag = |node: &Brace| -> Result<String> { Ok(bool_text(node)?.to_string()) };
    let height = |node: &Brace| -> Result<String> {
        let tenths = number(node)?;
        if tenths % 10 != 0 {
            bail!("font height {tenths} is not whole points");
        }
        Ok((tenths / 10).to_string())
    };
    let bold = |node: &Brace| -> Result<String> {
        Ok(match atom(node)? {
            "700" => "true",
            "400" => "false",
            other => bail!("unknown font weight {other}"),
        }
        .to_string())
    };
    if kind == "0" {
        // `{7,0,575,height,0,0,0,weight,italic,underline,strikeout,0,0,0,0,0,
        // "face",1,scale}`, 8.5 appending `0`.
        if atom(item(fields, 2)?)? != "575" {
            bail!("unsupported absolute font {}", short(node));
        }
        return Ok(vec![
            ("faceName", xml_text(string(item(fields, 16)?)?)),
            ("height", height(item(fields, 3)?)?),
            ("bold", bold(item(fields, 7)?)?),
            ("italic", flag(item(fields, 8)?)?),
            ("underline", flag(item(fields, 9)?)?),
            ("strikeout", flag(item(fields, 10)?)?),
            ("kind", "Absolute".to_string()),
            ("scale", atom(item(fields, 18)?)?.to_string()),
        ]);
    }
    let mask = number(item(fields, 2)?)?;
    let mut position = 3;
    let mut out = Vec::new();
    let kind_text = match kind {
        "1" | "2" => {
            let slot = list(item(fields, position)?)?;
            position += 1;
            let reference = match (kind, slot) {
                ("1", [code]) => match atom(code)? {
                    "0" => "sys:DefaultGUIFont".to_string(),
                    "2" => "sys:ANSIFixedFont".to_string(),
                    other => bail!("unknown Windows font {other}"),
                },
                ("2", [code]) => {
                    let code = atom(code)?;
                    let name = PLATFORM_STYLE_FONT_CODES
                        .iter()
                        .find_map(|(name, candidate)| (*candidate == code).then_some(*name))
                        .ok_or_else(|| anyhow!("unknown style font {code}"))?;
                    format!("style:{name}")
                }
                ("2", [zero, uuid]) if atom(zero)? == "0" => {
                    format!("style:{}", style_item_name(atom(uuid)?, names)?)
                }
                _ => bail!("unsupported font reference {}", short(node)),
            };
            out.push(("ref", reference));
            if kind == "1" {
                "WindowsFont"
            } else {
                "StyleItem"
            }
        }
        "3" => "AutoFont",
        other => bail!("unknown font kind {other}"),
    };
    // The values follow the mask: height, weight, italic, underline,
    // strikeout, face name; the XML spells the face name first.
    let mut take = |bit: i64| -> Option<&Brace> {
        if mask & (1 << bit) == 0 {
            return None;
        }
        let value = fields.get(position);
        position += 1;
        value
    };
    let mut values: Vec<(&'static str, String)> = Vec::new();
    if let Some(value) = take(1) {
        values.push(("height", height(value)?));
    }
    if let Some(value) = take(2) {
        values.push(("bold", bold(value)?));
    }
    for (bit, name) in [(3, "italic"), (4, "underline"), (5, "strikeout")] {
        if let Some(value) = take(bit) {
            values.push((name, flag(value)?));
        }
    }
    if let Some(value) = take(0) {
        out.push(("faceName", xml_text(string(value)?)));
    }
    out.extend(values);
    out.push(("kind", kind_text.to_string()));
    if atom(item(fields, position)?)? != "1" {
        bail!("unexpected font tail {}", short(node));
    }
    if mask & (1 << 9) != 0 {
        out.push(("scale", atom(item(fields, position + 1)?)?.to_string()));
    }
    Ok(out)
}

/// `ControlBorderType` by the style code a border stores.
const BORDER_STYLES: &[(&str, i64)] = &[
    ("WithoutBorder", 0),
    ("Single", 1),
    ("Embossed", 2),
    ("Indented", 3),
    ("Underline", 4),
    ("Overline", 7),
    ("Double", 200),
];

/// `{"#",<type>,<n>,<value>[,0]}` -> the value member.
fn typed_payload<'a>(node: &'a Brace, type_id: &str) -> Result<&'a Brace> {
    let fields = list(node)?;
    if fields.first().and_then(Brace::as_str) != Some("#") || atom(item(fields, 1)?)? != type_id {
        bail!("expected a {type_id} value: {}", short(node));
    }
    item(fields, 3)
}

/// `{3,<kind>,<value>,<header>}`
fn style_item(payload: &Brace, names: &NameIndex) -> Result<Element> {
    let fields = record(payload, "3", "StyleItem")?;
    let head = header(item(fields, 3)?)?;
    let value = item(fields, 2)?;
    let (type_name, element) = match atom(item(fields, 1)?)? {
        "0" => (
            "Color",
            leaf(
                "Value",
                color_text(typed_payload(value, COLOR_TYPE)?, names)?,
            )
            .attr("type", "v8ui:Color"),
        ),
        "1" => {
            let mut element = el("Value").attr("type", "v8ui:Font");
            for (key, text) in font_attributes(typed_payload(value, FONT_TYPE)?, names)? {
                element = element.attr(key, text);
            }
            ("Font", element)
        }
        "2" => {
            let border = list(typed_payload(value, BORDER_TYPE)?)?;
            let style = code_text(item(border, 3)?, BORDER_STYLES)?;
            (
                "Border",
                el("Value")
                    .attr("type", "v8ui:Border")
                    .attr("width", atom(item(border, 4)?)?)
                    .child(leaf("v8ui:style", style).attr("type", "v8ui:ControlBorderType")),
            )
        }
        other => bail!("unknown style item kind {other}"),
    };
    let properties = header_properties(&head)?
        .child(leaf("Type", type_name))
        .child(element);
    Ok(object("StyleItem", &head.uuid).child(properties))
}

/// `{0,<header>,<colour>}` (8.5)
fn palette_color(payload: &Brace, names: &NameIndex) -> Result<Element> {
    let fields = record(payload, "0", "PaletteColor")?;
    let head = header(item(fields, 1)?)?;
    let properties =
        header_properties(&head)?.child(leaf("Color", color_text(item(fields, 2)?, names)?));
    Ok(object("PaletteColor", &head.uuid).child(properties))
}

// ---------------------------------------------------------------------------
// Pictures, commands, groups, bots.

/// `{4,<present>,{0}|{<code>}|{0,<uuid>},"",<x>,<y>,<load transparent>,0,""}`
/// -> `<Picture>`: the inverse of `common::Picture::to_brace`.
fn picture(node: &Brace, names: &NameIndex) -> Result<Element> {
    let fields = list(node)?;
    if atom(item(fields, 0)?)? != "4" {
        bail!("not a picture: {}", short(node));
    }
    if atom(item(fields, 1)?)? == "0" {
        return Ok(el("Picture"));
    }
    let reference = match list(item(fields, 2)?)? {
        [code] => {
            let code = number(code)?;
            let name = STD_PICTURE_CODES
                .iter()
                .find_map(|(name, candidate)| (*candidate == code).then_some(*name))
                .ok_or_else(|| anyhow!("unknown picture code {code}"))?;
            format!("StdPicture.{name}")
        }
        [zero, uuid] if atom(zero)? == "0" => {
            let uuid = atom(uuid)?;
            match crate::mssql_dump::standard_picture_name(uuid) {
                Some(name) => name.to_string(),
                None => match names.name(uuid) {
                    Some(name) => name.to_string(),
                    // A common picture that no longer exists.
                    None => format!("0:{uuid}"),
                },
            }
        }
        _ => bail!("unsupported picture {}", short(node)),
    };
    let mut element = el("Picture")
        .child(leaf("xr:Ref", reference))
        .child(leaf("xr:LoadTransparent", bool_text(item(fields, 6)?)?));
    let (x, y) = (atom(item(fields, 4)?)?, atom(item(fields, 5)?)?);
    if x != "-1" || y != "-1" {
        element
            .children
            .push(el("xr:TransparentPixel").attr("x", x).attr("y", y));
    }
    Ok(element)
}

/// `{3,<picture>,category,representation,<tooltip>,{0},<header>}`
fn command_group(payload: &Brace, names: &NameIndex) -> Result<Element> {
    let fields = record(payload, "3", "CommandGroup")?;
    let head = header(item(fields, 6)?)?;
    let properties = header_properties(&head)?
        .child(coded("Representation", item(fields, 3)?, REPRESENTATIONS)?)
        .child(localized_element("ToolTip", item(fields, 4)?)?)
        .child(picture(item(fields, 1)?, names)?)
        .child(coded(
            "Category",
            item(fields, 2)?,
            COMMAND_GROUP_CATEGORIES,
        )?);
    Ok(object("CommandGroup", &head.uuid).child(properties))
}

/// A command's `{1,<group>}` -> a platform group's name, `CommandGroup.X`,
/// or `0:<uuid>`: the inverse of `common::command_group_uuid`.
fn command_group_text(node: &Brace, names: &NameIndex) -> Result<String> {
    let uuid = atom(item(list(node)?, 1)?)?;
    if let Some((_, name)) = crate::mssql_dump::COMMON_COMMAND_GROUPS
        .iter()
        .find(|(candidate, _)| *candidate == uuid)
    {
        return Ok(name.to_string());
    }
    Ok(match names.name(uuid) {
        Some(name) => name.to_string(),
        None => format!("0:{uuid}"),
    })
}

/// `{2,{1,{2,<uuid>,<class>},{9,<picture>,representation,<tooltip>,1,
/// <shortcut>,help,{1,<group>},<parameter type>,<header>,modifies data,use
/// mode,behaviour}}}`
fn common_command(payload: &Brace, context: &ExportContext) -> Result<Element> {
    let names = &context.names;
    let fields = record(payload, "2", "CommonCommand")?;
    let body = list(item(fields, 1)?)?;
    let details = record(item(body, 2)?, "9", "CommonCommand")?;
    let head = header(item(details, 9)?)?;
    let properties = header_properties(&head)?
        .child(leaf("Group", command_group_text(item(details, 7)?, names)?))
        .child(coded("Representation", item(details, 2)?, REPRESENTATIONS)?)
        .child(localized_element("ToolTip", item(details, 3)?)?)
        .child(picture(item(details, 1)?, names)?)
        .child(leaf(
            "Shortcut",
            crate::metadata_model::objects::export::shortcut(item(details, 5)?)?,
        ))
        .child(flag("IncludeHelpInContents", item(details, 6)?)?)
        .child(type_element(
            "CommandParameterType",
            item(details, 8)?,
            names,
        )?)
        .child(coded(
            "ParameterUseMode",
            item(details, 11)?,
            PARAMETER_USE_MODES,
        )?)
        .child(flag("ModifiesData", item(details, 10)?)?)
        .child(coded(
            "OnMainServerUnavalableBehavior",
            item(details, 12)?,
            MAIN_SERVER_UNAVAILABLE_BEHAVIORS,
        )?);
    Ok(object("CommonCommand", &head.uuid).child(properties))
}

/// `{1,<header>,predefined,<picture>}`
fn bot(payload: &Brace, names: &NameIndex) -> Result<Element> {
    let fields = record(payload, "1", "Bot")?;
    let head = header(item(fields, 1)?)?;
    let properties = header_properties(&head)?
        .child(flag("Predefined", item(fields, 2)?)?)
        .child(picture(item(fields, 3)?, names)?);
    Ok(object("Bot", &head.uuid).child(properties))
}

// ---------------------------------------------------------------------------
// Services.

/// `{1,{0,<header>,<type>,<value>,"address"},1,{acb7e81f-...,N,
/// {{1,<header>,<type>,<value>,"name","handler",direction,transactioned},0}...}}`
fn integration_service(root: &[Brace]) -> Result<Element> {
    let fields = record(item(root, 1)?, "0", "IntegrationService")?;
    let head = header(item(fields, 1)?)?;
    let internal = el("InternalInfo").child(manager_type(
        format!("IntegrationServiceManager.{}", head.name),
        item(fields, 2)?,
        item(fields, 3)?,
    )?);
    let properties = header_properties(&head)?
        .child(text("ExternalIntegrationServiceAddress", item(fields, 4)?)?);
    let mut children = el("ChildObjects");
    for stored in only_collection(root, INTEGRATION_CHANNELS)? {
        let channel = record(item(list(stored)?, 0)?, "1", "IntegrationServiceChannel")?;
        let channel_head = header(item(channel, 1)?)?;
        let internal = el("InternalInfo").child(manager_type(
            format!(
                "IntegrationServiceChannelManager.{}.{}",
                head.name, channel_head.name
            ),
            item(channel, 2)?,
            item(channel, 3)?,
        )?);
        let channel_properties = header_properties(&channel_head)?
            .child(text(
                "ExternalIntegrationServiceChannelName",
                item(channel, 4)?,
            )?)
            .child(coded(
                "MessageDirection",
                item(channel, 6)?,
                MESSAGE_DIRECTIONS,
            )?)
            .child(text("ReceiveMessageProcessing", item(channel, 5)?)?)
            .child(flag("Transactioned", item(channel, 7)?)?);
        children.children.push(
            object("IntegrationServiceChannel", &channel_head.uuid)
                .child(internal)
                .child(channel_properties),
        );
    }
    Ok(object("IntegrationService", &head.uuid)
        .child(internal)
        .child(properties)
        .child(children))
}

/// `{1,{2,"RootURL",<header>,reuse,max age},1,{ec6896c2-...,N,
/// {{0,"template",<header>},1,{21c96ea8-...,M,
/// {{0,"handler",code,<header>},0}...}}...}}`
fn http_service(root: &[Brace]) -> Result<Element> {
    let fields = record(item(root, 1)?, "2", "HTTPService")?;
    let head = header(item(fields, 2)?)?;
    let properties = header_properties(&head)?
        .child(text("RootURL", item(fields, 1)?)?)
        .child(coded("ReuseSessions", item(fields, 3)?, REUSE_SESSIONS)?)
        .child(leaf("SessionMaxAge", atom(item(fields, 4)?)?));
    let mut children = el("ChildObjects");
    for stored in only_collection(root, HTTP_URL_TEMPLATES)? {
        let template = list(stored)?;
        let record = self::record(item(template, 0)?, "0", "URLTemplate")?;
        let template_head = header(item(record, 2)?)?;
        let mut methods = el("ChildObjects");
        if number(item(template, 1)?)? != 1 {
            bail!("an URL template has one collection");
        }
        for stored_method in collection(item(template, 2)?, HTTP_METHODS)? {
            let method = self::record(item(list(stored_method)?, 0)?, "0", "Method")?;
            let method_head = header(item(method, 3)?)?;
            methods.children.push(
                object("Method", &method_head.uuid).child(
                    header_properties(&method_head)?
                        .child(coded("HTTPMethod", item(method, 2)?, HTTP_METHOD_CODES)?)
                        .child(text("Handler", item(method, 1)?)?),
                ),
            );
        }
        children.children.push(
            object("URLTemplate", &template_head.uuid)
                .child(
                    header_properties(&template_head)?.child(text("Template", item(record, 1)?)?),
                )
                .child(methods),
        );
    }
    Ok(object("HTTPService", &head.uuid)
        .child(properties)
        .child(children))
}

/// The namespace prefix the metadata root declares for `uri`.
fn root_prefix(uri: &str) -> Option<&'static str> {
    [
        "xs", "v8", "xsi", "v8ui", "app", "cfg", "ent", "xr", "xen", "style", "web", "win", "lf",
        "cmi", "sys", "xpr",
    ]
    .into_iter()
    .find(|prefix| standard_namespace(prefix) == Some(uri))
}

/// `{0,"namespace","name"}` -> an XDTO type element: a root prefix when the
/// root declares the namespace, else `d<depth>p1` declared on the element
/// (`depth` is its 1-based element depth).
fn xdto_type(qname: &str, node: &Brace, depth: usize) -> Result<Element> {
    let fields = list(node)?;
    let uri = string(item(fields, 1)?)?;
    let name = string(item(fields, 2)?)?;
    if uri.is_empty() && name.is_empty() {
        return Ok(el(qname));
    }
    Ok(match root_prefix(uri) {
        Some(prefix) => leaf(qname, format!("{prefix}:{name}")),
        None => {
            let prefix = format!("d{depth}p1");
            let mut element = leaf(qname, format!("{prefix}:{name}"));
            element.namespaces.push((prefix, uri.to_string()));
            element
        }
    })
}

/// `<xr:Item><xr:Presentation/><xr:CheckState>0</xr:CheckState><xr:Value ..>`.
fn xdto_package_item(value_type: &str, value: String) -> Element {
    el("xr:Item")
        .child(el("xr:Presentation"))
        .child(leaf("xr:CheckState", "0"))
        .child(leaf("xr:Value", value).attr("type", value_type))
}

/// `{1,{4,"Namespace",<header>,{0,N,<package refs>},"descriptor",{M,"ns"...},
/// reuse,max age},1,{<operations>,K,{{1,<header>,<type>,nillable,
/// transactioned,"procedure",lock mode},1,{<parameters>,L,{{0,<header>,
/// <type>,nillable,direction},0}...}}...}}`
fn web_service(root: &[Brace], names: &NameIndex) -> Result<Element> {
    let fields = record(item(root, 1)?, "4", "WebService")?;
    let head = header(item(fields, 2)?)?;
    // The XML lists the packages the service names, then the namespaces.
    let mut packages = el("XDTOPackages");
    let references = list(item(fields, 3)?)?;
    let count = match references.get(1) {
        Some(count) => number(count)? as usize,
        None => 0,
    };
    for reference in references.iter().skip(2).take(count) {
        let uuid = atom(item(list(item(list(reference)?, 2)?)?, 1)?)?;
        let name = names
            .name(uuid)
            .map(str::to_string)
            .unwrap_or_else(|| uuid.to_string());
        packages
            .children
            .push(xdto_package_item("xr:MDObjectRef", name));
    }
    let namespaces = list(item(fields, 5)?)?;
    let count = number(item(namespaces, 0)?)? as usize;
    for namespace in namespaces.iter().skip(1).take(count) {
        packages
            .children
            .push(xdto_package_item("xs:string", xml_text(string(namespace)?)));
    }
    let properties = header_properties(&head)?
        .child(text("Namespace", item(fields, 1)?)?)
        .child(packages)
        .child(text("DescriptorFileName", item(fields, 4)?)?)
        .child(coded("ReuseSessions", item(fields, 6)?, REUSE_SESSIONS)?)
        .child(leaf("SessionMaxAge", atom(item(fields, 7)?)?));
    let mut children = el("ChildObjects");
    for stored in only_collection(root, WEB_OPERATIONS)? {
        let operation = list(stored)?;
        let record = self::record(item(operation, 0)?, "1", "Operation")?;
        let operation_head = header(item(record, 1)?)?;
        if number(item(operation, 1)?)? != 1 {
            bail!("an operation has one collection");
        }
        let mut parameters = el("ChildObjects");
        for stored_parameter in collection(item(operation, 2)?, WEB_PARAMETERS)? {
            let parameter = self::record(item(list(stored_parameter)?, 0)?, "0", "Parameter")?;
            let parameter_head = header(item(parameter, 1)?)?;
            parameters.children.push(
                object("Parameter", &parameter_head.uuid).child(
                    header_properties(&parameter_head)?
                        .child(xdto_type("XDTOValueType", item(parameter, 2)?, 8)?)
                        .child(flag("Nillable", item(parameter, 3)?)?)
                        .child(coded(
                            "TransferDirection",
                            item(parameter, 4)?,
                            TRANSFER_DIRECTIONS,
                        )?),
                ),
            );
        }
        children.children.push(
            object("Operation", &operation_head.uuid)
                .child(
                    header_properties(&operation_head)?
                        .child(xdto_type("XDTOReturningValueType", item(record, 2)?, 6)?)
                        .child(flag("Nillable", item(record, 3)?)?)
                        .child(flag("Transactioned", item(record, 4)?)?)
                        .child(text("ProcedureName", item(record, 5)?)?)
                        .child(coded(
                            "DataLockControlMode",
                            item(record, 6)?,
                            DATA_LOCK_CONTROL_MODES,
                        )?),
                )
                .child(parameters),
        );
    }
    Ok(object("WebService", &head.uuid)
        .child(properties)
        .child(children))
}

// ---------------------------------------------------------------------------
// Names.

fn manager_name(name: String, type_id: &Brace, value_id: &Brace) -> Result<GeneratedTypeName> {
    Ok(GeneratedTypeName {
        name,
        category: "Manager".to_string(),
        type_id: atom(type_id)?.to_string(),
        value_id: atom(value_id)?.to_string(),
    })
}

/// What a common object's row contributes to a name index.
pub(crate) fn names(kind: &str, row: &Brace) -> Result<ObjectNames> {
    if kind == "ExternalDataSource" {
        return crate::metadata_model::external_data_source::names(row);
    }
    let root = list(row)?;
    let fields = list(item(root, 1)?)?;
    let mut children = Vec::new();
    let mut types = Vec::new();
    let head = match kind {
        "CommonModule" | "CommonPicture" | "CommonTemplate" | "Role" | "Style" | "XDTOPackage"
        | "PaletteColor" | "Bot" => header(item(fields, 1)?)?,
        "WebSocketClient" => {
            return Ok(
                crate::metadata_model::websocket_client::WebSocketClient::from_brace(row)?.names(),
            );
        }
        "StyleItem" => header(item(fields, 3)?)?,
        "CommandGroup" => header(item(fields, 6)?)?,
        "CommonCommand" => header(item(list(item(list(item(fields, 1)?)?, 2)?)?, 9)?)?,
        "WSReference" => {
            let head = header(item(fields, 2)?)?;
            types.push(manager_name(
                format!("WSReferenceManager.{}", head.name),
                item(fields, 3)?,
                item(fields, 4)?,
            )?);
            head
        }
        "IntegrationService" => {
            let head = header(item(fields, 1)?)?;
            types.push(manager_name(
                format!("IntegrationServiceManager.{}", head.name),
                item(fields, 2)?,
                item(fields, 3)?,
            )?);
            for stored in only_collection(root, INTEGRATION_CHANNELS)? {
                let channel = list(item(list(stored)?, 0)?)?;
                let channel_head = header(item(channel, 1)?)?;
                types.push(manager_name(
                    format!(
                        "IntegrationServiceChannelManager.{}.{}",
                        head.name, channel_head.name
                    ),
                    item(channel, 2)?,
                    item(channel, 3)?,
                )?);
                children.push((
                    format!(
                        "IntegrationService.{}.IntegrationServiceChannel.{}",
                        head.name, channel_head.name
                    ),
                    channel_head.uuid,
                ));
            }
            head
        }
        "HTTPService" => {
            let head = header(item(fields, 2)?)?;
            for stored in only_collection(root, HTTP_URL_TEMPLATES)? {
                let template = list(stored)?;
                let template_head = header(item(list(item(template, 0)?)?, 2)?)?;
                let template_full = format!(
                    "HTTPService.{}.URLTemplate.{}",
                    head.name, template_head.name
                );
                for stored_method in collection(item(template, 2)?, HTTP_METHODS)? {
                    let method = list(item(list(stored_method)?, 0)?)?;
                    let method_head = header(item(method, 3)?)?;
                    children.push((
                        format!("{template_full}.Method.{}", method_head.name),
                        method_head.uuid,
                    ));
                }
                children.push((template_full, template_head.uuid));
            }
            head
        }
        "WebService" => {
            let head = header(item(fields, 2)?)?;
            for stored in only_collection(root, WEB_OPERATIONS)? {
                let operation = list(stored)?;
                let operation_head = header(item(list(item(operation, 0)?)?, 1)?)?;
                let operation_full =
                    format!("WebService.{}.Operation.{}", head.name, operation_head.name);
                for stored_parameter in collection(item(operation, 2)?, WEB_PARAMETERS)? {
                    let parameter = list(item(list(stored_parameter)?, 0)?)?;
                    let parameter_head = header(item(parameter, 1)?)?;
                    children.push((
                        format!("{operation_full}.Parameter.{}", parameter_head.name),
                        parameter_head.uuid,
                    ));
                }
                children.push((operation_full, operation_head.uuid));
            }
            head
        }
        other => bail!("{other} is not a common kind"),
    };
    Ok(ObjectNames {
        uuid: head.uuid.clone(),
        full_name: format!("{kind}.{}", head.name),
        children,
        types,
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::metadata_model::ObjectXml;
    use crate::metadata_model::export::write_document;
    use crate::metadata_model::objects::parts::Compat;
    use crate::metadata_model::types::tests::{context, element};

    /// XML -> row (the load direction) -> model -> XML: the same text.
    fn round_trip(kind: &str, xml: &str) {
        let context = context();
        let original = element(xml);
        let object = ObjectXml {
            element: &original,
            kind,
            uuid: Element::attr(&original, "uuid")
                .unwrap_or_default()
                .to_string(),
            name: original
                .path(&["Properties", "Name"])
                .map(|name| name.text.clone())
                .unwrap_or_default(),
            path: Path::new("."),
        };
        let row = super::super::compile(&object, &context).unwrap();
        let export = ExportContext {
            names: NameIndex::from_config_index(&context.index),
            version: "2.20".to_string(),
            compat: Compat(8, 3, 27),
        };
        let decoded = decode(kind, &row, &export).unwrap();
        assert_eq!(
            write_document(&decoded, "2.20"),
            write_document(&original, "2.20")
        );
        assert_eq!(
            names(kind, &row).unwrap().full_name,
            format!("{kind}.{}", object.name)
        );
    }

    #[test]
    fn style_items_round_trip() {
        round_trip(
            "StyleItem",
            r##"<StyleItem uuid="2d2a4e7c-0000-4000-8000-000000000001"><Properties><Name>Шрифт</Name><Synonym/><Comment/><Type>Font</Type><Value xsi:type="v8ui:Font" ref="style:NormalTextFont" height="10" bold="true" kind="StyleItem"/></Properties></StyleItem>"##,
        );
        round_trip(
            "StyleItem",
            r##"<StyleItem uuid="2d2a4e7c-0000-4000-8000-000000000002"><Properties><Name>Цвет</Name><Synonym/><Comment/><Type>Color</Type><Value xsi:type="v8ui:Color">#FFEC9D</Value></Properties></StyleItem>"##,
        );
        round_trip(
            "StyleItem",
            r##"<StyleItem uuid="2d2a4e7c-0000-4000-8000-000000000003"><Properties><Name>Шрифт2</Name><Synonym/><Comment/><Type>Font</Type><Value xsi:type="v8ui:Font" faceName="Arial" height="8" bold="false" italic="false" underline="false" strikeout="false" kind="Absolute" scale="100"/></Properties></StyleItem>"##,
        );
    }

    #[test]
    fn a_web_service_round_trips_with_a_declared_namespace() {
        round_trip(
            "WebService",
            r##"<WebService uuid="300c8045-655d-41da-bf92-d3de432334e1"><Properties><Name>Сервис</Name><Synonym/><Comment/><Namespace>http://www.1c.ru/test</Namespace><XDTOPackages><xr:Item><xr:Presentation/><xr:CheckState>0</xr:CheckState><xr:Value xsi:type="xs:string">http://v8.1c.ru/8.1/data/core</xr:Value></xr:Item></XDTOPackages><DescriptorFileName>ws.1cws</DescriptorFileName><ReuseSessions>DontUse</ReuseSessions><SessionMaxAge>20</SessionMaxAge></Properties><ChildObjects><Operation uuid="0dd304ca-64fa-498e-b327-3219a9ef243e"><Properties><Name>Ping</Name><Synonym/><Comment/><XDTOReturningValueType xmlns:d6p1="http://www.1c.ru/test/Message">d6p1:Answer</XDTOReturningValueType><Nillable>false</Nillable><Transactioned>false</Transactioned><ProcedureName>Ping</ProcedureName><DataLockControlMode>Managed</DataLockControlMode></Properties><ChildObjects><Parameter uuid="4b7df58c-b9fb-4e11-875a-9b1990515b8d"><Properties><Name>Data</Name><Synonym/><Comment/><XDTOValueType>v8:ValueStorage</XDTOValueType><Nillable>true</Nillable><TransferDirection>InOut</TransferDirection></Properties></Parameter></ChildObjects></Operation></ChildObjects></WebService>"##,
        );
    }

    #[test]
    fn a_common_command_round_trips() {
        round_trip(
            "CommonCommand",
            r##"<CommonCommand uuid="dc4b6b3e-8f6a-4375-bcf3-ce827153474a"><Properties><Name>Команда</Name><Synonym/><Comment/><Group>FormCommandBarImportant</Group><Representation>Picture</Representation><ToolTip><v8:item><v8:lang>ru</v8:lang><v8:content>Подсказка</v8:content></v8:item></ToolTip><Picture><xr:Ref>StdPicture.Print</xr:Ref><xr:LoadTransparent>true</xr:LoadTransparent></Picture><Shortcut>Ctrl+Shift+P</Shortcut><IncludeHelpInContents>false</IncludeHelpInContents><CommandParameterType><v8:Type>cfg:CatalogRef.Валюты</v8:Type></CommandParameterType><ParameterUseMode>Multiple</ParameterUseMode><ModifiesData>true</ModifiesData><OnMainServerUnavalableBehavior>Auto</OnMainServerUnavalableBehavior></Properties></CommonCommand>"##,
        );
    }
}
