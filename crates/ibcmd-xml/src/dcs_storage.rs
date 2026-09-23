//! Source-to-storage writer for `DataCompositionSchema` templates.
//!
//! The platform keeps a report schema as a framed run of XML documents: the
//! primary `SchemaFile`, one `Settings` document per root `settingsVariant`,
//! and a terminal `SchemaFile` holding the root-level area templates, whose
//! table-cell appearances are moved out into a de-duplicated side table the
//! cells select by ordinal. This module writes those documents from the
//! source `Template.xml` alone -- the inverse of the exporter's
//! storage-to-source transliteration
//! ([`crate::rewrite_dcs_primary_schema_storage_document`] and its terminal
//! and settings twins).
//!
//! The source document hoists every data-composition namespace onto its root;
//! storage declares a namespace where it is first needed, which is what this
//! writer reproduces:
//!
//! * an element outside the namespace in force re-declares the default
//!   namespace on itself, unless a prefix for its namespace is already in
//!   scope (the children of a `v8:LocalStringType` value are spelled through
//!   the prefix the value's own `xsi:type` declared);
//! * a QName value -- an `xsi:type`, a `v8:Type`, a colour -- whose namespace
//!   nothing in scope binds gets a prefix declared on the very element that
//!   carries it: `dcsset`, `dcscor` and `dcsat` for the three composition
//!   namespaces, the prefix the source itself spelled for a namespace it
//!   declares at the point of use (`sys`, `style`, ...), and a generated
//!   `d<depth>p<n>` for everything else;
//! * a `Settings` object re-declares the settings writer's own namespace set;
//! * declarations appear in the order their uses are met: the element name,
//!   then its attributes in order, then its character data.
//!
//! Configuration types are written the way the current platform writes them:
//! by `TypeId`, grouped behind the literal types of the same list and ordered
//! by uuid. The exporter merges such a list back into the source order; a list
//! whose source order that merge would not reproduce keeps its references
//! spelled by name instead, which storage also carries and which the exporter
//! leaves exactly in place.
//!
//! Measured against the stored bodies of the БСП demo and ERP УХ 3.2.12.6
//! (8.3.27, September 2026), this reproduces every body the current writer
//! produced byte for byte: all 69 БСП bodies and 865 of the 1 541 УХ ones.
//! The other 676 carry what older platform writers left in bodies nobody
//! re-saved -- empty `appearance`/`inputParameters`/`outputParameters`
//! placeholders, table cells pointing at empty side-table appearances, and
//! references spelled by name -- which the source export drops, so no source
//! can bring them back; every one of them still reads back as its source.

use std::collections::BTreeMap;

use crate::dcs_schema::{TypeRunMember, evidenced_type_run_permutation};
use crate::dcs_template::{DcsSchemaTemplateError, DcsSchemaTemplateOwnedDocuments};

const SCHEMA: &str = "http://v8.1c.ru/8.1/data-composition-system/schema";
const COMMON: &str = "http://v8.1c.ru/8.1/data-composition-system/common";
const DCS_CORE: &str = "http://v8.1c.ru/8.1/data-composition-system/core";
const SETTINGS: &str = "http://v8.1c.ru/8.1/data-composition-system/settings";
const AREA_TEMPLATE: &str = "http://v8.1c.ru/8.1/data-composition-system/area-template";
const DATA_CORE: &str = "http://v8.1c.ru/8.1/data/core";
const DATA_UI: &str = "http://v8.1c.ru/8.1/data/ui";
const STYLE: &str = "http://v8.1c.ru/8.1/data/ui/style";
const FONTS_SYSTEM: &str = "http://v8.1c.ru/8.1/data/ui/fonts/system";
const COLORS_WEB: &str = "http://v8.1c.ru/8.1/data/ui/colors/web";
const COLORS_WINDOWS: &str = "http://v8.1c.ru/8.1/data/ui/colors/windows";
const XS: &str = "http://www.w3.org/2001/XMLSchema";
const XSI: &str = "http://www.w3.org/2001/XMLSchema-instance";
const CURRENT_CONFIG: &str = "http://v8.1c.ru/8.1/data/enterprise/current-config";

const DOCUMENT_HEAD: &str = "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n";
const SCHEMA_FILE_OPEN: &str = "<SchemaFile xmlns=\"\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">";
const MAX_DEPTH: usize = 256;
const MAX_NODES: usize = 1_000_000;

/// The namespaces the source document declares on its root. The exporter
/// spells every one of them through that root declaration, so a declaration
/// storage makes for one of them never survives into the source, and a source
/// declaration of one of them says nothing about storage.
const SOURCE_ROOT_NAMESPACES: [&str; 8] = [
    SCHEMA, COMMON, DCS_CORE, SETTINGS, DATA_CORE, DATA_UI, XS, XSI,
];

/// The namespace set a `Settings` object's writer declares on its root, in
/// the order it declares them.
const SETTINGS_NAMESPACES: [(&str, &str); 10] = [
    ("", SETTINGS),
    ("dcscor", DCS_CORE),
    ("style", STYLE),
    ("sys", FONTS_SYSTEM),
    ("v8", DATA_CORE),
    ("v8ui", DATA_UI),
    ("web", COLORS_WEB),
    ("win", COLORS_WINDOWS),
    ("xs", XS),
    ("xsi", XSI),
];

/// Root-level schema children the platform keeps in the terminal document.
const TERMINAL_ELEMENTS: [&str; 5] = [
    "template",
    "fieldTemplate",
    "groupTemplate",
    "groupHeaderTemplate",
    "totalFieldsTemplate",
];

/// Reference-family protocol identifiers: the `TypeId` storage writes for a
/// `<v8:TypeSet>` naming a whole family rather than one configuration type.
const REFERENCE_FAMILIES: [(&str, &str); 11] = [
    ("ExchangePlanRef", "0a52f9de-73ea-4507-81e8-66217bead73a"),
    (
        "BusinessProcessRoutePointRef",
        "11e5f865-1501-40c6-b4d4-022095a296a5",
    ),
    ("BusinessProcessRef", "214fa4d8-6ba4-4748-a5e1-6332b5887780"),
    ("DocumentRef", "38bfd075-3e63-4aaa-a93e-94521380d579"),
    ("EnumRef", "474c3bf6-08b5-4ddc-a2ad-989cedf11583"),
    (
        "ChartOfCalculationTypesRef",
        "593cd424-0877-470d-91f9-b90a982059b4",
    ),
    ("TaskRef", "6291e9b3-8df5-44e1-b6b2-d9fe008016c0"),
    (
        "ChartOfCharacteristicTypesRef",
        "99892482-ed55-4fb5-a7f7-20888820a758",
    ),
    ("ChartOfAccountsRef", "ac606d60-0209-4159-8e4c-794bc091ce38"),
    ("CatalogRef", "e61ef7b8-f3e1-4f4b-8ac7-676e90524997"),
    ("AnyIBRef", "280f5f0e-9c8a-49cc-bf6d-4d296cc17a63"),
];

/// Resolves a configuration generated type by name to its storage `TypeId`.
///
/// The name is the one the source spells after its current-config prefix:
/// `CatalogRef.Валюты`, `Characteristic.ВидыСубконто`, `DocumentRef.X`.
pub trait DcsStorageTypeResolver {
    fn generated_type_id(&self, qualified_name: &str) -> Option<String>;
}

impl<F> DcsStorageTypeResolver for F
where
    F: Fn(&str) -> Option<String>,
{
    fn generated_type_id(&self, qualified_name: &str) -> Option<String> {
        self(qualified_name)
    }
}

/// How the source spells one `TypeId` the writer put into storage -- the
/// entry the exporter's type index needs to spell it back.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DcsStorageTypeSpelling {
    /// `<v8:Type>` of a configuration type, by its qualified name.
    Type(String),
    /// `<v8:TypeSet>` of a reference family or a characteristic.
    TypeSet(String),
    /// `<v8:TypeId>`, which the source already spells by id.
    Kept,
}

/// The storage documents of one schema template, plus every reference the
/// writer resolved while writing them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DcsStorageDocuments {
    documents: DcsSchemaTemplateOwnedDocuments,
    type_ids: BTreeMap<String, DcsStorageTypeSpelling>,
    style_items: BTreeMap<String, String>,
}

impl DcsStorageDocuments {
    pub fn documents(&self) -> &DcsSchemaTemplateOwnedDocuments {
        &self.documents
    }

    pub fn into_documents(self) -> DcsSchemaTemplateOwnedDocuments {
        self.documents
    }

    /// Lowercase `TypeId` -> the spelling the source gives it.
    pub fn type_ids(&self) -> &BTreeMap<String, DcsStorageTypeSpelling> {
        &self.type_ids
    }

    /// Lowercase `StyleItem` uuid -> its name, for every configuration style
    /// item the writer spelled by uuid.
    pub fn style_items(&self) -> &BTreeMap<String, String> {
        &self.style_items
    }
}

