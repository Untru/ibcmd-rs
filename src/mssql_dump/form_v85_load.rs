//! Loading a platform 8.5 (dialect 2.21) `Form.xml`: the inverse of
//! `form_v85` (the down-conversion of a stored 8.5 body) and of
//! `form_v85_writer` (the 2.21 pass over the 8.3.27 reading).
//!
//! A 2.21 form loads in three steps:
//!
//! 1. [`down_convert_v85_form_xml`] takes out of the XML what only the members
//!    8.5 appends can hold, keeping it as facts by item id, and puts back the
//!    8.3.27 spelling of what 8.5 keeps in the 8.3.27 part of a record -- the
//!    XML the 8.3.27 form writer reads;
//! 2. the 8.3.27 native form writer compiles that XML;
//! 3. [`up_convert_v83_form_body`] bumps every record to its 8.5 revision,
//!    appends the members the facts name (their defaults elsewhere), spells
//!    colours and fonts the 8.5 way and lays the text out as the platform does.
//!
//! Every code and default below is read off the 1 120 forms of the 8.5.1.1150
//! BSP 3.2.1.356 clone, whose bodies 8.5 re-saved: each rule pairs the native
//! 2.21 element with the member the stored body holds and with what the
//! 8.3.27 part of the same record reads (`F:/ibcmd/lab/v85/tools/jq.py` over
//! `findings/join3-bsp.pickle`). A spelling outside that evidence refuses the
//! form.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};

use crate::module_blob::MetadataSourceContext;
use super::form_v85::Node;
use super::form_v85_writer::{
    FIELD_TAGS, GROUPING, TRI_STATE, XmlEdits, child_text, element_name, simple_text,
};

const FORM_ITEM_CLASS_UUID: &str = "02023637-7868-4a5f-8576-835a76e0c9ba";
const FORM_COMMAND_CLASS_UUID: &str = "409b9a53-7f7e-4178-86c1-33176c7c7a7a";
const FORM_CHOICE_LIST_VALUE_UUID: &str = "0e704aa2-07bd-48b9-8223-a0212c4d5fc2";
/// The event a usual group handles in 8.5 (`Click`), as its record names it.
const GROUP_CLICK_EVENT_UUID: &str = "a3da1388-983a-4d28-87f2-5096d7e3a4ef";

/// The picture member of a record that names none.
const EMPTY_PICTURE: &str = "{4,0,{0},\"\",-1,-1,1,0,\"\"}";
/// The colour member of a record that names none (8.3.27 spelling; the
/// primitive pass spells it the 8.5 way).
const UNSET_COLOR: &str = "{3,4,{0}}";
/// The control border an HTML document field appends.
const DEFAULT_BORDER: &str = "{3,0,{0},0,1,0,48312c09-257f-4b29-b280-284dd89efc1e}";

/// Whether a `Form.xml` is written in dialect 2.21 (platform 8.5).
pub(crate) fn is_v85_form_xml(xml: &[u8]) -> bool {
    let head = &xml[..xml.len().min(8192)];
    let text = String::from_utf8_lossy(head);
    let Some(at) = text.find("<Form") else {
        return false;
    };
    let end = text[at..].find('>').map_or(text.len(), |end| at + end);
    text[at..end].contains("version=\"2.21\"")
}

/// What a 2.21 `Form.xml` says that only the members 8.5 appends hold.
#[derive(Debug, Clone, Default)]
pub(crate) struct V85FormLoadFacts {
    /// Root trailer members by index.
    root_tail: BTreeMap<usize, String>,
    /// The root's scale percentage, when not 100.
    root_scale: Option<String>,
    /// The root's `Group` is unset or `AutoScreenTypeSensitive`: 8.5 keeps
    /// the 8.3.27 "group set" flag (head member 11) and group code (the tenth
    /// member from the 8.3.27 root's end) at 1 (all 252 such BSP forms).
    root_group_horizontal: bool,
    /// Facts by form item id.
    items: BTreeMap<String, ItemFacts>,
    /// Appended form-command members by command id.
    commands: BTreeMap<String, BTreeMap<usize, String>>,
    /// The picture 8.5 appends to every choice-list value, in document order.
    choice_value_pictures: Vec<String>,
    /// A report form's state in the root bag, when 8.5 stores it
    /// ([`report_state_of`]).
    report_state: Option<ReportState>,
}

/// What a report form's root bag says about the report beyond `Form.xml`'s
/// own elements ([`add_report_state`]).
#[derive(Debug, Clone)]
struct ReportState {
    /// `Отчет.<name>` of the report the main attribute holds; empty for the
    /// generic `cfg:ReportObject` of a common report form.
    report: String,
    /// `Configuration.xml`'s uuid, which the default report form URNs name.
    configuration: String,
}

#[derive(Debug, Clone, Default)]
struct ItemFacts {
    tag: String,
    name: String,
    /// Members the item record appends, by index.
    tail: BTreeMap<usize, String>,
    /// Members the item's property bag appends, by index.
    bag_tail: BTreeMap<usize, String>,
    /// Members of the 8.3.27 part of the item's property bag, by index.
    bag: BTreeMap<usize, String>,
    /// Members of the 8.3.27 part of the record (before the optional common
    /// prefix shifts them), by index.
    record: BTreeMap<usize, String>,
    /// A table's `ComplexSettingsViewMode` of `Show`: key 21 of its keyed
    /// property bag, which then holds no empty key 19 (the three BSP tables).
    complex_settings_view_mode: bool,
}

/// The code a value table gives a 2.21 spelling (`None` = absent). A value the
/// table does not list, or an absence several codes share, is refused.
fn code_of(table: &[(&str, Option<&str>)], value: Option<&str>, what: &str) -> Result<String> {
    let found: Vec<&str> = table
        .iter()
        .filter(|(_, spelled)| *spelled == value)
        .map(|(code, _)| *code)
        .collect();
    match found.as_slice() {
        [code] => Ok((*code).to_owned()),
        [] => bail!("{what}: the 2.21 value {value:?} has no 8.5 code"),
        _ => bail!("{what}: the 2.21 value {value:?} maps to several 8.5 codes"),
    }
}

/// Reads a one-line child element's value.
fn peek(edits: &XmlEdits<'_>, parent: usize, tag: &str) -> Result<Option<String>> {
    Ok(child_text(edits, parent, tag)?.map(|(_, value)| value.to_owned()))
}

/// Reads a one-line child element's value and removes the element.
fn take(edits: &mut XmlEdits<'_>, parent: usize, tag: &str) -> Result<Option<String>> {
    match child_text(edits, parent, tag)? {
        Some((child, value)) => {
            let value = value.to_owned();
            edits.remove(child);
            Ok(Some(value))
        }
        None => Ok(None),
    }
}

/// Removes a child element, one-line or not, and returns its whole text.
fn take_subtree(edits: &mut XmlEdits<'_>, parent: usize, tag: &str) -> Result<Option<usize>> {
    let children = edits.direct_children(parent, tag);
    match children.as_slice() {
        [] => Ok(None),
        [child] => {
            edits.remove(*child);
            Ok(Some(*child))
        }
        _ => bail!("<{}> writes <{tag}> twice", edits.elements[parent].tag),
    }
}

/// Replaces (or adds) a one-line child element with the 8.3.27 spelling.
fn put(edits: &mut XmlEdits<'_>, parent: usize, tag: &str, value: &str) -> Result<()> {
    let line = format!("<{tag}>{value}</{tag}>\r\n");
    match edits.direct_children(parent, tag).as_slice() {
        [child] => {
            edits.replace(*child, &line);
            Ok(())
        }
        [] => edits.insert_child(parent, tag, &line),
        _ => bail!("<{}> writes <{tag}> twice", edits.elements[parent].tag),
    }
}

/// Keeps a one-line element only when its value is in `keep`.
fn keep_only(edits: &mut XmlEdits<'_>, parent: usize, tag: &str, keep: &[&str]) -> Result<()> {
    if let Some((child, value)) = child_text(edits, parent, tag)?
        && !keep.contains(&value)
    {
        edits.remove(child);
    }
    Ok(())
}

fn unescape_xml(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

fn quote_1c(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

/// The text between an element's opening and closing tags.
fn inner_text<'x>(edits: &XmlEdits<'x>, element: usize) -> &'x str {
    let el = &edits.elements[element];
    let text = &edits.xml[el.open_start..el.line_end];
    let Some(open_end) = text.find('>') else {
        return "";
    };
    if el.self_closing {
        return "";
    }
    let close = text.rfind("</").unwrap_or(text.len());
    &text[open_end + 1..close.max(open_end + 1)]
}

/// A localized string element (`<v8:item><v8:lang>..</v8:lang><v8:content>..`)
/// as a body stores it: `{1,<count>,{"<lang>","<content>"}...}`.
fn localized_member(edits: &XmlEdits<'_>, element: usize) -> Result<String> {
    let mut pairs = Vec::new();
    for &item in &edits.elements[element].children {
        if edits.elements[item].tag != "v8:item" {
            bail!("<{}> holds <{}>", edits.elements[element].tag, edits.elements[item].tag);
        }
        let mut lang = None;
        let mut content = None;
        for &part in &edits.elements[item].children {
            match edits.elements[part].tag.as_str() {
                "v8:lang" => lang = Some(unescape_xml(inner_text(edits, part))),
                "v8:content" => content = Some(unescape_xml(inner_text(edits, part))),
                other => bail!("a localized string holds <{other}>"),
            }
        }
        let (Some(lang), Some(content)) = (lang, content) else {
            bail!("a localized string item lacks its language or content");
        };
        pairs.push((lang, content));
    }
    let mut out = format!("{{1,{}", pairs.len());
    for (lang, content) in pairs {
        out.push_str(&format!(",{{{},{}}}", quote_1c(&lang), quote_1c(&content)));
    }
    out.push('}');
    Ok(out)
}

