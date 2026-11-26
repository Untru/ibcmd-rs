//! An adopted object's metadata XML as 8.3.27.2214 dumps it: `ObjectBelonging`
//! first, the extended configuration object after the comment, only the
//! properties the extension controls, and `xr:PropertyState` `Extended` for
//! every module or form it extends (after the generated types; an adopted
//! object with neither writes `<InternalInfo/>`).
//!
//! Property uuids, each set apart by an 8.3.27.2214 probe
//! (`_onecdec/make_adopted_fixtures.py`: every flag, then three binary
//! partitions) or by the ИТК dumps.

use anyhow::{Context, Result};

use super::adoption::{Adoption, NIL_UUID};

/// Printed as the `<ExtendedConfigurationObject>` element (every object that
/// names one controls it; ИТК x7, probes).
const EXTENDED_OBJECT: &str = "9595ddd6-e72c-47ad-a156-672db811628c";

/// Controlled properties printed as elements. One uuid names one property
/// whatever the class (`Type` of an attribute and of a style item differ);
/// an element is printed only when the object's XML has it.
const ELEMENTS: &[(&str, &str)] = &[
    // An attribute's type (fixture `adopted/document_children`; a real extension).
    ("b1053250-abe6-11d4-9434-004095e12fc7", "Type"),
    // Fixtures `adopted/props_*` (every property, then three bit partitions).
    ("cf4abea3-37b2-11d4-940f-008048da11f9", "Synonym"),
    ("b69601f0-4cf6-11d4-9415-008048da11f9", "Owners"),
    ("b0ec341d-eede-4bb4-b303-ed50020b4c7f", "Hierarchical"),
    ("a865d4e5-8584-4894-8e49-4b4206f98f5a", "HierarchyType"),
    ("37f2fa9d-b276-11d4-9435-004095e12fc7", "CodeLength"),
    ("37f2fa9e-b276-11d4-9435-004095e12fc7", "CodeType"),
    ("37f2fa9f-b276-11d4-9435-004095e12fc7", "DescriptionLength"),
    ("bf9cc511-eb2a-48b6-a666-7afb78b83f36", "CodeAllowedLength"),
    ("60643a07-120a-4a63-9dba-67369c0f0145", "NumberType"),
    ("490eb6bd-24c4-4943-82aa-d0c4b666861b", "NumberLength"),
    (
        "c6c9689d-2978-42e8-863f-0280c6a85b56",
        "NumberAllowedLength",
    ),
    (
        "13134205-f60b-11d5-a3c7-0050bae0a776",
        "InformationRegisterPeriodicity",
    ),
    ("09c412e0-0f30-11d6-a3c7-0050bae0a776", "WriteMode"),
    ("bd533460-4001-11d6-a3c7-0050bae0a776", "RegisterType"),
    ("1ab4ba03-e4e6-4bf0-93e9-fcc28cc67567", "Type"),
    ("36e0f718-e0ba-4369-af8b-aff9a4ad7a35", "Value"),
    ("482411f7-457f-4889-a7c9-9adbfb1c7bd4", "Group"),
    // A subsystem's content (printed whether controlled or not).
    ("c6690627-40cf-4741-8719-4e7feb832b84", "Content"),
    // The only candidate in a real extension's dump (one printed property each).
    ("41721fe8-6dcb-4585-9ec2-5577d430cf6b", "Source"),
    ("98eafc16-765b-49e5-95c7-1f2a20625a01", "Location"),
    ("93a0e223-71ec-4c9d-8020-a62842ce0560", "Content"),
    // A filter criterion's content (its type state comes from the widened
    // list, not from this uuid).
    ("e1e8ea40-0906-11d6-a3c7-0050bae0a776", "Content"),
    ("7dbb2bc7-6bae-4b81-91cb-681317272a0b", "Global"),
    (
        "74ce8a02-abd2-46a6-8544-8cfbb4e8c6e0",
        "ClientManagedApplication",
    ),
    ("6275a02e-96f0-4347-975a-2d661e6a0675", "Server"),
    ("d12660a6-7298-4ae2-a332-b95a6459a280", "ExternalConnection"),
    (
        "436af77a-e846-4084-818b-740a3378518e",
        "ClientOrdinaryApplication",
    ),
    ("c474bab9-d13a-4fbd-bfb0-9214d6dc2fde", "ServerCall"),
    ("07ddee68-6fc0-4b88-9616-7792446d12b8", "ReturnValuesReuse"),
    ("e3331ed0-3854-478d-b6b5-4f14acdd6edb", "FormType"),
    ("4b2a2bcf-a845-41ba-a03d-05b09a2c6c11", "LanguageCode"),
];

