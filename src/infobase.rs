//! `infobase config export`, `import` and `save` against a Microsoft SQL
//! Server infobase, without the platform: the export reads the Config table
//! and writes the XML tree (`mssql_dump`), the import stages the tree into
//! ConfigSave (`mssql::stage_source_objects`), where the platform's own
//! `config apply` finds it, as it finds what its own `config import` wrote,
//! and the save writes the rows themselves as a `.cf`
//! (`mssql_dump::config_save`).
//!
//! The drop-in command line (`crate::dropin`) and the research round trip
//! (`crate::infobase_oracle`, `platform-oracle` builds only) call these.

pub mod export_commands;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use ibcmd_core::version::{PlatformBuild, XmlDialect};
use serde::Serialize;
use serde_json::Value;
use walkdir::WalkDir;

use crate::adapters::mssql_legacy::MssqlLegacyAdapter;
use crate::cli::{
    InfobaseConfigExportArgs, InfobaseConfigFormat, InfobaseConfigImportArgs,
    InfobaseConfigLoadArgs, InfobaseConfigSaveArgs, InfobaseConfigSourceVersion,
    InfobaseImportStageMode, InfobaseImportVerify, MssqlDumpConfigArgs, MssqlDumpExtensionArgs,
    MssqlExtensionImage, MssqlStageSourceObjectsArgs,
};
use crate::legacy_version::LegacyVersionAxes;
use crate::platform::PlatformSpec;
use crate::settings::{DatabaseTarget, PlatformHint, SettingSource, Settings};

#[derive(Debug, Serialize)]
pub struct InfobaseConfigExportReport {
    pub operation: &'static str,
    pub backend: &'static str,
    pub format: &'static str,
    pub source_version: &'static str,
    pub dbms: String,
    pub db_server: String,
    pub db_name: String,
    pub db_user: Option<String>,
    pub password_source: Option<String>,
    /// The platform's ibcmd configuration file (`--config`), as given.
    pub native_config: Option<PathBuf>,
    pub output_dir: PathBuf,
    pub temp_dump_dir: PathBuf,
    /// Files of the written tree; counted only when asked (a walk of the
    /// whole tree: 140 709 files for ERP УХ).
    pub exported_files: Option<usize>,
    pub raw_rows: usize,
    pub metadata_xml_rows: usize,
    pub module_text_rows: usize,
    pub source_asset_rows: usize,
    pub dump_timings: crate::mssql_dump::MssqlDumpTimingReport,
    /// What `--base` and `--sync` did (absent without them).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub incremental: Option<crate::mssql_dump::incremental::IncrementalExportSummary>,
}

#[derive(Debug, Serialize)]
pub struct InfobaseConfigImportReport {
    pub operation: &'static str,
    pub backend: &'static str,
    pub format: &'static str,
    /// The XML version the tree was read as: the one given, else the tree's
    /// own (`Configuration.xml`).
    pub source_version: Option<String>,
    pub dbms: String,
    pub db_server: String,
    pub db_name: String,
    pub db_user: Option<String>,
    pub native_config: Option<PathBuf>,
    pub source_dir: PathBuf,
    /// `base-free` (every row compiled from the tree: the target holds none
    /// of its configuration) or `patch` (the target's own rows patched).
    pub stage_mode: &'static str,
    /// Why that mode: asked for, or what the target's Config holds.
    pub stage_mode_reason: String,
    /// `import files`: the files whose rows were staged (relative to
    /// `source_dir`); absent for the whole tree.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<String>,
    /// Rows in the target's Config before the import (-1: not read).
    pub target_config_rows: i64,
    pub staged_rows_before: i64,
    pub staged_rows_after: i64,
    pub scripts: Vec<PathBuf>,
    /// What the guard compared with the tree before the stage was written
    /// (absent when it did not run: a stage compiled from the tree, or
    /// `--no-verify`).
    pub verification: Option<crate::mssql::StageVerification>,
    /// What a patch stage built from the tree in place of the target's rows.
    pub overrides: Option<crate::mssql::StageOverrides>,
    /// The copy of a file infobase's data file kept before the write.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup: Option<PathBuf>,
}

/// The export refuses a directory that already holds files, as the
/// platform's own export does (`--force` does not change that).
#[derive(Debug)]
pub struct OutputDirectoryNotEmpty(pub PathBuf);

impl std::fmt::Display for OutputDirectoryNotEmpty {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "output directory is not empty: {}",
            self.0.display()
        )
    }
}

impl std::error::Error for OutputDirectoryNotEmpty {}

/// What an operation lets tell its platform when nothing names it (see
/// `crate::settings::PlatformHint`).
#[derive(Debug, Clone, Copy)]
pub(crate) enum PlatformNeed<'a> {
    /// The research round trip: the XML version given, else 2.20.
    Given,
    /// An export into `output_dir`: the settings, then the configuration's
    /// compatibility mode (read through the built-in client, or `sqlcmd`
    /// when given, and only when nothing above decides).
    Export {
        sqlcmd: Option<&'a Path>,
        output_dir: &'a Path,
        overwrite: bool,
        /// `--base`/`--sync`: the directory may hold an earlier export.
        incremental: bool,
    },
    /// An import of `source_dir`: the settings, then the tree's own
    /// `Configuration.xml`; a platform named for the database must be the
    /// tree's.
    Import { source_dir: &'a Path },
}

/// Everything a command was given to reach its database with.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ConnectionRequest<'a> {
    pub settings: Option<&'a Path>,
    pub native_config: Option<&'a Path>,
    pub format: Option<InfobaseConfigFormat>,
    /// `--platform`: its XML format, above everything else.
    pub platform: Option<PlatformSpec>,
    pub source_version: Option<InfobaseConfigSourceVersion>,
    /// `--db-path`: a file infobase; no DBMS value may come with it.
    pub db_path: Option<&'a Path>,
    pub dbms: Option<&'a str>,
    pub db_server: Option<&'a str>,
    pub db_name: Option<&'a str>,
    pub db_user: Option<&'a str>,
    pub db_pwd: Option<&'a str>,
    pub db_pwd_env: &'a str,
    pub need: PlatformNeed<'a>,
}

#[derive(Debug, Clone)]
pub(crate) struct ConnectionConfig {
    pub dbms: String,
    pub db_server: String,
    pub db_name: String,
    pub db_user: Option<String>,
    pub db_pwd: Option<String>,
    pub password_source: Option<String>,
    pub native_config: Option<PathBuf>,
    pub format: InfobaseConfigFormat,
    pub legacy_adapter: MssqlLegacyAdapter,
    /// Whether the XML version was named (command line, `--settings`, or
    /// the platform the settings give the database) rather than defaulted:
    /// an import reads an unnamed one off the tree.
    pub xml_version_given: bool,
    /// The data file of a file infobase (`--db-path`); `None` for a DBMS.
    pub file_infobase: Option<PathBuf>,
    /// Retained only for this resolve/export operation; clones share one-use state.
    pub postgres_snapshot: Option<crate::mssql_dump::PostgresExportSnapshot>,
}

impl ConnectionConfig {
    /// The PostgreSQL database of a `--dbms=PostgreSQL` connection (#369).
    pub(crate) fn postgres(&self) -> Result<Option<crate::sql::postgres::PgTarget>> {
        if !is_postgres(&self.dbms) {
            return Ok(None);
        }
        crate::sql::postgres::PgTarget::new(
            &self.db_server,
            &self.db_name,
            self.db_user.as_deref(),
            self.db_pwd.as_deref(),
        )
        .map(Some)
    }

    pub(crate) fn legacy_source_version(&self) -> Result<InfobaseConfigSourceVersion> {
        self.legacy_adapter.legacy_selector().ok_or_else(|| {
            anyhow!(
                "legacy MSSQL adapter does not support XML dialect {}",
                self.legacy_adapter.xml_dialect()
            )
        })
    }
}

impl InfobaseConfigExportArgs {
    fn connection(&self) -> ConnectionRequest<'_> {
        ConnectionRequest {
            settings: self.settings.as_deref(),
            native_config: self.native_config.as_deref(),
            format: self.format,
            platform: self.platform,
            source_version: self.source_version,
            db_path: self.db_path.as_deref(),
            dbms: self.dbms.as_deref(),
            db_server: self.db_server.as_deref(),
            db_name: self.db_name.as_deref(),
            db_user: self.db_user.as_deref(),
            db_pwd: self.db_pwd.as_deref(),
            db_pwd_env: &self.db_pwd_env,
            need: PlatformNeed::Export {
                sqlcmd: self.sqlcmd.as_deref(),
                output_dir: &self.output_dir,
                overwrite: self.overwrite,
                incremental: self.base.is_some() || self.sync,
            },
        }
    }
}

impl InfobaseConfigImportArgs {
    fn connection(&self) -> ConnectionRequest<'_> {
        ConnectionRequest {
            settings: self.settings.as_deref(),
            native_config: self.native_config.as_deref(),
            format: self.format,
            platform: self.platform,
            source_version: self.source_version,
            db_path: self.db_path.as_deref(),
            dbms: self.dbms.as_deref(),
            db_server: self.db_server.as_deref(),
            db_name: self.db_name.as_deref(),
            db_user: self.db_user.as_deref(),
            db_pwd: self.db_pwd.as_deref(),
            db_pwd_env: &self.db_pwd_env,
            need: PlatformNeed::Import {
                source_dir: &self.source_dir,
            },
        }
    }
}

/// What `infobase config save` did.
#[derive(Debug, Serialize)]
pub struct InfobaseConfigSaveReport {
    pub operation: &'static str,
    pub backend: &'static str,
    pub dbms: String,
    pub db_server: String,
    pub db_name: String,
    pub db_user: Option<String>,
    pub password_source: Option<String>,
    pub native_config: Option<PathBuf>,
    pub save: crate::mssql_dump::config_save::ConfigSaveReport,
}