/// A `<Picture>` element as the picture member of a body.
fn picture_member(
    edits: &XmlEdits<'_>,
    element: usize,
    holder: &str,
    item_name: &str,
    source: Option<&MetadataSourceContext>,
    items_root: Option<&Path>,
) -> Result<String> {
    let mut abs = None;
    let mut reference = None;
    let mut load_transparent = None;
    let mut transparent_x = None;
    let mut transparent_y = None;
    for &part in &edits.elements[element].children {
        let tag = edits.elements[part].tag.as_str();
        match tag {
            "xr:Abs" => abs = simple_text(edits, part).map(unescape_xml),
            "xr:Ref" => reference = simple_text(edits, part).map(unescape_xml),
            "xr:LoadTransparent" => load_transparent = simple_text(edits, part).map(str::to_owned),
            "xr:TransparentPixel" => {
                let el = &edits.elements[part];
                let open = &edits.xml[el.open_start..el.line_end];
                let open = &open[..open.find('>').unwrap_or(open.len())];
                let attribute = |name: &str| {
                    let key = format!(" {name}=\"");
                    open.find(&key).and_then(|at| {
                        let value = &open[at + key.len()..];
                        value.find('"').map(|end| value[..end].to_owned())
                    })
                };
                transparent_x = attribute("x");
                transparent_y = attribute("y");
            }
            other => bail!("<{holder}> <Picture> holds <{other}>"),
        }
    }
    crate::module_blob::native_picture_from_parts(
        holder,
        item_name,
        abs,
        reference,
        load_transparent,
        transparent_x,
        transparent_y,
        source,
        items_root,
    )
}

fn color_member(value: &str, what: &str, source: Option<&MetadataSourceContext>) -> Result<String> {
    crate::module_blob::native_item_color(Some(value), source)
        .ok_or_else(|| anyhow!("{what}: the writer cannot place the colour {value}"))
}

/// Takes out of a 2.21 `Form.xml` what only the members 8.5 appends hold and
/// returns the XML the 8.3.27 writer reads, with those members as facts.
pub(crate) fn down_convert_v85_form_xml(
    xml: &str,
    source: Option<&MetadataSourceContext>,
    items_root: Option<&Path>,
) -> Result<(String, V85FormLoadFacts)> {
    let mut facts = V85FormLoadFacts::default();
    let mut edits = XmlEdits::new_lenient(xml)?;
    let root = edits
        .elements
        .iter()
        .position(|element| element.parent.is_none() && element.tag == "Form")
        .ok_or_else(|| anyhow!("2.21 form XML has no <Form> root"))?;
    facts.report_state = report_state_of(&edits, root, source, items_root)?;
    load_root(&mut edits, root, &mut facts)?;

    // Items by id outside the attribute, command and parameter sections,
    // whose ids number different spaces; commands by id inside theirs.
    let mut items = Vec::new();
    let mut commands = Vec::new();
    for (index, element) in edits.elements.iter().enumerate() {
        let Some(id) = element.id.clone() else {
            continue;
        };
        let mut section = None;
        let mut parent = element.parent;
        while let Some(ancestor) = parent {
            let tag = edits.elements[ancestor].tag.as_str();
            if matches!(tag, "Attributes" | "Commands" | "Parameters" | "CommandInterface") {
                section = Some(tag);
                break;
            }
            parent = edits.elements[ancestor].parent;
        }
        match section {
            None => items.push((index, id)),
            Some("Commands") if element.tag == "Command" => commands.push((index, id)),
            Some(_) => {}
        }
    }
    for (element, id) in items {
        let tag = edits.elements[element].tag.clone();
        let name = element_name(&edits, element).unwrap_or_default();
        let mut item = ItemFacts {
            tag: tag.clone(),
            name: name.clone(),
            ..ItemFacts::default()
        };
        load_item(&mut edits, element, &tag, &name, &mut item, source, items_root)
            .with_context(|| format!("2.21 form item <{tag}> {name} (id {id})"))?;
        facts.items.insert(id, item);
    }
    for (element, id) in commands {
        let mut tail = BTreeMap::new();
        let purpose = take(&mut edits, element, "ActionPurpose")?;
        tail.insert(
            0,
            code_of(
                &[("0", None), ("1", Some("Create")), ("2", Some("Finish"))],
                purpose.as_deref(),
                "<Command> ActionPurpose",
            )?,
        );
        let rows = take(&mut edits, element, "SelectedRowsUse")?;
        tail.insert(
            1,
            code_of(
                &[("0", None), ("1", Some("Use")), ("2", Some("DontUse"))],
                rows.as_deref(),
                "<Command> SelectedRowsUse",
            )?,
        );
        facts.commands.insert(id, tail);
    }
    load_choice_value_pictures(&mut edits, &mut facts, source, items_root)?;
    // The reading is 8.3.27's: its root says 2.20, so nothing takes it for a
    // 2.21 form again.
    let mut xml20 = edits.finish()?;
    let at = xml20
        .find("<Form")
        .ok_or_else(|| anyhow!("2.21 form XML has no <Form> root"))?;
    let end = xml20[at..].find('>').map_or(xml20.len(), |end| at + end);
    let version = xml20[at..end]
        .find("version=\"2.21\"")
        .map(|offset| at + offset)
        .ok_or_else(|| anyhow!("the 2.21 <Form> root does not declare version 2.21"))?;
    xml20.replace_range(version..version + "version=\"2.21\"".len(), "version=\"2.20\"");
    Ok((xml20, facts))
}

/// Whether 8.5 stores a report form's state in the root bag, and what names
/// it. The state is not in `Form.xml` and the export reads none of it back;
/// which forms carry it is the forms' history. Measured on the stored rows
/// of both corpora (8.5.1.1150 BSP and ERP УХ; the 8.3.27 bodies of both hold
/// the same keys), for forms whose main attribute holds a report object:
///
/// * every form whose `<ReportFormType>` is `Main` or `Variant` carries it
///   (BSP 7 of 7, ERP УХ 206 of 208), and so does a common settings form
///   (4 of 4); a report's own settings form does not (BSP 1 of 1, ERP УХ 8 of
///   12) -- the rule this follows;
/// * key 12 names the main attribute's report (`Отчет.<name>`, empty for a
///   common form's `cfg:ReportObject`): BSP 9 of 9, ERP УХ 186 of 212, the
///   rest the names of external reports the forms were copied from;
/// * keys 8-11 are `urn:form:md:14/13/16/15:<configuration uuid>`: BSP 9 of
///   9, ERP УХ 161 of 212 (the rest external-report URNs).
fn report_state_of(
    edits: &XmlEdits<'_>,
    root: usize,
    source: Option<&MetadataSourceContext>,
    items_root: Option<&Path>,
) -> Result<Option<ReportState>> {
    let Some(form_type) = peek(edits, root, "ReportFormType")? else {
        return Ok(None);
    };
    let mut report = None;
    'attributes: for attributes in edits.direct_children(root, "Attributes") {
        for attribute in edits.direct_children(attributes, "Attribute") {
            if peek(edits, attribute, "MainAttribute")?.as_deref() != Some("true") {
                continue;
            }
            for types in edits.direct_children(attribute, "Type") {
                for value in edits.direct_children(types, "v8:Type") {
                    let text = simple_text(edits, value).unwrap_or_default().trim();
                    if text == "cfg:ReportObject" {
                        report = Some(String::new());
                    } else if let Some(name) = text.strip_prefix("cfg:ReportObject.") {
                        report = Some(format!("Отчет.{name}"));
                    }
                    if report.is_some() {
                        break 'attributes;
                    }
                }
            }
            break 'attributes;
        }
    }
    let Some(report) = report else {
        return Ok(None);
    };
    // `<tree>/CommonForms/<form>/Ext/Form/Items` or
    // `<tree>/<kind>/<owner>/Forms/<form>/Ext/Form/Items`.
    let folder = items_root
        .and_then(|path| path.ancestors().nth(4))
        .and_then(Path::file_name)
        .and_then(|folder| folder.to_str());
    let common = match folder {
        Some("CommonForms") => true,
        Some("Forms") => false,
        _ => return Ok(None),
    };
    if form_type == "Settings" && !common {
        return Ok(None);
    }
    let Some(configuration) = source.and_then(MetadataSourceContext::configuration_uuid) else {
        return Ok(None);
    };
    Ok(Some(ReportState {
        report,
        configuration,
    }))
}

