//! The spreadsheet template writer: `Template.xml` in, the MOXCEL body the
//! platform stores out.
//!
//! The stored body is one positional record (the grammar below consumes all
//! 14 046 ERP УХ and 61 БСП spreadsheet rows field for field):
//!
//! ```text
//! {8,1,12, <language>, <default format>, <lines>, 0, <header/footer x6>,
//!  <template mode>, <step direction>,
//!  N, (row, format, n, (column, cell) x n) x N,
//!  <default column set>, <height>, M, <column set> x M, P, (row, set) x P,
//!  <last drawing id>, D, <drawing> x D,
//!  V, (<row group>, -1) x V, H, (<column group>, -1) x H, 0, 0,
//!  <merges>, <row merges>, <column merges>, <named items>, "",
//!  <print properties>, <print area>, <10 scalars>,
//!  F, <format> x F, <fonts>, <number formats>, <value types>,
//!  <control types>, <palette>, <pictures>, 0, 0, "", 0, <print record>,
//!  <palette overrides>, <masks>, 0, 0, 1, 0, 0, 0}
//! ```
//!
//! Every document-level table -- formats, fonts, lines, pictures, the colour
//! palette, number formats, masks, value and control types -- is numbered by
//! first use in the order the body itself is written: the leading default
//! format, the header/footer records, the rows with their cells and notes, the
//! column sets, the drawings, the group/header colour scalars and then the
//! format table itself. The exporter numbers the same objects by first use in
//! its own XML order, so a `Template.xml` it wrote is renumbered here and read
//! back to the same indexes.
//!
//! Whatever the source does not spell is refused rather than guessed.

use std::collections::HashMap;

use anyhow::{Context, Result, anyhow, bail};
use quick_xml::Reader;
use quick_xml::escape::{resolve_xml_entity, unescape};
use quick_xml::events::{BytesStart, Event};

use crate::module_blob::MetadataSourceContext;

const NIL_UUID: &str = "00000000-0000-0000-0000-000000000000";
const CELL_LINE_KIND: &str = "f527dc88-1d39-40b3-bcbb-d98b690ead68";
const DRAWING_LINE_KIND: &str = "b7438842-27cc-42a3-846f-2250cd9c1bc3";
/// The value type a stored `<r>` reference carries; the XML publishes only the
/// index, and every stored reference of both corpora names this type.
const REFERENCE_VALUE_TYPE: &str = "3031edd8-c3df-47b2-98ca-47f628d4ec18";
const STRUCTURE_VALUE_TYPE: &str = "4238019d-7e49-4fc9-91db-b6b951d5cf8e";
const ARRAY_VALUE_TYPE: &str = "51e7a0d2-530b-11d4-b98a-008048da3034";
const WEB_COLORS_NS: &str = "http://v8.1c.ru/8.1/data/ui/colors/web";
const WINDOWS_COLORS_NS: &str = "http://v8.1c.ru/8.1/data/ui/colors/windows";
const CURRENT_CONFIG_NS: &str = "http://v8.1c.ru/8.1/data/enterprise/current-config";
const SYSTEM_FONTS_NS: &str = "http://v8.1c.ru/8.1/data/ui/fonts/system";

/// The stored body of a `Template.xml`, from `{8,` to the closing brace, with
/// the platform's line layout (CRLF before every `{` but the first and before
/// a `}` that closes right behind another).
pub(crate) fn write_native_moxel_body(
    xml: &[u8],
    source: Option<&MetadataSourceContext>,
) -> Result<String> {
    // Element spans index the text behind the byte-order mark.
    let xml = xml.strip_prefix(b"\xef\xbb\xbf").unwrap_or(xml);
    let root = parse_dom(xml)?;
    if root.name != "document" {
        bail!("SpreadsheetDocument XML root is <{}>, not <document>", root.name);
    }
    let source_text =
        std::str::from_utf8(xml).context("SpreadsheetDocument XML is not UTF-8")?;
    let document = Document::collect(&root, source_text)?;
    let mut writer = BodyWriter::new(&document, source);
    let body = writer.write()?;
    Ok(layout(&body).replace('\n', "\r\n"))
}

// ---------------------------------------------------------------------------
// XML tree
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
struct Node {
    /// Local name.
    name: String,
    /// Qualified attribute names as written, with unescaped values.
    attributes: Vec<(String, String)>,
    children: Vec<Node>,
    text: String,
    /// The element's byte span in the source, start tag to end tag.
    span: (usize, usize),
}

impl Node {
    fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|child| child.name == name)
    }

    fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Node> + 'a {
        self.children.iter().filter(move |child| child.name == name)
    }

    fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    /// An attribute by local name, whatever prefix it is written with, xmlns
    /// declarations aside.
    fn attribute_local(&self, local: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(key, _)| {
                !key.starts_with("xmlns")
                    && key.rsplit_once(':').map_or(key.as_str(), |(_, name)| name) == local
            })
            .map(|(_, value)| value.as_str())
    }

    fn xsi_type(&self) -> Option<&str> {
        self.attribute_local("type")
    }

    fn is_nil(&self) -> bool {
        self.attribute_local("nil") == Some("true")
    }

    /// The namespace an `xmlns:<prefix>` declared on this very element binds.
    fn own_namespace(&self, prefix: &str) -> Option<&str> {
        self.attribute(&format!("xmlns:{prefix}"))
    }

    fn text_of(&self, name: &str) -> Option<&str> {
        self.child(name).map(|child| child.text.as_str())
    }

    fn only_children(&self, allowed: &[&str], owner: &str) -> Result<()> {
        for child in &self.children {
            if !allowed.contains(&child.name.as_str()) {
                bail!("<{}> inside <{owner}> has no writer", child.name);
            }
        }
        Ok(())
    }

    fn only_attributes(&self, allowed: &[&str], owner: &str) -> Result<()> {
        for (key, _) in &self.attributes {
            if key.starts_with("xmlns") {
                continue;
            }
            let local = key.rsplit_once(':').map_or(key.as_str(), |(_, name)| name);
            if !allowed.contains(&local) {
                bail!("attribute {key} of <{owner}> has no writer");
            }
        }
        Ok(())
    }
}

fn parse_dom(xml: &[u8]) -> Result<Node> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(false);
    let mut buffer = Vec::new();
    let mut stack: Vec<Node> = Vec::new();
    let mut root = None;
    loop {
        // Text is an event of its own (nothing is trimmed), so the position
        // in front of a tag event is that tag's `<`.
        let before = reader.buffer_position() as usize;
        match reader
            .read_event_into(&mut buffer)
            .context("SpreadsheetDocument XML is not well-formed")?
        {
            Event::Start(start) => {
                let mut node = start_node(&start)?;
                node.span.0 = before;
                stack.push(node);
            }
            Event::Empty(start) => {
                let mut node = start_node(&start)?;
                node.span = (before, reader.buffer_position() as usize);
                match stack.last_mut() {
                    Some(parent) => parent.children.push(node),
                    None => root = Some(node),
                }
            }
            Event::End(_) => {
                let mut node = stack
                    .pop()
                    .ok_or_else(|| anyhow!("SpreadsheetDocument XML closes an unopened element"))?;
                node.span.1 = reader.buffer_position() as usize;
                match stack.last_mut() {
                    Some(parent) => parent.children.push(node),
                    None => root = Some(node),
                }
            }
            Event::Text(text) => {
                if let Some(node) = stack.last_mut() {
                    let decoded = text.decode()?;
                    node.text.push_str(&unescape(&decoded)?);
                }
            }
            Event::CData(text) => {
                if let Some(node) = stack.last_mut() {
                    node.text.push_str(&text.decode()?);
                }
            }
            Event::GeneralRef(reference) => {
                if let Some(node) = stack.last_mut() {
                    if let Some(ch) = reference.resolve_char_ref()? {
                        node.text.push(ch);
                    } else {
                        let entity = reference.decode()?;
                        let value = resolve_xml_entity(&entity)
                            .ok_or_else(|| anyhow!("unrecognized XML entity: {entity}"))?;
                        node.text.push_str(value);
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    root.ok_or_else(|| anyhow!("SpreadsheetDocument XML has no root element"))
}

fn start_node(start: &BytesStart<'_>) -> Result<Node> {
    let qualified = String::from_utf8_lossy(start.name().as_ref()).into_owned();
    let name = qualified
        .rsplit_once(':')
        .map_or(qualified.as_str(), |(_, local)| local)
        .to_string();
    let mut attributes = Vec::new();
    for attribute in start.attributes() {
        let attribute = attribute?;
        let key = String::from_utf8_lossy(attribute.key.as_ref()).into_owned();
        let value = attribute.unescape_value()?.into_owned();
        attributes.push((key, value));
    }
    Ok(Node {
        name,
        attributes,
        children: Vec::new(),
        text: String::new(),
        span: (0, 0),
    })
}

// ---------------------------------------------------------------------------
// Scalar helpers
// ---------------------------------------------------------------------------

fn parse_i64(text: &str, what: &str) -> Result<i64> {
    text.trim()
        .parse::<i64>()
        .map_err(|_| anyhow!("{what} `{text}` is not an integer"))
}

fn parse_usize(text: &str, what: &str) -> Result<usize> {
    text.trim()
        .parse::<usize>()
        .map_err(|_| anyhow!("{what} `{text}` is not a non-negative integer"))
}

fn parse_bool(text: &str, what: &str) -> Result<bool> {
    match text.trim() {
        "true" => Ok(true),
        "false" => Ok(false),
        other => bail!("{what} `{other}` is not a boolean"),
    }
}

fn required_text<'a>(node: &'a Node, name: &str, owner: &str) -> Result<&'a str> {
    node.text_of(name)
        .ok_or_else(|| anyhow!("<{owner}> has no <{name}>"))
}

fn required_i64(node: &Node, name: &str, owner: &str) -> Result<i64> {
    parse_i64(required_text(node, name, owner)?, &format!("<{owner}><{name}>"))
}

fn required_usize(node: &Node, name: &str, owner: &str) -> Result<usize> {
    parse_usize(required_text(node, name, owner)?, &format!("<{owner}><{name}>"))
}

fn optional_usize(node: &Node, name: &str, owner: &str) -> Result<Option<usize>> {
    node.text_of(name)
        .map(|text| parse_usize(text, &format!("<{owner}><{name}>")))
        .transpose()
}

/// A 1C string literal.
fn quote(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        if ch == '"' {
            out.push('"');
        }
        out.push(ch);
    }
    out.push('"');
    out
}

fn is_uuid(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => *byte == b'-',
            _ => byte.is_ascii_hexdigit(),
        })
}

fn uuid_text(text: &str, what: &str) -> Result<String> {
    let trimmed = text.trim();
    if !is_uuid(trimmed) {
        bail!("{what} `{text}` is not a uuid");
    }
    Ok(trimmed.to_ascii_lowercase())
}

// ---------------------------------------------------------------------------
// Brace writer
// ---------------------------------------------------------------------------

/// Accumulates compact brace text; [`layout`] gives it the platform's line
/// breaks once the whole body is written.
#[derive(Default)]
struct Out {
    text: String,
}

impl Out {
    fn open(&mut self) {
        self.text.push('{');
    }

    fn close(&mut self) {
        self.text.push('}');
    }

    fn comma(&mut self) {
        self.text.push(',');
    }

    fn raw(&mut self, text: &str) {
        self.text.push_str(text);
    }

    fn record(&mut self, record: &str) {
        self.text.push_str(record);
    }
}

/// The platform's layout of brace text: a line break before every `{` but the
/// very first, and before a `}` that closes right behind another `}`; string
/// contents are copied as they are. Line breaks are `\n` here and become CRLF
/// afterwards, together with every `\n` inside a string -- which is how the
/// platform stores a text's own line breaks.
fn layout(compact: &str) -> String {
    let mut text = String::with_capacity(compact.len() + compact.len() / 8);
    let mut in_string = false;
    for ch in compact.chars() {
        if in_string {
            text.push(ch);
            if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => {
                in_string = true;
                text.push(ch);
            }
            '{' => {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push('{');
            }
            '}' => {
                if text.ends_with('}') {
                    text.push('\n');
                }
                text.push('}');
            }
            _ => text.push(ch),
        }
    }
    text
}

// ---------------------------------------------------------------------------
// Document collection
// ---------------------------------------------------------------------------

/// Header/footer elements in the order the body stores their records.
const HEADER_FOOTER_BLOCK: [&str; 6] = [
    "leftHeader",
    "leftFooter",
    "centerHeader",
    "centerFooter",
    "rightHeader",
    "rightFooter",
];

