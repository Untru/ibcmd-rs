//! Strict byte-slice XML reader.
use std::fmt;

use quick_xml::{Reader, events::Event};

use crate::node::{
    Attribute, AttributeKind, QName, XmlCData, XmlComment, XmlDeclaration, XmlDocument, XmlElement,
    XmlNode, XmlRawNode, XmlText,
};

/// Reason a document could not be read.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum XmlErrorCause {
    InvalidUtf8,
    Parser(String),
    InvalidName(String),
    DuplicateAttribute(String),
    MultipleRoots,
    TextOutsideRoot,
    UnsupportedOutsideRoot(String),
    InvalidDocTypePosition,
    /// A general entity other than an XML predefined or numeric reference.
    UnresolvedEntity(String),
    InvalidCharacter,
    InvalidDeclaration(String),
    UnclosedElement(String),
    MissingRoot,
}
/// Position-aware XML error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct XmlError {
    offset: usize,
    line: usize,
    column: usize,
    cause: XmlErrorCause,
}
impl XmlError {
    pub fn offset(&self) -> usize {
        self.offset
    }
    pub fn line(&self) -> usize {
        self.line
    }
    pub fn column(&self) -> usize {
        self.column
    }
    pub fn cause(&self) -> &XmlErrorCause {
        &self.cause
    }
}
impl fmt::Display for XmlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "XML error at {}:{} (byte {}): {:?}",
            self.line, self.column, self.offset, self.cause
        )
    }
}
impl std::error::Error for XmlError {}

