//! Validating XML serializer.
use crate::node::{AttributeKind, XmlDocument, XmlElement, XmlNode, valid_name};
use std::collections::HashSet;
use std::fmt;

const MAX_OUTPUT_BYTES: usize = 33_554_432;
const XML_NAMESPACE: &str = "http://www.w3.org/XML/1998/namespace";
const XMLNS_NAMESPACE: &str = "http://www.w3.org/2000/xmlns/";

/// Serialization representation policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LexicalPolicy {
    Preserve,
    Normalized,
}
/// Serialization error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WriteError(String);
impl fmt::Display for WriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for WriteError {}
/// Stateless XML writer.
pub struct XmlWriter;
impl XmlWriter {
    /// Validates and returns all bytes; no caller-owned output is modified on failure.
    pub fn to_vec(document: &XmlDocument, policy: LexicalPolicy) -> Result<Vec<u8>, WriteError> {
        validate_document(document)?;
        let normalized_len = document_output_len(document, LexicalPolicy::Normalized)?;
        enforce_output_limit(normalized_len)?;
        let output_len = if policy == LexicalPolicy::Normalized {
            normalized_len
        } else {
            let output_len = document_output_len(document, policy)?;
            enforce_output_limit(output_len)?;
            output_len
        };
        // Preflight independently so an invalid generated subtree never yields partial bytes.
        let mut scratch = String::with_capacity(normalized_len);
        if let Some(decl) = document.declaration() {
            xml_chars(decl, "XML declaration")?;
            if decl.contains("?>") {
                return Err(WriteError("invalid XML declaration".into()));
            }
            scratch.push_str("<?");
            scratch.push_str(decl);
            scratch.push_str("?>");
        }
        for node in document.before_root() {
            node_out(node, &mut scratch, LexicalPolicy::Normalized)?;
        }
        element_out(document.root(), &mut scratch, LexicalPolicy::Normalized)?;
        for node in document.after_root() {
            node_out(node, &mut scratch, LexicalPolicy::Normalized)?;
        }
        debug_assert_eq!(scratch.len(), normalized_len);
        drop(scratch);
        let mut out = String::with_capacity(output_len);
        if policy == LexicalPolicy::Preserve
            && let Some(raw) = document.declaration_raw()
        {
            out.push_str(raw);
        } else if let Some(decl) = document.declaration() {
            out.push_str("<?");
            out.push_str(decl);
            out.push_str("?>");
        }
        for node in document.before_root() {
            node_out(node, &mut out, policy)?;
        }
        element_out(document.root(), &mut out, policy)?;
        for node in document.after_root() {
            node_out(node, &mut out, policy)?;
        }
        debug_assert_eq!(out.len(), output_len);
        Ok(out.into_bytes())
    }
}

/// Returns the exact retained lexeme of one parsed node, or its normalized
/// spelling for generated nodes. Kept crate-private for opaque-slot capture.
pub(crate) fn node_to_vec(node: &XmlNode, policy: LexicalPolicy) -> Result<Vec<u8>, WriteError> {
    match node {
        XmlNode::Element(element) => validate_element(element)?,
        _ => validate_node_value(node)?,
    }
    let normalized_len = node_output_len(node, LexicalPolicy::Normalized)?;
    enforce_output_limit(normalized_len)?;
    let output_len = if policy == LexicalPolicy::Normalized {
        normalized_len
    } else {
        let output_len = node_output_len(node, policy)?;
        enforce_output_limit(output_len)?;
        output_len
    };
    let mut scratch = String::with_capacity(normalized_len);
    match node {
        XmlNode::Element(element) => element_out(element, &mut scratch, LexicalPolicy::Normalized)?,
        _ => node_out(node, &mut scratch, LexicalPolicy::Normalized)?,
    }
    debug_assert_eq!(scratch.len(), normalized_len);
    drop(scratch);
    let mut out = String::with_capacity(output_len);
    match node {
        XmlNode::Element(element) => element_out(element, &mut out, policy)?,
        _ => node_out(node, &mut out, policy)?,
    }
    debug_assert_eq!(out.len(), output_len);
    Ok(out.into_bytes())
}