/// Writes the storage documents of a `DataCompositionSchema` template from
/// its source `Template.xml`.
///
/// `style_items` maps a configuration `StyleItem` name to its uuid; a style
/// reference to one of them is stored as `0:<uuid>`, any other style name as
/// the QName it is. `types` resolves configuration types to their `TypeId`;
/// a reference it cannot resolve is stored by name.
pub fn compile_dcs_schema_storage_documents(
    source: &[u8],
    style_items: &BTreeMap<String, String>,
    types: &dyn DcsStorageTypeResolver,
) -> Result<DcsStorageDocuments, DcsSchemaTemplateError> {
    let text = std::str::from_utf8(source)
        .map_err(|_| DcsSchemaTemplateError::Malformed("DCS source is not UTF-8".to_string()))?;
    let body = strip_shell(text)?;
    let root = parse_document(body)?;
    if root.name != "DataCompositionSchema" {
        return Err(DcsSchemaTemplateError::UnsupportedSource(
            "source root is not an unprefixed DataCompositionSchema",
        ));
    }
    if !root.plain_attributes().is_empty() {
        return Err(DcsSchemaTemplateError::UnsupportedSource(
            "source root carries attributes beyond namespace declarations",
        ));
    }
    let mut source_scopes: Scopes = vec![root.declarations()];
    if resolve_prefix(&source_scopes, "") != Some(SCHEMA) {
        return Err(DcsSchemaTemplateError::UnsupportedSource(
            "source root is not in the data-composition schema namespace",
        ));
    }
    let children = root.element_children()?;

    // The area templates the platform keeps in the terminal document are the
    // run of root children standing right before the first settingsVariant:
    // that is where the exporter splices the terminal document back in. A
    // template anywhere else stays where it is, in the primary document,
    // which the exporter spells in place just the same.
    let first_variant = children
        .iter()
        .position(|child| is_root_child(child, &source_scopes, "settingsVariant"));
    let terminal_run = match first_variant {
        Some(first) => {
            let mut start = first;
            while start > 0 && is_terminal_element(children[start - 1], &source_scopes) {
                start -= 1;
            }
            let run = start..first;
            let elsewhere = children.iter().enumerate().any(|(index, child)| {
                !run.contains(&index) && is_terminal_element(child, &source_scopes)
            });
            if elsewhere { 0..0 } else { run }
        }
        None => 0..0,
    };

    let mut writer = Writer {
        style_items,
        types,
        type_ids: BTreeMap::new(),
        used_style_items: BTreeMap::new(),
        appearances: Vec::new(),
        side_table: false,
        nodes: 0,
    };
    let mut primary = String::with_capacity(body.len() + body.len() / 4);
    primary.push_str(DOCUMENT_HEAD);
    primary.push_str(SCHEMA_FILE_OPEN);
    primary.push_str("\r\n\t<dataCompositionSchema xmlns=\"");
    primary.push_str(SCHEMA);
    primary.push_str("\">");
    let mut storage_scopes = schema_storage_scopes();
    let mut settings = Vec::new();
    for (index, child) in children.iter().enumerate() {
        if terminal_run.contains(&index) {
            continue;
        }
        primary.push_str("\r\n\t\t");
        if is_root_child(child, &source_scopes, "settingsVariant") {
            settings.push(writer.write_variant(
                child,
                &mut source_scopes,
                &mut storage_scopes,
                &mut primary,
            )?);
        } else {
            writer.write_element(
                child,
                "DataCompositionSchema",
                3,
                2,
                &mut source_scopes,
                &mut storage_scopes,
                &mut primary,
            )?;
        }
    }
    primary.push_str("\r\n\t</dataCompositionSchema>\r\n</SchemaFile>");
    if settings.is_empty() {
        return Err(DcsSchemaTemplateError::UnsupportedSource(
            "a schema without a settingsVariant has no framed Settings document",
        ));
    }

    let terminal = if terminal_run.is_empty() {
        format!(
            "{DOCUMENT_HEAD}{SCHEMA_FILE_OPEN}\r\n\t<dataCompositionSchema xmlns=\"{SCHEMA}\"/>\r\n</SchemaFile>"
        )
    } else {
        let mut terminal = String::new();
        terminal.push_str(DOCUMENT_HEAD);
        terminal.push_str(SCHEMA_FILE_OPEN);
        terminal.push_str("\r\n\t<dataCompositionSchema xmlns=\"");
        terminal.push_str(SCHEMA);
        terminal.push_str("\">");
        writer.side_table = true;
        for child in &children[terminal_run] {
            terminal.push_str("\r\n\t\t");
            writer.write_element(
                child,
                "DataCompositionSchema",
                3,
                2,
                &mut source_scopes,
                &mut storage_scopes,
                &mut terminal,
            )?;
        }
        writer.side_table = false;
        terminal.push_str("\r\n\t</dataCompositionSchema>");
        for appearance in &writer.appearances {
            terminal.push_str(appearance);
        }
        terminal.push_str("\r\n</SchemaFile>");
        terminal
    };

    Ok(DcsStorageDocuments {
        documents: DcsSchemaTemplateOwnedDocuments::from_parts(
            primary.into_bytes(),
            settings.into_iter().map(String::into_bytes).collect(),
            terminal.into_bytes(),
        ),
        type_ids: writer.type_ids,
        style_items: writer.used_style_items,
    })
}

fn schema_storage_scopes() -> Scopes {
    vec![
        vec![
            (String::new(), String::new()),
            ("xs".to_string(), XS.to_string()),
            ("xsi".to_string(), XSI.to_string()),
        ],
        vec![(String::new(), SCHEMA.to_string())],
    ]
}

/// A root child in the schema namespace by `local`, spelled without a
/// declaration of its own.
fn is_root_child(element: &Element, scopes: &Scopes, local: &str) -> bool {
    element.local() == local
        && !element.has_declarations()
        && resolve_prefix(scopes, element.prefix()) == Some(SCHEMA)
}

fn is_terminal_element(element: &Element, scopes: &Scopes) -> bool {
    TERMINAL_ELEMENTS
        .iter()
        .any(|local| is_root_child(element, scopes, local))
}

fn strip_shell(text: &str) -> Result<&str, DcsSchemaTemplateError> {
    let mut body = text.strip_prefix('\u{feff}').unwrap_or(text);
    if let Some(after) = body.strip_prefix("<?xml") {
        let end = after.find("?>").ok_or_else(|| {
            DcsSchemaTemplateError::Malformed("DCS XML declaration is not closed".to_string())
        })?;
        body = after[end + 2..].trim_start_matches(['\r', '\n', ' ', '\t']);
    }
    Ok(body)
}

// ---------------------------------------------------------------------------
// Lexical tree: every value keeps its original escaped spelling.

#[derive(Debug)]
struct Element {
    name: String,
    attributes: Vec<(String, String)>,
    children: Vec<Node>,
}

#[derive(Debug)]
enum Node {
    Element(Element),
    Text(String),
}

impl Element {
    fn prefix(&self) -> &str {
        self.name.split_once(':').map_or("", |(prefix, _)| prefix)
    }

    fn local(&self) -> &str {
        self.name
            .split_once(':')
            .map_or(&self.name, |(_, local)| local)
    }

    fn has_declarations(&self) -> bool {
        self.attributes
            .iter()
            .any(|(key, _)| key == "xmlns" || key.starts_with("xmlns:"))
    }

    fn declarations(&self) -> Vec<(String, String)> {
        self.attributes
            .iter()
            .filter_map(|(key, value)| {
                if key == "xmlns" {
                    Some((String::new(), value.clone()))
                } else {
                    key.strip_prefix("xmlns:")
                        .map(|prefix| (prefix.to_string(), value.clone()))
                }
            })
            .collect()
    }

    fn plain_attributes(&self) -> Vec<(&str, &str)> {
        self.attributes
            .iter()
            .filter(|(key, _)| key != "xmlns" && !key.starts_with("xmlns:"))
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect()
    }

    fn attribute(&self, key: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.as_str())
    }

    /// The element children, refusing character data mixed in with them: the
    /// layout between elements is regenerated, so anything but whitespace
    /// there would be lost.
    fn element_children(&self) -> Result<Vec<&Element>, DcsSchemaTemplateError> {
        let mut elements = Vec::new();
        let mut text = false;
        for child in &self.children {
            match child {
                Node::Element(element) => elements.push(element),
                Node::Text(value) => {
                    if !value
                        .bytes()
                        .all(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n'))
                    {
                        text = true;
                    }
                }
            }
        }
        if text && !elements.is_empty() {
            return Err(DcsSchemaTemplateError::UnsupportedSource(
                "an element mixes character data with child elements",
            ));
        }
        Ok(elements)
    }

    /// The character data of an element without element children.
    fn text(&self) -> Option<String> {
        if self
            .children
            .iter()
            .any(|child| matches!(child, Node::Element(_)))
        {
            return None;
        }
        Some(
            self.children
                .iter()
                .map(|child| match child {
                    Node::Text(value) => value.as_str(),
                    Node::Element(_) => "",
                })
                .collect(),
        )
    }
}