/// Extended modules, forms and other parts, in the order their states are
/// written; controlled but not extended, they print nothing.
const STATES: &[(&str, &str)] = &[
    // Predefined items (a real extension: before the object module; controlled only, the
    // object prints nothing).
    ("f440939a-f130-413b-9c9a-2b18c4af69c6", "Predefined"),
    ("d5963243-262e-4398-b4d7-fb16d06484f6", "Module"),
    ("a637f77f-3840-441d-a1c3-699c8c5cb7e0", "ObjectModule"),
    ("d1b64a2c-8078-4982-8190-8f81aefda192", "ManagerModule"),
    ("32e087ab-1491-49b6-aba7-43571b41ac2b", "Form"),
    // A subsystem's command interface (fixtures `adopted/props_*`; the root's
    // own uses the same uuid).
    ("7f676314-716a-4d54-8335-a71a3857b21c", "CommandInterface"),
    // A real extension, one candidate each: a common command's module, an exchange
    // plan's content (`Ext/Content.xml`), a role's rights.
    ("078a6af8-d22c-4248-9c33-7e90075a3d2c", "CommandModule"),
    ("ee705b5d-dd16-46e4-b613-3a7edba5ba84", "Content"),
    ("3245b9fe-57f9-43dc-9c11-030f0617ce16", "Rights"),
];

const STATE_EXTENDED: u8 = 3;

/// A listed property the extension does not control (a real extension: a catalog listing
/// sixteen properties, all `0`, prints none of them; a style item's `Value`
/// is printed at `3`, not at `0`).
const STATE_NOT_CONTROLLED: u8 = 0;

/// What the rewrite could not name: the uuids of controlled properties no
/// probe has set apart. Their object keeps every property the pipeline wrote.
#[derive(Debug, Default, Eq, PartialEq)]
pub struct Unknown(pub Vec<String>);

/// The extension controls `property` of the object.
fn controls(adoption: &Adoption, property: &str) -> bool {
    adoption
        .state(property)
        .is_some_and(|state| state != STATE_NOT_CONTROLLED)
}

fn known(uuid: &str) -> bool {
    uuid == EXTENDED_OBJECT
        || ELEMENTS.iter().any(|(id, _)| *id == uuid)
        || STATES.iter().any(|(id, _)| *id == uuid)
}

/// A document's `<RegisterRecords>` is printed whether the extension controls
/// anything or not: it lists the registers of the extension that record the
/// document, `<RegisterRecords/>` when none does (a real extension: 19 empty, one listing
/// its own register; fixture `adopted/document_children`).
/// A subsystem's `<Content>` likewise: the extension's objects in it,
/// `<Content/>` when none (fixture `adopted/props_b0`).
const ALWAYS_PRINTED: [(&str, &str); 2] =
    [("Document", "RegisterRecords"), ("Subsystem", "Content")];

/// The `<Properties>` members at `prefix` (the member indentation plus `<`):
/// `(element name, lines)`.
fn members<'a>(block: &'a str, prefix: &str) -> Vec<(&'a str, &'a str)> {
    let mut starts = Vec::new();
    let mut offset = 0;
    for line in block.split_inclusive('\n') {
        if let Some(rest) = line.strip_prefix(prefix)
            && rest.starts_with(|c: char| c.is_alphabetic())
        {
            let name_end = rest
                .find(|c: char| c == '>' || c == '/' || c == ' ')
                .unwrap_or(rest.len());
            starts.push((offset, &rest[..name_end]));
        }
        offset += line.len();
    }
    let mut out = Vec::with_capacity(starts.len());
    for (index, (start, name)) in starts.iter().enumerate() {
        let end = starts.get(index + 1).map_or(block.len(), |(next, _)| *next);
        out.push((*name, &block[*start..end]));
    }
    out
}

