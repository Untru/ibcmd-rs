//! The root object: the infobase locale and the table descriptions.
//!
//! The root object is a blob object (see [`crate::blob`]). Its chain starting
//! at block 1 holds a 32-byte locale name, a `u32` table count and one `u32`
//! per table: the first blob block of that table's description. A description
//! is UTF-8 text in the platform's brace format:
//!
//! ```text
//! {"CONFIG",0,
//! {"Fields",
//! {"FILENAME","NVC",0,128,0,"CI"},
//! ...},
//! {"Indexes",{"BYNAMENO_CONFIG",0,{"FILENAME",128},{"PARTNO",0}}},
//! {"Recordlock","0"},
//! {"Files",6,7,8}
//! }
//! ```
//!
//! `Files` names the header pages of the record, blob and index objects
//! (0 = absent).

use std::collections::BTreeMap;

use crate::blob;
use crate::error::{Result, corrupt};
use crate::object::Object;
use crate::pager::Pager;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Value {
    List(Vec<Value>),
    Text(String),
    Atom(String),
}

impl Value {
    fn as_text(&self) -> Option<&str> {
        match self {
            Value::Text(t) | Value::Atom(t) => Some(t),
            Value::List(_) => None,
        }
    }

    fn as_list(&self) -> Option<&[Value]> {
        match self {
            Value::List(items) => Some(items),
            _ => None,
        }
    }
}

fn parse_braces_checked(text: &str, check: &dyn Fn() -> Result<()>) -> Result<Value> {
    check()?;
    let chars: Vec<char> = text
        .chars()
        .map(|ch| {
            check()?;
            Ok(ch)
        })
        .collect::<Result<_>>()?;
    let mut pos = 0usize;
    let value = parse_value(&chars, &mut pos, 0, check)?;
    Ok(value)
}

fn skip(chars: &[char], pos: &mut usize, check: &dyn Fn() -> Result<()>) -> Result<()> {
    while *pos < chars.len() && (chars[*pos].is_whitespace() || chars[*pos] == ',') {
        check()?;
        *pos += 1;
    }
    Ok(())
}

fn parse_value(
    chars: &[char],
    pos: &mut usize,
    depth: usize,
    check: &dyn Fn() -> Result<()>,
) -> Result<Value> {
    check()?;
    if depth > 32 {
        return Err(corrupt("Описание таблицы: слишком глубокая вложенность"));
    }
    skip(chars, pos, check)?;
    let Some(&c) = chars.get(*pos) else {
        return Err(corrupt("Описание таблицы обрывается"));
    };
    match c {
        '{' => {
            *pos += 1;
            let mut items = Vec::new();
            loop {
                skip(chars, pos, check)?;
                match chars.get(*pos) {
                    Some('}') => {
                        *pos += 1;
                        return Ok(Value::List(items));
                    }
                    Some(_) => items.push(parse_value(chars, pos, depth + 1, check)?),
                    None => return Err(corrupt("Описание таблицы: нет закрывающей скобки")),
                }
            }
        }
        '"' => {
            *pos += 1;
            let mut out = String::new();
            loop {
                check()?;
                match chars.get(*pos) {
                    Some('"') if chars.get(*pos + 1) == Some(&'"') => {
                        out.push('"');
                        *pos += 2;
                    }
                    Some('"') => {
                        *pos += 1;
                        return Ok(Value::Text(out));
                    }
                    Some(&ch) => {
                        out.push(ch);
                        *pos += 1;
                    }
                    None => return Err(corrupt("Описание таблицы: незакрытая строка")),
                }
            }
        }
        _ => {
            let start = *pos;
            while *pos < chars.len()
                && !matches!(chars[*pos], ',' | '}' | '{')
                && !chars[*pos].is_whitespace()
            {
                check()?;
                *pos += 1;
            }
            if start == *pos {
                return Err(corrupt("Описание таблицы: пустое значение"));
            }
            Ok(Value::Atom(chars[start..*pos].iter().collect()))
        }
    }
}

/// One column of a table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub name: String,
    pub kind: FieldKind,
    pub nullable: bool,
    pub length: u32,
    pub precision: u32,
    pub case_insensitive: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    /// `B`: fixed binary of `length` bytes.
    Binary,
    /// `L`: boolean, one byte.
    Bool,
    /// `N`: packed decimal of `length` digits and a sign nibble.
    Numeric,
    /// `NC`: fixed UTF-16 string of `length` characters.
    FixedString,
    /// `NVC`: `u16` character count and `length` UTF-16 characters.
    VarString,
    /// `RV`: row version, 16 bytes.
    RowVersion,
    /// `NT`: UTF-16 text in the blob object, `u32` block and `u32` length.
    Text,
    /// `I`: binary in the blob object, `u32` block and `u32` length.
    Image,
    /// `DT`: packed `yyyyMMddhhmmss`, 7 bytes.
    DateTime,
}

