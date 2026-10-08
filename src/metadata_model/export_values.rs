//! Row decoders every family shares in the export direction: the md header,
//! localized strings, type descriptions, typed values, design-time and
//! metadata references, data paths, the attribute body with its choice
//! parameters and links, standard attributes. Each is the inverse of a load
//! direction encoder (`md_base`, `localized`, `types.rs`, `attribute.rs`, the
//! standard-attribute bag of `objects_parts.rs`) and writes XML elements.

use anyhow::{Result, anyhow, bail};

use super::super::brace::Brace;
use super::{
    Build, ExportContext, NameIndex, atom, el, is_nil, item, leaf, list, number, short, string,
    xml_text,
};
use crate::metadata_model::xml::Element;

// The inverse of `types.rs` and `attribute.rs` lives next to their forward
// tables (`types_export.rs`, `attribute_export.rs`): one owner per table
// pair. Re-exported here for the decoders that call them.
#[allow(unused_imports)]
pub(crate) use crate::metadata_model::attribute::export::{
    ATTRIBUTE_BODY_PROPERTIES, AttributeElements, attribute_elements, attribute_header,
    attribute_properties, choice_parameter_links_element, choice_parameters_element,
    link_by_type_element, typed_header_elements,
};
#[allow(unused_imports)]
pub(crate) use crate::metadata_model::attribute::{
    CHOICE_FOLDERS_AND_ITEMS, CHOICE_HISTORY_ON_INPUT, CREATE_ON_INPUT, FILL_CHECKING, QUICK_CHOICE,
};
#[allow(unused_imports)]
pub(crate) use crate::metadata_model::types::export::{
    data_path_text, design_time_ref_text, field_text, metadata_ref_text, standard_attribute_name,
    type_children, type_element, value_element,
};
#[allow(unused_imports)]
pub(crate) use crate::metadata_model::types::standard_attribute_codes;

/// The object a row belongs to: data paths and standard attributes are
/// spelled relative to it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Owner<'a> {
    pub kind: &'a str,
    /// `Catalog.X`
    pub full_name: &'a str,
}

/// `{3,{1,0,<uuid>},"Name",<synonym>,"Comment",0,0,<nil>,0}`.
pub(crate) struct Header {
    pub uuid: String,
    pub name: String,
    pub synonym: Brace,
    pub comment: String,
}

pub(crate) fn header(node: &Brace) -> Result<Header> {
    let items = list(node)?;
    if atom(item(items, 0)?)? != "3" || items.len() != 9 {
        bail!("not an md header: {}", short(node));
    }
    let identity = list(item(items, 1)?)?;
    Ok(Header {
        uuid: atom(item(identity, 2)?)?.to_string(),
        name: string(item(items, 2)?)?.to_string(),
        synonym: item(items, 3)?.clone(),
        comment: string(item(items, 4)?)?.to_string(),
    })
}

/// `Name`, `Synonym`, `Comment` of an md header (unprefixed, as every object
/// and child object writes them).
pub(crate) fn header_elements(header: &Header) -> Result<[Element; 3]> {
    Ok([
        leaf("Name", header.name.clone()),
        localized_element("Synonym", &header.synonym)?,
        leaf("Comment", xml_text(&header.comment)),
    ])
}

/// `{N,"lang","text",...}` -> `<qname><v8:item><v8:lang/><v8:content/></v8:item>...`.
pub(crate) fn localized_element(qname: &str, node: &Brace) -> Result<Element> {
    let items = list(node)?;
    let count = usize::try_from(number(item(items, 0)?)?)
        .map_err(|_| anyhow!("invalid localized string count {}", short(node)))?;
    let expected_len = count
        .checked_mul(2)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| anyhow!("localized string count overflows {}", short(node)))?;
    if items.len() != expected_len {
        bail!("bad localized string {}", short(node));
    }
    let mut element = el(qname);
    for pair in items[1..].chunks(2) {
        element.children.push(
            el("v8:item")
                .child(leaf("v8:lang", string(&pair[0])?))
                .child(leaf("v8:content", xml_text(string(&pair[1])?))),
        );
    }
    Ok(element)
}

/// `0`/`1` -> `false`/`true`.
pub(crate) fn bool_text(node: &Brace) -> Result<&'static str> {
    match atom(node)? {
        "0" => Ok("false"),
        "1" => Ok("true"),
        other => bail!("expected 0/1, got {other}"),
    }
}