impl InfobaseConfigSaveArgs {
    fn connection(&self) -> ConnectionRequest<'_> {
        ConnectionRequest {
            settings: self.settings.as_deref(),
            native_config: self.native_config.as_deref(),
            format: None,
            platform: None,
            source_version: None,
            db_path: self.db_path.as_deref(),
            dbms: self.dbms.as_deref(),
            db_server: self.db_server.as_deref(),
            db_name: self.db_name.as_deref(),
            db_user: self.db_user.as_deref(),
            db_pwd: self.db_pwd.as_deref(),
            db_pwd_env: &self.db_pwd_env,
            // A .cf holds the stored rows: no XML format is read or written.
            need: PlatformNeed::Given,
        }
    }
}

/// `infobase config save`: the configuration of the connection's database
/// written as a `.cf` from its rows (`mssql_dump::config_save`).
pub fn save_config(args: &InfobaseConfigSaveArgs) -> Result<InfobaseConfigSaveReport> {
    use crate::mssql_dump::config_save::{ConfigSaveRequest, SavedConfiguration, save_config};

    let config = resolve_connection(args.connection())?;
    if config.file_infobase.is_none() {
        ensure_mssql(&config.dbms)?;
    }
    let output = if args.output.is_absolute() {
        args.output.clone()
    } else {
        env::current_dir()?.join(&args.output)
    };
    // A file infobase answers every read from its 1Cv8.1CD; no login.
    let sql = if config.file_infobase.is_some() {
        crate::sql::SqlExec::detached("the rows come from a file infobase")
    } else {
        crate::sql::SqlExec::from_options(crate::sql::SqlOptions {
            sqlcmd: args.sqlcmd.as_deref(),
            bcp: None,
            server: &config.db_server,
            user: config.db_user.as_deref(),
            password: config.db_pwd.as_deref(),
            password_env: &args.db_pwd_env,
            trust_server_certificate: false,
        })?
    };
    let save = save_config(&ConfigSaveRequest {
        sql: &sql,
        database: &config.db_name,
        rows_dir: None,
        file_infobase: config.file_infobase.as_deref(),
        configuration: if args.database_configuration {
            SavedConfiguration::Database
        } else {
            SavedConfiguration::Main
        },
        output: &output,
        overwrite: args.overwrite,
    })?;
    Ok(InfobaseConfigSaveReport {
        operation: "infobase config save",
        backend: if config.file_infobase.is_some() {
            "file-infobase-config-rows"
        } else {
            "mssql-config-rows"
        },
        dbms: config.dbms,
        db_server: config.db_server,
        db_name: config.db_name,
        db_user: config.db_user,
        password_source: config.password_source,
        native_config: config.native_config,
        save,
    })
}

pub fn export_config(args: &InfobaseConfigExportArgs) -> Result<InfobaseConfigExportReport> {
    export_config_policy(args, false)
}

pub(crate) fn export_config_native(
    args: &InfobaseConfigExportArgs,
) -> Result<InfobaseConfigExportReport> {
    export_config_policy(args, true)
}

fn export_config_policy(
    args: &InfobaseConfigExportArgs,
    native_versions: bool,
) -> Result<InfobaseConfigExportReport> {
    let config = resolve_connection(args.connection())?;
    if config.postgres()?.is_some() {
        if args.extension.is_some() {
            bail!("the export of extensions from PostgreSQL is not supported yet");
        }
    } else if config.file_infobase.is_none() {
        ensure_mssql(&config.dbms)?;
    }
    if let Some(extension) = args.extension.as_deref() {
        if args.base.is_some() || args.sync {
            bail!("--base and --sync export the configuration; an extension is exported in full");
        }
        return export_extension_report(
            &config,
            args.sqlcmd.as_deref(),
            &args.db_pwd_env,
            extension,
            &args.output_dir,
        );
    }
    export_config_report_policy(
        &config,
        args.sqlcmd.as_deref(),
        &args.db_pwd_env,
        &args.output_dir,
        args.overwrite,
        args.count_files,
        Vec::new(),
        args.base.as_deref(),
        args.sync,
        native_versions,
    )
}

/// The export of the configuration into `output_dir_arg`. With `base` or
/// `sync` (`--base`, `--sync`, `mssql_dump::incremental`) the directory may
/// hold an earlier export, which the export updates instead of refusing it.
#[allow(clippy::too_many_arguments)]
pub(crate) fn export_config_report(
    config: &ConnectionConfig,
    sqlcmd: Option<&Path>,
    db_pwd_env: &str,
    output_dir_arg: &Path,
    overwrite: bool,
    count_exported_files: bool,
    file_names: Vec<String>,
    base: Option<&Path>,
    sync: bool,
) -> Result<InfobaseConfigExportReport> {
    export_config_report_policy(
        config,
        sqlcmd,
        db_pwd_env,
        output_dir_arg,
        overwrite,
        count_exported_files,
        file_names,
        base,
        sync,
        false,
    )
}

#[allow(clippy::too_many_arguments)]
fn export_config_report_policy(
    config: &ConnectionConfig,
    sqlcmd: Option<&Path>,
    db_pwd_env: &str,
    output_dir_arg: &Path,
    overwrite: bool,
    count_exported_files: bool,
    file_names: Vec<String>,
    base: Option<&Path>,
    sync: bool,
    native_versions: bool,
) -> Result<InfobaseConfigExportReport> {
    let postgres_snapshot = config
        .postgres_snapshot
        .as_ref()
        .map(|snapshot| snapshot.transfer())
        .transpose()?
        .flatten();
    let source_version = config.legacy_source_version()?;
    let output_dir = absolute_path(output_dir_arg)?;
    if base.is_some() || sync {
        if !native_versions {
            crate::mssql_dump::incremental::prepare_output_dir(&output_dir)?;
        }
    } else {
        prepare_output_dir(&output_dir, overwrite)?;
    }
    let mut dump_args = dump_args(
        config,
        postgres_snapshot,
        sqlcmd,
        db_pwd_env,
        output_dir.clone(),
        file_names,
        source_version,
    );
    dump_args.base = base.map(Path::to_path_buf);
    dump_args.sync = sync;
    let dump = crate::mssql_dump::dump_config_with(
        &dump_args,
        crate::mssql_dump::DumpHooks {
            native_versions,
            ..Default::default()
        },
    )?;

    let exported_files = if count_exported_files {
        Some(count_files(&output_dir)?)
    } else {
        None
    };

    Ok(InfobaseConfigExportReport {
        operation: "infobase config export",
        backend: if config.file_infobase.is_some() {
            "file-infobase-direct"
        } else if is_postgres(&config.dbms) {
            "postgres-config-direct"
        } else {
            "mssql-config-direct"
        },
        format: format_name(config.format),
        source_version: source_version.as_str(),
        dbms: config.dbms.clone(),
        db_server: config.db_server.clone(),
        db_name: config.db_name.clone(),
        db_user: config.db_user.clone(),
        password_source: config.password_source.clone(),
        native_config: config.native_config.clone(),
        output_dir,
        temp_dump_dir: dump_args.output_dir,
        exported_files,
        raw_rows: dump.total_rows,
        metadata_xml_rows: dump.total_metadata_xml_rows,
        module_text_rows: dump.total_module_text_rows,
        source_asset_rows: dump.total_source_asset_rows,
        dump_timings: dump.timings,
        incremental: dump.incremental,
    })
}

/// `infobase config export --extension=<name>`: the tree of one extension, as
/// the platform's export writes it (the staged image when the extension has
/// one, else the active one). Like the export of the configuration it refuses
/// a directory that holds files, and it fails, having written the tree, when
/// a row of the extension cannot be written exactly.
pub(crate) fn export_extension_report(
    config: &ConnectionConfig,
    sqlcmd: Option<&Path>,
    db_pwd_env: &str,
    extension: &str,
    output_dir_arg: &Path,
) -> Result<InfobaseConfigExportReport> {
    let source_version = config.legacy_source_version()?;
    if config.file_infobase.is_some()
        && fs::symlink_metadata(output_dir_arg)
            .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        bail!("refusing to export an extension through an output symlink");
    }
    let output_dir = absolute_path(output_dir_arg)?;
    if output_dir.exists() {
        prepare_output_dir(&output_dir, false)?;
    }
    // An existing directory is empty by now; the tree replaces it.
    let (export, backend) = if let Some(path) = config.file_infobase.as_deref() {
        (
            crate::file_extension_export::export(path, extension, &output_dir, source_version)?,
            "file-infobase-extension",
        )
    } else {
        let dump_args = extension_dump_args(
            config,
            sqlcmd,
            db_pwd_env,
            extension,
            output_dir.clone(),
            output_dir.exists(),
            source_version,
        );
        let dump = crate::mssql_extension_export::dump_extensions(&dump_args)?;
        let [entry]: [_; 1] = dump.extensions.try_into().map_err(|entries: Vec<_>| {
            anyhow!(
                "the export of extension {extension:?} produced {} trees, one expected",
                entries.len()
            )
        })?;
        if !entry.complete {
            bail!(
                "the export of extension {extension:?} is incomplete ({}); the tree in {} holds what could be written",
                entry.warnings.join("; "),
                output_dir.display()
            );
        }
        (entry.export, "mssql-extension-direct")
    };

    Ok(InfobaseConfigExportReport {
        operation: "infobase config export",
        backend,
        format: format_name(config.format),
        source_version: source_version.as_str(),
        dbms: config.dbms.clone(),
        db_server: config.db_server.clone(),
        db_name: config.db_name.clone(),
        db_user: config.db_user.clone(),
        password_source: config.password_source.clone(),
        native_config: config.native_config.clone(),
        temp_dump_dir: output_dir.clone(),
        output_dir,
        exported_files: Some(export.files_written),
        raw_rows: export.storage.physical_entries,
        metadata_xml_rows: 0,
        module_text_rows: 0,
        source_asset_rows: 0,
        dump_timings: Default::default(),
        incremental: None,
    })
}