impl Field {
    /// Bytes the field takes in a record, including the null flag.
    pub fn size(&self) -> usize {
        let data = match self.kind {
            FieldKind::Binary => self.length as usize,
            FieldKind::Bool => 1,
            FieldKind::Numeric => (self.length as usize + 2) / 2,
            FieldKind::FixedString => self.length as usize * 2,
            FieldKind::VarString => self.length as usize * 2 + 2,
            FieldKind::RowVersion => 16,
            FieldKind::Text | FieldKind::Image => 8,
            FieldKind::DateTime => 7,
        };
        data + usize::from(self.nullable)
    }
}

/// One table of the infobase.
#[derive(Debug, Clone)]
pub struct Table {
    pub name: String,
    pub fields: Vec<Field>,
    pub index_names: Vec<String>,
    /// Header pages of the record, blob and index objects; 0 = absent.
    pub records_page: u32,
    pub blob_page: u32,
    pub index_page: u32,
}

impl Table {
    /// Bytes per record: the free flag plus the fields, at least 5 (a free
    /// record keeps the flag and the next free record number).
    pub fn record_size(&self) -> usize {
        (1 + self.fields.iter().map(Field::size).sum::<usize>()).max(5)
    }

    #[cfg(test)]
    pub(crate) fn field_offset(&self, name: &str) -> Option<(usize, &Field)> {
        let mut offset = 1usize;
        for field in &self.fields {
            if field.name == name {
                return Some((offset, field));
            }
            offset += field.size();
        }
        None
    }
}

#[cfg(test)]
fn parse_table(text: &str) -> Result<Table> {
    parse_table_checked(text, &|| Ok(()))
}
fn parse_table_checked(text: &str, check: &dyn Fn() -> Result<()>) -> Result<Table> {
    let value = parse_braces_checked(text, check)?;
    let items = value
        .as_list()
        .ok_or_else(|| corrupt("Описание таблицы не список"))?;
    let name = items
        .first()
        .and_then(Value::as_text)
        .ok_or_else(|| corrupt("Описание таблицы без имени"))?
        .to_string();
    let mut fields = Vec::new();
    let mut index_names = Vec::new();
    let mut files: Option<[u32; 3]> = None;
    for part in items.iter().skip(2) {
        check()?;
        let Some(list) = part.as_list() else { continue };
        let Some(tag) = list.first().and_then(Value::as_text) else {
            continue;
        };
        match tag {
            "Fields" => {
                for f in &list[1..] {
                    fields.push(parse_field(&name, f)?);
                }
            }
            "Indexes" => {
                for i in &list[1..] {
                    if let Some(index_name) =
                        i.as_list().and_then(|l| l.first()).and_then(Value::as_text)
                    {
                        index_names.push(index_name.to_string());
                    }
                }
            }
            "Files" => {
                let numbers: Vec<u32> = list[1..]
                    .iter()
                    .map(|v| v.as_text().and_then(|t| t.parse().ok()))
                    .collect::<Option<_>>()
                    .ok_or_else(|| corrupt(format!("Таблица {name}: неверный раздел Files")))?;
                if numbers.len() != 3 {
                    return Err(corrupt(format!(
                        "Таблица {name}: в разделе Files не три страницы"
                    )));
                }
                files = Some([numbers[0], numbers[1], numbers[2]]);
            }
            _ => {}
        }
    }
    let [records_page, blob_page, index_page] =
        files.ok_or_else(|| corrupt(format!("Таблица {name}: нет раздела Files")))?;
    Ok(Table {
        name,
        fields,
        index_names,
        records_page,
        blob_page,
        index_page,
    })
}

fn parse_field(table: &str, value: &Value) -> Result<Field> {
    let bad = || corrupt(format!("Таблица {table}: неверное описание поля"));
    let list = value.as_list().ok_or_else(bad)?;
    if list.len() < 6 {
        return Err(bad());
    }
    let text = |i: usize| list[i].as_text().ok_or_else(bad);
    let number = |i: usize| -> Result<u32> { text(i)?.parse().map_err(|_| bad()) };
    let kind = match text(1)? {
        "B" => FieldKind::Binary,
        "L" => FieldKind::Bool,
        "N" => FieldKind::Numeric,
        "NC" => FieldKind::FixedString,
        "NVC" => FieldKind::VarString,
        "RV" => FieldKind::RowVersion,
        "NT" => FieldKind::Text,
        "I" => FieldKind::Image,
        "DT" => FieldKind::DateTime,
        other => {
            return Err(corrupt(format!(
                "Таблица {table}: неизвестный тип поля {other}"
            )));
        }
    };
    Ok(Field {
        name: text(0)?.to_string(),
        kind,
        nullable: number(2)? != 0,
        length: number(3)?,
        precision: number(4)?,
        case_insensitive: text(5)? == "CI",
    })
}

