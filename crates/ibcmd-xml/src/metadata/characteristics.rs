//! Exact XML codec for the canonical metadata `Characteristics` model.

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use ibcmd_core::characteristics::{
    Characteristic, CharacteristicField, CharacteristicFieldSentinel, CharacteristicFilterValue,
    CharacteristicReference, CharacteristicTypes, CharacteristicValues, Characteristics,
};
use ibcmd_core::model::CanonicalObjectParts;
use ibcmd_core::value::{CanonicalField, CanonicalText, CanonicalValue};

use super::common::{
    MetadataDecodeError, ResolvedNamespaces, XR_NAMESPACE, element_text, namespace_uri_for_prefix,
    typed,
};
use super::language::canonical_field;
use crate::{AttributeKind, XmlElement, XmlNode};

const XSI_NAMESPACE: &str = "http://www.w3.org/2001/XMLSchema-instance";
const XML_SCHEMA_NAMESPACE: &str = "http://www.w3.org/2001/XMLSchema";
const STRING_TYPE: &str = "xs:string";
const DESIGN_TIME_REF_TYPE: &str = "xr:DesignTimeRef";

/// A Characteristics value cannot be represented as a valid XML 1.0 fragment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CharacteristicsXmlError {
    InvalidIndent,
    InvalidXmlCharacter {
        context: &'static str,
        code_point: u32,
    },
}

impl Display for CharacteristicsXmlError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIndent => {
                formatter.write_str("Characteristics XML indent contains non-whitespace")
            }
            Self::InvalidXmlCharacter {
                context,
                code_point,
            } => write!(
                formatter,
                "Characteristics {context} contains XML 1.0-invalid U+{code_point:04X}"
            ),
        }
    }
}

impl Error for CharacteristicsXmlError {}

/// Renders the exact property element. Qualified names and child order are
/// intentionally centralized here rather than in a physical adapter.
pub fn render_characteristics_xml(
    model: &Characteristics,
    indent: &str,
) -> Result<String, CharacteristicsXmlError> {
    if !indent
        .chars()
        .all(|character| matches!(character, ' ' | '\t'))
    {
        return Err(CharacteristicsXmlError::InvalidIndent);
    }
    validate_model(model)?;
    if model.is_empty() {
        return Ok(format!("{indent}<Characteristics/>\r\n"));
    }
    let child_indent = format!("{indent}\t");
    let group_indent = format!("{child_indent}\t");
    let field_indent = format!("{group_indent}\t");
    let mut xml = format!("{indent}<Characteristics>\r\n");
    for item in model.items() {
        xml.push_str(&format!("{child_indent}<xr:Characteristic>\r\n"));
        xml.push_str(&format!(
            "{group_indent}<xr:CharacteristicTypes from=\"{}\">\r\n",
            escape_attribute(item.types().source().path())
        ));
        push_field(
            &mut xml,
            &field_indent,
            "KeyField",
            item.types().key_field(),
        );
        push_field(
            &mut xml,
            &field_indent,
            "TypesFilterField",
            item.types().types_filter_field(),
        );
        push_filter_value(&mut xml, &field_indent, item.types().types_filter_value());
        push_field(
            &mut xml,
            &field_indent,
            "DataPathField",
            item.types().data_path_field(),
        );
        push_field(
            &mut xml,
            &field_indent,
            "MultipleValuesUseField",
            item.types().multiple_values_use_field(),
        );
        xml.push_str(&format!("{group_indent}</xr:CharacteristicTypes>\r\n"));
        xml.push_str(&format!(
            "{group_indent}<xr:CharacteristicValues from=\"{}\">\r\n",
            escape_attribute(item.values().source().path())
        ));
        push_field(
            &mut xml,
            &field_indent,
            "ObjectField",
            item.values().object_field(),
        );
        push_field(
            &mut xml,
            &field_indent,
            "TypeField",
            item.values().type_field(),
        );
        push_field(
            &mut xml,
            &field_indent,
            "ValueField",
            item.values().value_field(),
        );
        push_field(
            &mut xml,
            &field_indent,
            "MultipleValuesKeyField",
            item.values().multiple_values_key_field(),
        );
        push_field(
            &mut xml,
            &field_indent,
            "MultipleValuesOrderField",
            item.values().multiple_values_order_field(),
        );
        xml.push_str(&format!("{group_indent}</xr:CharacteristicValues>\r\n"));
        xml.push_str(&format!("{child_indent}</xr:Characteristic>\r\n"));
    }
    xml.push_str(&format!("{indent}</Characteristics>\r\n"));
    Ok(xml)
}

