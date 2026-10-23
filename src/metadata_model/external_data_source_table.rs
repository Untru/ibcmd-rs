//! The stored row of an `ExternalDataSourceTable` (object code 11).
//!
//! The row is a descriptor like every other metadata record, but its fields
//! live in their own collection (`c7075688-…`) and each field carries its type
//! as a `{"Pattern", …}` list. This module reads the row into the facts the
//! writer needs; it does not yet emit XML.

use anyhow::{Result, bail};

use super::brace::{Brace, parse_row, serialize};
use super::export::values::{header, localized_element};
use super::export::{Build, atom, el, leaf, list, xml_text};
use super::xml::Element;

/// Object code of a stored external-data-source table row.
pub(crate) const TABLE_RECORD_CODE: &str = "11";

/// The field collection id the platform stores.
pub(crate) const FIELD_COLLECTION: &str = "c7075688-01c1-41ba-bd1c-1023c8189079";

/// Type id of the `<KeyFields>` reference list.
pub(crate) const KEY_FIELD_REF_TYPE: &str = "157fa490-4ce9-11d4-9415-008048da11f9";

/// Type id of the `<InputByString>` reference list.
pub(crate) const INPUT_BY_STRING_REF_TYPE: &str = "60ea359f-3a6e-48bb-8e71-d2a457572918";

/// What a table record keeps behind its `NameInDataSource`, with the key-field
/// list, the `InputByString` list and the generated type ids masked, in the one
/// table measured against the platform (ServiceDesk 3.10.28, 8.5.1.1529). The
/// writer spells every other table property as that table's value, so a record
/// that stores anything else is refused instead of written with those values.
const MEASURED_TABLE_TAIL: &str = r#"00000000-0000-0000-0000-000000000000,0,1,0,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000,{0},{0},{0},{0},{0},{0,{0}},<InputByString>,{0},{0},<type>,<type>,00000000-0000-0000-0000-000000000000,{"U"},0,{1,2,0},0,0,{0,"","",{0,0,0}},0,0,00000000-0000-0000-0000-000000000000,1,{0,0},{1,{0,0}},0"#;

/// What a field record keeps around its header, type and `NameInDataSource`,
/// with the fill value masked; every field of the measured table stores it.
const MEASURED_FIELD_TAIL: &str = r#"0,{0},{0},0,"",0,{"U"},{"U"},0,00000000-0000-0000-0000-000000000000,2,0,{5006,0},{3,0,0},{0,0},0,{0},<fill>,0,0,0,0,0,0"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FieldRecord {
    pub uuid: String,
    pub name: String,
    /// `NameInDataSource`: the column name in the external source. It differs
    /// from the field name (`_c_id` is stored as `1c_id`).
    pub name_in_source: String,
    pub synonym: Brace,
    pub comment: String,
    /// `{"N",10,0,0}`, `{"S",255,1}`, … as stored.
    pub pattern: Vec<String>,
    /// `<FillValue>` as stored (`{"N",0}` -> `0`, `{"S",""}` -> empty).
    pub fill_value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TableRecord {
    pub uuid: String,
    pub name: String,
    /// `NameInDataSource`: the first string the record body stores.
    pub name_in_source: String,
    /// The header's synonym, as stored (`<Synonym>` content).
    pub synonym: Brace,
    pub comment: String,
    /// Generated type pairs in stored order (the XML order differs).
    pub generated: Vec<(String, String)>,
    pub fields: Vec<FieldRecord>,
    /// `<KeyFields>`: names of the key fields, resolved from the typed
    /// reference list (`157fa490-…`).
    pub key_fields: Vec<String>,
    /// `<InputByString>`: names in the typed reference list `60ea359f-…`.
    pub input_by_string: Vec<String>,
}

pub(crate) fn parse(text: &str) -> Result<TableRecord> {
    let row = parse_row(text.as_bytes())?;
    from_brace(&row)
}

