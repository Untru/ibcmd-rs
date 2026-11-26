//! The export direction of `attribute.rs`: the `{27,...}` attribute body with
//! its choice parameters, links and link by type -> the XML properties, in
//! an owner's order or by name. It reads the forward code tables of
//! `attribute.rs`; `export::values` re-exports it.

use anyhow::{Result, anyhow, bail};

use super::{
    CHOICE_FOLDERS_AND_ITEMS, CHOICE_HISTORY_ON_INPUT, CREATE_ON_INPUT, FILL_CHECKING, QUICK_CHOICE,
};
use crate::metadata_model::brace::Brace;
use crate::metadata_model::export::values::{
    Header, Owner, bool_text, code_text, header, header_elements, localized_element, reference_name,
};
use crate::metadata_model::export::{
    Build, ExportContext, NameIndex, atom, el, item, leaf, list, number, short, string, xml_text,
};
use crate::metadata_model::types::export::{data_path_text, type_element, value_element};
use crate::metadata_model::xml::Element;

// ---------------------------------------------------------------------------
// Choice parameters, links, link by type.

/// `{3,<n>,<segment>...,<link item>}` -> `<qname>` (self-closed when empty).
pub(crate) fn link_by_type_element(
    qname: &str,
    node: &Brace,
    owner: Owner<'_>,
    names: &NameIndex,
) -> Result<Element> {
    let fields = list(node)?;
    let count = number(item(fields, 1)?)? as usize;
    if count == 0 {
        return Ok(el(qname));
    }
    let segments = fields
        .get(2..2 + count)
        .ok_or_else(|| anyhow!("short link by type {}", short(node)))?;
    let link_item = atom(item(fields, 2 + count)?)?;
    Ok(el(qname)
        .child(leaf("xr:DataPath", data_path_text(segments, owner, names)?))
        .child(leaf("xr:LinkItem", link_item)))
}

/// `{5006,<n>,"name",<segments>,<segment>...,<0|1>...}` -> `<qname>`.
pub(crate) fn choice_parameter_links_element(
    qname: &str,
    node: &Brace,
    owner: Owner<'_>,
    names: &NameIndex,
) -> Result<Element> {
    let fields = list(node)?;
    let count = number(item(fields, 1)?)? as usize;
    let mut element = el(qname);
    let mut position = 2;
    for _ in 0..count {
        let name = string(item(fields, position)?)?;
        let segment_count = number(item(fields, position + 1)?)? as usize;
        let segments = fields
            .get(position + 2..position + 2 + segment_count)
            .ok_or_else(|| anyhow!("short choice parameter link {}", short(node)))?;
        let change = match atom(item(fields, position + 2 + segment_count)?)? {
            "0" => "Clear",
            "1" => "DontChange",
            other => bail!("unknown ValueChange {other}"),
        };
        element.children.push(
            el("xr:Link")
                .child(leaf("xr:Name", name))
                .child(
                    leaf("xr:DataPath", data_path_text(segments, owner, names)?)
                        .attr("type", "xs:string"),
                )
                .child(leaf("xr:ValueChange", change)),
        );
        position += 3 + segment_count;
    }
    Ok(element)
}

/// `{0,<n>,"name",<value>...}` -> `<qname><app:item name=..><app:value/>`.
pub(crate) fn choice_parameters_element(
    qname: &str,
    node: &Brace,
    names: &NameIndex,
) -> Result<Element> {
    let fields = list(node)?;
    let count = number(item(fields, 1)?)? as usize;
    let mut element = el(qname);
    for index in 0..count {
        let name = string(item(fields, 2 + 2 * index)?)?;
        let value = item(fields, 3 + 2 * index)?;
        element
            .children
            .push(el("app:item").attr("name", name).child(value_element(
                "app:value",
                value,
                names,
            )?));
    }
    Ok(element)
}

// ---------------------------------------------------------------------------
// The attribute body.

/// The uuid of an attribute body `{27,{2,<md base>,<pattern>},...}`.
pub(crate) fn attribute_header(body: &Brace) -> Result<Header> {
    let slots = list(body)?;
    if atom(item(slots, 0)?)? != "27" {
        bail!("not an attribute body: {}", short(body));
    }
    let typed = list(item(slots, 1)?)?;
    header(item(typed, 1)?)
}

