use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "ibcmd-rs")]
#[command(about = "Research-first replacement path for loading 1C configuration sources")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Locate installed 1C command-line tools and print environment details.
    Probe(ProbeArgs),
    /// Scan a 1C XML source tree and produce a deterministic manifest.
    Scan(ScanArgs),
    /// Build a load plan by comparing manifests.
    Plan(PlanArgs),
    /// Print the current compatibility matrix for implemented operations.
    Compatibility(CompatibilityArgs),
    /// Run an external command, measure it, and capture stdout/stderr.
    ProfileRun(ProfileRunArgs),
    /// Write SQL Server and tech-log trace templates for an ibcmd run.
    TraceTemplate(TraceTemplateArgs),
    /// Analyze exported SQL Server Extended Events XML.
    TraceAnalyze(TraceAnalyzeArgs),
    /// Compare two SQL Server 1C databases by table shape and row counts.
    MssqlCompare(MssqlCompareArgs),
    /// Clone a SQL Server database with backup/restore.
    MssqlClone(MssqlCloneArgs),
    /// Export ConfigSave/Params storage tables to a native BCP bundle.
    MssqlStorageExport(MssqlStorageExportArgs),
    /// Import a native BCP storage bundle into an empty SQL Server infobase.
    MssqlStorageImport(MssqlStorageImportArgs),
    /// Export staged ConfigSave rows as a native BCP delta bundle.
    MssqlDeltaExport(MssqlDeltaExportArgs),
    /// Import staged ConfigSave rows into an existing SQL Server infobase.
    MssqlDeltaImport(MssqlDeltaImportArgs),
    /// Build a 1C common-module body blob from BSL text.
    ModuleBlobPack(ModuleBlobPackArgs),
    /// Patch a Config versions blob for staged ConfigSave changes.
    VersionsBlobPatch(VersionsBlobPatchArgs),
    /// Stage one common module body change directly into SQL Server ConfigSave.
    MssqlStageCommonModule(MssqlStageCommonModuleArgs),
    /// Stage several common module body changes directly into SQL Server ConfigSave.
    MssqlStageCommonModules(MssqlStageCommonModulesArgs),
    /// Stage one common module metadata XML change directly into SQL Server ConfigSave.
    MssqlStageCommonModuleMetadata(MssqlStageCommonModuleMetadataArgs),
    /// Stage one complete common module object from XML and BSL sources.
    MssqlStageCommonModuleObject(MssqlStageCommonModuleObjectArgs),
    /// Stage several complete common module objects from XML and sibling BSL sources.
    MssqlStageCommonModuleObjects(MssqlStageCommonModuleObjectsArgs),
    /// Stage metadata-only XML changes for several simple metadata objects.
    MssqlStageMetadataObjects(MssqlStageMetadataObjectsArgs),
}

#[derive(Debug, Args)]
pub struct ProbeArgs {
    /// Also search common 1C installation folders under Program Files.
    #[arg(long)]
    pub deep: bool,
}