fn load_root(edits: &mut XmlEdits<'_>, root: usize, facts: &mut V85FormLoadFacts) -> Result<()> {
    // WindowOpeningMode: 8.5 keeps `LockOwnerWindow` and `LockWholeInterface`
    // in the 8.3.27 slot and leaves an explicit `DontBlock` unset there.
    let mode = peek(edits, root, "WindowOpeningMode")?;
    facts.root_tail.insert(
        4,
        code_of(
            &[
                ("0", Some("DontBlock")),
                ("1", Some("LockOwner")),
                ("2", Some("LockWholeInterface")),
                ("3", None),
            ],
            mode.as_deref(),
            "<Form> WindowOpeningMode",
        )?,
    );
    match mode.as_deref() {
        Some("LockOwner") => put(edits, root, "WindowOpeningMode", "LockOwnerWindow")?,
        Some("DontBlock") => {
            take(edits, root, "WindowOpeningMode")?;
        }
        _ => {}
    }
    let view = take(edits, root, "WindowViewMode")?;
    facts.root_tail.insert(
        5,
        code_of(
            &[("0", None), ("1", Some("InMainWindow")), ("2", Some("InDialogWindow"))],
            view.as_deref(),
            "<Form> WindowViewMode",
        )?,
    );
    let bar = take(edits, root, "ShowCommandBar")?;
    facts
        .root_tail
        .insert(6, code_of(TRI_STATE, bar.as_deref(), "<Form> ShowCommandBar")?);
    let group = peek(edits, root, "Group")?;
    let group_code = code_of(GROUPING, group.as_deref(), "<Form> Group")?;
    facts.root_group_horizontal = matches!(group_code.as_str(), "4" | "5");
    facts.root_tail.insert(7, group_code);
    keep_only(
        edits,
        root,
        "Group",
        &["Horizontal", "HorizontalIfPossible", "AlwaysHorizontal"],
    )?;
    let title = peek(edits, root, "ShowTitle")?;
    facts
        .root_tail
        .insert(10, code_of(TRI_STATE, title.as_deref(), "<Form> ShowTitle")?);
    keep_only(edits, root, "ShowTitle", &["false"])?;
    if let Some(element) = take_subtree(edits, root, "CreateButtonsGroupTitle")? {
        facts.root_tail.insert(0, localized_member(edits, element)?);
    }
    facts.root_scale = take(edits, root, "Scale")?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn load_item(
    edits: &mut XmlEdits<'_>,
    element: usize,
    tag: &str,
    name: &str,
    item: &mut ItemFacts,
    source: Option<&MetadataSourceContext>,
    items_root: Option<&Path>,
) -> Result<()> {
    if FIELD_TAGS.contains(&tag) {
        load_field(edits, element, tag, name, item, source, items_root)?;
    }
    match tag {
        "Button" => {
            let importance = take(edits, element, "ButtonImportance")?;
            item.tail.insert(
                5,
                code_of(
                    &[("0", Some("Main")), ("1", None), ("2", Some("Supplementary"))],
                    importance.as_deref(),
                    "ButtonImportance",
                )?,
            );
            // The last member of the 8.3.27 record: `2` only on a button that
            // writes `DontChangeBehavior`; the writer keeps its own otherwise.
            match take(edits, element, "OnMainServerUnavalableBehavior")?.as_deref() {
                None => {}
                Some("DontChangeBehavior") => {
                    item.record.insert(51, "2".to_owned());
                }
                Some(other) => bail!("OnMainServerUnavalableBehavior {other} has no 8.5 code"),
            }
        }
        "Table" => load_table(edits, element, item)?,
        "UsualGroup" => load_usual_group(edits, element, item)?,
        "Page" => {
            let group = peek(edits, element, "Group")?;
            let group_code = code_of(GROUPING, group.as_deref(), "Group")?;
            // Unset or AutoScreenTypeSensitive: 8.5 keeps both 8.3.27 group
            // members of the page bag at 1 (184 of 184 BSP pages).
            if matches!(group_code.as_str(), "4" | "5") {
                item.bag.insert(2, "1".to_owned());
                item.bag.insert(16, "1".to_owned());
            }
            item.bag_tail.insert(0, group_code);
            keep_only(
                edits,
                element,
                "Group",
                &["Horizontal", "HorizontalIfPossible", "AlwaysHorizontal"],
            )?;
            // A page keeps an 8.3.27 `ScrollOnCompress` of `true` (141 BSP
            // pages); the member reads all three states.
            let scroll = peek(edits, element, "ScrollOnCompress")?;
            item.bag_tail
                .insert(1, code_of(TRI_STATE, scroll.as_deref(), "ScrollOnCompress")?);
            keep_only(edits, element, "ScrollOnCompress", &["true"])?;
            let title = peek(edits, element, "ShowTitle")?;
            item.bag_tail
                .insert(3, code_of(TRI_STATE, title.as_deref(), "ShowTitle")?);
            keep_only(edits, element, "ShowTitle", &["false"])?;
        }
        "ColumnGroup" => {
            let card = take(edits, element, "ShowTitleInCard")?;
            item.bag_tail.insert(
                2,
                code_of(&[("0", Some("false")), ("2", None)], card.as_deref(), "ShowTitleInCard")?,
            );
            let title = peek(edits, element, "ShowTitle")?;
            item.bag_tail
                .insert(3, code_of(TRI_STATE, title.as_deref(), "ShowTitle")?);
            keep_only(edits, element, "ShowTitle", &["false"])?;
        }
        "CommandBar" => {
            let mode = take(edits, element, "AppearanceMode")?;
            item.bag_tail.insert(
                0,
                code_of(&[("0", None), ("1", Some("CommandBar"))], mode.as_deref(), "AppearanceMode")?,
            );
        }
        "Popup" => {
            let importance = take(edits, element, "Importance")?;
            item.bag_tail.insert(
                0,
                code_of(
                    &[("0", Some("Main")), ("1", None), ("2", Some("Supplementary"))],
                    importance.as_deref(),
                    "Importance",
                )?,
            );
        }
        "PictureDecoration" => {
            if let Some(color) = take(edits, element, "PictureColor")? {
                item.bag_tail
                    .insert(1, color_member(&color, "PictureColor", source)?);
            }
        }
        _ => {}
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn load_field(
    edits: &mut XmlEdits<'_>,
    element: usize,
    tag: &str,
    name: &str,
    item: &mut ItemFacts,
    source: Option<&MetadataSourceContext>,
    items_root: Option<&Path>,
) -> Result<()> {
    let mark = take(edits, element, "MarkRequiredComplete")?;
    item.tail
        .insert(0, code_of(TRI_STATE, mark.as_deref(), "MarkRequiredComplete")?);
    // AutoEditMode reads together with EditMode: `true` over EnterOnInput is
    // 3; without it the member is EditMode's own code (Directly 0, unset 1,
    // EnterOnInput 2). The bars and the chart keep EditMode in record member
    // 26, where the 8.3.27 reader skips it.
    let auto = take(edits, element, "AutoEditMode")?;
    let bar_like = matches!(tag, "ProgressBarField" | "TrackBarField" | "ChartField");
    let edit_mode = if bar_like {
        take(edits, element, "EditMode")?
    } else {
        peek(edits, element, "EditMode")?
    };
    let edit_code = code_of(
        &[("0", Some("Directly")), ("1", None), ("2", Some("EnterOnInput"))],
        edit_mode.as_deref(),
        "EditMode",
    )?;
    let auto_code = match (auto.as_deref(), edit_mode.as_deref()) {
        (Some("true"), Some("EnterOnInput")) => "3".to_owned(),
        (None, _) => edit_code.clone(),
        (Some(auto), mode) => bail!("AutoEditMode {auto} beside EditMode {mode:?} has no 8.5 code"),
    };
    item.tail.insert(2, auto_code);
    if bar_like {
        item.record.insert(26, edit_code);
    }
    let card = take(edits, element, "ShowTitleInCard")?;
    item.tail
        .insert(7, code_of(TRI_STATE, card.as_deref(), "ShowTitleInCard")?);
    let width = take(edits, element, "WidthInCard")?;
    item.tail.insert(
        8,
        code_of(&[("0", None), ("2", Some("Half"))], width.as_deref(), "WidthInCard")?,
    );
    let auto_width = take(edits, element, "AutoWidthInTable")?;
    item.tail.insert(
        12,
        code_of(
            &[
                ("0", None),
                ("1", Some("ByData")),
                ("2", Some("None")),
                ("3", Some("ByDataAndTitle")),
            ],
            auto_width.as_deref(),
            "AutoWidthInTable",
        )?,
    );
    let representation = take(edits, element, "CellHyperlinkRepresentation")?;
    item.tail.insert(
        13,
        code_of(
            &[("0", None), ("1", Some("Show"))],
            representation.as_deref(),
            "CellHyperlinkRepresentation",
        )?,
    );
    let variant = take(edits, element, "CellHyperlinkDisplayVariant")?;
    item.tail.insert(
        14,
        code_of(
            &[("0", None), ("1", Some("Always"))],
            variant.as_deref(),
            "CellHyperlinkDisplayVariant",
        )?,
    );
    match tag {
        "InputField" => {
            if let Some(hint) = take_subtree(edits, element, "DropListHint")? {
                item.bag_tail.insert(0, localized_member(edits, hint)?);
            }
            if let Some(picture) = take_subtree(edits, element, "Picture")? {
                item.bag_tail.insert(
                    1,
                    picture_member(edits, picture, tag, name, source, items_root)?,
                );
            }
            if let Some(title) = take_subtree(edits, element, "ChoiceButtonTitle")? {
                item.bag_tail.insert(2, localized_member(edits, title)?);
            }
            let size = take(edits, element, "TextSize")?;
            item.bag_tail.insert(
                3,
                code_of(&[("0", Some("Enlarged")), ("1", None)], size.as_deref(), "TextSize")?,
            );
        }
        "LabelField" => {
            let copy = take(edits, element, "UseCopy")?;
            item.bag_tail.insert(
                0,
                code_of(&[("1", Some("true")), ("2", None)], copy.as_deref(), "UseCopy")?,
            );
        }
        "CheckBoxField" => {
            // 8.5 has no `Auto` check box: an unset type is 8.3.27's `Auto`
            // unless the box has three states.
            if peek(edits, element, "CheckBoxType")?.is_none()
                && peek(edits, element, "ThreeState")?.as_deref() != Some("true")
            {
                put(edits, element, "CheckBoxType", "Auto")?;
            }
        }
        "RadioButtonField" => {
            let orientation = take(edits, element, "Orientation")?;
            item.bag_tail.insert(
                1,
                code_of(
                    &[("1", None), ("2", Some("HorizontalIfPossible"))],
                    orientation.as_deref(),
                    "Orientation",
                )?,
            );
            let stretch = take(edits, element, "HorizontalStretch")?;
            item.bag_tail.insert(
                2,
                code_of(&[("1", Some("true")), ("2", None)], stretch.as_deref(), "HorizontalStretch")?,
            );
        }
        "PictureField" => {
            if let Some(color) = take(edits, element, "PictureColor")? {
                item.bag_tail
                    .insert(1, color_member(&color, "PictureColor", source)?);
            }
        }
        _ => {}
    }
    Ok(())
}

fn load_table(edits: &mut XmlEdits<'_>, element: usize, item: &mut ItemFacts) -> Result<()> {
    // The line and alternation flags: 8.5 reads the `...BWA` members and keeps
    // the 8.3.27 slot true only when the member says true.
    for (index, bwa, old, old_when_not_true) in [
        (0, "HorizontalLinesBWA", "HorizontalLines", Some("false")),
        (1, "VerticalLinesBWA", "VerticalLines", Some("false")),
        (10, "UseAlternationRowColorBWA", "UseAlternationRowColor", None),
    ] {
        let value = take(edits, element, bwa)?;
        item.tail
            .insert(index, code_of(TRI_STATE, value.as_deref(), bwa)?);
        if let Some(stale) = take(edits, element, old)? {
            bail!("<Table> writes the 8.3.27 <{old}>{stale} under 2.21");
        }
        match (value.as_deref(), old_when_not_true) {
            (Some("true"), Some(_)) => {}
            (Some("true"), None) => put(edits, element, old, "true")?,
            (_, Some(spelling)) => put(edits, element, old, spelling)?,
            (_, None) => {}
        }
    }
    let activation = take(edits, element, "InitialRowActivation")?;
    item.tail.insert(
        6,
        code_of(
            &[("0", None), ("1", Some("Activate")), ("2", Some("NoActivate"))],
            activation.as_deref(),
            "InitialRowActivation",
        )?,
    );
    let actions = take(edits, element, "RowActionsShowType")?;
    item.tail.insert(
        9,
        code_of(
            &[("0", None), ("1", Some("DontShow")), ("2", Some("ShowOnHover"))],
            actions.as_deref(),
            "RowActionsShowType",
        )?,
    );
    // Member 11 says whether the table sets its command bar visibility, 12
    // which: false 0, true 1, auto 2 -- or, unset, the bar location's own
    // reading (None 0, Top/Bottom 1, absent 2).
    let bar = take(edits, element, "ShowCommandBar")?;
    let location = peek(edits, element, "CommandBarLocation")?;
    let (set, which) = match bar.as_deref() {
        Some("false") => ("1", "0"),
        Some("true") => ("1", "1"),
        Some("auto") => ("1", "2"),
        Some(other) => bail!("<Table> ShowCommandBar {other} has no 8.5 code"),
        None => (
            "0",
            match location.as_deref() {
                Some("None") => "0",
                Some("Top" | "Bottom") => "1",
                None => "2",
                Some(other) => bail!("<Table> CommandBarLocation {other} has no 8.5 code"),
            },
        ),
    };
    item.tail.insert(11, set.to_owned());
    item.tail.insert(12, which.to_owned());
    let selection = peek(edits, element, "RowSelectionMode")?;
    item.tail.insert(
        17,
        code_of(
            &[("0", Some("Cell")), ("1", Some("Row")), ("2", None)],
            selection.as_deref(),
            "RowSelectionMode",
        )?,
    );
    keep_only(edits, element, "RowSelectionMode", &["Row"])?;
    let card = take(edits, element, "AutoMaxCardHeight")?;
    item.tail.insert(
        18,
        code_of(&[("0", Some("false")), ("1", None)], card.as_deref(), "AutoMaxCardHeight")?,
    );
    let links = take(edits, element, "CellHyperlinksRepresentation")?;
    item.tail.insert(
        23,
        code_of(
            &[("0", None), ("3", Some("DontShow"))],
            links.as_deref(),
            "CellHyperlinksRepresentation",
        )?,
    );
    match take(edits, element, "ComplexSettingsViewMode")?.as_deref() {
        None => {}
        Some("Show") => item.complex_settings_view_mode = true,
        Some(other) => bail!("<Table> ComplexSettingsViewMode {other} has no 8.5 code"),
    }
    Ok(())
}

fn load_usual_group(edits: &mut XmlEdits<'_>, element: usize, item: &mut ItemFacts) -> Result<()> {
    let card = take(edits, element, "ShowAsCard")?;
    item.bag_tail.insert(
        0,
        code_of(&[("0", None), ("1", Some("true"))], card.as_deref(), "ShowAsCard")?,
    );
    let link = take(edits, element, "Hyperlink")?;
    item.bag_tail.insert(
        2,
        code_of(&[("0", None), ("1", Some("true"))], link.as_deref(), "Hyperlink")?,
    );
    if let Some(events) = take_subtree(edits, element, "Events")? {
        let mut handlers = Vec::new();
        for &event in &edits.elements[events].children {
            let open = &edits.xml[edits.elements[event].open_start..edits.elements[event].line_end];
            let name = open
                .find(" name=\"")
                .and_then(|at| open[at + 7..].find('"').map(|end| &open[at + 7..at + 7 + end]))
                .ok_or_else(|| anyhow!("a usual group event has no name"))?;
            if name != "Click" {
                bail!("usual group event {name} has no 8.5 identifier");
            }
            handlers.push(unescape_xml(inner_text(edits, event)));
        }
        let mut record = format!("{{{}", handlers.len());
        for handler in &handlers {
            record.push_str(&format!(",{GROUP_CLICK_EVENT_UUID},{}", quote_1c(handler)));
        }
        record.push_str(",1,0");
        for _ in &handlers {
            record.push_str(&format!(",{GROUP_CLICK_EVENT_UUID},0,1"));
        }
        record.push('}');
        item.bag_tail.insert(4, record);
    }
    // WeakSeparation and an unset representation both leave the 8.3.27 slot
    // at its default; the others keep their 8.3.27 spelling.
    let representation = peek(edits, element, "Representation")?;
    item.bag_tail.insert(
        6,
        code_of(
            &[
                ("0", Some("None")),
                ("1", Some("StrongSeparation")),
                ("2", Some("WeakSeparation")),
                ("3", Some("NormalSeparation")),
                ("4", None),
            ],
            representation.as_deref(),
            "Representation",
        )?,
    );
    keep_only(
        edits,
        element,
        "Representation",
        &["None", "StrongSeparation", "NormalSeparation"],
    )?;
    // A usual group's 8.3.27 default is HorizontalIfPossible.
    let group = peek(edits, element, "Group")?;
    let group_code = code_of(GROUPING, group.as_deref(), "Group")?;
    // Unset or AutoScreenTypeSensitive: 8.5 keeps the 8.3.27 group member of
    // the bag at 1 (858 of 858 BSP usual groups).
    if matches!(group_code.as_str(), "4" | "5") {
        item.bag.insert(22, "1".to_owned());
    }
    item.bag_tail.insert(7, group_code);
    keep_only(
        edits,
        element,
        "Group",
        &["Vertical", "Horizontal", "AlwaysHorizontal"],
    )?;
    let scroll = take(edits, element, "ScrollOnCompress")?;
    item.bag_tail.insert(
        8,
        if scroll.as_deref() == Some("true") { "1" } else { "0" }.to_owned(),
    );
    item.bag_tail
        .insert(9, code_of(TRI_STATE, scroll.as_deref(), "ScrollOnCompress")?);
    let title = peek(edits, element, "ShowTitle")?;
    item.bag_tail
        .insert(12, code_of(TRI_STATE, title.as_deref(), "ShowTitle")?);
    keep_only(edits, element, "ShowTitle", &["false"])?;
    // 8.3.27 spelled the button representation `Picture`.
    match peek(edits, element, "ControlRepresentation")?.as_deref() {
        None => {}
        Some("Button") => put(edits, element, "ControlRepresentation", "Picture")?,
        Some(other) => bail!("ControlRepresentation {other} has no 8.3.27 spelling"),
    }
    Ok(())
}

/// The pictures 8.5 appends to choice-list values, in document order.
fn load_choice_value_pictures(
    edits: &mut XmlEdits<'_>,
    facts: &mut V85FormLoadFacts,
    source: Option<&MetadataSourceContext>,
    items_root: Option<&Path>,
) -> Result<()> {
    let values = edits
        .elements
        .iter()
        .enumerate()
        .filter(|(_, element)| {
            element.tag == "xr:Value"
                && edits.xml[element.open_start..element.line_end]
                    .split('>')
                    .next()
                    .is_some_and(|open| open.contains("xsi:type=\"FormChoiceListDesTimeValue\""))
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    for value in values {
        let picture = match take_subtree(edits, value, "Picture")? {
            Some(picture) => {
                picture_member(edits, picture, "ChoiceList", "", source, items_root)?
            }
            None => EMPTY_PICTURE.to_owned(),
        };
        facts.choice_value_pictures.push(picture);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The body side.

/// Parses a brace tuple keeping every leaf as written, save the whitespace
/// around it -- except a base64 leaf, which keeps its own trailing line breaks.
fn parse_raw(text: &str) -> Result<Node> {
    let start = text
        .find('{')
        .ok_or_else(|| anyhow!("form body has no opening brace"))?;
    let (node, end) = parse_raw_list(text, start)?;
    if !text[end..].trim().is_empty() {
        bail!("form body has text after its closing brace");
    }
    Ok(node)
}

fn raw_leaf(text: &str) -> String {
    let text = text.trim_start();
    if text.starts_with("#base64:") {
        text.to_owned()
    } else {
        text.trim_end().to_owned()
    }
}

fn parse_raw_list(text: &str, start: usize) -> Result<(Node, usize)> {
    let bytes = text.as_bytes();
    let mut items = Vec::new();
    let mut index = start + 1;
    let mut leaf_start = index;
    let mut pending_leaf = true;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => {
                index += 1;
                loop {
                    match bytes.get(index) {
                        Some(b'"') if bytes.get(index + 1) == Some(&b'"') => index += 2,
                        Some(b'"') => {
                            index += 1;
                            break;
                        }
                        Some(_) => index += 1,
                        None => bail!("form body has an unterminated string"),
                    }
                }
            }
            b'{' => {
                let (node, end) = parse_raw_list(text, index)?;
                items.push(node);
                pending_leaf = false;
                index = end;
                leaf_start = index;
            }
            b',' => {
                if pending_leaf {
                    items.push(Node::Leaf(raw_leaf(&text[leaf_start..index])));
                } else if !text[leaf_start..index].trim().is_empty() {
                    bail!("form body has text between a tuple and its separator");
                }
                index += 1;
                leaf_start = index;
                pending_leaf = true;
            }
            b'}' => {
                if pending_leaf {
                    let leaf = raw_leaf(&text[leaf_start..index]);
                    if !leaf.is_empty() || !items.is_empty() {
                        items.push(Node::Leaf(leaf));
                    }
                } else if !text[leaf_start..index].trim().is_empty() {
                    bail!("form body has text between a tuple and its closing brace");
                }
                return Ok((Node::List(items), index + 1));
            }
            _ => index += 1,
        }
    }
    bail!("form body has an unterminated tuple")
}

/// Lays a tuple out as the platform writes a form body: a line break before
/// every nested tuple, and before the closing brace of a tuple whose last
/// member is a tuple (1 111 of the 1 120 BSP 8.5 bodies byte for byte with
/// these two rules alone; the rest differ in base64 line breaks only).
fn emit_1c(node: &Node, out: &mut String) {
    match node {
        Node::Leaf(text) => out.push_str(text),
        Node::List(items) => {
            out.push('{');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                if matches!(item, Node::List(_)) {
                    out.push_str("\r\n");
                }
                emit_1c(item, out);
            }
            if matches!(items.last(), Some(Node::List(_))) {
                out.push_str("\r\n");
            }
            out.push('}');
        }
    }
}

fn leaf(node: &Node) -> Option<&str> {
    match node {
        Node::Leaf(text) => Some(text),
        Node::List(_) => None,
    }
}

fn is_int(text: &str) -> bool {
    let digits = text.strip_prefix('-').unwrap_or(text);
    !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
}

fn is_uuid(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => *byte == b'-',
            _ => byte.is_ascii_hexdigit(),
        })
}

/// `{3,<space>,{<payload>}}`, an 8.3.27 colour.
fn is_v83_color(members: &[Node]) -> bool {
    if members.len() != 3 || leaf(&members[0]) != Some("3") {
        return false;
    }
    if !matches!(leaf(&members[1]), Some("0" | "1" | "2" | "3" | "4")) {
        return false;
    }
    match &members[2] {
        Node::List(payload) => match payload.as_slice() {
            [Node::Leaf(value)] => is_int(value),
            [Node::Leaf(zero), Node::Leaf(uuid)] => zero == "0" && is_uuid(uuid),
            _ => false,
        },
        Node::Leaf(_) => false,
    }
}

/// `{7,<kind>,<mask>,...,1,<scale>}`, an 8.3.27 font (the 8.5 `{8,...}` is
/// the same tuple renumbered).
fn is_v83_font(members: &[Node]) -> bool {
    if members.len() < 5 || leaf(&members[0]) != Some("7") {
        return false;
    }
    let (Some(kind), Some(mask)) = (leaf(&members[1]), leaf(&members[2])) else {
        return false;
    };
    if !is_int(mask) || leaf(&members[members.len() - 2]) != Some("1") {
        return false;
    }
    if !leaf(&members[members.len() - 1]).is_some_and(is_int) {
        return false;
    }
    match kind {
        "3" => members.len() == 5 && mask == "0",
        "1" | "2" => matches!(members[3], Node::List(_)),
        _ => false,
    }
}

/// `{7,0,<mask>,...,"<face>",1,<scale>}` of 19 members, an 8.3.27 absolute
/// font; 8.5 appends one `0`.
fn is_v83_absolute_font(members: &[Node]) -> bool {
    members.len() == 19
        && leaf(&members[0]) == Some("7")
        && leaf(&members[1]) == Some("0")
        && leaf(&members[2]).is_some_and(is_int)
        && leaf(&members[16]).is_some_and(|face| face.starts_with('"'))
        && leaf(&members[17]) == Some("1")
        && leaf(&members[18]).is_some_and(is_int)
}

/// Spells every colour and font of a tuple the 8.5 way. No 8.3.27-shaped
/// colour occurs in any of the 9 935 rows of the BSP 8.5 clone
/// (`F:/ibcmd/lab/v85/tools/primcensus.py`), so every one the 8.3.27 writer
/// produces is one 8.5 re-spells.
fn up_convert_primitives(node: &mut Node) {
    let Node::List(members) = node else {
        return;
    };
    for member in members.iter_mut() {
        up_convert_primitives(member);
    }
    if is_v83_color(members) {
        let space = members[1].clone();
        members[0] = Node::Leaf("4".to_owned());
        members.push(space);
    } else if is_v83_font(members) {
        members[0] = Node::Leaf("8".to_owned());
    } else if is_v83_absolute_font(members) {
        members[0] = Node::Leaf("8".to_owned());
        members.push(Node::Leaf("0".to_owned()));
    }
}

/// Rewrites, in place, every 8.3.27 colour and font tuple of a stored text
/// into the 8.5 spelling, leaving every other byte as written: the inverse of
/// `form_v85::rewrite_v85_primitives_in_place`. For a body 8.5 stores whole
/// (a spreadsheet template), where no 8.3.27-shaped tuple survives
/// (`F:/ibcmd/lab/v85/tools/primcensus.py`: none in the 9 935 BSP 8.5 rows).
pub(crate) fn up_convert_v85_primitives_in_place(text: &str) -> String {
    const MAX_TUPLE: usize = 512;
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len() + text.len() / 16);
    let mut cursor = 0;
    let mut index = 0;
    let mut in_string = false;
    while index < bytes.len() {
        let byte = bytes[index];
        if in_string {
            if byte == b'"' {
                if bytes.get(index + 1) == Some(&b'"') {
                    index += 2;
                    continue;
                }
                in_string = false;
            }
            index += 1;
            continue;
        }
        if byte == b'"' {
            in_string = true;
            index += 1;
            continue;
        }
        if byte == b'{'
            && matches!(bytes.get(index + 1), Some(b'3' | b'7'))
            && bytes.get(index + 2) == Some(&b',')
            && let Some(end) = tuple_end(bytes, index, MAX_TUPLE)
            && let Ok(Node::List(members)) = parse_raw(&text[index..end])
        {
            let mut node = Node::List(members);
            let converted = match &mut node {
                Node::List(members) if is_v83_color(members) => {
                    let space = members[1].clone();
                    members[0] = Node::Leaf("4".to_owned());
                    members.push(space);
                    true
                }
                Node::List(members) if is_v83_font(members) => {
                    members[0] = Node::Leaf("8".to_owned());
                    true
                }
                Node::List(members) if is_v83_absolute_font(members) => {
                    members[0] = Node::Leaf("8".to_owned());
                    members.push(Node::Leaf("0".to_owned()));
                    true
                }
                _ => false,
            };
            if converted {
                out.push_str(&text[cursor..index]);
                emit_1c(&node, &mut out);
                cursor = end;
                index = end;
                continue;
            }
        }
        index += 1;
    }
    out.push_str(&text[cursor..]);
    out
}

/// The chart records of a spreadsheet template the way 8.5 stores them: the
/// 8.3.27 `{74,...}` record (`{{11},{74,...}}`, its length set by the series
/// and points) becomes `{75,...}` with eight automatic colours appended
/// (8.5.1.1150 BSP, both Gantt chart templates; the exporter reads nothing
/// else in those eight).
pub(crate) fn up_convert_v85_chart_records(text: &str) -> Result<String> {
    const RECORD_LIMIT: usize = 16 * 1024 * 1024;
    let mut out = String::with_capacity(text.len() + 256);
    let mut cursor = 0;
    let mut search = 0;
    while let Some(relative) = text[search..].find("{11},") {
        let at = search + relative + "{11},".len();
        search = at;
        let start = at + (text[at..].len() - text[at..].trim_start().len());
        if !text[start..].starts_with("{74,") {
            continue;
        }
        let end = tuple_end(text.as_bytes(), start, RECORD_LIMIT)
            .ok_or_else(|| anyhow!("a chart record does not close"))?;
        let Node::List(mut members) = parse_raw(&text[start..end])? else {
            bail!("a chart record is not a tuple");
        };
        // The record's fixed part alone is 97 members (`moxel.rs`).
        if members.len() < 97 {
            bail!("a chart record `{{74,...}}` carries {} members, fewer than 97", members.len());
        }
        members[0] = Node::Leaf("75".to_owned());
        for _ in 0..8 {
            members.push(parse_raw("{4,4,{0},4}")?);
        }
        out.push_str(&text[cursor..start]);
        emit_1c(&Node::List(members), &mut out);
        cursor = end;
        search = end;
    }
    out.push_str(&text[cursor..]);
    Ok(out)
}

/// The end of the brace tuple that opens at `start`, if it closes within
/// `limit` bytes.
fn tuple_end(bytes: &[u8], start: usize, limit: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut index = start;
    let end = bytes.len().min(start.saturating_add(limit));
    while index < end {
        match bytes[index] {
            b'"' => {
                index += 1;
                while index < end {
                    if bytes[index] == b'"' {
                        if bytes.get(index + 1) == Some(&b'"') {
                            index += 2;
                            continue;
                        }
                        break;
                    }
                    index += 1;
                }
            }
            b'{' => depth += 1,
            b'}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(index + 1);
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}

fn node_of(text: &str) -> Result<Node> {
    let mut node = if text.trim_start().starts_with('{') {
        parse_raw(text)?
    } else {
        Node::Leaf(text.to_owned())
    };
    up_convert_primitives(&mut node);
    Ok(node)
}

/// A group record 8.5 appends to a field or table: the panel the platform
/// generates for it, named after its owner (kind 10 selected-rows actions,
/// 11 row actions).
fn generated_panel(name: &str, kind: u8) -> String {
    format!(
        "{{22,{{0}},1,0,0,{kind},{name},{{1,0}},{{1,0}},0,1,0,0,0,2,2,{UNSET_COLOR},{{7,3,0,1,100}},{{0,0,0}},1,{{0,1,0,1}},0,1,0,0,0,3,3,0}}",
        name = quote_1c(name),
    )
}

/// The search string (`kind` 0) or search control (`kind` 2) addition 8.5
/// appends to a table's selected-rows panel.
fn generated_search_panel(table: &str, table_id: &str, kind: u8) -> String {
    let (suffix, payload, own) = match kind {
        0 => (
            "СтрокаПоиска",
            format!("{{1,0,2,{UNSET_COLOR},{UNSET_COLOR},{UNSET_COLOR},{{7,3,0,1,100}},{{0,1,0}},1,0,0}}"),
            "СтрокаПоиска",
        ),
        _ => (
            "УправлениеПоиском",
            format!("{{1,0,{UNSET_COLOR},{UNSET_COLOR},{UNSET_COLOR},{{7,3,0,1,100}},{{0,1,0}},1,0,0,2}}"),
            "УправлениеПоиском",
        ),
    };
    let panel = format!("{table}ПанельДействийВыделенныхСтрок{suffix}");
    format!(
        "{{6,{{0}},0,0,0,{kind},{panel_name},{{1,0}},{{1,0}},1,1,0,1,{payload},1,\
{{22,{{0}},0,0,0,8,{menu},{{1,0}},{{1,0}},0,1,0,0,0,2,2,{UNSET_COLOR},{{7,3,0,1,100}},{{0,0,0}},1,{{1,1}},0,1,0,0,0,3,3,0}},1,\
{{12,{{0}},0,0,0,0,{tooltip},{{1,0}},{{1,0}},1,0,0,2,2,{UNSET_COLOR},{{7,3,0,1,100}},{{0,0,0}},1,\
{{5,0,0,3,0,{{0,1,0}},{UNSET_COLOR},{UNSET_COLOR},{DEFAULT_BORDER}}},0,1,2,{{1,{{1,0}},0}},0,0,1,0,0,1,0,3,3,0,0}},\
2,{{{table_id},{kind}}},0,3,3,0,{own_name}}}",
        panel_name = quote_1c(&panel),
        menu = quote_1c(&format!("{panel}КонтекстноеМеню")),
        tooltip = quote_1c(&format!("{panel}РасширеннаяПодсказка")),
        own_name = quote_1c(&format!("{table}{own}")),
    )
}

/// The inverse of `form_v85::bag_revision`: (8.3.27 owner revision, item
/// kind, 8.3.27 bag revision, 8.3.27 bag length) -> (8.5 bag revision, the
/// defaults of the members 8.5 appends).
fn up_bag(owner: &str, kind: &str, revision: &str, len: usize) -> Option<(&'static str, Vec<&'static str>)> {
    Some(match (owner, kind, revision, len) {
        ("22", "0", "1", 3) => ("2", vec!["0"]),
        ("22", "1", "7", 9) => ("8", vec!["1"]),
        ("22", "2", "2", 12) => ("5", vec!["1", "2", "2", "2"]),
        ("22", "3", "4", 6) => ("4", vec![]),
        ("22", "4", "18", 20) => ("22", vec!["4", "2", "0", "2"]),
        ("22", "5", "29", 29) => (
            "38",
            vec!["0", "0", "0", EMPTY_PICTURE, "{0,1,0}", "0", "4", "4", "0", "2", "0", "0", "2"],
        ),
        ("22", "6", "2", 4) => ("2", vec![]),
        ("22", "8", "1", 2) => ("1", vec![]),
        ("22", "9", "0", 3) => ("1", vec!["0"]),
        ("37", "1", "11", 20) => ("12", vec!["2"]),
        ("37", "2", "36", 66) => ("38", vec!["{1,0}", EMPTY_PICTURE, "{1,0}", "1", "0"]),
        ("37", "3", "11", 13) => ("13", vec!["0", "2"]),
        ("37", "4", "10", 24) => ("12", vec!["0", UNSET_COLOR]),
        ("37", "5", "8", 12) => ("11", vec!["0", "1", "2"]),
        ("37", "6", "13", 32) => ("15", vec!["0", "0"]),
        ("37", "7", "5", 16) => ("5", vec![]),
        ("37", "8", "6", 24) => ("6", vec![]),
        ("37", "9", "4", 16) => ("4", vec![]),
        ("37", "10", "2", 18) => ("2", vec![]),
        ("37", "11", "1", 11) => ("1", vec![]),
        ("37", "14", "3", 14) => ("3", vec![]),
        ("37", "15", "3", 13) => ("4", vec![DEFAULT_BORDER]),
        ("37", "17", "1", 16) => ("1", vec![]),
        // Unmeasured under 8.5 (see `form_v85::bag_revision`): kept as is.
        ("37", "12", "3", 16) => ("3", vec![]),
        ("37", "20", "1", 14) => ("1", vec![]),
        ("12", "0", "5", 9) => ("5", vec![]),
        ("12", "1", "4", 13) => ("6", vec!["0", UNSET_COLOR]),
        _ => return None,
    })
}

struct UpConversion<'f> {
    facts: &'f V85FormLoadFacts,
    choice_values: usize,
}

/// Bumps one form body written in the 8.3.27 layout to what 8.5 stores.
pub(crate) fn up_convert_v83_form_body(body: &str, facts: &V85FormLoadFacts) -> Result<String> {
    let text = body.trim_start_matches('\u{feff}');
    let mut container = parse_raw(text)?;
    let mut state = UpConversion {
        facts,
        choice_values: 0,
    };
    let Node::List(members) = &mut container else {
        bail!("form body is not a tuple");
    };
    // The records first (items, commands, values), then the root trailer.
    for member in members.iter_mut() {
        state.convert(member)?;
    }
    let root = members
        .get_mut(1)
        .ok_or_else(|| anyhow!("form body has no layout"))?;
    up_convert_root(root, facts)?;
    if state.choice_values != facts.choice_value_pictures.len()
        && facts
            .choice_value_pictures
            .iter()
            .any(|picture| picture != EMPTY_PICTURE)
    {
        bail!(
            "the form XML names {} choice-list values with pictures and the body holds {}",
            facts.choice_value_pictures.len(),
            state.choice_values
        );
    }
    up_convert_primitives(&mut container);
    up_convert_leaves(&mut container);
    let mut out = String::with_capacity(text.len() + text.len() / 4);
    emit_1c(&container, &mut out);
    Ok(out)
}

/// Spells the leaves of a form body the way 8.5 stores them (all 1 120 BSP
/// 8.5.1.1150 bodies; the 8.3.27 writer spells them the 8.3.27 way):
///
/// * a line break inside a quoted string is CR LF -- not one lone LF in the
///   strings of those bodies, where the 8.3.27 writer keeps the LF the XML
///   reader gives it;
/// * a base64 payload runs 64 characters a line, each full line followed by
///   CR CR LF (1 523 of 1 523 payloads, the nine whose length is a multiple
///   of 64 included), where 8.3.27 writes CR LF or one line;
/// * an embedded XML document declares the palette namespace wherever it
///   declares the style one, as the exporter's 2.21 files do: the settings
///   document of the attributes section, `<Settings ... xmlns:dcscor=...
///   xmlns:pal=... xmlns:style=...>` in all 1 120 bodies, the only embedded
///   documents that declare the style namespace.
fn up_convert_leaves(node: &mut Node) {
    match node {
        Node::List(members) => {
            for member in members.iter_mut() {
                up_convert_leaves(member);
            }
        }
        Node::Leaf(text) => {
            if text.starts_with('"') {
                if text.contains('\n') {
                    *text = crlf_line_breaks(text);
                }
            } else if let Some(payload) = text.strip_prefix("#base64:") {
                *text = v85_base64_leaf(payload);
            }
        }
    }
}

/// Every line feed not preceded by a carriage return becomes CR LF.
fn crlf_line_breaks(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + text.len() / 32);
    let mut previous = '\0';
    for character in text.chars() {
        if character == '\n' && previous != '\r' {
            out.push('\r');
        }
        out.push(character);
        previous = character;
    }
    out
}

/// A base64 leaf as 8.5 stores it; see [`up_convert_leaves`]. A payload that
/// does not decode is only laid out; one that is not base64 text is kept.
fn v85_base64_leaf(payload: &str) -> String {
    let compact: String = payload.chars().filter(|c| !c.is_ascii_whitespace()).collect();
    if !compact.is_ascii() {
        return format!("#base64:{payload}");
    }
    let declared = crate::module_blob::decode_base64_mime(&compact).and_then(|bytes| {
        let document = bytes.strip_prefix(b"\xEF\xBB\xBF".as_slice()).unwrap_or(&bytes);
        if !document.starts_with(b"<?xml") {
            return None;
        }
        let declared = super::declare_palette_namespace_beside_style(bytes.clone());
        (declared != bytes).then_some(declared)
    });
    let encoded = match declared {
        Some(bytes) => crate::module_blob::encode_base64(&bytes),
        None => compact,
    };
    let mut out = String::with_capacity(8 + encoded.len() + encoded.len() / 64 * 3);
    out.push_str("#base64:");
    for chunk in encoded.as_bytes().chunks(64) {
        out.push_str(std::str::from_utf8(chunk).expect("base64 is ASCII"));
        if chunk.len() == 64 {
            out.push_str("\r\r\n");
        }
    }
    out
}

fn up_convert_root(root: &mut Node, facts: &V85FormLoadFacts) -> Result<()> {
    let Node::List(members) = root else {
        bail!("form layout is not a tuple");
    };
    if members.first().and_then(leaf) != Some("50") {
        bail!("the 8.3.27 form layout does not declare revision 50");
    }
    members[0] = Node::Leaf("59".to_owned());
    if let Some(state) = &facts.report_state {
        add_report_state(members, state)?;
    }
    let len = members.len();
    if facts.root_group_horizontal {
        let group_at = len
            .checked_sub(10)
            .ok_or_else(|| anyhow!("form root is too short for its group"))?;
        for at in [11, group_at] {
            match members.get_mut(at) {
                Some(Node::Leaf(value)) if value == "0" => *value = "1".to_owned(),
                _ => bail!("the 8.3.27 form root keeps no group member {at} to set"),
            }
        }
    }
    // The trailer's own `{50,...}` tuple sits one member before the end.
    match members.get_mut(len.checked_sub(2).ok_or_else(|| anyhow!("form root is too short"))?) {
        Some(Node::List(tuple)) if tuple.first().and_then(leaf) == Some("50") => {
            tuple[0] = Node::Leaf("59".to_owned());
        }
        _ => bail!("the 8.3.27 form root carries no `{{50,...}}` trailer tuple"),
    }
    if let Some(scale) = &facts.root_scale {
        let at = len
            .checked_sub(8)
            .ok_or_else(|| anyhow!("form root is too short for its scale"))?;
        if leaf(&members[at]) != Some("100") {
            bail!("the 8.3.27 form root keeps no scale where 8.5 reads it");
        }
        members[at] = Node::Leaf(scale.clone());
    }
    let defaults = [
        "{1,0}",
        EMPTY_PICTURE,
        "0",
        "0",
        "3",
        "0",
        "2",
        "4",
        "0",
        "0",
        "2",
        "0",
    ];
    for (index, default) in defaults.iter().enumerate() {
        let value = facts.root_tail.get(&index).map_or(*default, String::as_str);
        members.push(node_of(value)?);
    }
    Ok(())
}

impl UpConversion<'_> {
    fn convert(&mut self, node: &mut Node) -> Result<()> {
        let Node::List(members) = node else {
            return Ok(());
        };
        for member in members.iter_mut() {
            self.convert(member)?;
        }
        if let Some(id) = item_identity(members) {
            return self.convert_item(members, &id);
        }
        if let Some(id) = command_identity(members) {
            if leaf(&members[0]) != Some("9") || members.len() != 19 {
                bail!(
                    "form command {id} is not the 8.3.27 `{{9,...}}` of 19 members ({} members)",
                    members.len()
                );
            }
            members[0] = Node::Leaf("11".to_owned());
            let tail = self.facts.commands.get(&id);
            for index in 0..2 {
                let value = tail.and_then(|tail| tail.get(&index)).map_or("0", String::as_str);
                members.push(node_of(value)?);
            }
            return Ok(());
        }
        // A choice-list value: `{"#",<value class>,{0,...}}` with six members,
        // 8.5 `{1,...}` with its picture appended.
        if members.len() == 3
            && leaf(&members[0]) == Some("\"#\"")
            && leaf(&members[1]) == Some(FORM_CHOICE_LIST_VALUE_UUID)
            && let Node::List(value) = &mut members[2]
        {
            if value.first().and_then(leaf) != Some("0") || value.len() != 6 {
                bail!("a choice-list value is not the 8.3.27 `{{0,...}}` of six members");
            }
            value[0] = Node::Leaf("1".to_owned());
            let picture = self
                .facts
                .choice_value_pictures
                .get(self.choice_values)
                .map_or(EMPTY_PICTURE, String::as_str);
            value.push(node_of(picture)?);
            self.choice_values += 1;
        }
        Ok(())
    }

    fn convert_item(&mut self, members: &mut Vec<Node>, id: &str) -> Result<()> {
        let revision = leaf(&members[0]).unwrap_or_default().to_owned();
        let facts = self.facts.items.get(id);
        let (v85, tail_defaults): (&str, Vec<String>) = match revision.as_str() {
            "37" => {
                let name = facts
                    .map(|facts| facts.name.clone())
                    .ok_or_else(|| anyhow!("form field {id} is not in the XML"))?;
                let panel = generated_panel(&format!("{name}ПанельДействийВыделенныхСтрок"), 10);
                (
                    "48",
                    [
                        "2", "0", "1", "1", &panel, "0", "0", "2", "0", "0", "1", "2", "0", "0", "0",
                    ]
                    .iter()
                    .map(|value| (*value).to_owned())
                    .collect(),
                )
            }
            "31" => (
                "34",
                ["0", EMPTY_PICTURE, "0", "0", "0", "1", "\"\""]
                    .iter()
                    .map(|value| (*value).to_owned())
                    .collect(),
            ),
            "55" => {
                let name = facts
                    .map(|facts| facts.name.clone())
                    .ok_or_else(|| anyhow!("form table {id} is not in the XML"))?;
                let selected = generated_panel(&format!("{name}ПанельДействийВыделенныхСтрок"), 10);
                let rows = generated_panel(&format!("{name}ДействияСтроки"), 11);
                let search = generated_search_panel(&name, id, 0);
                let control = generated_search_panel(&name, id, 2);
                (
                    "73",
                    [
                        "2", "2", "0", "0", "\"\"", "\"\"", "0", "1", &selected, "0", "2", "0", "2",
                        "1", &rows, "2", "0", "2", "1", "0", "0", "0", "0", "0", "1", &search, "1",
                        &control,
                    ]
                    .iter()
                    .map(|value| (*value).to_owned())
                    .collect(),
                )
            }
            "22" => ("22", Vec::new()),
            "12" => ("12", Vec::new()),
            "5" => ("6", vec!["\"\"".to_owned()]),
            other => bail!("form item {id} declares 8.3.27 revision {other}, which 8.5 does not name"),
        };
        let prefix = usize::from(matches!(members.get(5), Some(Node::List(_))));
        if revision == "55" && facts.is_some_and(|facts| facts.complex_settings_view_mode) {
            set_complex_settings_view_mode(members, prefix)
                .with_context(|| format!("form table {id}"))?;
        }
        if let Some(facts) = facts {
            for (index, value) in &facts.record {
                let at = index + prefix;
                if at >= members.len() {
                    bail!("form item {id} has no record member {index}");
                }
                members[at] = node_of(value)?;
            }
        }
        // The property bag, for the records that carry one.
        let bag_slot = match revision.as_str() {
            "22" => Some(20),
            "37" => Some(39),
            "12" => Some(18),
            _ => None,
        };
        if let Some(base) = bag_slot {
            let kind = members
                .get(5 + prefix)
                .and_then(leaf)
                .ok_or_else(|| anyhow!("form item {id} carries no kind code"))?
                .to_owned();
            let slot = base + prefix;
            let Some(Node::List(bag)) = members.get_mut(slot) else {
                bail!("form item {id} (kind {kind}) carries no property bag at member {slot}");
            };
            let bag_revision = bag.first().and_then(leaf).unwrap_or_default().to_owned();
            let (bag_v85, bag_defaults) = up_bag(&revision, &kind, &bag_revision, bag.len())
                .ok_or_else(|| {
                    anyhow!(
                        "form item {id} (revision {revision}, kind {kind}) carries a property bag {{{bag_revision},...}} of {} members 8.5 does not name",
                        bag.len()
                    )
                })?;
            if let Some(facts) = facts {
                for (index, value) in &facts.bag {
                    let member = bag.get_mut(*index).ok_or_else(|| {
                        anyhow!("form item {id} has no property bag member {index}")
                    })?;
                    *member = node_of(value)?;
                }
            }
            bag[0] = Node::Leaf(bag_v85.to_owned());
            for (index, default) in bag_defaults.iter().enumerate() {
                let value = facts
                    .and_then(|facts| facts.bag_tail.get(&index))
                    .map_or(*default, String::as_str);
                bag.push(node_of(value)?);
            }
            if let Some(facts) = facts
                && let Some(index) = facts.bag_tail.keys().find(|index| **index >= bag_defaults.len())
            {
                bail!("form item {id} names bag member {index}, which its 8.5 bag does not append");
            }
        } else if let Some(facts) = facts
            && (!facts.bag_tail.is_empty() || !facts.bag.is_empty())
        {
            bail!("form item {id} ({}) names bag members and has no bag", facts.tag);
        }
        members[0] = Node::Leaf(v85.to_owned());
        for (index, default) in tail_defaults.iter().enumerate() {
            let value = facts
                .and_then(|facts| facts.tail.get(&index))
                .map_or(default.as_str(), String::as_str);
            members.push(node_of(value)?);
        }
        if let Some(facts) = facts
            && let Some(index) = facts.tail.keys().find(|index| **index >= tail_defaults.len())
        {
            bail!("form item {id} ({}) names record member {index}, which 8.5 does not append", facts.tag);
        }
        Ok(())
    }
}