pub(crate) fn from_brace(row: &Brace) -> Result<TableRecord> {
    let outer = list(row)?;
    if outer.len() < 3 || atom(&outer[0])? != "1" {
        bail!("external-data-source table: outer layout");
    }
    let body = list(&outer[1])?;
    if body.len() < 4 || atom(&body[0])? != TABLE_RECORD_CODE {
        bail!("external-data-source table: record code");
    }
    let identity = list(&body[1])?;
    if identity.len() != 2 || atom(&identity[0])? != "0" {
        bail!("external-data-source table: identity wrapper");
    }
    let head = header(&identity[1])?;
    // Every generated type is a pair of top-level identifiers. The stored
    // order is not the XML order: `RecordManager` sits after the table's own
    // scalar run, behind the field-map lists.
    let mut generated = Vec::new();
    let mut pending: Option<&str> = None;
    for node in body.iter().skip(2) {
        let Some(value) = node.as_atom() else {
            pending = None;
            continue;
        };
        if value.len() != 36 || value == super::brace::NIL_UUID {
            pending = None;
            continue;
        }
        match pending.take() {
            Some(first) => generated.push((first.to_string(), value.to_string())),
            None => pending = Some(value),
        }
    }
    let name_index = body
        .iter()
        .enumerate()
        .skip(2)
        .find_map(|(index, node)| node.as_str().map(|_| index))
        .ok_or_else(|| anyhow::anyhow!("external-data-source table: no NameInDataSource"))?;
    let name_in_source = body[name_index].as_str().unwrap_or_default().to_string();
    let collection = outer
        .iter()
        .skip(3)
        .find_map(|candidate| {
            let collection = list(candidate).ok()?;
            (collection.first().and_then(|first| first.as_atom()) == Some(FIELD_COLLECTION))
                .then_some(collection)
        })
        .ok_or_else(|| anyhow::anyhow!("external-data-source table: no field collection"))?;
    let fields = collection
        .iter()
        .skip(2)
        .map(field_record)
        .collect::<Result<Vec<_>>>()?;
    let field_names = fields
        .iter()
        .map(|field| (field.uuid.as_str(), field.name.as_str()))
        .collect::<std::collections::BTreeMap<_, _>>();
    let resolve = |ids: Vec<String>| {
        ids.into_iter()
            .map(|id| {
                field_names
                    .get(id.as_str())
                    .map(|name| (*name).to_string())
                    .ok_or_else(|| {
                        anyhow::anyhow!("external-data-source table: reference to no field {id}")
                    })
            })
            .collect::<Result<Vec<_>>>()
    };
    // `{0,<count>,…}` right behind the name: the key fields.
    let key_node = body
        .get(name_index + 1)
        .ok_or_else(|| anyhow::anyhow!("external-data-source table: no key-field list"))?;
    let key_ids = match list(key_node)? {
        [kind, count] if kind.as_atom() == Some("0") && count.as_atom() == Some("0") => Vec::new(),
        _ => typed_ref_list(key_node, KEY_FIELD_REF_TYPE)
            .ok_or_else(|| anyhow::anyhow!("external-data-source table: key-field list"))?,
    };
    let key_fields = resolve(key_ids)?;
    // `InputByString` is stored one level deeper, inside a `{1,…}` wrapper.
    let input_by_string =
        match find_typed_ref_list(&Brace::List(body.to_vec()), INPUT_BY_STRING_REF_TYPE) {
            Some(ids) => resolve(ids)?,
            None => Vec::new(),
        };
    let tail = masked_table_tail(&body[name_index + 2..], &generated);
    if tail != MEASURED_TABLE_TAIL {
        bail!(
            "external-data-source table {}: stores properties the writer has not measured \
             ({tail})",
            head.name
        );
    }
    Ok(TableRecord {
        uuid: head.uuid,
        name: head.name,
        name_in_source,
        synonym: head.synonym,
        comment: head.comment,
        generated,
        fields,
        key_fields,
        input_by_string,
    })
}

