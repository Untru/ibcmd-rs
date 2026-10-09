//! The SQL Server backend: the TDS protocol in-process (tiberius).
//!
//! What is SQL Server's own stays here: server names with instances and
//! ports, the sqlcmd script dialect (`GO`), `sp_executesql`, `FOR JSON`
//! documents split over rows, and bulk writes into `varbinary(max)` columns.

mod address;
#[cfg(all(test, feature = "mssql-live-tests"))]
mod live_tests;
mod owned;
mod script;
mod tds;

use std::ops::Range;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use anyhow::{Context, Result, bail};

pub use address::{DEFAULT_PORT, ServerAddress};
pub use script::SealedSqlScript;
pub use script::{ScriptBatch, split_batches};
pub use tds::{REQUEST_FAILED, TdsConnection, TdsPool, sql_row, sql_value};

use super::{Dbms, ScriptVariables, SqlClient, SqlParam, SqlRow, SqlTarget};

/// SQL Server takes at most 2 100 parameters in one request.
pub(super) const MAX_PARAMETERS: usize = 2100;
/// Rows one INSERT statement carries: measured on the lab server, 25-100
/// rows of ~30 KB each moved ~50 MB/s, 1 row 15 MB/s, 400 rows 30 MB/s
/// (the plan of a long VALUES list costs more than it saves).
pub(super) const INSERT_ROWS: usize = 50;
/// Bytes of parameters one INSERT statement carries before the next starts;
/// a single larger row still gets a statement of its own.
pub(super) const INSERT_BYTES: usize = 8 * 1024 * 1024;
/// Below this many bytes a bulk write uses one connection.
const PARALLEL_WRITE_BYTES: usize = 64 * 1024 * 1024;

/// The built-in SQL Server client.
pub struct MssqlClient {
    pool: TdsPool,
}

impl MssqlClient {
    /// A client for `target` with at most `connections` open at once; nothing
    /// connects until the first request.
    pub fn new(target: SqlTarget, connections: usize) -> Result<Self> {
        Ok(Self {
            pool: TdsPool::new(target, connections)?,
        })
    }

    pub fn pool(&self) -> &TdsPool {
        &self.pool
    }
}

impl SqlClient for MssqlClient {
    fn dbms(&self) -> Dbms {
        Dbms::SqlServer
    }

    fn open_owned_command(&self) -> Result<super::OwnedSqlCommand> {
        Ok(owned::open(self.pool.dedicated()?))
    }

    fn max_connections(&self) -> usize {
        self.pool.capacity()
    }

    fn run_script(&self, script: &str, variables: ScriptVariables) -> Result<()> {
        self.pool.run_script(script, variables)
    }

