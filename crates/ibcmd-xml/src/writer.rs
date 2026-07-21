//! Validating XML serializer.
use crate::node::{AttributeKind, XmlDocument, XmlElement, XmlNode, valid_name};
use std::collections::HashSet;
use std::fmt;

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
        validate_document_structure(document)?;
        // Preflight independently so an invalid generated subtree never yields partial bytes.
        let mut scratch = String::new();
        if let Some(decl) = document.declaration() {
            xml_chars(decl, "XML declaration")?;
            if decl.contains("?>") {
                return Err(WriteError("invalid XML declaration".into()));
            }
        }
        for node in document.before_root() {
            node_out(node, &mut scratch, LexicalPolicy::Normalized)?;
        }
        element_out(document.root(), &mut scratch, LexicalPolicy::Normalized)?;
        for node in document.after_root() {
            node_out(node, &mut scratch, LexicalPolicy::Normalized)?;
        }
        let mut out = String::new();
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
        Ok(out.into_bytes())
    }
}
fn validate_document_structure(document: &XmlDocument) -> Result<(), WriteError> {
    let mut has_doctype = false;
    for node in document.before_root() {
        match node {
            XmlNode::DocType(_) if !has_doctype => has_doctype = true,
            XmlNode::Text(value) if value.value().trim().is_empty() => {}
            XmlNode::Comment(_) | XmlNode::ProcessingInstruction(_) => {}
            _ => return Err(WriteError("invalid node in XML prolog".into())),
        }
    }
    validate_element_structure(document.root())?;
    for node in document.after_root() {
        match node {
            XmlNode::Text(value) if value.value().trim().is_empty() => {}
            XmlNode::Comment(_) | XmlNode::ProcessingInstruction(_) => {}
            _ => return Err(WriteError("invalid node in XML epilog".into())),
        }
    }
    Ok(())
}
fn validate_element_structure(root: &XmlElement) -> Result<(), WriteError> {
    let mut pending = vec![root];
    while let Some(element) = pending.pop() {
        for child in element.children() {
            match child {
                XmlNode::Element(element) => pending.push(element),
                XmlNode::DocType(_) => {
                    return Err(WriteError(
                        "document type is only valid in the prolog".into(),
                    ));
                }
                XmlNode::Text(_)
                | XmlNode::CData(_)
                | XmlNode::Comment(_)
                | XmlNode::ProcessingInstruction(_) => {}
            }
        }
    }
    Ok(())
}
fn element_out(
    element: &XmlElement,
    out: &mut String,
    policy: LexicalPolicy,
) -> Result<(), WriteError> {
    name(element.name().raw())?;
    let mut serialized_names = HashSet::new();
    for attribute in element.attributes() {
        xml_chars(attribute.value(), "attribute value")?;
        let serialized_name = match attribute.kind() {
            AttributeKind::Ordinary(name) => name.raw().to_owned(),
            AttributeKind::Namespace(None) => "xmlns".to_owned(),
            AttributeKind::Namespace(Some(prefix)) => {
                if !valid_name(prefix) {
                    return Err(WriteError(format!("invalid namespace prefix `{prefix}`")));
                }
                format!("xmlns:{prefix}")
            }
        };
        if !serialized_names.insert(serialized_name.clone()) {
            return Err(WriteError(format!(
                "duplicate attribute `{serialized_name}`"
            )));
        }
    }
    let use_raw_start = policy == LexicalPolicy::Preserve
        && element.raw_start().is_some()
        && (element.children().is_empty() || element.raw_end().is_some());
    if use_raw_start {
        out.push_str(element.raw_start().unwrap());
    } else {
        out.push('<');
        out.push_str(element.name().raw());
        for attribute in element.attributes() {
            out.push(' ');
            match attribute.kind() {
                AttributeKind::Ordinary(n) => {
                    name(n.raw())?;
                    out.push_str(n.raw());
                }
                AttributeKind::Namespace(None) => out.push_str("xmlns"),
                AttributeKind::Namespace(Some(p)) => {
                    out.push_str("xmlns:");
                    out.push_str(p);
                }
            };
            out.push_str("=\"");
            escape_attr(attribute.value(), out);
            out.push('"');
        }
    }
    if element.children().is_empty() {
        if use_raw_start {
            if let Some(raw) = element.raw_end() {
                out.push_str(raw);
            }
            return Ok(());
        }
        out.push_str("/>");
        return Ok(());
    }
    if !use_raw_start {
        out.push('>');
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
