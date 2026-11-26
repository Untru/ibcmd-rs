//! `a07b62f0-1f01-484a-93d9-d42764cedac0.si`: the names of the objects and the tables that store them.
//!
//! The row is `{0,{<count>,<entry>...}}`, an entry
//! `"Catalog.Name","Справочник.Name",<object uuid>,<a>,<b>[,"Reference572",572,<Ref TypeId>]`: the
//! English and the Russian full name, the object, two numbers (`1,0` for the table of an object) and,
//! for a table, its SQL name (`_Reference572`), the number `DBNames` gave it and the object's `Ref`
//! type. Without the row the server does not start; with a stale one the client hangs
//! (`docs/apply/restructuring.md` 12.5).
//!
//! The entries are **not** sorted: they follow the metadata order, kind by kind, each kind in the
//! order of its root collection (so a new catalog stands where the root lists it, and the
//! "Удалить..." objects, which the root lists last, come last).

use anyhow::{Context, Result, bail};

use crate::metadata_model::brace::{Brace, parse_row, serialize_row};

/// The SQL name of the table a row of `kind` objects lives in and the Russian name of the kind.
/// Measured on the БСП corpus (`docs/apply/derived-caches.md`): one entry per object.
pub fn kind_names(kind: &str) -> Option<(&'static str, &'static str)> {
    Some(match kind {
        "Catalog" => ("Справочник", "Reference"),
        "Document" => ("Документ", "Document"),
        "Enum" => ("Перечисление", "Enum"),
        "ExchangePlan" => ("ПланОбмена", "Node"),
        "ChartOfCharacteristicTypes" => ("ПланВидовХарактеристик", "Chrc"),
        "ChartOfAccounts" => ("ПланСчетов", "Acc"),
        "ChartOfCalculationTypes" => ("ПланВидовРасчета", "CKinds"),
        "DocumentJournal" => ("ЖурналДокументов", "DocumentJournal"),
        _ => return None,
    })
}

/// A new object's table entry.
#[derive(Clone, Copy, Debug)]
pub struct NewTable<'a> {
    pub kind: &'a str,
    pub name: &'a str,
    pub object: &'a str,
    /// The number `DBNames` gave the table (`Reference11036` -> 11036).
    pub table_number: u64,
    /// The `Ref` generated type of the object.
    pub ref_type_id: &'a str,
    /// The objects the root lists before and after the new one (of the same kind).
    pub predecessor: Option<&'a str>,
    pub successor: Option<&'a str>,
}

/// The table part of an entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableRef {
    /// `Reference572`
    pub sql: String,
    pub number: u64,
    /// The `Ref` type of the object.
    pub type_id: String,
}

/// One entry of the row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// `Catalog.Name`
    pub name: String,
    /// `Справочник.Name`
    pub russian_name: String,
    pub object: String,
    pub a: i64,
    pub b: i64,
    pub table: Option<TableRef>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NamesTables {
    pub entries: Vec<Entry>,
    /// What follows the entries in the row: a second list (the tables of the change registration,
    /// `...Changes`), which a new object does not touch.
    pub rest: Vec<Brace>,
}

