//! The export direction of `types.rs`: a type pattern -> the `<v8:Type>` /
//! `<v8:TypeSet>` / `<v8:TypeId>` items of a type description with their
//! qualifiers, a typed value -> its value element, design-time and metadata
//! references and data paths -> their text. It reads the forward tables of
//! `types.rs` (platform types, reference families, standard attribute codes),
//! so each table has one owner; `export::values` re-exports it.

use anyhow::{Result, anyhow, bail};

use super::{
    ACCOUNT_TYPE_TYPE, DESIGN_TIME_REF_TYPE, FIXED_ARRAY_TYPE, METADATA_OBJECT_REF_TYPE,
    TYPE_DESCRIPTION_TYPE, builtin_type_qname, ref_family, standard_attribute_codes,
};
use crate::metadata_model::brace::Brace;
use crate::metadata_model::export::values::{Owner, bool_text};
use crate::metadata_model::export::{
    Build, NameIndex, atom, el, is_nil, item, leaf, list, number, short, string, xml_text,
};
use crate::metadata_model::xml::Element;

// ---------------------------------------------------------------------------
// Type descriptions.

/// A named type is a type set when it stands for many types: a defined
/// type, a characteristic, any reference, or every object of one family.
fn is_type_set(name: &str) -> bool {
    let Some(local) = name.strip_prefix("cfg:") else {
        return false;
    };
    // Every object of one family (`cfg:CatalogRef`, `cfg:DocumentObject`,
    // `cfg:InformationRegisterRecordSet`, `cfg:ConstantValueManager`) is a
    // set; the unnamed managers (`cfg:CatalogManager`) and the four unnamed
    // platform types are types.
    local.starts_with("DefinedType.")
        || local.starts_with("Characteristic.")
        || local == "AnyIBRef"
        || local == "ConstantValueManager"
        || (!local.contains('.')
            && (local.ends_with("Ref")
                || local.ends_with("Object")
                || local.ends_with("RecordSet"))
            && local != "ReportObject")
}

/// The order the XML writes family type sets in. References follow their
/// type ids; objects and record sets follow the platform's own order, which
/// is not their type ids' (every event subscription source of the four
/// reference trees agrees; `ConstantValueManager` and `ExchangePlanObject`
/// never meet, nor does `CalculationRegisterRecordSet` meet another set).
/// Defined types and characteristics keep their stored order, ahead.
const FAMILY_TYPE_SET_ORDER: &[&str] = &[
    "cfg:AnyIBRef",
    "cfg:ExchangePlanRef",
    "cfg:BusinessProcessRoutePointRef",
    "cfg:BusinessProcessRef",
    "cfg:DocumentRef",
    "cfg:EnumRef",
    "cfg:ChartOfCalculationTypesRef",
    "cfg:TaskRef",
    "cfg:ChartOfCharacteristicTypesRef",
    "cfg:ChartOfAccountsRef",
    "cfg:CatalogRef",
    "cfg:BusinessProcessObject",
    "cfg:ChartOfCalculationTypesObject",
    "cfg:ChartOfAccountsObject",
    "cfg:ChartOfCharacteristicTypesObject",
    "cfg:ConstantValueManager",
    "cfg:ExchangePlanObject",
    "cfg:CatalogObject",
    "cfg:TaskObject",
    "cfg:DocumentObject",
    "cfg:InformationRegisterRecordSet",
    "cfg:AccountingRegisterRecordSet",
    "cfg:AccumulationRegisterRecordSet",
    "cfg:CalculationRegisterRecordSet",
    "cfg:SequenceRecordSet",
    "cfg:RecalculationRecordSet",
];

fn type_set_rank(name: &str) -> usize {
    FAMILY_TYPE_SET_ORDER
        .iter()
        .position(|candidate| *candidate == name)
        .map_or(0, |position| position + 1)
}

