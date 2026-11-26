//! The plan as T-SQL text: the structure phase that runs inside another transaction.
//!
//! [`exec`](crate::restructure::exec) runs the plan from Rust, statement by statement, and reads the
//! database back between the phases. The own apply (`mssql_config_apply`) moves the staged rows in **one
//! script** of its own; a restructure that belongs in the same transaction has to be text it can splice
//! in. This module renders that text: the platform's statements with the binary values written as
//! literals, and the read-backs written as assertions that `THROW` (the apply's `CATCH` rolls the whole
//! transaction back). The text assumes what the apply's script gives it: a transaction, `XACT_ABORT`,
//! exclusive locks and exclusive access. It does not touch `Config`, `ConfigSave` and the derived caches
//! (`Params` `*.si`): the apply moves the rows, and writes the caches ([`Plan::caches`]) through its own
//! guarded rewrite.

use std::fmt::Write as _;

use anyhow::{Result, bail};

use crate::mssql_config_apply::model::hex_upper;
use crate::restructure::plan::{Method, Plan};
use crate::restructure::schema::{
    Column, PhysicalTable, SqlType, create_index_sql, create_table_sql,
};
use crate::restructure::storage::EMPTY_GENERATION;

/// Error numbers of the assertions (the apply's own are 57300..57399).
pub mod code {
    pub const SCHEMA_DRIFTED: u32 = 57400;
    pub const TABLE_STATE: u32 = 57401;
    pub const NAMES_DRIFTED: u32 = 57402;
    pub const COPY_COUNT: u32 = 57403;
    pub const STRUCTURE: u32 = 57404;
    pub const PUBLISH: u32 = 57405;
    /// What the extensions keep is not what it was before the restructure (S1-I).
    pub const EXTENSIONS_CHANGED: u32 = 57406;
}

fn quote(text: &str) -> String {
    text.replace('\'', "''")
}

fn assert_that(sql: &mut String, failing: &str, code: u32, message: &str) {
    writeln!(sql, "IF {failing} THROW {code}, N'{}', 1;", quote(message)).unwrap();
}

/// `name|type|max_length|precision|scale|nullable` of a column as `sys.columns` reports it (the
/// length only for the types that have one, precision and scale only for the types that have them).
fn column_signature(column: &Column) -> String {
    let (type_name, max_length, precision, scale) = match &column.sql_type {
        SqlType::Binary(n) => ("binary", i64::from(*n), 0, 0),
        SqlType::VarBinary(n) => ("varbinary", i64::from(*n), 0, 0),
        SqlType::VarBinaryMax => ("varbinary", -1, 0, 0),
        SqlType::Numeric(p, s) => ("numeric", 0, i64::from(*p), i64::from(*s)),
        SqlType::Int => ("int", 0, 0, 0),
        SqlType::BigInt => ("bigint", 0, 0, 0),
        SqlType::DateTime2 => ("datetime2", 0, 0, 0),
        SqlType::NVarChar(n) => ("nvarchar", 2 * i64::from(*n), 0, 0),
        SqlType::NVarCharMax => ("nvarchar", -1, 0, 0),
        SqlType::NChar(n) => ("nchar", 2 * i64::from(*n), 0, 0),
        SqlType::Timestamp => ("timestamp", 0, 0, 0),
    };
    format!(
        "{}|{type_name}|{max_length}|{precision}|{scale}|{}",
        column.name,
        u8::from(column.nullable)
    )
}

/// The assertion that one index of `dbo.<table><suffix>` exists (a primary key has the empty name).
fn assert_index(
    sql: &mut String,
    object: &str,
    name: &str,
    unique: bool,
    clustered: bool,
    columns: &str,
) {
    assert_that(
        sql,
        &format!(
            "NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'{object}') \
             AND i.type = {} AND i.is_unique = {} \
             AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'{}' \
             AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) \
             FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id \
             WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'{}')",
            if clustered { 1 } else { 2 },
            u8::from(unique),
            quote(name),
            quote(columns)
        ),
        code::STRUCTURE,
        &format!("{object} lacks the index {name} ({columns})"),
    );
}

