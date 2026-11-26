//! Runs a [`Plan`] in **one transaction** and verifies it before it commits.
//!
//! The platform commits in steps and records its progress in `SchemaStorage` so an interrupted apply can
//! be resumed. SQL Server's DDL is transactional (`CREATE TABLE`, `CREATE INDEX`, `DROP TABLE` and
//! `sp_rename` all roll back), so the same protocol -- `NG` tables, copy, indexes, drop, rename, publish --
//! runs inside a single transaction: a failure at any point rolls the database back to exactly where it
//! started, and no intermediate `Status` is ever visible. The price is the log of the whole copy in one
//! transaction (fine for a catalog, not for a table of 100 GB).
//!
//! Between the phases the executor reads the database back and compares it with the model -- row counts
//! of the copy, the columns and indexes of every new table -- and rolls back on the first difference.

use std::time::Instant;

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::restructure::plan::{Method, Phase, Plan, Statement, StatementParam};
use crate::restructure::reader::{database_name, other_sessions};
use crate::restructure::schema::{Column, PhysicalTable, SqlType};
use crate::sql::SqlParam;
use crate::sql::mssql::{TdsConnection, sql_row};

/// How to run.
#[derive(Clone, Copy, Debug, Default)]
pub struct ExecOptions {
    /// Run everything, verify, and roll back instead of committing.
    pub trial: bool,
    /// Do not refuse when other sessions are connected to the database.
    pub skip_session_check: bool,
}

#[derive(Debug, Serialize)]
pub struct StepReport {
    pub phase: String,
    pub label: String,
    pub milliseconds: u128,
    pub rows: Option<u64>,
}

#[derive(Debug, Default, Serialize)]
pub struct ExecReport {
    pub database: String,
    pub trial: bool,
    pub committed: bool,
    pub other_sessions: i64,
    pub steps: Vec<StepReport>,
    pub verified: Vec<String>,
    pub milliseconds: u128,
}

fn phase_name(phase: Phase) -> &'static str {
    match phase {
        Phase::Guard => "guard",
        Phase::Create => "create",
        Phase::Load => "load",
        Phase::Indexes => "indexes",
        Phase::DropOld => "drop-old",
        Phase::Rename => "rename",
        Phase::Publish => "publish",
    }
}

/// Runs the plan. On any error the transaction is rolled back before the error is returned.
pub fn run(
    connection: &mut TdsConnection,
    plan: &Plan,
    options: ExecOptions,
) -> Result<ExecReport> {
    let started = Instant::now();
    let mut report = ExecReport {
        database: database_name(connection)?,
        trial: options.trial,
        ..ExecReport::default()
    };
    report.other_sessions = other_sessions(connection)?;
    if report.other_sessions > 0 && !options.skip_session_check {
        bail!(
            "{} other session(s) are connected to {}: a restructure needs the infobase to itself",
            report.other_sessions,
            report.database
        );
    }
    connection
        .run_batch("SET XACT_ABORT ON; SET LOCK_TIMEOUT 60000; BEGIN TRANSACTION;")
        .context("BEGIN TRANSACTION")?;
    let outcome = run_inside(connection, plan, &mut report);
    match outcome {
        Ok(()) if options.trial => {
            connection
                .run_batch("ROLLBACK TRANSACTION;")
                .context("ROLLBACK (trial)")?;
            report.verified.push("trial: rolled back".to_owned());
        }
        Ok(()) => {
            connection
                .run_batch("COMMIT TRANSACTION;")
                .context("COMMIT")?;
            report.committed = true;
        }
        Err(error) => {
            // XACT_ABORT already rolled back most errors; make sure of the rest.
            let _ = connection.run_batch("IF @@TRANCOUNT > 0 ROLLBACK TRANSACTION;");
            return Err(error);
        }
    }
    report.milliseconds = started.elapsed().as_millis();
    Ok(report)
}

