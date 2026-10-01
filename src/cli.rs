use std::ffi::OsString;
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use uuid::Uuid;

pub use crate::legacy_version::InfobaseConfigSourceVersion;
use crate::mssql_platform_profile::MssqlNativePlatformProfile;

#[derive(Debug, Parser)]
#[command(name = "ibcmd-rs")]
#[command(about = "Research-first replacement path for loading 1C configuration sources")]
#[command(version, disable_version_flag = true)]
#[command(disable_help_subcommand = true)]
pub struct Cli {
    /// Print version (`-v` as the platform's ibcmd, `-V` too)
    #[arg(short = 'v', short_alias = 'V', long, action = clap::ArgAction::Version)]
    version: (),
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Convert complete configurations offline through explicit format and profile adapters.
    Convert(ConvertArgs),
    /// Inspect, verify, export, or overlay CF without an installed 1C platform.
    Cf(CfArgs),
    /// Drop-in `ibcmd infobase`: `config export`, `config import` and
    /// `config apply` in the platform ibcmd's syntax, against Microsoft SQL
    /// Server; other commands are refused. `infobase --help` prints its help
    /// (in Russian).
    #[command(disable_help_flag = true)]
    Infobase(NativeModeArgs),
    /// The platform ibcmd's other modes, refused with a clear message.
    #[command(hide = true, disable_help_flag = true)]
    Server(NativeModeArgs),
    #[command(hide = true, disable_help_flag = true)]
    Eventlog(NativeModeArgs),
    #[command(hide = true, disable_help_flag = true)]
    Config(NativeModeArgs),
    #[command(hide = true, disable_help_flag = true)]
    Extension(NativeModeArgs),
    #[command(hide = true, disable_help_flag = true)]
    MobileApp(NativeModeArgs),
    #[command(hide = true, disable_help_flag = true)]
    MobileClient(NativeModeArgs),
    #[command(hide = true, disable_help_flag = true)]
    Session(NativeModeArgs),
    #[command(hide = true, disable_help_flag = true)]
    Lock(NativeModeArgs),
    #[command(hide = true, disable_help_flag = true)]
    BinaryDataStorage(NativeModeArgs),
    /// `help [MODE]`: the platform ibcmd's help mode (Russian) for its
    /// modes, this program's help for its own commands.
    #[command(hide = true, disable_help_flag = true)]
    Help(NativeModeArgs),
    /// Locate installed 1C command-line tools and print environment details.
    #[cfg(feature = "platform-oracle")]
    Probe(ProbeArgs),
    /// Scan a 1C XML source tree and produce a deterministic manifest.
    Scan(ScanArgs),
    /// Dry-run pack SpreadsheetDocument templates from a 1C XML source tree.
    AuditSpreadsheetTemplates(AuditSpreadsheetTemplatesArgs),
    /// Dry-run SpreadsheetDocument pack/extract/repack round-trip from a 1C XML source tree.
    AuditSpreadsheetRoundtrip(AuditSpreadsheetRoundtripArgs),
    /// Audit managed Form.xml source coverage and complexity.
    AuditFormSources(AuditFormSourcesArgs),
    /// Compare two Form.xml/blob pairs and suggest XML path -> layout path mappings.
    FormDiffCandidates(FormDiffCandidatesArgs),
    /// Correlate production Form-layout item traces with native and candidate Form.xml corpora.
    FormProvenanceCorpus(FormProvenanceCorpusArgs),
    /// Trace final MXL palette lines from raw candidate_dump assets.
    MxlLineProvenanceCorpus(MxlLineProvenanceCorpusArgs),
    /// Build and summarize offline semantic indexes from a saved Config dump.
    FormContextSummary(FormContextSummaryArgs),
    /// Audit source-tree files that current SQL loader can or cannot consume.
    AuditSourceLoadCoverage(AuditSourceLoadCoverageArgs),
    /// Census every Form.xml of a source tree against the base-free body model.
    AuditFormBodyBlockers(AuditFormBodyBlockersArgs),
    /// Measure the native form-body writer against the bodies the platform stored.
    AuditNativeFormWriter(AuditNativeFormWriterArgs),
    /// Measure the base-free role rights writer: round trip through the
    /// exporter and parity with the rows the platform stored.
    #[command(hide = true)]
    AuditRoleRightsWriter(AuditRoleRightsWriterArgs),
    /// Measure the base-free command-interface family writers against the stored rows.
    #[command(hide = true)]
    AuditInterfaceWriter(AuditInterfaceWriterArgs),
    /// Measure the help and HTML template writer against the stored rows and
    /// round-trip every row it writes through the exporter.
    #[command(hide = true)]
    AuditHelpWriter(AuditInterfaceWriterArgs),
    /// Round-trip every DataCompositionSchema template through the loader and the exporter.
    AuditDcsTemplateWriter(AuditDcsTemplateWriterArgs),
    /// Measure the spreadsheet template writer: compile, compare with the stored
    /// row, and read the compiled row back into Template.xml.
    #[command(hide = true)]
    AuditMxlWriter(AuditMxlWriterArgs),
    /// Measure the base-free descriptor compiler: compile every metadata XML
    /// without a base row and compare it with the row the platform stored.
    #[command(hide = true)]
    AuditMetadataCompiler(AuditMetadataCompilerArgs),
    /// Build the whole row set an empty infobase load would stage (no base
    /// rows, no database) and compare it with a stored Config row set.
    #[command(hide = true)]
    AuditEmptyStage(AuditEmptyStageArgs),
    /// Measure the export direction of the descriptor model: decode every
    /// stored row, write its XML and compare it with the tree's file.
    #[command(hide = true)]
    AuditMetadataExport(AuditMetadataExportArgs),
    /// Build the export's name index from a folder of stored Config rows
    /// and compare it with the index the source tree gives.
    #[command(hide = true)]
    AuditNameIndex(AuditNameIndexArgs),
    /// Build a load plan by comparing manifests.
    Plan(PlanArgs),
    /// Compare two 1C XML source trees by path and content hash.
    SourceDiff(SourceDiffArgs),
    /// Explain exact indexed XML leaf differences for one source file.
    SourceDiffExplain(SourceDiffExplainArgs),
    /// Summarize repeated XML diff signatures from a source-diff JSON report.
    SourceDiffSignatures(SourceDiffSignaturesArgs),
    /// Build a versioned per-file raw parity matrix from a source-diff JSON report.
    SourceDiffMatrix(SourceDiffMatrixArgs),
    /// Merge independently produced parity matrices without collapsing database results.
    SourceDiffMatrixMerge(SourceDiffMatrixMergeArgs),
    /// Compare pre-existing native, EDT, and ibcmd-rs source trees without launching external tools.
    SourceThreeWayOracle(SourceThreeWayOracleArgs),
    /// Print the current compatibility matrix for implemented operations.
    Compatibility(CompatibilityArgs),
    /// Show the settings a command takes when its flags do not say (ibcmd-rs.toml,
    /// the environment, native --config) and where each value came from.
    Settings(SettingsArgs),
    /// Run an external command, measure it, and capture stdout/stderr.
    #[cfg(feature = "platform-oracle")]
    ProfileRun(ProfileRunArgs),
    /// Export a 1C infobase configuration or extension to hierarchical XML sources.
    #[cfg(feature = "platform-oracle")]
    DumpSources(DumpSourcesArgs),
    /// Dump Config/ConfigSave storage rows directly from SQL Server.
    MssqlDumpConfig(MssqlDumpConfigArgs),
    /// List configuration extensions directly from the SQL Server registry.
    MssqlExtensionList(MssqlExtensionListArgs),
    /// Export one or all configuration extensions directly from SQL Server CAS.
    MssqlDumpExtension(MssqlDumpExtensionArgs),
    /// Compile and stage one or all configuration extensions in ConfigCASSave.
    MssqlLoadExtension(MssqlLoadExtensionArgs),
    /// Publish one already staged extension without native ibcmd.
    MssqlActivateStagedExtension(MssqlActivateStagedExtensionArgs),
    /// Summarize saved mssql-dump-config JSON timing reports.
    MssqlDumpTimingSummary(MssqlDumpTimingSummaryArgs),
    /// Write SQL Server and tech-log trace templates for an ibcmd run.
    TraceTemplate(TraceTemplateArgs),
    /// Analyze exported SQL Server Extended Events XML.
    TraceAnalyze(TraceAnalyzeArgs),
    /// Build a storage-mapping report from exported SQL Server Extended Events XML.
    StorageMap(TraceAnalyzeArgs),
    /// Compare two SQL Server 1C databases by table shape and row counts.
    MssqlCompare(MssqlCompareArgs),
    /// Capture direct-SQL evidence needed to reverse the ConfigSave activation protocol.
    MssqlActivationSnapshot(MssqlActivationSnapshotArgs),
    /// Diff two activation snapshots without connecting to SQL Server.
    MssqlActivationDiff(MssqlActivationDiffArgs),
    /// Publish an already staged non-structural main-configuration change without native ibcmd.
    MssqlActivateStagedMain(MssqlActivateStagedMainArgs),
    /// Tell whether the ConfigSave of a database (with --tree: a source tree against the database) needs the platform's own apply, a restructuring; read-only.
    MssqlApplyCheck(crate::apply_check::cli::MssqlApplyCheckArgs),
    /// Tell whether the change from one XML tree to another needs the platform's own apply.
    ApplyCheckTrees(crate::apply_check::cli::ApplyCheckTreesArgs),
    /// Apply the staged main configuration (ConfigSave to Config) without the
    /// platform, as an exclusive native `config apply` does for a
    /// configuration that needs no restructuring: changed modules, forms,
    /// templates, pictures and help pages of any object, new forms and
    /// templates of existing objects, and the removal of forms and templates
    /// that a stage's `deleted` list names. Anything else is refused with the
    /// list of the rows that need the native apply. Takes the stage of this
    /// program's `infobase config import`; a `deleted` list it cannot account
    /// for name by name (a removal with a table or a column that its
    /// restructuring gate does not judge, one file of an object that stays)
    /// is refused whole.
    MssqlConfigApply(MssqlConfigApplyArgs),
    /// Compile, stage, and publish one existing module or managed form without native ibcmd.
    MssqlApplySourceChange(MssqlApplySourceChangeArgs),
    /// Restructure the tables of a catalog that got new attributes in the staged
    /// configuration, in one transaction (research prototype of the own config
    /// apply, issue #341; lab databases only).
    MssqlRestructure(crate::restructure::command::MssqlRestructureArgs),
    /// Dry-run source load parity and bootstrap base-blob readiness without writing ConfigSave.
    MssqlAuditSourceParity(MssqlAuditSourceParityArgs),
    /// Clone a SQL Server database with backup/restore.
    MssqlClone(MssqlCloneArgs),
    /// Export ConfigSave/Params storage tables to a native BCP bundle (lab;
    /// runs bcp.exe, which must be installed).
    MssqlStorageExport(MssqlStorageExportArgs),
    /// Import a native BCP storage bundle into an empty SQL Server infobase
    /// (lab; runs bcp.exe, which must be installed).
    MssqlStorageImport(MssqlStorageImportArgs),
    /// Export staged ConfigSave rows as a native BCP delta bundle (lab; runs
    /// bcp.exe, which must be installed).
    MssqlDeltaExport(MssqlDeltaExportArgs),
    /// Import staged ConfigSave rows into an existing SQL Server infobase
    /// (lab; runs bcp.exe, which must be installed).
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
    ///
    /// Forms are compiled by the native form writer first; set
    /// IBCMD_RS_NATIVE_FORM_WRITER=never for the older order, which keeps a
    /// target's unchanged form row when the form has item assets.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ConversionFormat {
    /// Hierarchical 1C XML source tree.
    Xml,
    /// EDT project directory containing DT-INF and src.
    Edt,
    /// Binary 1C CF configuration archive.
    Cf,
}

