use std::borrow::Cow;
use std::sync::Mutex;

use anyhow::{Context, Result, bail, ensure};

use super::{Dbms, ScriptVariables, SqlClient, SqlParam, SqlRow, SqlValue};

pub use ibcmd_postgres::{PgConnection, PgTarget, decode_bytea};

/// Built-in PostgreSQL backend for the shared SQL interface.
pub struct PgClient {
    target: PgTarget,
    session: Mutex<()>,
}

impl PgClient {
    pub fn new(target: PgTarget) -> Self {
        Self {
            target,
            session: Mutex::new(()),
        }
    }

    /// Keep transaction state on one connection until the operation ends.
    pub fn with_session<T>(&self, run: impl FnOnce(&mut PgConnection) -> Result<T>) -> Result<T> {
        let _session = self
            .session
            .lock()
            .map_err(|_| anyhow::anyhow!("PostgreSQL session lock poisoned"))?;
        run(&mut PgConnection::connect(&self.target)?)
    }
}

impl SqlClient for PgClient {
    fn dbms(&self) -> Dbms {
        Dbms::PostgreSql
    }

    fn max_connections(&self) -> usize {
        1
    }

    fn run_script(&self, script: &str, _variables: ScriptVariables) -> Result<()> {
        // PostgreSQL does not have sqlcmd's client-side variable substitution.
        ensure!(!script.contains('\0'), "PostgreSQL script contains NUL");
        self.with_session(|connection| connection.query(script, &mut |_| Ok(())))
    }

    fn execute(&self, statement: &str, params: &[SqlParam<'_>]) -> Result<u64> {
        self.with_session(|connection| query(connection, statement, params, &mut |_| Ok(())))
    }

    fn read_rows(
        &self,
        statement: &str,
        params: &[SqlParam<'_>],
        each: &mut dyn FnMut(SqlRow) -> Result<()>,
    ) -> Result<()> {
        self.with_session(|connection| query(connection, statement, params, each).map(|_| ()))
    }

    fn query_json(&self, statement: &str) -> Result<Option<String>> {
        match self.query_scalar(statement, &[])? {
            None | Some(SqlValue::Null) => Ok(None),
            Some(SqlValue::Text(json)) => Ok(Some(json)),
            Some(_) => bail!("PostgreSQL JSON query returned a non-text value"),
        }
    }

    fn write_rows(&self, table: &str, columns: &[&str], rows: &[Vec<SqlParam<'_>>]) -> Result<u64> {
        if rows.is_empty() {
            return Ok(0);
        }
        self.with_session(|connection| {
            copy_rows(
                connection,
                table,
                columns,
                rows.iter().map(|row| row.iter().copied().map(Ok)),
            )
        })?;
        Ok(u64::try_from(rows.len())?)
    }
}

/// COPY on a caller-owned transaction, using the same encoders as the shared backend.
/// Table and columns are trusted SQL identifiers in PostgreSQL syntax.
/// Each input must match the binary format of its column (for example U8
/// maps to smallint and I32 maps to integer).
pub fn copy_rows<'a>(
    connection: &mut PgConnection,
    table: &str,
    columns: &[&str],
    rows: impl IntoIterator<Item = impl IntoIterator<Item = Result<SqlParam<'a>>>>,
) -> Result<()> {
    ensure!(
        !columns.is_empty(),
        "PostgreSQL COPY needs at least one column"
    );
    let sql = format!("COPY {table} ({}) FROM STDIN BINARY", columns.join(", "));
    connection.copy_binary(&sql, |copy| {
        for row in rows {
            let row: Vec<_> = row
                .into_iter()
                .map(|value| value.map(encode))
                .collect::<Result<_>>()?;
            let fields: Vec<_> = row.iter().map(|(_, bytes)| Some(bytes.as_ref())).collect();
            copy.row(&fields)?;
        }
        Ok(())
    })
}

fn query(
    connection: &mut PgConnection,
    statement: &str,
    params: &[SqlParam<'_>],
    each: &mut dyn FnMut(SqlRow) -> Result<()>,
) -> Result<u64> {
    let encoded: Vec<_> = params.iter().copied().map(encode).collect();
    let params: Vec<_> = encoded
        .iter()
        .map(|(oid, bytes)| (*oid, bytes.as_ref()))
        .collect();
    connection.query_params(statement, &params, |types, fields| {
        ensure!(
            types.len() == fields.len(),
            "PostgreSQL row width differs from its description"
        );
        let values = types
            .iter()
            .zip(fields)
            .map(|(&oid, field)| decode(oid, field))
            .collect::<Result<_>>()?;
        each(SqlRow {
            result_set: 0,
            values,
        })
    })
}

// OIDs and binary encodings of PostgreSQL's built-in scalar types.
fn encode(value: SqlParam<'_>) -> (u32, Cow<'_, [u8]>) {
    match value {
        SqlParam::Text(text) => (25, Cow::Borrowed(text.as_bytes())),
        SqlParam::U8(value) => (21, Cow::Owned(i16::from(value).to_be_bytes().to_vec())),
        SqlParam::I32(value) => (23, Cow::Owned(value.to_be_bytes().to_vec())),
        SqlParam::I64(value) => (20, Cow::Owned(value.to_be_bytes().to_vec())),
        SqlParam::Binary(value) => (17, Cow::Borrowed(value)),
    }
}

fn decode(oid: u32, bytes: Option<Vec<u8>>) -> Result<SqlValue> {
    let Some(bytes) = bytes else {
        return Ok(SqlValue::Null);
    };
    if oid == 17 {
        return Ok(SqlValue::Binary(decode_binary(&bytes)?));
    }
    if oid == 16 {
        return match bytes.as_slice() {
            [b't'] => Ok(SqlValue::Int(1)),
            [b'f'] => Ok(SqlValue::Int(0)),
            _ => bail!("invalid PostgreSQL boolean encoding"),
        };
    }
    let text = String::from_utf8(bytes).context("PostgreSQL result is not UTF-8")?;
    Ok(match oid {
        20 | 21 | 23 | 26 => SqlValue::Int(text.parse()?),
        700 | 701 => SqlValue::Float(text.parse()?),
        1700 => text.parse().map_or(SqlValue::Other(text), SqlValue::Int),
        18 | 19 | 25 | 114 | 142 | 1042 | 1043 | 3802 => SqlValue::Text(text),
        _ => SqlValue::Other(text),
    })
}

// bytea can use either of PostgreSQL's output formats.
fn decode_binary(bytes: &[u8]) -> Result<Vec<u8>> {
    if matches!(bytes, [b'\\', b'x', ..]) {
        return decode_bytea(bytes);
    }
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut input = bytes.iter().copied();
    while let Some(byte) = input.next() {
        if byte != b'\\' {
            decoded.push(byte);
            continue;
        }
        match input.next() {
            Some(b'\\') => decoded.push(b'\\'),
            Some(high @ b'0'..=b'3') => {
                let middle = input
                    .next()
                    .filter(|b| (b'0'..=b'7').contains(b))
                    .context("invalid bytea escape")?;
                let low = input
                    .next()
                    .filter(|b| (b'0'..=b'7').contains(b))
                    .context("invalid bytea escape")?;
                decoded.push((high - b'0') * 64 + (middle - b'0') * 8 + (low - b'0'));
            }
            _ => bail!("invalid bytea escape"),
        }
    }
    Ok(decoded)
}
