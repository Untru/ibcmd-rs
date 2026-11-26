//! The database side of the plan's input: `SchemaStorage`, the `DBNames` rows, the file lists and the
//! descriptor rows of `Config` and `ConfigSave`.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, bail};

use crate::metadata_model::brace::parse_row;
use crate::mssql_config_apply::model::quote_ident;
use crate::restructure::caches::root::{class_of_kind, collections};
use crate::restructure::plan::{Inputs, StagedImage, configuration_uuid, row_bytes};
use crate::restructure::storage::SchemaStorageRow;
use crate::sql::mssql::{TdsConnection, sql_row};
use crate::sql::{SqlClient, SqlRow};

/// Where the plan's input is read from: a dedicated connection of the research command, or the client
/// of the own apply (the structural gate reads through the apply's handle).
pub trait RowSource {
    fn rows(&mut self, query: &str, each: &mut dyn FnMut(SqlRow) -> Result<()>) -> Result<()>;
}

impl RowSource for TdsConnection {
    fn rows(&mut self, query: &str, each: &mut dyn FnMut(SqlRow) -> Result<()>) -> Result<()> {
        self.query_each(query, &[], |row| each(sql_row(row)))
    }
}

/// The apply's SQL client on one database (`USE` in front of every query: the client is server-level).
pub struct ClientSource<'a> {
    pub client: &'a dyn SqlClient,
    pub database: &'a str,
}

impl RowSource for ClientSource<'_> {
    fn rows(&mut self, query: &str, each: &mut dyn FnMut(SqlRow) -> Result<()>) -> Result<()> {
        let database = quote_ident(self.database)?;
        self.client
            .read_rows(&format!("USE {database}; {query}"), &[], each)
    }
}

pub fn rows(
    source: &mut dyn RowSource,
    query: &str,
    mut each: impl FnMut(SqlRow) -> Result<()>,
) -> Result<()> {
    source.rows(query, &mut each)
}

/// Reads everything a plan needs, and the `SchemaStorage` rows.
pub fn read_inputs(connection: &mut dyn RowSource) -> Result<(Inputs, Vec<SchemaStorageRow>)> {
    let mut storage = Vec::new();
    rows(
        connection,
        "SELECT SchemaID, Status, CurrentSchema, NewGenCreated, NewGenDropped FROM dbo.SchemaStorage ORDER BY SchemaID",
        |mut row| {
            storage.push(SchemaStorageRow {
                schema_id: row.i64(0)?,
                status: row.i64(1)?,
                current_schema: row.take_binary(2)?,
                new_gen_created: row.take_binary(3)?,
                new_gen_dropped: row.take_binary(4)?,
            });
            Ok(())
        },
    )
    .context("SchemaStorage")?;
    let main = storage
        .iter()
        .find(|row| row.schema_id == 0)
        .context("SchemaStorage has no row for the main configuration")?;

    let mut inputs = Inputs {
        schema: main.current_schema.clone(),
        ..Inputs::default()
    };
    let mut have_main_names = false;
    rows(
        connection,
        "SELECT FileName, BinaryData FROM dbo.Params WHERE PartNo = 0 AND (FileName = N'DBNames' OR FileName LIKE N'DBNames-Ext-%')",
        |mut row| {
            let name = row.take_text(0)?;
            let data = row.take_binary(1)?;
            if name == "DBNames" {
                inputs.main_names = data;
                have_main_names = true;
            } else {
                inputs.extension_names.push((name, data));
            }
            Ok(())
        },
    )
    .context("Params DBNames")?;
    if !have_main_names {
        bail!("Params has no DBNames row");
    }
    rows(
        connection,
        "SELECT t.name FROM sys.tables t JOIN sys.partitions p ON p.object_id = t.object_id AND p.index_id IN (0, 1)          WHERE t.name LIKE N'[_]RefSInf%' GROUP BY t.name HAVING SUM(p.rows) > 0",
        |mut row| {
            let name = row.take_text(0)?;
            inputs
                .predefined_tables
                .insert(name.trim_start_matches('_').to_owned());
            Ok(())
        },
    )
    .context("RefSInf tables")?;
    rows(
        connection,
        "SELECT FileName, BinaryData, CONVERT(bigint, DataSize) FROM dbo.Params WHERE PartNo = 0 AND (FileName LIKE N'%.si' OR FileName = N'siVersions')",
        |mut row| {
            let name = row.take_text(0)?;
            let size = row.i64(2)?;
            inputs.cache_sizes.insert(name.clone(), size);
            inputs.cache_rows.push((name, row.take_binary(1)?));
            Ok(())
        },
    )
    .context("Params *.si")?;
    rows(
        connection,
        "SELECT BinaryData FROM dbo.Config WHERE FileName = N'root' AND PartNo = 0",
        |mut row| {
            inputs.root_row = row.take_binary(0)?;
            Ok(())
        },
    )
    .context("Config root")?;
    inputs.staged = read_staged(connection, &inputs.root_row)?;
    if !crate::restructure::create::new_descriptor_names(&inputs.staged).is_empty() {
        inputs.objects = read_objects(connection, &inputs.root_row, &inputs.staged)?;
    }
    inputs.extensions = crate::restructure::extensions::read_state(connection, &storage)
        .context("the extensions")?;
    Ok((inputs, storage))
}