impl ConversionFormat {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Xml => "xml",
            Self::Edt => "edt",
            Self::Cf => "cf",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ConversionLossPolicy {
    /// Fail closed when a conversion would lose known information.
    Error,
    /// Retain a codec-declared loss as a warning when the codec permits it.
    Warn,
    /// Drop only an exact loss that its codec explicitly declares safe to drop.
    Drop,
}

#[derive(Debug, Args)]
pub struct ConvertArgs {
    /// Source artifact: an XML source directory, EDT project, or CF file.
    pub input: PathBuf,
    /// New destination artifact. Existing paths are never overwritten.
    pub output: PathBuf,
    /// Source artifact format, independent from its version profile.
    #[arg(long, value_enum)]
    pub source_format: ConversionFormat,
    /// Target artifact format, independent from its version profile.
    #[arg(long, value_enum)]
    pub target_format: ConversionFormat,
    /// Exact source profile ID, for example `xml-2.20` or `platform-8.3.27.1989`.
    #[arg(long)]
    pub source_profile: String,
    /// Exact target profile ID, for example `xml-2.21` or `platform-8.3.27.1989`.
    #[arg(long)]
    pub target_profile: String,
    /// Explicit policy for codec-declared losses.
    #[arg(long, alias = "loss-policy", value_enum, default_value_t = ConversionLossPolicy::Error)]
    pub loss: ConversionLossPolicy,
    /// Complete decode, validation, planning, migration, and encode preflight without publication.
    #[arg(long)]
    pub dry_run: bool,
    /// Also write the stable JSON report to this file.
    #[arg(long)]
    pub report: Option<PathBuf>,
    /// Optional directory with additive experimental JSON profiles.
    #[arg(long)]
    pub profile_dir: Option<PathBuf>,
    /// Explicit representation of present payloads in a source CF.
    #[arg(long, value_enum, default_value_t = CfCompression::RawDeflate)]
    pub source_compression: CfCompression,
    /// Physical revision for a newly bootstrapped target CF.
    #[arg(long, value_enum, default_value_t = CfRevision::Format16)]
    pub target_revision: CfRevision,
    /// Native storage-version header for a newly bootstrapped target CF.
    #[arg(long, default_value_t = 5)]
    pub target_storage_version: u32,
    /// Optional physical page-size override for a newly bootstrapped target CF.
    #[arg(long)]
    pub target_page_size: Option<u32>,
    /// Native reserved header word for a newly bootstrapped target CF.
    #[arg(long, default_value_t = 0)]
    pub target_reserved: u32,
}

#[derive(Debug, Args)]
pub struct CfArgs {
    #[command(subcommand)]
    pub command: CfCommands,
}

#[derive(Debug, Subcommand)]
pub enum CfCommands {
    /// Print a bounded JSON inventory of a CF archive.
    Inspect(CfInspectArgs),
    /// Verify CF structure, selected payloads, and optional SHA-256 expectations.
    Verify(CfVerifyArgs),
    /// Extract one exact CF storage element into a new evidence directory.
    Extract(CfExtractArgs),
    /// Export recognized CF storage records to hierarchical XML sources offline.
    Export(CfExportArgs),
    /// Apply selected source files to a base CF and publish a new CF offline.
    Overlay(CfOverlayArgs),
    /// Build a new CF from a complete XML source tree without a base or 1C platform.
    Bootstrap(CfBootstrapArgs),
    /// Overlay the module and form edits of an exported tree onto the file it came from.
    Load(CfLoadArgs),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CfCompression {
    /// Treat present payloads as complete raw RFC 1951 DEFLATE streams.
    RawDeflate,
    /// Treat present payloads as stored verbatim.
    Stored,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CfRevision {
    /// 32-bit V8 container layout.
    Format15,
    /// 64-bit V8 container layout with a semantic Format15 preamble.
    Format16,
}

#[derive(Debug, Args)]
pub struct CfInspectArgs {
    /// CF archive to inspect.
    pub input: PathBuf,
    /// Stable source-storage profile recorded in the JSON report.
    #[arg(long, default_value = "storage:cf-cli")]
    pub profile: String,
    /// Explicit payload representation; payload bytes are never guessed.
    #[arg(long, value_enum, default_value_t = CfCompression::RawDeflate)]
    pub compression: CfCompression,
    /// Inspect only this exact top-level element name; may be repeated.
    #[arg(long = "element")]
    pub elements: Vec<String>,
}

#[derive(Debug, Args)]
pub struct CfVerifyArgs {
    /// CF archive to verify.
    pub input: PathBuf,
    /// Stable source-storage profile recorded in the JSON report.
    #[arg(long, default_value = "storage:cf-cli")]
    pub profile: String,
    /// Explicit payload representation; payload bytes are never guessed.
    #[arg(long, value_enum, default_value_t = CfCompression::RawDeflate)]
    pub compression: CfCompression,
    /// Verify only this exact top-level element name; may be repeated.
    #[arg(long = "element")]
    pub elements: Vec<String>,
    /// List and structurally index selected entries without reading payload bytes.
    #[arg(long)]
    pub list_only: bool,
    /// Require the selected element's unpacked SHA-256 (`NAME=64-lowercase-hex`).
    #[arg(long = "expect-sha256", value_name = "NAME=SHA256")]
    pub expected_sha256: Vec<String>,
}

#[derive(Debug, Args)]
pub struct CfExtractArgs {
    /// CF archive to read.
    pub input: PathBuf,
    /// Exact top-level storage element name.
    pub element: String,
    /// New directory that will receive packed.bin and unpacked.bin.
    pub output_dir: PathBuf,
    /// Stable source-storage profile recorded in the JSON report.
    #[arg(long, default_value = "storage:cf-cli")]
    pub profile: String,
    /// Explicit payload representation; payload bytes are never guessed.
    #[arg(long, value_enum, default_value_t = CfCompression::RawDeflate)]
    pub compression: CfCompression,
}

#[derive(Debug, Args)]
pub struct CfExportArgs {
    /// CF archive to export.
    pub input: PathBuf,
    /// Output directory for hierarchical XML sources.
    pub output_dir: PathBuf,
    /// Stable source-storage profile recorded in the JSON report.
    #[arg(long, default_value = "storage:cf-cli")]
    pub profile: String,
    /// Explicit payload representation; payload bytes are never guessed.
    #[arg(long, value_enum, default_value_t = CfCompression::RawDeflate)]
    pub compression: CfCompression,
    /// Platform the XML is for: a release (8.3.27, 8.5.1) or an exact build
    /// (8.3.27.2214, 8.5.1.1150); 8.3.x reads and writes XML 2.20, 8.5.x 2.21.
    #[arg(
        long,
        value_name = "VERSION",
        value_parser = crate::platform::parse_flag,
        conflicts_with = "source_version"
    )]
    pub platform: Option<crate::platform::PlatformSpec>,
    /// Target source XML dialect.
    #[arg(long, value_enum, default_value_t = InfobaseConfigSourceVersion::V2_20, hide = true)]
    pub source_version: InfobaseConfigSourceVersion,
    /// Replace files under the output directory.
    #[arg(long, alias = "force")]
    pub overwrite: bool,
    /// Also write `.ibcmd/index.tsv`: the input's digest and every file's, so
    /// that `cf load` of this tree onto the same file needs no re-export.
    #[arg(long)]
    pub index: bool,
    /// Bring an indexed tree exported from an earlier version of this file up
    /// to it, rewriting only the entries that changed (a full export when
    /// objects were added, removed or their metadata changed). Refuses a tree
    /// without an index or with edits not loaded yet; files in dot
    /// directories (`.git`) are never touched. Implies --index.
    #[arg(long)]
    pub update: bool,
}

#[derive(Debug, Args)]
pub struct CfOverlayArgs {
    /// Base CF whose physical layout and untouched entries must be retained.
    pub base: PathBuf,
    /// New CF destination. Existing files are never overwritten.
    pub output: PathBuf,
    /// Exact storage profile used by the legacy source compiler.
    #[arg(long, default_value = "storage:mssql-config-configsave")]
    pub profile: String,
    /// Explicit representation of every present top-level CF payload.
    #[arg(long, value_enum, default_value_t = CfCompression::RawDeflate)]
    pub compression: CfCompression,
    /// Platform the XML is for: a release (8.3.27, 8.5.1) or an exact build
    /// (8.3.27.2214, 8.5.1.1150); 8.3.x reads and writes XML 2.20, 8.5.x 2.21.
    #[arg(
        long,
        value_name = "VERSION",
        value_parser = crate::platform::parse_flag,
        conflicts_with = "source_version"
    )]
    pub platform: Option<crate::platform::PlatformSpec>,
    /// Source XML dialect; it is independent from the CF container revision.
    #[arg(long, value_enum, default_value_t = InfobaseConfigSourceVersion::V2_20, hide = true)]
    pub source_version: InfobaseConfigSourceVersion,
    /// Replace a native module row from BSL text (`STORAGE_KEY=FILE`); repeatable.
    #[arg(long = "module", value_name = "KEY=FILE")]
    pub modules: Vec<String>,
    /// Replace a raw-deflated asset from exact source bytes (`STORAGE_KEY=FILE`); repeatable.
    ///
    /// The plain source bytes are deflated exactly once. For a body that a
    /// compiler already produced as a raw-deflate stream, use
    /// `--compiled-asset` instead: deflating such bytes again would store a
    /// double-compressed payload the platform cannot export.
    #[arg(long = "raw-asset", value_name = "KEY=FILE")]
    pub raw_assets: Vec<String>,
    /// Write an already-compiled body verbatim as the final physical payload (`STORAGE_KEY=FILE`); repeatable.
    ///
    /// Unlike `--raw-asset`, the file bytes are never re-compressed. The file
    /// must already be one complete raw RFC 1951 DEFLATE stream (for example a
    /// DCS Template body produced by the offline compiler); this is validated
    /// fail-closed, and plain source bytes are rejected with a pointer back to
    /// `--raw-asset`.
    #[arg(long = "compiled-asset", value_name = "KEY=FILE")]
    pub compiled_assets: Vec<String>,
    /// Patch a metadata row using its base row and source XML (`STORAGE_KEY=FILE`); repeatable.
    #[arg(long = "metadata-xml", value_name = "KEY=FILE")]
    pub metadata_xml: Vec<String>,
    /// Patch CommonModule metadata using its base row (`STORAGE_KEY=FILE`); repeatable.
    #[arg(long = "common-module-xml", value_name = "KEY=FILE")]
    pub common_module_xml: Vec<String>,
    /// Patch CommandInterface using its base row when necessary (`STORAGE_KEY=FILE`); repeatable.
    #[arg(long = "command-interface", value_name = "KEY=FILE")]
    pub command_interfaces: Vec<String>,
    /// Compile managed Form.xml against its existing native body row (`STORAGE_KEY=FILE`); repeatable.
    #[arg(long = "form-xml", value_name = "KEY=FILE")]
    pub form_xml: Vec<String>,
}

#[derive(Debug, Args)]
pub struct CfLoadArgs {
    /// Edited hierarchical XML source tree (a `cf export` of the base, changed).
    pub source_dir: PathBuf,
    /// New file destination. Existing files are never overwritten.
    pub output: PathBuf,
    /// The .cf/.cfe/.epf/.erf the tree was exported from.
    #[arg(long)]
    pub base: PathBuf,
    /// Platform the tree's XML is for: a release (8.3.27, 8.5.1) or an exact
    /// build; 8.3.x reads XML 2.20, 8.5.x 2.21.
    #[arg(
        long,
        value_name = "VERSION",
        value_parser = crate::platform::parse_flag,
        conflicts_with = "source_version"
    )]
    pub platform: Option<crate::platform::PlatformSpec>,
    /// Source XML dialect of the tree (`--platform` names it through the
    /// platform).
    #[arg(long, value_enum, default_value_t = InfobaseConfigSourceVersion::V2_20, hide = true)]
    pub source_version: InfobaseConfigSourceVersion,
}

#[derive(Debug, Args)]
pub struct CfBootstrapArgs {
    /// Complete hierarchical XML source directory.
    pub source_dir: PathBuf,
    /// New CF destination. Existing files are never overwritten.
    pub output: PathBuf,
    /// Platform the XML is for: a release (8.3.27, 8.5.1) or an exact build
    /// (8.3.27.2214, 8.5.1.1150); 8.3.x reads and writes XML 2.20, 8.5.x 2.21.
    #[arg(
        long,
        value_name = "VERSION",
        value_parser = crate::platform::parse_flag,
        conflicts_with = "source_version"
    )]
    pub platform: Option<crate::platform::PlatformSpec>,
    /// The source XML dialect itself (`--platform` names it through the
    /// platform); independent from the container revision.
    #[arg(long, value_enum, default_value_t = InfobaseConfigSourceVersion::V2_20, hide = true)]
    pub source_version: InfobaseConfigSourceVersion,
    /// Exact target platform profile selecting native family layouts.
    #[arg(long, default_value = "platform-8.3.27.1989")]
    pub target_profile: String,
    /// Optional directory with additive experimental JSON profiles.
    #[arg(long)]
    pub profile_dir: Option<PathBuf>,
    /// Independent physical CF container revision.
    #[arg(long, value_enum, default_value_t = CfRevision::Format16)]
    pub revision: CfRevision,
    /// Native CF storage-version header word.
    #[arg(long, default_value_t = 5)]
    pub storage_version: u32,
    /// Optional physical page-size override for the selected revision.
    #[arg(long)]
    pub page_size: Option<u32>,
    /// Native CF reserved header word.
    #[arg(long, default_value_t = 0)]
    pub reserved: u32,
}

/// Raw arguments of one of the platform ibcmd's modes (`infobase`, `server`,
/// ...): `crate::dropin` parses them the way the platform's ibcmd does.
#[derive(Debug, Args)]
pub struct NativeModeArgs {
    /// Commands, options and arguments in the platform ibcmd's syntax.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, num_args = 0..)]
    pub args: Vec<OsString>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum InfobaseConfigFormat {
    /// Hierarchical 1C XML source tree compatible with ibcmd config export/import.
    #[value(name = "xml", alias = "ibcmd-xml", alias = "source-tree")]
    Xml,
}

/// How `infobase config import` stages the tree into ConfigSave.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InfobaseImportStageMode {
    /// Compile every row from the tree when the target holds none of the
    /// tree's configuration (an empty infobase), else patch the target's
    /// own rows.
    #[default]
    Auto,
    /// Patch the target's own rows: it holds the configuration.
    Patch,
    /// Compile every row from the tree (`--base-free`).
    BaseFree,
}

/// Whether `infobase config import` checks the state its stage would leave
/// against the tree before it writes anything (`--verify`, `--no-verify`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InfobaseImportVerify {
    /// Check every stage: a patch stage can leave a change of the tree out,
    /// and a stage compiled from the tree can carry a slip of the compiler.
    #[default]
    Auto,
    /// Check every stage (`--verify`, the same as the default).
    On,
    /// Check nothing (`--no-verify`).
    Off,
}

/// `infobase config export`: what the drop-in command line (`crate::dropin`)
/// or the research round trip asks for.
#[derive(Debug, Clone)]
pub struct InfobaseConfigExportArgs {
    /// Optional JSON settings file. Supports autumn-properties.json/vRunner DB keys and ibcmd-rs format keys.
    pub settings: Option<PathBuf>,
    /// The platform ibcmd's configuration file (`--config`/`-c`), kept for
    /// the settings layer.
    pub native_config: Option<PathBuf>,
    /// Source format. Can also be set in settings as format/config-format.
    pub format: Option<InfobaseConfigFormat>,
    /// `--platform`: the platform whose XML format is written. Without it
    /// (and without `source_version`) the settings name it
    /// (`crate::settings`), then the configuration's compatibility mode.
    pub platform: Option<crate::platform::PlatformSpec>,
    /// Source XML version. 2.20 matches 1C 8.3.27, 2.21 matches 1C 8.5.1. Can also be set in settings.
    pub source_version: Option<InfobaseConfigSourceVersion>,
    /// DBMS type. Only MSSQLServer is supported by the direct exporter.
    pub dbms: Option<String>,
    /// Database server.
    pub db_server: Option<String>,
    /// Database name.
    pub db_name: Option<String>,
    /// Database user for SQL authentication.
    pub db_user: Option<String>,
    /// Database password.
    pub db_pwd: Option<String>,
    /// Environment variable containing the database password.
    pub db_pwd_env: String,
    /// Infobase user name. Accepted for ibcmd CLI compatibility; direct SQL export does not use it.
    pub user: Option<String>,
    /// Infobase user password.
    pub password: Option<String>,
    /// Environment variable containing the infobase user password.
    pub password_env: String,
    /// sqlcmd.exe (and bcp.exe beside it) to run instead of the built-in SQL
    /// Server client, as ibcmd-rs 0.2 did (`--sqlcmd`).
    pub sqlcmd: Option<PathBuf>,
    /// Clear a non-empty output directory first (the research round trip).
    /// The drop-in export refuses one, as the platform's own export does.
    /// Export this configuration extension instead of the configuration
    /// (`--extension` of the platform's `config export`).
    pub extension: Option<String>,
    pub overwrite: bool,
    /// Count the exported files for the report (walks the whole tree).
    pub count_files: bool,
    /// Output directory for hierarchical XML sources.
    pub output_dir: PathBuf,
}

