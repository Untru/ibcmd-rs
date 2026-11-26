//! Typed ConfigDumpInfo version-inventory codec, independent of database rows.
//!
//! Stored row IDs and configVersion values are opaque here. The adapter owns
//! their extraction and metadata-name routing; this module owns XML grammar,
//! attribute escaping and the measured native lexical form.
use quick_xml::{
    Reader,
    events::{BytesStart, Event},
};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

pub const FILE_NAME: &str = "ConfigDumpInfo.xml";
pub const HIERARCHICAL: &str = "Hierarchical";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeVersion {
    V2_20,
    V2_21,
}
impl NativeVersion {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::V2_20 => "2.20",
            Self::V2_21 => "2.21",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Part {
    pub name: String,
    pub id: String,
}

/// A versioned stored row and its unversioned metadata parts, in source order.
/// Parts use full metadata references; their row ownership is the semantic
/// relation, including when a reference file nests parts more deeply.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entry {
    pub name: String,
    pub id: String,
    pub config_version: String,
    pub children: Vec<Part>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ConfigDumpInfo {
    pub layout: Option<String>,
    pub format_version: Option<String>,
    pub entries: Vec<Entry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodecError(String);
impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for CodecError {}
fn invalid(message: impl Into<String>) -> CodecError {
    CodecError(message.into())
}
fn xml_character(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\r' | '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}')
}
fn attributes(start: &BytesStart<'_>) -> Result<BTreeMap<String, String>, CodecError> {
    let mut result = BTreeMap::new();
    for attribute in start.attributes() {
        let attribute = attribute.map_err(|e| invalid(format!("bad XML attribute: {e}")))?;
        let qualified =
            std::str::from_utf8(attribute.key.as_ref()).map_err(|e| invalid(e.to_string()))?;
        crate::QName::new(qualified).map_err(|e| invalid(e.to_string()))?;
        let key = std::str::from_utf8(attribute.key.local_name().as_ref())
            .map_err(|e| invalid(format!("bad XML attribute name: {e}")))?
            .to_owned();
        let value = attribute
            .unescape_value()
            .map_err(|e| invalid(format!("bad XML attribute value: {e}")))?
            .into_owned();
        if !value.chars().all(xml_character) {
            return Err(invalid("invalid XML character in attribute"));
        }
        if result.insert(key, value).is_some() {
            return Err(invalid("duplicate XML attribute local name"));
        }
    }
    Ok(result)
}
fn required(attrs: &mut BTreeMap<String, String>, key: &str) -> Result<String, CodecError> {
    attrs
        .remove(key)
        .ok_or_else(|| invalid(format!("a <Metadata> of <ConfigVersions> has no {key}")))
}

impl ConfigDumpInfo {
    pub fn native(version: NativeVersion, entries: Vec<Entry>) -> Self {
        Self {
            layout: Some(HIERARCHICAL.into()),
            format_version: Some(version.as_str().into()),
            entries,
        }
    }