impl NamesTables {
    /// Parses the inflated row text (BOM optional).
    pub fn parse(text: &[u8]) -> Result<Self> {
        let tree = parse_row(text).context("a07b62f0 is not brace text")?;
        let outer = tree.as_list().context("a07b62f0 is not a list")?;
        let [zero, body, trailer @ ..] = outer else {
            bail!("a07b62f0 has {} elements, expected at least 2", outer.len());
        };
        if zero.as_atom() != Some("0") {
            bail!("a07b62f0 has version {zero:?}");
        }
        let body = body.as_list().context("a07b62f0 has no body")?;
        let (count, mut rest) = body.split_first().context("a07b62f0 body is empty")?;
        let count: usize = count
            .as_atom()
            .and_then(|count| count.parse().ok())
            .context("a07b62f0 has no entry count")?;
        let text_of = |node: &Brace| {
            node.as_str()
                .map(str::to_owned)
                .context("a07b62f0 expects a string")
        };
        let atom_of = |node: &Brace| {
            node.as_atom()
                .map(str::to_owned)
                .context("a07b62f0 expects a plain token")
        };
        let number_of = |node: &Brace| -> Result<i64> {
            node.as_atom()
                .and_then(|value| value.parse().ok())
                .context("a07b62f0 expects a number")
        };
        let mut entries = Vec::with_capacity(count);
        for _ in 0..count {
            let [name, russian, object, a, b, tail @ ..] = rest else {
                bail!("a07b62f0 ends inside an entry");
            };
            rest = tail;
            let table = if let [sql, number, type_id, tail @ ..] = rest
                && sql.as_str().is_some()
                && number.as_atom().is_some()
            {
                rest = tail;
                Some(TableRef {
                    sql: text_of(sql)?,
                    number: u64::try_from(number_of(number)?).context("a table number")?,
                    type_id: atom_of(type_id)?,
                })
            } else {
                None
            };
            entries.push(Entry {
                name: text_of(name)?,
                russian_name: text_of(russian)?,
                object: atom_of(object)?,
                a: number_of(a)?,
                b: number_of(b)?,
                table,
            });
        }
        if !rest.is_empty() {
            bail!("a07b62f0 has {} tokens after its entries", rest.len());
        }
        Ok(Self {
            entries,
            rest: trailer.to_vec(),
        })
    }

    pub fn render(&self) -> Vec<u8> {
        let mut body = vec![Brace::num(self.entries.len() as i64)];
        for entry in &self.entries {
            body.push(Brace::str(&entry.name));
            body.push(Brace::str(&entry.russian_name));
            body.push(Brace::atom(&entry.object));
            body.push(Brace::num(entry.a));
            body.push(Brace::num(entry.b));
            if let Some(table) = &entry.table {
                body.push(Brace::str(&table.sql));
                body.push(Brace::num(table.number as i64));
                body.push(Brace::atom(&table.type_id));
            }
        }
        let mut root = vec![Brace::atom("0"), Brace::List(body)];
        root.extend(self.rest.iter().cloned());
        serialize_row(&Brace::List(root))
    }

    /// The table entry of an object (`1,0`, with a table part).
    fn table_entry(&self, object: &str) -> Vec<usize> {
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.object == object && entry.table.is_some())
            .map(|(index, _)| index)
            .collect()
    }

    /// Adds the table entry of a new object: right after the entry of the object the root lists
    /// before it (`predecessor`), or right before the one it lists after (`successor`).
    pub fn add_object(&mut self, new: &NewTable<'_>) -> Result<()> {
        let NewTable {
            kind,
            name,
            object,
            table_number,
            ref_type_id,
            predecessor,
            successor,
        } = *new;
        let (russian_kind, prefix) =
            kind_names(kind).with_context(|| format!("no table names are known for {kind}"))?;
        if !self.entries.iter().all(|entry| entry.object != object) {
            bail!("a07b62f0 has an entry for {object} already");
        }
        let at = if let Some(predecessor) = predecessor {
            match self.table_entry(predecessor)[..] {
                [index] => index + 1,
                _ => bail!("a07b62f0 has no single table entry for the predecessor {predecessor}"),
            }
        } else if let Some(successor) = successor {
            match self.table_entry(successor)[..] {
                [index] => index,
                _ => bail!("a07b62f0 has no single table entry for the successor {successor}"),
            }
        } else {
            bail!("a07b62f0: the new object has neither a predecessor nor a successor");
        };
        self.entries.insert(
            at,
            Entry {
                name: format!("{kind}.{name}"),
                russian_name: format!("{russian_kind}.{name}"),
                object: object.to_ascii_lowercase(),
                a: 1,
                b: 0,
                table: Some(TableRef {
                    sql: format!("{prefix}{table_number}"),
                    number: table_number,
                    type_id: ref_type_id.to_ascii_lowercase(),
                }),
            },
        );
        Ok(())
    }
}