/// A `{0,<count>,{"#",<type>,{<kind>,<uuid>}}…}` reference list of `type`.
fn typed_ref_list(node: &Brace, type_id: &str) -> Option<Vec<String>> {
    let items = list(node).ok()?;
    if items.first().and_then(Brace::as_atom) != Some("0") {
        return None;
    }
    let mut identities = Vec::new();
    for entry in items.iter().skip(2) {
        let typed = list(entry).ok()?;
        if typed.first().and_then(Brace::as_str) != Some("#") {
            return None;
        }
        if typed.get(1).and_then(Brace::as_atom) != Some(type_id) {
            return None;
        }
        let inner = list(typed.get(2)?).ok()?;
        identities.push(inner.get(1).and_then(Brace::as_atom)?.to_string());
    }
    if identities.is_empty() {
        // An empty `{0,0,…}` list of the same shape is not the property.
        return None;
    }
    Some(identities)
}

fn find_typed_ref_list(node: &Brace, type_id: &str) -> Option<Vec<String>> {
    if let Some(identities) = typed_ref_list(node, type_id) {
        return Some(identities);
    }
    let items = list(node).ok()?;
    for child in items {
        if let Some(identities) = find_typed_ref_list(child, type_id) {
            return Some(identities);
        }
    }
    None
}

/// The serialized members, line breaks dropped, with the reference lists and
/// generated type ids replaced by placeholders.
fn masked_table_tail(members: &[Brace], generated: &[(String, String)]) -> String {
    let type_ids: std::collections::BTreeSet<&str> = generated
        .iter()
        .flat_map(|(type_id, value_id)| [type_id.as_str(), value_id.as_str()])
        .collect();
    members
        .iter()
        .map(|node| {
            if find_typed_ref_list(node, INPUT_BY_STRING_REF_TYPE).is_some() {
                "<InputByString>".to_string()
            } else if node.as_atom().is_some_and(|atom| type_ids.contains(atom)) {
                "<type>".to_string()
            } else {
                compact(node)
            }
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn compact(node: &Brace) -> String {
    serialize(node).replace("\r\n", "")
}

fn member<'a>(items: &'a [Brace], index: usize, what: &str) -> Result<&'a Brace> {
    items
        .get(index)
        .ok_or_else(|| anyhow::anyhow!("external-data-source table: field record has no {what}"))
}

fn field_record(entry: &Brace) -> Result<FieldRecord> {
    // `{{1,{27,{2,{3,<header>,{"Pattern",…}},…}},"<name>",0,0},0}`
    let outer = list(entry)?;
    let wrapped = list(member(outer, 0, "wrapper")?)?;
    let ordinal = list(member(wrapped, 1, "body")?)?;
    let holder = list(member(ordinal, 1, "header holder")?)?;
    let head = header(member(holder, 1, "header")?)?;
    let pattern = list(member(holder, 2, "type")?)?;
    if pattern.first().and_then(Brace::as_str) != Some("Pattern") || pattern.len() != 2 {
        bail!("external-data-source table: field type is not a single-type Pattern");
    }
    let kind = list(member(pattern, 1, "type pattern")?)?
        .iter()
        .map(|part| {
            part.as_atom()
                .or_else(|| part.as_str())
                .map(str::to_string)
                .ok_or_else(|| {
                    anyhow::anyhow!("external-data-source table: non-scalar type member")
                })
        })
        .collect::<Result<Vec<_>>>()?;
    let name_in_source = member(wrapped, 2, "NameInDataSource")?
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("external-data-source table: NameInDataSource"))?
        .to_string();
    // The fill value is the fourth member from the end of the body:
    // `{"N",0}`, `{"S",""}`.
    let fill_index = ordinal
        .len()
        .checked_sub(4)
        .ok_or_else(|| anyhow::anyhow!("external-data-source table: short field body"))?;
    let fill_value = match list(member(ordinal, fill_index, "fill value")?)? {
        [code, value] if code.as_str() == kind.first().map(String::as_str) => value
            .as_atom()
            .or_else(|| value.as_str())
            .ok_or_else(|| anyhow::anyhow!("external-data-source table: fill value"))?
            .to_string(),
        _ => bail!("external-data-source table: fill value of another type"),
    };
    let tail = ordinal
        .iter()
        .enumerate()
        .skip(2)
        .map(|(index, node)| {
            if index == fill_index {
                "<fill>".to_string()
            } else {
                compact(node)
            }
        })
        .chain(wrapped.iter().skip(3).map(compact))
        .chain(outer.iter().skip(1).map(compact))
        .collect::<Vec<_>>()
        .join(",");
    if tail != MEASURED_FIELD_TAIL {
        bail!(
            "external-data-source table: field {} stores properties the writer has not \
             measured ({tail})",
            head.name
        );
    }
    Ok(FieldRecord {
        uuid: head.uuid,
        name: head.name,
        name_in_source,
        synonym: head.synonym,
        comment: head.comment,
        pattern: kind,
        fill_value,
    })
}