/// Renders a Characteristics property at the verified metadata-properties
/// indentation used by all three supported owner families.
pub fn render_metadata_characteristics_xml(
    model: &Characteristics,
) -> Result<String, CharacteristicsXmlError> {
    render_characteristics_xml(model, "\t\t\t")
}

/// Renders the CCT Characteristics property together with the immediately
/// following verified properties whose order is part of the same XML schema.
pub fn render_cct_characteristics_xml(
    model: &Characteristics,
    predefined_data_update: &str,
    edit_type: &str,
    quick_choice: &str,
) -> Result<String, CharacteristicsXmlError> {
    validate_xml_1_0(predefined_data_update, "PredefinedDataUpdate")?;
    validate_xml_1_0(edit_type, "EditType")?;
    validate_xml_1_0(quick_choice, "QuickChoice")?;
    let mut xml = render_metadata_characteristics_xml(model)?;
    let predefined_data_update = escape_text(predefined_data_update);
    let edit_type = escape_text(edit_type);
    let quick_choice = escape_text(quick_choice);
    xml.push_str(&format!(
        "\t\t\t<PredefinedDataUpdate>{predefined_data_update}</PredefinedDataUpdate>\r\n\
\t\t\t<EditType>{edit_type}</EditType>\r\n\
\t\t\t<QuickChoice>{quick_choice}</QuickChoice>\r\n\
\t\t\t<ChoiceMode>BothWays</ChoiceMode>\r\n"
    ));
    Ok(xml)
}

pub(super) fn project_characteristics(
    parts: &mut CanonicalObjectParts,
    element: &XmlElement,
    uris: &ResolvedNamespaces,
) -> Result<(), MetadataDecodeError> {
    let model = decode_characteristics(element, uris)?;
    if model.is_empty() {
        return Ok(());
    }
    let values = model
        .items()
        .iter()
        .map(characteristic_value)
        .collect::<Result<Vec<_>, _>>()?;
    parts.properties.push(canonical_field(
        "Characteristics",
        CanonicalValue::sequence(values)
            .map_err(|error| MetadataDecodeError::Core(error.to_string()))?,
    )?);
    Ok(())
}

fn decode_characteristics(
    element: &XmlElement,
    uris: &ResolvedNamespaces,
) -> Result<Characteristics, MetadataDecodeError> {
    require_no_attributes(element, "Characteristics")?;
    let mut items = Vec::new();
    for item in strict_element_children(element)? {
        if !typed(item, "Characteristic", Some(XR_NAMESPACE), uris) {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "Characteristics contains an unknown item",
            ));
        }
        require_no_attributes(item, "Characteristic")?;
        let groups = exact_elements(item, 2)?;
        if !typed(groups[0], "CharacteristicTypes", Some(XR_NAMESPACE), uris)
            || !typed(groups[1], "CharacteristicValues", Some(XR_NAMESPACE), uris)
        {
            return Err(MetadataDecodeError::InvalidEnvelope(
                "Characteristic group order is not exact",
            ));
        }
        let types_source = group_source(groups[0])?;
        let values_source = group_source(groups[1])?;
        let type_fields = exact_elements(groups[0], 5)?;
        let value_fields = exact_elements(groups[1], 5)?;
        require_order(
            &type_fields,
            &[
                "KeyField",
                "TypesFilterField",
                "TypesFilterValue",
                "DataPathField",
                "MultipleValuesUseField",
            ],
            uris,
        )?;
        require_order(
            &value_fields,
            &[
                "ObjectField",
                "TypeField",
                "ValueField",
                "MultipleValuesKeyField",
                "MultipleValuesOrderField",
            ],
            uris,
        )?;
        let types = CharacteristicTypes::new(
            types_source,
            decode_field(type_fields[0])?,
            decode_field(type_fields[1])?,
            decode_filter_value(type_fields[2], uris)?,
            decode_field(type_fields[3])?,
            decode_field(type_fields[4])?,
        )
        .map_err(|error| MetadataDecodeError::Core(error.to_string()))?;
        let values = CharacteristicValues::new(
            values_source,
            decode_field(value_fields[0])?,
            decode_field(value_fields[1])?,
            decode_field(value_fields[2])?,
            decode_field(value_fields[3])?,
            decode_field(value_fields[4])?,
        )
        .map_err(|error| MetadataDecodeError::Core(error.to_string()))?;
        items.push(Characteristic::new(types, values));
    }
    Characteristics::new(items).map_err(|error| MetadataDecodeError::Core(error.to_string()))
}