/// The `<Properties>` children of an attribute body, in XML order: the
/// header, `Type`, the body's properties (`FillFromFillingValue` and
/// `FillValue` only when `with_fill`), then the owner's own `tail`.
pub(crate) fn attribute_properties(
    body: &Brace,
    with_fill: bool,
    tail: Vec<Element>,
    owner: Owner<'_>,
    context: &ExportContext,
) -> Result<Element> {
    let names = &context.names;
    let slots = list(body)?;
    if atom(item(slots, 0)?)? != "27" || slots.len() != 23 {
        bail!("not an attribute body: {}", short(body));
    }
    let typed = list(item(slots, 1)?)?;
    let head = header(item(typed, 1)?)?;
    let [name, synonym, comment] = header_elements(&head)?;
    let mut properties = el("Properties")
        .child(name)
        .child(synonym)
        .child(comment)
        .child(type_element("Type", item(typed, 2)?, names)?)
        .child(leaf("PasswordMode", bool_text(&slots[2])?))
        .child(localized_element("Format", &slots[3])?)
        .child(localized_element("EditFormat", &slots[18])?)
        .child(localized_element("ToolTip", &slots[4])?)
        .child(leaf("MarkNegatives", bool_text(&slots[5])?))
        .child(leaf("Mask", xml_text(string(&slots[6])?)))
        .child(leaf("MultiLine", bool_text(&slots[7])?))
        .child(leaf("ExtendedEdit", bool_text(&slots[17])?))
        .child(value_element("MinValue", &slots[8], names)?)
        .child(value_element("MaxValue", &slots[9], names)?);
    if with_fill {
        properties = properties
            .child(leaf("FillFromFillingValue", bool_text(&slots[20])?))
            .child(value_element("FillValue", &slots[19], names)?);
    }
    properties = properties
        .child(leaf("FillChecking", code_text(&slots[13], FILL_CHECKING)?))
        .child(leaf(
            "ChoiceFoldersAndItems",
            code_text(&slots[10], CHOICE_FOLDERS_AND_ITEMS)?,
        ))
        .child(choice_parameter_links_element(
            "ChoiceParameterLinks",
            &slots[14],
            owner,
            names,
        )?)
        .child(choice_parameters_element(
            "ChoiceParameters",
            &slots[16],
            names,
        )?)
        .child(leaf("QuickChoice", code_text(&slots[12], QUICK_CHOICE)?))
        .child(leaf(
            "CreateOnInput",
            code_text(&slots[21], CREATE_ON_INPUT)?,
        ))
        .child(leaf(
            "ChoiceForm",
            reference_name(atom(&slots[11])?, names)?,
        ))
        .child(link_by_type_element(
            "LinkByType",
            &slots[15],
            owner,
            names,
        )?)
        .child(leaf(
            "ChoiceHistoryOnInput",
            code_text(&slots[22], CHOICE_HISTORY_ON_INPUT)?,
        ));
    Ok(properties.children(tail))
}

// ---------------------------------------------------------------------------
// The attribute body by property, in any XML order.

/// The XML properties an attribute body `{27,...}` holds, by name: the md
/// header, `Type`, and the 21 properties of its slots.
pub(crate) const ATTRIBUTE_BODY_PROPERTIES: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "Type",
    "PasswordMode",
    "Format",
    "EditFormat",
    "ToolTip",
    "MarkNegatives",
    "Mask",
    "MultiLine",
    "ExtendedEdit",
    "MinValue",
    "MaxValue",
    "FillFromFillingValue",
    "FillValue",
    "FillChecking",
    "ChoiceFoldersAndItems",
    "ChoiceParameterLinks",
    "ChoiceParameters",
    "QuickChoice",
    "CreateOnInput",
    "ChoiceForm",
    "LinkByType",
    "ChoiceHistoryOnInput",
];

/// A decoded attribute body: its header and its properties as XML elements
/// by name, for an owner to place in its own order (a constant writes its
/// own properties between `Type` and `PasswordMode` and has no fill value; a
/// register dimension or resource adds its own): the general form of
/// [`attribute_properties`].
pub(crate) struct AttributeElements {
    elements: Vec<(&'static str, Option<Element>)>,
}

impl AttributeElements {
    /// The element of one property of [`ATTRIBUTE_BODY_PROPERTIES`], once.
    pub fn take(&mut self, name: &str) -> Result<Element> {
        self.elements
            .iter_mut()
            .find(|(candidate, _)| *candidate == name)
            .and_then(|(_, element)| element.take())
            .ok_or_else(|| anyhow!("attribute body has no <{name}> left"))
    }

