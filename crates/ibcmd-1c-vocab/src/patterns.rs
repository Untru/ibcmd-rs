//! Readers of a stored type pattern (`{"Pattern",...}`) into value types.

use std::collections::BTreeMap;

use ibcmd_1c_text::{parse_uuid_field, split_1c_braced_fields};

use crate::types::{ConstantValueType, form_builtin_type_reference};

// Platform `Null` type, serialized in type patterns as the bare element `{"L"}`.
const NULL_TYPE_REFERENCE: &str = "v8:Null";

/// A form attribute's stored type pattern. `keeps_unresolved_type_id` is
/// asked for an element whose type id neither `type_index` nor the form's
/// platform table resolves: `true` keeps the id as
/// [`ConstantValueType::TypeId`] at once, `false` lets the form-schema
/// column table try it first.
pub fn parse_form_metadata_type_pattern(
    value: &str,
    type_index: &BTreeMap<String, String>,
    keeps_unresolved_type_id: fn() -> bool,
) -> Option<Vec<ConstantValueType>> {
    let fields = split_1c_braced_fields(value, 0)?;
    if fields.first()?.trim() != r#""Pattern""# {
        return None;
    }
    fields
        .iter()
        .skip(1)
        .map(|field| {
            parse_metadata_type_pattern_element_with_builtin(
                field,
                type_index,
                form_builtin_type_reference,
                keeps_unresolved_type_id,
            )
            .or_else(|| {
                let element = split_1c_braced_fields(field, 0)?;
                let type_id = (element.first()?.trim() == r##""#""## && element.len() >= 2)
                    .then(|| parse_uuid_field(element.get(1)?.trim()))
                    .flatten()?;
                ibcmd_form_schema::form_attribute_column_builtin_type_reference(&type_id).map(
                    |reference| ConstantValueType::Reference {
                        reference: reference.to_owned(),
                    },
                )
            })
            .or_else(|| {
                let type_id = metadata_type_pattern_field_type_id(field)?;
                (!type_id.is_empty()).then_some(ConstantValueType::TypeId { type_id })
            })
        })
        .collect()
}

/// One element of a type pattern. `keeps_unresolved_type_id` is asked for a
/// `{"#",<id>}` element that neither `type_index` nor `builtin_reference`
/// resolves: `true` answers [`ConstantValueType::TypeId`], `false` `None`.
pub fn parse_metadata_type_pattern_element_with_builtin(
    value: &str,
    type_index: &BTreeMap<String, String>,
    builtin_reference: fn(&str) -> Option<&'static str>,
    keeps_unresolved_type_id: fn() -> bool,
) -> Option<ConstantValueType> {
    let element = split_1c_braced_fields(value, 0)?;
    match element.first()?.trim() {
        r#""B""# => Some(ConstantValueType::Boolean),
        r#""S""# if element.len() == 1 => Some(ConstantValueType::String {
            length: None,
            // Same reasoning as `parse_information_register_type_pattern_element`
            // (mod.rs:~16260): the serializer already forces "Variable" when
            // `length` is `None`, so `1` is the value that matches actual
            // emitted output.
            allowed_length_flag: 1,
        }),
        r#""S""# if element.len() == 3 => Some(ConstantValueType::String {
            length: Some(element.get(1)?.trim().parse().ok()?),
            allowed_length_flag: element.get(2)?.trim().parse().ok()?,
        }),
        r#""N""# if element.len() == 1 => Some(ConstantValueType::Number {
            digits: 0,
            fraction_digits: 0,
            allowed_sign_flag: 0,
        }),
        r#""N""# if element.len() == 4 => Some(ConstantValueType::Number {
            digits: element.get(1)?.trim().parse().ok()?,
            fraction_digits: element.get(2)?.trim().parse().ok()?,
            allowed_sign_flag: element.get(3)?.trim().parse().ok()?,
        }),
        r#""R""# if element.len() == 1 => Some(ConstantValueType::BinaryData {
            length: 0,
            allowed_length_flag: 1,
        }),
        r#""R""# if element.len() == 3 => Some(ConstantValueType::BinaryData {
            length: element.get(1)?.trim().parse().ok()?,
            allowed_length_flag: element
                .get(2)?
                .trim()
                .parse()
                .ok()
                .filter(|flag| *flag <= 1)?,
        }),
        r#""D""# => Some(ConstantValueType::DateTime {
            date_fractions: match element.get(1).map(|field| field.trim()) {
                Some(r#""D""#) => "Date",
                Some(r#""T""#) => "Time",
                _ => "DateTime",
            },
        }),
        // `{"L"}` is the serialized form of the platform `Null` type. Every one
        // of the 15 `v8:Null` type entries the platform writes across the 12
        // carrying files of 1C:Trade Management 11.5.27.75 is a bare
        // `<v8:Type>v8:Null</v8:Type>` with no qualifier of its own and with
        // its pattern position preserved, so it parses as an ordinary
        // reference-shaped leaf.
        r#""L""# if element.len() == 1 => Some(ConstantValueType::Reference {
            reference: NULL_TYPE_REFERENCE.to_string(),
        }),
        r##""#""## if element.len() >= 2 => {
            let type_id = parse_uuid_field(element.get(1)?.trim())?;
            let Some(reference) = type_index
                .get(&type_id)
                .cloned()
                .or_else(|| builtin_reference(&type_id).map(ToOwned::to_owned))
            else {
                // A type the configuration index and the builtin table both
                // miss: the platform prints its identifier. The substitution
                // belongs to the pattern reader, not to this leaf, so a
                // column- or form-scoped table still gets its turn first.
                return keeps_unresolved_type_id().then_some(ConstantValueType::TypeId { type_id });
            };
            Some(ConstantValueType::Reference { reference })
        }
        _ => None,
    }
}

pub fn metadata_type_pattern_field_type_id(value: &str) -> Option<String> {
    let element = split_1c_braced_fields(value, 0)?;
    if element.first()?.trim() == r##""#""## && element.len() >= 2 {
        parse_uuid_field(element.get(1)?.trim())
    } else {
        Some(String::new())
    }
}