/// The assertions that the columns and the indexes of `dbo.<table><suffix>` are the model's.
fn assert_structure(sql: &mut String, table: &PhysicalTable, suffix: &str) {
    let object = format!("dbo.{}{suffix}", table.name);
    let expected = table
        .columns
        .iter()
        .map(column_signature)
        .collect::<Vec<_>>()
        .join(";");
    assert_that(
        sql,
        &format!(
            "ISNULL((SELECT STRING_AGG(CONVERT(nvarchar(max), CONCAT(c.name, N'|', t.name, N'|', \
             CASE WHEN t.name IN (N'binary', N'varbinary', N'nvarchar', N'nchar') THEN c.max_length ELSE 0 END, N'|', \
             CASE WHEN t.name = N'numeric' THEN c.precision ELSE 0 END, N'|', \
             CASE WHEN t.name IN (N'numeric', N'datetime2') THEN c.scale ELSE 0 END, N'|', c.is_nullable)), N';') \
             WITHIN GROUP (ORDER BY c.column_id) \
             FROM sys.columns c JOIN sys.types t ON t.user_type_id = c.user_type_id \
             WHERE c.object_id = OBJECT_ID(N'{object}')), N'') <> N'{}'",
            quote(&expected)
        ),
        code::STRUCTURE,
        &format!("the columns of {object} are not the model's"),
    );
    let mut count = 0usize;
    for index in &table.indexes {
        count += 1;
        assert_index(
            sql,
            &object,
            &format!("{}{suffix}", index.name),
            index.unique,
            index.clustered,
            &index.columns.join(","),
        );
    }
    if let Some(pk) = &table.inline_pk {
        count += 1;
        assert_index(sql, &object, "", true, true, pk);
    }
    assert_that(
        sql,
        &format!(
            "(SELECT COUNT(*) FROM sys.indexes WHERE object_id = OBJECT_ID(N'{object}') AND type IN (1, 2)) <> {count}"
        ),
        code::STRUCTURE,
        &format!("{object} has indexes the model does not"),
    );
}

/// T-SQL that assigns to `@ext_<which>` the fingerprint of everything the extensions keep (S1-I,
/// `restructure::extensions::fingerprint` in T-SQL): the `SchemaStorage` rows but the main one, the
/// `Params` rows `DBNames*-Ext-*`, count and checksum of `_ExtensionsInfo`, `_ExtensionsRestruct` and
/// `_ExtensionsRestructNGS` (when they exist), and the row count of every table of the extension schema
/// (the names that end in `X1`). A restructure of the main configuration leaves it as it was.
fn extension_state_sql(which: &str) -> String {
    let mut sql = String::new();
    writeln!(
        sql,
        "SET @ext_{which} = CONCAT(\n\
         (SELECT CONVERT(nvarchar(max), STRING_AGG(CONVERT(nvarchar(max), CONCAT(SchemaID, N':', Status, N':', \
         CONVERT(varchar(64), HASHBYTES('SHA2_256', CurrentSchema), 2), N':', \
         CONVERT(varchar(64), HASHBYTES('SHA2_256', NewGenCreated), 2), N':', \
         CONVERT(varchar(64), HASHBYTES('SHA2_256', NewGenDropped), 2))), N';') WITHIN GROUP (ORDER BY SchemaID)) \
         FROM dbo.SchemaStorage WHERE SchemaID <> 0), N'|',\n\
         (SELECT CONVERT(nvarchar(max), STRING_AGG(CONVERT(nvarchar(max), CONCAT(FileName, N'#', PartNo, N':', \
         CONVERT(varchar(64), HASHBYTES('SHA2_256', BinaryData), 2))), N';') WITHIN GROUP (ORDER BY FileName, PartNo)) \
         FROM dbo.Params WHERE FileName LIKE N'DBNames%-Ext-%'), N'|',\n\
         (SELECT CONVERT(nvarchar(max), STRING_AGG(CONVERT(nvarchar(max), CONCAT(t.name, N'=', s.rows)), N';') \
         WITHIN GROUP (ORDER BY t.name)) FROM sys.tables t \
         JOIN (SELECT object_id, SUM(rows) AS rows FROM sys.partitions WHERE index_id IN (0, 1) GROUP BY object_id) s \
         ON s.object_id = t.object_id WHERE t.name LIKE N'%X1'));"
    )
    .unwrap();
    for table in [
        "_ExtensionsInfo",
        "_ExtensionsRestruct",
        "_ExtensionsRestructNGS",
    ] {
        writeln!(
            sql,
            "IF OBJECT_ID(N'dbo.{table}', N'U') IS NOT NULL BEGIN EXEC sp_executesql \
             N'SELECT @r = CONCAT(COUNT_BIG(*), N''/'', ISNULL(CHECKSUM_AGG(BINARY_CHECKSUM(*)), 0)) FROM dbo.{table}', \
             N'@r nvarchar(100) OUTPUT', @r = @ext_part OUTPUT; \
             SET @ext_{which} = CONCAT(@ext_{which}, N'|{table}=', @ext_part); END"
        )
        .unwrap();
    }
    sql
}

