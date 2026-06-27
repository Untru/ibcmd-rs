use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, anyhow};
use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::cli::{
    MssqlCloneArgs, MssqlCompareArgs, MssqlDeltaExportArgs, MssqlDeltaImportArgs,
    MssqlStageAccountingRegisterObjectArgs, MssqlStageAccumulationRegisterObjectArgs,
    MssqlStageBotObjectArgs, MssqlStageBusinessProcessObjectArgs,
    MssqlStageCalculationRegisterObjectArgs, MssqlStageCatalogObjectArgs,
    MssqlStageChartOfAccountsObjectArgs, MssqlStageChartOfCalculationRegistersObjectArgs,
    MssqlStageChartOfCalculationTypesObjectArgs, MssqlStageChartOfCharacteristicTypesObjectArgs,
    MssqlStageCommandGroupObjectArgs, MssqlStageCommonAttributeObjectArgs,
    MssqlStageCommonCommandObjectArgs, MssqlStageCommonFormObjectArgs, MssqlStageCommonModuleArgs,
    MssqlStageCommonModuleMetadataArgs, MssqlStageCommonModuleObjectArgs,
    MssqlStageCommonModuleObjectsArgs, MssqlStageCommonModulesArgs,
    MssqlStageCommonPictureObjectArgs, MssqlStageCommonTemplateObjectArgs,
    MssqlStageConstantObjectArgs, MssqlStageDataProcessorObjectArgs,
    MssqlStageDefinedTypeObjectArgs, MssqlStageDocumentJournalObjectArgs,
    MssqlStageDocumentNumeratorObjectArgs, MssqlStageDocumentObjectArgs, MssqlStageEnumObjectArgs,
    MssqlStageEventSubscriptionObjectArgs, MssqlStageExchangePlanObjectArgs,
    MssqlStageFilterCriteriaObjectArgs, MssqlStageFunctionalOptionObjectArgs,
    MssqlStageFunctionalOptionsParameterObjectArgs, MssqlStageHTTPServiceObjectArgs,
    MssqlStageInformationRegisterObjectArgs, MssqlStageIntegrationServiceObjectArgs,
    MssqlStageLanguageObjectArgs, MssqlStageMetadataObjectsArgs, MssqlStageReportObjectArgs,
    MssqlStageRoleObjectArgs, MssqlStageScheduledJobObjectArgs, MssqlStageSequenceObjectArgs,
    MssqlStageSessionParameterObjectArgs, MssqlStageSettingsStorageObjectArgs,
    MssqlStageSourceCommonModuleObjectsArgs, MssqlStageSourceMetadataObjectsArgs,
    MssqlStageSourceObjectsArgs, MssqlStageStyleItemObjectArgs, MssqlStageStyleObjectArgs,
    MssqlStageSubsystemObjectArgs, MssqlStageTaskObjectArgs, MssqlStageWSReferenceObjectArgs,
    MssqlStageWebServiceObjectArgs, MssqlStageXdtopackageObjectArgs, MssqlStorageExportArgs,
    MssqlStorageImportArgs,
};
use crate::module_blob::{
    CommonModuleXmlProperties, MetadataSourceContext, SimpleMetadataXmlProperties,
    VersionReplacement, hex_sha256, pack_base64_payload_blob_from_bytes,
    pack_business_process_flowchart_blob_from_xml, pack_command_interface_blob_from_xml,
    pack_common_module_metadata_blob_from_xml, pack_exchange_plan_content_blob_from_xml,
    pack_ext_picture_blob_from_bytes, pack_form_body_blob_from_module_text,
    pack_help_blob_from_parts, pack_module_blob_bytes,
    pack_moxel_spreadsheet_blob_from_xml_with_source, pack_predefined_data_blob_from_xml,
    pack_raw_deflated_blob_from_bytes, pack_role_rights_blob_from_xml, pack_schedule_blob_from_xml,
    pack_simple_metadata_blob_from_xml_with_source, pack_style_body_blob_from_xml,
    parse_common_module_xml_properties, parse_ext_picture_file_name_from_xml,
    parse_help_pages_from_xml, parse_simple_metadata_xml_properties, parse_template_type_from_xml,
    patch_versions_blob_bytes,
};
use crate::parallel;
use crate::source::scan_sources;

