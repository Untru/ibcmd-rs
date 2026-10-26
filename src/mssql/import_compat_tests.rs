//! Clean-room typed-row coverage for the SQL Server 2012 import reads.

use std::sync::Mutex;

use anyhow::{Result, bail, ensure};

use crate::sql::{
    Dbms, ScriptVariables, SqlClient, SqlExec, SqlLogin, SqlParam, SqlRow, SqlTarget, SqlValue,
};

struct ImportReadClient {
    expected_query: &'static str,
    rows: Mutex<Option<Vec<SqlRow>>>,
}

impl SqlClient for ImportReadClient {
    fn dbms(&self) -> Dbms {
        Dbms::SqlServer
    }

    fn uses_mssql_2012_compatibility(&self) -> bool {
        true
    }

    fn max_connections(&self) -> usize {
        1
    }

    fn read_rows(
        &self,
        query: &str,
        _: &[SqlParam<'_>],
        each: &mut dyn FnMut(SqlRow) -> Result<()>,
    ) -> Result<()> {
        ensure!(
            query.contains(self.expected_query),
            "unexpected import query"
        );
        ensure!(
            !query.contains("FOR JSON"),
            "SQL Server 2012 has no FOR JSON"
        );
        ensure!(
            !query.contains("HASHBYTES"),
            "import metadata needs no server hashing"
        );
        for row in self.rows.lock().unwrap().take().expect("one read only") {
            each(row)?;
        }
        Ok(())
    }

    fn query_json(&self, _: &str) -> Result<Option<String>> {
        bail!("SQL Server 2012 has no FOR JSON")
    }

    fn execute(&self, _: &str, _: &[SqlParam<'_>]) -> Result<u64> {
        panic!("import metadata must not write")
    }

    fn run_script(&self, _: &str, _: ScriptVariables) -> Result<()> {
        panic!("import metadata must not write")
    }

    fn write_rows(&self, _: &str, _: &[&str], _: &[Vec<SqlParam<'_>>]) -> Result<u64> {
        panic!("import metadata must not write")
    }
}

fn sql(expected_query: &'static str, values: Vec<Vec<SqlValue>>) -> SqlExec {
    SqlExec::with_client(
        SqlTarget {
            server: "fake".to_owned(),
            database: None,
            login: SqlLogin::Integrated,
            trust_server_certificate: false,
        },
        Box::new(ImportReadClient {
            expected_query,
            rows: Mutex::new(Some(
                values
                    .into_iter()
                    .map(|values| SqlRow {
                        result_set: 0,
                        values,
                    })
                    .collect(),
            )),
        }),
    )
}

#[test]
fn storage_stats_preserve_large_counts_and_checksum() {
    let sql = sql(
        "USE [db]]name]",
        vec![vec![
            SqlValue::Text("ConfigSave".to_owned()),
            SqlValue::Int(4_294_967_297),
            SqlValue::Int(9_000_000_000),
            SqlValue::Int(-123),
        ]],
    );
    let stats = super::storage_table_stats(&sql, "db]name", "ConfigSave").unwrap();

    assert_eq!(stats.table_name, "ConfigSave");
    assert_eq!(stats.file_name, "ConfigSave.bcp");
    assert_eq!(stats.row_count, 4_294_967_297);
    assert_eq!(stats.binary_bytes, 9_000_000_000);
    assert_eq!(stats.row_checksum, Some(-123));
}

#[test]
fn empty_storage_stats_preserve_null_checksum() {
    let sql = sql(
        "FROM [ConfigSave]",
        vec![vec![
            SqlValue::Text("ConfigSave".to_owned()),
            SqlValue::Int(0),
            SqlValue::Int(0),
            SqlValue::Null,
        ]],
    );
    let stats = super::storage_table_stats(&sql, "db", "ConfigSave").unwrap();

    assert_eq!(stats.row_count, 0);
    assert_eq!(stats.binary_bytes, 0);
    assert_eq!(stats.row_checksum, None);
}

#[test]
fn dynamic_removal_names_keep_marker_aliases_and_unicode_sort() {
    let sql = sql(
        "USE [db]]name]",
        vec![
            vec![SqlValue::Text("Я_dynupdate_generation".to_owned())],
            vec![SqlValue::Text("Б_dynupdate_generation".to_owned())],
            vec![SqlValue::Text("DynamicallyUpdated".to_owned())],
            vec![SqlValue::Text("а_dynupdate_generation".to_owned())],
        ],
    );
    let names = super::override_stage::dynamic_update_rows(&sql, "db]name").unwrap();

    assert_eq!(
        names,
        [
            "DynamicallyUpdated",
            "а_dynupdate_generation",
            "Б_dynupdate_generation",
            "Я_dynupdate_generation",
        ]
    );
}

#[test]
fn no_dynamic_rows_produce_no_removal_names() {
    let sql = sql("PartNo = 0", vec![]);

    assert!(
        super::override_stage::dynamic_update_rows(&sql, "db")
            .unwrap()
            .is_empty()
    );
}

#[test]
fn target_counts_select_matching_empty_and_different_configurations() {
    for (total, matching, expected) in [(12, 1, true), (0, 0, false), (12, 0, false)] {
        let sql = sql(
            "USE [db]]name]",
            vec![vec![SqlValue::Int(total), SqlValue::Int(matching)]],
        );
        let state = super::import_target_state_with_sql(
            &sql,
            "db]name",
            "12345678-1234-4234-8234-123456789abc",
        )
        .unwrap();

        assert_eq!(state.config_rows, total);
        assert_eq!(state.holds_configuration, expected);
    }
}

#[test]
fn target_counts_do_not_truncate_large_configuration_tables() {
    let sql = sql(
        "COUNT_BIG(*)",
        vec![vec![SqlValue::Int(4_294_967_297), SqlValue::Int(1)]],
    );
    let state =
        super::import_target_state_with_sql(&sql, "db", "12345678-1234-4234-8234-123456789abc")
            .unwrap();

    assert_eq!(state.config_rows, 4_294_967_297);
    assert!(state.holds_configuration);
}

#[test]
fn target_counts_require_an_aggregate_result() {
    let sql = sql("COUNT_BIG(*)", vec![]);
    let error =
        super::import_target_state_with_sql(&sql, "db", "12345678-1234-4234-8234-123456789abc")
            .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("row count query returned no row"),
        "{error:#}"
    );
}

/// Leaves the trait's compatibility default in force, as modern clients do.
struct ModernImportClient {
    expected_query: &'static str,
    response: &'static str,
}

impl SqlClient for ModernImportClient {
    fn dbms(&self) -> Dbms {
        Dbms::SqlServer
    }

    fn max_connections(&self) -> usize {
        1
    }

    fn read_rows(
        &self,
        _: &str,
        _: &[SqlParam<'_>],
        _: &mut dyn FnMut(SqlRow) -> Result<()>,
    ) -> Result<()> {
        bail!("a modern client must keep the original JSON read")
    }

    fn query_json(&self, query: &str) -> Result<Option<String>> {
        ensure!(
            query == self.expected_query,
            "the original JSON query changed"
        );
        Ok(Some(self.response.to_owned()))
    }

    fn execute(&self, _: &str, _: &[SqlParam<'_>]) -> Result<u64> {
        panic!("import metadata must not write")
    }

    fn run_script(&self, _: &str, _: ScriptVariables) -> Result<()> {
        panic!("import metadata must not write")
    }

    fn write_rows(&self, _: &str, _: &[&str], _: &[Vec<SqlParam<'_>>]) -> Result<u64> {
        panic!("import metadata must not write")
    }
}

fn modern_sql(expected_query: &'static str, response: &'static str) -> SqlExec {
    SqlExec::with_client(
        SqlTarget {
            server: "fake".to_owned(),
            database: None,
            login: SqlLogin::Integrated,
            trust_server_certificate: false,
        },
        Box::new(ModernImportClient {
            expected_query,
            response,
        }),
    )
}

#[test]
fn modern_target_state_retains_original_json_query() {
    let sql = modern_sql(
        "SET NOCOUNT ON; USE [db]; SELECT COUNT_BIG(*) AS config_rows, COUNT_BIG(CASE WHEN FileName = N'12345678-1234-4234-8234-123456789abc' THEN 1 END) AS configuration_rows FROM dbo.Config FOR JSON PATH;",
        r#"[{"config_rows":12,"configuration_rows":1}]"#,
    );
    let state =
        super::import_target_state_with_sql(&sql, "db", "12345678-1234-4234-8234-123456789abc")
            .unwrap();

    assert_eq!(state.config_rows, 12);
    assert!(state.holds_configuration);
}

#[test]
fn modern_storage_stats_retain_original_json_query() {
    let sql = modern_sql(
        "SET NOCOUNT ON; USE [db]; SELECT N'ConfigSave' AS table_name, COUNT_BIG(*) AS row_count, ISNULL(SUM(CONVERT(bigint, DATALENGTH(BinaryData))), 0) AS binary_bytes, CONVERT(bigint, CHECKSUM_AGG(BINARY_CHECKSUM(*))) AS row_checksum FROM [ConfigSave] FOR JSON PATH;",
        r#"[{"table_name":"ConfigSave","row_count":0,"binary_bytes":0}]"#,
    );
    let stats = super::storage_table_stats(&sql, "db", "ConfigSave").unwrap();

    assert_eq!(stats.file_name, "ConfigSave.bcp");
    assert_eq!(stats.row_count, 0);
    assert_eq!(stats.row_checksum, None);
}

#[test]
fn modern_dynamic_names_retain_original_json_query() {
    let sql = modern_sql(
        "SET NOCOUNT ON; USE [db]; SELECT FileName AS name FROM dbo.Config WHERE PartNo = 0 AND (FileName = N'DynamicallyUpdated' OR FileName LIKE N'%[_]dynupdate[_]%') ORDER BY FileName FOR JSON PATH;",
        r#"[{"name":"Я_dynupdate_generation"},{"name":"DynamicallyUpdated"}]"#,
    );
    let names = super::override_stage::dynamic_update_rows(&sql, "db").unwrap();

    assert_eq!(names, ["DynamicallyUpdated", "Я_dynupdate_generation"]);
}