/// `infobase config import`: what the drop-in command line (`crate::dropin`)
/// or the research round trip asks for.
#[derive(Debug, Clone)]
pub struct InfobaseConfigImportArgs {
    /// Optional JSON settings file. Supports autumn-properties.json/vRunner DB keys and ibcmd-rs format keys.
    pub settings: Option<PathBuf>,
    /// The platform ibcmd's configuration file (`--config`/`-c`), kept for
    /// the settings layer.
    pub native_config: Option<PathBuf>,
    /// Source format. Can also be set in settings as format/config-format.
    pub format: Option<InfobaseConfigFormat>,
    /// `--platform`: the platform whose XML format the tree must be in.
    /// Without it (and without `source_version`) the settings name it
    /// (`crate::settings`), then the tree's own `Configuration.xml`.
    pub platform: Option<crate::platform::PlatformSpec>,
    /// Source XML version; when neither this nor the settings give one, the
    /// tree's own (`Configuration.xml`) is read.
    pub source_version: Option<InfobaseConfigSourceVersion>,
    /// DBMS type. Only MSSQLServer is supported by the direct importer.
    pub dbms: Option<String>,
    /// Database server.
    pub db_server: Option<String>,
    /// Database name.
    pub db_name: Option<String>,
    /// Database user for SQL authentication.
    pub db_user: Option<String>,
    /// Database password.
    pub db_pwd: Option<String>,
    /// Environment variable containing the database password.
    pub db_pwd_env: String,
    /// Infobase user name. Accepted for ibcmd CLI compatibility; direct SQL import does not use it.
    pub user: Option<String>,
    /// Infobase user password.
    pub password: Option<String>,
    /// Environment variable containing the infobase user password.
    pub password_env: String,
    /// sqlcmd.exe (and bcp.exe beside it) to run instead of the built-in SQL
    /// Server client, as ibcmd-rs 0.2 did (`--sqlcmd`).
    pub sqlcmd: Option<PathBuf>,
    /// Replace existing ConfigSave rows before staging import rows (an
    /// import always does, as the platform's own does).
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    pub allow_non_lab: bool,
    /// Optional maximum number of staged XML objects per SQL batch.
    pub batch_size: Option<usize>,
    /// Optional source path prefix to import. Can be repeated.
    pub path_prefix: Vec<String>,
    /// Optional path for the generated SQL scripts (and the bulk rows file).
    pub script_output: Option<PathBuf>,
    /// Patch the target's rows, compile every row, or decide by the target.
    pub stage_mode: InfobaseImportStageMode,
    /// Check the state the stage would leave against the tree first.
    pub verify: InfobaseImportVerify,
    /// Root directory with hierarchical XML sources.
    pub source_dir: PathBuf,
}

/// Research commands under `infobase config` that run the installed
/// platform: `ibcmd-rs infobase config roundtrip|sweep ...`.
#[cfg(feature = "platform-oracle")]
#[derive(Debug, Parser)]
#[command(name = "ibcmd-rs infobase config")]
pub struct InfobaseOracleCli {
    #[command(subcommand)]
    pub command: InfobaseOracleCommands,
}

#[cfg(feature = "platform-oracle")]
#[derive(Debug, Subcommand)]
pub enum InfobaseOracleCommands {
    /// Clone, direct-export, direct-import, apply, direct-export and diff one MSSQL infobase roundtrip.
    Roundtrip(InfobaseConfigRoundtripArgs),
    /// Run a representative family-by-family scoped MSSQL roundtrip sweep and emit a compact JSON report.
    Sweep(InfobaseConfigSweepArgs),
}