/// The export of one extension of the connection's database into
/// `output_dir`, which `overwrite` says already exists (empty).
fn extension_dump_args(
    config: &ConnectionConfig,
    sqlcmd: Option<&Path>,
    db_pwd_env: &str,
    extension: &str,
    output_dir: PathBuf,
    overwrite: bool,
    source_version: InfobaseConfigSourceVersion,
) -> MssqlDumpExtensionArgs {
    MssqlDumpExtensionArgs {
        sqlcmd: sqlcmd.map(Path::to_path_buf),
        bcp_executable: None,
        server: config.db_server.clone(),
        sql_user: config.db_user.clone(),
        sql_pwd: config.db_pwd.clone(),
        sql_pwd_env: db_pwd_env.to_string(),
        sqlcmd_trust_cert: false,
        database: config.db_name.clone(),
        extension: Some(extension.to_string()),
        all_extensions: false,
        output_dir,
        overwrite,
        image: MssqlExtensionImage::Auto,
        platform: None,
        source_version,
    }
}

/// The export of the connection's database into `output_dir`.
fn dump_args(
    config: &ConnectionConfig,
    postgres_snapshot: Option<crate::mssql_dump::PostgresExportSnapshot>,
    sqlcmd: Option<&Path>,
    db_pwd_env: &str,
    output_dir: PathBuf,
    file_names: Vec<String>,
    source_version: InfobaseConfigSourceVersion,
) -> MssqlDumpConfigArgs {
    MssqlDumpConfigArgs {
        rows_dir: None,
        file_infobase: config.file_infobase.clone(),
        // Checked when the connection is resolved (`resolve_connection_with`).
        postgres: config.postgres().ok().flatten(),
        postgres_snapshot,
        model_export: false,
        legacy_export: false,
        sqlcmd: sqlcmd.map(Path::to_path_buf),
        bcp_executable: None,
        runtime_journal: None,
        server: config.db_server.clone(),
        sql_user: config.db_user.clone(),
        sql_pwd: config.db_pwd.clone(),
        sql_pwd_env: db_pwd_env.to_string(),
        database: config.db_name.clone(),
        output_dir,
        overwrite: false,
        include_config_save: false,
        main_configuration: true,
        file_names,
        file_name_lists: Vec::new(),
        objects: Vec::new(),
        inflate: false,
        extract_module_text: true,
        extract_metadata_xml: true,
        require_complete_root_metadata: false,
        require_complete_source_assets: false,
        collect_all_source_asset_diagnostics: false,
        no_binary_rows: true,
        write_binary_rows: false,
        write_manifest: false,
        platform: None,
        source_version,
        base: None,
        sync: false,
    }
}

impl InfobaseConfigLoadArgs {
    fn connection(&self) -> ConnectionRequest<'_> {
        ConnectionRequest {
            settings: self.settings.as_deref(),
            native_config: self.native_config.as_deref(),
            format: None,
            platform: None,
            source_version: None,
            db_path: self.db_path.as_deref(),
            dbms: self.dbms.as_deref(),
            db_server: self.db_server.as_deref(),
            db_name: self.db_name.as_deref(),
            db_user: self.db_user.as_deref(),
            db_pwd: self.db_pwd.as_deref(),
            db_pwd_env: &self.db_pwd_env,
            // A .cf holds the stored rows: no XML format is read.
            need: PlatformNeed::Given,
        }
    }
}

/// `infobase config load <file.cf>` (#353): the file's elements staged in
/// `ConfigSave` for `config apply`, in a file infobase or on PostgreSQL. On
/// SQL Server `mssql-load-config --script-only` writes the stage's scripts.
pub fn load_config(args: &InfobaseConfigLoadArgs) -> Result<InfobaseConfigImportReport> {
    let config = resolve_connection(args.connection())?;
    let input = absolute_path(&args.input)?;
    if config.file_infobase.is_some() {
        return crate::file_infobase::load_cf(&config, &input);
    }
    if config.postgres()?.is_some() {
        return crate::postgres_infobase::load_cf(&config, &input);
    }
    ensure_mssql(&config.dbms)?;
    let sql = crate::sql::SqlExec::from_options(crate::sql::SqlOptions {
        sqlcmd: args.sqlcmd.as_deref(),
        bcp: None,
        server: &config.db_server,
        user: config.db_user.as_deref(),
        password: config.db_pwd.as_deref(),
        password_env: &args.db_pwd_env,
        trust_server_certificate: true,
    })?;
    let loaded = crate::mssql::cf_load_stage::load_into_database(&sql, &config.db_name, &input)?;
    Ok(InfobaseConfigImportReport {
        operation: "infobase config load",
        backend: "mssql-configsave-stage",
        format: "cf",
        source_version: None,
        dbms: config.dbms.clone(),
        db_server: config.db_server.clone(),
        db_name: config.db_name.clone(),
        db_user: config.db_user.clone(),
        native_config: config.native_config.clone(),
        source_dir: input,
        stage_mode: "base-free",
        stage_mode_reason: format!(
            "the .cf replaces the main configuration ({} elements, {} rows of Config deleted)",
            loaded.rows, loaded.deleted
        ),
        files: Vec::new(),
        target_config_rows: -1,
        staged_rows_before: loaded.config_save_rows_before,
        staged_rows_after: loaded.config_save_rows_after,
        scripts: Vec::new(),
        verification: None,
        overrides: None,
        backup: None,
    })
}

pub fn import_config(args: &InfobaseConfigImportArgs) -> Result<InfobaseConfigImportReport> {
    let config = resolve_connection(args.connection())?;
    if config.file_infobase.is_some() {
        return crate::file_infobase::import(&config, args);
    }
    if config.postgres()?.is_some() {
        refuse_postgres_files(args)?;
        return crate::postgres_infobase::import(&config, args);
    }
    ensure_mssql(&config.dbms)?;

    let mut stage_args = build_import_stage_args(&config, args)?;
    if !stage_args.source_root.is_dir() {
        bail!(
            "каталог файлов конфигурации не найден: {}",
            args.source_dir.display()
        );
    }
    let (base_free, reason, target_config_rows) = match args.stage_mode {
        // A partial import patches the rows of the objects it names; a
        // sparse directory has no Configuration.xml to compare the target by.
        _ if !args.files.is_empty() => {
            if matches!(args.stage_mode, InfobaseImportStageMode::BaseFree) {
                bail!(
                    "частичная загрузка файлов не собирает конфигурацию с нуля: --base-free не применим"
                );
            }
            (
                false,
                format!("a partial import of {} files", args.files.len()),
                -1,
            )
        }
        InfobaseImportStageMode::BaseFree => (true, "asked for (--base-free)".to_string(), -1),
        InfobaseImportStageMode::Patch => (false, "asked for".to_string(), -1),
        InfobaseImportStageMode::Auto => {
            let configuration = tree_configuration_uuid(&stage_args.source_root)?;
            let target = crate::mssql::import_target_state(&stage_args, &configuration)?;
            let base_free = !target.holds_configuration;
            let reason = if target.config_rows == 0 {
                "the target's Config is empty".to_string()
            } else if base_free {
                format!(
                    "the target's Config ({} rows) holds no row of configuration {configuration}",
                    target.config_rows
                )
            } else {
                format!(
                    "the target's Config ({} rows) holds configuration {configuration}",
                    target.config_rows
                )
            };
            (base_free, reason, target.config_rows)
        }
    };
    stage_args.base_free = base_free;
    // A stage can leave a change of the tree out without a word (a patch stage
    // takes the target's own rows; a compiled one can carry a slip of the
    // compiler): the guard checks every stage unless asked not to.
    stage_args.verify = !matches!(args.verify, InfobaseImportVerify::Off);
    let report = crate::mssql::stage_source_objects(&stage_args)?;

    Ok(InfobaseConfigImportReport {
        operation: "infobase config import",
        backend: "mssql-configsave-stage",
        format: format_name(config.format),
        source_version: report
            .source_version
            .clone()
            .or_else(|| crate::metadata_model::export::tree_version(&stage_args.source_root)),
        dbms: config.dbms,
        db_server: config.db_server,
        db_name: config.db_name,
        db_user: config.db_user,
        native_config: config.native_config,
        source_dir: stage_args.source_root,
        stage_mode: if base_free { "base-free" } else { "patch" },
        stage_mode_reason: reason,
        files: args.files.clone(),
        target_config_rows,
        staged_rows_before: report.before.row_count,
        staged_rows_after: report.after.row_count,
        scripts: report.scripts,
        verification: report.verification,
        overrides: report.overrides,
        backup: None,
    })
}

/// The uuid of the configuration a tree describes (its `Configuration.xml`).
fn tree_configuration_uuid(source_root: &Path) -> Result<String> {
    let path = source_root.join("Configuration.xml");
    let xml = fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
    Ok(crate::metadata_model::root::configuration_facts(&xml)
        .with_context(|| format!("failed to read {}", path.display()))?
        .uuid)
}

fn build_import_stage_args(
    config: &ConnectionConfig,
    args: &InfobaseConfigImportArgs,
) -> Result<MssqlStageSourceObjectsArgs> {
    Ok(MssqlStageSourceObjectsArgs {
        server: config.db_server.clone(),
        sql_user: config.db_user.clone(),
        sql_pwd: config.db_pwd.clone(),
        sql_pwd_env: args.db_pwd_env.clone(),
        database: config.db_name.clone(),
        source_root: absolute_path(&args.source_dir)?,
        sqlcmd: args.sqlcmd.clone(),
        replace_config_save: args.replace_config_save,
        allow_non_lab: args.allow_non_lab,
        batch_size: args.batch_size,
        platform: None,
        // An import reads the tree's own version unless one was asked for.
        source_version: if config.xml_version_given {
            Some(config.legacy_source_version()?)
        } else {
            None
        },
        path_prefix: args.path_prefix.clone(),
        files: args.files.clone(),
        native_files: args.native_files,
        script_output: args.script_output.clone(),
        script_only: false,
        bulk: false,
        per_row: false,
        bcp_executable: None,
        base_free: matches!(args.stage_mode, InfobaseImportStageMode::BaseFree),
        // Decided by `import_config` once the mode is known.
        verify: false,
    })
}