fn property_states(adoption: &Adoption, indent: &str) -> String {
    let mut out = String::new();
    for &(uuid, name) in STATES {
        if adoption.state(uuid) == Some(STATE_EXTENDED) {
            out.push_str(&format!(
                "{indent}\t\t<xr:PropertyState>\r\n{indent}\t\t\t<xr:Property>{name}</xr:Property>\r\n\
                 {indent}\t\t\t<xr:State>Extended</xr:State>\r\n{indent}\t\t</xr:PropertyState>\r\n"
            ));
        }
    }
    if widens_type(adoption) {
        out.push_str(&format!(
            "{indent}\t\t<xr:PropertyState>\r\n{indent}\t\t\t<xr:Property>Type</xr:Property>\r\n\
             {indent}\t\t\t<xr:State>MultiState</xr:State>\r\n{indent}\t\t</xr:PropertyState>\r\n"
        ));
    }
    out
}

/// The type property in a widened list (see `adoption`).
const WIDENED_TYPE: &str = "b1053250-abe6-11d4-9434-004095e12fc7";

fn widens_type(adoption: &Adoption) -> bool {
    adoption
        .widened
        .iter()
        .any(|(property, state, _)| property == WIDENED_TYPE && *state != STATE_NOT_CONTROLLED)
}

/// The type an adopted element with a widened type checks, rendered at the
/// given indent: `(element uuid, indent)` → its `<v8:Type>`… lines, empty
/// when it checks none.
pub type CheckedType<'a> = &'a dyn Fn(&str, &str) -> Result<String>;

/// The pipeline's `<Type>` member (it reads the types the extension adds as
/// the element's type) as the platform prints a widened type (a real extension, 15
/// elements): `ExtendedProperty` of the checked type, when there is one,
/// and the added ones.
fn widened_type_xml(member: &str, indent: &str, checked: &str) -> Result<String> {
    let lines = member
        .strip_prefix(&format!("{indent}<Type>\r\n"))
        .and_then(|rest| rest.strip_suffix(&format!("{indent}</Type>\r\n")))
        .context("a widened type adds no types")?;
    let mut out = format!("{indent}<Type xsi:type=\"xr:ExtendedProperty\">\r\n");
    if !checked.is_empty() {
        out.push_str(&format!(
            "{indent}\t<xr:CheckValue xsi:type=\"v8:TypeDescription\">\r\n{checked}{indent}\t</xr:CheckValue>\r\n"
        ));
    }
    out.push_str(&format!(
        "{indent}\t<xr:ExtendValue xsi:type=\"v8:TypeDescription\">\r\n"
    ));
    for line in lines.split_inclusive('\n') {
        out.push('\t');
        out.push_str(line);
    }
    out.push_str(&format!(
        "{indent}\t</xr:ExtendValue>\r\n{indent}</Type>\r\n"
    ));
    Ok(out)
}

/// `xml` (the pipeline's metadata XML of one adopted object) rewritten from
/// the object's `adoption`.
pub fn rewrite(xml: &str, adoption: &Adoption, checked: CheckedType) -> Result<(String, Unknown)> {
    let (mut out, unknown) = rewrite_element(xml, "\t", adoption, checked)?;
    // An adopted register adopting none of its dimensions and resources
    // still prints `<ChildObjects/>` (fixture `adopted/props_all`); the
    // pipeline's register writers omit an empty one.
    let kind = out
        .find("\t<")
        .map(|at| &out[at + 2..])
        .and_then(|rest| rest.split([' ', '>']).next())
        .unwrap_or_default()
        .to_owned();
    if CHILD_OBJECTS_KINDS.contains(&kind.as_str()) && !out.contains("\r\n\t\t<ChildObjects") {
        let close = format!("\t</{kind}>");
        if let Some(at) = out.rfind(&close) {
            out.insert_str(at, "\t\t<ChildObjects/>\r\n");
        }
    }
    Ok((out, unknown))
}

/// Kinds whose dump always carries `<ChildObjects>` (a real extension, every kind it
/// adopts that has children).
const CHILD_OBJECTS_KINDS: [&str; 6] = [
    "InformationRegister",
    "AccumulationRegister",
    "AccountingRegister",
    "CalculationRegister",
    "ExchangePlan",
    "FilterCriterion",
];