/// The children a type description writes: `v8:Type` items in stored
/// order, type sets behind them, then the qualifiers (number, string, date,
/// binary).
pub(crate) fn type_children(pattern: &Brace, names: &NameIndex) -> Result<Vec<Element>> {
    let items = list(pattern)?;
    if items.first().and_then(Brace::as_str) != Some("Pattern") {
        bail!("not a type pattern: {}", short(pattern));
    }
    let mut types = Vec::new();
    let mut sets = Vec::new();
    // Type ids the configuration names nothing by follow the named ones.
    let mut unnamed = Vec::new();
    let mut number_q = None;
    let mut string_q = None;
    let mut date_q = None;
    let mut binary_q = None;
    for entry in &items[1..] {
        let fields = list(entry)?;
        let tag = fields.first().and_then(Brace::as_str).unwrap_or_default();
        match tag {
            "B" => types.push(leaf("v8:Type", "xs:boolean")),
            "S" => {
                types.push(leaf("v8:Type", "xs:string"));
                let (length, allowed) = if fields.len() >= 3 {
                    (
                        atom(&fields[1])?.to_string(),
                        if atom(&fields[2])? == "0" {
                            "Fixed"
                        } else {
                            "Variable"
                        },
                    )
                } else {
                    ("0".to_string(), "Variable")
                };
                string_q = Some(
                    el("v8:StringQualifiers")
                        .child(leaf("v8:Length", length))
                        .child(leaf("v8:AllowedLength", allowed)),
                );
            }
            "N" => {
                types.push(leaf("v8:Type", "xs:decimal"));
                let (digits, fraction, sign) = if fields.len() >= 4 {
                    (
                        atom(&fields[1])?.to_string(),
                        atom(&fields[2])?.to_string(),
                        if atom(&fields[3])? == "1" {
                            "Nonnegative"
                        } else {
                            "Any"
                        },
                    )
                } else {
                    ("0".to_string(), "0".to_string(), "Any")
                };
                number_q = Some(
                    el("v8:NumberQualifiers")
                        .child(leaf("v8:Digits", digits))
                        .child(leaf("v8:FractionDigits", fraction))
                        .child(leaf("v8:AllowedSign", sign)),
                );
            }
            "D" => {
                types.push(leaf("v8:Type", "xs:dateTime"));
                let fractions = match fields.get(1).and_then(Brace::as_str) {
                    Some("D") => "Date",
                    Some("T") => "Time",
                    _ => "DateTime",
                };
                date_q = Some(el("v8:DateQualifiers").child(leaf("v8:DateFractions", fractions)));
            }
            "L" => types.push(leaf("v8:Type", "v8:Null")),
            "R" => {
                types.push(leaf("v8:Type", "xs:base64Binary"));
                let (length, allowed) = if fields.len() >= 3 {
                    (
                        atom(&fields[1])?.to_string(),
                        if atom(&fields[2])? == "0" {
                            "Fixed"
                        } else {
                            "Variable"
                        },
                    )
                } else {
                    ("0".to_string(), "Variable")
                };
                binary_q = Some(
                    el("v8:BinaryDataQualifiers")
                        .child(leaf("v8:Length", length))
                        .child(leaf("v8:AllowedLength", allowed)),
                );
            }
            "#" => {
                let type_id = atom(item(fields, 1)?)?;
                let name = match builtin_type_qname(type_id) {
                    Some(name) => name.to_string(),
                    None => match names.type_name(type_id) {
                        Some(name) => format!("cfg:{name}"),
                        None => {
                            unnamed.push(leaf("v8:TypeId", type_id));
                            continue;
                        }
                    },
                };
                if is_type_set(&name) {
                    sets.push(leaf("v8:TypeSet", name));
                } else {
                    types.push(leaf("v8:Type", name));
                }
            }
            other => bail!("unknown type pattern item {other:?}"),
        }
    }
    sets.sort_by_key(|set| type_set_rank(&set.text));
    types.extend(sets);
    types.extend(unnamed);
    types.extend(number_q);
    types.extend(string_q);
    types.extend(date_q);
    types.extend(binary_q);
    Ok(types)
}

/// A type description property (`<Type>`, `<CommandParameterType>`).
pub(crate) fn type_element(qname: &str, pattern: &Brace, names: &NameIndex) -> Result<Element> {
    Ok(el(qname).children(type_children(pattern, names)?))
}

// ---------------------------------------------------------------------------
// Typed values.