fn group_source(group: &XmlElement) -> Result<CharacteristicReference, MetadataDecodeError> {
    let mut source = None;
    for attribute in group.attributes() {
        match attribute.kind() {
            AttributeKind::Ordinary(name) if name.prefix().is_none() && name.local() == "from" => {
                if source.replace(attribute.value()).is_some() {
                    return Err(MetadataDecodeError::Duplicate("Characteristic from"));
                }
            }
            _ => {
                return Err(MetadataDecodeError::InvalidEnvelope(
                    "Characteristic group attribute is unknown",
                ));
            }
        }
    }
    CharacteristicReference::new(
        source.ok_or(MetadataDecodeError::Missing("Characteristic from"))?,
        None,
    )
    .map_err(|error| MetadataDecodeError::Core(error.to_string()))
}

fn decode_field(element: &XmlElement) -> Result<CharacteristicField, MetadataDecodeError> {
    require_no_attributes(element, "Characteristic field")?;
    let value = element_text(element)?.unwrap_or_default();
    match value.as_str() {
        "-1" => Ok(CharacteristicField::Sentinel(
            CharacteristicFieldSentinel::Undefined,
        )),
        "0" => Ok(CharacteristicField::Sentinel(
            CharacteristicFieldSentinel::Empty,
        )),
        _ => CharacteristicReference::new(&value, None)
            .map(CharacteristicField::Reference)
            .map_err(|error| MetadataDecodeError::Core(error.to_string())),
    }
}

fn decode_filter_value(
    element: &XmlElement,
    uris: &ResolvedNamespaces,
) -> Result<CharacteristicFilterValue, MetadataDecodeError> {
    let mut xsi_type = None;
    for attribute in element.attributes() {
        match attribute.kind() {
            AttributeKind::Ordinary(name)
                if name.local() == "type"
                    && name
                        .prefix()
                        .and_then(|prefix| namespace_uri_for_prefix(element, prefix, uris))
                        == Some(XSI_NAMESPACE) =>
            {
                if xsi_type.replace(attribute.value()).is_some() {
                    return Err(MetadataDecodeError::Duplicate("TypesFilterValue xsi:type"));
                }
            }
            _ => {
                return Err(MetadataDecodeError::InvalidEnvelope(
                    "TypesFilterValue attribute is unknown",
                ));
            }
        }
    }
    let value = element_text(element)?.unwrap_or_default();
    let xsi_type = xsi_type.ok_or(MetadataDecodeError::Missing("TypesFilterValue xsi:type"))?;
    let (prefix, local) = xsi_type
        .split_once(':')
        .ok_or(MetadataDecodeError::InvalidEnvelope(
            "TypesFilterValue type QName is unqualified",
        ))?;
    let namespace = namespace_uri_for_prefix(element, prefix, uris).ok_or(
        MetadataDecodeError::InvalidEnvelope("TypesFilterValue type QName prefix is unbound"),
    )?;
    match (namespace, local) {
        (XML_SCHEMA_NAMESPACE, "string") => CharacteristicFilterValue::string(&value)
            .map_err(|error| MetadataDecodeError::Core(error.to_string())),
        (XR_NAMESPACE, "DesignTimeRef") if value.is_empty() => {
            Ok(CharacteristicFilterValue::DesignTimeRef(None))
        }
        (XR_NAMESPACE, "DesignTimeRef") => CharacteristicReference::new(&value, None)
            .map(|reference| CharacteristicFilterValue::DesignTimeRef(Some(reference)))
            .map_err(|error| MetadataDecodeError::Core(error.to_string())),
        _ => Err(MetadataDecodeError::InvalidEnvelope(
            "TypesFilterValue union is unsupported",
        )),
    }
}

