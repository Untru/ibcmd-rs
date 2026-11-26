//! The `<Type>` blocks written from value types: the `<v8:Type>` lines with
//! their inline namespace declarations, and the number, string, date and
//! binary qualifiers.

use crate::types::{ConstantValueType, DATA_PROCESSOR_SETTINGS_COMPOSER_TYPE_NAME};

pub const DATA_COMPOSITION_SETTINGS_NAMESPACE_ATTRIBUTE: &str =
    r#" xmlns:dcsset="http://v8.1c.ru/8.1/data-composition-system/settings""#;

pub fn format_form_metadata_types_xml(value_types: &[ConstantValueType]) -> String {
    format_form_metadata_types_xml_with_indent(value_types, "\t\t\t")
}

pub fn format_form_metadata_types_xml_with_indent(
    value_types: &[ConstantValueType],
    indent: &str,
) -> String {
    let nested = format!("{indent}\t");
    let nested2 = format!("{nested}\t");
    let mut xml = format!("{indent}<Type>\r\n");
    for value_type in value_types {
        let value_type = scoped_value_type(value_type, indent.matches('\t').count() + 2);
        let tag_name = form_metadata_type_xml_tag(&value_type);
        let namespace_attr = form_metadata_type_xml_namespace_attr(&value_type);
        xml.push_str(&format!(
            "{nested}<v8:{tag_name}{namespace_attr}>{}</v8:{tag_name}>\r\n",
            metadata_type_xml_name(&value_type)
        ));
    }

    if let Some(number) = value_types.iter().find_map(|value_type| match value_type {
        ConstantValueType::Number {
            digits,
            fraction_digits,
            allowed_sign_flag,
        } => Some((*digits, *fraction_digits, *allowed_sign_flag)),
        _ => None,
    }) {
        xml.push_str(&format!("{nested}<v8:NumberQualifiers>\r\n"));
        xml.push_str(&format!("{nested2}<v8:Digits>{}</v8:Digits>\r\n", number.0));
        xml.push_str(&format!(
            "{nested2}<v8:FractionDigits>{}</v8:FractionDigits>\r\n",
            number.1
        ));
        xml.push_str(&format!(
            "{nested2}<v8:AllowedSign>{}</v8:AllowedSign>\r\n",
            number_allowed_sign_xml(number.2)
        ));
        xml.push_str(&format!("{nested}</v8:NumberQualifiers>\r\n"));
    }

    if let Some(string) = value_types.iter().find_map(|value_type| match value_type {
        ConstantValueType::String {
            length,
            allowed_length_flag,
        } => Some((
            length.unwrap_or(0),
            length.map(|_| *allowed_length_flag).unwrap_or(1),
        )),
        _ => None,
    }) {
        xml.push_str(&format!("{nested}<v8:StringQualifiers>\r\n"));
        xml.push_str(&format!("{nested2}<v8:Length>{}</v8:Length>\r\n", string.0));
        xml.push_str(&format!(
            "{nested2}<v8:AllowedLength>{}</v8:AllowedLength>\r\n",
            string_allowed_length_xml(string.1)
        ));
        xml.push_str(&format!("{nested}</v8:StringQualifiers>\r\n"));
    }

    if let Some(date_fractions) = value_types.iter().find_map(|value_type| {
        if let ConstantValueType::DateTime { date_fractions } = value_type {
            Some(*date_fractions)
        } else {
            None
        }
    }) {
        xml.push_str(&format!(
            "{nested}<v8:DateQualifiers>\r\n\
{nested2}<v8:DateFractions>{date_fractions}</v8:DateFractions>\r\n\
{nested}</v8:DateQualifiers>\r\n"
        ));
    }
    push_binary_data_qualifiers_xml(&mut xml, value_types, &nested);

    xml.push_str(&format!("{indent}</Type>\r\n"));
    xml
}