    /// The elements of `order`: body properties are taken, any other name
    /// comes from `extra` (the owner's own properties, by name).
    pub fn ordered(
        mut self,
        order: &[&str],
        mut extra: impl FnMut(&str) -> Result<Element>,
    ) -> Result<Vec<Element>> {
        let mut out = Vec::with_capacity(order.len());
        for name in order {
            if ATTRIBUTE_BODY_PROPERTIES.contains(name) {
                out.push(self.take(name)?);
            } else {
                out.push(extra(name)?);
            }
        }
        Ok(out)
    }
}

/// `{27,{2,<md base>,<pattern>},...}` -> its properties by name.
pub(crate) fn attribute_elements(
    body: &Brace,
    owner: Owner<'_>,
    context: &ExportContext,
) -> Result<AttributeElements> {
    let names = &context.names;
    let slots = list(body)?;
    if atom(item(slots, 0)?)? != "27" || slots.len() != 23 {
        bail!("not an attribute body: {}", short(body));
    }
    let typed = list(item(slots, 1)?)?;
    let head = header(item(typed, 1)?)?;
    let [name, synonym, comment] = header_elements(&head)?;
    let elements = vec![
        ("Name", name),
        ("Synonym", synonym),
        ("Comment", comment),
        ("Type", type_element("Type", item(typed, 2)?, names)?),
        ("PasswordMode", leaf("PasswordMode", bool_text(&slots[2])?)),
        ("Format", localized_element("Format", &slots[3])?),
        ("EditFormat", localized_element("EditFormat", &slots[18])?),
        ("ToolTip", localized_element("ToolTip", &slots[4])?),
        (
            "MarkNegatives",
            leaf("MarkNegatives", bool_text(&slots[5])?),
        ),
        ("Mask", leaf("Mask", xml_text(string(&slots[6])?))),
        ("MultiLine", leaf("MultiLine", bool_text(&slots[7])?)),
        ("ExtendedEdit", leaf("ExtendedEdit", bool_text(&slots[17])?)),
        ("MinValue", value_element("MinValue", &slots[8], names)?),
        ("MaxValue", value_element("MaxValue", &slots[9], names)?),
        (
            "FillFromFillingValue",
            leaf("FillFromFillingValue", bool_text(&slots[20])?),
        ),
        ("FillValue", value_element("FillValue", &slots[19], names)?),
        (
            "FillChecking",
            leaf("FillChecking", code_text(&slots[13], FILL_CHECKING)?),
        ),
        (
            "ChoiceFoldersAndItems",
            leaf(
                "ChoiceFoldersAndItems",
                code_text(&slots[10], CHOICE_FOLDERS_AND_ITEMS)?,
            ),
        ),
        (
            "ChoiceParameterLinks",
            choice_parameter_links_element("ChoiceParameterLinks", &slots[14], owner, names)?,
        ),
        (
            "ChoiceParameters",
            choice_parameters_element("ChoiceParameters", &slots[16], names)?,
        ),
        (
            "QuickChoice",
            leaf("QuickChoice", code_text(&slots[12], QUICK_CHOICE)?),
        ),
        (
            "CreateOnInput",
            leaf("CreateOnInput", code_text(&slots[21], CREATE_ON_INPUT)?),
        ),
        (
            "ChoiceForm",
            leaf("ChoiceForm", reference_name(atom(&slots[11])?, names)?),
        ),
        (
            "LinkByType",
            link_by_type_element("LinkByType", &slots[15], owner, names)?,
        ),
        (
            "ChoiceHistoryOnInput",
            leaf(
                "ChoiceHistoryOnInput",
                code_text(&slots[22], CHOICE_HISTORY_ON_INPUT)?,
            ),
        ),
    ];
    Ok(AttributeElements {
        elements: elements
            .into_iter()
            .map(|(name, element)| (name, Some(element)))
            .collect(),
    })
}

/// `{2,<md base>,<pattern>}` (a session parameter, a sequence dimension, the
/// head of every attribute body) -> the header and the `Type` element.
pub(crate) fn typed_header_elements(node: &Brace, names: &NameIndex) -> Result<(Header, Element)> {
    let typed = list(node)?;
    if atom(item(typed, 0)?)? != "2" {
        bail!("not a typed header: {}", short(node));
    }
    Ok((
        header(item(typed, 1)?)?,
        type_element("Type", item(typed, 2)?, names)?,
    ))
}
