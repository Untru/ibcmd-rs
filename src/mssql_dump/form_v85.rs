//! Managed-form bodies written by platform 8.5 (form layout revision `59`).
//!
//! Measured on the 8.5.1.1150 BSP 3.2.1.356 clone against the 8.3.27 BSP
//! 3.1.11.466 corpus the 8.3.27 form codec reproduces byte for byte, pairing
//! 971 forms and 38 000-odd items by form uuid and item id: platform 8.5 keeps
//! every member of every form record at the position 8.3.27 gives it and only
//! *appends* members at the end of a record, bumping the record's leading
//! revision. The root trailer gains 12 members (`50` -> `59`), an input-field
//! record 15 (`37` -> `48`), a button 7 (`31` -> `34`), a table 28 (`55` ->
//! `73`), a list addition 1 (`5` -> `6`), a form command 2 (`9` -> `11`), and
//! each property bag its own count. Colour tuples gain one trailing kind member
//! (`{3,s,{p}}` -> `{4,s,{p},s}`) and font tuples only renumber (`7` -> `8`).
//!
//! So an 8.5 body is read by *down-converting* it: every record is cut back to
//! the members 8.3.27 declares and relabelled with the 8.3.27 revision, which
//! the existing codec then reads with every slot it already knows. What the
//! appended members say is kept aside, keyed by item id, for the 2.21 writer
//! to use. The conversion is fail-closed: a record revision or property-bag
//! shape this table does not name is refused, never passed through.

use std::collections::BTreeMap;

use anyhow::{Result, anyhow, bail};

use crate::module_blob::ParsedFormBodyBlob;

/// Layout revision of a platform 8.5 managed-form root.
pub(super) const V85_FORM_ROOT_REVISION: &str = "59";
const V83_FORM_ROOT_REVISION: &str = "50";
/// Members the 8.5 root trailer appends after the 8.3.27 trailer.
const V85_ROOT_TRAILER_APPENDED: usize = 12;

const FORM_ITEM_CLASS_UUID: &str = "02023637-7868-4a5f-8576-835a76e0c9ba";
const FORM_COMMAND_CLASS_UUID: &str = "409b9a53-7f7e-4178-86c1-33176c7c7a7a";
const FORM_CHOICE_LIST_VALUE_UUID: &str = "0e704aa2-07bd-48b9-8223-a0212c4d5fc2";

/// A parsed brace tuple. Leaves keep their exact text (quoted strings with
/// their quotes, base64 payloads with their own line breaks).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Node {
    Leaf(String),
    List(Vec<Node>),
}

impl Node {
    pub(super) fn as_leaf(&self) -> Option<&str> {
        self.leaf()
    }

    fn leaf(&self) -> Option<&str> {
        match self {
            Self::Leaf(text) => Some(text),
            Self::List(_) => None,
        }
    }

    fn list(&self) -> Option<&[Node]> {
        match self {
            Self::Leaf(_) => None,
            Self::List(items) => Some(items),
        }
    }

    pub(super) fn to_text(&self) -> String {
        let mut out = String::new();
        self.emit(&mut out);
        out
    }

    fn emit(&self, out: &mut String) {
        match self {
            Self::Leaf(text) => out.push_str(text),
            Self::List(items) => {
                out.push('{');
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    item.emit(out);
                }
                out.push('}');
            }
        }
    }
}

/// Parses one brace tuple that starts at the first `{` of `text`.
pub(super) fn parse_node(text: &str) -> Result<Node> {
    let start = text
        .find('{')
        .ok_or_else(|| anyhow!("8.5 form body tuple has no opening brace"))?;
    let bytes = text.as_bytes();
    let (node, end) = parse_list(text, bytes, start)?;
    if !text[end..].trim().is_empty() {
        bail!("8.5 form body tuple has trailing text after its closing brace");
    }
    Ok(node)
}

