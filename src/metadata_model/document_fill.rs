//! Base-preserving updates of Document tabular-section attribute fill pairs.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    sync::Arc,
};

use anyhow::{Result, anyhow, ensure};
use ibcmd_xml::{AttributeKind, QName, XmlElement, XmlNode, XmlReader};

use super::{
    DescriptorContext,
    attribute::bool_prop,
    brace::{Brace, parse_row, serialize_row},
    export::values::{bool_text, header},
    export::{NameIndex, atom, item, list, number},
    objects::{TS_ATTRIBUTES, layout, parts::Slot},
    types::{empty_string_value, typed_value},
    xml::{Element, MetadataXml},
};
use crate::module_blob::MetadataSourceContext;

const MD_NS: &str = "http://v8.1c.ru/8.3/MDClasses";
const XSI_NS: &str = "http://www.w3.org/2001/XMLSchema-instance";
const XS_NS: &str = "http://www.w3.org/2001/XMLSchema";
const XR_NS: &str = "http://v8.1c.ru/8.3/xcf/readable";
const CORE_NS: &str = "http://v8.1c.ru/8.1/data/core";
const ENTERPRISE_NS: &str = "http://v8.1c.ru/8.1/data/enterprise";
pub(crate) const KIND: &str = ibcmd_schema::GeneratedMetadataOwnerFamily::Document.token();

/// Physical coordinates shared with the typed Document/attribute writers.
struct Schema;
impl Schema {
    const ROW_TAG: usize = 0;
    const RECORD_TAG: usize = 0;
    const HEADER_IDENTITY: usize = 1;
    const IDENTITY_TAG: usize = 0;
    const IDENTITY_FLAGS: usize = 1;
    const IDENTITY_LENGTH: usize = 3;
    const OWNER: usize = 1;
    const COLLECTION_COUNT: usize = 2;
    const COLLECTIONS: usize = 3;
    const COLLECTION_ID: usize = 0;
    const ITEM_COUNT: usize = 1;
    const ITEMS: usize = 2;
    const WRAPPED: usize = 0;
    const WRAPPER_TAG: usize = 0;
    const WRAPPER_RECORD: usize = 1;
    const ZERO_TAIL: usize = 1;
    const HEADER_WRAPPER_LENGTH: usize = 2;
    const TS_CONTAINER_LENGTH: usize = 3;
    const TS_CONTAINER_MARKER: usize = 1;
    const TS_OLD_WRAPPER_LENGTH: usize = 2;
    const TS_MODERN_WRAPPER_LENGTH: usize = 3;
    const TS_RECORD_LENGTH: usize = 9;
    const TS_HEADER: usize = 5;
    const TS_ATTRIBUTES: usize = 2;
    const ATTRIBUTE_BODY: usize = 1;
    const ATTRIBUTE_WRAPPER_LENGTH: usize = 2;
    const ATTRIBUTE_RECORD_LENGTH: usize = 5;
    const TYPED_HEADER: usize = 1;
    const TYPED_HEADER_LENGTH: usize = 3;
    const TYPE_PATTERN: usize = 2;
    const FILL_VALUE: usize = 19;
    const FILL_FROM: usize = 20;
    const BODY_LENGTH: usize = 23;
}

fn uuid(text: &str) -> Result<String> {
    let value = uuid::Uuid::parse_str(text)?;
    ensure!(!value.is_nil(), "Document fill: nil object UUID");
    Ok(value.hyphenated().to_string())
}

fn native_header(node: &Brace) -> Result<String> {
    let fields = list(node)?;
    let identity = list(item(fields, Schema::HEADER_IDENTITY)?)?;
    ensure!(
        identity.len() == Schema::IDENTITY_LENGTH
            && atom(&identity[Schema::IDENTITY_TAG])? == "1"
            && atom(&identity[Schema::IDENTITY_FLAGS])? == "0",
        "Document fill: unsupported metadata identity"
    );
    uuid(&header(node)?.uuid)
}

fn unique_child<'a>(parent: &'a Element, name: &'a str) -> Result<Option<&'a Element>> {
    let mut matches = parent.children_named(name);
    let first = matches.next();
    ensure!(
        matches.next().is_none(),
        "Document fill: duplicate <{name}>"
    );
    Ok(first)
}