fn run_inside(connection: &mut TdsConnection, plan: &Plan, report: &mut ExecReport) -> Result<()> {
    let extensions_before = crate::restructure::extensions::fingerprint(connection)
        .context("the state of the extensions before the restructure")?;
    let statements = plan.statements();
    let mut previous: Option<Phase> = None;
    for statement in &statements {
        if let Some(finished) = previous.filter(|phase| *phase != statement.phase) {
            after_phase(connection, plan, finished, report)?;
        }
        previous = Some(statement.phase);
        run_statement(connection, statement, report)
            .with_context(|| format!("{}: {}", phase_name(statement.phase), statement.label))?;
    }
    if let Some(finished) = previous {
        after_phase(connection, plan, finished, report)?;
    }
    let extensions_after = crate::restructure::extensions::fingerprint(connection)
        .context("the state of the extensions after the restructure")?;
    let changed = extensions_before.differences(&extensions_after);
    if !changed.is_empty() {
        bail!(
            "the restructure changed what the extensions keep ({}): rolled back",
            changed.join(", ")
        );
    }
    report.verified.push(format!(
        "extensions: {} parts as they were (SchemaStorage(1), DBNames-Ext-*, _ExtensionsInfo, _ExtensionsRestruct*, row counts of the X1 tables)",
        extensions_after.len()
    ));
    Ok(())
}

fn run_statement(
    connection: &mut TdsConnection,
    statement: &Statement,
    report: &mut ExecReport,
) -> Result<()> {
    let started = Instant::now();
    let rows = if statement.params.is_empty() {
        connection.run_batch(&statement.sql)?;
        None
    } else {
        let params: Vec<SqlParam<'_>> = statement
            .params
            .iter()
            .map(|param| match param {
                StatementParam::Bytes(bytes) => SqlParam::Binary(bytes),
                StatementParam::Int(value) => SqlParam::I64(*value),
            })
            .collect();
        Some(connection.execute(&statement.sql, &params)?)
    };
    report.steps.push(StepReport {
        phase: phase_name(statement.phase).to_owned(),
        label: statement.label.clone(),
        milliseconds: started.elapsed().as_millis(),
        rows,
    });
    if let (Some(expected), Some(changed)) = (statement.expect_rows, rows)
        && expected != changed
    {
        bail!("the statement changed {changed} row(s), the plan expects {expected}");
    }
    Ok(())
}

/// The read-back checks after a phase.
fn after_phase(
    connection: &mut TdsConnection,
    plan: &Plan,
    phase: Phase,
    report: &mut ExecReport,
) -> Result<()> {
    match (phase, plan.method) {
        (Phase::Create, Method::AlterAdd) => {
            for object in &plan.objects {
                let table = &object.tables[0].table;
                let physical_order = verify_structure(connection, table, "", false)?;
                report.verified.push(format!(
                    "{}: columns as the model (set), physical order {}",
                    table.name,
                    if physical_order {
                        "as the model"
                    } else {
                        "differs from the model: the new column is last"
                    }
                ));
            }
        }
        (Phase::Load, Method::Rebuild) => {
            for table in plan.tables().filter(|table| !table.create) {
                let old = count(connection, &table.table.name)?;
                let new = count(connection, &format!("{}NG", table.table.name))?;
                if old != new {
                    bail!(
                        "the copy of {} has {new} rows, the table {old}",
                        table.table.name
                    );
                }
                report
                    .verified
                    .push(format!("{}: {old} rows copied", table.table.name));
            }
        }
        (Phase::Indexes, Method::Rebuild) => {
            for table in plan.tables() {
                verify_structure(connection, &table.table, "NG", true)?;
                report.verified.push(format!(
                    "{}NG: columns and indexes as the model",
                    table.table.name
                ));
            }
        }
        (Phase::Rename, Method::Rebuild) => {
            for table in plan.tables() {
                verify_structure(connection, &table.table, "", true)?;
                let rows = count(connection, &table.table.name)?;
                report.verified.push(format!(
                    "{}: columns and indexes as the model, {rows} rows",
                    table.table.name
                ));
            }
            for table in plan.tables() {
                if table_exists(connection, &format!("{}NG", table.table.name))? {
                    bail!("{}NG is left behind", table.table.name);
                }
            }
        }
        (Phase::Publish, _) => {
            verify_published(connection, plan)?;
            report
                .verified
                .push("SchemaStorage, DBSchema, DBNames and Config as the plan".to_owned());
        }
        _ => {}
    }
    Ok(())
}

