//! `Params.DBNames`: the numbering of the infobase's tables and fields.
//!
//! The row is the raw deflate of brace text `{<max number>,{<count>,{<uuid>,"<kind>",<number>},...}}`
//! (UTF-8 BOM first). `<kind>` is the family of the SQL name (`Reference`, `Fld`, `VT`, `LineNo`, ...) and
//! `<number>` its suffix (`_Reference20`, `_Fld11034`). Numbers are never reused and entries never removed
//! (the platform keeps a deleted attribute's number), so a change only appends. A new number is the
//! maximum over the header of the main `DBNames` and the headers of every extension's `DBNames-Ext-*`, plus
//! one -- the counter is shared. `Params.DBNamesVersion-DBNames` is `{0,<guid>}`, a new random guid at every
//! change.

use std::io::{Read, Write};

use anyhow::{Context, Result, bail};
use flate2::Compression;
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;

use crate::brace::{Brace, parse_row, serialize_row};
use crate::brace_list;

/// One `{<uuid>,"<kind>",<number>}` entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NameEntry {
    pub uuid: String,
    pub kind: String,
    pub number: u64,
}

/// A parsed `DBNames` (main or an extension's).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DbNames {
    /// The header: the highest number handed out so far.
    pub max: u64,
    pub entries: Vec<NameEntry>,
}

impl DbNames {
    /// Parses the inflated text (BOM optional).
    pub fn parse(text: &[u8]) -> Result<Self> {
        let root = parse_row(text).context("DBNames is not brace text")?;
        let outer = root.as_list().context("DBNames is not a list")?;
        let [max, table] = outer else {
            bail!("DBNames has {} elements, expected 2", outer.len());
        };
        let max = number(max, "the header number")?;
        let table = table.as_list().context("DBNames has no entry table")?;
        let Some((count, rows)) = table.split_first() else {
            bail!("DBNames entry table is empty");
        };
        let count = number(count, "the entry count")?;
        if count != rows.len() as u64 {
            bail!(
                "DBNames header counts {count} entries, the table has {}",
                rows.len()
            );
        }
        let mut entries = Vec::with_capacity(rows.len());
        for row in rows {
            let fields = row.as_list().context("a DBNames entry is not a list")?;
            let [uuid, kind, number_node] = fields else {
                bail!("a DBNames entry has {} elements, expected 3", fields.len());
            };
            entries.push(NameEntry {
                uuid: uuid
                    .as_atom()
                    .context("a DBNames entry has no uuid")?
                    .to_owned(),
                kind: kind
                    .as_str()
                    .context("a DBNames entry has no kind")?
                    .to_owned(),
                number: number(number_node, "an entry number")?,
            });
        }
        Ok(Self { max, entries })
    }

    /// The `Params` row: raw deflate of the text.
    pub fn parse_row(compressed: &[u8]) -> Result<Self> {
        Self::parse(&inflate(compressed)?)
    }

    /// The inflated text with its BOM, as the platform lays it out.
    pub fn to_text(&self) -> Vec<u8> {
        let entries = self
            .entries
            .iter()
            .map(|entry| {
                brace_list![
                    Brace::atom(&entry.uuid),
                    Brace::str(&entry.kind),
                    Brace::atom(entry.number)
                ]
            })
            .collect::<Vec<_>>();
        let mut table = vec![Brace::atom(self.entries.len())];
        table.extend(entries);
        serialize_row(&brace_list![Brace::atom(self.max), Brace::List(table)])
    }

    /// The `Params` row to store.
    pub fn to_row(&self) -> Result<Vec<u8>> {
        deflate(&self.to_text())
    }

    /// The number of `(uuid, kind)`, if it has one.
    pub fn number_of(&self, uuid: &str, kind: &str) -> Option<u64> {
        self.entries
            .iter()
            .find(|entry| entry.kind == kind && entry.uuid.eq_ignore_ascii_case(uuid))
            .map(|entry| entry.number)
    }

    /// Every entry of an object (any kind).
    pub fn entries_of<'a>(&'a self, uuid: &'a str) -> impl Iterator<Item = &'a NameEntry> {
        self.entries
            .iter()
            .filter(move |entry| entry.uuid.eq_ignore_ascii_case(uuid))
    }

    /// The entry with a number of a kind.
    pub fn find_number(&self, kind: &str, number: u64) -> Option<&NameEntry> {
        self.entries
            .iter()
            .find(|entry| entry.kind == kind && entry.number == number)
    }

    /// Appends `{uuid,"kind",number}` and moves the header to `number`. The
    /// number must be above every number handed out.
    pub fn append(&mut self, uuid: &str, kind: &str, number: u64) -> Result<()> {
        if number <= self.max {
            bail!(
                "DBNames numbers only grow: {number} is not above the header {}",
                self.max
            );
        }
        self.entries.push(NameEntry {
            uuid: uuid.to_ascii_lowercase(),
            kind: kind.to_owned(),
            number,
        });
        self.max = number;
        Ok(())
    }
}