    /// Streaming parse: large native inventories need no XML DOM or raw tree.
    /// An inventory must have exactly one ConfigVersions container. Versioned
    /// row IDs must be unique; part IDs retain their original source order.
    pub fn parse(xml: &[u8]) -> Result<Self, CodecError> {
        let xml = xml.strip_prefix(b"\xef\xbb\xbf").unwrap_or(xml);
        let text =
            std::str::from_utf8(xml).map_err(|e| invalid(format!("invalid UTF-8 XML: {e}")))?;
        if !text.chars().all(xml_character) {
            return Err(invalid("invalid XML character"));
        }
        let mut reader = Reader::from_reader(xml);
        reader.config_mut().check_comments = true;
        let mut buffer = Vec::new();
        let mut doc = Self::default();
        let mut ids = BTreeSet::new();
        let mut open = Vec::<String>::new();
        let mut root_seen = false;
        let mut versions_seen = false;
        let mut declaration_seen = false;
        loop {
            let event_start = reader.buffer_position() as usize;
            let event = reader.read_event_into(&mut buffer).map_err(|e| {
                invalid(format!(
                    "XML parse error at {}: {e}",
                    reader.error_position()
                ))
            })?;
            let (start, empty) = match &event {
                Event::Start(start) => (start, false),
                Event::Empty(start) => (start, true),
                Event::End(_) => {
                    open.pop();
                    buffer.clear();
                    continue;
                }
                Event::Eof => {
                    if let Some(name) = open.last() {
                        return Err(invalid(format!("the file ended inside <{name}>")));
                    }
                    break;
                }
                Event::Text(text) => {
                    let text = text
                        .decode()
                        .map_err(|e| invalid(format!("bad XML text: {e}")))?;
                    if !text.chars().all(|c| matches!(c, ' ' | '\t' | '\r' | '\n')) {
                        return Err(invalid("ConfigDumpInfo contains unexpected text"));
                    }
                    buffer.clear();
                    continue;
                }
                Event::Decl(_) => {
                    if root_seen || declaration_seen || event_start != 0 {
                        return Err(invalid("misplaced XML declaration"));
                    }
                    declaration_seen = true;
                    let raw =
                        std::str::from_utf8(&xml[event_start..reader.buffer_position() as usize])
                            .map_err(|e| invalid(e.to_string()))?;
                    crate::reader::validate_declaration(xml, event_start, raw)
                        .map_err(|e| invalid(e.to_string()))?;
                    buffer.clear();
                    continue;
                }
                Event::Comment(_) | Event::PI(_) => {
                    buffer.clear();
                    continue;
                }
                _ => return Err(invalid("unsupported ConfigDumpInfo XML content")),
            };
            let qualified = std::str::from_utf8(start.name().as_ref())
                .map_err(|e| invalid(e.to_string()))?
                .to_owned();
            crate::QName::new(qualified).map_err(|e| invalid(e.to_string()))?;
            let name = std::str::from_utf8(start.local_name().as_ref())
                .map_err(|e| invalid(e.to_string()))?
                .to_owned();
            match open.len() {
                0 => {
                    if root_seen || name != "ConfigDumpInfo" {
                        return Err(invalid(format!(
                            "the root element is <{name}>, not <ConfigDumpInfo>"
                        )));
                    }
                    root_seen = true;
                    let attrs = attributes(start)?;
                    doc.layout = attrs.get("format").cloned();
                    doc.format_version = attrs.get("version").cloned();
                }
                1 if name == "ConfigVersions" => {
                    if versions_seen {
                        return Err(invalid("the file lists ConfigVersions twice"));
                    }
                    attributes(start)?;
                    versions_seen = true;
                }
                2 if name == "Metadata" && open[1] == "ConfigVersions" => {
                    let mut attrs = attributes(start)?;
                    let entry = Entry {
                        name: required(&mut attrs, "name")?,
                        id: required(&mut attrs, "id")?,
                        config_version: required(&mut attrs, "configVersion")?,
                        children: Vec::new(),
                    };
                    if !ids.insert(entry.id.clone()) {
                        return Err(invalid(format!("the entry {} is listed twice", entry.id)));
                    }
                    doc.entries.push(entry);
                }
                depth if depth > 2 && name == "Metadata" && open[1] == "ConfigVersions" => {
                    let mut attrs = attributes(start)?;
                    let part = Part {
                        name: required(&mut attrs, "name")?,
                        id: required(&mut attrs, "id")?,
                    };
                    doc.entries
                        .last_mut()
                        .ok_or_else(|| invalid("metadata part has no row"))?
                        .children
                        .push(part);
                }
                _ => return Err(invalid(format!("unexpected ConfigDumpInfo element {name}"))),
            }
            if !empty {
                open.push(name);
            }
            buffer.clear();
        }
        if !root_seen {
            return Err(invalid("the file holds no <ConfigDumpInfo>"));
        }
        if !versions_seen {
            return Err(invalid("the file holds no <ConfigVersions>"));
        }
        Ok(doc)
    }

    /// Canonical native bytes: BOM, CRLF, stable attribute order, tabs and no
    /// trailing newline. Empty ConfigVersions uses the native self-closing form.
    pub fn to_native_bytes(&self) -> Result<Vec<u8>, CodecError> {
        if self.layout.as_deref() != Some(HIERARCHICAL) {
            return Err(invalid("unsupported ConfigDumpInfo layout"));
        }
        let version = match self.format_version.as_deref() {
            Some("2.20") => NativeVersion::V2_20,
            Some("2.21") => NativeVersion::V2_21,
            _ => return Err(invalid("unsupported ConfigDumpInfo native version")),
        };
        let mut xml = String::from("\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n");
        xml.push_str("<ConfigDumpInfo xmlns=\"http://v8.1c.ru/8.3/xcf/dumpinfo\" xmlns:xen=\"http://v8.1c.ru/8.3/xcf/enums\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" format=\"Hierarchical\" version=\"");
        xml.push_str(version.as_str());
        xml.push_str("\">\r\n");
        if self.entries.is_empty() {
            xml.push_str("\t<ConfigVersions/>\r\n</ConfigDumpInfo>");
            return Ok(xml.into_bytes());
        }
        xml.push_str("\t<ConfigVersions>\r\n");
        let mut ids = BTreeSet::new();
        for entry in &self.entries {
            if !ids.insert(&entry.id) {
                return Err(invalid(format!("the entry {} is listed twice", entry.id)));
            }
            xml.push_str("\t\t<Metadata name=\"");
            escape(&entry.name, &mut xml)?;
            xml.push_str("\" id=\"");
            escape(&entry.id, &mut xml)?;
            xml.push_str("\" configVersion=\"");
            escape(&entry.config_version, &mut xml)?;
            if entry.children.is_empty() {
                xml.push_str("\"/>\r\n");
                continue;
            }
            xml.push_str("\">\r\n");
            for part in &entry.children {
                xml.push_str("\t\t\t<Metadata name=\"");
                escape(&part.name, &mut xml)?;
                xml.push_str("\" id=\"");
                escape(&part.id, &mut xml)?;
                xml.push_str("\"/>\r\n");
            }
            xml.push_str("\t\t</Metadata>\r\n");
        }
        xml.push_str("\t</ConfigVersions>\r\n</ConfigDumpInfo>");
        Ok(xml.into_bytes())
    }
}
fn escape(value: &str, xml: &mut String) -> Result<(), CodecError> {
    for c in value.chars() {
        if !xml_character(c) {
            return Err(invalid("invalid XML character in attribute"));
        }
        match c {
            '&' => xml.push_str("&amp;"),
            '<' => xml.push_str("&lt;"),
            '>' => xml.push_str("&gt;"),
            '"' => xml.push_str("&quot;"),
            _ => xml.push(c),
        }
    }
    Ok(())
}
