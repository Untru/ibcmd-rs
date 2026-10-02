//! `ibcmd-rs objects`: what the editor server (`serve --stdio`) does with the
//! objects of a configuration, from the command line -- the tree, the
//! comparison with a source folder, one file as the export writes it, the
//! export of selected objects into a source folder. The same library calls
//! (`stored_objects::StoredConfiguration`) and the same answers as the
//! server's methods `tree/children`, `objects/status`, `source/read` and
//! `objects/export`; the server's `objects/import` and `config/pending` are
//! `mssql-stage-source-objects --path-prefix` and `mssql-apply-check`.

use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::{Args, Subcommand};
use serde_json::json;

use crate::platform::PlatformSpec;
use crate::settings::{DatabaseTarget, PlatformHint, Settings, resolve_platform};
use crate::sql::{SqlExec, SqlOptions};
use crate::stored_objects::{ConfigSource, StoredConfiguration};

#[derive(Debug, Args)]
pub struct ObjectsArgs {
    #[command(subcommand)]
    pub command: ObjectsCommand,
}

#[derive(Debug, Subcommand)]
pub enum ObjectsCommand {
    /// Print the children of a node of the configuration tree (JSON), as the
    /// server's `tree/children` answers: no --node for the configuration,
    /// `Configuration` for its kinds, `Configuration/Catalogs` for a kind's
    /// objects, `Catalog.X` for an object's groups, `Catalog.X/Forms`.
    Tree(ObjectsTreeArgs),
    /// Compare objects (default: the whole configuration) with a source
    /// folder, file by file (JSON), as the server's `objects/status`.
    Status(ObjectsStatusArgs),
    /// Print one file as the export writes it, nothing written to disk, as
    /// the server's `source/read`.
    Read(ObjectsReadArgs),
    /// Export objects into a source folder: their files written, the files of
    /// theirs the export no longer writes removed, nothing else touched
    /// (JSON report), as the server's `objects/export`.
    Export(ObjectsExportArgs),
}

/// Where the Config table is read from.
#[derive(Debug, Args)]
pub struct ObjectsSourceArgs {
    /// Read the Config table from a folder of `<FileName>__part<N>.bin`
    /// files instead of SQL Server, as `mssql-dump-config --rows-dir`.
    #[arg(long)]
    pub rows_dir: Option<PathBuf>,
    /// SQL Server name.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// SQL Server database name (not needed with --rows-dir).
    #[arg(long, default_value = "")]
    pub database: String,
    /// SQL Server login. Uses Windows (integrated) authentication when omitted.
    #[arg(long)]
    pub sql_user: Option<String>,
    /// SQL Server password. Prefer --sql-pwd-env for shell history.
    #[arg(long)]
    pub sql_pwd: Option<String>,
    /// Environment variable containing the SQL Server password.
    #[arg(long, default_value = "IBCMD_DB_PSW")]
    pub sql_pwd_env: String,
    /// Platform the XML is for: a release (8.3.27, 8.5.1) or an exact build;
    /// by default the settings' (`ibcmd-rs.toml`), else 8.3.27.
    #[arg(long, value_name = "VERSION", value_parser = crate::platform::parse_flag)]
    pub platform: Option<PlatformSpec>,
}

#[derive(Debug, Args)]
pub struct ObjectsTreeArgs {
    #[command(flatten)]
    pub source: ObjectsSourceArgs,
    /// The node whose children to print.
    #[arg(long)]
    pub node: Option<String>,
}

#[derive(Debug, Args)]
pub struct ObjectsStatusArgs {
    #[command(flatten)]
    pub source: ObjectsSourceArgs,
    /// The source folder (an export of the configuration).
    #[arg(long)]
    pub source_dir: PathBuf,
    /// Compare only this object (full name, `Catalog.Банки`). Can be repeated.
    #[arg(long = "object", value_name = "FULL_NAME")]
    pub objects: Vec<String>,
}

#[derive(Debug, Args)]
pub struct ObjectsReadArgs {
    #[command(flatten)]
    pub source: ObjectsSourceArgs,
    /// The file, relative to the root of an export
    /// (`CommonModules/X/Ext/Module.bsl`).
    #[arg(long)]
    pub path: String,
    /// Write the file here instead of stdout.
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct ObjectsExportArgs {
    #[command(flatten)]
    pub source: ObjectsSourceArgs,
    /// The source folder to export into (created when absent).
    #[arg(long)]
    pub target_dir: PathBuf,
    /// The object to export (full name, `Catalog.Банки`, `Configuration`).
    /// Can be repeated.
    #[arg(long = "object", value_name = "FULL_NAME", required = true)]
    pub objects: Vec<String>,
}

fn open(args: &ObjectsSourceArgs) -> Result<StoredConfiguration> {
    let platform = match args.platform {
        Some(platform) => platform,
        None => {
            let settings = Settings::load(None)?;
            let server = args.rows_dir.is_none().then_some(args.server.as_str());
            resolve_platform(
                None,
                &settings,
                DatabaseTarget::new(server, &args.database),
                PlatformHint::None,
            )?
        }
    };
    let source = match &args.rows_dir {
        Some(dir) => ConfigSource::RowsDir(dir.clone()),
        None => {
            if args.database.trim().is_empty() {
                bail!("--database is required unless --rows-dir is given");
            }
            let password = args.sql_user.as_ref().and_then(|_| {
                args.sql_pwd
                    .clone()
                    .filter(|value| !value.is_empty())
                    .or_else(|| std::env::var(&args.sql_pwd_env).ok())
            });
            ConfigSource::Database {
                sql: SqlExec::from_options(SqlOptions {
                    sqlcmd: None,
                    bcp: None,
                    server: &args.server,
                    user: args.sql_user.as_deref(),
                    password: password.as_deref(),
                    password_env: &args.sql_pwd_env,
                    trust_server_certificate: true,
                })?,
                database: args.database.clone(),
            }
        }
    };
    Ok(StoredConfiguration::new(source, platform))
}

/// Runs one `objects` command.
pub fn run(args: ObjectsArgs) -> Result<()> {
    let quiet = |_: usize, _: usize, _: &str| {};
    match args.command {
        ObjectsCommand::Tree(args) => {
            let nodes = open(&args.source)?.children(args.node.as_deref())?;
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({ "nodes": nodes }))?
            );
        }
        ObjectsCommand::Status(args) => {
            let names = (!args.objects.is_empty()).then_some(args.objects.as_slice());
            let statuses = open(&args.source)?.status(names, &args.source_dir, &quiet)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({ "objects": statuses }))?
            );
        }
        ObjectsCommand::Read(args) => {
            let (exported, path) = open(&args.source)?.read_file(&args.path)?;
            let Some(bytes) = exported.files.get(&path) else {
                bail!(
                    "the export of {} writes no file {path}",
                    exported.object.full_name
                );
            };
            match &args.output {
                Some(output) => std::fs::write(output, bytes)
                    .with_context(|| format!("failed to write {}", output.display()))?,
                None => std::io::stdout().write_all(bytes)?,
            }
        }
        ObjectsCommand::Export(args) => {
            let writes =
                open(&args.source)?.export_into(&args.objects, &args.target_dir, &quiet)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({ "objects": writes }))?
            );
        }
    }
    Ok(())
}