/// The stored pair index of each generated type, in the order the platform
/// writes `<xr:GeneratedType>`: the stored order is different
/// (`RecordManager` is kept after the fields).
const XML_ORDER: &[(&str, usize)] = &[
    ("Manager", 0),
    ("Object", 4),
    ("Ref", 5),
    ("List", 6),
    ("Record", 2),
    ("RecordSet", 1),
    ("RecordKey", 3),
    ("RecordManager", 7),
];

/// The object element of the table file.
pub(crate) fn to_element(table: &TableRecord, owner: &str) -> Result<Element> {
    let full = format!("ExternalDataSource.{owner}.Table.{}", table.name);
    let mut internal = el("InternalInfo");
    for (category, stored) in XML_ORDER {
        let Some((type_id, value_id)) = table.generated.get(*stored) else {
            bail!("external-data-source table: no type pair {stored} for {category}");
        };
        internal = internal.child(
            el("xr:GeneratedType")
                .attr(
                    "name",
                    format!("ExternalDataSourceTable{category}.{owner}.{}", table.name),
                )
                .attr("category", *category)
                .child(leaf("xr:TypeId", type_id.clone()))
                .child(leaf("xr:ValueId", value_id.clone())),
        );
    }
    let fields = table
        .fields
        .iter()
        .map(field_element)
        .collect::<Result<Vec<_>>>()?;
    Ok(el("Table")
        .attr("uuid", &table.uuid)
        .child(internal)
        .child(table_properties(table, &full)?)
        .child(el("ChildObjects").children(fields)))
}

fn key_field_elements(full: &str, key_fields: &[String]) -> Vec<Element> {
    key_fields
        .iter()
        .map(|name| leaf("xr:Field", format!("{full}.Field.{name}")))
        .collect()
}

