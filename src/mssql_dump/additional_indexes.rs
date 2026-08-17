//! `Ext/AdditionalIndexes.xml` for the two owners that store one.
//!
//! The body is a serialized 1C value, not XML: routing it through the generic
//! inflate-and-write path published the braced text verbatim, which no XML
//! reader accepts. This module reads that value and projects it into the
//! platform's `extrnprops` document.
//!
//! Grammar, as the bytes spell it:
//!
//! ```text
//! {1,{<count>,<entry>,...}}
//! <entry>  = {"#",4b3b32e1-14f6-4ce8-b4c4-1bc85a74237e,
//!             {1,<id>,<field list>,<field list>,"<name>",{0,<table>},1,<uuid>}}
//! <fields> = {<count>,<field>,...}
//! <field>  = {"#",07c5e7a4-56de-47f1-9895-724a499e8a8c,{1,<slot>}}
//! <slot>   = {0,<uuid>}   a declared attribute, dimension or resource
//!          | {<code>}     a standard field, keyed by the table below
//! ```
//!
//! Every construct this module does not recognise is an error rather than a
//! guess: the two type uuids are required literally, the entry arity is
//! required, and an unlisted standard-field code refuses instead of inventing a
//! name.

use anyhow::{Result, anyhow};
use std::collections::BTreeMap;

use super::source_assets::AdditionalIndexesOwner;
use super::{
    escape_xml_text, parse_1c_quoted_string_with_len, parse_uuid_field, split_1c_braced_fields,
};

/// Type uuid of one additional-index record.
const INDEX_TYPE_UUID: &str = "4b3b32e1-14f6-4ce8-b4c4-1bc85a74237e";
/// Type uuid of one indexed-field reference inside a record.
const FIELD_TYPE_UUID: &str = "07c5e7a4-56de-47f1-9895-724a499e8a8c";

/// Standard fields addressed by a negative code instead of a uuid.
///
/// One global table, not one per owner family: every code observed in the
/// corpus names the same field wherever it appears, so a per-family table would
/// be two tables saying the same thing. UT 11.5.27.75 spells out all four --
/// `AccumulationRegister.ВыручкаИСебестоимостьПродаж` writes `Period`,
/// `Recorder` and `LineNumber` for `-2`, `-3` and `-4`, and
/// `Document.ЗаявкаНаЗакупку.Товары` writes `Ref` for `-5`.
const STANDARD_FIELDS: &[(&str, &str)] = &[
    ("-2", "Period"),
    ("-3", "Recorder"),
    ("-4", "LineNumber"),
    ("-5", "Ref"),
];

const HEADER: &str = "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<AdditionalIndexes xmlns=\"http://v8.1c.ru/8.3/xcf/extrnprops\" \
xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" \
xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" \
xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" \
xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"2.20\"";

#[derive(Debug)]
pub(super) struct AdditionalIndex {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) table: String,
    pub(super) indexed_fields: Vec<String>,
    pub(super) additional_fields: Vec<String>,
}

/// Resolves the table an index is declared on.
///
/// The owner's own uuid names the owner table directly; any other uuid must
/// resolve to a tabular section of that owner, and the data-table name drops
/// the `TabularSection` segment the metadata reference carries.
fn resolve_table(
    uuid: &str,
    owner: &AdditionalIndexesOwner,
    object_refs: &BTreeMap<String, String>,
) -> Result<String> {
    let root = format!("{}.{}", owner.kind, owner.name);
    if uuid == owner.uuid {
        return Ok(root);
    }
    let reference = object_refs
        .get(uuid)
        .ok_or_else(|| anyhow!("additional-index table uuid {uuid} is not a known object"))?;
    let section = reference
        .strip_prefix(&format!("{root}.TabularSection."))
        .ok_or_else(|| {
            anyhow!("additional-index table {reference} is not a tabular section of {root}")
        })?;
    if section.contains('.') {
        return Err(anyhow!(
            "additional-index table {reference} is nested below a tabular section"
        ));
    }
    Ok(format!("{root}.{section}"))
}

