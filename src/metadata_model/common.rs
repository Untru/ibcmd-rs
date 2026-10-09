//! Common objects and services: CommonModule, CommonPicture, CommonTemplate,
//! CommonCommand, CommandGroup, Role, XDTOPackage, StyleItem, Style,
//! WebService, HTTPService, WSReference, IntegrationService, Bot, Subsystem,
//! owned Form and Template, CommonForm.
//!
//! One typed model per kind: `from_xml` reads the metadata XML and
//! `to_brace` writes the stored descriptor, so the export can later decode a
//! row into the same struct.
//!
//! Empty ExternalDataSource is measured against a native 8.3.27.2214 CF;
//! its nonempty child families and Interface require separate layouts.

#[path = "common_export.rs"]
pub(crate) mod export;
#[path = "forms_export.rs"]
pub(crate) mod forms_export;

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use anyhow::{Context, Result, anyhow, bail};

use super::brace::{Brace, parse_row};
use super::xml::{Element, MetadataXml};
use super::{DescriptorContext, ObjectXml, native_text, not_yet, parse_bool};
use crate::brace_list;
use crate::compiler::bodies::form_native::{
    format_native_color, format_native_control_border, format_native_font, format_native_shortcut,
};

pub fn compile(object: &ObjectXml<'_>, context: &DescriptorContext) -> Result<Brace> {
    let mut tree = compile_tree(object, context)?;
    crlf_strings(&mut tree);
    Ok(tree)
}

fn compile_tree(object: &ObjectXml<'_>, context: &DescriptorContext) -> Result<Brace> {
    Ok(match object.kind {
        "CommonModule" => CommonModule::from_xml(object)?.to_brace(),
        "CommonPicture" => CommonPicture::from_xml(object)?.to_brace(),
        "CommonTemplate" => Template::from_xml(object)?.to_common_brace(),
        "Template" => Template::from_xml(object)?.to_owned_brace(),
        "Role" => Role::from_xml(object)?.to_brace(),
        "XDTOPackage" => XdtoPackage::from_xml(object)?.to_brace(),
        "Style" => Style::from_xml(object)?.to_brace(),
        "PaletteColor" => PaletteColor::from_xml(object, context)?.to_brace(),
        "StyleItem" => StyleItem::from_xml(object, context)?.to_brace(),
        "CommandGroup" => CommandGroup::from_xml(object, context)?.to_brace(),
        "CommonCommand" => CommonCommand::from_xml(object, context)?.to_brace(),
        "WSReference" => WsReference::from_xml(object)?.to_brace(),
        "Bot" => Bot::from_xml(object, context)?.to_brace(),
        "IntegrationService" => IntegrationService::from_xml(object)?.to_brace(),
        "HTTPService" => HttpService::from_xml(object)?.to_brace(),
        "WebSocketClient" => super::websocket_client::WebSocketClient::from_xml(object)?.to_brace(),
        "WebService" => WebService::from_xml(object, context)?.to_brace(),
        "ExternalDataSource" => super::external_data_source::compile(object, context)?,
        "Subsystem" => Subsystem::from_xml(object, context)?.to_brace(),
        "Form" => Form::from_xml(object, context)?.to_brace(),
        "CommonForm" => CommonForm::from_xml(object, context)?.to_brace(),
        _ => return Err(not_yet(object)),
    })
}

// ---------------------------------------------------------------------------
// Shared pieces
// ---------------------------------------------------------------------------

/// A stored string breaks its lines with CR LF where the XML text holds a
/// bare LF (multi-line synonyms, tooltips, explanations): `native_text` over
/// every string of a compiled tree.
pub fn crlf_strings(node: &mut Brace) {
    match node {
        Brace::Str(value) if value.contains('\n') => *value = native_text(value),
        Brace::List(items) => items.iter_mut().for_each(crlf_strings),
        _ => {}
    }
}

/// `(lang, text)` pairs of a localized string.
pub type Localized = Vec<(String, String)>;

/// The `{3,{1,0,<uuid>},"Name",<synonym>,"Comment",0,0,<nil>,0}` block every
/// object and child object starts with.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Header {
    pub uuid: String,
    pub name: String,
    pub synonym: Localized,
    pub comment: String,
}

impl Header {
    pub fn from_properties(uuid: &str, properties: &Element) -> Self {
        Self {
            uuid: uuid.to_ascii_lowercase(),
            name: text(properties, "Name").to_string(),
            synonym: localized_of(properties, "Synonym"),
            comment: text(properties, "Comment").to_string(),
        }
    }

    pub fn of(object: &ObjectXml<'_>) -> Result<Self> {
        Ok(Self::from_properties(&object.uuid, object.properties()?))
    }

    /// A child object element (`<Operation uuid=...>`).
    pub fn of_child(element: &Element) -> Result<Self> {
        let uuid = element
            .attr("uuid")
            .ok_or_else(|| anyhow!("<{}> has no uuid", element.name))?;
        Ok(Self::from_properties(uuid, child_properties(element)?))
    }

    pub fn to_brace(&self) -> Brace {
        brace_list![
            Brace::num(3),
            brace_list![Brace::num(1), Brace::num(0), Brace::uuid(&self.uuid)],
            Brace::str(self.name.clone()),
            localized_brace(&self.synonym),
            Brace::str(self.comment.clone()),
            Brace::num(0),
            Brace::num(0),
            Brace::nil_uuid(),
            Brace::num(0),
        ]
    }
}

fn child_properties(element: &Element) -> Result<&Element> {
    element
        .child("Properties")
        .ok_or_else(|| anyhow!("<{}> has no <Properties>", element.name))
}

/// `{N,"lang","text",...}`, `{0}` when empty.
pub fn localized_brace(pairs: &[(String, String)]) -> Brace {
    let mut items = vec![Brace::num(pairs.len() as i64)];
    for (lang, content) in pairs {
        items.push(Brace::str(lang.clone()));
        items.push(Brace::str(content.clone()));
    }
    Brace::List(items)
}

fn text<'a>(properties: &'a Element, name: &str) -> &'a str {
    properties.child_text(name).unwrap_or_default()
}

fn localized_of(properties: &Element, name: &str) -> Localized {
    properties
        .child(name)
        .map(Element::localized)
        .unwrap_or_default()
}

/// A boolean property; an absent one reads `false`.
fn flag(properties: &Element, name: &str) -> Result<bool> {
    match properties.child_text(name) {
        None => Ok(false),
        Some(value) => parse_bool(value.trim()).with_context(|| format!("<{name}>")),
    }
}

fn number(properties: &Element, name: &str) -> Result<i64> {
    let value = text(properties, name).trim();
    value
        .parse::<i64>()
        .with_context(|| format!("<{name}> is not a number: {value:?}"))
}

/// The code a table gives an enumeration value.
fn code(value: &str, table: &[(&str, i64)], what: &str) -> Result<i64> {
    let value = value.trim();
    table
        .iter()
        .find_map(|(name, code)| (*name == value).then_some(*code))
        .ok_or_else(|| anyhow!("no code for {what} {value:?}"))
}

fn enum_prop(properties: &Element, name: &str, table: &[(&str, i64)]) -> Result<i64> {
    code(text(properties, name), table, name)
}

/// A brace literal written as text (`{3,0,{255}}`).
fn literal(text: &str) -> Result<Brace> {
    parse_row(text.as_bytes()).with_context(|| format!("bad brace literal {text:?}"))
}

/// `{"#",157fa490-...,{1,<uuid>}}`: a design-time reference to a metadata
/// object (subsystem content, web service packages).
const DESIGN_TIME_REFERENCE: &str = "157fa490-4ce9-11d4-9415-008048da11f9";

fn design_time_reference(uuid: &str) -> Brace {
    brace_list![
        Brace::str("#"),
        Brace::atom(DESIGN_TIME_REFERENCE),
        brace_list![Brace::num(1), Brace::uuid(uuid)],
    ]
}

/// The uuid of `Kind.Name[.Child.Name...]`.
fn resolve(context: &DescriptorContext, full_name: &str) -> Result<String> {
    context
        .index
        .uuid_of(full_name)
        .map(str::to_string)
        .ok_or_else(|| anyhow!("unresolved reference {full_name}"))
}

/// A metadata reference as the XML spells it: a full name, or a uuid when the
/// object it named no longer exists (`0:<uuid>` or the bare uuid).
fn resolve_reference(context: &DescriptorContext, spelled: &str) -> Result<String> {
    let bare = spelled.strip_prefix("0:").unwrap_or(spelled);
    if is_uuid(bare) {
        return Ok(bare.to_ascii_lowercase());
    }
    resolve(context, spelled)
}

fn is_uuid(text: &str) -> bool {
    text.len() == 36
        && text.char_indices().all(|(index, ch)| match index {
            8 | 13 | 18 | 23 => ch == '-',
            _ => ch.is_ascii_hexdigit(),
        })
}

/// This object's full name in the index (`Subsystem.A.Subsystem.B`).
fn full_name_of<'a>(object: &ObjectXml<'_>, context: &'a DescriptorContext) -> Result<&'a str> {
    context
        .index
        .objects_by_uuid
        .get(&object.uuid)
        .map(String::as_str)
        .ok_or_else(|| anyhow!("{} {} is not in the index", object.kind, object.name))
}

