//! Live original TDS boundary; no pool/reset/reconnect is reachable here.
use super::TdsConnection;
use crate::sql::owned::OriginalSqlDriver;
use crate::sql::{OwnedSqlCommand, SqlParam, SqlRow};
use anyhow::Result;

struct OriginalTdsDriver {
    connection: TdsConnection,
}

pub(super) fn open(connection: TdsConnection) -> OwnedSqlCommand {
    // Even a lost/malformed first identity response returns the retained owner
    // in Unknown, rather than dropping it as a proved clean refusal.
    OwnedSqlCommand::from_original(Box::new(OriginalTdsDriver { connection }))
}

impl OriginalSqlDriver for OriginalTdsDriver {
    fn simple_rows(&mut self, sql: &str, each: &mut dyn FnMut(SqlRow) -> Result<()>) -> Result<()> {
        self.connection.owned_simple_rows(sql, each)
    }
    fn run_batch(&mut self, sql: &str) -> Result<()> {
        self.connection.run_batch(sql)
    }
    fn execute(&mut self, sql: &str, params: &[SqlParam<'_>]) -> Result<u64> {
        self.connection.owned_execute(sql, params)
    }
    fn read_rows(
        &mut self,
        sql: &str,
        params: &[SqlParam<'_>],
        each: &mut dyn FnMut(SqlRow) -> Result<()>,
    ) -> Result<()> {
        self.connection.owned_query_each(sql, params, each)
    }
}