#[derive(Default)]
struct Document<'a> {
    /// The source, which a chart drawing is handed to the chart codec from.
    source_text: &'a str,
    language: Option<&'a Node>,
    column_sets: Vec<&'a Node>,
    rows: Vec<&'a Node>,
    template_mode: bool,
    step_direction: Option<&'a str>,
    default_format_index: Option<usize>,
    height: Option<usize>,
    vg_rows: Option<usize>,
    vg_levels: Option<usize>,
    row_groups: Vec<&'a Node>,
    column_groups: Vec<&'a Node>,
    merges: Vec<&'a Node>,
    vertical_unmerges: Vec<&'a Node>,
    horizontal_unmerges: Vec<&'a Node>,
    named_items: Vec<&'a Node>,
    print_settings: Option<&'a Node>,
    print_area: Option<&'a Node>,
    group_colors: [Option<&'a Node>; 4],
    lines: Vec<&'a Node>,
    fonts: Vec<&'a Node>,
    formats: Vec<&'a Node>,
    pictures: Vec<&'a Node>,
    drawings: Vec<&'a Node>,
    headers_footers: [Option<&'a Node>; 6],
}

impl<'a> Document<'a> {
    fn collect(root: &'a Node, source_text: &'a str) -> Result<Self> {
        let mut document = Document {
            source_text,
            ..Document::default()
        };
        for child in &root.children {
            match child.name.as_str() {
                "languageSettings" => {
                    if document.language.replace(child).is_some() {
                        bail!("the document spells <languageSettings> twice");
                    }
                }
                "columns" => document.column_sets.push(child),
                "rowsItem" => document.rows.push(child),
                "templateMode" => {
                    document.template_mode = parse_bool(&child.text, "<templateMode>")?
                }
                "stepDirection" => document.step_direction = Some(child.text.as_str()),
                "defaultFormatIndex" => {
                    document.default_format_index =
                        Some(parse_usize(&child.text, "<defaultFormatIndex>")?)
                }
                "height" => document.height = Some(parse_usize(&child.text, "<height>")?),
                "vgRows" => document.vg_rows = Some(parse_usize(&child.text, "<vgRows>")?),
                "vgLevels" => document.vg_levels = Some(parse_usize(&child.text, "<vgLevels>")?),
                "vg" => document.row_groups.push(child),
                "hg" => document.column_groups.push(child),
                "merge" => document.merges.push(child),
                "verticalUnmerge" => document.vertical_unmerges.push(child),
                "horizontalUnmerge" => document.horizontal_unmerges.push(child),
                "namedItem" => document.named_items.push(child),
                "printSettings" => document.print_settings = Some(child),
                "printArea" => document.print_area = Some(child),
                "groupsBackColor" => document.group_colors[0] = Some(child),
                "groupsColor" => document.group_colors[1] = Some(child),
                "headersBackColor" => document.group_colors[2] = Some(child),
                "headersColor" => document.group_colors[3] = Some(child),
                "line" => document.lines.push(child),
                "font" => document.fonts.push(child),
                "format" => document.formats.push(child),
                "picture" => document.pictures.push(child),
                "drawing" => document.drawings.push(child),
                other => {
                    if let Some(slot) = HEADER_FOOTER_BLOCK.iter().position(|tag| *tag == other) {
                        document.headers_footers[slot] = Some(child);
                    } else {
                        bail!("<{other}> of a SpreadsheetDocument has no writer");
                    }
                }
            }
        }
        Ok(document)
    }
}

// ---------------------------------------------------------------------------
// Pools
// ---------------------------------------------------------------------------

/// Objects the XML numbers by position, renumbered by first use.
#[derive(Default)]
struct IdentityPool {
    /// XML key -> stored position (0-based).
    positions: HashMap<usize, usize>,
    /// Stored order, as XML keys.
    order: Vec<usize>,
}

impl IdentityPool {
    fn intern(&mut self, key: usize) -> usize {
        if let Some(position) = self.positions.get(&key) {
            return *position;
        }
        let position = self.order.len();
        self.positions.insert(key, position);
        self.order.push(key);
        position
    }
}

/// Values the body keeps once each, numbered by first use.
#[derive(Default)]
struct ContentPool {
    positions: HashMap<String, usize>,
    entries: Vec<String>,
}

impl ContentPool {
    fn intern(&mut self, entry: String) -> usize {
        if let Some(position) = self.positions.get(&entry) {
            return *position;
        }
        let position = self.entries.len();
        self.positions.insert(entry.clone(), position);
        self.entries.push(entry);
        position
    }
}

// ---------------------------------------------------------------------------
// Body writer
// ---------------------------------------------------------------------------

struct BodyWriter<'a, 'd> {
    document: &'a Document<'d>,
    source: Option<&'a MetadataSourceContext>,
    formats: IdentityPool,
    fonts: IdentityPool,
    lines: IdentityPool,
    pictures: IdentityPool,
    palette: ContentPool,
    overrides: Vec<(usize, String)>,
    number_formats: ContentPool,
    masks: ContentPool,
    value_types: ContentPool,
    control_types: ContentPool,
    /// Formats a drawing or a note names, which spend border members on
    /// drawing members.
    drawing_formats: std::collections::HashSet<usize>,
}

impl<'a, 'd> BodyWriter<'a, 'd> {
    fn new(document: &'a Document<'d>, source: Option<&'a MetadataSourceContext>) -> Self {
        Self {
            document,
            source,
            formats: IdentityPool::default(),
            fonts: IdentityPool::default(),
            lines: IdentityPool::default(),
            pictures: IdentityPool::default(),
            palette: ContentPool::default(),
            overrides: Vec::new(),
            number_formats: ContentPool::default(),
            masks: ContentPool::default(),
            value_types: ContentPool::default(),
            control_types: ContentPool::default(),
            drawing_formats: std::collections::HashSet::new(),
        }
    }

    /// The stored reference of an XML format index: 0 for none, else the
    /// 1-based position in the stored table.
    fn format_ref(&mut self, xml_index: usize, owner: &str) -> Result<usize> {
        if xml_index == 0 {
            return Ok(0);
        }
        if xml_index > self.document.formats.len() {
            bail!(
                "{owner} names format {xml_index}, and the document spells {}",
                self.document.formats.len()
            );
        }
        Ok(self.formats.intern(xml_index) + 1)
    }

    fn write(&mut self) -> Result<String> {
        let document = self.document;
        self.mark_drawing_formats()?;

        // The leading default format is written first and claims its colours,
        // fonts and lines before anything else.
        let default_format = match document.default_format_index {
            Some(index) if index > 0 => {
                let node = *document.formats.get(index - 1).ok_or_else(|| {
                    anyhow!(
                        "<defaultFormatIndex> {index} names no format of {}",
                        document.formats.len()
                    )
                })?;
                // The exporter renumbers a line table that is not in the order
                // its published formats cite it, and it renumbers every format
                // but the leading default one. A default format that cites a
                // line therefore keeps the table in that very order -- the XML's
                // own, which is first-citation order -- so nothing is
                // renumbered. The platform's rows split both ways here (four
                // ERP УХ rows first-use, two last-use, of seventeen).
                let cites_line = node.children.iter().any(|member| {
                    matches!(
                        member.name.as_str(),
                        "border"
                            | "leftBorder"
                            | "topBorder"
                            | "rightBorder"
                            | "bottomBorder"
                            | "drawingBorder"
                    )
                });
                if cites_line {
                    for line in 0..document.lines.len() {
                        self.lines.intern(line);
                    }
                }
                self.format_record(node, false)?
            }
            _ => "{0}".to_string(),
        };

        let header_footers = self.header_footer_records()?;
        let rows = self.rows()?;
        let column_sets = self.column_sets()?;
        let drawings = self.drawings()?;
        let groups = self.groups()?;
        let merges = self.merges()?;
        let named_items = self.named_items()?;
        let print_properties = self.print_properties()?;
        let print_area = self.print_area()?;
        let scalars = self.scalars()?;
        let format_table = self.format_table()?;
        let fonts = self.font_table()?;
        let pictures = self.picture_table()?;
        let lines = self.line_table()?;

        let mut out = Out::default();
        out.open();
        out.raw("8,1,12,");
        self.language(&mut out)?;
        out.comma();
        out.record(&default_format);
        out.comma();
        out.record(&lines);
        out.raw(",0");
        for record in &header_footers {
            out.comma();
            out.record(record);
        }
        out.comma();
        out.raw(if document.template_mode { "1" } else { "0" });
        out.comma();
        out.raw(match document.step_direction {
            None => "2",
            Some("WithoutMove") => "3",
            Some(other) => bail!("<stepDirection> {other} has no stored code"),
        });
        out.comma();
        out.raw(&rows.count.to_string());
        out.text.push_str(&rows.text);
        out.comma();
        out.text.push_str(&column_sets);
        out.comma();
        out.text.push_str(&drawings);
        out.comma();
        out.text.push_str(&groups);
        out.comma();
        out.text.push_str(&merges);
        out.comma();
        out.text.push_str(&named_items);
        out.raw(",\"\",");
        out.text.push_str(&print_properties);
        out.comma();
        out.text.push_str(&print_area);
        out.comma();
        out.raw(&scalars);
        out.comma();
        out.text.push_str(&format_table);
        out.comma();
        out.text.push_str(&fonts);
        out.comma();
        self.content_table(&mut out, ContentTable::NumberFormats);
        out.comma();
        self.content_table(&mut out, ContentTable::ValueTypes);
        out.comma();
        self.content_table(&mut out, ContentTable::ControlTypes);
        out.comma();
        self.palette_table(&mut out);
        out.comma();
        out.text.push_str(&pictures);
        out.raw(",0,0,\"\",0,");
        out.record("{3,0,0,100,1,1,0,1,1,0,0,0,0,0,0,0,0,0,0,0,0,\"\",0,0,0,0,0,0,0}");
        out.comma();
        self.override_table(&mut out);
        out.comma();
        self.content_table(&mut out, ContentTable::Masks);
        out.raw(",0,0,1,0,0,0");
        out.close();
        Ok(out.text)
    }

    /// Formats a drawing or a note names spend members 1, 3 and 4 on drawing
    /// members; a format a cell, row or column also names keeps them.
    fn mark_drawing_formats(&mut self) -> Result<()> {
        let document = self.document;
        let mut drawing = std::collections::HashSet::new();
        for node in &document.drawings {
            drawing.insert(required_usize(node, "formatIndex", "drawing")?);
        }
        let mut cell = std::collections::HashSet::new();
        for item in &document.rows {
            let Some(row) = item.child("row") else {
                continue;
            };
            if let Some(index) = optional_usize(row, "formatIndex", "row")? {
                cell.insert(index);
            }
            for outer in row.children_named("c") {
                let Some(inner) = outer.child("c") else {
                    continue;
                };
                if let Some(index) = optional_usize(inner, "f", "c")? {
                    cell.insert(index);
                }
                if let Some(note) = inner.child("note") {
                    let index = required_usize(note, "formatIndex", "note")?;
                    drawing.insert(index);
                }
            }
        }
        for set in &document.column_sets {
            if let Some(index) = optional_usize(set, "formatIndex", "columns")? {
                cell.insert(index);
            }
            for item in set.children_named("columnsItem") {
                if let Some(column) = item.child("column")
                    && let Some(index) = optional_usize(column, "formatIndex", "column")?
                {
                    cell.insert(index);
                }
            }
        }
        for node in document.headers_footers.iter().flatten() {
            if let Some(index) = optional_usize(node, "f", &node.name)? {
                cell.insert(index);
            }
        }
        self.drawing_formats = drawing.difference(&cell).copied().collect();
        Ok(())
    }

    // -- language ----------------------------------------------------------

    fn language(&self, out: &mut Out) -> Result<()> {
        let Some(settings) = self.document.language else {
            out.record("{\"#\",\"\",1,1,\"#\",\"Язык по умолчанию\",\"Язык по умолчанию\",0}");
            return Ok(());
        };
        settings.only_children(
            &["currentLanguage", "defaultLanguage", "languageInfo"],
            "languageSettings",
        )?;
        let current = settings.text_of("currentLanguage").unwrap_or_default();
        let default = settings.text_of("defaultLanguage").unwrap_or_default();
        let infos = settings.children_named("languageInfo").collect::<Vec<_>>();
        let mut record = format!("{{{},{},1,{}", quote(current), quote(default), infos.len());
        for info in &infos {
            info.only_children(&["id", "code", "description"], "languageInfo")?;
            record.push_str(&format!(
                ",{},{},{}",
                quote(info.text_of("id").unwrap_or_default()),
                quote(info.text_of("code").unwrap_or_default()),
                quote(info.text_of("description").unwrap_or_default())
            ));
        }
        // What a load from XML stores behind the descriptors: 0 for a
        // single language and 1 for several, over every БСП row and every ERP
        // УХ row that carries the load's own leading 1.
        record.push_str(if infos.len() > 1 { ",1}" } else { ",0}" });
        out.record(&record);
        Ok(())
    }

    // -- header / footer ---------------------------------------------------

    fn header_footer_records(&mut self) -> Result<Vec<String>> {
        let mut records = Vec::with_capacity(6);
        for slot in 0..6 {
            let Some(node) = self.document.headers_footers[slot] else {
                records.push("{0,0}".to_string());
                continue;
            };
            let tag = HEADER_FOOTER_BLOCK[slot];
            node.only_children(&["f", "tl", "tfl"], tag)?;
            let format = optional_usize(node, "f", tag)?.unwrap_or(0);
            let format = self.format_ref(format, tag)?;
            let record = match (node.child("tl"), node.child("tfl")) {
                // `{0,0}` is the absent record, which publishes nothing.
                (None, None) if format == 0 => {
                    bail!("<{tag}> names neither a format nor a text")
                }
                (None, None) => format!("{{0,{format}}}"),
                (Some(text), None) => {
                    format!("{{16,{format},{},0}}", localized_items(text, tag)?)
                }
                (None, Some(text)) => {
                    let items = localized_items(text, tag)?;
                    let plain = localized_items_stripped(text, tag)?;
                    format!("{{16,{format},{plain},1,{{1,{items},1}}}}")
                }
                (Some(_), Some(_)) => bail!("<{tag}> spells both <tl> and <tfl>"),
            };
            records.push(record);
        }
        Ok(records)
    }

    // -- rows --------------------------------------------------------------

    fn rows(&mut self) -> Result<RowsOut> {
        let document = self.document;
        let mut text = String::new();
        let mut count = 0usize;
        let mut last_index: Option<usize> = None;
        for item in &document.rows {
            item.only_children(&["index", "indexTo", "row"], "rowsItem")?;
            let index = required_usize(item, "index", "rowsItem")?;
            let index_to = optional_usize(item, "indexTo", "rowsItem")?.unwrap_or(index);
            if index_to < index || last_index.is_some_and(|last| index <= last) {
                bail!("<rowsItem> {index}..{index_to} is out of order");
            }
            last_index = Some(index_to);
            let row = item
                .child("row")
                .ok_or_else(|| anyhow!("<rowsItem> {index} has no <row>"))?;
            row.only_children(&["columnsID", "formatIndex", "empty", "c"], "row")?;
            let empty = match row.text_of("empty") {
                Some(text) => parse_bool(text, "<row><empty>")?,
                None => false,
            };
            let cells = row.children_named("c").collect::<Vec<_>>();
            if empty != cells.is_empty() {
                bail!("<rowsItem> {index} spells <empty> against its cells");
            }
            let xml_format = optional_usize(row, "formatIndex", "row")?.unwrap_or(0);
            for row_index in index..=index_to {
                let format = self.format_ref(xml_format, "a <row>")?;
                let mut out = Out {
                    text: std::mem::take(&mut text),
                };
                out.raw(&format!(",{row_index},{format},{}", cells.len()));
                let mut column = 0usize;
                for outer in &cells {
                    outer.only_children(&["i", "c"], "c")?;
                    if let Some(explicit) = optional_usize(outer, "i", "c")? {
                        column = explicit;
                    }
                    let inner = outer
                        .child("c")
                        .ok_or_else(|| anyhow!("a <c> of row {row_index} has no inner <c>"))?;
                    out.raw(&format!(",{column},"));
                    self.cell(&mut out, inner)?;
                    column += 1;
                }
                text = out.text;
                count += 1;
            }
        }
        Ok(RowsOut { count, text })
    }

    fn cell(&mut self, out: &mut Out, cell: &Node) -> Result<()> {
        cell.only_children(
            &[
                "f",
                "tl",
                "tfl",
                "parameter",
                "v",
                "r",
                "d",
                "detailParameter",
                "pictureParameter",
                "note",
                "control",
            ],
            "c",
        )?;
        let xml_format = optional_usize(cell, "f", "c")?.unwrap_or(0);
        let format = self.format_ref(xml_format, "a cell")?;
        let mut mask = 0u32;
        let mut members = String::new();

        let control = cell.child("control");
        let value = cell.child("v");
        let reference = cell.child("r");
        let detail = cell.child("d");
        let detail_parameter = cell.child("detailParameter");
        let picture_parameter = cell.child("pictureParameter");
        let text = cell.child("tl");
        let formatted = cell.child("tfl");
        let parameter = cell.child("parameter");
        let note = cell.child("note");

        if let Some(control) = control {
            if control.xsi_type() != Some("xs:base64Binary") {
                bail!("a cell <control> is not xs:base64Binary");
            }
            mask |= 1;
            members.push_str(&format!(",1,{{#base64:{}}}", base64_payload(&control.text)));
        }
        if let Some(value) = value {
            mask |= 1 << 1;
            members.push(',');
            members.push_str(&typed_value(value)?);
        }
        // `<r>` is what a stored reference publishes as, from either value
        // member; all 28 of ERP УХ are the detail value's.
        let detail_record = match (detail, reference) {
            (Some(detail), None) => Some(typed_value(detail)?),
            (None, Some(reference)) => Some(format!(
                "{{\"#\",{REFERENCE_VALUE_TYPE},{{{}}}}}",
                parse_usize(&reference.text, "a cell <r>")?
            )),
            (None, None) => None,
            (Some(_), Some(_)) => bail!("a cell spells both <d> and <r>"),
        };
        if let Some(record) = detail_record {
            mask |= 1 << 2;
            members.push(',');
            members.push_str(&record);
        }
        if let Some(detail_parameter) = detail_parameter {
            mask |= 1 << 3;
            members.push(',');
            members.push_str(&quote(&detail_parameter.text));
        }
        if let Some(picture_parameter) = picture_parameter {
            mask |= 1 << 6;
            members.push(',');
            members.push_str(&quote(&picture_parameter.text));
        }
        let text_member = match (text, formatted, parameter) {
            (None, None, None) => None,
            (Some(text), None, None) => Some((localized_items(text, "tl")?, None)),
            (None, Some(text), None) => Some((
                localized_items_stripped(text, "tfl")?,
                Some(localized_items(text, "tfl")?),
            )),
            (None, None, Some(parameter)) => {
                Some((format!("{{1,1,{{\"\",{}}}}}", quote(&parameter.text)), None))
            }
            _ => bail!("a cell spells more than one of <tl>, <tfl> and <parameter>"),
        };
        if let Some((items, _)) = &text_member {
            mask |= 1 << 4;
            members.push(',');
            members.push_str(items);
        }
        if let Some(note) = note {
            mask |= 1 << 5;
            members.push(',');
            members.push_str(&self.note(note)?);
        }
        if let Some((_, formatted)) = &text_member {
            match formatted {
                None => members.push_str(",0"),
                Some(items) => members.push_str(&format!(",1,{{1,{items},1}}")),
            }
        }
        out.record(&format!("{{{mask},{format}{members}}}"));
        Ok(())
    }

    fn note(&mut self, note: &Node) -> Result<String> {
        note.only_children(
            &[
                "drawingType",
                "id",
                "formatIndex",
                "text",
                "beginRow",
                "beginRowOffset",
                "endRow",
                "endRowOffset",
                "beginColumn",
                "beginColumnOffset",
                "endColumn",
                "endColumnOffset",
                "autoSize",
                "pictureSize",
            ],
            "note",
        )?;
        if note.text_of("drawingType") != Some("Comment") {
            bail!("a cell <note> is not a Comment");
        }
        if note.text_of("id").is_some_and(|id| id != "0") {
            bail!("a cell <note> carries an id");
        }
        if note.text_of("pictureSize").is_some_and(|size| size != "Stretch") {
            bail!("a cell <note> sizes a picture");
        }
        let format = required_usize(note, "formatIndex", "note")?;
        let format = self.format_ref(format, "a note")?;
        if format == 0 {
            bail!("a cell <note> names no format");
        }
        let text = note
            .child("text")
            .ok_or_else(|| anyhow!("a cell <note> has no <text>"))?;
        let items = localized_items(text, "note")?;
        if items == "{1,0}" {
            bail!("a cell <note> has an empty text");
        }
        let auto_size = parse_bool(required_text(note, "autoSize", "note")?, "<note><autoSize>")?;
        Ok(format!(
            "{items},1,{{{{16,{format},{items},0}},6,{},{},{},{},{},{},{},{},0,{}}}",
            required_i64(note, "beginColumn", "note")?,
            required_i64(note, "beginRow", "note")?,
            required_i64(note, "beginColumnOffset", "note")?,
            required_i64(note, "beginRowOffset", "note")?,
            required_i64(note, "endColumn", "note")?,
            required_i64(note, "endRow", "note")?,
            required_i64(note, "endColumnOffset", "note")?,
            required_i64(note, "endRowOffset", "note")?,
            u8::from(auto_size)
        ))
    }

    // -- column sets -------------------------------------------------------

    fn column_sets(&mut self) -> Result<String> {
        let document = self.document;
        let mut default_set = None;
        let mut additional = Vec::new();
        for set in &document.column_sets {
            set.only_children(&["id", "formatIndex", "size", "columnsItem"], "columns")?;
            match set.text_of("id") {
                None => {
                    if default_set.replace(*set).is_some() {
                        bail!("the document spells two <columns> without an <id>");
                    }
                    if !additional.is_empty() {
                        bail!("the default <columns> follows an identified one");
                    }
                }
                Some(_) => additional.push(*set),
            }
        }
        let default_set =
            default_set.ok_or_else(|| anyhow!("the document spells no default <columns>"))?;
        let mut text = self.column_set_record(default_set, None)?;
        let height = document.height.unwrap_or(0);
        if document.vg_rows.unwrap_or(0) != height {
            bail!("<vgRows> {:?} disagrees with <height> {height}", document.vg_rows);
        }
        text.push_str(&format!(",{height},{}", additional.len()));
        let mut ids = Vec::with_capacity(additional.len());
        for set in &additional {
            let id = uuid_text(set.text_of("id").unwrap_or_default(), "a <columns><id>")?;
            text.push(',');
            text.push_str(&self.column_set_record(set, Some(&id))?);
            ids.push(id);
        }
        // Rows that name a column set, in row order.
        let mut pairs = Vec::new();
        for item in &document.rows {
            let index = required_usize(item, "index", "rowsItem")?;
            let index_to = optional_usize(item, "indexTo", "rowsItem")?.unwrap_or(index);
            let Some(row) = item.child("row") else {
                continue;
            };
            let Some(columns_id) = row.text_of("columnsID") else {
                continue;
            };
            let columns_id = uuid_text(columns_id, "a <row><columnsID>")?;
            let set = ids
                .iter()
                .position(|id| *id == columns_id)
                .ok_or_else(|| anyhow!("a row names column set {columns_id}, which the document does not spell"))?;
            for row_index in index..=index_to {
                pairs.push(format!("{row_index},{set}"));
            }
        }
        text.push_str(&format!(",{}", pairs.len()));
        for pair in pairs {
            text.push(',');
            text.push_str(&pair);
        }
        let mut out = Out::default();
        out.record(&text);
        Ok(out.text)
    }

    fn column_set_record(&mut self, set: &Node, id: Option<&str>) -> Result<String> {
        let size = required_usize(set, "size", "columns")?;
        let format = optional_usize(set, "formatIndex", "columns")?.unwrap_or(0);
        let format = self.format_ref(format, "a <columns>")?;
        let items = set.children_named("columnsItem").collect::<Vec<_>>();
        let mut record = format!("{{{size},{format},{},{}", id.unwrap_or(NIL_UUID), items.len());
        for item in items {
            item.only_children(&["index", "column"], "columnsItem")?;
            let index = required_i64(item, "index", "columnsItem")?;
            let column = item
                .child("column")
                .ok_or_else(|| anyhow!("<columnsItem> {index} has no <column>"))?;
            column.only_children(&["formatIndex"], "column")?;
            let format = optional_usize(column, "formatIndex", "column")?.unwrap_or(0);
            let format = self.format_ref(format, "a <column>")?;
            record.push_str(&format!(",{index},{format}"));
        }
        record.push('}');
        Ok(record)
    }

    // -- drawings ----------------------------------------------------------

    fn drawings(&mut self) -> Result<String> {
        let document = self.document;
        let mut last_id = 0usize;
        let mut records = Vec::with_capacity(document.drawings.len());
        for drawing in &document.drawings {
            let (record, id) = self.drawing(drawing)?;
            last_id = last_id.max(id);
            records.push(record);
        }
        let mut out = Out::default();
        out.raw(&format!("{last_id},{}", records.len()));
        for record in records {
            out.comma();
            out.record(&record);
        }
        Ok(out.text)
    }

    fn drawing(&mut self, drawing: &Node) -> Result<(String, usize)> {
        drawing.only_children(
            &[
                "drawingType",
                "id",
                "formatIndex",
                "text",
                "parameter",
                "value",
                "detailParameter",
                "beginRow",
                "beginRowOffset",
                "endRow",
                "endRowOffset",
                "beginColumn",
                "beginColumnOffset",
                "endColumn",
                "endColumnOffset",
                "autoSize",
                "pictureSize",
                "zOrder",
                "pictureIndex",
                "object",
            ],
            "drawing",
        )?;
        let kind = required_text(drawing, "drawingType", "drawing")?;
        let id = required_usize(drawing, "id", "drawing")?;
        let format = required_usize(drawing, "formatIndex", "drawing")?;
        let format = self.format_ref(format, "a <drawing>")?;
        let mut mask = 0u32;
        let mut members = String::new();
        if let Some(value) = drawing.child("value") {
            if value.xsi_type() != Some("xs:string") {
                bail!("a drawing <value> is not xs:string");
            }
            mask |= 1 << 1;
            members.push_str(&format!(",{{\"S\",{}}}", quote(&value.text)));
        }
        if let Some(detail_parameter) = drawing.child("detailParameter") {
            mask |= 1 << 3;
            members.push(',');
            members.push_str(&quote(&detail_parameter.text));
        }
        match (drawing.child("text"), drawing.child("parameter")) {
            (Some(text), None) => {
                mask |= 1 << 4;
                members.push(',');
                members.push_str(&localized_items(text, "drawing text")?);
                members.push_str(",0");
            }
            (None, Some(parameter)) => {
                mask |= 1 << 4;
                members.push_str(&format!(",{{1,1,{{\"\",{}}}}},0", quote(&parameter.text)));
            }
            (None, None) => {}
            (Some(_), Some(_)) => bail!("a drawing spells both <text> and <parameter>"),
        }
        let geometry = format!(
            "{},{},{},{},{},{},{},{}",
            required_i64(drawing, "beginColumn", "drawing")?,
            required_i64(drawing, "beginRow", "drawing")?,
            required_i64(drawing, "beginColumnOffset", "drawing")?,
            required_i64(drawing, "beginRowOffset", "drawing")?,
            required_i64(drawing, "endColumn", "drawing")?,
            required_i64(drawing, "endRow", "drawing")?,
            required_i64(drawing, "endColumnOffset", "drawing")?,
            required_i64(drawing, "endRowOffset", "drawing")?,
        );
        let auto_size = u8::from(parse_bool(
            required_text(drawing, "autoSize", "drawing")?,
            "<drawing><autoSize>",
        )?);
        let picture_size = required_text(drawing, "pictureSize", "drawing")?;
        let head = format!("{{{mask},{format}{members}}}");
        let record = match kind {
            "Line" | "Rectangle" | "Text" => {
                if picture_size != "Stretch" || drawing.child("pictureIndex").is_some() {
                    bail!("a {kind} drawing spells a picture");
                }
                let code = match kind {
                    "Line" => 1,
                    "Rectangle" => 2,
                    _ => 3,
                };
                format!("{{{head},{code},{geometry},{id},{auto_size}}}")
            }
            "Picture" => {
                let picture = required_usize(drawing, "pictureIndex", "drawing")?;
                let picture = if picture == 0 {
                    0
                } else {
                    self.picture_ref(picture - 1)? + 1
                };
                let size = match picture_size {
                    "RealSize" => 0,
                    "Stretch" => 1,
                    "Proportionally" => 2,
                    "Tile" => 3,
                    "AutoSize" => 4,
                    "ByFontSize" => 7,
                    other => bail!("a picture drawing's <pictureSize> {other} has no stored code"),
                };
                format!("{{{head},5,{geometry},{id},{picture},{size},{auto_size}}}")
            }
            "Chart" | "GanttChart" => {
                if picture_size != "Stretch"
                    || auto_size != 0
                    || drawing.child("pictureIndex").is_some()
                {
                    bail!("a {kind} drawing spells a picture");
                }
                let object = drawing
                    .child("object")
                    .ok_or_else(|| anyhow!("a {kind} drawing has no <object>"))?;
                let (type_uuid, chart) = self.chart(object, kind == "GanttChart")?;
                format!("{{{head},10,{geometry},{id},{type_uuid},{chart},0}}")
            }
            other => bail!("a {other} drawing has no writer"),
        };
        if kind != "Chart" && kind != "GanttChart" && drawing.child("object").is_some() {
            bail!("a {kind} drawing carries an <object>");
        }
        Ok((record, id))
    }

    /// A chart drawing's object, written by the form chart codec: the same
    /// serialization a `Chart`/`GanttChart` form attribute stores, whose
    /// `<Settings>` spells the chart one namespace level deeper (`d4p1`
    /// where the template's `<object>` sits at `d3p1`). The template keeps
    /// the chart's own members -- `{11},{74,…}` for a chart, `{19,…}` for a
    /// Gantt chart -- in one more pair of braces.
    fn chart(&self, object: &Node, gantt: bool) -> Result<(&'static str, String)> {
        const CHART_TYPE: &str = "a8b97779-1a4b-4059-b09c-807f86d2a461";
        const GANTT_CHART_TYPE: &str = "e5fdc112-5c84-4a16-9728-72b85692b6e2";
        const CHART_VALUE: &str = "{0,1,\"Chart\",{\"#\",3543ef08-3316-4f7e-9447-0cd0a1cbf1d5,";
        const GANTT_CHART_VALUE: &str =
            "{0,1,\"GanttChart\",{\"#\",3a6e63bf-16aa-42eb-b48c-2fff9670ad2f,";
        let expected = if gantt { "d3p1:GanttChart" } else { "d3p1:Chart" };
        if object.xsi_type() != Some(expected) {
            bail!("a chart drawing's <object> is not {expected}");
        }
        let raw = self
            .document
            .source_text
            .get(object.span.0..object.span.1)
            .ok_or_else(|| anyhow!("a chart drawing's <object> has no source span"))?;
        let settings = deepen_generated_prefixes(raw)?;
        // The chart codec spells a configuration style colour by its uuid
        // (`0:<uuid>`, what it stores as `{3,3,{0,<uuid>}}`); the name is
        // resolved here, against the configuration the loader reads.
        let settings = self.resolve_chart_style_items(&settings)?;
        let settings = settings
            .strip_prefix("<object ")
            .and_then(|rest| rest.strip_suffix("</object>"))
            .map(|body| format!("<Settings {body}</Settings>"))
            .ok_or_else(|| anyhow!("a chart drawing's <object> is not one element"))?;
        let (value, head) = if gantt {
            (
                crate::compiler::bodies::form_chart::gantt_chart_value(
                    &settings,
                    crate::compiler::bodies::form_chart::ChartHost::SpreadsheetTemplate,
                )?,
                GANTT_CHART_VALUE,
            )
        } else {
            (
                crate::compiler::bodies::form_chart::chart_value(
                    &settings,
                    crate::compiler::bodies::form_chart::ChartHost::SpreadsheetTemplate,
                )?,
                CHART_VALUE,
            )
        };
        let members = value
            .strip_prefix(head)
            .and_then(|rest| rest.strip_suffix("}}"))
            .ok_or_else(|| anyhow!("the chart codec wrote an unexpected value"))?;
        let field = format!("{{{members}}}");
        // The chart has to read back into the very element it came from,
        // through the template exporter's own chart reader.
        let object_refs = match self.source {
            Some(source) => source.moxel_object_refs()?,
            None => std::collections::BTreeMap::new(),
        };
        let rendered = crate::mssql_dump::render_moxel_chart_object_xml(&field, gantt, &object_refs)
            .ok_or_else(|| anyhow!("the exporter cannot read back the chart the writer built"))?;
        let significant = |text: &str| {
            text.lines()
                .map(|line| line.trim_start_matches([' ', '\t']).trim_end_matches('\r').to_string())
                .filter(|line| !line.is_empty())
                .collect::<Vec<_>>()
        };
        let expected = significant(raw);
        let actual = significant(&rendered);
        if expected != actual {
            let at = expected
                .iter()
                .zip(&actual)
                .position(|(left, right)| left != right)
                .unwrap_or_else(|| expected.len().min(actual.len()));
            bail!(
                "the chart does not export back to its own XML: line {} would read {:?} where the source reads {:?}",
                at + 1,
                actual.get(at).map(String::as_str).unwrap_or("<end>"),
                expected.get(at).map(String::as_str).unwrap_or("<end>"),
            );
        }
        Ok((if gantt { GANTT_CHART_TYPE } else { CHART_TYPE }, field))
    }

    /// Every `style:<name>` element text naming a configuration style item,
    /// rewritten as `0:<uuid>`.
    fn resolve_chart_style_items(&self, xml: &str) -> Result<String> {
        const HEAD: &str = ">style:";
        let Some(source) = self.source else {
            return Ok(xml.to_string());
        };
        let mut out = String::with_capacity(xml.len());
        let mut rest = xml;
        while let Some(at) = rest.find(HEAD) {
            out.push_str(&rest[..=at]);
            let tail = &rest[at + 1..];
            let end = tail
                .find('<')
                .ok_or_else(|| anyhow!("a chart colour is not closed"))?;
            let name = &tail["style:".len()..end];
            match source.resolve_style_item_uuid(&format!("StyleItem.{name}")) {
                Ok(uuid) if platform_style_color(name).is_none() => {
                    out.push_str(&format!("0:{}", uuid.to_ascii_lowercase()));
                }
                _ => out.push_str(&tail[..end]),
            }
            rest = &tail[end..];
        }
        out.push_str(rest);
        Ok(out)
    }

    fn picture_ref(&mut self, xml_index: usize) -> Result<usize> {
        if xml_index >= self.document.pictures.len() {
            bail!(
                "a reference names picture {xml_index}, and the document spells {}",
                self.document.pictures.len()
            );
        }
        Ok(self.pictures.intern(xml_index))
    }

    // -- groups ------------------------------------------------------------

    fn groups(&mut self) -> Result<String> {
        let document = self.document;
        let row_groups = group_records(&document.row_groups, "vg")?;
        let column_groups = group_records(&document.column_groups, "hg")?;
        let levels = document
            .row_groups
            .iter()
            .enumerate()
            .map(|(index, _)| row_groups.levels[index] + 1)
            .max()
            .unwrap_or(0);
        match document.vg_levels {
            Some(declared) if declared != levels => {
                bail!("<vgLevels> {declared} disagrees with the groups' nesting depth {levels}")
            }
            None if levels > 0 => bail!("the document groups rows and spells no <vgLevels>"),
            _ => {}
        }
        let mut out = Out::default();
        out.raw(&row_groups.records.len().to_string());
        for record in &row_groups.records {
            out.comma();
            out.record(record);
            out.raw(",-1");
        }
        out.comma();
        out.raw(&column_groups.records.len().to_string());
        for record in &column_groups.records {
            out.comma();
            out.record(record);
            out.raw(",-1");
        }
        out.raw(",0,0");
        Ok(out.text)
    }

    // -- merges ------------------------------------------------------------

    fn merges(&mut self) -> Result<String> {
        let document = self.document;
        // (row, column, record, list)
        let mut regions: Vec<(i64, i64, String, usize)> = Vec::new();
        for (nodes, kind, tag) in [
            (&document.merges, 0, "merge"),
            (&document.vertical_unmerges, 2, "verticalUnmerge"),
            (&document.horizontal_unmerges, 1, "horizontalUnmerge"),
        ] {
            for node in nodes.iter() {
                node.only_children(&["r", "c", "w", "h", "columnsID"], tag)?;
                let row = required_i64(node, "r", tag)?;
                let column = required_i64(node, "c", tag)?;
                let width = node
                    .text_of("w")
                    .map(|text| parse_i64(text, "<w>"))
                    .transpose()?
                    .unwrap_or(0);
                let height = node
                    .text_of("h")
                    .map(|text| parse_i64(text, "<h>"))
                    .transpose()?
                    .unwrap_or(0);
                let columns_id = node
                    .text_of("columnsID")
                    .map(|text| uuid_text(text, "a merge's <columnsID>"))
                    .transpose()?;
                let (list, record) = match (row < 0, column < 0) {
                    (false, false) => {
                        if columns_id.is_some() {
                            bail!("a cell-range <{tag}> names a column set");
                        }
                        (
                            0,
                            format!(
                                "{{{column},{row},{},{},{kind}}}",
                                column + width,
                                row + height
                            ),
                        )
                    }
                    (false, true) => {
                        if columns_id.is_some() || kind != 0 {
                            bail!("a row-range <{tag}> spells what no stored row merge carries");
                        }
                        (1, format!("{{{column},{row},{},{},0}}", column + width, row + height))
                    }
                    (true, false) => {
                        if kind != 0 {
                            bail!("a column-range <{tag}> has no stored form");
                        }
                        (
                            2,
                            format!(
                                "{{{column},{row},{},{},0,{}}}",
                                column + width,
                                row + height,
                                columns_id.as_deref().unwrap_or(NIL_UUID)
                            ),
                        )
                    }
                    (true, true) => bail!("a <{tag}> spans neither a row nor a column"),
                };
                regions.push((row, column, record, list));
            }
        }
        // Each list is kept sorted by its top-left corner, row first; equal
        // corners keep the XML order (merges, then vertical unmerges, then
        // horizontal ones).
        regions.sort_by_key(|(row, column, _, _)| (*row, *column));
        let mut out = Out::default();
        for list in 0..3 {
            if list > 0 {
                out.comma();
            }
            let entries = regions
                .iter()
                .filter(|(_, _, _, owner)| *owner == list)
                .collect::<Vec<_>>();
            let mut record = format!("{{{}", entries.len());
            for (_, _, entry, _) in entries {
                record.push(',');
                record.push_str(entry);
            }
            record.push('}');
            out.record(&record);
        }
        Ok(out.text)
    }

    // -- named items -------------------------------------------------------

    fn named_items(&mut self) -> Result<String> {
        let document = self.document;
        let mut record = format!("{{{}", document.named_items.len());
        for item in &document.named_items {
            let name = required_text(item, "name", "namedItem")?;
            match item.xsi_type() {
                Some("NamedItemCells") => {
                    item.only_children(&["name", "area"], "namedItem")?;
                    let area = item
                        .child("area")
                        .ok_or_else(|| anyhow!("<namedItem> {name} has no <area>"))?;
                    record.push_str(&format!(",{},{{1,{},0}}", quote(name), area_record(area)?));
                }
                Some("NamedItemDrawing") => {
                    item.only_children(&["name", "drawingID"], "namedItem")?;
                    let id = required_usize(item, "drawingID", "namedItem")?;
                    record.push_str(&format!(",{},{{2,{id},0}}", quote(name)));
                }
                other => bail!("a <namedItem> of type {other:?} has no writer"),
            }
        }
        record.push('}');
        let mut out = Out::default();
        out.record(&record);
        Ok(out.text)
    }

    // -- print settings ----------------------------------------------------

    fn print_properties(&mut self) -> Result<String> {
        let Some(settings) = self.document.print_settings else {
            let mut out = Out::default();
            out.record(
                "{{0,6,6,{\"N\",1000},7,{\"N\",1000},8,{\"N\",1000},9,{\"N\",1000},10,{\"N\",1000},11,{\"N\",1000}}}",
            );
            return Ok(out.text);
        };
        let mut entries: Vec<(usize, String)> = Vec::new();
        for member in &settings.children {
            let value = member.text.as_str();
            let what = format!("<printSettings><{}>", member.name);
            let (key, stored) = match member.name.as_str() {
                "paper" => (0, number(value, &what)?),
                "pageOrientation" => (
                    1,
                    match value {
                        "Portrait" => "{\"N\",1}".to_string(),
                        "Landscape" => "{\"N\",2}".to_string(),
                        other => bail!("{what} {other} has no stored code"),
                    },
                ),
                "scale" => (2, number(value, &what)?),
                "collate" => (3, flag(value, &what)?),
                "copies" => (4, number(value, &what)?),
                "perPage" => (5, number(value, &what)?),
                "topMargin" => (6, number(value, &what)?),
                "leftMargin" => (7, number(value, &what)?),
                "bottomMargin" => (8, number(value, &what)?),
                "rightMargin" => (9, number(value, &what)?),
                "headerSize" => (10, number(value, &what)?),
                "footerSize" => (11, number(value, &what)?),
                "fitToPage" => (12, flag(value, &what)?),
                "blackAndWhite" => (13, flag(value, &what)?),
                "printerName" => (14, format!("{{\"S\",{}}}", quote(value))),
                "paperSource" => (15, number(value, &what)?),
                "pageWidth" => (16, decimal(value, &what)?),
                "pageHeight" => (17, decimal(value, &what)?),
                "duplexType" => (
                    19,
                    match value {
                        "None" => "{\"N\",1}".to_string(),
                        "FlipPagesLeft" => "{\"N\",2}".to_string(),
                        "UsePrinterSettings" => "{\"N\",4}".to_string(),
                        other => bail!("{what} {other} has no stored code"),
                    },
                ),
                "pagePlacementAlternation" => (
                    20,
                    match value {
                        "Auto" => "{\"N\",0}".to_string(),
                        other => bail!("{what} {other} has no stored code"),
                    },
                ),
                "firstPageNumber" => (21, number(value, &what)?),
                other => bail!("<printSettings><{other}> has no writer"),
            };
            if entries.iter().any(|(existing, _)| *existing == key) {
                bail!("{what} is spelled twice");
            }
            entries.push((key, stored));
        }
        entries.sort_by_key(|(key, _)| *key);
        let mut record = format!("{{{{0,{}", entries.len());
        for (key, stored) in entries {
            record.push_str(&format!(",{key},{stored}"));
        }
        record.push_str("}}");
        let mut out = Out::default();
        out.record(&record);
        Ok(out.text)
    }

    fn print_area(&mut self) -> Result<String> {
        let mut out = Out::default();
        match self.document.print_area {
            None => out.record(&format!("{{0,-1,-1,-1,-1,{NIL_UUID}}}")),
            Some(area) => out.record(&area_record(area)?),
        }
        Ok(out.text)
    }

    /// The ten scalars behind the print area: the repeated rows (none on
    /// either corpus), two zeros and the group/header colour slots.
    fn scalars(&mut self) -> Result<String> {
        let defaults = [
            "style:FormBackColor",
            "style:FormTextColor",
            "style:FormBackColor",
            "style:FormTextColor",
        ];
        let mut slots = Vec::with_capacity(4);
        for (role, default) in defaults.iter().enumerate() {
            let slot = match self.document.group_colors[role] {
                Some(node) => self.color_slot(node, &node.text)?,
                None => self.palette_slot_for_spelling(default, None)?,
            };
            slots.push(slot.to_string());
        }
        Ok(format!("0,0,0,0,0,0,{}", slots.join(",")))
    }

    // -- formats -----------------------------------------------------------

    fn format_table(&mut self) -> Result<String> {
        // Formats nothing names follow the named ones in XML order; the default
        // format is written ahead of the table and is not one of them.
        for index in 1..=self.document.formats.len() {
            if Some(index) != self.document.default_format_index {
                self.formats.intern(index);
            }
        }
        let mut out = Out::default();
        let mut position = 0usize;
        let mut records = Vec::new();
        while position < self.formats.order.len() {
            let xml_index = self.formats.order[position];
            let node = self.document.formats[xml_index - 1];
            // The exporter reads a drawing's format as a drawing format only
            // while it carries no width.
            let drawing =
                self.drawing_formats.contains(&xml_index) && node.child("width").is_none();
            records.push(self.format_record(node, drawing)?);
            position += 1;
        }
        out.raw(&records.len().to_string());
        for record in records {
            out.comma();
            out.record(&record);
        }
        Ok(out.text)
    }

    /// One format record: `{mask, members in bit order}`.
    fn format_record(&mut self, node: &Node, drawing: bool) -> Result<String> {
        let mut members: Vec<(u32, String)> = Vec::new();
        let mut put = |bit: u32, value: String| members.push((bit, value));
        for member in &node.children {
            let text = member.text.as_str();
            let what = format!("<format><{}>", member.name);
            match member.name.as_str() {
                "font" => {
                    let index = parse_usize(text, &what)?;
                    if index >= self.document.fonts.len() {
                        bail!("{what} names font {index} of {}", self.document.fonts.len());
                    }
                    put(0, self.fonts.intern(index).to_string());
                }
                // A drawing's format spends members 1, 3 and 4 on its own
                // border, the packed `drawingHave*Border` flags and `print`.
                "border" if !drawing => {
                    let line = self.line_ref(text, &what)?;
                    for bit in 1..=4 {
                        put(bit, line.to_string());
                    }
                }
                "leftBorder" if !drawing => put(1, self.line_ref(text, &what)?.to_string()),
                "topBorder" => put(2, self.line_ref(text, &what)?.to_string()),
                "rightBorder" if !drawing => put(3, self.line_ref(text, &what)?.to_string()),
                "bottomBorder" if !drawing => put(4, self.line_ref(text, &what)?.to_string()),
                "drawingBorder" if drawing => put(1, self.line_ref(text, &what)?.to_string()),
                "drawingHaveLeftBorder"
                | "drawingHaveTopBorder"
                | "drawingHaveRightBorder"
                | "drawingHaveBottomBorder"
                    if drawing => {}
                "print" if drawing => {
                    put(4, if parse_bool(text, &what)? { "0" } else { "1" }.to_string())
                }
                "borderColor" => put(5, self.color_slot(member, text)?.to_string()),
                "height" => put(6, parse_i64(text, &what)?.to_string()),
                "width" => put(7, parse_usize(text, &what)?.to_string()),
                "horizontalAlignment" => put(
                    8,
                    match text {
                        "Left" => "0",
                        "Right" => "2",
                        "Justify" => "4",
                        "Auto" => "5",
                        "Center" => "6",
                        other => bail!("{what} {other} has no stored code"),
                    }
                    .to_string(),
                ),
                "verticalAlignment" => put(
                    9,
                    match text {
                        "Top" => "0",
                        "Center" => "24",
                        "Bottom" => "8",
                        other => bail!("{what} {other} has no stored code"),
                    }
                    .to_string(),
                ),
                "textColor" => put(10, self.color_slot(member, text)?.to_string()),
                "backColor" => put(11, self.color_slot(member, text)?.to_string()),
                "pattern" => put(12, pattern_code(text, &what)?.to_string()),
                "patternColor" => put(13, self.color_slot(member, text)?.to_string()),
                "textPlacement" => put(
                    14,
                    match text {
                        "Auto" => "0",
                        "Cut" => "1",
                        "Block" => "2",
                        "Wrap" => "3",
                        other => bail!("{what} {other} has no stored code"),
                    }
                    .to_string(),
                ),
                "fillType" => put(
                    15,
                    match text {
                        "Text" => "0",
                        "Parameter" => "1",
                        "Template" => "2",
                        other => bail!("{what} {other} has no stored code"),
                    }
                    .to_string(),
                ),
                "protection" => put(16, if parse_bool(text, &what)? { "0" } else { "1" }.to_string()),
                "hidden" => put(17, u8::from(parse_bool(text, &what)?).to_string()),
                "textOrientation" => put(18, parse_usize(text, &what)?.to_string()),
                "detailsUse" => put(
                    19,
                    match text {
                        "Cell" => "0",
                        "Row" => "1",
                        "WithoutProcessing" => "2",
                        other => bail!("{what} {other} has no stored code"),
                    }
                    .to_string(),
                ),
                "bySelectedColumns" => put(20, u8::from(parse_bool(text, &what)?).to_string()),
                "markNegatives" => put(21, u8::from(parse_bool(text, &what)?).to_string()),
                "containsValue" => put(22, u8::from(parse_bool(text, &what)?).to_string()),
                "valueType" => {
                    let entry = self.value_type_entry(member)?;
                    put(23, self.value_types.intern(entry).to_string());
                }
                "format" => {
                    let entry = localized_items(member, "format")?;
                    put(24, self.number_formats.intern(entry).to_string());
                }
                "controlType" => {
                    let entry = uuid_text(text, &what)?;
                    put(25, self.control_types.intern(entry).to_string());
                }
                "hyperLink" => put(26, u8::from(parse_bool(text, &what)?).to_string()),
                "autoMarkIncomplete" => put(28, u8::from(parse_bool(text, &what)?).to_string()),
                "markIncomplete" => {
                    if parse_bool(text, &what)? {
                        bail!("{what} true has no stored code");
                    }
                    put(29, "0".to_string());
                }
                "indent" => put(30, parse_usize(text, &what)?.to_string()),
                "autoIndent" => put(31, parse_usize(text, &what)?.to_string()),
                "editFormat" => {
                    let entry = localized_items(member, "editFormat")?;
                    put(32, self.number_formats.intern(entry).to_string());
                }
                "columnSizeChange" => put(
                    33,
                    match text {
                        "Normal" => "0",
                        "QuickChange" => "1",
                        other => bail!("{what} {other} has no stored code"),
                    }
                    .to_string(),
                ),
                "mask" => {
                    let entry = localized_items(member, "mask")?;
                    put(34, self.masks.intern(entry).to_string());
                }
                "picIndex" => {
                    let index = parse_usize(text, &what)?;
                    let stored = if index == 0 {
                        0
                    } else {
                        self.picture_ref(index - 1)? + 1
                    };
                    put(35, stored.to_string());
                }
                "picHorizontalAlignment" => put(
                    36,
                    match text {
                        "Left" => "0",
                        "Right" => "2",
                        "Auto" => "5",
                        "Center" => "6",
                        other => bail!("{what} {other} has no stored code"),
                    }
                    .to_string(),
                ),
                "picVerticalAlignment" => put(
                    37,
                    match text {
                        "Top" => "0",
                        "Center" => "24",
                        "Bottom" => "8",
                        other => bail!("{what} {other} has no stored code"),
                    }
                    .to_string(),
                ),
                "pictureSizeMode" => put(
                    38,
                    match text {
                        "RealSize" => "0",
                        "Stretch" => "1",
                        "Proportionally" => "2",
                        "AutoSize" => "4",
                        "ByFontSize" => "7",
                        other => bail!("{what} {other} has no stored code"),
                    }
                    .to_string(),
                ),
                "textPosition" => put(
                    39,
                    match text {
                        "Left" => "0",
                        "Right" => "1",
                        "Top" => "2",
                        "Bottom" => "3",
                        "OnTop" => "4",
                        "Auto" => "5",
                        other => bail!("{what} {other} has no stored code"),
                    }
                    .to_string(),
                ),
                "autoWidthCalculation" => put(40, u8::from(parse_bool(text, &what)?).to_string()),
                "widthWeightFactor" => put(41, parse_usize(text, &what)?.to_string()),
                "leftMargin" => put(42, parse_usize(text, &what)?.to_string()),
                "topMargin" => put(43, parse_usize(text, &what)?.to_string()),
                "rightMargin" => put(44, parse_usize(text, &what)?.to_string()),
                "bottomMargin" => put(45, parse_usize(text, &what)?.to_string()),
                other => bail!(
                    "<format><{other}> has no writer{}",
                    if drawing { " on a drawing format" } else { "" }
                ),
            }
        }
        if drawing {
            // The four packed flags share member 3.
            let flags = [
                "drawingHaveLeftBorder",
                "drawingHaveTopBorder",
                "drawingHaveRightBorder",
                "drawingHaveBottomBorder",
            ];
            let mut packed = None::<u32>;
            for (bit, tag) in flags.iter().enumerate() {
                if let Some(flag) = node.child(tag) {
                    let value = parse_bool(&flag.text, tag)?;
                    let current = packed.get_or_insert(0);
                    if value {
                        *current |= 1 << bit;
                    }
                }
            }
            if let Some(packed) = packed {
                members.push((3, packed.to_string()));
            }
        }
        members.sort_by_key(|(bit, _)| *bit);
        for pair in members.windows(2) {
            if pair[0].0 == pair[1].0 {
                bail!("a <format> spells member {} twice", pair[0].0);
            }
        }
        let mask = members
            .iter()
            .fold(0u64, |mask, (bit, _)| mask | (1u64 << bit));
        let mut record = format!("{{{mask}");
        for (_, value) in members {
            record.push(',');
            record.push_str(&value);
        }
        record.push('}');
        Ok(record)
    }

    fn line_ref(&mut self, text: &str, what: &str) -> Result<usize> {
        let index = parse_usize(text, what)?;
        if index >= self.document.lines.len() {
            bail!("{what} names line {index} of {}", self.document.lines.len());
        }
        Ok(self.lines.intern(index))
    }

    fn value_type_entry(&mut self, node: &Node) -> Result<String> {
        node.only_children(
            &[
                "Type",
                "TypeId",
                "NumberQualifiers",
                "StringQualifiers",
                "DateQualifiers",
            ],
            "valueType",
        )?;
        let mut descriptors = Vec::new();
        let number = node.child("NumberQualifiers");
        let string = node.child("StringQualifiers");
        let date = node.child("DateQualifiers");
        for child in &node.children {
            match child.name.as_str() {
                "Type" => {
                    let spelled = child.text.trim();
                    let descriptor = match spelled {
                        "xs:boolean" => "{\"B\"}".to_string(),
                        // The unqualified descriptor is the qualifiers' own
                        // default: `Length 0`/`Variable`, `Digits 0`/`0`/`Any`.
                        "xs:string" => match string {
                            Some(qualifiers) => {
                                let length = required_usize(qualifiers, "Length", "StringQualifiers")?;
                                let allowed = match required_text(qualifiers, "AllowedLength", "StringQualifiers")? {
                                    "Fixed" => 0,
                                    "Variable" => 1,
                                    other => bail!("<AllowedLength> {other} has no stored code"),
                                };
                                if length == 0 && allowed == 1 {
                                    "{\"S\"}".to_string()
                                } else {
                                    format!("{{\"S\",{length},{allowed}}}")
                                }
                            }
                            None => "{\"S\"}".to_string(),
                        },
                        "xs:decimal" => match number {
                            Some(qualifiers) => {
                                let digits = required_usize(qualifiers, "Digits", "NumberQualifiers")?;
                                let fraction = required_usize(qualifiers, "FractionDigits", "NumberQualifiers")?;
                                let sign = match required_text(qualifiers, "AllowedSign", "NumberQualifiers")? {
                                    "Any" => 0,
                                    "Nonnegative" => 1,
                                    other => bail!("<AllowedSign> {other} has no stored code"),
                                };
                                if digits == 0 && fraction == 0 && sign == 0 {
                                    "{\"N\"}".to_string()
                                } else {
                                    format!("{{\"N\",{digits},{fraction},{sign}}}")
                                }
                            }
                            None => "{\"N\"}".to_string(),
                        },
                        "xs:dateTime" => match date {
                            Some(qualifiers) => match required_text(qualifiers, "DateFractions", "DateQualifiers")? {
                                "DateTime" => "{\"D\"}".to_string(),
                                "Date" => "{\"D\",\"D\"}".to_string(),
                                "Time" => "{\"D\",\"T\"}".to_string(),
                                other => bail!("<DateFractions> {other} has no stored code"),
                            },
                            None => "{\"D\"}".to_string(),
                        },
                        other => {
                            let (prefix, name) = other
                                .split_once(':')
                                .ok_or_else(|| anyhow!("<v8:Type> {other} has no writer"))?;
                            if child.own_namespace(prefix) != Some(CURRENT_CONFIG_NS) {
                                bail!("<v8:Type> {other} is not a configuration type");
                            }
                            let source = self
                                .source
                                .ok_or_else(|| anyhow!("<v8:Type> {other} needs the configuration"))?;
                            let type_id = source.resolve_metadata_type_id(&format!("cfg:{name}"))?;
                            format!("{{\"#\",{}}}", type_id.to_ascii_lowercase())
                        }
                    };
                    descriptors.push(descriptor);
                }
                "TypeId" => {
                    descriptors.push(format!("{{\"#\",{}}}", uuid_text(&child.text, "<v8:TypeId>")?));
                }
                _ => {}
            }
        }
        let mut entry = "{\"Pattern\"".to_string();
        for descriptor in descriptors {
            entry.push(',');
            entry.push_str(&descriptor);
        }
        entry.push('}');
        Ok(entry)
    }

    // -- colours -----------------------------------------------------------

    fn color_slot(&mut self, node: &Node, text: &str) -> Result<usize> {
        let spelled = text.trim();
        let namespace = spelled
            .split_once(':')
            .and_then(|(prefix, _)| node.own_namespace(prefix).map(str::to_string));
        self.palette_slot_for_spelling(spelled, namespace.as_deref())
    }

    /// A report colour's slot stores the same value an explicit colour of that
    /// value stores, and only its override tells the two apart: they are two
    /// palette entries, never one.
    fn palette_slot_for_spelling(&mut self, spelled: &str, namespace: Option<&str>) -> Result<usize> {
        let (slot, overridden) = self.palette_entry(spelled, namespace)?;
        let key = match &overridden {
            Some(style) => format!("{slot}|{style}"),
            None => slot.clone(),
        };
        if let Some(position) = self.palette.positions.get(&key) {
            return Ok(*position);
        }
        let position = self.palette.entries.len();
        self.palette.positions.insert(key, position);
        self.palette.entries.push(slot);
        if let Some(style) = overridden {
            self.overrides.push((position, style));
        }
        Ok(position)
    }

    /// The palette slot a colour spelling stores, and the style record a
    /// report colour overrides that slot with.
    fn palette_entry(&self, spelled: &str, namespace: Option<&str>) -> Result<(String, Option<String>)> {
        if spelled == "auto" {
            return Ok(("{3,4,{0}}".to_string(), None));
        }
        if let Some(hex) = spelled.strip_prefix('#') {
            if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                bail!("colour {spelled} is not #RRGGBB");
            }
            let value = u32::from_str_radix(hex, 16)?;
            let (red, green, blue) = (value >> 16 & 0xff, value >> 8 & 0xff, value & 0xff);
            return Ok((format!("{{3,0,{{{}}}}}", red | green << 8 | blue << 16), None));
        }
        if let Some(name) = spelled.strip_prefix("style:") {
            if let Some((code, rgb)) = report_style_color(name) {
                return Ok((
                    format!("{{3,0,{{{rgb}}}}}"),
                    Some(format!("{{3,3,{{{code}}}}}")),
                ));
            }
            if let Some(code) = platform_style_color(name) {
                return Ok((format!("{{3,3,{{{code}}}}}"), None));
            }
            let source = self
                .source
                .ok_or_else(|| anyhow!("style colour {spelled} needs the configuration"))?;
            let uuid = source.resolve_style_item_uuid(&format!("StyleItem.{name}"))?;
            return Ok((format!("{{3,3,{{0,{}}}}}", uuid.to_ascii_lowercase()), None));
        }
        let Some((_, name)) = spelled.split_once(':') else {
            bail!("colour {spelled} has no writer");
        };
        match namespace {
            Some(WEB_COLORS_NS) => {
                let code = web_color_code(name).ok_or_else(|| anyhow!("web colour {name} has no stored code"))?;
                Ok((format!("{{3,2,{{{code}}}}}"), None))
            }
            Some(WINDOWS_COLORS_NS) => {
                let code = windows_color_code(name)
                    .ok_or_else(|| anyhow!("windows colour {name} has no stored code"))?;
                Ok((format!("{{3,1,{{{code}}}}}"), None))
            }
            _ => bail!("colour {spelled} names an unknown namespace"),
        }
    }

    fn palette_table(&self, out: &mut Out) {
        out.raw(&self.palette.entries.len().to_string());
        for entry in &self.palette.entries {
            out.comma();
            out.record(entry);
        }
    }

    fn override_table(&self, out: &mut Out) {
        let mut record = format!("{{{}", self.overrides.len());
        for (slot, style) in &self.overrides {
            record.push_str(&format!(",{slot},{style}"));
        }
        record.push('}');
        out.record(&record);
    }

    fn content_table(&self, out: &mut Out, table: ContentTable) {
        let pool = match table {
            ContentTable::NumberFormats => &self.number_formats,
            ContentTable::ValueTypes => &self.value_types,
            ContentTable::ControlTypes => &self.control_types,
            ContentTable::Masks => &self.masks,
        };
        out.raw(&pool.entries.len().to_string());
        for entry in &pool.entries {
            out.comma();
            out.record(entry);
        }
    }

    // -- fonts -------------------------------------------------------------

    fn font_table(&mut self) -> Result<String> {
        let mut out = Out::default();
        out.raw(&self.fonts.order.len().to_string());
        for position in 0..self.fonts.order.len() {
            let node = self.document.fonts[self.fonts.order[position]];
            out.comma();
            let record = self.font_record(node)?;
            out.record(&record);
        }
        Ok(out.text)
    }

    fn font_record(&self, node: &Node) -> Result<String> {
        node.only_attributes(
            &[
                "ref",
                "faceName",
                "height",
                "bold",
                "italic",
                "underline",
                "strikeout",
                "kind",
                "scale",
            ],
            "font",
        )?;
        if !node.children.is_empty() {
            bail!("a <font> has children");
        }
        let kind = node
            .attribute("kind")
            .ok_or_else(|| anyhow!("a <font> has no kind"))?;
        let height = node
            .attribute("height")
            .map(|height| font_height(height))
            .transpose()?;
        let weight = node
            .attribute("bold")
            .map(|bold| parse_bool(bold, "font bold").map(|bold| if bold { 700 } else { 400 }))
            .transpose()?;
        let flag = |name: &str| -> Result<Option<u8>> {
            node.attribute(name)
                .map(|value| parse_bool(value, name).map(u8::from))
                .transpose()
        };
        let italic = flag("italic")?;
        let underline = flag("underline")?;
        let strikeout = flag("strikeout")?;
        let scale = node
            .attribute("scale")
            .map(|scale| parse_usize(scale, "font scale"))
            .transpose()?;
        let face = node.attribute("faceName");
        match kind {
            "Absolute" => {
                if node.attribute("ref").is_some() {
                    bail!("an Absolute <font> names a ref");
                }
                let (Some(face), Some(height), Some(weight), Some(italic), Some(underline), Some(strikeout), Some(scale)) =
                    (face, height, weight, italic, underline, strikeout, scale)
                else {
                    bail!("an Absolute <font> leaves a member unspelled");
                };
                Ok(format!(
                    "{{7,0,575,{height},0,0,0,{weight},{italic},{underline},{strikeout},0,0,0,0,0,{},1,{scale}}}",
                    quote(face)
                ))
            }
            "AutoFont" => {
                if !node.attributes.iter().all(|(key, _)| key == "kind") {
                    bail!("an AutoFont <font> spells members");
                }
                Ok("{7,3,0,1,100}".to_string())
            }
            "StyleItem" | "WindowsFont" => {
                let reference = node
                    .attribute("ref")
                    .ok_or_else(|| anyhow!("a {kind} <font> names no ref"))?;
                let (code, reference) = if kind == "WindowsFont" {
                    let index = match reference {
                        "sys:DefaultGUIFont" => "0",
                        "sys:ANSIFixedFont" => "2",
                        other => bail!("system font {other} has no stored code"),
                    };
                    if !reference.starts_with("sys:")
                        || node.own_namespace("sys").is_some_and(|ns| ns != SYSTEM_FONTS_NS)
                    {
                        bail!("system font {reference} names an unknown namespace");
                    }
                    (1, format!("{{{index}}}"))
                } else {
                    let name = reference
                        .strip_prefix("style:")
                        .ok_or_else(|| anyhow!("style font {reference} has no writer"))?;
                    let stored = match name {
                        "TextFont" => "{-20}".to_string(),
                        "SmallTextFont" => "{-30}".to_string(),
                        "NormalTextFont" => "{-31}".to_string(),
                        "LargeTextFont" => "{-32}".to_string(),
                        "ExtraLargeTextFont" => "{-33}".to_string(),
                        _ => {
                            let source = self
                                .source
                                .ok_or_else(|| anyhow!("style font {reference} needs the configuration"))?;
                            let uuid = source.resolve_style_item_uuid(&format!("StyleItem.{name}"))?;
                            format!("{{0,{}}}", uuid.to_ascii_lowercase())
                        }
                    };
                    (2, stored)
                };
                let mut mask = 0u32;
                let mut members = String::new();
                if let Some(height) = height {
                    mask |= 1 << 1;
                    members.push_str(&format!(",{height}"));
                }
                if let Some(weight) = weight {
                    mask |= 1 << 2;
                    members.push_str(&format!(",{weight}"));
                }
                if let Some(italic) = italic {
                    mask |= 1 << 3;
                    members.push_str(&format!(",{italic}"));
                }
                if let Some(underline) = underline {
                    mask |= 1 << 4;
                    members.push_str(&format!(",{underline}"));
                }
                if let Some(strikeout) = strikeout {
                    mask |= 1 << 5;
                    members.push_str(&format!(",{strikeout}"));
                }
                if let Some(face) = face {
                    mask |= 1;
                    members.push_str(&format!(",{}", quote(face)));
                }
                let tail = match scale {
                    Some(scale) => {
                        mask |= 1 << 9;
                        scale.to_string()
                    }
                    None => "100".to_string(),
                };
                Ok(format!("{{7,{code},{mask},{reference}{members},1,{tail}}}"))
            }
            other => bail!("a {other} <font> has no writer"),
        }
    }

    // -- lines -------------------------------------------------------------

    fn line_table(&mut self) -> Result<String> {
        let mut record = format!("{{{}", self.lines.order.len());
        for position in 0..self.lines.order.len() {
            let node = self.document.lines[self.lines.order[position]];
            node.only_attributes(&["width", "gap"], "line")?;
            node.only_children(&["style"], "line")?;
            if node.attribute("gap").is_some_and(|gap| gap != "false") {
                bail!("a <line> with a gap has no writer");
            }
            let width = parse_usize(node.attribute("width").unwrap_or("0"), "line width")?;
            let style = node
                .child("style")
                .ok_or_else(|| anyhow!("a <line> has no style"))?;
            let (kind, code) = match style.xsi_type() {
                Some("v8ui:SpreadsheetDocumentCellLineType") => (
                    CELL_LINE_KIND,
                    match style.text.as_str() {
                        "None" => 0,
                        "Solid" => 1,
                        "Dotted" => 2,
                        "Double" => 3,
                        "ThinDashed" => 4,
                        "ThickDashed" => 5,
                        "LargeDashed" => 6,
                        other => bail!("cell line style {other} has no stored code"),
                    },
                ),
                Some("v8ui:SpreadsheetDocumentDrawingLineType") => (
                    DRAWING_LINE_KIND,
                    match style.text.as_str() {
                        "None" => 0,
                        "Solid" => 1,
                        "Dotted" => 3,
                        other => bail!("drawing line style {other} has no stored code"),
                    },
                ),
                other => bail!("a line style of type {other:?} has no writer"),
            };
            record.push_str(&format!(",1,{{4,0,{{0}},{code},{width},0,{kind},0}},0"));
        }
        record.push('}');
        let mut out = Out::default();
        out.record(&record);
        Ok(out.text)
    }

    // -- pictures ----------------------------------------------------------

    fn picture_table(&mut self) -> Result<String> {
        // Pictures nothing names follow the named ones in XML order.
        for index in 0..self.document.pictures.len() {
            self.pictures.intern(index);
        }
        let mut out = Out::default();
        out.raw(&self.pictures.order.len().to_string());
        for position in 0..self.pictures.order.len() {
            let node = self.document.pictures[self.pictures.order[position]];
            out.comma();
            let record = self.picture_record(node)?;
            out.record(&record);
        }
        Ok(out.text)
    }

    /// `{4, kind, ref, "", tx, ty, t, [{payload},] 0, ""}`: kind 3 carries the
    /// picture's own bytes, 1 a reference and 0 nothing at all; `t` is 0 for
    /// `t="false"` and 1 where the attribute is absent.
    fn picture_record(&self, node: &Node) -> Result<String> {
        node.only_children(&["index", "picture"], "picture")?;
        let picture = node
            .child("picture")
            .ok_or_else(|| anyhow!("a <picture> has no body"))?;
        picture.only_attributes(&["t", "tx", "ty", "ref"], "picture")?;
        if !picture.children.is_empty() {
            bail!("a <picture> body has children");
        }
        let transparency = match picture.attribute("t") {
            Some("false") => 0,
            None => 1,
            Some(other) => bail!("a picture's t=\"{other}\" has no stored code"),
        };
        let (tx, ty) = match (picture.attribute("tx"), picture.attribute("ty")) {
            (Some(x), Some(y)) => (parse_i64(x, "picture tx")?, parse_i64(y, "picture ty")?),
            (None, None) => (-1, -1),
            _ => bail!("a picture spells one of tx and ty"),
        };
        let has_payload = !picture.text.trim().is_empty();
        match (picture.attribute("ref"), has_payload) {
            (Some(_), true) => bail!("a picture spells both a body and a ref"),
            (Some(reference), false) => {
                let reference = self.picture_reference(reference)?;
                Ok(format!(
                    "{{4,1,{reference},\"\",{tx},{ty},{transparency},0,\"\"}}"
                ))
            }
            (None, true) => Ok(format!(
                "{{4,3,{{0}},\"\",{tx},{ty},{transparency},{{{{#base64:{}}}}},0,\"\"}}",
                base64_payload(&picture.text)
            )),
            (None, false) => Ok(format!(
                "{{4,0,{{0}},\"\",{tx},{ty},{transparency},0,\"\"}}"
            )),
        }
    }

    /// A picture reference as the record stores it.
    fn picture_reference(&self, reference: &str) -> Result<String> {
        let name = reference
            .strip_prefix("v8ui:")
            .ok_or_else(|| anyhow!("picture ref {reference} has no writer"))?;
        match name {
            "Print" => return Ok("{-13}".to_string()),
            "InputFieldCalculator" => return Ok("{-6}".to_string()),
            "Information" => return Ok("{0,4b54770b-d069-4c0e-9b17-5cc2a01134d9}".to_string()),
            "SaveFile" => return Ok("{0,818ab7d0-4654-4542-bd5e-fd9d1352b5a1}".to_string()),
            _ => {}
        }
        let source = self
            .source
            .ok_or_else(|| anyhow!("picture ref {reference} needs the configuration"))?;
        let uuid = source.resolve_common_picture_uuid(&format!("CommonPicture.{name}"))?;
        Ok(format!("{{0,{}}}", uuid.to_ascii_lowercase()))
    }
}