fn table_properties(table: &TableRecord, full: &str) -> Result<Element> {
    Ok(el("Properties")
        .child(leaf("Name", &table.name))
        .child(localized_element("Synonym", &table.synonym)?)
        .child(leaf("Comment", xml_text(&table.comment)))
        .child(leaf("TableType", "Table"))
        .child(leaf("NameInDataSource", &table.name_in_source))
        .child(leaf("ExpressionInDataSource", ""))
        .child(leaf("TableDataType", "ObjectData"))
        .child(el("KeyFields").children(key_field_elements(full, &table.key_fields)))
        .child(leaf("PresentationField", ""))
        .child(leaf("ParentField", ""))
        .child(el("UnfilledParentValue").attr("nil", "true"))
        .child(leaf("Characteristics", ""))
        .child(leaf("UseStandardCommands", "true"))
        .child(leaf("QuickChoice", "false"))
        .child(el("InputByString").children(key_field_elements(full, &table.input_by_string)))
        .child(leaf("CreateOnInput", "Auto"))
        .child(leaf("SearchStringModeOnInputByString", "Begin"))
        .child(leaf("ChoiceDataGetModeOnInputByString", "Directly"))
        .child(leaf("ChoiceHistoryOnInput", "Auto"))
        .child(leaf("DefaultObjectForm", ""))
        .child(leaf("DefaultRecordForm", ""))
        .child(leaf("DefaultListForm", ""))
        .child(leaf("DefaultChoiceForm", ""))
        .child(leaf("ObjectPresentation", ""))
        .child(leaf("ExtendedObjectPresentation", ""))
        .child(leaf("RecordPresentation", ""))
        .child(leaf("ExtendedRecordPresentation", ""))
        .child(leaf("ListPresentation", ""))
        .child(leaf("ExtendedListPresentation", ""))
        .child(leaf("Explanation", ""))
        .child(leaf("IncludeHelpInContents", "false"))
        .child(leaf("ReadOnly", "false"))
        .child(leaf("TransactionsIsolationLevel", "Auto"))
        .child(leaf("DataVersionField", ""))
        .child(leaf("EditType", "InDialog"))
        .child(leaf("BasedOn", ""))
        .child(leaf("DataLockFields", ""))
        .child(leaf("DataLockControlMode", "Automatic")))
}

fn field_element(field: &FieldRecord) -> Result<Element> {
    let type_element = field_type(&field.pattern)?;
    Ok(el("Field").attr("uuid", &field.uuid).child(
        el("Properties")
            .child(leaf("Name", &field.name))
            .child(localized_element("Synonym", &field.synonym)?)
            .child(leaf("Comment", xml_text(&field.comment)))
            .child(type_element)
            .child(leaf("PasswordMode", "false"))
            .child(leaf("Format", ""))
            .child(leaf("EditFormat", ""))
            .child(leaf("ToolTip", ""))
            .child(leaf("MarkNegatives", "false"))
            .child(leaf("Mask", ""))
            .child(leaf("MultiLine", "false"))
            .child(leaf("ExtendedEdit", "false"))
            .child(el("MinValue").attr("nil", "true"))
            .child(el("MaxValue").attr("nil", "true"))
            .child(leaf("FillFromFillingValue", "false"))
            .child(fill_value(field)?)
            .child(leaf("FillChecking", "DontCheck"))
            .child(leaf("ChoiceParameterLinks", ""))
            .child(leaf("ChoiceParameters", ""))
            .child(leaf("QuickChoice", "Auto"))
            .child(leaf("CreateOnInput", "Auto"))
            .child(leaf("ChoiceHistoryOnInput", "Auto"))
            .child(leaf("ChoiceForm", ""))
            .child(leaf("NameInDataSource", &field.name_in_source))
            .child(leaf("ReadOnly", "false"))
            .child(leaf("AllowNull", "false")),
    ))
}

/// The platform's field type element, by the qualifier rules of
/// `types_export::type_children`: `{"N",digits,fraction,sign}` (`0` Any, `1`
/// Nonnegative) and `{"S",length,allowed}` (`0` Fixed, `1` Variable). Any
/// other code or flag is refused, not defaulted.
fn field_type(pattern: &[String]) -> Result<Element> {
    let element = el("Type");
    Ok(match pattern {
        [code, digits, fraction, sign] if code == "N" => {
            let sign = match sign.as_str() {
                "0" => "Any",
                "1" => "Nonnegative",
                other => bail!("external-data-source table: unknown number sign {other:?}"),
            };
            element.child(leaf("v8:Type", "xs:decimal")).child(
                el("v8:NumberQualifiers")
                    .child(leaf("v8:Digits", digits.clone()))
                    .child(leaf("v8:FractionDigits", fraction.clone()))
                    .child(leaf("v8:AllowedSign", sign)),
            )
        }
        [code, length, allowed] if code == "S" => {
            let allowed = match allowed.as_str() {
                "0" => "Fixed",
                "1" => "Variable",
                other => bail!("external-data-source table: unknown string length rule {other:?}"),
            };
            element.child(leaf("v8:Type", "xs:string")).child(
                el("v8:StringQualifiers")
                    .child(leaf("v8:Length", length.clone()))
                    .child(leaf("v8:AllowedLength", allowed)),
            )
        }
        other => bail!("external-data-source table: unknown field type {other:?}"),
    })
}