fn exact_elements(
    parent: &XmlElement,
    expected: usize,
) -> Result<Vec<&XmlElement>, MetadataDecodeError> {
    let elements = strict_element_children(parent)?;
    if elements.len() != expected {
        return Err(MetadataDecodeError::InvalidEnvelope(
            "Characteristic child inventory is not exact",
        ));
    }
    Ok(elements)
}

fn strict_element_children(parent: &XmlElement) -> Result<Vec<&XmlElement>, MetadataDecodeError> {
    let mut elements = Vec::new();
    for node in parent.children() {
        match node {
            XmlNode::Element(element) => elements.push(element),
            XmlNode::Text(value) if value.value().trim().is_empty() => {}
            XmlNode::CData(value) if value.value().trim().is_empty() => {}
            XmlNode::Text(_)
            | XmlNode::CData(_)
            | XmlNode::Comment(_)
            | XmlNode::ProcessingInstruction(_)
            | XmlNode::DocType(_) => {
                return Err(MetadataDecodeError::InvalidEnvelope(
                    "Characteristic mixed content is not allowed",
                ));
            }
        }
    }
    Ok(elements)
}

fn require_order(
    elements: &[&XmlElement],
    expected: &[&str],
    uris: &ResolvedNamespaces,
) -> Result<(), MetadataDecodeError> {
    if elements
        .iter()
        .zip(expected)
        .all(|(element, local)| typed(element, local, Some(XR_NAMESPACE), uris))
    {
        Ok(())
    } else {
        Err(MetadataDecodeError::InvalidEnvelope(
            "Characteristic field order is not exact",
        ))
    }
}

fn require_no_attributes(
    element: &XmlElement,
    label: &'static str,
) -> Result<(), MetadataDecodeError> {
    if element.attributes().is_empty() {
        Ok(())
    } else {
        Err(MetadataDecodeError::InvalidEnvelope(label))
    }
}

fn characteristic_value(item: &Characteristic) -> Result<CanonicalValue, MetadataDecodeError> {
    // This is the XML envelope projection: XML carries resolved paths but not
    // native source UUIDs. Raw provenance remains authoritative in the
    // dedicated ibcmd-core model used by physical adapters.
    let mut fields = vec![
        text_field("TypesFrom", item.types().source().path())?,
        field_value("KeyField", item.types().key_field())?,
        field_value("TypesFilterField", item.types().types_filter_field())?,
    ];
    match item.types().types_filter_value() {
        CharacteristicFilterValue::String(value) => {
            fields.push(text_field("TypesFilterValueKind", "string")?);
            fields.push(text_field("TypesFilterValue", value)?);
        }
        CharacteristicFilterValue::DesignTimeRef(reference) => {
            fields.push(text_field("TypesFilterValueKind", "design_time_ref")?);
            fields.push(text_field(
                "TypesFilterValue",
                reference.as_ref().map_or("", |reference| reference.path()),
            )?);
        }
    }
    fields.extend([
        field_value("DataPathField", item.types().data_path_field())?,
        field_value(
            "MultipleValuesUseField",
            item.types().multiple_values_use_field(),
        )?,
        text_field("ValuesFrom", item.values().source().path())?,
        field_value("ObjectField", item.values().object_field())?,
        field_value("TypeField", item.values().type_field())?,
        field_value("ValueField", item.values().value_field())?,
        field_value(
            "MultipleValuesKeyField",
            item.values().multiple_values_key_field(),
        )?,
        field_value(
            "MultipleValuesOrderField",
            item.values().multiple_values_order_field(),
        )?,
    ]);
    CanonicalValue::record(fields).map_err(|error| MetadataDecodeError::Core(error.to_string()))
}

