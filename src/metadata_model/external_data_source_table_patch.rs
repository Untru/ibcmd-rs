//! A bounded TableDataType update over a verified external-table descriptor.

use anyhow::{Result, anyhow, bail, ensure};
use ibcmd_schema::external_data_source_children as schema;

use super::{Row, decode_row, owned_names};
use crate::metadata_model::brace::{Brace, parse_row, serialize_row};
use crate::metadata_model::export::{ExportContext, NameIndex, write_document};
use crate::metadata_model::objects::parts::Compat;
use crate::metadata_model::xml::MetadataXml;

/// Preserve the existing row, changing only the measured TableDataType slot.
/// Every other source coordinate must equal the verified row's own export.
pub(crate) fn patch_table_data_type_text(base: &str, xml: &[u8]) -> Result<String> {
    ibcmd_xml::XmlReader::from_slice(xml)?;
    let document = MetadataXml::parse(xml)?;
    ensure!(
        document.root.name == "MetaDataObject",
        "Table: expected MetaDataObject"
    );
    let version = document
        .version()
        .ok_or_else(|| anyhow!("Table: no XML version"))?;
    let compat = match version {
        "2.20" => Compat(8, 3, 27),
        "2.21" => Compat(8, 5, 1),
        _ => bail!("Table: unsupported XML version"),
    };
    let object = document.object()?;
    ensure!(
        object.name == schema::TABLE_KIND,
        "Table: unexpected metadata kind"
    );
    let properties = object
        .child("Properties")
        .ok_or_else(|| anyhow!("Table: no Properties"))?;
    let data_type = properties
        .child_text("TableDataType")
        .and_then(schema::table_data_type_code)
        .ok_or_else(|| anyhow!("Table: unsupported or missing TableDataType"))?;

    let manager = schema::TABLE_GENERATED_CATEGORIES
        .iter()
        .find(|category| category.category == "Manager")
        .ok_or_else(|| anyhow!("Table: no Manager category"))?;
    let manager_name = object
        .child("InternalInfo")
        .and_then(|info| {
            info.children.iter().find(|item| {
                item.name == "GeneratedType" && item.attr("category") == Some(manager.category)
            })
        })
        .and_then(|item| item.attr("name"))
        .ok_or_else(|| anyhow!("Table: no generated Manager name"))?;
    let stem = format!("{}.", manager.name_stem);
    let (owner, table) = manager_name
        .strip_prefix(&stem)
        .and_then(|name| name.split_once('.'))
        .ok_or_else(|| anyhow!("Table: generated Manager name has no typed owner"))?;
    ensure!(
        !owner.is_empty() && !table.is_empty() && !table.contains('.'),
        "Table: invalid generated Manager owner"
    );

    let mut row = parse_row(base.as_bytes())?;
    let parsed = Row::parse(&row, schema::TABLE_KIND)?;
    let head = parsed.header(schema::TABLE_KIND)?;
    ensure!(
        table == head.name,
        "Table: generated Manager name differs from native name"
    );
    let mut names = NameIndex::default();
    names.add(&owned_names(
        schema::TABLE_KIND,
        &row,
        &format!("ExternalDataSource.{owner}"),
    )?);
    let context = ExportContext {
        names,
        version: version.to_owned(),
        compat,
    };
    // Decode before updating: this validates the known 52-slot profile,
    // unmodeled defaults, generated coordinates and field-reference lists.
    decode_row(schema::TABLE_KIND, &row, &context)?;

    let payload = row
        .as_list_mut()
        .and_then(|root| root.get_mut(schema::ROW_PAYLOAD_SLOT))
        .and_then(Brace::as_list_mut)
        .ok_or_else(|| anyhow!("Table: missing native payload"))?;
    payload[schema::TABLE_DATA_TYPE_SLOT] = Brace::num(data_type);
    let candidate = write_document(&decode_row(schema::TABLE_KIND, &row, &context)?, version);
    ensure!(
        ibcmd_xml::equal_ignoring_structural_indentation(
            without_shell(xml),
            without_shell(candidate.as_bytes()),
        ),
        "Table: source changes unsupported metadata, fields or native profile coordinates"
    );
    Ok(String::from_utf8(serialize_row(&row))?)
}

fn without_shell(document: &[u8]) -> &[u8] {
    let mut body = document.strip_prefix(b"\xef\xbb\xbf").unwrap_or(document);
    if body.starts_with(b"<?xml")
        && let Some(end) = body.windows(2).position(|window| window == b"?>")
    {
        body = &body[end + 2..];
        while let Some((byte, rest)) = body.split_first()
            && matches!(byte, b' ' | b'\t' | b'\r' | b'\n')
        {
            body = rest;
        }
    }
    body
}