/// A generated type of an element's `<InternalInfo>` by category:
/// `(TypeId, ValueId)`.
fn generated_ids(element: &Element, category: &str) -> Result<(String, String)> {
    let generated = element
        .child("InternalInfo")
        .into_iter()
        .flat_map(|info| info.children_named("GeneratedType"))
        .find(|generated| generated.attr("category") == Some(category))
        .ok_or_else(|| anyhow!("<{}> has no {category} generated type", element.name))?;
    Ok((
        generated
            .child_text("TypeId")
            .unwrap_or_default()
            .to_ascii_lowercase(),
        generated
            .child_text("ValueId")
            .unwrap_or_default()
            .to_ascii_lowercase(),
    ))
}

/// The child elements of `<ChildObjects>` with this local name.
fn child_objects<'a>(
    element: &'a Element,
    name: &'a str,
) -> impl Iterator<Item = &'a Element> + 'a {
    element
        .child("ChildObjects")
        .into_iter()
        .flat_map(move |children| children.children_named(name))
}

/// Whether descriptors take the layout 8.5.1 introduced (form record 14,
/// `{4,...}` colours, `{8,...}` fonts): the tree's platform stores that
/// layout (the registry's `form_layout()`) and the tree is stored in it
/// ([`tree_stores_layout_8_5_1`]).
pub fn stores_layout_8_5_1(context: &DescriptorContext) -> bool {
    context.platform().form_layout() >= crate::platform::FormLayout::V8_5_1
        && if context.source.original_source().is_some() {
            context.source.descriptor_layout_8_5_1()
        } else {
            tree_stores_layout_8_5_1(&context.root)
        }
}

/// Whether platform 8.5 stores the configuration of the XML 2.21 tree at
/// `root` in the 8.5.1 layout (forms, styles, the `{76,...}` Configuration
/// tuple) rather than the 8.3.27 one: its `CompatibilityMode` is 8.5 or
/// later, or one of its managed forms can only be held by the 8.5.1 layout
/// ([`form_needs_layout_8_5_1`]).
///
/// The layout is the configuration's, not the XML dialect's, and not its
/// compatibility alone. The BSP 8.5 clone (`Version8_5_1`) stores every form
/// and style item the 8.5 way; the ERP УХ 8.5 clone (`Version8_3_27`, a
/// database carried over from 8.3.27) stores all 13 053 of them the 8.3.27
/// way, while both trees spell the descriptors identically; and the
/// configuration 8.5.1.1529 built from XML at `Version8_3_27` and saved
/// (`home_page/one_column_v85/input.cf`, `_onecdec/make_home_page_fixtures.py`)
/// stores its forms (`{59,...}` bodies, `{14,...}` descriptors) and its
/// Configuration tuple (`{76,...}`) the 8.5 way. Every 2.21 form of a
/// configuration stored the 8.3.27 way names its window opening mode and
/// group, which the forms of the third one do not; a tree with no form at
/// all and an 8.3 compatibility mode keeps the 8.3.27 layout, as before.
pub fn tree_stores_layout_8_5_1(root: &std::path::Path) -> bool {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, bool>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(known) = cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(root)
    {
        return *known;
    }
    let stores = compatibility_at_least_8_5(root) || any_form_needs_layout_8_5_1(root);
    cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(root.to_path_buf(), stores);
    stores
}

/// The original-bound Main path uses the same layout facts, with fallible
/// reads of complete census members and no process-wide pathname memo.
pub(crate) fn original_tree_stores_layout_8_5_1(
    source: &crate::module_blob::MetadataSourceContext,
) -> Result<bool> {
    let configuration = source.source_root().join("Configuration.xml");
    if !source.source_file_exists(&configuration)? {
        return Ok(true);
    }
    let bytes = source.read_source(&configuration)?;
    let doc = MetadataXml::parse(&bytes)?;
    if compatibility_from_document(&doc) {
        return Ok(true);
    }
    let original = source
        .original_source()
        .ok_or_else(|| anyhow!("layout requires original source"))?;
    for member in original.baseline().files() {
        let path = std::path::Path::new(member.path());
        if path.file_name().is_some_and(|name| name == "Form.xml")
            && path
                .parent()
                .and_then(|parent| parent.file_name())
                .is_some_and(|name| name == "Ext")
        {
            let bytes = original.source_bytes(member.path())?;
            if form_needs_layout_8_5_1(&String::from_utf8_lossy(&bytes)) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Whether some managed form of the tree (`.../Ext/Form.xml`) is one only
/// the 8.5.1 layout holds ([`form_needs_layout_8_5_1`]); stops at the first.
fn any_form_needs_layout_8_5_1(root: &std::path::Path) -> bool {
    use std::io::Read;
    walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry.file_type().is_file()
                && entry.file_name() == "Form.xml"
                && entry
                    .path()
                    .parent()
                    .and_then(|parent| parent.file_name())
                    .is_some_and(|name| name == "Ext")
        })
        .any(|entry| {
            // The root's leading elements come first (`xml_2_21_order`):
            // the head of the file is enough.
            let mut head = Vec::with_capacity(FORM_HEAD_BYTES);
            std::fs::File::open(entry.path())
                .and_then(|file| file.take(FORM_HEAD_BYTES as u64).read_to_end(&mut head))
                .is_ok()
                && form_needs_layout_8_5_1(&String::from_utf8_lossy(&head))
        })
}

/// How much of a `Form.xml` [`form_needs_layout_8_5_1`] reads.
const FORM_HEAD_BYTES: usize = 64 * 1024;

/// Whether the head of a 2.21 `Form.xml` describes a form only the 8.5.1
/// layout holds: its root names no `WindowOpeningMode` or no `Group`. The
/// 8.3.27 root record has no unset state for either, and platform 8.5 prints
/// a form stored that way with both (`DontBlock`, `Vertical` when 8.3.27
/// leaves them out; `mssql_dump::form::xml_2_21_writer::upgrade_form_root`,
/// fitted on ERP УХ 8.5), so a 2.21 form without one was stored the 8.5 way,
/// where both may be unset. A head that does not reach the root's first
/// nested element (`AutoCommandBar`, `ChildItems`, ...) decides nothing.
fn form_needs_layout_8_5_1(head: &str) -> bool {
    let Some(form) = head.find("<Form ") else {
        return false;
    };
    let open_end = head[form..].find('>').map_or(head.len(), |end| form + end);
    if !head[form..open_end].contains("version=\"2.21\"") {
        return false;
    }
    let Some(leading_end) = [
        "\n\t<AutoCommandBar",
        "\n\t<Events>",
        "\n\t<ChildItems>",
        "\n\t<Attributes>",
        "\n\t<Commands>",
        "\n\t<Parameters>",
        "\n\t<CommandInterface>",
        "\n</Form>",
    ]
    .iter()
    .filter_map(|marker| head[open_end..].find(marker))
    .min() else {
        return false;
    };
    let leading = &head[open_end..open_end + leading_end];
    !leading.contains("\n\t<WindowOpeningMode>") || !leading.contains("\n\t<Group>")
}

/// `CompatibilityMode` of the tree's `Configuration.xml` is `Version8_5_x`
/// or later, or `DontUse` (the platform's own version); `true` when the file
/// says nothing.
fn compatibility_at_least_8_5(root: &std::path::Path) -> bool {
    let Ok(bytes) = std::fs::read(root.join("Configuration.xml")) else {
        return true;
    };
    let Ok(doc) = MetadataXml::parse(&bytes) else {
        return true;
    };
    compatibility_from_document(&doc)
}

fn compatibility_from_document(doc: &MetadataXml) -> bool {
    let mode = doc
        .object()
        .ok()
        .and_then(|object| object.path(&["Properties", "CompatibilityMode"]))
        .map(|mode| mode.text.trim().to_string())
        .unwrap_or_default();
    let Some(version) = mode.strip_prefix("Version") else {
        return true;
    };
    let parts = version
        .split('_')
        .map(|part| part.parse::<u32>().unwrap_or(0))
        .collect::<Vec<_>>();
    (
        parts.first().copied().unwrap_or(0),
        parts.get(1).copied().unwrap_or(0),
    ) >= (8, 5)
}

// ---------------------------------------------------------------------------
// Pictures
// ---------------------------------------------------------------------------

/// What a picture reference stores in member 2.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PictureSource {
    /// No picture: `{0}`.
    None,
    /// A platform picture stored by code: `{-13}`.
    Code(i64),
    /// A common picture or a platform picture stored by uuid: `{0,<uuid>}`.
    Uuid(String),
}

/// The nine-member `{4,...}` picture reference of commands, groups,
/// subsystems and bots.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picture {
    pub source: PictureSource,
    pub load_transparent: bool,
    pub pixel: Option<(i64, i64)>,
}

impl Picture {
    pub fn from_xml(element: Option<&Element>, context: &DescriptorContext) -> Result<Self> {
        let none = Picture {
            source: PictureSource::None,
            load_transparent: true,
            pixel: None,
        };
        let Some(element) = element else {
            return Ok(none);
        };
        if element.child("Abs").is_some() {
            bail!("an inline <Picture><Abs> is not supported");
        }
        let reference = element.child_text("Ref").unwrap_or_default().trim();
        if reference.is_empty() {
            return Ok(none);
        }
        let load_transparent = match element.child_text("LoadTransparent") {
            Some(value) => parse_bool(value.trim()).context("<LoadTransparent>")?,
            None => false,
        };
        let pixel = match element.child("TransparentPixel") {
            Some(pixel) => Some((
                pixel.attr("x").unwrap_or("-1").trim().parse::<i64>()?,
                pixel.attr("y").unwrap_or("-1").trim().parse::<i64>()?,
            )),
            None => None,
        };
        let source = picture_source(reference, context)?;
        Ok(Picture {
            source,
            load_transparent,
            pixel,
        })
    }