/// Field name for one `{"#",<field type>,{1,<slot>}}` reference.
fn resolve_field(field: &str, object_refs: &BTreeMap<String, String>) -> Result<String> {
    let fields = split_1c_braced_fields(field.trim(), 0)
        .ok_or_else(|| anyhow!("additional-index field is not a braced value"))?;
    let type_uuid = fields
        .get(1)
        .map(|value| value.trim())
        .ok_or_else(|| anyhow!("additional-index field carries no type uuid"))?;
    if type_uuid != FIELD_TYPE_UUID {
        return Err(anyhow!(
            "additional-index field carries unexpected type uuid {type_uuid}"
        ));
    }
    let payload = fields
        .get(2)
        .and_then(|value| split_1c_braced_fields(value.trim(), 0))
        .ok_or_else(|| anyhow!("additional-index field carries no payload"))?;
    let slot = payload
        .get(1)
        .and_then(|value| split_1c_braced_fields(value.trim(), 0))
        .ok_or_else(|| anyhow!("additional-index field payload carries no slot"))?;
    match slot.as_slice() {
        [kind, uuid] if kind.trim() == "0" => {
            let uuid = parse_uuid_field(uuid.trim())
                .ok_or_else(|| anyhow!("additional-index field slot holds no uuid"))?;
            let reference = object_refs.get(&uuid).ok_or_else(|| {
                anyhow!("additional-index field uuid {uuid} is not a known object")
            })?;
            let (_, leaf) = reference.rsplit_once('.').ok_or_else(|| {
                anyhow!("additional-index field reference {reference} has no leaf name")
            })?;
            Ok(leaf.to_string())
        }
        [code] => STANDARD_FIELDS
            .iter()
            .find(|(marker, _)| *marker == code.trim())
            .map(|(_, name)| (*name).to_string())
            .ok_or_else(|| {
                anyhow!(
                    "additional-index standard-field code {} is not in the evidenced table",
                    code.trim()
                )
            }),
        _ => Err(anyhow!("additional-index field slot has an unknown shape")),
    }
}

fn resolve_field_list(list: &str, object_refs: &BTreeMap<String, String>) -> Result<Vec<String>> {
    let fields = split_1c_braced_fields(list.trim(), 0)
        .ok_or_else(|| anyhow!("additional-index field list is not a braced value"))?;
    let count = fields
        .first()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .ok_or_else(|| anyhow!("additional-index field list carries no count"))?;
    if fields.len() != count + 1 {
        return Err(anyhow!(
            "additional-index field list declares {count} fields but holds {}",
            fields.len() - 1
        ));
    }
    fields
        .iter()
        .skip(1)
        .map(|field| resolve_field(field, object_refs))
        .collect()
}

pub(super) fn parse_additional_indexes(
    bytes: &[u8],
    owner: &AdditionalIndexesOwner,
    object_refs: &BTreeMap<String, String>,
) -> Result<Vec<AdditionalIndex>> {
    let text = std::str::from_utf8(bytes)
        .map_err(|error| anyhow!("additional-index body is not UTF-8: {error}"))?
        .trim_start_matches('\u{feff}');
    let outer = split_1c_braced_fields(text.trim(), 0)
        .ok_or_else(|| anyhow!("additional-index body is not a braced value"))?;
    let list = outer
        .get(1)
        .and_then(|value| split_1c_braced_fields(value.trim(), 0))
        .ok_or_else(|| anyhow!("additional-index body carries no record list"))?;
    let count = list
        .first()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .ok_or_else(|| anyhow!("additional-index record list carries no count"))?;
    if list.len() != count + 1 {
        return Err(anyhow!(
            "additional-index record list declares {count} records but holds {}",
            list.len() - 1
        ));
    }
    let mut indexes = Vec::with_capacity(count);
    for entry in list.iter().skip(1) {
        let entry_fields = split_1c_braced_fields(entry.trim(), 0)
            .ok_or_else(|| anyhow!("additional-index record is not a braced value"))?;
        let type_uuid = entry_fields
            .get(1)
            .map(|value| value.trim())
            .ok_or_else(|| anyhow!("additional-index record carries no type uuid"))?;
        if type_uuid != INDEX_TYPE_UUID {
            return Err(anyhow!(
                "additional-index record carries unexpected type uuid {type_uuid}"
            ));
        }
        let body = entry_fields
            .get(2)
            .and_then(|value| split_1c_braced_fields(value.trim(), 0))
            .ok_or_else(|| anyhow!("additional-index record carries no body"))?;
        if body.len() != 8 {
            return Err(anyhow!(
                "additional-index record body holds {} fields, not 8",
                body.len()
            ));
        }
        let id = parse_uuid_field(body[1].trim())
            .ok_or_else(|| anyhow!("additional-index record carries no id"))?;
        let (name, _) = parse_1c_quoted_string_with_len(body[4].trim())
            .ok_or_else(|| anyhow!("additional-index record carries no name"))?;
        let table_fields = split_1c_braced_fields(body[5].trim(), 0)
            .ok_or_else(|| anyhow!("additional-index record carries no table"))?;
        let table_uuid = table_fields
            .get(1)
            .and_then(|value| parse_uuid_field(value.trim()))
            .ok_or_else(|| anyhow!("additional-index table holds no uuid"))?;
        indexes.push(AdditionalIndex {
            id,
            name,
            table: resolve_table(&table_uuid, owner, object_refs)?,
            indexed_fields: resolve_field_list(body[2], object_refs)?,
            additional_fields: resolve_field_list(body[3], object_refs)?,
        });
    }
    Ok(indexes)
}