fn field_value(
    name: &str,
    field: &CharacteristicField,
) -> Result<CanonicalField, MetadataDecodeError> {
    let value = match field {
        CharacteristicField::Reference(reference) => reference.path(),
        CharacteristicField::Sentinel(CharacteristicFieldSentinel::Undefined) => "undefined",
        CharacteristicField::Sentinel(CharacteristicFieldSentinel::Empty) => "empty",
    };
    text_field(name, value)
}

fn text_field(name: &str, value: &str) -> Result<CanonicalField, MetadataDecodeError> {
    CanonicalField::named(
        name,
        CanonicalValue::text(
            CanonicalText::new(value)
                .map_err(|error| MetadataDecodeError::Core(error.to_string()))?,
        ),
    )
    .map_err(|error| MetadataDecodeError::Core(error.to_string()))
}

fn validate_model(model: &Characteristics) -> Result<(), CharacteristicsXmlError> {
    for item in model.items() {
        validate_reference(item.types().source(), "types source")?;
        validate_field(item.types().key_field())?;
        validate_field(item.types().types_filter_field())?;
        match item.types().types_filter_value() {
            CharacteristicFilterValue::String(value) => {
                validate_xml_1_0(value, "string filter value")?;
            }
            CharacteristicFilterValue::DesignTimeRef(Some(reference)) => {
                validate_reference(reference, "design-time reference")?;
            }
            CharacteristicFilterValue::DesignTimeRef(None) => {}
        }
        validate_field(item.types().data_path_field())?;
        validate_field(item.types().multiple_values_use_field())?;
        validate_reference(item.values().source(), "values source")?;
        validate_field(item.values().object_field())?;
        validate_field(item.values().type_field())?;
        validate_field(item.values().value_field())?;
        validate_field(item.values().multiple_values_key_field())?;
        validate_field(item.values().multiple_values_order_field())?;
    }
    Ok(())
}

fn validate_field(field: &CharacteristicField) -> Result<(), CharacteristicsXmlError> {
    if let CharacteristicField::Reference(reference) = field {
        validate_reference(reference, "field reference")?;
    }
    Ok(())
}

fn validate_reference(
    reference: &CharacteristicReference,
    context: &'static str,
) -> Result<(), CharacteristicsXmlError> {
    validate_xml_1_0(reference.path(), context)
}

fn validate_xml_1_0(value: &str, context: &'static str) -> Result<(), CharacteristicsXmlError> {
    if let Some(character) = value.chars().find(|character| !is_xml_1_0(*character)) {
        return Err(CharacteristicsXmlError::InvalidXmlCharacter {
            context,
            code_point: character as u32,
        });
    }
    Ok(())
}

const fn is_xml_1_0(character: char) -> bool {
    matches!(character, '\u{9}' | '\u{A}' | '\u{D}')
        || character as u32 >= 0x20 && character as u32 <= 0xD7FF
        || character as u32 >= 0xE000 && character as u32 <= 0xFFFD
        || character as u32 >= 0x10000
}

fn push_field(xml: &mut String, indent: &str, name: &str, field: &CharacteristicField) {
    let value = match field {
        CharacteristicField::Reference(reference) => reference.path(),
        CharacteristicField::Sentinel(CharacteristicFieldSentinel::Undefined) => "-1",
        CharacteristicField::Sentinel(CharacteristicFieldSentinel::Empty) => "0",
    };
    xml.push_str(&format!(
        "{indent}<xr:{name}>{}</xr:{name}>\r\n",
        escape_text(value)
    ));
}

fn push_filter_value(xml: &mut String, indent: &str, value: &CharacteristicFilterValue) {
    match value {
        CharacteristicFilterValue::String(value) if value.is_empty() => xml.push_str(&format!(
            "{indent}<xr:TypesFilterValue xsi:type=\"{STRING_TYPE}\"/>\r\n"
        )),
        CharacteristicFilterValue::String(value) => xml.push_str(&format!(
            "{indent}<xr:TypesFilterValue xsi:type=\"{STRING_TYPE}\">{}</xr:TypesFilterValue>\r\n",
            escape_text(value)
        )),
        CharacteristicFilterValue::DesignTimeRef(None) => xml.push_str(&format!(
            "{indent}<xr:TypesFilterValue xsi:type=\"{DESIGN_TIME_REF_TYPE}\"/>\r\n"
        )),
        CharacteristicFilterValue::DesignTimeRef(Some(reference)) => xml.push_str(&format!(
            "{indent}<xr:TypesFilterValue xsi:type=\"{DESIGN_TIME_REF_TYPE}\">{}</xr:TypesFilterValue>\r\n",
            escape_text(reference.path())
        )),
    }
}

