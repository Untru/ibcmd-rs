//! Values that cross the client boundary, whatever the DBMS: query
//! parameters in, owned typed column values out.

use anyhow::{Result, anyhow, bail};

/// One parameter of a parameterized statement, in order (`@P1`, `@P2`, ...
/// on SQL Server, `$1`, `$2`, ... on PostgreSQL).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SqlParam<'a> {
    Text(&'a str),
    U8(u8),
    I32(i32),
    I64(i64),
    Binary(&'a [u8]),
}

impl SqlParam<'_> {
    /// Bytes the parameter adds to a request, for sizing batches.
    pub fn wire_bytes(&self) -> usize {
        match self {
            Self::Text(value) => value.len() * 2,
            Self::U8(_) => 1,
            Self::I32(_) => 4,
            Self::I64(_) => 8,
            Self::Binary(value) => value.len(),
        }
    }
}

/// One column value of a result row.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum SqlValue {
    #[default]
    Null,
    /// Booleans and integers of every width (and an exact numeric of scale 0
    /// that fits).
    Int(i64),
    Float(f64),
    /// Character data of every kind, and XML.
    Text(String),
    /// Binary data of every kind.
    Binary(Vec<u8>),
    /// Any other type (dates, GUIDs, fractional decimals) as the client
    /// renders it; nothing in ibcmd-rs reads these.
    Other(String),
}

impl SqlValue {
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Int(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Text(value) => Some(value),
            _ => None,
        }
    }

    /// The value as a command-line tool prints it (`NULL`, decimal numbers,
    /// text as is, `0x`-prefixed upper-case hex for binary): what the parsers
    /// of line-oriented queries (`CONCAT(N'X|', ...)`) read.
    pub fn to_text(&self) -> String {
        match self {
            Self::Null => "NULL".to_owned(),
            Self::Int(value) => value.to_string(),
            Self::Float(value) => value.to_string(),
            Self::Text(value) | Self::Other(value) => value.clone(),
            Self::Binary(bytes) => {
                const HEX: &[u8; 16] = b"0123456789ABCDEF";
                let mut text = String::with_capacity(2 + bytes.len() * 2);
                text.push_str("0x");
                for byte in bytes {
                    text.push(char::from(HEX[usize::from(byte >> 4)]));
                    text.push(char::from(HEX[usize::from(byte & 0x0f)]));
                }
                text
            }
        }
    }
}

/// One result row.
#[derive(Debug, Clone, PartialEq)]
pub struct SqlRow {
    /// Which result set of the request the row belongs to, from 0.
    pub result_set: usize,
    pub values: Vec<SqlValue>,
}

impl SqlRow {
    pub fn value(&self, index: usize) -> Result<&SqlValue> {
        self.values
            .get(index)
            .ok_or_else(|| anyhow!("result row has no column {index}"))
    }

    pub fn i64(&self, index: usize) -> Result<i64> {
        match self.value(index)? {
            SqlValue::Int(value) => Ok(*value),
            other => bail!("column {index} is not an integer: {other:?}"),
        }
    }

    pub fn text(&self, index: usize) -> Result<&str> {
        match self.value(index)? {
            SqlValue::Text(value) => Ok(value),
            other => bail!("column {index} is not text: {other:?}"),
        }
    }

    pub fn binary(&self, index: usize) -> Result<&[u8]> {
        match self.value(index)? {
            SqlValue::Binary(value) => Ok(value),
            other => bail!("column {index} is not binary: {other:?}"),
        }
    }

    /// Takes a text column out of the row, leaving `Null`.
    pub fn take_text(&mut self, index: usize) -> Result<String> {
        match self.take(index)? {
            SqlValue::Text(value) => Ok(value),
            other => bail!("column {index} is not text: {other:?}"),
        }
    }

    /// Takes a binary column out of the row, leaving `Null`.
    pub fn take_binary(&mut self, index: usize) -> Result<Vec<u8>> {
        match self.take(index)? {
            SqlValue::Binary(value) => Ok(value),
            other => bail!("column {index} is not binary: {other:?}"),
        }
    }

    fn take(&mut self, index: usize) -> Result<SqlValue> {
        self.values
            .get_mut(index)
            .map(std::mem::take)
            .ok_or_else(|| anyhow!("result row has no column {index}"))
    }
}

#[cfg(test)]
mod tests {
    use super::{SqlRow, SqlValue};

    #[test]
    fn values_render_like_a_command_line_tool() {
        assert_eq!(
            SqlValue::Text("Конфигурация".to_owned()).to_text(),
            "Конфигурация"
        );
        assert_eq!(SqlValue::Binary(vec![0x0a, 0xff]).to_text(), "0x0AFF");
        assert_eq!(SqlValue::Int(-42).to_text(), "-42");
        assert_eq!(SqlValue::Null.to_text(), "NULL");
    }

    #[test]
    fn typed_accessors_refuse_other_types() {
        let mut row = SqlRow {
            result_set: 0,
            values: vec![
                SqlValue::Text("root".to_owned()),
                SqlValue::Int(3),
                SqlValue::Binary(vec![1, 2]),
            ],
        };
        assert_eq!(row.text(0).unwrap(), "root");
        assert_eq!(row.i64(1).unwrap(), 3);
        assert_eq!(row.binary(2).unwrap(), &[1, 2]);
        assert!(row.i64(0).is_err());
        assert!(row.text(1).is_err());
        assert!(row.value(3).is_err());
        assert_eq!(row.take_binary(2).unwrap(), vec![1, 2]);
        assert!(row.value(2).unwrap().is_null());
        assert_eq!(row.take_text(0).unwrap(), "root");
    }
}