fn push_field_list(xml: &mut String, tag: &str, fields: &[String]) {
    if fields.is_empty() {
        xml.push_str(&format!("\t\t<{tag}/>\r\n"));
        return;
    }
    xml.push_str(&format!("\t\t<{tag}>\r\n"));
    for field in fields {
        xml.push_str(&format!(
            "\t\t\t<Field>{}</Field>\r\n",
            escape_xml_text(field)
        ));
    }
    xml.push_str(&format!("\t\t</{tag}>\r\n"));
}

pub(super) fn format_additional_indexes_xml(indexes: &[AdditionalIndex]) -> String {
    if indexes.is_empty() {
        return format!("{HEADER}/>");
    }
    let mut xml = format!("{HEADER}>\r\n");
    for index in indexes {
        xml.push_str(&format!(
            "\t<AdditionalIndex id=\"{}\">\r\n",
            escape_xml_text(&index.id)
        ));
        xml.push_str(&format!(
            "\t\t<Name>{}</Name>\r\n",
            escape_xml_text(&index.name)
        ));
        xml.push_str(&format!(
            "\t\t<Table>{}</Table>\r\n",
            escape_xml_text(&index.table)
        ));
        push_field_list(&mut xml, "IndexedFields", &index.indexed_fields);
        push_field_list(&mut xml, "AdditionalFields", &index.additional_fields);
        xml.push_str("\t</AdditionalIndex>\r\n");
    }
    xml.push_str("</AdditionalIndexes>");
    xml
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The body bytes of `AccumulationRegisters/ВыручкаИСебестоимостьПродаж`
    /// row `cc9cedf1-410d-414a-a05f-f167b8908470.4` in UT 11.5.27.75, verbatim.
    const REGISTER_BODY: &str = concat!(
        "\u{feff}{1,\r\n{1,\r\n{\"#\",4b3b32e1-14f6-4ce8-b4c4-1bc85a74237e,\r\n",
        "{1,5dd3e46c-d960-4a3d-8e62-5e799351d2df,\r\n{7,\r\n",
        "{\"#\",07c5e7a4-56de-47f1-9895-724a499e8a8c,\r\n{1,\r\n",
        "{0,76d9c27f-a066-45ee-8646-a029deff4e99}\r\n}\r\n},\r\n",
        "{\"#\",07c5e7a4-56de-47f1-9895-724a499e8a8c,\r\n{1,\r\n",
        "{0,7eadb439-cbf2-4e6a-9022-fc66b8de8d58}\r\n}\r\n},\r\n",
        "{\"#\",07c5e7a4-56de-47f1-9895-724a499e8a8c,\r\n{1,\r\n",
        "{0,cf1201fe-569c-4901-85d3-29da5aa53077}\r\n}\r\n},\r\n",
        "{\"#\",07c5e7a4-56de-47f1-9895-724a499e8a8c,\r\n{1,\r\n",
        "{0,a5e9fff7-057a-42a9-81e9-1335240572b7}\r\n}\r\n},\r\n",
        "{\"#\",07c5e7a4-56de-47f1-9895-724a499e8a8c,\r\n{1,\r\n{-2}\r\n}\r\n},\r\n",
        "{\"#\",07c5e7a4-56de-47f1-9895-724a499e8a8c,\r\n{1,\r\n{-3}\r\n}\r\n},\r\n",
        "{\"#\",07c5e7a4-56de-47f1-9895-724a499e8a8c,\r\n{1,\r\n{-4}\r\n}\r\n}\r\n},\r\n",
        "{0},\"ИндексПоЗаказамИАналитике\",\r\n",
        "{0,cc9cedf1-410d-414a-a05f-f167b8908470},1,",
        "00000000-0000-0000-0000-000000000000}\r\n}\r\n}\r\n}",
    );

    fn register_owner() -> AdditionalIndexesOwner {
        AdditionalIndexesOwner {
            kind: "AccumulationRegister".to_string(),
            name: "ВыручкаИСебестоимостьПродаж".to_string(),
            uuid: "cc9cedf1-410d-414a-a05f-f167b8908470".to_string(),
        }
    }

    fn register_refs() -> BTreeMap<String, String> {
        [
            ("76d9c27f-a066-45ee-8646-a029deff4e99", "ЗаказКлиента"),
            (
                "7eadb439-cbf2-4e6a-9022-fc66b8de8d58",
                "АналитикаУчетаНоменклатуры",
            ),
            (
                "cf1201fe-569c-4901-85d3-29da5aa53077",
                "АналитикаУчетаПоПартнерам",
            ),
            ("a5e9fff7-057a-42a9-81e9-1335240572b7", "Партия"),
        ]
        .into_iter()
        .map(|(uuid, name)| {
            (
                uuid.to_string(),
                format!("AccumulationRegister.ВыручкаИСебестоимостьПродаж.Dimension.{name}"),
            )
        })
        .collect()
    }

    /// Byte-exact against the platform's own
    /// `AccumulationRegisters/ВыручкаИСебестоимостьПродаж/Ext/AdditionalIndexes.xml`.
    #[test]
    fn projects_the_platform_register_body_byte_for_byte() {
        let indexes = parse_additional_indexes(
            REGISTER_BODY.as_bytes(),
            &register_owner(),
            &register_refs(),
        )
        .expect("the platform body parses");

        assert_eq!(
            format_additional_indexes_xml(&indexes),
            concat!(
                "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
                "<AdditionalIndexes xmlns=\"http://v8.1c.ru/8.3/xcf/extrnprops\"",
                " xmlns:v8=\"http://v8.1c.ru/8.1/data/core\"",
                " xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\"",
                " xmlns:xs=\"http://www.w3.org/2001/XMLSchema\"",
                " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"",
                " version=\"2.20\">\r\n",
                "\t<AdditionalIndex id=\"5dd3e46c-d960-4a3d-8e62-5e799351d2df\">\r\n",
                "\t\t<Name>ИндексПоЗаказамИАналитике</Name>\r\n",
                "\t\t<Table>AccumulationRegister.ВыручкаИСебестоимостьПродаж</Table>\r\n",
                "\t\t<IndexedFields>\r\n",
                "\t\t\t<Field>ЗаказКлиента</Field>\r\n",
                "\t\t\t<Field>АналитикаУчетаНоменклатуры</Field>\r\n",
                "\t\t\t<Field>АналитикаУчетаПоПартнерам</Field>\r\n",
                "\t\t\t<Field>Партия</Field>\r\n",
                "\t\t\t<Field>Period</Field>\r\n",
                "\t\t\t<Field>Recorder</Field>\r\n",
                "\t\t\t<Field>LineNumber</Field>\r\n",
                "\t\t</IndexedFields>\r\n",
                "\t\t<AdditionalFields/>\r\n",
                "\t</AdditionalIndex>\r\n",
                "</AdditionalIndexes>",
            )
        );
    }

    /// A standard-field code outside the evidenced table refuses instead of
    /// inventing a field name.
    #[test]
    fn refuses_an_unevidenced_standard_field_code() {
        let body = REGISTER_BODY.replace("{-2}", "{-9}");
        let error = parse_additional_indexes(body.as_bytes(), &register_owner(), &register_refs())
            .expect_err("an unlisted code refuses");
        assert!(error.to_string().contains("-9"), "{error}",);
    }

    /// A table uuid that is neither the owner nor one of its tabular sections
    /// refuses rather than naming some other object's table.
    #[test]
    fn refuses_a_table_outside_the_owner() {
        let error = parse_additional_indexes(
            REGISTER_BODY.as_bytes(),
            &AdditionalIndexesOwner {
                kind: "AccumulationRegister".to_string(),
                name: "Доставка".to_string(),
                uuid: "cafbf14f-ad21-43c5-a951-a852b28b0932".to_string(),
            },
            &register_refs(),
        )
        .expect_err("a foreign table refuses");
        assert!(
            error.to_string().contains("is not a known object"),
            "{error}",
        );
    }
}