fn parse_list(text: &str, bytes: &[u8], start: usize) -> Result<(Node, usize)> {
    debug_assert_eq!(bytes[start], b'{');
    let mut items = Vec::new();
    let mut index = start + 1;
    let mut leaf_start = index;
    // Whether the member being read is still a leaf (no nested list yet).
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
                        None => bail!("8.5 form body has an unterminated string"),
                    }
                }
            }
            b'{' => {
                let (node, end) = parse_list(text, bytes, index)?;
                items.push(node);
                pending_leaf = false;
                index = end;
                leaf_start = index;
            }
            b',' => {
                if pending_leaf {
                    items.push(Node::Leaf(text[leaf_start..index].trim().to_owned()));
                } else if !text[leaf_start..index].trim().is_empty() {
                    bail!("8.5 form body has text between a tuple and its separator");
                }
                index += 1;
                leaf_start = index;
                pending_leaf = true;
            }
            b'}' => {
                if pending_leaf {
                    let leaf = text[leaf_start..index].trim();
                    if !leaf.is_empty() || !items.is_empty() {
                        items.push(Node::Leaf(leaf.to_owned()));
                    }
                } else if !text[leaf_start..index].trim().is_empty() {
                    bail!("8.5 form body has text between a tuple and its closing brace");
                }
                return Ok((Node::List(items), index + 1));
            }
            _ => index += 1,
        }
    }
    bail!("8.5 form body has an unterminated tuple")
}

/// What the 8.5 members appended to one form item said.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct FormV85ItemFacts {
    /// The item record's own 8.5 revision.
    pub(super) revision: String,
    /// The members 8.5 appended to the item record, in order.
    pub(super) tail: Vec<Node>,
    /// Every member of the 8.5 record ahead of the appended ones, and how
    /// far the optional common prefix shifts them.
    pub(super) record: Vec<Node>,
    pub(super) prefix_offset: usize,
    /// The item's kind code (group, field or decoration kind).
    pub(super) kind: Option<String>,
    /// The item's property bag: its 8.5 revision, every 8.5 member, and the
    /// members 8.5 appended.
    pub(super) bag_revision: Option<String>,
    pub(super) bag: Vec<Node>,
    pub(super) bag_tail: Vec<Node>,
}

/// Everything an 8.5 body carries beyond its 8.3.27 down-conversion.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct FormV85Facts {
    /// The 12 members the root trailer appends.
    pub(super) root_tail: Vec<Node>,
    /// Item facts by form item id.
    pub(super) items: BTreeMap<String, FormV85ItemFacts>,
    /// Appended form-command members by command id.
    pub(super) commands: BTreeMap<String, Vec<Node>>,
    /// Palette colour tuples left in their 8.5 shape (no 8.3.27 spelling).
    pub(super) palette_colors: usize,
    /// Colour tuples of a kind or palette index nothing names.
    pub(super) unknown_palette_colors: Vec<String>,
}

pub(super) fn is_v85_form_body(body: &ParsedFormBodyBlob) -> bool {
    body.layout
        .trim_start()
        .strip_prefix('{')
        .map(|rest| rest.trim_start())
        .is_some_and(|rest| {
            rest.strip_prefix(V85_FORM_ROOT_REVISION)
                .is_some_and(|after| after.trim_start().starts_with(','))
        })
}