    pub fn to_brace(&self) -> Brace {
        let (present, reference) = match &self.source {
            PictureSource::None => (0, brace_list![Brace::num(0)]),
            PictureSource::Code(code) => (1, brace_list![Brace::num(*code)]),
            PictureSource::Uuid(uuid) => (1, brace_list![Brace::num(0), Brace::uuid(uuid)]),
        };
        let (x, y) = self.pixel.unwrap_or((-1, -1));
        brace_list![
            Brace::num(4),
            Brace::num(present),
            reference,
            Brace::str(""),
            Brace::num(x),
            Brace::num(y),
            Brace::flag(self.load_transparent),
            Brace::num(0),
            Brace::str(""),
        ]
    }
}

/// Platform pictures a reference stores by a negative code rather than a
/// uuid.
const STD_PICTURE_CODES: &[(&str, i64)] = &[
    ("CheckAll", -10),
    ("Clear", -200),
    ("InputFieldCalculator", -6),
    ("InputFieldCalendar", -5),
    ("InputFieldChooseType", -14),
    ("InputFieldClear", -2),
    ("InputFieldOpen", -7),
    ("InputFieldSelect", -1),
    ("MoveDown", -4),
    ("MoveLeft", -8),
    ("MoveRight", -9),
    ("MoveUp", -3),
    ("Print", -13),
    ("Select", -100),
    ("UncheckAll", -11),
    ("ZoomIn", -16),
    ("ZoomOut", -15),
];

fn picture_source(reference: &str, context: &DescriptorContext) -> Result<PictureSource> {
    if let Some(name) = reference.strip_prefix("StdPicture.") {
        if let Some(code) = STD_PICTURE_CODES
            .iter()
            .find_map(|(candidate, code)| (*candidate == name).then_some(*code))
        {
            return Ok(PictureSource::Code(code));
        }
        let uuid = crate::mssql_dump::standard_picture_uuid(reference)
            .ok_or_else(|| anyhow!("no uuid for {reference}"))?;
        return Ok(PictureSource::Uuid(uuid.to_string()));
    }
    if reference.starts_with("CommonPicture.") {
        return Ok(PictureSource::Uuid(resolve(context, reference)?));
    }
    // `0:<uuid>`: a common picture that no longer exists, spelled by uuid.
    if let Some(uuid) = reference.strip_prefix("0:") {
        return Ok(PictureSource::Uuid(uuid.to_ascii_lowercase()));
    }
    // `0`: a reference naming nothing, stored `{0}` behind the present flag.
    if reference == "0" {
        return Ok(PictureSource::Code(0));
    }
    bail!("unsupported picture reference {reference}")
}

// ---------------------------------------------------------------------------
// CommonModule
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommonModule {
    pub header: Header,
    pub global: bool,
    pub client_managed_application: bool,
    pub server: bool,
    pub external_connection: bool,
    pub client_ordinary_application: bool,
    pub server_call: bool,
    pub privileged: bool,
    /// `DontUse` 0, `DuringRequest` 1, `DuringSession` 2.
    pub return_values_reuse: i64,
}

const RETURN_VALUES_REUSE: &[(&str, i64)] =
    &[("DontUse", 0), ("DuringRequest", 1), ("DuringSession", 2)];

impl CommonModule {
    pub fn from_xml(object: &ObjectXml<'_>) -> Result<Self> {
        let p = object.properties()?;
        Ok(Self {
            header: Header::of(object)?,
            global: flag(p, "Global")?,
            client_managed_application: flag(p, "ClientManagedApplication")?,
            server: flag(p, "Server")?,
            external_connection: flag(p, "ExternalConnection")?,
            client_ordinary_application: flag(p, "ClientOrdinaryApplication")?,
            server_call: flag(p, "ServerCall")?,
            privileged: flag(p, "Privileged")?,
            return_values_reuse: enum_prop(p, "ReturnValuesReuse", RETURN_VALUES_REUSE)?,
        })
    }

    /// `{1,{12,<header>,ordinary,server,external,privileged,global,managed,
    /// reuse,server call},0}`
    pub fn to_brace(&self) -> Brace {
        brace_list![
            Brace::num(1),
            brace_list![
                Brace::num(12),
                self.header.to_brace(),
                Brace::flag(self.client_ordinary_application),
                Brace::flag(self.server),
                Brace::flag(self.external_connection),
                Brace::flag(self.privileged),
                Brace::flag(self.global),
                Brace::flag(self.client_managed_application),
                Brace::num(self.return_values_reuse),
                Brace::flag(self.server_call),
            ],
            Brace::num(0),
        ]
    }
}

// ---------------------------------------------------------------------------
// CommonPicture
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommonPicture {
    pub header: Header,
    pub availability_for_choice: bool,
    pub availability_for_appearance: bool,
}

impl CommonPicture {
    pub fn from_xml(object: &ObjectXml<'_>) -> Result<Self> {
        let p = object.properties()?;
        Ok(Self {
            header: Header::of(object)?,
            availability_for_choice: flag(p, "AvailabilityForChoice")?,
            availability_for_appearance: flag(p, "AvailabilityForAppearance")?,
        })
    }

    /// `{1,{4,<header>,choice,appearance},0}`
    pub fn to_brace(&self) -> Brace {
        brace_list![
            Brace::num(1),
            brace_list![
                Brace::num(4),
                self.header.to_brace(),
                Brace::flag(self.availability_for_choice),
                Brace::flag(self.availability_for_appearance),
            ],
            Brace::num(0),
        ]
    }
}

// ---------------------------------------------------------------------------
// Templates
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Template {
    pub header: Header,
    pub template_type: i64,
}

// Existing native enumeration codes have one schema owner.
use ibcmd_schema::external_named::{
    FORM_TYPES, INTERFACE_COMPATIBILITY_MODES, TEMPLATE_TYPES, USE_PURPOSES,
};

impl Template {
    pub fn from_xml(object: &ObjectXml<'_>) -> Result<Self> {
        let p = object.properties()?;
        Ok(Self {
            header: Header::of(object)?,
            template_type: enum_prop(p, "TemplateType", TEMPLATE_TYPES)?,
        })
    }

    /// A common template: `{1,{4,<header>,type},0}`.
    pub fn to_common_brace(&self) -> Brace {
        brace_list![
            Brace::num(1),
            brace_list![
                Brace::num(4),
                self.header.to_brace(),
                Brace::num(self.template_type)
            ],
            Brace::num(0),
        ]
    }

    /// An object's template: `{1,{2,type,<header>},0}`.
    pub fn to_owned_brace(&self) -> Brace {
        brace_list![
            Brace::num(1),
            brace_list![
                Brace::num(2),
                Brace::num(self.template_type),
                self.header.to_brace()
            ],
            Brace::num(0),
        ]
    }
}

// ---------------------------------------------------------------------------
// Role, XDTOPackage, Style
// ---------------------------------------------------------------------------

/// A role's descriptor carries only its header; the rights live in the
/// `<uuid>.0` row.
///
/// The three members after the header are `1,0,0` in every role of the four
/// corpora but one: ERP УХ `ИспользованиеПлатежногоКалендаряУХ` stores
/// `1,0,1`, and neither its XML nor its `Rights.xml` (whose three flags do not
/// correlate with these members) says so.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Role {
    pub header: Header,
}

impl Role {
    pub fn from_xml(object: &ObjectXml<'_>) -> Result<Self> {
        Ok(Self {
            header: Header::of(object)?,
        })
    }