/// The staged image against the stored one: the file lists of both, the descriptors of the staged
/// image, and of the stored one only those the stage replaces, the configuration's own (which lists
/// the objects in the order the platform walks them) and the catalogs and documents it lists (the caches of
/// a tabular section traverse the sections of every object of the kind).
fn read_staged(connection: &mut dyn RowSource, root_row: &[u8]) -> Result<StagedImage> {
    let mut image = StagedImage::default();
    let stored_filter = match configuration_uuid(root_row) {
        Ok(uuid) => format!(
            " AND (FileName IN (SELECT FileName FROM dbo.ConfigSave) OR FileName = N'{uuid}')"
        ),
        // No usable root row: read them all (the plan refuses when it needs the configuration).
        Err(_) => String::new(),
    };
    for (table, files, descriptors, filter) in [
        (
            "Config",
            &mut image.old_files,
            &mut image.old_descriptors,
            stored_filter.as_str(),
        ),
        (
            "ConfigSave",
            &mut image.new_files,
            &mut image.new_descriptors,
            "",
        ),
    ] {
        collect_files(connection, table, files)?;
        collect_descriptors(connection, table, filter, descriptors)?;
    }
    collect_listed_objects(connection, root_row, &mut image)?;
    rows(
        connection,
        "SELECT BinaryData FROM dbo.ConfigSave WHERE FileName = N'deleted' AND PartNo = 0",
        |mut row| {
            image.deleted = Some(row.take_binary(0)?);
            Ok(())
        },
    )
    .context("ConfigSave deleted")?;
    Ok(image)
}

/// The stored descriptors a created object's plan reads (see [`Inputs::objects`]).
fn read_objects(
    connection: &mut dyn RowSource,
    root_row: &[u8],
    staged: &StagedImage,
) -> Result<BTreeMap<String, Vec<u8>>> {
    let uuid = configuration_uuid(root_row)?;
    let configuration = staged
        .old_descriptors
        .get(&uuid)
        .context("the stored configuration's descriptor was not read")?;
    let wanted = crate::restructure::create::context_uuids(configuration)?;
    let mut out = BTreeMap::new();
    for chunk in wanted.chunks(200) {
        let list = chunk
            .iter()
            .map(|uuid| format!("N'{uuid}'"))
            .collect::<Vec<_>>()
            .join(",");
        rows(
            connection,
            &format!(
                "SELECT FileName, BinaryData FROM dbo.Config WHERE PartNo = 0 AND FileName IN ({list})"
            ),
            |mut row| {
                let name = row.take_text(0)?;
                out.insert(name, row.take_binary(1)?);
                Ok(())
            },
        )
        .context("Config descriptors of the objects")?;
    }
    Ok(out)
}