fn count(connection: &mut TdsConnection, table: &str) -> Result<i64> {
    let mut rows = 0;
    connection.query_each(
        &format!("SELECT COUNT_BIG(*) FROM dbo.{table}"),
        &[],
        |row| {
            rows = sql_row(row).i64(0)?;
            Ok(())
        },
    )?;
    Ok(rows)
}

fn table_exists(connection: &mut TdsConnection, table: &str) -> Result<bool> {
    let object = format!("dbo.{table}");
    let mut exists = false;
    connection.query_each(
        "SELECT CASE WHEN OBJECT_ID(@P1, N'U') IS NULL THEN 0 ELSE 1 END",
        &[SqlParam::Text(&object)],
        |row| {
            exists = sql_row(row).i64(0)? == 1;
            Ok(())
        },
    )?;
    Ok(exists)
}

fn column_matches(
    column: &Column,
    type_name: &str,
    max_length: i64,
    precision: i64,
    scale: i64,
    nullable: i64,
) -> bool {
    let type_ok = match &column.sql_type {
        SqlType::Binary(n) => type_name == "binary" && max_length == i64::from(*n),
        SqlType::VarBinary(n) => type_name == "varbinary" && max_length == i64::from(*n),
        SqlType::VarBinaryMax => type_name == "varbinary" && max_length == -1,
        SqlType::Numeric(p, s) => {
            type_name == "numeric" && precision == i64::from(*p) && scale == i64::from(*s)
        }
        SqlType::Int => type_name == "int",
        SqlType::BigInt => type_name == "bigint",
        SqlType::DateTime2 => type_name == "datetime2" && scale == 0,
        SqlType::NVarChar(n) => type_name == "nvarchar" && max_length == 2 * i64::from(*n),
        SqlType::NVarCharMax => type_name == "nvarchar" && max_length == -1,
        SqlType::NChar(n) => type_name == "nchar" && max_length == 2 * i64::from(*n),
        SqlType::Timestamp => type_name == "timestamp",
    };
    type_ok && (nullable != 0) == column.nullable
}

/// Compares the columns and the indexes of `dbo.<table><suffix>` with the model. With `ordered` the
/// physical column order must be the model's too; without it only the set of columns is compared.
/// Returns whether the physical order is the model's.
fn verify_structure(
    connection: &mut TdsConnection,
    table: &PhysicalTable,
    suffix: &str,
    ordered: bool,
) -> Result<bool> {
    let object = format!("dbo.{}{suffix}", table.name);
    let mut actual: Vec<(String, String, i64, i64, i64, i64)> = Vec::new();
    connection.query_each(
        "SELECT c.name, t.name, c.max_length, c.precision, c.scale, c.is_nullable \
         FROM sys.columns c JOIN sys.types t ON t.user_type_id = c.user_type_id \
         WHERE c.object_id = OBJECT_ID(@P1) ORDER BY c.column_id",
        &[SqlParam::Text(&object)],
        |row| {
            let mut row = sql_row(row);
            actual.push((
                row.take_text(0)?,
                row.take_text(1)?,
                row.i64(2)?,
                row.i64(3)?,
                row.i64(4)?,
                row.i64(5)?,
            ));
            Ok(())
        },
    )?;
    if actual.len() != table.columns.len() {
        bail!(
            "{object} has {} columns, the model {}",
            actual.len(),
            table.columns.len()
        );
    }
    let in_model_order = table
        .columns
        .iter()
        .zip(&actual)
        .all(|(column, found)| found.0 == column.name);
    if ordered && !in_model_order {
        bail!("{object}: the columns are not in the model's order");
    }
    for column in &table.columns {
        let Some(found) = actual.iter().find(|found| found.0 == column.name) else {
            bail!("{object}: column {} is missing", column.name);
        };
        if !column_matches(column, &found.1, found.2, found.3, found.4, found.5) {
            bail!(
                "{object}: column {} is {:?}, the model has {}",
                found.0,
                found,
                column.ddl()
            );
        }
    }

    // Key columns of every index in key order; the primary key is unnamed in the model.
    let mut indexes: Vec<(String, bool, bool, bool, String)> = Vec::new();
    connection.query_each(
        "SELECT i.name, i.is_unique, i.type, i.is_primary_key, \
         STRING_AGG(c.name, ',') WITHIN GROUP (ORDER BY ic.key_ordinal) \
         FROM sys.indexes i \
         JOIN sys.index_columns ic ON ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0 \
         JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id \
         WHERE i.object_id = OBJECT_ID(@P1) AND i.type IN (1, 2) \
         GROUP BY i.name, i.is_unique, i.type, i.is_primary_key",
        &[SqlParam::Text(&object)],
        |row| {
            let mut row = sql_row(row);
            indexes.push((
                row.take_text(0)?,
                row.i64(1)? != 0,
                row.i64(2)? == 1,
                row.i64(3)? != 0,
                row.take_text(4)?,
            ));
            Ok(())
        },
    )?;
    let mut expected: Vec<(String, bool, bool, String)> = table
        .indexes
        .iter()
        .map(|index| {
            (
                format!("{}{suffix}", index.name),
                index.unique,
                index.clustered,
                index.columns.join(","),
            )
        })
        .collect();
    if let Some(pk) = &table.inline_pk {
        expected.push((String::new(), true, true, pk.clone()));
    }
    for index in &indexes {
        let position = if index.3 {
            expected
                .iter()
                .position(|(name, unique, clustered, columns)| {
                    name.is_empty()
                        && *unique == index.1
                        && *clustered == index.2
                        && *columns == index.4
                })
        } else {
            expected
                .iter()
                .position(|(name, unique, clustered, columns)| {
                    *name == index.0
                        && *unique == index.1
                        && *clustered == index.2
                        && *columns == index.4
                })
        };
        match position {
            Some(position) => {
                expected.swap_remove(position);
            }
            None => bail!("{object}: index {index:?} is not in the model"),
        }
    }
    if let Some(missing) = expected.first() {
        bail!("{object}: the model's index {missing:?} was not created");
    }
    Ok(in_model_order)
}