/// The settings of `crate::settings` -- the environment, `ibcmd-rs.toml`
/// and the platform ibcmd's own `--config` -- read once, and only when the
/// command line and `--settings` leave something open.
struct Defaults<'a> {
    native_config: Option<&'a Path>,
    load: SettingsLoader<'a>,
    loaded: Option<Settings>,
}

/// Reads the settings: [`Settings::load`] outside the tests.
type SettingsLoader<'a> = &'a dyn Fn(Option<&Path>) -> Result<Settings>;

impl<'a> Defaults<'a> {
    fn new(native_config: Option<&'a Path>, load: SettingsLoader<'a>) -> Self {
        Self {
            native_config,
            load,
            loaded: None,
        }
    }

    fn get(&mut self) -> Result<&Settings> {
        if self.loaded.is_none() {
            self.loaded = Some((self.load)(self.native_config)?);
        }
        Ok(self.loaded.as_ref().expect("the settings were read above"))
    }
}

/// Each value, highest first: the command line, the `--settings` file (the
/// vRunner JSON), the settings (`crate::settings`: the environment,
/// `ibcmd-rs.toml`, the `database:` section of native `--config`), then the
/// default (`MSSQLServer`, `localhost`). The XML version: `--platform`, the
/// hidden `--source-version` or `--settings`; otherwise the platform the
/// settings give the database, then what the operation shows
/// ([`PlatformNeed`]).
pub(crate) fn resolve_connection(request: ConnectionRequest<'_>) -> Result<ConnectionConfig> {
    resolve_connection_with(request, &Settings::load)
}

/// A query route whose resolved backend is not supported by its protocol.
#[derive(Debug)]
pub(crate) struct UnsupportedExportQuery(pub String);

impl std::fmt::Display for UnsupportedExportQuery {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for UnsupportedExportQuery {}

/// Resolve settings once and refuse unsupported queries before any platform probe.
pub(crate) fn resolve_export_query_connection(
    request: ConnectionRequest<'_>,
    command: &str,
) -> Result<ConnectionConfig> {
    resolve_connection_with_guard(request, &Settings::load, |config| {
        if is_postgres(&config.dbms) {
            return Err(UnsupportedExportQuery(crate::dropin::postgres_refusal(command)).into());
        }
        Ok(())
    })
}

fn resolve_connection_with(
    request: ConnectionRequest<'_>,
    load: SettingsLoader<'_>,
) -> Result<ConnectionConfig> {
    resolve_connection_with_guard(request, load, |_| Ok(()))
}

fn resolve_connection_with_guard(
    request: ConnectionRequest<'_>,
    load: SettingsLoader<'_>,
    backend_guard: impl FnOnce(&ConnectionConfig) -> Result<()>,
) -> Result<ConnectionConfig> {
    let settings = match request.settings {
        Some(path) => Some(read_settings(path)?),
        None => None,
    };
    let mut defaults = Defaults::new(request.native_config, load);
    let native_config = request.native_config.map(Path::to_path_buf);

    let format = request
        .format
        .or_else(|| settings_format(&settings))
        .unwrap_or(InfobaseConfigFormat::Xml);
    let xml_version_given = request.platform.is_some()
        || request.source_version.is_some()
        || settings_xml_dialect(&settings)?.is_some()
        || settings_platform_build(&settings)?.is_some();
    let legacy_adapter = match request.platform {
        Some(platform) => adapter_for(platform.xml_version())?,
        None => resolve_legacy_adapter(&settings, request.source_version)?,
    };
    if let Some(db_path) = request.db_path {
        return resolve_file_infobase(
            db_path,
            &request,
            native_config,
            format,
            legacy_adapter,
            xml_version_given,
            &mut defaults,
        );
    }
    let dbms = match first_value(request.dbms, settings_value(&settings, "dbms-type")) {
        Some(dbms) => dbms,
        None => defaults
            .get()?
            .dbms()
            .map(|dbms| dbms.value)
            .unwrap_or_else(|| "MSSQLServer".to_string()),
    };
    let db_server = match first_value(request.db_server, settings_value(&settings, "dbms-server")) {
        Some(server) => server,
        None => defaults
            .get()?
            .db_server()
            .map(|server| server.value)
            .unwrap_or_else(|| "localhost".to_string()),
    };
    let db_name = match first_value(request.db_name, settings_value(&settings, "dbms-base")) {
        Some(name) => name,
        None => defaults
            .get()?
            .db_name()
            .map(|name| name.value)
            .ok_or_else(|| {
                anyhow!(
                    "не указана информационная база: передайте --db-path для файловой базы \
                     или --dbms=MSSQLServer, --db-server и --db-name"
                )
            })?,
    };
    let db_user = match first_value(request.db_user, settings_value(&settings, "dbms-user")) {
        Some(user) => Some(user),
        None => defaults.get()?.db_user().map(|user| user.value),
    };
    let (db_pwd, password_source) = match db_user {
        // A PostgreSQL server may log the user in without one (`trust` in
        // pg_hba.conf); it asks for a password when it wants one
        // (`sql::postgres`).
        _ if is_postgres(&dbms) => {
            resolve_optional_password(request.db_pwd, &settings, request.db_pwd_env, &mut defaults)?
        }
        Some(_) => resolve_password(request.db_pwd, &settings, request.db_pwd_env, &mut defaults)?,
        None => (None, None),
    };

    let mut config = ConnectionConfig {
        dbms,
        db_server,
        db_name,
        db_user,
        db_pwd,
        password_source,
        native_config,
        format,
        legacy_adapter,
        xml_version_given,
        file_infobase: None,
        postgres_snapshot: None,
    };
    backend_guard(&config)?;
    // A malformed PostgreSQL server is refused here, before the platform
    // probe reads the database through `dump_args`.
    config.postgres()?;
    settle_platform(&mut config, &request, &mut defaults)?;
    Ok(config)
}

/// `--db-path`: a file infobase. No DBMS value may come with it; the settings
/// can still name its platform by the infobase directory (a database entry
/// without a server, its name a mask of the path).
fn resolve_file_infobase(
    db_path: &Path,
    request: &ConnectionRequest<'_>,
    native_config: Option<PathBuf>,
    format: InfobaseConfigFormat,
    legacy_adapter: MssqlLegacyAdapter,
    xml_version_given: bool,
    defaults: &mut Defaults<'_>,
) -> Result<ConnectionConfig> {
    for (given, option) in [
        (request.dbms, "--dbms"),
        (request.db_server, "--db-server"),
        (request.db_name, "--db-name"),
        (request.db_user, "--db-user"),
    ] {
        if given.is_some() {
            bail!(
                "--db-path задаёт файловую информационную базу: {option} вместе с ним не указывается"
            );
        }
    }
    let data_file = ibcmd_filedb::data_file(&absolute_path(db_path)?);
    if !data_file.is_file() {
        bail!(
            "Файловая информационная база не найдена: нет файла {}",
            data_file.display()
        );
    }
    let directory = data_file
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| data_file.clone());
    let mut config = ConnectionConfig {
        dbms: "File".to_string(),
        db_server: String::new(),
        db_name: directory.display().to_string(),
        db_user: None,
        db_pwd: None,
        password_source: None,
        native_config,
        format,
        legacy_adapter,
        xml_version_given,
        file_infobase: Some(data_file),
        postgres_snapshot: None,
    };
    settle_platform(&mut config, request, defaults)?;
    Ok(config)
}

/// The XML version the command line and `--settings` left open, from the
/// platform the settings give the database (`crate::settings`), and the
/// check of an import's tree against a platform named for it.
fn settle_platform(
    config: &mut ConnectionConfig,
    request: &ConnectionRequest<'_>,
    defaults: &mut Defaults<'_>,
) -> Result<()> {
    config.postgres_snapshot = None;
    match request.need {
        PlatformNeed::Given => Ok(()),
        PlatformNeed::Export {
            sqlcmd,
            output_dir,
            overwrite,
            incremental,
        } => {
            if config.xml_version_given {
                // An exact build on the command line spells its own
                // defaults (`platform::note_export_platform`), as for
                // `cf export --platform`.
                let platform = match request.platform {
                    Some(platform) => Some(platform),
                    None => config
                        .legacy_adapter
                        .version_axes()
                        .platform_build()
                        .map(|build| crate::platform::parse(&build.to_string()))
                        .transpose()?,
                };
                if let Some(platform) = platform {
                    crate::platform::note_export_platform(platform);
                } else {
                    crate::platform::clear_export_platform();
                }
                return Ok(());
            }
            let pending_snapshot = std::cell::RefCell::new(None);
            let platform = {
                let known: &ConnectionConfig = config;
                let probe = || {
                    // The probe reads the database: a directory that holds
                    // files is refused first, as the export always did
                    // before reading it (`--base`/`--sync` update one).
                    if !incremental {
                        prepare_output_dir(&absolute_path(output_dir)?, overwrite)?;
                    }
                    let (compatible, snapshot) =
                        crate::mssql_dump::model_export::configuration_compatibility_with_snapshot(
                            &dump_args(
                                known,
                                None,
                                sqlcmd,
                                request.db_pwd_env,
                                PathBuf::new(),
                                Vec::new(),
                                InfobaseConfigSourceVersion::V2_20,
                            ),
                        )?;
                    *pending_snapshot.borrow_mut() = snapshot;
                    Ok(compatible)
                };
                crate::settings::resolve_platform(
                    None,
                    defaults.get()?,
                    DatabaseTarget::new(Some(&known.db_server), &known.db_name),
                    PlatformHint::Export {
                        compatibility_8_5_or_later: &probe,
                    },
                )?
            };
            crate::platform::note_export_platform(platform);
            config.legacy_adapter = adapter_for(platform.xml_version())?;
            config.postgres_snapshot = pending_snapshot.into_inner();
            Ok(())
        }
        PlatformNeed::Import { source_dir } => {
            if let Some(platform) = request.platform {
                let source_root = absolute_path(source_dir)?;
                return crate::settings::commands::check_tree(
                    platform,
                    &source_root,
                    &config.db_name,
                );
            }
            if config.xml_version_given {
                return Ok(());
            }
            let source_root = absolute_path(source_dir)?;
            let resolved = crate::settings::resolve_platform_with_source(
                None,
                defaults.get()?,
                DatabaseTarget::new(Some(&config.db_server), &config.db_name),
                PlatformHint::Import {
                    source_root: &source_root,
                },
            )?;
            // Nothing names the platform and the tree has no
            // Configuration.xml to say it: each file is read in the format
            // it declares, as before the settings existed.
            if resolved.source != SettingSource::Default {
                crate::settings::commands::check_tree(
                    resolved.value,
                    &source_root,
                    &config.db_name,
                )?;
                config.legacy_adapter = adapter_for(resolved.value.xml_version())?;
                config.xml_version_given = true;
            }
            Ok(())
        }
    }
}