fn parse_document(body: &str) -> Result<Element, DcsSchemaTemplateError> {
    let malformed = |reason: &str| DcsSchemaTemplateError::Malformed(reason.to_string());
    let bytes = body.as_bytes();
    let mut stack: Vec<Element> = Vec::new();
    let mut root: Option<Element> = None;
    let mut position = 0usize;
    let mut nodes = 0usize;
    while position < bytes.len() {
        let Some(relative) = body[position..].find('<') else {
            let rest = &body[position..];
            match stack.last_mut() {
                Some(parent) => parent.children.push(Node::Text(rest.to_string())),
                None if rest.trim().is_empty() => {}
                None => return Err(malformed("DCS XML has text outside its root")),
            }
            break;
        };
        if relative > 0 {
            let text = &body[position..position + relative];
            match stack.last_mut() {
                Some(parent) => parent.children.push(Node::Text(text.to_string())),
                None if text.trim().is_empty() => {}
                None => return Err(malformed("DCS XML has text outside its root")),
            }
        }
        let start = position + relative;
        let mut quote = None::<u8>;
        let mut end = None;
        for (offset, byte) in bytes.iter().enumerate().skip(start + 1) {
            match (quote, *byte) {
                (Some(open), byte) if byte == open => quote = None,
                (Some(_), _) => {}
                (None, b'"' | b'\'') => quote = Some(*byte),
                (None, b'>') => {
                    end = Some(offset);
                    break;
                }
                (None, _) => {}
            }
        }
        let end = end.ok_or_else(|| malformed("unterminated XML tag"))?;
        let tag = &body[start..=end];
        position = end + 1;
        if tag.starts_with("<?") || tag.starts_with("<!") {
            return Err(DcsSchemaTemplateError::UnsupportedSource(
                "comments, CDATA, processing instructions and document types are outside the source shape",
            ));
        }
        nodes += 1;
        if nodes > MAX_NODES {
            return Err(DcsSchemaTemplateError::UnsupportedSource(
                "DCS XML exceeds the bounded node limit",
            ));
        }
        if let Some(name) = tag.strip_prefix("</") {
            let name = name[..name.len() - 1].trim_end();
            let element = stack
                .pop()
                .ok_or_else(|| malformed("closing element has no opener"))?;
            if element.name != name {
                return Err(malformed("DCS XML element nesting is inconsistent"));
            }
            match stack.last_mut() {
                Some(parent) => parent.children.push(Node::Element(element)),
                None if root.is_none() => root = Some(element),
                None => return Err(malformed("DCS XML contains multiple roots")),
            }
            continue;
        }
        let (element, self_closing) = parse_start_tag(tag)?;
        if stack.is_empty() && root.is_some() {
            return Err(malformed("DCS XML contains multiple roots"));
        }
        if self_closing {
            match stack.last_mut() {
                Some(parent) => parent.children.push(Node::Element(element)),
                None => root = Some(element),
            }
        } else {
            stack.push(element);
            if stack.len() > MAX_DEPTH {
                return Err(DcsSchemaTemplateError::UnsupportedSource(
                    "DCS XML depth exceeds the bounded limit",
                ));
            }
        }
    }
    if !stack.is_empty() {
        return Err(malformed("DCS XML root is unclosed"));
    }
    root.ok_or_else(|| malformed("DCS XML has no root"))
}

fn parse_start_tag(tag: &str) -> Result<(Element, bool), DcsSchemaTemplateError> {
    let malformed = |reason: &str| DcsSchemaTemplateError::Malformed(reason.to_string());
    let inner = &tag[1..tag.len() - 1];
    let (inner, self_closing) = match inner.strip_suffix('/') {
        Some(inner) => (inner, true),
        None => (inner, false),
    };
    let name_end = inner
        .find(|character: char| character.is_ascii_whitespace())
        .unwrap_or(inner.len());
    let name = &inner[..name_end];
    if !valid_qname(name) {
        return Err(malformed("invalid element name"));
    }
    let mut attributes: Vec<(String, String)> = Vec::new();
    let mut rest = &inner[name_end..];
    loop {
        rest = rest.trim_start_matches(|character: char| character.is_ascii_whitespace());
        if rest.is_empty() {
            break;
        }
        let equals = rest
            .find('=')
            .ok_or_else(|| malformed("attribute has no value"))?;
        let key = rest[..equals].trim_end();
        if !valid_qname(key) {
            return Err(malformed("invalid attribute name"));
        }
        let after = rest[equals + 1..]
            .trim_start_matches(|character: char| character.is_ascii_whitespace());
        if !after.starts_with('"') {
            // The exporter writes every attribute double-quoted; a
            // single-quoted one cannot come back the way it was written.
            return Err(DcsSchemaTemplateError::UnsupportedSource(
                "attribute values must be double-quoted",
            ));
        }
        let value_end = after[1..]
            .find('"')
            .ok_or_else(|| malformed("attribute value is not terminated"))?;
        let value = &after[1..1 + value_end];
        if value.contains('<') {
            return Err(malformed("attribute value contains `<`"));
        }
        if attributes.iter().any(|(existing, _)| existing == key) {
            return Err(malformed("duplicate attribute"));
        }
        attributes.push((key.to_string(), value.to_string()));
        rest = &after[1 + value_end + 1..];
    }
    Ok((
        Element {
            name: name.to_string(),
            attributes,
            children: Vec::new(),
        },
        self_closing,
    ))
}

fn valid_qname(name: &str) -> bool {
    let mut parts = name.split(':');
    let first = parts.next().unwrap_or_default();
    let second = parts.next();
    parts.next().is_none() && valid_ncname(first) && second.is_none_or(valid_ncname)
}

fn valid_ncname(name: &str) -> bool {
    let mut characters = name.chars();
    characters
        .next()
        .is_some_and(|first| first.is_alphabetic() || first == '_')
        && characters
            .all(|character| character.is_alphanumeric() || matches!(character, '_' | '-' | '.'))
}

// ---------------------------------------------------------------------------
// Namespace scopes.

type Scopes = Vec<Vec<(String, String)>>;

fn resolve_prefix<'a>(scopes: &'a [Vec<(String, String)>], prefix: &str) -> Option<&'a str> {
    scopes
        .iter()
        .rev()
        .find_map(|scope| {
            scope
                .iter()
                .rev()
                .find(|(declared, _)| declared == prefix)
                .map(|(_, uri)| uri.as_str())
        })
        .or(if prefix.is_empty() { Some("") } else { None })
}

/// Resolves `prefix` against `own` (an element's declarations not yet pushed)
/// and then `scopes`.
fn resolve_with<'a>(
    scopes: &'a [Vec<(String, String)>],
    own: &'a [(String, String)],
    prefix: &str,
) -> Option<&'a str> {
    own.iter()
        .rev()
        .find(|(declared, _)| declared == prefix)
        .map(|(_, uri)| uri.as_str())
        .or_else(|| resolve_prefix(scopes, prefix))
}

fn is_generated_prefix(prefix: &str) -> bool {
    let Some(rest) = prefix.strip_prefix('d') else {
        return false;
    };
    let Some((depth, point)) = rest.split_once('p') else {
        return false;
    };
    !depth.is_empty()
        && !point.is_empty()
        && depth.bytes().all(|byte| byte.is_ascii_digit())
        && point.bytes().all(|byte| byte.is_ascii_digit())
}

fn split_qname(value: &str) -> (&str, &str) {
    value.split_once(':').unwrap_or(("", value))
}

/// One element's storage declarations, accumulated in the order of need.
struct Declarations<'a> {
    depth: usize,
    above: &'a [Vec<(String, String)>],
    own: Vec<(String, String)>,
    generated: usize,
    /// The source's own point-of-use declarations on this element for
    /// namespaces outside the source root: (uri, source prefix).
    source: Vec<(String, String)>,
}

impl<'a> Declarations<'a> {
    fn new(
        depth: usize,
        above: &'a [Vec<(String, String)>],
        source: Vec<(String, String)>,
    ) -> Self {
        Self {
            depth,
            above,
            own: Vec::new(),
            generated: 0,
            source,
        }
    }

    /// The innermost prefix bound to `uri` that no inner declaration shadows.
    fn prefix_for(&self, uri: &str) -> Option<String> {
        let mut seen: Vec<&str> = Vec::new();
        let scopes = self
            .above
            .iter()
            .map(Vec::as_slice)
            .chain(std::iter::once(self.own.as_slice()));
        let scopes: Vec<&[(String, String)]> = scopes.collect();
        for scope in scopes.into_iter().rev() {
            for (declared, bound) in scope.iter().rev() {
                if seen.contains(&declared.as_str()) {
                    continue;
                }
                seen.push(declared);
                if !declared.is_empty() && bound == uri {
                    return Some(declared.clone());
                }
            }
        }
        None
    }

    fn resolve(&self, prefix: &str) -> Option<&str> {
        resolve_with(self.above, &self.own, prefix)
    }

    fn element_name(&mut self, uri: &str, local: &str) -> String {
        if let Some(prefix) = self.prefix_for(uri) {
            return format!("{prefix}:{local}");
        }
        if self.resolve("") == Some(uri) {
            return local.to_string();
        }
        self.own.push((String::new(), uri.to_string()));
        local.to_string()
    }

    fn qname(&mut self, uri: &str, local: &str, allow_default: bool) -> String {
        if uri == XS {
            return format!("xs:{local}");
        }
        if allow_default && self.resolve("") == Some(uri) {
            return local.to_string();
        }
        if let Some(prefix) = self.prefix_for(uri) {
            return format!("{prefix}:{local}");
        }
        let prefix = self.declare(uri);
        format!("{prefix}:{local}")
    }