#[derive(Debug, Serialize, Deserialize)]
pub struct MssqlCompareReport {
    pub left: String,
    pub right: String,
    pub same: bool,
    pub summary: MssqlCompareSummary,
    pub differences: Vec<MssqlDifference>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct MssqlCompareSummary {
    pub left_tables: usize,
    pub right_tables: usize,
    pub missing_in_left: usize,
    pub missing_in_right: usize,
    pub row_count_differences: usize,
    pub column_differences: usize,
    pub checksum_differences: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MssqlDifference {
    pub kind: String,
    pub table: String,
    pub detail: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MssqlCloneReport {
    pub source: String,
    pub target: String,
    pub backup: PathBuf,
    pub restored: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StorageBundleManifest {
    pub source_database: Option<String>,
    pub format: String,
    pub tables: Vec<StorageTableManifest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageTableManifest {
    pub table_name: String,
    #[serde(default)]
    pub file_name: String,
    pub row_count: i64,
    pub binary_bytes: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub row_checksum: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StorageBundleExportReport {
    pub database: String,
    pub output_dir: PathBuf,
    pub manifest: StorageBundleManifest,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StorageBundleImportReport {
    pub database: String,
    pub input_dir: PathBuf,
    pub before: Vec<StorageTableManifest>,
    pub after: Vec<StorageTableManifest>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeltaBundleManifest {
    pub source_database: Option<String>,
    pub format: String,
    pub table: StorageTableManifest,
    pub rows: Vec<ConfigSaveRowDigest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSaveRowDigest {
    pub file_name: String,
    pub part_no: i32,
    pub data_size: i64,
    pub binary_bytes: i64,
    pub sha256: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeltaBundleExportReport {
    pub database: String,
    pub output_dir: PathBuf,
    pub manifest: DeltaBundleManifest,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeltaBundleImportReport {
    pub database: String,
    pub input_dir: PathBuf,
    pub manifest_rows: i64,
    pub before: StorageTableManifest,
    pub after: StorageTableManifest,
}

#[derive(Debug, Serialize)]
pub struct StageCommonModuleReport {
    pub database: String,
    pub module_id: String,
    pub module_body_id: String,
    pub text: PathBuf,
    pub script: PathBuf,
    pub before: StorageTableManifest,
    pub after: StorageTableManifest,
    pub module_blob: GeneratedBlobReport,
    pub versions_blob: GeneratedBlobReport,
    pub version_replacements: Vec<VersionReplacement>,
}

#[derive(Debug, Serialize)]
pub struct StageCommonModulesReport {
    pub database: String,
    pub modules: Vec<StagedCommonModuleReport>,
    pub script: PathBuf,
    pub before: StorageTableManifest,
    pub after: StorageTableManifest,
    pub versions_blob: GeneratedBlobReport,
    pub version_replacements: Vec<VersionReplacement>,
}

#[derive(Debug, Serialize)]
pub struct StageCommonModuleMetadataReport {
    pub database: String,
    pub module_id: String,
    pub xml: PathBuf,
    pub properties: CommonModuleXmlProperties,
    pub script: PathBuf,
    pub before: StorageTableManifest,
    pub after: StorageTableManifest,
    pub metadata_plain_bytes: usize,
    pub metadata_blob: GeneratedBlobReport,
    pub versions_blob: GeneratedBlobReport,
    pub version_replacements: Vec<VersionReplacement>,
}

#[derive(Debug, Serialize)]
pub struct StageCommonModuleObjectReport {
    pub database: String,
    pub module_id: String,
    pub module_body_id: String,
    pub xml: PathBuf,
    pub text: PathBuf,
    pub properties: CommonModuleXmlProperties,
    pub script: PathBuf,
    pub before: StorageTableManifest,
    pub after: StorageTableManifest,
    pub metadata_plain_bytes: usize,
    pub metadata_blob: GeneratedBlobReport,
    pub text_bytes: usize,
    pub module_blob: GeneratedBlobReport,
    pub versions_blob: GeneratedBlobReport,
    pub version_replacements: Vec<VersionReplacement>,
}

#[derive(Debug, Serialize)]
pub struct StageCommonModuleObjectsReport {
    pub database: String,
    pub modules: Vec<StagedCommonModuleObjectReport>,
    pub script: PathBuf,
    pub before: StorageTableManifest,
    pub after: StorageTableManifest,
    pub versions_blob: GeneratedBlobReport,
    pub version_replacements: Vec<VersionReplacement>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StagedCommonModuleObjectReport {
    pub module_id: String,
    pub module_body_id: String,
    pub xml: PathBuf,
    pub text: PathBuf,
    pub properties: CommonModuleXmlProperties,
    pub metadata_plain_bytes: usize,
    pub metadata_blob: GeneratedBlobReport,
    pub text_bytes: usize,
    pub module_blob: GeneratedBlobReport,
}

#[derive(Debug, Clone, Serialize)]
pub struct StagedCommonModuleReport {
    pub module_id: String,
    pub module_body_id: String,
    pub text: PathBuf,
    pub text_bytes: usize,
    pub module_blob: GeneratedBlobReport,
}

#[derive(Debug, Serialize)]
pub struct StageSourceObjectsReport {
    pub database: String,
    pub metadata_objects: Vec<StagedMetadataObjectReport>,
    pub common_modules: Vec<StagedCommonModuleObjectReport>,
    pub scripts: Vec<PathBuf>,
    pub script: PathBuf,
    pub before: StorageTableManifest,
    pub after: StorageTableManifest,
    pub versions_blob: GeneratedBlobReport,
    pub version_replacements: Vec<VersionReplacement>,
}

#[derive(Debug, Serialize)]
pub struct StageMetadataObjectsReport {
    pub database: String,
    pub objects: Vec<StagedMetadataObjectReport>,
    pub script: PathBuf,
    pub before: StorageTableManifest,
    pub after: StorageTableManifest,
    pub versions_blob: GeneratedBlobReport,
    pub version_replacements: Vec<VersionReplacement>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StagedMetadataObjectReport {
    pub object_id: String,
    pub kind: String,
    pub xml: PathBuf,
    pub properties: SimpleMetadataXmlProperties,
    pub metadata_plain_bytes: usize,
    pub metadata_blob: GeneratedBlobReport,
    pub body_rows: Vec<StagedMetadataBodyReport>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StagedMetadataBodyReport {
    pub body_id: String,
    pub path: PathBuf,
    pub blob: GeneratedBlobReport,
}

#[derive(Debug, Clone, Serialize)]
pub struct GeneratedBlobReport {
    pub bytes: usize,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TableShape {
    table_name: String,
    row_count: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    row_checksum: Option<i64>,
    #[serde(default)]
    columns: Vec<ColumnShape>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
struct ColumnShape {
    name: String,
    type_name: String,
    max_length: i16,
    precision: u8,
    scale: u8,
    is_nullable: bool,
}

#[derive(Debug, Deserialize)]
struct DatabaseFile {
    name: String,
    type_desc: String,
    physical_name: String,
}

#[derive(Debug, Deserialize)]
struct BinaryBlobRow {
    file_name: String,
    data_size: i64,
    binary_hex: String,
}

#[derive(Debug, Clone)]
struct CommonModuleStageSpec {
    module_id: String,
    text: PathBuf,
}

#[derive(Debug, Clone)]
struct PreparedCommonModuleStage {
    spec: CommonModuleStageSpec,
    module_body_id: String,
    text_bytes: usize,
    blob: Vec<u8>,
    blob_sha256: String,
}

#[derive(Debug, Clone)]
struct PreparedCommonModuleObjectStage {
    module_id: String,
    module_body_id: String,
    xml: PathBuf,
    text: PathBuf,
    properties: CommonModuleXmlProperties,
    metadata_plain_bytes: usize,
    metadata_blob: Vec<u8>,
    metadata_blob_sha256: String,
    text_bytes: usize,
    module_blob: Vec<u8>,
    module_blob_sha256: String,
}

#[derive(Debug, Clone)]
struct PreparedMetadataObjectStage {
    object_id: String,
    kind: String,
    xml: PathBuf,
    properties: SimpleMetadataXmlProperties,
    metadata_plain_bytes: usize,
    metadata_blob: Vec<u8>,
    metadata_blob_sha256: String,
    body_rows: Vec<PreparedMetadataBodyStage>,
}

#[derive(Debug, Clone)]
struct PreparedMetadataBodyStage {
    body_id: String,
    path: PathBuf,
    blob: Vec<u8>,
    blob_sha256: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct NestedCommandModuleSource {
    command_id: String,
    command_name: String,
    body_path: PathBuf,
}

fn staged_metadata_object_report(
    object: PreparedMetadataObjectStage,
) -> StagedMetadataObjectReport {
    StagedMetadataObjectReport {
        object_id: object.object_id,
        kind: object.kind,
        xml: object.xml,
        properties: object.properties,
        metadata_plain_bytes: object.metadata_plain_bytes,
        metadata_blob: GeneratedBlobReport {
            bytes: object.metadata_blob.len(),
            sha256: object.metadata_blob_sha256,
        },
        body_rows: object
            .body_rows
            .into_iter()
            .map(|body| StagedMetadataBodyReport {
                body_id: body.body_id,
                path: body.path,
                blob: GeneratedBlobReport {
                    bytes: body.blob.len(),
                    sha256: body.blob_sha256,
                },
            })
            .collect(),
    }
}

pub fn compare_databases(args: &MssqlCompareArgs) -> Result<MssqlCompareReport> {
    let left = load_table_shapes(&args.sqlcmd, &args.server, &args.left)?;
    let right = load_table_shapes(&args.sqlcmd, &args.server, &args.right)?;
    Ok(compare_shapes(&args.left, &args.right, &left, &right))
}

fn require_non_lab_confirmation(allowed: bool, action: &str) -> Result<()> {
    if allowed {
        Ok(())
    } else {
        Err(anyhow!(
            "{action} is gated for non-lab runs; pass --allow-non-lab to continue"
        ))
    }
}

pub fn write_compare_report(report: &MssqlCompareReport, output: &Path) -> Result<()> {
    let json = serde_json::to_string_pretty(report)?;
    fs::write(output, json).with_context(|| format!("failed to write {}", output.display()))
}

pub fn clone_database(args: &MssqlCloneArgs) -> Result<MssqlCloneReport> {
    require_non_lab_confirmation(args.allow_non_lab, "database clone")?;
    let backup = args.backup.clone().unwrap_or_else(|| {
        PathBuf::from(format!(
            r"C:\temp\ibcmd-rs\{}_to_{}.bak",
            args.source, args.target
        ))
    });
    if let Some(parent) = backup.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    if database_exists(&args.sqlcmd, &args.server, &args.target)? {
        if !args.overwrite {
            return Err(anyhow!(
                "target database {} already exists; pass --overwrite to replace it",
                args.target
            ));
        }
        let drop_sql = format!(
            "ALTER DATABASE {target} SET SINGLE_USER WITH ROLLBACK IMMEDIATE; DROP DATABASE {target};",
            target = quote_ident(&args.target)
        );
        run_sql(&args.sqlcmd, &args.server, &drop_sql)?;
    }

    let files = load_database_files(&args.sqlcmd, &args.server, &args.source)?;
    let data_file = files
        .iter()
        .find(|file| file.type_desc.eq_ignore_ascii_case("ROWS"))
        .ok_or_else(|| anyhow!("source database has no ROWS file"))?;
    let log_file = files
        .iter()
        .find(|file| file.type_desc.eq_ignore_ascii_case("LOG"))
        .ok_or_else(|| anyhow!("source database has no LOG file"))?;

    let data_target = sibling_path(&data_file.physical_name, &format!("{}.mdf", args.target))?;
    let log_target = sibling_path(&log_file.physical_name, &format!("{}_log.ldf", args.target))?;

    let sql = format!(
        "BACKUP DATABASE {source} TO DISK = N'{backup}' WITH INIT, COPY_ONLY, STATS = 10;\n\
         RESTORE DATABASE {target} FROM DISK = N'{backup}' WITH \
         MOVE N'{data_logical}' TO N'{data_target}', \
         MOVE N'{log_logical}' TO N'{log_target}', \
         REPLACE, RECOVERY, STATS = 10;",
        source = quote_ident(&args.source),
        target = quote_ident(&args.target),
        backup = quote_string_path(&backup),
        data_logical = quote_string(&data_file.name),
        log_logical = quote_string(&log_file.name),
        data_target = quote_string(&data_target),
        log_target = quote_string(&log_target),
    );
    run_sql(&args.sqlcmd, &args.server, &sql)?;

    Ok(MssqlCloneReport {
        source: args.source.clone(),
        target: args.target.clone(),
        backup,
        restored: true,
    })
}

pub fn export_storage_bundle(args: &MssqlStorageExportArgs) -> Result<StorageBundleExportReport> {
    fs::create_dir_all(&args.output_dir)
        .with_context(|| format!("failed to create {}", args.output_dir.display()))?;

    let mut tables = Vec::new();
    for table in storage_tables() {
        let target = args.output_dir.join(format!("{table}.bcp"));
        if target.exists() && !args.overwrite {
            return Err(anyhow!(
                "{} already exists; pass --overwrite to replace bundle files",
                target.display()
            ));
        }
        run_bcp_out(&args.bcp, &args.server, &args.database, table, &target)?;
        let stats = storage_table_stats(&args.sqlcmd, &args.server, &args.database, table)?;
        tables.push(StorageTableManifest {
            table_name: table.to_string(),
            file_name: target
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_string(),
            row_count: stats.row_count,
            binary_bytes: stats.binary_bytes,
            row_checksum: stats.row_checksum,
        });
    }

    let manifest = StorageBundleManifest {
        source_database: Some(args.database.clone()),
        format: "mssql-native-bcp-v1".to_string(),
        tables,
    };
    write_storage_manifest(&args.output_dir, &manifest)?;

    Ok(StorageBundleExportReport {
        database: args.database.clone(),
        output_dir: args.output_dir.clone(),
        manifest,
    })
}

pub fn import_storage_bundle(args: &MssqlStorageImportArgs) -> Result<StorageBundleImportReport> {
    if !args.replace {
        return Err(anyhow!(
            "storage import deletes Config, ConfigSave and Params rows; pass --replace"
        ));
    }
    require_non_lab_confirmation(args.allow_non_lab, "storage import")?;

    let manifest = read_storage_manifest(&args.input_dir)?;
    validate_storage_manifest(&manifest)?;

    let before = storage_tables()
        .iter()
        .map(|table| storage_table_stats(&args.sqlcmd, &args.server, &args.database, table))
        .collect::<Result<Vec<_>>>()?;

    let reset_sql = format!(
        "USE {db}; DELETE FROM ConfigSave; DELETE FROM Config; DELETE FROM Params;",
        db = quote_ident(&args.database)
    );
    run_sql(&args.sqlcmd, &args.server, &reset_sql)?;

    for table in storage_tables() {
        let file = args.input_dir.join(format!("{table}.bcp"));
        if !file.is_file() {
            return Err(anyhow!("bundle file not found: {}", file.display()));
        }
        run_bcp_in(&args.bcp, &args.server, &args.database, table, &file)?;
    }

    let after = storage_tables()
        .iter()
        .map(|table| storage_table_stats(&args.sqlcmd, &args.server, &args.database, table))
        .collect::<Result<Vec<_>>>()?;
    compare_storage_bundle_tables(&manifest.tables, &after)?;

    Ok(StorageBundleImportReport {
        database: args.database.clone(),
        input_dir: args.input_dir.clone(),
        before,
        after,
    })
}

pub fn export_delta_bundle(args: &MssqlDeltaExportArgs) -> Result<DeltaBundleExportReport> {
    fs::create_dir_all(&args.output_dir)
        .with_context(|| format!("failed to create {}", args.output_dir.display()))?;

    let target = args.output_dir.join("ConfigSave.bcp");
    if target.exists() && !args.overwrite {
        return Err(anyhow!(
            "{} already exists; pass --overwrite to replace bundle files",
            target.display()
        ));
    }

    run_bcp_out(
        &args.bcp,
        &args.server,
        &args.database,
        "ConfigSave",
        &target,
    )?;
    let table = storage_table_stats(&args.sqlcmd, &args.server, &args.database, "ConfigSave")?;
    let rows = configsave_row_digests(&args.sqlcmd, &args.server, &args.database)?;

    let manifest = DeltaBundleManifest {
        source_database: Some(args.database.clone()),
        format: "mssql-configsave-delta-v1".to_string(),
        table: StorageTableManifest {
            file_name: "ConfigSave.bcp".to_string(),
            ..table
        },
        rows,
    };
    write_delta_manifest(&args.output_dir, &manifest)?;

    Ok(DeltaBundleExportReport {
        database: args.database.clone(),
        output_dir: args.output_dir.clone(),
        manifest,
    })
}

pub fn import_delta_bundle(args: &MssqlDeltaImportArgs) -> Result<DeltaBundleImportReport> {
    require_non_lab_confirmation(args.allow_non_lab, "delta import")?;
    let manifest = read_delta_manifest(&args.input_dir)?;
    validate_delta_manifest(&manifest)?;

    let before = storage_table_stats(&args.sqlcmd, &args.server, &args.database, "ConfigSave")?;
    if before.row_count != 0 && !args.replace_config_save {
        return Err(anyhow!(
            "target ConfigSave has {} rows; pass --replace-config-save to delete them first",
            before.row_count
        ));
    }

    if args.replace_config_save {
        let reset_sql = format!(
            "USE {db}; DELETE FROM ConfigSave;",
            db = quote_ident(&args.database)
        );
        run_sql(&args.sqlcmd, &args.server, &reset_sql)?;
    }

    let file = args.input_dir.join("ConfigSave.bcp");
    if !file.is_file() {
        return Err(anyhow!("bundle file not found: {}", file.display()));
    }
    run_bcp_in(&args.bcp, &args.server, &args.database, "ConfigSave", &file)?;

    let after = storage_table_stats(&args.sqlcmd, &args.server, &args.database, "ConfigSave")?;
    compare_storage_table_manifests(&manifest.table, &after)?;

    Ok(DeltaBundleImportReport {
        database: args.database.clone(),
        input_dir: args.input_dir.clone(),
        manifest_rows: manifest.table.row_count,
        before,
        after,
    })
}

pub fn stage_common_module(args: &MssqlStageCommonModuleArgs) -> Result<StageCommonModuleReport> {
    require_non_lab_confirmation(args.allow_non_lab, "common module staging")?;
    let report = stage_common_module_specs(
        &args.sqlcmd,
        &args.server,
        &args.database,
        vec![CommonModuleStageSpec {
            module_id: args.module_id.clone(),
            text: args.text.clone(),
        }],
        args.replace_config_save,
        args.script_output.clone(),
    )?;
    let module = report
        .modules
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("stage report did not contain the requested module"))?;

    Ok(StageCommonModuleReport {
        database: report.database,
        module_id: module.module_id,
        module_body_id: module.module_body_id,
        text: module.text,
        script: report.script,
        before: report.before,
        after: report.after,
        module_blob: module.module_blob,
        versions_blob: report.versions_blob,
        version_replacements: report.version_replacements,
    })
}

pub fn stage_common_modules(
    args: &MssqlStageCommonModulesArgs,
) -> Result<StageCommonModulesReport> {
    require_non_lab_confirmation(args.allow_non_lab, "common module batch staging")?;
    let specs = parse_common_module_specs(&args.modules)?;
    stage_common_module_specs(
        &args.sqlcmd,
        &args.server,
        &args.database,
        specs,
        args.replace_config_save,
        args.script_output.clone(),
    )
}

pub fn stage_common_module_metadata(
    args: &MssqlStageCommonModuleMetadataArgs,
) -> Result<StageCommonModuleMetadataReport> {
    if !args.replace_config_save {
        return Err(anyhow!(
            "staging deletes existing ConfigSave rows; pass --replace-config-save"
        ));
    }
    require_non_lab_confirmation(args.allow_non_lab, "common module metadata staging")?;

    let module_id = normalize_uuid_arg(&args.module_id)?;
    let xml = fs::read(&args.xml)
        .with_context(|| format!("failed to read XML {}", args.xml.display()))?;
    let base_metadata_blob =
        fetch_config_blob(&args.sqlcmd, &args.server, &args.database, &module_id)?;
    let packed_metadata = pack_common_module_metadata_blob_from_xml(&base_metadata_blob, &xml)?;
    if packed_metadata.properties.uuid != module_id {
        return Err(anyhow!(
            "XML CommonModule uuid {} does not match --module-id {}",
            packed_metadata.properties.uuid,
            module_id
        ));
    }

    let versions_blob = fetch_config_blob(&args.sqlcmd, &args.server, &args.database, "versions")?;
    let patched_versions = patch_versions_blob_bytes(&versions_blob, &[module_id.clone()], true)?;

    let before = storage_table_stats(&args.sqlcmd, &args.server, &args.database, "ConfigSave")?;
    let script = args.script_output.clone().unwrap_or_else(|| {
        default_stage_script_path(
            &args.database,
            &format!("common_module_metadata_{module_id}"),
        )
    });
    if let Some(parent) = script.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let sql = build_stage_common_module_metadata_sql(
        &args.database,
        &module_id,
        &packed_metadata.blob,
        &patched_versions.blob,
    );
    fs::write(&script, sql).with_context(|| format!("failed to write {}", script.display()))?;
    run_sql_file(&args.sqlcmd, &args.server, &script)?;

    let after = storage_table_stats(&args.sqlcmd, &args.server, &args.database, "ConfigSave")?;

    Ok(StageCommonModuleMetadataReport {
        database: args.database.clone(),
        module_id,
        xml: args.xml.clone(),
        properties: packed_metadata.properties,
        script,
        before,
        after,
        metadata_plain_bytes: packed_metadata.plain_bytes,
        metadata_blob: GeneratedBlobReport {
            bytes: packed_metadata.blob.len(),
            sha256: packed_metadata.output_sha256,
        },
        versions_blob: GeneratedBlobReport {
            bytes: patched_versions.blob.len(),
            sha256: patched_versions.output_sha256,
        },
        version_replacements: patched_versions.replacements,
    })
}

pub fn stage_common_module_object(
    args: &MssqlStageCommonModuleObjectArgs,
) -> Result<StageCommonModuleObjectReport> {
    require_non_lab_confirmation(args.allow_non_lab, "common module object staging")?;
    let prepared = prepare_common_module_object_stage(
        &args.sqlcmd,
        &args.server,
        &args.database,
        args.xml.clone(),
        args.text.clone(),
    )?;
    if let Some(expected_module_id) = args
        .module_id
        .as_deref()
        .map(normalize_uuid_arg)
        .transpose()?
        .as_deref()
    {
        if expected_module_id != prepared.module_id {
            return Err(anyhow!(
                "XML CommonModule uuid {} does not match --module-id {}",
                prepared.module_id,
                expected_module_id
            ));
        }
    }

    let default_name = format!("common_module_object_{}", prepared.module_id);
    let report = stage_prepared_common_module_objects(
        &args.sqlcmd,
        &args.server,
        &args.database,
        vec![prepared],
        args.replace_config_save,
        args.script_output.clone(),
        &default_name,
    )?;
    let module = report
        .modules
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("stage report did not contain the requested module"))?;

    Ok(StageCommonModuleObjectReport {
        database: report.database,
        module_id: module.module_id,
        module_body_id: module.module_body_id,
        xml: module.xml,
        text: module.text,
        properties: module.properties,
        script: report.script,
        before: report.before,
        after: report.after,
        metadata_plain_bytes: module.metadata_plain_bytes,
        metadata_blob: module.metadata_blob,
        text_bytes: module.text_bytes,
        module_blob: module.module_blob,
        versions_blob: report.versions_blob,
        version_replacements: report.version_replacements,
    })
}

pub fn stage_common_module_objects(
    args: &MssqlStageCommonModuleObjectsArgs,
) -> Result<StageCommonModuleObjectsReport> {
    if args.xmls.is_empty() {
        return Err(anyhow!("at least one common module XML must be staged"));
    }
    require_non_lab_confirmation(args.allow_non_lab, "common module object batch staging")?;

    let prepared = parallel::install(|| {
        args.xmls
            .par_iter()
            .map(|xml| {
                prepare_common_module_object_stage(
                    &args.sqlcmd,
                    &args.server,
                    &args.database,
                    xml.clone(),
                    None,
                )
            })
            .collect::<Result<Vec<_>>>()
    })??;
    ensure_unique_common_module_object_ids(&prepared)?;

    stage_prepared_common_module_objects(
        &args.sqlcmd,
        &args.server,
        &args.database,
        prepared,
        args.replace_config_save,
        args.script_output.clone(),
        &format!("common_module_objects_{}", args.xmls.len()),
    )
}

pub fn stage_metadata_objects(
    args: &MssqlStageMetadataObjectsArgs,
) -> Result<StageMetadataObjectsReport> {
    if !args.replace_config_save {
        return Err(anyhow!(
            "staging deletes existing ConfigSave rows; pass --replace-config-save"
        ));
    }
    require_non_lab_confirmation(args.allow_non_lab, "metadata object staging")?;
    if args.xmls.is_empty() {
        return Err(anyhow!("at least one metadata XML must be staged"));
    }

    let source = args.source_root.clone().map(MetadataSourceContext::new);
    let prepared = parallel::install(|| {
        args.xmls
            .par_iter()
            .map(|xml| {
                prepare_metadata_object_stage(
                    &args.sqlcmd,
                    &args.server,
                    &args.database,
                    xml.clone(),
                    source.as_ref(),
                )
            })
            .collect::<Result<Vec<_>>>()
    })??;
    ensure_unique_metadata_object_ids(&prepared)?;

    let versions_blob = fetch_config_blob(&args.sqlcmd, &args.server, &args.database, "versions")?;
    let changes = prepared
        .iter()
        .flat_map(|object| {
            std::iter::once(object.object_id.clone())
                .chain(object.body_rows.iter().map(|body| body.body_id.clone()))
        })
        .collect::<Vec<_>>();
    let patched_versions = patch_versions_blob_bytes(&versions_blob, &changes, true)?;

    let before = storage_table_stats(&args.sqlcmd, &args.server, &args.database, "ConfigSave")?;
    let script = args.script_output.clone().unwrap_or_else(|| {
        default_stage_script_path(
            &args.database,
            &format!("metadata_objects_{}", prepared.len()),
        )
    });
    if let Some(parent) = script.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let sql = build_stage_metadata_objects_sql(&args.database, &prepared, &patched_versions.blob);
    fs::write(&script, sql).with_context(|| format!("failed to write {}", script.display()))?;
    run_sql_file(&args.sqlcmd, &args.server, &script)?;

    let after = storage_table_stats(&args.sqlcmd, &args.server, &args.database, "ConfigSave")?;
    let objects = prepared
        .into_iter()
        .map(staged_metadata_object_report)
        .collect();

    Ok(StageMetadataObjectsReport {
        database: args.database.clone(),
        objects,
        script,
        before,
        after,
        versions_blob: GeneratedBlobReport {
            bytes: patched_versions.blob.len(),
            sha256: patched_versions.output_sha256,
        },
        version_replacements: patched_versions.replacements,
    })
}

pub fn stage_source_metadata_objects(
    args: &MssqlStageSourceMetadataObjectsArgs,
) -> Result<StageMetadataObjectsReport> {
    require_non_lab_confirmation(args.allow_non_lab, "source metadata staging")?;
    let manifest = scan_sources(&args.source_root)?;
    let xmls = manifest
        .files
        .iter()
        .filter(|file| is_stage_metadata_xml(&file.path))
        .map(|file| args.source_root.join(&file.path))
        .collect::<Vec<_>>();
    let stage_args = MssqlStageMetadataObjectsArgs {
        server: args.server.clone(),
        database: args.database.clone(),
        xmls,
        source_root: Some(args.source_root.clone()),
        sqlcmd: args.sqlcmd.clone(),
        replace_config_save: args.replace_config_save,
        allow_non_lab: args.allow_non_lab,
        script_output: args.script_output.clone(),
    };
    stage_metadata_objects(&stage_args)
}

pub fn stage_source_common_module_objects(
    args: &MssqlStageSourceCommonModuleObjectsArgs,
) -> Result<StageCommonModuleObjectsReport> {
    require_non_lab_confirmation(args.allow_non_lab, "source common module staging")?;
    let manifest = scan_sources(&args.source_root)?;
    let xmls = manifest
        .files
        .iter()
        .filter(|file| is_root_common_module_xml(&file.path))
        .map(|file| args.source_root.join(&file.path))
        .collect::<Vec<_>>();
    let stage_args = MssqlStageCommonModuleObjectsArgs {
        server: args.server.clone(),
        database: args.database.clone(),
        xmls,
        sqlcmd: args.sqlcmd.clone(),
        replace_config_save: args.replace_config_save,
        allow_non_lab: args.allow_non_lab,
        script_output: args.script_output.clone(),
    };
    stage_common_module_objects(&stage_args)
}

pub fn stage_source_objects(
    args: &MssqlStageSourceObjectsArgs,
) -> Result<StageSourceObjectsReport> {
    require_non_lab_confirmation(args.allow_non_lab, "source tree staging")?;
    if !args.replace_config_save {
        return Err(anyhow!(
            "staging deletes existing ConfigSave rows; pass --replace-config-save"
        ));
    }

    let manifest = scan_sources(&args.source_root)?;
    let metadata_xmls = source_metadata_xmls(&manifest, &args.source_root);
    let common_module_xmls = source_common_module_xmls(&manifest, &args.source_root);
    if metadata_xmls.is_empty() && common_module_xmls.is_empty() {
        return Err(anyhow!(
            "no supported root XML objects or common modules found under {}",
            args.source_root.display()
        ));
    }

    let source = MetadataSourceContext::new(args.source_root.clone());
    let metadata_objects = parallel::install(|| {
        metadata_xmls
            .par_iter()
            .map(|xml| {
                prepare_metadata_object_stage(
                    &args.sqlcmd,
                    &args.server,
                    &args.database,
                    xml.clone(),
                    Some(&source),
                )
            })
            .collect::<Result<Vec<_>>>()
    })??;
    let metadata_object_count = metadata_objects.len();
    let common_modules = parallel::install(|| {
        common_module_xmls
            .par_iter()
            .map(|xml| {
                prepare_common_module_object_stage(
                    &args.sqlcmd,
                    &args.server,
                    &args.database,
                    xml.clone(),
                    None,
                )
            })
            .collect::<Result<Vec<_>>>()
    })??;
    let common_module_count = common_modules.len();
    ensure_unique_source_stage_ids(&metadata_objects, &common_modules)?;

    let versions_blob = fetch_config_blob(&args.sqlcmd, &args.server, &args.database, "versions")?;
    let changes = metadata_objects
        .iter()
        .flat_map(|object| {
            std::iter::once(object.object_id.clone())
                .chain(object.body_rows.iter().map(|body| body.body_id.clone()))
        })
        .chain(
            common_modules
                .iter()
                .flat_map(|module| [module.module_id.clone(), module.module_body_id.clone()]),
        )
        .collect::<Vec<_>>();
    let patched_versions = patch_versions_blob_bytes(&versions_blob, &changes, true)?;

    let batch_size = args.batch_size.unwrap_or(500).max(1);
    let batches =
        build_source_stage_batches(metadata_objects.clone(), common_modules.clone(), batch_size);
    let before = storage_table_stats(&args.sqlcmd, &args.server, &args.database, "ConfigSave")?;
    let mut scripts = Vec::with_capacity(batches.len());
    let mut running_rows = 0usize;
    let mut after = before.clone();

    for (index, batch) in batches.iter().enumerate() {
        let script = batch_stage_script_path(
            args.script_output.as_ref(),
            &args.database,
            "source_objects",
            index,
        );
        if let Some(parent) = script.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        let include_stable_rows = index == 0;
        let include_versions_row = index + 1 == batches.len();
        running_rows += batch.row_count;
        let expected_total_rows = running_rows
            + if include_stable_rows { 2 } else { 0 }
            + if include_versions_row { 1 } else { 0 };
        let sql = build_stage_source_objects_sql(
            &args.database,
            &batch.metadata_objects,
            &batch.common_modules,
            &patched_versions.blob,
            include_stable_rows,
            include_versions_row,
            expected_total_rows,
        );
        fs::write(&script, sql).with_context(|| format!("failed to write {}", script.display()))?;
        run_sql_file(&args.sqlcmd, &args.server, &script)?;
        after = storage_table_stats(&args.sqlcmd, &args.server, &args.database, "ConfigSave")?;
        scripts.push(script);
    }

    let script = scripts.last().cloned().unwrap_or_else(|| {
        args.script_output.clone().unwrap_or_else(|| {
            default_stage_script_path(
                &args.database,
                &format!(
                    "source_objects_{}_{}",
                    metadata_object_count, common_module_count
                ),
            )
        })
    });
    let metadata_objects = metadata_objects
        .into_iter()
        .map(staged_metadata_object_report)
        .collect();
    let common_modules = common_modules
        .into_iter()
        .map(|module| StagedCommonModuleObjectReport {
            module_id: module.module_id,
            module_body_id: module.module_body_id,
            xml: module.xml,
            text: module.text,
            properties: module.properties,
            metadata_plain_bytes: module.metadata_plain_bytes,
            metadata_blob: GeneratedBlobReport {
                bytes: module.metadata_blob.len(),
                sha256: module.metadata_blob_sha256,
            },
            text_bytes: module.text_bytes,
            module_blob: GeneratedBlobReport {
                bytes: module.module_blob.len(),
                sha256: module.module_blob_sha256,
            },
        })
        .collect();

    Ok(StageSourceObjectsReport {
        database: args.database.clone(),
        metadata_objects,
        common_modules,
        scripts,
        script,
        before,
        after,
        versions_blob: GeneratedBlobReport {
            bytes: patched_versions.blob.len(),
            sha256: patched_versions.output_sha256,
        },
        version_replacements: patched_versions.replacements,
    })
}

pub fn stage_exchange_plan_object(
    args: &MssqlStageExchangePlanObjectArgs,
) -> Result<StageMetadataObjectsReport> {
    let metadata_args = MssqlStageMetadataObjectsArgs {
        server: args.server.clone(),
        database: args.database.clone(),
        xmls: vec![args.xml.clone()],
        source_root: args.source_root.clone(),
        sqlcmd: args.sqlcmd.clone(),
        replace_config_save: args.replace_config_save,
        allow_non_lab: args.allow_non_lab,
        script_output: args.script_output.clone(),
    };
    stage_metadata_objects(&metadata_args)
}

fn is_root_metadata_xml(path: &str) -> bool {
    let lower = path.replace('\\', "/").to_ascii_lowercase();
    if !lower.ends_with(".xml") {
        return false;
    }
    let parts = lower.split('/').collect::<Vec<_>>();
    parts.len() == 2 && is_stage_root_metadata_collection(parts[0])
}

fn is_stage_root_metadata_collection(value: &str) -> bool {
    matches!(
        value,
        "catalogs"
            | "documents"
            | "informationregisters"
            | "accumulationregisters"
            | "accountingregisters"
            | "calculationregisters"
            | "chartsofcharacteristictypes"
            | "chartsofaccounts"
            | "chartsofcalculationtypes"
            | "chartsofcalculationregisters"
            | "commonforms"
            | "commonpictures"
            | "commontemplates"
            | "commonattributes"
            | "commandgroups"
            | "documentjournals"
            | "reports"
            | "dataprocessors"
            | "enums"
            | "exchangeplans"
            | "eventsubscriptions"
            | "filtercriteria"
            | "functionaloptions"
            | "functionaloptionsparameters"
            | "httpservices"
            | "languages"
            | "scheduledjobs"
            | "sessionparameters"
            | "settingsstorages"
            | "styleitems"
            | "styles"
            | "subsystems"
            | "roles"
            | "commoncommands"
            | "businessprocesses"
            | "bots"
            | "definedtypes"
            | "tasks"
            | "constants"
            | "documentnumerators"
            | "integrationservices"
            | "sequences"
            | "webservices"
            | "wsreferences"
            | "xdtopackages"
    )
}

fn is_template_metadata_xml(path: &str) -> bool {
    let lower = path.replace('\\', "/").to_ascii_lowercase();
    if !lower.ends_with(".xml") || lower.contains("/ext/") {
        return false;
    }
    let parts = lower.split('/').collect::<Vec<_>>();
    parts.len() >= 4 && parts[parts.len() - 2] == "templates"
}

fn is_form_metadata_xml(path: &str) -> bool {
    let lower = path.replace('\\', "/").to_ascii_lowercase();
    if !lower.ends_with(".xml") || lower.contains("/ext/") {
        return false;
    }
    let parts = lower.split('/').collect::<Vec<_>>();
    parts.len() >= 4 && parts[parts.len() - 2] == "forms"
}

fn is_stage_metadata_xml(path: &str) -> bool {
    is_configuration_metadata_xml(path)
        || is_root_metadata_xml(path)
        || is_template_metadata_xml(path)
        || is_form_metadata_xml(path)
}

fn is_configuration_metadata_xml(path: &str) -> bool {
    path.replace('\\', "/")
        .eq_ignore_ascii_case("Configuration.xml")
}

fn is_root_common_module_xml(path: &str) -> bool {
    let lower = path.replace('\\', "/").to_ascii_lowercase();
    if !lower.ends_with(".xml") {
        return false;
    }
    let parts = lower.split('/').collect::<Vec<_>>();
    parts.len() == 2 && parts[0] == "commonmodules"
}

pub fn stage_business_process_object(
    args: &MssqlStageBusinessProcessObjectArgs,
) -> Result<StageMetadataObjectsReport> {
    let metadata_args = MssqlStageMetadataObjectsArgs {
        server: args.server.clone(),
        database: args.database.clone(),
        xmls: vec![args.xml.clone()],
        source_root: args.source_root.clone(),
        sqlcmd: args.sqlcmd.clone(),
        replace_config_save: args.replace_config_save,
        allow_non_lab: args.allow_non_lab,
        script_output: args.script_output.clone(),
    };
    stage_metadata_objects(&metadata_args)
}

pub fn stage_document_journal_object(
    args: &MssqlStageDocumentJournalObjectArgs,
) -> Result<StageMetadataObjectsReport> {
    let metadata_args = MssqlStageMetadataObjectsArgs {
        server: args.server.clone(),
        database: args.database.clone(),
        xmls: vec![args.xml.clone()],
        source_root: args.source_root.clone(),
        sqlcmd: args.sqlcmd.clone(),
        replace_config_save: args.replace_config_save,
        allow_non_lab: args.allow_non_lab,
        script_output: args.script_output.clone(),
    };
    stage_metadata_objects(&metadata_args)
}

pub fn stage_report_object(
    args: &MssqlStageReportObjectArgs,
) -> Result<StageMetadataObjectsReport> {
    let metadata_args = MssqlStageMetadataObjectsArgs {
        server: args.server.clone(),
        database: args.database.clone(),
        xmls: vec![args.xml.clone()],
        source_root: args.source_root.clone(),
        sqlcmd: args.sqlcmd.clone(),
        replace_config_save: args.replace_config_save,
        allow_non_lab: args.allow_non_lab,
        script_output: args.script_output.clone(),
    };
    stage_metadata_objects(&metadata_args)
}

pub fn stage_data_processor_object(
    args: &MssqlStageDataProcessorObjectArgs,
) -> Result<StageMetadataObjectsReport> {
    let metadata_args = MssqlStageMetadataObjectsArgs {
        server: args.server.clone(),
        database: args.database.clone(),
        xmls: vec![args.xml.clone()],
        source_root: args.source_root.clone(),
        sqlcmd: args.sqlcmd.clone(),
        replace_config_save: args.replace_config_save,
        allow_non_lab: args.allow_non_lab,
        script_output: args.script_output.clone(),
    };
    stage_metadata_objects(&metadata_args)
}

pub fn stage_catalog_object(
    args: &MssqlStageCatalogObjectArgs,
) -> Result<StageMetadataObjectsReport> {
    let metadata_args = MssqlStageMetadataObjectsArgs {
        server: args.server.clone(),
        database: args.database.clone(),
        xmls: vec![args.xml.clone()],
        source_root: args.source_root.clone(),
        sqlcmd: args.sqlcmd.clone(),
        replace_config_save: args.replace_config_save,
        allow_non_lab: args.allow_non_lab,
        script_output: args.script_output.clone(),
    };
    stage_metadata_objects(&metadata_args)
}

pub fn stage_information_register_object(
    args: &MssqlStageInformationRegisterObjectArgs,
) -> Result<StageMetadataObjectsReport> {
    let metadata_args = MssqlStageMetadataObjectsArgs {
        server: args.server.clone(),
        database: args.database.clone(),
        xmls: vec![args.xml.clone()],
        source_root: args.source_root.clone(),
        sqlcmd: args.sqlcmd.clone(),
        replace_config_save: args.replace_config_save,
        allow_non_lab: args.allow_non_lab,
        script_output: args.script_output.clone(),
    };
    stage_metadata_objects(&metadata_args)
}

pub fn stage_scheduled_job_object(
    args: &MssqlStageScheduledJobObjectArgs,
) -> Result<StageMetadataObjectsReport> {
    let metadata_args = MssqlStageMetadataObjectsArgs {
        server: args.server.clone(),
        database: args.database.clone(),
        xmls: vec![args.xml.clone()],
        source_root: args.source_root.clone(),
        sqlcmd: args.sqlcmd.clone(),
        replace_config_save: args.replace_config_save,
        allow_non_lab: args.allow_non_lab,
        script_output: args.script_output.clone(),
    };
    stage_metadata_objects(&metadata_args)
}

pub fn stage_xdtopackage_object(
    args: &MssqlStageXdtopackageObjectArgs,
) -> Result<StageMetadataObjectsReport> {
    let metadata_args = MssqlStageMetadataObjectsArgs {
        server: args.server.clone(),
        database: args.database.clone(),
        xmls: vec![args.xml.clone()],
        source_root: args.source_root.clone(),
        sqlcmd: args.sqlcmd.clone(),
        replace_config_save: args.replace_config_save,
        allow_non_lab: args.allow_non_lab,
        script_output: args.script_output.clone(),
    };
    stage_metadata_objects(&metadata_args)
}

pub fn stage_role_object(args: &MssqlStageRoleObjectArgs) -> Result<StageMetadataObjectsReport> {
    let metadata_args = MssqlStageMetadataObjectsArgs {
        server: args.server.clone(),
        database: args.database.clone(),
        xmls: vec![args.xml.clone()],
        source_root: args.source_root.clone(),
        sqlcmd: args.sqlcmd.clone(),
        replace_config_save: args.replace_config_save,
        allow_non_lab: args.allow_non_lab,
        script_output: args.script_output.clone(),
    };
    stage_metadata_objects(&metadata_args)
}

pub fn stage_constant_object(
    args: &MssqlStageConstantObjectArgs,
) -> Result<StageMetadataObjectsReport> {
    let metadata_args = MssqlStageMetadataObjectsArgs {
        server: args.server.clone(),
        database: args.database.clone(),
        xmls: vec![args.xml.clone()],
        source_root: args.source_root.clone(),
        sqlcmd: args.sqlcmd.clone(),
        replace_config_save: args.replace_config_save,
        allow_non_lab: args.allow_non_lab,
        script_output: args.script_output.clone(),
    };
    stage_metadata_objects(&metadata_args)
}

pub fn stage_defined_type_object(
    args: &MssqlStageDefinedTypeObjectArgs,
) -> Result<StageMetadataObjectsReport> {
    let metadata_args = MssqlStageMetadataObjectsArgs {
        server: args.server.clone(),
        database: args.database.clone(),
        xmls: vec![args.xml.clone()],
        source_root: args.source_root.clone(),
        sqlcmd: args.sqlcmd.clone(),
        replace_config_save: args.replace_config_save,
        allow_non_lab: args.allow_non_lab,
        script_output: args.script_output.clone(),
    };
    stage_metadata_objects(&metadata_args)
}

pub fn stage_session_parameter_object(
    args: &MssqlStageSessionParameterObjectArgs,
) -> Result<StageMetadataObjectsReport> {
    let metadata_args = MssqlStageMetadataObjectsArgs {
        server: args.server.clone(),
        database: args.database.clone(),
        xmls: vec![args.xml.clone()],
        source_root: args.source_root.clone(),
        sqlcmd: args.sqlcmd.clone(),
        replace_config_save: args.replace_config_save,
        allow_non_lab: args.allow_non_lab,
        script_output: args.script_output.clone(),
    };
    stage_metadata_objects(&metadata_args)
}

pub fn stage_settings_storage_object(
    args: &MssqlStageSettingsStorageObjectArgs,
) -> Result<StageMetadataObjectsReport> {
    let metadata_args = MssqlStageMetadataObjectsArgs {
        server: args.server.clone(),
        database: args.database.clone(),
        xmls: vec![args.xml.clone()],
        source_root: args.source_root.clone(),
        sqlcmd: args.sqlcmd.clone(),
        replace_config_save: args.replace_config_save,
        allow_non_lab: args.allow_non_lab,
        script_output: args.script_output.clone(),
    };
    stage_metadata_objects(&metadata_args)
}

pub fn stage_functional_option_object(
    args: &MssqlStageFunctionalOptionObjectArgs,
) -> Result<StageMetadataObjectsReport> {
    let metadata_args = MssqlStageMetadataObjectsArgs {
        server: args.server.clone(),
        database: args.database.clone(),
        xmls: vec![args.xml.clone()],
        source_root: args.source_root.clone(),
        sqlcmd: args.sqlcmd.clone(),
        replace_config_save: args.replace_config_save,
        allow_non_lab: args.allow_non_lab,
        script_output: args.script_output.clone(),
    };
    stage_metadata_objects(&metadata_args)
}

pub fn stage_functional_options_parameter_object(
    args: &MssqlStageFunctionalOptionsParameterObjectArgs,
) -> Result<StageMetadataObjectsReport> {
    let metadata_args = MssqlStageMetadataObjectsArgs {
        server: args.server.clone(),
        database: args.database.clone(),
        xmls: vec![args.xml.clone()],
        source_root: args.source_root.clone(),
        sqlcmd: args.sqlcmd.clone(),
        replace_config_save: args.replace_config_save,
        allow_non_lab: args.allow_non_lab,
        script_output: args.script_output.clone(),
    };
    stage_metadata_objects(&metadata_args)
}

pub fn stage_event_subscription_object(
    args: &MssqlStageEventSubscriptionObjectArgs,
) -> Result<StageMetadataObjectsReport> {
    let metadata_args = MssqlStageMetadataObjectsArgs {
        server: args.server.clone(),
        database: args.database.clone(),
        xmls: vec![args.xml.clone()],
        source_root: args.source_root.clone(),
        sqlcmd: args.sqlcmd.clone(),
        replace_config_save: args.replace_config_save,
        allow_non_lab: args.allow_non_lab,
        script_output: args.script_output.clone(),
    };
    stage_metadata_objects(&metadata_args)
}

pub fn stage_http_service_object(
    args: &MssqlStageHTTPServiceObjectArgs,
) -> Result<StageMetadataObjectsReport> {
    let metadata_args = MssqlStageMetadataObjectsArgs {
        server: args.server.clone(),
        database: args.database.clone(),
        xmls: vec![args.xml.clone()],
        source_root: args.source_root.clone(),
        sqlcmd: args.sqlcmd.clone(),
        replace_config_save: args.replace_config_save,
        allow_non_lab: args.allow_non_lab,
        script_output: args.script_output.clone(),
    };
    stage_metadata_objects(&metadata_args)
}

pub fn stage_web_service_object(
    args: &MssqlStageWebServiceObjectArgs,
) -> Result<StageMetadataObjectsReport> {
    let metadata_args = MssqlStageMetadataObjectsArgs {
        server: args.server.clone(),
        database: args.database.clone(),
        xmls: vec![args.xml.clone()],
        source_root: args.source_root.clone(),
        sqlcmd: args.sqlcmd.clone(),
        replace_config_save: args.replace_config_save,
        allow_non_lab: args.allow_non_lab,
        script_output: args.script_output.clone(),
    };
    stage_metadata_objects(&metadata_args)
}

pub fn stage_common_attribute_object(
    args: &MssqlStageCommonAttributeObjectArgs,
) -> Result<StageMetadataObjectsReport> {
    let metadata_args = MssqlStageMetadataObjectsArgs {
        server: args.server.clone(),
        database: args.database.clone(),
        xmls: vec![args.xml.clone()],
        source_root: args.source_root.clone(),
        sqlcmd: args.sqlcmd.clone(),
        replace_config_save: args.replace_config_save,
        allow_non_lab: args.allow_non_lab,
        script_output: args.script_output.clone(),
    };
    stage_metadata_objects(&metadata_args)
}

pub fn stage_language_object(
    args: &MssqlStageLanguageObjectArgs,
) -> Result<StageMetadataObjectsReport> {
    let metadata_args = MssqlStageMetadataObjectsArgs {
        server: args.server.clone(),
        database: args.database.clone(),
        xmls: vec![args.xml.clone()],
        source_root: args.source_root.clone(),
        sqlcmd: args.sqlcmd.clone(),
        replace_config_save: args.replace_config_save,
        allow_non_lab: args.allow_non_lab,
        script_output: args.script_output.clone(),
    };
    stage_metadata_objects(&metadata_args)
}

pub fn stage_style_item_object(
    args: &MssqlStageStyleItemObjectArgs,
) -> Result<StageMetadataObjectsReport> {
    let metadata_args = MssqlStageMetadataObjectsArgs {
        server: args.server.clone(),
        database: args.database.clone(),
        xmls: vec![args.xml.clone()],
        source_root: args.source_root.clone(),
        sqlcmd: args.sqlcmd.clone(),
        replace_config_save: args.replace_config_save,
        allow_non_lab: args.allow_non_lab,
        script_output: args.script_output.clone(),
    };
    stage_metadata_objects(&metadata_args)
}

macro_rules! passthrough_metadata_stage {
    ($fn_name:ident, $args_ty:ty) => {
        pub fn $fn_name(args: &$args_ty) -> Result<StageMetadataObjectsReport> {
            let metadata_args = MssqlStageMetadataObjectsArgs {
                server: args.server.clone(),
                database: args.database.clone(),
                xmls: vec![args.xml.clone()],
                source_root: args.source_root.clone(),
                sqlcmd: args.sqlcmd.clone(),
                replace_config_save: args.replace_config_save,
                allow_non_lab: args.allow_non_lab,
                script_output: args.script_output.clone(),
            };
            stage_metadata_objects(&metadata_args)
        }
    };
}

passthrough_metadata_stage!(stage_style_object, MssqlStageStyleObjectArgs);
passthrough_metadata_stage!(stage_bot_object, MssqlStageBotObjectArgs);
passthrough_metadata_stage!(
    stage_document_numerator_object,
    MssqlStageDocumentNumeratorObjectArgs
);
passthrough_metadata_stage!(
    stage_integration_service_object,
    MssqlStageIntegrationServiceObjectArgs
);
passthrough_metadata_stage!(stage_sequence_object, MssqlStageSequenceObjectArgs);
passthrough_metadata_stage!(stage_ws_reference_object, MssqlStageWSReferenceObjectArgs);

pub fn stage_task_object(args: &MssqlStageTaskObjectArgs) -> Result<StageMetadataObjectsReport> {
    let metadata_args = MssqlStageMetadataObjectsArgs {
        server: args.server.clone(),
        database: args.database.clone(),
        xmls: vec![args.xml.clone()],
        source_root: args.source_root.clone(),
        sqlcmd: args.sqlcmd.clone(),
        replace_config_save: args.replace_config_save,
        allow_non_lab: args.allow_non_lab,
        script_output: args.script_output.clone(),
    };
    stage_metadata_objects(&metadata_args)
}

pub fn stage_subsystem_object(
    args: &MssqlStageSubsystemObjectArgs,
) -> Result<StageMetadataObjectsReport> {
    let metadata_args = MssqlStageMetadataObjectsArgs {
        server: args.server.clone(),
        database: args.database.clone(),
        xmls: vec![args.xml.clone()],
        source_root: args.source_root.clone(),
        sqlcmd: args.sqlcmd.clone(),
        replace_config_save: args.replace_config_save,
        allow_non_lab: args.allow_non_lab,
        script_output: args.script_output.clone(),
    };
    stage_metadata_objects(&metadata_args)
}

pub fn stage_command_group_object(
    args: &MssqlStageCommandGroupObjectArgs,
) -> Result<StageMetadataObjectsReport> {
    let metadata_args = MssqlStageMetadataObjectsArgs {
        server: args.server.clone(),
        database: args.database.clone(),
        xmls: vec![args.xml.clone()],
        source_root: args.source_root.clone(),
        sqlcmd: args.sqlcmd.clone(),
        replace_config_save: args.replace_config_save,
        allow_non_lab: args.allow_non_lab,
        script_output: args.script_output.clone(),
    };
    stage_metadata_objects(&metadata_args)
}

pub fn stage_enum_object(args: &MssqlStageEnumObjectArgs) -> Result<StageMetadataObjectsReport> {
    let metadata_args = MssqlStageMetadataObjectsArgs {
        server: args.server.clone(),
        database: args.database.clone(),
        xmls: vec![args.xml.clone()],
        source_root: args.source_root.clone(),
        sqlcmd: args.sqlcmd.clone(),
        replace_config_save: args.replace_config_save,
        allow_non_lab: args.allow_non_lab,
        script_output: args.script_output.clone(),
    };
    stage_metadata_objects(&metadata_args)
}

pub fn stage_document_object(
    args: &MssqlStageDocumentObjectArgs,
) -> Result<StageMetadataObjectsReport> {
    let metadata_args = MssqlStageMetadataObjectsArgs {
        server: args.server.clone(),
        database: args.database.clone(),
        xmls: vec![args.xml.clone()],
        source_root: args.source_root.clone(),
        sqlcmd: args.sqlcmd.clone(),
        replace_config_save: args.replace_config_save,
        allow_non_lab: args.allow_non_lab,
        script_output: args.script_output.clone(),
    };
    stage_metadata_objects(&metadata_args)
}

pub fn stage_filter_criteria_object(
    args: &MssqlStageFilterCriteriaObjectArgs,
) -> Result<StageMetadataObjectsReport> {
    let metadata_args = MssqlStageMetadataObjectsArgs {
        server: args.server.clone(),
        database: args.database.clone(),
        xmls: vec![args.xml.clone()],
        source_root: args.source_root.clone(),
        sqlcmd: args.sqlcmd.clone(),
        replace_config_save: args.replace_config_save,
        allow_non_lab: args.allow_non_lab,
        script_output: args.script_output.clone(),
    };
    stage_metadata_objects(&metadata_args)
}

pub fn stage_accounting_register_object(
    args: &MssqlStageAccountingRegisterObjectArgs,
) -> Result<StageMetadataObjectsReport> {
    let metadata_args = MssqlStageMetadataObjectsArgs {
        server: args.server.clone(),
        database: args.database.clone(),
        xmls: vec![args.xml.clone()],
        source_root: args.source_root.clone(),
        sqlcmd: args.sqlcmd.clone(),
        replace_config_save: args.replace_config_save,
        allow_non_lab: args.allow_non_lab,
        script_output: args.script_output.clone(),
    };
    stage_metadata_objects(&metadata_args)
}

pub fn stage_accumulation_register_object(
    args: &MssqlStageAccumulationRegisterObjectArgs,
) -> Result<StageMetadataObjectsReport> {
    let metadata_args = MssqlStageMetadataObjectsArgs {
        server: args.server.clone(),
        database: args.database.clone(),
        xmls: vec![args.xml.clone()],
        source_root: args.source_root.clone(),
        sqlcmd: args.sqlcmd.clone(),
        replace_config_save: args.replace_config_save,
        allow_non_lab: args.allow_non_lab,
        script_output: args.script_output.clone(),
    };
    stage_metadata_objects(&metadata_args)
}

pub fn stage_calculation_register_object(
    args: &MssqlStageCalculationRegisterObjectArgs,
) -> Result<StageMetadataObjectsReport> {
    let metadata_args = MssqlStageMetadataObjectsArgs {
        server: args.server.clone(),
        database: args.database.clone(),
        xmls: vec![args.xml.clone()],
        source_root: args.source_root.clone(),
        sqlcmd: args.sqlcmd.clone(),
        replace_config_save: args.replace_config_save,
        allow_non_lab: args.allow_non_lab,
        script_output: args.script_output.clone(),
    };
    stage_metadata_objects(&metadata_args)
}

pub fn stage_chart_of_characteristic_types_object(
    args: &MssqlStageChartOfCharacteristicTypesObjectArgs,
) -> Result<StageMetadataObjectsReport> {
    let metadata_args = MssqlStageMetadataObjectsArgs {
        server: args.server.clone(),
        database: args.database.clone(),
        xmls: vec![args.xml.clone()],
        source_root: args.source_root.clone(),
        sqlcmd: args.sqlcmd.clone(),
        replace_config_save: args.replace_config_save,
        allow_non_lab: args.allow_non_lab,
        script_output: args.script_output.clone(),
    };
    stage_metadata_objects(&metadata_args)
}

pub fn stage_chart_of_accounts_object(
    args: &MssqlStageChartOfAccountsObjectArgs,
) -> Result<StageMetadataObjectsReport> {
    let metadata_args = MssqlStageMetadataObjectsArgs {
        server: args.server.clone(),
        database: args.database.clone(),
        xmls: vec![args.xml.clone()],
        source_root: args.source_root.clone(),
        sqlcmd: args.sqlcmd.clone(),
        replace_config_save: args.replace_config_save,
        allow_non_lab: args.allow_non_lab,
        script_output: args.script_output.clone(),
    };
    stage_metadata_objects(&metadata_args)
}

pub fn stage_chart_of_calculation_types_object(
    args: &MssqlStageChartOfCalculationTypesObjectArgs,
) -> Result<StageMetadataObjectsReport> {
    let metadata_args = MssqlStageMetadataObjectsArgs {
        server: args.server.clone(),
        database: args.database.clone(),
        xmls: vec![args.xml.clone()],
        source_root: args.source_root.clone(),
        sqlcmd: args.sqlcmd.clone(),
        replace_config_save: args.replace_config_save,
        allow_non_lab: args.allow_non_lab,
        script_output: args.script_output.clone(),
    };
    stage_metadata_objects(&metadata_args)
}

pub fn stage_chart_of_calculation_registers_object(
    args: &MssqlStageChartOfCalculationRegistersObjectArgs,
) -> Result<StageMetadataObjectsReport> {
    let metadata_args = MssqlStageMetadataObjectsArgs {
        server: args.server.clone(),
        database: args.database.clone(),
        xmls: vec![args.xml.clone()],
        source_root: args.source_root.clone(),
        sqlcmd: args.sqlcmd.clone(),
        replace_config_save: args.replace_config_save,
        allow_non_lab: args.allow_non_lab,
        script_output: args.script_output.clone(),
    };
    stage_metadata_objects(&metadata_args)
}

pub fn stage_common_command_object(
    args: &MssqlStageCommonCommandObjectArgs,
) -> Result<StageMetadataObjectsReport> {
    let metadata_args = MssqlStageMetadataObjectsArgs {
        server: args.server.clone(),
        database: args.database.clone(),
        xmls: vec![args.xml.clone()],
        source_root: args.source_root.clone(),
        sqlcmd: args.sqlcmd.clone(),
        replace_config_save: args.replace_config_save,
        allow_non_lab: args.allow_non_lab,
        script_output: args.script_output.clone(),
    };
    stage_metadata_objects(&metadata_args)
}

pub fn stage_common_form_object(
    args: &MssqlStageCommonFormObjectArgs,
) -> Result<StageMetadataObjectsReport> {
    let metadata_args = MssqlStageMetadataObjectsArgs {
        server: args.server.clone(),
        database: args.database.clone(),
        xmls: vec![args.xml.clone()],
        source_root: args.source_root.clone(),
        sqlcmd: args.sqlcmd.clone(),
        replace_config_save: args.replace_config_save,
        allow_non_lab: args.allow_non_lab,
        script_output: args.script_output.clone(),
    };
    stage_metadata_objects(&metadata_args)
}

pub fn stage_common_picture_object(
    args: &MssqlStageCommonPictureObjectArgs,
) -> Result<StageMetadataObjectsReport> {
    let metadata_args = MssqlStageMetadataObjectsArgs {
        server: args.server.clone(),
        database: args.database.clone(),
        xmls: vec![args.xml.clone()],
        source_root: args.source_root.clone(),
        sqlcmd: args.sqlcmd.clone(),
        replace_config_save: args.replace_config_save,
        allow_non_lab: args.allow_non_lab,
        script_output: args.script_output.clone(),
    };
    stage_metadata_objects(&metadata_args)
}

pub fn stage_common_template_object(
    args: &MssqlStageCommonTemplateObjectArgs,
) -> Result<StageMetadataObjectsReport> {
    let metadata_args = MssqlStageMetadataObjectsArgs {
        server: args.server.clone(),
        database: args.database.clone(),
        xmls: vec![args.xml.clone()],
        source_root: args.source_root.clone(),
        sqlcmd: args.sqlcmd.clone(),
        replace_config_save: args.replace_config_save,
        allow_non_lab: args.allow_non_lab,
        script_output: args.script_output.clone(),
    };
    stage_metadata_objects(&metadata_args)
}

fn prepare_metadata_object_stage(
    sqlcmd: &Path,
    server: &str,
    database: &str,
    xml_path: PathBuf,
    source: Option<&MetadataSourceContext>,
) -> Result<PreparedMetadataObjectStage> {
    let xml = fs::read(&xml_path)
        .with_context(|| format!("failed to read XML {}", xml_path.display()))?;
    let properties = parse_simple_metadata_xml_properties(&xml)?;
    let object_id = properties.uuid.clone();
    let base_metadata_blob = fetch_config_blob(sqlcmd, server, database, &object_id)?;
    let packed_metadata =
        pack_simple_metadata_blob_from_xml_with_source(&base_metadata_blob, &xml, source)?;
    if packed_metadata.properties.uuid != object_id {
        return Err(anyhow!(
            "XML metadata uuid {} changed while packing {}",
            packed_metadata.properties.uuid,
            object_id
        ));
    }
    let body_rows = prepare_metadata_body_rows(
        sqlcmd,
        server,
        database,
        &xml_path,
        &xml,
        &packed_metadata.properties,
        source,
    )?;

    Ok(PreparedMetadataObjectStage {
        object_id,
        kind: packed_metadata.properties.kind.clone(),
        xml: xml_path,
        properties: packed_metadata.properties,
        metadata_plain_bytes: packed_metadata.plain_bytes,
        metadata_blob: packed_metadata.blob,
        metadata_blob_sha256: packed_metadata.output_sha256,
        body_rows,
    })
}

fn prepare_metadata_body_rows(
    sqlcmd: &Path,
    server: &str,
    database: &str,
    xml_path: &Path,
    xml: &[u8],
    properties: &SimpleMetadataXmlProperties,
    source: Option<&MetadataSourceContext>,
) -> Result<Vec<PreparedMetadataBodyStage>> {
    let mut rows = match properties.kind.as_str() {
        "Style" => prepare_style_body_row(sqlcmd, server, database, xml_path, properties, source),
        "ScheduledJob" => {
            prepare_scheduled_job_body_row(sqlcmd, server, database, xml_path, properties)
        }
        "XDTOPackage" => prepare_raw_deflated_body_row(
            sqlcmd,
            server,
            database,
            infer_xdto_package_body_path(xml_path),
            properties,
            "XDTOPackage body",
        ),
        "WSReference" => prepare_raw_deflated_body_row(
            sqlcmd,
            server,
            database,
            infer_ws_reference_definition_path(xml_path),
            properties,
            "WSReference definition",
        ),
        "CommonTemplate" | "Template" => {
            prepare_template_body_row(sqlcmd, server, database, xml_path, xml, properties, source)
        }
        "CommonPicture" => {
            prepare_common_picture_body_row(sqlcmd, server, database, xml_path, properties)
        }
        "Configuration" => {
            prepare_configuration_asset_body_rows(sqlcmd, server, database, xml_path, properties)
        }
        "BusinessProcess" => prepare_business_process_flowchart_body_row(
            sqlcmd, server, database, xml_path, properties,
        ),
        "Catalog" | "ChartOfCharacteristicTypes" => {
            prepare_predefined_data_body_row(sqlcmd, server, database, xml_path, properties)
        }
        "ExchangePlan" => prepare_exchange_plan_content_body_row(
            sqlcmd, server, database, xml_path, properties, source,
        ),
        "Form" | "CommonForm" => {
            prepare_form_body_row(sqlcmd, server, database, xml_path, properties)
        }
        "Role" => prepare_role_rights_body_row(sqlcmd, server, database, xml_path, properties),
        _ => Ok(Vec::new()),
    }?;
    rows.extend(prepare_object_help_body_row(
        sqlcmd, server, database, xml_path, properties,
    )?);
    rows.extend(prepare_object_module_body_rows(
        sqlcmd, server, database, xml_path, properties,
    )?);
    rows.extend(prepare_nested_command_module_body_rows(
        sqlcmd, server, database, xml_path, xml, properties,
    )?);
    rows.extend(prepare_command_interface_body_row(
        sqlcmd, server, database, xml_path, properties,
    )?);
    Ok(rows)
}

fn prepare_style_body_row(
    sqlcmd: &Path,
    server: &str,
    database: &str,
    xml_path: &Path,
    properties: &SimpleMetadataXmlProperties,
    source: Option<&MetadataSourceContext>,
) -> Result<Vec<PreparedMetadataBodyStage>> {
    let body_path = infer_style_body_path(xml_path);
    if !body_path.exists() {
        return Ok(Vec::new());
    }
    let source = source.ok_or_else(|| {
        anyhow!(
            "source root is required to stage Style body {}",
            body_path.display()
        )
    })?;
    let body_id = format!("{}.0", properties.uuid);
    let _base_body = fetch_config_blob(sqlcmd, server, database, &body_id)?;
    let xml = fs::read(&body_path)
        .with_context(|| format!("failed to read Style body XML {}", body_path.display()))?;
    let packed = pack_style_body_blob_from_xml(&xml, Some(source))
        .with_context(|| format!("failed to pack Style body {}", body_path.display()))?;
    Ok(vec![PreparedMetadataBodyStage {
        body_id,
        path: body_path,
        blob: packed.blob,
        blob_sha256: packed.output_sha256,
    }])
}

fn prepare_scheduled_job_body_row(
    sqlcmd: &Path,
    server: &str,
    database: &str,
    xml_path: &Path,
    properties: &SimpleMetadataXmlProperties,
) -> Result<Vec<PreparedMetadataBodyStage>> {
    let body_path = infer_scheduled_job_schedule_path(xml_path);
    if !body_path.exists() {
        return Ok(Vec::new());
    }
    let body_id = format!("{}.0", properties.uuid);
    let _base_body = fetch_config_blob(sqlcmd, server, database, &body_id)?;
    let xml = fs::read(&body_path)
        .with_context(|| format!("failed to read JobSchedule XML {}", body_path.display()))?;
    let packed = pack_schedule_blob_from_xml(&xml)
        .with_context(|| format!("failed to pack JobSchedule {}", body_path.display()))?;
    Ok(vec![PreparedMetadataBodyStage {
        body_id,
        path: body_path,
        blob: packed.blob,
        blob_sha256: packed.output_sha256,
    }])
}

fn prepare_raw_deflated_body_row(
    sqlcmd: &Path,
    server: &str,
    database: &str,
    body_path: PathBuf,
    properties: &SimpleMetadataXmlProperties,
    label: &str,
) -> Result<Vec<PreparedMetadataBodyStage>> {
    if !body_path.exists() {
        return Ok(Vec::new());
    }
    let body_id = format!("{}.0", properties.uuid);
    let _base_body = fetch_config_blob(sqlcmd, server, database, &body_id)?;
    let bytes = fs::read(&body_path)
        .with_context(|| format!("failed to read {label} {}", body_path.display()))?;
    let packed = pack_raw_deflated_blob_from_bytes(&bytes)
        .with_context(|| format!("failed to pack {label} {}", body_path.display()))?;
    Ok(vec![PreparedMetadataBodyStage {
        body_id,
        path: body_path,
        blob: packed.blob,
        blob_sha256: packed.output_sha256,
    }])
}

fn prepare_template_body_row(
    sqlcmd: &Path,
    server: &str,
    database: &str,
    xml_path: &Path,
    xml: &[u8],
    properties: &SimpleMetadataXmlProperties,
    source: Option<&MetadataSourceContext>,
) -> Result<Vec<PreparedMetadataBodyStage>> {
    let Some(template_type) = parse_template_type_from_xml(xml)? else {
        return Ok(Vec::new());
    };
    match template_type.as_str() {
        "DataCompositionAppearanceTemplate"
        | "DataCompositionSchema"
        | "GraphicalSchema"
        | "TextDocument" => {
            let Some(body_path) = infer_raw_deflated_template_body_path(xml_path, &template_type)
            else {
                return Ok(Vec::new());
            };
            prepare_raw_deflated_body_row(
                sqlcmd,
                server,
                database,
                body_path,
                properties,
                "Template body",
            )
        }
        "HTMLDocument" => {
            prepare_html_template_body_row(sqlcmd, server, database, xml_path, properties)
        }
        "SpreadsheetDocument" => prepare_spreadsheet_template_body_row(
            sqlcmd, server, database, xml_path, properties, source,
        ),
        "AddIn" | "BinaryData" => {
            prepare_binary_template_body_row(sqlcmd, server, database, xml_path, properties)
        }
        _ => Ok(Vec::new()),
    }
}

fn prepare_spreadsheet_template_body_row(
    sqlcmd: &Path,
    server: &str,
    database: &str,
    xml_path: &Path,
    properties: &SimpleMetadataXmlProperties,
    source: Option<&MetadataSourceContext>,
) -> Result<Vec<PreparedMetadataBodyStage>> {
    let body_path = infer_spreadsheet_template_body_path(xml_path);
    if !body_path.exists() {
        return Ok(Vec::new());
    }
    let body_id = format!("{}.0", properties.uuid);
    let _base_body = fetch_config_blob(sqlcmd, server, database, &body_id)?;
    let xml = fs::read(&body_path).with_context(|| {
        format!(
            "failed to read SpreadsheetDocument Template body {}",
            body_path.display()
        )
    })?;
    let packed =
        pack_moxel_spreadsheet_blob_from_xml_with_source(&xml, source).with_context(|| {
            format!(
                "failed to pack SpreadsheetDocument Template body {}",
                body_path.display()
            )
        })?;
    Ok(vec![PreparedMetadataBodyStage {
        body_id,
        path: body_path,
        blob: packed.blob,
        blob_sha256: packed.output_sha256,
    }])
}

fn prepare_html_template_body_row(
    sqlcmd: &Path,
    server: &str,
    database: &str,
    xml_path: &Path,
    properties: &SimpleMetadataXmlProperties,
) -> Result<Vec<PreparedMetadataBodyStage>> {
    let body_path = infer_html_template_body_path(xml_path);
    if !body_path.exists() {
        return Ok(Vec::new());
    }
    let body_id = format!("{}.0", properties.uuid);
    prepare_help_blob_body_row(
        sqlcmd,
        server,
        database,
        body_id,
        body_path,
        "HTML Template",
    )
}

fn prepare_binary_template_body_row(
    sqlcmd: &Path,
    server: &str,
    database: &str,
    xml_path: &Path,
    properties: &SimpleMetadataXmlProperties,
) -> Result<Vec<PreparedMetadataBodyStage>> {
    let body_path = infer_binary_template_body_path(xml_path);
    if !body_path.exists() {
        return Ok(Vec::new());
    }
    let body_id = format!("{}.0", properties.uuid);
    let _base_body = fetch_config_blob(sqlcmd, server, database, &body_id)?;
    let bytes = fs::read(&body_path).with_context(|| {
        format!(
            "failed to read binary Template body {}",
            body_path.display()
        )
    })?;
    let packed = pack_base64_payload_blob_from_bytes(&bytes).with_context(|| {
        format!(
            "failed to pack binary Template body {}",
            body_path.display()
        )
    })?;
    Ok(vec![PreparedMetadataBodyStage {
        body_id,
        path: body_path,
        blob: packed.blob,
        blob_sha256: packed.output_sha256,
    }])
}

fn prepare_common_picture_body_row(
    sqlcmd: &Path,
    server: &str,
    database: &str,
    xml_path: &Path,
    properties: &SimpleMetadataXmlProperties,
) -> Result<Vec<PreparedMetadataBodyStage>> {
    let body_path = infer_common_picture_body_path(xml_path);
    if !body_path.exists() {
        return Ok(Vec::new());
    }
    let body_id = format!("{}.0", properties.uuid);
    let _base_body = fetch_config_blob(sqlcmd, server, database, &body_id)?;
    let xml = fs::read(&body_path)
        .with_context(|| format!("failed to read ExtPicture XML {}", body_path.display()))?;
    let file_name = parse_ext_picture_file_name_from_xml(&xml)
        .with_context(|| format!("failed to parse ExtPicture XML {}", body_path.display()))?;
    let picture_path = body_path.with_extension("").join(&file_name);
    let picture = fs::read(&picture_path)
        .with_context(|| format!("failed to read ExtPicture file {}", picture_path.display()))?;
    let packed = pack_ext_picture_blob_from_bytes(&picture)
        .with_context(|| format!("failed to pack ExtPicture {}", picture_path.display()))?;
    Ok(vec![PreparedMetadataBodyStage {
        body_id,
        path: body_path,
        blob: packed.blob,
        blob_sha256: packed.output_sha256,
    }])
}

fn prepare_configuration_asset_body_rows(
    sqlcmd: &Path,
    server: &str,
    database: &str,
    xml_path: &Path,
    properties: &SimpleMetadataXmlProperties,
) -> Result<Vec<PreparedMetadataBodyStage>> {
    let mut rows = Vec::new();
    rows.extend(prepare_configuration_ext_picture_body_row(
        sqlcmd,
        server,
        database,
        properties,
        infer_configuration_ext_body_path(xml_path, "Splash.xml"),
        "2",
    )?);
    rows.extend(prepare_configuration_binary_body_row(
        sqlcmd,
        server,
        database,
        properties,
        infer_configuration_ext_body_path(xml_path, "ParentConfigurations.bin"),
        "4",
    )?);
    rows.extend(prepare_configuration_ext_picture_body_row(
        sqlcmd,
        server,
        database,
        properties,
        infer_configuration_ext_body_path(xml_path, "MainSectionPicture.xml"),
        "c",
    )?);
    Ok(rows)
}

fn prepare_configuration_ext_picture_body_row(
    sqlcmd: &Path,
    server: &str,
    database: &str,
    properties: &SimpleMetadataXmlProperties,
    body_path: PathBuf,
    suffix: &str,
) -> Result<Vec<PreparedMetadataBodyStage>> {
    if !body_path.exists() {
        return Ok(Vec::new());
    }
    let body_id = format!("{}.{}", properties.uuid, suffix);
    let _base_body = fetch_config_blob(sqlcmd, server, database, &body_id)?;
    let xml = fs::read(&body_path).with_context(|| {
        format!(
            "failed to read Configuration ExtPicture {}",
            body_path.display()
        )
    })?;
    let file_name = parse_ext_picture_file_name_from_xml(&xml).with_context(|| {
        format!(
            "failed to parse Configuration ExtPicture {}",
            body_path.display()
        )
    })?;
    let picture_path = body_path.with_extension("").join(&file_name);
    let picture = fs::read(&picture_path).with_context(|| {
        format!(
            "failed to read Configuration ExtPicture file {}",
            picture_path.display()
        )
    })?;
    let packed = pack_ext_picture_blob_from_bytes(&picture).with_context(|| {
        format!(
            "failed to pack Configuration ExtPicture {}",
            picture_path.display()
        )
    })?;
    Ok(vec![PreparedMetadataBodyStage {
        body_id,
        path: body_path,
        blob: packed.blob,
        blob_sha256: packed.output_sha256,
    }])
}

fn prepare_configuration_binary_body_row(
    sqlcmd: &Path,
    server: &str,
    database: &str,
    properties: &SimpleMetadataXmlProperties,
    body_path: PathBuf,
    suffix: &str,
) -> Result<Vec<PreparedMetadataBodyStage>> {
    if !body_path.exists() {
        return Ok(Vec::new());
    }
    let body_id = format!("{}.{}", properties.uuid, suffix);
    let _base_body = fetch_config_blob(sqlcmd, server, database, &body_id)?;
    let blob = fs::read(&body_path).with_context(|| {
        format!(
            "failed to read Configuration binary {}",
            body_path.display()
        )
    })?;
    let blob_sha256 = hex_sha256(&blob);
    Ok(vec![PreparedMetadataBodyStage {
        body_id,
        path: body_path,
        blob,
        blob_sha256,
    }])
}

fn prepare_exchange_plan_content_body_row(
    sqlcmd: &Path,
    server: &str,
    database: &str,
    xml_path: &Path,
    properties: &SimpleMetadataXmlProperties,
    source: Option<&MetadataSourceContext>,
) -> Result<Vec<PreparedMetadataBodyStage>> {
    let body_path = infer_exchange_plan_content_body_path(xml_path);
    if !body_path.exists() {
        return Ok(Vec::new());
    }
    let source = source.ok_or_else(|| {
        anyhow!(
            "source root is required to stage ExchangePlan Content.xml {}",
            body_path.display()
        )
    })?;
    let body_id = format!("{}.1", properties.uuid);
    let base_body = fetch_config_blob(sqlcmd, server, database, &body_id)?;
    let xml = fs::read(&body_path).with_context(|| {
        format!(
            "failed to read ExchangePlan Content {}",
            body_path.display()
        )
    })?;
    let packed =
        pack_exchange_plan_content_blob_from_xml(&base_body, &xml, source).with_context(|| {
            format!(
                "failed to pack ExchangePlan Content {}",
                body_path.display()
            )
        })?;
    Ok(vec![PreparedMetadataBodyStage {
        body_id,
        path: body_path,
        blob: packed.blob,
        blob_sha256: packed.output_sha256,
    }])
}

fn prepare_predefined_data_body_row(
    sqlcmd: &Path,
    server: &str,
    database: &str,
    xml_path: &Path,
    properties: &SimpleMetadataXmlProperties,
) -> Result<Vec<PreparedMetadataBodyStage>> {
    let Some(suffix) = predefined_data_body_suffix(&properties.kind) else {
        return Ok(Vec::new());
    };
    let body_path = infer_predefined_data_body_path(xml_path);
    if !body_path.exists() {
        return Ok(Vec::new());
    }
    let body_id = format!("{}.{}", properties.uuid, suffix);
    let base_body = fetch_config_blob(sqlcmd, server, database, &body_id)?;
    let xml = fs::read(&body_path)
        .with_context(|| format!("failed to read PredefinedData {}", body_path.display()))?;
    let packed = pack_predefined_data_blob_from_xml(&base_body, &xml)
        .with_context(|| format!("failed to pack PredefinedData {}", body_path.display()))?;
    Ok(vec![PreparedMetadataBodyStage {
        body_id,
        path: body_path,
        blob: packed.blob,
        blob_sha256: packed.output_sha256,
    }])
}

fn prepare_business_process_flowchart_body_row(
    sqlcmd: &Path,
    server: &str,
    database: &str,
    xml_path: &Path,
    properties: &SimpleMetadataXmlProperties,
) -> Result<Vec<PreparedMetadataBodyStage>> {
    let body_path = infer_business_process_flowchart_body_path(xml_path);
    if !body_path.exists() {
        return Ok(Vec::new());
    }
    let body_id = format!("{}.7", properties.uuid);
    let base_body = fetch_config_blob(sqlcmd, server, database, &body_id)?;
    let xml = fs::read(&body_path).with_context(|| {
        format!(
            "failed to read BusinessProcess Flowchart {}",
            body_path.display()
        )
    })?;
    let packed =
        pack_business_process_flowchart_blob_from_xml(&base_body, &xml).with_context(|| {
            format!(
                "failed to pack BusinessProcess Flowchart {}",
                body_path.display()
            )
        })?;
    Ok(vec![PreparedMetadataBodyStage {
        body_id,
        path: body_path,
        blob: packed.blob,
        blob_sha256: packed.output_sha256,
    }])
}

fn prepare_form_body_row(
    sqlcmd: &Path,
    server: &str,
    database: &str,
    xml_path: &Path,
    properties: &SimpleMetadataXmlProperties,
) -> Result<Vec<PreparedMetadataBodyStage>> {
    let module_path = infer_form_module_body_path(xml_path);
    if !module_path.exists() {
        return Ok(Vec::new());
    }
    let body_id = format!("{}.0", properties.uuid);
    let base_body = fetch_config_blob(sqlcmd, server, database, &body_id)?;
    let module_text = fs::read(&module_path)
        .with_context(|| format!("failed to read Form module {}", module_path.display()))?;
    let packed = pack_form_body_blob_from_module_text(&base_body, &module_text)
        .with_context(|| format!("failed to pack Form body {}", module_path.display()))?;
    Ok(vec![PreparedMetadataBodyStage {
        body_id,
        path: module_path,
        blob: packed.blob,
        blob_sha256: packed.output_sha256,
    }])
}

fn prepare_role_rights_body_row(
    sqlcmd: &Path,
    server: &str,
    database: &str,
    xml_path: &Path,
    properties: &SimpleMetadataXmlProperties,
) -> Result<Vec<PreparedMetadataBodyStage>> {
    let body_path = infer_role_rights_body_path(xml_path);
    if !body_path.exists() {
        return Ok(Vec::new());
    }
    let body_id = format!("{}.0", properties.uuid);
    let base_body = fetch_config_blob(sqlcmd, server, database, &body_id)?;
    let xml = fs::read(&body_path)
        .with_context(|| format!("failed to read Role rights XML {}", body_path.display()))?;
    let packed = pack_role_rights_blob_from_xml(&base_body, &xml)
        .with_context(|| format!("failed to pack Role rights {}", body_path.display()))?;
    Ok(vec![PreparedMetadataBodyStage {
        body_id,
        path: body_path,
        blob: packed.blob,
        blob_sha256: packed.output_sha256,
    }])
}

fn prepare_command_interface_body_row(
    sqlcmd: &Path,
    server: &str,
    database: &str,
    xml_path: &Path,
    properties: &SimpleMetadataXmlProperties,
) -> Result<Vec<PreparedMetadataBodyStage>> {
    let Some(suffix) = command_interface_body_suffix(&properties.kind) else {
        return Ok(Vec::new());
    };
    let body_path = infer_command_interface_body_path(xml_path);
    if !body_path.exists() {
        return Ok(Vec::new());
    }
    let body_id = format!("{}.{}", properties.uuid, suffix);
    let base_body = fetch_config_blob(sqlcmd, server, database, &body_id)?;
    let xml = fs::read(&body_path).with_context(|| {
        format!(
            "failed to read CommandInterface XML {}",
            body_path.display()
        )
    })?;
    let packed = pack_command_interface_blob_from_xml(&base_body, &xml)
        .with_context(|| format!("failed to pack CommandInterface {}", body_path.display()))?;
    Ok(vec![PreparedMetadataBodyStage {
        body_id,
        path: body_path,
        blob: packed.blob,
        blob_sha256: packed.output_sha256,
    }])
}

fn prepare_object_help_body_row(
    sqlcmd: &Path,
    server: &str,
    database: &str,
    xml_path: &Path,
    properties: &SimpleMetadataXmlProperties,
) -> Result<Vec<PreparedMetadataBodyStage>> {
    let body_path = infer_object_help_body_path(xml_path);
    if !body_path.exists() {
        return Ok(Vec::new());
    }
    let body_id = infer_help_body_id(properties);
    prepare_help_blob_body_row(sqlcmd, server, database, body_id, body_path, "Help")
}

fn prepare_help_blob_body_row(
    sqlcmd: &Path,
    server: &str,
    database: &str,
    body_id: String,
    body_path: PathBuf,
    label: &str,
) -> Result<Vec<PreparedMetadataBodyStage>> {
    let _base_body = fetch_config_blob(sqlcmd, server, database, &body_id)?;
    let xml = fs::read(&body_path)
        .with_context(|| format!("failed to read {label} XML {}", body_path.display()))?;
    let page_names = parse_help_pages_from_xml(&xml)
        .with_context(|| format!("failed to parse {label} XML {}", body_path.display()))?;
    let help_dir = body_path.with_extension("");
    let mut pages = Vec::with_capacity(page_names.len());
    for page in page_names {
        if page.contains('/') || page.contains('\\') || page == "." || page == ".." {
            return Err(anyhow!("unsupported {label} page name: {page}"));
        }
        let page_path = help_dir.join(format!("{page}.html"));
        let content = fs::read(&page_path)
            .with_context(|| format!("failed to read {label} page {}", page_path.display()))?;
        pages.push((page, content));
    }
    let mut files = Vec::<(String, Vec<u8>)>::new();
    let files_dir = help_dir.join("_files");
    if files_dir.exists() {
        for entry in fs::read_dir(&files_dir)
            .with_context(|| format!("failed to read {label} files dir {}", files_dir.display()))?
        {
            let entry = entry
                .with_context(|| format!("failed to read entry in {}", files_dir.display()))?;
            let file_type = entry
                .file_type()
                .with_context(|| format!("failed to stat {}", entry.path().display()))?;
            if !file_type.is_file() {
                continue;
            }
            let file_name = entry.file_name().to_string_lossy().to_string();
            let content = fs::read(entry.path()).with_context(|| {
                format!("failed to read {label} file {}", entry.path().display())
            })?;
            files.push((file_name, content));
        }
        files.sort_by(|left, right| left.0.cmp(&right.0));
    }
    let packed = pack_help_blob_from_parts(&pages, &files)
        .with_context(|| format!("failed to pack {label} {}", body_path.display()))?;
    Ok(vec![PreparedMetadataBodyStage {
        body_id,
        path: body_path,
        blob: packed.blob,
        blob_sha256: packed.output_sha256,
    }])
}

fn infer_help_body_id(properties: &SimpleMetadataXmlProperties) -> String {
    let suffix = if matches!(properties.kind.as_str(), "Form" | "CommonForm") {
        "1"
    } else {
        "5"
    };
    format!("{}.{}", properties.uuid, suffix)
}

fn prepare_object_module_body_rows(
    sqlcmd: &Path,
    server: &str,
    database: &str,
    xml_path: &Path,
    properties: &SimpleMetadataXmlProperties,
) -> Result<Vec<PreparedMetadataBodyStage>> {
    let mut rows = Vec::new();
    for (suffix, file_name) in object_module_body_suffixes(&properties.kind) {
        let body_path = if properties.kind == "Configuration" {
            infer_configuration_module_body_path(xml_path, file_name)
        } else {
            infer_object_module_body_path(xml_path, file_name)
        };
        if !body_path.exists() {
            continue;
        }
        let body_id = format!("{}.{}", properties.uuid, suffix);
        let base_body = fetch_config_blob(sqlcmd, server, database, &body_id)?;
        let text = fs::read(&body_path)
            .with_context(|| format!("failed to read module body {}", body_path.display()))?;
        let packed = pack_module_blob_bytes(&text, Some(&base_body), None)
            .with_context(|| format!("failed to pack module body {}", body_path.display()))?;
        rows.push(PreparedMetadataBodyStage {
            body_id,
            path: body_path,
            blob: packed.blob,
            blob_sha256: packed.output_sha256,
        });
    }
    Ok(rows)
}

fn object_module_body_suffixes(kind: &str) -> &'static [(&'static str, &'static str)] {
    match kind {
        "Bot" => &[("1", "Module.bsl")],
        "Configuration" => &[
            ("0", "OrdinaryApplicationModule.bsl"),
            ("5", "ExternalConnectionModule.bsl"),
            ("6", "ManagedApplicationModule.bsl"),
            ("7", "SessionModule.bsl"),
        ],
        "CommonCommand" => &[("2", "CommandModule.bsl")],
        "Constant" => &[("0", "ValueManagerModule.bsl"), ("1", "ManagerModule.bsl")],
        "SettingsStorage" => &[("8", "ManagerModule.bsl")],
        "Sequence" => &[("0", "RecordSetModule.bsl")],
        "Catalog" => &[("0", "ObjectModule.bsl"), ("3", "ManagerModule.bsl")],
        "Report" | "DataProcessor" | "Document" => {
            &[("0", "ObjectModule.bsl"), ("2", "ManagerModule.bsl")]
        }
        "Enum" => &[("0", "ManagerModule.bsl")],
        "ExchangePlan" => &[("2", "ObjectModule.bsl"), ("3", "ManagerModule.bsl")],
        "AccumulationRegister"
        | "AccountingRegister"
        | "CalculationRegister"
        | "InformationRegister" => &[("1", "RecordSetModule.bsl"), ("2", "ManagerModule.bsl")],
        "DocumentJournal" => &[("1", "ManagerModule.bsl")],
        "Task" => &[("6", "ObjectModule.bsl"), ("7", "ManagerModule.bsl")],
        "BusinessProcess" => &[("6", "ObjectModule.bsl"), ("8", "ManagerModule.bsl")],
        "ChartOfCharacteristicTypes" => &[("15", "ObjectModule.bsl"), ("16", "ManagerModule.bsl")],
        "HTTPService" | "WebService" => &[("0", "Module.bsl")],
        "IntegrationService" => &[("0", "Module.bsl")],
        _ => &[],
    }
}

fn command_interface_body_suffix(kind: &str) -> Option<&'static str> {
    match kind {
        "Subsystem" => Some("1"),
        _ => None,
    }
}

fn predefined_data_body_suffix(kind: &str) -> Option<&'static str> {
    match kind {
        "Catalog" => Some("1c"),
        "ChartOfCharacteristicTypes" => Some("7"),
        _ => None,
    }
}

fn prepare_nested_command_module_body_rows(
    sqlcmd: &Path,
    server: &str,
    database: &str,
    xml_path: &Path,
    xml: &[u8],
    properties: &SimpleMetadataXmlProperties,
) -> Result<Vec<PreparedMetadataBodyStage>> {
    let sources = nested_command_module_sources(xml_path, xml, properties)?;
    let mut rows = Vec::with_capacity(sources.len());
    for source in sources {
        let body_id = format!("{}.2", source.command_id);
        let base_body = fetch_config_blob(sqlcmd, server, database, &body_id)?;
        let text = fs::read(&source.body_path).with_context(|| {
            format!(
                "failed to read nested command module body {}",
                source.body_path.display()
            )
        })?;
        let packed = pack_module_blob_bytes(&text, Some(&base_body), None).with_context(|| {
            format!(
                "failed to pack nested command module body {}",
                source.body_path.display()
            )
        })?;
        rows.push(PreparedMetadataBodyStage {
            body_id,
            path: source.body_path,
            blob: packed.blob,
            blob_sha256: packed.output_sha256,
        });
    }
    Ok(rows)
}

fn nested_command_module_sources(
    xml_path: &Path,
    xml: &[u8],
    properties: &SimpleMetadataXmlProperties,
) -> Result<Vec<NestedCommandModuleSource>> {
    if !metadata_kind_can_own_commands(&properties.kind) {
        return Ok(Vec::new());
    }
    let commands_dir = xml_path.with_extension("").join("Commands");
    if !commands_dir.exists() {
        return Ok(Vec::new());
    }
    let command_ids = parse_nested_command_ids_by_name(xml)?;
    let mut sources = Vec::new();
    for entry in fs::read_dir(&commands_dir)
        .with_context(|| format!("failed to read Commands dir {}", commands_dir.display()))?
    {
        let entry =
            entry.with_context(|| format!("failed to read entry in {}", commands_dir.display()))?;
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to stat {}", entry.path().display()))?;
        if !file_type.is_dir() {
            continue;
        }
        let command_name = entry.file_name().to_string_lossy().to_string();
        let body_path = entry.path().join("Ext").join("CommandModule.bsl");
        if !body_path.exists() {
            continue;
        }
        let command_id = command_ids.get(&command_name).cloned().ok_or_else(|| {
            anyhow!(
                "nested command module {} has no matching Command named {} in {}",
                body_path.display(),
                command_name,
                xml_path.display()
            )
        })?;
        sources.push(NestedCommandModuleSource {
            command_id,
            command_name,
            body_path,
        });
    }
    sources.sort_by(|left, right| left.body_path.cmp(&right.body_path));
    Ok(sources)
}

fn metadata_kind_can_own_commands(kind: &str) -> bool {
    matches!(
        kind,
        "AccountingRegister"
            | "AccumulationRegister"
            | "BusinessProcess"
            | "CalculationRegister"
            | "Catalog"
            | "ChartOfAccounts"
            | "ChartOfCalculationTypes"
            | "ChartOfCharacteristicTypes"
            | "DataProcessor"
            | "Document"
            | "DocumentJournal"
            | "Enum"
            | "ExchangePlan"
            | "InformationRegister"
            | "Report"
            | "SettingsStorage"
            | "Task"
    )
}

fn parse_nested_command_ids_by_name(xml: &[u8]) -> Result<BTreeMap<String, String>> {
    let mut reader = Reader::from_reader(xml);
    let mut buffer = Vec::new();
    let mut path = Vec::<String>::new();
    let mut commands = BTreeMap::<String, String>::new();
    let mut pending_uuid = None::<String>;
    let mut pending_name = None::<String>;

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                let local = xml_local_name_for_stage(event.local_name().as_ref());
                if local == "Command" && pending_uuid.is_none() {
                    pending_uuid = Some(command_xml_uuid(&event)?);
                    pending_name = None;
                } else if pending_uuid.is_some()
                    && path_ends_with_for_stage(&path, &["Command", "Properties"])
                    && local == "Name"
                {
                    pending_name = Some(String::new());
                }
                path.push(local);
            }
            Ok(Event::Empty(_)) => {}
            Ok(Event::Text(text)) => {
                if path_ends_with_for_stage(&path, &["Command", "Properties", "Name"]) {
                    if let Some(name) = pending_name.as_mut() {
                        name.push_str(text.xml_content()?.as_ref());
                    }
                }
            }
            Ok(Event::CData(text)) => {
                if path_ends_with_for_stage(&path, &["Command", "Properties", "Name"]) {
                    if let Some(name) = pending_name.as_mut() {
                        name.push_str(text.xml_content()?.as_ref());
                    }
                }
            }
            Ok(Event::End(event)) => {
                let local = xml_local_name_for_stage(event.local_name().as_ref());
                if local == "Command" {
                    if let (Some(uuid), Some(name)) = (pending_uuid.take(), pending_name.take()) {
                        commands.insert(name, uuid);
                    }
                }
                let _ = path.pop();
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(error.into()),
        }
        buffer.clear();
    }

    Ok(commands)
}

fn command_xml_uuid(event: &BytesStart<'_>) -> Result<String> {
    let value = xml_attr_value_for_stage(event, "uuid")
        .ok_or_else(|| anyhow!("Command XML element has no uuid attribute"))?;
    Ok(uuid::Uuid::parse_str(&value)
        .with_context(|| format!("invalid Command uuid {value}"))?
        .hyphenated()
        .to_string())
}

fn xml_attr_value_for_stage(event: &BytesStart<'_>, name: &str) -> Option<String> {
    event
        .attributes()
        .filter_map(Result::ok)
        .find(|attr| attr.key.as_ref() == name.as_bytes())
        .map(|attr| String::from_utf8_lossy(attr.value.as_ref()).to_string())
}

fn xml_local_name_for_stage(name: &[u8]) -> String {
    String::from_utf8_lossy(name).to_string()
}

fn path_ends_with_for_stage(path: &[String], suffix: &[&str]) -> bool {
    path.len() >= suffix.len()
        && path[path.len() - suffix.len()..]
            .iter()
            .zip(suffix)
            .all(|(left, right)| left == right)
}

fn prepare_common_module_object_stage(
    sqlcmd: &Path,
    server: &str,
    database: &str,
    xml_path: PathBuf,
    text_path: Option<PathBuf>,
) -> Result<PreparedCommonModuleObjectStage> {
    let xml = fs::read(&xml_path)
        .with_context(|| format!("failed to read XML {}", xml_path.display()))?;
    let properties = parse_common_module_xml_properties(&xml)?;
    let module_id = properties.uuid.clone();
    let text_path = text_path.unwrap_or_else(|| infer_common_module_text_path(&xml_path));
    let text = fs::read(&text_path)
        .with_context(|| format!("failed to read BSL text {}", text_path.display()))?;

    let base_metadata_blob = fetch_config_blob(sqlcmd, server, database, &module_id)?;
    let packed_metadata = pack_common_module_metadata_blob_from_xml(&base_metadata_blob, &xml)?;
    let module_body_id = format!("{module_id}.0");
    let base_module_blob = fetch_config_blob(sqlcmd, server, database, &module_body_id)?;
    let packed_module = pack_module_blob_bytes(&text, Some(&base_module_blob), None)?;

    Ok(PreparedCommonModuleObjectStage {
        module_id,
        module_body_id,
        xml: xml_path,
        text: text_path,
        properties: packed_metadata.properties,
        metadata_plain_bytes: packed_metadata.plain_bytes,
        metadata_blob: packed_metadata.blob,
        metadata_blob_sha256: packed_metadata.output_sha256,
        text_bytes: text.len(),
        module_blob: packed_module.blob,
        module_blob_sha256: packed_module.output_sha256,
    })
}

fn stage_prepared_common_module_objects(
    sqlcmd: &Path,
    server: &str,
    database: &str,
    prepared: Vec<PreparedCommonModuleObjectStage>,
    replace_config_save: bool,
    script_output: Option<PathBuf>,
    default_script_name: &str,
) -> Result<StageCommonModuleObjectsReport> {
    if !replace_config_save {
        return Err(anyhow!(
            "staging deletes existing ConfigSave rows; pass --replace-config-save"
        ));
    }
    if prepared.is_empty() {
        return Err(anyhow!("at least one common module object must be staged"));
    }
    ensure_unique_common_module_object_ids(&prepared)?;

    let versions_blob = fetch_config_blob(sqlcmd, server, database, "versions")?;
    let changes = prepared
        .iter()
        .flat_map(|module| [module.module_id.clone(), module.module_body_id.clone()])
        .collect::<Vec<_>>();
    let patched_versions = patch_versions_blob_bytes(&versions_blob, &changes, true)?;

    let before = storage_table_stats(sqlcmd, server, database, "ConfigSave")?;
    let script =
        script_output.unwrap_or_else(|| default_stage_script_path(database, default_script_name));
    if let Some(parent) = script.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let sql = build_stage_common_module_objects_sql(database, &prepared, &patched_versions.blob);
    fs::write(&script, sql).with_context(|| format!("failed to write {}", script.display()))?;
    run_sql_file(sqlcmd, server, &script)?;

    let after = storage_table_stats(sqlcmd, server, database, "ConfigSave")?;
    let modules = prepared
        .into_iter()
        .map(|module| StagedCommonModuleObjectReport {
            module_id: module.module_id,
            module_body_id: module.module_body_id,
            xml: module.xml,
            text: module.text,
            properties: module.properties,
            metadata_plain_bytes: module.metadata_plain_bytes,
            metadata_blob: GeneratedBlobReport {
                bytes: module.metadata_blob.len(),
                sha256: module.metadata_blob_sha256,
            },
            text_bytes: module.text_bytes,
            module_blob: GeneratedBlobReport {
                bytes: module.module_blob.len(),
                sha256: module.module_blob_sha256,
            },
        })
        .collect();

    Ok(StageCommonModuleObjectsReport {
        database: database.to_string(),
        modules,
        script,
        before,
        after,
        versions_blob: GeneratedBlobReport {
            bytes: patched_versions.blob.len(),
            sha256: patched_versions.output_sha256,
        },
        version_replacements: patched_versions.replacements,
    })
}

fn stage_common_module_specs(
    sqlcmd: &Path,
    server: &str,
    database: &str,
    specs: Vec<CommonModuleStageSpec>,
    replace_config_save: bool,
    script_output: Option<PathBuf>,
) -> Result<StageCommonModulesReport> {
    if !replace_config_save {
        return Err(anyhow!(
            "staging deletes existing ConfigSave rows; pass --replace-config-save"
        ));
    }
    if specs.is_empty() {
        return Err(anyhow!("at least one common module must be staged"));
    }
    ensure_unique_module_ids(&specs)?;

    let prepared = parallel::install(|| {
        specs
            .into_par_iter()
            .map(|spec| {
                let module_body_id = format!("{}.0", spec.module_id);
                let text = fs::read(&spec.text)
                    .with_context(|| format!("failed to read BSL text {}", spec.text.display()))?;
                let base_module_blob =
                    fetch_config_blob(sqlcmd, server, database, &module_body_id)?;
                let packed_module = pack_module_blob_bytes(&text, Some(&base_module_blob), None)?;
                Ok(PreparedCommonModuleStage {
                    spec,
                    module_body_id,
                    text_bytes: text.len(),
                    blob_sha256: packed_module.output_sha256,
                    blob: packed_module.blob,
                })
            })
            .collect::<Result<Vec<_>>>()
    })??;

    let versions_blob = fetch_config_blob(sqlcmd, server, database, "versions")?;
    let changes = prepared
        .iter()
        .flat_map(|module| [module.spec.module_id.clone(), module.module_body_id.clone()])
        .collect::<Vec<_>>();
    let patched_versions = patch_versions_blob_bytes(&versions_blob, &changes, true)?;

    let before = storage_table_stats(sqlcmd, server, database, "ConfigSave")?;
    let script = script_output.unwrap_or_else(|| {
        default_stage_script_path(database, &format!("common_modules_{}", prepared.len()))
    });
    if let Some(parent) = script.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let sql = build_stage_common_modules_sql(database, &prepared, &patched_versions.blob);
    fs::write(&script, sql).with_context(|| format!("failed to write {}", script.display()))?;
    run_sql_file(sqlcmd, server, &script)?;

    let after = storage_table_stats(sqlcmd, server, database, "ConfigSave")?;
    let modules = prepared
        .into_iter()
        .map(|module| StagedCommonModuleReport {
            module_id: module.spec.module_id,
            module_body_id: module.module_body_id,
            text: module.spec.text,
            text_bytes: module.text_bytes,
            module_blob: GeneratedBlobReport {
                bytes: module.blob.len(),
                sha256: module.blob_sha256,
            },
        })
        .collect();

    Ok(StageCommonModulesReport {
        database: database.to_string(),
        modules,
        script,
        before,
        after,
        versions_blob: GeneratedBlobReport {
            bytes: patched_versions.blob.len(),
            sha256: patched_versions.output_sha256,
        },
        version_replacements: patched_versions.replacements,
    })
}

fn parse_common_module_specs(values: &[String]) -> Result<Vec<CommonModuleStageSpec>> {
    values
        .iter()
        .map(|value| {
            let (module_id, text) = value.split_once('=').ok_or_else(|| {
                anyhow!("module value must have the form <metadata-uuid>=<path>, got {value}")
            })?;
            let module_id = module_id.trim();
            if module_id.is_empty() {
                return Err(anyhow!("module id cannot be empty in {value}"));
            }
            let text = text.trim();
            if text.is_empty() {
                return Err(anyhow!("module text path cannot be empty in {value}"));
            }
            Ok(CommonModuleStageSpec {
                module_id: module_id.to_string(),
                text: PathBuf::from(text),
            })
        })
        .collect()
}

fn normalize_uuid_arg(value: &str) -> Result<String> {
    Ok(uuid::Uuid::parse_str(value.trim())?
        .hyphenated()
        .to_string())
}

fn ensure_unique_module_ids(specs: &[CommonModuleStageSpec]) -> Result<()> {
    let mut seen = BTreeSet::new();
    for spec in specs {
        if !seen.insert(spec.module_id.as_str()) {
            return Err(anyhow!("duplicate module id: {}", spec.module_id));
        }
    }
    Ok(())
}

fn ensure_unique_common_module_object_ids(
    modules: &[PreparedCommonModuleObjectStage],
) -> Result<()> {
    let mut seen = BTreeSet::new();
    for module in modules {
        if !seen.insert(module.module_id.as_str()) {
            return Err(anyhow!(
                "duplicate common module object id: {}",
                module.module_id
            ));
        }
        if !seen.insert(module.module_body_id.as_str()) {
            return Err(anyhow!(
                "duplicate common module body id: {}",
                module.module_body_id
            ));
        }
    }
    Ok(())
}

fn ensure_unique_metadata_object_ids(objects: &[PreparedMetadataObjectStage]) -> Result<()> {
    let mut seen = BTreeSet::new();
    for object in objects {
        if !seen.insert(object.object_id.as_str()) {
            return Err(anyhow!(
                "duplicate metadata object id: {}",
                object.object_id
            ));
        }
        for body in &object.body_rows {
            if !seen.insert(body.body_id.as_str()) {
                return Err(anyhow!("duplicate metadata body id: {}", body.body_id));
            }
        }
    }
    Ok(())
}

fn ensure_unique_source_stage_ids(
    metadata_objects: &[PreparedMetadataObjectStage],
    common_modules: &[PreparedCommonModuleObjectStage],
) -> Result<()> {
    let mut seen = BTreeSet::new();
    for object in metadata_objects {
        if !seen.insert(object.object_id.as_str()) {
            return Err(anyhow!(
                "duplicate metadata object id in source tree stage: {}",
                object.object_id
            ));
        }
        for body in &object.body_rows {
            if !seen.insert(body.body_id.as_str()) {
                return Err(anyhow!(
                    "duplicate metadata body id in source tree stage: {}",
                    body.body_id
                ));
            }
        }
    }
    for module in common_modules {
        if !seen.insert(module.module_id.as_str()) {
            return Err(anyhow!(
                "duplicate common module id in source tree stage: {}",
                module.module_id
            ));
        }
        if !seen.insert(module.module_body_id.as_str()) {
            return Err(anyhow!(
                "duplicate common module body id in source tree stage: {}",
                module.module_body_id
            ));
        }
    }
    Ok(())
}

fn source_metadata_xmls(
    manifest: &crate::source::SourceManifest,
    source_root: &Path,
) -> Vec<PathBuf> {
    manifest
        .files
        .iter()
        .filter(|file| is_stage_metadata_xml(&file.path))
        .map(|file| source_root.join(&file.path))
        .collect()
}

fn source_common_module_xmls(
    manifest: &crate::source::SourceManifest,
    source_root: &Path,
) -> Vec<PathBuf> {
    manifest
        .files
        .iter()
        .filter(|file| is_root_common_module_xml(&file.path))
        .map(|file| source_root.join(&file.path))
        .collect()
}

fn compare_shapes(
    left_name: &str,
    right_name: &str,
    left: &[TableShape],
    right: &[TableShape],
) -> MssqlCompareReport {
    let left_by_name = by_table_name(left);
    let right_by_name = by_table_name(right);
    let all_names = left_by_name
        .keys()
        .chain(right_by_name.keys())
        .cloned()
        .collect::<BTreeSet<_>>();

    let mut summary = MssqlCompareSummary {
        left_tables: left.len(),
        right_tables: right.len(),
        ..MssqlCompareSummary::default()
    };
    let mut differences = Vec::new();

    for name in all_names {
        match (left_by_name.get(&name), right_by_name.get(&name)) {
            (None, Some(_)) => {
                summary.missing_in_left += 1;
                differences.push(MssqlDifference {
                    kind: "missing_in_left".to_string(),
                    table: name,
                    detail: format!("exists in {right_name} only"),
                });
            }
            (Some(_), None) => {
                summary.missing_in_right += 1;
                differences.push(MssqlDifference {
                    kind: "missing_in_right".to_string(),
                    table: name,
                    detail: format!("exists in {left_name} only"),
                });
            }
            (Some(left), Some(right)) => {
                if left.row_count != right.row_count {
                    summary.row_count_differences += 1;
                    differences.push(MssqlDifference {
                        kind: "row_count".to_string(),
                        table: name.clone(),
                        detail: format!("{} rows vs {} rows", left.row_count, right.row_count),
                    });
                }
                if left.columns != right.columns {
                    summary.column_differences += 1;
                    differences.push(MssqlDifference {
                        kind: "columns".to_string(),
                        table: name.clone(),
                        detail: "column definitions differ".to_string(),
                    });
                }
                if left.row_checksum != right.row_checksum {
                    summary.checksum_differences += 1;
                    differences.push(MssqlDifference {
                        kind: "checksum".to_string(),
                        table: name,
                        detail: format!(
                            "row checksum {:?} vs {:?}",
                            left.row_checksum, right.row_checksum
                        ),
                    });
                }
            }
            (None, None) => unreachable!("table union cannot produce an empty match"),
        }
    }

    MssqlCompareReport {
        left: left_name.to_string(),
        right: right_name.to_string(),
        same: differences.is_empty(),
        summary,
        differences,
    }
}

fn load_table_shapes(sqlcmd: &Path, server: &str, database: &str) -> Result<Vec<TableShape>> {
    let sql = format!(
        "SET NOCOUNT ON; USE {db};\n\
         SELECT t.name AS table_name,\n\
                ISNULL(SUM(CASE WHEN ps.index_id IN (0, 1) THEN ps.row_count ELSE 0 END), 0) AS row_count,\n\
                CHECKSUM_AGG(BINARY_CHECKSUM(*)) AS row_checksum,\n\
                JSON_QUERY((\n\
                    SELECT c.name,\n\
                           TYPE_NAME(c.user_type_id) AS type_name,\n\
                           c.max_length,\n\
                           c.precision,\n\
                           c.scale,\n\
                           CONVERT(bit, c.is_nullable) AS is_nullable\n\
                    FROM sys.columns c\n\
                    WHERE c.object_id = t.object_id\n\
                    ORDER BY c.column_id\n\
                    FOR JSON PATH\n\
                )) AS columns\n\
         FROM sys.tables t\n\
         LEFT JOIN sys.dm_db_partition_stats ps ON ps.object_id = t.object_id AND ps.index_id IN (0, 1)\n\
         GROUP BY t.object_id, t.name\n\
         ORDER BY t.name\n\
         FOR JSON PATH;",
        db = quote_ident(database)
    );
    let stdout = run_sql_capture(sqlcmd, server, &sql)?;
    let json = extract_json_array(&stdout, &format!("load_table_shapes({database})"))?;
    serde_json::from_str(&json)
        .with_context(|| format!("failed to parse table JSON for {database}"))
}

fn load_database_files(sqlcmd: &Path, server: &str, database: &str) -> Result<Vec<DatabaseFile>> {
    let sql = format!(
        "SET NOCOUNT ON; USE {db}; SELECT name, type_desc, physical_name FROM sys.database_files FOR JSON PATH;",
        db = quote_ident(database)
    );
    let stdout = run_sql_capture(sqlcmd, server, &sql)?;
    let json = extract_json_array(&stdout, &format!("load_database_files({database})"))?;
    serde_json::from_str(&json).with_context(|| format!("failed to parse file JSON for {database}"))
}

fn storage_table_stats(
    sqlcmd: &Path,
    server: &str,
    database: &str,
    table: &str,
) -> Result<StorageTableManifest> {
    let sql = format!(
        "SET NOCOUNT ON; USE {db}; SELECT N'{table}' AS table_name, COUNT_BIG(*) AS row_count, ISNULL(SUM(CONVERT(bigint, DATALENGTH(BinaryData))), 0) AS binary_bytes, CONVERT(bigint, CHECKSUM_AGG(BINARY_CHECKSUM(*))) AS row_checksum FROM {table_ident} FOR JSON PATH;",
        db = quote_ident(database),
        table = quote_string(table),
        table_ident = quote_ident(table),
    );
    let stdout = run_sql_capture(sqlcmd, server, &sql)?;
    let json = extract_json_array(&stdout, &format!("storage_table_stats({table})"))?;
    let mut values: Vec<StorageTableManifest> = serde_json::from_str(&json)
        .with_context(|| format!("failed to parse storage stats JSON for {table}"))?;
    let mut value = values
        .pop()
        .ok_or_else(|| anyhow!("storage stats query returned no rows for {table}"))?;
    value.file_name = format!("{table}.bcp");
    Ok(value)
}

fn configsave_row_digests(
    sqlcmd: &Path,
    server: &str,
    database: &str,
) -> Result<Vec<ConfigSaveRowDigest>> {
    let sql = format!(
        "SET NOCOUNT ON; USE {db};\n\
         SELECT FileName AS file_name,\n\
                PartNo AS part_no,\n\
                DataSize AS data_size,\n\
                DATALENGTH(BinaryData) AS binary_bytes,\n\
                CONVERT(varchar(64), HASHBYTES('SHA2_256', BinaryData), 2) AS sha256\n\
         FROM ConfigSave\n\
         ORDER BY FileName, PartNo\n\
         FOR JSON PATH;",
        db = quote_ident(database),
    );
    let stdout = run_sql_capture(sqlcmd, server, &sql)?;
    let json = extract_json_array(&stdout, &format!("configsave_row_digests({database})"))?;
    serde_json::from_str(&json)
        .with_context(|| format!("failed to parse ConfigSave digests JSON for {database}"))
}

fn fetch_config_blob(
    sqlcmd: &Path,
    server: &str,
    database: &str,
    file_name: &str,
) -> Result<Vec<u8>> {
    let sql = format!(
        "SET NOCOUNT ON; USE {db};\n\
         SELECT FileName AS file_name,\n\
                DataSize AS data_size,\n\
                CONVERT(varchar(max), BinaryData, 2) AS binary_hex\n\
         FROM Config\n\
         WHERE FileName = N'{file_name}' AND PartNo = 0\n\
         FOR JSON PATH;",
        db = quote_ident(database),
        file_name = quote_string(file_name),
    );
    let stdout = run_sql_capture(sqlcmd, server, &sql)?;
    let json = extract_json_array(&stdout, &format!("fetch_config_blob({file_name})"))?;
    let mut rows: Vec<BinaryBlobRow> = serde_json::from_str(&json)
        .with_context(|| format!("failed to parse Config blob JSON for {file_name}"))?;
    let row = rows
        .pop()
        .ok_or_else(|| anyhow!("Config row not found: {file_name}"))?;
    let bytes = decode_hex(&row.binary_hex)?;
    if bytes.len() != row.data_size as usize {
        return Err(anyhow!(
            "Config row {} DataSize {} does not match BinaryData length {}",
            row.file_name,
            row.data_size,
            bytes.len()
        ));
    }
    Ok(bytes)
}

fn database_exists(sqlcmd: &Path, server: &str, database: &str) -> Result<bool> {
    let sql = format!(
        "SET NOCOUNT ON; SELECT COUNT(*) FROM sys.databases WHERE name = N'{}';",
        quote_string(database)
    );
    let stdout = run_sql_capture(sqlcmd, server, &sql)?;
    Ok(first_i32(&stdout).unwrap_or_default() > 0)
}

fn by_table_name(tables: &[TableShape]) -> BTreeMap<String, &TableShape> {
    tables
        .iter()
        .map(|table| (table.table_name.clone(), table))
        .collect()
}

fn run_sql(sqlcmd: &Path, server: &str, sql: &str) -> Result<()> {
    let output = sqlcmd_command(sqlcmd, server, sql).output()?;
    if output.status.success() {
        return Ok(());
    }
    Err(anyhow!(
        "sqlcmd failed: {}",
        String::from_utf8_lossy(&output.stderr)
    ))
}

fn run_sql_capture(sqlcmd: &Path, server: &str, sql: &str) -> Result<String> {
    let output = sqlcmd_command(sqlcmd, server, sql).output()?;
    if !output.status.success() {
        return Err(anyhow!(
            "sqlcmd failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn run_sql_file(sqlcmd: &Path, server: &str, script: &Path) -> Result<()> {
    let output = sqlcmd_file_command(sqlcmd, server, script).output()?;
    if output.status.success() {
        return Ok(());
    }
    Err(anyhow!(
        "sqlcmd script failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ))
}

fn run_bcp_out(bcp: &Path, server: &str, database: &str, table: &str, output: &Path) -> Result<()> {
    let table_name = qualified_table(database, table);
    let command = bcp_command(bcp, &table_name, "out", output, server);
    run_bcp(command)
}

fn run_bcp_in(bcp: &Path, server: &str, database: &str, table: &str, input: &Path) -> Result<()> {
    let table_name = qualified_table(database, table);
    let command = bcp_command(bcp, &table_name, "in", input, server);
    run_bcp(command)
}

fn bcp_command(
    bcp: &Path,
    table_name: &str,
    direction: &str,
    file: &Path,
    server: &str,
) -> Command {
    let mut command = Command::new(bcp);
    command
        .arg(table_name)
        .arg(direction)
        .arg(file)
        .arg("-S")
        .arg(server)
        .arg("-T")
        .arg("-n")
        .arg("-u")
        .arg("-b")
        .arg("1000");
    command
}

fn run_bcp(mut command: Command) -> Result<()> {
    let output = command.output()?;
    if output.status.success() {
        return Ok(());
    }
    Err(anyhow!(
        "bcp failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ))
}

fn sqlcmd_command(sqlcmd: &Path, server: &str, sql: &str) -> Command {
    let mut command = Command::new(sqlcmd);
    command
        .arg("-S")
        .arg(server)
        .arg("-E")
        .arg("-C")
        .arg("-f")
        .arg("65001")
        .arg("-b")
        .arg("-w")
        .arg("65535")
        .arg("-y")
        .arg("0")
        .arg("-Y")
        .arg("0")
        .arg("-Q")
        .arg(sql);
    command
}

fn sqlcmd_file_command(sqlcmd: &Path, server: &str, script: &Path) -> Command {
    let mut command = Command::new(sqlcmd);
    command
        .arg("-S")
        .arg(server)
        .arg("-E")
        .arg("-C")
        .arg("-f")
        .arg("65001")
        .arg("-b")
        .arg("-w")
        .arg("65535")
        .arg("-y")
        .arg("0")
        .arg("-Y")
        .arg("0")
        .arg("-i")
        .arg(script);
    command
}

fn first_i32(stdout: &str) -> Option<i32> {
    stdout
        .lines()
        .map(str::trim)
        .find_map(|line| line.parse::<i32>().ok())
}

fn extract_json_array(stdout: &str, context: &str) -> Result<String> {
    let start = stdout.find('[').ok_or_else(|| {
        anyhow!(
            "{context}: sqlcmd output does not contain JSON array: {}",
            summarize_text(stdout)
        )
    })?;
    let end = stdout.rfind(']').ok_or_else(|| {
        anyhow!(
            "{context}: sqlcmd output does not contain JSON array end: {}",
            summarize_text(stdout)
        )
    })?;
    Ok(stdout[start..=end]
        .chars()
        .filter(|ch| !ch.is_control())
        .collect())
}

fn summarize_text(text: &str) -> String {
    let summary: String = text.chars().take(400).collect();
    summary.replace('\r', "\\r").replace('\n', "\\n")
}

fn write_storage_manifest(output_dir: &Path, manifest: &StorageBundleManifest) -> Result<()> {
    let json = serde_json::to_string_pretty(manifest)?;
    fs::write(output_dir.join("bundle.json"), json).with_context(|| {
        format!(
            "failed to write {}",
            output_dir.join("bundle.json").display()
        )
    })
}

fn write_delta_manifest(output_dir: &Path, manifest: &DeltaBundleManifest) -> Result<()> {
    let json = serde_json::to_string_pretty(manifest)?;
    fs::write(output_dir.join("delta.json"), json).with_context(|| {
        format!(
            "failed to write {}",
            output_dir.join("delta.json").display()
        )
    })
}

fn read_storage_manifest(input_dir: &Path) -> Result<StorageBundleManifest> {
    let path = input_dir.join("bundle.json");
    let text =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("failed to parse {}", path.display()))
}

fn read_delta_manifest(input_dir: &Path) -> Result<DeltaBundleManifest> {
    let path = input_dir.join("delta.json");
    let text =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("failed to parse {}", path.display()))
}

fn validate_storage_manifest(manifest: &StorageBundleManifest) -> Result<()> {
    if manifest.format != "mssql-native-bcp-v1" {
        return Err(anyhow!(
            "unsupported storage bundle format: {}",
            manifest.format
        ));
    }
    let names = manifest
        .tables
        .iter()
        .map(|table| table.table_name.as_str())
        .collect::<BTreeSet<_>>();
    for required in storage_tables() {
        if !names.contains(required) {
            return Err(anyhow!("bundle is missing required table {required}"));
        }
    }
    Ok(())
}

fn validate_delta_manifest(manifest: &DeltaBundleManifest) -> Result<()> {
    if manifest.format != "mssql-configsave-delta-v1" {
        return Err(anyhow!(
            "unsupported delta bundle format: {}",
            manifest.format
        ));
    }
    if manifest.table.table_name != "ConfigSave" {
        return Err(anyhow!(
            "delta bundle must contain ConfigSave, got {}",
            manifest.table.table_name
        ));
    }
    if manifest.table.file_name != "ConfigSave.bcp" {
        return Err(anyhow!(
            "delta bundle points to unexpected file {}",
            manifest.table.file_name
        ));
    }
    if manifest.table.row_count != manifest.rows.len() as i64 {
        return Err(anyhow!(
            "delta manifest row_count {} does not match digest rows {}",
            manifest.table.row_count,
            manifest.rows.len()
        ));
    }
    Ok(())
}

fn compare_storage_table_manifests(
    expected: &StorageTableManifest,
    actual: &StorageTableManifest,
) -> Result<()> {
    if expected.table_name != actual.table_name {
        return Err(anyhow!(
            "table name mismatch: {} vs {}",
            expected.table_name,
            actual.table_name
        ));
    }
    if expected.row_count != actual.row_count {
        return Err(anyhow!(
            "row count mismatch for {}: {} vs {}",
            expected.table_name,
            expected.row_count,
            actual.row_count
        ));
    }
    if expected.binary_bytes != actual.binary_bytes {
        return Err(anyhow!(
            "binary byte mismatch for {}: {} vs {}",
            expected.table_name,
            expected.binary_bytes,
            actual.binary_bytes
        ));
    }
    if let Some(expected_checksum) = expected.row_checksum {
        if actual.row_checksum != Some(expected_checksum) {
            return Err(anyhow!(
                "row checksum mismatch for {}: {:?} vs {:?}",
                expected.table_name,
                expected.row_checksum,
                actual.row_checksum
            ));
        }
    }
    Ok(())
}

fn compare_storage_bundle_tables(
    expected: &[StorageTableManifest],
    actual: &[StorageTableManifest],
) -> Result<()> {
    let expected_by_name = expected
        .iter()
        .map(|table| (table.table_name.as_str(), table))
        .collect::<BTreeMap<_, _>>();
    let actual_by_name = actual
        .iter()
        .map(|table| (table.table_name.as_str(), table))
        .collect::<BTreeMap<_, _>>();

    for (name, expected_table) in expected_by_name {
        let actual_table = actual_by_name
            .get(name)
            .ok_or_else(|| anyhow!("imported bundle is missing table {name}"))?;
        compare_storage_table_manifests(expected_table, actual_table)?;
    }

    Ok(())
}

fn storage_tables() -> [&'static str; 2] {
    ["ConfigSave", "Params"]
}

fn build_stage_common_modules_sql(
    database: &str,
    modules: &[PreparedCommonModuleStage],
    versions_blob: &[u8],
) -> String {
    let versions_blob_hex = encode_hex(versions_blob);
    let expected_stable_rows = modules.len() + 2;
    let expected_total_rows = modules.len() * 2 + 3;
    let module_ids = modules
        .iter()
        .map(|module| format!("N'{}'", quote_string(&module.spec.module_id)))
        .collect::<Vec<_>>()
        .join(", ");
    let mut sql = format!(
        "SET NOCOUNT ON;\n\
         SET XACT_ABORT ON;\n\
         USE {db};\n\
         BEGIN TRAN;\n\
         DELETE FROM ConfigSave;\n\
         INSERT INTO ConfigSave (FileName, Creation, Modified, Attributes, DataSize, BinaryData, PartNo)\n\
         SELECT FileName, SYSUTCDATETIME(), SYSUTCDATETIME(), Attributes, DataSize, BinaryData, PartNo\n\
         FROM Config\n\
         WHERE FileName IN (N'root', N'version'{module_filter}) AND PartNo = 0;\n\
         IF @@ROWCOUNT <> {expected_stable_rows} THROW 51000, 'Unexpected number of stable Config rows copied into ConfigSave', 1;\n",
        db = quote_ident(database),
        module_filter = if module_ids.is_empty() {
            String::new()
        } else {
            format!(", {module_ids}")
        },
        expected_stable_rows = expected_stable_rows,
    );

    for (index, module) in modules.iter().enumerate() {
        let module_blob_hex = encode_hex(&module.blob);
        let error_number = 51001 + index;
        sql.push_str(&format!(
            "INSERT INTO ConfigSave (FileName, Creation, Modified, Attributes, DataSize, BinaryData, PartNo)\n\
             SELECT N'{module_body_id}', SYSUTCDATETIME(), SYSUTCDATETIME(), Attributes, {module_blob_len}, 0x{module_blob_hex}, PartNo\n\
             FROM Config\n\
             WHERE FileName = N'{module_body_id}' AND PartNo = 0;\n\
             IF @@ROWCOUNT <> 1 THROW {error_number}, 'Expected to insert module body row into ConfigSave', 1;\n",
            module_body_id = quote_string(&module.module_body_id),
            module_blob_len = module.blob.len(),
            module_blob_hex = module_blob_hex,
            error_number = error_number,
        ));
    }

    sql.push_str(&format!(
        "INSERT INTO ConfigSave (FileName, Creation, Modified, Attributes, DataSize, BinaryData, PartNo)\n\
         SELECT N'versions', SYSUTCDATETIME(), SYSUTCDATETIME(), Attributes, {versions_blob_len}, 0x{versions_blob_hex}, PartNo\n\
         FROM Config\n\
         WHERE FileName = N'versions' AND PartNo = 0;\n\
         IF @@ROWCOUNT <> 1 THROW 51998, 'Expected to insert versions row into ConfigSave', 1;\n\
         IF (SELECT COUNT_BIG(*) FROM ConfigSave) <> {expected_total_rows} THROW 51999, 'Unexpected ConfigSave row count after staging', 1;\n\
         COMMIT;\n",
        versions_blob_len = versions_blob.len(),
        versions_blob_hex = versions_blob_hex,
        expected_total_rows = expected_total_rows,
    ));

    sql
}

fn build_stage_common_module_metadata_sql(
    database: &str,
    module_id: &str,
    metadata_blob: &[u8],
    versions_blob: &[u8],
) -> String {
    let metadata_blob_hex = encode_hex(metadata_blob);
    let versions_blob_hex = encode_hex(versions_blob);
    format!(
        "SET NOCOUNT ON;\n\
         SET XACT_ABORT ON;\n\
         USE {db};\n\
         BEGIN TRAN;\n\
         DELETE FROM ConfigSave;\n\
         INSERT INTO ConfigSave (FileName, Creation, Modified, Attributes, DataSize, BinaryData, PartNo)\n\
         SELECT FileName, SYSUTCDATETIME(), SYSUTCDATETIME(), Attributes, DataSize, BinaryData, PartNo\n\
         FROM Config\n\
         WHERE FileName IN (N'root', N'version') AND PartNo = 0;\n\
         IF @@ROWCOUNT <> 2 THROW 52000, 'Unexpected number of stable Config rows copied into ConfigSave', 1;\n\
         INSERT INTO ConfigSave (FileName, Creation, Modified, Attributes, DataSize, BinaryData, PartNo)\n\
         SELECT N'{module_id}', SYSUTCDATETIME(), SYSUTCDATETIME(), Attributes, {metadata_blob_len}, 0x{metadata_blob_hex}, PartNo\n\
         FROM Config\n\
         WHERE FileName = N'{module_id}' AND PartNo = 0;\n\
         IF @@ROWCOUNT <> 1 THROW 52001, 'Expected to insert common module metadata row into ConfigSave', 1;\n\
         INSERT INTO ConfigSave (FileName, Creation, Modified, Attributes, DataSize, BinaryData, PartNo)\n\
         SELECT N'versions', SYSUTCDATETIME(), SYSUTCDATETIME(), Attributes, {versions_blob_len}, 0x{versions_blob_hex}, PartNo\n\
         FROM Config\n\
         WHERE FileName = N'versions' AND PartNo = 0;\n\
         IF @@ROWCOUNT <> 1 THROW 52002, 'Expected to insert versions row into ConfigSave', 1;\n\
         IF (SELECT COUNT_BIG(*) FROM ConfigSave) <> 4 THROW 52003, 'Unexpected ConfigSave row count after metadata staging', 1;\n\
         COMMIT;\n",
        db = quote_ident(database),
        module_id = quote_string(module_id),
        metadata_blob_len = metadata_blob.len(),
        metadata_blob_hex = metadata_blob_hex,
        versions_blob_len = versions_blob.len(),
        versions_blob_hex = versions_blob_hex,
    )
}

fn build_stage_common_module_objects_sql(
    database: &str,
    modules: &[PreparedCommonModuleObjectStage],
    versions_blob: &[u8],
) -> String {
    let versions_blob_hex = encode_hex(versions_blob);
    let expected_total_rows = modules.len() * 2 + 3;
    let mut sql = format!(
        "SET NOCOUNT ON;\n\
         SET XACT_ABORT ON;\n\
         USE {db};\n\
         BEGIN TRAN;\n\
         DELETE FROM ConfigSave;\n\
         INSERT INTO ConfigSave (FileName, Creation, Modified, Attributes, DataSize, BinaryData, PartNo)\n\
         SELECT FileName, SYSUTCDATETIME(), SYSUTCDATETIME(), Attributes, DataSize, BinaryData, PartNo\n\
         FROM Config\n\
         WHERE FileName IN (N'root', N'version') AND PartNo = 0;\n\
         IF @@ROWCOUNT <> 2 THROW 53000, 'Unexpected number of stable Config rows copied into ConfigSave', 1;\n",
        db = quote_ident(database),
    );

    for (index, module) in modules.iter().enumerate() {
        let metadata_blob_hex = encode_hex(&module.metadata_blob);
        let module_blob_hex = encode_hex(&module.module_blob);
        let metadata_error = 53001 + index * 2;
        let body_error = metadata_error + 1;
        sql.push_str(&format!(
            "INSERT INTO ConfigSave (FileName, Creation, Modified, Attributes, DataSize, BinaryData, PartNo)\n\
             SELECT N'{module_id}', SYSUTCDATETIME(), SYSUTCDATETIME(), Attributes, {metadata_blob_len}, 0x{metadata_blob_hex}, PartNo\n\
             FROM Config\n\
             WHERE FileName = N'{module_id}' AND PartNo = 0;\n\
             IF @@ROWCOUNT <> 1 THROW {metadata_error}, 'Expected to insert common module metadata row into ConfigSave', 1;\n\
             INSERT INTO ConfigSave (FileName, Creation, Modified, Attributes, DataSize, BinaryData, PartNo)\n\
             SELECT N'{module_body_id}', SYSUTCDATETIME(), SYSUTCDATETIME(), Attributes, {module_blob_len}, 0x{module_blob_hex}, PartNo\n\
             FROM Config\n\
             WHERE FileName = N'{module_body_id}' AND PartNo = 0;\n\
             IF @@ROWCOUNT <> 1 THROW {body_error}, 'Expected to insert common module body row into ConfigSave', 1;\n",
            module_id = quote_string(&module.module_id),
            module_body_id = quote_string(&module.module_body_id),
            metadata_blob_len = module.metadata_blob.len(),
            metadata_blob_hex = metadata_blob_hex,
            module_blob_len = module.module_blob.len(),
            module_blob_hex = module_blob_hex,
            metadata_error = metadata_error,
            body_error = body_error,
        ));
    }

    sql.push_str(&format!(
        "INSERT INTO ConfigSave (FileName, Creation, Modified, Attributes, DataSize, BinaryData, PartNo)\n\
         SELECT N'versions', SYSUTCDATETIME(), SYSUTCDATETIME(), Attributes, {versions_blob_len}, 0x{versions_blob_hex}, PartNo\n\
         FROM Config\n\
         WHERE FileName = N'versions' AND PartNo = 0;\n\
         IF @@ROWCOUNT <> 1 THROW 53998, 'Expected to insert versions row into ConfigSave', 1;\n\
         IF (SELECT COUNT_BIG(*) FROM ConfigSave) <> {expected_total_rows} THROW 53999, 'Unexpected ConfigSave row count after common module object staging', 1;\n\
         COMMIT;\n",
        versions_blob_len = versions_blob.len(),
        versions_blob_hex = versions_blob_hex,
        expected_total_rows = expected_total_rows,
    ));

    sql
}

fn build_stage_metadata_objects_sql(
    database: &str,
    objects: &[PreparedMetadataObjectStage],
    versions_blob: &[u8],
) -> String {
    let versions_blob_hex = encode_hex(versions_blob);
    let body_row_count = objects
        .iter()
        .map(|object| object.body_rows.len())
        .sum::<usize>();
    let expected_total_rows = objects.len() + body_row_count + 3;
    let mut sql = format!(
        "SET NOCOUNT ON;\n\
         SET XACT_ABORT ON;\n\
         USE {db};\n\
         BEGIN TRAN;\n\
         DELETE FROM ConfigSave;\n\
         INSERT INTO ConfigSave (FileName, Creation, Modified, Attributes, DataSize, BinaryData, PartNo)\n\
         SELECT FileName, SYSUTCDATETIME(), SYSUTCDATETIME(), Attributes, DataSize, BinaryData, PartNo\n\
         FROM Config\n\
         WHERE FileName IN (N'root', N'version') AND PartNo = 0;\n\
         IF @@ROWCOUNT <> 2 THROW 54000, 'Unexpected number of stable Config rows copied into ConfigSave', 1;\n",
        db = quote_ident(database),
    );

    for (index, object) in objects.iter().enumerate() {
        let metadata_blob_hex = encode_hex(&object.metadata_blob);
        let error_number = 54001 + index;
        sql.push_str(&format!(
            "INSERT INTO ConfigSave (FileName, Creation, Modified, Attributes, DataSize, BinaryData, PartNo)\n\
             SELECT N'{object_id}', SYSUTCDATETIME(), SYSUTCDATETIME(), Attributes, {metadata_blob_len}, 0x{metadata_blob_hex}, PartNo\n\
             FROM Config\n\
             WHERE FileName = N'{object_id}' AND PartNo = 0;\n\
             IF @@ROWCOUNT <> 1 THROW {error_number}, 'Expected to insert metadata object row into ConfigSave', 1;\n",
            object_id = quote_string(&object.object_id),
            metadata_blob_len = object.metadata_blob.len(),
            metadata_blob_hex = metadata_blob_hex,
            error_number = error_number,
        ));
        for (body_index, body) in object.body_rows.iter().enumerate() {
            let body_blob_hex = encode_hex(&body.blob);
            let body_error_number = 54501 + index * 10 + body_index;
            sql.push_str(&format!(
                "INSERT INTO ConfigSave (FileName, Creation, Modified, Attributes, DataSize, BinaryData, PartNo)\n\
                 SELECT N'{body_id}', SYSUTCDATETIME(), SYSUTCDATETIME(), Attributes, {body_blob_len}, 0x{body_blob_hex}, PartNo\n\
                 FROM Config\n\
                 WHERE FileName = N'{body_id}' AND PartNo = 0;\n\
                 IF @@ROWCOUNT <> 1 THROW {body_error_number}, 'Expected to insert metadata body row into ConfigSave', 1;\n",
                body_id = quote_string(&body.body_id),
                body_blob_len = body.blob.len(),
                body_blob_hex = body_blob_hex,
                body_error_number = body_error_number,
            ));
        }
    }

    sql.push_str(&format!(
        "INSERT INTO ConfigSave (FileName, Creation, Modified, Attributes, DataSize, BinaryData, PartNo)\n\
         SELECT N'versions', SYSUTCDATETIME(), SYSUTCDATETIME(), Attributes, {versions_blob_len}, 0x{versions_blob_hex}, PartNo\n\
         FROM Config\n\
         WHERE FileName = N'versions' AND PartNo = 0;\n\
         IF @@ROWCOUNT <> 1 THROW 54998, 'Expected to insert versions row into ConfigSave', 1;\n\
         IF (SELECT COUNT_BIG(*) FROM ConfigSave) <> {expected_total_rows} THROW 54999, 'Unexpected ConfigSave row count after metadata object staging', 1;\n\
         COMMIT;\n",
        versions_blob_len = versions_blob.len(),
        versions_blob_hex = versions_blob_hex,
        expected_total_rows = expected_total_rows,
    ));

    sql
}

fn build_stage_source_objects_sql(
    database: &str,
    metadata_objects: &[PreparedMetadataObjectStage],
    common_modules: &[PreparedCommonModuleObjectStage],
    versions_blob: &[u8],
    include_stable_rows: bool,
    include_versions_row: bool,
    expected_total_rows: usize,
) -> String {
    let versions_blob_hex = encode_hex(versions_blob);
    let mut sql = format!(
        "SET NOCOUNT ON;\n\
         SET XACT_ABORT ON;\n\
         USE {db};\n\
         BEGIN TRAN;\n\
         DELETE FROM ConfigSave;\n\
         INSERT INTO ConfigSave (FileName, Creation, Modified, Attributes, DataSize, BinaryData, PartNo)\n\
         SELECT FileName, SYSUTCDATETIME(), SYSUTCDATETIME(), Attributes, DataSize, BinaryData, PartNo\n\
         FROM Config\n\
         WHERE FileName IN (N'root', N'version') AND PartNo = 0;\n\
         IF @@ROWCOUNT <> 2 THROW 55000, 'Unexpected number of stable Config rows copied into ConfigSave', 1;\n",
        db = quote_ident(database),
    );
    if !include_stable_rows {
        sql.clear();
        sql.push_str(&format!(
            "SET NOCOUNT ON;\n\
             SET XACT_ABORT ON;\n\
             USE {db};\n\
             BEGIN TRAN;\n",
            db = quote_ident(database),
        ));
    }

    for (index, object) in metadata_objects.iter().enumerate() {
        let metadata_blob_hex = encode_hex(&object.metadata_blob);
        let error_number = 55001 + index;
        sql.push_str(&format!(
            "INSERT INTO ConfigSave (FileName, Creation, Modified, Attributes, DataSize, BinaryData, PartNo)\n\
             SELECT N'{object_id}', SYSUTCDATETIME(), SYSUTCDATETIME(), Attributes, {metadata_blob_len}, 0x{metadata_blob_hex}, PartNo\n\
             FROM Config\n\
             WHERE FileName = N'{object_id}' AND PartNo = 0;\n\
             IF @@ROWCOUNT <> 1 THROW {error_number}, 'Expected to insert metadata object row into ConfigSave', 1;\n",
            object_id = quote_string(&object.object_id),
            metadata_blob_len = object.metadata_blob.len(),
            metadata_blob_hex = metadata_blob_hex,
            error_number = error_number,
        ));
        for (body_index, body) in object.body_rows.iter().enumerate() {
            let body_blob_hex = encode_hex(&body.blob);
            let body_error_number = 55501 + index * 10 + body_index;
            sql.push_str(&format!(
                "INSERT INTO ConfigSave (FileName, Creation, Modified, Attributes, DataSize, BinaryData, PartNo)\n\
                 SELECT N'{body_id}', SYSUTCDATETIME(), SYSUTCDATETIME(), Attributes, {body_blob_len}, 0x{body_blob_hex}, PartNo\n\
                 FROM Config\n\
                 WHERE FileName = N'{body_id}' AND PartNo = 0;\n\
                 IF @@ROWCOUNT <> 1 THROW {body_error_number}, 'Expected to insert metadata body row into ConfigSave', 1;\n",
                body_id = quote_string(&body.body_id),
                body_blob_len = body.blob.len(),
                body_blob_hex = body_blob_hex,
                body_error_number = body_error_number,
            ));
        }
    }

    for (index, module) in common_modules.iter().enumerate() {
        let metadata_blob_hex = encode_hex(&module.metadata_blob);
        let module_blob_hex = encode_hex(&module.module_blob);
        let metadata_error = 56001 + index * 2;
        let body_error = metadata_error + 1;
        sql.push_str(&format!(
            "INSERT INTO ConfigSave (FileName, Creation, Modified, Attributes, DataSize, BinaryData, PartNo)\n\
             SELECT N'{module_id}', SYSUTCDATETIME(), SYSUTCDATETIME(), Attributes, {metadata_blob_len}, 0x{metadata_blob_hex}, PartNo\n\
             FROM Config\n\
             WHERE FileName = N'{module_id}' AND PartNo = 0;\n\
             IF @@ROWCOUNT <> 1 THROW {metadata_error}, 'Expected to insert common module metadata row into ConfigSave', 1;\n\
             INSERT INTO ConfigSave (FileName, Creation, Modified, Attributes, DataSize, BinaryData, PartNo)\n\
             SELECT N'{module_body_id}', SYSUTCDATETIME(), SYSUTCDATETIME(), Attributes, {module_blob_len}, 0x{module_blob_hex}, PartNo\n\
             FROM Config\n\
             WHERE FileName = N'{module_body_id}' AND PartNo = 0;\n\
             IF @@ROWCOUNT <> 1 THROW {body_error}, 'Expected to insert common module body row into ConfigSave', 1;\n",
            module_id = quote_string(&module.module_id),
            module_body_id = quote_string(&module.module_body_id),
            metadata_blob_len = module.metadata_blob.len(),
            metadata_blob_hex = metadata_blob_hex,
            module_blob_len = module.module_blob.len(),
            module_blob_hex = module_blob_hex,
            metadata_error = metadata_error,
            body_error = body_error,
        ));
    }

    if include_versions_row {
        sql.push_str(&format!(
            "INSERT INTO ConfigSave (FileName, Creation, Modified, Attributes, DataSize, BinaryData, PartNo)\n\
             SELECT N'versions', SYSUTCDATETIME(), SYSUTCDATETIME(), Attributes, {versions_blob_len}, 0x{versions_blob_hex}, PartNo\n\
             FROM Config\n\
             WHERE FileName = N'versions' AND PartNo = 0;\n\
             IF @@ROWCOUNT <> 1 THROW 56998, 'Expected to insert versions row into ConfigSave', 1;\n",
            versions_blob_len = versions_blob.len(),
            versions_blob_hex = versions_blob_hex,
        ));
    }
    sql.push_str(&format!(
        "IF (SELECT COUNT_BIG(*) FROM ConfigSave) <> {expected_total_rows} THROW 56999, 'Unexpected ConfigSave row count after source tree staging', 1;\n\
         COMMIT;\n",
        expected_total_rows = expected_total_rows,
    ));

    sql
}

#[derive(Debug, Clone)]
struct SourceStageBatch {
    metadata_objects: Vec<PreparedMetadataObjectStage>,
    common_modules: Vec<PreparedCommonModuleObjectStage>,
    row_count: usize,
}

fn build_source_stage_batches(
    metadata_objects: Vec<PreparedMetadataObjectStage>,
    common_modules: Vec<PreparedCommonModuleObjectStage>,
    batch_size: usize,
) -> Vec<SourceStageBatch> {
    let mut items = metadata_objects
        .into_iter()
        .map(SourceStageItem::Metadata)
        .chain(
            common_modules
                .into_iter()
                .map(SourceStageItem::CommonModule),
        )
        .collect::<Vec<_>>();
    items.sort_by(|left, right| left.path().cmp(right.path()));

    let mut batches = Vec::new();
    let mut current = SourceStageBatch {
        metadata_objects: Vec::new(),
        common_modules: Vec::new(),
        row_count: 0,
    };
    let mut current_items = 0usize;

    for item in items {
        if current_items == batch_size {
            batches.push(current);
            current = SourceStageBatch {
                metadata_objects: Vec::new(),
                common_modules: Vec::new(),
                row_count: 0,
            };
            current_items = 0;
        }
        match item {
            SourceStageItem::Metadata(object) => {
                current.row_count += 1 + object.body_rows.len();
                current.metadata_objects.push(object);
            }
            SourceStageItem::CommonModule(module) => {
                current.row_count += 2;
                current.common_modules.push(module);
            }
        }
        current_items += 1;
    }

    if current_items > 0 {
        batches.push(current);
    }

    batches
}

#[derive(Debug, Clone)]
enum SourceStageItem {
    Metadata(PreparedMetadataObjectStage),
    CommonModule(PreparedCommonModuleObjectStage),
}

impl SourceStageItem {
    fn path(&self) -> &Path {
        match self {
            SourceStageItem::Metadata(object) => &object.xml,
            SourceStageItem::CommonModule(module) => &module.xml,
        }
    }
}

fn default_stage_script_path(database: &str, name: &str) -> PathBuf {
    PathBuf::from(format!(
        r"C:\temp\ibcmd-rs\stage_{}_{}.sql",
        sanitize_file_part(database),
        sanitize_file_part(name)
    ))
}

fn batch_stage_script_path(
    base: Option<&PathBuf>,
    database: &str,
    name: &str,
    batch_index: usize,
) -> PathBuf {
    if let Some(base) = base {
        let parent = base.parent().unwrap_or_else(|| Path::new(""));
        let stem = base
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or(name);
        let extension = base
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("sql");
        return parent.join(format!(
            "{stem}_batch{batch}.{}",
            extension,
            batch = batch_index + 1
        ));
    }
    default_stage_script_path(database, &format!("{name}_batch{}", batch_index + 1))
}

fn sanitize_file_part(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn infer_common_module_text_path(xml: &Path) -> PathBuf {
    let module_name = xml.file_stem().unwrap_or_default();
    xml.parent()
        .unwrap_or_else(|| Path::new(""))
        .join(module_name)
        .join("Ext")
        .join("Module.bsl")
}

fn infer_style_body_path(xml: &Path) -> PathBuf {
    let style_name = xml.file_stem().unwrap_or_default();
    xml.parent()
        .unwrap_or_else(|| Path::new(""))
        .join(style_name)
        .join("Ext")
        .join("Style.xml")
}

fn infer_common_picture_body_path(xml: &Path) -> PathBuf {
    let picture_name = xml.file_stem().unwrap_or_default();
    xml.parent()
        .unwrap_or_else(|| Path::new(""))
        .join(picture_name)
        .join("Ext")
        .join("Picture.xml")
}

fn infer_scheduled_job_schedule_path(xml: &Path) -> PathBuf {
    let job_name = xml.file_stem().unwrap_or_default();
    xml.parent()
        .unwrap_or_else(|| Path::new(""))
        .join(job_name)
        .join("Ext")
        .join("Schedule.xml")
}

fn infer_object_help_body_path(xml: &Path) -> PathBuf {
    xml.with_extension("").join("Ext").join("Help.xml")
}

fn infer_object_module_body_path(xml: &Path, file_name: &str) -> PathBuf {
    xml.with_extension("").join("Ext").join(file_name)
}

fn infer_configuration_module_body_path(xml: &Path, file_name: &str) -> PathBuf {
    xml.parent()
        .unwrap_or_else(|| Path::new(""))
        .join("Ext")
        .join(file_name)
}

fn infer_configuration_ext_body_path(xml: &Path, file_name: &str) -> PathBuf {
    xml.parent()
        .unwrap_or_else(|| Path::new(""))
        .join("Ext")
        .join(file_name)
}

fn infer_form_body_path(xml: &Path) -> PathBuf {
    xml.with_extension("").join("Ext").join("Form.xml")
}

fn infer_form_module_body_path(xml: &Path) -> PathBuf {
    infer_form_body_path(xml)
        .with_extension("")
        .join("Module.bsl")
}

fn infer_role_rights_body_path(xml: &Path) -> PathBuf {
    xml.with_extension("").join("Ext").join("Rights.xml")
}

fn infer_command_interface_body_path(xml: &Path) -> PathBuf {
    xml.with_extension("")
        .join("Ext")
        .join("CommandInterface.xml")
}

fn infer_exchange_plan_content_body_path(xml: &Path) -> PathBuf {
    xml.with_extension("").join("Ext").join("Content.xml")
}

fn infer_predefined_data_body_path(xml: &Path) -> PathBuf {
    xml.with_extension("").join("Ext").join("Predefined.xml")
}

fn infer_business_process_flowchart_body_path(xml: &Path) -> PathBuf {
    xml.with_extension("").join("Ext").join("Flowchart.xml")
}

fn infer_xdto_package_body_path(xml: &Path) -> PathBuf {
    let package_name = xml.file_stem().unwrap_or_default();
    xml.parent()
        .unwrap_or_else(|| Path::new(""))
        .join(package_name)
        .join("Ext")
        .join("Package.bin")
}

fn infer_ws_reference_definition_path(xml: &Path) -> PathBuf {
    let reference_name = xml.file_stem().unwrap_or_default();
    xml.parent()
        .unwrap_or_else(|| Path::new(""))
        .join(reference_name)
        .join("Ext")
        .join("WSDefinition.xml")
}

fn infer_raw_deflated_template_body_path(xml: &Path, template_type: &str) -> Option<PathBuf> {
    let file_name = match template_type {
        "DataCompositionAppearanceTemplate" => "Template.xml",
        "DataCompositionSchema" => "Template.xml",
        "GraphicalSchema" => "Template.xml",
        "TextDocument" => "Template.txt",
        _ => return None,
    };
    Some(xml.with_extension("").join("Ext").join(file_name))
}

fn infer_binary_template_body_path(xml: &Path) -> PathBuf {
    xml.with_extension("").join("Ext").join("Template.bin")
}

fn infer_html_template_body_path(xml: &Path) -> PathBuf {
    xml.with_extension("").join("Ext").join("Template.xml")
}

fn infer_spreadsheet_template_body_path(xml: &Path) -> PathBuf {
    xml.with_extension("").join("Ext").join("Template.xml")
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0F) as usize] as char);
    }
    output
}

fn decode_hex(value: &str) -> Result<Vec<u8>> {
    let bytes = value.as_bytes();
    if bytes.len() % 2 != 0 {
        return Err(anyhow!("hex string has odd length"));
    }
    let mut output = Vec::with_capacity(bytes.len() / 2);
    for pair in bytes.chunks_exact(2) {
        let high = hex_nibble(pair[0])?;
        let low = hex_nibble(pair[1])?;
        output.push((high << 4) | low);
    }
    Ok(output)
}

fn hex_nibble(value: u8) -> Result<u8> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err(anyhow!("invalid hex digit {}", value as char)),
    }
}

fn qualified_table(database: &str, table: &str) -> String {
    format!("{}.dbo.{}", quote_ident(database), quote_ident(table))
}

fn sibling_path(source: &str, file_name: &str) -> Result<String> {
    let parent = Path::new(source)
        .parent()
        .ok_or_else(|| anyhow!("cannot find parent path for {source}"))?;
    Ok(parent.join(file_name).to_string_lossy().to_string())
}

fn quote_ident(value: &str) -> String {
    format!("[{}]", value.replace(']', "]]"))
}

fn quote_string(value: &str) -> String {
    value.replace('\'', "''")
}

fn quote_string_path(path: &Path) -> String {
    quote_string(&path.to_string_lossy())
}

#[cfg(test)]
mod tests {
    use super::{
        ColumnShape, CommonModuleStageSpec, ConfigSaveRowDigest, DeltaBundleManifest,
        PreparedCommonModuleObjectStage, PreparedCommonModuleStage, PreparedMetadataBodyStage,
        PreparedMetadataObjectStage, StorageBundleManifest, StorageTableManifest, TableShape,
        compare_shapes, compare_storage_table_manifests, infer_common_module_text_path,
        is_root_common_module_xml, is_root_metadata_xml, is_stage_metadata_xml, quote_ident,
        quote_string, require_non_lab_confirmation, source_common_module_xmls,
        source_metadata_xmls, validate_delta_manifest, validate_storage_manifest,
    };
    use crate::module_blob::{
        CommonModuleXmlProperties, ReturnValuesReuse, SimpleMetadataXmlProperties,
    };
    use crate::source::{SourceFile, SourceKind, SourceManifest};
    use std::path::PathBuf;

    #[test]
    fn quotes_sql_identifier_and_string() {
        assert_eq!(quote_ident("a]b"), "[a]]b]");
        assert_eq!(quote_string("a'b"), "a''b");
    }

    #[test]
    fn selects_root_objects_from_scanned_source_tree() {
        let manifest = SourceManifest {
            root: PathBuf::from(r"C:\sources"),
            generated_at_unix: 0,
            files: vec![
                SourceFile {
                    path: "CommonModules/Foo.xml".to_string(),
                    size_bytes: 1,
                    sha256: "aa".to_string(),
                    kind: SourceKind::MetadataXml,
                    xml_root: Some("CommonModule".to_string()),
                    object_hint: Some("CommonModules/Foo".to_string()),
                },
                SourceFile {
                    path: "CommonModules/Foo/Ext/Module.bsl".to_string(),
                    size_bytes: 1,
                    sha256: "aa".to_string(),
                    kind: SourceKind::Module,
                    xml_root: None,
                    object_hint: Some("CommonModules/Foo".to_string()),
                },
                SourceFile {
                    path: "Bots/Notify.xml".to_string(),
                    size_bytes: 1,
                    sha256: "aa".to_string(),
                    kind: SourceKind::MetadataXml,
                    xml_root: Some("Bot".to_string()),
                    object_hint: Some("Bots/Notify".to_string()),
                },
                SourceFile {
                    path: "CommonModules/Foo/Ext/Module.xml".to_string(),
                    size_bytes: 1,
                    sha256: "aa".to_string(),
                    kind: SourceKind::MetadataXml,
                    xml_root: Some("CommonModule".to_string()),
                    object_hint: Some("CommonModules/Foo".to_string()),
                },
                SourceFile {
                    path: "Ext/CommandInterface.xml".to_string(),
                    size_bytes: 1,
                    sha256: "aa".to_string(),
                    kind: SourceKind::MetadataXml,
                    xml_root: Some("CommandInterface".to_string()),
                    object_hint: Some("Ext".to_string()),
                },
                SourceFile {
                    path: "Reports/Sales/Templates/Main.xml".to_string(),
                    size_bytes: 1,
                    sha256: "aa".to_string(),
                    kind: SourceKind::Template,
                    xml_root: Some("MetaDataObject".to_string()),
                    object_hint: Some("Reports/Sales".to_string()),
                },
                SourceFile {
                    path: "Reports/Sales/Templates/Main/Ext/Template.xml".to_string(),
                    size_bytes: 1,
                    sha256: "aa".to_string(),
                    kind: SourceKind::Template,
                    xml_root: Some("document".to_string()),
                    object_hint: Some("Reports/Sales".to_string()),
                },
                SourceFile {
                    path: "Catalogs/Products/Forms/ItemForm.xml".to_string(),
                    size_bytes: 1,
                    sha256: "aa".to_string(),
                    kind: SourceKind::Form,
                    xml_root: Some("MetaDataObject".to_string()),
                    object_hint: Some("Catalogs/Products".to_string()),
                },
                SourceFile {
                    path: "Catalogs/Products/Forms/ItemForm/Ext/Form.xml".to_string(),
                    size_bytes: 1,
                    sha256: "aa".to_string(),
                    kind: SourceKind::Form,
                    xml_root: Some("Form".to_string()),
                    object_hint: Some("Catalogs/Products".to_string()),
                },
                SourceFile {
                    path: "Configuration.xml".to_string(),
                    size_bytes: 1,
                    sha256: "aa".to_string(),
                    kind: SourceKind::ConfigurationRoot,
                    xml_root: Some("Configuration".to_string()),
                    object_hint: Some("Configuration".to_string()),
                },
            ],
        };

        let metadata = manifest
            .files
            .iter()
            .filter(|file| is_root_metadata_xml(&file.path))
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>();
        let modules = manifest
            .files
            .iter()
            .filter(|file| is_root_common_module_xml(&file.path))
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>();

        assert_eq!(metadata, vec!["Bots/Notify.xml"]);
        assert!(!is_root_metadata_xml("Ext/CommandInterface.xml"));
        assert!(is_stage_metadata_xml("Reports/Sales/Templates/Main.xml"));
        assert!(!is_stage_metadata_xml(
            "Reports/Sales/Templates/Main/Ext/Template.xml"
        ));
        assert!(is_stage_metadata_xml(
            "Catalogs/Products/Forms/ItemForm.xml"
        ));
        assert!(!is_stage_metadata_xml(
            "Catalogs/Products/Forms/ItemForm/Ext/Form.xml"
        ));
        assert!(is_stage_metadata_xml("Configuration.xml"));
        assert_eq!(modules, vec!["CommonModules/Foo.xml"]);
    }

    #[test]
    fn selects_source_tree_stage_candidates() {
        let manifest = SourceManifest {
            root: PathBuf::from(r"C:\sources"),
            generated_at_unix: 0,
            files: vec![
                SourceFile {
                    path: "Bots/Notify.xml".to_string(),
                    size_bytes: 1,
                    sha256: "aa".to_string(),
                    kind: SourceKind::MetadataXml,
                    xml_root: Some("Bot".to_string()),
                    object_hint: Some("Bots/Notify".to_string()),
                },
                SourceFile {
                    path: "Bots/Notify/Ext/Module.bsl".to_string(),
                    size_bytes: 1,
                    sha256: "aa".to_string(),
                    kind: SourceKind::Module,
                    xml_root: None,
                    object_hint: Some("Bots/Notify".to_string()),
                },
                SourceFile {
                    path: "CommonModules/Foo.xml".to_string(),
                    size_bytes: 1,
                    sha256: "aa".to_string(),
                    kind: SourceKind::MetadataXml,
                    xml_root: Some("CommonModule".to_string()),
                    object_hint: Some("CommonModules/Foo".to_string()),
                },
                SourceFile {
                    path: "CommonModules/Foo/Ext/Module.bsl".to_string(),
                    size_bytes: 1,
                    sha256: "aa".to_string(),
                    kind: SourceKind::Module,
                    xml_root: None,
                    object_hint: Some("CommonModules/Foo".to_string()),
                },
                SourceFile {
                    path: "CommonModules/Foo/Ext/Module.xml".to_string(),
                    size_bytes: 1,
                    sha256: "aa".to_string(),
                    kind: SourceKind::MetadataXml,
                    xml_root: Some("CommonModule".to_string()),
                    object_hint: Some("CommonModules/Foo".to_string()),
                },
                SourceFile {
                    path: "Styles/Theme.xml".to_string(),
                    size_bytes: 1,
                    sha256: "aa".to_string(),
                    kind: SourceKind::MetadataXml,
                    xml_root: Some("Style".to_string()),
                    object_hint: Some("Styles/Theme".to_string()),
                },
                SourceFile {
                    path: "DataProcessors/Import/Templates/Schema.xml".to_string(),
                    size_bytes: 1,
                    sha256: "aa".to_string(),
                    kind: SourceKind::Template,
                    xml_root: Some("MetaDataObject".to_string()),
                    object_hint: Some("DataProcessors/Import".to_string()),
                },
                SourceFile {
                    path: "DataProcessors/Import/Templates/Schema/Ext/Template.xml".to_string(),
                    size_bytes: 1,
                    sha256: "aa".to_string(),
                    kind: SourceKind::Template,
                    xml_root: Some("document".to_string()),
                    object_hint: Some("DataProcessors/Import".to_string()),
                },
                SourceFile {
                    path: "Catalogs/Products/Forms/ItemForm.xml".to_string(),
                    size_bytes: 1,
                    sha256: "aa".to_string(),
                    kind: SourceKind::Form,
                    xml_root: Some("MetaDataObject".to_string()),
                    object_hint: Some("Catalogs/Products".to_string()),
                },
                SourceFile {
                    path: "Catalogs/Products/Forms/ItemForm/Ext/Form.xml".to_string(),
                    size_bytes: 1,
                    sha256: "aa".to_string(),
                    kind: SourceKind::Form,
                    xml_root: Some("Form".to_string()),
                    object_hint: Some("Catalogs/Products".to_string()),
                },
                SourceFile {
                    path: "Configuration.xml".to_string(),
                    size_bytes: 1,
                    sha256: "aa".to_string(),
                    kind: SourceKind::ConfigurationRoot,
                    xml_root: Some("Configuration".to_string()),
                    object_hint: Some("Configuration".to_string()),
                },
            ],
        };

        let metadata_xmls = source_metadata_xmls(&manifest, std::path::Path::new(r"C:\sources"));
        let common_module_xmls =
            source_common_module_xmls(&manifest, std::path::Path::new(r"C:\sources"));

        assert_eq!(
            metadata_xmls
                .iter()
                .map(|path| path.to_string_lossy().replace('\\', "/"))
                .collect::<Vec<_>>(),
            vec![
                "C:/sources/Bots/Notify.xml",
                "C:/sources/Styles/Theme.xml",
                "C:/sources/DataProcessors/Import/Templates/Schema.xml",
                "C:/sources/Catalogs/Products/Forms/ItemForm.xml",
                "C:/sources/Configuration.xml"
            ]
        );
        assert_eq!(
            common_module_xmls
                .iter()
                .map(|path| path.to_string_lossy().replace('\\', "/"))
                .collect::<Vec<_>>(),
            vec!["C:/sources/CommonModules/Foo.xml"]
        );
    }

    #[test]
    fn compare_detects_same_shapes() {
        let table = TableShape {
            table_name: "Config".to_string(),
            row_count: 1,
            row_checksum: Some(42),
            columns: vec![ColumnShape {
                name: "FileName".to_string(),
                type_name: "nvarchar".to_string(),
                max_length: 256,
                precision: 0,
                scale: 0,
                is_nullable: false,
            }],
        };

        let report = compare_shapes("left", "right", &[table.clone()], &[table]);
        assert!(report.same);
        assert_eq!(report.summary.left_tables, 1);
        assert_eq!(report.summary.right_tables, 1);
    }

    #[test]
    fn compare_detects_checksum_differences() {
        let left = TableShape {
            table_name: "Config".to_string(),
            row_count: 1,
            row_checksum: Some(1),
            columns: vec![],
        };
        let right = TableShape {
            table_name: "Config".to_string(),
            row_count: 1,
            row_checksum: Some(2),
            columns: vec![],
        };

        let report = compare_shapes("left", "right", &[left], &[right]);
        assert!(!report.same);
        assert_eq!(report.summary.checksum_differences, 1);
        assert!(
            report
                .differences
                .iter()
                .any(|difference| difference.kind == "checksum")
        );
    }

    #[test]
    fn compare_storage_table_manifests_checks_row_counts_and_checksums() {
        let expected = StorageTableManifest {
            table_name: "ConfigSave".to_string(),
            file_name: "ConfigSave.bcp".to_string(),
            row_count: 2,
            binary_bytes: 128,
            row_checksum: Some(42),
        };
        let actual = StorageTableManifest {
            table_name: "ConfigSave".to_string(),
            file_name: "ConfigSave.bcp".to_string(),
            row_count: 2,
            binary_bytes: 128,
            row_checksum: Some(42),
        };

        compare_storage_table_manifests(&expected, &actual).unwrap();
    }

    #[test]
    fn compare_storage_table_manifests_rejects_checksum_differences() {
        let expected = StorageTableManifest {
            table_name: "ConfigSave".to_string(),
            file_name: "ConfigSave.bcp".to_string(),
            row_count: 2,
            binary_bytes: 128,
            row_checksum: Some(42),
        };
        let actual = StorageTableManifest {
            table_name: "ConfigSave".to_string(),
            file_name: "ConfigSave.bcp".to_string(),
            row_count: 2,
            binary_bytes: 128,
            row_checksum: Some(41),
        };

        let error = compare_storage_table_manifests(&expected, &actual).unwrap_err();
        assert!(error.to_string().contains("row checksum mismatch"));
    }

    #[test]
    fn validate_storage_manifest_accepts_checksum_backed_bundles() {
        let manifest = StorageBundleManifest {
            source_database: Some("Demo".to_string()),
            format: "mssql-native-bcp-v1".to_string(),
            tables: vec![
                StorageTableManifest {
                    table_name: "ConfigSave".to_string(),
                    file_name: "ConfigSave.bcp".to_string(),
                    row_count: 2,
                    binary_bytes: 128,
                    row_checksum: Some(42),
                },
                StorageTableManifest {
                    table_name: "Params".to_string(),
                    file_name: "Params.bcp".to_string(),
                    row_count: 1,
                    binary_bytes: 64,
                    row_checksum: Some(7),
                },
            ],
        };

        validate_storage_manifest(&manifest).unwrap();
    }

    #[test]
    fn validate_storage_manifest_rejects_missing_required_tables() {
        let manifest = StorageBundleManifest {
            source_database: None,
            format: "mssql-native-bcp-v1".to_string(),
            tables: vec![StorageTableManifest {
                table_name: "ConfigSave".to_string(),
                file_name: "ConfigSave.bcp".to_string(),
                row_count: 2,
                binary_bytes: 128,
                row_checksum: Some(42),
            }],
        };

        let error = validate_storage_manifest(&manifest).unwrap_err();
        assert!(error.to_string().contains("missing required table Params"));
    }

    #[test]
    fn validate_delta_manifest_accepts_row_digests_and_checksum() {
        let manifest = DeltaBundleManifest {
            source_database: Some("Demo".to_string()),
            format: "mssql-configsave-delta-v1".to_string(),
            table: StorageTableManifest {
                table_name: "ConfigSave".to_string(),
                file_name: "ConfigSave.bcp".to_string(),
                row_count: 2,
                binary_bytes: 128,
                row_checksum: Some(42),
            },
            rows: vec![
                ConfigSaveRowDigest {
                    file_name: "root".to_string(),
                    part_no: 0,
                    data_size: 64,
                    binary_bytes: 64,
                    sha256: "aa".repeat(32),
                },
                ConfigSaveRowDigest {
                    file_name: "version".to_string(),
                    part_no: 0,
                    data_size: 64,
                    binary_bytes: 64,
                    sha256: "bb".repeat(32),
                },
            ],
        };

        validate_delta_manifest(&manifest).unwrap();
    }

    #[test]
    fn validate_delta_manifest_rejects_row_count_mismatches() {
        let manifest = DeltaBundleManifest {
            source_database: None,
            format: "mssql-configsave-delta-v1".to_string(),
            table: StorageTableManifest {
                table_name: "ConfigSave".to_string(),
                file_name: "ConfigSave.bcp".to_string(),
                row_count: 3,
                binary_bytes: 128,
                row_checksum: Some(42),
            },
            rows: vec![ConfigSaveRowDigest {
                file_name: "root".to_string(),
                part_no: 0,
                data_size: 64,
                binary_bytes: 64,
                sha256: "aa".repeat(32),
            }],
        };

        let error = validate_delta_manifest(&manifest).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("delta manifest row_count 3 does not match digest rows 1")
        );
    }

    #[test]
    fn infers_common_module_text_path_from_xml_path() {
        assert_eq!(
            infer_common_module_text_path(r"CommonModules\РаботаСБанкамиВызовСервера.xml".as_ref()),
            std::path::PathBuf::from(r"CommonModules\РаботаСБанкамиВызовСервера\Ext\Module.bsl")
        );
    }

    #[test]
    fn infers_raw_deflated_metadata_body_paths() {
        assert_eq!(
            super::infer_common_picture_body_path(r"CommonPictures\Address.xml".as_ref()),
            std::path::PathBuf::from(r"CommonPictures\Address\Ext\Picture.xml")
        );
        assert_eq!(
            super::infer_object_help_body_path(r"Catalogs\Products.xml".as_ref()),
            std::path::PathBuf::from(r"Catalogs\Products\Ext\Help.xml")
        );
        assert_eq!(
            super::infer_xdto_package_body_path(r"XDTOPackages\Exchange.xml".as_ref()),
            std::path::PathBuf::from(r"XDTOPackages\Exchange\Ext\Package.bin")
        );
        assert_eq!(
            super::infer_ws_reference_definition_path(r"WSReferences\UpdateFiles.xml".as_ref()),
            std::path::PathBuf::from(r"WSReferences\UpdateFiles\Ext\WSDefinition.xml")
        );
    }

    #[test]
    fn infers_help_body_ids_for_objects_and_forms() {
        let object = SimpleMetadataXmlProperties {
            kind: "Catalog".to_string(),
            uuid: "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa".to_string(),
            name: "Products".to_string(),
            synonyms: Vec::new(),
            comment: String::new(),
        };
        let form = SimpleMetadataXmlProperties {
            kind: "Form".to_string(),
            uuid: "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb".to_string(),
            name: "ItemForm".to_string(),
            synonyms: Vec::new(),
            comment: String::new(),
        };
        let common_form = SimpleMetadataXmlProperties {
            kind: "CommonForm".to_string(),
            uuid: "cccccccc-cccc-4ccc-cccc-cccccccccccc".to_string(),
            name: "SharedForm".to_string(),
            synonyms: Vec::new(),
            comment: String::new(),
        };

        assert_eq!(
            super::infer_help_body_id(&object),
            "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa.5"
        );
        assert_eq!(
            super::infer_help_body_id(&form),
            "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb.1"
        );
        assert_eq!(
            super::infer_help_body_id(&common_form),
            "cccccccc-cccc-4ccc-cccc-cccccccccccc.1"
        );
    }

    #[test]
    fn maps_object_module_body_suffixes_for_load() {
        assert_eq!(
            super::object_module_body_suffixes("Catalog"),
            &[("0", "ObjectModule.bsl"), ("3", "ManagerModule.bsl")]
        );
        assert_eq!(
            super::object_module_body_suffixes("InformationRegister"),
            &[("1", "RecordSetModule.bsl"), ("2", "ManagerModule.bsl")]
        );
        assert_eq!(
            super::object_module_body_suffixes("Constant"),
            &[("0", "ValueManagerModule.bsl"), ("1", "ManagerModule.bsl")]
        );
        assert_eq!(
            super::object_module_body_suffixes("CommonCommand"),
            &[("2", "CommandModule.bsl")]
        );
        assert_eq!(
            super::object_module_body_suffixes("IntegrationService"),
            &[("0", "Module.bsl")]
        );
        assert_eq!(
            super::object_module_body_suffixes("HTTPService"),
            &[("0", "Module.bsl")]
        );
        assert_eq!(
            super::object_module_body_suffixes("WebService"),
            &[("0", "Module.bsl")]
        );
        assert_eq!(
            super::object_module_body_suffixes("Configuration"),
            &[
                ("0", "OrdinaryApplicationModule.bsl"),
                ("5", "ExternalConnectionModule.bsl"),
                ("6", "ManagedApplicationModule.bsl"),
                ("7", "SessionModule.bsl")
            ]
        );
        assert!(super::object_module_body_suffixes("Role").is_empty());
    }

    #[test]
    fn infers_object_module_body_paths() {
        assert_eq!(
            super::infer_object_module_body_path(
                r"Catalogs\Products.xml".as_ref(),
                "ObjectModule.bsl"
            ),
            std::path::PathBuf::from(r"Catalogs\Products\Ext\ObjectModule.bsl")
        );
        assert_eq!(
            super::infer_object_module_body_path(
                r"InformationRegisters\Prices.xml".as_ref(),
                "RecordSetModule.bsl"
            ),
            std::path::PathBuf::from(r"InformationRegisters\Prices\Ext\RecordSetModule.bsl")
        );
        assert_eq!(
            super::infer_configuration_module_body_path(
                r"Configuration.xml".as_ref(),
                "ManagedApplicationModule.bsl"
            ),
            std::path::PathBuf::from(r"Ext\ManagedApplicationModule.bsl")
        );
    }

    #[test]
    fn infers_configuration_ext_body_paths() {
        assert_eq!(
            super::infer_configuration_ext_body_path(r"Configuration.xml".as_ref(), "Splash.xml"),
            std::path::PathBuf::from(r"Ext\Splash.xml")
        );
        assert_eq!(
            super::infer_configuration_ext_body_path(
                r"Configuration.xml".as_ref(),
                "ParentConfigurations.bin"
            ),
            std::path::PathBuf::from(r"Ext\ParentConfigurations.bin")
        );
    }

    #[test]
    fn infers_form_body_paths() {
        assert_eq!(
            super::infer_form_body_path(r"Catalogs\Products\Forms\ItemForm.xml".as_ref()),
            std::path::PathBuf::from(r"Catalogs\Products\Forms\ItemForm\Ext\Form.xml")
        );
        assert_eq!(
            super::infer_form_module_body_path(r"CommonForms\SharedForm.xml".as_ref()),
            std::path::PathBuf::from(r"CommonForms\SharedForm\Ext\Form\Module.bsl")
        );
    }

    #[test]
    fn infers_role_rights_body_path() {
        assert_eq!(
            super::infer_role_rights_body_path(r"Roles\Editor.xml".as_ref()),
            std::path::PathBuf::from(r"Roles\Editor\Ext\Rights.xml")
        );
    }

    #[test]
    fn infers_command_interface_body_path_and_suffix() {
        assert_eq!(
            super::infer_command_interface_body_path(r"Subsystems\Admin.xml".as_ref()),
            std::path::PathBuf::from(r"Subsystems\Admin\Ext\CommandInterface.xml")
        );
        assert_eq!(super::command_interface_body_suffix("Subsystem"), Some("1"));
        assert_eq!(super::command_interface_body_suffix("Catalog"), None);
    }

    #[test]
    fn infers_exchange_plan_content_body_path() {
        assert_eq!(
            super::infer_exchange_plan_content_body_path(r"ExchangePlans\Sync.xml".as_ref()),
            std::path::PathBuf::from(r"ExchangePlans\Sync\Ext\Content.xml")
        );
    }

    #[test]
    fn infers_predefined_data_body_path_and_suffix() {
        assert_eq!(
            super::infer_predefined_data_body_path(r"Catalogs\Products.xml".as_ref()),
            std::path::PathBuf::from(r"Catalogs\Products\Ext\Predefined.xml")
        );
        assert_eq!(super::predefined_data_body_suffix("Catalog"), Some("1c"));
        assert_eq!(
            super::predefined_data_body_suffix("ChartOfCharacteristicTypes"),
            Some("7")
        );
        assert_eq!(super::predefined_data_body_suffix("Document"), None);
    }

    #[test]
    fn infers_business_process_flowchart_body_path() {
        assert_eq!(
            super::infer_business_process_flowchart_body_path(
                r"BusinessProcesses\Approval.xml".as_ref()
            ),
            std::path::PathBuf::from(r"BusinessProcesses\Approval\Ext\Flowchart.xml")
        );
    }

    #[test]
    fn finds_nested_command_module_sources_from_owner_xml() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-nested-command-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        let xml_path = root.join("DataProcessors/Scanning.xml");
        let module_path =
            root.join("DataProcessors/Scanning/Commands/ScanSheet/Ext/CommandModule.bsl");
        std::fs::create_dir_all(module_path.parent().unwrap()).unwrap();
        std::fs::write(&module_path, "Procedure Run()\nEndProcedure\n").unwrap();

        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.21">
  <DataProcessor uuid="aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa">
    <Properties>
      <Name>Scanning</Name>
    </Properties>
    <ChildObjects>
      <Command uuid="bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb">
        <Properties>
          <Name>ScanSheet</Name>
        </Properties>
      </Command>
    </ChildObjects>
  </DataProcessor>
</MetaDataObject>
"#;
        std::fs::create_dir_all(xml_path.parent().unwrap()).unwrap();
        std::fs::write(&xml_path, xml).unwrap();
        let properties = SimpleMetadataXmlProperties {
            kind: "DataProcessor".to_string(),
            uuid: "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa".to_string(),
            name: "Scanning".to_string(),
            synonyms: Vec::new(),
            comment: String::new(),
        };

        let sources = super::nested_command_module_sources(&xml_path, xml, &properties).unwrap();

        assert_eq!(
            sources,
            vec![super::NestedCommandModuleSource {
                command_id: "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb".to_string(),
                command_name: "ScanSheet".to_string(),
                body_path: module_path,
            }]
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn parses_nested_command_ids_by_name() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.21">
  <Task uuid="aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa">
    <ChildObjects>
      <Command uuid="BBBBBBBB-BBBB-4BBB-BBBB-BBBBBBBBBBBB">
        <Properties>
          <Name>ВсеЗадачи</Name>
        </Properties>
      </Command>
    </ChildObjects>
  </Task>
</MetaDataObject>
"#
        .as_bytes();

        let commands = super::parse_nested_command_ids_by_name(xml).unwrap();

        assert_eq!(
            commands.get("ВсеЗадачи").map(String::as_str),
            Some("bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb")
        );
    }

    #[test]
    fn infers_raw_deflated_template_body_paths() {
        assert_eq!(
            super::infer_raw_deflated_template_body_path(
                r"CommonTemplates\SharedText.xml".as_ref(),
                "TextDocument"
            ),
            Some(std::path::PathBuf::from(
                r"CommonTemplates\SharedText\Ext\Template.txt"
            ))
        );
        assert_eq!(
            super::infer_raw_deflated_template_body_path(
                r"DataProcessors\ImportData\Templates\Schema.xml".as_ref(),
                "DataCompositionSchema"
            ),
            Some(std::path::PathBuf::from(
                r"DataProcessors\ImportData\Templates\Schema\Ext\Template.xml"
            ))
        );
        assert_eq!(
            super::infer_raw_deflated_template_body_path(
                r"CommonTemplates\ReportAppearance.xml".as_ref(),
                "DataCompositionAppearanceTemplate"
            ),
            Some(std::path::PathBuf::from(
                r"CommonTemplates\ReportAppearance\Ext\Template.xml"
            ))
        );
        assert_eq!(
            super::infer_raw_deflated_template_body_path(
                r"DataProcessors\Routes\Templates\RouteSchema.xml".as_ref(),
                "GraphicalSchema"
            ),
            Some(std::path::PathBuf::from(
                r"DataProcessors\Routes\Templates\RouteSchema\Ext\Template.xml"
            ))
        );
        assert_eq!(
            super::infer_raw_deflated_template_body_path(
                r"CommonTemplates\Table.xml".as_ref(),
                "SpreadsheetDocument"
            ),
            None
        );
        assert_eq!(
            super::infer_spreadsheet_template_body_path(r"CommonTemplates\Table.xml".as_ref()),
            std::path::PathBuf::from(r"CommonTemplates\Table\Ext\Template.xml")
        );
        assert_eq!(
            super::infer_binary_template_body_path(r"CommonTemplates\Archive.xml".as_ref()),
            std::path::PathBuf::from(r"CommonTemplates\Archive\Ext\Template.bin")
        );
        assert_eq!(
            super::infer_html_template_body_path(
                r"Catalogs\Products\Templates\Description.xml".as_ref()
            ),
            std::path::PathBuf::from(r"Catalogs\Products\Templates\Description\Ext\Template.xml")
        );
    }

    #[test]
    fn defaults_stage_script_path_with_sanitized_parts() {
        let path = super::default_stage_script_path("Test Db]", "common modules/2");

        assert_eq!(
            path,
            PathBuf::from(r"C:\temp\ibcmd-rs\stage_Test_Db__common_modules_2.sql")
        );
    }

    #[test]
    fn sanitizes_file_parts_for_paths() {
        assert_eq!(
            super::sanitize_file_part("Db name[]/2026"),
            "Db_name___2026"
        );
        assert_eq!(super::sanitize_file_part("safe-name_1"), "safe-name_1");
    }

    #[test]
    fn derives_sibling_path_from_source_parent() {
        let path = super::sibling_path(r"C:\temp\source\db.mdf", "target.mdf").unwrap();

        assert_eq!(path, r"C:\temp\source\target.mdf");
    }

    #[test]
    fn qualifies_table_names_with_database() {
        assert_eq!(
            super::qualified_table("Test Db]", "ConfigSave"),
            "[Test Db]]].dbo.[ConfigSave]"
        );
    }

    #[test]
    fn quotes_paths_for_sql_string_literals() {
        let path = PathBuf::from(r"C:\temp\O'Brien\backup.bak");

        assert_eq!(
            super::quote_string_path(&path),
            r"C:\temp\O''Brien\backup.bak"
        );
    }

    #[test]
    fn builds_metadata_object_stage_sql_with_expected_row_counts() {
        let prepared = vec![PreparedMetadataObjectStage {
            object_id: "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa".to_string(),
            kind: "CommonPicture".to_string(),
            xml: PathBuf::from("CommonPictures/TestPicture.xml"),
            properties: SimpleMetadataXmlProperties {
                kind: "CommonPicture".to_string(),
                uuid: "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa".to_string(),
                name: "TestPicture".to_string(),
                synonyms: Vec::new(),
                comment: String::new(),
            },
            metadata_plain_bytes: 12,
            metadata_blob: vec![0x01, 0x23, 0x45],
            metadata_blob_sha256: "deadbeef".to_string(),
            body_rows: Vec::new(),
        }];

        let sql = super::build_stage_metadata_objects_sql("TestDb", &prepared, &[0xAA, 0xBB]);

        assert!(sql.contains("USE [TestDb];"));
        assert!(sql.contains("IF @@ROWCOUNT <> 2 THROW 54000"));
        assert!(sql.contains("N'aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa'"));
        assert!(sql.contains("0x012345"));
        assert!(sql.contains("0xAABB"));
        assert!(sql.contains("IF (SELECT COUNT_BIG(*) FROM ConfigSave) <> 4"));
    }

    #[test]
    fn builds_metadata_object_stage_sql_with_body_rows() {
        let prepared = vec![PreparedMetadataObjectStage {
            object_id: "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa".to_string(),
            kind: "Style".to_string(),
            xml: PathBuf::from("Styles/Main.xml"),
            properties: SimpleMetadataXmlProperties {
                kind: "Style".to_string(),
                uuid: "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa".to_string(),
                name: "Main".to_string(),
                synonyms: Vec::new(),
                comment: String::new(),
            },
            metadata_plain_bytes: 12,
            metadata_blob: vec![0x01, 0x23, 0x45],
            metadata_blob_sha256: "deadbeef".to_string(),
            body_rows: vec![PreparedMetadataBodyStage {
                body_id: "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa.0".to_string(),
                path: PathBuf::from("Styles/Main/Ext/Style.xml"),
                blob: vec![0xAA, 0xBB, 0xCC],
                blob_sha256: "feedface".to_string(),
            }],
        }];

        let sql = super::build_stage_metadata_objects_sql("TestDb", &prepared, &[0xDD, 0xEE]);

        assert!(sql.contains("N'aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa'"));
        assert!(sql.contains("N'aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa.0'"));
        assert!(sql.contains("0x012345"));
        assert!(sql.contains("0xAABBCC"));
        assert!(sql.contains("IF @@ROWCOUNT <> 1 THROW 54501"));
        assert!(sql.contains("IF (SELECT COUNT_BIG(*) FROM ConfigSave) <> 5"));
    }

    #[test]
    fn builds_source_tree_stage_sql_with_metadata_body_rows() {
        let metadata = vec![PreparedMetadataObjectStage {
            object_id: "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa".to_string(),
            kind: "Style".to_string(),
            xml: PathBuf::from("Styles/Main.xml"),
            properties: SimpleMetadataXmlProperties {
                kind: "Style".to_string(),
                uuid: "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa".to_string(),
                name: "Main".to_string(),
                synonyms: Vec::new(),
                comment: String::new(),
            },
            metadata_plain_bytes: 12,
            metadata_blob: vec![0x01, 0x23, 0x45],
            metadata_blob_sha256: "deadbeef".to_string(),
            body_rows: vec![PreparedMetadataBodyStage {
                body_id: "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa.0".to_string(),
                path: PathBuf::from("Styles/Main/Ext/Style.xml"),
                blob: vec![0xAA, 0xBB, 0xCC],
                blob_sha256: "feedface".to_string(),
            }],
        }];

        let sql = super::build_stage_source_objects_sql(
            "TestDb",
            &metadata,
            &[],
            &[0xDD, 0xEE],
            true,
            true,
            5,
        );

        assert!(sql.contains("N'aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa.0'"));
        assert!(sql.contains("0xAABBCC"));
        assert!(sql.contains("IF @@ROWCOUNT <> 1 THROW 55501"));
        assert!(sql.contains("IF (SELECT COUNT_BIG(*) FROM ConfigSave) <> 5"));
    }

    #[test]
    fn builds_source_tree_stage_sql_with_scheduled_job_schedule_body_row() {
        let metadata = vec![PreparedMetadataObjectStage {
            object_id: "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa".to_string(),
            kind: "ScheduledJob".to_string(),
            xml: PathBuf::from("ScheduledJobs/LoadRates.xml"),
            properties: SimpleMetadataXmlProperties {
                kind: "ScheduledJob".to_string(),
                uuid: "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa".to_string(),
                name: "LoadRates".to_string(),
                synonyms: Vec::new(),
                comment: String::new(),
            },
            metadata_plain_bytes: 12,
            metadata_blob: vec![0x01, 0x23, 0x45],
            metadata_blob_sha256: "deadbeef".to_string(),
            body_rows: vec![PreparedMetadataBodyStage {
                body_id: "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa.0".to_string(),
                path: PathBuf::from("ScheduledJobs/LoadRates/Ext/Schedule.xml"),
                blob: vec![0x10, 0x20, 0x30],
                blob_sha256: "feedface".to_string(),
            }],
        }];

        let sql = super::build_stage_source_objects_sql(
            "TestDb",
            &metadata,
            &[],
            &[0xDD, 0xEE],
            true,
            true,
            5,
        );

        assert!(sql.contains("N'aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa'"));
        assert!(sql.contains("N'aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa.0'"));
        assert!(sql.contains("0x102030"));
        assert!(sql.contains("IF @@ROWCOUNT <> 1 THROW 55501"));
    }

    #[test]
    fn builds_metadata_object_stage_sql_with_multiple_objects() {
        let prepared = vec![
            PreparedMetadataObjectStage {
                object_id: "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa".to_string(),
                kind: "CommonPicture".to_string(),
                xml: PathBuf::from("CommonPictures/TestPicture.xml"),
                properties: SimpleMetadataXmlProperties {
                    kind: "CommonPicture".to_string(),
                    uuid: "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa".to_string(),
                    name: "TestPicture".to_string(),
                    synonyms: Vec::new(),
                    comment: String::new(),
                },
                metadata_plain_bytes: 12,
                metadata_blob: vec![0x01, 0x23, 0x45],
                metadata_blob_sha256: "deadbeef".to_string(),
                body_rows: Vec::new(),
            },
            PreparedMetadataObjectStage {
                object_id: "abababab-abab-4aba-baba-abababababab".to_string(),
                kind: "CommonAttribute".to_string(),
                xml: PathBuf::from(
                    "CommonAttributes/ОтредактированныеПредопределенныеРеквизиты.xml",
                ),
                properties: SimpleMetadataXmlProperties {
                    kind: "CommonAttribute".to_string(),
                    uuid: "abababab-abab-4aba-baba-abababababab".to_string(),
                    name: "ОтредактированныеПредопределенныеРеквизиты".to_string(),
                    synonyms: Vec::new(),
                    comment: String::new(),
                },
                metadata_plain_bytes: 14,
                metadata_blob: vec![0xAA, 0xBB, 0xCC],
                metadata_blob_sha256: "abad1dea".to_string(),
                body_rows: Vec::new(),
            },
            PreparedMetadataObjectStage {
                object_id: "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb".to_string(),
                kind: "CommonForm".to_string(),
                xml: PathBuf::from("CommonForms/ФормаОтчета.xml"),
                properties: SimpleMetadataXmlProperties {
                    kind: "CommonForm".to_string(),
                    uuid: "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb".to_string(),
                    name: "ФормаОтчета".to_string(),
                    synonyms: Vec::new(),
                    comment: String::new(),
                },
                metadata_plain_bytes: 10,
                metadata_blob: vec![0xAB, 0xCD],
                metadata_blob_sha256: "cafed00d".to_string(),
                body_rows: Vec::new(),
            },
            PreparedMetadataObjectStage {
                object_id: "cccccccc-cccc-4ccc-cccc-cccccccccccc".to_string(),
                kind: "CommonTemplate".to_string(),
                xml: PathBuf::from("CommonTemplates/СтруктураПодчиненности.xml"),
                properties: SimpleMetadataXmlProperties {
                    kind: "CommonTemplate".to_string(),
                    uuid: "cccccccc-cccc-4ccc-cccc-cccccccccccc".to_string(),
                    name: "СтруктураПодчиненности".to_string(),
                    synonyms: Vec::new(),
                    comment: String::new(),
                },
                metadata_plain_bytes: 8,
                metadata_blob: vec![0x11, 0x22, 0x33],
                metadata_blob_sha256: "f00dbabe".to_string(),
                body_rows: Vec::new(),
            },
            PreparedMetadataObjectStage {
                object_id: "dddddddd-dddd-4ddd-dddd-dddddddddddd".to_string(),
                kind: "CommonCommand".to_string(),
                xml: PathBuf::from("CommonCommands/АвтономнаяРабота.xml"),
                properties: SimpleMetadataXmlProperties {
                    kind: "CommonCommand".to_string(),
                    uuid: "dddddddd-dddd-4ddd-dddd-dddddddddddd".to_string(),
                    name: "АвтономнаяРабота".to_string(),
                    synonyms: Vec::new(),
                    comment: String::new(),
                },
                metadata_plain_bytes: 9,
                metadata_blob: vec![0x44, 0x55, 0x66],
                metadata_blob_sha256: "baadf00d".to_string(),
                body_rows: Vec::new(),
            },
            PreparedMetadataObjectStage {
                object_id: "eeeeeeee-eeee-4eee-eeee-eeeeeeeeeeee".to_string(),
                kind: "CommandGroup".to_string(),
                xml: PathBuf::from("CommandGroups/Органайзер.xml"),
                properties: SimpleMetadataXmlProperties {
                    kind: "CommandGroup".to_string(),
                    uuid: "eeeeeeee-eeee-4eee-eeee-eeeeeeeeeeee".to_string(),
                    name: "Органайзер".to_string(),
                    synonyms: Vec::new(),
                    comment: String::new(),
                },
                metadata_plain_bytes: 11,
                metadata_blob: vec![0x77, 0x88],
                metadata_blob_sha256: "facefeed".to_string(),
                body_rows: Vec::new(),
            },
            PreparedMetadataObjectStage {
                object_id: "ffffffff-ffff-4fff-ffff-ffffffffffff".to_string(),
                kind: "ReportObject".to_string(),
                xml: PathBuf::from("Reports/БизнесПроцессы.xml"),
                properties: SimpleMetadataXmlProperties {
                    kind: "ReportObject".to_string(),
                    uuid: "ffffffff-ffff-4fff-ffff-ffffffffffff".to_string(),
                    name: "БизнесПроцессы".to_string(),
                    synonyms: Vec::new(),
                    comment: String::new(),
                },
                metadata_plain_bytes: 13,
                metadata_blob: vec![0x99, 0xAA],
                metadata_blob_sha256: "decafbad".to_string(),
                body_rows: Vec::new(),
            },
            PreparedMetadataObjectStage {
                object_id: "99999999-9999-4999-9999-999999999999".to_string(),
                kind: "ExchangePlanObject".to_string(),
                xml: PathBuf::from("ExchangePlans/ОбновлениеИнформационнойБазы.xml"),
                properties: SimpleMetadataXmlProperties {
                    kind: "ExchangePlanObject".to_string(),
                    uuid: "99999999-9999-4999-9999-999999999999".to_string(),
                    name: "ОбновлениеИнформационнойБазы".to_string(),
                    synonyms: Vec::new(),
                    comment: String::new(),
                },
                metadata_plain_bytes: 15,
                metadata_blob: vec![0xDE, 0xAD, 0xBE, 0xEF],
                metadata_blob_sha256: "b16b00b5".to_string(),
                body_rows: Vec::new(),
            },
            PreparedMetadataObjectStage {
                object_id: "c39750ca-e33f-40c2-b830-119423d9a2ae".to_string(),
                kind: "Enum".to_string(),
                xml: PathBuf::from("Enums/ВариантыВажностиЗадачи.xml"),
                properties: SimpleMetadataXmlProperties {
                    kind: "Enum".to_string(),
                    uuid: "c39750ca-e33f-40c2-b830-119423d9a2ae".to_string(),
                    name: "ВариантыВажностиЗадачи".to_string(),
                    synonyms: Vec::new(),
                    comment: String::new(),
                },
                metadata_plain_bytes: 17,
                metadata_blob: vec![0xC3, 0x97, 0x50, 0xCA],
                metadata_blob_sha256: "0badc0de".to_string(),
                body_rows: Vec::new(),
            },
            PreparedMetadataObjectStage {
                object_id: "ad083c26-7461-4e94-b524-0174242fbd91".to_string(),
                kind: "ChartOfCharacteristicTypes".to_string(),
                xml: PathBuf::from("ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач.xml"),
                properties: SimpleMetadataXmlProperties {
                    kind: "ChartOfCharacteristicTypes".to_string(),
                    uuid: "ad083c26-7461-4e94-b524-0174242fbd91".to_string(),
                    name: "ОбъектыАдресацииЗадач".to_string(),
                    synonyms: Vec::new(),
                    comment: String::new(),
                },
                metadata_plain_bytes: 21,
                metadata_blob: vec![0x10, 0x20, 0x30, 0x40],
                metadata_blob_sha256: "c0ffee00".to_string(),
                body_rows: Vec::new(),
            },
        ];

        let sql = super::build_stage_metadata_objects_sql("TestDb", &prepared, &[0xAA, 0xBB]);

        assert!(sql.contains("IF @@ROWCOUNT <> 2 THROW 54000"));
        assert!(sql.contains("IF @@ROWCOUNT <> 1 THROW 54001"));
        assert!(sql.contains("IF @@ROWCOUNT <> 1 THROW 54002"));
        assert!(sql.contains("IF @@ROWCOUNT <> 1 THROW 54003"));
        assert!(sql.contains("IF @@ROWCOUNT <> 1 THROW 54004"));
        assert!(sql.contains("IF @@ROWCOUNT <> 1 THROW 54005"));
        assert!(sql.contains("IF @@ROWCOUNT <> 1 THROW 54006"));
        assert!(sql.contains("IF @@ROWCOUNT <> 1 THROW 54007"));
        assert!(sql.contains("IF @@ROWCOUNT <> 1 THROW 54008"));
        assert!(sql.contains("IF @@ROWCOUNT <> 1 THROW 54009"));
        assert!(sql.contains("0x012345"));
        assert!(sql.contains("0xAABBCC"));
        assert!(sql.contains("0xABCD"));
        assert!(sql.contains("0x112233"));
        assert!(sql.contains("0x445566"));
        assert!(sql.contains("0x7788"));
        assert!(sql.contains("0x99AA"));
        assert!(sql.contains("0xDEADBEEF"));
        assert!(sql.contains("0xC39750CA"));
        assert!(sql.contains("0x10203040"));
        assert!(sql.contains("0xAABB"));
        assert!(sql.contains("IF (SELECT COUNT_BIG(*) FROM ConfigSave) <> 13"));
    }

    #[test]
    fn builds_common_module_metadata_stage_sql_with_expected_row_counts() {
        let sql = super::build_stage_common_module_metadata_sql(
            "TestDb",
            "dddddddd-dddd-4ddd-dddd-dddddddddddd",
            &[0x10, 0x20, 0x30],
            &[0xAA, 0xBB],
        );

        assert!(sql.contains("USE [TestDb];"));
        assert!(sql.contains("IF @@ROWCOUNT <> 2 THROW 52000"));
        assert!(sql.contains("N'dddddddd-dddd-4ddd-dddd-dddddddddddd'"));
        assert!(sql.contains("0x102030"));
        assert!(sql.contains("0xAABB"));
        assert!(sql.contains("IF (SELECT COUNT_BIG(*) FROM ConfigSave) <> 4"));
    }

    #[test]
    fn builds_common_module_object_stage_sql_with_expected_row_counts() {
        let prepared = vec![PreparedCommonModuleObjectStage {
            module_id: "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb".to_string(),
            module_body_id: "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb.0".to_string(),
            xml: PathBuf::from("CommonModules/TestModule.xml"),
            text: PathBuf::from("CommonModules/TestModule/Ext/Module.bsl"),
            properties: CommonModuleXmlProperties {
                uuid: "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb".to_string(),
                name: "TestModule".to_string(),
                synonyms: Vec::new(),
                comment: String::new(),
                global: true,
                client_managed_application: false,
                server: true,
                external_connection: false,
                client_ordinary_application: false,
                server_call: false,
                privileged: false,
                return_values_reuse: ReturnValuesReuse::DontUse,
            },
            metadata_plain_bytes: 14,
            metadata_blob: vec![0x10, 0x20],
            metadata_blob_sha256: "cafebabe".to_string(),
            text_bytes: 7,
            module_blob: vec![0xAA, 0xBB, 0xCC],
            module_blob_sha256: "feedface".to_string(),
        }];

        let sql = super::build_stage_common_module_objects_sql("TestDb", &prepared, &[0xCC, 0xDD]);

        assert!(sql.contains("USE [TestDb];"));
        assert!(sql.contains("IF @@ROWCOUNT <> 2 THROW 53000"));
        assert!(sql.contains("N'bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb'"));
        assert!(sql.contains("N'bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb.0'"));
        assert!(sql.contains("0x1020"));
        assert!(sql.contains("0xAABBCC"));
        assert!(sql.contains("0xCCDD"));
        assert!(sql.contains("IF (SELECT COUNT_BIG(*) FROM ConfigSave) <> 5"));
    }

    #[test]
    fn builds_common_module_object_stage_sql_with_multiple_modules() {
        let prepared = vec![
            PreparedCommonModuleObjectStage {
                module_id: "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb".to_string(),
                module_body_id: "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb.0".to_string(),
                xml: PathBuf::from("CommonModules/TestModule.xml"),
                text: PathBuf::from("CommonModules/TestModule/Ext/Module.bsl"),
                properties: CommonModuleXmlProperties {
                    uuid: "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb".to_string(),
                    name: "TestModule".to_string(),
                    synonyms: Vec::new(),
                    comment: String::new(),
                    global: true,
                    client_managed_application: false,
                    server: true,
                    external_connection: false,
                    client_ordinary_application: false,
                    server_call: false,
                    privileged: false,
                    return_values_reuse: ReturnValuesReuse::DontUse,
                },
                metadata_plain_bytes: 14,
                metadata_blob: vec![0x10, 0x20],
                metadata_blob_sha256: "cafebabe".to_string(),
                text_bytes: 7,
                module_blob: vec![0xAA, 0xBB, 0xCC],
                module_blob_sha256: "feedface".to_string(),
            },
            PreparedCommonModuleObjectStage {
                module_id: "cccccccc-cccc-4ccc-cccc-cccccccccccc".to_string(),
                module_body_id: "cccccccc-cccc-4ccc-cccc-cccccccccccc.0".to_string(),
                xml: PathBuf::from("CommonModules/Batch.xml"),
                text: PathBuf::from("CommonModules/Batch/Ext/Module.bsl"),
                properties: CommonModuleXmlProperties {
                    uuid: "cccccccc-cccc-4ccc-cccc-cccccccccccc".to_string(),
                    name: "Batch".to_string(),
                    synonyms: Vec::new(),
                    comment: String::new(),
                    global: false,
                    client_managed_application: false,
                    server: true,
                    external_connection: false,
                    client_ordinary_application: false,
                    server_call: false,
                    privileged: false,
                    return_values_reuse: ReturnValuesReuse::DuringRequest,
                },
                metadata_plain_bytes: 16,
                metadata_blob: vec![0x21, 0x43],
                metadata_blob_sha256: "b16b00b5".to_string(),
                text_bytes: 9,
                module_blob: vec![0xDD, 0xEE],
                module_blob_sha256: "decafbad".to_string(),
            },
        ];

        let sql = super::build_stage_common_module_objects_sql("TestDb", &prepared, &[0xCC, 0xDD]);

        assert!(sql.contains("IF @@ROWCOUNT <> 2 THROW 53000"));
        assert!(sql.contains("IF @@ROWCOUNT <> 1 THROW 53001"));
        assert!(sql.contains("IF @@ROWCOUNT <> 1 THROW 53002"));
        assert!(sql.contains("IF @@ROWCOUNT <> 1 THROW 53003"));
        assert!(sql.contains("0x1020"));
        assert!(sql.contains("0x2143"));
        assert!(sql.contains("0xAABBCC"));
        assert!(sql.contains("0xDDEE"));
        assert!(sql.contains("IF (SELECT COUNT_BIG(*) FROM ConfigSave) <> 7"));
    }

    #[test]
    fn builds_common_module_batch_stage_sql_with_expected_row_counts() {
        let prepared = vec![PreparedCommonModuleStage {
            spec: CommonModuleStageSpec {
                module_id: "cccccccc-cccc-4ccc-cccc-cccccccccccc".to_string(),
                text: PathBuf::from("CommonModules/Batch/Ext/Module.bsl"),
            },
            module_body_id: "cccccccc-cccc-4ccc-cccc-cccccccccccc.0".to_string(),
            text_bytes: 11,
            blob: vec![0x11, 0x22, 0x33],
            blob_sha256: "facefeed".to_string(),
        }];

        let sql = super::build_stage_common_modules_sql("TestDb", &prepared, &[0xDD, 0xEE]);

        assert!(sql.contains("USE [TestDb];"));
        assert!(sql.contains("IF @@ROWCOUNT <> 3 THROW 51000"));
        assert!(sql.contains("N'cccccccc-cccc-4ccc-cccc-cccccccccccc'"));
        assert!(sql.contains("N'cccccccc-cccc-4ccc-cccc-cccccccccccc.0'"));
        assert!(sql.contains("0x112233"));
        assert!(sql.contains("0xDDEE"));
        assert!(sql.contains("IF (SELECT COUNT_BIG(*) FROM ConfigSave) <> 5"));
    }

    #[test]
    fn builds_common_module_batch_stage_sql_with_multiple_modules() {
        let prepared = vec![
            PreparedCommonModuleStage {
                spec: CommonModuleStageSpec {
                    module_id: "cccccccc-cccc-4ccc-cccc-cccccccccccc".to_string(),
                    text: PathBuf::from("CommonModules/Batch/One/Ext/Module.bsl"),
                },
                module_body_id: "cccccccc-cccc-4ccc-cccc-cccccccccccc.0".to_string(),
                text_bytes: 11,
                blob: vec![0x11, 0x22, 0x33],
                blob_sha256: "facefeed".to_string(),
            },
            PreparedCommonModuleStage {
                spec: CommonModuleStageSpec {
                    module_id: "dddddddd-dddd-4ddd-dddd-dddddddddddd".to_string(),
                    text: PathBuf::from("CommonModules/Batch/Two/Ext/Module.bsl"),
                },
                module_body_id: "dddddddd-dddd-4ddd-dddd-dddddddddddd.0".to_string(),
                text_bytes: 13,
                blob: vec![0x44, 0x55, 0x66],
                blob_sha256: "beadfeed".to_string(),
            },
        ];

        let sql = super::build_stage_common_modules_sql("TestDb", &prepared, &[0xDD, 0xEE]);

        assert!(sql.contains("IF @@ROWCOUNT <> 4 THROW 51000"));
        assert!(sql.contains("IF @@ROWCOUNT <> 1 THROW 51001"));
        assert!(sql.contains("IF @@ROWCOUNT <> 1 THROW 51002"));
        assert!(sql.contains("0x112233"));
        assert!(sql.contains("0x445566"));
        assert!(sql.contains("0xDDEE"));
        assert!(sql.contains("IF (SELECT COUNT_BIG(*) FROM ConfigSave) <> 7"));
    }

    #[test]
    fn rejects_non_lab_destructive_actions_without_confirmation() {
        let error = require_non_lab_confirmation(false, "storage import").unwrap_err();
        assert!(error.to_string().contains("--allow-non-lab"));
    }
}