/// The next number to hand out: one above the highest header of the main
/// `DBNames` and of every extension's.
pub fn next_number<'a>(all: impl IntoIterator<Item = &'a DbNames>) -> u64 {
    all.into_iter().map(|names| names.max).max().unwrap_or(0) + 1
}

/// `{0,<guid>}` of `Params.DBNamesVersion-DBNames`.
pub fn version_row(guid: &str) -> Vec<u8> {
    serialize_row(&brace_list![
        Brace::num(0),
        Brace::atom(guid.to_ascii_lowercase())
    ])
}

/// The guid of a `DBNamesVersion*` row.
pub fn parse_version(row: &[u8]) -> Result<String> {
    let root = parse_row(row).context("DBNamesVersion is not brace text")?;
    let items = root.as_list().context("DBNamesVersion is not a list")?;
    match items {
        [zero, guid] if zero.as_atom() == Some("0") => Ok(guid
            .as_atom()
            .context("DBNamesVersion has no guid")?
            .to_owned()),
        _ => bail!("DBNamesVersion is not {{0,<guid>}}"),
    }
}

fn number(node: &Brace, what: &str) -> Result<u64> {
    node.as_atom()
        .with_context(|| format!("{what} is not a number"))?
        .parse::<u64>()
        .with_context(|| format!("{what} is not a number"))
}

/// Raw deflate, as `Params` and `Config` rows store their text.
pub fn inflate(compressed: &[u8]) -> Result<Vec<u8>> {
    let mut text = Vec::with_capacity(compressed.len() * 8);
    DeflateDecoder::new(compressed)
        .read_to_end(&mut text)
        .context("the row is not raw deflate")?;
    Ok(text)
}

pub fn deflate(text: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(text)?;
    Ok(encoder.finish()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\u{feff}{10824,\r\n{3,\r\n{6b1c0ab5-f6c2-4f38-8b9a-1f7a0b3a3d11,\"Reference\",20},\r\n{c60cdc87-198a-4f6e-8f17-76bcb1b1914b,\"Fld\",11034},\r\n{00000000-0000-0000-0000-000000000000,\"Fld\",10824}\r\n}\r\n}";

    #[test]
    fn a_names_text_round_trips_byte_for_byte() {
        let names = DbNames::parse(SAMPLE.as_bytes()).unwrap();
        assert_eq!(names.max, 10824);
        assert_eq!(names.entries.len(), 3);
        assert_eq!(names.to_text(), SAMPLE.as_bytes());
    }

    #[test]
    fn the_row_form_is_raw_deflate_of_the_text() {
        let names = DbNames::parse(SAMPLE.as_bytes()).unwrap();
        let row = names.to_row().unwrap();
        assert_eq!(inflate(&row).unwrap(), SAMPLE.as_bytes());
        assert_eq!(DbNames::parse_row(&row).unwrap(), names);
    }

    #[test]
    fn a_new_number_is_above_every_header_and_only_appends() {
        let mut main = DbNames::parse(SAMPLE.as_bytes()).unwrap();
        let extension = DbNames {
            max: 11033,
            entries: Vec::new(),
        };
        let next = next_number([&main, &extension]);
        assert_eq!(next, 11034);
        main.append("C60CDC87-198A-4F6E-8F17-76BCB1B1914B", "Fld", next)
            .unwrap();
        assert_eq!(main.max, 11034);
        assert_eq!(
            main.entries.last().unwrap().uuid,
            "c60cdc87-198a-4f6e-8f17-76bcb1b1914b"
        );
        assert!(
            main.append("00000000-0000-0000-0000-000000000000", "Fld", 5)
                .is_err()
        );
        assert_eq!(
            main.number_of("c60cdc87-198a-4f6e-8f17-76bcb1b1914b", "Fld"),
            Some(11034)
        );
        assert_eq!(DbNames::parse(&main.to_text()).unwrap(), main);
    }

    #[test]
    fn a_header_that_disagrees_with_the_table_is_refused() {
        let bad = SAMPLE.replace("{3,", "{4,");
        assert!(DbNames::parse(bad.as_bytes()).is_err());
    }

    #[test]
    fn the_version_row_is_zero_and_a_guid() {
        let row = version_row("8312E710-0000-4000-8000-00000000ABCD");
        assert_eq!(row.len(), 43);
        assert_eq!(
            parse_version(&row).unwrap(),
            "8312e710-0000-4000-8000-00000000abcd"
        );
        assert!(parse_version(b"{1,x}").is_err());
    }
}