fn read_metadata_chain(data: &[u8], start: u32, pager: &mut Pager<'_>) -> Result<Vec<u8>> {
    let mut copied = 0u64;
    let bounded = pager.read_is_bounded();
    blob::read_chain_accounted(data, start, None, &mut |bytes| {
        copied = copied
            .checked_add(bytes)
            .ok_or_else(|| corrupt("metadata size overflow"))?;
        if bounded && copied > 1024 * 1024 {
            return Err(corrupt("table metadata exceeds 1 MiB"));
        }
        pager.materialize_object(bytes)
    })
}

/// Reads the locale and every table description from the root object.
pub(crate) fn read_tables(pager: &mut Pager<'_>) -> Result<(String, BTreeMap<String, Table>)> {
    let root = Object::open(pager, 2)?;
    let data = root.read_all(pager)?;
    let head = read_metadata_chain(&data, 1, pager)?;
    if head.len() < 36 {
        return Err(corrupt("Корневой объект короче заголовка"));
    }
    let locale = String::from_utf8_lossy(&head[..32])
        .trim_end_matches('\0')
        .to_string();
    let count = u32::from_le_bytes(head[32..36].try_into().unwrap()) as usize;
    let end = count
        .checked_mul(4)
        .and_then(|bytes| bytes.checked_add(36))
        .ok_or_else(|| corrupt("table count overflow"))?;
    if head.len() < end {
        return Err(corrupt("Корневой объект: список таблиц обрывается"));
    }
    let mut tables = BTreeMap::new();
    for i in 0..count {
        pager.materialize_record(0)?;
        let block = u32::from_le_bytes(head[36 + i * 4..40 + i * 4].try_into().unwrap());
        let text_bytes = read_metadata_chain(&data, block, pager)?;
        let text =
            String::from_utf8(text_bytes).map_err(|_| corrupt("Описание таблицы не в UTF-8"))?;
        let table = parse_table_checked(&text, &|| pager.check_read())?;
        tables.insert(table.name.clone(), table);
    }
    Ok((locale, tables))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONFIG: &str = "{\"CONFIG\",0,\n{\"Fields\",\n{\"FILENAME\",\"NVC\",0,128,0,\"CI\"},\n{\"CREATION\",\"DT\",0,0,0,\"CS\"},\n{\"MODIFIED\",\"DT\",0,0,0,\"CS\"},\n{\"ATTRIBUTES\",\"N\",0,5,0,\"CS\"},\n{\"DATASIZE\",\"N\",0,20,0,\"CS\"},\n{\"BINARYDATA\",\"I\",0,0,0,\"CS\"},\n{\"PARTNO\",\"N\",0,10,0,\"CS\"}\n},\n{\"Indexes\",\n{\"BYNAMENO_CONFIG\",0,\n{\"FILENAME\",128},\n{\"PARTNO\",0}\n}\n},\n{\"Recordlock\",\"0\"},\n{\"Files\",6,7,8}\n}";

    #[test]
    fn config_description_gives_the_native_record_size() {
        let table = parse_table(CONFIG).unwrap();
        assert_eq!(table.name, "CONFIG");
        assert_eq!(table.fields.len(), 7);
        assert_eq!(table.record_size(), 301);
        assert_eq!(
            (table.records_page, table.blob_page, table.index_page),
            (6, 7, 8)
        );
        assert_eq!(table.index_names, ["BYNAMENO_CONFIG"]);
        assert_eq!(
            table.field_offset("PARTNO").unwrap().0,
            1 + 258 + 7 + 7 + 3 + 11 + 8
        );
    }

    #[test]
    fn malformed_descriptions_fail_closed() {
        for text in [
            "",
            "{",
            "{\"T\",0,{\"Fields\",{\"A\",\"ZZ\",0,1,0,\"CS\"}},{\"Files\",1,2,3}}",
            "{\"T\",0}",
            "{\"T\",0,{\"Files\",1,2}}",
        ] {
            assert!(parse_table(text).is_err(), "{text}");
        }
    }
}

#[cfg(test)]
mod budget_cancellation_tests {
    use super::*;
    #[test]
    fn metadata_parser_observes_cancellation() {
        let calls = std::cell::Cell::new(0);
        let error = parse_braces_checked(&format!("{{\"{}\"}}", "x".repeat(10000)), &|| {
            calls.set(calls.get() + 1);
            if calls.get() >= 100 {
                Err(crate::Error::Refused("cancelled".into()))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert!(error.to_string().contains("cancelled"));
        assert_eq!(calls.get(), 100);
    }
}