/// The object element in `element` (its opening line indented by `indent`)
/// rewritten from its `adoption`; nested elements are left as they are
/// (their `<Properties>` sit deeper).
fn rewrite_element(
    element: &str,
    indent: &str,
    adoption: &Adoption,
    checked: CheckedType,
) -> Result<(String, Unknown)> {
    let xml = element;
    let kind = xml
        .find(&format!("{indent}<"))
        .map(|at| &xml[at + indent.len() + 1..])
        .and_then(|rest| rest.split([' ', '>']).next())
        .unwrap_or_default();
    let unknown = Unknown(
        adoption
            .properties
            .iter()
            .filter(|(uuid, state)| *state != STATE_NOT_CONTROLLED && !known(uuid))
            .map(|(uuid, _)| uuid.clone())
            .collect(),
    );
    let open_tag = format!("{indent}\t<Properties>\r\n");
    let close_tag = format!("{indent}\t</Properties>");
    let open = xml.find(&open_tag).context("no <Properties>")? + open_tag.len();
    let close = open + xml[open..].find(&close_tag).context("no </Properties>")?;
    let members = members(&xml[open..close], &format!("{indent}\t\t<"));
    let member = |name: &str| {
        members
            .iter()
            .find(|(tag, _)| *tag == name)
            .map(|(_, text)| *text)
    };
    let always = ALWAYS_PRINTED
        .iter()
        .filter(|(owner, _)| *owner == kind)
        .map(|(_, name)| *name)
        .collect::<Vec<_>>();

    let mut properties = format!("{indent}\t\t<ObjectBelonging>Adopted</ObjectBelonging>\r\n");
    properties.push_str(member("Name").context("no <Name>")?);
    if !unknown.0.is_empty() {
        // Refuse to guess which of them the extension controls: keep them all.
        for (tag, text) in &members {
            if *tag == "Name" {
                continue;
            }
            properties.push_str(text);
            if *tag == "Comment" && adoption.extended_object != NIL_UUID {
                properties.push_str(&extended_object(adoption, indent));
            }
        }
    } else {
        let controlled = ELEMENTS
            .iter()
            .filter(|(uuid, _)| controls(adoption, uuid))
            .map(|(_, name)| *name)
            .collect::<Vec<_>>();
        // A controlled synonym keeps its place between the name and the
        // comment (fixture `adopted/props_all`).
        if controlled.contains(&"Synonym")
            && let Some(synonym) = member("Synonym")
        {
            properties.push_str(synonym);
        }
        let comment = format!("{indent}\t\t<Comment/>\r\n");
        properties.push_str(member("Comment").unwrap_or(&comment));
        if controls(adoption, EXTENDED_OBJECT) && adoption.extended_object != NIL_UUID {
            properties.push_str(&extended_object(adoption, indent));
        }
        for (tag, text) in &members {
            if *tag == "Synonym" {
                continue;
            }
            if !(controlled.contains(tag) || always.contains(tag)) {
                continue;
            }
            if *tag == "Type" && widens_type(adoption) {
                let uuid = xml[xml.find(" uuid=\"").context("no uuid")? + 7..]
                    .split('"')
                    .next()
                    .unwrap_or_default();
                // A defined type prints no checked type, whatever its field
                // holds (fixture `adopted/widened`: `{"S",10,1}`, the default
                // an adopted defined type is created with).
                let checked = if kind == "DefinedType" {
                    String::new()
                } else {
                    checked(uuid, &format!("{indent}\t\t\t\t"))?
                };
                properties.push_str(&widened_type_xml(text, &format!("{indent}\t\t"), &checked)?);
            } else {
                properties.push_str(text);
            }
        }
    }

    let mut out = String::with_capacity(xml.len());
    out.push_str(&xml[..open]);
    out.push_str(&properties);
    out.push_str(&xml[close..]);

    // The element's own InternalInfo precedes its Properties; a nested one
    // sits deeper and after them.
    let states = property_states(adoption, indent);
    let properties_at = out.find(&open_tag).context("no <Properties>")?;
    let info_close = format!("{indent}\t</InternalInfo>");
    let info_empty = format!("{indent}\t<InternalInfo/>");
    if let Some(at) = out[..properties_at].find(&info_close) {
        out.insert_str(at, &states);
    } else if let Some(at) = out[..properties_at].find(&info_empty) {
        if !states.is_empty() {
            out.replace_range(
                at..at + info_empty.len(),
                &format!("{indent}\t<InternalInfo>\r\n{states}{info_close}"),
            );
        }
    } else {
        let info = if states.is_empty() {
            format!("{info_empty}\r\n")
        } else {
            format!("{indent}\t<InternalInfo>\r\n{states}{info_close}\r\n")
        };
        out.insert_str(properties_at, &info);
    }
    Ok((out, unknown))
}