/// Returns the exact bounded size of an element start-tag projection.
pub(crate) fn element_start_len(
    element: &XmlElement,
    policy: LexicalPolicy,
) -> Result<usize, WriteError> {
    if use_raw_start(element, policy) {
        return Ok(element.raw_start().expect("checked by use_raw_start").len());
    }
    normalized_element_start_len(element)
}

/// Serializes only a borrowed element start tag. Parsed Preserve mode returns
/// its exact source lexeme; generated trees use deterministic normalized XML.
pub(crate) fn element_start_to_vec(
    element: &XmlElement,
    policy: LexicalPolicy,
) -> Result<Vec<u8>, WriteError> {
    validate_element_start(element)?;
    let normalized_len = element_start_len(element, LexicalPolicy::Normalized)?;
    enforce_output_limit(normalized_len)?;
    let output_len = element_start_len(element, policy)?;
    enforce_output_limit(output_len)?;
    if use_raw_start(element, policy) {
        return Ok(element
            .raw_start()
            .expect("checked by use_raw_start")
            .as_bytes()
            .to_vec());
    }
    let mut out = String::with_capacity(output_len);
    normalized_element_start_out(element, &mut out);
    debug_assert_eq!(out.len(), output_len);
    Ok(out.into_bytes())
}

fn enforce_output_limit(length: usize) -> Result<(), WriteError> {
    if length > MAX_OUTPUT_BYTES {
        return Err(WriteError(format!(
            "XML output exceeds {MAX_OUTPUT_BYTES} bytes"
        )));
    }
    Ok(())
}

fn add_output_len(total: &mut usize, value: usize) -> Result<(), WriteError> {
    *total = total
        .checked_add(value)
        .ok_or_else(|| WriteError("XML output length overflow".into()))?;
    Ok(())
}

fn escaped_output_len(value: &str, attribute: bool) -> Result<usize, WriteError> {
    value.chars().try_fold(0usize, |total, character| {
        let width = match character {
            '&' => 5,
            '<' | '>' => 4,
            '"' | '\'' if attribute => 6,
            _ => character.len_utf8(),
        };
        total
            .checked_add(width)
            .ok_or_else(|| WriteError("XML output length overflow".into()))
    })
}

fn document_output_len(document: &XmlDocument, policy: LexicalPolicy) -> Result<usize, WriteError> {
    let mut total = if policy == LexicalPolicy::Preserve {
        document.declaration_raw().map_or_else(
            || document.declaration().map_or(0, |value| value.len() + 4),
            str::len,
        )
    } else {
        document.declaration().map_or(0, |value| value.len() + 4)
    };
    for node in document.before_root() {
        add_output_len(&mut total, node_output_len(node, policy)?)?;
    }
    add_output_len(&mut total, element_output_len(document.root(), policy)?)?;
    for node in document.after_root() {
        add_output_len(&mut total, node_output_len(node, policy)?)?;
    }
    Ok(total)
}

fn node_output_len(node: &XmlNode, policy: LexicalPolicy) -> Result<usize, WriteError> {
    if policy == LexicalPolicy::Preserve
        && let Some(raw) = node.raw()
    {
        return Ok(raw.len());
    }
    match node {
        XmlNode::Element(element) => element_output_len(element, policy),
        XmlNode::Text(value) => escaped_output_len(value.value(), false),
        XmlNode::CData(value) => value
            .value()
            .len()
            .checked_add(12)
            .ok_or_else(|| WriteError("XML output length overflow".into())),
        XmlNode::Comment(value) => value
            .value()
            .len()
            .checked_add(7)
            .ok_or_else(|| WriteError("XML output length overflow".into())),
        XmlNode::ProcessingInstruction(value) => value
            .value()
            .len()
            .checked_add(4)
            .ok_or_else(|| WriteError("XML output length overflow".into())),
        XmlNode::DocType(value) => value
            .value()
            .len()
            .checked_add(11)
            .ok_or_else(|| WriteError("XML output length overflow".into())),
    }
}

