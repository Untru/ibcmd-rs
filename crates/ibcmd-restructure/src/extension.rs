//! Additive extension schema planning. No database access or metadata-slot guessing.

use crate::brace::Brace;
use crate::names::DbNames;
use crate::schema::{
    Column, DbSchema, PhysicalTable, SqlType, TableView, create_index_sql, create_table_sql,
    physical_tables,
};
use anyhow::{Context, Result, bail};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

/// An ordered allocation requested by a typed metadata adapter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NameRequest {
    pub uuid: String,
    pub kind: String,
}

/// Allocate above *all* main/extension headers, retaining every existing mapping.
pub fn allocate_names(
    before: &DbNames,
    headers: &[u64],
    requests: &[NameRequest],
) -> Result<DbNames> {
    let mut after = before.clone();
    let mut next = headers
        .iter()
        .copied()
        .chain(std::iter::once(before.max))
        .max()
        .unwrap_or(0);
    let mut seen = BTreeSet::new();
    for request in requests {
        if !is_uuid(&request.uuid)
            || !matches!(
                request.kind.as_str(),
                "Reference"
                    | "Document"
                    | "Enum"
                    | "InfoRg"
                    | "Fld"
                    | "VT"
                    | "LineNo"
                    | "ScheduledJobs"
            )
        {
            bail!(
                "invalid extension name allocation {}:{}",
                request.uuid,
                request.kind
            );
        }
        let key = (request.uuid.to_ascii_lowercase(), request.kind.clone());
        if !seen.insert(key) || before.number_of(&request.uuid, &request.kind).is_some() {
            bail!(
                "extension name allocation already exists {}:{}",
                request.uuid,
                request.kind
            );
        }
        next = next
            .checked_add(1)
            .context("extension DBNames counter overflow")?;
        after.append(&request.uuid, &request.kind, next)?;
    }
    Ok(after)
}

/// Native allocation passes can reserve a companion number for each created catalog.
#[derive(Clone, Debug)]
pub enum NameStep {
    Allocate(NameRequest),
    ReserveCatalog,
}

/// The target mappings and the shared extension header advance separately.
pub fn allocate_extension_names(
    target: &DbNames,
    system: &DbNames,
    headers: &[u64],
    steps: &[NameStep],
) -> Result<(DbNames, DbNames)> {
    let mut target = target.clone();
    let mut system = system.clone();
    let mut counter = headers
        .iter()
        .copied()
        .chain([target.max, system.max])
        .max()
        .unwrap_or(0);
    for step in steps {
        counter = counter
            .checked_add(1)
            .context("extension DBNames counter overflow")?;
        match step {
            NameStep::Allocate(request) => {
                if !is_uuid(&request.uuid)
                    || !matches!(
                        request.kind.as_str(),
                        "Reference"
                            | "Document"
                            | "Enum"
                            | "InfoRg"
                            | "Fld"
                            | "VT"
                            | "LineNo"
                            | "ScheduledJobs"
                    )
                    || target.number_of(&request.uuid, &request.kind).is_some()
                {
                    bail!(
                        "invalid or duplicate extension allocation {}:{}",
                        request.uuid,
                        request.kind
                    );
                }
                target.append(&request.uuid, &request.kind, counter)?;
            }
            NameStep::ReserveCatalog => system.max = counter,
        }
    }
    Ok((target, system))
}

fn is_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(i, b)| match i {
            8 | 13 | 18 | 23 => b == b'-',
            _ => b.is_ascii_hexdigit(),
        })
}

/// One generated table entry. `before` is absent for a genuinely new object.
#[derive(Clone, Debug)]
pub struct TableChange {
    pub before: Option<Brace>,
    pub after: Brace,
}

/// A physical table created empty, or rebuilt with all prior columns retained.
#[derive(Clone, Debug)]
pub struct TablePlan {
    pub before: Option<PhysicalTable>,
    pub after: PhysicalTable,
}

/// A plan changes only the supplied entries, leaving all others byte-for-byte intact.
#[derive(Clone, Debug)]
pub struct SchemaPlan {
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub tables: Vec<TablePlan>,
}