enum ContentTable {
    NumberFormats,
    ValueTypes,
    ControlTypes,
    Masks,
}

struct RowsOut {
    count: usize,
    text: String,
}

struct GroupRecords {
    records: Vec<String>,
    levels: Vec<usize>,
}

fn group_records(groups: &[&Node], tag: &str) -> Result<GroupRecords> {
    let mut ranges = Vec::with_capacity(groups.len());
    let mut records = Vec::with_capacity(groups.len());
    let mut levels = Vec::with_capacity(groups.len());
    for group in groups {
        group.only_children(&["b", "e", "t", "o", "g"], tag)?;
        let begin = required_usize(group, "b", tag)?;
        let end = optional_usize(group, "e", tag)?.unwrap_or(begin);
        // The level is the group's nesting depth among the groups before it.
        let level = ranges
            .iter()
            .filter(|(outer_begin, outer_end)| *outer_begin <= begin && end <= *outer_end)
            .count();
        let text = match group.child("t") {
            Some(text) => localized_items(text, "t")?,
            None => "{1,0}".to_string(),
        };
        let closed = match group.text_of("o") {
            Some(value) => u8::from(!parse_bool(value, "<o>")?),
            None => 0,
        };
        let group_begin = match group.text_of("g") {
            Some("Begin") => 1,
            Some(other) => bail!("<{tag}><g> {other} has no stored code"),
            None => 0,
        };
        records.push(format!("{{{begin},{end},{level},{text},{closed},{group_begin}}}"));
        ranges.push((begin, end));
        levels.push(level);
    }
    Ok(GroupRecords { records, levels })
}