fn escape_attribute(value: &str) -> String {
    escape_text(value).replace('"', "&quot;")
}

fn escape_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::XmlReader;
    use crate::metadata::common::resolve_namespaces;

    fn reference(path: &str) -> CharacteristicReference {
        CharacteristicReference::new(path, None).unwrap()
    }

    fn model_with(types_source: &str, filter: &str) -> Characteristics {
        let undefined = CharacteristicField::Sentinel(CharacteristicFieldSentinel::Undefined);
        let empty = CharacteristicField::Sentinel(CharacteristicFieldSentinel::Empty);
        let types = CharacteristicTypes::new(
            reference(types_source),
            empty.clone(),
            undefined.clone(),
            CharacteristicFilterValue::string(filter).unwrap(),
            empty.clone(),
            undefined.clone(),
        )
        .unwrap();
        let values = CharacteristicValues::new(
            reference("Catalog.Values"),
            empty.clone(),
            undefined.clone(),
            empty.clone(),
            undefined.clone(),
            empty,
        )
        .unwrap();
        Characteristics::new(vec![Characteristic::new(types, values)]).unwrap()
    }

    #[test]
    fn renderer_owns_exact_group_and_field_order() {
        let undefined = CharacteristicField::Sentinel(CharacteristicFieldSentinel::Undefined);
        let empty = CharacteristicField::Sentinel(CharacteristicFieldSentinel::Empty);
        let types = CharacteristicTypes::new(
            reference("Catalog.Types"),
            empty.clone(),
            undefined.clone(),
            CharacteristicFilterValue::string("a&b").unwrap(),
            empty.clone(),
            undefined.clone(),
        )
        .unwrap();
        let values = CharacteristicValues::new(
            reference("Catalog.Values"),
            empty.clone(),
            undefined.clone(),
            empty.clone(),
            undefined.clone(),
            empty,
        )
        .unwrap();
        let model = Characteristics::new(vec![Characteristic::new(types, values)]).unwrap();
        let xml = render_characteristics_xml(&model, "\t").unwrap();
        let ordered = [
            "CharacteristicTypes",
            "KeyField",
            "TypesFilterField",
            "TypesFilterValue",
            "DataPathField",
            "MultipleValuesUseField",
            "CharacteristicValues",
            "ObjectField",
            "TypeField",
            "ValueField",
            "MultipleValuesKeyField",
            "MultipleValuesOrderField",
        ];
        let mut previous = 0;
        for name in ordered {
            let index = xml[previous..].find(name).unwrap() + previous;
            assert!(index >= previous);
            previous = index;
        }
        assert!(xml.contains("a&amp;b"));
    }

    #[test]
    fn decoder_rejects_non_whitespace_mixed_content_at_every_level() {
        let cases = [
            "<Characteristics>garbage</Characteristics>",
            "<Characteristics><xr:Characteristic>garbage</xr:Characteristic></Characteristics>",
            "<Characteristics><xr:Characteristic><xr:CharacteristicTypes from=\"Catalog.Types\">garbage</xr:CharacteristicTypes><xr:CharacteristicValues from=\"Catalog.Values\"/></xr:Characteristic></Characteristics>",
            "<Characteristics><xr:Characteristic><xr:CharacteristicTypes from=\"Catalog.Types\"/><xr:CharacteristicValues from=\"Catalog.Values\">garbage</xr:CharacteristicValues></xr:Characteristic></Characteristics>",
        ];
        for body in cases {
            let xml = format!("<Root xmlns=\"urn:test\" xmlns:xr=\"{XR_NAMESPACE}\">{body}</Root>");
            let document = XmlReader::from_slice(xml.as_bytes()).unwrap();
            let uris = resolve_namespaces(document.root()).unwrap();
            let characteristics = document
                .root()
                .children()
                .iter()
                .find_map(|node| match node {
                    XmlNode::Element(element) => Some(element),
                    _ => None,
                })
                .unwrap();
            assert!(decode_characteristics(characteristics, &uris).is_err());
        }
    }

    fn qname_fixture(xsi_uri: &str, schema_uri: &str) -> String {
        format!(
            "<Root xmlns=\"urn:test\" xmlns:r=\"{XR_NAMESPACE}\" xmlns:i=\"{xsi_uri}\" xmlns:s=\"{schema_uri}\"><Characteristics><r:Characteristic><r:CharacteristicTypes from=\"Catalog.Types\"><r:KeyField>0</r:KeyField><r:TypesFilterField>-1</r:TypesFilterField><r:TypesFilterValue i:type=\"s:string\">safe</r:TypesFilterValue><r:DataPathField>0</r:DataPathField><r:MultipleValuesUseField>-1</r:MultipleValuesUseField></r:CharacteristicTypes><r:CharacteristicValues from=\"Catalog.Values\"><r:ObjectField>0</r:ObjectField><r:TypeField>-1</r:TypeField><r:ValueField>0</r:ValueField><r:MultipleValuesKeyField>-1</r:MultipleValuesKeyField><r:MultipleValuesOrderField>0</r:MultipleValuesOrderField></r:CharacteristicValues></r:Characteristic></Characteristics></Root>"
        )
    }

    fn decode_fixture(xml: &str) -> Result<Characteristics, MetadataDecodeError> {
        let document = XmlReader::from_slice(xml.as_bytes()).unwrap();
        let uris = resolve_namespaces(document.root()).unwrap();
        let element = document
            .root()
            .children()
            .iter()
            .find_map(|node| match node {
                XmlNode::Element(element) => Some(element),
                _ => None,
            })
            .unwrap();
        decode_characteristics(element, &uris)
    }

    #[test]
    fn xsi_type_is_resolved_by_namespace_uri_not_prefix_lexeme() {
        let alternate = qname_fixture(XSI_NAMESPACE, XML_SCHEMA_NAMESPACE);
        assert!(decode_fixture(&alternate).is_ok());
        assert!(decode_fixture(&qname_fixture("urn:spoof", XML_SCHEMA_NAMESPACE)).is_err());
        assert!(decode_fixture(&qname_fixture(XSI_NAMESPACE, "urn:spoof")).is_err());
    }

    #[test]
    fn renderer_rejects_xml_1_0_controls_in_paths_filters_and_cct_tail() {
        for model in [
            model_with("Catalog.Bad\u{1}", "safe"),
            model_with("Catalog.Types", "bad\u{1}"),
        ] {
            assert!(matches!(
                render_metadata_characteristics_xml(&model),
                Err(CharacteristicsXmlError::InvalidXmlCharacter { .. })
            ));
        }

        for values in [
            ("bad\u{1}", "Dialog", "true"),
            ("Auto", "bad\u{1}", "true"),
            ("Auto", "Dialog", "bad\u{1}"),
        ] {
            assert!(matches!(
                render_cct_characteristics_xml(
                    &Characteristics::default(),
                    values.0,
                    values.1,
                    values.2,
                ),
                Err(CharacteristicsXmlError::InvalidXmlCharacter { .. })
            ));
        }
    }

    #[test]
    fn renderer_escapes_attributes_filter_text_and_all_cct_tail_values() {
        let model = model_with("Catalog.T&\"<", "a&<b>");
        let xml = render_cct_characteristics_xml(&model, "A&<", "D>\"", "Q<&").unwrap();
        assert!(xml.contains("from=\"Catalog.T&amp;&quot;&lt;\""));
        assert!(xml.contains(">a&amp;&lt;b&gt;</xr:TypesFilterValue>"));
        assert!(xml.contains("<PredefinedDataUpdate>A&amp;&lt;</PredefinedDataUpdate>"));
        assert!(xml.contains("<EditType>D&gt;\"</EditType>"));
        assert!(xml.contains("<QuickChoice>Q&lt;&amp;</QuickChoice>"));
    }
}
