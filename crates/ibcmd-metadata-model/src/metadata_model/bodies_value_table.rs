//! The platform's value table and value tree serialization, as the body rows
//! of predefined data and register aggregates store it.
//!
//! A table is `{9,<columns>,<row data>,{0,0}}`, a tree `{1,<columns>,<row
//! data>}`; a value of either type inside a row is wrapped as
//! `{"#",<type uuid>,<table>}`.
//!
//! - columns: `{N,{<id>,"<name>",<pattern>,"<title>",<width>}...}`;
//! - row data: `{2,N,<value offset>,<column id>...,{1,<count>,<row>...},X,Y}`,
//!   the offset/id pairs in column order;
//! - row: `{2,<index>,<value count>,<value>...,0}`, or with children
//!   `{...,1,{1,<count>,<row>...}}`.
//!
//! Rows are numbered in document order from 0 (children right after their
//! parent). `Y` is the largest index, `-1` for no rows; `X` is `-1`, except
//! that a table with no rows writes its last column position there. Measured
//! over every table and tree of the predefined and aggregate rows of the four
//! corpora; the ones that disagree carry deleted or reordered rows, which
//! the source does not record.

use ibcmd_restructure::brace_list;

use super::brace::Brace;

/// `{"#",<this>,{9,...}}`: a value table value.
pub const VALUE_TABLE_TYPE: &str = "acf6192e-81ca-46ef-93a6-5a6968b78663";

/// One column: `{<id>,"<name>",<pattern>,"",0}`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Column {
    pub id: i64,
    pub name: String,
    /// `{"Pattern",...}`.
    pub pattern: Brace,
    /// Where a row stores this column's value.
    pub offset: i64,
}

impl Column {
    pub fn new(id: i64, name: impl Into<String>, pattern: Brace, offset: i64) -> Self {
        Self {
            id,
            name: name.into(),
            pattern,
            offset,
        }
    }

    fn to_brace(&self) -> Brace {
        brace_list![
            Brace::num(self.id),
            Brace::str(self.name.clone()),
            self.pattern.clone(),
            Brace::str(""),
            Brace::num(0),
        ]
    }
}

/// One row: its values in offset order and its child rows (trees only).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Row {
    pub values: Vec<Brace>,
    pub children: Vec<Row>,
}

impl Row {
    pub fn new(values: Vec<Brace>) -> Self {
        Self {
            values,
            children: Vec::new(),
        }
    }
}

fn columns_brace(columns: &[Column]) -> Brace {
    let mut items = vec![Brace::num(columns.len() as i64)];
    items.extend(columns.iter().map(Column::to_brace));
    Brace::List(items)
}

fn row_list(rows: &[Row], next: &mut i64) -> Brace {
    let mut items = vec![Brace::num(1), Brace::num(rows.len() as i64)];
    for row in rows {
        let index = *next;
        *next += 1;
        let mut members = vec![
            Brace::num(2),
            Brace::num(index),
            Brace::num(row.values.len() as i64),
        ];
        members.extend(row.values.iter().cloned());
        if row.children.is_empty() {
            members.push(Brace::num(0));
        } else {
            members.push(Brace::num(1));
            members.push(row_list(&row.children, next));
        }
        items.push(Brace::List(members));
    }
    Brace::List(items)
}

fn row_data(columns: &[Column], rows: &[Row]) -> Brace {
    let mut items = vec![Brace::num(2), Brace::num(columns.len() as i64)];
    for column in columns {
        items.push(Brace::num(column.offset));
        items.push(Brace::num(column.id));
    }
    let mut next = 0;
    items.push(row_list(rows, &mut next));
    let x = if rows.is_empty() {
        columns.len() as i64 - 1
    } else {
        -1
    };
    items.push(Brace::num(x));
    items.push(Brace::num(next - 1));
    Brace::List(items)
}

/// `{9,<columns>,<row data>,{0,0}}`.
pub fn value_table(columns: &[Column], rows: &[Row]) -> Brace {
    brace_list![
        Brace::num(9),
        columns_brace(columns),
        row_data(columns, rows),
        brace_list![Brace::num(0), Brace::num(0)],
    ]
}

/// `{"#",acf6192e-...,{9,...}}`: a value table held as a value.
pub fn value_table_value(columns: &[Column], rows: &[Row]) -> Brace {
    brace_list![
        Brace::str("#"),
        Brace::atom(VALUE_TABLE_TYPE),
        value_table(columns, rows),
    ]
}

/// `{1,<columns>,<row data>}`.
pub fn value_tree(columns: &[Column], rows: &[Row]) -> Brace {
    brace_list![
        Brace::num(1),
        columns_brace(columns),
        row_data(columns, rows)
    ]
}

/// `{"Pattern",<item>...}`.
pub fn pattern(items: Vec<Brace>) -> Brace {
    let mut out = vec![Brace::str("Pattern")];
    out.extend(items);
    Brace::List(out)
}

/// `{"S",""}`-like typed values.
pub fn string_value(text: &str) -> Brace {
    brace_list![Brace::str("S"), Brace::str(text)]
}

pub fn bool_value(value: bool) -> Brace {
    brace_list![Brace::str("B"), Brace::flag(value)]
}

pub fn number_value(text: &str) -> Brace {
    brace_list![Brace::str("N"), Brace::atom(text)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata_model::brace::serialize;

    #[test]
    fn numbers_rows_in_document_order_and_closes_with_the_last_index() {
        let columns = vec![
            Column::new(0, "", pattern(vec![brace_list![Brace::str("S")]]), 0),
            Column::new(1, "", pattern(vec![brace_list![Brace::str("B")]]), 1),
        ];
        let mut root = Row::new(vec![string_value("root")]);
        let mut child = Row::new(vec![string_value("a"), bool_value(true)]);
        child.children = vec![Row::new(vec![string_value("a1")])];
        root.children = vec![child, Row::new(vec![string_value("b")])];
        let text = serialize(&value_tree(&columns, &[root])).replace("\r\n", "");
        assert!(text.contains(r#"{2,0,1,{"S","root"},1,{1,2,{2,1,2,{"S","a"},{"B",1},1,{1,1,{2,2,1,{"S","a1"},0}}},{2,3,1,{"S","b"},0}}}},-1,3}"#), "{text}");
    }

    #[test]
    fn an_empty_table_writes_its_last_column_position() {
        let columns = vec![
            Column::new(0, "", pattern(Vec::new()), 0),
            Column::new(1, "x", pattern(Vec::new()), 1),
        ];
        let text = serialize(&value_table(&columns, &[])).replace("\r\n", "");
        assert!(
            text.ends_with(r#"{2,2,0,0,1,1,{1,0},1,-1},{0,0}}"#),
            "{text}"
        );
        let text = serialize(&value_table(&[], &[])).replace("\r\n", "");
        assert_eq!(text, "{9,{0},{2,0,{1,0},-1,-1},{0,0}}");
    }
}