/// The stored descriptors of the catalogs and documents the configuration's own descriptor lists that are not
/// read yet. A configuration that cannot be read adds nothing (the plan refuses when it needs the listing).
fn collect_listed_objects(
    connection: &mut dyn RowSource,
    root_row: &[u8],
    image: &mut StagedImage,
) -> Result<()> {
    let Ok(configuration) = configuration_uuid(root_row) else {
        return Ok(());
    };
    let Some(stored) = image.old_descriptors.get(&configuration) else {
        return Ok(());
    };
    let Ok(tree) = parse_row(&row_bytes(stored)) else {
        return Ok(());
    };
    let classes: Vec<&str> = ["Catalog", "Document"]
        .into_iter()
        .filter_map(class_of_kind)
        .collect();
    let wanted: Vec<String> = collections(&tree)
        .into_iter()
        .filter(|collection| classes.contains(&collection.class.as_str()))
        .flat_map(|collection| collection.objects)
        .map(|uuid| uuid.to_ascii_lowercase())
        .filter(|uuid| {
            uuid.len() == 36
                && uuid.chars().all(|c| c == '-' || c.is_ascii_hexdigit())
                && !image.old_descriptors.contains_key(uuid)
        })
        .collect();
    for chunk in wanted.chunks(200) {
        let list = chunk
            .iter()
            .map(|uuid| format!("N'{uuid}'"))
            .collect::<Vec<_>>()
            .join(", ");
        rows(
            connection,
            &format!(
                "SELECT FileName, BinaryData FROM dbo.Config WHERE PartNo = 0 AND FileName IN ({list})"
            ),
            |mut row| {
                let name = row.take_text(0)?;
                image
                    .old_descriptors
                    .insert(name.to_ascii_lowercase(), row.take_binary(1)?);
                Ok(())
            },
        )
        .context("Config listed objects")?;
    }
    Ok(())
}

fn collect_files(
    connection: &mut dyn RowSource,
    table: &str,
    files: &mut BTreeSet<String>,
) -> Result<()> {
    rows(
        connection,
        &format!("SELECT FileName FROM dbo.{table}"),
        |mut row| {
            files.insert(row.take_text(0)?);
            Ok(())
        },
    )
    .with_context(|| format!("{table} file names"))
}

/// The rows named by a bare uuid: the object descriptors.
fn collect_descriptors(
    connection: &mut dyn RowSource,
    table: &str,
    filter: &str,
    descriptors: &mut BTreeMap<String, Vec<u8>>,
) -> Result<()> {
    rows(
        connection,
        &format!(
            "SELECT FileName, BinaryData FROM dbo.{table} WHERE PartNo = 0 AND LEN(FileName) = 36 AND FileName NOT LIKE N'%.%'{filter}"
        ),
        |mut row| {
            let name = row.take_text(0)?;
            descriptors.insert(name, row.take_binary(1)?);
            Ok(())
        },
    )
    .with_context(|| format!("{table} descriptors"))
}

/// Other user sessions in the current database (an exclusive restructure wants none).
pub fn other_sessions(connection: &mut TdsConnection) -> Result<i64> {
    let mut count = 0;
    rows(
        connection,
        "SELECT COUNT(*) FROM sys.dm_exec_sessions WHERE database_id = DB_ID() AND session_id <> @@SPID AND is_user_process = 1",
        |row| {
            count = row.i64(0)?;
            Ok(())
        },
    )
    .context("cannot count the sessions of the database (VIEW SERVER STATE?)")?;
    Ok(count)
}

/// The current database's name.
pub fn database_name(connection: &mut TdsConnection) -> Result<String> {
    let mut name = String::new();
    rows(connection, "SELECT DB_NAME()", |mut row| {
        name = row.take_text(0)?;
        Ok(())
    })?;
    Ok(name)
}