/// Reader for a UTF-8 XML document.
pub struct XmlReader;
impl XmlReader {
    /// Parses a complete UTF-8 XML document without trimming text.
    pub fn from_slice(input: &[u8]) -> Result<XmlDocument, XmlError> {
        if std::str::from_utf8(input).is_err() {
            return Err(error(input, 0, XmlErrorCause::InvalidUtf8));
        }
        for (offset, value) in std::str::from_utf8(input)
            .expect("checked above")
            .char_indices()
        {
            if !valid_xml_char(value) {
                return Err(error(input, offset, XmlErrorCause::InvalidCharacter));
            }
        }
        if input.starts_with(&[0xef, 0xbb, 0xbf]) {
            return Err(error(
                input,
                0,
                XmlErrorCause::Parser("UTF-8 BOM is unsupported".into()),
            ));
        }
        let mut reader = Reader::from_reader(input);
        reader.config_mut().trim_text(false);
        reader.config_mut().check_end_names = true;
        reader.config_mut().check_comments = true;
        let mut buf = Vec::new();
        let mut stack: Vec<Building> = Vec::new();
        let mut declaration = None;
        let mut before = Vec::new();
        let mut after = Vec::new();
        let mut root = None;
        let mut has_doctype = false;
        loop {
            let start = reader.buffer_position() as usize;
            let event = match reader.read_event_into(&mut buf) {
                Ok(e) => e.into_owned(),
                Err(e) => {
                    return Err(error(
                        input,
                        reader.error_position() as usize,
                        XmlErrorCause::Parser(e.to_string()),
                    ));
                }
            };
            let pos = reader.buffer_position() as usize;
            match event {
                Event::Eof => break,
                Event::Decl(e) => {
                    if !stack.is_empty()
                        || root.is_some()
                        || declaration.is_some()
                        || !before.is_empty()
                    {
                        return Err(error(
                            input,
                            start,
                            XmlErrorCause::UnsupportedOutsideRoot(
                                "XML declaration position".into(),
                            ),
                        ));
                    }
                    let raw = lexeme(input, start, pos)?;
                    validate_declaration(input, start, &raw)?;
                    declaration = Some(XmlDeclaration::parsed(
                        bytes(input, start, e.as_ref())?,
                        raw,
                    ));
                }
                Event::Start(e) => {
                    let element = element(input, start, e.name().as_ref(), &e)?;
                    stack.push(Building {
                        element,
                        children: Vec::new(),
                        raw_start: lexeme(input, start, pos)?,
                    });
                }
                Event::Empty(e) => {
                    let element = element(input, start, e.name().as_ref(), &e)?;
                    push_element(
                        input,
                        pos,
                        XmlElement::parsed(
                            element.name,
                            element.attributes,
                            Vec::new(),
                            lexeme(input, start, pos)?,
                            None,
                            true,
                        ),
                        &mut stack,
                        &mut root,
                    )?;
                }
                Event::End(_) => {
                    let Some(building) = stack.pop() else {
                        return Err(error(
                            input,
                            pos,
                            XmlErrorCause::Parser("unexpected closing tag".into()),
                        ));
                    };
                    push_element(
                        input,
                        pos,
                        XmlElement::parsed(
                            building.element.name,
                            building.element.attributes,
                            building.children,
                            building.raw_start,
                            Some(lexeme(input, start, pos)?),
                            false,
                        ),
                        &mut stack,
                        &mut root,
                    )?;
                }
                Event::Text(e) => {
                    let encoded = bytes(input, start, e.as_ref())?;
                    let value = quick_xml::escape::unescape(&encoded)
                        .map_err(|x| error(input, pos, XmlErrorCause::Parser(x.to_string())))?
                        .into_owned();
                    push_node(
                        input,
                        pos,
                        XmlNode::Text(XmlText::parsed(value, lexeme(input, start, pos)?)),
                        &mut stack,
                        &mut before,
                        &mut after,
                        root.is_some(),
                    )?;
                }
                Event::CData(e) => push_node(
                    input,
                    pos,
                    XmlNode::CData(XmlCData::parsed(
                        bytes(input, start, e.as_ref())?,
                        lexeme(input, start, pos)?,
                    )),
                    &mut stack,
                    &mut before,
                    &mut after,
                    root.is_some(),
                )?,
                Event::Comment(e) => push_node(
                    input,
                    pos,
                    XmlNode::Comment(XmlComment::parsed(
                        bytes(input, start, e.as_ref())?,
                        lexeme(input, start, pos)?,
                    )),
                    &mut stack,
                    &mut before,
                    &mut after,
                    root.is_some(),
                )?,
                Event::PI(e) => push_node(
                    input,
                    pos,
                    XmlNode::ProcessingInstruction(XmlRawNode::parsed(
                        bytes(input, start, e.as_ref())?,
                        lexeme(input, start, pos)?,
                    )),
                    &mut stack,
                    &mut before,
                    &mut after,
                    root.is_some(),
                )?,
                Event::DocType(e) => {
                    if !stack.is_empty() || root.is_some() || has_doctype {
                        return Err(error(input, start, XmlErrorCause::InvalidDocTypePosition));
                    }
                    has_doctype = true;
                    before.push(XmlNode::DocType(XmlRawNode::parsed(
                        bytes(input, start, e.as_ref())?,
                        lexeme(input, start, pos)?,
                    )));
                }
                Event::GeneralRef(e) => push_node(
                    input,
                    pos,
                    XmlNode::Text(XmlText::parsed(
                        resolve_reference(input, start, &bytes(input, start, e.as_ref())?)?,
                        lexeme(input, start, pos)?,
                    )),
                    &mut stack,
                    &mut before,
                    &mut after,
                    root.is_some(),
                )?,
            }
            buf.clear();
        }
        if let Some(open) = stack.last() {
            return Err(error(
                input,
                input.len(),
                XmlErrorCause::UnclosedElement(open.element.name.raw().into()),
            ));
        }
        let Some(root) = root else {
            return Err(error(input, input.len(), XmlErrorCause::MissingRoot));
        };
        Ok(XmlDocument::parsed(declaration, before, root, after))
    }
}
struct Bare {
    name: QName,
    attributes: Vec<Attribute>,
}
struct Building {
    element: Bare,
    children: Vec<XmlNode>,
    raw_start: String,
}
fn element(
    input: &[u8],
    pos: usize,
    name: &[u8],
    event: &quick_xml::events::BytesStart<'_>,
) -> Result<Bare, XmlError> {
    let name = qname(input, pos, name)?;
    let mut attributes = Vec::new();
    for attribute in event.attributes().with_checks(true) {
        let attribute =
            attribute.map_err(|e| error(input, pos, XmlErrorCause::Parser(e.to_string())))?;
        let key = qname(input, pos, attribute.key.as_ref())?;
        if attributes
            .iter()
            .any(|a: &Attribute| matches!(a.kind(), AttributeKind::Ordinary(n) if n == &key))
        {
            return Err(error(
                input,
                pos,
                XmlErrorCause::DuplicateAttribute(key.raw().into()),
            ));
        }
        let raw = bytes(input, pos, attribute.value.as_ref())?;
        let value = quick_xml::escape::unescape(&raw)
            .map_err(|e| error(input, pos, XmlErrorCause::Parser(e.to_string())))?
            .into_owned();
        if !value.chars().all(valid_xml_char) {
            return Err(error(input, pos, XmlErrorCause::InvalidCharacter));
        }
        let kind = if key.raw() == "xmlns" {
            AttributeKind::Namespace(None)
        } else if key.prefix() == Some("xmlns") {
            AttributeKind::Namespace(Some(key.local().into()))
        } else {
            AttributeKind::Ordinary(key)
        };
        attributes.push(Attribute { kind, value });
    }
    Ok(Bare { name, attributes })
}
fn qname(input: &[u8], pos: usize, raw: &[u8]) -> Result<QName, XmlError> {
    let raw = bytes(input, pos, raw)?;
    QName::new(raw.clone()).map_err(|_| error(input, pos, XmlErrorCause::InvalidName(raw)))
}
fn bytes(input: &[u8], pos: usize, value: &[u8]) -> Result<String, XmlError> {
    std::str::from_utf8(value)
        .map(str::to_owned)
        .map_err(|_| error(input, pos, XmlErrorCause::InvalidUtf8))
}
fn lexeme(input: &[u8], start: usize, end: usize) -> Result<String, XmlError> {
    bytes(
        input,
        start,
        &input[start.min(input.len())..end.min(input.len())],
    )
}
fn resolve_reference(input: &[u8], pos: usize, reference: &str) -> Result<String, XmlError> {
    let value = match reference {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        number if number.starts_with("#x") => {
            return numeric_reference(input, pos, &number[2..], 16);
        }
        number if number.starts_with('#') => {
            return numeric_reference(input, pos, &number[1..], 10);
        }
        _ => {
            return Err(error(
                input,
                pos,
                XmlErrorCause::UnresolvedEntity(reference.to_owned()),
            ));
        }
    };
    Ok(value
        .expect("predefined references are characters")
        .to_string())
}
fn numeric_reference(
    input: &[u8],
    pos: usize,
    digits: &str,
    radix: u32,
) -> Result<String, XmlError> {
    let value = u32::from_str_radix(digits, radix)
        .ok()
        .and_then(char::from_u32)
        .filter(|value| valid_xml_char(*value))
        .ok_or_else(|| error(input, pos, XmlErrorCause::InvalidCharacter))?;
    Ok(value.to_string())
}
fn valid_xml_char(value: char) -> bool {
    matches!(value, '\u{9}' | '\u{a}' | '\u{d}')
        || ('\u{20}'..='\u{d7ff}').contains(&value)
        || ('\u{e000}'..='\u{fffd}').contains(&value)
        || ('\u{10000}'..='\u{10ffff}').contains(&value)
}
fn validate_declaration(input: &[u8], pos: usize, raw: &str) -> Result<(), XmlError> {
    let content = raw
        .strip_prefix("<?xml")
        .and_then(|value| value.strip_suffix("?>"))
        .ok_or_else(|| declaration_error(input, pos, "malformed delimiters"))?;
    let attributes = declaration_attributes(input, pos, content)?;
    let keys: Vec<&str> = attributes.iter().map(|(key, _)| key.as_str()).collect();
    if !matches!(
        keys.as_slice(),
        ["version"]
            | ["version", "encoding"]
            | ["version", "standalone"]
            | ["version", "encoding", "standalone"]
    ) {
        return Err(declaration_error(
            input,
            pos,
            "attributes are missing, unknown, duplicated, or out of order",
        ));
    }
    if attributes[0].1 != "1.0" {
        return Err(declaration_error(
            input,
            pos,
            "only XML version 1.0 is supported",
        ));
    }
    for (key, value) in &attributes[1..] {
        match key.as_str() {
            "encoding" if valid_encoding_name(value) && value.eq_ignore_ascii_case("utf-8") => {}
            "encoding" => {
                return Err(declaration_error(
                    input,
                    pos,
                    "encoding must be a valid UTF-8 encoding name",
                ));
            }
            "standalone" if value == "yes" || value == "no" => {}
            "standalone" => {
                return Err(declaration_error(
                    input,
                    pos,
                    "standalone must be yes or no",
                ));
            }
            _ => unreachable!("attribute order checked above"),
        }
    }
    Ok(())
}
fn declaration_attributes(
    input: &[u8],
    pos: usize,
    content: &str,
) -> Result<Vec<(String, String)>, XmlError> {
    let bytes = content.as_bytes();
    let mut index = 0;
    let mut result = Vec::new();
    while index < bytes.len() {
        let whitespace_start = index;
        while index < bytes.len() && is_xml_space(bytes[index]) {
            index += 1;
        }
        if whitespace_start == index {
            return Err(declaration_error(
                input,
                pos,
                "attributes must be separated by whitespace",
            ));
        }
        if index == bytes.len() {
            break;
        }
        let key_start = index;
        while index < bytes.len() && bytes[index].is_ascii_alphabetic() {
            index += 1;
        }
        if key_start == index {
            return Err(declaration_error(input, pos, "invalid attribute name"));
        }
        let key = &content[key_start..index];
        while index < bytes.len() && is_xml_space(bytes[index]) {
            index += 1;
        }
        if bytes.get(index) != Some(&b'=') {
            return Err(declaration_error(
                input,
                pos,
                "attribute is missing equals sign",
            ));
        }
        index += 1;
        while index < bytes.len() && is_xml_space(bytes[index]) {
            index += 1;
        }
        let Some(&quote @ (b'\'' | b'"')) = bytes.get(index) else {
            return Err(declaration_error(
                input,
                pos,
                "attribute value must be quoted",
            ));
        };
        index += 1;
        let value_start = index;
        while index < bytes.len() && bytes[index] != quote {
            index += 1;
        }
        if index == bytes.len() {
            return Err(declaration_error(
                input,
                pos,
                "unterminated attribute value",
            ));
        }
        result.push((key.to_owned(), content[value_start..index].to_owned()));
        index += 1;
    }
    Ok(result)
}
fn is_xml_space(value: u8) -> bool {
    matches!(value, b' ' | b'\t' | b'\r' | b'\n')
}
fn valid_encoding_name(value: &str) -> bool {
    let mut bytes = value.bytes();
    bytes
        .next()
        .is_some_and(|value| value.is_ascii_alphabetic())
        && bytes.all(|value| value.is_ascii_alphanumeric() || matches!(value, b'.' | b'_' | b'-'))
}
fn declaration_error(input: &[u8], pos: usize, message: &str) -> XmlError {
    error(
        input,
        pos,
        XmlErrorCause::InvalidDeclaration(message.to_owned()),
    )
}
fn push_element(
    input: &[u8],
    pos: usize,
    element: XmlElement,
    stack: &mut [Building],
    root: &mut Option<XmlElement>,
) -> Result<(), XmlError> {
    if let Some(parent) = stack.last_mut() {
        parent.children.push(XmlNode::Element(element));
    } else if root.replace(element).is_some() {
        return Err(error(input, pos, XmlErrorCause::MultipleRoots));
    }
    Ok(())
}
fn push_node(
    input: &[u8],
    pos: usize,
    node: XmlNode,
    stack: &mut [Building],
    before: &mut Vec<XmlNode>,
    after: &mut Vec<XmlNode>,
    has_root: bool,
) -> Result<(), XmlError> {
    if let Some(parent) = stack.last_mut() {
        parent.children.push(node);
        return Ok(());
    }
    match node {
        XmlNode::Text(ref s) if s.value().trim().is_empty() => {
            if has_root {
                after.push(node)
            } else {
                before.push(node)
            };
            Ok(())
        }
        XmlNode::Comment(_) | XmlNode::ProcessingInstruction(_) | XmlNode::DocType(_)
            if !has_root =>
        {
            before.push(node);
            Ok(())
        }
        XmlNode::Comment(_) | XmlNode::ProcessingInstruction(_) if has_root => {
            after.push(node);
            Ok(())
        }
        _ => Err(error(input, pos, XmlErrorCause::TextOutsideRoot)),
    }
}
fn error(input: &[u8], offset: usize, cause: XmlErrorCause) -> XmlError {
    let point = offset.min(input.len());
    let prefix = &input[..point];
    let line = prefix.iter().filter(|&&b| b == b'\n').count() + 1;
    let column = point
        - prefix
            .iter()
            .rposition(|&b| b == b'\n')
            .map_or(0, |i| i + 1)
        + 1;
    XmlError {
        offset: point,
        line,
        column,
        cause,
    }
}