    /// `{1,{6,<header>,1,0,0},0}`
    pub fn to_brace(&self) -> Brace {
        brace_list![
            Brace::num(1),
            brace_list![
                Brace::num(6),
                self.header.to_brace(),
                Brace::num(1),
                Brace::num(0),
                Brace::num(0),
            ],
            Brace::num(0),
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XdtoPackage {
    pub header: Header,
    pub namespace: String,
}

impl XdtoPackage {
    pub fn from_xml(object: &ObjectXml<'_>) -> Result<Self> {
        let p = object.properties()?;
        Ok(Self {
            header: Header::of(object)?,
            namespace: text(p, "Namespace").to_string(),
        })
    }

    /// `{1,{1,<header>,"Namespace"},0}`
    pub fn to_brace(&self) -> Brace {
        brace_list![
            Brace::num(1),
            brace_list![
                Brace::num(1),
                self.header.to_brace(),
                Brace::str(self.namespace.clone())
            ],
            Brace::num(0),
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Style {
    pub header: Header,
}

impl Style {
    pub fn from_xml(object: &ObjectXml<'_>) -> Result<Self> {
        Ok(Self {
            header: Header::of(object)?,
        })
    }

    /// `{1,{3,<header>},0}`
    pub fn to_brace(&self) -> Brace {
        brace_list![
            Brace::num(1),
            brace_list![Brace::num(3), self.header.to_brace()],
            Brace::num(0),
        ]
    }
}

// ---------------------------------------------------------------------------
// PaletteColor (8.5)
// ---------------------------------------------------------------------------

/// An 8.5 palette colour: `{1,{0,<header>,<colour>},0}`, the colour in the
/// 8.5 record (`#FFEC9D` is `{4,0,{10349823},0}`, blue first). Only an 8.5
/// configuration has palette colours, so the record is always the 8.5 one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaletteColor {
    pub header: Header,
    pub color: Brace,
}

impl PaletteColor {
    pub fn from_xml(object: &ObjectXml<'_>, context: &DescriptorContext) -> Result<Self> {
        let p = object.properties()?;
        let spelled = text(p, "Color").trim();
        let native = format_native_color(Some(spelled), |name| {
            context
                .index
                .uuid_of(&format!("StyleItem.{name}"))
                .map(str::to_string)
        })
        .ok_or_else(|| anyhow!("unsupported palette colour {spelled:?}"))?;
        let mut color = literal(&native)?;
        up_convert_primitives_8_5_1(&mut color);
        Ok(Self {
            header: Header::of(object)?,
            color,
        })
    }

    pub fn to_brace(&self) -> Brace {
        brace_list![
            Brace::num(1),
            brace_list![Brace::num(0), self.header.to_brace(), self.color.clone()],
            Brace::num(0),
        ]
    }
}

// ---------------------------------------------------------------------------
// StyleItem
// ---------------------------------------------------------------------------

const COLOR_TYPE: &str = "9cd510c7-abfc-11d4-9434-004095e12fc7";
const FONT_TYPE: &str = "9cd510c8-abfc-11d4-9434-004095e12fc7";
const BORDER_TYPE: &str = "4d10ca00-111a-4d43-9c96-92cd773716de";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StyleItemValue {
    /// `{3,...}` as a form body stores a colour.
    Color(Brace),
    /// `{7,...}` as a form body stores a font.
    Font(Brace),
    /// `{3,0,{0},style,width,0,<appearance>}` as a form body stores a border.
    Border(Brace),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StyleItem {
    pub header: Header,
    pub value: StyleItemValue,
}

impl StyleItem {
    pub fn from_xml(object: &ObjectXml<'_>, context: &DescriptorContext) -> Result<Self> {
        let p = object.properties()?;
        let value = p
            .child("Value")
            .ok_or_else(|| anyhow!("StyleItem {} has no <Value>", object.name))?;
        let style_item = |name: &str| {
            context
                .index
                .uuid_of(&format!("StyleItem.{name}"))
                .map(str::to_string)
        };
        let value = match text(p, "Type").trim() {
            "Color" => {
                let spelled = value.text.trim();
                let native = format_native_color(Some(spelled), style_item)
                    .ok_or_else(|| anyhow!("unsupported colour {spelled:?}"))?;
                StyleItemValue::Color(primitive_8_5_1(literal(&native)?, context))
            }
            "Font" => {
                let attributes = value
                    .attrs
                    .iter()
                    .filter(|(key, _)| key != "type")
                    .cloned()
                    .collect::<BTreeMap<_, _>>();
                let native = format_native_font(&attributes, style_item)
                    .ok_or_else(|| anyhow!("unsupported font {attributes:?}"))?;
                StyleItemValue::Font(primitive_8_5_1(literal(&native)?, context))
            }
            "Border" => {
                let style = value.child_text("style").map(str::trim);
                let width = value.attr("width").unwrap_or("1").trim();
                let native = format_native_control_border(style, width)
                    .ok_or_else(|| anyhow!("unsupported border {style:?} {width:?}"))?;
                StyleItemValue::Border(literal(&native)?)
            }
            other => bail!("unsupported StyleItem type {other:?}"),
        };
        Ok(Self {
            header: Header::of(object)?,
            value,
        })
    }

    /// Colour: `{1,{3,0,{"#",<colour type>,2,<colour>},<header>},0}`;
    /// font: `{1,{3,1,{"#",<font type>,1,<font>,0},<header>},0}`;
    /// border: `{1,{3,2,{"#",<border type>,1,<border>,0},<header>},0}`.
    pub fn to_brace(&self) -> Brace {
        let (kind, value) = match &self.value {
            StyleItemValue::Color(color) => (
                0,
                brace_list![
                    Brace::str("#"),
                    Brace::atom(COLOR_TYPE),
                    Brace::num(2),
                    color.clone()
                ],
            ),
            StyleItemValue::Font(font) => (
                1,
                brace_list![
                    Brace::str("#"),
                    Brace::atom(FONT_TYPE),
                    Brace::num(1),
                    font.clone(),
                    Brace::num(0)
                ],
            ),
            StyleItemValue::Border(border) => (
                2,
                brace_list![
                    Brace::str("#"),
                    Brace::atom(BORDER_TYPE),
                    Brace::num(1),
                    border.clone(),
                    Brace::num(0)
                ],
            ),
        };
        brace_list![
            Brace::num(1),
            brace_list![
                Brace::num(3),
                Brace::num(kind),
                value,
                self.header.to_brace()
            ],
            Brace::num(0),
        ]
    }
}

/// A colour or font as 8.5 stores it, from the 8.3.27 spelling the form
/// writer produces: a colour `{3,<space>,{..}}` becomes `{4,<space>,{..},<space>}`,
/// a font `{7,...}` becomes `{8,...}`, and the nineteen-member absolute font
/// also appends `0` (the rule of `load_8_5_1::up_convert_primitives`).
fn primitive_8_5_1(mut value: Brace, context: &DescriptorContext) -> Brace {
    if !stores_layout_8_5_1(context) {
        return value;
    }
    let Some(members) = value.as_list_mut() else {
        return value;
    };
    match members.first().and_then(Brace::as_atom) {
        Some("3") if members.len() == 3 => {
            let space = members[1].clone();
            members[0] = Brace::num(4);
            members.push(space);
        }
        Some("7") => {
            let absolute = members.len() == 19 && members[1].as_atom() == Some("0");
            members[0] = Brace::num(8);
            if absolute {
                members.push(Brace::num(0));
            }
        }
        _ => {}
    }
    value
}

/// Every colour and font tuple of a tree respelled the 8.5 way, members
/// recognised by shape: a colour is `{3,<space 0-4>,{<int>}|{0,<uuid>}}`,
/// a font `{7,<kind>,<mask>,...,1,<scale>}` (kind 3 with no members, kinds
/// 1 and 2 with a reference list) or the nineteen-member absolute font --
/// the tests of `load_8_5_1::up_convert_primitives`.
pub fn up_convert_primitives_8_5_1(node: &mut Brace) {
    let Brace::List(members) = node else {
        return;
    };
    for member in members.iter_mut() {
        up_convert_primitives_8_5_1(member);
    }
    let atom = |index: usize| members.get(index).and_then(Brace::as_atom);
    let is_int = |text: Option<&str>| {
        text.is_some_and(|text| {
            let digits = text.strip_prefix('-').unwrap_or(text);
            !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
        })
    };
    let color = members.len() == 3
        && atom(0) == Some("3")
        && matches!(atom(1), Some("0" | "1" | "2" | "3" | "4"))
        && match members[2].as_list() {
            Some([Brace::Atom(value)]) => is_int(Some(value)),
            Some([Brace::Atom(zero), Brace::Atom(uuid)]) => {
                zero == "0" && super::types::is_uuid(uuid)
            }
            _ => false,
        };
    let absolute_font = members.len() == 19
        && atom(0) == Some("7")
        && atom(1) == Some("0")
        && is_int(atom(2))
        && matches!(members[16], Brace::Str(_))
        && atom(17) == Some("1")
        && is_int(atom(18));
    let font = members.len() >= 5
        && atom(0) == Some("7")
        && is_int(atom(2))
        && atom(members.len() - 2) == Some("1")
        && is_int(atom(members.len() - 1))
        && match atom(1) {
            Some("3") => members.len() == 5 && atom(2) == Some("0"),
            Some("1" | "2") => matches!(members[3], Brace::List(_)),
            _ => false,
        };
    if color {
        let space = members[1].clone();
        members[0] = Brace::num(4);
        members.push(space);
    } else if absolute_font {
        members[0] = Brace::num(8);
        members.push(Brace::num(0));
    } else if font {
        members[0] = Brace::num(8);
    }
}

// ---------------------------------------------------------------------------
// Commands and command groups
// ---------------------------------------------------------------------------

/// `ButtonRepresentation`.
const REPRESENTATIONS: &[(&str, i64)] = &[
    ("Text", 0),
    ("Picture", 1),
    ("PictureAndText", 2),
    ("Auto", 3),
];

const COMMAND_GROUP_CATEGORIES: &[(&str, i64)] = &[
    ("NavigationPanel", 1),
    ("FormNavigationPanel", 2),
    ("ActionsPanel", 4),
    ("FormCommandBar", 8),
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandGroup {
    pub header: Header,
    pub picture: Picture,
    pub category: i64,
    pub representation: i64,
    pub tooltip: Localized,
}

impl CommandGroup {
    pub fn from_xml(object: &ObjectXml<'_>, context: &DescriptorContext) -> Result<Self> {
        let p = object.properties()?;
        Ok(Self {
            header: Header::of(object)?,
            picture: Picture::from_xml(p.child("Picture"), context)?,
            category: enum_prop(p, "Category", COMMAND_GROUP_CATEGORIES)?,
            representation: enum_prop(p, "Representation", REPRESENTATIONS)?,
            tooltip: localized_of(p, "ToolTip"),
        })
    }

    /// `{1,{3,<picture>,category,representation,<tooltip>,{0},<header>},0}`
    pub fn to_brace(&self) -> Brace {
        brace_list![
            Brace::num(1),
            brace_list![
                Brace::num(3),
                self.picture.to_brace(),
                Brace::num(self.category),
                Brace::num(self.representation),
                localized_brace(&self.tooltip),
                brace_list![Brace::num(0)],
                self.header.to_brace(),
            ],
            Brace::num(0),
        ]
    }
}

/// The class a common command's `{2,<uuid>,<class>}` identity names.
const COMMAND_CLASS: &str = "078a6af8-d22c-4248-9c33-7e90075a3d2c";

const PARAMETER_USE_MODES: &[(&str, i64)] = &[("Single", 0), ("Multiple", 1)];

const MAIN_SERVER_UNAVAILABLE_BEHAVIORS: &[(&str, i64)] = &[("Auto", 0)];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommonCommand {
    pub header: Header,
    /// The command group's uuid (a platform group or a `CommandGroup`).
    pub group: String,
    pub representation: i64,
    pub tooltip: Localized,
    pub picture: Picture,
    /// `{0,<key>,<modifiers>}`, `{0,0,0}` when none.
    pub shortcut: Brace,
    pub include_help_in_contents: bool,
    /// `{"Pattern",...}`.
    pub parameter_type: Brace,
    pub parameter_use_mode: i64,
    pub modifies_data: bool,
    pub on_main_server_unavailable_behavior: i64,
}

impl CommonCommand {
    pub fn from_xml(object: &ObjectXml<'_>, context: &DescriptorContext) -> Result<Self> {
        let p = object.properties()?;
        let shortcut = match text(p, "Shortcut").trim() {
            "" => brace_list![Brace::num(0), Brace::num(0), Brace::num(0)],
            spelled => literal(
                &format_native_shortcut(spelled)
                    .ok_or_else(|| anyhow!("unsupported shortcut {spelled:?}"))?,
            )?,
        };
        Ok(Self {
            header: Header::of(object)?,
            group: command_group_uuid(text(p, "Group"), context)?,
            representation: enum_prop(p, "Representation", REPRESENTATIONS)?,
            tooltip: localized_of(p, "ToolTip"),
            picture: Picture::from_xml(p.child("Picture"), context)?,
            shortcut,
            include_help_in_contents: flag(p, "IncludeHelpInContents")?,
            parameter_type: type_pattern(p.child("CommandParameterType"), context)?,
            parameter_use_mode: enum_prop(p, "ParameterUseMode", PARAMETER_USE_MODES)?,
            modifies_data: flag(p, "ModifiesData")?,
            on_main_server_unavailable_behavior: enum_prop(
                p,
                "OnMainServerUnavalableBehavior",
                MAIN_SERVER_UNAVAILABLE_BEHAVIORS,
            )?,
        })
    }

    /// `{1,{2,{1,{2,<uuid>,<class>},{9,<picture>,representation,<tooltip>,1,
    /// <shortcut>,help,{1,<group>},<parameter type>,<header>,modifies data,
    /// use mode,main server behaviour}}},0}`
    pub fn to_brace(&self) -> Brace {
        let details = brace_list![
            Brace::num(9),
            self.picture.to_brace(),
            Brace::num(self.representation),
            localized_brace(&self.tooltip),
            Brace::num(1),
            self.shortcut.clone(),
            Brace::flag(self.include_help_in_contents),
            brace_list![Brace::num(1), Brace::uuid(&self.group)],
            self.parameter_type.clone(),
            self.header.to_brace(),
            Brace::flag(self.modifies_data),
            Brace::num(self.parameter_use_mode),
            Brace::num(self.on_main_server_unavailable_behavior),
        ];
        brace_list![
            Brace::num(1),
            brace_list![
                Brace::num(2),
                brace_list![
                    Brace::num(1),
                    brace_list![
                        Brace::num(2),
                        Brace::uuid(&self.header.uuid),
                        Brace::atom(COMMAND_CLASS)
                    ],
                    details,
                ],
            ],
            Brace::num(0),
        ]
    }
}

/// A `<Group>` reference: a platform group by name or `CommandGroup.<name>`.
fn command_group_uuid(reference: &str, context: &DescriptorContext) -> Result<String> {
    let reference = reference.trim();
    if let Some((uuid, _)) = crate::mssql_dump::COMMON_COMMAND_GROUPS
        .iter()
        .find(|(_, name)| *name == reference)
    {
        return Ok(uuid.to_string());
    }
    if reference.starts_with("CommandGroup.") {
        return resolve(context, reference);
    }
    // `0:<uuid>`: a group spelled by its uuid.
    if let Some(uuid) = reference.strip_prefix("0:") {
        return Ok(uuid.to_ascii_lowercase());
    }
    bail!("unsupported command group {reference:?}")
}

/// A `<Type>`-like element (`<CommandParameterType>`) as `{"Pattern",...}`:
/// the simple-objects track's type description encoder.
fn type_pattern(element: Option<&Element>, context: &DescriptorContext) -> Result<Brace> {
    super::types::type_pattern(element, context)
}

// ---------------------------------------------------------------------------
// Services
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WsReference {
    pub header: Header,
    pub location_url: String,
    pub manager_type_id: String,
    pub manager_value_id: String,
}

impl WsReference {
    pub fn from_xml(object: &ObjectXml<'_>) -> Result<Self> {
        let p = object.properties()?;
        let (manager_type_id, manager_value_id) = generated_ids(object.element, "Manager")?;
        Ok(Self {
            header: Header::of(object)?,
            location_url: text(p, "LocationURL").to_string(),
            manager_type_id,
            manager_value_id,
        })
    }

    /// `{1,{2,{"LocationURL",0},<header>,<manager type>,<manager value>},0}`
    pub fn to_brace(&self) -> Brace {
        brace_list![
            Brace::num(1),
            brace_list![
                Brace::num(2),
                brace_list![Brace::str(self.location_url.clone()), Brace::num(0)],
                self.header.to_brace(),
                Brace::uuid(&self.manager_type_id),
                Brace::uuid(&self.manager_value_id),
            ],
            Brace::num(0),
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bot {
    pub header: Header,
    pub predefined: bool,
    pub picture: Picture,
}

impl Bot {
    pub fn from_xml(object: &ObjectXml<'_>, context: &DescriptorContext) -> Result<Self> {
        let p = object.properties()?;
        Ok(Self {
            header: Header::of(object)?,
            predefined: flag(p, "Predefined")?,
            picture: Picture::from_xml(p.child("Picture"), context)?,
        })
    }

    /// `{1,{1,<header>,predefined,<picture>},0}`
    pub fn to_brace(&self) -> Brace {
        brace_list![
            Brace::num(1),
            brace_list![
                Brace::num(1),
                self.header.to_brace(),
                Brace::flag(self.predefined),
                self.picture.to_brace(),
            ],
            Brace::num(0),
        ]
    }
}

const INTEGRATION_CHANNELS: &str = "acb7e81f-0637-4ebd-88ff-954ba075ae51";

const MESSAGE_DIRECTIONS: &[(&str, i64)] = &[("Send", 0), ("Receive", 1)];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntegrationServiceChannel {
    pub header: Header,
    pub manager_type_id: String,
    pub manager_value_id: String,
    pub external_name: String,
    pub receive_message_processing: String,
    pub message_direction: i64,
    pub transactioned: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntegrationService {
    pub header: Header,
    pub manager_type_id: String,
    pub manager_value_id: String,
    pub external_address: String,
    pub channels: Vec<IntegrationServiceChannel>,
}

impl IntegrationService {
    pub fn from_xml(object: &ObjectXml<'_>) -> Result<Self> {
        let p = object.properties()?;
        let (manager_type_id, manager_value_id) = generated_ids(object.element, "Manager")?;
        let mut channels = Vec::new();
        for channel in child_objects(object.element, "IntegrationServiceChannel") {
            let cp = child_properties(channel)?;
            let (manager_type_id, manager_value_id) = generated_ids(channel, "Manager")?;
            channels.push(IntegrationServiceChannel {
                header: Header::of_child(channel)?,
                manager_type_id,
                manager_value_id,
                external_name: text(cp, "ExternalIntegrationServiceChannelName").to_string(),
                receive_message_processing: text(cp, "ReceiveMessageProcessing").to_string(),
                message_direction: enum_prop(cp, "MessageDirection", MESSAGE_DIRECTIONS)?,
                transactioned: flag(cp, "Transactioned")?,
            });
        }
        Ok(Self {
            header: Header::of(object)?,
            manager_type_id,
            manager_value_id,
            external_address: text(p, "ExternalIntegrationServiceAddress").to_string(),
            channels,
        })
    }

    /// `{1,{0,<header>,<type>,<value>,"address"},1,{<channels>,N,
    /// {{1,<header>,<type>,<value>,"name","handler",direction,transactioned},0}...}}`
    pub fn to_brace(&self) -> Brace {
        let mut channels = vec![
            Brace::atom(INTEGRATION_CHANNELS),
            Brace::num(self.channels.len() as i64),
        ];
        for channel in &self.channels {
            channels.push(brace_list![
                brace_list![
                    Brace::num(1),
                    channel.header.to_brace(),
                    Brace::uuid(&channel.manager_type_id),
                    Brace::uuid(&channel.manager_value_id),
                    Brace::str(channel.external_name.clone()),
                    Brace::str(channel.receive_message_processing.clone()),
                    Brace::num(channel.message_direction),
                    Brace::flag(channel.transactioned),
                ],
                Brace::num(0),
            ]);
        }
        brace_list![
            Brace::num(1),
            brace_list![
                Brace::num(0),
                self.header.to_brace(),
                Brace::uuid(&self.manager_type_id),
                Brace::uuid(&self.manager_value_id),
                Brace::str(self.external_address.clone()),
            ],
            Brace::num(1),
            Brace::List(channels),
        ]
    }
}

const HTTP_URL_TEMPLATES: &str = "ec6896c2-9b28-42d8-9140-48491146b8ea";
const HTTP_METHODS: &str = "21c96ea8-c8fc-424a-a0b4-e1ffb2fa1a73";

const REUSE_SESSIONS: &[(&str, i64)] = &[("DontUse", 0), ("Use", 1), ("AutoUse", 2)];

/// `HTTPMethod` codes: `DELETE` 2, `GET` 3, `PATCH` 10 (the БСП 8.5 extension
/// ServiceDesk), `POST` 11 and `PUT` 14 are measured; the rest follow the
/// platform's alphabetical enumeration.
const HTTP_METHOD_CODES: &[(&str, i64)] = &[
    ("Any", 0),
    ("CONNECT", 1),
    ("DELETE", 2),
    ("GET", 3),
    ("HEAD", 4),
    ("LOCK", 5),
    ("MERGE", 6),
    ("MKCOL", 7),
    ("MOVE", 8),
    ("OPTIONS", 9),
    ("PATCH", 10),
    ("POST", 11),
    ("PROPFIND", 12),
    ("PROPPATCH", 13),
    ("PUT", 14),
    ("TRACE", 15),
    ("UNLOCK", 16),
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpMethod {
    pub header: Header,
    pub handler: String,
    pub http_method: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpUrlTemplate {
    pub header: Header,
    pub template: String,
    pub methods: Vec<HttpMethod>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpService {
    pub header: Header,
    pub root_url: String,
    pub reuse_sessions: i64,
    pub session_max_age: i64,
    pub templates: Vec<HttpUrlTemplate>,
}

impl HttpService {
    pub fn from_xml(object: &ObjectXml<'_>) -> Result<Self> {
        let p = object.properties()?;
        let mut templates = Vec::new();
        for template in child_objects(object.element, "URLTemplate") {
            let tp = child_properties(template)?;
            let mut methods = Vec::new();
            for method in child_objects(template, "Method") {
                let mp = child_properties(method)?;
                methods.push(HttpMethod {
                    header: Header::of_child(method)?,
                    handler: text(mp, "Handler").to_string(),
                    http_method: enum_prop(mp, "HTTPMethod", HTTP_METHOD_CODES)?,
                });
            }
            templates.push(HttpUrlTemplate {
                header: Header::of_child(template)?,
                template: text(tp, "Template").to_string(),
                methods,
            });
        }
        Ok(Self {
            header: Header::of(object)?,
            root_url: text(p, "RootURL").to_string(),
            reuse_sessions: enum_prop(p, "ReuseSessions", REUSE_SESSIONS)?,
            session_max_age: number(p, "SessionMaxAge")?,
            templates,
        })
    }

    /// `{1,{2,"RootURL",<header>,reuse,max age},1,{<templates>,N,
    /// {{0,"template",<header>},1,{<methods>,M,{{0,"handler",code,<header>},0}...}}...}}`
    pub fn to_brace(&self) -> Brace {
        let mut templates = vec![
            Brace::atom(HTTP_URL_TEMPLATES),
            Brace::num(self.templates.len() as i64),
        ];
        for template in &self.templates {
            let mut methods = vec![
                Brace::atom(HTTP_METHODS),
                Brace::num(template.methods.len() as i64),
            ];
            for method in &template.methods {
                methods.push(brace_list![
                    brace_list![
                        Brace::num(0),
                        Brace::str(method.handler.clone()),
                        Brace::num(method.http_method),
                        method.header.to_brace(),
                    ],
                    Brace::num(0),
                ]);
            }
            templates.push(brace_list![
                brace_list![
                    Brace::num(0),
                    Brace::str(template.template.clone()),
                    template.header.to_brace(),
                ],
                Brace::num(1),
                Brace::List(methods),
            ]);
        }
        brace_list![
            Brace::num(1),
            brace_list![
                Brace::num(2),
                Brace::str(self.root_url.clone()),
                self.header.to_brace(),
                Brace::num(self.reuse_sessions),
                Brace::num(self.session_max_age),
            ],
            Brace::num(1),
            Brace::List(templates),
        ]
    }
}

const WEB_OPERATIONS: &str = "36186084-c23a-43bd-876c-a3a8ba1a9622";
const WEB_PARAMETERS: &str = "b78a00b2-2260-4ef5-a70c-17889cfee695";

const TRANSFER_DIRECTIONS: &[(&str, i64)] = &[("In", 0), ("Out", 1), ("InOut", 2)];

const DATA_LOCK_CONTROL_MODES: &[(&str, i64)] = &[("Automatic", 0), ("Managed", 1)];

/// An XDTO type named by a QName: `{0,"namespace","name"}`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XdtoType {
    pub namespace: String,
    pub name: String,
}

impl XdtoType {
    fn from_xml(element: Option<&Element>) -> Result<Self> {
        let Some(element) = element else {
            return Ok(Self {
                namespace: String::new(),
                name: String::new(),
            });
        };
        let value = element.text.trim();
        let (prefix, name) = value.split_once(':').unwrap_or(("", value));
        let namespace = element
            .namespaces
            .iter()
            .find(|(declared, _)| declared == prefix)
            .map(|(_, uri)| uri.as_str())
            .or_else(|| standard_namespace(prefix))
            .ok_or_else(|| anyhow!("undeclared XML prefix in {value:?}"))?;
        Ok(Self {
            namespace: namespace.to_string(),
            name: name.to_string(),
        })
    }

    fn to_brace(&self) -> Brace {
        brace_list![
            Brace::num(0),
            Brace::str(self.namespace.clone()),
            Brace::str(self.name.clone())
        ]
    }
}

/// The prefixes every metadata XML declares on `<MetaDataObject>`.
fn standard_namespace(prefix: &str) -> Option<&'static str> {
    Some(match prefix {
        "xs" => "http://www.w3.org/2001/XMLSchema",
        "v8" => "http://v8.1c.ru/8.1/data/core",
        "xsi" => "http://www.w3.org/2001/XMLSchema-instance",
        "v8ui" => "http://v8.1c.ru/8.1/data/ui",
        "app" => "http://v8.1c.ru/8.2/managed-application/core",
        "cfg" => "http://v8.1c.ru/8.1/data/enterprise/current-config",
        "ent" => "http://v8.1c.ru/8.1/data/enterprise",
        "xr" => "http://v8.1c.ru/8.3/xcf/readable",
        "xen" => "http://v8.1c.ru/8.3/xcf/enums",
        "style" => "http://v8.1c.ru/8.1/data/ui/style",
        "web" => "http://v8.1c.ru/8.1/data/ui/colors/web",
        "win" => "http://v8.1c.ru/8.1/data/ui/colors/windows",
        "lf" => "http://v8.1c.ru/8.2/managed-application/logform",
        "cmi" => "http://v8.1c.ru/8.2/managed-application/cmi",
        "sys" => "http://v8.1c.ru/8.1/data/ui/fonts/system",
        "xpr" => "http://v8.1c.ru/8.3/xcf/predef",
        _ => return None,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebServiceParameter {
    pub header: Header,
    pub value_type: XdtoType,
    pub nillable: bool,
    pub transfer_direction: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebServiceOperation {
    pub header: Header,
    pub returning_value_type: XdtoType,
    pub nillable: bool,
    pub transactioned: bool,
    pub procedure_name: String,
    pub data_lock_control_mode: i64,
    pub parameters: Vec<WebServiceParameter>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebService {
    pub header: Header,
    pub namespace: String,
    /// `XDTOPackage` references, in source order.
    pub packages: Vec<String>,
    /// Namespace strings of the same list, in source order.
    pub namespaces: Vec<String>,
    pub descriptor_file_name: String,
    pub reuse_sessions: i64,
    pub session_max_age: i64,
    pub operations: Vec<WebServiceOperation>,
}

impl WebService {
    pub fn from_xml(object: &ObjectXml<'_>, context: &DescriptorContext) -> Result<Self> {
        let p = object.properties()?;
        let mut packages = Vec::new();
        let mut namespaces = Vec::new();
        for item in p
            .child("XDTOPackages")
            .into_iter()
            .flat_map(|list| list.children_named("Item"))
        {
            let Some(value) = item.child("Value") else {
                continue;
            };
            let spelled = value.text.trim();
            match value.attr("type") {
                Some("xr:MDObjectRef") => packages.push(resolve(context, spelled)?),
                Some("xs:string") => namespaces.push(spelled.to_string()),
                other => bail!("unsupported XDTOPackages item type {other:?}"),
            }
        }
        let mut operations = Vec::new();
        for operation in child_objects(object.element, "Operation") {
            let op = child_properties(operation)?;
            let mut parameters = Vec::new();
            for parameter in child_objects(operation, "Parameter") {
                let pp = child_properties(parameter)?;
                parameters.push(WebServiceParameter {
                    header: Header::of_child(parameter)?,
                    value_type: XdtoType::from_xml(pp.child("XDTOValueType"))?,
                    nillable: flag(pp, "Nillable")?,
                    transfer_direction: enum_prop(pp, "TransferDirection", TRANSFER_DIRECTIONS)?,
                });
            }
            operations.push(WebServiceOperation {
                header: Header::of_child(operation)?,
                returning_value_type: XdtoType::from_xml(op.child("XDTOReturningValueType"))?,
                nillable: flag(op, "Nillable")?,
                transactioned: flag(op, "Transactioned")?,
                procedure_name: text(op, "ProcedureName").to_string(),
                data_lock_control_mode: enum_prop(
                    op,
                    "DataLockControlMode",
                    DATA_LOCK_CONTROL_MODES,
                )?,
                parameters,
            });
        }
        Ok(Self {
            header: Header::of(object)?,
            namespace: text(p, "Namespace").to_string(),
            packages,
            namespaces,
            descriptor_file_name: text(p, "DescriptorFileName").to_string(),
            reuse_sessions: enum_prop(p, "ReuseSessions", REUSE_SESSIONS)?,
            session_max_age: number(p, "SessionMaxAge")?,
            operations,
        })
    }

    /// `{1,{4,"Namespace",<header>,{0,N,<package refs>},"descriptor",
    /// {M,"ns"...},reuse,max age},1,{<operations>,K,{{1,<header>,<type>,
    /// nillable,transactioned,"procedure",lock mode},1,{<parameters>,L,
    /// {{0,<header>,<type>,nillable,direction},0}...}}...}}`
    pub fn to_brace(&self) -> Brace {
        let mut packages = vec![Brace::num(0), Brace::num(self.packages.len() as i64)];
        packages.extend(self.packages.iter().map(|uuid| design_time_reference(uuid)));
        let mut namespaces = vec![Brace::num(self.namespaces.len() as i64)];
        namespaces.extend(self.namespaces.iter().map(|ns| Brace::str(ns.clone())));
        let mut operations = vec![
            Brace::atom(WEB_OPERATIONS),
            Brace::num(self.operations.len() as i64),
        ];
        for operation in &self.operations {
            let mut parameters = vec![
                Brace::atom(WEB_PARAMETERS),
                Brace::num(operation.parameters.len() as i64),
            ];
            for parameter in &operation.parameters {
                parameters.push(brace_list![
                    brace_list![
                        Brace::num(0),
                        parameter.header.to_brace(),
                        parameter.value_type.to_brace(),
                        Brace::flag(parameter.nillable),
                        Brace::num(parameter.transfer_direction),
                    ],
                    Brace::num(0),
                ]);
            }
            operations.push(brace_list![
                brace_list![
                    Brace::num(1),
                    operation.header.to_brace(),
                    operation.returning_value_type.to_brace(),
                    Brace::flag(operation.nillable),
                    Brace::flag(operation.transactioned),
                    Brace::str(operation.procedure_name.clone()),
                    Brace::num(operation.data_lock_control_mode),
                ],
                Brace::num(1),
                Brace::List(parameters),
            ]);
        }
        brace_list![
            Brace::num(1),
            brace_list![
                Brace::num(4),
                Brace::str(self.namespace.clone()),
                self.header.to_brace(),
                Brace::List(packages),
                Brace::str(self.descriptor_file_name.clone()),
                Brace::List(namespaces),
                Brace::num(self.reuse_sessions),
                Brace::num(self.session_max_age),
            ],
            Brace::num(1),
            Brace::List(operations),
        ]
    }
}

// ---------------------------------------------------------------------------
// Subsystem
// ---------------------------------------------------------------------------

const SUBSYSTEM_CHILDREN: &str = "37f2fa9a-b276-11d4-9435-004095e12fc7";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Subsystem {
    pub header: Header,
    pub include_help_in_contents: bool,
    pub include_in_command_interface: bool,
    pub use_one_command: bool,
    pub explanation: Localized,
    pub picture: Picture,
    /// Content object uuids, in source order.
    pub content: Vec<String>,
    /// Nested subsystem uuids, in source order.
    pub children: Vec<String>,
}

impl Subsystem {
    pub fn from_xml(object: &ObjectXml<'_>, context: &DescriptorContext) -> Result<Self> {
        let p = object.properties()?;
        let mut content = Vec::new();
        for item in p
            .child("Content")
            .into_iter()
            .flat_map(|list| list.children_named("Item"))
        {
            content.push(resolve_reference(context, item.text.trim())?);
        }
        let own = full_name_of(object, context)?;
        let children = child_objects(object.element, "Subsystem")
            .map(|child| resolve(context, &format!("{own}.Subsystem.{}", child.text.trim())))
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            header: Header::of(object)?,
            include_help_in_contents: flag(p, "IncludeHelpInContents")?,
            include_in_command_interface: flag(p, "IncludeInCommandInterface")?,
            use_one_command: flag(p, "UseOneCommand")?,
            explanation: localized_of(p, "Explanation"),
            picture: Picture::from_xml(p.child("Picture"), context)?,
            content,
            children,
        })
    }

    /// `{1,{22,<header>,help,{0,0},in interface,<picture>,<explanation>,
    /// {0,N,<content refs>},one command},1,{<children>,K,<uuids>}}`
    pub fn to_brace(&self) -> Brace {
        let mut content = vec![Brace::num(0), Brace::num(self.content.len() as i64)];
        content.extend(self.content.iter().map(|uuid| design_time_reference(uuid)));
        let mut children = vec![
            Brace::atom(SUBSYSTEM_CHILDREN),
            Brace::num(self.children.len() as i64),
        ];
        children.extend(self.children.iter().map(|uuid| Brace::uuid(uuid)));
        brace_list![
            Brace::num(1),
            brace_list![
                Brace::num(22),
                self.header.to_brace(),
                Brace::flag(self.include_help_in_contents),
                brace_list![Brace::num(0), Brace::num(0)],
                Brace::flag(self.include_in_command_interface),
                self.picture.to_brace(),
                localized_brace(&self.explanation),
                Brace::List(content),
                Brace::flag(self.use_one_command),
            ],
            Brace::num(1),
            Brace::List(children),
        ]
    }
}

// ---------------------------------------------------------------------------
// Forms
// ---------------------------------------------------------------------------

/// The class of an `ApplicationUsePurpose` value.
const USE_PURPOSE_CLASS: &str = "1708fdaa-cbce-4289-b373-07a5a74bee91";

/// The record every form descriptor holds:
/// `{13,<header>,help,type,{N,{"#",<class>,purpose}...}}`, and on 8.5
/// `{14,...,compatibility mode}`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormRecord {
    pub header: Header,
    pub include_help_in_contents: bool,
    pub form_type: i64,
    pub use_purposes: Vec<i64>,
    /// `UseInInterfaceCompatibilityMode`, written by 8.5 only.
    pub interface_compatibility_mode: Option<i64>,
}

impl FormRecord {
    fn from_xml(object: &ObjectXml<'_>, context: &DescriptorContext) -> Result<Self> {
        let p = object.properties()?;
        let use_purposes = p
            .child("UsePurposes")
            .into_iter()
            .flat_map(|list| list.children_named("Value"))
            .map(|value| code(&value.text, USE_PURPOSES, "UsePurposes"))
            .collect::<Result<Vec<_>>>()?;
        let interface_compatibility_mode = if stores_layout_8_5_1(context) {
            Some(match p.child_text("UseInInterfaceCompatibilityMode") {
                Some(value) => code(
                    value,
                    INTERFACE_COMPATIBILITY_MODES,
                    "UseInInterfaceCompatibilityMode",
                )?,
                None => 0,
            })
        } else {
            None
        };
        Ok(Self {
            header: Header::of(object)?,
            include_help_in_contents: flag(p, "IncludeHelpInContents")?,
            form_type: enum_prop(p, "FormType", FORM_TYPES)?,
            use_purposes,
            interface_compatibility_mode,
        })
    }

    fn to_brace(&self) -> Brace {
        let mut purposes = vec![Brace::num(self.use_purposes.len() as i64)];
        purposes.extend(self.use_purposes.iter().map(|purpose| {
            brace_list![
                Brace::str("#"),
                Brace::atom(USE_PURPOSE_CLASS),
                Brace::num(*purpose)
            ]
        }));
        let version = if self.interface_compatibility_mode.is_some() {
            14
        } else {
            13
        };
        let mut items = vec![
            Brace::num(version),
            self.header.to_brace(),
            Brace::flag(self.include_help_in_contents),
            Brace::num(self.form_type),
            Brace::List(purposes),
        ];
        if let Some(mode) = self.interface_compatibility_mode {
            items.push(Brace::num(mode));
        }
        Brace::List(items)
    }
}

/// How an owner kind wraps its forms' record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormWrapper {
    /// `{1,<record>,0}`: business processes, tasks, charts of accounts and of
    /// characteristic types, accounting registers.
    Bare,
    /// `{1,{0,<record>},0}`: catalogs, documents, registers, ...
    Versioned,
    /// `{1,{1,{0,<record>,<extended presentation>}},0}`: data processors and
    /// reports.
    WithPresentation,
}

fn form_wrapper(owner_folder: &str) -> Result<FormWrapper> {
    Ok(match owner_folder {
        "BusinessProcesses"
        | "Tasks"
        | "ChartsOfAccounts"
        | "ChartsOfCharacteristicTypes"
        | "AccountingRegisters" => FormWrapper::Bare,
        "Catalogs"
        | "Documents"
        | "DocumentJournals"
        | "Enums"
        | "ExchangePlans"
        | "FilterCriteria"
        | "InformationRegisters"
        | "AccumulationRegisters"
        | "CalculationRegisters"
        | "ChartsOfCalculationTypes"
        | "SettingsStorages" => FormWrapper::Versioned,
        "DataProcessors" | "Reports" => FormWrapper::WithPresentation,
        other => bail!("no form wrapper for owner folder {other}"),
    })
}

/// The first folder of an object's path under the tree (`Catalogs`).
fn owner_folder(object: &ObjectXml<'_>, context: &DescriptorContext) -> Result<String> {
    let relative = object
        .path
        .strip_prefix(&context.root)
        .unwrap_or(object.path);
    relative
        .components()
        .next()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .ok_or_else(|| anyhow!("{} {} has no owner folder", object.kind, object.name))
}

/// An object's form (`Catalogs/X/Forms/F.xml`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Form {
    pub record: FormRecord,
    pub wrapper: FormWrapper,
    pub extended_presentation: Localized,
}

impl Form {
    pub fn from_xml(object: &ObjectXml<'_>, context: &DescriptorContext) -> Result<Self> {
        let p = object.properties()?;
        Ok(Self {
            record: FormRecord::from_xml(object, context)?,
            wrapper: form_wrapper(&owner_folder(object, context)?)?,
            extended_presentation: localized_of(p, "ExtendedPresentation"),
        })
    }

    pub fn to_brace(&self) -> Brace {
        let record = self.record.to_brace();
        let body = match self.wrapper {
            FormWrapper::Bare => record,
            FormWrapper::Versioned => brace_list![Brace::num(0), record],
            FormWrapper::WithPresentation => brace_list![
                Brace::num(1),
                brace_list![
                    Brace::num(0),
                    record,
                    localized_brace(&self.extended_presentation)
                ],
            ],
        };
        brace_list![Brace::num(1), body, Brace::num(0)]
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommonForm {
    pub record: FormRecord,
    pub extended_presentation: Localized,
    pub explanation: Localized,
    pub use_standard_commands: bool,
}

impl CommonForm {
    pub fn from_xml(object: &ObjectXml<'_>, context: &DescriptorContext) -> Result<Self> {
        let p = object.properties()?;
        Ok(Self {
            record: FormRecord::from_xml(object, context)?,
            extended_presentation: localized_of(p, "ExtendedPresentation"),
            explanation: localized_of(p, "Explanation"),
            use_standard_commands: flag(p, "UseStandardCommands")?,
        })
    }

    /// `{1,{4,<record>,<extended presentation>,<explanation>,standard commands},0}`
    pub fn to_brace(&self) -> Brace {
        brace_list![
            Brace::num(1),
            brace_list![
                Brace::num(4),
                self.record.to_brace(),
                localized_brace(&self.extended_presentation),
                localized_brace(&self.explanation),
                Brace::flag(self.use_standard_commands),
            ],
            Brace::num(0),
        ]
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::metadata_model::brace::serialize_row;
    use crate::metadata_model::compile_descriptor;
    use crate::metadata_model::xml::parse_element_tree;

    const NIL: &str = "00000000-0000-0000-0000-000000000000";

    #[test]
    fn a_2_21_form_without_its_opening_mode_or_group_needs_the_8_5_layout() {
        let form = |leading: &str| {
            format!(
                "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<Form xmlns=\"http://v8.1c.ru/8.3/xcf/logform\" version=\"2.21\">\r\n{leading}\t<AutoCommandBar name=\"ФормаКоманднаяПанель\" id=\"-1\"/>\r\n\t<ChildItems>\r\n\t\t<Group>x</Group>\r\n\t</ChildItems>\r\n</Form>"
            )
        };
        // How 8.5 prints a form stored the 8.3.27 way (ERP УХ 8.5).
        let both =
            "\t<WindowOpeningMode>DontBlock</WindowOpeningMode>\r\n\t<Group>Vertical</Group>\r\n";
        assert!(!form_needs_layout_8_5_1(&form(both)));
        // `home_page/one_column_v85`: neither (a nested `<Group>` does not count).
        assert!(form_needs_layout_8_5_1(&form("")));
        assert!(form_needs_layout_8_5_1(&form(
            "\t<WindowOpeningMode>LockOwner</WindowOpeningMode>\r\n"
        )));
        // 2.20 is read by 8.3.27, and a head that ends inside the root's
        // leading elements decides nothing.
        assert!(!form_needs_layout_8_5_1(
            &form("").replace("version=\"2.21\"", "version=\"2.20\"")
        ));
        let whole = form("");
        let cut = whole.find("\t<AutoCommandBar").unwrap();
        assert!(!form_needs_layout_8_5_1(&whole[..cut]));
    }

    #[test]
    fn writes_a_common_module_as_bsp_stores_it() {
        let module = CommonModule {
            header: Header {
                uuid: "ac847dc9-e222-45cf-af4a-6fa863c919a8".into(),
                name: "GoogleПереводчик".into(),
                synonym: vec![("ru".into(), "Google переводчик".into())],
                comment: String::new(),
            },
            global: false,
            client_managed_application: false,
            server: true,
            external_connection: true,
            client_ordinary_application: true,
            server_call: false,
            privileged: false,
            return_values_reuse: 0,
        };
        let expected = format!(
            "\u{feff}{{1,\r\n{{12,\r\n{{3,\r\n{{1,0,ac847dc9-e222-45cf-af4a-6fa863c919a8}},\
             \"GoogleПереводчик\",\r\n{{1,\"ru\",\"Google переводчик\"}},\"\",0,0,{NIL},0}},\
             1,1,1,0,0,0,0,0}},0}}"
        );
        assert_eq!(
            String::from_utf8(serialize_row(&module.to_brace())).unwrap(),
            expected
        );
    }

    #[test]
    fn resolves_xdto_type_prefixes_declared_on_the_element() {
        let own = parse_element_tree(
            br#"<XDTOReturningValueType xmlns:d6p1="urn:exchange">d6p1:Features</XDTOReturningValueType>"#,
        )
        .unwrap();
        assert_eq!(
            XdtoType::from_xml(Some(&own)).unwrap(),
            XdtoType {
                namespace: "urn:exchange".into(),
                name: "Features".into()
            }
        );
        let standard = parse_element_tree(br#"<XDTOValueType>xs:string</XDTOValueType>"#).unwrap();
        assert_eq!(
            XdtoType::from_xml(Some(&standard)).unwrap().namespace,
            "http://www.w3.org/2001/XMLSchema"
        );
    }

    fn write(root: &Path, relative: &str, body: &str) -> PathBuf {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<MetaDataObject \
                 xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" \
                 xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" \
                 xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"2.20\">\n{body}\n</MetaDataObject>"
            ),
        )
        .unwrap();
        path
    }

    #[test]
    fn compiles_a_subsystem_with_content_children_and_a_multiline_explanation() {
        let root = std::env::temp_dir().join(format!("ibcmd_rs_common_{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        write(
            &root,
            "CommonModules/M.xml",
            "<CommonModule uuid=\"cccccccc-0000-0000-0000-000000000003\"><Properties><Name>M</Name>\
             </Properties></CommonModule>",
        );
        write(
            &root,
            "Subsystems/A/Subsystems/B.xml",
            "<Subsystem uuid=\"bbbbbbbb-0000-0000-0000-000000000002\"><Properties><Name>B</Name>\
             </Properties><ChildObjects/></Subsystem>",
        );
        let a = write(
            &root,
            "Subsystems/A.xml",
            "<Subsystem uuid=\"AAAAAAAA-0000-0000-0000-000000000001\"><Properties><Name>A</Name>\
             <Synonym><v8:item><v8:lang>ru</v8:lang><v8:content>А</v8:content></v8:item></Synonym>\
             <Comment/><IncludeHelpInContents>true</IncludeHelpInContents>\
             <IncludeInCommandInterface>true</IncludeInCommandInterface>\
             <UseOneCommand>false</UseOneCommand>\
             <Explanation><v8:item><v8:lang>ru</v8:lang><v8:content>Line one\nline two</v8:content>\
             </v8:item></Explanation><Picture/>\
             <Content><xr:Item xsi:type=\"xr:MDObjectRef\">CommonModule.M</xr:Item>\
             <xr:Item xsi:type=\"xr:MDObjectRef\">0:dddddddd-0000-0000-0000-000000000004</xr:Item>\
             </Content></Properties><ChildObjects><Subsystem>B</Subsystem></ChildObjects></Subsystem>",
        );
        let context = DescriptorContext::new(&root, "2.20").unwrap();
        let compiled =
            compile_descriptor("Subsystem", &a, &fs::read(&a).unwrap(), &context).unwrap();
        let _ = fs::remove_dir_all(&root);
        let reference =
            |uuid: &str| format!("\r\n{{\"#\",{DESIGN_TIME_REFERENCE},\r\n{{1,{uuid}}}\r\n}}");
        let expected = format!(
            "\u{feff}{{1,\r\n{{22,\r\n{{3,\r\n{{1,0,aaaaaaaa-0000-0000-0000-000000000001}},\"A\",\
             \r\n{{1,\"ru\",\"А\"}},\"\",0,0,{NIL},0}},1,\r\n{{0,0}},1,\
             \r\n{{4,0,\r\n{{0}},\"\",-1,-1,1,0,\"\"}},\r\n{{1,\"ru\",\"Line one\r\nline two\"}},\
             \r\n{{0,2,{},{}\r\n}},0}},1,\r\n{{{SUBSYSTEM_CHILDREN},1,bbbbbbbb-0000-0000-0000-000000000002}}\r\n}}",
            reference("cccccccc-0000-0000-0000-000000000003"),
            reference("dddddddd-0000-0000-0000-000000000004"),
        );
        assert_eq!(String::from_utf8(compiled).unwrap(), expected);
    }
}
