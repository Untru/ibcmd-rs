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
    /// Dry-run pack SpreadsheetDocument templates from a 1C XML source tree.
    AuditSpreadsheetTemplates(AuditSpreadsheetTemplatesArgs),
    /// Dry-run SpreadsheetDocument pack/extract/repack round-trip from a 1C XML source tree.
    AuditSpreadsheetRoundtrip(AuditSpreadsheetRoundtripArgs),
    /// Audit managed Form.xml source coverage and complexity.
    AuditFormSources(AuditFormSourcesArgs),
    /// Audit source-tree files that current SQL loader can or cannot consume.
    AuditSourceLoadCoverage(AuditSourceLoadCoverageArgs),
    /// Build a load plan by comparing manifests.
    Plan(PlanArgs),
    /// Print the current compatibility matrix for implemented operations.
    Compatibility(CompatibilityArgs),
    /// Run an external command, measure it, and capture stdout/stderr.
    ProfileRun(ProfileRunArgs),
    /// Export a 1C infobase configuration or extension to hierarchical XML sources.
    DumpSources(DumpSourcesArgs),
    /// Dump Config/ConfigSave storage rows directly from SQL Server.
    MssqlDumpConfig(MssqlDumpConfigArgs),
    /// Write SQL Server and tech-log trace templates for an ibcmd run.
    TraceTemplate(TraceTemplateArgs),
    /// Analyze exported SQL Server Extended Events XML.
    TraceAnalyze(TraceAnalyzeArgs),
    /// Build a storage-mapping report from exported SQL Server Extended Events XML.
    StorageMap(TraceAnalyzeArgs),
    /// Compare two SQL Server 1C databases by table shape and row counts.
    MssqlCompare(MssqlCompareArgs),
    /// Dry-run source load parity against SQL base blobs without writing ConfigSave.
    MssqlAuditSourceParity(MssqlAuditSourceParityArgs),
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
    /// Stage all root metadata XML objects found under a source tree.
    MssqlStageSourceMetadataObjects(MssqlStageSourceMetadataObjectsArgs),
    /// Stage all common module objects found under a source tree.
    MssqlStageSourceCommonModuleObjects(MssqlStageSourceCommonModuleObjectsArgs),
    /// Stage all supported root XML objects and common modules found under a source tree.
    MssqlStageSourceObjects(MssqlStageSourceObjectsArgs),
    /// Stage one exchange plan object from XML.
    MssqlStageExchangePlanObject(MssqlStageExchangePlanObjectArgs),
    /// Stage one business process object from XML.
    MssqlStageBusinessProcessObject(MssqlStageBusinessProcessObjectArgs),
    /// Stage one document journal object from XML.
    MssqlStageDocumentJournalObject(MssqlStageDocumentJournalObjectArgs),
    /// Stage one report object from XML.
    MssqlStageReportObject(MssqlStageReportObjectArgs),
    /// Stage one data processor object from XML.
    MssqlStageDataProcessorObject(MssqlStageDataProcessorObjectArgs),
    /// Stage one catalog object from XML.
    MssqlStageCatalogObject(MssqlStageCatalogObjectArgs),
    /// Stage one information register object from XML.
    MssqlStageInformationRegisterObject(MssqlStageInformationRegisterObjectArgs),
    /// Stage one scheduled job object from XML.
    MssqlStageScheduledJobObject(MssqlStageScheduledJobObjectArgs),
    /// Stage one XDTO package object from XML.
    MssqlStageXdtopackageObject(MssqlStageXdtopackageObjectArgs),
    /// Stage one role object from XML.
    MssqlStageRoleObject(MssqlStageRoleObjectArgs),
    /// Stage one constant object from XML.
    MssqlStageConstantObject(MssqlStageConstantObjectArgs),
    /// Stage one defined type object from XML.
    MssqlStageDefinedTypeObject(MssqlStageDefinedTypeObjectArgs),
    /// Stage one session parameter object from XML.
    MssqlStageSessionParameterObject(MssqlStageSessionParameterObjectArgs),
    /// Stage one settings storage object from XML.
    MssqlStageSettingsStorageObject(MssqlStageSettingsStorageObjectArgs),
    /// Stage one functional option object from XML.
    MssqlStageFunctionalOptionObject(MssqlStageFunctionalOptionObjectArgs),
    /// Stage one functional options parameter object from XML.
    MssqlStageFunctionalOptionsParameterObject(MssqlStageFunctionalOptionsParameterObjectArgs),
    /// Stage one event subscription object from XML.
    MssqlStageEventSubscriptionObject(MssqlStageEventSubscriptionObjectArgs),
    /// Stage one HTTP service object from XML.
    MssqlStageHTTPServiceObject(MssqlStageHTTPServiceObjectArgs),
    /// Stage one web service object from XML.
    MssqlStageWebServiceObject(MssqlStageWebServiceObjectArgs),
    /// Stage one common attribute object from XML.
    MssqlStageCommonAttributeObject(MssqlStageCommonAttributeObjectArgs),
    /// Stage one language object from XML.
    MssqlStageLanguageObject(MssqlStageLanguageObjectArgs),
    /// Stage one style item object from XML.
    MssqlStageStyleItemObject(MssqlStageStyleItemObjectArgs),
    /// Stage one style object from XML.
    MssqlStageStyleObject(MssqlStageStyleObjectArgs),
    /// Stage one bot object from XML.
    MssqlStageBotObject(MssqlStageBotObjectArgs),
    /// Stage one document numerator object from XML.
    MssqlStageDocumentNumeratorObject(MssqlStageDocumentNumeratorObjectArgs),
    /// Stage one integration service object from XML.
    MssqlStageIntegrationServiceObject(MssqlStageIntegrationServiceObjectArgs),
    /// Stage one sequence object from XML.
    MssqlStageSequenceObject(MssqlStageSequenceObjectArgs),
    /// Stage one WS reference object from XML.
    MssqlStageWSReferenceObject(MssqlStageWSReferenceObjectArgs),
    /// Stage one task object from XML.
    MssqlStageTaskObject(MssqlStageTaskObjectArgs),
    /// Stage one subsystem object from XML.
    MssqlStageSubsystemObject(MssqlStageSubsystemObjectArgs),
    /// Stage one command group object from XML.
    MssqlStageCommandGroupObject(MssqlStageCommandGroupObjectArgs),
    /// Stage one enum object from XML.
    MssqlStageEnumObject(MssqlStageEnumObjectArgs),
    /// Stage one document object from XML.
    MssqlStageDocumentObject(MssqlStageDocumentObjectArgs),
    /// Stage one filter criteria object from XML.
    MssqlStageFilterCriteriaObject(MssqlStageFilterCriteriaObjectArgs),
    /// Stage one accounting register object from XML.
    MssqlStageAccountingRegisterObject(MssqlStageAccountingRegisterObjectArgs),
    /// Stage one accumulation register object from XML.
    MssqlStageAccumulationRegisterObject(MssqlStageAccumulationRegisterObjectArgs),
    /// Stage one calculation register object from XML.
    MssqlStageCalculationRegisterObject(MssqlStageCalculationRegisterObjectArgs),
    /// Stage one chart of characteristic types object from XML.
    MssqlStageChartOfCharacteristicTypesObject(MssqlStageChartOfCharacteristicTypesObjectArgs),
    /// Stage one chart of accounts object from XML.
    MssqlStageChartOfAccountsObject(MssqlStageChartOfAccountsObjectArgs),
    /// Stage one chart of calculation types object from XML.
    MssqlStageChartOfCalculationTypesObject(MssqlStageChartOfCalculationTypesObjectArgs),
    /// Stage one chart of calculation registers object from XML.
    MssqlStageChartOfCalculationRegistersObject(MssqlStageChartOfCalculationRegistersObjectArgs),
    /// Stage one common command object from XML.
    MssqlStageCommonCommandObject(MssqlStageCommonCommandObjectArgs),
    /// Stage one common form object from XML.
    MssqlStageCommonFormObject(MssqlStageCommonFormObjectArgs),
    /// Stage one common picture object from XML.
    MssqlStageCommonPictureObject(MssqlStageCommonPictureObjectArgs),
    /// Stage one common template object from XML.
    MssqlStageCommonTemplateObject(MssqlStageCommonTemplateObjectArgs),
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
pub struct AuditSpreadsheetTemplatesArgs {
    /// Root folder with 1C XML sources.
    pub root: PathBuf,
    /// Optional JSON output file. Prints to stdout when omitted.
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct AuditSpreadsheetRoundtripArgs {
    /// Root folder with 1C XML sources.
    pub root: PathBuf,
    /// Optional JSON output file. Prints to stdout when omitted.
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct AuditFormSourcesArgs {
    /// Root folder with 1C XML sources.
    pub root: PathBuf,
    /// Optional JSON output file. Prints to stdout when omitted.
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct AuditSourceLoadCoverageArgs {
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
pub struct DumpSourcesArgs {
    /// Optional autumn-properties.json compatible settings file.
    #[arg(long)]
    pub settings: Option<PathBuf>,
    /// ibcmd executable path. Auto-detects a recent 8.3 build when omitted.
    #[arg(long)]
    pub ibcmd: Option<PathBuf>,
    /// DBMS type passed to ibcmd.
    #[arg(long)]
    pub dbms: Option<String>,
    /// Database server passed to ibcmd.
    #[arg(long)]
    pub db_server: Option<String>,
    /// Database name passed to ibcmd.
    #[arg(long)]
    pub db_name: Option<String>,
    /// Database user passed to ibcmd.
    #[arg(long)]
    pub db_user: Option<String>,
    /// Database password passed to ibcmd. Prefer --db-pwd-env for shell history.
    #[arg(long)]
    pub db_pwd: Option<String>,
    /// Environment variable containing the database password.
    #[arg(long, default_value = "IBCMD_DB_PSW")]
    pub db_pwd_env: String,
    /// Output directory for hierarchical XML sources.
    #[arg(short, long)]
    pub output_dir: PathBuf,
    /// Extension name. Omit to export the main configuration.
    #[arg(long)]
    pub extension: Option<String>,
    /// ibcmd --data directory. Uses a temporary directory when omitted.
    #[arg(long)]
    pub data_dir: Option<PathBuf>,
    /// Kill ibcmd after this many seconds.
    #[arg(long, default_value_t = 300)]
    pub timeout_sec: u64,
    /// Replace files under the output directory after a successful export.
    #[arg(long)]
    pub overwrite: bool,
    /// Convert TaxiEnableVersion8_2 to TaxiEnableOld in exported Configuration.xml.
    #[arg(long)]
    pub normalize_taxi_old: bool,
}

#[derive(Debug, Args)]
pub struct MssqlDumpConfigArgs {
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// SQL Server name.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// SQL Server database name.
    #[arg(long)]
    pub database: String,
    /// Output directory for dumped rows and manifest.json.
    #[arg(short, long)]
    pub output_dir: PathBuf,
    /// Replace files under the output directory.
    #[arg(long)]
    pub overwrite: bool,
    /// Include pending ConfigSave rows in addition to Config.
    #[arg(long)]
    pub include_config_save: bool,
    /// Dump only selected Config/ConfigSave FileName values. Can be repeated.
    #[arg(long = "file-name")]
    pub file_names: Vec<String>,
    /// Try to inflate raw deflate blobs and write readable *.txt files.
    #[arg(long)]
    pub inflate: bool,
    /// Extract module `text` elements into <table>_module_text/*.bsl when a row is a module blob.
    #[arg(long)]
    pub extract_module_text: bool,
    /// Try to reconstruct minimal source XML for recognized metadata blobs.
    #[arg(long)]
    pub extract_metadata_xml: bool,
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
pub struct MssqlAuditSourceParityArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Baseline database name whose Config blobs are used for dry-run packing.
    #[arg(long)]
    pub database: String,
    /// Root folder with XML sources to scan.
    #[arg(long)]
    pub source_root: PathBuf,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Optional maximum number of staged XML objects per SQL batch.
    #[arg(long)]
    pub batch_size: Option<usize>,
    /// Optional source path prefix to audit. Can be repeated.
    #[arg(long)]
    pub path_prefix: Vec<String>,
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
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
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
    /// Pass bcp -u (trust server certificate). Needed for bcp 18+ over an
    /// encrypted connection to a server with a self-signed certificate;
    /// bcp 13 and earlier reject -u, so it must stay off there.
    #[arg(long)]
    pub bcp_trust_cert: bool,
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
    /// Pass bcp -u (trust server certificate). Needed for bcp 18+ over an
    /// encrypted connection to a server with a self-signed certificate;
    /// bcp 13 and earlier reject -u, so it must stay off there.
    #[arg(long)]
    pub bcp_trust_cert: bool,
    /// Required confirmation: delete existing Config/ConfigSave/Params rows first.
    #[arg(long)]
    pub replace: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
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
    /// Pass bcp -u (trust server certificate). Needed for bcp 18+ over an
    /// encrypted connection to a server with a self-signed certificate;
    /// bcp 13 and earlier reject -u, so it must stay off there.
    #[arg(long)]
    pub bcp_trust_cert: bool,
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
    /// Pass bcp -u (trust server certificate). Needed for bcp 18+ over an
    /// encrypted connection to a server with a self-signed certificate;
    /// bcp 13 and earlier reject -u, so it must stay off there.
    #[arg(long)]
    pub bcp_trust_cert: bool,
    /// Delete existing ConfigSave rows before import.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
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
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
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
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
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
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
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
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
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
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
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
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageSourceMetadataObjectsArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Root folder with XML sources to scan.
    #[arg(long)]
    pub source_root: PathBuf,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageSourceCommonModuleObjectsArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Root folder with XML sources to scan.
    #[arg(long)]
    pub source_root: PathBuf,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageSourceObjectsArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Root folder with XML sources to scan.
    #[arg(long)]
    pub source_root: PathBuf,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional maximum number of staged XML objects per SQL batch.
    #[arg(long)]
    pub batch_size: Option<usize>,
    /// Optional source path prefix to stage. Can be repeated.
    #[arg(long)]
    pub path_prefix: Vec<String>,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageExchangePlanObjectArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Exchange plan XML file.
    #[arg(long)]
    pub xml: PathBuf,
    /// Root folder with full XML sources, used to resolve metadata references.
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageBusinessProcessObjectArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Business process XML file.
    #[arg(long)]
    pub xml: PathBuf,
    /// Root folder with full XML sources, used to resolve metadata references.
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageDocumentJournalObjectArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Document journal XML file.
    #[arg(long)]
    pub xml: PathBuf,
    /// Root folder with full XML sources, used to resolve metadata references.
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageReportObjectArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Report XML file.
    #[arg(long)]
    pub xml: PathBuf,
    /// Root folder with full XML sources, used to resolve metadata references.
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageDataProcessorObjectArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Data processor XML file.
    #[arg(long)]
    pub xml: PathBuf,
    /// Root folder with full XML sources, used to resolve metadata references.
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageCatalogObjectArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Catalog XML file.
    #[arg(long)]
    pub xml: PathBuf,
    /// Root folder with full XML sources, used to resolve metadata references.
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageInformationRegisterObjectArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Information register XML file.
    #[arg(long)]
    pub xml: PathBuf,
    /// Root folder with full XML sources, used to resolve metadata references.
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageScheduledJobObjectArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Scheduled job XML file.
    #[arg(long)]
    pub xml: PathBuf,
    /// Root folder with full XML sources, used to resolve metadata references.
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageXdtopackageObjectArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// XDTO package XML file.
    #[arg(long)]
    pub xml: PathBuf,
    /// Root folder with full XML sources, used to resolve metadata references.
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageRoleObjectArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Role XML file.
    #[arg(long)]
    pub xml: PathBuf,
    /// Root folder with full XML sources, used to resolve metadata references.
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageConstantObjectArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Constant XML file.
    #[arg(long)]
    pub xml: PathBuf,
    /// Root folder with full XML sources, used to resolve metadata references.
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageDefinedTypeObjectArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Defined type XML file.
    #[arg(long)]
    pub xml: PathBuf,
    /// Root folder with full XML sources, used to resolve metadata references.
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageSessionParameterObjectArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Session parameter XML file.
    #[arg(long)]
    pub xml: PathBuf,
    /// Root folder with full XML sources, used to resolve metadata references.
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageSettingsStorageObjectArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Settings storage XML file.
    #[arg(long)]
    pub xml: PathBuf,
    /// Root folder with full XML sources, used to resolve metadata references.
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageFunctionalOptionObjectArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Functional option XML file.
    #[arg(long)]
    pub xml: PathBuf,
    /// Root folder with full XML sources, used to resolve metadata references.
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageFunctionalOptionsParameterObjectArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Functional options parameter XML file.
    #[arg(long)]
    pub xml: PathBuf,
    /// Root folder with full XML sources, used to resolve metadata references.
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageEventSubscriptionObjectArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Event subscription XML file.
    #[arg(long)]
    pub xml: PathBuf,
    /// Root folder with full XML sources, used to resolve metadata references.
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageHTTPServiceObjectArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// HTTP service XML file.
    #[arg(long)]
    pub xml: PathBuf,
    /// Root folder with full XML sources, used to resolve metadata references.
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageWebServiceObjectArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Web service XML file.
    #[arg(long)]
    pub xml: PathBuf,
    /// Root folder with full XML sources, used to resolve metadata references.
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageCommonAttributeObjectArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Common attribute XML file.
    #[arg(long)]
    pub xml: PathBuf,
    /// Root folder with full XML sources, used to resolve metadata references.
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageLanguageObjectArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Language XML file.
    #[arg(long)]
    pub xml: PathBuf,
    /// Root folder with full XML sources, used to resolve metadata references.
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageStyleItemObjectArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Style item XML file.
    #[arg(long)]
    pub xml: PathBuf,
    /// Root folder with full XML sources, used to resolve metadata references.
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageStyleObjectArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Style XML file.
    #[arg(long)]
    pub xml: PathBuf,
    /// Root folder with full XML sources, used to resolve metadata references.
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageBotObjectArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Bot XML file.
    #[arg(long)]
    pub xml: PathBuf,
    /// Root folder with full XML sources, used to resolve metadata references.
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageDocumentNumeratorObjectArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Document numerator XML file.
    #[arg(long)]
    pub xml: PathBuf,
    /// Root folder with full XML sources, used to resolve metadata references.
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageIntegrationServiceObjectArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Integration service XML file.
    #[arg(long)]
    pub xml: PathBuf,
    /// Root folder with full XML sources, used to resolve metadata references.
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageSequenceObjectArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Sequence XML file.
    #[arg(long)]
    pub xml: PathBuf,
    /// Root folder with full XML sources, used to resolve metadata references.
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageWSReferenceObjectArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// WS reference XML file.
    #[arg(long)]
    pub xml: PathBuf,
    /// Root folder with full XML sources, used to resolve metadata references.
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageTaskObjectArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Task XML file.
    #[arg(long)]
    pub xml: PathBuf,
    /// Root folder with full XML sources, used to resolve metadata references.
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageSubsystemObjectArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Subsystem XML file.
    #[arg(long)]
    pub xml: PathBuf,
    /// Root folder with full XML sources, used to resolve metadata references.
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageCommandGroupObjectArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Command group XML file.
    #[arg(long)]
    pub xml: PathBuf,
    /// Root folder with full XML sources, used to resolve metadata references.
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageEnumObjectArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Enum XML file.
    #[arg(long)]
    pub xml: PathBuf,
    /// Root folder with full XML sources, used to resolve metadata references.
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageDocumentObjectArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Document XML file.
    #[arg(long)]
    pub xml: PathBuf,
    /// Root folder with full XML sources, used to resolve metadata references.
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageFilterCriteriaObjectArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Filter criteria XML file.
    #[arg(long)]
    pub xml: PathBuf,
    /// Root folder with full XML sources, used to resolve metadata references.
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageAccountingRegisterObjectArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Accounting register XML file.
    #[arg(long)]
    pub xml: PathBuf,
    /// Root folder with full XML sources, used to resolve metadata references.
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageAccumulationRegisterObjectArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Accumulation register XML file.
    #[arg(long)]
    pub xml: PathBuf,
    /// Root folder with full XML sources, used to resolve metadata references.
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageCalculationRegisterObjectArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Calculation register XML file.
    #[arg(long)]
    pub xml: PathBuf,
    /// Root folder with full XML sources, used to resolve metadata references.
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageChartOfCharacteristicTypesObjectArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Chart of characteristic types XML file.
    #[arg(long)]
    pub xml: PathBuf,
    /// Root folder with full XML sources, used to resolve metadata references.
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageChartOfAccountsObjectArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Chart of accounts XML file.
    #[arg(long)]
    pub xml: PathBuf,
    /// Root folder with full XML sources, used to resolve metadata references.
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageChartOfCalculationTypesObjectArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Chart of calculation types XML file.
    #[arg(long)]
    pub xml: PathBuf,
    /// Root folder with full XML sources, used to resolve metadata references.
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageChartOfCalculationRegistersObjectArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Chart of calculation registers XML file.
    #[arg(long)]
    pub xml: PathBuf,
    /// Root folder with full XML sources, used to resolve metadata references.
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageCommonCommandObjectArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Common command XML file.
    #[arg(long)]
    pub xml: PathBuf,
    /// Root folder with full XML sources, used to resolve metadata references.
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageCommonFormObjectArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Common form XML file.
    #[arg(long)]
    pub xml: PathBuf,
    /// Root folder with full XML sources, used to resolve metadata references.
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageCommonPictureObjectArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Common picture XML file.
    #[arg(long)]
    pub xml: PathBuf,
    /// Root folder with full XML sources, used to resolve metadata references.
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlStageCommonTemplateObjectArgs {
    /// SQL Server name passed to sqlcmd -S.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Common template XML file.
    #[arg(long)]
    pub xml: PathBuf,
    /// Root folder with full XML sources, used to resolve metadata references.
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    /// sqlcmd executable path.
    #[arg(long, default_value = "sqlcmd")]
    pub sqlcmd: PathBuf,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_exchange_plan_stage_command() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-stage-exchange-plan-object",
            "--database",
            "TestDb",
            "--xml",
            r"ExchangePlans\ОбновлениеИнформационнойБазы.xml",
            "--replace-config-save",
            "--allow-non-lab",
        ]);

        match cli.command {
            Commands::MssqlStageExchangePlanObject(args) => {
                assert_eq!(args.database, "TestDb");
                assert_eq!(
                    args.xml,
                    PathBuf::from(r"ExchangePlans\ОбновлениеИнформационнойБазы.xml")
                );
                assert!(args.replace_config_save);
                assert!(args.allow_non_lab);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_business_process_stage_command() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-stage-business-process-object",
            "--database",
            "TestDb",
            "--xml",
            r"BusinessProcesses\Задание.xml",
            "--replace-config-save",
            "--allow-non-lab",
        ]);

        match cli.command {
            Commands::MssqlStageBusinessProcessObject(args) => {
                assert_eq!(args.database, "TestDb");
                assert_eq!(args.xml, PathBuf::from(r"BusinessProcesses\Задание.xml"));
                assert!(args.replace_config_save);
                assert!(args.allow_non_lab);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_document_journal_stage_command() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-stage-document-journal-object",
            "--database",
            "TestDb",
            "--xml",
            r"DocumentJournals\Взаимодействия.xml",
            "--replace-config-save",
            "--allow-non-lab",
        ]);

        match cli.command {
            Commands::MssqlStageDocumentJournalObject(args) => {
                assert_eq!(args.database, "TestDb");
                assert_eq!(
                    args.xml,
                    PathBuf::from(r"DocumentJournals\Взаимодействия.xml")
                );
                assert!(args.replace_config_save);
                assert!(args.allow_non_lab);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_report_stage_command() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-stage-report-object",
            "--database",
            "TestDb",
            "--xml",
            r"Reports\БизнесПроцессы.xml",
            "--replace-config-save",
            "--allow-non-lab",
        ]);

        match cli.command {
            Commands::MssqlStageReportObject(args) => {
                assert_eq!(args.database, "TestDb");
                assert_eq!(args.xml, PathBuf::from(r"Reports\БизнесПроцессы.xml"));
                assert!(args.replace_config_save);
                assert!(args.allow_non_lab);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_data_processor_stage_command() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-stage-data-processor-object",
            "--database",
            "TestDb",
            "--xml",
            r"DataProcessors\ВыгрузкаЗагрузкаEnterpriseData.xml",
            "--replace-config-save",
            "--allow-non-lab",
        ]);

        match cli.command {
            Commands::MssqlStageDataProcessorObject(args) => {
                assert_eq!(args.database, "TestDb");
                assert_eq!(
                    args.xml,
                    PathBuf::from(r"DataProcessors\ВыгрузкаЗагрузкаEnterpriseData.xml")
                );
                assert!(args.replace_config_save);
                assert!(args.allow_non_lab);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_catalog_stage_command() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-stage-catalog-object",
            "--database",
            "TestDb",
            "--xml",
            r"Catalogs\Валюты.xml",
            "--replace-config-save",
            "--allow-non-lab",
        ]);

        match cli.command {
            Commands::MssqlStageCatalogObject(args) => {
                assert_eq!(args.database, "TestDb");
                assert_eq!(args.xml, PathBuf::from(r"Catalogs\Валюты.xml"));
                assert!(args.replace_config_save);
                assert!(args.allow_non_lab);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_information_register_stage_command() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-stage-information-register-object",
            "--database",
            "TestDb",
            "--xml",
            r"InformationRegisters\ВерсииОбъектов.xml",
            "--replace-config-save",
            "--allow-non-lab",
        ]);

        match cli.command {
            Commands::MssqlStageInformationRegisterObject(args) => {
                assert_eq!(args.database, "TestDb");
                assert_eq!(
                    args.xml,
                    PathBuf::from(r"InformationRegisters\ВерсииОбъектов.xml")
                );
                assert!(args.replace_config_save);
                assert!(args.allow_non_lab);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_scheduled_job_stage_command() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-stage-scheduled-job-object",
            "--database",
            "TestDb",
            "--xml",
            r"ScheduledJobs\ЗагрузкаКурсовВалют.xml",
            "--replace-config-save",
            "--allow-non-lab",
        ]);

        match cli.command {
            Commands::MssqlStageScheduledJobObject(args) => {
                assert_eq!(args.database, "TestDb");
                assert_eq!(
                    args.xml,
                    PathBuf::from(r"ScheduledJobs\ЗагрузкаКурсовВалют.xml")
                );
                assert!(args.replace_config_save);
                assert!(args.allow_non_lab);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_xdto_package_stage_command() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-stage-xdtopackage-object",
            "--database",
            "TestDb",
            "--xml",
            r"XDTOPackages\АдминистрированиеОбменаДанными_2_4_5_1.xml",
            "--replace-config-save",
            "--allow-non-lab",
        ]);

        match cli.command {
            Commands::MssqlStageXdtopackageObject(args) => {
                assert_eq!(args.database, "TestDb");
                assert_eq!(
                    args.xml,
                    PathBuf::from(r"XDTOPackages\АдминистрированиеОбменаДанными_2_4_5_1.xml")
                );
                assert!(args.replace_config_save);
                assert!(args.allow_non_lab);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_role_stage_command() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-stage-role-object",
            "--database",
            "TestDb",
            "--xml",
            r"Roles\АдминистраторСистемы.xml",
            "--replace-config-save",
            "--allow-non-lab",
        ]);

        match cli.command {
            Commands::MssqlStageRoleObject(args) => {
                assert_eq!(args.database, "TestDb");
                assert_eq!(args.xml, PathBuf::from(r"Roles\АдминистраторСистемы.xml"));
                assert!(args.replace_config_save);
                assert!(args.allow_non_lab);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_constant_stage_command() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-stage-constant-object",
            "--database",
            "TestDb",
            "--xml",
            r"Constants\АвтоматическиНастраиватьРазрешенияВПрофиляхБезопасности.xml",
            "--replace-config-save",
            "--allow-non-lab",
        ]);

        match cli.command {
            Commands::MssqlStageConstantObject(args) => {
                assert_eq!(args.database, "TestDb");
                assert_eq!(
                    args.xml,
                    PathBuf::from(
                        r"Constants\АвтоматическиНастраиватьРазрешенияВПрофиляхБезопасности.xml"
                    )
                );
                assert!(args.replace_config_save);
                assert!(args.allow_non_lab);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_defined_type_stage_command() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-stage-defined-type-object",
            "--database",
            "TestDb",
            "--xml",
            r"DefinedTypes\Пользователь.xml",
            "--replace-config-save",
            "--allow-non-lab",
        ]);

        match cli.command {
            Commands::MssqlStageDefinedTypeObject(args) => {
                assert_eq!(args.database, "TestDb");
                assert_eq!(args.xml, PathBuf::from(r"DefinedTypes\Пользователь.xml"));
                assert!(args.replace_config_save);
                assert!(args.allow_non_lab);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_session_parameter_stage_command() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-stage-session-parameter-object",
            "--database",
            "TestDb",
            "--xml",
            r"SessionParameters\АвторизованныйПользователь.xml",
            "--replace-config-save",
            "--allow-non-lab",
        ]);

        match cli.command {
            Commands::MssqlStageSessionParameterObject(args) => {
                assert_eq!(args.database, "TestDb");
                assert_eq!(
                    args.xml,
                    PathBuf::from(r"SessionParameters\АвторизованныйПользователь.xml")
                );
                assert!(args.replace_config_save);
                assert!(args.allow_non_lab);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_settings_storage_stage_command() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-stage-settings-storage-object",
            "--database",
            "TestDb",
            "--xml",
            r"SettingsStorages\ХранилищеВариантовОтчетов.xml",
            "--replace-config-save",
            "--allow-non-lab",
        ]);

        match cli.command {
            Commands::MssqlStageSettingsStorageObject(args) => {
                assert_eq!(args.database, "TestDb");
                assert_eq!(
                    args.xml,
                    PathBuf::from(r"SettingsStorages\ХранилищеВариантовОтчетов.xml")
                );
                assert!(args.replace_config_save);
                assert!(args.allow_non_lab);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_functional_option_stage_command() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-stage-functional-option-object",
            "--database",
            "TestDb",
            "--xml",
            r"FunctionalOptions\ВыполнятьЗамерыПроизводительности.xml",
            "--replace-config-save",
            "--allow-non-lab",
        ]);

        match cli.command {
            Commands::MssqlStageFunctionalOptionObject(args) => {
                assert_eq!(args.database, "TestDb");
                assert_eq!(
                    args.xml,
                    PathBuf::from(r"FunctionalOptions\ВыполнятьЗамерыПроизводительности.xml")
                );
                assert!(args.replace_config_save);
                assert!(args.allow_non_lab);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_functional_options_parameter_stage_command() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-stage-functional-options-parameter-object",
            "--database",
            "TestDb",
            "--xml",
            r"FunctionalOptionsParameters\ОбщиеНастройкиУзлов.xml",
            "--replace-config-save",
            "--allow-non-lab",
        ]);

        match cli.command {
            Commands::MssqlStageFunctionalOptionsParameterObject(args) => {
                assert_eq!(args.database, "TestDb");
                assert_eq!(
                    args.xml,
                    PathBuf::from(r"FunctionalOptionsParameters\ОбщиеНастройкиУзлов.xml")
                );
                assert!(args.replace_config_save);
                assert!(args.allow_non_lab);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_event_subscription_stage_command() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-stage-event-subscription-object",
            "--database",
            "TestDb",
            "--xml",
            r"EventSubscriptions\СобытиеПередЗаписью.xml",
            "--replace-config-save",
            "--allow-non-lab",
        ]);

        match cli.command {
            Commands::MssqlStageEventSubscriptionObject(args) => {
                assert_eq!(args.database, "TestDb");
                assert_eq!(
                    args.xml,
                    PathBuf::from(r"EventSubscriptions\СобытиеПередЗаписью.xml")
                );
                assert!(args.replace_config_save);
                assert!(args.allow_non_lab);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_http_service_stage_command() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-stage-http-service-object",
            "--database",
            "TestDb",
            "--xml",
            r"HTTPServices\exchange_dsl_1_0_0_1.xml",
            "--replace-config-save",
            "--allow-non-lab",
        ]);

        match cli.command {
            Commands::MssqlStageHTTPServiceObject(args) => {
                assert_eq!(args.database, "TestDb");
                assert_eq!(
                    args.xml,
                    PathBuf::from(r"HTTPServices\exchange_dsl_1_0_0_1.xml")
                );
                assert!(args.replace_config_save);
                assert!(args.allow_non_lab);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_web_service_stage_command() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-stage-web-service-object",
            "--database",
            "TestDb",
            "--xml",
            r"WebServices\RemoteControl.xml",
            "--replace-config-save",
            "--allow-non-lab",
        ]);

        match cli.command {
            Commands::MssqlStageWebServiceObject(args) => {
                assert_eq!(args.database, "TestDb");
                assert_eq!(args.xml, PathBuf::from(r"WebServices\RemoteControl.xml"));
                assert!(args.replace_config_save);
                assert!(args.allow_non_lab);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_common_attribute_stage_command() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-stage-common-attribute-object",
            "--database",
            "TestDb",
            "--xml",
            r"CommonAttributes\КомментарийЯзык1.xml",
            "--replace-config-save",
            "--allow-non-lab",
        ]);

        match cli.command {
            Commands::MssqlStageCommonAttributeObject(args) => {
                assert_eq!(args.database, "TestDb");
                assert_eq!(
                    args.xml,
                    PathBuf::from(r"CommonAttributes\КомментарийЯзык1.xml")
                );
                assert!(args.replace_config_save);
                assert!(args.allow_non_lab);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_language_stage_command() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-stage-language-object",
            "--database",
            "TestDb",
            "--xml",
            r"Languages\Русский.xml",
            "--replace-config-save",
            "--allow-non-lab",
        ]);

        match cli.command {
            Commands::MssqlStageLanguageObject(args) => {
                assert_eq!(args.database, "TestDb");
                assert_eq!(args.xml, PathBuf::from(r"Languages\Русский.xml"));
                assert!(args.replace_config_save);
                assert!(args.allow_non_lab);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_style_item_stage_command() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-stage-style-item-object",
            "--database",
            "TestDb",
            "--xml",
            r"StyleItems\ВажнаяНадписьШрифт.xml",
            "--replace-config-save",
            "--allow-non-lab",
        ]);

        match cli.command {
            Commands::MssqlStageStyleItemObject(args) => {
                assert_eq!(args.database, "TestDb");
                assert_eq!(
                    args.xml,
                    PathBuf::from(r"StyleItems\ВажнаяНадписьШрифт.xml")
                );
                assert!(args.replace_config_save);
                assert!(args.allow_non_lab);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_new_metadata_stage_commands() {
        macro_rules! assert_stage_command {
            ($name:literal, $variant:ident) => {{
                let cli = Cli::parse_from([
                    "ibcmd-rs",
                    $name,
                    "--database",
                    "TestDb",
                    "--xml",
                    r"Dummy\Object.xml",
                    "--replace-config-save",
                    "--allow-non-lab",
                ]);

                match cli.command {
                    Commands::$variant(args) => {
                        assert_eq!(args.database, "TestDb");
                        assert_eq!(args.xml, PathBuf::from(r"Dummy\Object.xml"));
                    }
                    other => panic!("unexpected command: {other:?}"),
                }
            }};
        }

        assert_stage_command!("mssql-stage-style-object", MssqlStageStyleObject);
        assert_stage_command!("mssql-stage-bot-object", MssqlStageBotObject);
        assert_stage_command!(
            "mssql-stage-document-numerator-object",
            MssqlStageDocumentNumeratorObject
        );
        assert_stage_command!(
            "mssql-stage-integration-service-object",
            MssqlStageIntegrationServiceObject
        );
        assert_stage_command!("mssql-stage-sequence-object", MssqlStageSequenceObject);
        assert_stage_command!(
            "mssql-stage-ws-reference-object",
            MssqlStageWSReferenceObject
        );
    }

    #[test]
    fn parses_source_tree_stage_commands() {
        let audit = Cli::parse_from([
            "ibcmd-rs",
            "audit-spreadsheet-templates",
            r"C:\sources",
            "-o",
            r"C:\audit\spreadsheet.json",
        ]);
        match audit.command {
            Commands::AuditSpreadsheetTemplates(args) => {
                assert_eq!(args.root, PathBuf::from(r"C:\sources"));
                assert_eq!(
                    args.output,
                    Some(PathBuf::from(r"C:\audit\spreadsheet.json"))
                );
            }
            other => panic!("unexpected command: {other:?}"),
        }

        let roundtrip = Cli::parse_from([
            "ibcmd-rs",
            "audit-spreadsheet-roundtrip",
            r"C:\sources",
            "-o",
            r"C:\audit\spreadsheet-roundtrip.json",
        ]);
        match roundtrip.command {
            Commands::AuditSpreadsheetRoundtrip(args) => {
                assert_eq!(args.root, PathBuf::from(r"C:\sources"));
                assert_eq!(
                    args.output,
                    Some(PathBuf::from(r"C:\audit\spreadsheet-roundtrip.json"))
                );
            }
            other => panic!("unexpected command: {other:?}"),
        }

        let forms = Cli::parse_from([
            "ibcmd-rs",
            "audit-form-sources",
            r"C:\sources",
            "-o",
            r"C:\audit\forms.json",
        ]);
        match forms.command {
            Commands::AuditFormSources(args) => {
                assert_eq!(args.root, PathBuf::from(r"C:\sources"));
                assert_eq!(args.output, Some(PathBuf::from(r"C:\audit\forms.json")));
            }
            other => panic!("unexpected command: {other:?}"),
        }

        let load_coverage = Cli::parse_from([
            "ibcmd-rs",
            "audit-source-load-coverage",
            r"C:\sources",
            "-o",
            r"C:\audit\load-coverage.json",
        ]);
        match load_coverage.command {
            Commands::AuditSourceLoadCoverage(args) => {
                assert_eq!(args.root, PathBuf::from(r"C:\sources"));
                assert_eq!(
                    args.output,
                    Some(PathBuf::from(r"C:\audit\load-coverage.json"))
                );
            }
            other => panic!("unexpected command: {other:?}"),
        }

        let metadata = Cli::parse_from([
            "ibcmd-rs",
            "mssql-stage-source-metadata-objects",
            "--database",
            "TestDb",
            "--source-root",
            r"C:\sources",
            "--replace-config-save",
            "--allow-non-lab",
        ]);
        match metadata.command {
            Commands::MssqlStageSourceMetadataObjects(args) => {
                assert_eq!(args.database, "TestDb");
                assert_eq!(args.source_root, PathBuf::from(r"C:\sources"));
                assert!(args.replace_config_save);
                assert!(args.allow_non_lab);
            }
            other => panic!("unexpected command: {other:?}"),
        }

        let common_modules = Cli::parse_from([
            "ibcmd-rs",
            "mssql-stage-source-common-module-objects",
            "--database",
            "TestDb",
            "--source-root",
            r"C:\sources",
            "--replace-config-save",
            "--allow-non-lab",
        ]);
        match common_modules.command {
            Commands::MssqlStageSourceCommonModuleObjects(args) => {
                assert_eq!(args.database, "TestDb");
                assert_eq!(args.source_root, PathBuf::from(r"C:\sources"));
                assert!(args.replace_config_save);
                assert!(args.allow_non_lab);
            }
            other => panic!("unexpected command: {other:?}"),
        }

        let tree = Cli::parse_from([
            "ibcmd-rs",
            "mssql-stage-source-objects",
            "--database",
            "TestDb",
            "--source-root",
            r"C:\sources",
            "--path-prefix",
            "Catalogs/Products",
            "--replace-config-save",
            "--allow-non-lab",
        ]);
        match tree.command {
            Commands::MssqlStageSourceObjects(args) => {
                assert_eq!(args.database, "TestDb");
                assert_eq!(args.source_root, PathBuf::from(r"C:\sources"));
                assert_eq!(args.path_prefix, vec!["Catalogs/Products"]);
                assert!(args.replace_config_save);
                assert!(args.allow_non_lab);
            }
            other => panic!("unexpected command: {other:?}"),
        }

        let parity = Cli::parse_from([
            "ibcmd-rs",
            "mssql-audit-source-parity",
            "--database",
            "TestDb",
            "--source-root",
            r"C:\sources",
            "--batch-size",
            "2",
            "--path-prefix",
            "Catalogs/Products",
            "-o",
            r"C:\audit\parity.json",
        ]);
        match parity.command {
            Commands::MssqlAuditSourceParity(args) => {
                assert_eq!(args.database, "TestDb");
                assert_eq!(args.source_root, PathBuf::from(r"C:\sources"));
                assert_eq!(args.batch_size, Some(2));
                assert_eq!(args.path_prefix, vec!["Catalogs/Products"]);
                assert_eq!(args.output, Some(PathBuf::from(r"C:\audit\parity.json")));
            }
            other => panic!("unexpected command: {other:?}"),
        }

        let tree = Cli::parse_from([
            "ibcmd-rs",
            "mssql-stage-source-objects",
            "--database",
            "TestDb",
            "--source-root",
            r"C:\sources",
            "--replace-config-save",
            "--allow-non-lab",
        ]);
        match tree.command {
            Commands::MssqlStageSourceObjects(args) => {
                assert_eq!(args.database, "TestDb");
                assert_eq!(args.source_root, PathBuf::from(r"C:\sources"));
                assert!(args.replace_config_save);
                assert!(args.allow_non_lab);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_dump_sources_command() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "dump-sources",
            "--settings",
            r"C:\repo\autumn-properties.json",
            "--extension",
            "EmergingTravelGroup",
            "-o",
            r"C:\repo\src\cfe\EmergingTravelGroup",
            "--timeout-sec",
            "180",
            "--overwrite",
            "--normalize-taxi-old",
        ]);

        match cli.command {
            Commands::DumpSources(args) => {
                assert_eq!(
                    args.settings,
                    Some(PathBuf::from(r"C:\repo\autumn-properties.json"))
                );
                assert_eq!(args.extension, Some("EmergingTravelGroup".to_string()));
                assert_eq!(
                    args.output_dir,
                    PathBuf::from(r"C:\repo\src\cfe\EmergingTravelGroup")
                );
                assert_eq!(args.timeout_sec, 180);
                assert!(args.overwrite);
                assert!(args.normalize_taxi_old);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_mssql_dump_config_command() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-dump-config",
            "--database",
            "TestDb",
            "--file-name",
            "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa",
            "-o",
            r"C:\dump",
            "--include-config-save",
            "--inflate",
            "--extract-module-text",
            "--extract-metadata-xml",
            "--overwrite",
        ]);

        match cli.command {
            Commands::MssqlDumpConfig(args) => {
                assert_eq!(args.database, "TestDb");
                assert_eq!(
                    args.file_names,
                    vec!["aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa".to_string()]
                );
                assert_eq!(args.output_dir, PathBuf::from(r"C:\dump"));
                assert!(args.include_config_save);
                assert!(args.inflate);
                assert!(args.extract_module_text);
                assert!(args.extract_metadata_xml);
                assert!(args.overwrite);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_task_stage_command() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-stage-task-object",
            "--database",
            "TestDb",
            "--xml",
            r"Tasks\ЗадачаИсполнителя.xml",
            "--replace-config-save",
            "--allow-non-lab",
        ]);

        match cli.command {
            Commands::MssqlStageTaskObject(args) => {
                assert_eq!(args.database, "TestDb");
                assert_eq!(args.xml, PathBuf::from(r"Tasks\ЗадачаИсполнителя.xml"));
                assert!(args.replace_config_save);
                assert!(args.allow_non_lab);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_subsystem_stage_command() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-stage-subsystem-object",
            "--database",
            "TestDb",
            "--xml",
            r"Subsystems\СтандартныеПодсистемы.xml",
            "--replace-config-save",
            "--allow-non-lab",
        ]);

        match cli.command {
            Commands::MssqlStageSubsystemObject(args) => {
                assert_eq!(args.database, "TestDb");
                assert_eq!(
                    args.xml,
                    PathBuf::from(r"Subsystems\СтандартныеПодсистемы.xml")
                );
                assert!(args.replace_config_save);
                assert!(args.allow_non_lab);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_command_group_stage_command() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-stage-command-group-object",
            "--database",
            "TestDb",
            "--xml",
            r"CommandGroups\ВсеКоманды.xml",
            "--replace-config-save",
            "--allow-non-lab",
        ]);

        match cli.command {
            Commands::MssqlStageCommandGroupObject(args) => {
                assert_eq!(args.database, "TestDb");
                assert_eq!(args.xml, PathBuf::from(r"CommandGroups\ВсеКоманды.xml"));
                assert!(args.replace_config_save);
                assert!(args.allow_non_lab);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_enum_stage_command() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-stage-enum-object",
            "--database",
            "TestDb",
            "--xml",
            r"Enums\СостоянияДокумента.xml",
            "--replace-config-save",
            "--allow-non-lab",
        ]);

        match cli.command {
            Commands::MssqlStageEnumObject(args) => {
                assert_eq!(args.database, "TestDb");
                assert_eq!(args.xml, PathBuf::from(r"Enums\СостоянияДокумента.xml"));
                assert!(args.replace_config_save);
                assert!(args.allow_non_lab);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_document_stage_command() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-stage-document-object",
            "--database",
            "TestDb",
            "--xml",
            r"Documents\РеализацияТоваровУслуг.xml",
            "--replace-config-save",
            "--allow-non-lab",
        ]);

        match cli.command {
            Commands::MssqlStageDocumentObject(args) => {
                assert_eq!(args.database, "TestDb");
                assert_eq!(
                    args.xml,
                    PathBuf::from(r"Documents\РеализацияТоваровУслуг.xml")
                );
                assert!(args.replace_config_save);
                assert!(args.allow_non_lab);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_filter_criteria_stage_command() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-stage-filter-criteria-object",
            "--database",
            "TestDb",
            "--xml",
            r"FilterCriteria\ВажныеОтборы.xml",
            "--replace-config-save",
            "--allow-non-lab",
        ]);

        match cli.command {
            Commands::MssqlStageFilterCriteriaObject(args) => {
                assert_eq!(args.database, "TestDb");
                assert_eq!(args.xml, PathBuf::from(r"FilterCriteria\ВажныеОтборы.xml"));
                assert!(args.replace_config_save);
                assert!(args.allow_non_lab);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_accounting_register_stage_command() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-stage-accounting-register-object",
            "--database",
            "TestDb",
            "--xml",
            r"AccountingRegisters\БухгалтерскийУчет.xml",
            "--replace-config-save",
            "--allow-non-lab",
        ]);

        match cli.command {
            Commands::MssqlStageAccountingRegisterObject(args) => {
                assert_eq!(args.database, "TestDb");
                assert_eq!(
                    args.xml,
                    PathBuf::from(r"AccountingRegisters\БухгалтерскийУчет.xml")
                );
                assert!(args.replace_config_save);
                assert!(args.allow_non_lab);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_accumulation_register_stage_command() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-stage-accumulation-register-object",
            "--database",
            "TestDb",
            "--xml",
            r"AccumulationRegisters\Продажи.xml",
            "--replace-config-save",
            "--allow-non-lab",
        ]);

        match cli.command {
            Commands::MssqlStageAccumulationRegisterObject(args) => {
                assert_eq!(args.database, "TestDb");
                assert_eq!(
                    args.xml,
                    PathBuf::from(r"AccumulationRegisters\Продажи.xml")
                );
                assert!(args.replace_config_save);
                assert!(args.allow_non_lab);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_calculation_register_stage_command() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-stage-calculation-register-object",
            "--database",
            "TestDb",
            "--xml",
            r"CalculationRegisters\Начисления.xml",
            "--replace-config-save",
            "--allow-non-lab",
        ]);

        match cli.command {
            Commands::MssqlStageCalculationRegisterObject(args) => {
                assert_eq!(args.database, "TestDb");
                assert_eq!(
                    args.xml,
                    PathBuf::from(r"CalculationRegisters\Начисления.xml")
                );
                assert!(args.replace_config_save);
                assert!(args.allow_non_lab);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_chart_of_characteristic_types_stage_command() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-stage-chart-of-characteristic-types-object",
            "--database",
            "TestDb",
            "--xml",
            r"ChartsOfCharacteristicTypes\ВидыСвойств.xml",
            "--replace-config-save",
            "--allow-non-lab",
        ]);

        match cli.command {
            Commands::MssqlStageChartOfCharacteristicTypesObject(args) => {
                assert_eq!(args.database, "TestDb");
                assert_eq!(
                    args.xml,
                    PathBuf::from(r"ChartsOfCharacteristicTypes\ВидыСвойств.xml")
                );
                assert!(args.replace_config_save);
                assert!(args.allow_non_lab);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_chart_of_accounts_stage_command() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-stage-chart-of-accounts-object",
            "--database",
            "TestDb",
            "--xml",
            r"ChartsOfAccounts\Хозрасчетный.xml",
            "--replace-config-save",
            "--allow-non-lab",
        ]);

        match cli.command {
            Commands::MssqlStageChartOfAccountsObject(args) => {
                assert_eq!(args.database, "TestDb");
                assert_eq!(
                    args.xml,
                    PathBuf::from(r"ChartsOfAccounts\Хозрасчетный.xml")
                );
                assert!(args.replace_config_save);
                assert!(args.allow_non_lab);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_chart_of_calculation_types_stage_command() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-stage-chart-of-calculation-types-object",
            "--database",
            "TestDb",
            "--xml",
            r"ChartsOfCalculationTypes\ВидыРасчета.xml",
            "--replace-config-save",
            "--allow-non-lab",
        ]);

        match cli.command {
            Commands::MssqlStageChartOfCalculationTypesObject(args) => {
                assert_eq!(args.database, "TestDb");
                assert_eq!(
                    args.xml,
                    PathBuf::from(r"ChartsOfCalculationTypes\ВидыРасчета.xml")
                );
                assert!(args.replace_config_save);
                assert!(args.allow_non_lab);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_chart_of_calculation_registers_stage_command() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-stage-chart-of-calculation-registers-object",
            "--database",
            "TestDb",
            "--xml",
            r"ChartsOfCalculationRegisters\РегистрыРасчета.xml",
            "--replace-config-save",
            "--allow-non-lab",
        ]);

        match cli.command {
            Commands::MssqlStageChartOfCalculationRegistersObject(args) => {
                assert_eq!(args.database, "TestDb");
                assert_eq!(
                    args.xml,
                    PathBuf::from(r"ChartsOfCalculationRegisters\РегистрыРасчета.xml")
                );
                assert!(args.replace_config_save);
                assert!(args.allow_non_lab);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_common_command_stage_command() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-stage-common-command-object",
            "--database",
            "TestDb",
            "--xml",
            r"CommonCommands\АвтономнаяРабота.xml",
            "--replace-config-save",
            "--allow-non-lab",
        ]);

        match cli.command {
            Commands::MssqlStageCommonCommandObject(args) => {
                assert_eq!(args.database, "TestDb");
                assert_eq!(
                    args.xml,
                    PathBuf::from(r"CommonCommands\АвтономнаяРабота.xml")
                );
                assert!(args.replace_config_save);
                assert!(args.allow_non_lab);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_common_form_stage_command() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-stage-common-form-object",
            "--database",
            "TestDb",
            "--xml",
            r"CommonForms\АвтономнаяРабота.xml",
            "--replace-config-save",
            "--allow-non-lab",
        ]);

        match cli.command {
            Commands::MssqlStageCommonFormObject(args) => {
                assert_eq!(args.database, "TestDb");
                assert_eq!(args.xml, PathBuf::from(r"CommonForms\АвтономнаяРабота.xml"));
                assert!(args.replace_config_save);
                assert!(args.allow_non_lab);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_common_picture_stage_command() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-stage-common-picture-object",
            "--database",
            "TestDb",
            "--xml",
            r"CommonPictures\Адрес.xml",
            "--replace-config-save",
            "--allow-non-lab",
        ]);

        match cli.command {
            Commands::MssqlStageCommonPictureObject(args) => {
                assert_eq!(args.database, "TestDb");
                assert_eq!(args.xml, PathBuf::from(r"CommonPictures\Адрес.xml"));
                assert!(args.replace_config_save);
                assert!(args.allow_non_lab);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_common_template_stage_command() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-stage-common-template-object",
            "--database",
            "TestDb",
            "--xml",
            r"CommonTemplates\ВидыДокументовУдостоверяющихЛичность.xml",
            "--replace-config-save",
            "--allow-non-lab",
        ]);

        match cli.command {
            Commands::MssqlStageCommonTemplateObject(args) => {
                assert_eq!(args.database, "TestDb");
                assert_eq!(
                    args.xml,
                    PathBuf::from(r"CommonTemplates\ВидыДокументовУдостоверяющихЛичность.xml")
                );
                assert!(args.replace_config_save);
                assert!(args.allow_non_lab);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }
}