fn area_record(area: &Node) -> Result<String> {
    area.only_children(
        &[
            "type",
            "beginRow",
            "endRow",
            "beginColumn",
            "endColumn",
            "columnsID",
        ],
        &area.name,
    )?;
    let kind = match required_text(area, "type", &area.name)? {
        "Rows" => 1,
        "Columns" => 2,
        "Rectangle" => 3,
        other => bail!("an area of type {other} has no stored code"),
    };
    let columns_id = area
        .text_of("columnsID")
        .map(|text| uuid_text(text, "an area's <columnsID>"))
        .transpose()?;
    Ok(format!(
        "{{{kind},{},{},{},{},{}}}",
        required_i64(area, "beginColumn", &area.name)?,
        required_i64(area, "beginRow", &area.name)?,
        required_i64(area, "endColumn", &area.name)?,
        required_i64(area, "endRow", &area.name)?,
        columns_id.as_deref().unwrap_or(NIL_UUID)
    ))
}

/// A localized text list: `{1,<count>,{"<lang>","<content>"}...}`.
fn localized_items(node: &Node, owner: &str) -> Result<String> {
    let mut record = String::from("{1,");
    let items = node.children_named("item").collect::<Vec<_>>();
    if items.len() != node.children.len() {
        bail!("<{owner}> carries something other than <v8:item>");
    }
    record.push_str(&items.len().to_string());
    for item in items {
        item.only_children(&["lang", "content"], "v8:item")?;
        record.push_str(&format!(
            ",{{{},{}}}",
            quote(item.text_of("lang").unwrap_or_default()),
            quote(item.text_of("content").unwrap_or_default())
        ));
    }
    record.push('}');
    Ok(record)
}

