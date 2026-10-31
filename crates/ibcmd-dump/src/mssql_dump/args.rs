//! The command-line arguments of the exporter's two commands,
//! `mssql-dump-config` and `mssql-save-config`. They moved here from
//! `crate::cli`, which re-exports them at their old paths and wraps them in
//! its `Commands`, so they go wherever the exporter goes.

use std::path::PathBuf;

use clap::Args;

use crate::legacy_version::InfobaseConfigSourceVersion;

/// `mssql-save-config`: the configuration as a .cf, from SQL Server or from
/// a folder of stored Config rows (`--rows-dir`).
#[derive(Debug, Args)]
pub struct MssqlSaveConfigArgs {
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
    /// The bcp.exe of the --sqlcmd path (default: the one beside sqlcmd).
    #[arg(long)]
    pub bcp_executable: Option<PathBuf>,
    /// SQL Server name.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// SQL Server login. Uses Windows (integrated) authentication when omitted.
    #[arg(long)]
    pub sql_user: Option<String>,
    /// SQL Server password. Prefer --sql-pwd-env for shell history.
    #[arg(long)]
    pub sql_pwd: Option<String>,
    /// Environment variable containing the SQL Server password.
    #[arg(long, default_value = "IBCMD_DB_PSW")]
    pub sql_pwd_env: String,
    /// SQL Server database name (not needed with --rows-dir).
    #[arg(long, default_value = "")]
    pub database: String,
    /// Read the Config table from a folder of `<FileName>__part<N>.bin`
    /// files (the stored BinaryData, raw deflate) instead of SQL Server; no
    /// server or database is contacted.
    #[arg(long)]
    pub rows_dir: Option<PathBuf>,
    /// Save the database configuration (the Config table alone), as
    /// `config save --db`. Without it the main configuration is saved: a
    /// completed stage in ConfigSave over Config, as `config export` reads it.
    #[arg(long)]
    pub db: bool,
    /// Replace an existing file (only once the new one is complete).
    #[arg(long)]
    pub overwrite: bool,
    /// The .cf to write.
    pub output: PathBuf,
}