fn namespace_scope(
    element: &XmlElement,
    inherited: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let mut scope = inherited.clone();
    for attribute in element.attributes() {
        if let AttributeKind::Namespace(prefix) = attribute.kind() {
            scope.insert(
                prefix.clone().unwrap_or_default(),
                attribute.value().to_owned(),
            );
        }
    }
    scope
}

// XML Schema decimal's lexical form, restricted to an ordinary decimal atom:
// an optional sign, digits and at most one dot; never native tuple syntax.
fn valid_decimal(value: &str) -> bool {
    let value = value
        .strip_prefix('+')
        .or_else(|| value.strip_prefix('-'))
        .unwrap_or(value);
    let mut digits = false;
    let mut dot = false;
    for byte in value.bytes() {
        match byte {
            b'0'..=b'9' => digits = true,
            b'.' if !dot => dot = true,
            _ => return false,
        }
    }
    digits
}

// This scalar writer only knows the platform's second-resolution, timezone-free
// four-digit calendar form. Other dateTime facets remain refused.
fn valid_calendar(value: &str) -> bool {
    let Some((date, time)) = value.split_once('T') else {
        return false;
    };
    let mut date = date.split('-');
    let (Some(year), Some(month), Some(day), None) =
        (date.next(), date.next(), date.next(), date.next())
    else {
        return false;
    };
    let mut time = time.split(':');
    let (Some(hour), Some(minute), Some(second), None) =
        (time.next(), time.next(), time.next(), time.next())
    else {
        return false;
    };
    if year.len() != 4
        || [month, day, hour, minute, second]
            .iter()
            .any(|part| part.len() != 2)
        || [year, month, day, hour, minute, second]
            .iter()
            .any(|part| !part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return false;
    }
    let (Ok(year), Ok(month), Ok(day), Ok(hour), Ok(minute), Ok(second)) = (
        year.parse::<u16>(),
        month.parse::<u8>(),
        day.parse::<u8>(),
        hour.parse::<u8>(),
        minute.parse::<u8>(),
        second.parse::<u8>(),
    ) else {
        return false;
    };
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    year != 0 && day != 0 && day <= days && hour <= 23 && minute <= 59 && second <= 59
}

fn validate_fill_type(element: &XmlElement, scope: &BTreeMap<String, String>) -> Result<()> {
    for attribute in element.attributes() {
        let AttributeKind::Ordinary(name) = attribute.kind() else {
            continue;
        };
        if name.local() != "type" {
            continue;
        }
        let value = attribute.value();
        let name = QName::new(value).map_err(|error| anyhow!(error))?;
        let expected = match name.prefix() {
            Some("xs") => XS_NS,
            Some("xr") => XR_NS,
            Some("v8") => CORE_NS,
            Some("ent") => ENTERPRISE_NS,
            _ => return Err(anyhow!("Document fill: unsupported type QName")),
        };
        ensure!(
            scope.get(name.prefix().unwrap()).map(String::as_str) == Some(expected),
            "Document fill: unresolved or foreign type QName"
        );
        let text = element
            .children()
            .iter()
            .map(|node| match node {
                XmlNode::Text(text) => Ok(text.value()),
                _ => Err(anyhow!("Document fill: unsupported scalar content")),
            })
            .collect::<Result<Vec<_>>>()?
            .concat();
        match value {
            "xs:decimal" => ensure!(
                valid_decimal(&text),
                "Document fill: invalid decimal scalar"
            ),
            "xs:dateTime" => ensure!(
                valid_calendar(&text),
                "Document fill: invalid calendar scalar"
            ),
            "xs:boolean" => ensure!(
                matches!(text.as_str(), "true" | "false"),
                "Document fill: invalid Boolean scalar"
            ),
            _ => {}
        }
    }
    Ok(())
}