/// The same XML one element deeper: every generated `d<N>p1` prefix becomes
/// `d<N+1>p1`, in names, declarations and the QName values that use them.
fn deepen_generated_prefixes(xml: &str) -> Result<String> {
    let bytes = xml.as_bytes();
    let mut out = String::with_capacity(xml.len() + 64);
    let mut index = 0usize;
    let mut copied = 0usize;
    while index < bytes.len() {
        let starts_word = index == 0 || !bytes[index - 1].is_ascii_alphanumeric();
        if starts_word && bytes[index] == b'd' {
            let digits_start = index + 1;
            let mut digits_end = digits_start;
            while digits_end < bytes.len() && bytes[digits_end].is_ascii_digit() {
                digits_end += 1;
            }
            if digits_end > digits_start
                && xml[digits_end..].starts_with("p1")
                && xml[digits_end + 2..]
                    .chars()
                    .next()
                    .is_some_and(|next| next == ':' || next == '=')
            {
                let depth = xml[digits_start..digits_end].parse::<usize>()?;
                out.push_str(&xml[copied..index]);
                out.push_str(&format!("d{}p1", depth + 1));
                index = digits_end + 2;
                copied = index;
                continue;
            }
        }
        index += 1;
    }
    out.push_str(&xml[copied..]);
    Ok(out)
}

