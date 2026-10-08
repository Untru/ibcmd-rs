//! Namespace admission for the measured WebSocketClient Headers value list.
use std::collections::BTreeMap;

use super::MetadataDecodeError;
use crate::{AttributeKind, QName, XmlDocument, XmlElement, XmlNode};

const MD: &str = "http://v8.1c.ru/8.3/MDClasses";
const XR: &str = "http://v8.1c.ru/8.3/xcf/readable";
const V8: &str = "http://v8.1c.ru/8.1/data/core";
const XS: &str = "http://www.w3.org/2001/XMLSchema";
const XSI: &str = "http://www.w3.org/2001/XMLSchema-instance";
type Scope = BTreeMap<Option<String>, String>;

fn invalid() -> MetadataDecodeError {
    MetadataDecodeError::InvalidEnvelope("WebSocketClient Headers namespace or value type")
}
fn scope(element: &XmlElement, parent: &Scope) -> Scope {
    let mut result = parent.clone();
    for attribute in element.attributes() {
        if let AttributeKind::Namespace(prefix) = attribute.kind() {
            result.insert(prefix.clone(), attribute.value().into());
        }
    }
    result
}
fn uri<'a>(name: &QName, scope: &'a Scope, attribute: bool) -> Option<&'a str> {
    if attribute && name.prefix().is_none() {
        return None;
    }
    scope
        .get(&name.prefix().map(str::to_owned))
        .map(String::as_str)
}
fn children(element: &XmlElement) -> Vec<&XmlElement> {
    element
        .children()
        .iter()
        .filter_map(|node| match node {
            XmlNode::Element(element) => Some(element),
            _ => None,
        })
        .collect()
}
fn named<'a>(element: &'a XmlElement, name: &str) -> Result<&'a XmlElement, MetadataDecodeError> {
    let found: Vec<_> = children(element)
        .into_iter()
        .filter(|child| child.name().local() == name)
        .collect();
    if found.len() != 1 {
        return Err(invalid());
    }
    Ok(found[0])
}
fn checked(
    element: &XmlElement,
    parent: &Scope,
    expected: &str,
    value_type: Option<(&str, &str)>,
    optional: bool,
) -> Result<Scope, MetadataDecodeError> {
    let current = scope(element, parent);
    if uri(element.name(), &current, false) != Some(expected) {
        return Err(invalid());
    }
    let mut seen = false;
    for attribute in element.attributes() {
        if let AttributeKind::Ordinary(name) = attribute.kind() {
            let Some((type_uri, type_name)) = value_type else {
                return Err(invalid());
            };
            let ty = QName::new(attribute.value()).map_err(|_| invalid())?;
            if seen
                || name.local() != "type"
                || uri(name, &current, true) != Some(XSI)
                || ty.local() != type_name
                || uri(&ty, &current, false) != Some(type_uri)
            {
                return Err(invalid());
            }
            seen = true;
        }
    }
    if value_type.is_some() && !optional && !seen {
        return Err(invalid());
    }
    Ok(current)
}

/// Checks expanded names before the CLI's compact XML view discards prefixes.
/// Prefix aliases and element-local declarations are admitted by URI, including
/// an empty Headers element without an explicit ValueList type.
pub fn validate_websocket_client_headers_namespaces(
    document: &XmlDocument,
) -> Result<(), MetadataDecodeError> {
    let root = document.root();
    let root_scope = scope(root, &Scope::new());
    if root.name().local() != "MetaDataObject"
        || children(root).len() != 1
        || uri(root.name(), &root_scope, false) != Some(MD)
    {
        return Err(invalid());
    }
    let object = named(root, "WebSocketClient")?;
    let object_scope = scope(object, &root_scope);
    if uri(object.name(), &object_scope, false) != Some(MD) {
        return Err(invalid());
    }
    for attribute in object.attributes() {
        if let AttributeKind::Ordinary(name) = attribute.kind() {
            if name.prefix().is_some() || name.local() != "uuid" {
                return Err(invalid());
            }
        }
    }
    let properties = named(object, "Properties")?;
    let property_scope = checked(properties, &object_scope, MD, None, false)?;
    for property in children(properties) {
        let current = scope(property, &property_scope);
        if uri(property.name(), &current, false) != Some(MD) {
            return Err(invalid());
        }
    }
    let synonym = named(properties, "Synonym")?;
    let synonym_scope = checked(synonym, &property_scope, MD, None, false)?;
    for entry in children(synonym) {
        if entry.name().local() != "item" {
            return Err(invalid());
        }
        let entry_scope = checked(entry, &synonym_scope, V8, None, false)?;
        for field in children(entry) {
            checked(field, &entry_scope, V8, None, false)?;
        }
    }
    let headers = named(properties, "Headers")?;
    let header_scope = checked(
        headers,
        &property_scope,
        MD,
        Some((XR, "ValueList")),
        children(headers).is_empty(),
    )?;
    for item in children(headers) {
        if item.name().local() != "Item" {
            return Err(invalid());
        }
        let item_scope = checked(item, &header_scope, XR, None, false)?;
        let presentation = named(item, "Presentation")?;
        checked(presentation, &item_scope, XR, None, false)?;
        let state = named(item, "CheckState")?;
        checked(state, &item_scope, XR, None, false)?;
        let value = named(item, "Value")?;
        let value_scope = checked(value, &item_scope, XR, Some((V8, "KeyAndValue")), false)?;
        for name in ["Key", "Value"] {
            checked(
                named(value, name)?,
                &value_scope,
                V8,
                Some((XS, "string")),
                false,
            )?;
        }
    }
    Ok(())
}
