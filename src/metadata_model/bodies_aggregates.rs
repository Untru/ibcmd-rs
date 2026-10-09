//! Accumulation register aggregates, base-free: `Ext/Aggregates.xml` and the
//! register's own XML -> the `<uuid>.3` row.
//!
//! `{0,<value table>}`: column 0 the aggregate's id, 1 its use, 2 its
//! periodicity, then one boolean column per register dimension, named by
//! the text of the dimension's reference (`{"#",ae135932-...,{1,<uuid>}}`
//! serialized as a row would store it).
//!
//! A register without `Ext/Aggregates.xml` has no row: the export writes the
//! file for every row that holds an aggregate. The one ERP УХ row without one
//! (`ОперацииБюджетов`) holds no aggregate and columns of dimensions the
//! register no longer has.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};

use super::DescriptorContext;
use super::bodies_predefined::PREDEFINED_REF_TYPE;
use super::bodies_value_table::{Column, Row, bool_value, number_value, pattern, value_table};
use super::brace::{Brace, serialize};
use super::xml::{Element, parse_element_tree};
use crate::brace_list;

const USES: &[(&str, &str)] = &[("Auto", "0"), ("Always", "1")];

const PERIODICITIES: &[(&str, &str)] = &[
    ("Nonperiodical", "0"),
    ("Auto", "1"),
    ("Day", "2"),
    ("Month", "3"),
    ("Quarter", "4"),
    ("HalfYear", "5"),
    ("Year", "6"),
];

/// `Ext/Aggregates.xml` next to the register's XML.
pub fn aggregates_path(owner_xml: &Path) -> PathBuf {
    owner_xml
        .with_extension("")
        .join("Ext")
        .join("Aggregates.xml")
}

fn reference(uuid: &str) -> Brace {
    brace_list![
        Brace::str("#"),
        Brace::atom(PREDEFINED_REF_TYPE),
        brace_list![Brace::num(1), Brace::uuid(uuid)],
    ]
}

fn code<'a>(table: &'a [(&str, &str)], value: &str, what: &str) -> Result<&'a str> {
    let value = value.trim();
    table
        .iter()
        .find_map(|(name, code)| (*name == value).then_some(*code))
        .ok_or_else(|| anyhow!("no code for aggregate {what} {value:?}"))
}

/// The `<uuid>.3` row, `None` when the register has no `Ext/Aggregates.xml`.
pub fn aggregates_row(
    owner: &Element,
    owner_xml: &Path,
    context: &DescriptorContext,
) -> Result<Option<Brace>> {
    let path = aggregates_path(owner_xml);
    if !context.source.source_file_exists(&path)? {
        return Ok(None);
    }
    let bytes = context
        .source
        .read_source(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let document = parse_element_tree(&bytes)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    aggregates_tree(owner, Some(&document)).map(Some)
}

/// The `<uuid>.3` row of a register whose aggregates were all deleted: the
/// table's columns (the register's dimensions) and no row. A configuration
/// keeps such a row (its `ConfigDumpInfo.xml` lists it) while the export
/// writes no `Aggregates.xml` for it, so a load writes it from that list.
/// ERP УХ's one (`ОперацииБюджетов`) also names the columns and keeps the
/// dimensions of an older register; the source records neither.
pub fn emptied_aggregates_row(owner: &Element) -> Result<Brace> {
    aggregates_tree(owner, None)
}

fn aggregates_tree(owner: &Element, document: Option<&Element>) -> Result<Brace> {
    let register = owner
        .path(&["Properties", "Name"])
        .map(|name| name.text.clone())
        .unwrap_or_default();
    let dimensions = owner
        .child("ChildObjects")
        .into_iter()
        .flat_map(|children| children.children_named("Dimension"))
        .map(|dimension| {
            let uuid = dimension
                .attr("uuid")
                .ok_or_else(|| anyhow!("<Dimension> without uuid"))?
                .to_ascii_lowercase();
            let name = dimension
                .path(&["Properties", "Name"])
                .map(|name| name.text.clone())
                .unwrap_or_default();
            Ok((uuid, name))
        })
        .collect::<Result<Vec<_>>>()?;

    let mut columns = vec![
        Column::new(
            0,
            "",
            pattern(vec![brace_list![
                Brace::str("#"),
                Brace::atom(PREDEFINED_REF_TYPE)
            ]]),
            0,
        ),
        Column::new(1, "", pattern(vec![brace_list![Brace::str("N")]]), 1),
        Column::new(2, "", pattern(vec![brace_list![Brace::str("N")]]), 2),
    ];
    for (position, (uuid, _)) in dimensions.iter().enumerate() {
        columns.push(Column::new(
            3 + position as i64,
            serialize(&reference(uuid)),
            pattern(vec![brace_list![Brace::str("B")]]),
            3 + position as i64,
        ));
    }

    let mut rows = Vec::new();
    for aggregate in document
        .into_iter()
        .flat_map(|document| document.children_named("Aggregate"))
    {
        let id = aggregate
            .attr("id")
            .ok_or_else(|| anyhow!("<Aggregate> without id"))?;
        let mut values = vec![
            reference(id),
            number_value(code(
                USES,
                aggregate.child_text("Use").unwrap_or("Auto"),
                "Use",
            )?),
            number_value(code(
                PERIODICITIES,
                aggregate.child_text("Periodicity").unwrap_or("Auto"),
                "Periodicity",
            )?),
        ];
        let prefix = format!("AccumulationRegister.{register}.Dimension.");
        let included = aggregate
            .child("Dimensions")
            .into_iter()
            .flat_map(|list| list.children_named("Dimension"))
            .filter(|dimension| dimension.text.trim() == "true")
            .filter_map(|dimension| {
                let spelled = dimension.attr("ref")?;
                Some(
                    spelled
                        .strip_prefix(prefix.as_str())
                        .unwrap_or(spelled)
                        .to_string(),
                )
            })
            .collect::<Vec<_>>();
        for (uuid, name) in &dimensions {
            values.push(bool_value(
                included
                    .iter()
                    .any(|spelled| spelled == name || spelled == uuid),
            ));
        }
        rows.push(Row::new(values));
    }
    Ok(brace_list![Brace::num(0), value_table(&columns, &rows)])
}
