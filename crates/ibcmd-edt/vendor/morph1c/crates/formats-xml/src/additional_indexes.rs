//! Complete typed additional-index sidecars in the EDT and native XML layouts.
use crate::children::{format_uuid, parse_uuid};
use crate::{parse, Element, Envelope, Format, OutElement};
use morph1c_core::ir::additional_indexes::AdditionalIndex;
use std::collections::{BTreeMap, BTreeSet};

const NATIVE: &str = "http://v8.1c.ru/8.3/xcf/extrnprops";
const EDT: &str = "http://g5.1c.ru/v8/dt/md/aindex";
const XML: &str = "http://www.w3.org/XML/1998/namespace";
const DECL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>";
const NATIVE_NS: &[(&str, &str)] = &[
    ("xmlns", NATIVE),
    ("xmlns:v8", "http://v8.1c.ru/8.1/data/core"),
    ("xmlns:xr", "http://v8.1c.ru/8.3/xcf/readable"),
    ("xmlns:xs", "http://www.w3.org/2001/XMLSchema"),
    ("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance"),
];

// Resolve names against each actual scope before accessing any semantic cell.
// A namespace declaration is consumed only when its URI belongs to this schema.
fn namespaces(root: &mut Element) -> Result<(), String> {
    let mut stack = vec![(root, BTreeMap::from([("xml".to_owned(), XML.to_owned())]))];
    while let Some((element, mut scope)) = stack.pop() {
        let mut declared = BTreeSet::new();
        for attr in &element.attrs {
            let prefix = if attr.name == "xmlns" {
                Some("")
            } else {
                attr.name.strip_prefix("xmlns:")
            };
            if let Some(prefix) = prefix {
                if !declared.insert(prefix) {
                    return Err("duplicate additional-index namespace declaration".into());
                }
                if prefix == "xmlns"
                    || (prefix == "xml" && attr.value != XML)
                    || (prefix != "xml" && attr.value == XML)
                    || (prefix != "" && attr.value.is_empty())
                {
                    return Err("invalid reserved additional-index namespace binding".into());
                }
                if !attr.value.is_empty()
                    && attr.value != EDT
                    && attr.value != XML
                    && !NATIVE_NS.iter().any(|(_, uri)| *uri == attr.value)
                {
                    return Err("unknown additional-index namespace URI".into());
                }
                scope.insert(prefix.into(), attr.value.clone());
                attr.claimed.set(true);
            }
        }
        let resolve = |prefix: &str| -> Result<String, String> {
            match scope.get(prefix) {
                Some(uri) if prefix.is_empty() || !uri.is_empty() => Ok(uri.clone()),
                None if prefix.is_empty() => Ok(String::new()),
                _ => Err("unbound additional-index namespace prefix".into()),
            }
        };
        element.prefix = resolve(&element.prefix)?;
        let mut attrs = BTreeSet::new();
        for attr in &element.attrs {
            if attr.name == "xmlns" || attr.name.starts_with("xmlns:") {
                continue;
            }
            let expanded = match attr.name.split_once(':') {
                Some((prefix, local)) => (resolve(prefix)?, local),
                None => (String::new(), attr.name.as_str()),
            };
            if !attrs.insert(expanded) {
                return Err("duplicate additional-index attribute".into());
            }
        }
        for child in &mut element.children {
            stack.push((child, scope.clone()));
        }
    }
    Ok(())
}
fn one<'a>(parent: &'a Element, name: &str, uri: &str) -> Result<&'a Element, String> {
    let mut matching = parent
        .children
        .iter()
        .filter(|node| node.local == name && node.prefix == uri);
    let node = matching
        .next()
        .ok_or_else(|| format!("missing additional-index {name}"))?;
    if matching.next().is_some() {
        return Err(format!("duplicate additional-index {name}"));
    }
    node.claim();
    Ok(node)
}
fn leaf(parent: &Element, name: &str, uri: &str) -> Result<String, String> {
    let node = one(parent, name, uri)?;
    if !node.children.is_empty() {
        return Err(format!("additional-index {name} is not a scalar"));
    }
    node.claim_text();
    Ok(node.text.clone())
}
fn fields(parent: &Element, name: &str, format: Format) -> Result<Vec<String>, String> {
    let mut result = Vec::new();
    if format == Format::Designer {
        let container = one(parent, name, NATIVE)?;
        for field in &container.children {
            if field.local != "Field" || field.prefix != NATIVE || !field.children.is_empty() {
                return Err("unknown additional-index field structure".into());
            }
            field.claim_with_text();
            result.push(field.text.clone());
        }
    } else {
        for field in parent
            .children
            .iter()
            .filter(|node| node.local == name && node.prefix.is_empty())
        {
            field.claim();
            result.push(leaf(field, "path", "")?);
        }
    }
    if result.iter().any(|field| field.is_empty()) {
        return Err("empty additional-index field path".into());
    }
    Ok(result)
}
pub fn read(bytes: &[u8], format: Format) -> Result<Vec<AdditionalIndex>, String> {
    if format == Format::Cf {
        return Err("additional-index XML codec requires an XML format".into());
    }
    let mut document = parse(bytes).map_err(|error| error.to_string())?;
    if document.decl.as_deref() != Some(DECL) {
        return Err("unexpected additional-index XML declaration".into());
    }
    namespaces(&mut document.root)?;
    let root = &document.root;
    let uri = if format == Format::Edt { EDT } else { NATIVE };
    if root.local != "AdditionalIndexes" || root.prefix != uri {
        return Err("wrong additional-index root namespace".into());
    }
    root.claim();
    if format == Format::Designer {
        let version = root
            .attr("version")
            .ok_or("missing additional-index version")?;
        if !matches!(version.value.as_str(), "2.20" | "2.21") {
            return Err("unknown additional-index XML version".into());
        }
        if morph1c_core::version::current_source_version()
            .is_some_and(|expected| expected.to_string() != version.value)
        {
            return Err("additional-index version differs from source profile".into());
        }
        version.claimed.set(true);
    }
    let mut result = Vec::new();
    for node in &root.children {
        let (tag, child_uri) = if format == Format::Edt {
            ("indexes", "")
        } else {
            ("AdditionalIndex", NATIVE)
        };
        if node.local != tag || node.prefix != child_uri {
            return Err("unknown additional-index entry".into());
        }
        node.claim();
        let id = if format == Format::Edt {
            leaf(node, "id", "")?
        } else {
            let attr = node.attr("id").ok_or("missing additional-index id")?;
            attr.claimed.set(true);
            attr.value.clone()
        };
        if id.len() != 36
            || id.bytes().enumerate().any(|(position, byte)| {
                if matches!(position, 8 | 13 | 18 | 23) {
                    byte != b'-'
                } else {
                    !byte.is_ascii_hexdigit()
                }
            })
        {
            return Err("invalid additional-index UUID spelling".into());
        }
        let (name, table, indexed, additional) = if format == Format::Edt {
            ("name", "table", "indexedFields", "additionalFields")
        } else {
            ("Name", "Table", "IndexedFields", "AdditionalFields")
        };
        let index = AdditionalIndex {
            id: parse_uuid(&id).map_err(|error| error.to_string())?,
            name: leaf(node, name, child_uri)?,
            table: leaf(node, table, child_uri)?,
            indexed_fields: fields(node, indexed, format)?,
            additional_fields: fields(node, additional, format)?,
        };
        if index.name.is_empty() || index.table.is_empty() || index.indexed_fields.is_empty() {
            return Err("additional-index name, table and indexed fields must be present".into());
        }
        result.push(index);
    }
    if root.unclaimed_count() != 0 {
        return Err(format!(
            "unconsumed additional-index source cells: {:?}",
            root.unclaimed_names(16)
        ));
    }
    Ok(result)
}
pub fn write(indexes: &[AdditionalIndex], format: Format) -> Result<Vec<u8>, String> {
    if format == Format::Cf {
        return Err("additional-index XML codec requires an XML format".into());
    }
    let edt = format == Format::Edt;
    let mut root = OutElement::branch(if edt { "aindex" } else { "" }, "AdditionalIndexes");
    if edt {
        root = root.attr("xmlns:aindex", EDT);
    } else {
        for (name, uri) in NATIVE_NS {
            root = root.attr(*name, *uri);
        }
        let version =
            morph1c_core::version::current_roundtrip_target().unwrap_or(morph1c_core::version::SSL);
        if !matches!((version.major, version.minor), (2, 20 | 21)) {
            return Err("unsupported additional-index target version".into());
        }
        root = root.attr("version", version.to_string());
    }
    for index in indexes {
        if index.name.is_empty()
            || index.table.is_empty()
            || index.indexed_fields.is_empty()
            || index
                .indexed_fields
                .iter()
                .chain(&index.additional_fields)
                .any(|field| field.is_empty())
        {
            return Err("invalid current additional-index scalar values".into());
        }
        let mut node = OutElement::branch("", if edt { "indexes" } else { "AdditionalIndex" });
        if edt {
            node.push(OutElement::leaf("", "id", format_uuid(&index.id)));
        } else {
            node = node.attr("id", format_uuid(&index.id));
        }
        node.push(OutElement::leaf(
            "",
            if edt { "name" } else { "Name" },
            &index.name,
        ));
        node.push(OutElement::leaf(
            "",
            if edt { "table" } else { "Table" },
            &index.table,
        ));
        for (fields, tag) in [
            (
                &index.indexed_fields,
                if edt {
                    "indexedFields"
                } else {
                    "IndexedFields"
                },
            ),
            (
                &index.additional_fields,
                if edt {
                    "additionalFields"
                } else {
                    "AdditionalFields"
                },
            ),
        ] {
            if edt {
                for field in fields {
                    let mut value = OutElement::branch("", tag);
                    value.push(OutElement::leaf("", "path", field));
                    node.push(value);
                }
            } else {
                let mut values = if fields.is_empty() {
                    OutElement::self_closing("", tag)
                } else {
                    OutElement::branch("", tag)
                };
                for field in fields {
                    values.push(OutElement::leaf("", "Field", field));
                }
                node.push(values);
            }
        }
        root.push(node);
    }
    if indexes.is_empty() {
        root.self_closing = true;
    }
    Ok(crate::emit::render(
        &Envelope {
            bom: !edt,
            eol: "\r\n",
            indent_unit: if edt { "  " } else { "\t" },
            decl: DECL,
            trailing_eol: edt,
            escape_gt: !edt,
            escape_quot: edt,
            text_eol: if edt { "\r\n" } else { "\n" },
        },
        &root,
    ))
}