fn fill_value(field: &FieldRecord) -> Result<Element> {
    let value = field.fill_value.as_str();
    Ok(match field.pattern.first().map(String::as_str) {
        Some("N") => leaf("FillValue", value).attr("type", "xs:decimal"),
        Some("S") if value.is_empty() => el("FillValue").attr("type", "xs:string"),
        Some("S") => leaf("FillValue", value).attr("type", "xs:string"),
        other => bail!("external-data-source table: no fill value for {other:?}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attribute<'a>(element: &'a Element, name: &str) -> Option<&'a str> {
        element
            .attrs
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
    /// The real row of
    /// `ExternalDataSource.ВнешнийИсточникДанных1.Table.oc_product_to_1c`
    /// (ServiceDesk 3.10.28, 8.5.1.1529), inflated from the `Config` table.
    const ROW: &str = include_str!("fixtures/external_data_source_table_row.txt");

    #[test]
    fn reads_the_table_identity_types_and_fields() {
        let table = parse(ROW).expect("a table row");
        assert_eq!(table.name, "oc_product_to_1c");
        assert_eq!(table.uuid, "78afc3f6-2d63-4482-a33e-fd11f668c2b6");
        assert_eq!(table.generated.len(), 8, "seven pairs plus RecordManager");
        assert_eq!(
            table.generated[0],
            (
                "59d4ebec-6e82-490e-acae-9d528e1e0245".to_string(),
                "5c8729a1-e458-4723-a52d-35c80789f296".to_string()
            ),
            "Manager comes first"
        );
        assert_eq!(
            table.generated[7],
            (
                "57f5e963-2504-4a54-8192-6dc60932dd59".to_string(),
                "537441e5-2b29-4613-978b-eb29a76ca9fc".to_string()
            ),
            "RecordManager is stored last"
        );
        assert_eq!(table.fields.len(), 2);
        assert_eq!(table.fields[0].name, "product_id");
        assert_eq!(table.fields[0].name_in_source, "product_id");
        assert_eq!(table.fields[0].pattern, vec!["N", "10", "0", "0"]);
        assert_eq!(table.fields[1].name, "_c_id");
        // The column name in the source differs from the field name.
        assert_eq!(table.fields[1].name_in_source, "1c_id");
        assert_eq!(table.fields[1].pattern, vec!["S", "255", "1"]);
        assert_eq!(table.comment, "");
        assert_eq!(table.key_fields, vec!["product_id".to_string()]);
        assert_eq!(table.input_by_string, vec!["product_id".to_string()]);
    }

    #[test]
    fn writes_the_table_structure_in_the_platforms_order() {
        let table = parse(ROW).expect("a table row");
        let element = to_element(&table, "ВнешнийИсточникДанных1").expect("an element");
        assert_eq!(element.name, "Table");
        assert_eq!(attribute(&element, "uuid"), Some(table.uuid.as_str()));
        let sections = element
            .children
            .iter()
            .map(|child| child.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(sections, ["InternalInfo", "Properties", "ChildObjects"]);

        let internal = &element.children[0];
        assert_eq!(internal.children.len(), 8);
        let categories = internal
            .children
            .iter()
            .map(|child| child.attr("category").unwrap_or_default())
            .collect::<Vec<_>>();
        assert_eq!(
            categories,
            [
                "Manager",
                "Object",
                "Ref",
                "List",
                "Record",
                "RecordSet",
                "RecordKey",
                "RecordManager"
            ]
        );
        assert_eq!(
            attribute(&internal.children[4], "name"),
            Some("ExternalDataSourceTableRecord.ВнешнийИсточникДанных1.oc_product_to_1c")
        );

        let properties = &element.children[1];
        assert_eq!(properties.children.len(), 38);
        let key_fields = properties
            .children
            .iter()
            .find(|child| child.name == "KeyFields")
            .expect("KeyFields");
        assert_eq!(
            key_fields.children[0].text,
            "ExternalDataSource.ВнешнийИсточникДанных1.Table.oc_product_to_1c.Field.product_id"
        );

        let fields = &element.children[2];
        assert_eq!(fields.children.len(), 2);
        let field = &fields.children[0];
        let field_properties = &field.children[0];
        let type_element = field_properties
            .children
            .iter()
            .find(|child| child.name == "Type")
            .expect("Type");
        assert_eq!(type_element.children[0].name, "Type");
        assert_eq!(type_element.children[0].prefix, "v8");
        assert_eq!(type_element.children[0].text, "xs:decimal");
        assert_eq!(type_element.children[1].children[0].text, "10");
        let fill = field_properties
            .children
            .iter()
            .find(|child| child.name == "FillValue")
            .expect("FillValue");
        assert_eq!(attribute(fill, "type"), Some("xs:decimal"));
        assert_eq!(fill.text, "0");
    }

    /// `ROW` with the record's own `NameInDataSource` (the body's string, not
    /// the header's) spelled `other`.
    fn row_with_name_in_source(other: &str) -> String {
        let quoted = "\"oc_product_to_1c\"";
        let header = ROW.find(quoted).expect("header name");
        let body = header
            + quoted.len()
            + ROW[header + quoted.len()..]
                .find(quoted)
                .expect("body name");
        format!("{}\"{other}\"{}", &ROW[..body], &ROW[body + quoted.len()..])
    }

    #[test]
    fn writes_the_stored_name_in_data_source() {
        let table = parse(&row_with_name_in_source("oc_product")).expect("a table row");
        assert_eq!(table.name, "oc_product_to_1c");
        assert_eq!(table.name_in_source, "oc_product");
        let element = to_element(&table, "ВнешнийИсточникДанных1").expect("element");
        let name_in_source = element.children[1]
            .children
            .iter()
            .find(|child| child.name == "NameInDataSource")
            .expect("NameInDataSource");
        assert_eq!(name_in_source.text, "oc_product");
    }

    #[test]
    fn refuses_a_table_property_it_has_not_measured() {
        let row = ROW.replacen("{1,2,0}", "{1,3,0}", 1);
        assert_ne!(row, ROW);
        let error = parse(&row).expect_err("an unmeasured table property");
        assert!(error.to_string().contains("not measured"), "{error}");
    }

    #[test]
    fn refuses_a_field_property_it_has_not_measured() {
        let row = ROW.replacen("{3,0,0}", "{3,1,0}", 1);
        assert_ne!(row, ROW);
        let error = parse(&row).expect_err("an unmeasured field property");
        assert!(error.to_string().contains("field product_id"), "{error}");
    }

    #[test]
    fn spells_qualifiers_by_the_type_description_rules() {
        let fixed = field_type(&["S".into(), "10".into(), "0".into()]).expect("fixed string");
        assert_eq!(fixed.children[1].children[1].text, "Fixed");
        let unsigned =
            field_type(&["N".into(), "5".into(), "0".into(), "1".into()]).expect("unsigned number");
        assert_eq!(unsigned.children[1].children[2].text, "Nonnegative");
        assert!(field_type(&["S".into(), "10".into(), "2".into()]).is_err());
        assert!(field_type(&["N".into(), "5".into(), "0".into()]).is_err());
    }
}