    fn declare(&mut self, uri: &str) -> String {
        let source = self
            .source
            .iter()
            .find(|(bound, _)| bound == uri)
            .map(|(_, prefix)| prefix.clone());
        let prefix = match source {
            Some(prefix) if !is_generated_prefix(&prefix) => prefix,
            _ => match uri {
                SETTINGS => "dcsset".to_string(),
                DCS_CORE => "dcscor".to_string(),
                AREA_TEMPLATE => "dcsat".to_string(),
                _ => {
                    self.generated += 1;
                    format!("d{}p{}", self.depth, self.generated)
                }
            },
        };
        self.own.push((prefix.clone(), uri.to_string()));
        prefix
    }

    fn render(&self) -> String {
        let mut out = String::new();
        for (prefix, uri) in &self.own {
            if prefix.is_empty() {
                out.push_str(" xmlns=\"");
            } else {
                out.push_str(" xmlns:");
                out.push_str(prefix);
                out.push_str("=\"");
            }
            out.push_str(uri);
            out.push('"');
        }
        out
    }
}

// ---------------------------------------------------------------------------
// The writer.

struct Writer<'a> {
    style_items: &'a BTreeMap<String, String>,
    types: &'a dyn DcsStorageTypeResolver,
    type_ids: BTreeMap<String, DcsStorageTypeSpelling>,
    used_style_items: BTreeMap<String, String>,
    /// Rendered side-table appearances, in ordinal order.
    appearances: Vec<String>,
    /// Whether table-cell appearances go to the terminal side table.
    side_table: bool,
    nodes: usize,
}

/// One member of a run of data-core type siblings.
enum TypeMember<'e> {
    /// Written as the element it is: a builtin, or a reference spelled by
    /// name.
    Literal {
        element: &'e Element,
        /// The source QName, which is what orders a builtin.
        qname: String,
        reference: bool,
    },
    /// Written as a `TypeId`.
    Id {
        element: &'e Element,
        uuid: String,
        spelling: DcsStorageTypeSpelling,
    },
}