impl SchemaPlan {
    pub fn build(before: &[u8], changes: &[TableChange]) -> Result<Self> {
        let mut schema = DbSchema::parse(before)?;
        let mut names = BTreeSet::new();
        let mut tables = Vec::new();
        for change in changes {
            let next = TableView::new(&change.after)?;
            if !names.insert(next.name().to_owned()) {
                bail!("duplicate extension table {}", next.name());
            }
            let new_tables = physical_tables(&next)?;
            let old_tables = if let Some(old) = &change.before {
                let old_view = TableView::new(old)?;
                if old_view.name() != next.name() {
                    bail!("an extension table cannot be renamed");
                }
                let pos = schema
                    .position(next.name())
                    .with_context(|| format!("extension schema lacks {}", next.name()))?;
                if &schema.tables()[pos] != old {
                    bail!(
                        "extension schema entry {} changed before planning",
                        next.name()
                    );
                }
                schema.remove(next.name())?;
                physical_tables(&old_view)?
            } else {
                if schema.position(next.name()).is_some() {
                    bail!("new extension table {} already exists", next.name());
                }
                Vec::new()
            };
            let mut old: BTreeMap<_, _> = old_tables
                .into_iter()
                .map(|t| (t.name.clone(), t))
                .collect();
            for new in new_tables {
                let prior = old.remove(&new.name);
                validate_addition(prior.as_ref(), &new)?;
                if prior.as_ref() != Some(&new) {
                    tables.push(TablePlan {
                        before: prior,
                        after: new,
                    });
                }
            }
            if !old.is_empty() {
                bail!("extension migration removes existing tables");
            }
            schema.insert_rebuilt(change.after.clone());
        }
        Ok(Self {
            before: before.to_vec(),
            after: schema.to_text(),
            tables,
        })
    }

    /// DDL/data work for the caller's transaction; metadata publication follows.
    /// Every table uses the measured extension namespace X1.
    pub fn ddl(&self) -> Result<String> {
        let mut sql = String::new();
        for table in &self.tables {
            sql.push_str(&table.preflight());
        }
        for table in &self.tables {
            sql.push_str(&table.ddl()?);
        }
        Ok(sql)
    }
}

fn safe_identifier(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 116
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        bail!("unsafe generated SQL identifier {value:?}");
    }
    Ok(())
}

fn validate_addition(before: Option<&PhysicalTable>, after: &PhysicalTable) -> Result<()> {
    safe_identifier(&after.name)?;
    let mut column_names = BTreeSet::new();
    for column in &after.columns {
        if !column_names.insert(column.name.to_ascii_lowercase()) {
            bail!("duplicate SQL column {}", column.name);
        }
        safe_identifier(&column.name)?;
        if column.identity {
            bail!("extension identity columns are not supported");
        }
    }
    for index in &after.indexes {
        safe_identifier(&index.name)?;
        for name in &index.columns {
            safe_identifier(name)?;
        }
    }
    if let Some(before) = before {
        let old_names: BTreeSet<_> = before.columns.iter().map(|c| c.name.as_str()).collect();
        let retained: Vec<_> = after
            .columns
            .iter()
            .filter(|c| old_names.contains(c.name.as_str()))
            .collect();
        if retained != before.columns.iter().collect::<Vec<_>>() {
            bail!(
                "extension migration removes, reorders or changes existing columns of {}",
                after.name
            );
        }
        for column in after
            .columns
            .iter()
            .filter(|c| !old_names.contains(c.name.as_str()))
        {
            default_value(column)?;
        }
        if before.inline_pk != after.inline_pk
            || !before
                .indexes
                .iter()
                .all(|index| after.indexes.contains(index))
        {
            bail!(
                "extension migration changes existing indexes of {}",
                after.name
            );
        }
    }
    Ok(())
}

fn default_value(column: &Column) -> Result<String> {
    if column.nullable {
        return Ok("NULL".into());
    }
    Ok(match &column.sql_type {
        SqlType::Binary(n) => format!("CONVERT(binary({n}),0x)"),
        SqlType::VarBinary(_) | SqlType::VarBinaryMax => "0x".into(),
        SqlType::Numeric(_, _) | SqlType::Int | SqlType::BigInt => "0".into(),
        SqlType::DateTime2 => "CONVERT(datetime2(0),'20010101',112)".into(),
        SqlType::NVarChar(_) | SqlType::NVarCharMax | SqlType::NChar(_) => "N''".into(),
        SqlType::Timestamp => bail!("a new timestamp column cannot be filled from data"),
    })
}