#[cfg(feature = "platform-oracle")]
#[derive(Debug, Args)]
pub struct InfobaseConfigRoundtripArgs {
    /// Optional JSON settings file. Supports autumn-properties.json/vRunner DB keys and ibcmd-rs format keys.
    #[arg(long)]
    pub settings: Option<PathBuf>,
    /// Source format. Can also be set in settings as format/config-format.
    #[arg(long)]
    pub format: Option<InfobaseConfigFormat>,
    /// Source XML version. 2.20 matches 1C 8.3.27, 2.21 matches 1C 8.5.1. Can also be set in settings.
    #[arg(long, value_enum)]
    pub source_version: Option<InfobaseConfigSourceVersion>,
    /// DBMS type. Currently MSSQLServer is supported by the direct roundtrip.
    #[arg(long)]
    pub dbms: Option<String>,
    /// Reference database server.
    #[arg(long)]
    pub db_server: Option<String>,
    /// Reference database name.
    #[arg(long)]
    pub db_name: Option<String>,
    /// Database user for SQL authentication.
    #[arg(long)]
    pub db_user: Option<String>,
    /// Database password. Prefer --db-pwd-env for shell history.
    #[arg(long)]
    pub db_pwd: Option<String>,
    /// Environment variable containing the database password.
    #[arg(long, default_value = "IBCMD_DB_PSW")]
    pub db_pwd_env: String,
    /// Infobase user passed to ibcmd check/apply when needed.
    #[arg(long, short = 'u')]
    pub user: Option<String>,
    /// Infobase password passed to ibcmd check/apply. Prefer --password-env for shell history.
    #[arg(long, short = 'P')]
    pub password: Option<String>,
    /// Environment variable containing the infobase user password.
    #[arg(long, default_value = "IBCMD_USER_PSW")]
    pub password_env: String,
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
    /// ibcmd executable path. Auto-detects a recent 8.3 build when omitted.
    #[arg(long)]
    pub ibcmd: Option<PathBuf>,
    /// Optional target clone database name. Defaults to <db-name>_roundtrip_<timestamp>.
    #[arg(long)]
    pub target_db: Option<String>,
    /// Optional backup path used by clone.
    #[arg(long)]
    pub backup: Option<PathBuf>,
    /// Root directory for roundtrip artifacts.
    #[arg(long, default_value = "E:\\ibcmd_lab\\roundtrip")]
    pub work_dir: PathBuf,
    /// Reuse an existing source tree instead of exporting a fresh baseline from the reference database.
    #[arg(long)]
    pub source_dir: Option<PathBuf>,
    /// Reuse a dedicated ibcmd --data directory instead of a generated one.
    #[arg(long)]
    pub data_dir: Option<PathBuf>,
    /// Replace existing target database and artifact directory when needed.
    #[arg(long, alias = "force")]
    pub overwrite: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Kill ibcmd check/apply after this many seconds.
    #[arg(long, default_value_t = 600)]
    pub timeout_sec: u64,
    /// Optional maximum number of staged XML objects per SQL batch.
    #[arg(long)]
    pub batch_size: Option<usize>,
    /// Optional source path prefix to import and compare. Can be repeated.
    #[arg(long)]
    pub path_prefix: Vec<String>,
    /// Optional base path for generated staging SQL scripts.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[cfg(feature = "platform-oracle")]
#[derive(Debug, Args, Clone)]
pub struct InfobaseConfigSweepArgs {
    /// Optional JSON settings file. Supports autumn-properties.json/vRunner DB keys and ibcmd-rs format keys.
    #[arg(long)]
    pub settings: Option<PathBuf>,
    /// Source format. Can also be set in settings as format/config-format.
    #[arg(long)]
    pub format: Option<InfobaseConfigFormat>,
    /// Source XML version. 2.20 matches 1C 8.3.27, 2.21 matches 1C 8.5.1. Can also be set in settings.
    #[arg(long, value_enum)]
    pub source_version: Option<InfobaseConfigSourceVersion>,
    /// DBMS type. Currently MSSQLServer is supported by the direct sweep.
    #[arg(long)]
    pub dbms: Option<String>,
    /// Reference database server.
    #[arg(long)]
    pub db_server: Option<String>,
    /// Reference database name.
    #[arg(long)]
    pub db_name: Option<String>,
    /// Database user for SQL authentication.
    #[arg(long)]
    pub db_user: Option<String>,
    /// Database password. Prefer --db-pwd-env for shell history.
    #[arg(long)]
    pub db_pwd: Option<String>,
    /// Environment variable containing the database password.
    #[arg(long, default_value = "IBCMD_DB_PSW")]
    pub db_pwd_env: String,
    /// Infobase user passed to ibcmd check/apply when needed.
    #[arg(long, short = 'u')]
    pub user: Option<String>,
    /// Infobase password passed to ibcmd check/apply. Prefer --password-env for shell history.
    #[arg(long, short = 'P')]
    pub password: Option<String>,
    /// Environment variable containing the infobase user password.
    #[arg(long, default_value = "IBCMD_USER_PSW")]
    pub password_env: String,
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
    /// ibcmd executable path. Auto-detects a recent 8.3 build when omitted.
    #[arg(long)]
    pub ibcmd: Option<PathBuf>,
    /// Root directory for sweep artifacts.
    #[arg(long, default_value = "E:\\ibcmd_lab\\roundtrip")]
    pub work_dir: PathBuf,
    /// Reuse an existing source tree instead of exporting a fresh baseline from the reference database.
    #[arg(long)]
    pub source_dir: Option<PathBuf>,
    /// Replace existing target databases and artifact directories when needed.
    #[arg(long, alias = "force")]
    pub overwrite: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Kill ibcmd check/apply after this many seconds.
    #[arg(long, default_value_t = 600)]
    pub timeout_sec: u64,
    /// Optional maximum number of staged XML objects per SQL batch.
    #[arg(long)]
    pub batch_size: Option<usize>,
    /// Explicit source path prefixes to sweep. Can be repeated. When omitted, one representative prefix per family is selected automatically.
    #[arg(long)]
    pub path_prefix: Vec<String>,
    /// Restrict automatic selection to these top-level source families, for example Catalogs or Documents.
    #[arg(long)]
    pub family: Vec<String>,
    /// Skip this many highest-ranked automatic candidates within each family before selecting prefixes.
    #[arg(long, default_value_t = 0)]
    pub candidate_offset: usize,
    /// Select this many ranked automatic candidates per family when --path-prefix is omitted.
    #[arg(long, default_value_t = 1)]
    pub candidates_per_family: usize,
    /// Limit the number of automatically selected prefixes.
    #[arg(long)]
    pub max_prefixes: Option<usize>,
    /// Stop after the first diff or error result instead of sweeping the remaining prefixes.
    #[arg(long)]
    pub stop_on_first_non_ok: bool,
    /// Drop each temporary target database after its sweep entry completes to avoid disk growth during long discovery runs.
    #[arg(long)]
    pub drop_target_db_after_run: bool,
    /// Optional base path for generated staging SQL scripts.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
}

#[cfg(feature = "platform-oracle")]
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
pub struct FormDiffCandidatesArgs {
    /// Baseline Form.xml.
    #[arg(long)]
    pub base_xml: PathBuf,
    /// Variant Form.xml with one or a few controlled changes.
    #[arg(long)]
    pub variant_xml: PathBuf,
    /// Baseline raw deflated Form body blob from Config/ConfigSave.
    #[arg(long)]
    pub base_blob: PathBuf,
    /// Variant raw deflated Form body blob from Config/ConfigSave.
    #[arg(long)]
    pub variant_blob: PathBuf,
    /// Optional JSON output file. Prints to stdout when omitted.
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct FormProvenanceCorpusArgs {
    /// Parity run root containing candidate_dump, native, and candidate directories.
    #[arg(long)]
    pub run_root: PathBuf,
    /// Deterministic JSONL output path (outside the repository is recommended).
    #[arg(long)]
    pub output: PathBuf,
    /// Form property to include. Repeatable; defaults to AutoMaxWidth, AutoAddIncomplete, DataPath, TitleDataPath.
    #[arg(long = "property")]
    pub property: Vec<String>,
    /// Source commit recorded in output metadata.
    #[arg(long)]
    pub source_commit: Option<String>,
}

#[derive(Debug, Args)]
pub struct MxlLineProvenanceCorpusArgs {
    /// Parity run root containing candidate_dump.
    #[arg(long)]
    pub run_root: PathBuf,
    /// Deterministic JSONL output path (outside the repository is recommended).
    #[arg(long)]
    pub output: PathBuf,
    /// Raw asset path relative to candidate_dump. Repeat to avoid scanning a whole corpus.
    #[arg(long = "asset")]
    pub asset: Vec<PathBuf>,
}

#[derive(Debug, Args)]
pub struct FormContextSummaryArgs {
    #[arg(long)]
    pub run_root: PathBuf,
    #[arg(long)]
    pub source_commit: Option<String>,
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
pub struct AuditNativeFormWriterArgs {
    /// Root folder with 1C XML sources.
    pub root: PathBuf,
    /// Folder with the inflated form bodies of the same database.
    pub bodies: PathBuf,
    /// Optional JSON output file. Prints to stdout when omitted.
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct AuditInterfaceWriterArgs {
    /// Root folder with the native 1C XML sources.
    pub root: PathBuf,
    /// Folder with the inflated Config rows of the same database
    /// (`<file name>__part0.txt`).
    pub inflated: PathBuf,
    /// Platform the XML is for: a release (8.3.27, 8.5.1) or an exact build
    /// (8.3.27.2214, 8.5.1.1150); 8.3.x reads and writes XML 2.20, 8.5.x 2.21.
    #[arg(
        long,
        value_name = "VERSION",
        value_parser = crate::platform::parse_flag,
        conflicts_with = "source_version"
    )]
    pub platform: Option<crate::platform::PlatformSpec>,
    /// XML dialect the native tree was exported in.
    #[arg(long, value_enum, default_value_t = InfobaseConfigSourceVersion::V2_20, hide = true)]
    pub source_version: InfobaseConfigSourceVersion,
    /// Optional JSON output file with every difference.
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct AuditDcsTemplateWriterArgs {
    /// Root folder with 1C XML sources exported from the database.
    pub root: PathBuf,
    /// Folder with the inflated Config rows of the same database.
    pub bodies: PathBuf,
    /// Dump directory (manifest.json + Config_inflated) the export index is built from.
    pub dump: PathBuf,
    /// Optional JSON output file. Prints to stdout when omitted.
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct AuditMxlWriterArgs {
    /// Root folder with the native 1C XML sources.
    pub root: PathBuf,
    /// Folder with the inflated stored rows of the same database.
    pub bodies: PathBuf,
    /// Optional JSON output file. Prints to stdout when omitted.
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct AuditMetadataCompilerArgs {
    /// Root folder with the native 1C XML sources.
    pub root: PathBuf,
    /// Folder with the stored Config rows of the same database
    /// (`<FileName>__part0.bin`, raw deflate, or `<FileName>__part0.txt`).
    pub rows: PathBuf,
    /// Platform the XML is for: a release (8.3.27, 8.5.1) or an exact build
    /// (8.3.27.2214, 8.5.1.1150); 8.3.x reads and writes XML 2.20, 8.5.x 2.21.
    #[arg(
        long,
        value_name = "VERSION",
        value_parser = crate::platform::parse_flag,
        conflicts_with = "source_version"
    )]
    pub platform: Option<crate::platform::PlatformSpec>,
    /// XML dialect of the tree: 2.20 (8.3.27) or 2.21 (8.5).
    #[arg(long, default_value = "2.20", hide = true)]
    pub source_version: String,
    /// Only this kind, e.g. `Catalog` (repeatable).
    #[arg(long = "kind")]
    pub kinds: Vec<String>,
    /// Write the stored and the compiled text of differing samples here.
    #[arg(long)]
    pub diff_dir: Option<PathBuf>,
    /// Differing samples kept per kind.
    #[arg(long, default_value_t = 5)]
    pub max_samples: usize,
    /// Optional JSON report.
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct AuditMetadataExportArgs {
    /// Root folder with the native 1C XML sources (the oracle).
    pub root: PathBuf,
    /// Folder with the stored Config rows of the same database
    /// (`<FileName>__part0.bin`, raw deflate, or `<FileName>__part0.txt`).
    pub rows: PathBuf,
    /// Platform the XML is for: a release (8.3.27, 8.5.1) or an exact build
    /// (8.3.27.2214, 8.5.1.1150); 8.3.x reads and writes XML 2.20, 8.5.x 2.21.
    #[arg(
        long,
        value_name = "VERSION",
        value_parser = crate::platform::parse_flag,
        conflicts_with = "source_version"
    )]
    pub platform: Option<crate::platform::PlatformSpec>,
    /// XML dialect of the tree: 2.20 (8.3.27) or 2.21 (8.5); read from
    /// Configuration.xml when omitted.
    #[arg(long, hide = true)]
    pub source_version: Option<String>,
    /// Only this kind, e.g. `Catalog` (repeatable).
    #[arg(long = "kind")]
    pub kinds: Vec<String>,
    /// Write the expected and the written XML of differing samples here.
    #[arg(long)]
    pub diff_dir: Option<PathBuf>,
    /// Differing samples kept per kind.
    #[arg(long, default_value_t = 5)]
    pub max_samples: usize,
    /// Also time decode + write over preloaded rows with this many threads.
    #[arg(long)]
    pub timing_threads: Option<usize>,
    /// Optional JSON report.
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct AuditNameIndexArgs {
    /// Root folder with the native 1C XML sources (the oracle).
    pub root: PathBuf,
    /// Folder with the stored Config rows (`<FileName>__part<N>.bin`).
    pub rows: PathBuf,
    /// Platform the XML is for: a release (8.3.27, 8.5.1) or an exact build
    /// (8.3.27.2214, 8.5.1.1150); 8.3.x reads and writes XML 2.20, 8.5.x 2.21.
    #[arg(
        long,
        value_name = "VERSION",
        value_parser = crate::platform::parse_flag,
        conflicts_with = "source_version"
    )]
    pub platform: Option<crate::platform::PlatformSpec>,
    /// XML dialect: 2.20 (8.3.27) or 2.21 (8.5).
    #[arg(long, value_enum, default_value_t = InfobaseConfigSourceVersion::V2_20, hide = true)]
    pub source_version: InfobaseConfigSourceVersion,
    /// Differing entries kept per kind.
    #[arg(long, default_value_t = 5)]
    pub max_samples: usize,
    /// Optional JSON report.
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct AuditEmptyStageArgs {
    /// Root folder with the native 1C XML sources.
    pub root: PathBuf,
    /// Folder with the stored Config rows of the same database
    /// (`<FileName>__part0.bin`, raw deflate, or `<FileName>__part0.txt`).
    pub rows: PathBuf,
    /// Platform the XML is for: a release (8.3.27, 8.5.1) or an exact build
    /// (8.3.27.2214, 8.5.1.1150); 8.3.x reads and writes XML 2.20, 8.5.x 2.21.
    #[arg(
        long,
        value_name = "VERSION",
        value_parser = crate::platform::parse_flag,
        conflicts_with = "source_version"
    )]
    pub platform: Option<crate::platform::PlatformSpec>,
    /// XML dialect of the tree: 2.20 (8.3.27) or 2.21 (8.5); read from
    /// Configuration.xml when omitted.
    #[arg(long, hide = true)]
    pub source_version: Option<String>,
    /// Write the stored and the produced text of differing samples here.
    #[arg(long)]
    pub diff_dir: Option<PathBuf>,
    /// Samples kept per (outcome, pattern, kind).
    #[arg(long, default_value_t = 5)]
    pub max_samples: usize,
    /// Write a TSV of every produced row (name, family, bytes, sha256).
    #[arg(long)]
    pub manifest: Option<PathBuf>,
    /// Write every produced row as `<FileName>__part0.bin` (raw deflate, the
    /// layout of a rows cache) into this directory.
    #[arg(long)]
    pub rows_out: Option<PathBuf>,
    /// Optional JSON report.
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct AuditFormBodyBlockersArgs {
    /// Root folder with 1C XML sources.
    pub root: PathBuf,
    /// Optional JSON output file. Prints to stdout when omitted.
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct AuditRoleRightsWriterArgs {
    /// Root folder with 1C XML sources.
    pub root: PathBuf,
    /// Folder with the inflated Config rows of the same database
    /// (`<role uuid>.0__part0.txt`).
    pub inflated: PathBuf,
    /// Optional JSON output file; a one-line summary is printed instead.
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
pub struct SourceDiffArgs {
    /// Left/reference source tree.
    pub left: PathBuf,
    /// Right/candidate source tree.
    pub right: PathBuf,
    /// Optional source path prefix to compare. Can be repeated.
    #[arg(long)]
    pub path_prefix: Vec<String>,
    /// Optional JSON output file. Prints to stdout when omitted.
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct SourceDiffExplainArgs {
    /// Source-diff JSON produced by `source-diff`. Supplies left/right roots automatically.
    #[arg(long)]
    pub diff: Option<PathBuf>,
    /// Left/reference source tree. Required when --diff is omitted.
    #[arg(long)]
    pub left_root: Option<PathBuf>,
    /// Right/candidate source tree. Required when --diff is omitted.
    #[arg(long)]
    pub right_root: Option<PathBuf>,
    /// Relative source path to explain, such as DataProcessors/Name/Forms/Form/Ext/Form.xml.
    pub path: String,
    /// Optional indexed leaf-path prefix filter. Can be repeated.
    #[arg(long)]
    pub leaf_path_prefix: Vec<String>,
    /// Maximum diff rows to emit. Use 0 for no limit.
    #[arg(long, default_value_t = 200)]
    pub limit: usize,
    /// Optional JSON output file. Prints to stdout when omitted.
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct SourceDiffSignaturesArgs {
    /// Source-diff JSON produced by `source-diff`.
    pub diff: PathBuf,
    /// Optional maximum changed XML file pairs to parse per source kind.
    #[arg(long)]
    pub max_files_per_kind: Option<usize>,
    /// Per-kind sample limit override, such as `form=900` or `template=450`. Can be repeated.
    #[arg(long)]
    pub kind_limit: Vec<String>,
    /// Maximum number of signature rows to include.
    #[arg(long, default_value_t = 200)]
    pub top: usize,
    /// Maximum example file paths to keep per signature.
    #[arg(long, default_value_t = 3)]
    pub examples_per_signature: usize,
    /// Optional JSON output file. Prints to stdout when omitted.
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct SourceDiffMatrixArgs {
    /// Source-diff JSON produced by `source-diff`.
    pub diff: PathBuf,
    /// Stable database label, for example `ut_ibcmd` or `bsp`.
    #[arg(long)]
    pub database: String,
    /// Immutable run label unique within the database.
    #[arg(long)]
    pub run_id: String,
    /// Git commit that produced the candidate export.
    #[arg(long)]
    pub git_sha: String,
    /// Mark this as a full export; only full runs may report readiness percent.
    #[arg(long, conflicts_with = "scoped")]
    pub full: bool,
    /// Mark this as a scoped export; its readiness percent is intentionally withheld.
    #[arg(long)]
    pub scoped: bool,
    /// JSON destination for the matrix.
    #[arg(short, long)]
    pub output: PathBuf,
    /// Optional Markdown destination for a deterministic human report.
    #[arg(long)]
    pub markdown: Option<PathBuf>,
    /// Replace a pre-existing output only when explicitly requested.
    #[arg(long)]
    pub overwrite: bool,
}

#[derive(Debug, Args)]
pub struct SourceDiffMatrixMergeArgs {
    /// Input parity matrix JSON files.
    #[arg(required = true)]
    pub matrices: Vec<PathBuf>,
    /// JSON destination for the merged matrix.
    #[arg(short, long)]
    pub output: PathBuf,
    /// Optional Markdown destination for a deterministic human report.
    #[arg(long)]
    pub markdown: Option<PathBuf>,
    /// Replace a pre-existing output only when explicitly requested.
    #[arg(long)]
    pub overwrite: bool,
}

#[derive(Debug, Args)]
pub struct SourceThreeWayOracleArgs {
    /// Pre-existing source tree exported by native ibcmd. This command only reads it.
    #[arg(long)]
    pub native: PathBuf,
    /// Pre-existing source tree produced by an EDT import/export cycle. This command only reads it.
    #[arg(long)]
    pub edt: PathBuf,
    /// Pre-existing source tree produced by ibcmd-rs. This command only reads it.
    #[arg(long)]
    pub ours: PathBuf,
    /// Exact source/configuration version shared by the three input trees.
    #[arg(long)]
    pub source_version: String,
    /// Exact native ibcmd version used to produce --native.
    #[arg(long)]
    pub native_tool_version: String,
    /// Exact EDT version and import/export route used to produce --edt.
    #[arg(long)]
    pub edt_tool_version: String,
    /// Exact ibcmd-rs version or commit used to produce --ours.
    #[arg(long)]
    pub ours_tool_version: String,
    /// Stop before hashing when a tree exceeds this many files.
    #[arg(long, default_value_t = 100_000)]
    pub max_files: usize,
    /// Stop before hashing when a tree exceeds this total byte count.
    #[arg(long, default_value_t = 4 * 1024 * 1024 * 1024_u64)]
    pub max_total_bytes: u64,
    /// Stop before hashing a file larger than this many bytes.
    #[arg(long, default_value_t = 512 * 1024 * 1024_u64)]
    pub max_file_bytes: u64,
    /// New JSON report destination. Must share an existing parent with --markdown.
    #[arg(short, long)]
    pub output: PathBuf,
    /// New deterministic Markdown destination. Must share --output's existing parent.
    #[arg(long)]
    pub markdown: PathBuf,
}

#[derive(Debug, Args)]
pub struct CompatibilityArgs {
    /// Optional JSON output file. Prints to stdout when omitted.
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct SettingsArgs {
    #[command(subcommand)]
    pub command: SettingsCommands,
}

#[derive(Debug, Subcommand)]
pub enum SettingsCommands {
    /// Print the effective settings for a database and where each came from.
    Show(SettingsShowArgs),
}

#[derive(Debug, Args)]
pub struct SettingsShowArgs {
    /// The SQL Server of the database to show the settings for.
    #[arg(long)]
    pub db_server: Option<String>,
    /// The database to show the settings for (its [[database]] entry).
    #[arg(long)]
    pub db_name: Option<String>,
    /// Native ibcmd's config file (the YAML `ibcmd server config init` writes).
    #[arg(long)]
    pub config: Option<PathBuf>,
    /// Show what --platform would give instead of the settings.
    #[arg(long, value_name = "VERSION")]
    pub platform: Option<String>,
    /// Print JSON instead of text.
    #[arg(long)]
    pub json: bool,
}

#[cfg(feature = "platform-oracle")]
#[derive(Debug, Args)]
pub struct ProfileRunArgs {
    /// Keep full stdout/stderr in the JSON report.
    #[arg(long)]
    pub capture_output: bool,
    /// Command and arguments to run. Use `--` before the command.
    #[arg(required = true, trailing_var_arg = true)]
    pub command: Vec<String>,
}

#[cfg(feature = "platform-oracle")]
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
    /// Infobase user passed to ibcmd.
    #[arg(long, short = 'u')]
    pub user: Option<String>,
    /// Infobase password passed to ibcmd. Prefer --password-env for shell history.
    #[arg(long, short = 'P')]
    pub password: Option<String>,
    /// Environment variable containing the infobase password.
    #[arg(long, default_value = "IBCMD_USER_PSW")]
    pub password_env: String,
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
    /// Durable atomic JSON journal for the nested ibcmd subprocess.
    #[arg(long)]
    pub runtime_journal: Option<PathBuf>,
    /// Replace files under the output directory after a successful export.
    #[arg(long)]
    pub overwrite: bool,
    /// Convert TaxiEnableVersion8_2 to TaxiEnableOld in exported Configuration.xml.
    #[arg(long)]
    pub normalize_taxi_old: bool,
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
        conflicts_with_all = ["file_names", "file_name_lists"]
    )]
    pub require_complete_root_metadata: bool,
    /// Fail when a reconstructed source asset omits an opaque property.
    #[arg(
        long,
        requires_all = ["extract_metadata_xml", "no_binary_rows"],
        conflicts_with_all = ["file_names", "file_name_lists"]
    )]
    pub require_complete_source_assets: bool,
    /// Continue a full diagnostic export after form writer rejections that have structured source-asset diagnostics.
    #[arg(
        long,
        requires_all = ["extract_metadata_xml", "no_binary_rows"],
        conflicts_with_all = ["file_names", "file_name_lists"]
    )]
    pub collect_all_source_asset_diagnostics: bool,
    /// Platform the XML is for: a release (8.3.27, 8.5.1) or an exact build
    /// (8.3.27.2214, 8.5.1.1150); 8.3.x reads and writes XML 2.20, 8.5.x 2.21.
    #[arg(
        long,
        value_name = "VERSION",
        value_parser = crate::platform::parse_flag,
        conflicts_with = "source_version"
    )]
    pub platform: Option<crate::platform::PlatformSpec>,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum MssqlExtensionListFormat {
    /// A stable, machine-readable JSON document.
    Json,
    /// A compact human-readable table.
    Table,
}

#[derive(Debug, Args)]
pub struct MssqlExtensionListArgs {
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
    /// SQL Server name.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// SQL Server login. Uses integrated authentication when omitted.
    #[arg(long)]
    pub sql_user: Option<String>,
    /// SQL Server password. Prefer --sql-pwd-env for shell history.
    #[arg(long)]
    pub sql_pwd: Option<String>,
    /// Environment variable containing the SQL Server password.
    #[arg(long, default_value = "IBCMD_DB_PSW")]
    pub sql_pwd_env: String,
    /// Trust the SQL Server certificate without validating it (sqlcmd -C).
    #[arg(long)]
    pub sqlcmd_trust_cert: bool,
    /// SQL Server database name.
    #[arg(long)]
    pub database: String,
    /// Output representation.
    #[arg(long, value_enum, default_value_t = MssqlExtensionListFormat::Table)]
    pub format: MssqlExtensionListFormat,
}

/// Which stored image of an extension `mssql-dump-extension` reads.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum MssqlExtensionImage {
    /// The staged image (`ConfigCASSave`) when the extension has staged rows,
    /// as the native `config export --extension` does; otherwise the active one.
    Auto,
    /// Only the active image, the one the extension registry names.
    Active,
    /// Only the staged image; fails when the extension has no staged rows.
    Staged,
}