/// The publish statements took effect.
fn verify_published(connection: &mut TdsConnection, plan: &Plan) -> Result<()> {
    let mut status = (0i64, 0i64, 0i64);
    connection.query_each(
        "SELECT Status, DATALENGTH(NewGenCreated), DATALENGTH(NewGenDropped) FROM dbo.SchemaStorage WHERE SchemaID = 0",
        &[],
        |row| {
            let row = sql_row(row);
            status = (row.i64(0)?, row.i64(1)?, row.i64(2)?);
            Ok(())
        },
    )?;
    if status.0 != 100 {
        bail!("SchemaStorage status is {}, not 100", status.0);
    }
    let mut same = false;
    connection.query_each(
        "SELECT CASE WHEN (SELECT CurrentSchema FROM dbo.SchemaStorage WHERE SchemaID = 0) = (SELECT SerializedData FROM dbo.DBSchema) THEN 1 ELSE 0 END",
        &[],
        |row| {
            same = sql_row(row).i64(0)? == 1;
            Ok(())
        },
    )?;
    if !same {
        bail!("DBSchema and SchemaStorage.CurrentSchema differ");
    }
    let mut remaining = -1i64;
    connection.query_each("SELECT COUNT_BIG(*) FROM dbo.ConfigSave", &[], |row| {
        remaining = sql_row(row).i64(0)?;
        Ok(())
    })?;
    if remaining != 0 {
        bail!("ConfigSave still holds {remaining} rows");
    }
    let mut names_length = -1i64;
    connection.query_each(
        "SELECT DATALENGTH(BinaryData) FROM dbo.Params WHERE FileName = N'DBNames' AND PartNo = 0",
        &[],
        |row| {
            names_length = sql_row(row).i64(0)?;
            Ok(())
        },
    )?;
    if names_length != plan.new_names_row.len() as i64 {
        bail!(
            "Params DBNames holds {names_length} bytes, the plan {}",
            plan.new_names_row.len()
        );
    }
    for cache in &plan.caches {
        let mut length = -1i64;
        connection.query_each(
            "SELECT DATALENGTH(BinaryData) FROM dbo.Params WHERE FileName = @P1 AND PartNo = 0",
            &[SqlParam::Text(&cache.row_name)],
            |row| {
                length = sql_row(row).i64(0)?;
                Ok(())
            },
        )?;
        if length != cache.row.len() as i64 {
            bail!(
                "Params {} holds {length} bytes, the plan {}",
                cache.row_name,
                cache.row.len()
            );
        }
    }
    Ok(())
}