impl TablePlan {
    fn preflight(&self) -> String {
        let mut sql = String::new();
        let target = format!("{}X1", self.after.name);
        let temporary = format!("{}X1NG", self.after.name);
        if self.before.is_none() {
            sql.push_str(&format!("IF OBJECT_ID(N'dbo.{target}',N'U') IS NOT NULL OR OBJECT_ID(N'dbo.{temporary}',N'U') IS NOT NULL THROW 57501,N'New extension table already exists',1;\n"));
        } else {
            sql.push_str(&format!("IF OBJECT_ID(N'dbo.{target}',N'U') IS NULL OR OBJECT_ID(N'dbo.{temporary}',N'U') IS NOT NULL THROW 57501,N'Extension rebuild table state changed',1;\n"));
            sql.push_str(&format!("IF EXISTS(SELECT 1 FROM sys.triggers WHERE parent_id=OBJECT_ID(N'dbo.{target}')) OR EXISTS(SELECT 1 FROM sys.foreign_keys WHERE parent_object_id=OBJECT_ID(N'dbo.{target}') OR referenced_object_id=OBJECT_ID(N'dbo.{target}')) OR EXISTS(SELECT 1 FROM sys.check_constraints WHERE parent_object_id=OBJECT_ID(N'dbo.{target}')) OR EXISTS(SELECT 1 FROM sys.default_constraints WHERE parent_object_id=OBJECT_ID(N'dbo.{target}')) OR EXISTS(SELECT 1 FROM sys.database_permissions WHERE class=1 AND major_id=OBJECT_ID(N'dbo.{target}')) OR EXISTS(SELECT 1 FROM sys.extended_properties WHERE class=1 AND major_id=OBJECT_ID(N'dbo.{target}')) OR EXISTS(SELECT 1 FROM sys.indexes WHERE object_id=OBJECT_ID(N'dbo.{target}') AND (has_filter=1 OR is_hypothetical=1 OR is_disabled=1 OR type NOT IN(0,1,2))) OR EXISTS(SELECT 1 FROM sys.index_columns WHERE object_id=OBJECT_ID(N'dbo.{target}') AND (is_included_column=1 OR is_descending_key=1)) OR EXISTS(SELECT 1 FROM sys.columns WHERE object_id=OBJECT_ID(N'dbo.{target}') AND (is_computed=1 OR is_identity=1 OR is_sparse=1 OR is_column_set=1 OR is_rowguidcol=1 OR is_hidden=1 OR generated_always_type<>0 OR encryption_type IS NOT NULL OR is_masked=1 OR (collation_name IS NOT NULL AND collation_name<>CONVERT(nvarchar(128),DATABASEPROPERTYEX(DB_NAME(),'Collation'))))) OR EXISTS(SELECT 1 FROM sys.tables WHERE object_id=OBJECT_ID(N'dbo.{target}') AND temporal_type<>0) OR EXISTS(SELECT 1 FROM sys.partitions WHERE object_id=OBJECT_ID(N'dbo.{target}') AND data_compression<>0) THROW 57507,N'Unmodeled SQL constraints, permissions, triggers or indexes on extension table',1;\n"));
        }
        if let Some(old) = &self.before {
            assert_structure(&mut sql, old, "X1");
        }
        sql
    }