#[derive(Debug, Args)]
pub struct MssqlDumpExtensionArgs {
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
    /// The bcp.exe of the --sqlcmd path (default: the one beside sqlcmd).
    #[arg(long)]
    pub bcp_executable: Option<PathBuf>,
    /// SQL Server name.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// SQL Server login. Uses integrated authentication when omitted.
    #[arg(long)]
    pub sql_user: Option<String>,
    /// SQL Server password. Prefer --sql-pwd-env for shell history.
    #[arg(long)]
    pub sql_pwd: Option<String>,
    /// Environment variable containing the SQL Server password.
    #[arg(long, default_value = "IBCMD_DB_PSW")]
    pub sql_pwd_env: String,
    /// Trust the SQL Server certificate for registry and storage reads.
    #[arg(long)]
    pub sqlcmd_trust_cert: bool,
    /// SQL Server database name.
    #[arg(long)]
    pub database: String,
    /// Export exactly this extension name.
    #[arg(
        long,
        conflicts_with = "all_extensions",
        required_unless_present = "all_extensions"
    )]
    pub extension: Option<String>,
    /// Export every extension into a child directory named after the extension.
    #[arg(
        long,
        conflicts_with = "extension",
        required_unless_present = "extension"
    )]
    pub all_extensions: bool,
    /// Output source directory. For --all-extensions, contains one child per extension.
    #[arg(short, long)]
    pub output_dir: PathBuf,
    /// Replace an existing output tree.
    #[arg(long)]
    pub overwrite: bool,
    /// Which stored image to export: the staged one when the extension has
    /// staged rows (what the native export does), the active one, or exactly
    /// one of them.
    #[arg(long, value_enum, default_value_t = MssqlExtensionImage::Auto)]
    pub image: MssqlExtensionImage,
    /// Platform the XML is for: a release (8.3.27, 8.5.1) or an exact build
    /// (8.3.27.2214, 8.5.1.1150); 8.3.x reads and writes XML 2.20, 8.5.x 2.21.
    #[arg(
        long,
        value_name = "VERSION",
        value_parser = crate::platform::parse_flag,
        conflicts_with = "source_version"
    )]
    pub platform: Option<crate::platform::PlatformSpec>,
    /// Hierarchical XML source version.
    #[arg(long, value_enum, default_value_t = InfobaseConfigSourceVersion::V2_20, hide = true)]
    pub source_version: InfobaseConfigSourceVersion,
}

#[derive(Debug, Args)]
pub struct MssqlLoadExtensionArgs {
    /// Exact native MSSQL platform layout. Without --platform (which must name
    /// this build or its release) the XML format is this build's.
    #[arg(long)]
    pub platform_profile: MssqlNativePlatformProfile,
    /// rac executable used to verify the exact RAS agent build.
    #[arg(long, default_value = "rac")]
    pub rac: PathBuf,
    /// RAS endpoint whose exact agent build must match --platform-profile.
    #[arg(long, default_value = "localhost:1545")]
    pub ras_endpoint: String,
    /// Exact cluster UUID containing the target infobase registration.
    #[arg(long, required = true)]
    pub cluster_id: Option<Uuid>,
    /// Exact target infobase UUID.
    #[arg(long, required = true)]
    pub infobase_id: Option<Uuid>,
    /// Infobase administrator used by `rac infobase info`.
    #[arg(long)]
    pub infobase_user: Option<String>,
    /// Infobase administrator password; empty when omitted.
    #[arg(long)]
    pub infobase_pwd: Option<String>,
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
    /// The bcp.exe of the --sqlcmd path (default: the one beside sqlcmd).
    #[arg(long)]
    pub bcp_executable: Option<PathBuf>,
    /// SQL Server name.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// SQL Server login. Uses integrated authentication when omitted.
    #[arg(long)]
    pub sql_user: Option<String>,
    /// SQL Server password. Prefer --sql-pwd-env for shell history.
    #[arg(long)]
    pub sql_pwd: Option<String>,
    /// Environment variable containing the SQL Server password.
    #[arg(long, default_value = "IBCMD_DB_PSW")]
    pub sql_pwd_env: String,
    /// SQL Server database name.
    #[arg(long)]
    pub database: String,
    /// Stage exactly this extension name.
    #[arg(
        long,
        conflicts_with = "all_extensions",
        required_unless_present = "all_extensions"
    )]
    pub extension: Option<String>,
    /// Stage every extension from a same-named child directory.
    #[arg(
        long,
        conflicts_with = "extension",
        required_unless_present = "extension"
    )]
    pub all_extensions: bool,
    /// Input source directory. For --all-extensions, contains one child per extension.
    #[arg(short, long)]
    pub input_dir: PathBuf,
    /// Optional source path prefix to compile. Can be repeated.
    #[arg(long)]
    pub path_prefix: Vec<String>,
    /// Replace existing rows for only the selected extension prefix in ConfigCASSave.
    #[arg(long)]
    pub replace_staging: bool,
    /// Compile and calculate the staged CAS root without writing ConfigCASSave.
    #[arg(long)]
    pub dry_run: bool,
    /// Required acknowledgement for direct writes to a non-lab database.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Trust the SQL Server certificate during staging without validating
    /// it (sqlcmd -C).
    #[arg(long)]
    pub sqlcmd_trust_cert: bool,
    /// Platform the XML is for: a release (8.3.27, 8.5.1) or an exact build
    /// (8.3.27.2214, 8.5.1.1150); 8.3.x reads and writes XML 2.20, 8.5.x 2.21.
    #[arg(
        long,
        value_name = "VERSION",
        value_parser = crate::platform::parse_flag,
        conflicts_with = "source_version"
    )]
    pub platform: Option<crate::platform::PlatformSpec>,
    /// Hierarchical XML source version.
    #[arg(long, value_enum, default_value_t = InfobaseConfigSourceVersion::V2_20, hide = true)]
    pub source_version: InfobaseConfigSourceVersion,
}

#[derive(Debug, Args)]
pub struct MssqlActivateStagedExtensionArgs {
    /// Exact native MSSQL platform layout.
    #[arg(long)]
    pub platform_profile: MssqlNativePlatformProfile,
    #[arg(long, default_value = "rac")]
    pub rac: PathBuf,
    #[arg(long, default_value = "localhost:1545")]
    pub ras_endpoint: String,
    #[arg(long, required = true)]
    pub cluster_id: Option<Uuid>,
    #[arg(long, required = true)]
    pub infobase_id: Option<Uuid>,
    #[arg(long)]
    pub infobase_user: Option<String>,
    #[arg(long)]
    pub infobase_pwd: Option<String>,
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
    /// The bcp.exe of the --sqlcmd path (default: the one beside sqlcmd).
    #[arg(long)]
    pub bcp_executable: Option<PathBuf>,
    #[arg(long, default_value = "localhost")]
    pub server: String,
    #[arg(long)]
    pub sql_user: Option<String>,
    #[arg(long)]
    pub sql_pwd: Option<String>,
    #[arg(long, default_value = "IBCMD_DB_PSW")]
    pub sql_pwd_env: String,
    #[arg(long)]
    pub database: String,
    /// Exact extension name from mssql-extension-list.
    #[arg(long)]
    pub extension: String,
    /// Publication mode. Online keeps existing sessions on their loaded
    /// generation; new sessions switch after platform polling.
    #[arg(long, value_enum)]
    pub mode: MssqlMainActivationModeArg,
    #[arg(long)]
    pub dry_run: bool,
    #[arg(long)]
    pub allow_non_lab: bool,
    #[arg(long)]
    pub sqlcmd_trust_cert: bool,
    #[arg(long)]
    pub script_output: Option<PathBuf>,
    #[arg(long)]
    pub recovery_output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlDumpTimingSummaryArgs {
    /// JSON reports produced by mssql-dump-config.
    #[arg(required = true)]
    pub input: Vec<PathBuf>,
    /// Optional JSON output file. Prints to stdout when omitted.
    #[arg(short, long)]
    pub output: Option<PathBuf>,
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
    /// SQL Server name.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
pub struct MssqlActivationSnapshotArgs {
    /// SQL Server name.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Database to inspect. This command is read-only.
    #[arg(long)]
    pub database: String,
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
    /// JSON destination. Existing files are never replaced.
    #[arg(short, long)]
    pub output: PathBuf,
}

#[derive(Debug, Args)]
pub struct MssqlActivationDiffArgs {
    /// Snapshot taken before activation.
    #[arg(long)]
    pub before: PathBuf,
    /// Snapshot taken after activation.
    #[arg(long)]
    pub after: PathBuf,
    /// Optional JSON destination. Prints to stdout when omitted.
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum MssqlMainActivationModeArg {
    Exclusive,
    Online,
    Live,
    Worker,
}

#[derive(Debug, Clone, Args)]
pub struct MssqlActivateStagedMainArgs {
    /// Exact native MSSQL platform layout.
    #[arg(long)]
    pub platform_profile: MssqlNativePlatformProfile,
    /// Trust the SQL Server certificate while verifying and activating the
    /// native database (sqlcmd -C).
    #[arg(long)]
    pub sqlcmd_trust_cert: bool,
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
    /// The bcp.exe of the --sqlcmd path (default: the one beside sqlcmd).
    #[arg(long)]
    pub bcp_executable: Option<PathBuf>,
    /// SQL Server name.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// SQL Server login; integrated authentication is used when omitted.
    #[arg(long)]
    pub sql_user: Option<String>,
    /// SQL Server password. Prefer --sql-pwd-env.
    #[arg(long)]
    pub sql_pwd: Option<String>,
    /// Environment variable containing the SQL Server password.
    #[arg(long, default_value = "IBCMD_DB_PSW")]
    pub sql_pwd_env: String,
    /// Target MSSQL database.
    #[arg(long)]
    pub database: String,
    /// Publication mode. `exclusive` is carried out by the own config apply
    /// (built-in SQL client), which folds the rows of earlier online
    /// generations as the native apply does; with `--sqlcmd`, and for `live`
    /// and `worker`, a database that holds online generations is refused.
    #[arg(long, value_enum)]
    pub mode: MssqlMainActivationModeArg,
    /// Render and validate the exact transition without executing it.
    #[arg(long)]
    pub dry_run: bool,
    /// Required explicit acknowledgement for a database write.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional path for the generated SQL script.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
    /// Optional path for the bounded recovery JSON artifact.
    #[arg(long)]
    pub recovery_output: Option<PathBuf>,
    /// SQL Server-local tail-log backup retained by live activation.
    #[arg(long)]
    pub tail_log_output: Option<PathBuf>,
    /// live only (#409 F-10): accept that the live switch rolls back the open transactions and running requests of the sessions
    /// of the database. Without it the switch is refused while such sessions exist.
    #[arg(long)]
    pub interrupt_sessions: bool,
    /// rac executable used by worker activation.
    #[arg(long, default_value = "rac")]
    pub rac: PathBuf,
    /// RAS endpoint used by worker activation.
    #[arg(long, default_value = "localhost:1545")]
    pub ras_endpoint: String,
    /// 1C cluster UUID required to bind the verified registration.
    #[arg(long, required = true)]
    pub cluster_id: Option<Uuid>,
    /// 1C infobase UUID required to bind the verified registration.
    #[arg(long, required = true)]
    pub infobase_id: Option<Uuid>,
    #[arg(long)]
    pub infobase_user: Option<String>,
    #[arg(long)]
    pub infobase_pwd: Option<String>,
}

/// How `mssql-config-apply` establishes that nobody else is connected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum MssqlConfigApplyExclusivityArg {
    /// No other user session on the database as SQL Server sees it.
    Sql,
    /// The operator has proved it (for instance with `rac session list`).
    Assumed,
}

/// Which structural gate judges the stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum MssqlConfigApplyGateArg {
    /// The restructure check (`mssql-apply-check`): refuses on a
    /// restructuring and on anything it cannot place.
    ApplyCheck,
    /// Modules, forms, templates, pictures and help pages only.
    Conservative,
}

/// A class of restructuring the apply may let through.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum MssqlConfigApplyRestructureArg {
    /// S1 of the restructure track: attributes, tabular sections, string
    /// widening, the index flag, plain new catalogs and documents.
    S1,
}

/// Which overwritten rows the recovery artifact keeps the bytes of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum MssqlConfigApplyRecoveryArg {
    /// The rows the apply changes.
    Changed,
    /// Only a manifest of hashes.
    None,
}

#[derive(Debug, Clone, Args)]
pub struct MssqlConfigApplyArgs {
    /// Exact native MSSQL platform layout.
    #[arg(long)]
    pub platform_profile: MssqlNativePlatformProfile,
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// SQL Server login; integrated authentication is used when omitted.
    #[arg(long)]
    pub sql_user: Option<String>,
    /// SQL Server password. Prefer --sql-pwd-env.
    #[arg(long)]
    pub sql_pwd: Option<String>,
    #[arg(long, default_value = "IBCMD_DB_PSW")]
    pub sql_pwd_env: String,
    /// Target MSSQL database.
    #[arg(long)]
    pub database: String,
    /// Plan and check the staged configuration and render the transaction,
    /// without writing anything.
    #[arg(long)]
    pub dry_run: bool,
    /// Run the whole transaction and roll it back: proves the SQL and its
    /// postconditions on this database without changing it.
    #[arg(long, conflicts_with = "dry_run")]
    pub rehearse: bool,
    /// Acknowledgement for a write to a database outside the lab (the lab's own are
    /// `ibcmd_rs_04_*` and `ibcmd_rs_05_*`: no flag needed there).
    #[arg(long)]
    pub allow_non_lab: bool,
    /// How exclusive access is established.
    #[arg(long, value_enum, default_value_t = MssqlConfigApplyExclusivityArg::Sql)]
    pub exclusivity: MssqlConfigApplyExclusivityArg,
    /// Directory for the recovery artifact (default: a folder in the
    /// temporary directory named after the database and the plan).
    #[arg(long)]
    pub recovery_dir: Option<PathBuf>,
    #[arg(long, value_enum, default_value_t = MssqlConfigApplyRecoveryArg::Changed)]
    pub recovery_blobs: MssqlConfigApplyRecoveryArg,
    /// With no --recovery-dir: how many recovery artifacts of the database the
    /// default directory (%TEMP%\ibcmd-rs\config-apply-recovery) keeps after a
    /// successful run; older ones are removed. 0 keeps all of them.
    #[arg(long, default_value_t = 5)]
    pub recovery_keep: usize,
    /// Let a class of restructuring through, run inside the apply's transaction,
    /// instead of refusing it. Needs `--i-have-a-backup` or `--recovery-backup`.
    #[arg(long, value_enum)]
    pub allow_restructure: Option<MssqlConfigApplyRestructureArg>,
    /// A restructuring drops the old tables in the transaction: say you have a
    /// SQL Server backup to go back to.
    #[arg(long)]
    pub i_have_a_backup: bool,
    /// Take `BACKUP DATABASE ... WITH COPY_ONLY` to this file (a path the SQL
    /// Server service can write) before a restructuring.
    #[arg(long)]
    pub recovery_backup: Option<PathBuf>,
    /// With --allow-restructure: the most rows of tables a stage may rebuild in the transaction (the sum);
    /// above it the stage is refused and goes to the native apply. Without the flag:
    /// IBCMD_RS_RESTRUCTURE_LIMIT_ROWS, `restructure-limit-rows` of ibcmd-rs.toml, the measured default.
    #[arg(long)]
    pub restructure_limit_rows: Option<u64>,
    /// With --allow-restructure: the most bytes the rebuild may write to the log under full recovery (the
    /// data of the rebuilt tables twice, their other indexes once; the sum; `2GB`, `512MB`, a number of
    /// bytes). Without the flag: IBCMD_RS_RESTRUCTURE_LIMIT_BYTES, `restructure-limit-bytes` of
    /// ibcmd-rs.toml, the measured default. A raised limit still needs --i-have-a-backup or --recovery-backup.
    #[arg(long)]
    pub restructure_limit_bytes: Option<String>,
    /// The structural gate: the restructure check of `mssql-apply-check`
    /// (default), or the conservative rule.
    #[arg(long, value_enum, default_value_t = MssqlConfigApplyGateArg::ApplyCheck)]
    pub gate: MssqlConfigApplyGateArg,
    /// With `--gate conservative`: pass changed body rows of
    /// command-interface, rights, package and similar roles without proving
    /// their text unchanged (a stage made by `infobase config import` of a
    /// tree exported from this database).
    #[arg(long)]
    pub admit_unverified_roles: bool,
    /// Write the rendered SQL transaction here.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
    /// Write the JSON report here as well as to stdout.
    #[arg(long)]
    pub report: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct MssqlApplySourceChangeArgs {
    /// Exact native MSSQL platform layout. Without --platform (which must name
    /// this build or its release) the XML format is this build's.
    #[arg(long)]
    pub platform_profile: MssqlNativePlatformProfile,
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
    /// The bcp.exe of the --sqlcmd path (default: the one beside sqlcmd).
    #[arg(long)]
    pub bcp_executable: Option<PathBuf>,
    #[arg(long, default_value = "localhost")]
    pub server: String,
    #[arg(long)]
    pub sql_user: Option<String>,
    #[arg(long)]
    pub sql_pwd: Option<String>,
    #[arg(long, default_value = "IBCMD_DB_PSW")]
    pub sql_pwd_env: String,
    #[arg(long)]
    pub sqlcmd_trust_cert: bool,
    #[arg(long)]
    pub database: String,
    #[arg(long)]
    pub source_root: PathBuf,
    #[arg(long = "path")]
    pub source_path: PathBuf,
    #[arg(long)]
    pub extension: Option<String>,
    #[arg(long, value_enum)]
    pub mode: MssqlMainActivationModeArg,
    #[arg(long)]
    pub dry_run: bool,
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Platform the XML is for: a release (8.3.27, 8.5.1) or an exact build
    /// (8.3.27.2214, 8.5.1.1150); 8.3.x reads and writes XML 2.20, 8.5.x 2.21.
    #[arg(
        long,
        value_name = "VERSION",
        value_parser = crate::platform::parse_flag,
        conflicts_with = "source_version"
    )]
    pub platform: Option<crate::platform::PlatformSpec>,
    #[arg(long, value_enum, default_value_t = InfobaseConfigSourceVersion::V2_20, hide = true)]
    pub source_version: InfobaseConfigSourceVersion,
    #[arg(long)]
    pub script_output: Option<PathBuf>,
    #[arg(long)]
    pub recovery_output: Option<PathBuf>,
    /// SQL Server-local tail-log backup retained by live activation.
    #[arg(long)]
    pub tail_log_output: Option<PathBuf>,
    /// live only (#409 F-10): accept that the live switch rolls back the open transactions and running requests of the sessions
    /// of the database. Without it the switch is refused while such sessions exist.
    #[arg(long)]
    pub interrupt_sessions: bool,
    #[arg(long, default_value = "rac")]
    pub rac: PathBuf,
    #[arg(long, default_value = "localhost:1545")]
    pub ras_endpoint: String,
    #[arg(long, required = true)]
    pub cluster_id: Option<Uuid>,
    #[arg(long, required = true)]
    pub infobase_id: Option<Uuid>,
    #[arg(long)]
    pub infobase_user: Option<String>,
    #[arg(long)]
    pub infobase_pwd: Option<String>,
    /// Watch the selected source closure and activate every stable save.
    #[arg(long)]
    pub watch: bool,
    /// Stable interval used to debounce editor save bursts.
    #[arg(long, default_value_t = 300)]
    pub watch_debounce_ms: u64,
}