fn validate_source_namespaces(
    element: &XmlElement,
    inherited: &BTreeMap<String, String>,
    path: &mut Vec<String>,
) -> Result<()> {
    let scope = namespace_scope(element, inherited);
    let prefix = element.name().prefix().unwrap_or_default();
    let uri = scope.get(prefix).map(String::as_str).unwrap_or_default();
    ensure!(
        prefix.is_empty() || !uri.is_empty(),
        "Document fill: unresolved element prefix"
    );
    path.push(element.name().local().to_owned());
    let coordinate = [
        "MetaDataObject",
        "Document",
        "ChildObjects",
        "TabularSection",
        "ChildObjects",
        "Attribute",
        "Properties",
    ];
    let targeted = path.len() <= coordinate.len()
        && path
            .iter()
            .zip(coordinate)
            .all(|(left, right)| left == right);
    let fill = path.len() == coordinate.len() + 1
        && path
            .iter()
            .zip(coordinate)
            .all(|(left, right)| left == right)
        && matches!(element.name().local(), "FillValue" | "FillFromFillingValue");
    if targeted || fill {
        ensure!(uri == MD_NS, "Document fill: foreign metadata namespace");
    }
    let mut expanded = BTreeSet::new();
    for attribute in element.attributes() {
        let AttributeKind::Ordinary(name) = attribute.kind() else {
            continue;
        };
        let uri = match name.prefix() {
            None => "",
            Some(prefix) => scope
                .get(prefix)
                .map(String::as_str)
                .filter(|uri| !uri.is_empty())
                .ok_or_else(|| anyhow!("Document fill: unresolved attribute prefix"))?,
        };
        ensure!(
            expanded.insert((uri.to_owned(), name.local().to_owned())),
            "Document fill: duplicate expanded attribute"
        );
        if fill {
            ensure!(
                element.name().local() == "FillValue"
                    && uri == XSI_NS
                    && matches!(name.local(), "type" | "nil"),
                "Document fill: unsupported fill attribute"
            );
        }
    }
    if fill {
        ensure!(
            element
                .children()
                .iter()
                .all(|node| matches!(node, XmlNode::Text(_))),
            "Document fill: unsupported fill content"
        );
        validate_fill_type(element, &scope)?;
    }
    for child in element.children() {
        if let XmlNode::Element(child) = child {
            validate_source_namespaces(child, &scope, path)?;
        }
    }
    path.pop();
    Ok(())
}

fn source_pairs(
    xml: &[u8],
    source: Option<&MetadataSourceContext>,
) -> Result<(String, BTreeMap<String, BTreeMap<String, (Brace, Brace)>>)> {
    let parsed = XmlReader::from_slice(xml)?;
    let document = MetadataXml::parse(xml)?;
    ensure!(
        document.root.name == "MetaDataObject",
        "Document fill: expected MetaDataObject"
    );
    let object = document.object()?;
    ensure!(
        object.name == "Document",
        "Document fill: wrong metadata kind"
    );
    if !object
        .child("ChildObjects")
        .is_some_and(|children| children.children_named("TabularSection").next().is_some())
    {
        return Ok((String::new(), BTreeMap::new()));
    }
    let namespaces = BTreeMap::from([(
        "xml".to_string(),
        "http://www.w3.org/XML/1998/namespace".to_string(),
    )]);
    validate_source_namespaces(parsed.root(), &namespaces, &mut Vec::new())?;
    let id = uuid(
        object
            .attr("uuid")
            .ok_or_else(|| anyhow!("Document fill: no owner UUID"))?,
    )?;
    let root = Path::new("/document-fill-source");
    let mut context = DescriptorContext::with_files(
        root,
        document.version().unwrap_or("2.20"),
        &[(root.join("Documents/Probe.xml"), Arc::new(xml.to_vec()))],
    )?;
    if let Some(source) = source {
        context.source = source.clone();
    }
    let names = NameIndex::from_config_index(&context.index);
    let mut sections = BTreeMap::new();
    if let Some(children) = unique_child(object, "ChildObjects")? {
        for section in children.children_named("TabularSection") {
            let section_id = uuid(
                section
                    .attr("uuid")
                    .ok_or_else(|| anyhow!("Document fill: no section UUID"))?,
            )?;
            let mut attributes = BTreeMap::new();
            if let Some(children) = unique_child(section, "ChildObjects")? {
                for attribute in children.children_named("Attribute") {
                    let id = uuid(
                        attribute
                            .attr("uuid")
                            .ok_or_else(|| anyhow!("Document fill: no attribute UUID"))?,
                    )?;
                    let properties = unique_child(attribute, "Properties")?
                        .ok_or_else(|| anyhow!("Document fill: no Properties"))?;
                    let value = match unique_child(properties, "FillValue")? {
                        None => empty_string_value(),
                        Some(value) => {
                            let encoded = typed_value(Some(value), &context)?;
                            let decoded =
                                super::types::export::value_element("FillValue", &encoded, &names)?;
                            ensure!(
                                value.attrs == decoded.attrs
                                    && value.text == decoded.text
                                    && value.children.is_empty(),
                                "Document fill: non-round-trippable fill value"
                            );
                            encoded
                        }
                    };
                    unique_child(properties, "FillFromFillingValue")?;
                    let flag = bool_prop(properties, "FillFromFillingValue")?;
                    ensure!(
                        attributes.insert(id, (value, flag)).is_none(),
                        "Document fill: duplicate attribute UUID"
                    );
                }
            }
            ensure!(
                sections.insert(section_id, attributes).is_none(),
                "Document fill: duplicate section UUID"
            );
        }
    }
    Ok((id, sections))
}