#[derive(Debug, Args)]
pub struct MssqlDumpConfigArgs {
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
    /// The bcp.exe of the --sqlcmd path (default: the one beside sqlcmd).
    #[arg(long)]
    pub bcp_executable: Option<PathBuf>,
    /// Durable atomic JSON journal of the SQL requests (the built-in client's,
    /// or the sqlcmd and bcp runs of --sqlcmd): query digests, never the text.
    #[arg(long)]
    pub runtime_journal: Option<PathBuf>,
    /// SQL Server name.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// SQL Server login. Uses Windows (integrated) authentication when omitted.
    #[arg(long)]
    pub sql_user: Option<String>,
    /// SQL Server password. Prefer --sql-pwd-env for shell history.
    #[arg(long)]
    pub sql_pwd: Option<String>,
    /// Environment variable containing the SQL Server password.
    #[arg(long, default_value = "IBCMD_DB_PSW")]
    pub sql_pwd_env: String,
    /// SQL Server database name (not needed with --rows-dir).
    #[arg(long, default_value = "")]
    pub database: String,
    /// Read the Config table from a folder of `<FileName>__part<N>.bin`
    /// files (the stored BinaryData, raw deflate) instead of SQL Server; no
    /// server or database is contacted.
    #[arg(long)]
    pub rows_dir: Option<PathBuf>,
    /// Read the Config table, and a completed import stage in ConfigSave,
    /// from a file infobase: the directory holding 1Cv8.1CD or the file
    /// itself. No server or platform is used.
    #[arg(long, conflicts_with = "rows_dir")]
    pub file_infobase: Option<PathBuf>,
    /// The PostgreSQL database `infobase config export --dbms=PostgreSQL`
    /// reads (#369): its rows answer every read, as a file infobase's do.
    /// Not a command-line option.
    #[arg(skip)]
    pub postgres: Option<crate::sql::postgres::PgTarget>,
    /// One operation's retained PostgreSQL probe rows; never a CLI option.
    #[arg(skip)]
    pub postgres_snapshot: Option<crate::mssql_dump::PostgresExportSnapshot>,
    /// Write every descriptor through the metadata model (the default for a
    /// full export; kept for scripts that pass it).
    #[arg(long, hide = true, conflicts_with = "legacy_export")]
    pub model_export: bool,
    /// Write the descriptors through the legacy converters instead of the
    /// metadata model (also `IBCMD_RS_LEGACY_EXPORT=1`).
    #[arg(long)]
    pub legacy_export: bool,
    /// Output directory for dumped rows and manifest.json.
    #[arg(short, long)]
    pub output_dir: PathBuf,
    /// Replace files under the output directory.
    #[arg(long)]
    pub overwrite: bool,
    /// Include pending ConfigSave rows in addition to Config.
    #[arg(long)]
    pub include_config_save: bool,
    /// Publish the main configuration, as the platform's export does: the rows
    /// a completed import staged in ConfigSave in place of the Config rows of
    /// the same names, and the staged `versions` (default: the Config table
    /// alone).
    #[arg(long, conflicts_with_all = ["include_config_save", "rows_dir"])]
    pub main_configuration: bool,
    /// Dump only selected Config/ConfigSave FileName values. Can be repeated.
    #[arg(long = "file-name")]
    pub file_names: Vec<String>,
    /// Read selected Config/ConfigSave FileName values from a text file. Can be repeated.
    #[arg(long = "file-name-list")]
    pub file_name_lists: Vec<PathBuf>,
    /// Dump only the rows of this metadata object, by full name
    /// (`Catalog.Банки`, `Catalog.Банки.Form.ФормаЭлемента`, `Configuration`):
    /// its own rows and those of the objects it owns (forms, templates,
    /// recalculations, nested subsystems). Can be repeated; adds to --file-name.
    #[arg(
        long = "object",
        value_name = "FULL_NAME",
        conflicts_with = "include_config_save"
    )]
    pub objects: Vec<String>,
    /// Try to inflate raw deflate blobs and write readable *.txt files.
    #[arg(long)]
    pub inflate: bool,
    /// Extract module `text` elements into <table>_module_text/*.bsl when a row is a module blob.
    #[arg(long)]
    pub extract_module_text: bool,
    /// Try to reconstruct minimal source XML for recognized metadata blobs.
    #[arg(long)]
    pub extract_metadata_xml: bool,
    /// Fail a full Config export when any expected root metadata XML is absent.
    #[arg(
        long,
        requires = "extract_metadata_xml",
        conflicts_with_all = ["file_names", "file_name_lists", "objects"]
    )]
    pub require_complete_root_metadata: bool,
    /// Fail when a reconstructed source asset omits an opaque property.
    #[arg(
        long,
        requires_all = ["extract_metadata_xml", "no_binary_rows"],
        conflicts_with_all = ["file_names", "file_name_lists", "objects"]
    )]
    pub require_complete_source_assets: bool,
    /// Continue a full diagnostic export after form writer rejections that have structured source-asset diagnostics.
    #[arg(
        long,
        requires_all = ["extract_metadata_xml", "no_binary_rows"],
        conflicts_with_all = ["file_names", "file_name_lists", "objects"]
    )]
    pub collect_all_source_asset_diagnostics: bool,
    /// Platform the XML is for: a release (8.3.27, 8.5.1) or an exact build
    /// (8.3.27.2214, 8.5.1.1150); 8.3.x reads and writes XML 2.20, 8.5.x 2.21.
    #[arg(
        long,
        value_name = "VERSION",
        value_parser = ibcmd_platform_spec::platform::parse_flag,
        conflicts_with = "source_version"
    )]
    pub platform: Option<ibcmd_platform_spec::platform::PlatformSpec>,
    /// Source XML version for reconstructed source files.
    #[arg(long, value_enum, default_value_t = InfobaseConfigSourceVersion::V2_20, hide = true)]
    pub source_version: InfobaseConfigSourceVersion,
    /// Do not write raw Config/ConfigSave BinaryData rows. Useful for source parity and faster runs.
    #[arg(long)]
    pub no_binary_rows: bool,
    /// Write raw Config/ConfigSave BinaryData rows under <table>/*.bin.
    #[arg(long, default_value_t = true, hide = true)]
    pub write_binary_rows: bool,
    /// Write manifest.json with row-level dump details.
    #[arg(long, default_value_t = true, hide = true)]
    pub write_manifest: bool,
    /// Export only what changed since the export whose ConfigDumpInfo.xml
    /// this is (`--base` of the platform's `config export`): a row the file
    /// lists with the same configVersion is not converted and none of its
    /// files is written (every row is, when an object or a part was renamed);
    /// ConfigDumpInfo.xml is written for the whole configuration. The output
    /// directory may hold an earlier export.
    #[arg(
        long,
        value_name = "FILE",
        conflicts_with_all = [
            "overwrite",
            "file_names",
            "file_name_lists",
            "include_config_save",
            "require_complete_root_metadata",
            "require_complete_source_assets",
            "collect_all_source_asset_diagnostics",
        ]
    )]
    pub base: Option<PathBuf>,
    /// Bring the output directory in line with the configuration (`--sync`
    /// of the platform's `config export`): after the export, remove the files
    /// a fresh full export would not hold (objects removed or renamed, files a
    /// changed object no longer has). Only Configuration.xml,
    /// ConfigDumpInfo.xml, Ext/ and the collection folders are touched, never
    /// a dot directory.
    #[arg(
        long,
        conflicts_with_all = [
            "overwrite",
            "file_names",
            "file_name_lists",
            "include_config_save",
            "require_complete_root_metadata",
            "require_complete_source_assets",
            "collect_all_source_asset_diagnostics",
        ]
    )]
    pub sync: bool,
}