/// The adapter of one XML version.
fn adapter_for(selector: InfobaseConfigSourceVersion) -> Result<MssqlLegacyAdapter> {
    resolve_legacy_adapter(&None, Some(selector))
}

pub(crate) fn read_settings(path: &Path) -> Result<Value> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("failed to read settings {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("failed to parse {}", path.display()))
}

pub(crate) fn settings_value(settings: &Option<Value>, name: &str) -> Option<String> {
    settings
        .as_ref()?
        .get("vrunner")?
        .get(name)?
        .as_str()
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
}

fn settings_format(settings: &Option<Value>) -> Option<InfobaseConfigFormat> {
    let value = settings_string_at(settings, &["ibcmd-rs", "config-format"])
        .or_else(|| settings_string_at(settings, &["ibcmd-rs", "format"]))
        .or_else(|| settings_string_at(settings, &["format"]))
        .or_else(|| settings_value(settings, "format"))?;
    parse_format(&value)
}

fn resolve_legacy_adapter(
    settings: &Option<Value>,
    cli_source_version: Option<InfobaseConfigSourceVersion>,
) -> Result<MssqlLegacyAdapter> {
    let platform_build = settings_platform_build(settings)?;
    // An explicit XML selection wins; otherwise the platform names its XML
    // format through the platform registry (8.3.x -> 2.20, 8.5.x -> 2.21,
    // known builds only), and without either the dialect is 2.20.
    let xml_dialect = match cli_source_version {
        Some(selector) => selector.version_axes().xml_dialect().clone(),
        None => match settings_xml_dialect(settings)? {
            Some(dialect) => dialect,
            None => {
                let selector = match &platform_build {
                    Some(build) => crate::platform::parse(&build.to_string())
                        .with_context(|| format!("platform-version `{build}` in settings"))?
                        .xml_version(),
                    None => InfobaseConfigSourceVersion::V2_20,
                };
                selector.version_axes().xml_dialect().clone()
            }
        },
    };
    let version_axes = LegacyVersionAxes::new(xml_dialect, platform_build, None, None, None);
    let legacy_adapter = MssqlLegacyAdapter::new(version_axes)?;
    if legacy_adapter.legacy_selector().is_none() {
        bail!(
            "legacy MSSQL adapter does not support XML dialect {}",
            legacy_adapter.xml_dialect()
        );
    }
    Ok(legacy_adapter)
}

fn settings_xml_dialect(settings: &Option<Value>) -> Result<Option<XmlDialect>> {
    let value = settings_string_at(settings, &["ibcmd-rs", "source-version"])
        .or_else(|| settings_string_at(settings, &["ibcmd-rs", "xml-version"]))
        .or_else(|| settings_string_at(settings, &["ibcmd-rs", "xcf-version"]))
        .or_else(|| settings_string_at(settings, &["source-version"]))
        .or_else(|| settings_string_at(settings, &["xml-version"]))
        .or_else(|| settings_string_at(settings, &["xcf-version"]))
        .or_else(|| settings_value(settings, "source-version"))
        .or_else(|| settings_value(settings, "xml-version"))
        .or_else(|| settings_value(settings, "xcf-version"));
    let Some(value) = value else {
        return Ok(None);
    };
    if let Some(selector) = parse_legacy_source_selector(&value) {
        return Ok(Some(selector.version_axes().xml_dialect().clone()));
    }
    XmlDialect::parse(value.trim())
        .map(Some)
        .map_err(|error| anyhow!("invalid XML dialect `{value}` in settings: {error}"))
}

fn settings_platform_build(settings: &Option<Value>) -> Result<Option<PlatformBuild>> {
    let value = settings_string_at(settings, &["ibcmd-rs", "platform-version"])
        .or_else(|| settings_string_at(settings, &["platform-version"]))
        .or_else(|| settings_value(settings, "platform-version"));
    let Some(value) = value else {
        return Ok(None);
    };
    PlatformBuild::parse(value.trim())
        .map(Some)
        .map_err(|error| anyhow!("invalid platform build `{value}` in settings: {error}"))
}

pub(crate) fn settings_string_at(settings: &Option<Value>, path: &[&str]) -> Option<String> {
    let mut current = settings.as_ref()?;
    for segment in path {
        current = current.get(*segment)?;
    }
    current
        .as_str()
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
}

fn parse_format(value: &str) -> Option<InfobaseConfigFormat> {
    match value.trim().to_ascii_lowercase().as_str() {
        "xml" | "ibcmd-xml" | "source-tree" => Some(InfobaseConfigFormat::Xml),
        _ => None,
    }
}

fn parse_legacy_source_selector(value: &str) -> Option<InfobaseConfigSourceVersion> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "2.20" | "20" | "8.3" | "8.3.27" => Some(InfobaseConfigSourceVersion::V2_20),
        "2.21" | "21" | "8.5" | "8.5.1" => Some(InfobaseConfigSourceVersion::V2_21),
        _ if normalized.starts_with("8.3.27.") => Some(InfobaseConfigSourceVersion::V2_20),
        _ if normalized.starts_with("8.5.1.") => Some(InfobaseConfigSourceVersion::V2_21),
        _ => None,
    }
}

pub(crate) fn first_value(cli: Option<&str>, settings: Option<String>) -> Option<String> {
    cli.filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
        .or(settings)
}

fn resolve_password(
    cli_db_pwd: Option<&str>,
    settings: &Option<Value>,
    db_pwd_env: &str,
    defaults: &mut Defaults<'_>,
) -> Result<(Option<String>, Option<String>)> {
    match resolve_optional_password(cli_db_pwd, settings, db_pwd_env, defaults)? {
        (Some(password), source) => Ok((Some(password), source)),
        (None, _) => bail!(
            "не указан пароль пользователя сервера СУБД: передайте --db-pwd (--database-password), -W или переменную окружения {db_pwd_env}"
        ),
    }
}

/// The password from the first place that names one, if any does.
fn resolve_optional_password(
    cli_db_pwd: Option<&str>,
    settings: &Option<Value>,
    db_pwd_env: &str,
    defaults: &mut Defaults<'_>,
) -> Result<(Option<String>, Option<String>)> {
    if let Some(value) = cli_db_pwd.filter(|value| !value.is_empty()) {
        return Ok((Some(value.to_string()), Some("--db-pwd".to_string())));
    }
    if let Ok(value) = env::var(db_pwd_env) {
        return Ok((Some(value), Some(format!("env:{db_pwd_env}"))));
    }
    if let Some(value) = settings_value(settings, "dbms-pwd") {
        return Ok((Some(value), Some("settings".to_string())));
    }
    // `IBCMD_DB_PSW` or `database.password` of native `--config`
    if let Some(value) = defaults.get()?.db_password() {
        return Ok((Some(value.value), Some(value.source.to_string())));
    }
    Ok((None, None))
}

/// Whether `dbms` names PostgreSQL, as 1C spells it.
pub(crate) fn is_postgres(dbms: &str) -> bool {
    dbms.eq_ignore_ascii_case("PostgreSQL")
}

/// `import files` on PostgreSQL, the DBMS named by the settings too: the
/// PostgreSQL import replaces the whole stage, so a file list is refused
/// rather than ignored.
fn refuse_postgres_files(args: &InfobaseConfigImportArgs) -> Result<()> {
    if args.files.is_empty() {
        return Ok(());
    }
    Err(crate::mssql::files_stage::FilesRefused {
        message: crate::dropin::postgres_refusal("infobase config import files"),
        unsupported: true,
    }
    .into())
}

pub(crate) fn ensure_mssql(dbms: &str) -> Result<()> {
    if dbms.eq_ignore_ascii_case("MSSQLServer") || dbms.eq_ignore_ascii_case("MSSQL") {
        return Ok(());
    }
    bail!("unsupported dbms for direct infobase config operation: {dbms}")
}

fn format_name(format: InfobaseConfigFormat) -> &'static str {
    match format {
        InfobaseConfigFormat::Xml => "xml",
    }
}

pub(crate) fn absolute_path(path: &Path) -> Result<PathBuf> {
    if path.exists() {
        return fs::canonicalize(path)
            .with_context(|| format!("failed to resolve {}", path.display()));
    }
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(env::current_dir()?.join(path))
    }
}

pub(crate) fn prepare_output_dir(path: &Path, overwrite: bool) -> Result<()> {
    if path.exists() {
        if !path.is_dir() {
            bail!(
                "output path exists and is not a directory: {}",
                path.display()
            );
        }
        if fs::read_dir(path)?.next().is_some() && !overwrite {
            return Err(anyhow::Error::new(OutputDirectoryNotEmpty(
                path.to_path_buf(),
            )));
        }
        if overwrite {
            clear_directory(path)?;
        }
    } else {
        fs::create_dir_all(path).with_context(|| format!("failed to create {}", path.display()))?;
    }
    Ok(())
}