fn collection_items(node: &Brace) -> Result<&[Brace]> {
    let fields = list(node)?;
    let count = usize::try_from(number(item(fields, Schema::ITEM_COUNT)?)?)?;
    ensure!(
        fields.len().checked_sub(Schema::ITEMS) == Some(count),
        "Document fill: invalid collection count"
    );
    Ok(&fields[Schema::ITEMS..])
}

/// Changes exactly the matched fill pair, leaving all other native coordinates.
pub(crate) fn patch_document_ts_fill_text(
    base: &str,
    xml: &[u8],
    source: Option<&MetadataSourceContext>,
) -> Result<String> {
    let (owner_id, mut source_sections) = source_pairs(xml, source)?;
    if source_sections.is_empty() {
        return Ok(base.to_owned());
    }
    let mut row = parse_row(base.as_bytes())?;
    let root = list(&row)?;
    ensure!(
        atom(item(root, Schema::ROW_TAG)?)? == "1",
        "Document fill: unsupported root"
    );
    let count = usize::try_from(number(item(root, Schema::COLLECTION_COUNT)?)?)?;
    let layout = layout("Document").ok_or_else(|| anyhow!("Document fill: no layout"))?;
    ensure!(
        count == layout.collections.len()
            && root.len().checked_sub(Schema::COLLECTIONS) == Some(count),
        "Document fill: unsupported collections"
    );
    let owner = list(item(root, Schema::OWNER)?)?;
    ensure!(
        atom(item(owner, Schema::ROW_TAG)?)? == "40",
        "Document fill: unsupported owner tag"
    );
    let mut position = 0;
    let mut header_slot = None;
    for slot in layout.slots {
        if matches!(slot, Slot::WrappedHeader) {
            header_slot = Some(position);
        }
        position += if matches!(slot, Slot::Generated(_)) {
            2
        } else {
            1
        };
    }
    ensure!(
        owner.len() == position,
        "Document fill: unsupported owner profile"
    );
    let wrapped_header = list(item(
        owner,
        header_slot.ok_or_else(|| anyhow!("Document fill: no owner header"))?,
    )?)?;
    ensure!(
        wrapped_header.len() == Schema::HEADER_WRAPPER_LENGTH
            && atom(&wrapped_header[Schema::WRAPPER_TAG])? == "0",
        "Document fill: invalid owner header wrapper"
    );
    ensure!(
        native_header(&wrapped_header[Schema::WRAPPER_RECORD])? == owner_id,
        "Document fill: owner UUID mismatch"
    );
    let mut seen = BTreeSet::new();
    let mut sections_slot = None;
    for (offset, collection) in root[Schema::COLLECTIONS..].iter().enumerate() {
        let fields = list(collection)?;
        let id = atom(item(fields, Schema::COLLECTION_ID)?)?;
        uuid(id)?;
        ensure!(
            seen.insert(id)
                && layout
                    .collections
                    .iter()
                    .any(|(expected, _)| *expected == id),
            "Document fill: unexpected collection"
        );
        collection_items(collection)?;
        if layout.collections.iter().any(|(class, role)| {
            *class == id && matches!(role, super::objects::parts::Coll::TabularSections { .. })
        }) {
            sections_slot = Some(Schema::COLLECTIONS + offset);
        }
    }
    let sections_slot =
        sections_slot.ok_or_else(|| anyhow!("Document fill: no sections collection"))?;
    let sections = row.as_list_mut().unwrap()[sections_slot]
        .as_list_mut()
        .unwrap();
    let mut changed = false;
    for section in &mut sections[Schema::ITEMS..] {
        let fields = section
            .as_list_mut()
            .ok_or_else(|| anyhow!("Document fill: invalid section"))?;
        ensure!(
            fields.len() == Schema::TS_CONTAINER_LENGTH
                && atom(&fields[Schema::TS_CONTAINER_MARKER])? == "1",
            "Document fill: unsupported section wrapper"
        );
        let wrapped = list(&fields[Schema::WRAPPED])?;
        ensure!(
            matches!(
                (atom(item(wrapped, Schema::WRAPPER_TAG)?)?, wrapped.len()),
                ("1", Schema::TS_OLD_WRAPPER_LENGTH) | ("2", Schema::TS_MODERN_WRAPPER_LENGTH)
            ),
            "Document fill: unsupported section version"
        );
        let record = list(item(wrapped, Schema::WRAPPER_RECORD)?)?;
        ensure!(
            record.len() == Schema::TS_RECORD_LENGTH
                && atom(item(record, Schema::RECORD_TAG)?)? == "11",
            "Document fill: unsupported section record"
        );
        let head = list(item(record, Schema::TS_HEADER)?)?;
        ensure!(
            head.len() == Schema::HEADER_WRAPPER_LENGTH && atom(&head[Schema::WRAPPER_TAG])? == "0",
            "Document fill: unsupported section header"
        );
        let section_id = native_header(&head[Schema::WRAPPER_RECORD])?;
        let mut requested = source_sections
            .remove(&section_id)
            .ok_or_else(|| anyhow!("Document fill: unmatched section UUID {section_id}"))?;
        let attributes = &mut fields[Schema::TS_ATTRIBUTES];
        ensure!(
            atom(item(list(attributes)?, Schema::COLLECTION_ID)?)? == TS_ATTRIBUTES,
            "Document fill: unsupported attribute collection"
        );
        collection_items(attributes)?;
        for attribute in &mut attributes.as_list_mut().unwrap()[Schema::ITEMS..] {
            let wrapper = attribute
                .as_list_mut()
                .ok_or_else(|| anyhow!("Document fill: invalid attribute wrapper"))?;
            ensure!(
                wrapper.len() == Schema::ATTRIBUTE_WRAPPER_LENGTH
                    && atom(&wrapper[Schema::ZERO_TAIL])? == "0",
                "Document fill: unsupported attribute tail"
            );
            let record = wrapper[Schema::WRAPPED]
                .as_list_mut()
                .ok_or_else(|| anyhow!("Document fill: invalid attribute record"))?;
            ensure!(
                record.len() == Schema::ATTRIBUTE_RECORD_LENGTH
                    && atom(&record[Schema::RECORD_TAG])? == "8",
                "Document fill: unsupported attribute record"
            );
            let body = record[Schema::ATTRIBUTE_BODY]
                .as_list_mut()
                .ok_or_else(|| anyhow!("Document fill: invalid attribute body"))?;
            ensure!(
                body.len() == Schema::BODY_LENGTH && atom(&body[Schema::RECORD_TAG])? == "27",
                "Document fill: unsupported attribute body profile"
            );
            let typed = list(&body[Schema::TYPED_HEADER])?;
            ensure!(
                typed.len() == Schema::TYPED_HEADER_LENGTH
                    && atom(&typed[Schema::RECORD_TAG])? == "2",
                "Document fill: unsupported typed header"
            );
            ensure!(
                list(item(typed, Schema::TYPE_PATTERN)?)?
                    .first()
                    .and_then(Brace::as_str)
                    == Some("Pattern"),
                "Document fill: invalid type pattern"
            );
            let id = native_header(item(typed, Schema::TYPED_HEADER)?)?;
            let (value, flag) = requested
                .remove(&id)
                .ok_or_else(|| anyhow!("Document fill: unmatched attribute UUID {id}"))?;
            bool_text(&body[Schema::FILL_FROM])?;
            changed |= body[Schema::FILL_VALUE] != value || body[Schema::FILL_FROM] != flag;
            body[Schema::FILL_VALUE] = value;
            body[Schema::FILL_FROM] = flag;
        }
        ensure!(
            requested.is_empty(),
            "Document fill: source contains unmatched attributes"
        );
    }
    ensure!(
        source_sections.is_empty(),
        "Document fill: source contains unmatched sections"
    );
    if !changed {
        return Ok(base.to_owned());
    }
    let serialized = serialize_row(&row);
    ensure!(
        parse_row(&serialized)? == row,
        "Document fill: serialization changed native values"
    );
    Ok(String::from_utf8(serialized)?)
}