/// `{0,<ref type id>,<value uuid>}` -> `Catalog.X.EmptyRef`,
/// `Enum.E.EnumValue.V`, `Catalog.X.<predefined item>`, or the stored pair
/// when the type or the value is named by nothing.
pub(crate) fn design_time_ref_text(type_id: &str, value: &str, names: &NameIndex) -> String {
    if is_nil(type_id) && is_nil(value) {
        return String::new();
    }
    if let Some(type_name) = names.type_name(type_id)
        && let Some((prefix, object)) = type_name.split_once('.')
        && let Some(family) = ref_family(prefix)
    {
        let owner = format!("{family}.{object}");
        if is_nil(value) {
            return format!("{owner}.EmptyRef");
        }
        if let Some(full) = names.name(value)
            && full.starts_with(&format!("{owner}.EnumValue."))
        {
            return full.to_string();
        }
        if let Some(item) = names.predefined(value) {
            return format!("{owner}.{item}");
        }
    }
    format!("{type_id}.{value}")
}

/// `yyyymmddhhmmss` -> `yyyy-mm-ddThh:mm:ss`.
fn date_text(digits: &str) -> Result<String> {
    if digits.len() != 14 || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        bail!("bad stored date {digits}");
    }
    Ok(format!(
        "{}-{}-{}T{}:{}:{}",
        &digits[0..4],
        &digits[4..6],
        &digits[6..8],
        &digits[8..10],
        &digits[10..12],
        &digits[12..14]
    ))
}

/// A typed value (`MinValue`, `FillValue`, a choice parameter's value, a
/// fixed array's member) as an element named `qname`.
pub(crate) fn value_element(qname: &str, node: &Brace, names: &NameIndex) -> Result<Element> {
    let fields = list(node)?;
    let tag = fields.first().and_then(Brace::as_str).unwrap_or_default();
    let typed = |xsi_type: &str, text: String| leaf(qname, text).attr("type", xsi_type);
    Ok(match tag {
        "U" => el(qname).attr("nil", "true"),
        "S" => typed("xs:string", xml_text(string(item(fields, 1)?)?)),
        "B" => typed("xs:boolean", bool_text(item(fields, 1)?)?.to_string()),
        "N" => typed("xs:decimal", atom(item(fields, 1)?)?.to_string()),
        "D" => typed("xs:dateTime", date_text(atom(item(fields, 1)?)?)?),
        "#" => {
            let type_id = atom(item(fields, 1)?)?;
            let payload = item(fields, 2)?;
            match type_id {
                DESIGN_TIME_REF_TYPE => {
                    let pair = list(payload)?;
                    typed(
                        "xr:DesignTimeRef",
                        design_time_ref_text(atom(item(pair, 1)?)?, atom(item(pair, 2)?)?, names),
                    )
                }
                METADATA_OBJECT_REF_TYPE => {
                    let reference = list(payload)?;
                    let uuid = atom(item(reference, 1)?)?;
                    let name = names
                        .name(uuid)
                        .map(str::to_string)
                        .unwrap_or_else(|| uuid.to_string());
                    typed("xr:MDObjectRef", name)
                }
                TYPE_DESCRIPTION_TYPE => el(qname)
                    .attr("type", "v8:TypeDescription")
                    .children(type_children(payload, names)?),
                ACCOUNT_TYPE_TYPE => typed(
                    "ent:AccountType",
                    match number(payload)? {
                        0 => "Active",
                        1 => "Passive",
                        2 => "ActivePassive",
                        other => bail!("unknown account type {other}"),
                    }
                    .to_string(),
                ),
                FIXED_ARRAY_TYPE => {
                    let members = list(payload)?;
                    let count = number(item(members, 0)?)? as usize;
                    let mut array = el(qname).attr("type", "v8:FixedArray");
                    for member in members.iter().skip(1).take(count) {
                        array
                            .children
                            .push(value_element("v8:Value", member, names)?);
                    }
                    array
                }
                other => bail!("unsupported typed value {other}"),
            }
        }
        other => bail!("unsupported typed value tag {other:?}"),
    })
}

// ---------------------------------------------------------------------------
// Data paths and standard attribute names.

/// The name of a standard attribute of `base` (an object, or a tabular
/// section `X.Y.TabularSection.T`) by its code.
pub(crate) fn standard_attribute_name(base: &str, code: i64) -> Result<String> {
    let kind = base.split('.').next().unwrap_or_default();
    if base.contains(".TabularSection.")
        && code == crate::metadata_model::objects::line_number_marker(kind)
    {
        return Ok("LineNumber".to_string());
    }
    standard_attribute_codes(kind)
        .and_then(|table| {
            table
                .iter()
                .find_map(|(name, value)| (*value == code).then_some(*name))
        })
        .map(str::to_string)
        .ok_or_else(|| anyhow!("no standard attribute {code} of {base}"))
}