fn clear_directory(path: &Path) -> Result<()> {
    for entry in fs::read_dir(path).with_context(|| format!("failed to read {}", path.display()))? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            fs::remove_dir_all(entry.path())?;
        } else {
            fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

#[cfg(test)]
fn is_internal_dump_path(relative: &Path) -> bool {
    use std::ffi::OsStr;
    use std::path::Component;

    if relative == Path::new("manifest.json") {
        return true;
    }
    let Some(Component::Normal(first)) = relative.components().next() else {
        return false;
    };
    matches!(
        first,
        name if name == OsStr::new("Config")
            || name == OsStr::new("ConfigSave")
            || name == OsStr::new("Config_inflated")
            || name == OsStr::new("ConfigSave_inflated")
            || name == OsStr::new("Config_module_text")
            || name == OsStr::new("ConfigSave_module_text")
    )
}

fn count_files(root: &Path) -> Result<usize> {
    let mut count = 0;
    for entry in WalkDir::new(root) {
        let entry = entry?;
        if entry.file_type().is_file() {
            count += 1;
        }
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::InfobaseConfigImportArgs;
    use std::path::PathBuf;

    #[test]
    fn reads_format_from_top_level_settings() {
        let settings: Option<Value> = Some(serde_json::json!({
            "format": "ibcmd-xml",
            "ibcmd-rs": {
                "source-version": "8.5.1"
            },
            "vrunner": {
                "dbms-base": "servicedesk"
            }
        }));

        assert_eq!(settings_format(&settings), Some(InfobaseConfigFormat::Xml));
        assert_eq!(
            settings_xml_dialect(&settings)
                .unwrap()
                .map(|dialect| dialect.to_string()),
            Some("2.21".to_string())
        );
        assert_eq!(settings_platform_build(&settings).unwrap(), None);
        assert_eq!(
            settings_value(&settings, "dbms-base"),
            Some("servicedesk".to_string())
        );
    }

    #[test]
    fn settings_platform_maps_to_its_xml_format_and_fails_closed() {
        let default = resolve_legacy_adapter(&None, None).unwrap();
        assert_eq!(default.xml_dialect().to_string(), "2.20");
        assert_eq!(default.version_axes().platform_build(), None);

        // The explicit mapping of the platform registry: 8.3.x -> 2.20,
        // 8.5.x -> 2.21.
        for (build, xml) in [("8.5.1.1150", "2.21"), ("8.3.27.2214", "2.20")] {
            let platform_only = Some(serde_json::json!({
                "ibcmd-rs": { "platform-version": build }
            }));
            let resolved = resolve_legacy_adapter(&platform_only, None).unwrap();
            assert_eq!(resolved.xml_dialect().to_string(), xml, "{build}");
            assert_eq!(
                resolved
                    .version_axes()
                    .platform_build()
                    .map(ToString::to_string)
                    .as_deref(),
                Some(build)
            );
        }

        // An explicit XML selection, in the settings or on the command line,
        // still wins over the platform's own format.
        let both = Some(serde_json::json!({
            "ibcmd-rs": { "platform-version": "8.5.1.1150", "xml-version": "2.20" }
        }));
        assert_eq!(
            resolve_legacy_adapter(&both, None)
                .unwrap()
                .xml_dialect()
                .to_string(),
            "2.20"
        );
        let platform_85 = Some(serde_json::json!({
            "ibcmd-rs": { "platform-version": "8.5.1.1150" }
        }));
        assert_eq!(
            resolve_legacy_adapter(&platform_85, Some(InfobaseConfigSourceVersion::V2_20))
                .unwrap()
                .xml_dialect()
                .to_string(),
            "2.20"
        );

        // Only builds the registry knows map; nothing is mapped to the
        // nearest version.
        for (build, expected) in [
            ("8.3.24.1819", "is not supported"),
            ("8.4.2.1", "unknown platform build"),
        ] {
            let unknown = Some(serde_json::json!({
                "ibcmd-rs": { "platform-version": build }
            }));
            let error = format!("{:#}", resolve_legacy_adapter(&unknown, None).unwrap_err());
            assert!(error.contains(expected), "{build}: {error}");
        }

        for dialect in ["2.17", "2.99"] {
            let settings = Some(serde_json::json!({
                "ibcmd-rs": { "xml-version": dialect }
            }));
            let error = resolve_legacy_adapter(&settings, None).unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("legacy MSSQL adapter does not support XML dialect")
            );
        }

        let malformed_xml = Some(serde_json::json!({
            "ibcmd-rs": { "xml-version": "not-a-version" }
        }));
        assert!(
            resolve_legacy_adapter(&malformed_xml, None)
                .unwrap_err()
                .to_string()
                .contains("invalid XML dialect")
        );

        let malformed_platform = Some(serde_json::json!({
            "ibcmd-rs": { "platform-version": "8.5.invalid" }
        }));
        assert!(
            resolve_legacy_adapter(&malformed_platform, None)
                .unwrap_err()
                .to_string()
                .contains("invalid platform build")
        );
    }

    #[test]
    fn skips_only_internal_dump_roots() {
        assert!(is_internal_dump_path(Path::new("Config/versions.bin")));
        assert!(is_internal_dump_path(Path::new("Config_module_text/a.bsl")));
        assert!(is_internal_dump_path(Path::new("manifest.json")));
        assert!(!is_internal_dump_path(Path::new("Configuration.xml")));
        assert!(!is_internal_dump_path(Path::new(
            "Constants/UseFeature.xml"
        )));
        assert!(!is_internal_dump_path(Path::new("ConfigDumpInfo.xml")));
    }

    #[test]
    fn import_files_on_postgres_is_refused_not_widened() {
        let mut args = import_args();
        assert!(refuse_postgres_files(&args).is_ok());
        args.files = vec!["Catalogs/Валюты.xml".to_string()];
        let error = refuse_postgres_files(&args).unwrap_err();
        let refused = error
            .downcast_ref::<crate::mssql::files_stage::FilesRefused>()
            .unwrap();
        assert!(refused.unsupported);
        assert!(refused.message.contains("для PostgreSQL не поддерживается"));
    }

    fn import_args() -> InfobaseConfigImportArgs {
        InfobaseConfigImportArgs {
            db_path: None,
            settings: None,
            native_config: None,
            format: Some(InfobaseConfigFormat::Xml),
            platform: None,
            source_version: Some(InfobaseConfigSourceVersion::V2_21),
            dbms: Some("MSSQLServer".to_string()),
            db_server: Some("localhost".to_string()),
            db_name: Some("ut_ibcmd".to_string()),
            db_user: Some("sa_import".to_string()),
            db_pwd: Some("secret".to_string()),
            db_pwd_env: "IMPORT_SQL_PWD".to_string(),
            user: None,
            password: None,
            password_env: "IBCMD_USER_PSW".to_string(),
            sqlcmd: None,
            replace_config_save: true,
            allow_non_lab: true,
            batch_size: Some(250),
            path_prefix: vec!["Catalogs/Валюты".to_string()],
            files: Vec::new(),
            native_files: None,
            script_output: Some(PathBuf::from(r"C:\temp\stage.sql")),
            stage_mode: InfobaseImportStageMode::Auto,
            verify: InfobaseImportVerify::Auto,
            source_dir: PathBuf::from(r".\fixtures\source"),
        }
    }

    /// A scratch folder, removed on drop.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "ibcmd-rs-infobase-{label}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// The settings of `dir/ibcmd-rs.toml` (as the current directory's)
    /// and of `env`, nothing else.
    fn settings_in<'a>(
        dir: &'a Path,
        env: &'a [(&'a str, &'a str)],
    ) -> impl Fn(Option<&Path>) -> Result<Settings> + 'a {
        move |native_config| {
            let lookup = |name: &str| {
                env.iter()
                    .find(|(key, _)| *key == name)
                    .map(|(_, value)| value.to_string())
            };
            Settings::from_sources(&crate::settings::SettingsSources {
                env: &lookup,
                executable: None,
                current_dir: Some(dir.to_path_buf()),
                app_data: None,
                native_config,
            })
        }
    }

    fn export_args(db_name: &str) -> InfobaseConfigExportArgs {
        InfobaseConfigExportArgs {
            db_path: None,
            settings: None,
            native_config: None,
            format: None,
            platform: None,
            source_version: None,
            dbms: Some("MSSQLServer".to_string()),
            db_server: Some("localhost".to_string()),
            db_name: Some(db_name.to_string()),
            db_user: None,
            db_pwd: None,
            db_pwd_env: "IBCMD_RS_TEST_NO_SUCH_PSW".to_string(),
            user: None,
            password: None,
            password_env: "IBCMD_USER_PSW".to_string(),
            // never run: every case here settles before a probe
            sqlcmd: None,
            extension: None,
            overwrite: false,
            count_files: false,
            output_dir: PathBuf::from("out"),
            base: None,
            sync: false,
        }
    }

    fn native_pipeline_refuses_stale_preflight_reference(case: &str, existing: bool) {
        use std::io::Read;
        let root = std::env::temp_dir().join(format!("native-selection-{}", uuid::Uuid::new_v4()));
        let _scratch = crate::test_scratch::TestScratch::adopt(&root);
        let db = root.join("db");
        fs::create_dir_all(&db).unwrap();
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/file-infobase");
        let mut bytes = Vec::new();
        flate2::read::GzDecoder::new(fs::File::open(fixture.join("tiny.1CD.gz")).unwrap())
            .read_to_end(&mut bytes)
            .unwrap();
        fs::write(db.join("1Cv8.1CD"), bytes).unwrap();
        let mut args = export_args("unused");
        args.db_path = Some(db);
        args.dbms = None;
        args.db_server = None;
        args.db_name = None;
        args.source_version = Some(InfobaseConfigSourceVersion::V2_21);
        args.output_dir = root.join("must-not-be-created");
        let current = crate::infobase::export_commands::info(&args).unwrap();
        let mut base = ibcmd_xml::config_dump_info::ConfigDumpInfo::parse(&current).unwrap();
        // This is the actual export boundary after any earlier preflight.
        // A freshly loaded reference can now require full export.
        match case {
            "empty" => {
                let mut bytes = Vec::new();
                flate2::read::GzDecoder::new(fs::File::open(fixture.join("empty.1CD.gz")).unwrap())
                    .read_to_end(&mut bytes)
                    .unwrap();
                fs::write(args.db_path.as_ref().unwrap().join("1Cv8.1CD"), bytes).unwrap();
            }
            "rename" => base.entries[0].name.push_str("PreviousLabName"),
            "body-role" => {
                let entry = base
                    .entries
                    .iter_mut()
                    .find(|entry| entry.id.ends_with(".0"))
                    .unwrap();
                entry.name.push_str("PreviousLabRole");
            }
            "removed-part" => {
                let name = format!("{}.Attribute.RemovedLabPart", base.entries[0].name);
                base.entries[0]
                    .children
                    .push(ibcmd_xml::config_dump_info::Part {
                        id: "ad2cd611-6109-431f-bf22-0b1a9d5069e5".into(),
                        name,
                    });
            }
            _ => {
                let mut removed = base.entries[0].clone();
                removed.id = "ad2cd611-6109-431f-bf22-0b1a9d5069e5".into();
                removed.name = "CommonModule.RemovedLabObject".into();
                removed.children.clear();
                base.entries.push(removed);
            }
        }
        let reference = root.join("changed-reference.xml");
        fs::write(&reference, base.to_native_bytes().unwrap()).unwrap();
        args.base = Some(reference);
        let tree = |path: &Path| {
            let mut files = std::collections::BTreeMap::new();
            if path.exists() {
                for entry in WalkDir::new(path).into_iter().map(|entry| entry.unwrap()) {
                    if entry.file_type().is_file() {
                        files.insert(
                            entry.path().strip_prefix(path).unwrap().to_owned(),
                            fs::read(entry.path()).unwrap(),
                        );
                    }
                }
            }
            files
        };
        if existing {
            fs::create_dir_all(&args.output_dir).unwrap();
            for (path, bytes) in tree(&fixture.join("tiny-xml")) {
                let target = args.output_dir.join(path);
                fs::create_dir_all(target.parent().unwrap()).unwrap();
                fs::write(target, bytes).unwrap();
            }
            let base_path = args
                .output_dir
                .join(crate::mssql_dump::incremental::CONFIG_DUMP_INFO);
            fs::write(&base_path, base.to_native_bytes().unwrap()).unwrap();
            args.base = Some(base_path);
            args.sync = true;
            fs::write(args.output_dir.join("README.md"), b"keep user bytes").unwrap();
        }
        let before = tree(&args.output_dir);
        let result = super::export_config_native(&args);
        let error = result.expect_err("native export must refuse a stale-preflight selection");
        assert!(
            error
                .downcast_ref::<crate::mssql_dump::incremental::NativeFullExportRequired>()
                .is_some(),
            "unexpected failure: {error:#}"
        );
        if existing {
            assert_eq!(
                tree(&args.output_dir),
                before,
                "refusal must not publish or prune existing output"
            );
        } else {
            assert!(
                !args.output_dir.exists(),
                "refusal must precede output creation"
            );
        }
    }

    #[test]
    fn native_export_pipeline_refuses_stale_preflight_selections() {
        // The physical offline adapter has process-wide active state. Run these
        // real pipeline cases in a child isolated from other parallel unit tests.
        const CHILD: &str = "IBCMD_RS_TEST_NATIVE_SELECTION_CHILD";
        if std::env::var_os(CHILD).is_some() {
            for case in ["remove", "rename", "removed-part", "body-role", "empty"] {
                native_pipeline_refuses_stale_preflight_reference(case, false);
            }
            native_pipeline_refuses_stale_preflight_reference("remove", true);
            native_pipeline_refuses_stale_preflight_reference("empty", true);
            return;
        }
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "infobase::tests::native_export_pipeline_refuses_stale_preflight_selections",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn dialect_only_export_clears_previous_exact_build_defaults() {
        const CHILD: &str = "IBCMD_DIALECT_AFTER_BUILD_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "infobase::tests::dialect_only_export_clears_previous_exact_build_defaults",
                    "--nocapture",
                ])
                .env(CHILD, "1")
                .output()
                .unwrap();
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(
                output.status.success() && stdout.contains("test result: ok. 1 passed; 0 failed;"),
                "{stdout}\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        use std::io::Read;
        let scratch = Scratch::new("dialect-after-build");
        let db = scratch.0.join("db");
        fs::create_dir(&db).unwrap();
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/file-infobase/default-button.1CD.gz");
        let mut bytes = Vec::new();
        flate2::read::GzDecoder::new(fs::File::open(fixture).unwrap())
            .read_to_end(&mut bytes)
            .unwrap();
        fs::write(db.join("1Cv8.1CD"), bytes).unwrap();
        let settings = scratch.0.join("dialect.json");
        fs::write(&settings, r#"{"ibcmd-rs":{"source-version":"8.5.1"}}"#).unwrap();
        let load = settings_in(&scratch.0, &[]);
        let export = |platform, source_version, settings, output_dir| {
            let mut args = export_args("unused");
            args.db_path = Some(db.clone());
            args.dbms = None;
            args.db_server = None;
            args.db_name = None;
            args.platform = platform;
            args.source_version = source_version;
            args.settings = settings;
            args.output_dir = output_dir;
            let config = resolve_connection_with(args.connection(), &load).unwrap();
            export_config_report(
                &config,
                None,
                &args.db_pwd_env,
                &args.output_dir,
                false,
                false,
                Vec::new(),
                None,
                false,
            )
            .unwrap();
            fs::read_to_string(
                args.output_dir
                    .join("CommonForms/ДополнительноеОписание/Ext/Form.xml"),
            )
            .unwrap()
        };
        for mode in 0..2 {
            let exact = export(
                Some(crate::platform::parse("8.5.1.1529").unwrap()),
                None,
                None,
                scratch.0.join(format!("exact-{mode}")),
            );
            assert!(exact.contains("<ButtonImportance>Main</ButtonImportance>"));
            let dialect = export(
                None,
                (mode == 0).then_some(InfobaseConfigSourceVersion::V2_21),
                (mode == 1).then(|| settings.clone()),
                scratch.0.join(format!("dialect-{mode}")),
            );
            assert!(
                !dialect.contains("<ButtonImportance>Main</ButtonImportance>"),
                "dialect-only export inherited a previous exact build (mode {mode})",
            );
            assert!(dialect.contains("<DefaultButton>true</DefaultButton>"));
        }
    }

    #[test]
    fn a_database_entry_names_the_export_format_above_the_environment() {
        let scratch = Scratch::new("binding");
        fs::write(
            scratch.0.join("ibcmd-rs.toml"),
            "[[database]]\nname = \"uha_*\"\nplatform = \"8.5.1\"\n",
        )
        .unwrap();
        let load = settings_in(&scratch.0, &[("IBCMD_RS_PLATFORM", "8.3.27")]);
        let export = |name: &str| {
            resolve_connection_with(export_args(name).connection(), &load)
                .unwrap()
                .legacy_source_version()
                .unwrap()
        };
        // the entry beats IBCMD_RS_PLATFORM; the other database takes it
        assert_eq!(export("uha_85"), InfobaseConfigSourceVersion::V2_21);
        assert_eq!(export("bsp"), InfobaseConfigSourceVersion::V2_20);
        // --platform beats the entry
        let mut args = export_args("uha_85");
        args.platform = Some(crate::platform::parse("8.3.27").unwrap());
        let config = resolve_connection_with(args.connection(), &load).unwrap();
        assert_eq!(
            config.legacy_source_version().unwrap(),
            InfobaseConfigSourceVersion::V2_20
        );
    }

    #[test]
    fn postgres_snapshot_is_consumed_even_when_export_preflight_refuses() {
        let scratch = Scratch::new("postgres-snapshot-preflight");
        let load = settings_in(&scratch.0, &[]);
        let mut args = export_args("synthetic_pg");
        args.dbms = Some("PostgreSQL".into());
        args.platform = Some(crate::platform::parse("8.3.27").unwrap());
        let mut config = resolve_connection_with(args.connection(), &load).unwrap();
        config.postgres_snapshot = Some(crate::mssql_dump::PostgresExportSnapshot::synthetic(
            b"previous rows",
        ));
        let clone = config.clone();
        let out = scratch.0.join("occupied-output");
        fs::create_dir(&out).unwrap();
        fs::write(out.join("existing"), b"unchanged").unwrap();
        assert!(
            export_config_report(
                &config,
                None,
                "UNUSED",
                &out,
                false,
                false,
                Vec::new(),
                None,
                false
            )
            .is_err()
        );
        assert_eq!(fs::read(out.join("existing")).unwrap(), b"unchanged");
        assert!(
            config
                .postgres_snapshot
                .as_ref()
                .unwrap()
                .transfer()
                .unwrap()
                .is_none()
        );
        assert!(
            clone
                .postgres_snapshot
                .as_ref()
                .unwrap()
                .transfer()
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn postgres_snapshot_is_not_retained_when_platform_was_explicit() {
        let scratch = Scratch::new("postgres-explicit-platform-snapshot");
        let load = settings_in(&scratch.0, &[]);
        let mut args = export_args("synthetic_pg");
        args.dbms = Some("PostgreSQL".into());
        args.platform = Some(crate::platform::parse("8.3.27").unwrap());
        let mut config = resolve_connection_with(args.connection(), &load).unwrap();
        config.postgres_snapshot = Some(crate::mssql_dump::PostgresExportSnapshot::synthetic(
            b"previous rows",
        ));
        let mut defaults = Defaults::new(args.native_config.as_deref(), &load);
        settle_platform(&mut config, &args.connection(), &mut defaults).unwrap();
        assert!(config.postgres_snapshot.is_none());
    }

    #[test]
    fn a_malformed_postgres_server_is_refused_before_the_platform_probe() {
        let scratch = Scratch::new("postgres-bad-server");
        let load = settings_in(&scratch.0, &[]);
        let mut args = export_args("disposable_pg");
        args.dbms = Some("PostgreSQL".into());
        args.db_server = Some("srv other".into());
        let error = resolve_connection_with(args.connection(), &load).unwrap_err();
        assert!(
            format!("{error:#}").contains("unrecognised PostgreSQL server `srv other`"),
            "{error:#}"
        );
    }

    #[test]
    fn postgres_default_user_keeps_an_explicit_password() {
        let scratch = Scratch::new("postgres-default-password");
        let load = settings_in(&scratch.0, &[]);
        let mut args = export_args("disposable_pg");
        args.dbms = Some("PostgreSQL".into());
        args.platform = Some(crate::platform::parse("8.3.27").unwrap());
        args.db_pwd = Some("synthetic-secret".into());
        let config = resolve_connection_with(args.connection(), &load).unwrap();
        let target = config.postgres().unwrap().unwrap();
        assert_eq!(target.user, "postgres");
        assert_eq!(target.password.as_deref(), Some("synthetic-secret"));
    }

    #[test]
    fn the_connection_takes_what_its_flags_leave_open_from_the_settings() {
        let scratch = Scratch::new("connection");
        fs::write(
            scratch.0.join("ibcmd-rs.toml"),
            "platform = \"8.3.27\"\ndb-server = \"sql02\"\ndb-user = \"reader\"\n",
        )
        .unwrap();
        let load = settings_in(&scratch.0, &[("IBCMD_DB_PSW", "secret")]);
        let mut args = export_args("bsp");
        args.db_server = None;
        let config = resolve_connection_with(args.connection(), &load).unwrap();
        assert_eq!(config.db_server, "sql02");
        assert_eq!(config.db_user.as_deref(), Some("reader"));
        assert_eq!(config.db_pwd.as_deref(), Some("secret"));
        assert_eq!(config.password_source.as_deref(), Some("IBCMD_DB_PSW"));
        // a flag keeps its value
        let mut args = export_args("bsp");
        args.db_server = Some("sql01".to_string());
        args.db_user = Some("sa".to_string());
        args.db_pwd = Some("flag".to_string());
        let config = resolve_connection_with(args.connection(), &load).unwrap();
        assert_eq!(config.db_server, "sql01");
        assert_eq!(config.db_user.as_deref(), Some("sa"));
        assert_eq!(config.db_pwd.as_deref(), Some("flag"));
    }

    #[test]
    fn an_import_tree_must_be_in_the_format_of_its_databases_platform() {
        let scratch = Scratch::new("import-tree");
        fs::write(
            scratch.0.join("ibcmd-rs.toml"),
            "[[database]]\nname = \"bsp85\"\nplatform = \"8.5.1\"\n",
        )
        .unwrap();
        let tree = scratch.0.join("tree");
        fs::create_dir_all(&tree).unwrap();
        fs::write(
            tree.join("Configuration.xml"),
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" version=\"2.20\"><Configuration uuid=\"00000000-0000-0000-0000-000000000001\"/></MetaDataObject>\n",
        )
        .unwrap();
        let load = settings_in(&scratch.0, &[]);
        let import = |name: &str| {
            let mut args = import_args();
            args.source_version = None;
            args.db_name = Some(name.to_string());
            args.source_dir = tree.clone();
            resolve_connection_with(args.connection(), &load)
        };
        let error = import("bsp85").unwrap_err().to_string();
        assert!(
            error.contains("is XML 2.20, but the platform of bsp85 is 8.5.1"),
            "{error}"
        );
        // nothing names the platform of another database: the tree's own
        let config = import("bsp").unwrap();
        assert!(config.xml_version_given);
        assert_eq!(
            config.legacy_source_version().unwrap(),
            InfobaseConfigSourceVersion::V2_20
        );
    }

    #[test]
    fn builds_import_stage_args_with_sql_auth() {
        let args = import_args();
        let config = resolve_connection(args.connection()).unwrap();
        let stage_args = build_import_stage_args(&config, &args).unwrap();

        assert_eq!(stage_args.server, "localhost");
        assert_eq!(stage_args.sql_user.as_deref(), Some("sa_import"));
        assert_eq!(stage_args.sql_pwd.as_deref(), Some("secret"));
        assert_eq!(stage_args.sql_pwd_env, "IMPORT_SQL_PWD");
        assert_eq!(stage_args.database, "ut_ibcmd");
        assert_eq!(stage_args.batch_size, Some(250));
        assert_eq!(
            stage_args.source_version,
            Some(InfobaseConfigSourceVersion::V2_21)
        );
        assert_eq!(stage_args.path_prefix, vec!["Catalogs/Валюты".to_string()]);
        assert_eq!(
            stage_args.script_output,
            Some(PathBuf::from(r"C:\temp\stage.sql"))
        );
        assert!(!stage_args.base_free);
    }

    #[test]
    fn an_import_without_a_version_reads_the_trees_own() {
        let mut args = import_args();
        args.source_version = None;
        let config = resolve_connection(args.connection()).unwrap();
        assert!(!config.xml_version_given);
        let stage_args = build_import_stage_args(&config, &args).unwrap();
        assert_eq!(stage_args.source_version, None);

        args.stage_mode = InfobaseImportStageMode::BaseFree;
        let stage_args = build_import_stage_args(&config, &args).unwrap();
        assert!(stage_args.base_free);
    }

    #[test]
    fn a_connection_the_flags_complete_does_not_read_the_native_config() {
        // everything is given: the (missing) file is kept, not opened
        let mut args = import_args();
        args.native_config = Some(PathBuf::from(r"C:\ibcmd\ibcmd.yml"));
        let config = resolve_connection(args.connection()).unwrap();
        assert_eq!(
            config.native_config.as_deref(),
            Some(Path::new(r"C:\ibcmd\ibcmd.yml"))
        );
    }

    #[test]
    fn the_native_config_names_what_the_flags_do_not() {
        let scratch = Scratch::new("native");
        fs::write(
            scratch.0.join("ibcmd-rs.toml"),
            "platform = \"8.3.27\"
",
        )
        .unwrap();
        let native = scratch.0.join("ibcmd.yml");
        fs::write(
            &native,
            "database:
  dbms: MSSQLServer
  server: native-server
  name: erp_prod
  user: native-user
  password: native-secret
",
        )
        .unwrap();
        let load = settings_in(&scratch.0, &[]);
        let mut args = export_args("unused");
        args.native_config = Some(native.clone());
        args.dbms = None;
        args.db_server = None;
        args.db_name = None;
        let config = resolve_connection_with(args.connection(), &load).unwrap();
        assert_eq!(config.dbms, "MSSQLServer");
        assert_eq!(config.db_server, "native-server");
        assert_eq!(config.db_name, "erp_prod");
        assert_eq!(config.db_user.as_deref(), Some("native-user"));
        assert_eq!(config.db_pwd.as_deref(), Some("native-secret"));
        assert!(
            config
                .password_source
                .as_deref()
                .is_some_and(|source| source.starts_with("database.password in ")),
            "{:?}",
            config.password_source
        );
        assert_eq!(config.native_config.as_deref(), Some(native.as_path()));
    }

    #[test]
    fn a_non_empty_output_directory_is_refused_by_type() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-export-not-empty-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        let _scratch = crate::test_scratch::TestScratch::adopt(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("x.txt"), b"x").unwrap();
        let error = prepare_output_dir(&root, false).unwrap_err();
        assert!(error.downcast_ref::<OutputDirectoryNotEmpty>().is_some());
        assert!(root.join("x.txt").is_file());
    }

    #[test]
    fn an_extension_export_reads_the_image_the_platform_exports() {
        let scratch = Scratch::new("extension-args");
        let load = settings_in(&scratch.0, &[]);
        // the version is named: nothing reads the database
        let mut export = export_args("ibcmd_rs_x");
        export.source_version = Some(InfobaseConfigSourceVersion::V2_20);
        let config = resolve_connection_with(export.connection(), &load).unwrap();
        let args = extension_dump_args(
            &config,
            None,
            "IBCMD_RS_TEST_NO_SUCH_PSW",
            "Расширение",
            PathBuf::from("out"),
            true,
            InfobaseConfigSourceVersion::V2_20,
        );
        assert_eq!(args.extension.as_deref(), Some("Расширение"));
        assert!(!args.all_extensions);
        assert_eq!(args.image, MssqlExtensionImage::Auto);
        assert_eq!(args.database, "ibcmd_rs_x");
        assert_eq!(args.output_dir, PathBuf::from("out"));
        assert!(args.overwrite);
        assert!(!args.sqlcmd_trust_cert);
        assert_eq!(args.platform, None);
    }

    #[test]
    fn an_extension_export_refuses_a_non_empty_directory_before_reading() {
        let scratch = Scratch::new("extension-not-empty");
        let load = settings_in(&scratch.0, &[]);
        // the version is named: nothing reads the database
        let mut export = export_args("ibcmd_rs_x");
        export.source_version = Some(InfobaseConfigSourceVersion::V2_20);
        let config = resolve_connection_with(export.connection(), &load).unwrap();
        let out = scratch.0.join("out");
        fs::create_dir_all(&out).unwrap();
        fs::write(out.join("x.txt"), b"x").unwrap();
        // the directory is refused before the database is read
        let error = export_extension_report(
            &config,
            None,
            "IBCMD_RS_TEST_NO_SUCH_PSW",
            "Расширение",
            &out,
        )
        .unwrap_err();
        assert!(error.downcast_ref::<OutputDirectoryNotEmpty>().is_some());
        assert!(out.join("x.txt").is_file());
    }
}