/// Children an adopted object may adopt in turn.
const CHILD_TAGS: [&str; 10] = [
    // Fixture `adopted/props_all`; a real extension (34 values).
    "EnumValue",
    "Attribute",
    "TabularSection",
    "Dimension",
    "Resource",
    "AccountingFlag",
    "ExtDimensionAccountingFlag",
    "AddressingAttribute",
    "Command",
    "Column",
];

/// `xml` with every adopted child element (attributes, tabular sections and
/// the attributes in them, …) rewritten like its object: belonging, name,
/// comment, the extended object and only the controlled properties, an
/// `<InternalInfo/>` of its own (fixture `adopted/document_children`; a real extension).
/// `adoption_of` reads a child's adoption from the stored row.
pub fn rewrite_children(
    xml: &str,
    adoption_of: &dyn Fn(&str) -> Option<Adoption>,
    checked: CheckedType,
) -> Result<(String, Unknown)> {
    let mut starts = Vec::new();
    let mut offset = 0;
    for line in xml.split_inclusive('\n') {
        let trimmed = line.trim_start_matches('\t');
        let indent = &line[..line.len() - trimmed.len()];
        if indent.len() >= 3
            && let Some(rest) = trimmed.strip_prefix('<')
            && let Some(tag) = CHILD_TAGS.iter().find(|tag| {
                rest.strip_prefix(**tag)
                    .is_some_and(|after| after.starts_with(" uuid=\""))
            })
        {
            let uuid_start = offset + indent.len() + 1 + tag.len() + " uuid=\"".len();
            if let Some(len) = xml[uuid_start..].find('"') {
                starts.push((
                    offset,
                    indent.to_owned(),
                    *tag,
                    xml[uuid_start..uuid_start + len].to_owned(),
                ));
            }
        }
        offset += line.len();
    }
    let mut out = xml.to_owned();
    let mut unknown = Vec::new();
    for (start, indent, tag, uuid) in starts.into_iter().rev() {
        let Some(adoption) = adoption_of(&uuid).filter(|adoption| adoption.adopted) else {
            continue;
        };
        let close = format!("{indent}</{tag}>");
        let end = start
            + out[start..]
                .find(&close)
                .context("unterminated child element")?
            + close.len();
        let (rewritten, child_unknown) =
            rewrite_element(&out[start..end], &indent, &adoption, checked)
                .with_context(|| format!("adopted {tag} {uuid}"))?;
        out.replace_range(start..end, &rewritten);
        unknown.extend(child_unknown.0);
    }
    unknown.sort();
    unknown.dedup();
    Ok((out, Unknown(unknown)))
}