/// The plain copy a formatted text stores beside its formatted one: the same
/// items with every markup tag (`<b>`, `</>`, `<colorstyle -14>`, `<color
/// #009646>`, …) taken out and the text between them kept, as all eight ERP
/// УХ documents that carry markup store it.
fn localized_items_stripped(node: &Node, owner: &str) -> Result<String> {
    let mut record = String::from("{1,");
    let items = node.children_named("item").collect::<Vec<_>>();
    if items.len() != node.children.len() {
        bail!("<{owner}> carries something other than <v8:item>");
    }
    record.push_str(&items.len().to_string());
    for item in items {
        item.only_children(&["lang", "content"], "v8:item")?;
        let content = strip_formatted_markup(item.text_of("content").unwrap_or_default())
            .with_context(|| format!("a formatted <{owner}>"))?;
        record.push_str(&format!(
            ",{{{},{}}}",
            quote(item.text_of("lang").unwrap_or_default()),
            quote(&content)
        ));
    }
    record.push('}');
    Ok(record)
}

fn strip_formatted_markup(text: &str) -> Result<String> {
    let mut plain = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('<') {
        plain.push_str(&rest[..open]);
        let tag = &rest[open + 1..];
        let close = tag
            .find('>')
            .ok_or_else(|| anyhow!("markup opens a tag it never closes"))?;
        if tag[..close].contains('<') {
            bail!("markup nests a tag inside a tag");
        }
        rest = &tag[close + 1..];
    }
    plain.push_str(rest);
    Ok(plain)
}