#[derive(Debug, Args)]
pub struct ScanArgs {
    /// Root folder with 1C XML sources.
    pub root: PathBuf,
    /// Optional JSON output file. Prints to stdout when omitted.
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct PlanArgs {
    /// Current manifest JSON produced by `scan`.
    pub current: PathBuf,
    /// Baseline manifest JSON. If omitted, all current files are planned as upserts.
    #[arg(short, long)]
    pub baseline: Option<PathBuf>,
    /// Optional JSON output file. Prints to stdout when omitted.
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct CompatibilityArgs {
    /// Optional JSON output file. Prints to stdout when omitted.
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct ProfileRunArgs {
    /// Keep full stdout/stderr in the JSON report.
    #[arg(long)]
    pub capture_output: bool,
    /// Command and arguments to run. Use `--` before the command.
    #[arg(required = true, trailing_var_arg = true)]
    pub command: Vec<String>,
}

#[derive(Debug, Args)]
pub struct TraceTemplateArgs {
    /// Output directory for generated templates.
    pub output_dir: PathBuf,
    /// Replace existing template files.
    #[arg(long)]
    pub overwrite: bool,
}

#[derive(Debug, Args)]
pub struct TraceAnalyzeArgs {
    /// XML files exported from SQL Server Extended Events.
    #[arg(required = true)]
    pub input: Vec<PathBuf>,
    /// Optional JSON output file. Prints to stdout when omitted.
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlCompareArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Left database name.
    #[arg(long)]
    pub left: String,
    /// Right database name.
    #[arg(long)]
    pub right: String,
    /// Optional JSON output file. Prints to stdout when omitted.
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlCloneArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Source database name.
    #[arg(long)]
    pub source: String,
    /// Target database name.
    #[arg(long)]
    pub target: String,
    /// Backup file used for transfer. Defaults to C:\temp\ibcmd-rs\<source>_to_<target>.bak.
    #[arg(long)]
    pub backup: Option<PathBuf>,
    /// Drop target database first when it already exists.
    #[arg(long)]
    pub overwrite: bool,
}

#[derive(Debug, Args)]
pub struct MssqlStorageExportArgs {
    /// SQL Server name passed to sqlcmd and bcp -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Source database name.
    #[arg(long)]
    pub database: String,
    /// Output bundle directory.
    #[arg(short, long)]
    pub output_dir: PathBuf,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// bcp executable path.
    #[arg(long, default_value = "bcp")]
    pub bcp: PathBuf,
    /// Replace existing bundle files.
    #[arg(long)]
    pub overwrite: bool,
}

#[derive(Debug, Args)]
pub struct MssqlStorageImportArgs {
    /// SQL Server name passed to sqlcmd and bcp -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Input bundle directory produced by `mssql-storage-export`.
    #[arg(short, long)]
    pub input_dir: PathBuf,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// bcp executable path.
    #[arg(long, default_value = "bcp")]
    pub bcp: PathBuf,
    /// Required confirmation: delete existing Config/ConfigSave/Params rows first.
    #[arg(long)]
    pub replace: bool,
}

#[derive(Debug, Args)]
pub struct MssqlDeltaExportArgs {
    /// SQL Server name passed to sqlcmd and bcp -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Source database name with pending rows in ConfigSave.
    #[arg(long)]
    pub database: String,
    /// Output bundle directory.
    #[arg(short, long)]
    pub output_dir: PathBuf,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// bcp executable path.
    #[arg(long, default_value = "bcp")]
    pub bcp: PathBuf,
    /// Replace existing bundle files.
    #[arg(long)]
    pub overwrite: bool,
}

#[derive(Debug, Args)]
pub struct MssqlDeltaImportArgs {
    /// SQL Server name passed to sqlcmd and bcp -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Input bundle directory produced by `mssql-delta-export`.
    #[arg(short, long)]
    pub input_dir: PathBuf,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// bcp executable path.
    #[arg(long, default_value = "bcp")]
    pub bcp: PathBuf,
    /// Delete existing ConfigSave rows before import.
    #[arg(long)]
    pub replace_config_save: bool,
}

#[derive(Debug, Args)]
pub struct ModuleBlobPackArgs {
    /// BSL module body file.
    #[arg(long)]
    pub text: PathBuf,
    /// Output binary blob suitable for Config/ConfigSave BinaryData.
    #[arg(short, long)]
    pub output: PathBuf,
    /// Existing Config/ConfigSave module blob used as a header/template source.
    #[arg(long)]
    pub base_blob: Option<PathBuf>,
    /// Optional module info element. Defaults to `{3,1,0,"",0}` or base blob info.
    #[arg(long)]
    pub info_file: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct VersionsBlobPatchArgs {
    /// Input active Config `versions` blob.
    #[arg(short, long)]
    pub input: PathBuf,
    /// Output staged ConfigSave `versions` blob.
    #[arg(short, long)]
    pub output: PathBuf,
    /// Changed Config/ConfigSave file names whose version UUID must be replaced.
    #[arg(long = "change")]
    pub changes: Vec<String>,
    /// Do not automatically patch root, version and versions entries.
    #[arg(long)]
    pub no_standard_entries: bool,
}

#[derive(Debug, Args)]
pub struct MssqlStageCommonModuleArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Common module metadata UUID without `.0`.
    #[arg(long)]
    pub module_id: String,
    /// BSL module body file.
    #[arg(long)]
    pub text: PathBuf,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageCommonModulesArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Common module change in the form `<metadata-uuid>=<path-to-Module.bsl>`.
    #[arg(long = "module", required = true)]
    pub modules: Vec<String>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageCommonModuleMetadataArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Common module metadata UUID.
    #[arg(long)]
    pub module_id: String,
    /// Common module XML file.
    #[arg(long)]
    pub xml: PathBuf,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageCommonModuleObjectArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Optional CommonModule metadata UUID. When omitted, the UUID is read from XML.
    #[arg(long)]
    pub module_id: Option<String>,
    /// Common module XML file.
    #[arg(long)]
    pub xml: PathBuf,
    /// BSL module body file. Defaults to sibling <module-name>\Ext\Module.bsl.
    #[arg(long)]
    pub text: Option<PathBuf>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageCommonModuleObjectsArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Common module XML files. Each sibling <module-name>\Ext\Module.bsl is loaded too.
    #[arg(long = "xml", required = true)]
    pub xmls: Vec<PathBuf>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageMetadataObjectsArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Metadata XML files for supported metadata-only patchers.
    #[arg(long = "xml", required = true)]
    pub xmls: Vec<PathBuf>,
    /// Root folder with full XML sources, used to resolve metadata references.
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}