fn normalized_element_start_len(element: &XmlElement) -> Result<usize, WriteError> {
    let mut total = element
        .name()
        .raw()
        .len()
        .checked_add(1)
        .ok_or_else(|| WriteError("XML output length overflow".into()))?;
    for attribute in element.attributes() {
        let name_len = match attribute.kind() {
            AttributeKind::Ordinary(name) => name.raw().len(),
            AttributeKind::Namespace(None) => 5,
            AttributeKind::Namespace(Some(prefix)) => 6 + prefix.len(),
        };
        add_output_len(&mut total, 1 + name_len + 3)?;
        add_output_len(&mut total, escaped_output_len(attribute.value(), true)?)?;
    }
    add_output_len(
        &mut total,
        if element.children().is_empty() { 2 } else { 1 },
    )?;
    Ok(total)
}

fn use_raw_start(element: &XmlElement, policy: LexicalPolicy) -> bool {
    policy == LexicalPolicy::Preserve
        && element.raw_start().is_some()
        && (element.children().is_empty() || element.raw_end().is_some())
}

fn element_output_len(element: &XmlElement, policy: LexicalPolicy) -> Result<usize, WriteError> {
    let preserve_raw_start = use_raw_start(element, policy);
    let mut total = if preserve_raw_start {
        element.raw_start().expect("checked above").len()
    } else {
        let mut total = element
            .name()
            .raw()
            .len()
            .checked_add(1)
            .ok_or_else(|| WriteError("XML output length overflow".into()))?;
        for attribute in element.attributes() {
            let name_len = match attribute.kind() {
                AttributeKind::Ordinary(name) => name.raw().len(),
                AttributeKind::Namespace(None) => 5,
                AttributeKind::Namespace(Some(prefix)) => 6 + prefix.len(),
            };
            add_output_len(&mut total, 1 + name_len + 3)?;
            add_output_len(&mut total, escaped_output_len(attribute.value(), true)?)?;
        }
        total
    };
    if element.children().is_empty() {
        let suffix_len = if preserve_raw_start {
            element.raw_end().map_or(0, str::len)
        } else {
            2
        };
        add_output_len(&mut total, suffix_len)?;
        return Ok(total);
    }
    if !preserve_raw_start {
        add_output_len(&mut total, 1)?;
    }
    for child in element.children() {
        add_output_len(&mut total, node_output_len(child, policy)?)?;
    }
    let suffix_len = if policy == LexicalPolicy::Preserve {
        element
            .raw_end()
            .map_or(element.name().raw().len() + 3, str::len)
    } else {
        element.name().raw().len() + 3
    };
    add_output_len(&mut total, suffix_len)?;
    Ok(total)
}
/// Performs the writer's complete semantic preflight without allocating an
/// output buffer. Metadata adapters use this to reject ASTs that serialization
/// would reject before retaining any opaque bytes.
pub(crate) fn validate_document(document: &XmlDocument) -> Result<(), WriteError> {
    if let Some(declaration) = document.declaration() {
        xml_chars(declaration, "XML declaration")?;
        if declaration.contains("?>") {
            return Err(WriteError("invalid XML declaration".into()));
        }
    }
    let mut has_doctype = false;
    for node in document.before_root() {
        match node {
            XmlNode::DocType(_) if !has_doctype => {
                has_doctype = true;
                validate_node_value(node)?;
            }
            XmlNode::Text(value) if value.value().trim().is_empty() => {
                validate_node_value(node)?;
            }
            XmlNode::Comment(_) | XmlNode::ProcessingInstruction(_) => {
                validate_node_value(node)?;
            }
            _ => return Err(WriteError("invalid node in XML prolog".into())),
        }
    }
    validate_element(document.root())?;
    for node in document.after_root() {
        match node {
            XmlNode::Text(value) if value.value().trim().is_empty() => {
                validate_node_value(node)?;
            }
            XmlNode::Comment(_) | XmlNode::ProcessingInstruction(_) => {
                validate_node_value(node)?;
            }
            _ => return Err(WriteError("invalid node in XML epilog".into())),
        }
    }
    Ok(())
}