pub fn format_type_description_value_types_xml(
    value_types: &[ConstantValueType],
    indent: &str,
) -> String {
    let nested = format!("{indent}\t");
    let mut xml = String::new();
    for value_type in value_types {
        // This formatter is used only by a managed form's
        // `Settings xsi:type="v8:TypeDescription"`; use the form QName table
        // and its namespace declarations just like the attribute's own Type.
        // The generic table omits the txtedt declaration, which would leave a
        // `d5p1:TextDocument` value with an unbound prefix.
        let value_type = scoped_value_type(value_type, indent.matches('\t').count() + 1);
        let tag_name = form_metadata_type_xml_tag(&value_type);
        let namespace_attr = form_metadata_type_xml_namespace_attr(&value_type);
        xml.push_str(&format!(
            "{indent}<v8:{tag_name}{namespace_attr}>{}</v8:{tag_name}>\r\n",
            metadata_type_xml_name(&value_type)
        ));
    }
    if let Some(number) = value_types.iter().find_map(|value_type| match value_type {
        ConstantValueType::Number {
            digits,
            fraction_digits,
            allowed_sign_flag,
        } => Some((*digits, *fraction_digits, *allowed_sign_flag)),
        _ => None,
    }) {
        xml.push_str(&format!("{indent}<v8:NumberQualifiers>\r\n"));
        xml.push_str(&format!("{nested}<v8:Digits>{}</v8:Digits>\r\n", number.0));
        xml.push_str(&format!(
            "{nested}<v8:FractionDigits>{}</v8:FractionDigits>\r\n",
            number.1
        ));
        xml.push_str(&format!(
            "{nested}<v8:AllowedSign>{}</v8:AllowedSign>\r\n",
            number_allowed_sign_xml(number.2)
        ));
        xml.push_str(&format!("{indent}</v8:NumberQualifiers>\r\n"));
    }
    if let Some(string) = value_types.iter().find_map(|value_type| match value_type {
        ConstantValueType::String {
            length,
            allowed_length_flag,
        } => Some((
            length.unwrap_or(0),
            length.map(|_| *allowed_length_flag).unwrap_or(1),
        )),
        _ => None,
    }) {
        xml.push_str(&format!("{indent}<v8:StringQualifiers>\r\n"));
        xml.push_str(&format!("{nested}<v8:Length>{}</v8:Length>\r\n", string.0));
        xml.push_str(&format!(
            "{nested}<v8:AllowedLength>{}</v8:AllowedLength>\r\n",
            string_allowed_length_xml(string.1)
        ));
        xml.push_str(&format!("{indent}</v8:StringQualifiers>\r\n"));
    }
    if let Some(date_fractions) = value_types.iter().find_map(|value_type| {
        if let ConstantValueType::DateTime { date_fractions } = value_type {
            Some(*date_fractions)
        } else {
            None
        }
    }) {
        xml.push_str(&format!(
            "{indent}<v8:DateQualifiers>\r\n\
{nested}<v8:DateFractions>{date_fractions}</v8:DateFractions>\r\n\
{indent}</v8:DateQualifiers>\r\n"
        ));
    }
    push_binary_data_qualifiers_xml(&mut xml, value_types, indent);
    xml
}

/// `<v8:BinaryDataQualifiers>` of a type block that names `xs:base64Binary`, at
/// the indent of the qualifiers that precede it.
pub fn push_binary_data_qualifiers_xml(
    xml: &mut String,
    value_types: &[ConstantValueType],
    outer: &str,
) {
    if let Some((length, allowed_length_flag)) =
        value_types.iter().find_map(|value_type| match value_type {
            ConstantValueType::BinaryData {
                length,
                allowed_length_flag,
            } => Some((*length, *allowed_length_flag)),
            _ => None,
        })
    {
        xml.push_str(&format!(
            "{outer}<v8:BinaryDataQualifiers>\r\n{outer}\t<v8:Length>{length}</v8:Length>\r\n{outer}\t<v8:AllowedLength>{}</v8:AllowedLength>\r\n{outer}</v8:BinaryDataQualifiers>\r\n",
            string_allowed_length_xml(allowed_length_flag)
        ));
    }
}

/// A generated prefix records the declaration element's 1-based depth.
/// Rebase schema-known generated names to that depth; canonical names keep
/// their own prefix and namespace.
fn scoped_type_reference(reference: &str, depth: usize) -> std::borrow::Cow<'_, str> {
    let Some((prefix, local)) = reference.split_once(':') else {
        return std::borrow::Cow::Borrowed(reference);
    };
    let generated = ibcmd_schema::metadata_storage_facts::generated_type_prefix(prefix)
        && ibcmd_schema::metadata_storage_facts::data_processor_builtin_type_namespace_uri_ignoring_prefix(reference).is_some();
    let target_prefix = format!("d{depth}p1");
    if generated && prefix != target_prefix {
        std::borrow::Cow::Owned(format!("{target_prefix}:{local}"))
    } else {
        std::borrow::Cow::Borrowed(reference)
    }
}