#[derive(Debug, Args)]
pub struct MssqlAuditSourceParityArgs {
    /// SQL Server name.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Baseline database name whose Config blobs are used for dry-run packing.
    #[arg(long)]
    pub database: String,
    /// Root folder with XML sources to scan.
    #[arg(long)]
    pub source_root: PathBuf,
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
    /// Optional maximum number of staged XML objects per SQL batch.
    #[arg(long)]
    pub batch_size: Option<usize>,
    /// Platform the XML is for: a release (8.3.27, 8.5.1) or an exact build
    /// (8.3.27.2214, 8.5.1.1150); 8.3.x reads and writes XML 2.20, 8.5.x 2.21.
    #[arg(
        long,
        value_name = "VERSION",
        value_parser = crate::platform::parse_flag,
        conflicts_with = "source_version"
    )]
    pub platform: Option<crate::platform::PlatformSpec>,
    /// Expected source XML version. When set, selected root XML files must match it.
    #[arg(long, value_enum, hide = true)]
    pub source_version: Option<InfobaseConfigSourceVersion>,
    /// Optional source path prefix to audit. Can be repeated.
    #[arg(long)]
    pub path_prefix: Vec<String>,
    /// Optional JSON output file. Prints to stdout when omitted.
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct MssqlCloneArgs {
    /// SQL Server name.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name (bcp -S; statistics through the built-in client or
    /// --sqlcmd).
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Source database name.
    #[arg(long)]
    pub database: String,
    /// Output bundle directory.
    #[arg(short, long)]
    pub output_dir: PathBuf,
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
    /// bcp.exe: the bundle files are bcp native-format files, so these lab
    /// commands run bcp.exe even without --sqlcmd.
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
    /// SQL Server name (bcp -S; statistics through the built-in client or
    /// --sqlcmd).
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Input bundle directory produced by `mssql-storage-export`.
    #[arg(short, long)]
    pub input_dir: PathBuf,
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
    /// bcp.exe: the bundle files are bcp native-format files, so these lab
    /// commands run bcp.exe even without --sqlcmd.
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
    /// SQL Server name (bcp -S; statistics through the built-in client or
    /// --sqlcmd).
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Source database name with pending rows in ConfigSave.
    #[arg(long)]
    pub database: String,
    /// Output bundle directory.
    #[arg(short, long)]
    pub output_dir: PathBuf,
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
    /// bcp.exe: the bundle files are bcp native-format files, so these lab
    /// commands run bcp.exe even without --sqlcmd.
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
    /// SQL Server name (bcp -S; statistics through the built-in client or
    /// --sqlcmd).
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Input bundle directory produced by `mssql-delta-export`.
    #[arg(short, long)]
    pub input_dir: PathBuf,
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
    /// bcp.exe: the bundle files are bcp native-format files, so these lab
    /// commands run bcp.exe even without --sqlcmd.
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
    /// SQL Server name.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// SQL Server login. Uses Windows (integrated) authentication when omitted.
    #[arg(long)]
    pub sql_user: Option<String>,
    /// SQL Server password. Prefer --sql-pwd-env for shell history.
    #[arg(long)]
    pub sql_pwd: Option<String>,
    /// Environment variable containing the SQL password.
    #[arg(long, default_value = "IBCMD_DB_PSW")]
    pub sql_pwd_env: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Common module metadata UUID without `.0`.
    #[arg(long)]
    pub module_id: String,
    /// BSL module body file.
    #[arg(long)]
    pub text: PathBuf,
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// SQL Server login. Uses Windows (integrated) authentication when omitted.
    #[arg(long)]
    pub sql_user: Option<String>,
    /// SQL Server password. Prefer --sql-pwd-env for shell history.
    #[arg(long)]
    pub sql_pwd: Option<String>,
    /// Environment variable containing the SQL password.
    #[arg(long, default_value = "IBCMD_DB_PSW")]
    pub sql_pwd_env: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Common module change in the form `<metadata-uuid>=<path-to-Module.bsl>`.
    #[arg(long = "module", required = true)]
    pub modules: Vec<String>,
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// SQL Server login. Uses Windows (integrated) authentication when omitted.
    #[arg(long)]
    pub sql_user: Option<String>,
    /// SQL Server password. Prefer --sql-pwd-env for shell history.
    #[arg(long)]
    pub sql_pwd: Option<String>,
    /// Environment variable containing the SQL password.
    #[arg(long, default_value = "IBCMD_DB_PSW")]
    pub sql_pwd_env: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Common module metadata UUID.
    #[arg(long)]
    pub module_id: String,
    /// Common module XML file.
    #[arg(long)]
    pub xml: PathBuf,
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// SQL Server login. Uses Windows (integrated) authentication when omitted.
    #[arg(long)]
    pub sql_user: Option<String>,
    /// SQL Server password. Prefer --sql-pwd-env for shell history.
    #[arg(long)]
    pub sql_pwd: Option<String>,
    /// Environment variable containing the SQL password.
    #[arg(long, default_value = "IBCMD_DB_PSW")]
    pub sql_pwd_env: String,
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
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// SQL Server login. Uses Windows (integrated) authentication when omitted.
    #[arg(long)]
    pub sql_user: Option<String>,
    /// SQL Server password. Prefer --sql-pwd-env for shell history.
    #[arg(long)]
    pub sql_pwd: Option<String>,
    /// Environment variable containing the SQL password.
    #[arg(long, default_value = "IBCMD_DB_PSW")]
    pub sql_pwd_env: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Common module XML files. Each sibling <module-name>\Ext\Module.bsl is loaded too.
    #[arg(long = "xml", required = true)]
    pub xmls: Vec<PathBuf>,
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// SQL Server login. Uses Windows (integrated) authentication when omitted.
    #[arg(long)]
    pub sql_user: Option<String>,
    /// SQL Server password. Prefer --sql-pwd-env for shell history.
    #[arg(long)]
    pub sql_pwd: Option<String>,
    /// Environment variable containing the SQL password.
    #[arg(long, default_value = "IBCMD_DB_PSW")]
    pub sql_pwd_env: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Metadata XML files for supported metadata-only patchers.
    #[arg(long = "xml", required = true)]
    pub xmls: Vec<PathBuf>,
    /// Root folder with full XML sources, used to resolve metadata references.
    #[arg(long)]
    pub source_root: Option<PathBuf>,
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// SQL Server login. Uses Windows (integrated) authentication when omitted.
    #[arg(long)]
    pub sql_user: Option<String>,
    /// SQL Server password. Prefer --sql-pwd-env for shell history.
    #[arg(long)]
    pub sql_pwd: Option<String>,
    /// Environment variable containing the SQL password.
    #[arg(long, default_value = "IBCMD_DB_PSW")]
    pub sql_pwd_env: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Root folder with XML sources to scan.
    #[arg(long)]
    pub source_root: PathBuf,
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// SQL Server login. Uses Windows (integrated) authentication when omitted.
    #[arg(long)]
    pub sql_user: Option<String>,
    /// SQL Server password. Prefer --sql-pwd-env for shell history.
    #[arg(long)]
    pub sql_pwd: Option<String>,
    /// Environment variable containing the SQL password.
    #[arg(long, default_value = "IBCMD_DB_PSW")]
    pub sql_pwd_env: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Root folder with XML sources to scan.
    #[arg(long)]
    pub source_root: PathBuf,
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    /// SQL Server login. Uses Windows (integrated) authentication when omitted.
    #[arg(long)]
    pub sql_user: Option<String>,
    /// SQL Server password. Prefer --sql-pwd-env for shell history.
    #[arg(long)]
    pub sql_pwd: Option<String>,
    /// Environment variable containing the SQL password.
    #[arg(long, default_value = "IBCMD_DB_PSW")]
    pub sql_pwd_env: String,
    /// Target database name.
    #[arg(long)]
    pub database: String,
    /// Root folder with XML sources to scan.
    #[arg(long)]
    pub source_root: PathBuf,
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
    /// Required confirmation: delete existing ConfigSave rows first.
    #[arg(long)]
    pub replace_config_save: bool,
    /// Required confirmation for non-lab destructive runs.
    #[arg(long)]
    pub allow_non_lab: bool,
    /// Optional maximum number of staged XML objects per SQL batch.
    #[arg(long)]
    pub batch_size: Option<usize>,
    /// Platform the XML is for: a release (8.3.27, 8.5.1) or an exact build
    /// (8.3.27.2214, 8.5.1.1150); 8.3.x reads and writes XML 2.20, 8.5.x 2.21.
    #[arg(
        long,
        value_name = "VERSION",
        value_parser = crate::platform::parse_flag,
        conflicts_with = "source_version"
    )]
    pub platform: Option<crate::platform::PlatformSpec>,
    /// Expected source XML version. When set, selected root XML files must match it.
    #[arg(long, value_enum, hide = true)]
    pub source_version: Option<InfobaseConfigSourceVersion>,
    /// Optional source path prefix to stage. Can be repeated.
    #[arg(long)]
    pub path_prefix: Vec<String>,
    /// Optional path for generated SQL script. Defaults to C:\temp\ibcmd-rs.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
    /// Write every batch script without running it: the database is only
    /// read. The usual confirmations are still required.
    #[arg(long)]
    pub script_only: bool,
    /// Load the staged rows in bulk into a tempdb table and apply them in one
    /// set-based transaction, reading the base rows in one pass: the
    /// default (with --sqlcmd, through bcp.exe). Writes the same ConfigSave
    /// rows as --per-row.
    #[arg(long)]
    pub bulk: bool,
    /// Stage with per-row SQL batches and per-object base-row queries instead
    /// of the bulk load (cheaper for a handful of objects).
    #[arg(long, conflicts_with = "bulk")]
    pub per_row: bool,
    /// The bcp.exe of the --sqlcmd path (default: the one beside sqlcmd).
    #[arg(long)]
    pub bcp_executable: Option<PathBuf>,
    /// Stage for an EMPTY infobase: every row of the tree (descriptors from
    /// the base-free compiler, bodies from writers that read no base row,
    /// fresh root/version/versions) and an apply that reads nothing from
    /// Config. With --script-only it runs fully offline.
    #[arg(long, conflicts_with = "per_row")]
    pub base_free: bool,
    /// Before anything is written, export the state the stage would leave --
    /// the stored rows with the staged ones in place of theirs -- with the
    /// model and compare every file with the tree. A difference refuses the
    /// stage and ConfigSave is left as it was. `infobase config import` does
    /// this by itself for a patch stage (`--no-verify` there skips it).
    #[arg(long)]
    pub verify: bool,
}

#[derive(Debug, Args)]
pub struct MssqlStageExchangePlanObjectArgs {
    /// SQL Server name.
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
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
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
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
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
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
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
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
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
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
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
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
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
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
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
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
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
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
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
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
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
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
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
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
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
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
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
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
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
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
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
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
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
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
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
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
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
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
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
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
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
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
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
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
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
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
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
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
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
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
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
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
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
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
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
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
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
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
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
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
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
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
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
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
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
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
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
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
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
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
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
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
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
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
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
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
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
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
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
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
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
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
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
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
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
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
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
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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
    /// SQL Server name.
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
    /// Run this sqlcmd.exe (and bcp.exe) instead of the built-in SQL Server
    /// client, as ibcmd-rs 0.2 did; for scripts that still pass it.
    #[arg(long)]
    pub sqlcmd: Option<PathBuf>,
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