/// A base64 body as the record stores it: the XML's own lines, each break a
/// CRLF (which the body layout then writes as CR CR LF).
fn base64_payload(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\n', "\r\n")
}

fn typed_value(node: &Node) -> Result<String> {
    if node.is_nil() {
        if !node.children.is_empty() || !node.text.is_empty() {
            bail!("a nil value carries content");
        }
        return Ok("{\"U\"}".to_string());
    }
    let kind = node
        .xsi_type()
        .ok_or_else(|| anyhow!("a <{}> value has no xsi:type", node.name))?;
    match kind {
        "xs:string" => Ok(format!("{{\"S\",{}}}", quote(&node.text))),
        "xs:boolean" => Ok(format!(
            "{{\"B\",{}}}",
            u8::from(parse_bool(&node.text, "a boolean value")?)
        )),
        "xs:decimal" => {
            let number = node.text.trim();
            if number.is_empty()
                || !number
                    .chars()
                    .all(|ch| ch.is_ascii_digit() || ch == '-' || ch == '.')
            {
                bail!("decimal value {number} has no stored form");
            }
            Ok(format!("{{\"N\",{number}}}"))
        }
        "xs:dateTime" => {
            let stamp = node.text.trim();
            let digits = stamp
                .chars()
                .filter(|ch| ch.is_ascii_digit())
                .collect::<String>();
            if digits.len() != 14 || stamp.len() != 19 {
                bail!("dateTime value {stamp} has no stored form");
            }
            Ok(format!("{{\"D\",{digits}}}"))
        }
        "v8:Structure" => {
            let mut record = format!(
                "{{\"#\",{STRUCTURE_VALUE_TYPE},{{{}",
                node.children.len()
            );
            for property in &node.children {
                if property.name != "Property" {
                    bail!("a Structure value carries <{}>", property.name);
                }
                let name = property
                    .attribute("name")
                    .ok_or_else(|| anyhow!("a Structure property has no name"))?;
                let value = property
                    .child("Value")
                    .ok_or_else(|| anyhow!("a Structure property has no value"))?;
                record.push_str(&format!(",{{{{\"S\",{}}},{}}}", quote(name), typed_value(value)?));
            }
            record.push_str("}}");
            Ok(record)
        }
        "v8:Array" => {
            let mut record = format!("{{\"#\",{ARRAY_VALUE_TYPE},{{{}", node.children.len());
            for value in &node.children {
                if value.name != "Value" {
                    bail!("an Array value carries <{}>", value.name);
                }
                record.push(',');
                record.push_str(&typed_value(value)?);
            }
            record.push_str("}}");
            Ok(record)
        }
        other => bail!("a value of type {other} has no writer"),
    }
}