impl Writer<'_> {
    fn write_variant(
        &mut self,
        variant: &Element,
        source_scopes: &mut Scopes,
        storage_scopes: &mut Scopes,
        out: &mut String,
    ) -> Result<String, DcsSchemaTemplateError> {
        if !variant.plain_attributes().is_empty() {
            return Err(DcsSchemaTemplateError::UnsupportedSource(
                "a settingsVariant carries attributes",
            ));
        }
        let children = variant.element_children()?;
        let is_settings = |child: &Element, scopes: &Scopes| {
            child.local() == "settings"
                && resolve_with(scopes, &child.declarations(), child.prefix()) == Some(SETTINGS)
        };
        let settings_index = children
            .iter()
            .position(|child| is_settings(child, source_scopes))
            .ok_or(DcsSchemaTemplateError::UnsupportedSource(
                "a settingsVariant has no settings to frame",
            ))?;
        if settings_index + 1 != children.len() {
            return Err(DcsSchemaTemplateError::UnsupportedSource(
                "a settingsVariant's settings is not its last child",
            ));
        }
        out.push_str("<settingsVariant>");
        storage_scopes.push(Vec::new());
        let mut written = Ok(());
        for child in &children[..settings_index] {
            out.push_str("\r\n\t\t\t");
            written = self.write_element(
                child,
                "settingsVariant",
                4,
                3,
                source_scopes,
                storage_scopes,
                out,
            );
            if written.is_err() {
                break;
            }
        }
        storage_scopes.pop();
        written?;
        out.push_str("\r\n\t\t</settingsVariant>");
        self.settings_document(children[settings_index], source_scopes)
    }

    fn settings_document(
        &mut self,
        settings: &Element,
        source_scopes: &mut Scopes,
    ) -> Result<String, DcsSchemaTemplateError> {
        if !settings.plain_attributes().is_empty() {
            return Err(DcsSchemaTemplateError::UnsupportedSource(
                "an inline settings element carries attributes",
            ));
        }
        let mut out = String::from(DOCUMENT_HEAD);
        out.push_str("<Settings");
        for (prefix, uri) in SETTINGS_NAMESPACES {
            if prefix.is_empty() {
                out.push_str(" xmlns=\"");
            } else {
                out.push_str(" xmlns:");
                out.push_str(prefix);
                out.push_str("=\"");
            }
            out.push_str(uri);
            out.push('"');
        }
        let children = settings.element_children()?;
        if children.is_empty() {
            if settings.text().is_some_and(|text| !text.is_empty()) {
                return Err(DcsSchemaTemplateError::UnsupportedSource(
                    "an inline settings element carries character data",
                ));
            }
            out.push_str("/>");
            return Ok(out);
        }
        out.push('>');
        let mut storage_scopes = vec![
            SETTINGS_NAMESPACES
                .iter()
                .map(|(prefix, uri)| ((*prefix).to_string(), (*uri).to_string()))
                .collect::<Vec<_>>(),
        ];
        source_scopes.push(settings.declarations());
        let written = self.write_children(
            settings,
            &children,
            1,
            0,
            source_scopes,
            &mut storage_scopes,
            &mut out,
        );
        source_scopes.pop();
        written?;
        out.push_str("\r\n</Settings>");
        Ok(out)
    }

    /// Writes one source element and its subtree at storage `depth`, its
    /// opening tag indented by `level` tabs (already written by the caller).
    #[allow(clippy::too_many_arguments)]
    fn write_element(
        &mut self,
        element: &Element,
        parent_local: &str,
        depth: usize,
        level: usize,
        source_scopes: &mut Scopes,
        storage_scopes: &mut Scopes,
        out: &mut String,
    ) -> Result<(), DcsSchemaTemplateError> {
        self.nodes += 1;
        if depth > MAX_DEPTH || self.nodes > MAX_NODES {
            return Err(DcsSchemaTemplateError::UnsupportedSource(
                "DCS XML exceeds the bounded writer limits",
            ));
        }
        let own_source = element.declarations();
        let uri = resolve_with(source_scopes, &own_source, element.prefix())
            .ok_or(DcsSchemaTemplateError::UnsupportedSource(
                "an element uses an unbound namespace prefix",
            ))?
            .to_string();
        if uri.is_empty() {
            return Err(DcsSchemaTemplateError::UnsupportedSource(
                "an element is in no namespace",
            ));
        }
        if own_source
            .iter()
            .any(|(prefix, bound)| prefix.is_empty() && bound != &uri)
        {
            // A default declaration the element's own name does not use
            // would bind descendants the storage spelling knows nothing of.
            return Err(DcsSchemaTemplateError::UnsupportedSource(
                "a nested default namespace declaration is outside the source shape",
            ));
        }
        let point_of_use = own_source
            .iter()
            .filter(|(prefix, bound)| {
                !prefix.is_empty() && !SOURCE_ROOT_NAMESPACES.contains(&bound.as_str())
            })
            .map(|(prefix, bound)| (bound.clone(), prefix.clone()))
            .collect::<Vec<_>>();
        let mut declarations = Declarations::new(depth, storage_scopes, point_of_use);
        let name = declarations.element_name(&uri, element.local());
        let mut style_reference_only = false;
        let xsi_type = match element.attribute("xsi:type") {
            Some(value) => {
                let (prefix, local) = split_qname(value);
                resolve_with(source_scopes, &own_source, prefix)
                    .map(|uri| (uri.to_string(), local.to_string()))
            }
            None => None,
        };

        let mut attributes = String::new();
        for (key, value) in element.plain_attributes() {
            let (key_prefix, key_local) = split_qname(key);
            let key_out = if key_prefix.is_empty() {
                key.to_string()
            } else {
                let key_uri = resolve_with(source_scopes, &own_source, key_prefix).ok_or(
                    DcsSchemaTemplateError::UnsupportedSource(
                        "an attribute uses an unbound namespace prefix",
                    ),
                )?;
                if key_uri == XSI {
                    format!("xsi:{key_local}")
                } else {
                    let key_uri = key_uri.to_string();
                    declarations.qname(&key_uri, key_local, false)
                }
            };
            let (value_prefix, value_local) = split_qname(value);
            let value_out = if key_out == "xsi:type" {
                let value_uri = resolve_with(source_scopes, &own_source, value_prefix)
                    .ok_or(DcsSchemaTemplateError::UnsupportedSource(
                        "an xsi:type uses an unbound namespace prefix",
                    ))?
                    .to_string();
                if value_uri.is_empty() {
                    return Err(DcsSchemaTemplateError::UnsupportedSource(
                        "an xsi:type names a type in no namespace",
                    ));
                }
                declarations.qname(&value_uri, value_local, true)
            } else {
                match resolve_with(source_scopes, &own_source, value_prefix) {
                    Some(value_uri) if !value_prefix.is_empty() => {
                        let value_uri = value_uri.to_string();
                        if key_out == "ref"
                            && value_uri == STYLE
                            && !spelled_on_element(&own_source, value_prefix)
                            && let Some(uuid) = self.style_items.get(value_local)
                        {
                            style_reference_only = true;
                            self.used_style_items
                                .insert(uuid.to_ascii_lowercase(), value_local.to_string());
                            format!("0:{uuid}")
                        } else {
                            declarations.qname(&value_uri, value_local, false)
                        }
                    }
                    _ => value.to_string(),
                }
            };
            attributes.push(' ');
            attributes.push_str(&key_out);
            attributes.push_str("=\"");
            attributes.push_str(&value_out);
            attributes.push('"');
        }

        let children = element.element_children()?;
        let mut text_out = None::<String>;
        if children.is_empty() {
            let text = element.text().unwrap_or_default();
            if !text.is_empty() {
                let type_qname = (uri == DATA_CORE
                    && matches!(element.local(), "Type" | "TypeSet"))
                    || xsi_type
                        .as_ref()
                        .is_some_and(|(type_uri, local)| type_uri == DATA_CORE && local == "Type");
                let color = uri == DCS_CORE
                    && element.local() == "value"
                    && xsi_type
                        .as_ref()
                        .is_some_and(|(type_uri, local)| type_uri == DATA_UI && local == "Color");
                text_out = Some(if type_qname {
                    if text.trim() != text {
                        return Err(DcsSchemaTemplateError::UnsupportedSource(
                            "a type QName carries surrounding whitespace",
                        ));
                    }
                    let (prefix, local) = split_qname(&text);
                    let text_uri = resolve_with(source_scopes, &own_source, prefix)
                        .ok_or(DcsSchemaTemplateError::UnsupportedSource(
                            "a type QName uses an unbound namespace prefix",
                        ))?
                        .to_string();
                    declarations.qname(&text_uri, local, true)
                } else if color {
                    let (prefix, local) = split_qname(&text);
                    match resolve_with(source_scopes, &own_source, prefix) {
                        Some(text_uri) if !prefix.is_empty() => {
                            let text_uri = text_uri.to_string();
                            if text_uri == STYLE
                                && !spelled_on_element(&own_source, prefix)
                                && let Some(uuid) = self.style_items.get(local)
                            {
                                style_reference_only = true;
                                self.used_style_items
                                    .insert(uuid.to_ascii_lowercase(), local.to_string());
                                format!("0:{uuid}")
                            } else {
                                declarations.qname(&text_uri, local, false)
                            }
                        }
                        _ => text.clone(),
                    }
                } else {
                    text.clone()
                });
            }
        }

        if element.local() == "settings"
            && (uri == SETTINGS || (uri == SCHEMA && parent_local == "nestedSchema"))
        {
            // A `Settings` object re-declares its writer's namespace set.
            for (prefix, settings_uri) in &SETTINGS_NAMESPACES[1..] {
                if declarations.resolve(prefix) != Some(*settings_uri) {
                    declarations
                        .own
                        .push(((*prefix).to_string(), (*settings_uri).to_string()));
                }
            }
        }
        // A source point-of-use declaration no use on this element consumed
        // is carried over: the namespace is spelled by descendants.
        for (prefix, bound) in &own_source {
            if prefix.is_empty() || SOURCE_ROOT_NAMESPACES.contains(&bound.as_str()) {
                continue;
            }
            if bound == STYLE && style_reference_only {
                continue;
            }
            if declarations
                .own
                .iter()
                .any(|(_, declared)| declared == bound)
            {
                continue;
            }
            if is_generated_prefix(prefix) {
                declarations.generated += 1;
                let generated = format!("d{}p{}", depth, declarations.generated);
                declarations.own.push((generated, bound.clone()));
            } else {
                declarations.own.push((prefix.clone(), bound.clone()));
            }
        }

        out.push('<');
        out.push_str(&name);
        out.push_str(&declarations.render());
        out.push_str(&attributes);
        let own_storage = declarations.own;
        if !children.is_empty() {
            out.push('>');
            source_scopes.push(own_source);
            storage_scopes.push(own_storage);
            let written = self.write_children(
                element,
                &children,
                depth,
                level,
                source_scopes,
                storage_scopes,
                out,
            );
            storage_scopes.pop();
            source_scopes.pop();
            written?;
            out.push_str("\r\n");
            push_tabs(out, level);
            out.push_str("</");
            out.push_str(&name);
            out.push('>');
        } else if let Some(text) = text_out {
            out.push('>');
            out.push_str(&text);
            out.push_str("</");
            out.push_str(&name);
            out.push('>');
        } else {
            out.push_str("/>");
        }
        Ok(())
    }

    /// Writes the children of `parent` at storage `depth + 1`, the parent's
    /// own tag being indented by `level` tabs.
    #[allow(clippy::too_many_arguments)]
    fn write_children(
        &mut self,
        parent: &Element,
        children: &[&Element],
        depth: usize,
        level: usize,
        source_scopes: &mut Scopes,
        storage_scopes: &mut Scopes,
        out: &mut String,
    ) -> Result<(), DcsSchemaTemplateError> {
        let table_cell = self.side_table
            && parent.local() == "tableCell"
            && resolve_prefix(source_scopes, parent.prefix()) == Some(AREA_TEMPLATE);
        let mut index = 0usize;
        let mut appearance = None::<&Element>;
        while index < children.len() {
            let child = children[index];
            if is_type_member(child, source_scopes) {
                let mut end = index;
                while end < children.len() && is_type_member(children[end], source_scopes) {
                    end += 1;
                }
                self.write_type_run(
                    parent.local(),
                    &children[index..end],
                    depth,
                    level,
                    source_scopes,
                    storage_scopes,
                    out,
                )?;
                index = end;
                continue;
            }
            if table_cell
                && child.local() == "appearance"
                && resolve_with(source_scopes, &child.declarations(), child.prefix())
                    == Some(AREA_TEMPLATE)
            {
                if index + 1 != children.len() {
                    return Err(DcsSchemaTemplateError::UnsupportedSource(
                        "a table cell appearance is not the cell's last child",
                    ));
                }
                appearance = Some(child);
                index += 1;
                continue;
            }
            out.push_str("\r\n");
            push_tabs(out, level + 1);
            self.write_element(
                child,
                parent.local(),
                depth + 1,
                level + 1,
                source_scopes,
                storage_scopes,
                out,
            )?;
            index += 1;
        }
        if let Some(appearance) = appearance {
            let ordinal = self.side_table_ordinal(appearance, source_scopes)?;
            out.push_str("\r\n");
            push_tabs(out, level + 1);
            let mut declarations = Declarations::new(depth + 1, storage_scopes, Vec::new());
            let name = declarations.element_name(AREA_TEMPLATE, "appIndex");
            out.push('<');
            out.push_str(&name);
            out.push_str(&declarations.render());
            out.push('>');
            out.push_str(&ordinal.to_string());
            out.push_str("</");
            out.push_str(&name);
            out.push('>');
        }
        Ok(())
    }

    /// Renders one table-cell appearance as a side-table entry and returns its
    /// ordinal, sharing the entry of an identical earlier one.
    fn side_table_ordinal(
        &mut self,
        appearance: &Element,
        source_scopes: &mut Scopes,
    ) -> Result<usize, DcsSchemaTemplateError> {
        if !appearance.plain_attributes().is_empty() {
            return Err(DcsSchemaTemplateError::UnsupportedSource(
                "a table cell appearance carries attributes",
            ));
        }
        let items = appearance.element_children()?;
        if items.is_empty() {
            return Err(DcsSchemaTemplateError::UnsupportedSource(
                "an empty table cell appearance has no source spelling",
            ));
        }
        let mut storage_scopes = vec![
            vec![
                (String::new(), String::new()),
                ("xs".to_string(), XS.to_string()),
                ("xsi".to_string(), XSI.to_string()),
            ],
            vec![(String::new(), AREA_TEMPLATE.to_string())],
        ];
        let mut body = String::from("\r\n\t<appearance xmlns=\"");
        body.push_str(AREA_TEMPLATE);
        body.push_str("\" xsi:type=\"TableCellAppearance\">");
        let side_table = std::mem::replace(&mut self.side_table, false);
        source_scopes.push(appearance.declarations());
        let written = self.write_children(
            appearance,
            &items,
            2,
            1,
            source_scopes,
            &mut storage_scopes,
            &mut body,
        );
        source_scopes.pop();
        self.side_table = side_table;
        written?;
        body.push_str("\r\n\t</appearance>");
        if let Some(ordinal) = self
            .appearances
            .iter()
            .position(|existing| *existing == body)
        {
            return Ok(ordinal);
        }
        self.appearances.push(body);
        Ok(self.appearances.len() - 1)
    }

    #[allow(clippy::too_many_arguments)]
    fn write_type_run(
        &mut self,
        parent_local: &str,
        run: &[&Element],
        depth: usize,
        level: usize,
        source_scopes: &mut Scopes,
        storage_scopes: &mut Scopes,
        out: &mut String,
    ) -> Result<(), DcsSchemaTemplateError> {
        let mut members = Vec::with_capacity(run.len());
        for element in run {
            members.push(self.type_member(element, source_scopes)?);
        }
        // Storage writes the literal types first and the type ids after, by
        // uuid; the exporter merges them back. A run that merge would not
        // restore keeps its references spelled by name, which the exporter
        // leaves in place.
        let mut order = storage_order(&members);
        if !restores_source_order(&members, &order) {
            members =
                members
                    .into_iter()
                    .map(|member| match member {
                        TypeMember::Id {
                            element,
                            spelling:
                                DcsStorageTypeSpelling::Type(name)
                                | DcsStorageTypeSpelling::TypeSet(name),
                            ..
                        } => TypeMember::Literal {
                            element,
                            qname: name,
                            reference: true,
                        },
                        other => other,
                    })
                    .collect();
            order = storage_order(&members);
            if !restores_source_order(&members, &order) {
                return Err(DcsSchemaTemplateError::UnsupportedSource(
                    "a type list's order cannot be restored from any storage spelling",
                ));
            }
        }
        for index in order {
            out.push_str("\r\n");
            push_tabs(out, level + 1);
            match &members[index] {
                TypeMember::Literal { element, .. } => {
                    self.write_element(
                        element,
                        parent_local,
                        depth + 1,
                        level + 1,
                        source_scopes,
                        storage_scopes,
                        out,
                    )?;
                }
                TypeMember::Id { uuid, spelling, .. } => {
                    self.type_ids
                        .insert(uuid.to_ascii_lowercase(), spelling.clone());
                    let mut declarations = Declarations::new(depth + 1, storage_scopes, Vec::new());
                    let name = declarations.element_name(DATA_CORE, "TypeId");
                    out.push('<');
                    out.push_str(&name);
                    out.push_str(&declarations.render());
                    out.push('>');
                    out.push_str(uuid);
                    out.push_str("</");
                    out.push_str(&name);
                    out.push('>');
                }
            }
        }
        Ok(())
    }

    fn type_member<'e>(
        &mut self,
        element: &'e Element,
        source_scopes: &Scopes,
    ) -> Result<TypeMember<'e>, DcsSchemaTemplateError> {
        if !element.plain_attributes().is_empty() {
            return Err(DcsSchemaTemplateError::UnsupportedSource(
                "a type list member carries attributes",
            ));
        }
        let text = element
            .text()
            .ok_or(DcsSchemaTemplateError::UnsupportedSource(
                "a type list member has element content",
            ))?;
        let own = element.declarations();
        if element.local() == "TypeId" {
            if !own.is_empty() || text.trim() != text || text.is_empty() || text.contains('&') {
                return Err(DcsSchemaTemplateError::UnsupportedSource(
                    "a TypeId is outside its evidenced spelling",
                ));
            }
            return Ok(TypeMember::Id {
                element,
                uuid: text,
                spelling: DcsStorageTypeSpelling::Kept,
            });
        }
        let (prefix, local) = split_qname(text.trim());
        let uri = resolve_with(source_scopes, &own, prefix).unwrap_or_default();
        if uri == CURRENT_CONFIG && !local.contains('&') {
            let resolved = if element.local() == "TypeSet" {
                REFERENCE_FAMILIES
                    .iter()
                    .find(|(family, _)| *family == local)
                    .map(|(_, uuid)| (*uuid).to_string())
                    .or_else(|| self.types.generated_type_id(local))
                    .map(|uuid| (uuid, DcsStorageTypeSpelling::TypeSet(local.to_string())))
            } else {
                self.types
                    .generated_type_id(local)
                    .map(|uuid| (uuid, DcsStorageTypeSpelling::Type(local.to_string())))
            };
            if let Some((uuid, spelling)) = resolved {
                return Ok(TypeMember::Id {
                    element,
                    uuid,
                    spelling,
                });
            }
            return Ok(TypeMember::Literal {
                element,
                qname: local.to_string(),
                reference: true,
            });
        }
        let source_prefix = match uri {
            XS => "xs",
            DATA_CORE => "v8",
            DATA_UI => "v8ui",
            _ => "",
        };
        Ok(TypeMember::Literal {
            element,
            qname: if source_prefix.is_empty() {
                text.trim().to_string()
            } else {
                format!("{source_prefix}:{local}")
            },
            reference: false,
        })
    }
}