/// A stored code -> its XML spelling.
pub(crate) fn code_text(node: &Brace, table: &[(&'static str, i64)]) -> Result<&'static str> {
    let code = number(node)?;
    table
        .iter()
        .find_map(|(name, value)| (*value == code).then_some(*name))
        .ok_or_else(|| anyhow!("code {code} is not mapped"))
}

/// The full name of a referenced object; empty for the nil uuid.
pub(crate) fn reference_name(uuid: &str, names: &NameIndex) -> Result<String> {
    if is_nil(uuid) {
        return Ok(String::new());
    }
    names
        .name(uuid)
        .map(str::to_string)
        .ok_or_else(|| anyhow!("no name for uuid {uuid}"))
}

// ---------------------------------------------------------------------------
// Standard attributes.

const STANDARD_ATTRIBUTE_CLASS: &str = "510405d3-2a0c-4fea-960a-7fee59b32f9b";

/// `{1,<n>,{<code>},<class>,<bag>...}` -> the `xr:StandardAttribute`
/// elements, named through the family's codes.
pub(crate) fn standard_attribute_elements(
    body: &Brace,
    codes: &[(&'static str, i64)],
    owner: Owner<'_>,
    context: &ExportContext,
) -> Result<Vec<Element>> {
    let fields = list(body)?;
    if atom(item(fields, 0)?)? != "1" {
        bail!("not a standard attribute block: {}", short(body));
    }
    let count = number(item(fields, 1)?)? as usize;
    let mut out = Vec::with_capacity(count);
    for index in 0..count {
        let marker = list(item(fields, 2 + 3 * index)?)?;
        let code = number(item(marker, 0)?)?;
        if atom(item(fields, 3 + 3 * index)?)? != STANDARD_ATTRIBUTE_CLASS {
            bail!("unexpected standard attribute class");
        }
        let name = codes
            .iter()
            .find_map(|(name, value)| (*value == code).then_some(*name))
            .ok_or_else(|| anyhow!("no standard attribute {code} for {}", owner.kind))?;
        let bag = item(fields, 4 + 3 * index)?;
        out.push(standard_attribute(name, bag, owner, context)?);
    }
    Ok(out)
}

/// `{0}` -> none; `{1,<body>}` -> `<StandardAttributes>`.
pub(crate) fn standard_attributes_element(
    qname: &str,
    node: &Brace,
    codes: &[(&'static str, i64)],
    owner: Owner<'_>,
    context: &ExportContext,
) -> Result<Option<Element>> {
    let fields = list(node)?;
    match fields {
        [flag] if atom(flag)? == "0" => Ok(None),
        [flag, body] if atom(flag)? == "1" => Ok(Some(
            el(qname).children(standard_attribute_elements(body, codes, owner, context)?),
        )),
        _ => bail!("bad standard attributes {}", short(node)),
    }
}

/// The bag of one standard attribute -> `<xr:StandardAttribute name=..>`
/// with its 25 properties in XML order.
fn standard_attribute(
    name: &str,
    bag: &Brace,
    owner: Owner<'_>,
    context: &ExportContext,
) -> Result<Element> {
    let names = &context.names;
    let fields = list(bag)?;
    let declared = number(item(fields, 1)?)? as usize;
    if fields.len() != 2 + 2 * declared {
        bail!("bad standard attribute bag {}", short(bag));
    }
    let mut values = std::collections::HashMap::with_capacity(declared);
    for pair in fields[2..].chunks(2) {
        values.insert(atom(&pair[0])?, &pair[1]);
    }
    let get = |key: &str| -> Result<&Brace> {
        values
            .get(key)
            .copied()
            .ok_or_else(|| anyhow!("standard attribute {name} has no {key}"))
    };
    // `{"#",<type>,<payload>}` -> payload.
    let payload = |key: &str| -> Result<&Brace> { item(list(get(key)?)?, 2) };
    // `{"#",<type>,{<type>,<code>}}` -> code.
    let nested = |key: &str| -> Result<&Brace> { item(list(payload(key)?)?, 1) };
    let flag = |key: &str| -> Result<&'static str> { bool_text(item(list(get(key)?)?, 1)?) };
    let text = |key: &str| -> Result<String> { Ok(xml_text(string(item(list(get(key)?)?, 1)?)?)) };

    let choice_form = {
        let reference = list(payload("6c4f7074-e7d4-48eb-b31b-132873666262")?)?;
        reference_name(atom(item(reference, 1)?)?, names)?
    };
    let type_reduction = match values.get("3b10624f-1e3d-495d-8093-25225efc5313") {
        // Rows before 8.3.27 do not store it: a catalog's owner denies, every
        // other standard attribute transforms.
        None if name == "Owner" => "Deny",
        None => "TransformValues",
        Some(_) => match number(nested("3b10624f-1e3d-495d-8093-25225efc5313")?)? {
            0 => "TransformValues",
            2 => "Deny",
            other => bail!("unknown TypeReductionMode {other}"),
        },
    };
    Ok(el("xr:StandardAttribute")
        .attr("name", name)
        .child(link_by_type_element(
            "xr:LinkByType",
            payload("1183c14f-f814-49c6-9233-a3c26b3f64cf")?,
            owner,
            names,
        )?)
        .child(leaf(
            "xr:FillChecking",
            code_text(
                payload("2723eb98-b4c1-498a-a6f3-70444757902f")?,
                FILL_CHECKING,
            )?,
        ))
        .child(leaf(
            "xr:MultiLine",
            flag("2bbba66b-fabf-4863-8ba3-54b3c64c896e")?,
        ))
        .child(leaf(
            "xr:FillFromFillingValue",
            flag("2c8143d5-4248-4c43-8bfb-307c0be2e415")?,
        ))
        .child(leaf(
            "xr:CreateOnInput",
            code_text(
                nested("33c74a4d-561f-4bc0-9eaa-8d21c893c0a9")?,
                CREATE_ON_INPUT,
            )?,
        ))
        .child(leaf("xr:TypeReductionMode", type_reduction))
        .child(value_element(
            "xr:MaxValue",
            get("3eaf5a8b-06d6-47b0-ac7d-a9698247f499")?,
            names,
        )?)
        .child(localized_element(
            "xr:ToolTip",
            payload("4690ff70-e3fa-4914-9127-6a9acc5fc949")?,
        )?)
        .child(leaf(
            "xr:ExtendedEdit",
            flag("4de03908-56f4-4396-a61e-17253afca9ac")?,
        ))
        .child(localized_element(
            "xr:Format",
            payload("580c29e2-8af4-4258-882a-7cf8073e61c8")?,
        )?)
        .child(leaf("xr:ChoiceForm", choice_form))
        .child(leaf(
            "xr:QuickChoice",
            code_text(
                nested("6e3a1131-37a3-4da5-8895-572d9d0c9db6")?,
                QUICK_CHOICE,
            )?,
        ))
        .child(leaf(
            "xr:ChoiceHistoryOnInput",
            code_text(
                payload("7ba608f2-e654-42a3-8885-334fe88ca910")?,
                CHOICE_HISTORY_ON_INPUT,
            )?,
        ))
        .child(localized_element(
            "xr:EditFormat",
            payload("88149a78-9448-4767-867b-0e650d165d2e")?,
        )?)
        .child(leaf(
            "xr:PasswordMode",
            flag("90ae4b5d-e0fd-49ef-a008-d67c1e75038c")?,
        ))
        .child(leaf(
            "xr:DataHistory",
            code_text(
                nested("9288a8ed-b259-46d0-a8e3-70d87956ff2d")?,
                &[("DontUse", 0), ("Use", 1)],
            )?,
        ))
        .child(leaf(
            "xr:MarkNegatives",
            flag("b02800e9-a8d1-42ab-9a12-f673e92be968")?,
        ))
        .child(value_element(
            "xr:MinValue",
            get("c65a541f-0b91-4f33-bc88-fbaaa57f9992")?,
            names,
        )?)
        .child(localized_element(
            "xr:Synonym",
            payload("cf4abea3-37b2-11d4-940f-008048da11f9")?,
        )?)
        .child(leaf(
            "xr:Comment",
            text("cf4abea4-37b2-11d4-940f-008048da11f9")?,
        ))
        .child(leaf(
            "xr:FullTextSearch",
            code_text(
                nested("d4232326-022b-421e-b6d3-88e418f74327")?,
                &[("DontUse", 0), ("Use", 1)],
            )?,
        ))
        .child(choice_parameter_links_element(
            "xr:ChoiceParameterLinks",
            payload("e3da683b-c54a-457a-a243-b9b4f9bf76dd")?,
            owner,
            names,
        )?)
        .child(value_element(
            "xr:FillValue",
            get("e6b3f5f3-bdf3-4ad0-bc60-7323b3feb208")?,
            names,
        )?)
        .child(leaf(
            "xr:Mask",
            text("f49e4ced-4033-4e6c-8755-9fbaaccd6078")?,
        ))
        .child(choice_parameters_element(
            "xr:ChoiceParameters",
            payload("fcf503b8-1c06-454a-970c-06413e64aee5")?,
            names,
        )?))
}

// ---------------------------------------------------------------------------
// Procedures.

/// A procedure (`<module uuid>`, `"method"`) -> `CommonModule.X.method`;
/// empty for the nil module and an empty name.
pub(crate) fn handler_text(module: &Brace, method: &Brace, names: &NameIndex) -> Result<String> {
    let module = atom(module)?;
    let method = string(method)?;
    if is_nil(module) && method.is_empty() {
        return Ok(String::new());
    }
    Ok(format!("{}.{method}", reference_name(module, names)?))
}