/// The report state of a report form's root bag (member 18: a count, then
/// that many key/value pairs, keys ascending), as the 8.5.1.1150 BSP forms
/// store it beside the keys the 8.3.27 writer takes from `Form.xml` (5, 6,
/// 20 references, 7, 21, 23, 27, 29): the default report forms 8-11, the
/// report 12, the variant `Основной` (13, 18 and the one-item variant list
/// 17), flags 14 and 16, 15 undefined, 19 empty, 22 the empty uuid, and the
/// empty references 5, 6, 20 and item 0 in 23 where the XML names none. All
/// but 8-12 are one spelling in every form of both corpora that carries the
/// state (ERP УХ: 210 of 212, the other two external-report copies).
fn add_report_state(members: &mut Vec<Node>, state: &ReportState) -> Result<()> {
    const BAG: usize = 18;
    const EMPTY_REFERENCE: &str = "{\"#\",11cfd3e0-86f8-4480-aaa5-dc6a6ccac689,{0,\"\"}}";
    const VARIANT: &str = "{\"S\",\"Основной\"}";
    const VARIANTS: &str = "{\"#\",4772b3b4-f4a3-49c0-a1a5-8cb5961511a3,{6,1e512aab-1b41-4ef6-9375-f0137be9dd91,0,0,\
{1,{1e512aab-1b41-4ef6-9375-f0137be9dd91,{\"Основной\",0,{\"S\",\"Основной\"},{4,0,{0},\"\",-1,-1,0,0,\"\"},0,0,\"\"}}},\
{\"Pattern\"},0,0}}";
    let count: usize = members
        .get(BAG)
        .and_then(leaf)
        .and_then(|count| count.parse().ok())
        .ok_or_else(|| anyhow!("the report form root carries no property bag count at member {BAG}"))?;
    let mut pairs = BTreeMap::new();
    for index in 0..count {
        let key = members
            .get(BAG + 1 + 2 * index)
            .and_then(leaf)
            .and_then(|key| key.parse::<u32>().ok())
            .ok_or_else(|| anyhow!("the report form root bag has no key {index}"))?;
        let value = members
            .get(BAG + 2 + 2 * index)
            .cloned()
            .ok_or_else(|| anyhow!("the report form root bag has no value {index}"))?;
        pairs.insert(key, value);
    }
    if !pairs.contains_key(&7) {
        bail!("the report form root bag holds no report form type (key 7)");
    }
    let urn = |property: u8| format!("{{\"S\",\"urn:form:md:{property}:{}\"}}", state.configuration);
    let defaults = [
        (5, EMPTY_REFERENCE.to_owned()),
        (6, EMPTY_REFERENCE.to_owned()),
        (8, urn(14)),
        (9, urn(13)),
        (10, urn(16)),
        (11, urn(15)),
        (12, format!("{{\"S\",{}}}", quote_1c(&state.report))),
        (13, VARIANT.to_owned()),
        (14, "{\"B\",0}".to_owned()),
        (15, "{\"U\"}".to_owned()),
        (16, "{\"B\",0}".to_owned()),
        (17, VARIANTS.to_owned()),
        (18, VARIANT.to_owned()),
        (19, "{\"S\",\"\"}".to_owned()),
        (20, EMPTY_REFERENCE.to_owned()),
        (22, "{\"S\",\"00000000-0000-0000-0000-000000000000\"}".to_owned()),
        (23, "{\"N\",0}".to_owned()),
    ];
    for (key, value) in defaults {
        if !pairs.contains_key(&key) {
            pairs.insert(key, node_of(&value)?);
        }
    }
    let mut replacement = vec![Node::Leaf(pairs.len().to_string())];
    for (key, value) in pairs {
        replacement.push(Node::Leaf(key.to_string()));
        replacement.push(value);
    }
    members.splice(BAG..BAG + 1 + 2 * count, replacement);
    Ok(())
}