    fn ddl(&self) -> Result<String> {
        let mut sql = String::new();
        let suffix = if self.before.is_some() { "X1NG" } else { "X1" };
        let target = format!("{}X1", self.after.name);
        let temporary = format!("{}{suffix}", self.after.name);
        sql.push_str(&create_table_sql(&self.after, suffix));
        sql.push('\n');
        for index in &self.after.indexes {
            sql.push_str(&create_index_sql(&self.after.name, index, suffix));
            sql.push('\n');
        }
        assert_structure(&mut sql, &self.after, suffix);
        if let Some(old) = &self.before {
            let prior: BTreeSet<_> = old.columns.iter().map(|c| c.name.as_str()).collect();
            let columns: Vec<_> = self
                .after
                .columns
                .iter()
                .filter(|c| c.sql_type != SqlType::Timestamp)
                .collect();
            let names = columns
                .iter()
                .map(|c| format!("[{}]", c.name))
                .collect::<Vec<_>>()
                .join(",");
            let values = columns
                .iter()
                .map(|c| {
                    if prior.contains(c.name.as_str()) {
                        Ok(format!("[{}]", c.name))
                    } else {
                        default_value(c)
                    }
                })
                .collect::<Result<Vec<_>>>()?
                .join(",");
            sql.push_str(&format!("INSERT dbo.{temporary} ({names}) SELECT {values} FROM dbo.{target} WITH(TABLOCKX,HOLDLOCK);\nIF (SELECT COUNT_BIG(*) FROM dbo.{temporary})<>(SELECT COUNT_BIG(*) FROM dbo.{target}) THROW 57502,N'Extension data copy count differs',1;\n"));
            let retained = old
                .columns
                .iter()
                .filter(|c| c.sql_type != SqlType::Timestamp)
                .map(|c| format!("[{}]", c.name))
                .collect::<Vec<_>>()
                .join(",");
            if !retained.is_empty() {
                sql.push_str(&format!("IF EXISTS(SELECT {retained} FROM dbo.{target} EXCEPT SELECT {retained} FROM dbo.{temporary}) OR EXISTS(SELECT {retained} FROM dbo.{temporary} EXCEPT SELECT {retained} FROM dbo.{target}) THROW 57503,N'Extension data copy contents differ',1;\n"));
            }
            sql.push_str(&format!("DROP TABLE dbo.{target}; EXEC sys.sp_rename N'dbo.{temporary}',N'{target}',N'OBJECT';\n"));
            for index in &self.after.indexes {
                sql.push_str(&format!(
                    "EXEC sys.sp_rename N'dbo.{target}.{}{suffix}',N'{}X1',N'INDEX';\n",
                    index.name, index.name
                ));
            }
        }
        Ok(sql)
    }
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
        57504,
        &format!("{object} lacks the index {name} ({columns})"),
    );
}