/// Down-converts one 8.5 form body into the 8.3.27 shape the form codec reads.
pub(super) fn down_convert_v85_form_body(
    body: &ParsedFormBodyBlob,
) -> Result<(ParsedFormBodyBlob, FormV85Facts)> {
    let mut facts = FormV85Facts::default();
    let mut root = parse_node(&body.layout)?;
    root = convert_primitives(root, &mut facts);
    root = convert_items(root, &mut facts)?;
    root = convert_values_and_commands(root, &mut facts)?;
    if !facts.unknown_palette_colors.is_empty() {
        bail!(
            "8.5 form body carries colour tuples no palette names: {:?}",
            facts.unknown_palette_colors
        );
    }
    let Node::List(mut root_members) = root else {
        bail!("8.5 form layout is not a tuple");
    };
    if root_members.first().and_then(Node::leaf) != Some(V85_FORM_ROOT_REVISION) {
        bail!("8.5 form layout does not declare root revision 59");
    }
    root_members[0] = Node::Leaf(V83_FORM_ROOT_REVISION.to_owned());
    let kept = root_members
        .len()
        .checked_sub(V85_ROOT_TRAILER_APPENDED)
        .ok_or_else(|| anyhow!("8.5 form root is shorter than its appended trailer"))?;
    facts.root_tail = root_members.split_off(kept);
    // The trailer's own revision tuple sits one member before its end.
    let revision_index = root_members
        .len()
        .checked_sub(2)
        .ok_or_else(|| anyhow!("8.5 form root trailer is too short"))?;
    match &mut root_members[revision_index] {
        Node::List(members)
            if members.first().and_then(Node::leaf) == Some(V85_FORM_ROOT_REVISION) =>
        {
            members[0] = Node::Leaf(V83_FORM_ROOT_REVISION.to_owned());
        }
        _ => bail!("8.5 form root trailer does not carry its `{{59,...}}` revision tuple"),
    }
    let layout = Node::List(root_members).to_text();

    let mut trailing = Vec::with_capacity(body.trailing.len());
    for block in &body.trailing {
        if block.trim_start().starts_with('{') {
            let node = parse_node(block)?;
            let node = convert_primitives(node, &mut facts);
            let node = convert_items(node, &mut facts)?;
            let node = convert_values_and_commands(node, &mut facts)?;
            trailing.push(node.to_text());
        } else {
            trailing.push(block.clone());
        }
    }
    Ok((
        ParsedFormBodyBlob {
            revision: body.revision,
            layout,
            module_text: body.module_text.clone(),
            trailing,
            trailing_fields: body.trailing_fields,
        },
        facts,
    ))
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

/// `{4,<space>,{<payload>},<kind>}`: the 8.5 colour tuple.
fn is_v85_color(members: &[Node]) -> bool {
    if members.len() != 4 || members[0].leaf() != Some("4") {
        return false;
    }
    let Some(space) = members[1].leaf() else {
        return false;
    };
    if !matches!(space, "0" | "1" | "2" | "3" | "4") {
        return false;
    }
    let Some(kind) = members[3].leaf() else {
        return false;
    };
    if !is_int(kind) {
        return false;
    }
    match members[2].list() {
        Some([Node::Leaf(value)]) => is_int(value),
        Some([Node::Leaf(zero), Node::Leaf(uuid)]) => zero == "0" && is_uuid(uuid),
        _ => false,
    }
}

/// `{8,0,<mask>,...,"<face>",1,<scale>,0}`: the 8.5 absolute font, the
/// 8.3.27 `{7,0,...}` of 19 members plus one appended `0` (all 192 absolute
/// fonts of the 8.5 BSP spreadsheets).
fn is_v85_absolute_font(members: &[Node]) -> bool {
    members.len() == 20
        && members[0].leaf() == Some("8")
        && members[1].leaf() == Some("0")
        && members[2].leaf().is_some_and(is_int)
        && members[16].leaf().is_some_and(|face| face.starts_with('"'))
        && members[17].leaf() == Some("1")
        && members[18].leaf().is_some_and(is_int)
        && members[19].leaf() == Some("0")
}

/// `{8,<kind>,<mask>,...,1,<scale>}`: the 8.5 font tuple, the 8.3.27 `{7,...}`
/// member for member.
fn is_v85_font(members: &[Node]) -> bool {
    if members.len() < 5 || members[0].leaf() != Some("8") {
        return false;
    }
    let (Some(kind), Some(mask)) = (members[1].leaf(), members[2].leaf()) else {
        return false;
    };
    if !is_int(mask) || members[members.len() - 2].leaf() != Some("1") {
        return false;
    }
    if !members[members.len() - 1].leaf().is_some_and(is_int) {
        return false;
    }
    match kind {
        "3" => members.len() == 5 && mask == "0",
        "1" | "2" => members[3].list().is_some(),
        _ => false,
    }
}

/// The 2.21 spelling of a palette colour index.
///
/// 8.5.1.1150 BSP native tree: every item carrying exactly one palette tuple
/// and one `pal:` element pairs index and name without exception over the 78
/// such items. An index the table does not name refuses the body.
pub(super) fn v85_palette_color_name(index: i64) -> Option<&'static str> {
    Some(match index {
        0 => "pal:FirstBrand",
        1 => "pal:SecondBrand",
        2 => "pal:Red",
        3 => "pal:Orange",
        4 => "pal:Yellow",
        5 => "pal:Green",
        6 => "pal:LightBlue",
        // An enum value colour (`Enums/УдалитьСостоянияИнтеграцииОбъектов`),
        // beside the same record's `pal:Red` (2) and `pal:Green` (5).
        7 => "pal:Blue",
        15 => "pal:Gray",
        _ => return None,
    })
}

/// The 2.21 spelling of a palette colour tuple, if `text` is one.
pub(super) fn v85_palette_color(text: &str) -> Option<&'static str> {
    v85_palette_color_index(text.trim()).and_then(v85_palette_color_name)
}

/// Palette colour: space `4` ("automatic" to an 8.3.27 reader) with kind `5`.
pub(super) fn v85_palette_color_index(text: &str) -> Option<i64> {
    let node = parse_node(text).ok()?;
    let members = node.list()?;
    if !is_v85_color(members)
        || members[1].leaf() != Some("4")
        || members[3].leaf() != Some("5")
    {
        return None;
    }
    match members[2].list()? {
        [Node::Leaf(value)] => value.parse().ok(),
        _ => None,
    }
}