/// A field reference `{-N}` / `{0,<uuid>}` / `{0}` / `{-1}` -> its text,
/// relative to `base` for standard attributes.
pub(crate) fn field_text(segment: &Brace, base: &str, names: &NameIndex) -> Result<String> {
    let fields = list(segment)?;
    match fields {
        [code] => {
            let code = number(code)?;
            match code {
                0 => Ok("0".to_string()),
                -1 => Ok("-1".to_string()),
                _ => Ok(format!(
                    "{base}.StandardAttribute.{}",
                    standard_attribute_name(base, code)?
                )),
            }
        }
        [kind, uuid] if atom(kind)? == "0" => {
            let uuid = atom(uuid)?;
            names
                .name(uuid)
                .map(str::to_string)
                .ok_or_else(|| anyhow!("no name for field {uuid}"))
        }
        _ => bail!("unsupported field reference {}", short(segment)),
    }
}

/// A data path's segments -> its text; `{-N}` segments are standard
/// attributes of the object the path has reached (the owner at first).
///
/// A path is named only inside its owner: one that reaches into another
/// object, or names a standard attribute the owner does not have, is written
/// raw, segment by segment (`0:<uuid>/-8`).
pub(crate) fn data_path_text(
    segments: &[Brace],
    owner: Owner<'_>,
    names: &NameIndex,
) -> Result<String> {
    if let [segment] = segments
        && list(segment)?.len() == 1
    {
        let code = number(&list(segment)?[0])?;
        if code == 0 {
            return Ok("0".to_string());
        }
    }
    match named_data_path(segments, owner, names)? {
        Some(named) => Ok(named),
        None => raw_data_path(segments),
    }
}

fn named_data_path(
    segments: &[Brace],
    owner: Owner<'_>,
    names: &NameIndex,
) -> Result<Option<String>> {
    let inside = format!("{}.", owner.full_name);
    let mut current: Option<String> = None;
    for segment in segments {
        let fields = list(segment)?;
        match fields {
            [code] => {
                let code = number(code)?;
                let base = current
                    .clone()
                    .unwrap_or_else(|| owner.full_name.to_string());
                let Ok(name) = standard_attribute_name(&base, code) else {
                    return Ok(None);
                };
                current = Some(format!("{base}.StandardAttribute.{name}"));
            }
            [kind, uuid] if atom(kind)? == "0" => {
                let Some(name) = names.name(atom(uuid)?) else {
                    return Ok(None);
                };
                // A top-level object (another constant a constant's link
                // names) is named wherever it is.
                if !name.starts_with(&inside) && name.matches('.').count() != 1 {
                    return Ok(None);
                }
                current = Some(name.to_string());
            }
            _ => return Ok(None),
        }
    }
    Ok(current)
}

/// `{0,<uuid>}` -> `0:<uuid>`, `{-8}` -> `-8`, joined by `/`.
fn raw_data_path(segments: &[Brace]) -> Result<String> {
    let mut parts = Vec::with_capacity(segments.len());
    for segment in segments {
        let fields = list(segment)?;
        parts.push(match fields {
            [code] => atom(code)?.to_string(),
            [kind, uuid] => format!("{}:{}", atom(kind)?, atom(uuid)?),
            _ => bail!("unsupported data path segment {}", short(segment)),
        });
    }
    Ok(parts.join("/"))
}

// ---------------------------------------------------------------------------
// References.

/// `{"#",157fa490-...,{1,<uuid>}}` -> the full name it names, the uuid when
/// nothing does.
pub(crate) fn metadata_ref_text(node: &Brace, names: &NameIndex) -> Result<String> {
    let fields = list(node)?;
    if fields.first().and_then(Brace::as_str) != Some("#")
        || atom(item(fields, 1)?)? != METADATA_OBJECT_REF_TYPE
    {
        bail!("not a metadata reference: {}", short(node));
    }
    let uuid = atom(item(list(item(fields, 2)?)?, 1)?)?;
    Ok(names
        .name(uuid)
        .map(str::to_string)
        .unwrap_or_else(|| uuid.to_string()))
}