        let form_diff = Cli::parse_from([
            "ibcmd-rs",
            "form-diff-candidates",
            "--base-xml",
            r"C:\forms\base.xml",
            "--variant-xml",
            r"C:\forms\variant.xml",
            "--base-blob",
            r"C:\forms\base.bin",
            "--variant-blob",
            r"C:\forms\variant.bin",
            "-o",
            r"C:\audit\form-diff.json",
        ]);
        match form_diff.command {
            Commands::FormDiffCandidates(args) => {
                assert_eq!(args.base_xml, PathBuf::from(r"C:\forms\base.xml"));
                assert_eq!(args.variant_xml, PathBuf::from(r"C:\forms\variant.xml"));
                assert_eq!(args.base_blob, PathBuf::from(r"C:\forms\base.bin"));
                assert_eq!(args.variant_blob, PathBuf::from(r"C:\forms\variant.bin"));
                assert_eq!(args.output, Some(PathBuf::from(r"C:\audit\form-diff.json")));
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

        let diff = Cli::parse_from([
            "ibcmd-rs",
            "source-diff",
            r"C:\reference",
            r"C:\candidate",
            "--path-prefix",
            "Catalogs/Products",
            "-o",
            r"C:\audit\source-diff.json",
        ]);
        match diff.command {
            Commands::SourceDiff(args) => {
                assert_eq!(args.left, PathBuf::from(r"C:\reference"));
                assert_eq!(args.right, PathBuf::from(r"C:\candidate"));
                assert_eq!(args.path_prefix, vec!["Catalogs/Products"]);
                assert_eq!(
                    args.output,
                    Some(PathBuf::from(r"C:\audit\source-diff.json"))
                );
            }
            other => panic!("unexpected command: {other:?}"),
        }

        let signatures = Cli::parse_from([
            "ibcmd-rs",
            "source-diff-signatures",
            r"C:\audit\source-diff.json",
            "--max-files-per-kind",
            "100",
            "--kind-limit",
            "form=900",
            "--kind-limit",
            "template=450",
            "--top",
            "50",
            "--examples-per-signature",
            "2",
            "-o",
            r"C:\audit\source-diff-signatures.json",
        ]);
        match signatures.command {
            Commands::SourceDiffSignatures(args) => {
                assert_eq!(args.diff, PathBuf::from(r"C:\audit\source-diff.json"));
                assert_eq!(args.max_files_per_kind, Some(100));
                assert_eq!(args.kind_limit, vec!["form=900", "template=450"]);
                assert_eq!(args.top, 50);
                assert_eq!(args.examples_per_signature, 2);
                assert_eq!(
                    args.output,
                    Some(PathBuf::from(r"C:\audit\source-diff-signatures.json"))
                );
            }
            other => panic!("unexpected command: {other:?}"),
        }

        let oracle = Cli::parse_from([
            "ibcmd-rs",
            "source-three-way-oracle",
            "--native",
            r"C:\oracle\native",
            "--edt",
            r"C:\oracle\edt",
            "--ours",
            r"C:\oracle\ours",
            "--source-version",
            "2.20",
            "--native-tool-version",
            "8.3.27.1989",
            "--edt-tool-version",
            "2025.2.3+30 import/export",
            "--ours-tool-version",
            "git:abc",
            "--output",
            r"C:\oracle\report.json",
            "--markdown",
            r"C:\oracle\report.md",
        ]);
        match oracle.command {
            Commands::SourceThreeWayOracle(args) => {
                assert_eq!(args.native, PathBuf::from(r"C:\oracle\native"));
                assert_eq!(args.edt, PathBuf::from(r"C:\oracle\edt"));
                assert_eq!(args.ours, PathBuf::from(r"C:\oracle\ours"));
                assert_eq!(args.source_version, "2.20");
                assert_eq!(args.max_files, 100_000);
                assert_eq!(args.max_total_bytes, 4 * 1024 * 1024 * 1024_u64);
                assert_eq!(args.max_file_bytes, 512 * 1024 * 1024_u64);
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
            "--source-version=8.5.1",
            "--replace-config-save",
            "--allow-non-lab",
        ]);
        match tree.command {
            Commands::MssqlStageSourceObjects(args) => {
                assert_eq!(args.database, "TestDb");
                assert_eq!(args.source_root, PathBuf::from(r"C:\sources"));
                assert_eq!(args.path_prefix, vec!["Catalogs/Products"]);
                assert_eq!(
                    args.source_version,
                    Some(InfobaseConfigSourceVersion::V2_21)
                );
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
            "--source-version=2.20",
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
                assert_eq!(
                    args.source_version,
                    Some(InfobaseConfigSourceVersion::V2_20)
                );
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

    #[cfg(feature = "platform-oracle")]
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
            "--runtime-journal",
            r"C:\repo\logs\native-runtime.json",
            "--user",
            "ws",
            "--password-env",
            "IBCMD_USER_PSW",
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
                assert_eq!(
                    args.runtime_journal,
                    Some(PathBuf::from(r"C:\repo\logs\native-runtime.json"))
                );
                assert_eq!(args.user.as_deref(), Some("ws"));
                assert_eq!(args.password_env, "IBCMD_USER_PSW");
                assert!(args.overwrite);
                assert!(args.normalize_taxi_old);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[cfg(feature = "platform-oracle")]
    #[test]
    fn parses_infobase_config_roundtrip_command() {
        let cli = InfobaseOracleCli::parse_from([
            "ibcmd-rs infobase config",
            "roundtrip",
            "--db-server",
            "localhost",
            "--db-name",
            "ut_ibcmd",
            "--db-user",
            "sa",
            "--db-pwd",
            "dummy-sql-value-for-parser-test",
            "--ibcmd",
            r"C:\Program Files\1cv8\8.3.27.1989\bin\ibcmd.exe",
            "--target-db",
            "ut_ibcmd_roundtrip_test",
            "--work-dir",
            r"E:\ibcmd_lab\roundtrip",
            "--source-dir",
            r"E:\ibcmd_lab\roundtrip\baseline",
            "--allow-non-lab",
            "--batch-size",
            "25",
            "--path-prefix",
            "Catalogs/Валюты",
            "--timeout-sec",
            "900",
            "--overwrite",
        ]);

        match cli.command {
            InfobaseOracleCommands::Roundtrip(args) => {
                assert_eq!(args.db_server.as_deref(), Some("localhost"));
                assert_eq!(args.db_name.as_deref(), Some("ut_ibcmd"));
                assert_eq!(args.db_user.as_deref(), Some("sa"));
                assert_eq!(
                    args.db_pwd.as_deref(),
                    Some("dummy-sql-value-for-parser-test")
                );
                assert_eq!(
                    args.ibcmd,
                    Some(PathBuf::from(
                        r"C:\Program Files\1cv8\8.3.27.1989\bin\ibcmd.exe"
                    ))
                );
                assert_eq!(args.target_db.as_deref(), Some("ut_ibcmd_roundtrip_test"));
                assert_eq!(args.work_dir, PathBuf::from(r"E:\ibcmd_lab\roundtrip"));
                assert_eq!(
                    args.source_dir,
                    Some(PathBuf::from(r"E:\ibcmd_lab\roundtrip\baseline"))
                );
                assert!(args.allow_non_lab);
                assert_eq!(args.batch_size, Some(25));
                assert_eq!(args.path_prefix, vec!["Catalogs/Валюты"]);
                assert_eq!(args.timeout_sec, 900);
                assert!(args.overwrite);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[cfg(feature = "platform-oracle")]
    #[test]
    fn parses_infobase_config_sweep_command() {
        let cli = InfobaseOracleCli::parse_from([
            "ibcmd-rs infobase config",
            "sweep",
            "--db-server",
            "localhost",
            "--db-name",
            "ut_ibcmd",
            "--db-user",
            "sa",
            "--db-pwd",
            "dummy-sql-value-for-parser-test",
            "--ibcmd",
            r"C:\Program Files\1cv8\8.3.27.1989\bin\ibcmd.exe",
            "--work-dir",
            r"E:\ibcmd_lab\roundtrip",
            "--source-dir",
            r"E:\ibcmd_lab\roundtrip\baseline",
            "--allow-non-lab",
            "--batch-size",
            "25",
            "--family",
            "Catalogs",
            "--family",
            "Documents",
            "--candidate-offset",
            "1",
            "--candidates-per-family",
            "2",
            "--max-prefixes",
            "3",
            "--path-prefix",
            "Catalogs/Валюты",
            "--stop-on-first-non-ok",
            "--drop-target-db-after-run",
            "--timeout-sec",
            "900",
            "--overwrite",
        ]);

        match cli.command {
            InfobaseOracleCommands::Sweep(args) => {
                assert_eq!(args.db_server.as_deref(), Some("localhost"));
                assert_eq!(args.db_name.as_deref(), Some("ut_ibcmd"));
                assert_eq!(args.db_user.as_deref(), Some("sa"));
                assert_eq!(
                    args.db_pwd.as_deref(),
                    Some("dummy-sql-value-for-parser-test")
                );
                assert_eq!(
                    args.ibcmd,
                    Some(PathBuf::from(
                        r"C:\Program Files\1cv8\8.3.27.1989\bin\ibcmd.exe"
                    ))
                );
                assert_eq!(args.work_dir, PathBuf::from(r"E:\ibcmd_lab\roundtrip"));
                assert_eq!(
                    args.source_dir,
                    Some(PathBuf::from(r"E:\ibcmd_lab\roundtrip\baseline"))
                );
                assert!(args.allow_non_lab);
                assert_eq!(args.batch_size, Some(25));
                assert_eq!(args.family, vec!["Catalogs", "Documents"]);
                assert_eq!(args.candidate_offset, 1);
                assert_eq!(args.candidates_per_family, 2);
                assert_eq!(args.max_prefixes, Some(3));
                assert_eq!(args.path_prefix, vec!["Catalogs/Валюты"]);
                assert!(args.stop_on_first_non_ok);
                assert!(args.drop_target_db_after_run);
                assert_eq!(args.timeout_sec, 900);
                assert!(args.overwrite);
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
            "--runtime-journal",
            r"C:\logs\candidate-runtime.json",
            "--bcp-executable",
            r"C:\tools\bcp.exe",
            "--sql-user",
            "test-sql-user",
            "--sql-pwd",
            "dummy-sql-value-for-parser-test",
            "--file-name",
            "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa",
            "--file-name-list",
            r"C:\dump\selected.txt",
            "-o",
            r"C:\dump",
            "--include-config-save",
            "--inflate",
            "--extract-module-text",
            "--extract-metadata-xml",
            "--no-binary-rows",
            "--overwrite",
        ]);

        match cli.command {
            Commands::MssqlDumpConfig(args) => {
                assert_eq!(args.database, "TestDb");
                assert_eq!(
                    args.runtime_journal,
                    Some(PathBuf::from(r"C:\logs\candidate-runtime.json"))
                );
                assert_eq!(
                    args.bcp_executable,
                    Some(PathBuf::from(r"C:\tools\bcp.exe"))
                );
                assert_eq!(args.sql_user.as_deref(), Some("test-sql-user"));
                assert_eq!(
                    args.sql_pwd.as_deref(),
                    Some("dummy-sql-value-for-parser-test")
                );
                assert_eq!(
                    args.file_names,
                    vec!["aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa".to_string()]
                );
                assert_eq!(
                    args.file_name_lists,
                    vec![PathBuf::from(r"C:\dump\selected.txt")]
                );
                assert_eq!(args.output_dir, PathBuf::from(r"C:\dump"));
                assert!(args.include_config_save);
                assert!(args.inflate);
                assert!(args.extract_module_text);
                assert!(args.extract_metadata_xml);
                assert!(args.no_binary_rows);
                assert!(args.overwrite);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_strict_full_mssql_dump_config_command() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-dump-config",
            "--database",
            "TestDb",
            "-o",
            r"C:\dump",
            "--extract-metadata-xml",
            "--require-complete-root-metadata",
            "--include-config-save",
        ]);

        match cli.command {
            Commands::MssqlDumpConfig(args) => {
                assert!(args.extract_metadata_xml);
                assert!(args.require_complete_root_metadata);
                // ConfigSave may be staged in the same run, but the strict
                // inventory gate is applied only to the committed Config table.
                assert!(args.include_config_save);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_strict_source_asset_mssql_dump_config_command() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-dump-config",
            "--database",
            "TestDb",
            "-o",
            r"C:\dump",
            "--extract-metadata-xml",
            "--no-binary-rows",
            "--require-complete-source-assets",
        ]);

        match cli.command {
            Commands::MssqlDumpConfig(args) => {
                assert!(args.extract_metadata_xml);
                assert!(args.no_binary_rows);
                assert!(args.require_complete_source_assets);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_collect_all_source_asset_diagnostics_command() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-dump-config",
            "--database",
            "TestDb",
            "-o",
            r"C:\dump",
            "--extract-metadata-xml",
            "--no-binary-rows",
            "--collect-all-source-asset-diagnostics",
            "--require-complete-source-assets",
        ]);

        match cli.command {
            Commands::MssqlDumpConfig(args) => {
                assert!(args.collect_all_source_asset_diagnostics);
                assert!(args.require_complete_source_assets);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn rejects_invalid_strict_mssql_dump_config_combinations() {
        assert!(
            Cli::try_parse_from([
                "ibcmd-rs",
                "mssql-dump-config",
                "--database",
                "TestDb",
                "-o",
                r"C:\dump",
                "--require-complete-root-metadata",
            ])
            .is_err()
        );
        assert!(
            Cli::try_parse_from([
                "ibcmd-rs",
                "mssql-dump-config",
                "--database",
                "TestDb",
                "-o",
                r"C:\dump",
                "--collect-all-source-asset-diagnostics",
            ])
            .is_err()
        );
        assert!(
            Cli::try_parse_from([
                "ibcmd-rs",
                "mssql-dump-config",
                "--database",
                "TestDb",
                "-o",
                r"C:\dump",
                "--extract-metadata-xml",
                "--no-binary-rows",
                "--collect-all-source-asset-diagnostics",
                "--file-name",
                "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa",
            ])
            .is_err()
        );
        assert!(
            Cli::try_parse_from([
                "ibcmd-rs",
                "mssql-dump-config",
                "--database",
                "TestDb",
                "-o",
                r"C:\dump",
                "--extract-metadata-xml",
                "--require-complete-source-assets",
            ])
            .is_err()
        );
        assert!(
            Cli::try_parse_from([
                "ibcmd-rs",
                "mssql-dump-config",
                "--database",
                "TestDb",
                "-o",
                r"C:\dump",
                "--extract-metadata-xml",
                "--require-complete-root-metadata",
                "--file-name",
                "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa",
            ])
            .is_err()
        );
        assert!(
            Cli::try_parse_from([
                "ibcmd-rs",
                "mssql-dump-config",
                "--database",
                "TestDb",
                "-o",
                r"C:\dump",
                "--extract-metadata-xml",
                "--require-complete-root-metadata",
                "--file-name-list",
                r"C:\dump\selected.txt",
            ])
            .is_err()
        );
    }

    #[test]
    fn parses_mssql_dump_timing_summary_command() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-dump-timing-summary",
            r"E:\ibcmd_lab\perf\full-report.json",
            r"E:\ibcmd_lab\perf\selected-report.json",
            "-o",
            r"E:\ibcmd_lab\perf\timing-summary.json",
        ]);

        match cli.command {
            Commands::MssqlDumpTimingSummary(args) => {
                assert_eq!(
                    args.input,
                    vec![
                        PathBuf::from(r"E:\ibcmd_lab\perf\full-report.json"),
                        PathBuf::from(r"E:\ibcmd_lab\perf\selected-report.json")
                    ]
                );
                assert_eq!(
                    args.output,
                    Some(PathBuf::from(r"E:\ibcmd_lab\perf\timing-summary.json"))
                );
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_mssql_stage_source_objects_with_sql_auth() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-stage-source-objects",
            "--server",
            "sql01",
            "--sql-user",
            "stage-user",
            "--sql-pwd",
            "stage-secret",
            "--database",
            "ut_ibcmd",
            "--source-root",
            r"D:\src\ut_ibcmd",
            "--replace-config-save",
            "--allow-non-lab",
            "--batch-size",
            "250",
            "--source-version",
            "2.21",
            "--path-prefix",
            "Catalogs/Валюты",
            "--script-output",
            r"C:\temp\source-stage.sql",
        ]);

        match cli.command {
            Commands::MssqlStageSourceObjects(args) => {
                assert_eq!(args.server, "sql01");
                assert_eq!(args.sql_user.as_deref(), Some("stage-user"));
                assert_eq!(args.sql_pwd.as_deref(), Some("stage-secret"));
                assert_eq!(args.database, "ut_ibcmd");
                assert_eq!(args.source_root, PathBuf::from(r"D:\src\ut_ibcmd"));
                assert!(args.replace_config_save);
                assert!(args.allow_non_lab);
                assert_eq!(args.batch_size, Some(250));
                assert_eq!(
                    args.source_version,
                    Some(InfobaseConfigSourceVersion::V2_21)
                );
                assert_eq!(args.path_prefix, vec!["Catalogs/Валюты".to_string()]);
                assert_eq!(
                    args.script_output,
                    Some(PathBuf::from(r"C:\temp\source-stage.sql"))
                );
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_mssql_stage_metadata_objects_with_sql_auth() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-stage-metadata-objects",
            "--server",
            "sql02",
            "--sql-user",
            "meta-user",
            "--sql-pwd",
            "meta-secret",
            "--database",
            "ut_ibcmd",
            "--xml",
            r"Constants\SomeConstant.xml",
            "--source-root",
            r"D:\src\ut_ibcmd",
            "--replace-config-save",
            "--allow-non-lab",
        ]);

        match cli.command {
            Commands::MssqlStageMetadataObjects(args) => {
                assert_eq!(args.server, "sql02");
                assert_eq!(args.sql_user.as_deref(), Some("meta-user"));
                assert_eq!(args.sql_pwd.as_deref(), Some("meta-secret"));
                assert_eq!(args.database, "ut_ibcmd");
                assert_eq!(
                    args.xmls,
                    vec![PathBuf::from(r"Constants\SomeConstant.xml")]
                );
                assert_eq!(args.source_root, Some(PathBuf::from(r"D:\src\ut_ibcmd")));
                assert!(args.replace_config_save);
                assert!(args.allow_non_lab);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_mssql_stage_common_module_object_with_sql_auth() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-stage-common-module-object",
            "--server",
            "sql03",
            "--sql-user",
            "module-user",
            "--sql-pwd",
            "module-secret",
            "--database",
            "ut_ibcmd",
            "--xml",
            r"CommonModules\Module.xml",
            "--text",
            r"CommonModules\Module\Ext\Module.bsl",
            "--replace-config-save",
            "--allow-non-lab",
        ]);

        match cli.command {
            Commands::MssqlStageCommonModuleObject(args) => {
                assert_eq!(args.server, "sql03");
                assert_eq!(args.sql_user.as_deref(), Some("module-user"));
                assert_eq!(args.sql_pwd.as_deref(), Some("module-secret"));
                assert_eq!(args.database, "ut_ibcmd");
                assert_eq!(args.xml, PathBuf::from(r"CommonModules\Module.xml"));
                assert_eq!(
                    args.text,
                    Some(PathBuf::from(r"CommonModules\Module\Ext\Module.bsl"))
                );
                assert!(args.replace_config_save);
                assert!(args.allow_non_lab);
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

    #[test]
    fn parses_base_free_cf_bootstrap_with_independent_axes() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "cf",
            "bootstrap",
            "source-tree",
            "configuration.cf",
            "--source-version",
            "2.21",
            "--target-profile",
            "platform-next",
            "--profile-dir",
            "profiles-local",
            "--revision",
            "format15",
            "--storage-version",
            "7",
            "--page-size",
            "1024",
            "--reserved",
            "3",
        ]);

        let Commands::Cf(CfArgs {
            command: CfCommands::Bootstrap(args),
        }) = cli.command
        else {
            panic!("expected cf bootstrap command");
        };
        assert_eq!(args.source_dir, PathBuf::from("source-tree"));
        assert_eq!(args.output, PathBuf::from("configuration.cf"));
        assert_eq!(args.source_version, InfobaseConfigSourceVersion::V2_21);
        assert_eq!(args.target_profile, "platform-next");
        assert_eq!(args.profile_dir, Some(PathBuf::from("profiles-local")));
        assert_eq!(args.revision, CfRevision::Format15);
        assert_eq!(args.storage_version, 7);
        assert_eq!(args.page_size, Some(1024));
        assert_eq!(args.reserved, 3);
    }

    #[test]
    fn parses_exact_cf_extract_with_explicit_compression() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "cf",
            "extract",
            "configuration.cf",
            "3ad08f4a-6202-4099-b6cc-bc116e6731a0",
            "task-evidence",
            "--profile",
            "storage:native-evidence",
            "--compression",
            "raw-deflate",
        ]);

        let Commands::Cf(CfArgs {
            command: CfCommands::Extract(args),
        }) = cli.command
        else {
            panic!("expected cf extract command");
        };
        assert_eq!(args.input, PathBuf::from("configuration.cf"));
        assert_eq!(args.element, "3ad08f4a-6202-4099-b6cc-bc116e6731a0");
        assert_eq!(args.output_dir, PathBuf::from("task-evidence"));
        assert_eq!(args.profile, "storage:native-evidence");
        assert_eq!(args.compression, CfCompression::RawDeflate);
    }

    #[test]
    fn parses_mssql_extension_list_command() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-extension-list",
            "--database",
            "BSP_Service",
            "--server",
            "localhost",
            "--sql-user",
            "sa",
            "--sql-pwd-env",
            "TEST_SQL_PASSWORD",
            "--format",
            "json",
        ]);

        let Commands::MssqlExtensionList(args) = cli.command else {
            panic!("expected mssql-extension-list command");
        };
        assert_eq!(args.database, "BSP_Service");
        assert_eq!(args.server, "localhost");
        assert_eq!(args.sql_user.as_deref(), Some("sa"));
        assert_eq!(args.sql_pwd_env, "TEST_SQL_PASSWORD");
        assert_eq!(args.format, MssqlExtensionListFormat::Json);
    }

    #[test]
    fn parses_single_extension_dump_and_all_extension_load() {
        let dump = Cli::parse_from([
            "ibcmd-rs",
            "mssql-dump-extension",
            "--database",
            "BSP_Service",
            "--extension",
            "_ДемоПустоеРасширение",
            "-o",
            r"C:\dump\empty",
        ]);
        let Commands::MssqlDumpExtension(dump) = dump.command else {
            panic!("expected mssql-dump-extension command");
        };
        assert_eq!(dump.extension.as_deref(), Some("_ДемоПустоеРасширение"));
        assert!(!dump.all_extensions);
        assert_eq!(dump.output_dir, PathBuf::from(r"C:\dump\empty"));

        let load = Cli::parse_from([
            "ibcmd-rs",
            "mssql-load-extension",
            "--platform-profile",
            "platform-8.3.27.1989",
            "--cluster-id",
            "11111111-1111-1111-1111-111111111111",
            "--infobase-id",
            "22222222-2222-2222-2222-222222222222",
            "--database",
            "extension_lab",
            "--all-extensions",
            "-i",
            r"C:\dump\extensions",
            "--replace-staging",
            "--allow-non-lab",
        ]);
        let Commands::MssqlLoadExtension(load) = load.command else {
            panic!("expected mssql-load-extension command");
        };
        assert!(load.extension.is_none());
        assert!(load.all_extensions);
        assert!(load.replace_staging);
        assert!(load.allow_non_lab);
        assert!(!load.sqlcmd_trust_cert);
        assert_eq!(
            load.platform_profile,
            MssqlNativePlatformProfile::Platform8_3_27_1989
        );
    }

    #[test]
    fn extension_selection_is_mutually_exclusive_and_required() {
        assert!(
            Cli::try_parse_from([
                "ibcmd-rs",
                "mssql-dump-extension",
                "--database",
                "BSP_Service",
                "-o",
                r"C:\dump",
            ])
            .is_err()
        );
        assert!(
            Cli::try_parse_from([
                "ibcmd-rs",
                "mssql-load-extension",
                "--platform-profile",
                "platform-8.3.27.1989",
                "--database",
                "extension_lab",
                "--extension",
                "One",
                "--all-extensions",
                "-i",
                r"C:\dump",
            ])
            .is_err()
        );
    }
    #[test]
    fn parses_mssql_apply_source_change_for_one_extension_body() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-apply-source-change",
            "--platform-profile",
            "platform-8.3.27.1989",
            "--cluster-id",
            "11111111-1111-1111-1111-111111111111",
            "--infobase-id",
            "22222222-2222-2222-2222-222222222222",
            "--server",
            "localhost",
            "--database",
            "extension_lab",
            "--source-root",
            r"C:\src\extension",
            "--path",
            "CommonModules/Tools/Ext/Module.bsl",
            "--extension",
            "Demo",
            "--mode",
            "online",
            "--dry-run",
            "--allow-non-lab",
            "--sqlcmd-trust-cert",
        ]);
        let Commands::MssqlApplySourceChange(args) = cli.command else {
            panic!("unexpected command");
        };
        assert_eq!(args.database, "extension_lab");
        assert_eq!(args.source_root, PathBuf::from(r"C:\src\extension"));
        assert_eq!(
            args.source_path,
            PathBuf::from("CommonModules/Tools/Ext/Module.bsl")
        );
        assert_eq!(args.extension.as_deref(), Some("Demo"));
        assert_eq!(args.mode, MssqlMainActivationModeArg::Online);
        assert!(args.dry_run);
        assert!(args.allow_non_lab);
        assert!(args.sqlcmd_trust_cert);
    }

    #[test]
    fn parses_mssql_live_source_change_with_tail_log_artifact() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-apply-source-change",
            "--platform-profile",
            "platform-8.3.27.1989",
            "--cluster-id",
            "11111111-1111-1111-1111-111111111111",
            "--infobase-id",
            "22222222-2222-2222-2222-222222222222",
            "--database",
            "main_lab",
            "--source-root",
            r"C:\src\main",
            "--path",
            "CommonModules/Tools/Ext/Module.bsl",
            "--mode",
            "live",
            "--tail-log-output",
            r"C:\sql-backups\main-live.trn",
            "--allow-non-lab",
            "--sqlcmd-trust-cert",
        ]);
        let Commands::MssqlApplySourceChange(args) = cli.command else {
            panic!("unexpected command");
        };
        assert_eq!(args.mode, MssqlMainActivationModeArg::Live);
        assert_eq!(
            args.tail_log_output,
            Some(PathBuf::from(r"C:\sql-backups\main-live.trn"))
        );
        // the interruption of open work is the operator's word (#409 F-10): off unless given
        assert!(!args.interrupt_sessions);
    }

    #[test]
    fn parses_the_acceptance_that_the_live_switch_interrupts_sessions() {
        let common = [
            "ibcmd-rs",
            "mssql-activate-staged-main",
            "--platform-profile",
            "platform-8.3.27.2214",
            "--cluster-id",
            "11111111-1111-1111-1111-111111111111",
            "--infobase-id",
            "22222222-2222-2222-2222-222222222222",
            "--database",
            "main_lab",
            "--mode",
            "live",
            "--tail-log-output",
            r"C:\sql-backups\main-live.trn",
            "--allow-non-lab",
        ];
        let plain = Cli::parse_from(common);
        let Commands::MssqlActivateStagedMain(args) = plain.command else {
            panic!("unexpected command");
        };
        assert!(!args.interrupt_sessions);
        let accepted = Cli::parse_from(common.iter().copied().chain(["--interrupt-sessions"]));
        let Commands::MssqlActivateStagedMain(args) = accepted.command else {
            panic!("unexpected command");
        };
        assert!(args.interrupt_sessions);
    }

    #[test]
    fn parses_worker_editor_watch_contract() {
        let cli = Cli::parse_from([
            "ibcmd-rs",
            "mssql-apply-source-change",
            "--platform-profile",
            "platform-8.3.27.1989",
            "--database",
            "main_lab",
            "--source-root",
            r"C:\src\main",
            "--path",
            "CommonForms/Demo/Ext/Form/Module.bsl",
            "--mode",
            "worker",
            "--watch",
            "--watch-debounce-ms",
            "250",
            "--ras-endpoint",
            "localhost:2545",
            "--cluster-id",
            "24c580ef-d5de-4b78-b204-b94b64eb2fae",
            "--infobase-id",
            "aab1bddd-3840-4899-bcc5-625500ebb115",
            "--allow-non-lab",
            "--sqlcmd-trust-cert",
        ]);
        let Commands::MssqlApplySourceChange(args) = cli.command else {
            panic!("unexpected command");
        };
        assert_eq!(args.mode, MssqlMainActivationModeArg::Worker);
        assert!(args.watch);
        assert_eq!(args.watch_debounce_ms, 250);
        assert_eq!(args.ras_endpoint, "localhost:2545");
        assert!(args.cluster_id.is_some());
        assert!(args.infobase_id.is_some());
    }

    #[test]
    fn mssql_write_commands_require_a_closed_platform_profile() {
        let missing = Cli::try_parse_from([
            "ibcmd-rs",
            "mssql-activate-staged-main",
            "--database",
            "main_lab",
            "--mode",
            "exclusive",
            "--allow-non-lab",
        ]);
        assert!(missing.is_err());

        let unknown = Cli::try_parse_from([
            "ibcmd-rs",
            "mssql-activate-staged-main",
            "--platform-profile",
            "platform-8.5.2.9999",
            "--database",
            "main_lab",
            "--mode",
            "exclusive",
            "--allow-non-lab",
        ]);
        assert!(unknown.is_err());

        let known_read_only = Cli::parse_from([
            "ibcmd-rs",
            "mssql-activate-staged-main",
            "--platform-profile",
            "platform-8.5.1.1150",
            "--cluster-id",
            "11111111-1111-1111-1111-111111111111",
            "--infobase-id",
            "22222222-2222-2222-2222-222222222222",
            "--database",
            "main_lab",
            "--mode",
            "exclusive",
            "--allow-non-lab",
        ]);
        let Commands::MssqlActivateStagedMain(args) = known_read_only.command else {
            panic!("expected mssql-activate-staged-main command");
        };
        assert_eq!(
            args.platform_profile,
            MssqlNativePlatformProfile::Platform8_5_1_1150
        );
    }
}