impl Plan {
    /// The whole restructure as T-SQL for another transaction: guards, the platform's statements, the
    /// assertions that stand in for the read-back, and the publication of `SchemaStorage`, `DBSchema`
    /// and `DBNames`. `now` is the T-SQL expression of the timestamp the `Params` rows get (the apply's
    /// `@now`). It does not touch `Config`, `ConfigSave` and the derived caches ([`Plan::caches`]).
    pub fn phase_sql(&self, now: &str) -> Result<String> {
        if self.method == Method::AlterAdd {
            bail!("the alter method is a research switch of the command, not of the apply");
        }
        let mut sql = String::new();
        writeln!(sql, "-- restructure: what the extensions keep, before").unwrap();
        writeln!(
            sql,
            "DECLARE @ext_before nvarchar(max), @ext_after nvarchar(max), @ext_part nvarchar(100);"
        )
        .unwrap();
        sql.push_str(&extension_state_sql("before"));
        writeln!(sql, "-- restructure: guards").unwrap();
        assert_that(
            &mut sql,
            &format!(
                "NOT EXISTS (SELECT 1 FROM dbo.SchemaStorage WHERE SchemaID = 0 AND Status = 100 \
                 AND DATALENGTH(NewGenCreated) = {len} AND DATALENGTH(NewGenDropped) = {len} \
                 AND CONVERT(varchar(64), HASHBYTES('SHA2_256', CurrentSchema), 2) = '{hash}')",
                len = EMPTY_GENERATION.len(),
                hash = self.old_schema_sha256.to_ascii_uppercase()
            ),
            code::SCHEMA_DRIFTED,
            "SchemaStorage is not idle or its schema is not the one the restructure was planned from",
        );
        assert_that(
            &mut sql,
            &format!(
                "(SELECT COUNT_BIG(*) FROM dbo.Params WHERE FileName = N'DBNames' AND PartNo = 0 \
                 AND DATALENGTH(BinaryData) = {} AND CONVERT(varchar(64), HASHBYTES('SHA2_256', BinaryData), 2) = '{}') <> 1",
                self.old_names_size,
                self.old_names_sha256.to_ascii_uppercase()
            ),
            code::NAMES_DRIFTED,
            "Params DBNames changed since the restructure was planned",
        );
        assert_that(
            &mut sql,
            "(SELECT COUNT_BIG(*) FROM dbo.Params WHERE FileName = N'DBNamesVersion-DBNames' AND PartNo = 0) <> 1",
            code::NAMES_DRIFTED,
            "Params DBNamesVersion-DBNames is missing",
        );
        for table in self.tables() {
            let name = &table.table.name;
            if table.create {
                assert_that(
                    &mut sql,
                    &format!(
                        "OBJECT_ID(N'dbo.{name}', N'U') IS NOT NULL OR OBJECT_ID(N'dbo.{name}NG', N'U') IS NOT NULL"
                    ),
                    code::TABLE_STATE,
                    &format!("the table {name} to create exists already, or {name}NG is left over"),
                );
                continue;
            }
            assert_that(
                &mut sql,
                &format!(
                    "OBJECT_ID(N'dbo.{name}', N'U') IS NULL OR OBJECT_ID(N'dbo.{name}NG', N'U') IS NOT NULL"
                ),
                code::TABLE_STATE,
                &format!("the table {name} is missing or {name}NG is left over"),
            );
        }

        writeln!(sql, "-- restructure: the new generation").unwrap();
        for table in self.tables() {
            writeln!(sql, "{}", create_table_sql(&table.table, "NG")).unwrap();
        }
        for object in &self.objects {
            for (index, table) in object.tables.iter().enumerate() {
                if table.create {
                    continue;
                }
                writeln!(
                    sql,
                    "INSERT INTO dbo.{new}NG WITH(TABLOCK) ({columns}) SELECT\n{values}\nFROM dbo.{new} T{alias} WITH(NOLOCK);",
                    new = table.table.name,
                    columns = table.insert_columns.join(", "),
                    values = table.insert_values.join(",\n"),
                    alias = index + 1
                )
                .unwrap();
            }
        }
        for table in self.tables().filter(|table| !table.create) {
            let name = &table.table.name;
            assert_that(
                &mut sql,
                &format!(
                    "(SELECT COUNT_BIG(*) FROM dbo.{name}) <> (SELECT COUNT_BIG(*) FROM dbo.{name}NG)"
                ),
                code::COPY_COUNT,
                &format!("the copy of {name} has another number of rows"),
            );
        }
        for table in self.tables() {
            for index in &table.table.indexes {
                writeln!(sql, "{}", create_index_sql(&table.table.name, index, "NG")).unwrap();
            }
        }
        for table in self.tables() {
            assert_structure(&mut sql, &table.table, "NG");
        }
        for table in self.tables().filter(|table| !table.create) {
            writeln!(sql, "drop table dbo.{};", table.table.name).unwrap();
        }
        for table in self.tables() {
            writeln!(
                sql,
                "EXEC sp_rename N'{name}NG', N'{name}', 'OBJECT';",
                name = table.table.name
            )
            .unwrap();
        }
        for table in self.tables() {
            for index in table
                .table
                .indexes
                .iter()
                .filter(|index| !index.name.is_empty())
            {
                writeln!(
                    sql,
                    "EXEC sp_rename N'{}.{}NG', N'{}', 'INDEX';",
                    table.table.name, index.name, index.name
                )
                .unwrap();
            }
        }
        for table in self.tables() {
            let name = &table.table.name;
            assert_that(
                &mut sql,
                &format!("OBJECT_ID(N'dbo.{name}NG', N'U') IS NOT NULL"),
                code::TABLE_STATE,
                &format!("{name}NG is left behind"),
            );
            assert_structure(&mut sql, &table.table, "");
            if table.create {
                assert_that(
                    &mut sql,
                    &format!("EXISTS (SELECT 1 FROM dbo.{name})"),
                    code::COPY_COUNT,
                    &format!("the created table {name} is not empty"),
                );
            }
        }

        writeln!(sql, "-- restructure: publication").unwrap();
        writeln!(
            sql,
            "DECLARE @ddl_schema varbinary(max) = 0x{};",
            hex_upper(&self.new_schema)
        )
        .unwrap();
        writeln!(
            sql,
            "UPDATE dbo.SchemaStorage SET Status = 100, CurrentSchema = @ddl_schema, NewGenCreated = 0x{empty}, NewGenDropped = 0x{empty} WHERE SchemaID = 0;",
            empty = hex_upper(EMPTY_GENERATION)
        )
        .unwrap();
        assert_that(
            &mut sql,
            "@@ROWCOUNT <> 1",
            code::PUBLISH,
            "SchemaStorage was not rewritten",
        );
        writeln!(sql, "UPDATE dbo.DBSchema SET SerializedData = @ddl_schema;").unwrap();
        assert_that(
            &mut sql,
            "@@ROWCOUNT <> 1",
            code::PUBLISH,
            "DBSchema was not rewritten",
        );
        for (name, row) in [
            ("DBNames", &self.new_names_row),
            ("DBNamesVersion-DBNames", &self.names_version_row),
        ] {
            writeln!(
                sql,
                "UPDATE dbo.Params SET BinaryData = 0x{}, DataSize = {}, Modified = {now} WHERE FileName = N'{name}' AND PartNo = 0;",
                hex_upper(row),
                row.len()
            )
            .unwrap();
            assert_that(
                &mut sql,
                "@@ROWCOUNT <> 1",
                code::PUBLISH,
                &format!("Params {name} was not rewritten"),
            );
        }
        assert_that(
            &mut sql,
            "(SELECT DATALENGTH(SerializedData) FROM dbo.DBSchema) <> DATALENGTH(@ddl_schema)",
            code::PUBLISH,
            "DBSchema and SchemaStorage.CurrentSchema differ",
        );
        writeln!(sql, "-- restructure: what the extensions keep, after").unwrap();
        sql.push_str(&extension_state_sql("after"));
        assert_that(
            &mut sql,
            "@ext_after <> @ext_before",
            code::EXTENSIONS_CHANGED,
            "the restructure changed what the extensions keep",
        );
        Ok(sql)
    }
}