/// Key 21 of a table's keyed property bag (member 54: a count, then that many
/// key/value pairs, keys ascending): `ComplexSettingsViewMode` `Show`. The
/// three 8.5.1.1150 BSP tables that write it hold no empty key 19 beside it.
fn set_complex_settings_view_mode(members: &mut Vec<Node>, prefix: usize) -> Result<()> {
    let at = 54 + prefix;
    let count: usize = members
        .get(at)
        .and_then(leaf)
        .and_then(|count| count.parse().ok())
        .ok_or_else(|| anyhow!("the table record carries no property bag count at member {at}"))?;
    let mut pairs = Vec::with_capacity(count + 1);
    for index in 0..count {
        let key = members
            .get(at + 1 + 2 * index)
            .and_then(leaf)
            .and_then(|key| key.parse::<u32>().ok())
            .ok_or_else(|| anyhow!("the table property bag has no key {index}"))?;
        let value = members
            .get(at + 2 + 2 * index)
            .cloned()
            .ok_or_else(|| anyhow!("the table property bag has no value {index}"))?;
        pairs.push((key, value));
    }
    pairs.retain(|(key, value)| {
        !(*key == 19 && matches!(value, Node::List(items) if items.len() == 2 && leaf(&items[0]) == Some("\"S\"") && leaf(&items[1]) == Some("\"\"")))
    });
    if pairs.iter().any(|(key, _)| *key == 21) {
        bail!("the table property bag already holds key 21");
    }
    pairs.push((21, parse_raw("{\"#\",2eb62aaa-e6c1-48b6-a047-435354d5ae82,0}")?));
    pairs.sort_by_key(|(key, _)| *key);
    let mut replacement = vec![Node::Leaf(pairs.len().to_string())];
    for (key, value) in pairs {
        replacement.push(Node::Leaf(key.to_string()));
        replacement.push(value);
    }
    members.splice(at..at + 1 + 2 * count, replacement);
    Ok(())
}