pub fn scoped_value_type(value_type: &ConstantValueType, depth: usize) -> ConstantValueType {
    match value_type {
        ConstantValueType::Reference { reference } => ConstantValueType::Reference {
            reference: scoped_type_reference(reference, depth).into_owned(),
        },
        ConstantValueType::ReferenceTypeSet { reference } => ConstantValueType::ReferenceTypeSet {
            reference: scoped_type_reference(reference, depth).into_owned(),
        },
        other => other.clone(),
    }
}

pub fn form_metadata_type_xml_tag(value_type: &ConstantValueType) -> &'static str {
    match value_type {
        ConstantValueType::Reference { reference } => form_reference_type_tag(reference),
        ConstantValueType::ReferenceTypeSet { .. } => "TypeSet",
        ConstantValueType::TypeId { .. } => "TypeId",
        _ => "Type",
    }
}

pub fn metadata_type_xml_name(value_type: &ConstantValueType) -> String {
    match value_type {
        ConstantValueType::Boolean => "xs:boolean".to_string(),
        ConstantValueType::String { .. } => "xs:string".to_string(),
        ConstantValueType::Number { .. } => "xs:decimal".to_string(),
        ConstantValueType::DateTime { .. } => "xs:dateTime".to_string(),
        ConstantValueType::BinaryData { .. } => "xs:base64Binary".to_string(),
        ConstantValueType::Reference { reference, .. }
        | ConstantValueType::ReferenceTypeSet { reference, .. } => reference.clone(),
        ConstantValueType::TypeId { type_id } => type_id.clone(),
    }
}

pub fn metadata_type_xml_namespace_attr(value_type: &ConstantValueType) -> String {
    match value_type {
        ConstantValueType::Reference { reference }
        | ConstantValueType::ReferenceTypeSet { reference }
            if reference == DATA_PROCESSOR_SETTINGS_COMPOSER_TYPE_NAME =>
        {
            DATA_COMPOSITION_SETTINGS_NAMESPACE_ATTRIBUTE.to_owned()
        }
        ConstantValueType::Reference { reference }
        | ConstantValueType::ReferenceTypeSet { reference }
            if reference.starts_with("mxl:") =>
        {
            r#" xmlns:mxl="http://v8.1c.ru/8.2/data/spreadsheet""#.to_owned()
        }
        ConstantValueType::Reference { reference }
        | ConstantValueType::ReferenceTypeSet { reference } => {
            ibcmd_xml::metadata::data_processor_builtin_type_namespace_attribute_for_qname(
                reference,
            )
            .unwrap_or_default()
        }
        _ => String::new(),
    }
}

/// The inline binding a form body needs. SettingsComposer is root-bound;
/// ConditionalAppearance retains its measured spelling. Other bindings use
/// the shared XML renderer after the QName has been set from its depth.
fn form_metadata_type_xml_namespace_attr(value_type: &ConstantValueType) -> String {
    let reference = match value_type {
        ConstantValueType::Reference { reference }
        | ConstantValueType::ReferenceTypeSet { reference } => reference.as_str(),
        _ => "",
    };
    match reference {
        DATA_PROCESSOR_SETTINGS_COMPOSER_TYPE_NAME => String::new(),
        "d7p1:ConditionalAppearance" => {
            r#" xmlns:d7p1="http://v8.1c.ru/8.3/data/entext""#.to_owned()
        }
        _ => metadata_type_xml_namespace_attr(value_type),
    }
}

fn form_reference_type_tag(reference: &str) -> &'static str {
    if reference.starts_with("cfg:DefinedType.")
        || reference == "cfg:AnyIBRef"
        || reference == "cfg:ExchangePlanRef"
        || (reference.starts_with("cfg:") && reference.ends_with("Ref") && !reference.contains('.'))
    {
        "TypeSet"
    } else {
        "Type"
    }
}

pub fn string_allowed_length_xml(value: u8) -> &'static str {
    match value {
        0 => "Fixed",
        _ => "Variable",
    }
}

pub fn number_allowed_sign_xml(value: u8) -> &'static str {
    match value {
        1 => "Nonnegative",
        _ => "Any",
    }
}
