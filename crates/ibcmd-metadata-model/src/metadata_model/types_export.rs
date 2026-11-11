//! The export direction of `types.rs`: a type pattern -> the `<v8:Type>` /
//! `<v8:TypeSet>` / `<v8:TypeId>` items of a type description with their
//! qualifiers, a typed value -> its value element, design-time and metadata
//! references and data paths -> their text. It reads the forward tables of
//! `types.rs` (platform types, reference families, standard attribute codes),
//! so each table has one owner; `export::values` re-exports it.

use std::collections::HashSet;

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

/// The native exporter normalizes an empty string FillValue to undefined
/// only when the declared type's leaves are known to exclude strings.
/// This rule is specific to FillValue, not other typed-value properties.
pub(crate) fn fill_value_element(
    value: &Brace,
    pattern: &Brace,
    names: &NameIndex,
) -> Result<Element> {
    fn excludes_string(
        children: &[Element],
        names: &NameIndex,
        visiting: &mut HashSet<String>,
    ) -> Option<bool> {
        let mut found = false;
        let mut excludes = true;
        for child in children {
            let text = child.text.trim();
            match child.name.as_str() {
                "Type" if text == "xs:string" => {
                    found = true;
                    excludes = false;
                }
                "Type" | "TypeSet" if is_type_set(text) => {
                    found = true;
                    let name = text.strip_prefix("cfg:")?;
                    let definition = names.type_definition(name)?;
                    if !visiting.insert(name.to_string()) {
                        return None;
                    }
                    let nested = excludes_string(&definition, names, visiting);
                    visiting.remove(name);
                    excludes &= nested?;
                }
                "TypeSet" | "TypeId" => return None,
                "Type" if text.is_empty() => return None,
                "Type" => found = true,
                _ => {}
            }
        }
        found.then_some(excludes)
    }

    let empty_string = value.as_list().is_some_and(|fields| {
        fields.len() == 2 && fields[0].as_str() == Some("S") && fields[1].as_str() == Some("")
    });
    if empty_string
        && excludes_string(&type_children(pattern, names)?, names, &mut HashSet::new())
            == Some(true)
    {
        Ok(el("FillValue").attr("nil", "true"))
    } else {
        value_element("FillValue", value, names)
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata_model::brace::parse_row;
    use crate::metadata_model::export::{GeneratedTypeName, ObjectNames, object_names};
    use crate::metadata_model::{DescriptorContext, compile_descriptor};
    use ibcmd_restructure::brace_list;

    fn reference(id: &str) -> Brace {
        brace_list![
            Brace::str("Pattern"),
            brace_list![Brace::str("#"), Brace::atom(id)]
        ]
    }

    #[test]
    fn fill_value_resolves_named_leaves_and_keeps_unknown_or_cyclic_sets() {
        let mut names = NameIndex::default();
        for (id, name, category, pattern) in [
            (
                "number",
                "DefinedType.Number",
                "DefinedType",
                r##"{"Pattern",{"N",10,2,0}}"##,
            ),
            (
                "string",
                "DefinedType.String",
                "DefinedType",
                r##"{"Pattern",{"S",10,1}}"##,
            ),
            (
                "nested",
                "DefinedType.Nested",
                "DefinedType",
                r##"{"Pattern",{"#",number}}"##,
            ),
            (
                "characteristic",
                "Characteristic.Values",
                "Characteristic",
                r##"{"Pattern",{"B"},{"#",nested}}"##,
            ),
            (
                "mixed",
                "DefinedType.Mixed",
                "DefinedType",
                r##"{"Pattern",{"#",number},{"#",string}}"##,
            ),
            (
                "cycle",
                "DefinedType.Cycle",
                "DefinedType",
                r##"{"Pattern",{"#",cycle}}"##,
            ),
            (
                "unknown",
                "DefinedType.Unknown",
                "DefinedType",
                r##"{"Pattern",{"#",missing}}"##,
            ),
            (
                "empty",
                "DefinedType.Empty",
                "DefinedType",
                r##"{"Pattern"}"##,
            ),
        ] {
            // Synthetic declarations exercise graph resolution independently
            // of native record offsets; the snapshot/row test below covers
            // extraction of declarations from real descriptor layouts.
            names.add(&ObjectNames {
                types: vec![GeneratedTypeName {
                    name: name.to_string(),
                    category: category.to_string(),
                    type_id: id.to_string(),
                    value_id: String::new(),
                }],
                type_definition: Some(parse_row(pattern.as_bytes()).unwrap()),
                ..ObjectNames::default()
            });
        }
        let empty = brace_list![Brace::str("S"), Brace::str("")];
        for (id, nil) in [
            ("number", true),
            ("string", false),
            ("nested", true),
            ("characteristic", true),
            ("mixed", false),
            ("cycle", false),
            ("unknown", false),
            ("empty", false),
            ("missing", false),
        ] {
            let fill = fill_value_element(&empty, &reference(id), &names).unwrap();
            assert_eq!(Element::attr(&fill, "nil"), nil.then_some("true"), "{id}");
            assert_eq!(
                Element::attr(&fill, "type"),
                (!nil).then_some("xs:string"),
                "{id}"
            );
        }
        let unknown_set = "a9d5370f-d017-4419-b8db-85a1321dc0a6";
        names.insert_type(unknown_set, "DefinedType.NoDefinition");
        let fill = fill_value_element(&empty, &reference(unknown_set), &names).unwrap();
        assert_eq!(Element::attr(&fill, "type"), Some("xs:string"));
    }

    #[test]
    fn type_definitions_come_from_the_xml_snapshot_and_from_native_layout_rows() {
        use std::{path::PathBuf, sync::Arc};
        let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/1c-fixtures/coverage");
        // These paths deliberately do not exist: an in-memory descriptor
        // snapshot is authoritative and must not be reopened from disk.
        let root = PathBuf::from("in-memory-fill-value-snapshot");
        let fixtures = [
            ("DefinedType", "DefinedTypes/CovResourceType.xml", fixtures.join("8.3.27.2214/corpus-defined-type-resource/native/DefinedTypes/CovResourceType.xml")),
            ("ChartOfCharacteristicTypes", "ChartsOfCharacteristicTypes/CovTemplate.xml", fixtures.join("8.3.27.2342/chartsofcharacteristictypes/native/ChartsOfCharacteristicTypes/CovTemplate.xml")),
        ];
        // Synthetic whitespace and a nested alias exercise the XML spelling
        // accepted by type_pattern_items while retaining native row layouts.
        let mut descriptors = fixtures
            .iter()
            .map(|(kind, relative, native)| {
                let xml = std::fs::read_to_string(native).unwrap();
                let replacement = if *kind == "DefinedType" {
                    "<v8:Type> xs:string </v8:Type>"
                } else {
                    "<v8:TypeSet> cfg:DefinedType.CovNumeric </v8:TypeSet>"
                };
                (
                    *kind,
                    *relative,
                    Arc::new(
                        xml.replacen("<v8:Type>xs:string</v8:Type>", replacement, 1)
                            .into_bytes(),
                    ),
                )
            })
            .collect::<Vec<_>>();
        let numeric = r#"<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" xmlns:v8="http://v8.1c.ru/8.1/data/core" version="2.20"><DefinedType uuid="6bc7759a-d282-46e6-a1e6-4eb146988f25"><InternalInfo><xr:GeneratedType name="DefinedType.CovNumeric" category="DefinedType"><xr:TypeId>e8d28550-c509-4df0-b340-95bed3e0c564</xr:TypeId><xr:ValueId>63545e6e-24b1-473f-a6f9-4825af4560a5</xr:ValueId></xr:GeneratedType></InternalInfo><Properties><Name>CovNumeric</Name><Synonym/><Comment/><Type><v8:Type> xs:decimal </v8:Type></Type></Properties></DefinedType></MetaDataObject>"#;
        descriptors.push((
            "DefinedType",
            "DefinedTypes/CovNumeric.xml",
            Arc::new(numeric.as_bytes().to_vec()),
        ));
        let files = descriptors
            .iter()
            .map(|(_, relative, bytes)| (root.join(relative), bytes.clone()))
            .collect::<Vec<_>>();
        let source = DescriptorContext::with_files(&root, "2.20", &files).unwrap();
        assert_eq!(source.index.type_definitions.len(), 3);
        let from_xml = NameIndex::from_config_index(&source.index);
        let mut from_rows = NameIndex::default();
        for ((kind, _, _), (path, bytes)) in descriptors.iter().zip(&files) {
            let row = compile_descriptor(kind, path, bytes, &source).unwrap();
            from_rows.add(&object_names(kind, &parse_row(&row).unwrap()).unwrap());
        }
        // Also cover the parallel name-index builder's merge operation.
        let mut merged = NameIndex::default();
        merged.absorb(from_rows);
        let empty = brace_list![Brace::str("S"), Brace::str("")];
        for (name, nil) in [
            ("DefinedType.CovResourceType", false),
            ("Characteristic.CovTemplate", true),
            ("DefinedType.CovNumeric", true),
        ] {
            let expected = from_xml.type_definition(name).unwrap();
            assert!(!expected.is_empty());
            let from_row = merged.type_definition(name).unwrap();
            assert_eq!(
                crate::metadata_model::types::type_pattern(
                    Some(&el("Type").children(from_row)),
                    &source
                )
                .unwrap(),
                crate::metadata_model::types::type_pattern(
                    Some(&el("Type").children(expected)),
                    &source
                )
                .unwrap(),
                "{name}"
            );
            let pattern = reference(&source.index.generated_types[name].type_id);
            for index in [&from_xml, &merged] {
                let fill = fill_value_element(&empty, &pattern, index).unwrap();
                assert_eq!(Element::attr(&fill, "nil"), nil.then_some("true"), "{name}");
                assert_eq!(
                    Element::attr(&fill, "type"),
                    (!nil).then_some("xs:string"),
                    "{name}"
                );
            }
        }
    }

    #[test]
    fn fill_value_preserves_nonempty_and_nonstring_values() {
        let names = NameIndex::default();
        let pattern = parse_row(br##"{"Pattern",{"N",10,2,0}}"##).unwrap();
        for value in [
            r##"{"S","explicit"}"##,
            r##"{"N",0}"##,
            r##"{"B",0}"##,
            r##"{"U"}"##,
        ] {
            let value = parse_row(value.as_bytes()).unwrap();
            assert_eq!(
                crate::metadata_model::export::write_document(
                    &fill_value_element(&value, &pattern, &names).unwrap(),
                    "2.20"
                ),
                crate::metadata_model::export::write_document(
                    &value_element("FillValue", &value, &names).unwrap(),
                    "2.20"
                )
            );
        }
        let empty = brace_list![Brace::str("S"), Brace::str("")];
        let minimum = value_element("MinValue", &empty, &names).unwrap();
        assert_eq!(Element::attr(&minimum, "type"), Some("xs:string"));
    }
}