/// Storage's order of a type run: every literal in source order, then every
/// type id ascending by uuid.
fn storage_order(members: &[TypeMember<'_>]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..members.len())
        .filter(|index| matches!(members[*index], TypeMember::Literal { .. }))
        .collect();
    let mut ids: Vec<(String, usize)> = members
        .iter()
        .enumerate()
        .filter_map(|(index, member)| match member {
            TypeMember::Id { uuid, .. } => Some((uuid.to_ascii_lowercase(), index)),
            TypeMember::Literal { .. } => None,
        })
        .collect();
    ids.sort();
    order.extend(ids.into_iter().map(|(_, index)| index));
    order
}

/// Whether the exporter's merge of a run written in `order` yields the source
/// order (`0..members.len()`).
fn restores_source_order(members: &[TypeMember<'_>], order: &[usize]) -> bool {
    if members
        .iter()
        .all(|member| matches!(member, TypeMember::Literal { .. }))
    {
        return order.iter().copied().eq(0..members.len());
    }
    let keyed: Vec<TypeRunMember<'_>> = order
        .iter()
        .map(|index| match &members[*index] {
            TypeMember::Literal {
                qname, reference, ..
            } => {
                if *reference {
                    // A reference spelled by name sits in storage's literal
                    // group with no key of its own.
                    TypeRunMember::Builtin("")
                } else {
                    TypeRunMember::Builtin(qname.as_str())
                }
            }
            TypeMember::Id { uuid, spelling, .. } => match spelling {
                DcsStorageTypeSpelling::Type(_) => TypeRunMember::Reference(uuid.as_str()),
                DcsStorageTypeSpelling::TypeSet(_) => TypeRunMember::Family,
                DcsStorageTypeSpelling::Kept => TypeRunMember::Unevidenced,
            },
        })
        .collect();
    let Some(permutation) = evidenced_type_run_permutation(&keyed) else {
        return false;
    };
    permutation
        .iter()
        .map(|slot| order[*slot])
        .eq(0..members.len())
}

/// Whether `prefix` is one the element itself declares under a name the
/// platform spelled rather than generated (`style`, not `d7p1`).
///
/// A configuration style item the platform stored by uuid comes back from the
/// exporter under a prefix already in scope or under a generated one it
/// declares; a prefix the element itself spells by name can only have come
/// from storage holding the reference by name -- the platform does that in
/// the side table of an area template (`style:ВажнаяНадписьШрифт` in ERP УХ
/// `InformationRegisters/ДоступныеЗначенияЭлементовКонструктораВидовПродукцииИС`),
/// so such a reference stays the QName it is.
fn spelled_on_element(own: &[(String, String)], prefix: &str) -> bool {
    !is_generated_prefix(prefix) && own.iter().any(|(declared, _)| declared == prefix)
}

fn is_type_member(element: &Element, source_scopes: &Scopes) -> bool {
    matches!(element.local(), "Type" | "TypeSet" | "TypeId")
        && resolve_with(source_scopes, &element.declarations(), element.prefix()) == Some(DATA_CORE)
}

fn push_tabs(out: &mut String, count: usize) {
    for _ in 0..count {
        out.push('\t');
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROOT: &str = "<DataCompositionSchema xmlns=\"http://v8.1c.ru/8.1/data-composition-system/schema\" xmlns:dcscom=\"http://v8.1c.ru/8.1/data-composition-system/common\" xmlns:dcscor=\"http://v8.1c.ru/8.1/data-composition-system/core\" xmlns:dcsset=\"http://v8.1c.ru/8.1/data-composition-system/settings\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">";
    const INLINE_SETTINGS: &str = "<dcsset:settings xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\" xmlns:sys=\"http://v8.1c.ru/8.1/data/ui/fonts/system\" xmlns:web=\"http://v8.1c.ru/8.1/data/ui/colors/web\" xmlns:win=\"http://v8.1c.ru/8.1/data/ui/colors/windows\"";
    const VARIANT: [&str; 5] = [
        "\t<settingsVariant>",
        "\t\t<dcsset:name>Основной</dcsset:name>",
        "\t\t<dcsset:presentation xsi:type=\"xs:string\">Основной</dcsset:presentation>",
        "\t\t{inline}/>",
        "\t</settingsVariant>",
    ];

    fn source(children: &[&str]) -> Vec<u8> {
        let mut lines = vec![
            "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>".to_string(),
            ROOT.to_string(),
        ];
        lines.extend(
            children
                .iter()
                .map(|line| line.replace("{inline}", INLINE_SETTINGS)),
        );
        lines.push("</DataCompositionSchema>".to_string());
        lines.join("\r\n").into_bytes()
    }

    fn storage(children: &[&str]) -> String {
        let mut lines = vec![
            format!("{DOCUMENT_HEAD}{SCHEMA_FILE_OPEN}"),
            format!("\t<dataCompositionSchema xmlns=\"{SCHEMA}\">"),
        ];
        lines.extend(children.iter().map(|line| (*line).to_string()));
        lines.push("\t</dataCompositionSchema>".to_string());
        lines.push("</SchemaFile>".to_string());
        lines.join("\r\n")
    }

    const STORED_VARIANT: [&str; 4] = [
        "\t\t<settingsVariant>",
        "\t\t\t<name xmlns=\"http://v8.1c.ru/8.1/data-composition-system/settings\">Основной</name>",
        "\t\t\t<presentation xmlns=\"http://v8.1c.ru/8.1/data-composition-system/settings\" xsi:type=\"xs:string\">Основной</presentation>",
        "\t\t</settingsVariant>",
    ];

    fn no_types() -> impl Fn(&str) -> Option<String> {
        |_: &str| None
    }

    fn compile(source: &[u8], types: &dyn DcsStorageTypeResolver) -> DcsStorageDocuments {
        compile_dcs_schema_storage_documents(source, &BTreeMap::new(), types).unwrap()
    }

    fn primary(compiled: &DcsStorageDocuments) -> &str {
        std::str::from_utf8(compiled.documents().primary_schema_file()).unwrap()
    }

    #[test]
    fn declares_every_namespace_where_it_is_first_needed() {
        let mut children = vec![
            "\t<dataSet xsi:type=\"DataSetQuery\">",
            "\t\t<name>НаборДанных1</name>",
            "\t\t<field xsi:type=\"DataSetFieldField\">",
            "\t\t\t<dataPath>Сумма</dataPath>",
            "\t\t\t<field>Сумма</field>",
            "\t\t\t<title xsi:type=\"v8:LocalStringType\">",
            "\t\t\t\t<v8:item>",
            "\t\t\t\t\t<v8:lang>ru</v8:lang>",
            "\t\t\t\t\t<v8:content>Сумма</v8:content>",
            "\t\t\t\t</v8:item>",
            "\t\t\t</title>",
            "\t\t\t<role>",
            "\t\t\t\t<dcscom:dimension>true</dcscom:dimension>",
            "\t\t\t</role>",
            "\t\t\t<appearance>",
            "\t\t\t\t<dcscor:item xsi:type=\"dcsset:SettingsParameterValue\">",
            "\t\t\t\t\t<dcscor:parameter>Формат</dcscor:parameter>",
            "\t\t\t\t\t<dcscor:value xsi:type=\"xs:string\">ЧДЦ=2</dcscor:value>",
            "\t\t\t\t</dcscor:item>",
            "\t\t\t</appearance>",
            "\t\t</field>",
            "\t\t<query>ВЫБРАТЬ\r\n\t1 КАК Сумма</query>",
            "\t</dataSet>",
        ];
        children.extend(VARIANT);
        let compiled = compile(&source(&children), &no_types());
        let mut expected = vec![
            "\t\t<dataSet xsi:type=\"DataSetQuery\">",
            "\t\t\t<name>НаборДанных1</name>",
            "\t\t\t<field xsi:type=\"DataSetFieldField\">",
            "\t\t\t\t<dataPath>Сумма</dataPath>",
            "\t\t\t\t<field>Сумма</field>",
            "\t\t\t\t<title xmlns:d5p1=\"http://v8.1c.ru/8.1/data/core\" xsi:type=\"d5p1:LocalStringType\">",
            "\t\t\t\t\t<d5p1:item>",
            "\t\t\t\t\t\t<d5p1:lang>ru</d5p1:lang>",
            "\t\t\t\t\t\t<d5p1:content>Сумма</d5p1:content>",
            "\t\t\t\t\t</d5p1:item>",
            "\t\t\t\t</title>",
            "\t\t\t\t<role>",
            "\t\t\t\t\t<dimension xmlns=\"http://v8.1c.ru/8.1/data-composition-system/common\">true</dimension>",
            "\t\t\t\t</role>",
            "\t\t\t\t<appearance>",
            "\t\t\t\t\t<item xmlns=\"http://v8.1c.ru/8.1/data-composition-system/core\" xmlns:dcsset=\"http://v8.1c.ru/8.1/data-composition-system/settings\" xsi:type=\"dcsset:SettingsParameterValue\">",
            "\t\t\t\t\t\t<parameter>Формат</parameter>",
            "\t\t\t\t\t\t<value xsi:type=\"xs:string\">ЧДЦ=2</value>",
            "\t\t\t\t\t</item>",
            "\t\t\t\t</appearance>",
            "\t\t\t</field>",
            // Character data keeps its own line breaks and indentation.
            "\t\t\t<query>ВЫБРАТЬ\r\n\t1 КАК Сумма</query>",
            "\t\t</dataSet>",
        ];
        expected.extend(STORED_VARIANT);
        assert_eq!(primary(&compiled), storage(&expected));
        assert_eq!(
            compiled.documents().settings(),
            [format!(
                "{DOCUMENT_HEAD}<Settings xmlns=\"{SETTINGS}\" xmlns:dcscor=\"{DCS_CORE}\" xmlns:style=\"{STYLE}\" xmlns:sys=\"{FONTS_SYSTEM}\" xmlns:v8=\"{DATA_CORE}\" xmlns:v8ui=\"{DATA_UI}\" xmlns:web=\"{COLORS_WEB}\" xmlns:win=\"{COLORS_WINDOWS}\" xmlns:xs=\"{XS}\" xmlns:xsi=\"{XSI}\"/>"
            )
            .into_bytes()]
        );
        assert_eq!(
            compiled.documents().terminal_schema_file(),
            format!("{DOCUMENT_HEAD}{SCHEMA_FILE_OPEN}\r\n\t<dataCompositionSchema xmlns=\"{SCHEMA}\"/>\r\n</SchemaFile>")
                .as_bytes()
        );
    }

    #[test]
    fn writes_configuration_types_by_type_id_behind_the_literal_types() {
        let mut children = vec![
            "\t<parameter>",
            "\t\t<name>Счет</name>",
            "\t\t<valueType>",
            "\t\t\t<v8:Type>xs:string</v8:Type>",
            "\t\t\t<v8:Type xmlns:d4p1=\"http://v8.1c.ru/8.1/data/enterprise/current-config\">d4p1:CatalogRef.А</v8:Type>",
            "\t\t\t<v8:Type xmlns:d4p1=\"http://v8.1c.ru/8.1/data/enterprise/current-config\">d4p1:CatalogRef.Б</v8:Type>",
            "\t\t\t<v8:TypeSet xmlns:d4p1=\"http://v8.1c.ru/8.1/data/enterprise/current-config\">d4p1:DocumentRef</v8:TypeSet>",
            "\t\t\t<v8:StringQualifiers>",
            "\t\t\t\t<v8:Length>10</v8:Length>",
            "\t\t\t\t<v8:AllowedLength>Variable</v8:AllowedLength>",
            "\t\t\t</v8:StringQualifiers>",
            "\t\t</valueType>",
            "\t</parameter>",
        ];
        children.extend(VARIANT);
        let source = source(&children);
        let types = |name: &str| match name {
            "CatalogRef.А" => Some("aaaaaaaa-0000-4000-8000-000000000001".to_string()),
            "CatalogRef.Б" => Some("bbbbbbbb-0000-4000-8000-000000000002".to_string()),
            _ => None,
        };
        let compiled = compile(&source, &types);
        let mut expected = vec![
            "\t\t<parameter>",
            "\t\t\t<name>Счет</name>",
            "\t\t\t<valueType>",
            "\t\t\t\t<Type xmlns=\"http://v8.1c.ru/8.1/data/core\">xs:string</Type>",
            "\t\t\t\t<TypeId xmlns=\"http://v8.1c.ru/8.1/data/core\">38bfd075-3e63-4aaa-a93e-94521380d579</TypeId>",
            "\t\t\t\t<TypeId xmlns=\"http://v8.1c.ru/8.1/data/core\">aaaaaaaa-0000-4000-8000-000000000001</TypeId>",
            "\t\t\t\t<TypeId xmlns=\"http://v8.1c.ru/8.1/data/core\">bbbbbbbb-0000-4000-8000-000000000002</TypeId>",
            "\t\t\t\t<StringQualifiers xmlns=\"http://v8.1c.ru/8.1/data/core\">",
            "\t\t\t\t\t<Length>10</Length>",
            "\t\t\t\t\t<AllowedLength>Variable</AllowedLength>",
            "\t\t\t\t</StringQualifiers>",
            "\t\t\t</valueType>",
            "\t\t</parameter>",
        ];
        expected.extend(STORED_VARIANT);
        assert_eq!(primary(&compiled), storage(&expected));
        assert_eq!(
            compiled
                .type_ids()
                .get("aaaaaaaa-0000-4000-8000-000000000001"),
            Some(&DcsStorageTypeSpelling::Type("CatalogRef.А".to_string()))
        );
        assert_eq!(
            compiled
                .type_ids()
                .get("38bfd075-3e63-4aaa-a93e-94521380d579"),
            Some(&DcsStorageTypeSpelling::TypeSet("DocumentRef".to_string()))
        );

        // A type the configuration does not resolve stays spelled by name.
        let unresolved = compile(&source, &no_types());
        assert!(primary(&unresolved).contains(
            "<Type xmlns=\"http://v8.1c.ru/8.1/data/core\" xmlns:d5p1=\"http://v8.1c.ru/8.1/data/enterprise/current-config\">d5p1:CatalogRef.Б</Type>"
        ));
        assert!(
            !unresolved
                .type_ids()
                .contains_key("aaaaaaaa-0000-4000-8000-000000000001")
        );

        // References out of uuid order cannot come back from a TypeId run,
        // which the exporter orders by uuid, so such a run keeps them, and
        // the family beside them, spelled by name.
        let reordered = String::from_utf8(source.clone())
            .unwrap()
            .replacen("CatalogRef.А", "CatalogRef.Tmp", 1)
            .replacen("CatalogRef.Б", "CatalogRef.А", 1)
            .replacen("CatalogRef.Tmp", "CatalogRef.Б", 1);
        let kept_by_name = compile(reordered.as_bytes(), &types);
        let stored = primary(&kept_by_name);
        assert!(
            stored.find("d5p1:CatalogRef.Б</Type>").unwrap()
                < stored.find("d5p1:CatalogRef.А</Type>").unwrap()
        );
        assert!(stored.contains(
            "<TypeSet xmlns=\"http://v8.1c.ru/8.1/data/core\" xmlns:d5p1=\"http://v8.1c.ru/8.1/data/enterprise/current-config\">d5p1:DocumentRef</TypeSet>"
        ));
        assert!(!stored.contains("<TypeId"));
        assert!(kept_by_name.type_ids().is_empty());
    }

    #[test]
    fn moves_area_templates_to_the_terminal_document_and_shares_equal_appearances() {
        let cell = [
            "\t\t\t\t<dcsat:tableCell>",
            "\t\t\t\t\t<dcsat:item xsi:type=\"dcsat:Field\">",
            "\t\t\t\t\t\t<dcsat:value xsi:type=\"dcscor:Parameter\">П</dcsat:value>",
            "\t\t\t\t\t</dcsat:item>",
            "\t\t\t\t\t<dcsat:appearance>",
            "\t\t\t\t\t\t<dcscor:item>",
            "\t\t\t\t\t\t\t<dcscor:parameter>МинимальнаяШирина</dcscor:parameter>",
            "\t\t\t\t\t\t\t<dcscor:value xsi:type=\"xs:decimal\">2</dcscor:value>",
            "\t\t\t\t\t\t</dcscor:item>",
            "\t\t\t\t\t</dcsat:appearance>",
            "\t\t\t\t</dcsat:tableCell>",
        ];
        let mut children = vec![
            "\t<template>",
            "\t\t<name>Макет1</name>",
            "\t\t<template xmlns:dcsat=\"http://v8.1c.ru/8.1/data-composition-system/area-template\" xsi:type=\"dcsat:AreaTemplate\">",
            "\t\t\t<dcsat:item xsi:type=\"dcsat:TableRow\">",
        ];
        children.extend(cell);
        children.extend(cell);
        children.extend(["\t\t\t</dcsat:item>", "\t\t</template>", "\t</template>"]);
        children.extend(VARIANT);
        let compiled = compile(&source(&children), &no_types());
        assert_eq!(primary(&compiled), storage(&STORED_VARIANT));
        let stored_cell = [
            "\t\t\t\t\t<dcsat:tableCell>",
            "\t\t\t\t\t\t<dcsat:item xsi:type=\"dcsat:Field\">",
            "\t\t\t\t\t\t\t<dcsat:value xmlns:dcscor=\"http://v8.1c.ru/8.1/data-composition-system/core\" xsi:type=\"dcscor:Parameter\">П</dcsat:value>",
            "\t\t\t\t\t\t</dcsat:item>",
            "\t\t\t\t\t\t<dcsat:appIndex>0</dcsat:appIndex>",
            "\t\t\t\t\t</dcsat:tableCell>",
        ];
        let mut expected = vec![
            format!("{DOCUMENT_HEAD}{SCHEMA_FILE_OPEN}"),
            format!("\t<dataCompositionSchema xmlns=\"{SCHEMA}\">"),
            "\t\t<template>".to_string(),
            "\t\t\t<name>Макет1</name>".to_string(),
            "\t\t\t<template xmlns:dcsat=\"http://v8.1c.ru/8.1/data-composition-system/area-template\" xsi:type=\"dcsat:AreaTemplate\">".to_string(),
            "\t\t\t\t<dcsat:item xsi:type=\"dcsat:TableRow\">".to_string(),
        ];
        expected.extend(stored_cell.iter().map(|line| (*line).to_string()));
        expected.extend(stored_cell.iter().map(|line| (*line).to_string()));
        expected.extend(
            [
                "\t\t\t\t</dcsat:item>",
                "\t\t\t</template>",
                "\t\t</template>",
                "\t</dataCompositionSchema>",
                "\t<appearance xmlns=\"http://v8.1c.ru/8.1/data-composition-system/area-template\" xsi:type=\"TableCellAppearance\">",
                "\t\t<item xmlns=\"http://v8.1c.ru/8.1/data-composition-system/core\">",
                "\t\t\t<parameter>МинимальнаяШирина</parameter>",
                "\t\t\t<value xsi:type=\"xs:decimal\">2</value>",
                "\t\t</item>",
                "\t</appearance>",
                "</SchemaFile>",
            ]
            .iter()
            .map(|line| (*line).to_string()),
        );
        assert_eq!(
            std::str::from_utf8(compiled.documents().terminal_schema_file()).unwrap(),
            expected.join("\r\n")
        );
    }

    #[test]
    fn stores_a_configuration_style_item_by_uuid_and_a_platform_one_by_name() {
        let mut children = VARIANT[..3].to_vec();
        children.extend([
            "\t\t{inline}>",
            "\t\t\t<dcsset:conditionalAppearance>",
            "\t\t\t\t<dcsset:item>",
            "\t\t\t\t\t<dcsset:appearance>",
            "\t\t\t\t\t\t<dcscor:item xsi:type=\"dcsset:SettingsParameterValue\">",
            "\t\t\t\t\t\t\t<dcscor:parameter>ЦветТекста</dcscor:parameter>",
            "\t\t\t\t\t\t\t<dcscor:value xsi:type=\"v8ui:Color\">style:ЦветОсобогоТекста</dcscor:value>",
            "\t\t\t\t\t\t</dcscor:item>",
            "\t\t\t\t\t\t<dcscor:item xsi:type=\"dcsset:SettingsParameterValue\">",
            "\t\t\t\t\t\t\t<dcscor:parameter>ЦветФона</dcscor:parameter>",
            "\t\t\t\t\t\t\t<dcscor:value xsi:type=\"v8ui:Color\">style:ToolTipBackColor</dcscor:value>",
            "\t\t\t\t\t\t</dcscor:item>",
            "\t\t\t\t\t</dcsset:appearance>",
            "\t\t\t\t</dcsset:item>",
            "\t\t\t</dcsset:conditionalAppearance>",
            "\t\t</dcsset:settings>",
            "\t</settingsVariant>",
        ]);
        let mut style_items = BTreeMap::new();
        style_items.insert(
            "ЦветОсобогоТекста".to_string(),
            "0da019ca-1fc7-4aff-8998-3ddcccf10872".to_string(),
        );
        let compiled =
            compile_dcs_schema_storage_documents(&source(&children), &style_items, &no_types())
                .unwrap();
        let settings = std::str::from_utf8(&compiled.documents().settings()[0]).unwrap();
        assert!(settings.contains(
            "<dcscor:value xsi:type=\"v8ui:Color\">0:0da019ca-1fc7-4aff-8998-3ddcccf10872</dcscor:value>"
        ));
        assert!(settings.contains(
            "<dcscor:value xsi:type=\"v8ui:Color\">style:ToolTipBackColor</dcscor:value>"
        ));
        assert_eq!(
            compiled
                .style_items()
                .get("0da019ca-1fc7-4aff-8998-3ddcccf10872")
                .map(String::as_str),
            Some("ЦветОсобогоТекста")
        );
    }

    #[test]
    fn refuses_what_storage_cannot_carry() {
        let refused = |children: &[&str]| {
            matches!(
                compile_dcs_schema_storage_documents(
                    &source(children),
                    &BTreeMap::new(),
                    &no_types()
                ),
                Err(DcsSchemaTemplateError::UnsupportedSource(_))
            )
        };
        // No settingsVariant: nothing to frame a Settings document for.
        assert!(refused(&["\t<dataSource>", "\t</dataSource>"]));
        // A settingsVariant whose settings is not its last child.
        assert!(refused(&[
            "\t<settingsVariant>",
            "\t\t{inline}/>",
            "\t\t<dcsset:name>Основной</dcsset:name>",
            "\t</settingsVariant>",
        ]));
        // Markup the export never writes.
        let mut commented = vec!["\t<!-- note -->"];
        commented.extend(VARIANT);
        assert!(refused(&commented));
        // A table cell appearance ahead of the cell's content cannot be
        // spelled by an ordinal at the end of the cell.
        let mut misplaced = vec![
            "\t<template>",
            "\t\t<name>М</name>",
            "\t\t<template xmlns:dcsat=\"http://v8.1c.ru/8.1/data-composition-system/area-template\" xsi:type=\"dcsat:AreaTemplate\">",
            "\t\t\t<dcsat:item xsi:type=\"dcsat:TableRow\">",
            "\t\t\t\t<dcsat:tableCell>",
            "\t\t\t\t\t<dcsat:appearance>",
            "\t\t\t\t\t\t<dcscor:item>",
            "\t\t\t\t\t\t\t<dcscor:parameter>Ширина</dcscor:parameter>",
            "\t\t\t\t\t\t\t<dcscor:value xsi:type=\"xs:decimal\">2</dcscor:value>",
            "\t\t\t\t\t\t</dcscor:item>",
            "\t\t\t\t\t</dcsat:appearance>",
            "\t\t\t\t\t<dcsat:item xsi:type=\"dcsat:Field\">",
            "\t\t\t\t\t\t<dcsat:value xsi:type=\"dcscor:Parameter\">П</dcsat:value>",
            "\t\t\t\t\t</dcsat:item>",
            "\t\t\t\t</dcsat:tableCell>",
            "\t\t\t</dcsat:item>",
            "\t\t</template>",
            "\t</template>",
        ];
        misplaced.extend(VARIANT);
        assert!(refused(&misplaced));
    }
}