/// A form item record: `{<revision>,{<id>,<form item class>},...}`.
fn item_identity(members: &[Node]) -> Option<String> {
    if members.len() < 20 || !leaf(&members[0]).is_some_and(is_int) {
        return None;
    }
    match &members[1] {
        Node::List(identity) => match identity.as_slice() {
            [Node::Leaf(id), Node::Leaf(class)] if class == FORM_ITEM_CLASS_UUID && is_int(id) => {
                Some(id.clone())
            }
            _ => None,
        },
        Node::Leaf(_) => None,
    }
}

fn command_identity(members: &[Node]) -> Option<String> {
    if members.len() <= 4 {
        return None;
    }
    match &members[1] {
        Node::List(identity) => match identity.as_slice() {
            [Node::Leaf(id), Node::Leaf(class)] if class == FORM_COMMAND_CLASS_UUID && is_int(id) => {
                Some(id.clone())
            }
            _ => None,
        },
        Node::Leaf(_) => None,
    }
}

/// A 2.21 `Form.xml` through the 8.3.27 native writer, stored as 8.5 stores
/// it.
pub(crate) fn compile_v85_native_form_body(
    form_xml: &[u8],
    module_text: Option<&[u8]>,
    source: Option<&MetadataSourceContext>,
    items_root: Option<&Path>,
) -> Result<String> {
    let xml = std::str::from_utf8(form_xml).context("2.21 Form.xml is not valid UTF-8")?;
    let (xml20, facts) = down_convert_v85_form_xml(xml, source, items_root)?;
    let body = crate::module_blob::compile_native_form_body_v83(
        xml20.as_bytes(),
        module_text,
        source,
        items_root,
    )?;
    up_convert_v83_form_body(&body, &facts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bag_table_inverts_the_exporter_table() {
        let owners = [("22", 0..=9), ("37", 0..=20), ("12", 0..=1)];
        let mut checked = 0;
        for (owner, kinds) in owners {
            for kind in kinds {
                let kind = kind.to_string();
                for revision in 0..=40 {
                    for len in 0..=80 {
                        let revision = revision.to_string();
                        if let Some((v83, appended)) =
                            super::super::form_v85::bag_revision(owner, &kind, &revision, len)
                        {
                            let (v85, defaults) = up_bag(owner, &kind, v83, len - appended)
                                .unwrap_or_else(|| panic!("no inverse for {owner}/{kind}/{revision}/{len}"));
                            assert_eq!((v85, defaults.len()), (revision.as_str(), appended));
                            checked += 1;
                        }
                    }
                }
            }
        }
        assert_eq!(checked, 27);
    }

    #[test]
    fn spells_strings_and_base64_the_85_way() {
        let settings = "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<Settings xmlns=\"http://v8.1c.ru/8.1/data-composition-system/settings\" xmlns:dcscor=\"http://v8.1c.ru/8.1/data-composition-system/core\" xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\" xmlns:sys=\"http://v8.1c.ru/8.1/data/ui/fonts/system\"/>";
        let with_pal = settings.replace(
            " xmlns:style=",
            " xmlns:pal=\"http://v8.1c.ru/8.1/data/ui/colors/palette\" xmlns:style=",
        );
        let body = format!(
            "{{\"a\nb\",\"c\r\nd\",{{#base64:{}}}}}",
            crate::module_blob::encode_base64(settings.as_bytes())
        );
        let mut node = parse_raw(&body).unwrap();
        up_convert_leaves(&mut node);
        let Node::List(members) = &node else { panic!() };
        assert_eq!(leaf(&members[0]), Some("\"a\r\nb\""));
        assert_eq!(leaf(&members[1]), Some("\"c\r\nd\""));
        let Node::List(document) = &members[2] else { panic!() };
        let text = leaf(&document[0]).unwrap();
        let encoded = crate::module_blob::encode_base64(with_pal.as_bytes());
        let lines: Vec<&str> = text["#base64:".len()..].split("\r\r\n").collect();
        assert_eq!(lines.concat(), encoded);
        assert!(lines[..lines.len() - 1].iter().all(|line| line.len() == 64));
        assert!(!lines.last().unwrap().is_empty() || encoded.len() % 64 == 0);
        // A payload of exactly 64 characters keeps the break after it.
        assert_eq!(v85_base64_leaf(&"A".repeat(64)), format!("#base64:{}\r\r\n", "A".repeat(64)));
    }

    #[test]
    fn writes_the_report_state_beside_the_xml_keys() {
        let mut members = vec![Node::Leaf("59".to_owned())];
        members.extend((1..18).map(|_| Node::Leaf("0".to_owned())));
        members.push(Node::Leaf("2".to_owned()));
        members.push(Node::Leaf("7".to_owned()));
        members.push(parse_raw("{\"#\",acbc2eeb-2efb-48e4-b78a-661fd09fcf80,0}").unwrap());
        members.push(Node::Leaf("23".to_owned()));
        members.push(parse_raw("{\"N\",3}").unwrap());
        members.push(Node::Leaf("tail".to_owned()));
        let state = ReportState {
            report: "Отчет.Пример".to_owned(),
            configuration: "11111111-2222-3333-4444-555555555555".to_owned(),
        };
        add_report_state(&mut members, &state).unwrap();
        let mut out = String::new();
        emit_1c(&Node::List(members.clone()), &mut out);
        let flat = out.replace("\r\n", "");
        // Keys 7 and 23 from the XML, sixteen added: 5, 6, 8-20, 22.
        assert!(flat.contains(",18,5,{\"#\",11cfd3e0-86f8-4480-aaa5-dc6a6ccac689,{0,\"\"}},6,"));
        assert!(flat.contains(",8,{\"S\",\"urn:form:md:14:11111111-2222-3333-4444-555555555555\"},9,"));
        assert!(flat.contains(",12,{\"S\",\"Отчет.Пример\"},13,{\"S\",\"Основной\"},14,{\"B\",0},15,{\"U\"},16,{\"B\",0},17,"));
        assert!(flat.contains(",22,{\"S\",\"00000000-0000-0000-0000-000000000000\"},23,{\"N\",3},tail}"));
        assert_eq!(leaf(&members[18]), Some("18"));
    }

    #[test]
    fn lays_out_like_the_platform() {
        let node = parse_raw("{4,{59,0,{1,1,{\"ru\",\"a\"}},{0}},\"m\"}").unwrap();
        let mut out = String::new();
        emit_1c(&node, &mut out);
        assert_eq!(
            out,
            "{4,\r\n{59,0,\r\n{1,1,\r\n{\"ru\",\"a\"}\r\n},\r\n{0}\r\n},\"m\"}"
        );
    }

    #[test]
    fn respells_colours_and_fonts() {
        let mut node = parse_raw("{{3,4,{0}},{3,3,{0,11111111-2222-3333-4444-555555555555}},{7,3,0,1,100}}").unwrap();
        up_convert_primitives(&mut node);
        let mut out = String::new();
        emit_1c(&node, &mut out);
        assert_eq!(
            out.replace("\r\n", ""),
            "{{4,4,{0},4},{4,3,{0,11111111-2222-3333-4444-555555555555},3},{8,3,0,1,100}}"
        );
    }
}