fn number(value: &str, what: &str) -> Result<String> {
    Ok(format!("{{\"N\",{}}}", parse_usize(value, what)?))
}

fn flag(value: &str, what: &str) -> Result<String> {
    Ok(format!("{{\"N\",{}}}", u8::from(parse_bool(value, what)?)))
}

fn decimal(value: &str, what: &str) -> Result<String> {
    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
    if whole.is_empty()
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        bail!("{what} {value} is not a decimal");
    }
    Ok(format!("{{\"N\",{value}}}"))
}

fn font_height(text: &str) -> Result<String> {
    let (whole, fraction) = text.split_once('.').unwrap_or((text, "0"));
    let whole = parse_usize(whole, "font height")?;
    if fraction.len() != 1 || !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
        bail!("font height {text} has no stored form");
    }
    Ok((whole * 10 + (fraction.as_bytes()[0] - b'0') as usize).to_string())
}

fn pattern_code(text: &str, what: &str) -> Result<usize> {
    match text {
        "Solid" => Ok(0),
        "WithoutPattern" => Ok(255),
        other => other
            .strip_prefix("Pattern")
            .and_then(|number| number.parse::<usize>().ok())
            .filter(|number| (1..=18).contains(number))
            .ok_or_else(|| anyhow!("{what} {other} has no stored code")),
    }
}

/// The platform style colours a palette slot names by code.
fn platform_style_color(name: &str) -> Option<i32> {
    Some(match name {
        "FormBackColor" => -1,
        "FormTextColor" => -3,
        "ButtonBackColor" => -7,
        "FieldBackColor" => -10,
        "FieldTextColor" => -11,
        "FieldAlternativeBackColor" => -13,
        "FieldSelectionBackColor" => -14,
        "FieldSelectedTextColor" => -15,
        "SpecialTextColor" => -16,
        "NegativeTextColor" => -17,
        "ButtonTextColor" => -21,
        "BorderColor" => -22,
        "ToolTipBackColor" => -23,
        "ToolTipTextColor" => -24,
        "ButtonBorderColor" => -34,
        "TableHeaderBackColor" => -35,
        "TableHeaderTextColor" => -36,
        "TableFooterBackColor" => -37,
        "TableFooterTextColor" => -38,
        "NavigationColor" => -42,
        "AuxiliaryNavigationColor" => -43,
        "ActivityColor" => -44,
        "AccentColor" => -46,
        _ => return None,
    })
}

/// The report colours: the slot stores the colour's own value and an
/// override names the style. Every override of both corpora pairs each code
/// with exactly this value.
fn report_style_color(name: &str) -> Option<(i32, u32)> {
    Some(match name {
        "ReportHeaderBackColor" => (-25, 12_971_252),
        "ReportGroup1BackColor" => (-26, 14_217_976),
        "ReportGroup2BackColor" => (-27, 15_530_491),
        "ReportLineColor" => (-28, 8_765_644),
        _ => return None,
    })
}

fn web_color_code(name: &str) -> Option<u32> {
    Some(match name {
        "AliceBlue" => 1,
        "AntiqueWhite" => 2,
        "Azure" => 5,
        "Beige" => 6,
        "Black" => 8,
        "Blue" => 10,
        "Cream" => 20,
        "Crimson" => 21,
        "DarkBlue" => 23,
        "DarkGray" => 26,
        "DarkGreen" => 27,
        "DarkRed" => 33,
        "DarkSlateGray" => 37,
        "DimGray" => 42,
        "FireBrick" => 44,
        "FloralWhite" => 45,
        "ForestGreen" => 46,
        "Gainsboro" => 48,
        "GhostWhite" => 49,
        "Gray" => 52,
        "Green" => 53,
        "GreenYellow" => 54,
        "HoneyDew" => 55,
        "Ivory" => 59,
        "Lavender" => 61,
        "LemonChiffon" => 64,
        "LightCyan" => 67,
        "LightGoldenRod" => 68,
        "LightGoldenRodYellow" => 69,
        "LightGreen" => 70,
        "LightGray" => 71,
        "LightPink" => 72,
        "LightSalmon" => 73,
        "LightSkyBlue" => 75,
        "LightSlateGray" => 77,
        "LightYellow" => 79,
        "Maroon" => 84,
        "MediumBlue" => 86,
        "MediumGray" => 87,
        "MediumGreen" => 88,
        "MediumSpringGreen" => 93,
        "MintCream" => 97,
        "MistyRose" => 98,
        "PaleGoldenrod" => 108,
        "Red" => 119,
        "RosyBrown" => 120,
        "RoyalBlue" => 121,
        "SaddleBrown" => 122,
        "Silver" => 128,
        "SlateBlue" => 130,
        "SlateGray" => 131,
        "SteelBlue" => 134,
        "Tomato" => 138,
        "Violet" => 140,
        "VioletRed" => 141,
        "White" => 143,
        "WhiteSmoke" => 144,
        "Yellow" => 145,
        _ => return None,
    })
}

fn windows_color_code(name: &str) -> Option<u32> {
    Some(match name {
        "ActiveTitleBar" => 2,
        "InactiveTitleBar" => 3,
        "MenuBar" => 4,
        "InactiveBorder" => 11,
        "ButtonFace" => 15,
        "ButtonShadow" => 16,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn crlf(text: &str) -> String {
        text.replace('\n', "\r\n")
    }

    fn read_back(body: &str) -> String {
        let plain = format!("MOXCEL\0\u{8}\0\u{1}\0\u{c}\0\u{feff}{body}");
        let blob = crate::compiler::families::native::deflate_bytes(plain.as_bytes()).unwrap();
        crate::mssql_dump::try_extract_moxel_spreadsheet_xml(&blob, &BTreeMap::new()).unwrap()
    }

    /// БСП `DataProcessors/НастройкиПользователей/Templates/МакетОтчета`:
    /// the platform numbers its formats by first use in the body -- the two
    /// cell formats, then the column's -- where the XML lists the column's
    /// first, and keeps the default format out of the table.
    const REPORT_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<document xmlns="http://v8.1c.ru/8.2/data/spreadsheet" xmlns:style="http://v8.1c.ru/8.1/data/ui/style" xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:v8ui="http://v8.1c.ru/8.1/data/ui" xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
	<languageSettings>
		<currentLanguage>ru</currentLanguage>
		<defaultLanguage>ru</defaultLanguage>
		<languageInfo>
			<id>ru</id>
			<code>Русский</code>
			<description>Русский</description>
		</languageInfo>
	</languageSettings>
	<columns>
		<size>1</size>
		<columnsItem>
			<index>0</index>
			<column>
				<formatIndex>1</formatIndex>
			</column>
		</columnsItem>
	</columns>
	<rowsItem>
		<index>0</index>
		<row>
			<c>
				<c>
					<f>2</f>
					<parameter>Описание</parameter>
				</c>
			</c>
		</row>
	</rowsItem>
	<rowsItem>
		<index>1</index>
		<row>
			<empty>true</empty>
		</row>
	</rowsItem>
	<rowsItem>
		<index>2</index>
		<row>
			<c>
				<c>
					<f>3</f>
					<parameter>Название</parameter>
				</c>
			</c>
		</row>
	</rowsItem>
	<templateMode>true</templateMode>
	<defaultFormatIndex>4</defaultFormatIndex>
	<height>3</height>
	<vgRows>3</vgRows>
	<namedItem xsi:type="NamedItemCells">
		<name>Заголовок</name>
		<area>
			<type>Rows</type>
			<beginRow>0</beginRow>
			<endRow>0</endRow>
			<beginColumn>-1</beginColumn>
			<endColumn>-1</endColumn>
		</area>
	</namedItem>
	<namedItem xsi:type="NamedItemCells">
		<name>ПустаяСтрока</name>
		<area>
			<type>Rows</type>
			<beginRow>1</beginRow>
			<endRow>1</endRow>
			<beginColumn>-1</beginColumn>
			<endColumn>-1</endColumn>
		</area>
	</namedItem>
	<namedItem xsi:type="NamedItemCells">
		<name>СодержаниеОтчета</name>
		<area>
			<type>Rows</type>
			<beginRow>2</beginRow>
			<endRow>2</endRow>
			<beginColumn>-1</beginColumn>
			<endColumn>-1</endColumn>
		</area>
	</namedItem>
	<font faceName="Arial" height="8" bold="true" italic="false" underline="false" strikeout="false" kind="Absolute" scale="100"/>
	<format>
		<width>509</width>
	</format>
	<format>
		<font>0</font>
		<fillType>Parameter</fillType>
	</format>
	<format>
		<fillType>Parameter</fillType>
	</format>
	<format>
		<width>72</width>
	</format>
</document>"#;
    const REPORT_BODY: &str = r#"{8,1,12,
{"ru","ru",1,1,"ru","Русский","Русский",0},
{128,72},
{0},0,
{0,0},
{0,0},
{0,0},
{0,0},
{0,0},
{0,0},1,2,3,0,0,1,0,
{16,1,
{1,1,
{"","Описание"}
},0},1,0,0,2,0,1,0,
{16,2,
{1,1,
{"","Название"}
},0},
{1,0,00000000-0000-0000-0000-000000000000,1,0,3},3,0,0,0,0,0,0,0,0,
{0},
{0},
{0},
{3,"Заголовок",
{1,
{1,-1,0,-1,0,00000000-0000-0000-0000-000000000000},0},"ПустаяСтрока",
{1,
{1,-1,1,-1,1,00000000-0000-0000-0000-000000000000},0},"СодержаниеОтчета",
{1,
{1,-1,2,-1,2,00000000-0000-0000-0000-000000000000},0}
},"",
{
{0,6,6,
{"N",1000},7,
{"N",1000},8,
{"N",1000},9,
{"N",1000},10,
{"N",1000},11,
{"N",1000}
}
},
{0,-1,-1,-1,-1,00000000-0000-0000-0000-000000000000},0,0,0,0,0,0,0,1,0,1,3,
{32769,0,1},
{32768,1},
{128,509},1,
{7,0,575,80,0,0,0,700,0,0,0,0,0,0,0,0,"Arial",1,100},0,0,0,2,
{3,3,
{-1}
},
{3,3,
{-3}
},0,0,0,"",0,
{3,0,0,100,1,1,0,1,1,0,0,0,0,0,0,0,0,0,0,0,0,"",0,0,0,0,0,0,0},
{0},0,0,0,1,0,0,0}"#;

    #[test]
    fn writes_the_row_the_platform_stored_and_reads_it_back() {
        let xml = format!("\u{feff}{}", crlf(REPORT_XML));
        let body = write_native_moxel_body(xml.as_bytes(), None).unwrap();
        assert_eq!(body, crlf(REPORT_BODY));
        assert_eq!(read_back(&body), xml);
    }

    #[test]
    fn a_report_colour_and_its_own_value_are_two_palette_slots() {
        let xml = br#"<document xmlns="http://v8.1c.ru/8.2/data/spreadsheet" xmlns:style="http://v8.1c.ru/8.1/data/ui/style">
	<columns><size>1</size></columns>
	<rowsItem><index>0</index><row><c><c><f>1</f></c></c><c><c><f>2</f></c></c></row></rowsItem>
	<format><backColor>style:ReportHeaderBackColor</backColor></format>
	<format><backColor>#F4ECC5</backColor></format>
</document>"#;
        let body = write_native_moxel_body(xml, None).unwrap().replace("\r\n", "");
        assert!(body.contains("4,{3,3,{-1}},{3,3,{-3}},{3,0,{12971252}},{3,0,{12971252}},"));
        assert!(body.contains("{1,2,{3,3,{-25}}}"));
        let exported = read_back(&body);
        assert!(exported.contains("<backColor>style:ReportHeaderBackColor</backColor>"));
        assert!(exported.contains("<backColor>#F4ECC5</backColor>"));
    }
}