    fn execute(&self, statement: &str, params: &[SqlParam<'_>]) -> Result<u64> {
        self.pool
            .with(|connection| connection.execute(statement, params))
    }

    fn read_rows(
        &self,
        query: &str,
        params: &[SqlParam<'_>],
        each: &mut dyn FnMut(SqlRow) -> Result<()>,
    ) -> Result<()> {
        self.pool
            .query_each(query, params, |row| each(sql_row(row)))
    }

    /// SQL Server splits a long `FOR JSON` document into rows of about 2 000
    /// characters (one column); they are joined here. A top-level `FOR JSON`
    /// over no rows returns no row at all: `None`.
    fn query_json(&self, query: &str) -> Result<Option<String>> {
        let mut document: Option<String> = None;
        let mut result_set = None;
        self.pool.query_each(query, &[], |row| {
            let index = row.result_index();
            if *result_set.get_or_insert(index) != index {
                return Ok(());
            }
            let part = row
                .try_get::<&str, _>(0)
                .context("a FOR JSON query returned a non-text column")?
                .unwrap_or_default();
            document.get_or_insert_with(String::new).push_str(part);
            Ok(())
        })?;
        Ok(document)
    }

    /// Multi-row `INSERT ... VALUES` statements with parameters, on several
    /// connections at once for large writes. (tiberius' `INSERT BULK` cannot
    /// carry a `varbinary(max)` value over 65 535 bytes: it takes the column's
    /// 0xFFFF "max" marker for a length limit.)
    fn write_rows(&self, table: &str, columns: &[&str], rows: &[Vec<SqlParam<'_>>]) -> Result<u64> {
        if columns.is_empty() {
            bail!("a bulk write into {table} names no columns");
        }
        if let Some(row) = rows.iter().find(|row| row.len() != columns.len()) {
            bail!(
                "a row for {table} has {} values for {} columns",
                row.len(),
                columns.len()
            );
        }
        let chunks = insert_chunks(rows, columns.len());
        let total_bytes = rows
            .iter()
            .flatten()
            .map(SqlParam::wire_bytes)
            .sum::<usize>();
        let workers = if total_bytes < PARALLEL_WRITE_BYTES {
            1
        } else {
            self.pool.capacity().min(chunks.len()).max(1)
        };
        let insert = |connection: &mut TdsConnection, chunk: &Range<usize>| -> Result<u64> {
            let slice = &rows[chunk.clone()];
            let statement = insert_statement(table, columns, slice.len());
            let params = slice.iter().flatten().copied().collect::<Vec<_>>();
            connection.execute(&statement, &params).with_context(|| {
                format!(
                    "failed to write rows {}..{} into {table}",
                    chunk.start, chunk.end
                )
            })
        };
        if workers == 1 {
            return self.pool.with(|connection| {
                chunks
                    .iter()
                    .try_fold(0u64, |total, chunk| Ok(total + insert(connection, chunk)?))
            });
        }
        // Each worker takes the next chunk until none is left; the first
        // failure stops the others at their next chunk.
        let next = AtomicUsize::new(0);
        let failed = AtomicBool::new(false);
        std::thread::scope(|scope| {
            let handles = (0..workers)
                .map(|_| {
                    scope.spawn(|| {
                        self.pool.with(|connection| {
                            let mut written = 0u64;
                            loop {
                                if failed.load(Ordering::Relaxed) {
                                    return Ok(written);
                                }
                                let index = next.fetch_add(1, Ordering::Relaxed);
                                let Some(chunk) = chunks.get(index) else {
                                    return Ok(written);
                                };
                                match insert(connection, chunk) {
                                    Ok(count) => written += count,
                                    Err(error) => {
                                        failed.store(true, Ordering::Relaxed);
                                        return Err(error);
                                    }
                                }
                            }
                        })
                    })
                })
                .collect::<Vec<_>>();
            let mut total = 0u64;
            let mut first_error = None;
            for handle in handles {
                match handle.join() {
                    Ok(Ok(count)) => total += count,
                    Ok(Err(error)) => {
                        first_error.get_or_insert(error);
                    }
                    Err(panic) => std::panic::resume_unwind(panic),
                }
            }
            match first_error {
                Some(error) => Err(error),
                None => Ok(total),
            }
        })
    }
}

/// Row ranges of at most `INSERT_ROWS` rows, `INSERT_BYTES` bytes and
/// `MAX_PARAMETERS` parameters each (a single larger row alone).
fn insert_chunks(rows: &[Vec<SqlParam<'_>>], columns: usize) -> Vec<Range<usize>> {
    let max_rows = INSERT_ROWS.min(MAX_PARAMETERS / columns).max(1);
    let mut chunks = Vec::new();
    let mut start = 0usize;
    let mut bytes = 0usize;
    for (index, row) in rows.iter().enumerate() {
        let row_bytes = row.iter().map(SqlParam::wire_bytes).sum::<usize>();
        if index > start && (index - start == max_rows || bytes + row_bytes > INSERT_BYTES) {
            chunks.push(start..index);
            start = index;
            bytes = 0;
        }
        bytes += row_bytes;
    }
    if start < rows.len() {
        chunks.push(start..rows.len());
    }
    chunks
}

/// `INSERT INTO <table> (<columns>) VALUES (@P1, ...), (...)` for `rows` rows.
pub(super) fn insert_statement(table: &str, columns: &[&str], rows: usize) -> String {
    let mut statement = format!("INSERT INTO {table} ({}) VALUES ", columns.join(", "));
    let mut parameter = 0usize;
    for row in 0..rows {
        if row > 0 {
            statement.push_str(", ");
        }
        statement.push('(');
        for column in 0..columns.len() {
            if column > 0 {
                statement.push_str(", ");
            }
            parameter += 1;
            statement.push_str(&format!("@P{parameter}"));
        }
        statement.push(')');
    }
    statement
}

#[cfg(test)]
mod tests {
    use super::{INSERT_BYTES, INSERT_ROWS, insert_chunks, insert_statement};
    use crate::sql::SqlParam;

    #[test]
    fn insert_statements_number_parameters_row_by_row() {
        assert_eq!(
            insert_statement("tempdb.dbo.[t]", &["A", "B"], 2),
            "INSERT INTO tempdb.dbo.[t] (A, B) VALUES (@P1, @P2), (@P3, @P4)"
        );
    }

    #[test]
    fn chunks_respect_rows_bytes_and_single_large_rows() {
        let small = vec![0u8; 10];
        let rows = (0..INSERT_ROWS * 2 + 3)
            .map(|_| vec![SqlParam::U8(1), SqlParam::Binary(&small)])
            .collect::<Vec<_>>();
        let chunks = insert_chunks(&rows, 2);
        assert_eq!(
            chunks,
            vec![
                0..INSERT_ROWS,
                INSERT_ROWS..INSERT_ROWS * 2,
                INSERT_ROWS * 2..INSERT_ROWS * 2 + 3
            ]
        );

        let big = vec![0u8; INSERT_BYTES / 2 + 1];
        let huge = vec![0u8; INSERT_BYTES * 3];
        let rows = vec![
            vec![SqlParam::Binary(&big)],
            vec![SqlParam::Binary(&big)],
            vec![SqlParam::Binary(&huge)],
            vec![SqlParam::Binary(&small)],
        ];
        assert_eq!(insert_chunks(&rows, 1), vec![0..1, 1..2, 2..3, 3..4]);
        assert!(insert_chunks(&[], 4).is_empty());
    }

    #[test]
    fn chunks_stay_under_the_parameter_limit() {
        let rows = (0..1000)
            .map(|_| vec![SqlParam::U8(0); 100])
            .collect::<Vec<_>>();
        let chunks = insert_chunks(&rows, 100);
        assert!(chunks.iter().all(|chunk| chunk.len() * 100 <= 2100));
        assert_eq!(chunks.iter().map(|chunk| chunk.len()).sum::<usize>(), 1000);
    }
}