fn validate_element(element: &XmlElement) -> Result<(), WriteError> {
    validate_element_start(element)?;
    for child in element.children() {
        match child {
            XmlNode::Element(child) => validate_element(child)?,
            XmlNode::DocType(_) => {
                return Err(WriteError(
                    "document type is only valid in the prolog".into(),
                ));
            }
            _ => validate_node_value(child)?,
        }
    }
    Ok(())
}

fn validate_element_start(element: &XmlElement) -> Result<(), WriteError> {
    name(element.name().raw())?;
    if element.name().prefix() == Some("xmlns") {
        return Err(WriteError(
            "element name uses the reserved xmlns prefix".into(),
        ));
    }
    let mut names = HashSet::with_capacity(element.attributes().len());
    for attribute in element.attributes() {
        validate_attribute(attribute.kind(), attribute.value())?;
        if !names.insert(serialized_attribute_name(attribute.kind())) {
            return Err(WriteError("duplicate serialized attribute".into()));
        }
    }
    Ok(())
}

fn validate_attribute(kind: &AttributeKind, value: &str) -> Result<(), WriteError> {
    xml_chars(value, "attribute value")?;
    match kind {
        AttributeKind::Ordinary(qname) => {
            name(qname.raw())?;
            if qname.raw() == "xmlns" || qname.prefix() == Some("xmlns") {
                return Err(WriteError(
                    "namespace declaration encoded as ordinary attribute".into(),
                ));
            }
        }
        AttributeKind::Namespace(prefix) => {
            if let Some(prefix) = prefix
                && !valid_name(prefix)
            {
                return Err(WriteError(format!("invalid namespace prefix `{prefix}`")));
            }
            let prefix = prefix.as_deref();
            if prefix.is_some() && value.is_empty()
                || prefix == Some("xmlns")
                || value == XMLNS_NAMESPACE
                || value == XML_NAMESPACE && prefix != Some("xml")
                || prefix == Some("xml") && value != XML_NAMESPACE
            {
                return Err(WriteError("invalid reserved namespace binding".into()));
            }
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct SerializedAttributeName<'a> {
    prefix: Option<&'a str>,
    local: &'a str,
}

fn serialized_attribute_name(kind: &AttributeKind) -> SerializedAttributeName<'_> {
    match kind {
        AttributeKind::Ordinary(name) => SerializedAttributeName {
            prefix: name.prefix(),
            local: name.local(),
        },
        AttributeKind::Namespace(None) => SerializedAttributeName {
            prefix: None,
            local: "xmlns",
        },
        AttributeKind::Namespace(Some(prefix)) => SerializedAttributeName {
            prefix: Some("xmlns"),
            local: prefix,
        },
    }
}

fn validate_node_value(node: &XmlNode) -> Result<(), WriteError> {
    match node {
        XmlNode::Element(element) => validate_element(element),
        XmlNode::Text(value) => xml_chars(value.value(), "text"),
        XmlNode::CData(value) => {
            xml_chars(value.value(), "CDATA")?;
            if value.value().contains("]]>") {
                return Err(WriteError("CDATA may not contain ]]>".into()));
            }
            Ok(())
        }
        XmlNode::Comment(value) => {
            xml_chars(value.value(), "comment")?;
            if value.value().contains("--") || value.value().ends_with('-') {
                return Err(WriteError("invalid XML comment".into()));
            }
            Ok(())
        }
        XmlNode::ProcessingInstruction(value) => {
            xml_chars(value.value(), "processing instruction")?;
            if value.value().contains("?>") {
                return Err(WriteError("invalid processing instruction".into()));
            }
            Ok(())
        }
        XmlNode::DocType(value) => xml_chars(value.value(), "document type"),
    }
}
fn element_out(
    element: &XmlElement,
    out: &mut String,
    policy: LexicalPolicy,
) -> Result<(), WriteError> {
    let preserve_raw_start = use_raw_start(element, policy);
    if preserve_raw_start {
        out.push_str(element.raw_start().unwrap());
    } else {
        normalized_element_start_out(element, out);
    }
    if element.children().is_empty() {
        if preserve_raw_start {
            if let Some(raw) = element.raw_end() {
                out.push_str(raw);
            }
            return Ok(());
        }
        return Ok(());
    }
    for child in element.children() {
        node_out(child, out, policy)?;
    }
    if policy == LexicalPolicy::Preserve
        && let Some(raw) = element.raw_end()
    {
        out.push_str(raw);
        return Ok(());
    }
    out.push_str("</");
    out.push_str(element.name().raw());
    out.push('>');
    Ok(())
}

fn normalized_element_start_out(element: &XmlElement, out: &mut String) {
    out.push('<');
    out.push_str(element.name().raw());
    for attribute in element.attributes() {
        out.push(' ');
        match attribute.kind() {
            AttributeKind::Ordinary(name) => out.push_str(name.raw()),
            AttributeKind::Namespace(None) => out.push_str("xmlns"),
            AttributeKind::Namespace(Some(prefix)) => {
                out.push_str("xmlns:");
                out.push_str(prefix);
            }
        }
        out.push_str("=\"");
        escape_attr(attribute.value(), out);
        out.push('"');
    }
    if element.children().is_empty() {
        out.push_str("/>");
    } else {
        out.push('>');
    }
}
fn node_out(node: &XmlNode, out: &mut String, policy: LexicalPolicy) -> Result<(), WriteError> {
    match node {
        XmlNode::Element(_) => {}
        XmlNode::Text(value) => xml_chars(value.value(), "text")?,
        XmlNode::CData(value) => xml_chars(value.value(), "CDATA")?,
        XmlNode::Comment(value) => xml_chars(value.value(), "comment")?,
        XmlNode::ProcessingInstruction(value) => {
            xml_chars(value.value(), "processing instruction")?
        }
        XmlNode::DocType(value) => xml_chars(value.value(), "document type")?,
    }
    if policy == LexicalPolicy::Preserve
        && let Some(raw) = node.raw()
    {
        out.push_str(raw);
        return Ok(());
    }
    match node {
        XmlNode::Element(e) => element_out(e, out, policy),
        XmlNode::Text(t) => {
            escape_text(t.value(), out);
            Ok(())
        }
        XmlNode::CData(s) => {
            if s.value().contains("]]>") {
                return Err(WriteError("CDATA may not contain ]]>".into()));
            }
            out.push_str("<![CDATA[");
            out.push_str(s.value());
            out.push_str("]]>");
            Ok(())
        }
        XmlNode::Comment(s) => {
            if s.value().contains("--") || s.value().ends_with('-') {
                return Err(WriteError("invalid XML comment".into()));
            }
            out.push_str("<!--");
            out.push_str(s.value());
            out.push_str("-->");
            Ok(())
        }
        XmlNode::ProcessingInstruction(s) => {
            if s.value().contains("?>") {
                return Err(WriteError("invalid processing instruction".into()));
            }
            out.push_str("<?");
            out.push_str(s.value());
            out.push_str("?>");
            Ok(())
        }
        XmlNode::DocType(s) => {
            out.push_str("<!DOCTYPE ");
            out.push_str(s.value());
            out.push('>');
            Ok(())
        }
    }
}
fn name(value: &str) -> Result<(), WriteError> {
    let ok = value.split(':').count() <= 2 && value.split(':').all(valid_name);
    if ok {
        Ok(())
    } else {
        Err(WriteError(format!("invalid XML name `{value}`")))
    }
}
fn xml_chars(value: &str, context: &str) -> Result<(), WriteError> {
    if value.chars().all(valid_xml_char) {
        Ok(())
    } else {
        Err(WriteError(format!(
            "{context} contains a character forbidden by XML 1.0"
        )))
    }
}
fn valid_xml_char(value: char) -> bool {
    matches!(value, '\u{9}' | '\u{a}' | '\u{d}')
        || ('\u{20}'..='\u{d7ff}').contains(&value)
        || ('\u{e000}'..='\u{fffd}').contains(&value)
        || ('\u{10000}'..='\u{10ffff}').contains(&value)
}
fn escape_text(value: &str, out: &mut String) {
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
}
fn escape_attr(value: &str, out: &mut String) {
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
}