/// The assertions that the columns and the indexes of `dbo.<table><suffix>` are the model's.
pub fn assert_structure(sql: &mut String, table: &PhysicalTable, suffix: &str) {
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
        57504,
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
        57504,
        &format!("{object} has indexes the model does not"),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brace_list;
    use crate::schema::{FieldEntry, IndexEntry, TypeEntry};

    const UUID: &str = "aaaaaaaa-0000-4000-8000-000000000001";
    fn names(max: u64) -> DbNames {
        DbNames {
            max,
            entries: vec![],
        }
    }
    fn table(name: &str, fields: Vec<FieldEntry>) -> Brace {
        let mut f = vec![Brace::atom(fields.len())];
        f.extend(fields.into_iter().map(|f| f.to_brace()));
        brace_list![
            Brace::str(name),
            Brace::str("N"),
            Brace::num(1),
            Brace::str(""),
            Brace::List(f),
            brace_list![Brace::num(0)],
            brace_list![
                Brace::num(1),
                IndexEntry {
                    name: "ByDims".into(),
                    unique: true,
                    fields: vec!["Fld1".into()],
                    clustered: true
                }
                .to_brace()
            ],
            Brace::num(1),
            Brace::str("S"),
            brace_list![Brace::num(0)],
            brace_list![Brace::num(0)],
            Brace::str(""),
            Brace::num(0),
            Brace::num(0)
        ]
    }
    fn field(name: &str, len: u64) -> FieldEntry {
        FieldEntry::new(
            name,
            false,
            vec![TypeEntry::new("S", 0x8000_0000 | len, 0, "", 0)],
        )
    }

    #[test]
    fn catalog_reservations_advance_only_shared_header_and_do_not_reuse_numbers() {
        let target = names(20);
        let system = names(40);
        let steps = [
            NameStep::Allocate(NameRequest {
                uuid: UUID.into(),
                kind: "Reference".into(),
            }),
            NameStep::ReserveCatalog,
            NameStep::ReserveCatalog,
            NameStep::Allocate(NameRequest {
                uuid: UUID.into(),
                kind: "Fld".into(),
            }),
        ];
        let (target_after, system_after) =
            allocate_extension_names(&target, &system, &[99], &steps).unwrap();
        assert_eq!(target_after.max, 103);
        assert_eq!(
            target_after
                .entries
                .iter()
                .map(|e| e.number)
                .collect::<Vec<_>>(),
            [100, 103]
        );
        assert_eq!(
            system_after,
            DbNames {
                max: 102,
                entries: vec![]
            }
        );
        assert_eq!(target, names(20));
        assert_eq!(system, names(40));
    }

    #[test]
    fn duplicate_allocations_and_counter_overflow_are_refused() {
        let a = NameStep::Allocate(NameRequest {
            uuid: UUID.into(),
            kind: "Fld".into(),
        });
        assert!(
            allocate_extension_names(&names(1), &names(2), &[], &[a.clone(), a.clone()]).is_err()
        );
        assert!(allocate_extension_names(&names(u64::MAX), &names(0), &[], &[a]).is_err());
    }

    #[test]
    fn new_tables_are_created_empty_and_unknown_old_entries_stay_intact() {
        let kept = table("InfoRg1", vec![field("Fld1", 40)]);
        let before = DbSchema::from_tables(vec![kept.clone()]).to_text();
        let next = table("InfoRg2", vec![field("Fld1", 80), field("Fld2", 100)]);
        let plan = SchemaPlan::build(
            &before,
            &[TableChange {
                before: None,
                after: next.clone(),
            }],
        )
        .unwrap();
        assert_eq!(
            DbSchema::parse(&plan.after).unwrap().tables(),
            &[kept, next]
        );
        let sql = plan.ddl().unwrap();
        assert!(sql.contains("create table dbo._InfoRg2X1"));
        assert!(sql.contains("INDEX _InfoRg2_1X1 ON dbo._InfoRg2X1"));
        assert!(!sql.contains("DROP TABLE"));
        assert!(!sql.contains("INSERT dbo"));
        assert_eq!(plan.before, before);
    }

    #[test]
    fn rebuild_keeps_prior_values_and_checks_both_copy_directions_before_drop() {
        let old = table("InfoRg1", vec![field("Fld1", 40)]);
        let next = table(
            "InfoRg1",
            vec![
                field("Fld1", 40),
                FieldEntry::new("Fld2", false, vec![TypeEntry::new("N", 15, 2, "", 0)]),
            ],
        );
        let before = DbSchema::from_tables(vec![old.clone()]).to_text();
        let plan = SchemaPlan::build(
            &before,
            &[TableChange {
                before: Some(old),
                after: next,
            }],
        )
        .unwrap();
        let sql = plan.ddl().unwrap();
        assert!(sql.contains("INSERT dbo._InfoRg1X1NG ([_Fld1],[_Fld2]) SELECT [_Fld1],0 FROM dbo._InfoRg1X1 WITH(TABLOCKX,HOLDLOCK)"));
        let check = sql.find("Extension data copy contents differ").unwrap();
        let drop = sql.find("DROP TABLE dbo._InfoRg1X1").unwrap();
        assert!(check < drop);
        assert!(sql.contains("FROM dbo._InfoRg1X1 EXCEPT SELECT [_Fld1] FROM dbo._InfoRg1X1NG"));
        assert!(sql.contains("FROM dbo._InfoRg1X1NG EXCEPT SELECT [_Fld1] FROM dbo._InfoRg1X1"));
    }

    #[test]
    fn destructive_edits_and_sql_identifier_injection_are_rejected_before_ddl() {
        let old = table("InfoRg1", vec![field("Fld1", 40)]);
        let before = DbSchema::from_tables(vec![old.clone()]).to_text();
        let changed = table("InfoRg1", vec![field("Fld1", 10)]);
        assert!(
            SchemaPlan::build(
                &before,
                &[TableChange {
                    before: Some(old.clone()),
                    after: changed
                }]
            )
            .is_err()
        );
        let removed = table("InfoRg1", vec![field("Fld2", 40)]);
        assert!(
            SchemaPlan::build(
                &before,
                &[TableChange {
                    before: Some(old),
                    after: removed
                }]
            )
            .is_err()
        );
        let malicious = table("InfoRg2;DROP_TABLE", vec![field("Fld1", 40)]);
        assert!(
            SchemaPlan::build(
                &before,
                &[TableChange {
                    before: None,
                    after: malicious
                }]
            )
            .is_err()
        );
    }
}