fn extended_object(adoption: &Adoption, indent: &str) -> String {
    format!(
        "{indent}\t\t<ExtendedConfigurationObject>{}</ExtendedConfigurationObject>\r\n",
        adoption.extended_object
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unknown_controlled_property_keeps_every_property() {
        let xml = "\t<CommonModule uuid=\"u\">\r\n\t\t<Properties>\r\n\t\t\t<Name>М</Name>\r\n\
                   \t\t\t<Synonym/>\r\n\t\t\t<Comment/>\r\n\t\t\t<Global>false</Global>\r\n\
                   \t\t</Properties>\r\n\t</CommonModule>\r\n";
        let adoption = Adoption {
            adopted: true,
            properties: vec![
                (EXTENDED_OBJECT.to_owned(), 2),
                ("ffffffff-0000-4000-8000-000000000001".to_owned(), 2),
            ],
            extended_object: "b0000000-0000-4000-8000-000000000001".to_owned(),
            widened: Vec::new(),
        };
        let (out, unknown) = rewrite(xml, &adoption, &|_, _| Ok(String::new())).unwrap();
        assert_eq!(unknown.0, vec!["ffffffff-0000-4000-8000-000000000001"]);
        assert_eq!(
            out,
            "\t<CommonModule uuid=\"u\">\r\n\t\t<InternalInfo/>\r\n\t\t<Properties>\r\n\
             \t\t\t<ObjectBelonging>Adopted</ObjectBelonging>\r\n\t\t\t<Name>М</Name>\r\n\
             \t\t\t<Synonym/>\r\n\t\t\t<Comment/>\r\n\
             \t\t\t<ExtendedConfigurationObject>b0000000-0000-4000-8000-000000000001</ExtendedConfigurationObject>\r\n\
             \t\t\t<Global>false</Global>\r\n\t\t</Properties>\r\n\t</CommonModule>\r\n"
        );
    }

    #[test]
    fn a_listed_property_at_state_zero_is_neither_printed_nor_unknown() {
        // A real extension: sixteen properties listed at 0, none
        // printed; `Owners` (known) at 0 is dropped like the unknown ones.
        let xml = "\t<Catalog uuid=\"u\">\r\n\t\t<InternalInfo/>\r\n\t\t<Properties>\r\n\t\t\t<Name>С</Name>\r\n\
                   \t\t\t<Synonym/>\r\n\t\t\t<Comment/>\r\n\t\t\t<Owners/>\r\n\
                   \t\t</Properties>\r\n\t</Catalog>\r\n";
        let adoption = Adoption {
            adopted: true,
            properties: vec![
                (EXTENDED_OBJECT.to_owned(), 2),
                ("b69601f0-4cf6-11d4-9415-008048da11f9".to_owned(), 0),
                ("ffffffff-0000-4000-8000-000000000001".to_owned(), 0),
            ],
            extended_object: "b0000000-0000-4000-8000-000000000001".to_owned(),
            widened: Vec::new(),
        };
        let (out, unknown) = rewrite(xml, &adoption, &|_, _| Ok(String::new())).unwrap();
        assert!(unknown.0.is_empty());
        assert_eq!(
            out,
            "\t<Catalog uuid=\"u\">\r\n\t\t<InternalInfo/>\r\n\t\t<Properties>\r\n\
             \t\t\t<ObjectBelonging>Adopted</ObjectBelonging>\r\n\t\t\t<Name>С</Name>\r\n\t\t\t<Comment/>\r\n\
             \t\t\t<ExtendedConfigurationObject>b0000000-0000-4000-8000-000000000001</ExtendedConfigurationObject>\r\n\
             \t\t</Properties>\r\n\t</Catalog>\r\n"
        );
    }

    #[test]
    fn an_extended_part_fills_an_empty_internal_info() {
        // A real extension: the pipeline writes
        // `<InternalInfo/>`, the extended command module goes inside it.
        let xml = "\t<CommonCommand uuid=\"u\">\r\n\t\t<InternalInfo/>\r\n\t\t<Properties>\r\n\t\t\t<Name>К</Name>\r\n\
                   \t\t\t<Comment/>\r\n\t\t</Properties>\r\n\t</CommonCommand>\r\n";
        let adoption = Adoption {
            adopted: true,
            properties: vec![("078a6af8-d22c-4248-9c33-7e90075a3d2c".to_owned(), 3)],
            extended_object: NIL_UUID.to_owned(),
            widened: Vec::new(),
        };
        let (out, _) = rewrite(xml, &adoption, &|_, _| Ok(String::new())).unwrap();
        assert!(
            out.starts_with(
                "\t<CommonCommand uuid=\"u\">\r\n\t\t<InternalInfo>\r\n\t\t\t<xr:PropertyState>\r\n\
                 \t\t\t\t<xr:Property>CommandModule</xr:Property>\r\n\t\t\t\t<xr:State>Extended</xr:State>\r\n\
                 \t\t\t</xr:PropertyState>\r\n\t\t</InternalInfo>\r\n\t\t<Properties>"
            ),
            "{out}"
        );
    }

    #[test]
    fn splits_properties_at_depth_three() {
        let block = "\t\t\t<Name>Х</Name>\r\n\t\t\t<Synonym>\r\n\t\t\t\t<v8:item/>\r\n\t\t\t</Synonym>\r\n\t\t\t<Comment/>\r\n";
        let names = members(block, "			<")
            .into_iter()
            .map(|(n, _)| n)
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["Name", "Synonym", "Comment"]);
    }
}