/// The span of the brace tuple that opens at `start`, if it closes within
/// `limit` bytes.
fn short_tuple_end(bytes: &[u8], start: usize, limit: usize) -> Option<usize> {
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

/// Rewrites, in place, every 8.5 colour and font tuple of a stored text into
/// its 8.3.27 spelling, leaving every other byte as stored.
///
/// Both shapes are 8.5's own: neither a `{4,<space>,{<payload>},<kind>}`
/// colour nor an `{8,<kind>,<mask>,...,1,<scale>}` font occurs anywhere in the
/// 60 000-odd 8.3.27 rows of the BSP and ERP УХ corpora (metadata texts and
/// 25 030 form bodies, `F:/ibcmd/lab/v85/tools/shapescan.py`), while the 8.5
/// BSP rows carry them by the tens of thousands. Rewriting them therefore
/// changes nothing an 8.3.27 base stores. A palette colour, which 8.3.27
/// cannot spell, stays as stored for the readers that name palettes.
pub(super) fn rewrite_v85_primitives_in_place(text: &str) -> std::borrow::Cow<'_, str> {
    const MAX_TUPLE: usize = 512;
    let bytes = text.as_bytes();
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    let mut index = 0;
    while index + 3 <= bytes.len() {
        let lead = &bytes[index..index + 3];
        if lead != b"{4," && lead != b"{8," {
            index += 1;
            continue;
        }
        let Some(end) = short_tuple_end(bytes, index, MAX_TUPLE) else {
            index += 1;
            continue;
        };
        let Ok(node) = parse_node(&text[index..end]) else {
            index += 1;
            continue;
        };
        let members = node.list().unwrap_or_default();
        if is_v85_color(members) && members[1].leaf() == members[3].leaf() {
            let mut replacement = String::from("{3,");
            replacement.push_str(members[1].leaf().unwrap_or_default());
            replacement.push(',');
            replacement.push_str(&members[2].to_text());
            replacement.push('}');
            edits.push((index, end, replacement));
            index = end;
            continue;
        }
        if is_v85_font(members) {
            edits.push((index + 1, index + 2, "7".to_owned()));
            index = end;
            continue;
        }
        if is_v85_absolute_font(members) {
            let mut converted = members.to_vec();
            converted.truncate(19);
            converted[0] = Node::Leaf("7".to_owned());
            edits.push((index, end, Node::List(converted).to_text()));
            index = end;
            continue;
        }
        index += 1;
    }
    if edits.is_empty() {
        return std::borrow::Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0;
    for (start, end, replacement) in edits {
        out.push_str(&text[cursor..start]);
        out.push_str(&replacement);
        cursor = end;
    }
    out.push_str(&text[cursor..]);
    std::borrow::Cow::Owned(out)
}

/// Rewrites the 8.5 colour and font tuples of any brace text into their
/// 8.3.27 spelling. A palette colour, which 8.3.27 cannot spell, is refused.
pub(super) fn down_convert_v85_primitives_text(text: &str) -> Result<String> {
    let mut facts = FormV85Facts::default();
    let node = convert_primitives(parse_node(text)?, &mut facts);
    if facts.palette_colors != 0 {
        bail!("8.5 tuple carries {} palette colour(s)", facts.palette_colors);
    }
    Ok(node.to_text())
}

fn convert_primitives(node: Node, facts: &mut FormV85Facts) -> Node {
    let Node::List(members) = node else {
        return node;
    };
    let members: Vec<Node> = members
        .into_iter()
        .map(|member| convert_primitives(member, facts))
        .collect();
    if is_v85_color(&members) {
        if members[1].leaf() == members[3].leaf() {
            let mut members = members;
            members.truncate(3);
            members[0] = Node::Leaf("3".to_owned());
            return Node::List(members);
        }
        facts.palette_colors += 1;
        let index = match members[2].list() {
            Some([Node::Leaf(value)]) => value.parse::<i64>().ok(),
            _ => None,
        };
        if members[1].leaf() != Some("4")
            || members[3].leaf() != Some("5")
            || index.and_then(v85_palette_color_name).is_none()
        {
            facts.unknown_palette_colors.push(Node::List(members.clone()).to_text());
        }
        return Node::List(members);
    }
    if is_v85_font(&members) {
        let mut members = members;
        members[0] = Node::Leaf("7".to_owned());
        return Node::List(members);
    }
    if is_v85_absolute_font(&members) {
        let mut members = members;
        members.truncate(19);
        members[0] = Node::Leaf("7".to_owned());
        return Node::List(members);
    }
    Node::List(members)
}

/// A form item record: `{<revision>,{<id>,<form item class>},...}`.
fn item_identity(members: &[Node]) -> Option<&str> {
    if members.len() < 20 || !members[0].leaf().is_some_and(is_int) {
        return None;
    }
    match members[1].list()? {
        [Node::Leaf(id), Node::Leaf(class)] if class == FORM_ITEM_CLASS_UUID && is_int(id) => {
            Some(id)
        }
        _ => None,
    }
}

/// 8.5 item revision -> (8.3.27 revision, appended members).
fn item_revision(revision: &str) -> Option<(&'static str, usize)> {
    Some(match revision {
        "48" => ("37", 15),
        "34" => ("31", 7),
        "73" => ("55", 28),
        "22" => ("22", 0),
        "12" => ("12", 0),
        "6" => ("5", 1),
        _ => return None,
    })
}

/// The property-bag slot of a record that carries one, before the optional
/// common prefix shifts it.
fn bag_base_slot(v83_revision: &str) -> Option<usize> {
    match v83_revision {
        "22" => Some(20),
        "37" => Some(39),
        "12" => Some(18),
        _ => None,
    }
}

/// (owner 8.3.27 revision, item kind, 8.5 bag revision, 8.5 bag length) ->
/// (8.3.27 bag revision, appended members).
///
/// Keyed by the item kind because two kinds share a shape across the
/// revisions: a `ButtonGroup` (group kind 6) keeps its `{2,...}` bag of four
/// members, while a `CommandBar` (kind 0) moves from `{1,...}` of three to
/// `{2,...}` of four. Every (kind, shape) pair of the 8.5 BSP bodies is listed;
/// each maps onto the one shape the 8.3.27 BSP bodies give that kind.
fn bag_revision(
    v83_owner: &str,
    kind: &str,
    revision: &str,
    len: usize,
) -> Option<(&'static str, usize)> {
    Some(match (v83_owner, kind, revision, len) {
        // Groups: CommandBar, ColumnGroup, Popup, Pages, Page, UsualGroup,
        // ButtonGroup, ContextMenu, AutoCommandBar.
        ("22", "0", "2", 4) => ("1", 1),
        ("22", "1", "8", 10) => ("7", 1),
        ("22", "2", "5", 16) => ("2", 4),
        ("22", "3", "4", 6) => ("4", 0),
        ("22", "4", "22", 24) => ("18", 4),
        ("22", "5", "38", 42) => ("29", 13),
        ("22", "6", "2", 4) => ("2", 0),
        ("22", "8", "1", 2) => ("1", 0),
        ("22", "9", "1", 4) => ("0", 1),
        // Fields, by field kind.
        ("37", "1", "12", 21) => ("11", 1),
        ("37", "2", "38", 71) => ("36", 5),
        ("37", "3", "13", 15) => ("11", 2),
        ("37", "4", "12", 26) => ("10", 2),
        ("37", "5", "11", 15) => ("8", 3),
        ("37", "6", "15", 34) => ("13", 2),
        ("37", "7", "5", 16) => ("5", 0),
        ("37", "8", "6", 24) => ("6", 0),
        ("37", "9", "4", 16) => ("4", 0),
        ("37", "10", "2", 18) => ("2", 0),
        ("37", "11", "1", 11) => ("1", 0),
        ("37", "14", "3", 14) => ("3", 0),
        ("37", "15", "4", 14) => ("3", 1),
        ("37", "17", "1", 16) => ("1", 0),
        // Decorations: label (and extended tooltip), picture.
        ("12", "0", "5", 9) => ("5", 0),
        ("12", "1", "6", 15) => ("4", 2),
        _ => return None,
    })
}

fn convert_items(node: Node, facts: &mut FormV85Facts) -> Result<Node> {
    let Node::List(members) = node else {
        return Ok(node);
    };
    let mut members = members
        .into_iter()
        .map(|member| convert_items(member, facts))
        .collect::<Result<Vec<_>>>()?;
    let Some(id) = item_identity(&members).map(ToOwned::to_owned) else {
        return Ok(Node::List(members));
    };
    let revision = members[0].leaf().unwrap_or_default().to_owned();
    let (v83, appended) = item_revision(&revision)
        .ok_or_else(|| anyhow!("8.5 form item {id} declares unknown record revision {revision}"))?;
    let kept = members
        .len()
        .checked_sub(appended)
        .ok_or_else(|| anyhow!("8.5 form item {id} is shorter than its appended members"))?;
    let tail = members.split_off(kept);
    members[0] = Node::Leaf(v83.to_owned());
    let mut item_facts = FormV85ItemFacts {
        revision,
        tail,
        ..FormV85ItemFacts::default()
    };
    // The optional common prefix is a tuple ahead of the kind code and shifts
    // every later member, the bag with them.
    item_facts.prefix_offset = usize::from(matches!(members.get(5), Some(Node::List(_))));
    item_facts.record = members.clone();
    if let Some(base) = bag_base_slot(v83) {
        let offset = item_facts.prefix_offset;
        let kind = members
            .get(5 + offset)
            .and_then(Node::leaf)
            .ok_or_else(|| anyhow!("8.5 form item {id} carries no kind code"))?
            .to_owned();
        let slot = base + offset;
        let Some(Node::List(bag)) = members.get_mut(slot) else {
            bail!("8.5 form item {id} (kind {kind}) carries no property bag at member {slot}");
        };
        let bag_lead = bag
            .first()
            .and_then(Node::leaf)
            .filter(|lead| is_int(lead))
            .ok_or_else(|| anyhow!("8.5 form item {id} property bag has no revision"))?
            .to_owned();
        let (bag_v83, bag_appended) = bag_revision(v83, &kind, &bag_lead, bag.len())
            .ok_or_else(|| {
                anyhow!(
                    "8.5 form item {id} (revision {}, kind {kind}) carries unknown property bag {{{bag_lead},...}} with {} members",
                    item_facts.revision,
                    bag.len()
                )
            })?;
        item_facts.bag = bag.clone();
        let bag_kept = bag.len() - bag_appended;
        item_facts.bag_tail = bag.split_off(bag_kept);
        item_facts.bag_revision = Some(bag_lead);
        bag[0] = Node::Leaf(bag_v83.to_owned());
        item_facts.kind = Some(kind);
    }
    facts.items.insert(id, item_facts);
    Ok(Node::List(members))
}

/// Form commands: `{11,{<id>,<form command class>},...}` with 21 members, the
/// 8.3.27 `{9,...}` with 19 plus two appended.
fn convert_values_and_commands(node: Node, facts: &mut FormV85Facts) -> Result<Node> {
    let Node::List(members) = node else {
        return Ok(node);
    };
    let mut members = members
        .into_iter()
        .map(|member| convert_values_and_commands(member, facts))
        .collect::<Result<Vec<_>>>()?;
    let command_id = match members.get(1).and_then(Node::list) {
        Some([Node::Leaf(id), Node::Leaf(class)])
            if class == FORM_COMMAND_CLASS_UUID && is_int(id) && members.len() > 4 =>
        {
            Some(id.clone())
        }
        _ => None,
    };
    if let Some(id) = command_id {
        match (members[0].leaf(), members.len()) {
            (Some("11"), 21) => {
                let tail = members.split_off(19);
                members[0] = Node::Leaf("9".to_owned());
                facts.commands.insert(id, tail);
            }
            (lead, len) => bail!(
                "8.5 form command {id} declares unknown revision {lead:?} with {len} members"
            ),
        }
        return Ok(Node::List(members));
    }
    // A choice-list value: `{"#",<value class>,{1,...}}` with seven members,
    // the 8.3.27 `{0,...}` with six plus one appended.
    if members.len() == 3
        && members[0].leaf() == Some("\"#\"")
        && members[1].leaf() == Some(FORM_CHOICE_LIST_VALUE_UUID)
    {
        if let Node::List(value) = &mut members[2] {
            match (value.first().and_then(Node::leaf), value.len()) {
                (Some("1"), 7) => {
                    value.truncate(6);
                    value[0] = Node::Leaf("0".to_owned());
                }
                (lead, len) => bail!(
                    "8.5 form choice-list value declares unknown revision {lead:?} with {len} members"
                ),
            }
        }
    }
    Ok(Node::List(members))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_a_tuple_with_strings_and_base64() {
        let text = "{4,\r\n{59,0,\"a,\"\"b}\"},{#base64:QUJD\r\nREVG},{},{1,}}";
        let node = parse_node(text).unwrap();
        assert_eq!(
            node.to_text(),
            "{4,{59,0,\"a,\"\"b}\"},{#base64:QUJD\r\nREVG},{},{1,}}"
        );
    }
}
