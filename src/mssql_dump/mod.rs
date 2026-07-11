use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::Instant;

use anyhow::{Context, Result, anyhow, bail};
use flate2::read::DeflateDecoder;
use quick_xml::events::Event;
use quick_xml::name::ResolveResult;
use quick_xml::{NsReader, Reader};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::cli::{InfobaseConfigSourceVersion, MssqlDumpConfigArgs};
use crate::module_blob::{
    LocalizedString, ParsedFormBodyBlob, SpreadsheetNumberFormatHint, parse_form_body_blob,
    unpack_module_blob_text,
};
use crate::parallel;

mod command_interface;
mod config_dump_info;
mod config_rows;
mod dcs;
mod fetch;
mod form_body;
mod forms;
mod metadata;
mod moxel;
mod refs;
mod role_rights;
mod selected;
mod source_assets;
mod timing;

use command_interface::*;
use config_dump_info::*;
use config_rows::*;
use dcs::*;
use fetch::*;
use form_body::*;
use forms::*;
use metadata::*;
use moxel::*;
use refs::*;
use role_rights::*;
use selected::*;
use source_assets::*;

pub(crate) fn resolve_form_item_picture_owner(
    text: &str,
    marker_start: usize,
) -> Option<(String, &'static str)> {
    form_body::form_item_picture_owner_at(text, marker_start)
}

pub(crate) use form_body::{extract_form_body_xml, unpack_form_body_module_text};
#[cfg(test)]
pub(crate) use moxel::{
    DebugMoxelNumberFormatUsage, DebugMoxelSpreadsheetSummary, debug_moxel_number_format_usage,
    debug_moxel_spreadsheet_summary_from_blob,
};
pub(crate) use moxel::{extract_moxel_spreadsheet_xml, spreadsheet_number_format_hint_from_blob};

pub use timing::{
    MssqlDumpTableTimingSummary, MssqlDumpTimingReport, MssqlDumpTimingSummary,
    parse_dump_timing_summary, read_dump_timing_summaries, read_dump_timing_summary,
};

// Platform-level 1C standard pictures, not metadata UUIDs from one database.
const STD_PICTURE_INFORMATION_UUID: &str = "4b54770b-d069-4c0e-9b17-5cc2a01134d9";
const STD_PICTURE_SAVE_FILE_UUID: &str = "818ab7d0-4654-4542-bd5e-fd9d1352b5a1";
const STD_PICTURE_USER_UUID: &str = "6ff3ddbd-56e3-4ddf-a5bf-048c1e2dfb2f";
const STD_PICTURE_LOAD_REPORT_SETTINGS_UUID: &str = "283ecabd-aaed-41d1-ad46-6cca91c29120";
const STD_PICTURE_INFORMATION_REGISTER_UUID: &str = "5b87ad1b-d8cc-43c1-b5c4-dc43613c518c";
const STD_PICTURE_SHOW_DATA_UUID: &str = "a064544f-6037-48ca-b19f-8ad63e43af23";
const STD_PICTURE_CUSTOMIZE_LIST_UUID: &str = "f04794cb-c198-4172-86c3-649386013c85";
// Platform serialized-value type IDs, independent of metadata in any infobase.
const DESIGN_TIME_REF_TYPE_UUID: &str = "5c14e26f-099b-4d37-84a6-b433d87400da";
const FIXED_ARRAY_TYPE_UUID: &str = "4500381b-db30-4a10-9db4-990038032acf";
const METADATA_OBJECT_REF_TYPE_UUID: &str = "157fa490-4ce9-11d4-9415-008048da11f9";
const MAX_METADATA_CHOICE_PARAMETER_VALUE_DEPTH: usize = 64;
// Platform collection type IDs, stable across independent infobases.
const CATALOG_ATTRIBUTE_GROUP_UUID: &str = "cf4abea7-37b2-11d4-940f-008048da11f9";
const CATALOG_TABULAR_ATTRIBUTE_GROUP_UUID: &str = "888744e1-b616-11d4-9436-004095e12fc7";
const DOCUMENT_ATTRIBUTE_GROUP_UUID: &str = "45e46cbc-3e24-4165-8b7b-cc98a6f80211";
const DOCUMENT_TABULAR_ATTRIBUTE_GROUP_UUID: &str = "888744e1-b616-11d4-9436-004095e12fc7";
const WEB_SERVICE_OPERATION_COLLECTION_UUID: &str = "36186084-c23a-43bd-876c-a3a8ba1a9622";
const WEB_SERVICE_PARAMETER_COLLECTION_UUID: &str = "b78a00b2-2260-4ef5-a70c-17889cfee695";
const XDTO_XML_SCHEMA_NAMESPACE: &str = "http://www.w3.org/2001/XMLSchema";
const XDTO_CORE_NAMESPACE: &str = "http://v8.1c.ru/8.1/data/core";
const XML_NAMESPACE: &str = "http://www.w3.org/XML/1998/namespace";
const XMLNS_NAMESPACE: &str = "http://www.w3.org/2000/xmlns/";

const FORM_APPLICATION_USE_PURPOSE_TYPE_UUID: &str = "1708fdaa-cbce-4289-b373-07a5a74bee91";
// Platform picture descriptor code for StdPicture.Print.
const STD_PICTURE_PRINT_DESCRIPTOR_CODE: &str = "-13";

#[derive(Debug, Serialize, Deserialize)]
pub struct MssqlDumpConfigReport {
    pub database: String,
    pub output_dir: PathBuf,
    pub tables: Vec<MssqlDumpedTableReport>,
    pub total_rows: usize,
    pub total_binary_bytes: usize,
    pub total_inflated_rows: usize,
    pub total_module_text_rows: usize,
    pub total_metadata_xml_rows: usize,
    pub total_source_asset_rows: usize,
    pub timings: MssqlDumpTimingReport,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MssqlDumpedTableReport {
    pub table: String,
    pub rows: usize,
    pub binary_bytes: usize,
    pub inflated_rows: usize,
    pub module_text_rows: usize,
    pub metadata_xml_rows: usize,
    pub source_asset_rows: usize,
    pub timings: MssqlDumpTimingReport,
}

#[derive(Debug, Serialize)]
struct MssqlDumpManifest {
    database: String,
    tables: Vec<MssqlDumpTableManifest>,
}

#[derive(Debug, Serialize)]
struct MssqlDumpTableManifest {
    table: String,
    rows: Vec<MssqlDumpRowManifest>,
}

#[derive(Debug, Serialize)]
struct MssqlDumpRowManifest {
    file_name: String,
    part_no: i32,
    data_size: i64,
    binary_bytes: usize,
    binary_path: Option<String>,
    inflated_path: Option<String>,
    module_text_path: Option<String>,
    metadata_xml_path: Option<String>,
    source_asset_path: Option<String>,
}

pub fn dump_config(args: &MssqlDumpConfigArgs) -> Result<MssqlDumpConfigReport> {
    prepare_output_dir(&args.output_dir, args.overwrite)?;

    let mut table_names = vec!["Config"];
    if args.include_config_save {
        table_names.push("ConfigSave");
    }

    let mut reports = Vec::new();
    let mut manifest_tables = Vec::new();
    let mut total_timings = MssqlDumpTimingReport::default();
    let selected_file_names =
        selected_file_names_from_args(&args.file_names, &args.file_name_lists)?;
    let write_binary_rows = args.write_binary_rows && !args.no_binary_rows;
    for table in table_names {
        let dumped = dump_table_rows_streamed(
            &args.sqlcmd,
            &args.server,
            args.sql_user.as_deref(),
            sql_password(
                args.sql_user.as_deref(),
                args.sql_pwd.as_deref(),
                &args.sql_pwd_env,
            )
            .as_deref(),
            &args.database,
            table,
            &selected_file_names,
            !args.include_config_save,
            &args.output_dir,
            write_binary_rows,
            args.inflate,
            args.extract_module_text,
            args.extract_metadata_xml,
            args.source_version,
        )?;
        reports.push(MssqlDumpedTableReport {
            table: table.to_string(),
            rows: dumped.rows.len(),
            binary_bytes: dumped.binary_bytes,
            inflated_rows: dumped.inflated_rows,
            module_text_rows: dumped.module_text_rows,
            metadata_xml_rows: dumped.metadata_xml_rows,
            source_asset_rows: dumped.source_asset_rows,
            timings: dumped.timings.clone(),
        });
        total_timings.add_assign(&dumped.timings);
        if args.write_manifest {
            manifest_tables.push(MssqlDumpTableManifest {
                table: table.to_string(),
                rows: dumped.rows,
            });
        }
    }

    if args.write_manifest {
        let manifest = MssqlDumpManifest {
            database: args.database.clone(),
            tables: manifest_tables,
        };
        let manifest_json = serde_json::to_string_pretty(&manifest)?;
        fs::write(args.output_dir.join("manifest.json"), manifest_json).with_context(|| {
            format!(
                "failed to write {}",
                args.output_dir.join("manifest.json").display()
            )
        })?;
    }

    Ok(MssqlDumpConfigReport {
        database: args.database.clone(),
        output_dir: args.output_dir.clone(),
        total_rows: reports.iter().map(|table| table.rows).sum(),
        total_binary_bytes: reports.iter().map(|table| table.binary_bytes).sum(),
        total_inflated_rows: reports.iter().map(|table| table.inflated_rows).sum(),
        total_module_text_rows: reports.iter().map(|table| table.module_text_rows).sum(),
        total_metadata_xml_rows: reports.iter().map(|table| table.metadata_xml_rows).sum(),
        total_source_asset_rows: reports.iter().map(|table| table.source_asset_rows).sum(),
        timings: total_timings,
        tables: reports,
    })
}

fn normalize_source_xml_version_bytes(
    bytes: &[u8],
    source_version: InfobaseConfigSourceVersion,
) -> Vec<u8> {
    let from = match source_version {
        InfobaseConfigSourceVersion::V2_20 => "version=\"2.21\"",
        InfobaseConfigSourceVersion::V2_21 => "version=\"2.20\"",
    };
    let to = format!("version=\"{}\"", source_version.as_str());
    let Ok(text) = std::str::from_utf8(bytes) else {
        return bytes.to_vec();
    };
    if text.contains(from) {
        text.replace(from, &to).into_bytes()
    } else {
        bytes.to_vec()
    }
}

#[allow(dead_code)]
fn dump_table_rows_eager(
    sqlcmd: &Path,
    server: &str,
    user: Option<&str>,
    password: Option<&str>,
    database: &str,
    table: &str,
    selected_file_names: &BTreeSet<String>,
    output_dir: &Path,
    write_binary_rows: bool,
    inflate: bool,
    extract_module_text: bool,
    extract_metadata_xml: bool,
) -> Result<DumpedTable> {
    let rows = fetch_rows(
        sqlcmd,
        server,
        user,
        password,
        database,
        table,
        selected_file_names,
    )?;
    dump_table_rows_with_options(
        output_dir,
        table,
        rows,
        write_binary_rows,
        inflate,
        extract_module_text,
        extract_metadata_xml,
        InfobaseConfigSourceVersion::V2_20,
    )
}

struct DumpedTable {
    rows: Vec<MssqlDumpRowManifest>,
    binary_bytes: usize,
    inflated_rows: usize,
    module_text_rows: usize,
    metadata_xml_rows: usize,
    source_asset_rows: usize,
    timings: MssqlDumpTimingReport,
}

struct DumpedRow {
    manifest: MssqlDumpRowManifest,
    binary_bytes: usize,
    inflated_rows: usize,
    module_text_rows: usize,
    metadata_xml_rows: usize,
    source_asset_rows: usize,
    timings: MssqlDumpTimingReport,
}

struct DumpRowContext<'a> {
    output_dir: &'a Path,
    table: &'a str,
    source_version: InfobaseConfigSourceVersion,
    write_binary_rows: bool,
    inflate: bool,
    extract_module_text: bool,
    extract_metadata_xml: bool,
    module_text_paths: &'a BTreeMap<String, PathBuf>,
    source_assets: &'a BTreeMap<String, SourceAsset>,
    metadata_texts_by_file_name: &'a BTreeMap<&'a str, &'a MetadataTextRow>,
    command_refs: &'a BTreeMap<String, String>,
    metadata_refs: &'a BTreeMap<String, MetadataCommandReference>,
    type_index: &'a BTreeMap<String, String>,
    dcs_type_index: &'a DcsTypeIndex,
    object_refs: &'a BTreeMap<String, String>,
    metadata_object_refs: &'a BTreeMap<String, String>,
    configuration_root_object_refs: &'a BTreeMap<String, String>,
    recalculation_refs: &'a BTreeMap<String, CalculationRecalculationReference>,
    predefined_item_refs: &'a BTreeMap<String, String>,
    role_rights_object_refs: &'a BTreeMap<String, String>,
    metadata_order: &'a BTreeMap<String, usize>,
    field_refs: &'a BTreeMap<String, String>,
    functional_option_refs: &'a BTreeMap<String, String>,
    help_refs: &'a BTreeMap<String, String>,
    standalone_refs: &'a StandaloneContentReferences,
    form_refs: &'a BTreeMap<String, FormSourceReference>,
    template_refs: &'a BTreeMap<String, TemplateSourceReference>,
    subsystem_refs: &'a BTreeMap<String, SubsystemSourceReference>,
    file_names: &'a BTreeSet<String>,
    body_owners: &'a BTreeMap<String, BodyOwnerSourceReference>,
    configuration_module_groups: &'a BTreeSet<String>,
}

#[allow(dead_code)]
fn dump_table_rows(
    output_dir: &Path,
    table: &str,
    rows: Vec<ConfigRow>,
    inflate: bool,
    extract_module_text: bool,
    extract_metadata_xml: bool,
) -> Result<DumpedTable> {
    dump_table_rows_with_options(
        output_dir,
        table,
        rows,
        true,
        inflate,
        extract_module_text,
        extract_metadata_xml,
        InfobaseConfigSourceVersion::V2_20,
    )
}

#[cfg(test)]
fn dump_table_rows_with_source_version(
    output_dir: &Path,
    table: &str,
    rows: Vec<ConfigRow>,
    inflate: bool,
    extract_module_text: bool,
    extract_metadata_xml: bool,
    source_version: InfobaseConfigSourceVersion,
) -> Result<DumpedTable> {
    dump_table_rows_with_options(
        output_dir,
        table,
        rows,
        true,
        inflate,
        extract_module_text,
        extract_metadata_xml,
        source_version,
    )
}

fn dump_table_rows_with_options(
    output_dir: &Path,
    table: &str,
    rows: Vec<ConfigRow>,
    write_binary_rows: bool,
    inflate: bool,
    extract_module_text: bool,
    extract_metadata_xml: bool,
    source_version: InfobaseConfigSourceVersion,
) -> Result<DumpedTable> {
    let table_dir = output_dir.join(table);
    if write_binary_rows {
        fs::create_dir_all(&table_dir)
            .with_context(|| format!("failed to create {}", table_dir.display()))?;
    }
    let inflated_dir = output_dir.join(format!("{table}_inflated"));
    if inflate {
        fs::create_dir_all(&inflated_dir)
            .with_context(|| format!("failed to create {}", inflated_dir.display()))?;
    }
    let module_text_dir = output_dir.join(format!("{table}_module_text"));
    if extract_module_text {
        fs::create_dir_all(&module_text_dir)
            .with_context(|| format!("failed to create {}", module_text_dir.display()))?;
    }
    let file_names_owned = rows
        .iter()
        .map(|row| row.file_name.clone())
        .collect::<BTreeSet<_>>();
    let needs_standalone_refs =
        file_names_have_standalone_content_asset(file_names_owned.iter().map(String::as_str));
    let needs_source_layout_refs = !write_binary_rows;
    let standalone_required_refs = if needs_standalone_refs && !extract_metadata_xml {
        standalone_content_reference_uuids_from_config_rows(&rows)
    } else {
        BTreeSet::new()
    };
    let metadata_texts = if extract_metadata_xml
        || extract_module_text
        || needs_standalone_refs
        || needs_source_layout_refs
    {
        build_metadata_text_rows(&rows)
    } else {
        Vec::new()
    };
    let metadata_texts_by_file_name = metadata_texts
        .iter()
        .map(|row| (row.file_name.as_str(), row))
        .collect::<BTreeMap<_, _>>();
    let recalculation_refs = if extract_metadata_xml {
        build_calculation_recalculation_reference_index(&metadata_texts)
    } else {
        BTreeMap::new()
    };
    let module_text_paths = if extract_module_text {
        module_body_paths_from_texts(&rows, &metadata_texts)
    } else {
        BTreeMap::new()
    };
    let command_refs = if extract_metadata_xml {
        build_command_interface_reference_index_from_texts(&metadata_texts)
    } else {
        BTreeMap::new()
    };
    let metadata_refs = if extract_metadata_xml {
        build_metadata_command_reference_index_from_texts(&metadata_texts)
    } else {
        BTreeMap::new()
    };
    let MetadataTypeIndexes {
        references: type_index,
        dcs: dcs_type_index,
    } = if extract_metadata_xml || needs_source_layout_refs {
        build_metadata_type_indexes_from_texts(&metadata_texts)
    } else {
        MetadataTypeIndexes::default()
    };
    let refs_for_standalone =
        extract_metadata_xml || needs_standalone_refs || needs_source_layout_refs;
    let form_refs = if refs_for_standalone {
        build_complete_form_source_reference_index(&metadata_texts)
    } else {
        BTreeMap::new()
    };
    let template_refs = if refs_for_standalone {
        build_template_source_reference_index_from_texts(&rows, &metadata_texts)
    } else {
        BTreeMap::new()
    };
    let subsystem_refs = if refs_for_standalone {
        build_subsystem_source_reference_index_from_texts(&metadata_texts)
    } else {
        BTreeMap::new()
    };
    let object_refs = if extract_metadata_xml || needs_source_layout_refs {
        build_metadata_object_reference_index_from_texts(&metadata_texts)
    } else if needs_standalone_refs {
        build_standalone_object_reference_index_from_texts(
            &metadata_texts,
            &standalone_required_refs,
            &form_refs,
            &template_refs,
            &subsystem_refs,
        )
    } else {
        BTreeMap::new()
    };
    let configuration_root_object_refs = if extract_metadata_xml {
        build_configuration_root_object_reference_index_from_texts(&metadata_texts, &object_refs)
    } else {
        BTreeMap::new()
    };
    let role_rights_object_refs =
        build_role_rights_object_reference_index(&object_refs, &form_refs);
    let metadata_order = if extract_metadata_xml || needs_source_layout_refs {
        build_metadata_order_index_from_texts(&metadata_texts)
    } else {
        BTreeMap::new()
    };
    let field_refs = if extract_metadata_xml {
        build_metadata_field_reference_index_from_texts(&metadata_texts)
    } else {
        BTreeMap::new()
    };
    let functional_option_refs = if extract_metadata_xml {
        build_functional_option_reference_index_from_texts(
            &metadata_texts,
            &object_refs,
            &form_refs,
            &template_refs,
            &subsystem_refs,
        )
    } else {
        BTreeMap::new()
    };
    let source_assets = source_asset_paths_with_indexes(
        &rows,
        &metadata_texts,
        &command_refs,
        &metadata_refs,
        &object_refs,
        &field_refs,
        &type_index,
        &form_refs,
        &template_refs,
        &subsystem_refs,
    );
    let source_asset_diagnostics =
        build_form_owner_resolution_diagnostics_from_texts(&metadata_texts);
    let help_refs = if extract_metadata_xml {
        build_help_reference_index(&object_refs, &form_refs, &template_refs, &subsystem_refs)
    } else {
        BTreeMap::new()
    };
    let standalone_refs = if needs_standalone_refs
        && source_assets
            .values()
            .any(|asset| matches!(asset.kind, SourceAssetKind::StandaloneContent))
    {
        if extract_metadata_xml {
            build_standalone_content_references(
                &metadata_texts,
                &object_refs,
                &form_refs,
                &template_refs,
                &subsystem_refs,
            )
        } else {
            build_standalone_content_references_for_uuids(
                &metadata_texts,
                &standalone_required_refs,
                &object_refs,
                &form_refs,
                &template_refs,
                &subsystem_refs,
            )
        }
    } else {
        StandaloneContentReferences::default()
    };
    let body_owners = if extract_metadata_xml || needs_source_layout_refs {
        build_body_owner_source_index_from_texts(&metadata_texts, &subsystem_refs)
    } else {
        BTreeMap::new()
    };
    let needs_predefined_item_refs =
        predefined_data_needs_item_references(&file_names_owned, &body_owners);
    let predefined_item_refs = if needs_predefined_item_refs {
        build_predefined_item_reference_index(&rows, &body_owners, &type_index, &object_refs)?
    } else {
        BTreeMap::new()
    };
    let metadata_value_owner_file_names = eager_metadata_value_owner_file_names(&metadata_texts);
    let metadata_value_owner_ids = selected_metadata_predefined_owner_ids(
        &metadata_texts,
        &metadata_value_owner_file_names,
        &type_index,
        &object_refs,
        &body_owners,
    );
    let metadata_value_body_owners = body_owners
        .iter()
        .filter(|(uuid, _)| metadata_value_owner_ids.contains(*uuid))
        .map(|(uuid, owner)| (uuid.clone(), owner.clone()))
        .collect::<BTreeMap<_, _>>();
    let metadata_value_predefined_item_refs = build_predefined_item_reference_index(
        &rows,
        &metadata_value_body_owners,
        &type_index,
        &object_refs,
    )?;
    let mut metadata_object_refs = object_refs.clone();
    extend_metadata_owner_value_references(&mut metadata_object_refs, &predefined_item_refs)?;
    extend_metadata_owner_value_references(
        &mut metadata_object_refs,
        &metadata_value_predefined_item_refs,
    )?;
    let configuration_module_groups = configuration_module_groups(&file_names_owned);
    ensure_unique_source_asset_paths(&source_assets, &source_asset_diagnostics)?;

    let context = DumpRowContext {
        output_dir,
        table,
        source_version,
        write_binary_rows,
        inflate,
        extract_module_text,
        extract_metadata_xml,
        module_text_paths: &module_text_paths,
        source_assets: &source_assets,
        metadata_texts_by_file_name: &metadata_texts_by_file_name,
        command_refs: &command_refs,
        metadata_refs: &metadata_refs,
        type_index: &type_index,
        dcs_type_index: &dcs_type_index,
        object_refs: &object_refs,
        metadata_object_refs: &metadata_object_refs,
        configuration_root_object_refs: &configuration_root_object_refs,
        recalculation_refs: &recalculation_refs,
        predefined_item_refs: &predefined_item_refs,
        role_rights_object_refs: &role_rights_object_refs,
        metadata_order: &metadata_order,
        field_refs: &field_refs,
        functional_option_refs: &functional_option_refs,
        help_refs: &help_refs,
        standalone_refs: &standalone_refs,
        form_refs: &form_refs,
        template_refs: &template_refs,
        subsystem_refs: &subsystem_refs,
        file_names: &file_names_owned,
        body_owners: &body_owners,
        configuration_module_groups: &configuration_module_groups,
    };
    let dumped_rows = parallel::install(|| {
        rows.par_iter()
            .map(|row| dump_table_row(&context, row))
            .collect::<Vec<_>>()
    })?;

    let mut manifests = Vec::with_capacity(dumped_rows.len());
    let mut binary_bytes = 0;
    let mut inflated_rows = 0;
    let mut module_text_rows = 0;
    let mut metadata_xml_rows = 0;
    let mut source_asset_rows = 0;
    for dumped in dumped_rows {
        let dumped = dumped?;
        binary_bytes += dumped.binary_bytes;
        inflated_rows += dumped.inflated_rows;
        module_text_rows += dumped.module_text_rows;
        metadata_xml_rows += dumped.metadata_xml_rows;
        source_asset_rows += dumped.source_asset_rows;
        manifests.push(dumped.manifest);
    }

    Ok(DumpedTable {
        rows: manifests,
        binary_bytes,
        inflated_rows,
        module_text_rows,
        metadata_xml_rows,
        source_asset_rows,
        timings: MssqlDumpTimingReport::default(),
    })
}

fn dump_table_rows_streamed(
    sqlcmd: &Path,
    server: &str,
    user: Option<&str>,
    password: Option<&str>,
    database: &str,
    table: &str,
    selected_file_names: &BTreeSet<String>,
    allow_config_dump_info: bool,
    output_dir: &Path,
    write_binary_rows: bool,
    inflate: bool,
    extract_module_text: bool,
    extract_metadata_xml: bool,
    source_version: InfobaseConfigSourceVersion,
) -> Result<DumpedTable> {
    let generate_config_dump_info = allow_config_dump_info
        && table == "Config"
        && selected_file_names.is_empty()
        && !write_binary_rows
        && extract_module_text
        && extract_metadata_xml;
    let table_dir = output_dir.join(table);
    if write_binary_rows {
        fs::create_dir_all(&table_dir)
            .with_context(|| format!("failed to create {}", table_dir.display()))?;
    }
    let inflated_dir = output_dir.join(format!("{table}_inflated"));
    if inflate {
        fs::create_dir_all(&inflated_dir)
            .with_context(|| format!("failed to create {}", inflated_dir.display()))?;
    }
    let module_text_dir = output_dir.join(format!("{table}_module_text"));
    if extract_module_text {
        fs::create_dir_all(&module_text_dir)
            .with_context(|| format!("failed to create {}", module_text_dir.display()))?;
    }

    let headers_started = Instant::now();
    let headers = fetch_row_headers(
        sqlcmd,
        server,
        user,
        password,
        database,
        table,
        selected_file_names,
    )?;
    let fetch_headers_ms = elapsed_ms(headers_started);
    let mut timings = MssqlDumpTimingReport {
        fetch_headers_ms,
        fetch_headers_sqlcmd_ms: fetch_headers_ms,
        ..MssqlDumpTimingReport::default()
    };
    let prepare_started = Instant::now();
    let mut file_names = headers
        .iter()
        .map(|row| row.file_name.clone())
        .collect::<BTreeSet<_>>();
    let needs_standalone_refs =
        file_names_have_standalone_content_asset(file_names.iter().map(String::as_str));
    let standalone_body_file_names = if needs_standalone_refs && !extract_metadata_xml {
        standalone_content_asset_file_names(file_names.iter().map(String::as_str))
    } else {
        BTreeSet::new()
    };
    let metadata_file_names = file_names
        .iter()
        .filter(|file_name| !file_name.contains('.'))
        .cloned()
        .collect::<BTreeSet<_>>();
    let metadata_fetch_started = Instant::now();
    let mut metadata_fetch_used_bcp = false;
    let needs_source_layout_refs = !write_binary_rows;
    let mut metadata_rows = if extract_metadata_xml
        || extract_module_text
        || needs_standalone_refs
        || needs_source_layout_refs
    {
        if selected_file_names.is_empty() {
            metadata_fetch_used_bcp = true;
            fetch_metadata_rows_bcp(sqlcmd, server, user, password, database, table)?
        } else if metadata_file_names.is_empty() {
            Vec::new()
        } else {
            metadata_fetch_used_bcp = true;
            fetch_config_rows_bcp(
                sqlcmd,
                server,
                user,
                password,
                database,
                table,
                &metadata_file_names,
            )?
        }
    } else {
        Vec::new()
    };
    let elapsed = elapsed_ms(metadata_fetch_started);
    timings.prepare_metadata_fetch_ms += elapsed;
    if metadata_fetch_used_bcp {
        timings.prepare_metadata_fetch_bcp_ms += elapsed;
    }
    let standalone_body_rows = if !standalone_body_file_names.is_empty() {
        let metadata_fetch_started = Instant::now();
        let rows = fetch_config_rows_bcp(
            sqlcmd,
            server,
            user,
            password,
            database,
            table,
            &standalone_body_file_names,
        )?;
        let elapsed = elapsed_ms(metadata_fetch_started);
        timings.prepare_metadata_fetch_ms += elapsed;
        timings.prepare_metadata_fetch_bcp_ms += elapsed;
        rows
    } else {
        Vec::new()
    };
    let standalone_required_refs = if needs_standalone_refs && !extract_metadata_xml {
        standalone_content_reference_uuids_from_config_rows(&standalone_body_rows)
    } else {
        BTreeSet::new()
    };
    let selected_metadata_rows = if extract_metadata_xml
        || extract_module_text
        || needs_standalone_refs
        || needs_source_layout_refs
    {
        metadata_rows
            .iter()
            .filter(|row| metadata_file_names.contains(&row.file_name))
            .cloned()
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let metadata_texts_started = Instant::now();
    let selected_metadata_texts = if extract_metadata_xml
        || extract_module_text
        || needs_standalone_refs
        || needs_source_layout_refs
    {
        build_metadata_text_rows(&selected_metadata_rows)
    } else {
        Vec::new()
    };
    timings.prepare_metadata_texts_ms += elapsed_ms(metadata_texts_started);
    let selected_configuration_index_needs =
        selected_configuration_source_asset_index_needs_with_metadata(
            &file_names,
            &selected_metadata_texts,
        );
    let selected_metadata_index_needs = if selected_configuration_index_needs.is_none() {
        selected_metadata_source_reference_index_needs(&selected_metadata_texts)
    } else {
        None
    };
    let broad_metadata_indexes_without_command_refs = selected_file_names.is_empty()
        || selected_configuration_index_needs
            .is_some_and(SourceReferenceIndexNeeds::needs_broad_metadata_without_command_refs)
        || selected_metadata_index_needs
            .is_some_and(SourceReferenceIndexNeeds::needs_broad_metadata)
        || needs_standalone_refs
        || (selected_configuration_index_needs.is_none()
            && selected_metadata_index_needs.is_none()
            && selected_export_needs_broad_metadata_indexes(
                extract_module_text,
                &file_names,
                &selected_metadata_texts,
            ));
    let mut selected_configuration_body_rows = Vec::new();
    if !broad_metadata_indexes_without_command_refs
        && extract_metadata_xml
        && selected_configuration_index_needs
            .is_some_and(|needs| needs.command_refs || needs.metadata_refs)
    {
        let metadata_fetch_started = Instant::now();
        selected_configuration_body_rows =
            fetch_config_rows_bcp(sqlcmd, server, user, password, database, table, &file_names)?;
        let elapsed = elapsed_ms(metadata_fetch_started);
        timings.prepare_metadata_fetch_ms += elapsed;
        timings.prepare_metadata_fetch_bcp_ms += elapsed;
    }
    let selected_command_interface_refs =
        if selected_configuration_index_needs.is_some_and(|needs| needs.command_refs) {
            selected_configuration_command_interface_command_refs(&selected_configuration_body_rows)
        } else {
            Some(BTreeSet::new())
        };
    let mut broad_metadata_indexes = broad_metadata_indexes_without_command_refs;
    if selected_command_interface_refs.is_none() {
        broad_metadata_indexes = true;
    }
    if broad_metadata_indexes && !selected_file_names.is_empty() {
        let metadata_fetch_started = Instant::now();
        metadata_rows = fetch_metadata_rows_bcp(sqlcmd, server, user, password, database, table)?;
        let elapsed = elapsed_ms(metadata_fetch_started);
        timings.prepare_metadata_fetch_ms += elapsed;
        timings.prepare_metadata_fetch_bcp_ms += elapsed;
    } else if extract_metadata_xml
        && selected_configuration_index_needs
            .is_some_and(|needs| needs.command_refs || needs.metadata_refs)
    {
        let metadata_fetch_started = Instant::now();
        let targeted_metadata_file_names =
            selected_configuration_direct_metadata_reference_file_names(
                &selected_configuration_body_rows,
            );
        let mut metadata_fetch_used_bcp = false;
        if !targeted_metadata_file_names.is_empty() {
            metadata_fetch_used_bcp = true;
            metadata_rows = fetch_config_rows_bcp(
                sqlcmd,
                server,
                user,
                password,
                database,
                table,
                &targeted_metadata_file_names,
            )?;
        }
        let elapsed = elapsed_ms(metadata_fetch_started);
        timings.prepare_metadata_fetch_ms += elapsed;
        if metadata_fetch_used_bcp {
            timings.prepare_metadata_fetch_bcp_ms += elapsed;
        }
        if selected_configuration_index_needs.is_some_and(|needs| needs.command_refs) {
            let metadata_texts_started = Instant::now();
            let direct_metadata_texts = build_metadata_text_rows(&metadata_rows);
            let direct_command_refs =
                build_command_interface_reference_index_from_texts(&direct_metadata_texts);
            let direct_metadata_refs =
                build_metadata_command_reference_index_from_texts(&direct_metadata_texts);
            let unresolved_command_refs = unresolved_command_interface_reference_uuids(
                selected_command_interface_refs
                    .as_ref()
                    .expect("checked selected command interface refs"),
                &direct_command_refs,
                &direct_metadata_refs,
            );
            timings.prepare_metadata_texts_ms += elapsed_ms(metadata_texts_started);
            if !unresolved_command_refs.is_empty() {
                let metadata_fetch_started = Instant::now();
                let broad_metadata_rows =
                    fetch_metadata_rows_bcp(sqlcmd, server, user, password, database, table)?;
                let elapsed = elapsed_ms(metadata_fetch_started);
                timings.prepare_metadata_fetch_ms += elapsed;
                timings.prepare_metadata_fetch_bcp_ms += elapsed;
                let metadata_texts_started = Instant::now();
                let (owner_rows, resolved_uuids) = selected_owner_metadata_rows_for_uuids(
                    &broad_metadata_rows,
                    &unresolved_command_refs,
                );
                if !owner_rows.is_empty() {
                    metadata_rows = merge_config_rows_by_file_name(metadata_rows, owner_rows);
                }
                if !unresolved_command_refs.is_subset(&resolved_uuids) {
                    metadata_rows = broad_metadata_rows;
                    broad_metadata_indexes = true;
                }
                timings.prepare_metadata_texts_ms += elapsed_ms(metadata_texts_started);
            }
        }
    }
    if !selected_file_names.is_empty()
        && !broad_metadata_indexes
        && !selected_metadata_rows.is_empty()
    {
        let metadata_texts_started = Instant::now();
        let selected_form_file_names = form_metadata_file_names(&selected_metadata_rows);
        let direct_metadata_file_names = selected_metadata_direct_reference_file_names(
            &build_metadata_text_rows(&selected_metadata_rows),
        );
        timings.prepare_metadata_texts_ms += elapsed_ms(metadata_texts_started);
        if !direct_metadata_file_names.is_empty() {
            let metadata_fetch_started = Instant::now();
            let direct_metadata_rows = fetch_config_rows_bcp(
                sqlcmd,
                server,
                user,
                password,
                database,
                table,
                &direct_metadata_file_names,
            )?;
            let mut direct_form_file_names = form_metadata_file_names(&direct_metadata_rows);
            direct_form_file_names.extend(selected_form_file_names.iter().cloned());
            let elapsed = elapsed_ms(metadata_fetch_started);
            timings.prepare_metadata_fetch_ms += elapsed;
            timings.prepare_metadata_fetch_bcp_ms += elapsed;
            let resolved = direct_metadata_rows
                .iter()
                .map(|row| row.file_name.clone())
                .collect::<BTreeSet<_>>();
            let unresolved = direct_metadata_file_names
                .into_iter()
                .filter(|file_name| !resolved.contains(file_name))
                .collect::<BTreeSet<_>>();
            metadata_rows = merge_config_rows_by_file_name(metadata_rows, direct_metadata_rows);
            if !unresolved.is_empty() || !direct_form_file_names.is_empty() {
                let metadata_fetch_started = Instant::now();
                let broad_metadata_rows =
                    fetch_metadata_rows_bcp(sqlcmd, server, user, password, database, table)?;
                let elapsed = elapsed_ms(metadata_fetch_started);
                timings.prepare_metadata_fetch_ms += elapsed;
                timings.prepare_metadata_fetch_bcp_ms += elapsed;
                let metadata_texts_started = Instant::now();
                let (owner_rows, _) =
                    selected_owner_metadata_rows_for_uuids(&broad_metadata_rows, &unresolved);
                if !owner_rows.is_empty() {
                    metadata_rows = merge_config_rows_by_file_name(metadata_rows, owner_rows);
                }
                let form_owner_rows = selected_form_owner_metadata_rows(
                    &broad_metadata_rows,
                    &direct_form_file_names,
                );
                if !form_owner_rows.is_empty() {
                    metadata_rows = merge_config_rows_by_file_name(metadata_rows, form_owner_rows);
                }
                timings.prepare_metadata_texts_ms += elapsed_ms(metadata_texts_started);
            }
        } else if !selected_form_file_names.is_empty() {
            let metadata_fetch_started = Instant::now();
            let broad_metadata_rows =
                fetch_metadata_rows_bcp(sqlcmd, server, user, password, database, table)?;
            let elapsed = elapsed_ms(metadata_fetch_started);
            timings.prepare_metadata_fetch_ms += elapsed;
            timings.prepare_metadata_fetch_bcp_ms += elapsed;
            let metadata_texts_started = Instant::now();
            let form_owner_rows =
                selected_form_owner_metadata_rows(&broad_metadata_rows, &selected_form_file_names);
            if !form_owner_rows.is_empty() {
                metadata_rows = merge_config_rows_by_file_name(metadata_rows, form_owner_rows);
            }
            timings.prepare_metadata_texts_ms += elapsed_ms(metadata_texts_started);
        }
    }
    if !selected_file_names.is_empty() && !broad_metadata_indexes {
        let fetch_selected_rows_started = Instant::now();
        let selected_body_rows =
            fetch_config_rows_bcp(sqlcmd, server, user, password, database, table, &file_names)?;
        let elapsed = elapsed_ms(fetch_selected_rows_started);
        timings.prepare_metadata_fetch_ms += elapsed;
        timings.prepare_metadata_fetch_bcp_ms += elapsed;

        let metadata_texts_started = Instant::now();
        let direct_body_metadata_file_names =
            selected_body_direct_reference_file_names(&selected_body_rows);
        timings.prepare_metadata_texts_ms += elapsed_ms(metadata_texts_started);
        if !direct_body_metadata_file_names.is_empty() {
            let metadata_fetch_started = Instant::now();
            let direct_metadata_rows = fetch_config_rows_bcp(
                sqlcmd,
                server,
                user,
                password,
                database,
                table,
                &direct_body_metadata_file_names,
            )?;
            let elapsed = elapsed_ms(metadata_fetch_started);
            timings.prepare_metadata_fetch_ms += elapsed;
            timings.prepare_metadata_fetch_bcp_ms += elapsed;
            let resolved = direct_metadata_rows
                .iter()
                .map(|row| row.file_name.clone())
                .collect::<BTreeSet<_>>();
            let unresolved = direct_body_metadata_file_names
                .into_iter()
                .filter(|file_name| !resolved.contains(file_name))
                .collect::<BTreeSet<_>>();
            metadata_rows = merge_config_rows_by_file_name(metadata_rows, direct_metadata_rows);
            if !unresolved.is_empty() {
                let metadata_fetch_started = Instant::now();
                let broad_metadata_rows =
                    fetch_metadata_rows_bcp(sqlcmd, server, user, password, database, table)?;
                let elapsed = elapsed_ms(metadata_fetch_started);
                timings.prepare_metadata_fetch_ms += elapsed;
                timings.prepare_metadata_fetch_bcp_ms += elapsed;
                let metadata_texts_started = Instant::now();
                let (owner_rows, _) =
                    selected_owner_metadata_rows_for_uuids(&broad_metadata_rows, &unresolved);
                if !owner_rows.is_empty() {
                    metadata_rows = merge_config_rows_by_file_name(metadata_rows, owner_rows);
                }
                timings.prepare_metadata_texts_ms += elapsed_ms(metadata_texts_started);
            }
        }
    }
    let mut supplemental_owner_rows = Vec::new();
    if !selected_file_names.is_empty() && !broad_metadata_indexes && !metadata_rows.is_empty() {
        let mut known_metadata_file_names = metadata_rows
            .iter()
            .map(|row| row.file_name.clone())
            .collect::<BTreeSet<_>>();
        let mut frontier_rows = metadata_rows.clone();
        loop {
            let metadata_texts_started = Instant::now();
            let frontier_texts = build_metadata_text_rows(&frontier_rows);
            timings.prepare_metadata_texts_ms += elapsed_ms(metadata_texts_started);
            let next_metadata_file_names =
                selected_metadata_direct_reference_file_names(&frontier_texts)
                    .into_iter()
                    .filter(|file_name| !known_metadata_file_names.contains(file_name))
                    .collect::<BTreeSet<_>>();
            if next_metadata_file_names.is_empty() {
                break;
            }

            let metadata_fetch_started = Instant::now();
            let fetched_rows = fetch_config_rows_bcp(
                sqlcmd,
                server,
                user,
                password,
                database,
                table,
                &next_metadata_file_names,
            )?;
            let elapsed = elapsed_ms(metadata_fetch_started);
            timings.prepare_metadata_fetch_ms += elapsed;
            timings.prepare_metadata_fetch_bcp_ms += elapsed;
            if fetched_rows.is_empty() {
                break;
            }

            frontier_rows = fetched_rows
                .iter()
                .filter(|row| known_metadata_file_names.insert(row.file_name.clone()))
                .cloned()
                .collect::<Vec<_>>();
            if frontier_rows.is_empty() {
                break;
            }
            metadata_rows = merge_config_rows_by_file_name(metadata_rows, fetched_rows);
        }

        let metadata_texts_started = Instant::now();
        let supplemental_metadata_file_names = build_metadata_text_rows(&metadata_rows)
            .into_iter()
            .filter(|row| row.kind.as_deref() == Some("Subsystem"))
            .map(|row| row.file_name)
            .filter(|file_name| !metadata_file_names.contains(file_name))
            .collect::<BTreeSet<_>>();
        timings.prepare_metadata_texts_ms += elapsed_ms(metadata_texts_started);
        if !supplemental_metadata_file_names.is_empty() {
            let metadata_fetch_started = Instant::now();
            supplemental_owner_rows = fetch_metadata_owner_rows_bcp(
                sqlcmd,
                server,
                user,
                password,
                database,
                table,
                &supplemental_metadata_file_names,
            )?;
            let elapsed = elapsed_ms(metadata_fetch_started);
            timings.prepare_metadata_fetch_ms += elapsed;
            timings.prepare_metadata_fetch_bcp_ms += elapsed;
            file_names.extend(supplemental_metadata_file_names);
            file_names.extend(
                supplemental_owner_rows
                    .iter()
                    .map(|row| row.file_name.clone()),
            );
        }
    }

    let mut write_index_rows = rows_for_source_indexes(&headers, &selected_metadata_rows);
    if !metadata_rows.is_empty() {
        write_index_rows = merge_config_rows_by_file_name(write_index_rows, metadata_rows.clone());
    }
    if !supplemental_owner_rows.is_empty() {
        write_index_rows =
            merge_config_rows_by_file_name(write_index_rows, supplemental_owner_rows.clone());
    }
    let metadata_texts_started = Instant::now();
    let index_metadata_texts = if extract_metadata_xml
        || extract_module_text
        || needs_standalone_refs
        || needs_source_layout_refs
    {
        if broad_metadata_indexes {
            build_metadata_text_rows(&metadata_rows)
        } else if selected_configuration_index_needs.is_some() && !metadata_rows.is_empty() {
            build_metadata_text_rows(&metadata_rows)
        } else {
            build_metadata_text_rows(&selected_metadata_rows)
        }
    } else {
        Vec::new()
    };
    timings.prepare_metadata_texts_ms += elapsed_ms(metadata_texts_started);
    let reference_indexes_started = Instant::now();
    let metadata_texts_by_file_name = index_metadata_texts
        .iter()
        .map(|row| (row.file_name.as_str(), row))
        .collect::<BTreeMap<_, _>>();
    let recalculation_refs = if extract_metadata_xml {
        build_calculation_recalculation_reference_index(&index_metadata_texts)
    } else {
        BTreeMap::new()
    };

    let module_text_paths = if extract_module_text {
        module_body_paths_from_texts(&write_index_rows, &index_metadata_texts)
    } else {
        BTreeMap::new()
    };
    let index_part_started = Instant::now();
    let source_reference_needs = selected_configuration_index_needs
        .or(selected_metadata_index_needs)
        .unwrap_or_else(SourceReferenceIndexNeeds::full);
    let build_selected_local_refs = extract_metadata_xml && !selected_file_names.is_empty();
    let command_refs = if extract_metadata_xml
        && (source_reference_needs.command_refs || build_selected_local_refs)
    {
        build_command_interface_reference_index_from_texts(&index_metadata_texts)
    } else {
        BTreeMap::new()
    };
    timings.prepare_command_refs_ms += elapsed_ms(index_part_started);
    let index_part_started = Instant::now();
    let metadata_refs = if extract_metadata_xml
        && (source_reference_needs.metadata_refs || build_selected_local_refs)
    {
        build_metadata_command_reference_index_from_texts(&index_metadata_texts)
    } else {
        BTreeMap::new()
    };
    timings.prepare_metadata_refs_ms += elapsed_ms(index_part_started);
    let index_part_started = Instant::now();
    let MetadataTypeIndexes {
        references: type_index,
        dcs: dcs_type_index,
    } = if (extract_metadata_xml || needs_source_layout_refs)
        && (source_reference_needs.type_index || build_selected_local_refs)
    {
        build_metadata_type_indexes_from_texts(&index_metadata_texts)
    } else {
        MetadataTypeIndexes::default()
    };
    timings.prepare_type_index_ms += elapsed_ms(index_part_started);
    let index_part_started = Instant::now();
    let form_refs = if (extract_metadata_xml
        && (source_reference_needs.form_refs
            || source_reference_needs.object_refs
            || build_selected_local_refs))
        || needs_standalone_refs
    {
        build_complete_form_source_reference_index(&index_metadata_texts)
    } else {
        BTreeMap::new()
    };
    timings.prepare_form_refs_ms += elapsed_ms(index_part_started);
    let index_part_started = Instant::now();
    let template_refs = if (extract_metadata_xml
        && (source_reference_needs.template_refs || build_selected_local_refs))
        || needs_standalone_refs
    {
        build_template_source_reference_index_from_texts(&metadata_rows, &index_metadata_texts)
    } else {
        BTreeMap::new()
    };
    timings.prepare_template_refs_ms += elapsed_ms(index_part_started);
    let index_part_started = Instant::now();
    let subsystem_refs = if (extract_metadata_xml
        && (source_reference_needs.subsystem_refs || build_selected_local_refs))
        || needs_standalone_refs
    {
        build_subsystem_source_reference_index_from_texts(&index_metadata_texts)
    } else {
        BTreeMap::new()
    };
    timings.prepare_subsystem_refs_ms += elapsed_ms(index_part_started);
    let index_part_started = Instant::now();
    let object_refs = if (extract_metadata_xml || needs_source_layout_refs)
        && (source_reference_needs.object_refs || build_selected_local_refs)
    {
        build_metadata_object_reference_index_from_texts(&index_metadata_texts)
    } else if needs_standalone_refs {
        build_standalone_object_reference_index_from_texts(
            &index_metadata_texts,
            &standalone_required_refs,
            &form_refs,
            &template_refs,
            &subsystem_refs,
        )
    } else {
        BTreeMap::new()
    };
    let configuration_root_object_refs = if extract_metadata_xml {
        build_configuration_root_object_reference_index_from_texts(
            &index_metadata_texts,
            &object_refs,
        )
    } else {
        BTreeMap::new()
    };
    let role_rights_object_refs =
        build_role_rights_object_reference_index(&object_refs, &form_refs);
    timings.prepare_object_refs_ms += elapsed_ms(index_part_started);
    let metadata_order = if (extract_metadata_xml || needs_source_layout_refs)
        && source_reference_needs.metadata_order
    {
        build_metadata_order_index_from_texts(&index_metadata_texts)
    } else {
        BTreeMap::new()
    };
    let index_part_started = Instant::now();
    let field_refs = if extract_metadata_xml && source_reference_needs.field_refs {
        build_metadata_field_reference_index_from_texts(&index_metadata_texts)
    } else {
        BTreeMap::new()
    };
    timings.prepare_field_refs_ms += elapsed_ms(index_part_started);
    let index_part_started = Instant::now();
    let functional_option_refs =
        if extract_metadata_xml && source_reference_needs.functional_option_refs {
            build_functional_option_reference_index_from_texts(
                &index_metadata_texts,
                &object_refs,
                &form_refs,
                &template_refs,
                &subsystem_refs,
            )
        } else {
            BTreeMap::new()
        };
    timings.prepare_functional_option_refs_ms += elapsed_ms(index_part_started);
    let source_asset_metadata_texts = &index_metadata_texts;
    let index_part_started = Instant::now();
    let source_assets = source_asset_paths_with_indexes(
        &write_index_rows,
        source_asset_metadata_texts,
        &command_refs,
        &metadata_refs,
        &object_refs,
        &field_refs,
        &type_index,
        &form_refs,
        &template_refs,
        &subsystem_refs,
    );
    let source_asset_diagnostics =
        build_form_owner_resolution_diagnostics_from_texts(source_asset_metadata_texts);
    timings.prepare_source_assets_ms += elapsed_ms(index_part_started);
    let index_part_started = Instant::now();
    let help_refs = if extract_metadata_xml
        && (source_reference_needs.help_refs || build_selected_local_refs)
    {
        build_help_reference_index(&object_refs, &form_refs, &template_refs, &subsystem_refs)
    } else {
        BTreeMap::new()
    };
    timings.prepare_help_refs_ms += elapsed_ms(index_part_started);
    let index_part_started = Instant::now();
    let standalone_refs = if (needs_standalone_refs
        || (extract_metadata_xml && source_reference_needs.standalone_refs))
        && source_assets
            .values()
            .any(|asset| matches!(asset.kind, SourceAssetKind::StandaloneContent))
    {
        if extract_metadata_xml {
            build_standalone_content_references(
                &index_metadata_texts,
                &object_refs,
                &form_refs,
                &template_refs,
                &subsystem_refs,
            )
        } else {
            build_standalone_content_references_for_uuids(
                &index_metadata_texts,
                &standalone_required_refs,
                &object_refs,
                &form_refs,
                &template_refs,
                &subsystem_refs,
            )
        }
    } else {
        StandaloneContentReferences::default()
    };
    timings.prepare_standalone_refs_ms += elapsed_ms(index_part_started);
    let index_part_started = Instant::now();
    let body_owners = if (extract_metadata_xml
        && (source_reference_needs.body_owners || build_selected_local_refs))
        || needs_source_layout_refs
    {
        build_body_owner_source_index_from_texts(&index_metadata_texts, &subsystem_refs)
    } else {
        BTreeMap::new()
    };
    timings.prepare_body_owners_ms += elapsed_ms(index_part_started);
    let needs_predefined_item_refs =
        predefined_data_needs_item_references(&file_names, &body_owners);
    let predefined_item_refs = if needs_predefined_item_refs {
        let predefined_body_file_names = predefined_data_body_file_names(&body_owners);
        let metadata_fetch_started = Instant::now();
        let rows = if predefined_body_file_names.is_empty() {
            Vec::new()
        } else {
            fetch_config_rows_bcp(
                sqlcmd,
                server,
                user,
                password,
                database,
                table,
                &predefined_body_file_names,
            )?
        };
        let elapsed = elapsed_ms(metadata_fetch_started);
        timings.prepare_metadata_fetch_ms += elapsed;
        if !predefined_body_file_names.is_empty() {
            timings.prepare_metadata_fetch_bcp_ms += elapsed;
        }
        build_predefined_item_reference_index(&rows, &body_owners, &type_index, &object_refs)?
    } else {
        BTreeMap::new()
    };
    let metadata_value_owner_file_names =
        streamed_metadata_value_owner_file_names(&index_metadata_texts, &selected_file_names);
    let owner_ids = selected_metadata_predefined_owner_ids(
        &index_metadata_texts,
        &metadata_value_owner_file_names,
        &type_index,
        &object_refs,
        &body_owners,
    );
    let required_body_owners = body_owners
        .iter()
        .filter(|(uuid, _)| owner_ids.contains(*uuid))
        .map(|(uuid, owner)| (uuid.clone(), owner.clone()))
        .collect::<BTreeMap<_, _>>();
    let body_file_names = predefined_data_body_file_names(&required_body_owners);
    let metadata_fetch_started = Instant::now();
    let rows = if body_file_names.is_empty() {
        Vec::new()
    } else {
        fetch_config_rows_bcp(
            sqlcmd,
            server,
            user,
            password,
            database,
            table,
            &body_file_names,
        )?
    };
    let elapsed = elapsed_ms(metadata_fetch_started);
    timings.prepare_metadata_fetch_ms += elapsed;
    if !body_file_names.is_empty() {
        timings.prepare_metadata_fetch_bcp_ms += elapsed;
    }
    let metadata_value_predefined_item_refs = build_predefined_item_reference_index(
        &rows,
        &required_body_owners,
        &type_index,
        &object_refs,
    )?;
    let mut metadata_object_refs = object_refs.clone();
    extend_metadata_owner_value_references(&mut metadata_object_refs, &predefined_item_refs)?;
    extend_metadata_owner_value_references(
        &mut metadata_object_refs,
        &metadata_value_predefined_item_refs,
    )?;
    let configuration_module_groups = configuration_module_groups(&file_names);
    ensure_unique_source_asset_paths(&source_assets, &source_asset_diagnostics)?;
    timings.prepare_reference_indexes_ms += elapsed_ms(reference_indexes_started);
    timings.prepare_indexes_ms = elapsed_ms(prepare_started);

    let context = DumpRowContext {
        output_dir,
        table,
        source_version,
        write_binary_rows,
        inflate,
        extract_module_text,
        extract_metadata_xml,
        module_text_paths: &module_text_paths,
        source_assets: &source_assets,
        metadata_texts_by_file_name: &metadata_texts_by_file_name,
        command_refs: &command_refs,
        metadata_refs: &metadata_refs,
        type_index: &type_index,
        dcs_type_index: &dcs_type_index,
        object_refs: &object_refs,
        metadata_object_refs: &metadata_object_refs,
        configuration_root_object_refs: &configuration_root_object_refs,
        recalculation_refs: &recalculation_refs,
        predefined_item_refs: &predefined_item_refs,
        role_rights_object_refs: &role_rights_object_refs,
        metadata_order: &metadata_order,
        field_refs: &field_refs,
        functional_option_refs: &functional_option_refs,
        help_refs: &help_refs,
        standalone_refs: &standalone_refs,
        form_refs: &form_refs,
        template_refs: &template_refs,
        subsystem_refs: &subsystem_refs,
        file_names: &file_names,
        body_owners: &body_owners,
        configuration_module_groups: &configuration_module_groups,
    };

    let mut manifests = Vec::with_capacity(file_names.len());
    let mut binary_bytes = 0;
    let mut inflated_rows = 0;
    let mut module_text_rows = 0;
    let mut metadata_xml_rows = 0;
    let mut source_asset_rows = 0;
    let mut versions_blob = None;
    let file_name_batches = build_dump_file_name_batches(&headers, &file_names);
    for chunk in file_name_batches {
        let selected = chunk.iter().cloned().collect::<BTreeSet<_>>();
        let fetch_started = Instant::now();
        let rows = fetch_binary_rows_bcp(
            sqlcmd,
            server,
            user,
            password,
            database,
            table,
            &selected,
            selected_file_names.is_empty(),
        )
        .with_context(|| {
            let first = chunk.first().map(String::as_str).unwrap_or("<empty>");
            let last = chunk.last().map(String::as_str).unwrap_or("<empty>");
            format!("failed to fetch {table} rows batch {first}..{last}")
        })?;
        let elapsed = elapsed_ms(fetch_started);
        timings.fetch_rows_ms += elapsed;
        timings.fetch_rows_bcp_ms += elapsed;
        timings.fetch_row_batches += 1;
        timings.fetch_row_batch_max_rows = timings.fetch_row_batch_max_rows.max(rows.len() as u64);
        let batch_binary_bytes = rows.iter().map(|row| row.binary.len() as u64).sum::<u64>();
        timings.fetch_row_batch_max_binary_bytes = timings
            .fetch_row_batch_max_binary_bytes
            .max(batch_binary_bytes);
        if generate_config_dump_info {
            for row in rows.iter().filter(|row| row.file_name == "versions") {
                if row.part_no != 0 {
                    bail!("Config versions row has unsupported PartNo {}", row.part_no);
                }
                if versions_blob.replace(row.binary.clone()).is_some() {
                    bail!("Config versions row was fetched more than once");
                }
            }
        }
        let process_started = Instant::now();
        let dumped_rows = parallel::install(|| {
            rows.par_iter()
                .map(|row| dump_table_binary_row(&context, row))
                .collect::<Vec<_>>()
        })?;
        timings.process_rows_wall_ms += elapsed_ms(process_started);
        for dumped in dumped_rows {
            let dumped = dumped?;
            binary_bytes += dumped.binary_bytes;
            inflated_rows += dumped.inflated_rows;
            module_text_rows += dumped.module_text_rows;
            metadata_xml_rows += dumped.metadata_xml_rows;
            source_asset_rows += dumped.source_asset_rows;
            timings.add_assign(&dumped.timings);
            manifests.push(dumped.manifest);
        }
    }

    if generate_config_dump_info {
        let started = Instant::now();
        let mut emitted_source_asset_paths = BTreeMap::<String, PathBuf>::new();
        for manifest in &manifests {
            let Some(path) = manifest.source_asset_path.as_ref().map(PathBuf::from) else {
                continue;
            };
            if let Some(previous) =
                emitted_source_asset_paths.insert(manifest.file_name.clone(), path.clone())
                && previous != path
            {
                bail!(
                    "Config source asset {} was emitted to both {} and {}",
                    manifest.file_name,
                    previous.display(),
                    path.display()
                );
            }
        }
        write_config_dump_info(
            output_dir,
            source_version,
            versions_blob
                .as_deref()
                .ok_or_else(|| anyhow!("full Config export has no versions row"))?,
            ConfigDumpInfoInventory {
                file_names: &file_names,
                metadata_texts: &index_metadata_texts,
                object_refs: &object_refs,
                form_refs: &form_refs,
                template_refs: &template_refs,
                subsystem_refs: &subsystem_refs,
                module_text_paths: &module_text_paths,
                source_assets: &source_assets,
                emitted_source_asset_paths: &emitted_source_asset_paths,
                configuration_module_groups: &configuration_module_groups,
            },
        )?;
        timings.source_asset_config_dump_info_cpu_ms += elapsed_ms(started);
    }

    Ok(DumpedTable {
        rows: manifests,
        binary_bytes,
        inflated_rows,
        module_text_rows,
        metadata_xml_rows,
        source_asset_rows,
        timings,
    })
}

fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
}

fn build_dump_file_name_batches(
    headers: &[ConfigRowHeader],
    file_names: &BTreeSet<String>,
) -> Vec<Vec<String>> {
    let mut file_sizes = BTreeMap::<&str, u64>::new();
    for header in headers {
        file_sizes
            .entry(header.file_name.as_str())
            .and_modify(|size| *size = (*size).max(header.data_size.max(0) as u64))
            .or_insert(header.data_size.max(0) as u64);
    }

    let mut batches = Vec::new();
    let mut current = Vec::new();
    let mut current_bytes = 0u64;
    for file_name in file_names {
        let file_bytes = file_sizes
            .get(file_name.as_str())
            .copied()
            .unwrap_or_default();
        let would_exceed_bytes = current_bytes > 0
            && current_bytes.saturating_add(file_bytes) > SQLCMD_DUMP_BATCH_MAX_DATA_BYTES;
        let would_exceed_count = current.len() >= SQLCMD_DUMP_FILE_BATCH_SIZE;
        if !current.is_empty() && (would_exceed_bytes || would_exceed_count) {
            batches.push(std::mem::take(&mut current));
            current_bytes = 0;
        }
        current.push(file_name.clone());
        current_bytes = current_bytes.saturating_add(file_bytes);
    }
    if !current.is_empty() {
        batches.push(current);
    }
    batches
}

fn rows_for_source_indexes(
    headers: &[ConfigRowHeader],
    metadata_rows: &[ConfigRow],
) -> Vec<ConfigRow> {
    let metadata_file_names = metadata_rows
        .iter()
        .map(|row| row.file_name.as_str())
        .collect::<BTreeSet<_>>();
    let mut rows = metadata_rows.to_vec();
    let mut seen = BTreeSet::<&str>::new();
    for header in headers {
        if metadata_file_names.contains(header.file_name.as_str())
            || !seen.insert(header.file_name.as_str())
        {
            continue;
        }
        rows.push(ConfigRow {
            file_name: header.file_name.clone(),
            part_no: header.part_no,
            data_size: header.data_size,
            binary_hex: String::new(),
        });
    }
    rows
}

fn build_metadata_text_rows(rows: &[ConfigRow]) -> Vec<MetadataTextRow> {
    rows.iter()
        .filter(|row| !row.file_name.contains('.'))
        .filter_map(|row| {
            let bytes = decode_hex(&row.binary_hex).ok()?;
            let mut metadata = metadata_text_row_from_blob(&row.file_name, &bytes)?;
            if is_direct_code14_form_metadata_text(&metadata.text, &metadata.file_name) {
                metadata.kind = Some("Form".to_string());
                metadata.folder = None;
            }
            Some(metadata)
        })
        .collect()
}

fn form_metadata_file_names(rows: &[ConfigRow]) -> BTreeSet<String> {
    build_metadata_text_rows(rows)
        .into_iter()
        .filter(|row| {
            is_form_metadata_text(&row.text, &row.file_name)
                || is_direct_code14_form_metadata_text(&row.text, &row.file_name)
        })
        .map(|row| row.file_name)
        .collect()
}

fn is_direct_code14_form_metadata_text(text: &str, uuid: &str) -> bool {
    if parse_metadata_object_code(text) != Some(14) {
        return false;
    }
    let Some(fields) = metadata_object_fields(text) else {
        return false;
    };
    fields.len() == 6
        && fields.first().map(|field| field.trim()) == Some("14")
        && metadata_header_field_index(&fields, uuid) == Some(1)
        && information_register_bool(fields[2]).is_some()
        && information_register_bool(fields[3]).is_some()
        && direct_form_application_purposes_are_valid(fields[4])
        && information_register_bool(fields[5]).is_some()
}

fn direct_form_application_purposes_are_valid(value: &str) -> bool {
    let Some(fields) = split_1c_braced_fields(value, 0) else {
        return false;
    };
    let Some(count) = fields
        .first()
        .and_then(|field| field.trim().parse::<usize>().ok())
    else {
        return false;
    };
    if count.checked_add(1) != Some(fields.len()) {
        return false;
    }
    fields.iter().skip(1).all(|field| {
        let Some(item) = split_1c_braced_fields(field, 0) else {
            return false;
        };
        item.len() == 3
            && item.first().map(|field| field.trim()) == Some(r##""#""##)
            && item
                .get(1)
                .and_then(|field| parse_uuid_field(field.trim()))
                .as_deref()
                == Some(FORM_APPLICATION_USE_PURPOSE_TYPE_UUID)
            && matches!(item.get(2).map(|field| field.trim()), Some("1" | "2"))
    })
}

fn build_direct_code14_form_source_reference_index(
    rows: &[MetadataTextRow],
) -> BTreeMap<String, FormSourceReference> {
    let forms = rows
        .iter()
        .filter(|row| is_direct_code14_form_metadata_text(&row.text, &row.file_name))
        .filter_map(|row| row.header.clone())
        .collect::<Vec<_>>();
    let form_uuids = forms
        .iter()
        .map(|form| form.uuid.clone())
        .collect::<BTreeSet<_>>();
    if form_uuids.is_empty() {
        return BTreeMap::new();
    }

    let mut owner_paths_by_ref = BTreeMap::<String, BTreeSet<PathBuf>>::new();
    for row in rows {
        if is_form_metadata_text(&row.text, &row.file_name)
            || is_direct_code14_form_metadata_text(&row.text, &row.file_name)
        {
            continue;
        }
        let (Some(kind), Some(folder), Some(header)) =
            (row.kind.as_deref(), row.folder, row.header.as_ref())
        else {
            continue;
        };
        if !metadata_kind_can_own_forms(kind) {
            continue;
        }
        let Some(references) = owned_form_uuid_values_matching(&row.text, &form_uuids) else {
            continue;
        };
        let owner_path = PathBuf::from(folder).join(sanitize_source_path_segment(&header.name));
        for reference in references {
            owner_paths_by_ref
                .entry(reference)
                .or_default()
                .insert(owner_path.clone());
        }
    }

    forms
        .into_iter()
        .filter_map(|form| {
            let owner_paths = owner_paths_by_ref.get(&form.uuid)?;
            let mut owners = owner_paths.iter();
            let owner = owners.next()?;
            if owners.next().is_some() {
                return None;
            }
            Some((
                form.uuid,
                FormSourceReference {
                    relative_path: owner
                        .join("Forms")
                        .join(sanitize_source_path_segment(&form.name))
                        .with_extension("xml"),
                    kind: "Form",
                },
            ))
        })
        .collect()
}

fn build_complete_form_source_reference_index(
    rows: &[MetadataTextRow],
) -> BTreeMap<String, FormSourceReference> {
    let mut refs = build_form_source_reference_index_from_texts(rows);
    refs.extend(build_direct_code14_form_source_reference_index(rows));
    refs
}

fn selected_form_owner_metadata_rows(
    rows: &[ConfigRow],
    form_file_names: &BTreeSet<String>,
) -> Vec<ConfigRow> {
    if form_file_names.is_empty() {
        return Vec::new();
    }
    let owner_file_names = build_metadata_text_rows(rows)
        .into_iter()
        .filter(|row| {
            row.kind.as_deref().is_some_and(metadata_kind_can_own_forms)
                && owned_form_uuid_values_matching(&row.text, form_file_names)
                    .is_some_and(|references| !references.is_empty())
        })
        .map(|row| row.file_name)
        .collect::<BTreeSet<_>>();
    rows.iter()
        .filter(|row| owner_file_names.contains(&row.file_name))
        .cloned()
        .collect()
}

fn selected_metadata_predefined_owner_ids(
    rows: &[MetadataTextRow],
    selected_file_names: &BTreeSet<String>,
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
    body_owners: &BTreeMap<String, BodyOwnerSourceReference>,
) -> BTreeSet<String> {
    let mut owners = BTreeSet::new();
    for row in rows {
        if !matches!(
            row.kind.as_deref(),
            Some("InformationRegister" | "Catalog" | "Document")
        ) || !selected_file_names.contains(&row.file_name)
        {
            continue;
        }
        let mut pending = vec![row.text.as_str()];
        while let Some(value) = pending.pop() {
            if let Some((owner_uuid, value_uuid)) =
                parse_information_register_design_time_ref_ids(value)
                && !information_register_uuid_is_zero(&owner_uuid)
                && !information_register_uuid_is_zero(&value_uuid)
                && let Some(owner_reference) = information_register_design_time_owner_reference(
                    &owner_uuid,
                    type_index,
                    object_refs,
                )
                && !object_refs.get(&value_uuid).is_some_and(|reference| {
                    information_register_reference_belongs_to_owner(&owner_reference, reference)
                })
            {
                let mut matches = body_owners
                    .keys()
                    .filter(|candidate| object_refs.get(*candidate) == Some(&owner_reference));
                if let (Some(owner_metadata_uuid), None) = (matches.next().cloned(), matches.next())
                {
                    owners.insert(owner_metadata_uuid);
                }
            }
            let Some(fields) = split_1c_braced_fields(value, 0) else {
                continue;
            };
            pending.extend(
                fields
                    .into_iter()
                    .filter(|field| field.trim_start().starts_with('{')),
            );
        }
    }
    owners
}

fn metadata_value_owner_file_names(rows: &[MetadataTextRow]) -> BTreeSet<String> {
    rows.iter()
        .filter(|row| {
            matches!(
                row.kind.as_deref(),
                Some("InformationRegister" | "Catalog" | "Document")
            )
        })
        .map(|row| row.file_name.clone())
        .collect()
}

fn eager_metadata_value_owner_file_names(rows: &[MetadataTextRow]) -> BTreeSet<String> {
    metadata_value_owner_file_names(rows)
}

fn streamed_metadata_value_owner_file_names(
    rows: &[MetadataTextRow],
    selected_file_names: &BTreeSet<String>,
) -> BTreeSet<String> {
    if selected_file_names.is_empty() {
        metadata_value_owner_file_names(rows)
    } else {
        selected_file_names.clone()
    }
}

fn merge_config_rows_by_file_name(
    mut left: Vec<ConfigRow>,
    right: Vec<ConfigRow>,
) -> Vec<ConfigRow> {
    let mut rows = BTreeMap::<String, ConfigRow>::new();
    for row in left.drain(..).chain(right) {
        rows.insert(row.file_name.clone(), row);
    }
    rows.into_values().collect()
}

fn dump_table_row(context: &DumpRowContext<'_>, row: &ConfigRow) -> Result<DumpedRow> {
    let bytes = decode_hex(&row.binary_hex)
        .with_context(|| format!("failed to decode {} row {}", context.table, row.file_name))?;
    dump_table_row_bytes(context, &row.file_name, row.part_no, row.data_size, &bytes)
}

fn dump_table_binary_row(context: &DumpRowContext<'_>, row: &BinaryConfigRow) -> Result<DumpedRow> {
    dump_table_row_bytes(
        context,
        &row.file_name,
        row.part_no,
        row.data_size,
        &row.binary,
    )
}

fn dump_table_row_bytes(
    context: &DumpRowContext<'_>,
    file_name: &str,
    part_no: i32,
    data_size: i64,
    bytes: &[u8],
) -> Result<DumpedRow> {
    let mut timings = MssqlDumpTimingReport::default();
    if bytes.len() != data_size as usize {
        bail!(
            "{} row {} DataSize {} does not match BinaryData length {}",
            context.table,
            file_name,
            data_size,
            bytes.len()
        );
    }

    let safe_name = safe_storage_file_name(file_name, part_no);
    let binary_relative = if context.write_binary_rows {
        Some(PathBuf::from(context.table).join(format!("{safe_name}.bin")))
    } else {
        None
    };
    if context.write_binary_rows {
        let started = Instant::now();
        let binary_path = context
            .output_dir
            .join(binary_relative.as_ref().expect("binary path is present"));
        fs::write(&binary_path, &bytes)
            .with_context(|| format!("failed to write {}", binary_path.display()))?;
        timings.binary_write_cpu_ms += elapsed_ms(started);
    }

    let mut inflated_rows = 0;
    let inflated_relative = if context.inflate {
        let started = Instant::now();
        match inflate_raw_deflate(&bytes) {
            Ok(inflated) => {
                let relative = PathBuf::from(format!("{}_inflated", context.table))
                    .join(format!("{safe_name}.txt"));
                let path = context.output_dir.join(&relative);
                fs::write(&path, inflated)
                    .with_context(|| format!("failed to write {}", path.display()))?;
                inflated_rows = 1;
                timings.inflate_cpu_ms += elapsed_ms(started);
                Some(relative.to_string_lossy().replace('\\', "/"))
            }
            Err(_) => {
                timings.inflate_cpu_ms += elapsed_ms(started);
                None
            }
        }
    } else {
        None
    };

    let static_source_asset = context.source_assets.get(file_name);
    let parsed_form_body = if matches!(
        static_source_asset.map(|asset| &asset.kind),
        Some(SourceAssetKind::Form)
    ) {
        let started = Instant::now();
        let parsed = parse_form_body_blob(bytes).ok();
        timings.form_body_parse_cpu_ms += elapsed_ms(started);
        parsed
    } else {
        None
    };

    let mut module_text_rows = 0;
    let module_text_relative = if context.extract_module_text {
        let started = Instant::now();
        let module_text = match unpack_module_blob_text(&bytes) {
            Ok(text) => Some(text),
            Err(_) if let Some(body) = parsed_form_body.as_ref() => {
                form_body_module_text_bytes(body)
            }
            Err(_) if context.module_text_paths.contains_key(file_name) => {
                unpack_form_body_module_text(&bytes)
            }
            Err(_) => None,
        };
        match module_text {
            Some(text) => {
                let relative = context
                    .module_text_paths
                    .get(file_name)
                    .cloned()
                    .unwrap_or_else(|| {
                        PathBuf::from(format!("{}_module_text", context.table))
                            .join(format!("{safe_name}.bsl"))
                    });
                let path = context.output_dir.join(&relative);
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent)
                        .with_context(|| format!("failed to create {}", parent.display()))?;
                }
                fs::write(&path, text)
                    .with_context(|| format!("failed to write {}", path.display()))?;
                module_text_rows = 1;
                timings.module_text_cpu_ms += elapsed_ms(started);
                Some(relative.to_string_lossy().replace('\\', "/"))
            }
            None => {
                timings.module_text_cpu_ms += elapsed_ms(started);
                None
            }
        }
    } else {
        None
    };

    let mut metadata_xml_rows = 0;
    let metadata_xml_relative = if context.extract_metadata_xml {
        let started = Instant::now();
        let extracted = if let Some(row) = context.metadata_texts_by_file_name.get(file_name) {
            extract_metadata_source_xml_from_text_row(
                row,
                context.type_index,
                context.object_refs,
                context.metadata_object_refs,
                context.configuration_root_object_refs,
                context.recalculation_refs,
                context.functional_option_refs,
                context.form_refs,
                context.template_refs,
                context.subsystem_refs,
                context.source_version,
            )
        } else {
            extract_metadata_source_xml_with_recalculation_refs(
                &bytes,
                file_name,
                context.type_index,
                context.object_refs,
                context.metadata_object_refs,
                context.configuration_root_object_refs,
                context.recalculation_refs,
                context.functional_option_refs,
                context.form_refs,
                context.template_refs,
                context.subsystem_refs,
                context.source_version,
            )
        };
        match extracted {
            Some(extracted) => {
                let path = context.output_dir.join(&extracted.relative_path);
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent)
                        .with_context(|| format!("failed to create {}", parent.display()))?;
                }
                write_source_xml_file(&path, extracted.xml, context.source_version)?;
                metadata_xml_rows = 1;
                timings.metadata_xml_cpu_ms += elapsed_ms(started);
                Some(extracted.relative_path.to_string_lossy().replace('\\', "/"))
            }
            None => {
                timings.metadata_xml_cpu_ms += elapsed_ms(started);
                None
            }
        }
    } else {
        None
    };

    let mut source_asset_rows = 0;
    let source_asset_relative = if metadata_xml_relative.is_none() {
        let started = Instant::now();
        let dynamic_asset;
        let asset = match context.source_assets.get(file_name) {
            Some(asset) => Some(asset),
            None => {
                dynamic_asset = dynamic_source_asset(context, file_name, &bytes);
                dynamic_asset.as_ref()
            }
        };
        match asset {
            Some(asset) => {
                let relative = write_source_asset(
                    context,
                    asset,
                    &bytes,
                    parsed_form_body.as_ref(),
                    &mut timings,
                )?;
                source_asset_rows = 1;
                let elapsed = elapsed_ms(started);
                timings.source_asset_cpu_ms += elapsed;
                timings.add_source_asset_kind(&asset.kind, elapsed);
                Some(relative.to_string_lossy().replace('\\', "/"))
            }
            None => {
                timings.source_asset_cpu_ms += elapsed_ms(started);
                None
            }
        }
    } else {
        None
    };

    Ok(DumpedRow {
        manifest: MssqlDumpRowManifest {
            file_name: file_name.to_string(),
            part_no,
            data_size,
            binary_bytes: bytes.len(),
            binary_path: binary_relative.map(|path| path.to_string_lossy().replace('\\', "/")),
            inflated_path: inflated_relative,
            module_text_path: module_text_relative,
            metadata_xml_path: metadata_xml_relative,
            source_asset_path: source_asset_relative,
        },
        binary_bytes: bytes.len(),
        inflated_rows,
        module_text_rows,
        metadata_xml_rows,
        source_asset_rows,
        timings,
    })
}

#[derive(Debug)]
struct ExchangePlanContentItem {
    metadata_id: String,
    metadata: String,
    auto_record: &'static str,
}

struct BusinessProcessFlowchart {
    items: Vec<FlowchartItem>,
}

struct FlowchartItem {
    tag: &'static str,
    id: String,
    uuid: Option<String>,
    name: String,
    title: Vec<(String, String)>,
    tab_order: String,
    z_order: String,
    properties: FlowchartItemProperties,
    events: Vec<FlowchartEvent>,
}

struct FlowchartItemProperties {
    location: Option<FlowchartLocation>,
    pivot_points: Vec<FlowchartPoint>,
    from: Option<FlowchartConnectionEnd>,
    to: Option<FlowchartConnectionEnd>,
    font: FlowchartFont,
    decorative_line: bool,
    line_style: &'static str,
    begin_arrow: &'static str,
    end_arrow: &'static str,
    transparent: bool,
    horizontal_align: &'static str,
    explanation: Option<String>,
    task_description: Option<String>,
    subprocess: Option<String>,
    true_port_index: Option<String>,
    false_port_index: Option<String>,
}

struct FlowchartLocation {
    left: String,
    top: String,
    right: String,
    bottom: String,
}

struct FlowchartPoint {
    x: String,
    y: String,
}

struct FlowchartConnectionEnd {
    item: String,
    port_index: String,
}

struct FlowchartFont {
    reference: String,
    kind: &'static str,
    height: Option<String>,
    bold: bool,
    italic: bool,
    underline: bool,
    strikeout: bool,
    scale: Option<String>,
}

struct FlowchartEvent {
    name: &'static str,
    handler: Option<String>,
}

struct FlowchartBase {
    id: String,
    uuid: Option<String>,
    name: String,
    title: Vec<(String, String)>,
    tab_order: String,
}

#[derive(Clone)]
struct MetadataCommandReference {
    kind: String,
    name: String,
}

fn parse_exchange_plan_content_blob(
    bytes: &[u8],
    object_refs: &BTreeMap<String, String>,
    type_index: &BTreeMap<String, String>,
    metadata_order: &BTreeMap<String, usize>,
) -> Result<Vec<ExchangePlanContentItem>> {
    let inflated = inflate_raw_deflate(bytes).context("failed to inflate ExchangePlanContent")?;
    let text = String::from_utf8(inflated).context("ExchangePlanContent is not valid UTF-8")?;
    let fields = split_1c_braced_fields(text.trim_start_matches('\u{feff}'), 0)
        .context("ExchangePlanContent body is not a braced 1C value")?;
    let marker = fields
        .first()
        .map(|field| field.trim())
        .context("ExchangePlanContent body is empty")?;
    if marker != "2" {
        bail!("unsupported ExchangePlanContent marker {marker}, expected 2");
    }
    let count = fields
        .get(1)
        .context("ExchangePlanContent body has no item count")?
        .trim()
        .parse::<usize>()
        .context("ExchangePlanContent item count is not numeric")?;
    let mut items = Vec::with_capacity(count);
    let mut index = 2usize;
    for item_index in 0..count {
        let object_slot = fields
            .get(index)
            .with_context(|| format!("ExchangePlanContent item {item_index} has no metadata id"))?;
        let object_id = parse_uuid_field(object_slot.trim()).with_context(|| {
            format!(
                "ExchangePlanContent item {item_index} metadata id is not a UUID: {}",
                object_slot.trim()
            )
        })?;
        let auto_record_slot = fields.get(index + 1).with_context(|| {
            format!("ExchangePlanContent item {item_index} has no AutoRecord value")
        })?;
        let auto_record = exchange_plan_auto_record_xml(auto_record_slot.trim());
        let metadata = object_refs
            .get(&object_id)
            .or_else(|| type_index.get(&object_id))
            .cloned()
            .with_context(|| {
                format!(
                    "ExchangePlanContent item {item_index} references unsupported metadata id {object_id}"
                )
            })?;
        items.push(ExchangePlanContentItem {
            metadata_id: object_id,
            metadata,
            auto_record,
        });
        index += 2;
    }

    items.sort_by_key(|item| {
        metadata_order
            .get(&item.metadata_id)
            .copied()
            .unwrap_or(usize::MAX)
    });

    Ok(items)
}

fn parse_business_process_flowchart_blob(
    bytes: &[u8],
    object_refs: &BTreeMap<String, String>,
) -> Option<BusinessProcessFlowchart> {
    let inflated = inflate_raw_deflate(bytes).ok()?;
    let text = String::from_utf8(inflated).ok()?;
    parse_business_process_flowchart_text(text.trim_start_matches('\u{feff}'), object_refs)
}

fn parse_business_process_flowchart_text(
    text: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<BusinessProcessFlowchart> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.first()?.trim() != "5" {
        return None;
    }
    let item_count = fields.get(2)?.trim().parse::<usize>().ok()?;
    let mut raw_items = Vec::<(String, String)>::with_capacity(item_count);
    let mut names = BTreeMap::<String, String>::new();
    let mut index = 3usize;
    for _ in 0..item_count {
        let code = fields.get(index)?.trim().to_string();
        let body = fields.get(index + 1)?.to_string();
        let base = parse_flowchart_base(&code, &body)?;
        names.insert(base.id, base.name);
        raw_items.push((code, body));
        index += 2;
    }

    let mut items = Vec::with_capacity(item_count);
    for (z_order, (code, body)) in raw_items.into_iter().enumerate() {
        items.push(parse_flowchart_item(
            &code,
            &body,
            &names,
            object_refs,
            z_order.to_string(),
        )?);
    }

    Some(BusinessProcessFlowchart { items })
}

fn parse_flowchart_item(
    code: &str,
    body: &str,
    names: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
    z_order: String,
) -> Option<FlowchartItem> {
    let fields = split_1c_braced_fields(body, 0)?;
    let base = parse_flowchart_base(code, body)?;
    let mut properties = FlowchartItemProperties {
        location: None,
        pivot_points: Vec::new(),
        from: None,
        to: None,
        font: FlowchartFont::default(),
        decorative_line: false,
        line_style: "Solid",
        begin_arrow: "None",
        end_arrow: "None",
        transparent: false,
        horizontal_align: "Center",
        explanation: None,
        task_description: None,
        subprocess: None,
        true_port_index: None,
        false_port_index: None,
    };
    let mut events = Vec::new();
    let tag = match code {
        "0" => {
            parse_flowchart_shape_graphics(fields.get(2)?, &mut properties)?;
            properties.transparent = parse_flowchart_transparent(fields.get(2)?).unwrap_or(false);
            properties.horizontal_align = if properties.transparent {
                "Left"
            } else {
                "Center"
            };
            "Decoration"
        }
        "1" => {
            parse_flowchart_line_graphics(fields.get(6)?, &mut properties, names)?;
            let from_id = fields.get(2)?.trim();
            let to_id = fields.get(4)?.trim();
            if from_id != "-1"
                && let Some(from) = &mut properties.from
            {
                from.item = names.get(from_id).cloned().unwrap_or_default();
            }
            if to_id != "-1"
                && let Some(to) = &mut properties.to
            {
                to.item = names.get(to_id).cloned().unwrap_or_default();
            }
            properties.decorative_line = fields.get(5).map(|value| value.trim() == "1")?;
            properties.line_style = if properties.decorative_line {
                "Dashed"
            } else {
                "Solid"
            };
            properties.end_arrow = if properties.decorative_line
                && properties
                    .to
                    .as_ref()
                    .map(|end| end.item.is_empty())
                    .unwrap_or(true)
            {
                "None"
            } else {
                "Filled"
            };
            "ConnectionLine"
        }
        "2" => {
            parse_flowchart_shape_graphics(fields.get(2)?, &mut properties)?;
            events = parse_flowchart_named_events(fields.get(3)?, &["BeforeStart"])?;
            "Start"
        }
        "3" => {
            parse_flowchart_shape_graphics(fields.get(2)?, &mut properties)?;
            events = parse_flowchart_named_events(fields.get(3)?, &["OnComplete"])?;
            "Completion"
        }
        "4" => {
            parse_flowchart_shape_graphics(fields.get(2)?, &mut properties)?;
            properties.true_port_index = Some("3".to_string());
            properties.false_port_index = Some("1".to_string());
            events = parse_flowchart_named_events(fields.get(3)?, &["ConditionCheck"])?;
            "Condition"
        }
        "5" => {
            parse_flowchart_shape_graphics(fields.get(2)?, &mut properties)?;
            properties.explanation = fields.get(3).and_then(|value| parse_1c_string(value));
            properties.task_description = fields.get(7).and_then(|value| parse_1c_string(value));
            events = parse_flowchart_activity_events(fields.get(5)?)?;
            "Activity"
        }
        "7" => {
            parse_flowchart_shape_graphics(fields.get(2)?, &mut properties)?;
            "Split"
        }
        "8" => {
            parse_flowchart_shape_graphics(fields.get(2)?, &mut properties)?;
            "Join"
        }
        "9" => {
            parse_flowchart_shape_graphics(fields.get(2)?, &mut properties)?;
            events = parse_flowchart_named_events(fields.get(3)?, &["Processing"])?;
            "Processing"
        }
        "10" => {
            parse_flowchart_shape_graphics(fields.get(2)?, &mut properties)?;
            properties.subprocess = fields
                .get(4)
                .and_then(|field| parse_non_zero_uuid(field.trim()))
                .and_then(|uuid| object_refs.get(&uuid).cloned());
            properties.task_description = fields.get(5).and_then(|value| parse_1c_string(value));
            events = parse_flowchart_named_events(
                fields.get(3)?,
                &[
                    "BeforeCreateTasks",
                    "OnCreateTask",
                    "OnCreateSubBusinessProcesses",
                    "OnExecute",
                    "BeforeExecute",
                    "BeforeCreateSubBusinessProcesses",
                ],
            )?;
            "SubBusinessProcess"
        }
        _ => return None,
    };

    Some(FlowchartItem {
        tag,
        id: base.id,
        uuid: base.uuid,
        name: base.name,
        title: base.title,
        tab_order: base.tab_order,
        z_order,
        properties,
        events,
    })
}

fn parse_flowchart_base(code: &str, body: &str) -> Option<FlowchartBase> {
    let fields = split_1c_braced_fields(body, 0)?;
    let head = split_1c_braced_fields(fields.first()?, 0)?;
    let uuid = if matches!(code, "2" | "3" | "4" | "5" | "7" | "8" | "9" | "10") {
        head.get(2).map(|value| value.trim().to_string())
    } else {
        None
    }
    .filter(|value| is_uuid_text(value));
    let base_fields = if matches!(code, "2" | "3" | "4" | "5" | "7" | "8" | "9" | "10") {
        split_1c_braced_fields(head.first()?, 0)?
    } else {
        head
    };
    let id = base_fields.get(1)?.trim().to_string();
    let title = parse_flowchart_title(base_fields.get(2)?);
    let name = parse_1c_string(base_fields.get(3)?)?;
    let tab_order = base_fields.get(4)?.trim().to_string();

    Some(FlowchartBase {
        id,
        uuid,
        name,
        title,
        tab_order,
    })
}

fn parse_flowchart_shape_graphics(
    text: &str,
    properties: &mut FlowchartItemProperties,
) -> Option<()> {
    let outer = split_1c_braced_fields(text, 0)?;
    let wrapper = split_1c_braced_fields(outer.first()?, 0)?;
    let geometry = split_1c_braced_fields(wrapper.first()?, 0)?;
    if let Some(font) = geometry
        .first()
        .and_then(|style| parse_flowchart_style_font(style))
    {
        properties.font = font;
    }
    properties.location = Some(FlowchartLocation {
        left: geometry.get(2)?.trim().to_string(),
        top: geometry.get(3)?.trim().to_string(),
        right: geometry.get(4)?.trim().to_string(),
        bottom: geometry.get(5)?.trim().to_string(),
    });
    Some(())
}

fn parse_flowchart_line_graphics(
    text: &str,
    properties: &mut FlowchartItemProperties,
    names: &BTreeMap<String, String>,
) -> Option<()> {
    let outer = split_1c_braced_fields(text, 0)?;
    let geometry = split_1c_braced_fields(outer.first()?, 0)?;
    if let Some(font) = geometry
        .first()
        .and_then(|style| parse_flowchart_style_font(style))
    {
        properties.font = font;
    }
    let point_count = geometry.get(2)?.trim().parse::<usize>().ok()?;
    let mut points = Vec::with_capacity(point_count);
    let mut index = 3usize;
    for _ in 0..point_count {
        points.push(FlowchartPoint {
            x: geometry.get(index)?.trim().to_string(),
            y: geometry.get(index + 1)?.trim().to_string(),
        });
        index += 2;
    }
    properties.pivot_points = points;
    properties.line_style = parse_flowchart_line_style(geometry.get(index)?).unwrap_or("Solid");
    index += 2;
    let from_port = geometry.get(index)?.trim().to_string();
    let to_port = geometry.get(index + 1)?.trim().to_string();
    properties.from = Some(FlowchartConnectionEnd {
        item: String::new(),
        port_index: from_port,
    });
    properties.to = Some(FlowchartConnectionEnd {
        item: String::new(),
        port_index: to_port,
    });
    if geometry.get(index + 2).map(|value| value.trim() == "1") == Some(true) {
        properties.end_arrow = "Filled";
    }
    let _ = names;
    Some(())
}

impl Default for FlowchartFont {
    fn default() -> Self {
        Self {
            reference: "sys:DefaultGUIFont".to_string(),
            kind: "WindowsFont",
            height: None,
            bold: false,
            italic: false,
            underline: false,
            strikeout: false,
            scale: None,
        }
    }
}

fn parse_flowchart_style_font(style: &str) -> Option<FlowchartFont> {
    let fields = split_1c_braced_fields(style, 0)?;
    if fields.first()?.trim() != "7" {
        return None;
    }
    parse_flowchart_font_tuple(fields.get(4)?)
}

fn parse_flowchart_font_tuple(text: &str) -> Option<FlowchartFont> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.first()?.trim() != "7" {
        return None;
    }
    let kind = fields.get(1).map(|field| field.trim()).unwrap_or("1");
    let (reference, kind_xml) = match kind {
        "1" => ("sys:DefaultGUIFont".to_string(), "WindowsFont"),
        "2" => {
            let reference = fields
                .get(3)
                .and_then(|field| {
                    split_1c_braced_fields(field, 0)?
                        .first()
                        .and_then(|code| code.trim().parse::<i32>().ok())
                })
                .and_then(standard_style_item_for_code)
                .map(|(_, name)| format!("style:{name}"))
                .unwrap_or_else(|| "style:TextFont".to_string());
            (reference, "StyleItem")
        }
        _ => return None,
    };
    let height = font_height_xml(fields.get(2).map(|field| field.trim()));
    let (weight, italic, underline, strikeout, scale) = if fields.len() >= 10 {
        (
            fields
                .get(4)
                .and_then(|field| field.trim().parse::<i32>().ok())
                .unwrap_or(400),
            fields
                .get(5)
                .and_then(|field| parse_1c_bool_flag(field.trim()))
                .unwrap_or(false),
            fields
                .get(6)
                .and_then(|field| parse_1c_bool_flag(field.trim()))
                .unwrap_or(false),
            fields
                .get(7)
                .and_then(|field| parse_1c_bool_flag(field.trim()))
                .unwrap_or(false),
            fields.get(9).map(|field| field.trim()).unwrap_or("100"),
        )
    } else {
        (
            400,
            false,
            false,
            false,
            fields.get(5).map(|field| field.trim()).unwrap_or("100"),
        )
    };
    let scale = (scale != "100").then(|| scale.to_string());
    Some(FlowchartFont {
        reference,
        kind: kind_xml,
        height,
        bold: weight >= 700,
        italic,
        underline,
        strikeout,
        scale,
    })
}

fn parse_flowchart_line_style(text: &str) -> Option<&'static str> {
    let fields = split_1c_braced_fields(text, 0)?;
    match fields.get(3)?.trim() {
        "2" => Some("Dashed"),
        _ => Some("Solid"),
    }
}

fn parse_flowchart_transparent(text: &str) -> Option<bool> {
    let outer = split_1c_braced_fields(text, 0)?;
    let wrapper = split_1c_braced_fields(outer.first()?, 0)?;
    let style = split_1c_braced_fields(wrapper.first()?, 0)?;
    let flags = split_1c_braced_fields(style.first()?, 0)?;
    Some(flags.get(11)?.trim() == "1")
}

fn parse_flowchart_activity_events(text: &str) -> Option<Vec<FlowchartEvent>> {
    let handlers = parse_flowchart_event_handlers(text)?;
    Some(vec![
        FlowchartEvent {
            name: "InteractiveActivationProcessing",
            handler: None,
        },
        FlowchartEvent {
            name: "BeforeCreateTasks",
            handler: handlers.get(&1).cloned(),
        },
        FlowchartEvent {
            name: "OnCreateTask",
            handler: handlers.get(&2).cloned(),
        },
        FlowchartEvent {
            name: "OnExecute",
            handler: handlers.get(&3).cloned(),
        },
        FlowchartEvent {
            name: "CheckExecutionProcessing",
            handler: None,
        },
        FlowchartEvent {
            name: "BeforeExecute",
            handler: None,
        },
        FlowchartEvent {
            name: "BeforeExecuteInteractively",
            handler: None,
        },
    ])
}

fn parse_flowchart_named_events(text: &str, names: &[&'static str]) -> Option<Vec<FlowchartEvent>> {
    let handlers = parse_flowchart_event_handlers(text)?;
    Some(
        names
            .iter()
            .enumerate()
            .map(|(index, name)| FlowchartEvent {
                name,
                handler: handlers.get(&(index as i32)).cloned(),
            })
            .collect(),
    )
}

fn parse_flowchart_event_handlers(text: &str) -> Option<BTreeMap<i32, String>> {
    let fields = split_1c_braced_fields(text, 0)?;
    let count = fields.first()?.trim().parse::<usize>().ok()?;
    let mut handlers = BTreeMap::new();
    for field in fields.iter().skip(1).take(count) {
        let event = split_1c_braced_fields(field, 0)?;
        let index = event.first()?.trim().parse::<i32>().ok()?;
        let handler = parse_1c_string(event.get(1)?)?;
        handlers.insert(index, handler);
    }
    Some(handlers)
}

fn parse_flowchart_title(text: &str) -> Vec<(String, String)> {
    let Some(fields) = split_1c_braced_fields(text, 0) else {
        return Vec::new();
    };
    let count = fields
        .get(1)
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(0);
    fields
        .iter()
        .skip(2)
        .take(count)
        .filter_map(|field| {
            let pair = split_1c_braced_fields(field, 0)?;
            Some((
                parse_1c_string(pair.first()?)?,
                parse_1c_string(pair.get(1)?)?,
            ))
        })
        .collect()
}

fn parse_1c_string(text: &str) -> Option<String> {
    let text = text.trim();
    let inner = text.strip_prefix('"')?.strip_suffix('"')?;
    Some(inner.replace("\"\"", "\""))
}

fn exchange_plan_auto_record_xml(value: &str) -> &'static str {
    match value {
        "0" => "Deny",
        "1" => "Allow",
        "2" => "Auto",
        _ => "Auto",
    }
}

fn format_exchange_plan_content_xml(items: &[ExchangePlanContentItem]) -> String {
    let mut xml = String::from(
        "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<ExchangePlanContent xmlns=\"http://v8.1c.ru/8.3/xcf/extrnprops\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"2.20\">\r\n",
    );
    for item in items {
        xml.push_str(&format!(
            "\t<Item>\r\n\
\t\t<Metadata>{}</Metadata>\r\n\
\t\t<AutoRecord>{}</AutoRecord>\r\n\
\t</Item>\r\n",
            escape_xml_text(&item.metadata),
            item.auto_record
        ));
    }
    xml.push_str("</ExchangePlanContent>");
    xml
}

fn format_business_process_flowchart_xml(flowchart: &BusinessProcessFlowchart) -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<GraphicalSchema xmlns=\"http://v8.1c.ru/8.3/xcf/scheme\" xmlns:sch=\"http://v8.1c.ru/8.2/data/graphscheme\" xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" xmlns:web=\"http://v8.1c.ru/8.1/data/ui/colors/web\" xmlns:win=\"http://v8.1c.ru/8.1/data/ui/colors/windows\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"2.20\">\r\n\
\t<BackColor>style:FieldBackColor</BackColor>\r\n\
\t<GridEnabled>true</GridEnabled>\r\n\
\t<DrawGridMode>Lines</DrawGridMode>\r\n\
\t<GridHorizontalStep>20</GridHorizontalStep>\r\n\
\t<GridVerticalStep>20</GridVerticalStep>\r\n\
\t<PrintParameters>\r\n\
\t\t<TopMargin>10</TopMargin>\r\n\
\t\t<LeftMargin>10</LeftMargin>\r\n\
\t\t<BottomMargin>10</BottomMargin>\r\n\
\t\t<RightMargin>10</RightMargin>\r\n\
\t\t<BlackAndWhite>false</BlackAndWhite>\r\n\
\t\t<FitPageMode>Auto</FitPageMode>\r\n\
\t</PrintParameters>\r\n\
\t<Items>\r\n",
    );
    for item in &flowchart.items {
        push_flowchart_item_xml(&mut xml, item);
    }
    xml.push_str("\t</Items>\r\n</GraphicalSchema>\r\n");
    xml
}

fn push_flowchart_item_xml(xml: &mut String, item: &FlowchartItem) {
    if let Some(uuid) = &item.uuid {
        xml.push_str(&format!(
            "\t\t<{} id=\"{}\" uuid=\"{}\">\r\n",
            item.tag,
            escape_xml_text(&item.id),
            escape_xml_text(uuid)
        ));
    } else {
        xml.push_str(&format!(
            "\t\t<{} id=\"{}\">\r\n",
            item.tag,
            escape_xml_text(&item.id)
        ));
    }
    xml.push_str("\t\t\t<Properties>\r\n");
    xml.push_str(&format!(
        "\t\t\t\t<Name>{}</Name>\r\n",
        escape_xml_text(&item.name)
    ));
    push_flowchart_title_xml(xml, &item.title);
    xml.push_str("\t\t\t\t<ToolTip/>\r\n");
    xml.push_str(&format!(
        "\t\t\t\t<TabOrder>{}</TabOrder>\r\n",
        escape_xml_text(&item.tab_order)
    ));
    xml.push_str("\t\t\t\t<BackColor>auto</BackColor>\r\n");
    xml.push_str("\t\t\t\t<TextColor>style:FormTextColor</TextColor>\r\n");
    xml.push_str("\t\t\t\t<LineColor>style:BorderColor</LineColor>\r\n");
    xml.push_str("\t\t\t\t<GroupNumber>0</GroupNumber>\r\n");
    xml.push_str(&format!(
        "\t\t\t\t<ZOrder>{}</ZOrder>\r\n",
        escape_xml_text(&item.z_order)
    ));
    xml.push_str("\t\t\t\t<Hyperlink>false</Hyperlink>\r\n");
    xml.push_str(&format!(
        "\t\t\t\t<Transparent>{}</Transparent>\r\n",
        xml_bool(item.properties.transparent)
    ));
    push_flowchart_font_xml(xml, &item.properties.font);
    xml.push_str(&format!(
        "\t\t\t\t<HorizontalAlign>{}</HorizontalAlign>\r\n",
        item.properties.horizontal_align
    ));
    xml.push_str("\t\t\t\t<VerticalAlign>Center</VerticalAlign>\r\n");
    xml.push_str("\t\t\t\t<PictureLocation>Left</PictureLocation>\r\n");
    if item.tag == "ConnectionLine" {
        push_flowchart_line_properties_xml(xml, &item.properties);
    } else {
        push_flowchart_shape_properties_xml(xml, item);
    }
    xml.push_str("\t\t\t</Properties>\r\n");
    if !item.events.is_empty() {
        xml.push_str("\t\t\t<Events>\r\n");
        for event in &item.events {
            if let Some(handler) = &event.handler {
                xml.push_str(&format!(
                    "\t\t\t\t<Event name=\"{}\">{}</Event>\r\n",
                    event.name,
                    escape_xml_text(handler)
                ));
            } else {
                xml.push_str(&format!("\t\t\t\t<Event name=\"{}\"/>\r\n", event.name));
            }
        }
        xml.push_str("\t\t\t</Events>\r\n");
    }
    xml.push_str(&format!("\t\t</{}>\r\n", item.tag));
}

fn push_flowchart_title_xml(xml: &mut String, title: &[(String, String)]) {
    if title.is_empty() {
        xml.push_str("\t\t\t\t<Title/>\r\n");
        return;
    }
    xml.push_str("\t\t\t\t<Title>\r\n");
    for (lang, content) in title {
        xml.push_str("\t\t\t\t\t<v8:item>\r\n");
        xml.push_str(&format!(
            "\t\t\t\t\t\t<v8:lang>{}</v8:lang>\r\n",
            escape_xml_text(lang)
        ));
        xml.push_str(&format!(
            "\t\t\t\t\t\t<v8:content>{}</v8:content>\r\n",
            escape_xml_text(content)
        ));
        xml.push_str("\t\t\t\t\t</v8:item>\r\n");
    }
    xml.push_str("\t\t\t\t</Title>\r\n");
}

fn push_flowchart_font_xml(xml: &mut String, font: &FlowchartFont) {
    xml.push_str("\t\t\t\t<Font");
    if font.reference.starts_with("sys:") {
        xml.push_str(" xmlns:sys=\"http://v8.1c.ru/8.1/data/ui/fonts/system\"");
    }
    xml.push_str(&format!(" ref=\"{}\"", escape_xml_text(&font.reference)));
    if let Some(height) = &font.height {
        xml.push_str(&format!(" height=\"{}\"", escape_xml_text(height)));
    }
    if font.bold || font.italic || font.underline || font.strikeout {
        xml.push_str(&format!(
            " bold=\"{}\" italic=\"{}\" underline=\"{}\" strikeout=\"{}\"",
            xml_bool(font.bold),
            xml_bool(font.italic),
            xml_bool(font.underline),
            xml_bool(font.strikeout)
        ));
    }
    xml.push_str(&format!(" kind=\"{}\"", font.kind));
    if let Some(scale) = &font.scale {
        xml.push_str(&format!(" scale=\"{}\"", escape_xml_text(scale)));
    }
    xml.push_str("/>\r\n");
}

fn push_flowchart_shape_properties_xml(xml: &mut String, item: &FlowchartItem) {
    if let Some(location) = &item.properties.location {
        xml.push_str(&format!(
            "\t\t\t\t<Location top=\"{}\" left=\"{}\" bottom=\"{}\" right=\"{}\"/>\r\n",
            escape_xml_text(&location.top),
            escape_xml_text(&location.left),
            escape_xml_text(&location.bottom),
            escape_xml_text(&location.right)
        ));
    }
    if matches!(
        item.tag,
        "Start"
            | "Activity"
            | "Condition"
            | "Completion"
            | "Processing"
            | "Split"
            | "Join"
            | "SubBusinessProcess"
    ) {
        xml.push_str("\t\t\t\t<Border width=\"1\" gap=\"false\">\r\n");
        xml.push_str(
            "\t\t\t\t\t<v8ui:style xsi:type=\"sch:ConnectorLineType\">Solid</v8ui:style>\r\n",
        );
        xml.push_str("\t\t\t\t</Border>\r\n");
    }
    xml.push_str("\t\t\t\t<Picture/>\r\n");
    xml.push_str("\t\t\t\t<PictureSize>AutoSize</PictureSize>\r\n");
    if item.tag == "Activity" {
        xml.push_str(&format!(
            "\t\t\t\t<TaskDescription>{}</TaskDescription>\r\n",
            escape_xml_text(item.properties.task_description.as_deref().unwrap_or(""))
        ));
        xml.push_str(&format!(
            "\t\t\t\t<Explanation>{}</Explanation>\r\n",
            escape_xml_text(item.properties.explanation.as_deref().unwrap_or(""))
        ));
        xml.push_str("\t\t\t\t<Group>false</Group>\r\n");
    }
    if item.tag == "SubBusinessProcess" {
        if let Some(subprocess) = &item.properties.subprocess {
            xml.push_str(&format!(
                "\t\t\t\t<Subprocess>{}</Subprocess>\r\n",
                escape_xml_text(subprocess)
            ));
        }
        xml.push_str(&format!(
            "\t\t\t\t<TaskDescription>{}</TaskDescription>\r\n",
            escape_xml_text(item.properties.task_description.as_deref().unwrap_or(""))
        ));
    }
    if item.tag == "Condition" {
        xml.push_str(&format!(
            "\t\t\t\t<TruePortIndex>{}</TruePortIndex>\r\n",
            item.properties.true_port_index.as_deref().unwrap_or("3")
        ));
        xml.push_str(&format!(
            "\t\t\t\t<FalsePortIndex>{}</FalsePortIndex>\r\n",
            item.properties.false_port_index.as_deref().unwrap_or("1")
        ));
    }
    if item.tag == "Decoration" {
        xml.push_str("\t\t\t\t<Shape>Document</Shape>\r\n");
        xml.push_str("\t\t\t\t<FlipMode>0</FlipMode>\r\n");
        xml.push_str("\t\t\t\t<Angle xsi:type=\"xs:decimal\">0</Angle>\r\n");
    }
}

fn push_flowchart_line_properties_xml(xml: &mut String, properties: &FlowchartItemProperties) {
    xml.push_str("\t\t\t\t<PivotPoints>\r\n");
    for point in &properties.pivot_points {
        xml.push_str(&format!(
            "\t\t\t\t\t<Point x=\"{}\" y=\"{}\"/>\r\n",
            escape_xml_text(&point.x),
            escape_xml_text(&point.y)
        ));
    }
    xml.push_str("\t\t\t\t</PivotPoints>\r\n");
    xml.push_str("\t\t\t\t<Connect>\r\n");
    push_flowchart_connection_end_xml(xml, "From", properties.from.as_ref());
    push_flowchart_connection_end_xml(xml, "To", properties.to.as_ref());
    xml.push_str("\t\t\t\t</Connect>\r\n");
    xml.push_str("\t\t\t\t<Line width=\"1\" gap=\"false\">\r\n");
    xml.push_str(&format!(
        "\t\t\t\t\t<v8ui:style xsi:type=\"sch:ConnectorLineType\">{}</v8ui:style>\r\n",
        properties.line_style
    ));
    xml.push_str("\t\t\t\t</Line>\r\n");
    xml.push_str(&format!(
        "\t\t\t\t<DecorativeLine>{}</DecorativeLine>\r\n",
        xml_bool(properties.decorative_line)
    ));
    xml.push_str("\t\t\t\t<TextLocation>FirstSegment</TextLocation>\r\n");
    xml.push_str(&format!(
        "\t\t\t\t<BeginArrow>{}</BeginArrow>\r\n",
        properties.begin_arrow
    ));
    xml.push_str(&format!(
        "\t\t\t\t<EndArrow>{}</EndArrow>\r\n",
        properties.end_arrow
    ));
}

fn push_flowchart_connection_end_xml(
    xml: &mut String,
    tag: &str,
    end: Option<&FlowchartConnectionEnd>,
) {
    let item = end.map(|end| end.item.as_str()).unwrap_or("");
    let port_index = end.map(|end| end.port_index.as_str()).unwrap_or("0");
    xml.push_str(&format!("\t\t\t\t\t<{tag}>\r\n"));
    xml.push_str(&format!(
        "\t\t\t\t\t\t<Item>{}</Item>\r\n",
        escape_xml_text(item)
    ));
    xml.push_str(&format!(
        "\t\t\t\t\t\t<PortIndex>{}</PortIndex>\r\n",
        escape_xml_text(port_index)
    ));
    xml.push_str(&format!("\t\t\t\t\t</{tag}>\r\n"));
}

#[allow(dead_code)]
fn module_body_paths(rows: &[ConfigRow]) -> BTreeMap<String, PathBuf> {
    let metadata_texts = build_metadata_text_rows(rows);
    module_body_paths_from_texts(rows, &metadata_texts)
}

fn module_body_paths_from_texts(
    rows: &[ConfigRow],
    metadata_texts: &[MetadataTextRow],
) -> BTreeMap<String, PathBuf> {
    let file_names = rows
        .iter()
        .map(|row| row.file_name.as_str())
        .collect::<BTreeSet<_>>();
    let mut paths = configuration_module_body_paths(&file_names);

    for row in metadata_texts {
        let Some(entries) = parse_module_body_source_paths_from_metadata_text(row, &file_names)
        else {
            continue;
        };
        paths.extend(entries);
    }
    let form_refs = build_complete_form_source_reference_index(metadata_texts);
    paths.extend(form_module_body_paths(&form_refs, &file_names));

    paths
}

fn configuration_module_body_paths(file_names: &BTreeSet<&str>) -> BTreeMap<String, PathBuf> {
    let mut suffixes_by_id = BTreeMap::<&str, BTreeSet<&str>>::new();
    for file_name in file_names {
        let Some((metadata_id, suffix)) = file_name.rsplit_once('.') else {
            continue;
        };
        if metadata_id.is_empty() {
            continue;
        }
        suffixes_by_id
            .entry(metadata_id)
            .or_default()
            .insert(suffix);
    }

    let mut paths = BTreeMap::new();
    for (metadata_id, suffixes) in suffixes_by_id {
        if file_names.contains(metadata_id) || !is_configuration_module_group(&suffixes) {
            continue;
        }
        for (suffix, path) in CONFIGURATION_MODULE_SUFFIXES {
            let body_id = format!("{metadata_id}.{suffix}");
            if file_names.contains(body_id.as_str()) {
                paths.insert(body_id, PathBuf::from(path));
            }
        }
    }

    paths
}

fn form_module_body_paths(
    form_refs: &BTreeMap<String, FormSourceReference>,
    file_names: &BTreeSet<&str>,
) -> BTreeMap<String, PathBuf> {
    let mut paths = BTreeMap::new();
    for (form_uuid, form_ref) in form_refs {
        let body_id = format!("{form_uuid}.0");
        if !file_names.contains(body_id.as_str()) {
            continue;
        }
        let mut form_dir = form_ref.relative_path.clone();
        form_dir.set_extension("");
        paths.insert(
            body_id,
            form_dir.join("Ext").join("Form").join("Module.bsl"),
        );
    }
    paths
}

fn is_configuration_module_group(suffixes: &BTreeSet<&str>) -> bool {
    ["0", "5", "6", "7"]
        .iter()
        .all(|suffix| suffixes.contains(suffix))
        && ["2", "4", "8", "9", "a", "b", "c"]
            .iter()
            .any(|suffix| suffixes.contains(suffix))
}

const CONFIGURATION_MODULE_SUFFIXES: &[(&str, &str)] = &[
    ("0", "Ext/OrdinaryApplicationModule.bsl"),
    ("5", "Ext/ExternalConnectionModule.bsl"),
    ("6", "Ext/ManagedApplicationModule.bsl"),
    ("7", "Ext/SessionModule.bsl"),
];

#[allow(dead_code)]
fn parse_module_body_source_paths_from_metadata_blob(
    blob: &[u8],
    uuid: &str,
    file_names: &BTreeSet<&str>,
) -> Option<BTreeMap<String, PathBuf>> {
    let row = metadata_text_row_from_blob(uuid, blob)?;
    parse_module_body_source_paths_from_metadata_text(&row, file_names)
}

fn parse_module_body_source_paths_from_metadata_text(
    row: &MetadataTextRow,
    file_names: &BTreeSet<&str>,
) -> Option<BTreeMap<String, PathBuf>> {
    let uuid = row.file_name.as_str();
    let kind = row.kind.as_deref()?;
    let folder = row.folder?;
    let header = row.header.as_ref()?;
    let mut paths = BTreeMap::new();
    for suffix in MODULE_BODY_SUFFIXES {
        let body_id = format!("{uuid}.{suffix}");
        if !file_names.contains(body_id.as_str()) {
            continue;
        }
        if let Some(path) = module_owner_source_path(kind, folder, &header.name, suffix) {
            paths.insert(body_id, path);
        }
    }
    paths.extend(nested_command_module_source_paths(
        kind,
        folder,
        &header.name,
        uuid,
        &row.text,
        file_names,
    ));

    Some(paths)
}

fn nested_command_module_source_paths(
    kind: &str,
    folder: &str,
    owner_name: &str,
    owner_uuid: &str,
    text: &str,
    file_names: &BTreeSet<&str>,
) -> BTreeMap<String, PathBuf> {
    if !metadata_kind_can_own_commands(kind) {
        return BTreeMap::new();
    }

    let mut paths = BTreeMap::new();
    for command in nested_command_headers_for_owner_from_text(kind, text, owner_uuid) {
        let body_id = format!("{}.2", command.uuid);
        if !file_names.contains(body_id.as_str()) {
            continue;
        }
        let path = PathBuf::from(folder)
            .join(sanitize_source_path_segment(owner_name))
            .join("Commands")
            .join(sanitize_source_path_segment(&command.name))
            .join("Ext")
            .join("CommandModule.bsl");
        paths.insert(body_id, path);
    }

    paths
}

fn metadata_kind_can_own_commands(kind: &str) -> bool {
    !matches!(
        kind,
        "CommonModule"
            | "CommonCommand"
            | "CommonForm"
            | "CommonPicture"
            | "CommonTemplate"
            | "CommandGroup"
            | "Constant"
            | "DefinedType"
            | "EventSubscription"
            | "FunctionalOption"
            | "FunctionalOptionsParameter"
            | "Language"
            | "Role"
            | "SessionParameter"
            | "StyleItem"
            | "XDTOPackage"
    )
}

fn metadata_kind_can_own_forms(kind: &str) -> bool {
    matches!(
        kind,
        "AccumulationRegister"
            | "AccountingRegister"
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

fn metadata_kind_can_own_templates(kind: &str) -> bool {
    matches!(
        kind,
        "AccumulationRegister"
            | "AccountingRegister"
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

fn metadata_kind_needs_form_template_reference_indexes(kind: &str) -> bool {
    matches!(kind, "Enum") || metadata_kind_uses_simple_form_template_child_objects(kind)
}

fn nested_command_headers_from_text(text: &str, owner_uuid: &str) -> Vec<MetadataHeader> {
    nested_headers_from_text_inside_object_code(text, owner_uuid, 9)
}

fn nested_command_headers_for_owner_from_text(
    owner_kind: &str,
    text: &str,
    owner_uuid: &str,
) -> Vec<MetadataHeader> {
    nested_headers_with_offsets_from_text(text, owner_uuid, |marker_start| {
        is_offset_inside_metadata_object_code(text, marker_start, 9)
            && register_child_object_tag(owner_kind, text, marker_start).is_none()
    })
    .into_iter()
    .map(|(header, _)| header)
    .collect()
}

fn nested_child_commands_from_text(
    text: &str,
    owner_uuid: &str,
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
) -> Vec<MetadataChildCommand> {
    nested_command_headers_from_text(text, owner_uuid)
        .into_iter()
        .map(|header| {
            let properties = parse_common_command_properties_from_text(
                text,
                &header.uuid,
                type_index,
                object_refs,
            );
            MetadataChildCommand { header, properties }
        })
        .collect()
}

fn parse_information_register_child_commands(
    text: &str,
    owner_uuid: &str,
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
) -> Vec<MetadataChildCommand> {
    nested_headers_with_offsets_from_text(text, owner_uuid, |marker_start| {
        is_offset_inside_metadata_object_code(text, marker_start, 9)
            && register_child_object_tag("InformationRegister", text, marker_start).is_none()
    })
    .into_iter()
    .map(|(header, marker_start)| MetadataChildCommand {
        properties: parse_information_register_child_command_properties(
            text,
            marker_start,
            &header,
            type_index,
            object_refs,
        ),
        header,
    })
    .collect()
}

fn parse_information_register_child_command_properties(
    text: &str,
    marker_start: usize,
    header: &MetadataHeader,
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
) -> Option<CommonCommandProperties> {
    let mut parsed =
        metadata_object_field_candidates_around_header(text, marker_start, &header.uuid)
            .into_iter()
            .filter_map(|fields| {
                parse_information_register_child_command_properties_from_fields(
                    &fields,
                    header,
                    type_index,
                    object_refs,
                )
            });
    let value = parsed.next()?;
    if parsed.next().is_some() {
        return None;
    }
    Some(value)
}

fn parse_information_register_child_command_properties_from_fields(
    fields: &[&str],
    header: &MetadataHeader,
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
) -> Option<CommonCommandProperties> {
    if fields.len() != 13
        || fields.first()?.trim() != "9"
        || fields.get(4)?.trim() != "1"
        || fields.get(6)?.trim() != "0"
        || fields.get(12)?.trim() != "0"
    {
        return None;
    }
    let (picture_ref, picture_load_transparent) =
        parse_information_register_command_picture_descriptor(fields.get(1)?, object_refs)?;
    let header_fields = split_1c_braced_fields(fields.get(9)?, 0)?;
    if header_fields.len() != 9
        || header_fields.first()?.trim() != "3"
        || metadata_header_field_index(&header_fields, &header.uuid) != Some(1)
    {
        return None;
    }
    let parsed_header = parse_metadata_header_from_text(fields.get(9)?, &header.uuid)?;
    if parsed_header.uuid != header.uuid
        || parsed_header.name != header.name
        || parsed_header.synonyms != header.synonyms
        || parsed_header.comment != header.comment
    {
        return None;
    }

    let representation = match fields.get(2)?.trim() {
        "0" => "Text",
        "1" => "Picture",
        "2" => "PictureAndText",
        "3" => "Auto",
        _ => return None,
    };
    let shortcut = parse_information_register_command_shortcut(fields.get(5)?)?;
    let group_fields = split_1c_braced_fields(fields.get(7)?, 0)?;
    if group_fields.len() != 2
        || group_fields.first()?.trim() != "1"
        || parse_non_zero_uuid(group_fields.get(1)?.trim()).is_none()
    {
        return None;
    }
    let group = parse_common_command_group_value(fields.get(7)?, object_refs)?;
    let command_parameter_types = stable_partition_metadata_types(
        parse_information_register_type_pattern(fields.get(8)?, type_index)?,
    );
    let parameter_use_mode = match fields.get(11)?.trim() {
        "0" => "Single",
        "1" => "Multiple",
        _ => return None,
    };

    Some(CommonCommandProperties {
        representation,
        picture_ref,
        picture_load_transparent,
        tooltip: parse_information_register_localized_value(fields.get(3)?)?,
        shortcut,
        include_help_in_contents: false,
        group: Some(group),
        command_parameter_types,
        parameter_use_mode,
        modifies_data: information_register_bool(fields.get(10)?)?,
        on_main_server_unavailable_behavior: "Auto",
    })
}

fn parse_information_register_command_picture_descriptor(
    value: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<(Option<String>, bool)> {
    let fields = split_1c_braced_fields(value, 0)?;
    if fields.len() != 9
        || fields.first()?.trim() != "4"
        || parse_1c_quoted_string(fields.get(3)?.trim())?.as_str() != ""
        || fields.get(4)?.trim() != "-1"
        || fields.get(5)?.trim() != "-1"
        || fields.get(7)?.trim() != "0"
        || parse_1c_quoted_string(fields.get(8)?.trim())?.as_str() != ""
    {
        return None;
    }

    let load_transparent = information_register_bool(fields.get(6)?)?;
    let reference = split_1c_braced_fields(fields.get(2)?, 0)?;
    match fields.get(1)?.trim() {
        "0" if reference.len() == 1 && reference.first()?.trim() == "0" && load_transparent => {
            Some((None, load_transparent))
        }
        "1" if reference.len() == 1
            && reference.first()?.trim() == STD_PICTURE_PRINT_DESCRIPTOR_CODE =>
        {
            Some((Some("StdPicture.Print".to_string()), load_transparent))
        }
        "1" if reference.len() == 2 && reference.first()?.trim() == "0" => {
            let uuid = parse_non_zero_uuid(reference.get(1)?.trim())?;
            if let Some(picture) = information_register_command_standard_picture_name(&uuid) {
                return Some((Some(picture.to_string()), load_transparent));
            }
            let picture = object_refs.get(&uuid)?;
            let name = picture.strip_prefix("CommonPicture.")?;
            if name.is_empty() || name.contains('.') {
                return None;
            }
            Some((Some(picture.clone()), load_transparent))
        }
        _ => None,
    }
}

fn information_register_command_standard_picture_name(uuid: &str) -> Option<&'static str> {
    match uuid.to_ascii_lowercase().as_str() {
        "46598f81-5f95-4485-9b33-bfe4fd1276d0" => Some("StdPicture.SpreadsheetShowHeaders"),
        "caf2e58b-ca3d-4b63-82c9-f21f1c9bc9eb" => Some("StdPicture.Setting"),
        _ => common_command_standard_picture_name(uuid),
    }
}

fn parse_information_register_command_shortcut(value: &str) -> Option<Option<String>> {
    let fields = split_1c_braced_fields(value, 0)?;
    if fields.len() != 3 || fields.first()?.trim() != "0" {
        return None;
    }
    let key_code = fields.get(1)?.trim().parse::<u16>().ok()?;
    let modifier_code = fields.get(2)?.trim().parse::<u16>().ok()?;
    match (key_code, modifier_code) {
        (0, 0) => Some(None),
        (112..=123, 0) => Some(Some(format!("F{}", key_code - 111))),
        _ => None,
    }
}

fn nested_metadata_headers_from_text(text: &str, owner_uuid: &str) -> Vec<MetadataHeader> {
    nested_headers_from_text(text, owner_uuid, |_| true)
}

fn nested_web_service_operation_headers_from_text(
    text: &str,
    owner_uuid: &str,
) -> Vec<MetadataHeader> {
    nested_headers_from_text_inside_object_code(text, owner_uuid, 1)
}

fn nested_headers_from_text_inside_object_code(
    text: &str,
    owner_uuid: &str,
    code: u32,
) -> Vec<MetadataHeader> {
    nested_headers_from_text(text, owner_uuid, |marker_start| {
        is_offset_inside_metadata_object_code(text, marker_start, code)
    })
}

fn nested_headers_from_text(
    text: &str,
    owner_uuid: &str,
    accepts_marker: impl Fn(usize) -> bool,
) -> Vec<MetadataHeader> {
    nested_headers_with_offsets_from_text(text, owner_uuid, accepts_marker)
        .into_iter()
        .map(|(header, _)| header)
        .collect()
}

fn nested_headers_with_offsets_from_text(
    text: &str,
    owner_uuid: &str,
    accepts_marker: impl Fn(usize) -> bool,
) -> Vec<(MetadataHeader, usize)> {
    let mut headers = Vec::new();
    let mut seen = BTreeSet::new();
    let mut offset = 0usize;
    let marker = "{1,0,";

    while let Some(relative) = text[offset..].find(marker) {
        let marker_start = offset + relative;
        let uuid_start = marker_start + marker.len();
        let uuid_end = uuid_start + 36;
        offset = uuid_start;

        let Some(uuid) = text.get(uuid_start..uuid_end) else {
            continue;
        };
        if uuid == owner_uuid || !is_uuid_text(uuid) || !is_metadata_header_marker(text, uuid_end) {
            continue;
        }
        if !accepts_marker(marker_start) {
            continue;
        }
        if !seen.insert(uuid.to_string()) {
            continue;
        }
        if let Some(header) = parse_metadata_header_from_text(text, uuid) {
            headers.push((header, marker_start));
        }
    }

    headers
}

fn is_metadata_header_marker(text: &str, uuid_end: usize) -> bool {
    matches!(text.get(uuid_end..uuid_end + 2), Some("},"))
}

fn is_offset_inside_metadata_object_code(text: &str, offset: usize, code: u32) -> bool {
    let code_marker = format!("{{{code},");
    let Some(start) = text[..offset].rfind(&code_marker) else {
        return false;
    };
    scan_1c_braced_value(text, start)
        .map(|end| offset < end)
        .unwrap_or(false)
}

fn enclosing_metadata_header_for_code(
    text: &str,
    offset: usize,
    code: u32,
) -> Option<MetadataHeader> {
    let code_marker = format!("{{{code},");
    let mut search_end = offset;
    while let Some(relative_start) = text[..search_end].rfind(&code_marker) {
        let start = relative_start;
        search_end = start;
        let Some(end) = scan_1c_braced_value(text, start) else {
            continue;
        };
        if offset >= end {
            continue;
        }
        let Some(marker_relative) = text[start..end].find("{1,0,") else {
            continue;
        };
        let uuid_start = start + marker_relative + "{1,0,".len();
        let uuid_end = uuid_start + 36;
        let Some(uuid) = text.get(uuid_start..uuid_end) else {
            continue;
        };
        if !is_uuid_text(uuid) || !is_metadata_header_marker(text, uuid_end) {
            continue;
        }
        if let Some(header) = parse_metadata_header_from_text(text, uuid) {
            return Some(header);
        }
    }
    None
}

fn preceding_metadata_header_for_code(
    text: &str,
    offset: usize,
    code: u32,
) -> Option<MetadataHeader> {
    preceding_metadata_header_for_code_with_bounds(text, offset, code).map(|(header, _)| header)
}

fn preceding_metadata_header_for_code_with_bounds(
    text: &str,
    offset: usize,
    code: u32,
) -> Option<(MetadataHeader, usize)> {
    let code_marker = format!("{{{code},");
    let mut search_end = offset;
    while let Some(start) = text[..search_end].rfind(&code_marker) {
        search_end = start;
        let Some(end) = scan_1c_braced_value(text, start) else {
            continue;
        };
        let Some(marker_relative) = text[start..end].find("{1,0,") else {
            continue;
        };
        let uuid_start = start + marker_relative + "{1,0,".len();
        let uuid_end = uuid_start + 36;
        let Some(uuid) = text.get(uuid_start..uuid_end) else {
            continue;
        };
        if !is_uuid_text(uuid) || !is_metadata_header_marker(text, uuid_end) {
            continue;
        }
        if let Some(header) = parse_metadata_header_from_text(text, uuid) {
            return Some((header, end));
        }
    }
    None
}

fn contains_metadata_header_uuid_between(text: &str, start: usize, end: usize, uuid: &str) -> bool {
    start < end && text[start..end].contains(&format!("{{1,0,{uuid}}}"))
}

fn contains_metadata_header_name_between(text: &str, start: usize, end: usize, name: &str) -> bool {
    if start >= end {
        return false;
    }
    let mut offset = start;
    let marker = "{1,0,";
    while offset < end {
        let Some(relative) = text[offset..end].find(marker) else {
            return false;
        };
        let marker_start = offset + relative;
        let uuid_start = marker_start + marker.len();
        let uuid_end = uuid_start + 36;
        offset = uuid_start;
        let Some(uuid) = text.get(uuid_start..uuid_end) else {
            continue;
        };
        if !is_uuid_text(uuid) || !is_metadata_header_marker(text, uuid_end) {
            continue;
        }
        let Some(name_start) =
            expect_comma_at(text, uuid_end + 1).map(|pos| skip_ascii_ws_at(text, pos))
        else {
            continue;
        };
        let Some((header_name, consumed)) = parse_1c_quoted_string_with_len(&text[name_start..])
        else {
            continue;
        };
        if name_start + consumed <= end && header_name == name {
            return true;
        }
    }
    false
}

fn module_owner_source_path(kind: &str, folder: &str, name: &str, suffix: &str) -> Option<PathBuf> {
    module_owner_module_file(kind, suffix).map(|module_file| {
        PathBuf::from(folder)
            .join(sanitize_source_path_segment(name))
            .join("Ext")
            .join(module_file)
    })
}

fn module_owner_module_file(kind: &str, suffix: &str) -> Option<&'static str> {
    match (kind, suffix) {
        ("CommonModule", "0") | ("HTTPService", "0") | ("WebService", "0") => Some("Module.bsl"),
        ("Bot", "1") | ("IntegrationService", "0") => Some("Module.bsl"),
        ("CommonCommand", "2") => Some("CommandModule.bsl"),
        ("FilterCriterion", "0") => Some("ManagerModule.bsl"),
        ("Constant", "0") => Some("ValueManagerModule.bsl"),
        ("Constant", "1") => Some("ManagerModule.bsl"),
        ("SettingsStorage", "8") => Some("ManagerModule.bsl"),
        ("Sequence", "0") => Some("RecordSetModule.bsl"),
        ("Catalog", "0") => Some("ObjectModule.bsl"),
        ("Catalog", "3") => Some("ManagerModule.bsl"),
        ("Report", "0") => Some("ObjectModule.bsl"),
        ("Report", "2") => Some("ManagerModule.bsl"),
        ("DataProcessor", "0") => Some("ObjectModule.bsl"),
        ("DataProcessor", "2") => Some("ManagerModule.bsl"),
        ("Document", "0") => Some("ObjectModule.bsl"),
        ("Document", "2") => Some("ManagerModule.bsl"),
        ("Enum", "0") => Some("ManagerModule.bsl"),
        ("ExchangePlan", "2") => Some("ObjectModule.bsl"),
        ("ExchangePlan", "3") => Some("ManagerModule.bsl"),
        ("AccountingRegister", "6") => Some("RecordSetModule.bsl"),
        ("AccountingRegister", "7") => Some("ManagerModule.bsl"),
        ("AccumulationRegister", "1")
        | ("CalculationRegister", "1")
        | ("InformationRegister", "1") => Some("RecordSetModule.bsl"),
        ("AccumulationRegister", "2")
        | ("CalculationRegister", "2")
        | ("InformationRegister", "2") => Some("ManagerModule.bsl"),
        ("DocumentJournal", "1") => Some("ManagerModule.bsl"),
        ("Task", "6") => Some("ObjectModule.bsl"),
        ("Task", "7") => Some("ManagerModule.bsl"),
        ("BusinessProcess", "6") => Some("ObjectModule.bsl"),
        ("BusinessProcess", "8") => Some("ManagerModule.bsl"),
        ("ChartOfAccounts", "14") => Some("ObjectModule.bsl"),
        ("ChartOfAccounts", "15") => Some("ManagerModule.bsl"),
        ("ChartOfCalculationTypes", "0") => Some("ObjectModule.bsl"),
        ("ChartOfCalculationTypes", "3") => Some("ManagerModule.bsl"),
        ("ChartOfCharacteristicTypes", "15") => Some("ObjectModule.bsl"),
        ("ChartOfCharacteristicTypes", "16") => Some("ManagerModule.bsl"),
        _ => None,
    }
}

struct ExtractedMetadataSourceXml {
    relative_path: PathBuf,
    xml: Vec<u8>,
}

struct FormSourceReference {
    relative_path: PathBuf,
    kind: &'static str,
}

struct TemplateSourceReference {
    relative_path: PathBuf,
    kind: &'static str,
    template_type: &'static str,
}

struct SubsystemSourceReference {
    relative_path: PathBuf,
}

struct CommonPictureProperties {
    availability_for_choice: bool,
    availability_for_appearance: bool,
}

struct FunctionalOptionProperties {
    location: Option<String>,
    privileged_get_mode: bool,
    content: Vec<String>,
}

struct DefaultListFormMetadataProperties {
    use_standard_commands: Option<bool>,
    default_list_form: Option<String>,
}

struct SubsystemProperties {
    include_help_in_contents: bool,
    include_in_command_interface: bool,
    use_one_command: bool,
    explanation: Vec<(String, String)>,
    picture_ref: Option<String>,
    picture_load_transparent: bool,
    content: Vec<String>,
    child_subsystems: Vec<String>,
}

struct ExchangePlanProperties {
    this_node: String,
    generated_types: Vec<GeneratedTypeEntry>,
    use_standard_commands: bool,
    code_length: u32,
    code_allowed_length: &'static str,
    description_length: u32,
    default_presentation: &'static str,
    edit_type: &'static str,
    quick_choice: bool,
    choice_mode: &'static str,
    input_by_string: Vec<String>,
    search_string_mode_on_input_by_string: &'static str,
    full_text_search_on_input_by_string: &'static str,
    choice_data_get_mode_on_input_by_string: &'static str,
    default_object_form: Option<String>,
    default_list_form: Option<String>,
    default_choice_form: Option<String>,
    auxiliary_object_form: Option<String>,
    auxiliary_list_form: Option<String>,
    auxiliary_choice_form: Option<String>,
    standard_attributes: Vec<RegisterStandardAttribute>,
    based_on: Option<String>,
    distributed_infobase: bool,
    include_configuration_extensions: bool,
    create_on_input: &'static str,
    choice_history_on_input: &'static str,
    include_help_in_contents: bool,
    data_lock_fields: Vec<String>,
    data_lock_control_mode: &'static str,
    full_text_search: &'static str,
    object_presentation: Vec<(String, String)>,
    extended_object_presentation: Vec<(String, String)>,
    list_presentation: Vec<(String, String)>,
    extended_list_presentation: Vec<(String, String)>,
    explanation: Vec<(String, String)>,
    data_history: &'static str,
    update_data_history_immediately_after_write: bool,
    execute_after_write_data_history_version_processing: bool,
    child_objects: Vec<MetadataChildObject>,
}

struct ExchangePlanOwnerFields<'a> {
    physical: Vec<&'a str>,
}

impl<'a> ExchangePlanOwnerFields<'a> {
    fn get(&self, index: usize) -> Option<&'a str> {
        self.physical.get(index).copied()
    }
}

struct RegisterProperties {
    generated_types: Vec<GeneratedTypeEntry>,
    use_standard_commands: bool,
    information_register: Option<InformationRegisterOwnerProperties>,
    register_type: Option<&'static str>,
    include_help_in_contents: Option<bool>,
    chart_of_accounts: Option<String>,
    correspondence: Option<bool>,
    period_adjustment_length: Option<u32>,
    data_lock_control_mode: Option<&'static str>,
    enable_totals_splitting: Option<bool>,
    full_text_search: Option<&'static str>,
    default_list_form: Option<String>,
    auxiliary_list_form: Option<String>,
    list_presentation: Vec<(String, String)>,
    extended_list_presentation: Vec<(String, String)>,
    explanation: Vec<(String, String)>,
    standard_attributes: Vec<RegisterStandardAttribute>,
    child_objects: Vec<MetadataChildObject>,
}

struct InformationRegisterOwnerFields<'a> {
    logical: [&'a str; 25],
}

impl<'a> InformationRegisterOwnerFields<'a> {
    fn get(&self, index: usize) -> Option<&'a str> {
        self.logical.get(index).copied()
    }
}

struct InformationRegisterOwnerProperties {
    default_record_form: Option<String>,
    default_list_form: Option<String>,
    periodicity: &'static str,
    write_mode: &'static str,
    edit_type: &'static str,
    use_standard_commands: bool,
    include_help_in_contents: bool,
    main_filter_on_period: bool,
    data_lock_control_mode: &'static str,
    full_text_search: &'static str,
    standard_attributes: Vec<RegisterStandardAttribute>,
    auxiliary_record_form: Option<String>,
    auxiliary_list_form: Option<String>,
    record_presentation: Vec<(String, String)>,
    extended_record_presentation: Vec<(String, String)>,
    list_presentation: Vec<(String, String)>,
    extended_list_presentation: Vec<(String, String)>,
    explanation: Vec<(String, String)>,
    enable_totals_slice_last: bool,
    enable_totals_slice_first: bool,
    data_history: &'static str,
    update_data_history_immediately_after_write: bool,
    execute_after_write_data_history_version_processing: bool,
}

struct RecalculationProperties {
    generated_types: Vec<GeneratedTypeEntry>,
    data_lock_control_mode: &'static str,
    dimensions: Vec<RecalculationDimension>,
}

struct RecalculationDimension {
    header: MetadataHeader,
    register_dimension: String,
}

#[derive(Clone)]
struct MetadataChildObject {
    tag: &'static str,
    header: MetadataHeader,
    generated_types: Vec<GeneratedTypeEntry>,
    value_types: Vec<ConstantValueType>,
    emit_empty_type: bool,
    properties: Option<MetadataChildProperties>,
    tabular_section_properties: Option<MetadataTabularSectionProperties>,
    child_objects: Vec<MetadataChildObject>,
}

#[derive(Clone)]
struct MetadataChildProperties {
    password_mode: bool,
    format: Vec<(String, String)>,
    edit_format: Vec<(String, String)>,
    tooltip: Vec<(String, String)>,
    mark_negatives: bool,
    mask: String,
    multi_line: bool,
    extended_edit: bool,
    min_value: Option<String>,
    max_value: Option<String>,
    fill_from_filling_value: bool,
    emit_fill_from_filling_value: bool,
    fill_value: Option<MetadataChildFillValue>,
    emit_fill_value: bool,
    fill_checking: &'static str,
    choice_folders_and_items: Option<&'static str>,
    choice_parameter_links: Option<Vec<MetadataChoiceParameterLink>>,
    choice_parameters: Option<Vec<MetadataChoiceParameter>>,
    self_close_empty_choice_parameter_refs: bool,
    quick_choice: Option<&'static str>,
    create_on_input: Option<&'static str>,
    choice_form: Option<MetadataChoiceForm>,
    link_by_type_empty: bool,
    link_by_type: Option<MetadataChildLinkByType>,
    choice_history_on_input: Option<&'static str>,
    master: Option<bool>,
    main_filter: Option<bool>,
    balance: Option<bool>,
    accounting_flag: Option<String>,
    ext_dimension_accounting_flag: Option<String>,
    deny_incomplete_values: Option<bool>,
    use_mode: Option<&'static str>,
    indexing: Option<&'static str>,
    full_text_search: Option<&'static str>,
    data_history: Option<&'static str>,
    type_reduction_mode: Option<&'static str>,
    update_data_history_immediately_after_write: Option<bool>,
    execute_after_write_data_history_version_processing: Option<bool>,
}

#[derive(Clone)]
struct MetadataChoiceParameterLink {
    name: String,
    data_path: String,
    value_change: &'static str,
}

#[derive(Clone)]
struct MetadataChoiceParameter {
    name: String,
    value: MetadataChoiceParameterValue,
}

#[derive(Clone)]
enum MetadataChoiceParameterValue {
    Nil,
    Boolean(bool),
    Decimal(String),
    DateTime(String),
    DesignTimeRef(String),
    FixedArray(Vec<MetadataChoiceParameterValue>),
    String(String),
}

#[derive(Clone)]
struct MetadataChildLinkByType {
    data_path: String,
    link_item: u32,
}

#[derive(Clone)]
enum MetadataChoiceForm {
    Empty,
    Reference(String),
}

#[derive(Clone)]
struct MetadataTabularSectionProperties {
    tooltip: Vec<(String, String)>,
    fill_checking: &'static str,
    line_number_fill_checking: &'static str,
    use_mode: Option<&'static str>,
    line_number_length: Option<u32>,
}

#[derive(Clone)]
enum MetadataChildFillValue {
    Nil,
    Boolean(bool),
    Decimal(String),
    DateTime(String),
    DesignTimeRef(String),
    String(String),
}

struct RegisterStandardAttribute {
    name: &'static str,
    fill_checking: &'static str,
    fill_from_filling_value: bool,
    tooltip: Vec<(String, String)>,
    format: Vec<(String, String)>,
    edit_format: Vec<(String, String)>,
    synonym: Vec<(String, String)>,
    data_history: &'static str,
    full_text_search: &'static str,
    fill_value: MetadataChildFillValue,
    link_by_type: Option<RegisterStandardAttributeLinkByType>,
}

struct RegisterStandardAttributeLinkByType {
    data_path: String,
    link_item: u32,
}

struct CommonModuleFlags {
    global: bool,
    client_managed_application: bool,
    server: bool,
    external_connection: bool,
    client_ordinary_application: bool,
    server_call: bool,
    privileged: bool,
    return_values_reuse: ReturnValuesReuseValue,
}

#[derive(Clone, Copy)]
enum ReturnValuesReuseValue {
    DontUse,
    DuringRequest,
    DuringSession,
}

struct CatalogProperties {
    generated_types: Vec<GeneratedTypeEntry>,
    hierarchical: bool,
    level_count: u32,
    folders_on_top: bool,
    owners: Option<Vec<String>>,
    subordination_use: Option<&'static str>,
    use_standard_commands: bool,
    code_length: u32,
    description_length: u32,
    code_type: Option<&'static str>,
    code_allowed_length: Option<&'static str>,
    code_series: Option<&'static str>,
    check_unique: bool,
    autonumbering: bool,
    default_presentation: Option<&'static str>,
    quick_choice: bool,
    choice_mode: &'static str,
    input_by_string_fields: Vec<String>,
    default_object_form: Option<String>,
    default_folder_form: Option<String>,
    default_list_form: Option<String>,
    default_choice_form: Option<String>,
    default_folder_choice_form: Option<String>,
    auxiliary_object_form: Option<String>,
    auxiliary_folder_form: Option<String>,
    auxiliary_list_form: Option<String>,
    auxiliary_choice_form: Option<String>,
    auxiliary_folder_choice_form: Option<String>,
    include_help_in_contents: bool,
    object_presentation: Vec<(String, String)>,
    extended_object_presentation: Vec<(String, String)>,
    list_presentation: Vec<(String, String)>,
    extended_list_presentation: Vec<(String, String)>,
    explanation: Vec<(String, String)>,
    create_on_input: &'static str,
    choice_history_on_input: &'static str,
    data_history: &'static str,
    update_data_history_immediately_after_write: bool,
    execute_after_write_data_history_version_processing: bool,
    standard_attribute_details: BTreeMap<&'static str, CatalogStandardAttributeDetails>,
    child_metadata_objects: Vec<MetadataChildObject>,
    child_forms: Vec<String>,
    child_templates: Vec<String>,
}

#[derive(Default)]
struct CatalogStandardAttributeDetails {
    tooltip: Vec<(String, String)>,
    synonym: Vec<(String, String)>,
}

struct ReportProperties {
    generated_types: Vec<GeneratedTypeEntry>,
    use_standard_commands: bool,
    default_form: Option<String>,
    main_data_composition_schema: Option<String>,
    default_settings_form: Option<String>,
    default_variant_form: Option<String>,
    variants_storage: Option<String>,
    settings_storage: Option<String>,
    include_help_in_contents: bool,
    extended_presentation: Vec<(String, String)>,
    explanation: Vec<(String, String)>,
    child_metadata_objects: Vec<MetadataChildObject>,
    child_forms: Vec<String>,
    child_templates: Vec<String>,
    child_commands: Vec<MetadataChildCommand>,
}

struct DataProcessorProperties {
    generated_types: Vec<GeneratedTypeEntry>,
    use_standard_commands: bool,
    default_form: Option<String>,
    auxiliary_form: Option<String>,
    include_help_in_contents: bool,
    extended_presentation: Vec<(String, String)>,
    explanation: Vec<(String, String)>,
    child_metadata_objects: Vec<MetadataChildObject>,
    child_forms: Vec<String>,
    child_templates: Vec<String>,
    child_commands: Vec<MetadataChildCommand>,
}

struct MetadataChildCommand {
    header: MetadataHeader,
    properties: Option<CommonCommandProperties>,
}

struct DocumentProperties {
    generated_types: Vec<GeneratedTypeEntry>,
    use_standard_commands: bool,
    numbering: Option<DocumentNumberingProperties>,
    standard_attributes: Option<DocumentStandardAttributes>,
    default_object_form: Option<String>,
    default_list_form: Option<String>,
    default_choice_form: Option<String>,
    auxiliary_object_form: Option<String>,
    auxiliary_list_form: Option<String>,
    auxiliary_choice_form: Option<String>,
    include_help_in_contents: bool,
    child_metadata_objects: Vec<MetadataChildObject>,
    child_forms: Vec<String>,
    child_templates: Vec<String>,
}

struct DocumentNumberingProperties {
    numerator: Option<String>,
    number_type: &'static str,
    number_length: u32,
    number_allowed_length: &'static str,
    number_periodicity: &'static str,
    check_unique: bool,
    autonumbering: bool,
}

struct DocumentStandardAttributes {
    number_type: &'static str,
    details: BTreeMap<&'static str, CatalogStandardAttributeDetails>,
}

struct BusinessProcessProperties {
    generated_types: Vec<GeneratedTypeEntry>,
    use_standard_commands: bool,
    default_list_form: Option<String>,
}

struct TaskProperties {
    generated_types: Vec<GeneratedTypeEntry>,
    use_standard_commands: bool,
    default_list_form: Option<String>,
}

struct SettingsStorageProperties {
    generated_types: Vec<GeneratedTypeEntry>,
}

struct EnumProperties {
    generated_types: Vec<GeneratedTypeEntry>,
    use_standard_commands: bool,
    has_standard_attributes: bool,
    quick_choice: bool,
    choice_mode: &'static str,
    choice_history_on_input: &'static str,
    default_list_form: Option<String>,
    default_choice_form: Option<String>,
    auxiliary_list_form: Option<String>,
    auxiliary_choice_form: Option<String>,
    list_presentation: Vec<(String, String)>,
    extended_list_presentation: Vec<(String, String)>,
    explanation: Vec<(String, String)>,
    values: Vec<MetadataHeader>,
    child_forms: Vec<String>,
    child_templates: Vec<String>,
}

#[derive(Clone)]
struct GeneratedTypeEntry {
    name: String,
    category: &'static str,
    type_id: String,
    value_id: String,
}

struct ConstantProperties {
    generated_types: Vec<GeneratedTypeEntry>,
    value_type: ConstantValueType,
    tooltip: Vec<(String, String)>,
    extended_presentation: Vec<(String, String)>,
    explanation: Vec<(String, String)>,
    use_standard_commands: bool,
    default_form: Option<String>,
    password_mode: bool,
    format: Vec<(String, String)>,
    edit_format: Vec<(String, String)>,
    mask: String,
    min_value: Option<String>,
    max_value: Option<String>,
    fill_checking: &'static str,
    choice_parameters: Vec<ChoiceParameter>,
    choice_history_on_input: &'static str,
    data_lock_control_mode: &'static str,
}

struct ChoiceParameter {
    name: String,
    value_ref: String,
}

struct DefinedTypeProperties {
    generated_types: Vec<GeneratedTypeEntry>,
    value_types: Vec<ConstantValueType>,
}

struct CommonCommandProperties {
    representation: &'static str,
    picture_ref: Option<String>,
    picture_load_transparent: bool,
    tooltip: Vec<(String, String)>,
    shortcut: Option<String>,
    include_help_in_contents: bool,
    group: Option<String>,
    command_parameter_types: Vec<ConstantValueType>,
    parameter_use_mode: &'static str,
    modifies_data: bool,
    on_main_server_unavailable_behavior: &'static str,
}

struct CommandGroupProperties {
    representation: &'static str,
    picture_ref: Option<String>,
    picture_load_transparent: bool,
    tooltip: Vec<(String, String)>,
    category: &'static str,
}

struct StyleItemProperties {
    item_type: &'static str,
    value_xml: String,
}

struct ScheduledJobProperties {
    method_name: String,
    description: String,
    key: String,
    use_job: bool,
    predefined: bool,
    restart_count_on_failure: u32,
    restart_interval_on_failure: u32,
}

struct EventSubscriptionProperties {
    source_types: Vec<ConstantValueType>,
    event: String,
    handler: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct XdtoPackageProperties {
    namespace: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct WebServiceProperties {
    namespace: String,
    xdto_packages: Vec<WebServiceXdtoPackage>,
    descriptor_file_name: String,
    reuse_sessions: &'static str,
    session_max_age: u32,
    operations: Vec<WebServiceOperationProperties>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
enum WebServiceXdtoPackage {
    MetadataReference(String),
    Namespace(String),
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct WebServiceOperationProperties {
    header: MetadataHeader,
    returning_value_type: WebServiceXdtoType,
    nillable: bool,
    transactioned: bool,
    procedure_name: String,
    data_lock_control_mode: &'static str,
    parameters: Vec<WebServiceParameterProperties>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct WebServiceParameterProperties {
    header: MetadataHeader,
    value_type: WebServiceXdtoType,
    nillable: bool,
    transfer_direction: &'static str,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct WebServiceXdtoType {
    namespace: String,
    name: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct HttpServiceProperties {
    root_url: String,
    reuse_sessions: &'static str,
    session_max_age: u32,
    url_templates: Vec<HttpServiceUrlTemplateProperties>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct HttpServiceUrlTemplateProperties {
    header: MetadataHeader,
    template: String,
    methods: Vec<HttpServiceMethodProperties>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct HttpServiceMethodProperties {
    header: MetadataHeader,
    http_method: String,
    handler: String,
}

struct StyleBodyItem {
    name: String,
    standard_order: Option<usize>,
    value_xml: String,
}

struct TypedMetadataProperties {
    value_types: Vec<ConstantValueType>,
}

#[derive(Debug, Clone, Eq, PartialEq, Default)]
struct ConfigurationProperties {
    name_prefix: Option<String>,
    configuration_extension_compatibility_mode: Option<String>,
    default_run_mode: Option<&'static str>,
    use_purposes: Vec<&'static str>,
    localized_properties: Option<ConfigurationLocalizedProperties>,
    brief_information: Vec<(String, String)>,
    detailed_information: Vec<(String, String)>,
    copyright: Vec<(String, String)>,
    vendor_information_address: Vec<(String, String)>,
    configuration_information_address: Vec<(String, String)>,
    default_style: Option<String>,
    default_language: Option<String>,
    script_variant: Option<&'static str>,
    default_roles: Vec<String>,
    vendor: Option<String>,
    version: Option<String>,
    update_catalog_address: Option<String>,
    common_settings_storage: Option<ConfigurationRootReference>,
    reports_user_settings_storage: Option<ConfigurationRootReference>,
    reports_variants_storage: Option<ConfigurationRootReference>,
    form_data_settings_storage: Option<ConfigurationRootReference>,
    used_mobile_application_functionalities: Vec<ConfigurationMobileApplicationFunctionality>,
    compatibility_mode: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct ConfigurationLocalizedProperties {
    brief_information: Vec<(String, String)>,
    detailed_information: Vec<(String, String)>,
    copyright: Vec<(String, String)>,
    vendor_information_address: Vec<(String, String)>,
    configuration_information_address: Vec<(String, String)>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct ConfigurationMobileApplicationFunctionality {
    name: &'static str,
    use_functionality: bool,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct ConfigurationRootReference {
    value: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct ConfigurationContainedObject {
    class_id: String,
    object_id: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct ConfigurationRootLayout {
    contained_objects: Vec<ConfigurationContainedObject>,
    child_families: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct ConfigurationRootChildObject {
    kind: &'static str,
    name: String,
}

struct CommonAttributeProperties {
    value_types: Vec<ConstantValueType>,
    property_details: Option<CommonAttributePropertyDetails>,
    auto_use: &'static str,
    content: Vec<CommonAttributeContentItem>,
    separation: Option<CommonAttributeSeparationProperties>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct CommonAttributePropertyDetails {
    fill_value: Option<CommonAttributeFillValue>,
    fill_checking: &'static str,
}

#[derive(Debug, Clone, Eq, PartialEq)]
enum CommonAttributeFillValue {
    Nil,
    Boolean(bool),
    Decimal(String),
    String(String),
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct CommonAttributeSeparationProperties {
    data_separation: &'static str,
    separated_data_use: &'static str,
    data_separation_value: Option<String>,
    data_separation_use: Option<String>,
    conditional_separation: Option<String>,
    users_separation: &'static str,
    authentication_separation: &'static str,
    configuration_extensions_separation: &'static str,
    indexing: &'static str,
    full_text_search: &'static str,
    data_history: &'static str,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct CommonAttributeContentItem {
    metadata: String,
    use_mode: &'static str,
    conditional_separation: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct FunctionalOptionsParameterProperties {
    use_refs: Vec<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct LanguageProperties {
    language_code: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct DocumentNumeratorProperties {
    number_type: &'static str,
    number_length: u32,
    number_allowed_length: &'static str,
    number_periodicity: &'static str,
    check_unique: bool,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct WSReferenceProperties {
    location_url: String,
    manager_type_id: String,
    manager_value_id: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct IntegrationServiceProperties {
    external_address: String,
    manager_type_id: String,
    manager_value_id: String,
    channels: Vec<IntegrationServiceChannelProperties>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct IntegrationServiceChannelProperties {
    header: MetadataHeader,
    manager_type_id: String,
    manager_value_id: String,
    external_name: String,
    receive_message_processing: String,
    message_direction: &'static str,
    transactioned: bool,
}

#[derive(Debug, Clone, Eq, PartialEq)]
enum ConstantValueType {
    Boolean,
    String {
        length: Option<u32>,
        allowed_length_flag: u8,
    },
    Number {
        digits: u32,
        fraction_digits: u32,
        allowed_sign_flag: u8,
    },
    DateTime {
        date_fractions: &'static str,
    },
    Reference {
        reference: String,
    },
    ReferenceTypeSet {
        reference: String,
    },
}

#[allow(dead_code)]
fn build_metadata_type_index(rows: &[ConfigRow]) -> BTreeMap<String, String> {
    let metadata_texts = build_metadata_text_rows(rows);
    build_metadata_type_index_from_texts(&metadata_texts)
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum GeneratedTypeDcsPolicy {
    KeepId,
    Type,
    TypeSet,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct IndexedGeneratedType {
    type_id: String,
    reference: String,
    dcs_policy: GeneratedTypeDcsPolicy,
}

#[derive(Default)]
struct MetadataTypeIndexes {
    references: BTreeMap<String, String>,
    dcs: DcsTypeIndex,
}

// Platform reference-family TypeIds are protocol identifiers, not metadata UUIDs.
const DCS_BUILTIN_REFERENCE_TYPE_SETS: &[(&str, &str)] = &[
    (
        "0a52f9de-73ea-4507-81e8-66217bead73a",
        "cfg:ExchangePlanRef",
    ),
    (
        "214fa4d8-6ba4-4748-a5e1-6332b5887780",
        "cfg:BusinessProcessRef",
    ),
    ("38bfd075-3e63-4aaa-a93e-94521380d579", "cfg:DocumentRef"),
    (
        "593cd424-0877-470d-91f9-b90a982059b4",
        "cfg:ChartOfCalculationTypesRef",
    ),
    ("6291e9b3-8df5-44e1-b6b2-d9fe008016c0", "cfg:TaskRef"),
    (
        "99892482-ed55-4fb5-a7f7-20888820a758",
        "cfg:ChartOfCharacteristicTypesRef",
    ),
    (
        "ac606d60-0209-4159-8e4c-794bc091ce38",
        "cfg:ChartOfAccountsRef",
    ),
    ("e61ef7b8-f3e1-4f4b-8ac7-676e90524997", "cfg:CatalogRef"),
];

fn build_metadata_type_index_from_texts(rows: &[MetadataTextRow]) -> BTreeMap<String, String> {
    build_metadata_type_indexes_from_texts(rows).references
}

fn build_metadata_type_indexes_from_texts(rows: &[MetadataTextRow]) -> MetadataTypeIndexes {
    let mut indexes = MetadataTypeIndexes::default();
    let recalculation_refs = build_calculation_recalculation_reference_index(rows);
    for (type_id, qname) in DCS_BUILTIN_REFERENCE_TYPE_SETS {
        indexes.dcs.insert(
            (*type_id).to_string(),
            DcsTypeResolution::TypeSet {
                qname: (*qname).to_string(),
            },
        );
    }
    for row in rows {
        let entries = recalculation_refs
            .get(&row.file_name)
            .and_then(|recalculation_ref| {
                parse_indexed_recalculation_generated_types_from_text(row, recalculation_ref)
            })
            .or_else(|| parse_indexed_generated_types_from_text(row))
            .or_else(|| parse_indexed_generated_types_from_source_xml_text(&row.text));
        let Some(entries) = entries else { continue };
        for entry in entries {
            let type_id = entry.type_id.to_ascii_lowercase();
            indexes
                .references
                .insert(type_id.clone(), entry.reference.clone());
            let resolution = match entry.dcs_policy {
                GeneratedTypeDcsPolicy::KeepId => DcsTypeResolution::KeepId,
                GeneratedTypeDcsPolicy::Type => DcsTypeResolution::Type {
                    qname: entry.reference,
                },
                GeneratedTypeDcsPolicy::TypeSet => DcsTypeResolution::TypeSet {
                    qname: entry.reference,
                },
            };
            indexes.dcs.insert(type_id, resolution);
        }
    }
    indexes
}

#[allow(dead_code)]
fn parse_generated_type_entries_from_blob(
    blob: &[u8],
    uuid: &str,
) -> Option<Vec<(String, String)>> {
    let inflated = inflate_raw_deflate(blob).ok()?;
    let text = String::from_utf8(inflated).ok()?;
    let text = text.trim_start_matches('\u{feff}');
    let row = MetadataTextRow {
        file_name: uuid.to_string(),
        text: text.to_string(),
        object_code: parse_metadata_object_code(text),
        header: parse_metadata_header_from_text(text, uuid),
        kind: None,
        folder: None,
    };
    parse_generated_type_entries_from_text(&row)
}

fn parse_generated_type_entries_from_text(row: &MetadataTextRow) -> Option<Vec<(String, String)>> {
    parse_indexed_generated_types_from_text(row).map(|entries| {
        entries
            .into_iter()
            .map(|entry| (entry.type_id, entry.reference))
            .collect()
    })
}

fn parse_indexed_generated_types_from_text(
    row: &MetadataTextRow,
) -> Option<Vec<IndexedGeneratedType>> {
    let text = row.text.as_str();
    let object_code = row.object_code?;
    let header = row.header.as_ref()?;
    let root_fields = split_1c_braced_fields(text, 0)?;
    let object_text = *root_fields.get(1)?;
    let fields = split_1c_braced_fields(object_text, 0)?;
    let mut entries = Vec::new();
    let header_index = metadata_header_field_index(&fields, &row.file_name);

    for schema in raw_generated_type_schemas() {
        if schema.matches(object_code, header_index, &fields) {
            push_indexed_generated_type_slots(
                &mut entries,
                &fields,
                schema.slots,
                object_code,
                &header.name,
            );
        }
    }

    if object_code == 21 {
        let register_kind = if is_code21_accounting_register_fields(&fields, &row.file_name) {
            "AccountingRegister"
        } else {
            "CalculationRegister"
        };
        let start_index =
            register_generated_type_start_index(register_kind, &fields, &row.file_name)?;
        if register_kind == "AccountingRegister" {
            push_indexed_accounting_register_generated_type_entries(
                &mut entries,
                &fields,
                start_index,
                &header.name,
            );
        } else {
            push_indexed_register_generated_type_entries(
                &mut entries,
                &fields,
                start_index,
                register_kind,
                &header.name,
            );
        }
    }
    if object_code == 22 && header_index != Some(1) && field_is_unsigned_integer(fields.get(1)) {
        push_indexed_accounting_register_generated_type_entries(
            &mut entries,
            &fields,
            2,
            &header.name,
        );
    }

    Some(entries)
}

fn parse_indexed_recalculation_generated_types_from_text(
    row: &MetadataTextRow,
    recalculation_ref: &CalculationRecalculationReference,
) -> Option<Vec<IndexedGeneratedType>> {
    let header = row.header.as_ref()?;
    if row.object_code != Some(4) || header.name != recalculation_ref.recalculation_name {
        return None;
    }
    let (fields, _, _) = recalculation_object_fields(&row.text, &row.file_name)?;
    let name = format!(
        "{}.{}",
        recalculation_ref.owner_name, recalculation_ref.recalculation_name
    );
    let mut entries = Vec::new();
    for (field_index, generated_type) in [
        (1usize, "RecalculationRecord"),
        (3, "RecalculationManager"),
        (5, "RecalculationRecordSet"),
    ] {
        push_indexed_generated_type(
            &mut entries,
            &fields,
            field_index,
            generated_type,
            &name,
            GeneratedTypeDcsPolicy::Type,
        );
    }
    (entries.len() == 3).then_some(entries)
}

#[derive(Clone, Copy)]
struct RawGeneratedTypeSlot {
    field_index: usize,
    generated_type: &'static str,
}

#[derive(Clone, Copy)]
enum RawGeneratedTypeCondition {
    HeaderIndex(usize),
    FieldPrefix(usize, &'static str),
    FieldUuid(usize),
    FieldUuidRange(usize, usize),
}

#[derive(Clone, Copy)]
struct RawGeneratedTypeSchema {
    object_codes: &'static [u32],
    conditions: &'static [RawGeneratedTypeCondition],
    slots: &'static [RawGeneratedTypeSlot],
}

impl RawGeneratedTypeSchema {
    fn matches(self, object_code: u32, header_index: Option<usize>, fields: &[&str]) -> bool {
        self.object_codes.contains(&object_code)
            && self.conditions.iter().all(|condition| match *condition {
                RawGeneratedTypeCondition::HeaderIndex(expected) => header_index == Some(expected),
                RawGeneratedTypeCondition::FieldPrefix(index, prefix) => {
                    field_starts_with(fields.get(index), prefix)
                }
                RawGeneratedTypeCondition::FieldUuid(index) => fields
                    .get(index)
                    .copied()
                    .and_then(parse_uuid_field)
                    .is_some(),
                RawGeneratedTypeCondition::FieldUuidRange(start, count) => start
                    .checked_add(count)
                    .and_then(|end| fields.get(start..end))
                    .is_some_and(|fields| {
                        fields.iter().all(|field| parse_uuid_field(field).is_some())
                    }),
            })
    }
}

const RAW_GENERATED_TYPE_SCHEMAS: &[RawGeneratedTypeSchema] = &[
    RawGeneratedTypeSchema {
        object_codes: &[0],
        conditions: &[],
        slots: &[RawGeneratedTypeSlot {
            field_index: 1,
            generated_type: "DefinedType",
        }],
    },
    RawGeneratedTypeSchema {
        object_codes: &[16],
        conditions: &[],
        slots: &[
            RawGeneratedTypeSlot {
                field_index: 2,
                generated_type: "ConstantManager",
            },
            RawGeneratedTypeSlot {
                field_index: 4,
                generated_type: "ConstantValueManager",
            },
            RawGeneratedTypeSlot {
                field_index: 13,
                generated_type: "ConstantValueKey",
            },
        ],
    },
    RawGeneratedTypeSchema {
        object_codes: &[30],
        conditions: &[],
        slots: &[
            RawGeneratedTypeSlot {
                field_index: 3,
                generated_type: "BusinessProcessObject",
            },
            RawGeneratedTypeSlot {
                field_index: 5,
                generated_type: "BusinessProcessRef",
            },
            RawGeneratedTypeSlot {
                field_index: 7,
                generated_type: "BusinessProcessSelection",
            },
            RawGeneratedTypeSlot {
                field_index: 9,
                generated_type: "BusinessProcessList",
            },
            RawGeneratedTypeSlot {
                field_index: 11,
                generated_type: "BusinessProcessManager",
            },
            RawGeneratedTypeSlot {
                field_index: 13,
                generated_type: "BusinessProcessRoutePointRef",
            },
        ],
    },
    RawGeneratedTypeSchema {
        object_codes: &[36, 37],
        conditions: &[],
        slots: &[
            RawGeneratedTypeSlot {
                field_index: 1,
                generated_type: "ExchangePlanObject",
            },
            RawGeneratedTypeSlot {
                field_index: 3,
                generated_type: "ExchangePlanRef",
            },
            RawGeneratedTypeSlot {
                field_index: 5,
                generated_type: "ExchangePlanSelection",
            },
            RawGeneratedTypeSlot {
                field_index: 7,
                generated_type: "ExchangePlanList",
            },
            RawGeneratedTypeSlot {
                field_index: 9,
                generated_type: "ExchangePlanManager",
            },
        ],
    },
    RawGeneratedTypeSchema {
        object_codes: &[40],
        conditions: &[],
        slots: &[
            RawGeneratedTypeSlot {
                field_index: 1,
                generated_type: "DocumentObject",
            },
            RawGeneratedTypeSlot {
                field_index: 3,
                generated_type: "DocumentRef",
            },
            RawGeneratedTypeSlot {
                field_index: 26,
                generated_type: "DocumentManager",
            },
        ],
    },
    RawGeneratedTypeSchema {
        object_codes: &[17],
        conditions: &[],
        slots: &[
            RawGeneratedTypeSlot {
                field_index: 1,
                generated_type: "DataProcessorObject",
            },
            RawGeneratedTypeSlot {
                field_index: 7,
                generated_type: "DataProcessorManager",
            },
        ],
    },
    RawGeneratedTypeSchema {
        object_codes: &[19],
        conditions: &[],
        slots: &[
            RawGeneratedTypeSlot {
                field_index: 1,
                generated_type: "ReportObject",
            },
            RawGeneratedTypeSlot {
                field_index: 12,
                generated_type: "ReportManager",
            },
        ],
    },
    RawGeneratedTypeSchema {
        object_codes: &[2],
        conditions: &[
            RawGeneratedTypeCondition::HeaderIndex(1),
            RawGeneratedTypeCondition::FieldPrefix(1, "{0,"),
        ],
        slots: &[RawGeneratedTypeSlot {
            field_index: 2,
            generated_type: "SettingsStorageManager",
        }],
    },
    RawGeneratedTypeSchema {
        object_codes: &[56, 57],
        conditions: &[],
        slots: &[
            RawGeneratedTypeSlot {
                field_index: 1,
                generated_type: "CatalogObject",
            },
            RawGeneratedTypeSlot {
                field_index: 3,
                generated_type: "CatalogRef",
            },
            RawGeneratedTypeSlot {
                field_index: 5,
                generated_type: "CatalogSelection",
            },
            RawGeneratedTypeSlot {
                field_index: 7,
                generated_type: "CatalogList",
            },
            RawGeneratedTypeSlot {
                field_index: 34,
                generated_type: "CatalogManager",
            },
        ],
    },
    RawGeneratedTypeSchema {
        object_codes: &[20],
        conditions: &[RawGeneratedTypeCondition::HeaderIndex(5)],
        slots: &[RawGeneratedTypeSlot {
            field_index: 1,
            generated_type: "EnumRef",
        }],
    },
    RawGeneratedTypeSchema {
        object_codes: &[28],
        conditions: &[],
        slots: &[RawGeneratedTypeSlot {
            field_index: 9,
            generated_type: "AccumulationRegisterRecordSet",
        }],
    },
    RawGeneratedTypeSchema {
        object_codes: &[33],
        conditions: &[RawGeneratedTypeCondition::FieldUuid(1)],
        slots: &[
            RawGeneratedTypeSlot {
                field_index: 1,
                generated_type: "InformationRegisterRecord",
            },
            RawGeneratedTypeSlot {
                field_index: 3,
                generated_type: "InformationRegisterManager",
            },
            RawGeneratedTypeSlot {
                field_index: 5,
                generated_type: "InformationRegisterSelection",
            },
            RawGeneratedTypeSlot {
                field_index: 7,
                generated_type: "InformationRegisterList",
            },
            RawGeneratedTypeSlot {
                field_index: 9,
                generated_type: "InformationRegisterRecordSet",
            },
            RawGeneratedTypeSlot {
                field_index: 11,
                generated_type: "InformationRegisterRecordKey",
            },
            RawGeneratedTypeSlot {
                field_index: 13,
                generated_type: "InformationRegisterRecordManager",
            },
        ],
    },
    RawGeneratedTypeSchema {
        object_codes: &[33],
        conditions: &[RawGeneratedTypeCondition::HeaderIndex(1)],
        slots: &[
            RawGeneratedTypeSlot {
                field_index: 3,
                generated_type: "TaskObject",
            },
            RawGeneratedTypeSlot {
                field_index: 5,
                generated_type: "TaskRef",
            },
            RawGeneratedTypeSlot {
                field_index: 7,
                generated_type: "TaskSelection",
            },
            RawGeneratedTypeSlot {
                field_index: 9,
                generated_type: "TaskList",
            },
            RawGeneratedTypeSlot {
                field_index: 11,
                generated_type: "TaskManager",
            },
        ],
    },
    RawGeneratedTypeSchema {
        object_codes: &[34],
        conditions: &[],
        slots: &[
            RawGeneratedTypeSlot {
                field_index: 1,
                generated_type: "ChartOfCharacteristicTypesObject",
            },
            RawGeneratedTypeSlot {
                field_index: 3,
                generated_type: "ChartOfCharacteristicTypesRef",
            },
            RawGeneratedTypeSlot {
                field_index: 5,
                generated_type: "ChartOfCharacteristicTypesSelection",
            },
            RawGeneratedTypeSlot {
                field_index: 7,
                generated_type: "ChartOfCharacteristicTypesList",
            },
            RawGeneratedTypeSlot {
                field_index: 9,
                generated_type: "Characteristic",
            },
            RawGeneratedTypeSlot {
                field_index: 11,
                generated_type: "ChartOfCharacteristicTypesManager",
            },
        ],
    },
    RawGeneratedTypeSchema {
        object_codes: &[35],
        conditions: &[
            RawGeneratedTypeCondition::HeaderIndex(1),
            RawGeneratedTypeCondition::FieldUuidRange(2, 22),
        ],
        slots: &[
            RawGeneratedTypeSlot {
                field_index: 2,
                generated_type: "ChartOfCalculationTypesObject",
            },
            RawGeneratedTypeSlot {
                field_index: 4,
                generated_type: "ChartOfCalculationTypesRef",
            },
            RawGeneratedTypeSlot {
                field_index: 6,
                generated_type: "ChartOfCalculationTypesSelection",
            },
            RawGeneratedTypeSlot {
                field_index: 8,
                generated_type: "ChartOfCalculationTypesList",
            },
            RawGeneratedTypeSlot {
                field_index: 10,
                generated_type: "ChartOfCalculationTypesManager",
            },
            RawGeneratedTypeSlot {
                field_index: 12,
                generated_type: "DisplacingCalculationTypes",
            },
            RawGeneratedTypeSlot {
                field_index: 14,
                generated_type: "DisplacingCalculationTypesRow",
            },
            RawGeneratedTypeSlot {
                field_index: 16,
                generated_type: "BaseCalculationTypes",
            },
            RawGeneratedTypeSlot {
                field_index: 18,
                generated_type: "BaseCalculationTypesRow",
            },
            RawGeneratedTypeSlot {
                field_index: 20,
                generated_type: "LeadingCalculationTypes",
            },
            RawGeneratedTypeSlot {
                field_index: 22,
                generated_type: "LeadingCalculationTypesRow",
            },
        ],
    },
    RawGeneratedTypeSchema {
        object_codes: &[32],
        conditions: &[],
        slots: &[
            RawGeneratedTypeSlot {
                field_index: 1,
                generated_type: "ChartOfAccountsObject",
            },
            RawGeneratedTypeSlot {
                field_index: 3,
                generated_type: "ChartOfAccountsRef",
            },
            RawGeneratedTypeSlot {
                field_index: 5,
                generated_type: "ChartOfAccountsSelection",
            },
            RawGeneratedTypeSlot {
                field_index: 7,
                generated_type: "ChartOfAccountsList",
            },
            RawGeneratedTypeSlot {
                field_index: 9,
                generated_type: "ChartOfAccountsManager",
            },
        ],
    },
];

fn raw_generated_type_schemas() -> &'static [RawGeneratedTypeSchema] {
    RAW_GENERATED_TYPE_SCHEMAS
}

fn push_indexed_generated_type_slots(
    entries: &mut Vec<IndexedGeneratedType>,
    fields: &[&str],
    slots: &[RawGeneratedTypeSlot],
    object_code: u32,
    name: &str,
) {
    for slot in slots {
        push_indexed_generated_type(
            entries,
            fields,
            slot.field_index,
            slot.generated_type,
            name,
            raw_generated_type_dcs_policy(object_code, slot.field_index),
        );
    }
}

fn parse_indexed_generated_types_from_source_xml_text(
    text: &str,
) -> Option<Vec<IndexedGeneratedType>> {
    let text = text.trim_start_matches('\u{feff}').trim_start();
    if !text.starts_with('<') || !text.contains("GeneratedType") {
        return None;
    }

    let mut reader = NsReader::from_str(text);
    reader.config_mut().trim_text(true);
    let mut entries = Vec::new();
    let mut current_name = None::<String>;
    let mut current_category = None::<String>;
    let mut in_generated_type = false;
    let mut in_type_id = false;

    loop {
        match reader.read_event().ok()? {
            Event::Start(event) => {
                let (_, local) = reader.resolve_element(event.name());
                let local = local.as_ref();
                if local == b"GeneratedType" {
                    current_name = xml_attribute_value_ns(&reader, &event, "name")?;
                    current_category = xml_attribute_value_ns(&reader, &event, "category")?;
                    in_generated_type = current_name.is_some();
                } else if in_generated_type && local == b"TypeId" {
                    in_type_id = true;
                }
            }
            Event::Empty(event) => {
                let (_, local) = reader.resolve_element(event.name());
                if local.as_ref() == b"GeneratedType" {
                    current_name = None;
                    current_category = None;
                    in_generated_type = false;
                    in_type_id = false;
                }
            }
            Event::Text(event) => {
                if in_generated_type
                    && in_type_id
                    && let Some(name) = current_name.as_ref()
                    && let Ok(type_id) = event.decode()
                {
                    let type_id = type_id.trim();
                    if is_uuid_text(type_id) {
                        entries.push(IndexedGeneratedType {
                            type_id: type_id.to_string(),
                            reference: format!("cfg:{name}"),
                            dcs_policy: generated_type_dcs_policy(current_category.as_deref()),
                        });
                    }
                }
            }
            Event::End(event) => {
                let (_, local) = reader.resolve_element(event.name());
                let local = local.as_ref();
                if local == b"TypeId" {
                    in_type_id = false;
                } else if local == b"GeneratedType" {
                    current_name = None;
                    current_category = None;
                    in_generated_type = false;
                    in_type_id = false;
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }

    if entries.is_empty() {
        None
    } else {
        Some(entries)
    }
}

fn generated_type_dcs_policy(category: Option<&str>) -> GeneratedTypeDcsPolicy {
    match category {
        Some("DefinedType") => GeneratedTypeDcsPolicy::KeepId,
        Some("Characteristic") => GeneratedTypeDcsPolicy::TypeSet,
        _ => GeneratedTypeDcsPolicy::Type,
    }
}

fn raw_generated_type_dcs_policy(object_code: u32, field_index: usize) -> GeneratedTypeDcsPolicy {
    match (object_code, field_index) {
        (0, 1) => GeneratedTypeDcsPolicy::KeepId,
        (34, 9) => GeneratedTypeDcsPolicy::TypeSet,
        _ => GeneratedTypeDcsPolicy::Type,
    }
}

fn xml_attribute_value_ns(
    reader: &NsReader<&[u8]>,
    event: &quick_xml::events::BytesStart<'_>,
    name: &str,
) -> Option<Option<String>> {
    for attribute in event.attributes().with_checks(false) {
        let attribute = attribute.ok()?;
        let (_, local) = reader.resolve_attribute(attribute.key);
        if local.as_ref() == name.as_bytes() {
            return Some(Some(
                attribute
                    .decode_and_unescape_value(reader.decoder())
                    .ok()?
                    .into_owned(),
            ));
        }
    }
    Some(None)
}

fn push_indexed_generated_type(
    entries: &mut Vec<IndexedGeneratedType>,
    fields: &[&str],
    index: usize,
    generated_type: &str,
    name: &str,
    dcs_policy: GeneratedTypeDcsPolicy,
) {
    if let Some(type_id) = fields.get(index).copied().and_then(parse_uuid_field) {
        entries.push(IndexedGeneratedType {
            type_id,
            reference: format!("cfg:{generated_type}.{name}"),
            dcs_policy,
        });
    }
}

fn push_indexed_register_generated_type_entries(
    entries: &mut Vec<IndexedGeneratedType>,
    fields: &[&str],
    start_index: usize,
    type_prefix: &str,
    name: &str,
) {
    for (offset, suffix) in register_generated_type_suffixes() {
        push_indexed_generated_type(
            entries,
            fields,
            start_index + offset,
            &format!("{type_prefix}{suffix}"),
            name,
            GeneratedTypeDcsPolicy::Type,
        );
    }
}

fn push_indexed_accounting_register_generated_type_entries(
    entries: &mut Vec<IndexedGeneratedType>,
    fields: &[&str],
    start_index: usize,
    name: &str,
) {
    for (offset, suffix, _) in accounting_register_generated_type_slots() {
        push_indexed_generated_type(
            entries,
            fields,
            start_index + offset,
            &format!("AccountingRegister{suffix}"),
            name,
            GeneratedTypeDcsPolicy::Type,
        );
    }
}

fn parse_uuid_field(value: &str) -> Option<String> {
    let value = value.trim();
    if is_uuid_text(value) {
        Some(value.to_string())
    } else {
        None
    }
}

fn parse_non_zero_uuid(value: &str) -> Option<String> {
    let uuid = parse_uuid_field(value)?;
    if uuid == "00000000-0000-0000-0000-000000000000" {
        None
    } else {
        Some(uuid)
    }
}

fn is_uuid_text(value: &str) -> bool {
    value.len() == 36 && value.chars().all(|ch| ch.is_ascii_hexdigit() || ch == '-')
}

#[cfg(test)]
fn extract_metadata_source_xml(
    blob: &[u8],
    uuid: &str,
    type_index: &BTreeMap<String, String>,
    form_refs: &BTreeMap<String, FormSourceReference>,
    template_refs: &BTreeMap<String, TemplateSourceReference>,
) -> Option<ExtractedMetadataSourceXml> {
    extract_metadata_source_xml_with_refs(
        blob,
        uuid,
        type_index,
        &BTreeMap::new(),
        &BTreeMap::new(),
        form_refs,
        template_refs,
        &BTreeMap::new(),
        InfobaseConfigSourceVersion::V2_20,
    )
}

#[cfg(test)]
fn extract_metadata_source_xml_with_refs(
    blob: &[u8],
    uuid: &str,
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
    functional_option_refs: &BTreeMap<String, String>,
    form_refs: &BTreeMap<String, FormSourceReference>,
    template_refs: &BTreeMap<String, TemplateSourceReference>,
    subsystem_refs: &BTreeMap<String, SubsystemSourceReference>,
    source_version: InfobaseConfigSourceVersion,
) -> Option<ExtractedMetadataSourceXml> {
    extract_metadata_source_xml_with_recalculation_refs(
        blob,
        uuid,
        type_index,
        object_refs,
        object_refs,
        object_refs,
        &BTreeMap::new(),
        functional_option_refs,
        form_refs,
        template_refs,
        subsystem_refs,
        source_version,
    )
}

fn extract_metadata_source_xml_with_recalculation_refs(
    blob: &[u8],
    uuid: &str,
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
    metadata_object_refs: &BTreeMap<String, String>,
    configuration_root_object_refs: &BTreeMap<String, String>,
    recalculation_refs: &BTreeMap<String, CalculationRecalculationReference>,
    functional_option_refs: &BTreeMap<String, String>,
    form_refs: &BTreeMap<String, FormSourceReference>,
    template_refs: &BTreeMap<String, TemplateSourceReference>,
    subsystem_refs: &BTreeMap<String, SubsystemSourceReference>,
    source_version: InfobaseConfigSourceVersion,
) -> Option<ExtractedMetadataSourceXml> {
    if uuid.contains('.') {
        return None;
    }
    let row = metadata_text_row_from_blob(uuid, blob)?;
    extract_metadata_source_xml_from_text_row(
        &row,
        type_index,
        object_refs,
        metadata_object_refs,
        configuration_root_object_refs,
        recalculation_refs,
        functional_option_refs,
        form_refs,
        template_refs,
        subsystem_refs,
        source_version,
    )
}

fn extract_metadata_source_xml_from_text_row(
    row: &MetadataTextRow,
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
    metadata_object_refs: &BTreeMap<String, String>,
    configuration_root_object_refs: &BTreeMap<String, String>,
    recalculation_refs: &BTreeMap<String, CalculationRecalculationReference>,
    functional_option_refs: &BTreeMap<String, String>,
    form_refs: &BTreeMap<String, FormSourceReference>,
    template_refs: &BTreeMap<String, TemplateSourceReference>,
    subsystem_refs: &BTreeMap<String, SubsystemSourceReference>,
    source_version: InfobaseConfigSourceVersion,
) -> Option<ExtractedMetadataSourceXml> {
    let uuid = row.file_name.as_str();
    if uuid.contains('.') {
        return None;
    }
    let text = row.text.as_str();
    if let Some(xml) =
        extract_configuration_source_xml(text, uuid, configuration_root_object_refs, source_version)
    {
        return Some(ExtractedMetadataSourceXml {
            relative_path: PathBuf::from("Configuration.xml"),
            xml: xml.into_bytes(),
        });
    }
    let object_code = row.object_code?;
    if object_code == 4
        && let Some(recalculation_ref) = recalculation_refs.get(uuid)
    {
        let header = row.header.as_ref()?;
        if header.name != recalculation_ref.recalculation_name {
            return None;
        }
        let properties =
            parse_recalculation_properties_from_text(text, uuid, recalculation_ref, object_refs)?;
        let relative_path = PathBuf::from("CalculationRegisters")
            .join(sanitize_source_path_segment(&recalculation_ref.owner_name))
            .join("Recalculations")
            .join(sanitize_source_path_segment(&header.name))
            .with_extension("xml");
        let xml = format_recalculation_source_xml(header, &properties, source_version).into_bytes();
        return Some(ExtractedMetadataSourceXml { relative_path, xml });
    }
    if object_code == 12 {
        let header = row.header.as_ref()?;
        let flags = parse_common_module_flags_from_text(text, uuid)?;
        let relative_path = PathBuf::from("CommonModules")
            .join(sanitize_source_path_segment(&header.name))
            .with_extension("xml");
        let xml = format_common_module_source_xml(&header, &flags, source_version).into_bytes();
        return Some(ExtractedMetadataSourceXml { relative_path, xml });
    }
    if object_code == 16 {
        let header = row.header.as_ref()?;
        let constant =
            parse_constant_properties_from_text(text, uuid, type_index, object_refs, form_refs)?;
        let relative_path = PathBuf::from("Constants")
            .join(sanitize_source_path_segment(&header.name))
            .with_extension("xml");
        let xml = format_constant_source_xml(&header, &constant, source_version).into_bytes();
        return Some(ExtractedMetadataSourceXml { relative_path, xml });
    }
    if object_code == 3
        && row
            .kind
            .as_deref()
            .is_some_and(|kind| kind == "CommandGroup")
    {
        let header = row.header.as_ref()?;
        let command_group = parse_command_group_properties_from_text(text, uuid, object_refs)?;
        let relative_path = PathBuf::from("CommandGroups")
            .join(sanitize_source_path_segment(&header.name))
            .with_extension("xml");
        let xml =
            format_command_group_source_xml(&header, &command_group, source_version).into_bytes();
        return Some(ExtractedMetadataSourceXml { relative_path, xml });
    }
    if row
        .kind
        .as_deref()
        .is_some_and(|kind| kind == "CommonCommand")
    {
        let header = row.header.as_ref()?;
        let common_command =
            parse_common_command_properties_from_text(text, uuid, type_index, object_refs)?;
        let relative_path = PathBuf::from("CommonCommands")
            .join(sanitize_source_path_segment(&header.name))
            .with_extension("xml");
        let xml = format_common_command_source_xml_native(&header, &common_command, source_version)
            .into_bytes();
        return Some(ExtractedMetadataSourceXml { relative_path, xml });
    }
    if object_code == 3 && row.kind.as_deref().is_some_and(|kind| kind == "StyleItem") {
        let header = row.header.as_ref()?;
        let style_item = parse_style_item_properties_from_text(text, uuid)?;
        let relative_path = PathBuf::from("StyleItems")
            .join(sanitize_source_path_segment(&header.name))
            .with_extension("xml");
        let xml = format_style_item_source_xml(&header, &style_item).into_bytes();
        return Some(ExtractedMetadataSourceXml { relative_path, xml });
    }
    if row
        .kind
        .as_deref()
        .is_some_and(|kind| kind == "ScheduledJob")
    {
        if let Some(scheduled_job) =
            parse_scheduled_job_properties_from_text(text, uuid, object_refs)
        {
            let header = row.header.as_ref()?;
            let relative_path = PathBuf::from("ScheduledJobs")
                .join(sanitize_source_path_segment(&header.name))
                .with_extension("xml");
            let xml = format_scheduled_job_source_xml(&header, &scheduled_job, source_version)
                .into_bytes();
            return Some(ExtractedMetadataSourceXml { relative_path, xml });
        }
    }
    if row
        .kind
        .as_deref()
        .is_some_and(|kind| kind == "EventSubscription")
    {
        if let Some(event_subscription) =
            parse_event_subscription_properties_from_text(text, uuid, type_index, object_refs)
        {
            let header = row.header.as_ref()?;
            let relative_path = PathBuf::from("EventSubscriptions")
                .join(sanitize_source_path_segment(&header.name))
                .with_extension("xml");
            let xml =
                format_event_subscription_source_xml(&header, &event_subscription, source_version)
                    .into_bytes();
            return Some(ExtractedMetadataSourceXml { relative_path, xml });
        }
    }
    if object_code == 0 && is_defined_type_metadata_text(text, uuid) {
        let header = row.header.as_ref()?;
        let defined_type = parse_defined_type_properties_from_text(text, uuid, type_index)?;
        let relative_path = PathBuf::from("DefinedTypes")
            .join(sanitize_source_path_segment(&header.name))
            .with_extension("xml");
        let xml =
            format_defined_type_source_xml(&header, &defined_type, source_version).into_bytes();
        return Some(ExtractedMetadataSourceXml { relative_path, xml });
    }
    if row
        .kind
        .as_deref()
        .is_some_and(|kind| kind == "FunctionalOption")
    {
        let header = row.header.as_ref()?;
        let properties =
            parse_functional_option_properties_from_text(text, uuid, functional_option_refs)?;
        let relative_path = PathBuf::from("FunctionalOptions")
            .join(sanitize_source_path_segment(&header.name))
            .with_extension("xml");
        let xml =
            format_functional_option_source_xml(header, &properties, source_version).into_bytes();
        return Some(ExtractedMetadataSourceXml { relative_path, xml });
    }
    let direct_code14_form = is_direct_code14_form_metadata_text(text, uuid);
    if is_form_metadata_text(text, uuid) || direct_code14_form {
        let header = row.header.as_ref()?;
        let form_ref = form_refs.get(uuid);
        let (relative_path, kind) = if direct_code14_form {
            let form_ref = form_ref.filter(|form_ref| form_ref.kind == "Form")?;
            (form_ref.relative_path.clone(), form_ref.kind)
        } else {
            (
                form_ref
                    .map(|form_ref| form_ref.relative_path.clone())
                    .unwrap_or_else(|| {
                        PathBuf::from("CommonForms")
                            .join(sanitize_source_path_segment(&header.name))
                            .with_extension("xml")
                    }),
                form_ref
                    .map(|form_ref| form_ref.kind)
                    .unwrap_or("CommonForm"),
            )
        };
        let properties = parse_form_metadata_properties_from_text(text, kind, uuid);
        let xml = format_form_source_xml(kind, &header, &properties, source_version).into_bytes();
        return Some(ExtractedMetadataSourceXml { relative_path, xml });
    }
    if is_template_metadata_text(text, uuid) {
        let header = row.header.as_ref()?;
        let template_ref = template_refs.get(uuid);
        let relative_path = template_ref
            .map(|template_ref| template_ref.relative_path.clone())
            .unwrap_or_else(|| {
                PathBuf::from("CommonTemplates")
                    .join(sanitize_source_path_segment(&header.name))
                    .with_extension("xml")
            });
        let kind = template_ref
            .map(|template_ref| template_ref.kind)
            .unwrap_or("CommonTemplate");
        let template_type = template_ref
            .map(|template_ref| template_ref.template_type)
            .unwrap_or("BinaryData");
        let xml =
            format_template_source_xml(kind, &header, template_type, source_version).into_bytes();
        return Some(ExtractedMetadataSourceXml { relative_path, xml });
    }
    let kind = row.kind.as_deref()?;
    let folder = row.folder?;
    let header = row.header.as_ref()?;
    let relative_path = if kind == "Subsystem" {
        subsystem_refs
            .get(uuid)
            .map(|subsystem_ref| subsystem_ref.relative_path.clone())
            .unwrap_or_else(|| {
                PathBuf::from(folder)
                    .join(sanitize_source_path_segment(&header.name))
                    .with_extension("xml")
            })
    } else {
        PathBuf::from(folder)
            .join(sanitize_source_path_segment(&header.name))
            .with_extension("xml")
    };
    let nested_commands = if metadata_kind_can_own_commands(kind)
        && !matches!(kind, "Report" | "DataProcessor" | "InformationRegister")
    {
        nested_command_headers_for_owner_from_text(kind, text, uuid)
    } else {
        Vec::new()
    };
    let information_register_child_commands = if kind == "InformationRegister" {
        parse_information_register_child_commands(text, uuid, type_index, object_refs)
    } else {
        Vec::new()
    };
    let mut xml = if kind == "CommonPicture" {
        let picture = parse_common_picture_properties_from_text(text, uuid)?;
        format_common_picture_source_xml(&header, &picture, source_version).into_bytes()
    } else if kind == "Role" {
        format_full_metadata_source_xml(kind, &header, source_version).into_bytes()
    } else if kind == "Catalog" {
        let catalog = parse_catalog_properties_from_text(
            text,
            uuid,
            type_index,
            object_refs,
            metadata_object_refs,
            form_refs,
            template_refs,
        )?;
        format_catalog_source_xml(&header, &catalog).into_bytes()
    } else if kind == "Report" {
        let report = parse_report_properties_from_text(
            text,
            uuid,
            type_index,
            form_refs,
            template_refs,
            object_refs,
        )?;
        format_report_source_xml(&header, &report, source_version).into_bytes()
    } else if kind == "DataProcessor" {
        let data_processor = parse_data_processor_properties_from_text(
            text,
            uuid,
            type_index,
            object_refs,
            form_refs,
            template_refs,
        )?;
        format_data_processor_source_xml(&header, &data_processor, source_version).into_bytes()
    } else if kind == "Document" {
        let document = parse_document_properties_from_text(
            text,
            uuid,
            type_index,
            object_refs,
            metadata_object_refs,
            form_refs,
            template_refs,
        )?;
        format_document_source_xml(&header, &document, source_version).into_bytes()
    } else if kind == "BusinessProcess" {
        let business_process = parse_business_process_properties_from_text(text, uuid, form_refs)?;
        format_business_process_source_xml(&header, &business_process, source_version).into_bytes()
    } else if kind == "Task" {
        let task = parse_task_properties_from_text(text, uuid, form_refs)?;
        format_task_source_xml(&header, &task, source_version).into_bytes()
    } else if kind == "SettingsStorage" {
        let settings_storage = parse_settings_storage_properties_from_text(text, uuid)?;
        format_settings_storage_source_xml(&header, &settings_storage, source_version).into_bytes()
    } else if kind == "Enum" {
        let enumeration = parse_enum_properties_from_text(text, uuid, form_refs, template_refs)?;
        format_enum_source_xml(&header, &enumeration, source_version).into_bytes()
    } else if kind == "FunctionalOptionsParameter" {
        let properties =
            parse_functional_options_parameter_properties_from_text(text, uuid, object_refs)?;
        format_functional_options_parameter_source_xml(&header, &properties, source_version)
            .into_bytes()
    } else if kind == "Subsystem" {
        let subsystem =
            parse_subsystem_properties_from_text(text, uuid, object_refs, subsystem_refs)?;
        format_subsystem_source_xml(&header, &subsystem, source_version).into_bytes()
    } else if kind == "ExchangePlan" {
        let exchange_plan = parse_exchange_plan_properties_from_text(
            text,
            uuid,
            type_index,
            object_refs,
            form_refs,
        )?;
        format_exchange_plan_source_xml(&header, &exchange_plan, source_version).into_bytes()
    } else if metadata_kind_uses_register_resources(kind) {
        let register_object_refs = if kind == "InformationRegister" {
            metadata_object_refs
        } else {
            object_refs
        };
        let register = parse_register_properties_from_text(
            kind,
            text,
            uuid,
            type_index,
            register_object_refs,
            form_refs,
            source_version,
        )?;
        format_register_source_xml(kind, &header, &register, source_version).into_bytes()
    } else if kind == "Language" {
        let language = parse_language_properties_from_text(text, uuid)?;
        format_language_source_xml(&header, &language, source_version).into_bytes()
    } else if kind == "DocumentNumerator" {
        let document_numerator = parse_document_numerator_properties_from_text(text, uuid)?;
        format_document_numerator_source_xml(&header, &document_numerator, source_version)
            .into_bytes()
    } else if kind == "WSReference" {
        let ws_reference = parse_ws_reference_properties_from_text(text, uuid)?;
        format_ws_reference_source_xml(&header, &ws_reference, source_version).into_bytes()
    } else if kind == "IntegrationService" {
        let service = parse_integration_service_properties_from_text(text, uuid)?;
        format_integration_service_source_xml(&header, &service, source_version).into_bytes()
    } else if kind == "XDTOPackage" {
        let package = parse_xdto_package_properties_from_text(text, uuid)?;
        format_xdto_package_source_xml(&header, &package, source_version).into_bytes()
    } else if kind == "WebService" {
        let service = parse_web_service_properties_from_text(text, header, object_refs)?;
        format_web_service_source_xml(&header, &service, source_version).into_bytes()
    } else if kind == "HTTPService" {
        let service = parse_http_service_properties_from_text(text, uuid)?;
        format_http_service_source_xml(&header, &service, source_version).into_bytes()
    } else if kind == "CommonAttribute" {
        let common_attribute =
            parse_common_attribute_properties_from_text(text, uuid, type_index, object_refs)?;
        format_common_attribute_source_xml(&header, &common_attribute, source_version).into_bytes()
    } else if kind == "FilterCriterion" {
        format_filter_criterion_source_xml(&header, source_version).into_bytes()
    } else if metadata_kind_uses_default_list_form_properties(kind) {
        let properties =
            parse_default_list_form_metadata_properties_from_text(kind, text, uuid, form_refs)?;
        format_default_list_form_metadata_source_xml(kind, &header, &properties, source_version)
            .into_bytes()
    } else if is_typed_metadata_source(kind) {
        let typed = parse_typed_metadata_properties_from_text(text, uuid, type_index)?;
        format_typed_metadata_source_xml(kind, &header, &typed, source_version).into_bytes()
    } else {
        format_metadata_source_xml(kind, &header, source_version).into_bytes()
    };
    let owned_form_template_child_objects = simple_metadata_form_template_child_objects_xml(
        kind,
        folder,
        &header.name,
        text,
        form_refs,
        template_refs,
    );
    if !nested_commands.is_empty()
        || !information_register_child_commands.is_empty()
        || !owned_form_template_child_objects.is_empty()
    {
        let mut xml_text = String::from_utf8(xml).ok()?;
        insert_metadata_child_objects_xml(&mut xml_text, kind, &owned_form_template_child_objects);
        insert_metadata_child_command_objects_xml(
            &mut xml_text,
            kind,
            &information_register_child_commands,
        );
        insert_metadata_child_commands_xml(&mut xml_text, kind, &nested_commands);
        xml = xml_text.into_bytes();
    }

    Some(ExtractedMetadataSourceXml { relative_path, xml })
}

fn is_typed_metadata_source(kind: &str) -> bool {
    matches!(kind, "SessionParameter" | "CommonAttribute")
}

fn contains_wrapped_metadata_object_code(text: &str, code: u32, uuid: &str) -> bool {
    let marker = format!("{{1,0,{uuid}}}");
    let code_marker = format!("{{{code},");
    text.contains(&marker) && text.contains(&code_marker)
}

fn is_template_metadata_text(text: &str, uuid: &str) -> bool {
    let Some(fields) = metadata_object_fields(text) else {
        return false;
    };
    match parse_metadata_object_code(text) {
        Some(2) => {
            metadata_header_field_index(&fields, uuid) == Some(2)
                && field_is_unsigned_integer(fields.get(1))
                && !contains_wrapped_metadata_object_code(text, 9, uuid)
        }
        Some(4) => is_common_template_metadata_fields(&fields, uuid),
        _ => false,
    }
}

fn is_common_template_metadata_fields(fields: &[&str], uuid: &str) -> bool {
    fields.len() == 3
        && metadata_header_field_index(fields, uuid) == Some(1)
        && field_is_unsigned_integer(fields.get(2))
}

fn is_defined_type_metadata_text(text: &str, uuid: &str) -> bool {
    let Some(fields) = metadata_object_fields(text) else {
        return false;
    };
    metadata_header_field_index(&fields, uuid) == Some(3)
        && field_starts_with(fields.get(4), r#"{"Pattern""#)
}

fn metadata_object_fields(text: &str) -> Option<Vec<&str>> {
    let root_fields = split_1c_braced_fields(text, 0)?;
    let object_text = *root_fields.get(1)?;
    split_1c_braced_fields(object_text, 0)
}

fn metadata_header_field_index(fields: &[&str], uuid: &str) -> Option<usize> {
    let marker = format!("{{1,0,{uuid}}}");
    fields.iter().position(|field| field.contains(&marker))
}

fn field_starts_with(field: Option<&&str>, prefix: &str) -> bool {
    field
        .map(|value| value.trim_start().starts_with(prefix))
        .unwrap_or(false)
}

fn field_is_unsigned_integer(field: Option<&&str>) -> bool {
    field
        .map(|value| value.trim().chars().all(|ch| ch.is_ascii_digit()))
        .unwrap_or(false)
}

fn field_is_quoted_string(field: Option<&&str>) -> bool {
    field
        .map(|value| {
            let value = value.trim();
            value.len() >= 2 && value.starts_with('"') && value.ends_with('"')
        })
        .unwrap_or(false)
}

fn parse_common_module_flags_from_text(text: &str, uuid: &str) -> Option<CommonModuleFlags> {
    let marker = format!("{{1,0,{uuid}}},");
    let marker_start = text.find(&marker)?;
    let base_object_start = text[..marker_start].rfind("{3,")?;
    let owner_object_start = text[..base_object_start].rfind("{12,")?;
    let base_object_end = scan_1c_braced_value(text, base_object_start)?;
    let flags_start = expect_comma_at(text, base_object_end)?;
    let owner_object_end = scan_1c_braced_value(text, owner_object_start)?;
    let flags_end = owner_object_end.checked_sub(1)?;
    let flags = text[flags_start..flags_end]
        .split(',')
        .map(str::trim)
        .take(8)
        .collect::<Vec<_>>();
    if flags.len() != 8 {
        return None;
    }

    Some(CommonModuleFlags {
        client_ordinary_application: parse_1c_bool_flag(flags[0])?,
        server: parse_1c_bool_flag(flags[1])?,
        external_connection: parse_1c_bool_flag(flags[2])?,
        privileged: parse_1c_bool_flag(flags[3])?,
        global: parse_1c_bool_flag(flags[4])?,
        client_managed_application: parse_1c_bool_flag(flags[5])?,
        return_values_reuse: parse_return_values_reuse_flag(flags[6])?,
        server_call: parse_1c_bool_flag(flags[7])?,
    })
}

fn parse_common_picture_properties_from_text(
    text: &str,
    uuid: &str,
) -> Option<CommonPictureProperties> {
    let fields = metadata_object_fields(text)?;
    if fields.first().map(|value| value.trim()) != Some("4")
        || metadata_header_field_index(&fields, uuid) != Some(1)
    {
        return None;
    }
    Some(CommonPictureProperties {
        availability_for_choice: parse_1c_bool_flag(fields.get(2)?.trim())?,
        availability_for_appearance: parse_1c_bool_flag(fields.get(3)?.trim())?,
    })
}

fn parse_functional_option_properties_from_text(
    text: &str,
    uuid: &str,
    refs: &BTreeMap<String, String>,
) -> Option<FunctionalOptionProperties> {
    let fields = metadata_object_fields(text)?;
    if fields.first().map(|value| value.trim()) != Some("2")
        || metadata_header_field_index(&fields, uuid) != Some(1)
    {
        return None;
    }
    let location = fields
        .get(2)
        .and_then(|field| parse_non_zero_uuid(field.trim()))
        .and_then(|uuid| refs.get(&uuid).cloned());
    let content = fields
        .get(3)
        .map(|field| {
            uuid_like_values_in_text_order(field)
                .into_iter()
                .filter_map(|uuid| refs.get(&uuid).cloned())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Some(FunctionalOptionProperties {
        location,
        privileged_get_mode: parse_1c_bool_field(fields.get(4).copied()).unwrap_or(false),
        content,
    })
}

fn metadata_kind_uses_default_list_form_properties(kind: &str) -> bool {
    matches!(
        kind,
        "ChartOfAccounts" | "ChartOfCalculationTypes" | "ChartOfCharacteristicTypes"
    )
}

fn parse_default_list_form_metadata_properties_from_text(
    kind: &str,
    text: &str,
    uuid: &str,
    form_refs: &BTreeMap<String, FormSourceReference>,
) -> Option<DefaultListFormMetadataProperties> {
    let header = parse_metadata_header_from_text(text, uuid)?;
    let fields = metadata_object_fields(text)?;
    let header_index = metadata_header_field_index(&fields, uuid)?;
    let owner_folder = metadata_source_folder_for_kind(kind)?;
    Some(DefaultListFormMetadataProperties {
        use_standard_commands: parse_1c_bool_field(fields.get(header_index + 1).copied()),
        default_list_form: parse_default_list_form_ref(
            &fields,
            &[],
            form_refs,
            owner_folder,
            &header.name,
        ),
    })
}

fn parse_subsystem_properties_from_text(
    text: &str,
    uuid: &str,
    object_refs: &BTreeMap<String, String>,
    subsystem_refs: &BTreeMap<String, SubsystemSourceReference>,
) -> Option<SubsystemProperties> {
    let fields = metadata_object_fields(text)?;
    if fields.first().map(|value| value.trim()) != Some("22")
        || metadata_header_field_index(&fields, uuid) != Some(1)
    {
        return None;
    }
    let root_fields = split_1c_braced_fields(text, 0)?;
    let (picture_ref, picture_load_transparent) =
        parse_command_group_picture_value(fields.get(5)?, object_refs)?;
    Some(SubsystemProperties {
        include_help_in_contents: parse_1c_bool_field(fields.get(2).copied()).unwrap_or(false),
        include_in_command_interface: parse_1c_bool_field(fields.get(4).copied()).unwrap_or(true),
        use_one_command: parse_1c_bool_field(fields.get(8).copied()).unwrap_or(false),
        explanation: fields
            .get(6)
            .map(|field| parse_1c_synonyms(field))
            .unwrap_or_default(),
        picture_ref,
        picture_load_transparent,
        content: parse_subsystem_content(fields.get(7).copied(), object_refs),
        child_subsystems: parse_subsystem_child_references(
            root_fields.get(3).copied(),
            subsystem_refs,
        ),
    })
}

fn parse_subsystem_content(
    field: Option<&str>,
    object_refs: &BTreeMap<String, String>,
) -> Vec<String> {
    let Some(fields) = field.and_then(|field| split_1c_braced_fields(field, 0)) else {
        return Vec::new();
    };
    if fields.first().map(|field| field.trim()) != Some("0") {
        return Vec::new();
    }
    let count = fields
        .get(1)
        .and_then(|field| field.trim().parse::<usize>().ok())
        .unwrap_or(0);
    fields
        .iter()
        .skip(2)
        .take(count)
        .filter_map(|field| parse_design_time_reference(field, object_refs))
        .collect()
}

fn parse_subsystem_child_references(
    field: Option<&str>,
    subsystem_refs: &BTreeMap<String, SubsystemSourceReference>,
) -> Vec<String> {
    let Some(fields) = field.and_then(|field| split_1c_braced_fields(field, 0)) else {
        return Vec::new();
    };
    let count = fields
        .get(1)
        .and_then(|field| field.trim().parse::<usize>().ok())
        .unwrap_or(0);
    fields
        .iter()
        .skip(2)
        .take(count)
        .filter_map(|field| parse_uuid_field(field.trim()))
        .filter_map(|uuid| subsystem_refs.get(&uuid))
        .filter_map(|subsystem_ref| source_path_file_stem(&subsystem_ref.relative_path))
        .collect()
}

// Platform collection and field-reference type IDs confirmed across independent BSP and UT roots.
const EXCHANGE_PLAN_ROOT_COLLECTION_TYPE_UUIDS: [&str; 5] = [
    "1a1b4fea-e093-470d-94ff-1d2f16cda2ab",
    "3daea016-69b7-4ed4-9453-127911372fe6",
    "52293f4b-f98c-43ea-a80f-41047ae7ab58",
    "87c509ab-3d38-4d67-b379-aca796298578",
    "d5207c64-11d5-4d46-bba2-55b7b07ff4eb",
];
const EXCHANGE_PLAN_FIELD_REF_TYPE_UUID: &str = "60ea359f-3a6e-48bb-8e71-d2a457572918";

fn exchange_plan_root_collection_is_valid(
    value: &str,
    expected_type_uuid: &str,
    direct_uuid_items: bool,
) -> bool {
    let Some(fields) = split_information_register_braced_fields(value) else {
        return false;
    };
    if fields.len() < 2 || !information_register_uuid_matches(fields[0], expected_type_uuid) {
        return false;
    }
    let Some(count) = parse_information_register_usize(fields[1]) else {
        return false;
    };
    if fields.len() != count.checked_add(2).unwrap_or(usize::MAX) {
        return false;
    }
    if direct_uuid_items {
        let values = fields[2..]
            .iter()
            .map(|value| {
                parse_information_register_non_zero_uuid(value)
                    .map(|uuid| uuid.to_ascii_lowercase())
            })
            .collect::<Option<BTreeSet<_>>>();
        return values.is_some_and(|values| values.len() == count);
    }
    fields[2..].iter().all(|value| {
        split_information_register_braced_fields(value).is_some_and(|nested| !nested.is_empty())
    })
}

fn parse_exchange_plan_owner_fields<'a>(
    text: &'a str,
    expected_header: &MetadataHeader,
) -> Option<ExchangePlanOwnerFields<'a>> {
    let root = split_information_register_braced_fields(text.trim_start_matches('\u{feff}'))?;
    if root.len() != 8 || root.first()?.trim() != "1" || root.get(2)?.trim() != "5" {
        return None;
    }
    for (index, (field, expected_type_uuid)) in root[3..]
        .iter()
        .zip(EXCHANGE_PLAN_ROOT_COLLECTION_TYPE_UUIDS)
        .enumerate()
    {
        if !exchange_plan_root_collection_is_valid(
            field,
            expected_type_uuid,
            matches!(index, 1 | 3),
        ) {
            return None;
        }
    }

    let owner = split_information_register_braced_fields(root.get(1)?)?;
    match (owner.first().map(|field| field.trim()), owner.len()) {
        (Some("36"), 50) => {}
        (Some("37"), 51) if owner.get(50)?.trim() == "1" => {}
        _ => return None,
    }
    let owner_ids = owner[1..12]
        .iter()
        .map(|field| {
            parse_information_register_non_zero_uuid(field).map(|uuid| uuid.to_ascii_lowercase())
        })
        .collect::<Option<BTreeSet<_>>>()?;
    if owner_ids.len() != 11 {
        return None;
    }

    let parsed_header = parse_information_register_owner_header(owner.get(12)?)?;
    if !parsed_header
        .uuid
        .eq_ignore_ascii_case(&expected_header.uuid)
        || parsed_header.name != expected_header.name
        || parsed_header.synonyms != expected_header.synonyms
        || parsed_header.comment != expected_header.comment
    {
        return None;
    }
    Some(ExchangePlanOwnerFields { physical: owner })
}

fn parse_exchange_plan_generated_types(
    fields: &ExchangePlanOwnerFields<'_>,
    owner_name: &str,
) -> Option<Vec<GeneratedTypeEntry>> {
    let definitions = [
        (1, 2, "ExchangePlanObject", "Object"),
        (3, 4, "ExchangePlanRef", "Ref"),
        (5, 6, "ExchangePlanSelection", "Selection"),
        (7, 8, "ExchangePlanList", "List"),
        (9, 10, "ExchangePlanManager", "Manager"),
    ];
    definitions
        .into_iter()
        .map(|(type_index, value_index, prefix, category)| {
            Some(GeneratedTypeEntry {
                name: format!("{prefix}.{owner_name}"),
                category,
                type_id: parse_information_register_non_zero_uuid(fields.get(type_index)?)?,
                value_id: parse_information_register_non_zero_uuid(fields.get(value_index)?)?,
            })
        })
        .collect()
}

fn parse_exchange_plan_u32(value: &str) -> Option<u32> {
    u32::try_from(parse_information_register_usize(value)?).ok()
}

fn parse_exchange_plan_owned_form_ref(
    value: &str,
    owner_name: &str,
    form_refs: &BTreeMap<String, FormSourceReference>,
) -> Option<Option<String>> {
    let uuid = parse_information_register_uuid(value)?;
    if information_register_uuid_is_zero(&uuid) {
        return Some(None);
    }
    let mut matches = form_refs
        .iter()
        .filter(|(candidate, _)| candidate.eq_ignore_ascii_case(&uuid));
    let (_, form) = matches.next()?;
    if matches.next().is_some()
        || form.kind != "Form"
        || !is_owned_metadata_child_path(&form.relative_path, "ExchangePlans", owner_name, "Forms")
    {
        return None;
    }
    let reference = form_source_reference_name(form)?;
    let prefix = format!("ExchangePlan.{owner_name}.Form.");
    reference
        .strip_prefix(&prefix)
        .is_some_and(|name| !name.is_empty() && !name.contains('.'))
        .then_some(Some(reference))
}

fn resolve_exchange_plan_index_reference(
    uuid: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    let mut matches = object_refs
        .iter()
        .filter(|(candidate, _)| candidate.eq_ignore_ascii_case(uuid));
    let (_, reference) = matches.next()?;
    (matches.next().is_none() && !reference.is_empty()).then_some(reference.clone())
}

fn parse_exchange_plan_based_on(
    value: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<Option<String>> {
    let fields = split_information_register_braced_fields(value)?;
    match fields.as_slice() {
        [kind, count] if kind.trim() == "0" && count.trim() == "0" => Some(None),
        [kind, count, item] if kind.trim() == "0" && count.trim() == "1" => {
            let typed = split_information_register_braced_fields(item)?;
            if typed.len() != 3
                || typed.first()?.trim() != r##""#""##
                || !information_register_uuid_matches(typed.get(1)?, METADATA_OBJECT_REF_TYPE_UUID)
            {
                return None;
            }
            let reference = split_information_register_braced_fields(typed.get(2)?)?;
            if reference.len() != 2 || reference.first()?.trim() != "1" {
                return None;
            }
            let uuid = parse_information_register_non_zero_uuid(reference.get(1)?)?;
            Some(Some(resolve_exchange_plan_index_reference(
                &uuid,
                object_refs,
            )?))
        }
        _ => None,
    }
}

fn parse_exchange_plan_field_ref_payload(value: &str) -> Option<Vec<&str>> {
    let typed = split_information_register_braced_fields(value)?;
    if typed.len() != 3
        || typed.first()?.trim() != r##""#""##
        || !information_register_uuid_matches(typed.get(1)?, EXCHANGE_PLAN_FIELD_REF_TYPE_UUID)
    {
        return None;
    }
    split_information_register_braced_fields(typed.get(2)?)
}

fn parse_exchange_plan_field_ref_collection(value: &str) -> Option<Vec<&str>> {
    let outer = split_information_register_braced_fields(value)?;
    if outer.len() != 2 || outer.first()?.trim() != "1" {
        return None;
    }
    let payload = split_information_register_braced_fields(outer.get(1)?)?;
    if payload.len() < 2 || payload.first()?.trim() != "0" {
        return None;
    }
    let count = parse_information_register_usize(payload.get(1)?)?;
    (payload.len() == count.checked_add(2)?).then(|| payload[2..].to_vec())
}

fn parse_exchange_plan_input_by_string(value: &str, owner_name: &str) -> Option<Vec<String>> {
    let mut seen = BTreeSet::new();
    let fields = parse_exchange_plan_field_ref_collection(value)?
        .into_iter()
        .map(|value| {
            let payload = parse_exchange_plan_field_ref_payload(value)?;
            let marker = match payload.as_slice() {
                [marker] => marker.trim(),
                _ => return None,
            };
            let name = match marker {
                "-3" => "Description",
                "-2" => "Code",
                _ => return None,
            };
            seen.insert(marker)
                .then(|| format!("ExchangePlan.{owner_name}.StandardAttribute.{name}"))
        })
        .collect::<Option<Vec<_>>>()?;
    let suffixes = fields
        .iter()
        .filter_map(|field| field.rsplit('.').next())
        .collect::<Vec<_>>();
    matches!(
        suffixes.as_slice(),
        ["Description", "Code"] | ["Code", "Description"] | ["Description"]
    )
    .then_some(fields)
}

fn parse_exchange_plan_data_lock_fields(
    value: &str,
    owner_name: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<Vec<String>> {
    let mut seen = BTreeSet::new();
    parse_exchange_plan_field_ref_collection(value)?
        .into_iter()
        .map(|value| {
            let payload = parse_exchange_plan_field_ref_payload(value)?;
            let reference = match payload.as_slice() {
                [marker] => {
                    if marker.trim() != "-2" {
                        return None;
                    }
                    format!("ExchangePlan.{owner_name}.StandardAttribute.Code")
                }
                [kind, uuid] if kind.trim() == "0" => {
                    let uuid = parse_information_register_non_zero_uuid(uuid)?;
                    let reference = resolve_exchange_plan_index_reference(&uuid, object_refs)?;
                    let prefix = format!("ExchangePlan.{owner_name}.Attribute.");
                    if !reference
                        .strip_prefix(&prefix)
                        .is_some_and(|name| !name.is_empty() && !name.contains('.'))
                    {
                        return None;
                    }
                    reference
                }
                _ => return None,
            };
            seen.insert(reference.clone()).then_some(reference)
        })
        .collect()
}

fn exchange_plan_characteristics_is_empty(value: &str) -> bool {
    split_information_register_braced_fields(value).is_some_and(|fields| {
        fields.len() == 2
            && fields[0].trim() == "0"
            && split_information_register_braced_fields(fields[1])
                .is_some_and(|nested| nested.len() == 1 && nested[0].trim() == "0")
    })
}

fn exchange_plan_choice_data_get_mode_is_direct(value: &str) -> bool {
    split_information_register_braced_fields(value).is_some_and(|fields| {
        fields.len() == 3
            && fields[0].trim() == "1"
            && fields[1].trim() == "2"
            && fields[2].trim() == "0"
    })
}

fn parse_exchange_plan_properties_from_text(
    text: &str,
    uuid: &str,
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
    form_refs: &BTreeMap<String, FormSourceReference>,
) -> Option<ExchangePlanProperties> {
    let header = parse_metadata_header_from_text(text, uuid)?;
    let fields = parse_exchange_plan_owner_fields(text, &header)?;
    if fields.get(16)?.trim() != "0"
        || fields.get(21)?.trim() != "1"
        || fields.get(22)?.trim() != "2"
        || fields.get(25)?.trim() != "1"
        || !exchange_plan_characteristics_is_empty(fields.get(40)?)
        || fields.get(41)?.trim() != "1"
        || !exchange_plan_choice_data_get_mode_is_direct(fields.get(43)?)
        || fields.get(46)?.trim() != "0"
        || fields.get(47)?.trim() != "0"
        || fields.get(48)?.trim() != "0"
        || fields.get(49)?.trim() != "0"
    {
        return None;
    }

    Some(ExchangePlanProperties {
        this_node: parse_information_register_non_zero_uuid(fields.get(11)?)?,
        generated_types: parse_exchange_plan_generated_types(&fields, &header.name)?,
        use_standard_commands: information_register_bool(fields.get(13)?)?,
        code_length: parse_exchange_plan_u32(fields.get(15)?)?,
        code_allowed_length: match fields.get(39)?.trim() {
            "0" => "Fixed",
            "1" => "Variable",
            _ => return None,
        },
        description_length: parse_exchange_plan_u32(fields.get(17)?)?,
        default_presentation: "AsDescription",
        edit_type: "InDialog",
        quick_choice: information_register_bool(fields.get(23)?)?,
        choice_mode: "BothWays",
        input_by_string: parse_exchange_plan_input_by_string(fields.get(27)?, &header.name)?,
        search_string_mode_on_input_by_string: "Begin",
        full_text_search_on_input_by_string: "DontUse",
        choice_data_get_mode_on_input_by_string: "Directly",
        default_object_form: parse_exchange_plan_owned_form_ref(
            fields.get(14)?,
            &header.name,
            form_refs,
        )?,
        default_list_form: parse_exchange_plan_owned_form_ref(
            fields.get(19)?,
            &header.name,
            form_refs,
        )?,
        default_choice_form: parse_exchange_plan_owned_form_ref(
            fields.get(20)?,
            &header.name,
            form_refs,
        )?,
        auxiliary_object_form: parse_exchange_plan_owned_form_ref(
            fields.get(31)?,
            &header.name,
            form_refs,
        )?,
        auxiliary_list_form: parse_exchange_plan_owned_form_ref(
            fields.get(32)?,
            &header.name,
            form_refs,
        )?,
        auxiliary_choice_form: parse_exchange_plan_owned_form_ref(
            fields.get(33)?,
            &header.name,
            form_refs,
        )?,
        standard_attributes: parse_exchange_plan_standard_attributes(fields.get(30)?)?,
        based_on: parse_exchange_plan_based_on(fields.get(24)?, object_refs)?,
        distributed_infobase: information_register_bool(fields.get(26)?)?,
        include_configuration_extensions: information_register_bool(fields.get(45)?)?,
        create_on_input: "DontUse",
        choice_history_on_input: match fields.get(44)?.trim() {
            "0" => "Auto",
            "1" => "DontUse",
            _ => return None,
        },
        include_help_in_contents: information_register_bool(fields.get(18)?)?,
        data_lock_fields: parse_exchange_plan_data_lock_fields(
            fields.get(42)?,
            &header.name,
            object_refs,
        )?,
        data_lock_control_mode: match fields.get(28)?.trim() {
            "0" => "Automatic",
            "1" => "Managed",
            _ => return None,
        },
        full_text_search: match fields.get(29)?.trim() {
            "0" => "DontUse",
            "1" => "Use",
            _ => return None,
        },
        object_presentation: parse_information_register_owner_localized_value(fields.get(34)?)?,
        extended_object_presentation: parse_information_register_owner_localized_value(
            fields.get(35)?,
        )?,
        list_presentation: parse_information_register_owner_localized_value(fields.get(36)?)?,
        extended_list_presentation: parse_information_register_owner_localized_value(
            fields.get(37)?,
        )?,
        explanation: parse_information_register_owner_localized_value(fields.get(38)?)?,
        data_history: "DontUse",
        update_data_history_immediately_after_write: false,
        execute_after_write_data_history_version_processing: false,
        child_objects: parse_exchange_plan_child_objects(text, uuid, type_index),
    })
}

fn parse_exchange_plan_child_objects(
    text: &str,
    uuid: &str,
    type_index: &BTreeMap<String, String>,
) -> Vec<MetadataChildObject> {
    nested_headers_with_offsets_from_text(text, uuid, |_| true)
        .into_iter()
        .filter_map(|(header, marker_start)| {
            let tag = exchange_plan_child_object_tag(text, marker_start)?;
            let value_types =
                parse_metadata_child_value_types(text, marker_start, &header.uuid, type_index);
            let properties = parse_metadata_child_properties(
                "ExchangePlan",
                text,
                marker_start,
                &header.uuid,
                &value_types,
                &BTreeMap::new(),
            );
            Some(MetadataChildObject {
                tag,
                header,
                generated_types: Vec::new(),
                value_types,
                emit_empty_type: tag == "Attribute",
                properties,
                tabular_section_properties: None,
                child_objects: Vec::new(),
            })
        })
        .collect()
}

fn exchange_plan_child_object_tag(text: &str, marker_start: usize) -> Option<&'static str> {
    if is_offset_inside_metadata_object_code(text, marker_start, 4)
        && is_offset_inside_metadata_object_code(text, marker_start, 27)
    {
        Some("Attribute")
    } else {
        None
    }
}

fn parse_recalculation_properties_from_text(
    text: &str,
    uuid: &str,
    recalculation_ref: &CalculationRecalculationReference,
    object_refs: &BTreeMap<String, String>,
) -> Option<RecalculationProperties> {
    let (fields, data_lock_field, dimension_field) = recalculation_object_fields(text, uuid)?;
    let header = parse_metadata_header_from_text(text, uuid)?;
    if header.name != recalculation_ref.recalculation_name {
        return None;
    }

    let generated_type_name = format!(
        "{}.{}",
        recalculation_ref.owner_name, recalculation_ref.recalculation_name
    );
    let generated_types = [
        (1usize, 2usize, "RecalculationRecord", "Record"),
        (3, 4, "RecalculationManager", "Manager"),
        (5, 6, "RecalculationRecordSet", "RecordSet"),
    ]
    .into_iter()
    .map(|(type_index, value_index, prefix, category)| {
        Some(GeneratedTypeEntry {
            name: format!("{prefix}.{generated_type_name}"),
            category,
            type_id: parse_non_zero_uuid(fields.get(type_index)?.trim())?,
            value_id: parse_non_zero_uuid(fields.get(value_index)?.trim())?,
        })
    })
    .collect::<Option<Vec<_>>>()?;

    let data_lock_control_mode = match data_lock_field.trim() {
        "0" => "Automatic",
        "1" => "Managed",
        _ => return None,
    };
    let dimensions = parse_recalculation_dimensions(
        dimension_field,
        &recalculation_ref.owner_name,
        object_refs,
    )?;

    Some(RecalculationProperties {
        generated_types,
        data_lock_control_mode,
        dimensions,
    })
}

fn recalculation_object_fields<'a>(
    text: &'a str,
    uuid: &str,
) -> Option<(Vec<&'a str>, &'a str, &'a str)> {
    let root_fields = split_1c_braced_fields(text, 0)?;
    if root_fields.len() != 4 || root_fields.first()?.trim() != "1" {
        return None;
    }
    let fields = split_1c_braced_fields(root_fields.get(1)?, 0)?;
    let dimension_fields = split_1c_braced_fields(root_fields.get(3)?, 0)?;
    let dimension_count = dimension_fields.get(1)?.trim().parse::<usize>().ok()?;
    if fields.len() != 9
        || fields.first()?.trim() != "4"
        || metadata_header_field_index(&fields, uuid) != Some(7)
        || !is_recalculation_header_field(fields.get(7)?, uuid)
        || fields.get(8)?.trim() != "1"
        || !matches!(root_fields.get(2)?.trim(), "0" | "1")
        || dimension_fields.first()?.trim() != RECALCULATION_DIMENSION_LIST_MARKER
        || dimension_fields.len() != dimension_count + 2
        || !fields
            .iter()
            .skip(1)
            .take(6)
            .all(|field| parse_non_zero_uuid(field.trim()).is_some())
    {
        return None;
    }
    Some((fields, root_fields.get(2)?, root_fields.get(3)?))
}

fn is_recalculation_header_field(field: &str, uuid: &str) -> bool {
    let Some(wrapper) = split_1c_braced_fields(field, 0) else {
        return false;
    };
    if wrapper.len() != 2 || wrapper[0].trim() != "0" {
        return false;
    }
    let Some(header_fields) = split_1c_braced_fields(wrapper[1], 0) else {
        return false;
    };
    header_fields.first().map(|field| field.trim()) == Some("3")
        && metadata_header_field_index(&header_fields, uuid) == Some(1)
        && parse_metadata_header_from_text(wrapper[1], uuid).is_some()
}

fn parse_recalculation_dimensions(
    field: &str,
    owner_name: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<Vec<RecalculationDimension>> {
    let fields = split_1c_braced_fields(field, 0)?;
    if fields.first()?.trim() != RECALCULATION_DIMENSION_LIST_MARKER {
        return None;
    }
    let count = fields.get(1)?.trim().parse::<usize>().ok()?;
    if fields.len() != count + 2 {
        return None;
    }
    fields
        .iter()
        .skip(2)
        .map(|field| parse_recalculation_dimension(field, owner_name, object_refs))
        .collect()
}

fn parse_recalculation_dimension(
    field: &str,
    owner_name: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<RecalculationDimension> {
    let wrapper = split_1c_braced_fields(field, 0)?;
    if wrapper.len() != 2 || wrapper.get(1)?.trim() != "0" {
        return None;
    }
    let fields = split_1c_braced_fields(wrapper.first()?, 0)?;
    if fields.len() != 4 || fields.first()?.trim() != "1" {
        return None;
    }

    let header_field = fields.get(1)?;
    let header_fields = split_1c_braced_fields(header_field, 0)?;
    let dimension_uuid = uuid_like_values_in_text_order(header_field)
        .into_iter()
        .next()?;
    if header_fields.first()?.trim() != "3"
        || metadata_header_field_index(&header_fields, &dimension_uuid) != Some(1)
    {
        return None;
    }
    let header = parse_metadata_header_from_text(header_field, &dimension_uuid)?;

    let register_dimension_uuid = parse_non_zero_uuid(fields.get(2)?.trim())?;
    let leading = split_1c_braced_fields(fields.get(3)?, 0)?;
    if leading.len() != 3 || leading.first()?.trim() != "0" || leading.get(1)?.trim() != "1" {
        return None;
    }
    let typed_ref = split_1c_braced_fields(leading.get(2)?, 0)?;
    if typed_ref.len() != 3
        || typed_ref.first()?.trim() != "\"#\""
        || parse_non_zero_uuid(typed_ref.get(1)?.trim()).is_none()
    {
        return None;
    }
    let ref_value = split_1c_braced_fields(typed_ref.get(2)?, 0)?;
    if ref_value.len() != 2
        || ref_value.first()?.trim() != "1"
        || parse_non_zero_uuid(ref_value.get(1)?.trim()).as_deref()
            != Some(register_dimension_uuid.as_str())
    {
        return None;
    }

    let register_dimension = object_refs.get(&register_dimension_uuid)?.clone();
    let expected_prefix = format!("CalculationRegister.{owner_name}.Dimension.");
    if !register_dimension.starts_with(&expected_prefix)
        || register_dimension.len() == expected_prefix.len()
    {
        return None;
    }
    Some(RecalculationDimension {
        header,
        register_dimension,
    })
}

fn split_information_register_braced_fields(value: &str) -> Option<Vec<&str>> {
    let value = value.trim();
    if scan_1c_braced_value(value, 0)? != value.len() {
        return None;
    }
    split_1c_braced_fields(value, 0)
}

fn parse_information_register_usize(value: &str) -> Option<usize> {
    let value = value.trim();
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    value.parse().ok()
}

fn parse_information_register_uuid(value: &str) -> Option<String> {
    let value = value.trim();
    if value.len() != 36
        || value.bytes().enumerate().any(|(index, byte)| match index {
            8 | 13 | 18 | 23 => byte != b'-',
            _ => !byte.is_ascii_hexdigit(),
        })
    {
        return None;
    }
    Some(value.to_string())
}

fn parse_information_register_non_zero_uuid(value: &str) -> Option<String> {
    let value = parse_information_register_uuid(value)?;
    (!information_register_uuid_is_zero(&value)).then_some(value)
}

fn parse_information_register_quoted_string(value: &str) -> Option<String> {
    let value = value.trim();
    let (parsed, consumed) = parse_1c_quoted_string_with_len(value)?;
    (consumed == value.len() && parsed.chars().all(is_xml_1_0_char)).then_some(parsed)
}

fn parse_information_register_owner_localized_value(value: &str) -> Option<Vec<(String, String)>> {
    let fields = split_information_register_braced_fields(value)?;
    let count = parse_information_register_usize(fields.first()?)?;
    if fields.len() != count.checked_mul(2)?.checked_add(1)? {
        return None;
    }
    let mut languages = BTreeSet::new();
    let mut values = Vec::with_capacity(count);
    for pair in fields[1..].chunks_exact(2) {
        let language = parse_information_register_quoted_string(pair[0])?;
        let content = parse_information_register_quoted_string(pair[1])?;
        if !languages.insert(language.clone()) {
            return None;
        }
        values.push((language, content));
    }
    Some(values)
}

fn parse_information_register_owner_header(value: &str) -> Option<MetadataHeader> {
    let fields = split_information_register_braced_fields(value)?;
    if fields.len() != 9
        || fields.first()?.trim() != "3"
        || fields.get(5)?.trim() != "0"
        || fields.get(6)?.trim() != "0"
        || !parse_information_register_uuid(fields.get(7)?)
            .is_some_and(|uuid| information_register_uuid_is_zero(&uuid))
        || fields.get(8)?.trim() != "0"
    {
        return None;
    }
    let identity = split_information_register_braced_fields(fields.get(1)?)?;
    if identity.len() != 3 || identity.first()?.trim() != "1" || identity.get(1)?.trim() != "0" {
        return None;
    }
    Some(MetadataHeader {
        uuid: parse_information_register_non_zero_uuid(identity.get(2)?)?,
        name: parse_information_register_quoted_string(fields.get(2)?)?,
        synonyms: parse_information_register_owner_localized_value(fields.get(3)?)?,
        comment: parse_information_register_quoted_string(fields.get(4)?)?,
        template_type_code: None,
    })
}

fn information_register_root_collection_is_valid(value: &str) -> bool {
    let Some(fields) = split_information_register_braced_fields(value) else {
        return false;
    };
    if fields.len() < 2 || parse_information_register_non_zero_uuid(fields[0]).is_none() {
        return false;
    }
    let Some(count) = fields
        .get(1)
        .and_then(|value| parse_information_register_usize(value))
    else {
        return false;
    };
    if count == 0 {
        return fields.len() == 2;
    }
    if let [_, _, values] = fields.as_slice()
        && values.trim_start().starts_with('{')
    {
        return split_information_register_braced_fields(values).is_some_and(|values| {
            values.len() == count
                && values
                    .iter()
                    .all(|value| split_information_register_braced_fields(value).is_some())
        });
    }
    if fields.len() != count.checked_add(2).unwrap_or(usize::MAX) {
        return false;
    }
    let values = fields[2..]
        .iter()
        .map(|value| {
            parse_information_register_non_zero_uuid(value).map(|uuid| uuid.to_ascii_lowercase())
        })
        .collect::<Option<BTreeSet<_>>>();
    values.is_some_and(|values| values.len() == count)
}

fn parse_information_register_owner_fields<'a>(
    text: &'a str,
    expected_header: &MetadataHeader,
) -> Option<InformationRegisterOwnerFields<'a>> {
    let root = split_information_register_braced_fields(text.trim_start_matches('\u{feff}'))?;
    if root.len() != 9
        || root.first()?.trim() != "1"
        || root.get(2)?.trim() != "6"
        || !root[3..]
            .iter()
            .all(|field| information_register_root_collection_is_valid(field))
    {
        return None;
    }
    let collection_type_ids = root[3..]
        .iter()
        .map(|field| {
            split_information_register_braced_fields(field)
                .and_then(|fields| parse_information_register_non_zero_uuid(fields.first()?))
                .map(|uuid| uuid.to_ascii_lowercase())
        })
        .collect::<Option<BTreeSet<_>>>()?;
    if collection_type_ids.len() != 6 {
        return None;
    }

    let owner = split_information_register_braced_fields(root.get(1)?)?;
    if owner.len() != 39 || owner.first()?.trim() != "33" {
        return None;
    }
    let generated_type_ids = owner[1..15]
        .iter()
        .map(|field| {
            parse_information_register_non_zero_uuid(field).map(|uuid| uuid.to_ascii_lowercase())
        })
        .collect::<Option<BTreeSet<_>>>()?;
    if generated_type_ids.len() != 14 {
        return None;
    }

    let header_wrapper = split_information_register_braced_fields(owner.get(15)?)?;
    if header_wrapper.len() != 2 || header_wrapper.first()?.trim() != "0" {
        return None;
    }
    let header_field = header_wrapper.get(1)?;
    let parsed_header = parse_information_register_owner_header(header_field)?;
    if !parsed_header
        .uuid
        .eq_ignore_ascii_case(&expected_header.uuid)
        || parsed_header.name != expected_header.name
        || parsed_header.synonyms != expected_header.synonyms
        || parsed_header.comment != expected_header.comment
    {
        return None;
    }

    let matching_headers = owner
        .iter()
        .filter(|field| {
            let Some(wrapper) = split_information_register_braced_fields(field) else {
                return false;
            };
            if wrapper.len() != 2 || wrapper.first().map(|value| value.trim()) != Some("0") {
                return false;
            }
            let Some(header) = wrapper
                .get(1)
                .and_then(|value| parse_information_register_owner_header(value))
            else {
                return false;
            };
            header.uuid.eq_ignore_ascii_case(&expected_header.uuid)
        })
        .count();
    if matching_headers != 1 {
        return None;
    }

    let mut logical = Vec::with_capacity(25);
    logical.extend(header_wrapper);
    logical.extend(owner[16..].iter().copied());
    Some(InformationRegisterOwnerFields {
        logical: logical.try_into().ok()?,
    })
}

fn information_register_periodicity_xml(value: &str) -> Option<&'static str> {
    match value.trim() {
        "0" => Some("Nonperiodical"),
        "1" => Some("Year"),
        "2" => Some("Quarter"),
        "3" => Some("Month"),
        "4" => Some("Day"),
        "5" => Some("Second"),
        "6" => Some("RecorderPosition"),
        _ => None,
    }
}

fn information_register_write_mode_xml(value: &str) -> Option<&'static str> {
    match value.trim() {
        "0" => Some("Independent"),
        "1" => Some("RecorderSubordinate"),
        _ => None,
    }
}

fn information_register_edit_type_xml(value: &str) -> Option<&'static str> {
    match value.trim() {
        "0" => Some("InList"),
        "1" => Some("InDialog"),
        "2" => Some("BothWays"),
        _ => None,
    }
}

fn information_register_data_lock_control_mode_xml(value: &str) -> Option<&'static str> {
    match value.trim() {
        "0" => Some("Automatic"),
        "1" => Some("Managed"),
        _ => None,
    }
}

fn parse_information_register_owned_form_ref(
    value: &str,
    owner_name: &str,
    form_refs: &BTreeMap<String, FormSourceReference>,
) -> Option<Option<String>> {
    let uuid = parse_information_register_uuid(value)?;
    if information_register_uuid_is_zero(&uuid) {
        return Some(None);
    }
    let mut matches = form_refs
        .iter()
        .filter(|(candidate, _)| candidate.eq_ignore_ascii_case(&uuid));
    let (_, form) = matches.next()?;
    if matches.next().is_some()
        || form.kind != "Form"
        || !is_owned_metadata_child_path(
            &form.relative_path,
            "InformationRegisters",
            owner_name,
            "Forms",
        )
    {
        return None;
    }
    let reference = form_source_reference_name(form)?;
    let prefix = format!("InformationRegister.{owner_name}.Form.");
    reference
        .strip_prefix(&prefix)
        .is_some_and(|name| !name.is_empty() && !name.contains('.'))
        .then_some(Some(reference))
}

// Platform serialization IDs shared by the BSP, UT, and SFC cohorts; these are not
// identities of metadata objects from the information base being decoded.
const INFORMATION_REGISTER_STANDARD_ATTRIBUTE_SECTION_UUID: &str =
    "510405d3-2a0c-4fea-960a-7fee59b32f9b";
const INFORMATION_REGISTER_STANDARD_ATTRIBUTE_LOCALIZED_TYPE_UUID: &str =
    "87024738-fc2a-4436-ada1-df79d395c424";
const INFORMATION_REGISTER_STANDARD_ATTRIBUTE_LINK_BY_TYPE_UUID: &str =
    "9ad557b1-249e-48dc-824b-3e149ecf10a6";
const INFORMATION_REGISTER_STANDARD_ATTRIBUTE_FILL_CHECKING_UUID: &str =
    "98ea8e5a-b586-442b-b944-6e3447734aa7";
const INFORMATION_REGISTER_STANDARD_ATTRIBUTE_CREATE_ON_INPUT_UUID: &str =
    "ad3615c5-aae6-4725-89be-91827523abd9";
const INFORMATION_REGISTER_STANDARD_ATTRIBUTE_TYPE_REDUCTION_UUID: &str =
    "502b7765-f89c-4fd0-924f-0a28d3dc09b7";
const INFORMATION_REGISTER_STANDARD_ATTRIBUTE_QUICK_CHOICE_UUID: &str =
    "ace3fd07-11b2-477e-ab7f-36f0ea37c8dd";
const INFORMATION_REGISTER_STANDARD_ATTRIBUTE_CHOICE_HISTORY_UUID: &str =
    "12ca4003-ac70-450e-b897-37faf86bd313";
const INFORMATION_REGISTER_STANDARD_ATTRIBUTE_DATA_HISTORY_UUID: &str =
    "d46ea122-3201-4e5e-bed4-e669c6e463c8";
const INFORMATION_REGISTER_STANDARD_ATTRIBUTE_FULL_TEXT_SEARCH_UUID: &str =
    "3b8e6bdd-d648-49d5-af2f-d46d84f87dd5";
const INFORMATION_REGISTER_STANDARD_ATTRIBUTE_CHOICE_PARAMETER_LINKS_UUID: &str =
    "b76a58b9-2a56-4e46-bb31-8e04ad9f31ae";
const INFORMATION_REGISTER_STANDARD_ATTRIBUTE_CHOICE_PARAMETERS_UUID: &str =
    "f2eaae14-91a7-47b9-9d69-097877f41580";
const INFORMATION_REGISTER_STANDARD_ATTRIBUTE_LINK_BY_TYPE_PROPERTY_UUID: &str =
    "1183c14f-f814-49c6-9233-a3c26b3f64cf";
const INFORMATION_REGISTER_STANDARD_ATTRIBUTE_FILL_CHECKING_PROPERTY_UUID: &str =
    "2723eb98-b4c1-498a-a6f3-70444757902f";
const INFORMATION_REGISTER_STANDARD_ATTRIBUTE_MULTI_LINE_PROPERTY_UUID: &str =
    "2bbba66b-fabf-4863-8ba3-54b3c64c896e";
const INFORMATION_REGISTER_STANDARD_ATTRIBUTE_FILL_FROM_FILLING_VALUE_PROPERTY_UUID: &str =
    "2c8143d5-4248-4c43-8bfb-307c0be2e415";
const INFORMATION_REGISTER_STANDARD_ATTRIBUTE_CREATE_ON_INPUT_PROPERTY_UUID: &str =
    "33c74a4d-561f-4bc0-9eaa-8d21c893c0a9";
const INFORMATION_REGISTER_STANDARD_ATTRIBUTE_TYPE_REDUCTION_PROPERTY_UUID: &str =
    "3b10624f-1e3d-495d-8093-25225efc5313";
const INFORMATION_REGISTER_STANDARD_ATTRIBUTE_MAX_VALUE_PROPERTY_UUID: &str =
    "3eaf5a8b-06d6-47b0-ac7d-a9698247f499";
const INFORMATION_REGISTER_STANDARD_ATTRIBUTE_TOOLTIP_PROPERTY_UUID: &str =
    "4690ff70-e3fa-4914-9127-6a9acc5fc949";
const INFORMATION_REGISTER_STANDARD_ATTRIBUTE_EXTENDED_EDIT_PROPERTY_UUID: &str =
    "4de03908-56f4-4396-a61e-17253afca9ac";
const INFORMATION_REGISTER_STANDARD_ATTRIBUTE_FORMAT_PROPERTY_UUID: &str =
    "580c29e2-8af4-4258-882a-7cf8073e61c8";
const INFORMATION_REGISTER_STANDARD_ATTRIBUTE_CHOICE_FORM_PROPERTY_UUID: &str =
    "6c4f7074-e7d4-48eb-b31b-132873666262";
const INFORMATION_REGISTER_STANDARD_ATTRIBUTE_QUICK_CHOICE_PROPERTY_UUID: &str =
    "6e3a1131-37a3-4da5-8895-572d9d0c9db6";
const INFORMATION_REGISTER_STANDARD_ATTRIBUTE_CHOICE_HISTORY_PROPERTY_UUID: &str =
    "7ba608f2-e654-42a3-8885-334fe88ca910";
const INFORMATION_REGISTER_STANDARD_ATTRIBUTE_EDIT_FORMAT_PROPERTY_UUID: &str =
    "88149a78-9448-4767-867b-0e650d165d2e";
const INFORMATION_REGISTER_STANDARD_ATTRIBUTE_PASSWORD_MODE_PROPERTY_UUID: &str =
    "90ae4b5d-e0fd-49ef-a008-d67c1e75038c";
const INFORMATION_REGISTER_STANDARD_ATTRIBUTE_DATA_HISTORY_PROPERTY_UUID: &str =
    "9288a8ed-b259-46d0-a8e3-70d87956ff2d";
const INFORMATION_REGISTER_STANDARD_ATTRIBUTE_MARK_NEGATIVES_PROPERTY_UUID: &str =
    "b02800e9-a8d1-42ab-9a12-f673e92be968";
const INFORMATION_REGISTER_STANDARD_ATTRIBUTE_MIN_VALUE_PROPERTY_UUID: &str =
    "c65a541f-0b91-4f33-bc88-fbaaa57f9992";
const INFORMATION_REGISTER_STANDARD_ATTRIBUTE_SYNONYM_PROPERTY_UUID: &str =
    "cf4abea3-37b2-11d4-940f-008048da11f9";
const INFORMATION_REGISTER_STANDARD_ATTRIBUTE_COMMENT_PROPERTY_UUID: &str =
    "cf4abea4-37b2-11d4-940f-008048da11f9";
const INFORMATION_REGISTER_STANDARD_ATTRIBUTE_FULL_TEXT_SEARCH_PROPERTY_UUID: &str =
    "d4232326-022b-421e-b6d3-88e418f74327";
const INFORMATION_REGISTER_STANDARD_ATTRIBUTE_CHOICE_PARAMETER_LINKS_PROPERTY_UUID: &str =
    "e3da683b-c54a-457a-a243-b9b4f9bf76dd";
const INFORMATION_REGISTER_STANDARD_ATTRIBUTE_FILL_VALUE_PROPERTY_UUID: &str =
    "e6b3f5f3-bdf3-4ad0-bc60-7323b3feb208";
const INFORMATION_REGISTER_STANDARD_ATTRIBUTE_MASK_PROPERTY_UUID: &str =
    "f49e4ced-4033-4e6c-8755-9fbaaccd6078";
const INFORMATION_REGISTER_STANDARD_ATTRIBUTE_CHOICE_PARAMETERS_PROPERTY_UUID: &str =
    "fcf503b8-1c06-454a-970c-06413e64aee5";
const INFORMATION_REGISTER_STANDARD_ATTRIBUTE_KEYS: [&str; 25] = [
    INFORMATION_REGISTER_STANDARD_ATTRIBUTE_LINK_BY_TYPE_PROPERTY_UUID,
    INFORMATION_REGISTER_STANDARD_ATTRIBUTE_FILL_CHECKING_PROPERTY_UUID,
    INFORMATION_REGISTER_STANDARD_ATTRIBUTE_MULTI_LINE_PROPERTY_UUID,
    INFORMATION_REGISTER_STANDARD_ATTRIBUTE_FILL_FROM_FILLING_VALUE_PROPERTY_UUID,
    INFORMATION_REGISTER_STANDARD_ATTRIBUTE_CREATE_ON_INPUT_PROPERTY_UUID,
    INFORMATION_REGISTER_STANDARD_ATTRIBUTE_TYPE_REDUCTION_PROPERTY_UUID,
    INFORMATION_REGISTER_STANDARD_ATTRIBUTE_MAX_VALUE_PROPERTY_UUID,
    INFORMATION_REGISTER_STANDARD_ATTRIBUTE_TOOLTIP_PROPERTY_UUID,
    INFORMATION_REGISTER_STANDARD_ATTRIBUTE_EXTENDED_EDIT_PROPERTY_UUID,
    INFORMATION_REGISTER_STANDARD_ATTRIBUTE_FORMAT_PROPERTY_UUID,
    INFORMATION_REGISTER_STANDARD_ATTRIBUTE_CHOICE_FORM_PROPERTY_UUID,
    INFORMATION_REGISTER_STANDARD_ATTRIBUTE_QUICK_CHOICE_PROPERTY_UUID,
    INFORMATION_REGISTER_STANDARD_ATTRIBUTE_CHOICE_HISTORY_PROPERTY_UUID,
    INFORMATION_REGISTER_STANDARD_ATTRIBUTE_EDIT_FORMAT_PROPERTY_UUID,
    INFORMATION_REGISTER_STANDARD_ATTRIBUTE_PASSWORD_MODE_PROPERTY_UUID,
    INFORMATION_REGISTER_STANDARD_ATTRIBUTE_DATA_HISTORY_PROPERTY_UUID,
    INFORMATION_REGISTER_STANDARD_ATTRIBUTE_MARK_NEGATIVES_PROPERTY_UUID,
    INFORMATION_REGISTER_STANDARD_ATTRIBUTE_MIN_VALUE_PROPERTY_UUID,
    INFORMATION_REGISTER_STANDARD_ATTRIBUTE_SYNONYM_PROPERTY_UUID,
    INFORMATION_REGISTER_STANDARD_ATTRIBUTE_COMMENT_PROPERTY_UUID,
    INFORMATION_REGISTER_STANDARD_ATTRIBUTE_FULL_TEXT_SEARCH_PROPERTY_UUID,
    INFORMATION_REGISTER_STANDARD_ATTRIBUTE_CHOICE_PARAMETER_LINKS_PROPERTY_UUID,
    INFORMATION_REGISTER_STANDARD_ATTRIBUTE_FILL_VALUE_PROPERTY_UUID,
    INFORMATION_REGISTER_STANDARD_ATTRIBUTE_MASK_PROPERTY_UUID,
    INFORMATION_REGISTER_STANDARD_ATTRIBUTE_CHOICE_PARAMETERS_PROPERTY_UUID,
];

struct InformationRegisterStandardAttributeBag<'a> {
    values: BTreeMap<String, &'a str>,
    has_type_reduction_mode: bool,
}

impl<'a> InformationRegisterStandardAttributeBag<'a> {
    fn get(&self, key: &str) -> Option<&'a str> {
        self.values.get(key).copied()
    }
}

fn information_register_uuid_matches(value: &str, expected: &str) -> bool {
    parse_information_register_uuid(value).is_some_and(|value| value.eq_ignore_ascii_case(expected))
}

fn parse_information_register_standard_attribute_bag(
    value: &str,
) -> Option<InformationRegisterStandardAttributeBag<'_>> {
    let fields = split_information_register_braced_fields(value)?;
    let has_type_reduction_mode = match (
        fields.first().map(|field| field.trim()),
        fields.get(1).map(|field| field.trim()),
        fields.len(),
    ) {
        (Some("13"), Some("24"), 50) => false,
        (Some("14"), Some("25"), 52) => true,
        _ => return None,
    };
    let expected_keys = INFORMATION_REGISTER_STANDARD_ATTRIBUTE_KEYS
        .iter()
        .enumerate()
        .filter_map(|(index, key)| (has_type_reduction_mode || index != 5).then_some(*key));
    let mut values = BTreeMap::new();
    for (pair, expected_key) in fields[2..].chunks_exact(2).zip(expected_keys) {
        if !information_register_uuid_matches(pair[0], expected_key)
            || values.insert(expected_key.to_string(), pair[1]).is_some()
        {
            return None;
        }
    }
    let expected_count = if has_type_reduction_mode { 25 } else { 24 };
    (values.len() == expected_count).then_some(InformationRegisterStandardAttributeBag {
        values,
        has_type_reduction_mode,
    })
}

fn parse_information_register_standard_attribute_direct_enum<'a>(
    value: &'a str,
    type_uuid: &str,
) -> Option<&'a str> {
    let fields = split_information_register_braced_fields(value)?;
    if fields.len() != 3
        || fields.first()?.trim() != r##""#""##
        || !information_register_uuid_matches(fields.get(1)?, type_uuid)
    {
        return None;
    }
    Some(fields.get(2)?.trim())
}

fn parse_information_register_standard_attribute_nested_enum<'a>(
    value: &'a str,
    type_uuid: &str,
) -> Option<&'a str> {
    let fields = split_information_register_braced_fields(value)?;
    if fields.len() != 3
        || fields.first()?.trim() != r##""#""##
        || !information_register_uuid_matches(fields.get(1)?, type_uuid)
    {
        return None;
    }
    let nested = split_information_register_braced_fields(fields.get(2)?)?;
    if nested.len() != 2 || !information_register_uuid_matches(nested.first()?, type_uuid) {
        return None;
    }
    Some(nested.get(1)?.trim())
}

fn parse_information_register_standard_attribute_bool(value: &str) -> Option<bool> {
    let fields = split_information_register_braced_fields(value)?;
    if fields.len() != 2 || fields.first()?.trim() != r#""B""# {
        return None;
    }
    information_register_bool(fields.get(1)?)
}

fn information_register_standard_attribute_nil_is_valid(value: &str) -> bool {
    split_information_register_braced_fields(value)
        .is_some_and(|fields| fields.len() == 1 && fields[0].trim() == r#""U""#)
}

fn parse_information_register_standard_attribute_string(value: &str) -> Option<String> {
    let fields = split_information_register_braced_fields(value)?;
    if fields.len() != 2 || fields.first()?.trim() != r#""S""# {
        return None;
    }
    parse_information_register_quoted_string(fields.get(1)?)
}

fn parse_information_register_standard_attribute_localized(
    value: &str,
) -> Option<Vec<(String, String)>> {
    let fields = split_information_register_braced_fields(value)?;
    if fields.len() != 3
        || fields.first()?.trim() != r##""#""##
        || !information_register_uuid_matches(
            fields.get(1)?,
            INFORMATION_REGISTER_STANDARD_ATTRIBUTE_LOCALIZED_TYPE_UUID,
        )
    {
        return None;
    }
    parse_information_register_owner_localized_value(fields.get(2)?)
}

fn information_register_standard_attribute_nested_values_are(
    value: &str,
    type_uuid: &str,
    expected: &[&str],
) -> bool {
    let Some(fields) = split_information_register_braced_fields(value) else {
        return false;
    };
    if fields.len() != 3
        || fields.first().map(|field| field.trim()) != Some(r##""#""##)
        || !fields
            .get(1)
            .is_some_and(|field| information_register_uuid_matches(field, type_uuid))
    {
        return false;
    }
    split_information_register_braced_fields(fields[2]).is_some_and(|nested| {
        nested.len() == expected.len()
            && nested
                .iter()
                .zip(expected)
                .all(|(actual, expected)| actual.trim() == *expected)
    })
}

fn information_register_standard_attribute_choice_form_is_valid(value: &str) -> bool {
    let Some(fields) = split_information_register_braced_fields(value) else {
        return false;
    };
    if fields.len() != 3
        || fields.first().map(|field| field.trim()) != Some(r##""#""##)
        || !fields.get(1).is_some_and(|field| {
            information_register_uuid_matches(field, METADATA_OBJECT_REF_TYPE_UUID)
        })
    {
        return false;
    }
    split_information_register_braced_fields(fields[2]).is_some_and(|nested| {
        nested.len() == 2
            && nested[0].trim() == "1"
            && parse_information_register_uuid(nested[1])
                .is_some_and(|uuid| information_register_uuid_is_zero(&uuid))
    })
}

fn parse_information_register_standard_attribute_fill_value(
    value: &str,
    name: &str,
) -> Option<MetadataChildFillValue> {
    let fields = split_information_register_braced_fields(value)?;
    if fields.len() == 1 && fields.first()?.trim() == r#""U""# {
        return Some(MetadataChildFillValue::Nil);
    }
    match (name, fields.first()?.trim(), fields.len()) {
        ("Active", r#""B""#, 2) => {
            information_register_bool(fields.get(1)?).map(MetadataChildFillValue::Boolean)
        }
        ("LineNumber", r#""N""#, 2) if fields.get(1)?.trim() == "0" => {
            Some(MetadataChildFillValue::Decimal("0".to_string()))
        }
        ("Recorder", _, _) => None,
        ("Period", r#""D""#, 2) => {
            let raw = fields.get(1)?.trim();
            if raw.len() != 14 || !raw.bytes().all(|byte| byte.is_ascii_digit()) {
                return None;
            }
            format_1c_date_time(raw).map(MetadataChildFillValue::DateTime)
        }
        _ => None,
    }
}

fn parse_register_standard_attribute<'a>(
    name: &'static str,
    bag: &InformationRegisterStandardAttributeBag<'a>,
    fill_value: MetadataChildFillValue,
) -> Option<RegisterStandardAttribute> {
    let link_by_type =
        bag.get(INFORMATION_REGISTER_STANDARD_ATTRIBUTE_LINK_BY_TYPE_PROPERTY_UUID)?;
    let fill_checking =
        bag.get(INFORMATION_REGISTER_STANDARD_ATTRIBUTE_FILL_CHECKING_PROPERTY_UUID)?;
    let multi_line = bag.get(INFORMATION_REGISTER_STANDARD_ATTRIBUTE_MULTI_LINE_PROPERTY_UUID)?;
    let fill_from_filling_value =
        bag.get(INFORMATION_REGISTER_STANDARD_ATTRIBUTE_FILL_FROM_FILLING_VALUE_PROPERTY_UUID)?;
    let create_on_input =
        bag.get(INFORMATION_REGISTER_STANDARD_ATTRIBUTE_CREATE_ON_INPUT_PROPERTY_UUID)?;
    let type_reduction_mode = bag
        .has_type_reduction_mode
        .then(|| bag.get(INFORMATION_REGISTER_STANDARD_ATTRIBUTE_TYPE_REDUCTION_PROPERTY_UUID))
        .flatten();
    let max_value = bag.get(INFORMATION_REGISTER_STANDARD_ATTRIBUTE_MAX_VALUE_PROPERTY_UUID)?;
    let tooltip = bag.get(INFORMATION_REGISTER_STANDARD_ATTRIBUTE_TOOLTIP_PROPERTY_UUID)?;
    let extended_edit =
        bag.get(INFORMATION_REGISTER_STANDARD_ATTRIBUTE_EXTENDED_EDIT_PROPERTY_UUID)?;
    let format = bag.get(INFORMATION_REGISTER_STANDARD_ATTRIBUTE_FORMAT_PROPERTY_UUID)?;
    let choice_form = bag.get(INFORMATION_REGISTER_STANDARD_ATTRIBUTE_CHOICE_FORM_PROPERTY_UUID)?;
    let quick_choice =
        bag.get(INFORMATION_REGISTER_STANDARD_ATTRIBUTE_QUICK_CHOICE_PROPERTY_UUID)?;
    let choice_history =
        bag.get(INFORMATION_REGISTER_STANDARD_ATTRIBUTE_CHOICE_HISTORY_PROPERTY_UUID)?;
    let edit_format = bag.get(INFORMATION_REGISTER_STANDARD_ATTRIBUTE_EDIT_FORMAT_PROPERTY_UUID)?;
    let password_mode =
        bag.get(INFORMATION_REGISTER_STANDARD_ATTRIBUTE_PASSWORD_MODE_PROPERTY_UUID)?;
    let data_history =
        bag.get(INFORMATION_REGISTER_STANDARD_ATTRIBUTE_DATA_HISTORY_PROPERTY_UUID)?;
    let mark_negatives =
        bag.get(INFORMATION_REGISTER_STANDARD_ATTRIBUTE_MARK_NEGATIVES_PROPERTY_UUID)?;
    let min_value = bag.get(INFORMATION_REGISTER_STANDARD_ATTRIBUTE_MIN_VALUE_PROPERTY_UUID)?;
    let synonym = bag.get(INFORMATION_REGISTER_STANDARD_ATTRIBUTE_SYNONYM_PROPERTY_UUID)?;
    let comment = bag.get(INFORMATION_REGISTER_STANDARD_ATTRIBUTE_COMMENT_PROPERTY_UUID)?;
    let full_text_search =
        bag.get(INFORMATION_REGISTER_STANDARD_ATTRIBUTE_FULL_TEXT_SEARCH_PROPERTY_UUID)?;
    let choice_parameter_links =
        bag.get(INFORMATION_REGISTER_STANDARD_ATTRIBUTE_CHOICE_PARAMETER_LINKS_PROPERTY_UUID)?;
    bag.get(INFORMATION_REGISTER_STANDARD_ATTRIBUTE_FILL_VALUE_PROPERTY_UUID)?;
    let mask = bag.get(INFORMATION_REGISTER_STANDARD_ATTRIBUTE_MASK_PROPERTY_UUID)?;
    let choice_parameters =
        bag.get(INFORMATION_REGISTER_STANDARD_ATTRIBUTE_CHOICE_PARAMETERS_PROPERTY_UUID)?;

    if !information_register_standard_attribute_nested_values_are(
        link_by_type,
        INFORMATION_REGISTER_STANDARD_ATTRIBUTE_LINK_BY_TYPE_UUID,
        &["3", "0", "0"],
    ) || parse_information_register_standard_attribute_bool(multi_line)?
        || parse_information_register_standard_attribute_nested_enum(
            create_on_input,
            INFORMATION_REGISTER_STANDARD_ATTRIBUTE_CREATE_ON_INPUT_UUID,
        )? != "0"
        || (bag.has_type_reduction_mode
            && parse_information_register_standard_attribute_nested_enum(
                type_reduction_mode?,
                INFORMATION_REGISTER_STANDARD_ATTRIBUTE_TYPE_REDUCTION_UUID,
            )? != "0")
        || !information_register_standard_attribute_nil_is_valid(max_value)
        || parse_information_register_standard_attribute_bool(extended_edit)?
        || !information_register_standard_attribute_choice_form_is_valid(choice_form)
        || parse_information_register_standard_attribute_nested_enum(
            quick_choice,
            INFORMATION_REGISTER_STANDARD_ATTRIBUTE_QUICK_CHOICE_UUID,
        )? != "2"
        || parse_information_register_standard_attribute_direct_enum(
            choice_history,
            INFORMATION_REGISTER_STANDARD_ATTRIBUTE_CHOICE_HISTORY_UUID,
        )? != "0"
        || parse_information_register_standard_attribute_bool(password_mode)?
        || parse_information_register_standard_attribute_bool(mark_negatives)?
        || !information_register_standard_attribute_nil_is_valid(min_value)
        || !parse_information_register_standard_attribute_string(comment)?.is_empty()
        || !information_register_standard_attribute_nested_values_are(
            choice_parameter_links,
            INFORMATION_REGISTER_STANDARD_ATTRIBUTE_CHOICE_PARAMETER_LINKS_UUID,
            &["5006", "0"],
        )
        || !parse_information_register_standard_attribute_string(mask)?.is_empty()
        || !information_register_standard_attribute_nested_values_are(
            choice_parameters,
            INFORMATION_REGISTER_STANDARD_ATTRIBUTE_CHOICE_PARAMETERS_UUID,
            &["0", "0"],
        )
    {
        return None;
    }

    let fill_checking = match parse_information_register_standard_attribute_direct_enum(
        fill_checking,
        INFORMATION_REGISTER_STANDARD_ATTRIBUTE_FILL_CHECKING_UUID,
    )? {
        "0" => "DontCheck",
        "1" => "ShowError",
        _ => return None,
    };
    let data_history = match parse_information_register_standard_attribute_nested_enum(
        data_history,
        INFORMATION_REGISTER_STANDARD_ATTRIBUTE_DATA_HISTORY_UUID,
    )? {
        "0" => "DontUse",
        "1" => "Use",
        _ => return None,
    };
    let full_text_search = match parse_information_register_standard_attribute_nested_enum(
        full_text_search,
        INFORMATION_REGISTER_STANDARD_ATTRIBUTE_FULL_TEXT_SEARCH_UUID,
    )? {
        "0" => "DontUse",
        "1" => "Use",
        _ => return None,
    };
    Some(RegisterStandardAttribute {
        name,
        fill_checking,
        fill_from_filling_value: parse_information_register_standard_attribute_bool(
            fill_from_filling_value,
        )?,
        tooltip: parse_information_register_standard_attribute_localized(tooltip)?,
        format: parse_information_register_standard_attribute_localized(format)?,
        edit_format: parse_information_register_standard_attribute_localized(edit_format)?,
        synonym: parse_information_register_standard_attribute_localized(synonym)?,
        data_history,
        full_text_search,
        fill_value,
        link_by_type: None,
    })
}

fn parse_information_register_standard_attribute<'a>(
    name: &'static str,
    bag: &InformationRegisterStandardAttributeBag<'a>,
) -> Option<RegisterStandardAttribute> {
    let fill_value = bag.get(INFORMATION_REGISTER_STANDARD_ATTRIBUTE_FILL_VALUE_PROPERTY_UUID)?;
    parse_register_standard_attribute(
        name,
        bag,
        parse_information_register_standard_attribute_fill_value(fill_value, name)?,
    )
}

fn parse_exchange_plan_standard_attribute_fill_value(
    value: &str,
    marker: &str,
) -> Option<MetadataChildFillValue> {
    let fields = split_information_register_braced_fields(value)?;
    if fields.len() == 1 && fields.first()?.trim() == r#""U""# {
        return Some(MetadataChildFillValue::Nil);
    }
    match (marker, fields.first()?.trim(), fields.len()) {
        ("-3", r#""S""#, 2) => {
            let value = parse_information_register_quoted_string(fields.get(1)?)?;
            value
                .is_empty()
                .then_some(MetadataChildFillValue::String(value))
        }
        ("-2", r#""S""#, 2) => {
            let value = parse_information_register_quoted_string(fields.get(1)?)?;
            (value == "   ").then_some(MetadataChildFillValue::String(value))
        }
        _ => None,
    }
}

fn parse_exchange_plan_standard_attribute<'a>(
    marker: &str,
    name: &'static str,
    bag: &InformationRegisterStandardAttributeBag<'a>,
) -> Option<RegisterStandardAttribute> {
    let fill_value = bag.get(INFORMATION_REGISTER_STANDARD_ATTRIBUTE_FILL_VALUE_PROPERTY_UUID)?;
    let attribute = parse_register_standard_attribute(
        name,
        bag,
        parse_exchange_plan_standard_attribute_fill_value(fill_value, marker)?,
    )?;
    if attribute.fill_from_filling_value
        || attribute.data_history != "Use"
        || !attribute.format.is_empty()
        || !attribute.edit_format.is_empty()
        || (matches!(marker, "-14" | "-13" | "-10" | "-9" | "-6" | "-4")
            && (attribute.fill_checking != "DontCheck"
                || !attribute.tooltip.is_empty()
                || !attribute.synonym.is_empty()))
        || (marker == "-3" && attribute.full_text_search != "Use")
    {
        return None;
    }
    Some(attribute)
}

fn parse_information_register_standard_attributes(
    value: &str,
) -> Option<Vec<RegisterStandardAttribute>> {
    let outer = split_information_register_braced_fields(value)?;
    if outer.len() == 1 && outer.first()?.trim() == "0" {
        return Some(Vec::new());
    }
    if outer.len() != 2 || outer.first()?.trim() != "1" {
        return None;
    }
    let payload = split_information_register_braced_fields(outer.get(1)?)?;
    if payload.len() != 14 || payload.first()?.trim() != "1" || payload.get(1)?.trim() != "4" {
        return None;
    }
    let definitions = [
        ("-5", "Active"),
        ("-4", "LineNumber"),
        ("-3", "Recorder"),
        ("-2", "Period"),
    ];
    let mut attributes = Vec::with_capacity(definitions.len());
    let mut bag_shape = None;
    for ((marker, name), fields) in definitions.into_iter().zip(payload[2..].chunks_exact(3)) {
        let marker_fields = split_information_register_braced_fields(fields[0])?;
        if marker_fields.len() != 1
            || marker_fields.first()?.trim() != marker
            || !information_register_uuid_matches(
                fields[1],
                INFORMATION_REGISTER_STANDARD_ATTRIBUTE_SECTION_UUID,
            )
        {
            return None;
        }
        let bag = parse_information_register_standard_attribute_bag(fields[2])?;
        if bag_shape.is_some_and(|shape| shape != bag.has_type_reduction_mode) {
            return None;
        }
        bag_shape = Some(bag.has_type_reduction_mode);
        attributes.push(parse_information_register_standard_attribute(name, &bag)?);
    }
    Some(attributes)
}

fn parse_exchange_plan_standard_attributes(value: &str) -> Option<Vec<RegisterStandardAttribute>> {
    let outer = split_information_register_braced_fields(value)?;
    if outer.len() == 1 && outer.first()?.trim() == "0" {
        return Some(Vec::new());
    }
    if outer.len() != 2 || outer.first()?.trim() != "1" {
        return None;
    }
    let payload = split_information_register_braced_fields(outer.get(1)?)?;
    if payload.len() != 26 || payload.first()?.trim() != "1" || payload.get(1)?.trim() != "8" {
        return None;
    }
    let definitions = [
        ("-14", "ExchangeDate"),
        ("-13", "ThisNode"),
        ("-10", "ReceivedNo"),
        ("-9", "SentNo"),
        ("-6", "Ref"),
        ("-4", "DeletionMark"),
        ("-3", "Description"),
        ("-2", "Code"),
    ];
    let mut attributes = Vec::with_capacity(definitions.len());
    let mut bag_shape = None;
    for ((marker, name), fields) in definitions.into_iter().zip(payload[2..].chunks_exact(3)) {
        let marker_fields = split_information_register_braced_fields(fields[0])?;
        if marker_fields.len() != 1
            || marker_fields.first()?.trim() != marker
            || !information_register_uuid_matches(
                fields[1],
                INFORMATION_REGISTER_STANDARD_ATTRIBUTE_SECTION_UUID,
            )
        {
            return None;
        }
        let bag = parse_information_register_standard_attribute_bag(fields[2])?;
        if bag_shape.is_some_and(|shape| shape != bag.has_type_reduction_mode) {
            return None;
        }
        bag_shape = Some(bag.has_type_reduction_mode);
        attributes.push(parse_exchange_plan_standard_attribute(marker, name, &bag)?);
    }
    Some(attributes)
}

fn parse_information_register_owner_properties(
    fields: &InformationRegisterOwnerFields<'_>,
    header: &MetadataHeader,
    form_refs: &BTreeMap<String, FormSourceReference>,
) -> Option<InformationRegisterOwnerProperties> {
    Some(InformationRegisterOwnerProperties {
        default_record_form: parse_information_register_owned_form_ref(
            fields.get(2)?,
            &header.name,
            form_refs,
        )?,
        default_list_form: parse_information_register_owned_form_ref(
            fields.get(3)?,
            &header.name,
            form_refs,
        )?,
        periodicity: information_register_periodicity_xml(fields.get(4)?)?,
        write_mode: information_register_write_mode_xml(fields.get(5)?)?,
        edit_type: information_register_edit_type_xml(fields.get(6)?)?,
        use_standard_commands: information_register_bool(fields.get(7)?)?,
        include_help_in_contents: information_register_bool(fields.get(8)?)?,
        main_filter_on_period: information_register_bool(fields.get(9)?)?,
        data_lock_control_mode: information_register_data_lock_control_mode_xml(fields.get(10)?)?,
        full_text_search: information_register_full_text_search(fields.get(11)?)?,
        standard_attributes: parse_information_register_standard_attributes(fields.get(12)?)?,
        auxiliary_record_form: parse_information_register_owned_form_ref(
            fields.get(13)?,
            &header.name,
            form_refs,
        )?,
        auxiliary_list_form: parse_information_register_owned_form_ref(
            fields.get(14)?,
            &header.name,
            form_refs,
        )?,
        record_presentation: parse_information_register_owner_localized_value(fields.get(15)?)?,
        extended_record_presentation: parse_information_register_owner_localized_value(
            fields.get(16)?,
        )?,
        list_presentation: parse_information_register_owner_localized_value(fields.get(17)?)?,
        extended_list_presentation: parse_information_register_owner_localized_value(
            fields.get(18)?,
        )?,
        explanation: parse_information_register_owner_localized_value(fields.get(19)?)?,
        enable_totals_slice_last: information_register_bool(fields.get(20)?)?,
        enable_totals_slice_first: information_register_bool(fields.get(21)?)?,
        data_history: metadata_data_history_xml(fields.get(22)?.trim())?,
        update_data_history_immediately_after_write: information_register_bool(fields.get(23)?)?,
        execute_after_write_data_history_version_processing: information_register_bool(
            fields.get(24)?,
        )?,
    })
}

fn parse_register_properties_from_text(
    kind: &str,
    text: &str,
    uuid: &str,
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
    form_refs: &BTreeMap<String, FormSourceReference>,
    source_version: InfobaseConfigSourceVersion,
) -> Option<RegisterProperties> {
    if !metadata_kind_uses_register_resources(kind) {
        return None;
    }
    let header = parse_metadata_header_from_text(text, uuid)?;
    let fields = metadata_object_fields(text)?;
    let mut generated_types = Vec::new();
    let register_start_index = register_generated_type_start_index(kind, &fields, uuid);
    if let Some(start_index) = register_start_index {
        if kind == "AccountingRegister" {
            push_accounting_register_generated_type_entries(
                &mut generated_types,
                &fields,
                start_index,
                &header.name,
            );
        } else {
            push_register_generated_type_entries(
                &mut generated_types,
                &fields,
                start_index,
                kind,
                &header.name,
            );
        }
    }
    if kind == "InformationRegister" {
        push_generated_type_entry(
            &mut generated_types,
            &fields,
            1,
            2,
            &format!("InformationRegisterRecord.{}", header.name),
            "Record",
        );
        push_generated_type_entry(
            &mut generated_types,
            &fields,
            3,
            4,
            &format!("InformationRegisterManager.{}", header.name),
            "Manager",
        );
        push_generated_type_entry(
            &mut generated_types,
            &fields,
            5,
            6,
            &format!("InformationRegisterSelection.{}", header.name),
            "Selection",
        );
        push_generated_type_entry(
            &mut generated_types,
            &fields,
            7,
            8,
            &format!("InformationRegisterList.{}", header.name),
            "List",
        );
        push_generated_type_entry(
            &mut generated_types,
            &fields,
            9,
            10,
            &format!("InformationRegisterRecordSet.{}", header.name),
            "RecordSet",
        );
        push_generated_type_entry(
            &mut generated_types,
            &fields,
            11,
            12,
            &format!("InformationRegisterRecordKey.{}", header.name),
            "RecordKey",
        );
        push_generated_type_entry(
            &mut generated_types,
            &fields,
            13,
            14,
            &format!("InformationRegisterRecordManager.{}", header.name),
            "RecordManager",
        );
    }
    let information_register = if kind == "InformationRegister" {
        parse_information_register_owner_fields(text, &header).and_then(|fields| {
            parse_information_register_owner_properties(&fields, &header, form_refs)
        })
    } else {
        None
    };
    let use_standard_commands = if kind == "InformationRegister" {
        information_register
            .as_ref()
            .map(|properties| properties.use_standard_commands)
            .unwrap_or(true)
    } else {
        parse_register_use_standard_commands(&fields, uuid)
    };
    let register_type = parse_register_type(kind, &fields, uuid);
    let include_help_in_contents = if kind == "InformationRegister" {
        information_register
            .as_ref()
            .map(|properties| properties.include_help_in_contents)
    } else {
        parse_register_include_help_in_contents(kind, &fields, uuid)
    };
    let chart_of_accounts = parse_register_chart_of_accounts(kind, &fields, uuid, object_refs);
    let correspondence = parse_register_correspondence(kind, &fields, uuid);
    let period_adjustment_length = parse_register_period_adjustment_length(kind, &fields, uuid);
    let data_lock_control_mode = if kind == "InformationRegister" {
        information_register
            .as_ref()
            .map(|properties| properties.data_lock_control_mode)
    } else {
        parse_register_data_lock_control_mode(kind, &fields, uuid)
    };
    let enable_totals_splitting = parse_register_enable_totals_splitting(kind, &fields, uuid);
    let full_text_search = if kind == "InformationRegister" {
        information_register
            .as_ref()
            .map(|properties| properties.full_text_search)
    } else {
        parse_register_full_text_search(kind, &fields, uuid)
    };
    let register_form_refs = if kind == "InformationRegister" {
        (None, None)
    } else {
        parse_register_form_refs(kind, &fields, uuid, &header.name, form_refs)
    };
    let standard_attributes = if kind == "InformationRegister" {
        Vec::new()
    } else {
        register_standard_attributes(kind, &header.name, register_type, &fields, uuid)
    };
    let (list_presentation, extended_list_presentation, explanation) =
        if kind == "InformationRegister" {
            (Vec::new(), Vec::new(), Vec::new())
        } else {
            parse_register_presentations(kind, &fields, uuid)
        };
    let owner_name = header.name.clone();
    let mut child_objects = nested_headers_with_offsets_from_text(text, uuid, |_| true)
        .into_iter()
        .filter_map(|(header, marker_start)| {
            let tag = register_child_object_tag(kind, text, marker_start)?;
            let (value_types, properties, emit_empty_type) = if kind == "InformationRegister" {
                match parse_information_register_child_payload(
                    text,
                    marker_start,
                    &header,
                    &owner_name,
                    tag,
                    type_index,
                    object_refs,
                    form_refs,
                    source_version == InfobaseConfigSourceVersion::V2_21,
                ) {
                    Some((value_types, properties)) => {
                        let emit_empty_type = tag == "Attribute" && value_types.is_empty();
                        (value_types, Some(properties), emit_empty_type)
                    }
                    None => (Vec::new(), None, false),
                }
            } else {
                let value_types =
                    parse_metadata_child_value_types(text, marker_start, &header.uuid, type_index);
                let properties = parse_metadata_child_properties(
                    kind,
                    text,
                    marker_start,
                    &header.uuid,
                    &value_types,
                    object_refs,
                );
                let properties = properties.map(|properties| {
                    parse_register_child_extra_properties(
                        kind,
                        tag,
                        text,
                        marker_start,
                        &header.uuid,
                        object_refs,
                        properties,
                    )
                });
                (value_types, properties, tag == "Attribute")
            };
            Some(MetadataChildObject {
                tag,
                header,
                generated_types: Vec::new(),
                value_types,
                emit_empty_type,
                properties,
                tabular_section_properties: None,
                child_objects: Vec::new(),
            })
        })
        .collect::<Vec<_>>();
    if kind == "InformationRegister" {
        child_objects.sort_by_key(|child| match child.tag {
            "Resource" => 0,
            "Attribute" => 1,
            "Dimension" => 2,
            _ => 3,
        });
    }
    Some(RegisterProperties {
        generated_types,
        use_standard_commands,
        information_register,
        register_type,
        include_help_in_contents,
        chart_of_accounts,
        correspondence,
        period_adjustment_length,
        data_lock_control_mode,
        enable_totals_splitting,
        full_text_search,
        default_list_form: register_form_refs.0,
        auxiliary_list_form: register_form_refs.1,
        list_presentation,
        extended_list_presentation,
        explanation,
        standard_attributes,
        child_objects,
    })
}

fn parse_register_form_refs(
    kind: &str,
    fields: &[&str],
    uuid: &str,
    owner_name: &str,
    form_refs: &BTreeMap<String, FormSourceReference>,
) -> (Option<String>, Option<String>) {
    let Some(header_index) = metadata_header_field_index(fields, uuid) else {
        return (None, None);
    };
    let owner_folder = metadata_source_folder_for_kind(kind);
    let default_list_form_ref = |offset| {
        owner_folder.and_then(|owner_folder| {
            parse_default_list_form_ref(
                fields,
                &[header_index + offset],
                form_refs,
                owner_folder,
                owner_name,
            )
        })
    };
    match kind {
        "AccountingRegister" => (
            default_list_form_ref(4),
            parse_catalog_form_ref(fields.get(header_index + 10).copied(), form_refs),
        ),
        "AccumulationRegister" => (
            default_list_form_ref(2),
            parse_catalog_form_ref(fields.get(header_index + 3).copied(), form_refs),
        ),
        _ => (None, None),
    }
}

fn parse_register_type(kind: &str, fields: &[&str], uuid: &str) -> Option<&'static str> {
    if kind != "AccumulationRegister" {
        return None;
    }
    let header_index = metadata_header_field_index(fields, uuid)?;
    let value = parse_1c_u32_field(fields.get(header_index + 4).copied())?;
    accumulation_register_type_xml(value)
}

fn accumulation_register_type_xml(value: u32) -> Option<&'static str> {
    match value {
        0 => Some("Balance"),
        1 => Some("Turnovers"),
        _ => None,
    }
}

fn register_standard_attributes(
    kind: &str,
    owner_name: &str,
    register_type: Option<&'static str>,
    fields: &[&str],
    uuid: &str,
) -> Vec<RegisterStandardAttribute> {
    let mut attributes = Vec::new();
    let accounting_attributes = parse_accounting_register_standard_attributes(kind, fields, uuid);
    let accounting_overrides = &accounting_attributes.overrides;
    if kind == "AccumulationRegister" && register_type == Some("Balance") {
        attributes.push(register_standard_attribute(
            "RecordType",
            "DontCheck",
            &BTreeMap::new(),
            None,
        ));
    }
    if kind == "AccountingRegister" {
        let account_data_path =
            format!("AccountingRegister.{owner_name}.StandardAttribute.Account");
        attributes.push(register_standard_attribute(
            "Account",
            "DontCheck",
            accounting_overrides,
            None,
        ));
        if accounting_attributes.present.contains("RecordType") {
            attributes.push(register_standard_attribute(
                "RecordType",
                "DontCheck",
                accounting_overrides,
                None,
            ));
        }
        attributes.extend([
            register_standard_attribute("Active", "DontCheck", accounting_overrides, None),
            register_standard_attribute("LineNumber", "DontCheck", accounting_overrides, None),
            register_standard_attribute("Recorder", "DontCheck", accounting_overrides, None),
            register_standard_attribute("Period", "ShowError", accounting_overrides, None),
            register_standard_attribute(
                "ExtDimension1",
                "DontCheck",
                accounting_overrides,
                Some(RegisterStandardAttributeLinkByType {
                    data_path: account_data_path.clone(),
                    link_item: 1,
                }),
            ),
            register_standard_attribute(
                "ExtDimensionType1",
                "DontCheck",
                accounting_overrides,
                None,
            ),
            register_standard_attribute(
                "ExtDimension2",
                "DontCheck",
                accounting_overrides,
                Some(RegisterStandardAttributeLinkByType {
                    data_path: account_data_path.clone(),
                    link_item: 2,
                }),
            ),
            register_standard_attribute(
                "ExtDimensionType2",
                "DontCheck",
                accounting_overrides,
                None,
            ),
            register_standard_attribute(
                "ExtDimension3",
                "DontCheck",
                accounting_overrides,
                Some(RegisterStandardAttributeLinkByType {
                    data_path: account_data_path,
                    link_item: 3,
                }),
            ),
            register_standard_attribute(
                "ExtDimensionType3",
                "DontCheck",
                accounting_overrides,
                None,
            ),
        ]);
        return attributes;
    }
    if kind == "AccumulationRegister" {
        attributes.extend([
            register_standard_attribute("Active", "DontCheck", &BTreeMap::new(), None),
            register_standard_attribute("LineNumber", "DontCheck", &BTreeMap::new(), None),
            register_standard_attribute("Recorder", "DontCheck", &BTreeMap::new(), None),
            register_standard_attribute("Period", "ShowError", &BTreeMap::new(), None),
        ]);
    }
    attributes
}

#[derive(Default)]
struct RegisterStandardAttributeOverrides {
    tooltip: Vec<(String, String)>,
    synonym: Vec<(String, String)>,
}

#[derive(Default)]
struct AccountingRegisterStandardAttributes {
    present: BTreeSet<&'static str>,
    overrides: BTreeMap<&'static str, RegisterStandardAttributeOverrides>,
}

fn register_standard_attribute(
    name: &'static str,
    fill_checking: &'static str,
    overrides: &BTreeMap<&'static str, RegisterStandardAttributeOverrides>,
    link_by_type: Option<RegisterStandardAttributeLinkByType>,
) -> RegisterStandardAttribute {
    let override_values = overrides.get(name);
    RegisterStandardAttribute {
        name,
        fill_checking,
        fill_from_filling_value: false,
        tooltip: override_values
            .map(|values| values.tooltip.clone())
            .unwrap_or_default(),
        format: Vec::new(),
        edit_format: Vec::new(),
        synonym: override_values
            .map(|values| values.synonym.clone())
            .unwrap_or_default(),
        data_history: "Use",
        full_text_search: "Use",
        fill_value: MetadataChildFillValue::Nil,
        link_by_type,
    }
}

fn parse_accounting_register_standard_attributes(
    kind: &str,
    fields: &[&str],
    uuid: &str,
) -> AccountingRegisterStandardAttributes {
    if kind != "AccountingRegister" {
        return AccountingRegisterStandardAttributes::default();
    }
    let Some(header_index) = metadata_header_field_index(fields, uuid) else {
        return AccountingRegisterStandardAttributes::default();
    };
    fields
        .get(header_index + 9)
        .and_then(|field| parse_accounting_register_standard_attribute_collection(field))
        .unwrap_or_default()
}

fn parse_accounting_register_standard_attribute_collection(
    value: &str,
) -> Option<AccountingRegisterStandardAttributes> {
    let outer = split_information_register_braced_fields(value)?;
    if outer.len() != 2 || outer.first()?.trim() != "1" {
        return None;
    }
    let items = split_information_register_braced_fields(outer.get(1)?)?;
    if items.len() < 2 || items.first()?.trim() != "1" {
        return None;
    }
    let count = parse_information_register_usize(items.get(1)?)?;
    if items.len() != count.checked_mul(3)?.checked_add(2)? {
        return None;
    }

    let mut validated = Vec::with_capacity(count);
    let mut expected_type_reduction_mode = None;
    for triplet in items[2..].chunks_exact(3) {
        let marker = triplet.first()?.trim();
        let marker_fields = split_information_register_braced_fields(marker)?;
        let name = accounting_register_standard_attribute_name(&marker_fields);
        let marker_is_valid = match marker_fields.as_slice() {
            [marker] => marker.trim().strip_prefix('-').is_some_and(|value| {
                !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
            }),
            [_, _] => name.is_some(),
            _ => false,
        };
        if !marker_is_valid
            || !information_register_uuid_matches(
                triplet.get(1)?,
                INFORMATION_REGISTER_STANDARD_ATTRIBUTE_SECTION_UUID,
            )
        {
            return None;
        }
        let bag = parse_information_register_standard_attribute_bag(triplet.get(2)?)?;
        match expected_type_reduction_mode {
            Some(expected) if expected != bag.has_type_reduction_mode => return None,
            None => expected_type_reduction_mode = Some(bag.has_type_reduction_mode),
            _ => {}
        }
        let (tooltip, synonym) = if name.is_some() {
            (
                parse_information_register_standard_attribute_localized(
                    bag.get(INFORMATION_REGISTER_STANDARD_ATTRIBUTE_TOOLTIP_PROPERTY_UUID)?,
                )?,
                parse_information_register_standard_attribute_localized(
                    bag.get(INFORMATION_REGISTER_STANDARD_ATTRIBUTE_SYNONYM_PROPERTY_UUID)?,
                )?,
            )
        } else {
            (Vec::new(), Vec::new())
        };
        validated.push((name, tooltip, synonym));
    }

    let mut attributes = AccountingRegisterStandardAttributes::default();
    for (name, tooltip, synonym) in validated {
        let Some(name) = name else {
            continue;
        };
        attributes.present.insert(name);
        if !tooltip.is_empty() || !synonym.is_empty() {
            attributes.overrides.insert(
                name,
                RegisterStandardAttributeOverrides { tooltip, synonym },
            );
        }
    }
    Some(attributes)
}

fn accounting_register_standard_attribute_name(marker_fields: &[&str]) -> Option<&'static str> {
    match marker_fields {
        ["-10"] => Some("Account"),
        ["-9"] => Some("RecordType"),
        ["-5"] => Some("Active"),
        ["-4"] => Some("LineNumber"),
        ["-3"] => Some("Recorder"),
        ["-2"] => Some("Period"),
        [marker, family] if matches!(marker.trim(), "0" | "1" | "2") => {
            let item = marker.trim().parse::<u32>().ok()? + 1;
            match family.trim() {
                "91162600-3161-4326-89a0-4a7cecd5092a" => match item {
                    1 => Some("ExtDimension1"),
                    2 => Some("ExtDimension2"),
                    3 => Some("ExtDimension3"),
                    _ => None,
                },
                "b3b48b29-d652-47ab-9d21-7e06768c31b5" => match item {
                    1 => Some("ExtDimensionType1"),
                    2 => Some("ExtDimensionType2"),
                    3 => Some("ExtDimensionType3"),
                    _ => None,
                },
                _ => None,
            }
        }
        _ => None,
    }
}

fn parse_register_chart_of_accounts(
    kind: &str,
    fields: &[&str],
    uuid: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    if kind != "AccountingRegister" {
        return None;
    }
    let header_index = metadata_header_field_index(fields, uuid)?;
    parse_metadata_object_ref(fields.get(header_index + 3).copied(), object_refs)
}

fn parse_register_correspondence(kind: &str, fields: &[&str], uuid: &str) -> Option<bool> {
    if kind != "AccountingRegister" {
        return None;
    }
    let header_index = metadata_header_field_index(fields, uuid)?;
    parse_1c_bool_field(fields.get(header_index + 5).copied())
}

fn parse_register_period_adjustment_length(kind: &str, fields: &[&str], uuid: &str) -> Option<u32> {
    if kind != "AccountingRegister" {
        return None;
    }
    let header_index = metadata_header_field_index(fields, uuid)?;
    parse_1c_u32_field(fields.get(header_index + 6).copied())
}

fn parse_register_data_lock_control_mode(
    kind: &str,
    fields: &[&str],
    uuid: &str,
) -> Option<&'static str> {
    let header_index = metadata_header_field_index(fields, uuid)?;
    let field_offset = match kind {
        "AccountingRegister" => 7,
        "AccumulationRegister" => 5,
        _ => return None,
    };
    match fields
        .get(header_index + field_offset)
        .map(|field| field.trim())
    {
        Some("0") => Some("Automatic"),
        Some("1") => Some("Managed"),
        _ => None,
    }
}

fn parse_register_enable_totals_splitting(kind: &str, fields: &[&str], uuid: &str) -> Option<bool> {
    if kind != "AccountingRegister" {
        return None;
    }
    let header_index = metadata_header_field_index(fields, uuid)?;
    parse_1c_bool_field(fields.get(header_index + 8).copied())
}

fn parse_register_full_text_search(
    kind: &str,
    fields: &[&str],
    uuid: &str,
) -> Option<&'static str> {
    let header_index = metadata_header_field_index(fields, uuid)?;
    let field_offset = match kind {
        "AccountingRegister" => 14,
        "AccumulationRegister" => 6,
        _ => return None,
    };
    match fields
        .get(header_index + field_offset)
        .map(|field| field.trim())
    {
        Some("0") => Some("DontUse"),
        Some("1") => Some("Use"),
        _ => None,
    }
}

fn parse_register_use_standard_commands(fields: &[&str], uuid: &str) -> bool {
    let Some(header_index) = metadata_header_field_index(fields, uuid) else {
        return true;
    };
    fields
        .get(header_index + 1)
        .and_then(|field| parse_1c_bool_field(Some(*field)))
        .unwrap_or(true)
}

fn parse_register_include_help_in_contents(
    kind: &str,
    fields: &[&str],
    uuid: &str,
) -> Option<bool> {
    match kind {
        "AccumulationRegister" => Some(false),
        "AccountingRegister" => {
            let header_index = metadata_header_field_index(fields, uuid)?;
            parse_1c_bool_field(fields.get(header_index + 2).copied())
        }
        _ => None,
    }
}

fn parse_register_presentations(
    kind: &str,
    fields: &[&str],
    uuid: &str,
) -> (
    Vec<(String, String)>,
    Vec<(String, String)>,
    Vec<(String, String)>,
) {
    if kind != "AccountingRegister" {
        return (Vec::new(), Vec::new(), Vec::new());
    }
    let Some(header_index) = metadata_header_field_index(fields, uuid) else {
        return (Vec::new(), Vec::new(), Vec::new());
    };
    (
        fields
            .get(header_index + 11)
            .map(|field| parse_1c_synonyms(field))
            .unwrap_or_default(),
        fields
            .get(header_index + 12)
            .map(|field| parse_1c_synonyms(field))
            .unwrap_or_default(),
        fields
            .get(header_index + 13)
            .map(|field| parse_1c_synonyms(field))
            .unwrap_or_default(),
    )
}

fn push_register_generated_type_entries(
    entries: &mut Vec<GeneratedTypeEntry>,
    fields: &[&str],
    start_index: usize,
    type_prefix: &str,
    name: &str,
) {
    for (offset, suffix) in register_generated_type_suffixes() {
        let xml_suffix = register_generated_type_xml_suffix(type_prefix, suffix);
        push_generated_type_entry(
            entries,
            fields,
            start_index + offset,
            start_index + offset + 1,
            &format!("{type_prefix}{xml_suffix}.{name}"),
            register_generated_type_category(type_prefix, suffix),
        );
    }
}

fn push_accounting_register_generated_type_entries(
    entries: &mut Vec<GeneratedTypeEntry>,
    fields: &[&str],
    start_index: usize,
    name: &str,
) {
    for (offset, suffix, category) in accounting_register_generated_type_slots() {
        push_generated_type_entry(
            entries,
            fields,
            start_index + offset,
            start_index + offset + 1,
            &format!("AccountingRegister{suffix}.{name}"),
            category,
        );
    }
}

fn register_generated_type_start_index(kind: &str, fields: &[&str], uuid: &str) -> Option<usize> {
    match kind {
        "AccountingRegister" if is_code21_accounting_register_fields(fields, uuid) => Some(1),
        "AccountingRegister" => Some(2),
        "AccumulationRegister" | "CalculationRegister" => Some(1),
        _ => None,
    }
}

fn register_generated_type_suffixes() -> &'static [(usize, &'static str)] {
    &[
        (0, "Object"),
        (2, "Manager"),
        (4, "Selection"),
        (6, "List"),
        (8, "RecordSet"),
        (10, "RecordKey"),
    ]
}

fn accounting_register_generated_type_slots() -> &'static [(usize, &'static str, &'static str)] {
    &[
        (0, "Record", "Record"),
        (2, "ExtDimensions", "ExtDimensions"),
        (4, "RecordSet", "RecordSet"),
        (6, "RecordKey", "RecordKey"),
        (8, "Selection", "Selection"),
        (10, "List", "List"),
        (12, "Manager", "Manager"),
    ]
}

fn register_generated_type_xml_suffix<'a>(type_prefix: &str, suffix: &'a str) -> &'a str {
    match (type_prefix, suffix) {
        ("AccumulationRegister", "Object") => "Record",
        _ => suffix,
    }
}

fn register_generated_type_category(type_prefix: &str, suffix: &str) -> &'static str {
    if type_prefix == "AccumulationRegister" && suffix == "Object" {
        return "Record";
    }
    match suffix {
        "Object" => "Object",
        "Manager" => "Manager",
        "Selection" => "Selection",
        "List" => "List",
        "RecordSet" => "RecordSet",
        "RecordKey" => "RecordKey",
        _ => unreachable!("unknown register generated type suffix"),
    }
}

fn register_child_object_tag(kind: &str, text: &str, marker_start: usize) -> Option<&'static str> {
    if !metadata_kind_uses_register_resources(kind) {
        return None;
    }
    if kind == "InformationRegister"
        && is_offset_inside_metadata_object_code(text, marker_start, 27)
    {
        if is_offset_inside_metadata_object_code(text, marker_start, 9) {
            return Some("Dimension");
        }
        if is_offset_inside_metadata_object_code(text, marker_start, 7) {
            return Some("Resource");
        }
    }
    if kind == "AccountingRegister"
        && is_offset_inside_register_dimension_list(text, marker_start)
        && is_offset_inside_metadata_object_code(text, marker_start, 6)
    {
        return Some("Dimension");
    }
    if kind == "AccountingRegister"
        && is_offset_inside_register_resource_list(text, marker_start)
        && is_offset_inside_metadata_object_code(text, marker_start, 2)
    {
        return Some("Resource");
    }
    if kind == "AccountingRegister"
        && is_offset_inside_accounting_register_attribute_list(text, marker_start)
        && is_offset_inside_metadata_object_code(text, marker_start, 2)
    {
        return Some("Attribute");
    }
    if is_offset_inside_metadata_object_code(text, marker_start, 5)
        && is_offset_inside_register_resource_list(text, marker_start)
    {
        return Some("Resource");
    }
    if is_offset_inside_metadata_object_code(text, marker_start, 4)
        && is_offset_inside_metadata_object_code(text, marker_start, 27)
    {
        return Some("Attribute");
    }
    if is_offset_inside_metadata_object_code(text, marker_start, 5) {
        return Some("Attribute");
    }
    if is_offset_inside_metadata_object_code(text, marker_start, 6) {
        return Some("Attribute");
    }
    if is_offset_inside_metadata_object_code(text, marker_start, 8)
        && is_offset_inside_register_dimension_list(text, marker_start)
    {
        return Some("Dimension");
    }
    if is_offset_inside_metadata_object_code(text, marker_start, 8) {
        return Some("Resource");
    }
    if is_offset_inside_metadata_object_code(text, marker_start, 10) {
        return Some("Dimension");
    }
    None
}

fn parse_attribute_tabular_section_child_objects(
    owner_kind: &str,
    owner_name: &str,
    text: &str,
    owner_uuid: &str,
    catalog_direct_wrapper_code: Option<u32>,
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
    metadata_object_refs: &BTreeMap<String, String>,
    form_refs: &BTreeMap<String, FormSourceReference>,
) -> Vec<MetadataChildObject> {
    let mut roots = Vec::<MetadataChildObject>::new();
    let mut tabular_section_indexes = BTreeMap::<String, usize>::new();
    let mut pending_by_tabular_section = BTreeMap::<String, Vec<MetadataChildObject>>::new();
    let document_data_path_owner_proof = (owner_kind == "Document")
        .then(|| build_document_data_path_owner_proof(owner_name, text, owner_uuid));

    for (header, marker_start) in nested_headers_with_offsets_from_text(text, owner_uuid, |_| true)
    {
        let Some((tag, parent_tabular_section)) = attribute_tabular_section_child_object_tag(
            owner_kind,
            owner_name,
            owner_uuid,
            text,
            marker_start,
            &header,
        ) else {
            continue;
        };
        let value_types = if tag == "Attribute" {
            parse_metadata_child_value_types(text, marker_start, &header.uuid, type_index)
        } else {
            Vec::new()
        };
        let properties = if tag == "Attribute" && owner_kind == "Catalog" {
            let expected_wrapper_code = if parent_tabular_section.is_some() {
                8
            } else {
                let Some(wrapper_code) = catalog_direct_wrapper_code else {
                    continue;
                };
                wrapper_code
            };
            parse_catalog_child_properties(
                owner_name,
                text,
                marker_start,
                &header.uuid,
                expected_wrapper_code,
                &value_types,
                type_index,
                object_refs,
                metadata_object_refs,
                form_refs,
            )
        } else if tag == "Attribute" && owner_kind == "Document" {
            document_data_path_owner_proof.as_ref().and_then(|proof| {
                parse_document_child_properties(
                    owner_name,
                    text,
                    marker_start,
                    &header.uuid,
                    parent_tabular_section.is_some(),
                    &value_types,
                    type_index,
                    object_refs,
                    metadata_object_refs,
                    form_refs,
                    proof,
                )
            })
        } else if tag == "Attribute" {
            parse_metadata_child_properties(
                owner_kind,
                text,
                marker_start,
                &header.uuid,
                &value_types,
                object_refs,
            )
        } else {
            None
        };
        let tabular_section_properties = if tag == "TabularSection" {
            parse_metadata_tabular_section_properties(owner_kind, text, marker_start, &header.uuid)
        } else {
            None
        };
        let generated_types = if owner_kind == "DataProcessor" && tag == "TabularSection" {
            parse_data_processor_tabular_section_generated_types(
                text,
                marker_start,
                &header,
                owner_name,
            )
        } else {
            Vec::new()
        };
        let child = MetadataChildObject {
            tag,
            generated_types,
            value_types,
            emit_empty_type: tag == "Attribute",
            properties,
            tabular_section_properties,
            header,
            child_objects: Vec::new(),
        };
        if tag == "TabularSection" {
            let tabular_section_uuid = child.header.uuid.clone();
            let root_index = roots.len();
            roots.push(child);
            tabular_section_indexes.insert(tabular_section_uuid.clone(), root_index);
            if let Some(pending) = pending_by_tabular_section.remove(&tabular_section_uuid) {
                roots[root_index].child_objects.extend(pending);
            }
            continue;
        }
        if let Some(tabular_section) = parent_tabular_section {
            if let Some(root_index) = tabular_section_indexes.get(&tabular_section.uuid).copied() {
                roots[root_index].child_objects.push(child);
            } else {
                pending_by_tabular_section
                    .entry(tabular_section.uuid)
                    .or_default()
                    .push(child);
            }
            continue;
        }
        roots.push(child);
    }

    if matches!(owner_kind, "Catalog" | "DataProcessor") {
        roots.sort_by_key(|child| match child.tag {
            "Attribute" => 0usize,
            "TabularSection" => 1usize,
            _ => 2usize,
        });
    }

    roots
}

fn parse_metadata_child_value_types(
    text: &str,
    marker_start: usize,
    child_uuid: &str,
    type_index: &BTreeMap<String, String>,
) -> Vec<ConstantValueType> {
    let Some((_, _, fields)) =
        innermost_metadata_object_fields_around_header(text, marker_start, child_uuid)
    else {
        return Vec::new();
    };

    fields
        .iter()
        .filter_map(|field| parse_metadata_type_pattern_from_child_field(field, type_index))
        .find(|value_types| !value_types.is_empty())
        .unwrap_or_default()
}

fn parse_data_processor_tabular_section_generated_types(
    text: &str,
    marker_start: usize,
    header: &MetadataHeader,
    owner_name: &str,
) -> Vec<GeneratedTypeEntry> {
    for fields in metadata_object_field_candidates_around_header(text, marker_start, &header.uuid) {
        if fields.first().map(|field| field.trim()) != Some("11") {
            continue;
        }
        let mut generated_types = Vec::new();
        push_generated_type_entry(
            &mut generated_types,
            &fields,
            1,
            2,
            &format!("DataProcessorTabularSection.{owner_name}.{}", header.name),
            "TabularSection",
        );
        push_generated_type_entry(
            &mut generated_types,
            &fields,
            3,
            4,
            &format!(
                "DataProcessorTabularSectionRow.{owner_name}.{}",
                header.name
            ),
            "TabularSectionRow",
        );
        if !generated_types.is_empty() {
            return generated_types;
        }
    }
    Vec::new()
}

fn parse_metadata_child_properties(
    owner_kind: &str,
    text: &str,
    marker_start: usize,
    child_uuid: &str,
    value_types: &[ConstantValueType],
    object_refs: &BTreeMap<String, String>,
) -> Option<MetadataChildProperties> {
    if owner_kind == "AccountingRegister" {
        for fields in metadata_object_field_candidates_around_header(text, marker_start, child_uuid)
        {
            if let Some(properties) =
                parse_accounting_register_child_properties_from_fields(&fields, child_uuid)
            {
                return Some(properties);
            }
        }
    }
    for fields in metadata_object_field_candidates_around_header(text, marker_start, child_uuid) {
        if let Some(mut properties) = parse_metadata_child_properties_from_fields(
            owner_kind,
            &fields,
            child_uuid,
            value_types,
            object_refs,
        ) {
            if owner_kind == "DataProcessor"
                && is_offset_inside_metadata_object_code(text, marker_start, 27)
                && is_offset_inside_data_processor_legacy_attribute_list(text, marker_start)
            {
                properties.emit_fill_from_filling_value = false;
                properties.emit_fill_value = false;
            }
            return Some(properties);
        }
    }
    None
}

fn parse_information_register_child_payload(
    text: &str,
    marker_start: usize,
    child_header: &MetadataHeader,
    owner_name: &str,
    tag: &str,
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
    form_refs: &BTreeMap<String, FormSourceReference>,
    preserve_raw_data_paths: bool,
) -> Option<(Vec<ConstantValueType>, MetadataChildProperties)> {
    let mut parsed =
        metadata_object_field_candidates_around_header(text, marker_start, &child_header.uuid)
            .into_iter()
            .filter_map(|fields| {
                parse_information_register_child_payload_from_fields(
                    &fields,
                    child_header,
                    owner_name,
                    tag,
                    type_index,
                    object_refs,
                    form_refs,
                    preserve_raw_data_paths,
                )
            });
    let value = parsed.next()?;
    if parsed.next().is_some() {
        return None;
    }
    Some(value)
}

fn parse_information_register_child_payload_from_fields(
    fields: &[&str],
    child_header: &MetadataHeader,
    owner_name: &str,
    tag: &str,
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
    form_refs: &BTreeMap<String, FormSourceReference>,
    preserve_raw_data_paths: bool,
) -> Option<(Vec<ConstantValueType>, MetadataChildProperties)> {
    let (expected_tag, common_field) = match (fields.first()?.trim(), fields.len()) {
        ("7", 5) | ("8", 7) => ("Resource", fields.get(1)?),
        ("4", 5) | ("5", 7) => ("Attribute", fields.get(1)?),
        ("9", 8) | ("10", 9) => ("Dimension", fields.get(1)?),
        _ => return None,
    };
    if expected_tag != tag {
        return None;
    }

    let common_fields = split_1c_braced_fields(common_field, 0)?;
    if common_fields.len() != 23 || common_fields.first()?.trim() != "27" {
        return None;
    }
    let typed_header = split_1c_braced_fields(common_fields.get(1)?, 0)?;
    if typed_header.len() != 3 || typed_header.first()?.trim() != "2" {
        return None;
    }
    let header_fields = split_1c_braced_fields(typed_header.get(1)?, 0)?;
    if header_fields.len() != 9
        || header_fields.first()?.trim() != "3"
        || metadata_header_field_index(&header_fields, &child_header.uuid) != Some(1)
    {
        return None;
    }
    let parsed_header = parse_metadata_header_from_text(typed_header.get(1)?, &child_header.uuid)?;
    if parsed_header.uuid != child_header.uuid
        || parsed_header.name != child_header.name
        || parsed_header.synonyms != child_header.synonyms
        || parsed_header.comment != child_header.comment
    {
        return None;
    }

    let value_types = stable_partition_metadata_types(parse_information_register_type_pattern(
        typed_header.get(2)?,
        type_index,
    )?);
    let mut properties = parse_information_register_common_child_properties(
        &common_fields,
        owner_name,
        type_index,
        object_refs,
        form_refs,
        preserve_raw_data_paths,
    )?;

    match (fields.first()?.trim(), fields.len()) {
        ("7", 5) | ("4", 5) => {
            properties.indexing = information_register_indexing(fields.get(2)?);
            properties.full_text_search = information_register_full_text_search(fields.get(3)?);
            properties.data_history = information_register_data_history(fields.get(4)?);
        }
        ("8", 7) | ("5", 7) => {
            if fields.get(5)?.trim() != "0"
                || !information_register_new_child_tail_is_valid(fields.get(6)?)
            {
                return None;
            }
            properties.indexing = information_register_indexing(fields.get(2)?);
            properties.full_text_search = information_register_full_text_search(fields.get(3)?);
            properties.data_history = information_register_data_history(fields.get(4)?);
        }
        ("9", 8) => {
            properties.master = Some(information_register_bool(fields.get(2)?)?);
            properties.deny_incomplete_values = Some(information_register_bool(fields.get(3)?)?);
            properties.indexing = information_register_indexing(fields.get(4)?);
            properties.main_filter = Some(information_register_bool(fields.get(5)?)?);
            properties.full_text_search = information_register_full_text_search(fields.get(6)?);
            properties.data_history = information_register_data_history(fields.get(7)?);
            properties.type_reduction_mode = Some("TransformValues");
        }
        ("10", 9) => {
            properties.master = Some(information_register_bool(fields.get(2)?)?);
            properties.deny_incomplete_values = Some(information_register_bool(fields.get(3)?)?);
            properties.indexing = information_register_indexing(fields.get(4)?);
            properties.main_filter = Some(information_register_bool(fields.get(5)?)?);
            properties.full_text_search = information_register_full_text_search(fields.get(6)?);
            properties.data_history = information_register_data_history(fields.get(7)?);
            properties.type_reduction_mode = Some(match fields.get(8)?.trim() {
                "0" => "TransformValues",
                "1" => "DeleteData",
                _ => return None,
            });
        }
        _ => return None,
    }

    if properties.indexing.is_none()
        || properties.full_text_search.is_none()
        || properties.data_history.is_none()
    {
        return None;
    }
    Some((value_types, properties))
}

fn parse_information_register_type_pattern(
    value: &str,
    type_index: &BTreeMap<String, String>,
) -> Option<Vec<ConstantValueType>> {
    let fields = split_1c_braced_fields(value, 0)?;
    if fields.first()?.trim() != r#""Pattern""# {
        return None;
    }
    fields
        .iter()
        .skip(1)
        .map(|field| parse_information_register_type_pattern_element(field, type_index))
        .collect()
}

fn parse_information_register_type_pattern_element(
    value: &str,
    type_index: &BTreeMap<String, String>,
) -> Option<ConstantValueType> {
    let fields = split_1c_braced_fields(value, 0)?;
    match (fields.first()?.trim(), fields.len()) {
        (r#""B""#, 1) => Some(ConstantValueType::Boolean),
        (r#""S""#, 1) => Some(ConstantValueType::String {
            length: None,
            allowed_length_flag: 0,
        }),
        (r#""S""#, 3) => {
            let allowed_length_flag = fields.get(2)?.trim().parse::<u8>().ok()?;
            if allowed_length_flag > 2 {
                return None;
            }
            Some(ConstantValueType::String {
                length: Some(fields.get(1)?.trim().parse().ok()?),
                allowed_length_flag,
            })
        }
        (r#""N""#, 1) => Some(ConstantValueType::Number {
            digits: 0,
            fraction_digits: 0,
            allowed_sign_flag: 0,
        }),
        (r#""N""#, 4) => {
            let allowed_sign_flag = fields.get(3)?.trim().parse::<u8>().ok()?;
            if allowed_sign_flag > 1 {
                return None;
            }
            Some(ConstantValueType::Number {
                digits: fields.get(1)?.trim().parse().ok()?,
                fraction_digits: fields.get(2)?.trim().parse().ok()?,
                allowed_sign_flag,
            })
        }
        (r#""D""#, 1) => Some(ConstantValueType::DateTime {
            date_fractions: "DateTime",
        }),
        (r#""D""#, 2) => Some(ConstantValueType::DateTime {
            date_fractions: match fields.get(1)?.trim() {
                r#""D""# => "Date",
                r#""T""# => "Time",
                _ => return None,
            },
        }),
        (r##""#""##, 2) => {
            let type_id = parse_uuid_field(fields.get(1)?.trim())?;
            let reference = type_index
                .get(&type_id)
                .cloned()
                .or_else(|| information_register_builtin_reference(&type_id).map(str::to_string))
                .or_else(|| builtin_type_reference(&type_id).map(str::to_string))?;
            if metadata_reference_is_type_set(&reference) {
                Some(ConstantValueType::ReferenceTypeSet { reference })
            } else {
                Some(ConstantValueType::Reference { reference })
            }
        }
        _ => None,
    }
}

fn information_register_builtin_reference(type_id: &str) -> Option<&'static str> {
    if type_id == "474c3bf6-08b5-4ddc-a2ad-989cedf11583" {
        return Some("cfg:EnumRef");
    }
    DCS_BUILTIN_REFERENCE_TYPE_SETS
        .iter()
        .find_map(|(candidate, reference)| (*candidate == type_id).then_some(*reference))
}

fn metadata_reference_is_type_set(reference: &str) -> bool {
    reference.starts_with("cfg:DefinedType.")
        || reference.starts_with("cfg:Characteristic.")
        || reference == "cfg:AnyIBRef"
        || (reference.starts_with("cfg:") && reference.ends_with("Ref") && !reference.contains('.'))
}

fn stable_partition_metadata_types(value_types: Vec<ConstantValueType>) -> Vec<ConstantValueType> {
    let (mut ordinary, type_sets): (Vec<_>, Vec<_>) = value_types
        .into_iter()
        .partition(|value_type| !matches!(value_type, ConstantValueType::ReferenceTypeSet { .. }));
    ordinary.extend(type_sets);
    ordinary
}

fn parse_information_register_common_child_properties(
    fields: &[&str],
    owner_name: &str,
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
    form_refs: &BTreeMap<String, FormSourceReference>,
    preserve_raw_data_paths: bool,
) -> Option<MetadataChildProperties> {
    if fields.len() != 23 || fields.first()?.trim() != "27" {
        return None;
    }
    let min_value = parse_information_register_bound(fields.get(8)?)?;
    let max_value = parse_information_register_bound(fields.get(9)?)?;
    let link_by_type = parse_information_register_link_by_type(
        fields.get(15)?,
        owner_name,
        object_refs,
        preserve_raw_data_paths,
    )?;
    Some(MetadataChildProperties {
        password_mode: information_register_bool(fields.get(2)?)?,
        format: parse_information_register_localized_value(fields.get(3)?)?,
        edit_format: parse_information_register_localized_value(fields.get(18)?)?,
        tooltip: parse_information_register_localized_value(fields.get(4)?)?,
        mark_negatives: information_register_bool(fields.get(5)?)?,
        mask: parse_1c_quoted_string(fields.get(6)?.trim())?,
        multi_line: information_register_bool(fields.get(7)?)?,
        extended_edit: information_register_bool(fields.get(17)?)?,
        min_value,
        max_value,
        fill_from_filling_value: information_register_bool(fields.get(20)?)?,
        emit_fill_from_filling_value: true,
        fill_value: Some(parse_information_register_fill_value(
            fields.get(19)?,
            type_index,
            object_refs,
        )?),
        emit_fill_value: true,
        fill_checking: match fields.get(13)?.trim() {
            "0" => "DontCheck",
            "1" => "ShowError",
            _ => return None,
        },
        choice_folders_and_items: Some(match fields.get(10)?.trim() {
            "0" => "Items",
            "1" => "Folders",
            "2" => "FoldersAndItems",
            _ => return None,
        }),
        choice_parameter_links: Some(parse_information_register_choice_parameter_links(
            fields.get(14)?,
            owner_name,
            object_refs,
            preserve_raw_data_paths,
        )?),
        choice_parameters: Some(parse_information_register_choice_parameters(
            fields.get(16)?,
            type_index,
            object_refs,
        )?),
        self_close_empty_choice_parameter_refs: false,
        quick_choice: Some(match fields.get(12)?.trim() {
            "0" => "DontUse",
            "1" => "Use",
            "2" => "Auto",
            _ => return None,
        }),
        create_on_input: Some(match fields.get(21)?.trim() {
            "0" => "Auto",
            "1" => "DontUse",
            "2" => "Use",
            _ => return None,
        }),
        choice_form: Some(parse_information_register_choice_form(
            fields.get(11)?,
            type_index,
            object_refs,
            form_refs,
        )?),
        link_by_type_empty: link_by_type.is_none(),
        link_by_type,
        choice_history_on_input: Some(match fields.get(22)?.trim() {
            "0" => "Auto",
            "1" => "DontUse",
            _ => return None,
        }),
        master: None,
        main_filter: None,
        balance: None,
        accounting_flag: None,
        ext_dimension_accounting_flag: None,
        deny_incomplete_values: None,
        use_mode: None,
        indexing: None,
        full_text_search: None,
        data_history: None,
        type_reduction_mode: None,
        update_data_history_immediately_after_write: None,
        execute_after_write_data_history_version_processing: None,
    })
}

fn information_register_bool(value: &str) -> Option<bool> {
    match value.trim() {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

fn information_register_indexing(value: &str) -> Option<&'static str> {
    metadata_attribute_indexing_xml(value.trim())
}

fn information_register_full_text_search(value: &str) -> Option<&'static str> {
    register_child_full_text_search_xml(value.trim())
}

fn information_register_data_history(value: &str) -> Option<&'static str> {
    metadata_data_history_xml(value.trim())
}

fn information_register_new_child_tail_is_valid(value: &str) -> bool {
    let Some(fields) = split_1c_braced_fields(value, 0) else {
        return false;
    };
    fields.len() == 2
        && fields.first().map(|field| field.trim()) == Some("1")
        && fields
            .get(1)
            .and_then(|field| parse_uuid_field(field.trim()))
            .is_some_and(|uuid| information_register_uuid_is_zero(&uuid))
}

fn parse_information_register_localized_value(value: &str) -> Option<Vec<(String, String)>> {
    let fields = split_1c_braced_fields(value, 0)?;
    let count = fields.first()?.trim().parse::<usize>().ok()?;
    let expected_len = count.checked_mul(2)?.checked_add(1)?;
    if fields.len() != expected_len {
        return None;
    }
    let mut result = Vec::with_capacity(count);
    for pair in fields[1..].chunks_exact(2) {
        result.push((
            parse_1c_quoted_string(pair[0].trim())?,
            parse_1c_quoted_string(pair[1].trim())?,
        ));
    }
    Some(result)
}

fn parse_information_register_bound(value: &str) -> Option<Option<String>> {
    let fields = split_1c_braced_fields(value, 0)?;
    match fields.first()?.trim() {
        r#""U""# if fields.len() == 1 => Some(None),
        r#""S""# if fields.len() == 2 => Some(Some(parse_1c_quoted_string(fields.get(1)?.trim())?)),
        _ => None,
    }
}

fn parse_information_register_fill_value(
    value: &str,
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
) -> Option<MetadataChildFillValue> {
    let fields = split_1c_braced_fields(value, 0)?;
    match fields.first()?.trim() {
        r#""U""# if fields.len() == 1 => Some(MetadataChildFillValue::Nil),
        r#""S""# if fields.len() == 2 => Some(MetadataChildFillValue::String(
            parse_1c_quoted_string(fields.get(1)?.trim())?,
        )),
        r#""N""# if fields.len() == 2 => {
            let value = fields.get(1)?.trim();
            information_register_decimal_is_valid(value)
                .then(|| MetadataChildFillValue::Decimal(value.to_string()))
        }
        r#""D""# if fields.len() == 2 => {
            format_1c_date_time(fields.get(1)?.trim()).map(MetadataChildFillValue::DateTime)
        }
        r#""B""# if fields.len() == 2 => {
            information_register_bool(fields.get(1)?).map(MetadataChildFillValue::Boolean)
        }
        r##""#""## if fields.len() == 3 => {
            parse_information_register_design_time_ref(value, type_index, object_refs)
                .map(MetadataChildFillValue::DesignTimeRef)
        }
        _ => None,
    }
}

fn information_register_decimal_is_valid(value: &str) -> bool {
    let value = value.strip_prefix('-').unwrap_or(value);
    let mut parts = value.split('.');
    let Some(integer) = parts.next() else {
        return false;
    };
    if integer.is_empty() || !integer.chars().all(|ch| ch.is_ascii_digit()) {
        return false;
    }
    match (parts.next(), parts.next()) {
        (None, None) => true,
        (Some(fraction), None) => {
            !fraction.is_empty() && fraction.chars().all(|ch| ch.is_ascii_digit())
        }
        _ => false,
    }
}

fn parse_information_register_choice_form(
    value: &str,
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
    form_refs: &BTreeMap<String, FormSourceReference>,
) -> Option<MetadataChoiceForm> {
    if let Some(uuid) = parse_uuid_field(value.trim()) {
        if information_register_uuid_is_zero(&uuid) {
            return Some(MetadataChoiceForm::Empty);
        }
        return information_register_form_reference(&uuid, form_refs)
            .map(MetadataChoiceForm::Reference);
    }
    let (owner_uuid, form_uuid) = parse_information_register_design_time_ref_ids(value)?;
    let owner_is_zero = information_register_uuid_is_zero(&owner_uuid);
    let form_is_zero = information_register_uuid_is_zero(&form_uuid);
    match (owner_is_zero, form_is_zero) {
        (true, true) => Some(MetadataChoiceForm::Empty),
        (false, false) => {
            let owner = information_register_design_time_owner_reference(
                &owner_uuid,
                type_index,
                object_refs,
            )?;
            let reference = information_register_form_reference(&form_uuid, form_refs)?;
            let expected_prefix = format!("{owner}.Form.");
            reference
                .strip_prefix(&expected_prefix)
                .is_some_and(|name| !name.is_empty() && !name.contains('.'))
                .then_some(MetadataChoiceForm::Reference(reference))
        }
        _ => None,
    }
}

fn information_register_form_reference(
    uuid: &str,
    form_refs: &BTreeMap<String, FormSourceReference>,
) -> Option<String> {
    let form = form_refs.get(uuid)?;
    if !matches!(form.kind, "Form" | "CommonForm") {
        return None;
    }
    let reference = form_source_reference_name(form)?;
    information_register_form_reference_is_valid(&reference).then_some(reference)
}

fn information_register_form_reference_is_valid(reference: &str) -> bool {
    if let Some(name) = reference.strip_prefix("CommonForm.") {
        return !name.is_empty() && !name.contains('.');
    }
    let Some((owner, form)) = reference.split_once(".Form.") else {
        return false;
    };
    let Some((kind, owner_name)) = owner.split_once('.') else {
        return false;
    };
    metadata_kind_can_own_forms(kind)
        && !owner_name.is_empty()
        && !owner_name.contains('.')
        && !form.is_empty()
        && !form.contains('.')
}

fn parse_information_register_design_time_ref(
    value: &str,
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    let (owner_uuid, value_uuid) = parse_information_register_design_time_ref_ids(value)?;
    let owner_is_zero = information_register_uuid_is_zero(&owner_uuid);
    let value_is_zero = information_register_uuid_is_zero(&value_uuid);
    match (owner_is_zero, value_is_zero) {
        (true, true) => Some(String::new()),
        (false, true) => {
            let owner = information_register_design_time_owner_reference(
                &owner_uuid,
                type_index,
                object_refs,
            )?;
            information_register_owner_reference_is_valid(&owner)
                .then(|| format!("{owner}.EmptyRef"))
        }
        (false, false) => {
            let owner = information_register_design_time_owner_reference(
                &owner_uuid,
                type_index,
                object_refs,
            )?;
            let qualified_key = metadata_owner_value_reference_key(&owner, &value_uuid);
            let value = object_refs
                .get(&qualified_key)
                .or_else(|| object_refs.get(&value_uuid))?
                .clone();
            information_register_reference_belongs_to_owner(&owner, &value).then_some(value)
        }
        (true, false) => None,
    }
}

fn metadata_owner_value_reference_key(owner_reference: &str, value_uuid: &str) -> String {
    format!("owner-value:{owner_reference}:{value_uuid}")
}

fn metadata_owner_value_reference_key_parts(key: &str) -> Option<(&str, &str)> {
    let (owner_reference, value_uuid) = key.strip_prefix("owner-value:")?.rsplit_once(':')?;
    is_uuid_text(value_uuid).then_some((owner_reference, value_uuid))
}

fn metadata_owner_value_reference_key_is_valid(key: &str) -> bool {
    metadata_owner_value_reference_key_parts(key).is_some_and(|(owner_reference, _)| {
        information_register_owner_reference_is_valid(owner_reference)
    })
}

fn extend_metadata_owner_value_references(
    target: &mut BTreeMap<String, String>,
    source: &BTreeMap<String, String>,
) -> Result<()> {
    for (key, reference) in source
        .iter()
        .filter(|(key, _)| metadata_owner_value_reference_key_is_valid(key))
    {
        if let Some(previous) = target.get(key)
            && previous != reference
        {
            bail!(
                "owner-qualified metadata value {key} resolves to both {previous} and {reference}"
            );
        }
        target.insert(key.clone(), reference.clone());
    }
    Ok(())
}

fn information_register_owner_reference_is_valid(owner: &str) -> bool {
    let mut owner_parts = owner.split('.');
    let (Some(owner_kind), Some(owner_name), None) =
        (owner_parts.next(), owner_parts.next(), owner_parts.next())
    else {
        return false;
    };
    if owner_kind.is_empty() || owner_name.is_empty() {
        return false;
    }
    true
}

fn information_register_reference_belongs_to_owner(owner: &str, value: &str) -> bool {
    if !information_register_owner_reference_is_valid(owner) {
        return false;
    }
    let mut owner_parts = owner.split('.');
    let owner_kind = owner_parts.next().expect("validated owner kind");
    let owner_name = owner_parts.next().expect("validated owner name");
    let value_parts = value.split('.').collect::<Vec<_>>();
    match value_parts.as_slice() {
        [kind, name, item] => {
            owner_kind != "Enum" && *kind == owner_kind && *name == owner_name && !item.is_empty()
        }
        ["Enum", name, "EnumValue", item] => {
            owner_kind == "Enum" && *name == owner_name && !item.is_empty()
        }
        _ => false,
    }
}

fn parse_information_register_design_time_ref_ids(value: &str) -> Option<(String, String)> {
    let fields = split_1c_braced_fields(value, 0)?;
    if fields.len() != 3
        || fields.first()?.trim() != r##""#""##
        || !fields
            .get(1)?
            .trim()
            .eq_ignore_ascii_case(DESIGN_TIME_REF_TYPE_UUID)
    {
        return None;
    }
    let reference = split_1c_braced_fields(fields.get(2)?, 0)?;
    if reference.len() != 3 || reference.first()?.trim() != "0" {
        return None;
    }
    let owner_uuid = parse_uuid_field(reference.get(1)?.trim())?;
    let value_uuid = parse_uuid_field(reference.get(2)?.trim())?;
    Some((owner_uuid, value_uuid))
}

fn information_register_design_time_owner_reference(
    owner_uuid: &str,
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    if let Some(reference) = object_refs.get(owner_uuid) {
        return Some(reference.clone());
    }
    let generated_type = type_index.get(owner_uuid)?.strip_prefix("cfg:")?;
    let (kind, name) = generated_type.split_once("Ref.")?;
    if kind.is_empty() || name.is_empty() || name.contains('.') {
        return None;
    }
    Some(format!("{kind}.{name}"))
}

fn information_register_uuid_is_zero(value: &str) -> bool {
    value == "00000000-0000-0000-0000-000000000000"
}

fn parse_information_register_choice_parameter_links(
    value: &str,
    owner_name: &str,
    object_refs: &BTreeMap<String, String>,
    preserve_raw_data_paths: bool,
) -> Option<Vec<MetadataChoiceParameterLink>> {
    let fields = split_1c_braced_fields(value, 0)?;
    if fields.first()?.trim() != "5006" {
        return None;
    }
    let count = fields.get(1)?.trim().parse::<usize>().ok()?;
    let remaining = fields.len().checked_sub(2)?;
    if remaining < count.checked_mul(4)? {
        return None;
    }
    let mut index = 2usize;
    let mut result = Vec::with_capacity(count);
    for _ in 0..count {
        let name = parse_1c_quoted_string(fields.get(index)?.trim())?;
        let path_count = fields
            .get(index.checked_add(1)?)?
            .trim()
            .parse::<usize>()
            .ok()?;
        if path_count == 0 {
            return None;
        }
        index = index.checked_add(2)?;
        let path_end = index.checked_add(path_count)?;
        let path = parse_information_register_data_path(
            fields.get(index..path_end)?,
            owner_name,
            object_refs,
            preserve_raw_data_paths,
        )?;
        index = path_end;
        let value_change = match fields.get(index)?.trim() {
            "0" => "Clear",
            "1" => "DontChange",
            _ => return None,
        };
        index = index.checked_add(1)?;
        result.push(MetadataChoiceParameterLink {
            name,
            data_path: path,
            value_change,
        });
    }
    (index == fields.len()).then_some(result)
}

fn parse_information_register_link_by_type(
    value: &str,
    owner_name: &str,
    object_refs: &BTreeMap<String, String>,
    preserve_raw_data_paths: bool,
) -> Option<Option<MetadataChildLinkByType>> {
    let fields = split_1c_braced_fields(value, 0)?;
    if fields.first()?.trim() != "3" {
        return None;
    }
    let count = fields.get(1)?.trim().parse::<usize>().ok()?;
    let expected_len = count.checked_add(3)?;
    if fields.len() != expected_len {
        return None;
    }
    let link_item = fields.last()?.trim().parse::<u32>().ok()?;
    if link_item > 3 {
        return None;
    }
    if count == 0 {
        return (link_item == 0).then_some(None);
    }
    let path_end = 2usize.checked_add(count)?;
    Some(Some(MetadataChildLinkByType {
        data_path: parse_information_register_data_path(
            fields.get(2..path_end)?,
            owner_name,
            object_refs,
            preserve_raw_data_paths,
        )?,
        link_item,
    }))
}

fn parse_information_register_data_path(
    fields: &[&str],
    owner_name: &str,
    object_refs: &BTreeMap<String, String>,
    preserve_raw_data_paths: bool,
) -> Option<String> {
    fields
        .iter()
        .map(|field| {
            let segment = split_1c_braced_fields(field, 0)?;
            match (segment.first()?.trim(), segment.len()) {
                ("0", 1) => Some("0".to_string()),
                ("0", 2) => {
                    let uuid = parse_uuid_field(segment.get(1)?.trim())?;
                    if preserve_raw_data_paths {
                        Some(format!("0:{uuid}"))
                    } else {
                        Some(
                            object_refs
                                .get(&uuid)
                                .cloned()
                                .unwrap_or_else(|| format!("0:{uuid}")),
                        )
                    }
                }
                ("-2", 1) => Some(format!(
                    "InformationRegister.{owner_name}.StandardAttribute.Period"
                )),
                ("-3", 1) => Some(format!(
                    "InformationRegister.{owner_name}.StandardAttribute.Recorder"
                )),
                _ => None,
            }
        })
        .collect::<Option<Vec<_>>>()
        .map(|segments| segments.join("/"))
}

fn parse_information_register_choice_parameters(
    value: &str,
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
) -> Option<Vec<MetadataChoiceParameter>> {
    let fields = split_1c_braced_fields(value, 0)?;
    if fields.first()?.trim() != "0" {
        return None;
    }
    let count = fields.get(1)?.trim().parse::<usize>().ok()?;
    let expected_len = count.checked_mul(2)?.checked_add(2)?;
    if fields.len() != expected_len {
        return None;
    }
    let mut result = Vec::with_capacity(count);
    for pair in fields[2..].chunks_exact(2) {
        result.push(MetadataChoiceParameter {
            name: parse_1c_quoted_string(pair[0].trim())?,
            value: parse_information_register_choice_parameter_value(
                pair[1],
                type_index,
                object_refs,
            )?,
        });
    }
    Some(result)
}

fn parse_information_register_choice_parameter_value(
    value: &str,
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
) -> Option<MetadataChoiceParameterValue> {
    parse_metadata_choice_parameter_typed_value(value, type_index, object_refs, 0)
}

fn parse_metadata_choice_parameter_typed_value(
    value: &str,
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
    depth: usize,
) -> Option<MetadataChoiceParameterValue> {
    if depth > MAX_METADATA_CHOICE_PARAMETER_VALUE_DEPTH {
        return None;
    }
    let fields = split_1c_braced_fields(value, 0)?;
    match fields.first()?.trim() {
        r#""U""# if fields.len() == 1 => Some(MetadataChoiceParameterValue::Nil),
        r#""S""# if fields.len() == 2 => Some(MetadataChoiceParameterValue::String(
            parse_1c_quoted_string(fields.get(1)?.trim())?,
        )),
        r#""B""# if fields.len() == 2 => {
            information_register_bool(fields.get(1)?).map(MetadataChoiceParameterValue::Boolean)
        }
        r#""N""# if fields.len() == 2 => {
            let value = fields.get(1)?.trim();
            information_register_decimal_is_valid(value)
                .then(|| MetadataChoiceParameterValue::Decimal(value.to_string()))
        }
        r#""D""# if fields.len() == 2 => {
            format_1c_date_time(fields.get(1)?.trim()).map(MetadataChoiceParameterValue::DateTime)
        }
        r##""#""## if fields.len() == 3 => {
            let type_id = fields.get(1)?.trim();
            if type_id.eq_ignore_ascii_case(DESIGN_TIME_REF_TYPE_UUID) {
                return parse_information_register_design_time_ref(value, type_index, object_refs)
                    .map(MetadataChoiceParameterValue::DesignTimeRef);
            }
            if !type_id.eq_ignore_ascii_case(FIXED_ARRAY_TYPE_UUID) {
                return None;
            }
            let nested = split_1c_braced_fields(fields.get(2)?, 0)?;
            let count = nested.first()?.trim().parse::<usize>().ok()?;
            let expected_len = count.checked_add(1)?;
            if nested.len() != expected_len {
                return None;
            }
            nested
                .iter()
                .skip(1)
                .map(|value| {
                    parse_metadata_choice_parameter_typed_value(
                        value,
                        type_index,
                        object_refs,
                        depth.checked_add(1)?,
                    )
                })
                .collect::<Option<Vec<_>>>()
                .map(MetadataChoiceParameterValue::FixedArray)
        }
        _ => None,
    }
}

fn parse_catalog_child_properties(
    owner_name: &str,
    text: &str,
    marker_start: usize,
    child_uuid: &str,
    expected_wrapper_code: u32,
    value_types: &[ConstantValueType],
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
    metadata_object_refs: &BTreeMap<String, String>,
    form_refs: &BTreeMap<String, FormSourceReference>,
) -> Option<MetadataChildProperties> {
    let candidates = metadata_object_field_candidates_around_header(text, marker_start, child_uuid);
    let mut layouts = candidates
        .iter()
        .filter_map(|fields| parse_catalog_attribute_wrapper_fields(fields, Some(child_uuid)))
        .collect::<Vec<_>>();
    if layouts.len() != 1 {
        return None;
    }
    let (payload, wrapper, wrapper_code, _) = layouts.pop()?;
    let nested = wrapper_code == 8;
    if wrapper_code != expected_wrapper_code
        || candidates
            .iter()
            .filter(|fields| {
                catalog_attribute_collection_is_closed(fields, child_uuid, wrapper_code)
            })
            .count()
            != 1
    {
        return None;
    }

    let password_mode = information_register_bool(payload.get(2)?)?;
    let format = parse_metadata_child_localized_value(payload.get(3)?)?;
    let tooltip = parse_metadata_child_localized_value(payload.get(4)?)?;
    let mark_negatives = information_register_bool(payload.get(5)?)?;
    let mask = parse_1c_quoted_string(payload.get(6)?.trim())?;
    let multi_line = information_register_bool(payload.get(7)?)?;
    let min_value = parse_information_register_bound(payload.get(8)?)?;
    let max_value = parse_information_register_bound(payload.get(9)?)?;
    let choice_folders_and_items = catalog_choice_folders_and_items_xml(payload.get(10)?.trim())?;
    let choice_form = parse_catalog_child_choice_form(payload.get(11)?, value_types, form_refs)?;
    let quick_choice = catalog_quick_choice_xml(payload.get(12)?.trim())?;
    let fill_checking = match payload.get(13)?.trim() {
        "0" => "DontCheck",
        "1" => "ShowError",
        _ => return None,
    };
    let choice_parameter_links =
        parse_catalog_choice_parameter_links(payload.get(14)?, owner_name, nested, object_refs)?;
    let (link_by_type_empty, link_by_type) =
        parse_catalog_link_by_type(payload.get(15)?, owner_name, object_refs)?;
    let choice_parameters = parse_information_register_choice_parameters(
        payload.get(16)?,
        type_index,
        metadata_object_refs,
    )?;
    let extended_edit = information_register_bool(payload.get(17)?)?;
    let edit_format = parse_metadata_child_localized_value(payload.get(18)?)?;
    let fill_value =
        parse_information_register_fill_value(payload.get(19)?, type_index, metadata_object_refs)?;
    let fill_from_filling_value = information_register_bool(payload.get(20)?)?;
    if nested
        && (fill_from_filling_value
            || !matches!(&fill_value, MetadataChildFillValue::String(value) if value.is_empty()))
    {
        return None;
    }
    let create_on_input = catalog_create_on_input_xml(payload.get(21)?.trim())?;
    let choice_history_on_input = catalog_choice_history_on_input_xml(payload.get(22)?.trim())?;

    let indexing = metadata_attribute_indexing_xml(wrapper.get(2)?.trim())?;
    let (use_mode, full_text_search_index, data_history_index) = match wrapper_code {
        5 | 6 => (
            Some(catalog_attribute_use_mode_xml(wrapper.get(3)?.trim())?),
            4,
            5,
        ),
        8 => (None, 3, 4),
        _ => return None,
    };
    let full_text_search =
        catalog_attribute_full_text_search_xml(wrapper.get(full_text_search_index)?.trim())?;
    let data_history = metadata_data_history_xml(wrapper.get(data_history_index)?.trim())?;

    Some(MetadataChildProperties {
        password_mode,
        format,
        edit_format,
        tooltip,
        mark_negatives,
        mask,
        multi_line,
        extended_edit,
        min_value,
        max_value,
        fill_from_filling_value,
        emit_fill_from_filling_value: !nested,
        fill_value: Some(fill_value),
        emit_fill_value: !nested,
        fill_checking,
        choice_folders_and_items: Some(choice_folders_and_items),
        choice_parameter_links: Some(choice_parameter_links),
        choice_parameters: Some(choice_parameters),
        self_close_empty_choice_parameter_refs: false,
        quick_choice: Some(quick_choice),
        create_on_input: Some(create_on_input),
        choice_form: Some(choice_form),
        link_by_type_empty,
        link_by_type,
        choice_history_on_input: Some(choice_history_on_input),
        master: None,
        main_filter: None,
        balance: None,
        accounting_flag: None,
        ext_dimension_accounting_flag: None,
        deny_incomplete_values: None,
        use_mode,
        indexing: Some(indexing),
        full_text_search: Some(full_text_search),
        data_history: Some(data_history),
        type_reduction_mode: None,
        update_data_history_immediately_after_write: None,
        execute_after_write_data_history_version_processing: None,
    })
}

fn parse_catalog_attribute_wrapper_fields<'a>(
    fields: &[&'a str],
    expected_child_uuid: Option<&str>,
) -> Option<(Vec<&'a str>, Vec<&'a str>, u32, String)> {
    let wrapper_code = fields.first()?.trim().parse::<u32>().ok()?;
    let expected_len = match wrapper_code {
        5 => 6,
        6 => 8,
        8 => 5,
        _ => return None,
    };
    if fields.len() != expected_len {
        return None;
    }
    let (payload, child_uuid) = parse_metadata_code27_payload(fields.get(1)?.trim())?;
    if expected_child_uuid.is_some_and(|expected| expected != child_uuid) {
        return None;
    }
    metadata_attribute_indexing_xml(fields.get(2)?.trim())?;
    match wrapper_code {
        5 => {
            catalog_attribute_use_mode_xml(fields.get(3)?.trim())?;
            catalog_attribute_full_text_search_xml(fields.get(4)?.trim())?;
            metadata_data_history_xml(fields.get(5)?.trim())?;
        }
        6 => {
            catalog_attribute_use_mode_xml(fields.get(3)?.trim())?;
            catalog_attribute_full_text_search_xml(fields.get(4)?.trim())?;
            metadata_data_history_xml(fields.get(5)?.trim())?;
            if fields.get(6)?.trim() != "0" || !metadata_reserved_wrapper_tail(fields.get(7)?) {
                return None;
            }
        }
        8 => {
            catalog_attribute_full_text_search_xml(fields.get(3)?.trim())?;
            metadata_data_history_xml(fields.get(4)?.trim())?;
        }
        _ => return None,
    }
    Some((payload, fields.to_vec(), wrapper_code, child_uuid))
}

fn parse_metadata_code27_payload(field: &str) -> Option<(Vec<&str>, String)> {
    let payload = split_1c_braced_fields(field, 0)?;
    if payload.len() != 23 || payload.first()?.trim() != "27" {
        return None;
    }
    let detail = split_1c_braced_fields(payload.get(1)?.trim(), 0)?;
    if detail.len() != 3 || detail.first()?.trim() != "2" {
        return None;
    }
    let header = split_1c_braced_fields(detail.get(1)?.trim(), 0)?;
    if header.len() != 9 || header.first()?.trim() != "3" {
        return None;
    }
    let identity = split_1c_braced_fields(header.get(1)?.trim(), 0)?;
    if identity.len() != 3 || identity[0].trim() != "1" || identity[1].trim() != "0" {
        return None;
    }
    let child_uuid = parse_uuid_field(identity[2].trim())?;
    let parsed_header = parse_metadata_header_from_text(detail.get(1)?, &child_uuid)?;
    if parsed_header.uuid != child_uuid {
        return None;
    }
    let pattern = split_1c_braced_fields(detail.get(2)?.trim(), 0)?;
    if pattern.len() < 2 || pattern.first()?.trim() != r#""Pattern""# {
        return None;
    }
    Some((payload, child_uuid))
}

fn metadata_reserved_wrapper_tail(field: &str) -> bool {
    let Some(fields) = split_1c_braced_fields(field.trim(), 0) else {
        return false;
    };
    fields.len() == 2
        && fields[0].trim() == "1"
        && fields[1].trim() == "00000000-0000-0000-0000-000000000000"
}

fn catalog_attribute_collection_is_closed(
    fields: &[&str],
    child_uuid: &str,
    expected_wrapper_code: u32,
) -> bool {
    let expected_marker = if expected_wrapper_code == 8 {
        CATALOG_TABULAR_ATTRIBUTE_GROUP_UUID
    } else {
        CATALOG_ATTRIBUTE_GROUP_UUID
    };
    if fields.len() < 3
        || !parse_uuid_field(fields[0].trim())
            .is_some_and(|marker| marker.eq_ignore_ascii_case(expected_marker))
    {
        return false;
    }
    let Some(count) = fields[1].trim().parse::<usize>().ok() else {
        return false;
    };
    if count == 0 || count.checked_add(2) != Some(fields.len()) {
        return false;
    }
    let mut child_occurrences = 0usize;
    for item_field in fields.iter().skip(2) {
        let Some(item) = split_1c_braced_fields(item_field.trim(), 0) else {
            return false;
        };
        if item.len() != 2 || item[1].trim() != "0" {
            return false;
        }
        let Some(wrapper) = split_1c_braced_fields(item[0].trim(), 0) else {
            return false;
        };
        let Some((_, _, wrapper_code, item_child_uuid)) =
            parse_catalog_attribute_wrapper_fields(&wrapper, None)
        else {
            return false;
        };
        if wrapper_code != expected_wrapper_code {
            return false;
        }
        if item_child_uuid == child_uuid {
            child_occurrences += 1;
        }
    }
    child_occurrences == 1
}

struct DocumentDataPathOwnerProof {
    entries: BTreeMap<String, DocumentDataPathOwnerEntry>,
    ambiguous: BTreeSet<String>,
}

struct DocumentDataPathOwnerEntry {
    reference: String,
    role: DocumentDataPathOwnerRole,
}

#[derive(Eq, PartialEq)]
enum DocumentDataPathOwnerRole {
    DirectAttribute,
    TabularSection,
    TabularAttribute { parent_uuid: String },
}

fn build_document_data_path_owner_proof(
    owner_name: &str,
    text: &str,
    owner_uuid: &str,
) -> DocumentDataPathOwnerProof {
    let mut entries = BTreeMap::<String, DocumentDataPathOwnerEntry>::new();
    let mut ambiguous = BTreeSet::<String>::new();
    for (child, marker_start) in nested_headers_with_offsets_from_text(text, owner_uuid, |_| true) {
        let Some((tag, parent)) = attribute_tabular_section_child_object_tag(
            "Document",
            owner_name,
            owner_uuid,
            text,
            marker_start,
            &child,
        ) else {
            continue;
        };
        let (reference, role) = match (tag, parent) {
            ("Attribute", None) => (
                format!("Document.{owner_name}.Attribute.{}", child.name),
                DocumentDataPathOwnerRole::DirectAttribute,
            ),
            ("Attribute", Some(parent)) => (
                format!(
                    "Document.{owner_name}.TabularSection.{}.Attribute.{}",
                    parent.name, child.name
                ),
                DocumentDataPathOwnerRole::TabularAttribute {
                    parent_uuid: parent.uuid,
                },
            ),
            ("TabularSection", None) => (
                format!("Document.{owner_name}.TabularSection.{}", child.name),
                DocumentDataPathOwnerRole::TabularSection,
            ),
            _ => continue,
        };
        if ambiguous.contains(&child.uuid) || entries.contains_key(&child.uuid) {
            entries.remove(&child.uuid);
            ambiguous.insert(child.uuid);
            continue;
        }
        entries.insert(child.uuid, DocumentDataPathOwnerEntry { reference, role });
    }
    DocumentDataPathOwnerProof { entries, ambiguous }
}

fn parse_document_child_properties(
    owner_name: &str,
    text: &str,
    marker_start: usize,
    child_uuid: &str,
    expected_nested: bool,
    value_types: &[ConstantValueType],
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
    metadata_object_refs: &BTreeMap<String, String>,
    form_refs: &BTreeMap<String, FormSourceReference>,
    data_path_owner_proof: &DocumentDataPathOwnerProof,
) -> Option<MetadataChildProperties> {
    let candidates = metadata_object_field_candidates_around_header(text, marker_start, child_uuid);
    let mut layouts = candidates
        .iter()
        .filter_map(|fields| parse_document_attribute_wrapper_fields(fields, Some(child_uuid)))
        .collect::<Vec<_>>();
    if layouts.len() != 1 {
        return None;
    }
    let (payload, wrapper, wrapper_code, _) = layouts.pop()?;
    let nested = wrapper_code == 8;
    if nested != expected_nested
        || candidates
            .iter()
            .filter(|fields| {
                document_attribute_collection_is_closed(fields, child_uuid, wrapper_code)
            })
            .count()
            != 1
    {
        return None;
    }

    let password_mode = information_register_bool(payload.get(2)?)?;
    let format = parse_metadata_child_localized_value(payload.get(3)?)?;
    let tooltip = parse_metadata_child_localized_value(payload.get(4)?)?;
    let mark_negatives = information_register_bool(payload.get(5)?)?;
    let mask = parse_1c_quoted_string(payload.get(6)?.trim())?;
    let multi_line = information_register_bool(payload.get(7)?)?;
    let min_value = parse_information_register_bound(payload.get(8)?)?;
    let max_value = parse_information_register_bound(payload.get(9)?)?;
    let choice_folders_and_items = catalog_choice_folders_and_items_xml(payload.get(10)?.trim())?;
    let choice_form = parse_document_child_choice_form(payload.get(11)?, value_types, form_refs)?;
    let quick_choice = catalog_quick_choice_xml(payload.get(12)?.trim())?;
    let fill_checking = match payload.get(13)?.trim() {
        "0" => "DontCheck",
        "1" => "ShowError",
        _ => return None,
    };
    let choice_parameter_links = parse_document_choice_parameter_links(
        payload.get(14)?,
        owner_name,
        nested,
        object_refs,
        data_path_owner_proof,
    )?;
    let (link_by_type_empty, link_by_type) = parse_document_link_by_type(
        payload.get(15)?,
        owner_name,
        object_refs,
        data_path_owner_proof,
    )?;
    let choice_parameters = parse_information_register_choice_parameters(
        payload.get(16)?,
        type_index,
        metadata_object_refs,
    )?;
    let extended_edit = information_register_bool(payload.get(17)?)?;
    let edit_format = parse_metadata_child_localized_value(payload.get(18)?)?;
    let fill_value = parse_document_fill_value(
        payload.get(19)?,
        value_types,
        type_index,
        metadata_object_refs,
        form_refs,
    )?;
    let fill_from_filling_value = information_register_bool(payload.get(20)?)?;
    if nested
        && (fill_from_filling_value
            || !matches!(&fill_value, MetadataChildFillValue::String(value) if value.is_empty()))
    {
        return None;
    }
    let create_on_input = catalog_create_on_input_xml(payload.get(21)?.trim())?;
    let choice_history_on_input = catalog_choice_history_on_input_xml(payload.get(22)?.trim())?;

    let indexing = metadata_attribute_indexing_xml(wrapper.get(2)?.trim())?;
    let full_text_search = register_child_full_text_search_xml(wrapper.get(3)?.trim())?;
    let data_history = metadata_data_history_xml(wrapper.get(4)?.trim())?;

    Some(MetadataChildProperties {
        password_mode,
        format,
        edit_format,
        tooltip,
        mark_negatives,
        mask,
        multi_line,
        extended_edit,
        min_value,
        max_value,
        fill_from_filling_value,
        emit_fill_from_filling_value: !nested,
        fill_value: Some(fill_value),
        emit_fill_value: !nested,
        fill_checking,
        choice_folders_and_items: Some(choice_folders_and_items),
        choice_parameter_links: Some(choice_parameter_links),
        choice_parameters: Some(choice_parameters),
        self_close_empty_choice_parameter_refs: true,
        quick_choice: Some(quick_choice),
        create_on_input: Some(create_on_input),
        choice_form: Some(choice_form),
        link_by_type_empty,
        link_by_type,
        choice_history_on_input: Some(choice_history_on_input),
        master: None,
        main_filter: None,
        balance: None,
        accounting_flag: None,
        ext_dimension_accounting_flag: None,
        deny_incomplete_values: None,
        use_mode: None,
        indexing: Some(indexing),
        full_text_search: Some(full_text_search),
        data_history: Some(data_history),
        type_reduction_mode: None,
        update_data_history_immediately_after_write: None,
        execute_after_write_data_history_version_processing: None,
    })
}

fn parse_document_fill_value(
    value: &str,
    value_types: &[ConstantValueType],
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
    form_refs: &BTreeMap<String, FormSourceReference>,
) -> Option<MetadataChildFillValue> {
    if let Some(value) = parse_information_register_fill_value(value, type_index, object_refs) {
        return Some(value);
    }
    let (owner_uuid, value_uuid) = parse_information_register_design_time_ref_ids(value)?;
    if information_register_uuid_is_zero(&owner_uuid)
        || information_register_uuid_is_zero(&value_uuid)
    {
        return None;
    }
    let owner_type = type_index.get(&owner_uuid)?;
    let enum_name = owner_type.strip_prefix("cfg:EnumRef.")?;
    if enum_name.is_empty() || enum_name.contains('.') {
        return None;
    }
    if !matches!(
        value_types,
        [ConstantValueType::Reference { reference }] if reference == owner_type
    ) {
        return None;
    }
    if object_refs.contains_key(&owner_uuid)
        || object_refs.contains_key(&value_uuid)
        || object_refs.keys().any(|key| {
            metadata_owner_value_reference_key_parts(key)
                .is_some_and(|(_, candidate)| candidate == value_uuid)
        })
        || type_index.contains_key(&value_uuid)
        || form_refs.contains_key(&value_uuid)
    {
        return None;
    }
    Some(MetadataChildFillValue::DesignTimeRef(format!(
        "{owner_uuid}.{value_uuid}"
    )))
}

fn parse_document_attribute_wrapper_fields<'a>(
    fields: &[&'a str],
    expected_child_uuid: Option<&str>,
) -> Option<(Vec<&'a str>, Vec<&'a str>, u32, String)> {
    let wrapper_code = fields.first()?.trim().parse::<u32>().ok()?;
    let expected_len = match wrapper_code {
        5 | 8 => 5,
        6 => 7,
        _ => return None,
    };
    if fields.len() != expected_len {
        return None;
    }
    let (payload, child_uuid) = parse_metadata_code27_payload(fields.get(1)?.trim())?;
    if expected_child_uuid.is_some_and(|expected| !child_uuid.eq_ignore_ascii_case(expected)) {
        return None;
    }
    metadata_attribute_indexing_xml(fields.get(2)?.trim())?;
    register_child_full_text_search_xml(fields.get(3)?.trim())?;
    metadata_data_history_xml(fields.get(4)?.trim())?;
    if wrapper_code == 6
        && (fields.get(5)?.trim() != "0" || !metadata_reserved_wrapper_tail(fields.get(6)?))
    {
        return None;
    }
    Some((payload, fields.to_vec(), wrapper_code, child_uuid))
}

fn document_attribute_collection_is_closed(
    fields: &[&str],
    child_uuid: &str,
    expected_wrapper_code: u32,
) -> bool {
    let expected_marker = if expected_wrapper_code == 8 {
        DOCUMENT_TABULAR_ATTRIBUTE_GROUP_UUID
    } else if matches!(expected_wrapper_code, 5 | 6) {
        DOCUMENT_ATTRIBUTE_GROUP_UUID
    } else {
        return false;
    };
    if fields.len() < 3
        || !parse_uuid_field(fields[0].trim())
            .is_some_and(|marker| marker.eq_ignore_ascii_case(expected_marker))
    {
        return false;
    }
    let Some(count) = fields[1].trim().parse::<usize>().ok() else {
        return false;
    };
    if count == 0 || count.checked_add(2) != Some(fields.len()) {
        return false;
    }
    let mut child_occurrences = 0usize;
    for item_field in fields.iter().skip(2) {
        let Some(item) = split_1c_braced_fields(item_field.trim(), 0) else {
            return false;
        };
        if item.len() != 2 || item[1].trim() != "0" {
            return false;
        }
        let Some(wrapper) = split_1c_braced_fields(item[0].trim(), 0) else {
            return false;
        };
        let Some((_, _, wrapper_code, item_child_uuid)) =
            parse_document_attribute_wrapper_fields(&wrapper, None)
        else {
            return false;
        };
        if wrapper_code != expected_wrapper_code {
            return false;
        }
        if item_child_uuid.eq_ignore_ascii_case(child_uuid) {
            child_occurrences += 1;
        }
    }
    child_occurrences == 1
}

fn parse_document_child_choice_form(
    value: &str,
    value_types: &[ConstantValueType],
    form_refs: &BTreeMap<String, FormSourceReference>,
) -> Option<MetadataChoiceForm> {
    let uuid = parse_uuid_field(value.trim())?;
    if information_register_uuid_is_zero(&uuid) {
        return Some(MetadataChoiceForm::Empty);
    }
    let reference = information_register_form_reference(&uuid, form_refs)?;
    let (owner, form_name) = reference.split_once(".Form.")?;
    if form_name.is_empty() || form_name.contains('.') {
        return None;
    }
    let (owner_kind, owner_name) = owner.split_once('.')?;
    if !matches!(owner_kind, "Catalog" | "Document")
        || owner_name.is_empty()
        || owner_name.contains('.')
    {
        return None;
    }
    let expected_type = format!("cfg:{owner_kind}Ref.{owner_name}");
    (value_types
        .iter()
        .filter(|value_type| {
            matches!(
                value_type,
                ConstantValueType::Reference { reference } if reference == &expected_type
            )
        })
        .count()
        == 1)
        .then_some(MetadataChoiceForm::Reference(reference))
}

fn parse_document_choice_parameter_links(
    value: &str,
    owner_name: &str,
    nested: bool,
    object_refs: &BTreeMap<String, String>,
    data_path_owner_proof: &DocumentDataPathOwnerProof,
) -> Option<Vec<MetadataChoiceParameterLink>> {
    let fields = split_1c_braced_fields(value.trim(), 0)?;
    if fields.len() < 2 || fields.first()?.trim() != "5006" {
        return None;
    }
    let count = fields.get(1)?.trim().parse::<usize>().ok()?;
    let minimum_len = count.checked_mul(4)?.checked_add(2)?;
    if fields.len() < minimum_len {
        return None;
    }
    let mut links = Vec::with_capacity(count);
    let mut index = 2usize;
    for _ in 0..count {
        let name = parse_1c_quoted_string(fields.get(index)?.trim())?;
        index = index.checked_add(1)?;
        let path_count = fields.get(index)?.trim().parse::<usize>().ok()?;
        index = index.checked_add(1)?;
        if if nested {
            !matches!(path_count, 1 | 2)
        } else {
            path_count != 1
        } {
            return None;
        }
        let path_end = index.checked_add(path_count)?;
        if path_end >= fields.len() {
            return None;
        }
        let data_path = resolve_document_data_path(
            fields.get(index..path_end)?,
            owner_name,
            object_refs,
            data_path_owner_proof,
            true,
        )?;
        index = path_end;
        let value_change = match fields.get(index)?.trim() {
            "0" => "Clear",
            "1" => "DontChange",
            _ => return None,
        };
        index = index.checked_add(1)?;
        links.push(MetadataChoiceParameterLink {
            name,
            data_path,
            value_change,
        });
    }
    (index == fields.len()).then_some(links)
}

fn parse_document_link_by_type(
    value: &str,
    owner_name: &str,
    object_refs: &BTreeMap<String, String>,
    data_path_owner_proof: &DocumentDataPathOwnerProof,
) -> Option<(bool, Option<MetadataChildLinkByType>)> {
    let fields = split_1c_braced_fields(value.trim(), 0)?;
    if fields.len() < 3 || fields.first()?.trim() != "3" {
        return None;
    }
    let path_count = fields.get(1)?.trim().parse::<usize>().ok()?;
    if path_count.checked_add(3) != Some(fields.len()) {
        return None;
    }
    let link_item = fields.last()?.trim().parse::<u32>().ok()?;
    if link_item > 3 {
        return None;
    }
    if path_count == 0 {
        return (link_item == 0).then_some((true, None));
    }
    if !matches!(path_count, 1 | 2) {
        return None;
    }
    let path_end = 2usize.checked_add(path_count)?;
    let data_path = resolve_document_data_path(
        fields.get(2..path_end)?,
        owner_name,
        object_refs,
        data_path_owner_proof,
        false,
    )?;
    Some((
        false,
        Some(MetadataChildLinkByType {
            data_path,
            link_item,
        }),
    ))
}

fn resolve_document_data_path(
    fields: &[&str],
    owner_name: &str,
    object_refs: &BTreeMap<String, String>,
    data_path_owner_proof: &DocumentDataPathOwnerProof,
    allow_raw_fallback: bool,
) -> Option<String> {
    match fields {
        [field] => {
            let segment = split_1c_braced_fields(field.trim(), 0)?;
            match segment.as_slice() {
                [code] if code.trim() == "-3" => {
                    Some(format!("Document.{owner_name}.StandardAttribute.Date"))
                }
                [code] if code.trim() == "-5" => {
                    Some(format!("Document.{owner_name}.StandardAttribute.Ref"))
                }
                [code] if allow_raw_fallback && matches!(code.trim(), "-8" | "0") => {
                    Some(code.trim().to_string())
                }
                [kind, uuid] if kind.trim() == "0" => {
                    let uuid = parse_uuid_field(uuid.trim())?;
                    match (
                        data_path_owner_proof.entries.get(&uuid),
                        object_refs.get(&uuid),
                    ) {
                        (Some(entry), Some(reference))
                            if entry.role == DocumentDataPathOwnerRole::DirectAttribute
                                && reference == &entry.reference =>
                        {
                            Some(entry.reference.clone())
                        }
                        (None, reference)
                            if allow_raw_fallback
                                && !data_path_owner_proof.ambiguous.contains(&uuid)
                                && reference.is_none_or(|reference| {
                                    !document_data_path_reference_belongs_to_owner(
                                        reference, owner_name,
                                    )
                                }) =>
                        {
                            Some(format!("0:{uuid}"))
                        }
                        _ => None,
                    }
                }
                _ => None,
            }
        }
        [parent, child] => {
            let parent = split_1c_braced_fields(parent.trim(), 0)?;
            let child = split_1c_braced_fields(child.trim(), 0)?;
            if allow_raw_fallback
                && matches!(parent.as_slice(), [code] if code.trim() == "0")
                && matches!(child.as_slice(), [code] if code.trim() == "0")
            {
                return Some("0/0".to_string());
            }
            let (parent_uuid, child_uuid) = match (parent.as_slice(), child.as_slice()) {
                ([parent_kind, parent_uuid], [child_kind, child_uuid])
                    if parent_kind.trim() == "0" && child_kind.trim() == "0" =>
                {
                    (
                        parse_uuid_field(parent_uuid.trim())?,
                        parse_uuid_field(child_uuid.trim())?,
                    )
                }
                _ => return None,
            };
            match (
                data_path_owner_proof.entries.get(&parent_uuid),
                object_refs.get(&parent_uuid),
                data_path_owner_proof.entries.get(&child_uuid),
                object_refs.get(&child_uuid),
            ) {
                (
                    Some(parent_entry),
                    Some(parent_reference),
                    Some(child_entry),
                    Some(child_reference),
                ) if parent_entry.role == DocumentDataPathOwnerRole::TabularSection
                    && child_entry.role
                        == (DocumentDataPathOwnerRole::TabularAttribute {
                            parent_uuid: parent_uuid.clone(),
                        })
                    && parent_reference == &parent_entry.reference
                    && child_reference == &child_entry.reference =>
                {
                    Some(child_entry.reference.clone())
                }
                (None, parent_reference, None, child_reference)
                    if allow_raw_fallback
                        && !data_path_owner_proof.ambiguous.contains(&parent_uuid)
                        && !data_path_owner_proof.ambiguous.contains(&child_uuid)
                        && [parent_reference, child_reference]
                            .into_iter()
                            .all(|reference| {
                                reference.is_none_or(|reference| {
                                    !document_data_path_reference_belongs_to_owner(
                                        reference, owner_name,
                                    )
                                })
                            }) =>
                {
                    Some(format!("0:{parent_uuid}/0:{child_uuid}"))
                }
                _ => None,
            }
        }
        _ => None,
    }
}

fn document_data_path_reference_belongs_to_owner(reference: &str, owner_name: &str) -> bool {
    let owner_reference = format!("Document.{owner_name}");
    reference == owner_reference || reference.starts_with(&format!("{owner_reference}."))
}

fn parse_metadata_child_localized_value(value: &str) -> Option<Vec<(String, String)>> {
    let values = parse_information_register_localized_value(value)?;
    let mut languages = BTreeSet::new();
    values
        .iter()
        .all(|(language, _)| languages.insert(language))
        .then_some(values)
}

fn parse_catalog_child_choice_form(
    value: &str,
    value_types: &[ConstantValueType],
    form_refs: &BTreeMap<String, FormSourceReference>,
) -> Option<MetadataChoiceForm> {
    let uuid = parse_uuid_field(value.trim())?;
    let choice_form = if information_register_uuid_is_zero(&uuid) {
        MetadataChoiceForm::Empty
    } else {
        MetadataChoiceForm::Reference(information_register_form_reference(&uuid, form_refs)?)
    };
    let MetadataChoiceForm::Reference(reference) = &choice_form else {
        return Some(choice_form);
    };
    let owner_name = catalog_choice_form_owner_name(reference)?;
    let expected_type = format!("cfg:CatalogRef.{owner_name}");
    (value_types
        .iter()
        .filter(|value_type| {
            matches!(
                value_type,
                ConstantValueType::Reference { reference } if reference == &expected_type
            )
        })
        .count()
        == 1)
        .then_some(choice_form)
}

fn catalog_choice_form_owner_name(reference: &str) -> Option<&str> {
    let (owner_name, form_name) = reference.strip_prefix("Catalog.")?.split_once(".Form.")?;
    (!owner_name.is_empty()
        && !owner_name.contains('.')
        && !form_name.is_empty()
        && !form_name.contains('.'))
    .then_some(owner_name)
}

fn parse_catalog_choice_parameter_links(
    value: &str,
    owner_name: &str,
    nested: bool,
    object_refs: &BTreeMap<String, String>,
) -> Option<Vec<MetadataChoiceParameterLink>> {
    let fields = split_1c_braced_fields(value.trim(), 0)?;
    if fields.len() < 2 || fields.first()?.trim() != "5006" {
        return None;
    }
    let count = fields.get(1)?.trim().parse::<usize>().ok()?;
    let minimum_len = count.checked_mul(4)?.checked_add(2)?;
    if fields.len() < minimum_len {
        return None;
    }
    let mut links = Vec::with_capacity(count);
    let mut index = 2usize;
    for _ in 0..count {
        let name = parse_1c_quoted_string(fields.get(index)?.trim())?;
        index = index.checked_add(1)?;
        let path_count = fields.get(index)?.trim().parse::<usize>().ok()?;
        index = index.checked_add(1)?;
        if if nested {
            !matches!(path_count, 1 | 2)
        } else {
            path_count != 1
        } {
            return None;
        }
        let path_end = index.checked_add(path_count)?;
        if path_end >= fields.len() {
            return None;
        }
        let data_path =
            resolve_catalog_data_path(&fields[index..path_end], owner_name, object_refs)?;
        index = path_end;
        let value_change = match fields.get(index)?.trim() {
            "0" => "Clear",
            "1" => "DontChange",
            _ => return None,
        };
        index = index.checked_add(1)?;
        links.push(MetadataChoiceParameterLink {
            name,
            data_path,
            value_change,
        });
    }
    (index == fields.len()).then_some(links)
}

fn parse_catalog_link_by_type(
    value: &str,
    owner_name: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<(bool, Option<MetadataChildLinkByType>)> {
    let fields = split_1c_braced_fields(value.trim(), 0)?;
    if fields.len() < 3 || fields.first()?.trim() != "3" {
        return None;
    }
    let path_count = fields.get(1)?.trim().parse::<usize>().ok()?;
    if path_count.checked_add(3) != Some(fields.len()) {
        return None;
    }
    let link_item = fields.last()?.trim().parse::<u32>().ok()?;
    if link_item > 3 {
        return None;
    }
    if path_count == 0 {
        return (link_item == 0).then_some((true, None));
    }
    let path_end = 2usize.checked_add(path_count)?;
    let data_path = resolve_catalog_data_path(&fields[2..path_end], owner_name, object_refs)?;
    Some((
        false,
        Some(MetadataChildLinkByType {
            data_path,
            link_item,
        }),
    ))
}

fn resolve_catalog_data_path(
    fields: &[&str],
    owner_name: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    let owner_prefix = format!("Catalog.{owner_name}.");
    let mut resolved = Vec::with_capacity(fields.len());
    for field in fields {
        let path = resolve_catalog_data_path_segment(field, owner_name, object_refs)?;
        if !path.starts_with(&owner_prefix) {
            return None;
        }
        if let Some(parent) = resolved.last()
            && !path.starts_with(&format!("{parent}."))
        {
            return None;
        }
        resolved.push(path);
    }
    resolved.pop()
}

fn resolve_catalog_data_path_segment(
    value: &str,
    owner_name: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    let fields = split_1c_braced_fields(value.trim(), 0)?;
    match fields.as_slice() {
        [code] => {
            let standard_attribute = match code.trim() {
                "-3" => "Description",
                "-5" => "Owner",
                "-8" => "Ref",
                _ => return None,
            };
            Some(format!(
                "Catalog.{owner_name}.StandardAttribute.{standard_attribute}"
            ))
        }
        [kind, uuid] if kind.trim() == "0" => {
            let uuid = parse_uuid_field(uuid.trim())?;
            object_refs.get(&uuid).cloned()
        }
        _ => None,
    }
}

fn catalog_choice_folders_and_items_xml(value: &str) -> Option<&'static str> {
    match value {
        "0" => Some("Items"),
        "1" => Some("Folders"),
        "2" => Some("FoldersAndItems"),
        _ => None,
    }
}

fn catalog_quick_choice_xml(value: &str) -> Option<&'static str> {
    match value {
        "0" => Some("DontUse"),
        "1" => Some("Use"),
        "2" => Some("Auto"),
        _ => None,
    }
}

fn catalog_create_on_input_xml(value: &str) -> Option<&'static str> {
    match value {
        "0" => Some("Auto"),
        "1" => Some("DontUse"),
        "2" => Some("Use"),
        _ => None,
    }
}

fn catalog_choice_history_on_input_xml(value: &str) -> Option<&'static str> {
    match value {
        "0" => Some("Auto"),
        "1" => Some("DontUse"),
        _ => None,
    }
}

fn catalog_attribute_use_mode_xml(value: &str) -> Option<&'static str> {
    match value {
        "0" => Some("ForItem"),
        "1" => Some("ForFolder"),
        "2" => Some("ForFolderAndItem"),
        _ => None,
    }
}

fn catalog_attribute_full_text_search_xml(value: &str) -> Option<&'static str> {
    match value {
        "0" => Some("DontUse"),
        "1" => Some("Use"),
        _ => None,
    }
}

fn catalog_direct_attribute_wrapper_code(root_code: &str) -> Option<u32> {
    match root_code {
        "56" => Some(5),
        "57" => Some(6),
        _ => None,
    }
}

fn parse_accounting_register_child_properties_from_fields(
    fields: &[&str],
    child_uuid: &str,
) -> Option<MetadataChildProperties> {
    if fields.first().map(|field| field.trim()) != Some("27")
        || metadata_header_field_index(fields, child_uuid).is_none()
        || fields.len() < 18
    {
        return None;
    }
    Some(MetadataChildProperties {
        password_mode: parse_1c_bool_field(fields.get(2).copied()).unwrap_or(false),
        format: parse_1c_synonyms(fields.get(3).copied().unwrap_or("{0}")),
        edit_format: Vec::new(),
        tooltip: parse_1c_synonyms(fields.get(4).copied().unwrap_or("{0}")),
        mark_negatives: parse_1c_bool_field(fields.get(5).copied()).unwrap_or(false),
        mask: fields
            .get(6)
            .and_then(|field| parse_1c_quoted_string(field.trim()))
            .unwrap_or_default(),
        multi_line: parse_1c_bool_field(fields.get(7).copied()).unwrap_or(false),
        extended_edit: parse_1c_bool_field(fields.get(17).copied()).unwrap_or(false),
        min_value: parse_constant_bound_value(fields.get(8).copied()),
        max_value: parse_constant_bound_value(fields.get(9).copied()),
        fill_from_filling_value: false,
        emit_fill_from_filling_value: false,
        fill_value: None,
        emit_fill_value: false,
        fill_checking: metadata_fill_checking_xml(fields.get(13).copied()),
        choice_folders_and_items: Some("Items"),
        choice_parameter_links: Some(Vec::new()),
        choice_parameters: Some(Vec::new()),
        self_close_empty_choice_parameter_refs: false,
        quick_choice: Some("Auto"),
        create_on_input: Some("Auto"),
        choice_form: Some(MetadataChoiceForm::Empty),
        link_by_type_empty: true,
        link_by_type: None,
        choice_history_on_input: Some("Auto"),
        master: None,
        main_filter: None,
        balance: None,
        accounting_flag: None,
        ext_dimension_accounting_flag: None,
        deny_incomplete_values: None,
        use_mode: None,
        indexing: None,
        full_text_search: None,
        data_history: None,
        type_reduction_mode: None,
        update_data_history_immediately_after_write: None,
        execute_after_write_data_history_version_processing: None,
    })
}

fn parse_metadata_child_properties_from_fields(
    owner_kind: &str,
    fields: &[&str],
    child_uuid: &str,
    value_types: &[ConstantValueType],
    object_refs: &BTreeMap<String, String>,
) -> Option<MetadataChildProperties> {
    if owner_kind == "DataProcessor"
        && let Some(flattened) = flatten_data_processor_wrapped_child_fields(fields)
        && let Some(header_index) = metadata_header_field_index(&flattened, child_uuid)
        && let Some(properties) = parse_data_processor_wrapped_child_properties(
            &flattened,
            header_index,
            value_types,
            object_refs,
        )
    {
        return Some(properties);
    }

    let header_index = metadata_header_field_index(&fields, child_uuid)?;
    if fields.len() <= header_index + 13 {
        return None;
    }
    Some(MetadataChildProperties {
        password_mode: parse_1c_bool_field(fields.get(header_index + 1).copied())?,
        format: parse_1c_synonyms(fields.get(header_index + 2).copied().unwrap_or("{0}")),
        edit_format: parse_1c_synonyms(fields.get(header_index + 3).copied().unwrap_or("{0}")),
        tooltip: parse_1c_synonyms(fields.get(header_index + 4).copied().unwrap_or("{0}")),
        mark_negatives: parse_1c_bool_field(fields.get(header_index + 5).copied())?,
        mask: fields
            .get(header_index + 6)
            .and_then(|field| parse_1c_quoted_string(field.trim()))
            .unwrap_or_default(),
        multi_line: parse_1c_bool_field(fields.get(header_index + 7).copied())?,
        extended_edit: parse_1c_bool_field(fields.get(header_index + 8).copied())?,
        min_value: parse_constant_bound_value(fields.get(header_index + 9).copied()),
        max_value: parse_constant_bound_value(fields.get(header_index + 10).copied()),
        fill_from_filling_value: parse_1c_bool_field(fields.get(header_index + 11).copied())?,
        emit_fill_from_filling_value: true,
        fill_value: parse_metadata_child_fill_value(
            fields.get(header_index + 12).copied(),
            value_types,
            object_refs,
        ),
        emit_fill_value: true,
        fill_checking: metadata_fill_checking_xml(fields.get(header_index + 13).copied()),
        choice_folders_and_items: fields
            .get(header_index + 14)
            .and_then(|field| metadata_choice_folders_and_items_xml(field.trim())),
        choice_parameter_links: parse_metadata_child_choice_parameter_links(
            fields.get(header_index + 15).copied(),
            object_refs,
        ),
        choice_parameters: parse_metadata_child_choice_parameters(
            fields.get(header_index + 16).copied(),
            object_refs,
        ),
        self_close_empty_choice_parameter_refs: false,
        quick_choice: fields
            .get(header_index + 17)
            .and_then(|field| metadata_quick_choice_xml(field.trim())),
        create_on_input: fields
            .get(header_index + 18)
            .and_then(|field| metadata_create_on_input_xml(field.trim())),
        choice_form: parse_metadata_child_choice_form(
            fields.get(header_index + 19).copied(),
            object_refs,
        ),
        link_by_type_empty: metadata_child_collection_is_empty(
            fields.get(header_index + 20).copied(),
        ),
        link_by_type: None,
        choice_history_on_input: fields
            .get(header_index + 21)
            .and_then(|field| metadata_choice_history_on_input_xml(field.trim())),
        master: None,
        main_filter: None,
        balance: None,
        accounting_flag: None,
        ext_dimension_accounting_flag: None,
        deny_incomplete_values: None,
        use_mode: if owner_kind == "Catalog" {
            fields
                .get(header_index + 22)
                .and_then(|field| metadata_attribute_use_mode_xml(field.trim()))
        } else {
            None
        },
        indexing: if matches!(owner_kind, "Catalog" | "Document") {
            fields
                .get(header_index + 23)
                .and_then(|field| metadata_attribute_indexing_xml(field.trim()))
        } else {
            None
        },
        full_text_search: if matches!(owner_kind, "Catalog" | "Document") {
            fields
                .get(header_index + 24)
                .and_then(|field| metadata_attribute_full_text_search_xml(field.trim()))
        } else {
            None
        },
        data_history: fields
            .get(header_index + 25)
            .and_then(|field| metadata_data_history_xml(field.trim())),
        type_reduction_mode: None,
        update_data_history_immediately_after_write: if matches!(owner_kind, "Catalog" | "Document")
        {
            fields
                .get(header_index + 26)
                .and_then(|field| parse_1c_bool_flag(field.trim()))
        } else {
            None
        },
        execute_after_write_data_history_version_processing: if matches!(
            owner_kind,
            "Catalog" | "Document"
        ) {
            fields
                .get(header_index + 27)
                .and_then(|field| parse_1c_bool_flag(field.trim()))
        } else {
            None
        },
    })
}

fn flatten_data_processor_wrapped_child_fields<'a>(fields: &[&'a str]) -> Option<Vec<&'a str>> {
    match fields.first().map(|field| field.trim()) {
        Some("2") => Some(fields.to_vec()),
        Some("27") => {
            let nested = split_1c_braced_fields(fields.get(1)?.trim(), 0)?;
            if nested.first().map(|field| field.trim()) != Some("2") {
                return None;
            }
            let mut flattened = nested;
            flattened.extend(fields.iter().skip(2).copied());
            Some(flattened)
        }
        Some("0") => {
            let nested = split_1c_braced_fields(fields.get(1)?.trim(), 0)?;
            flatten_data_processor_wrapped_child_fields(&nested)
        }
        _ => None,
    }
}

fn parse_data_processor_wrapped_child_properties(
    fields: &[&str],
    header_index: usize,
    value_types: &[ConstantValueType],
    object_refs: &BTreeMap<String, String>,
) -> Option<MetadataChildProperties> {
    if fields.first().map(|field| field.trim()) != Some("2")
        || header_index != 1
        || fields.len() < 22
        || !field_starts_with(fields.get(2), r#"{"Pattern""#)
    {
        return None;
    }

    let (quick_choice, create_on_input) = data_processor_wrapped_quick_create_modes(
        fields.get(13).map(|field| field.trim()).unwrap_or("2"),
    );
    Some(MetadataChildProperties {
        password_mode: parse_1c_bool_field(fields.get(3).copied()).unwrap_or(false),
        format: parse_1c_synonyms(fields.get(4).copied().unwrap_or("{0}")),
        edit_format: parse_1c_synonyms(fields.get(19).copied().unwrap_or("{0}")),
        tooltip: parse_1c_synonyms(fields.get(5).copied().unwrap_or("{0}")),
        mark_negatives: parse_1c_bool_field(fields.get(6).copied()).unwrap_or(false),
        mask: fields
            .get(7)
            .and_then(|field| parse_1c_quoted_string(field.trim()))
            .unwrap_or_default(),
        multi_line: parse_1c_bool_field(fields.get(8).copied()).unwrap_or(false),
        extended_edit: parse_1c_bool_field(fields.get(21).copied()).unwrap_or(false),
        min_value: parse_constant_bound_value(fields.get(9).copied()),
        max_value: parse_constant_bound_value(fields.get(10).copied()),
        fill_from_filling_value: parse_1c_bool_field(fields.get(11).copied()).unwrap_or(false),
        emit_fill_from_filling_value: true,
        fill_value: parse_metadata_child_fill_value(
            fields.get(20).copied(),
            value_types,
            object_refs,
        ),
        emit_fill_value: true,
        fill_checking: match fields.get(14).map(|field| field.trim()) {
            Some("1") => "ShowError",
            _ => "DontCheck",
        },
        choice_folders_and_items: Some("Items"),
        choice_parameter_links: parse_metadata_child_choice_parameter_links(
            fields.get(15).copied(),
            object_refs,
        ),
        choice_parameters: parse_metadata_child_choice_parameters(
            fields.get(16).copied(),
            object_refs,
        ),
        self_close_empty_choice_parameter_refs: false,
        quick_choice,
        create_on_input,
        choice_form: parse_metadata_child_choice_form(fields.get(12).copied(), object_refs),
        link_by_type_empty: metadata_child_collection_is_empty(fields.get(17).copied()),
        link_by_type: None,
        choice_history_on_input: fields
            .get(18)
            .and_then(|field| metadata_choice_history_on_input_xml(field.trim()))
            .or(Some("Auto")),
        master: None,
        main_filter: None,
        balance: None,
        accounting_flag: None,
        ext_dimension_accounting_flag: None,
        deny_incomplete_values: None,
        use_mode: None,
        indexing: None,
        full_text_search: None,
        data_history: None,
        type_reduction_mode: None,
        update_data_history_immediately_after_write: None,
        execute_after_write_data_history_version_processing: None,
    })
}

fn parse_register_child_extra_properties(
    owner_kind: &str,
    tag: &str,
    text: &str,
    marker_start: usize,
    child_uuid: &str,
    object_refs: &BTreeMap<String, String>,
    mut properties: MetadataChildProperties,
) -> MetadataChildProperties {
    if owner_kind != "AccountingRegister" {
        return properties;
    }
    for fields in metadata_object_field_candidates_around_header(text, marker_start, child_uuid) {
        match (tag, fields.first().map(|field| field.trim())) {
            ("Dimension", Some("6")) if fields.len() >= 7 => {
                properties.balance = parse_1c_bool_field(fields.get(2).copied());
                properties.accounting_flag =
                    parse_metadata_object_ref(fields.get(3).copied(), object_refs);
                properties.indexing = fields
                    .get(4)
                    .and_then(|field| metadata_attribute_indexing_xml(field.trim()));
                properties.deny_incomplete_values = fields
                    .get(5)
                    .and_then(|field| parse_1c_bool_field(Some(*field)));
                properties.full_text_search = fields
                    .get(6)
                    .and_then(|field| register_child_full_text_search_xml(field.trim()));
                return properties;
            }
            ("Resource", Some("2"))
                if fields.len() >= 6 && field_starts_with(fields.get(1), "{27") =>
            {
                properties.balance = parse_1c_bool_field(fields.get(2).copied());
                properties.accounting_flag =
                    parse_metadata_object_ref(fields.get(3).copied(), object_refs);
                properties.ext_dimension_accounting_flag =
                    parse_metadata_object_ref(fields.get(4).copied(), object_refs)
                        .or_else(|| Some(String::new()));
                properties.full_text_search = fields
                    .get(5)
                    .and_then(|field| register_child_full_text_search_xml(field.trim()));
                return properties;
            }
            ("Attribute", Some("2"))
                if fields.len() >= 4 && field_starts_with(fields.get(1), "{27") =>
            {
                properties.indexing = fields
                    .get(2)
                    .and_then(|field| metadata_attribute_indexing_xml(field.trim()));
                properties.full_text_search = fields
                    .get(3)
                    .and_then(|field| register_child_full_text_search_xml(field.trim()));
                return properties;
            }
            _ => {}
        }
    }
    properties
}

fn register_child_full_text_search_xml(value: &str) -> Option<&'static str> {
    match value {
        "0" => Some("DontUse"),
        "1" => Some("Use"),
        _ => None,
    }
}

fn data_processor_wrapped_quick_create_modes(
    value: &str,
) -> (Option<&'static str>, Option<&'static str>) {
    match value {
        "0" => (Some("DontUse"), Some("Use")),
        "1" => (Some("Use"), Some("DontUse")),
        _ => (Some("Auto"), Some("Auto")),
    }
}

fn metadata_object_field_candidates_around_header<'a>(
    text: &'a str,
    marker_start: usize,
    uuid: &str,
) -> Vec<Vec<&'a str>> {
    let mut search_end = marker_start;
    let mut candidates = Vec::<(usize, Vec<&'a str>)>::new();
    while let Some(start) = text[..search_end].rfind('{') {
        search_end = start;
        let Some(end) = scan_1c_braced_value(text, start) else {
            continue;
        };
        if marker_start >= end {
            continue;
        }
        let Some(fields) = split_1c_braced_fields(text, start) else {
            continue;
        };
        if matches!(fields.first().map(|field| field.trim()), Some("1" | "3")) {
            continue;
        }
        if metadata_header_field_index(&fields, uuid).is_none() {
            continue;
        }
        candidates.push((end.saturating_sub(start), fields));
    }
    candidates.sort_by_key(|(span, _)| *span);
    candidates.into_iter().map(|(_, fields)| fields).collect()
}

fn parse_metadata_tabular_section_properties(
    owner_kind: &str,
    text: &str,
    marker_start: usize,
    child_uuid: &str,
) -> Option<MetadataTabularSectionProperties> {
    if owner_kind == "DataProcessor" {
        for fields in metadata_object_field_candidates_around_header(text, marker_start, child_uuid)
        {
            if let Some(properties) =
                parse_data_processor_tabular_section_properties_from_fields(&fields, child_uuid)
            {
                return Some(properties);
            }
        }
    }

    if let Some((_, _, fields)) =
        innermost_metadata_object_fields_around_header(text, marker_start, child_uuid)
        && let Some(header_index) = metadata_header_field_index(&fields, child_uuid)
    {
        let tooltip = parse_1c_synonyms(fields.get(header_index + 1).copied().unwrap_or("{0}"));
        let fill_checking = metadata_fill_checking_xml(fields.get(header_index + 2).copied());
        return Some(MetadataTabularSectionProperties {
            tooltip,
            fill_checking,
            line_number_fill_checking: fill_checking,
            use_mode: fields
                .get(header_index + 4)
                .and_then(|field| metadata_attribute_use_mode_xml(field.trim())),
            line_number_length: fields
                .get(header_index + 5)
                .and_then(|field| parse_1c_u32_field(Some(*field))),
        });
    }

    if owner_kind == "DataProcessor" {
        return Some(MetadataTabularSectionProperties {
            tooltip: Vec::new(),
            fill_checking: "DontCheck",
            line_number_fill_checking: "DontCheck",
            use_mode: None,
            line_number_length: None,
        });
    }

    None
}

fn parse_data_processor_tabular_section_properties_from_fields(
    fields: &[&str],
    child_uuid: &str,
) -> Option<MetadataTabularSectionProperties> {
    if fields.first().map(|field| field.trim()) != Some("11")
        || metadata_header_field_index(fields, child_uuid) != Some(5)
    {
        return None;
    }

    Some(MetadataTabularSectionProperties {
        tooltip: parse_1c_synonyms(fields.get(8).copied().unwrap_or("{0}")),
        fill_checking: metadata_fill_checking_xml(fields.get(6).copied()),
        line_number_fill_checking: "DontCheck",
        use_mode: None,
        line_number_length: None,
    })
}

fn parse_metadata_child_fill_value(
    field: Option<&str>,
    value_types: &[ConstantValueType],
    object_refs: &BTreeMap<String, String>,
) -> Option<MetadataChildFillValue> {
    let value = field?.trim();
    if matches!(value, "{0}" | "00000000-0000-0000-0000-000000000000") {
        return Some(MetadataChildFillValue::Nil);
    }
    if let Some(fields) = split_1c_braced_fields(value, 0) {
        match fields.first().map(|field| field.trim()) {
            Some(r#""U""#) => {
                return Some(MetadataChildFillValue::Nil);
            }
            Some(r#""S""#) => {
                return parse_constant_bound_value(Some(value)).map(MetadataChildFillValue::String);
            }
            Some(r#""N""#) => {
                return fields
                    .get(1)
                    .map(|field| field.trim().to_string())
                    .map(MetadataChildFillValue::Decimal);
            }
            Some(r#""D""#) => {
                return fields
                    .get(1)
                    .and_then(|field| format_1c_date_time(field.trim()))
                    .map(MetadataChildFillValue::DateTime);
            }
            Some("\"#\"") => {
                return parse_design_time_reference(value, object_refs)
                    .map(MetadataChildFillValue::DesignTimeRef);
            }
            _ => {}
        }
    }
    if value_types
        .iter()
        .any(|value_type| matches!(value_type, ConstantValueType::Boolean))
    {
        return parse_1c_bool_flag(value).map(MetadataChildFillValue::Boolean);
    }
    parse_constant_bound_value(Some(value)).map(MetadataChildFillValue::String)
}

fn metadata_fill_checking_xml(field: Option<&str>) -> &'static str {
    match field.map(str::trim) {
        Some("1") => "ShowError",
        _ => "DontCheck",
    }
}

fn metadata_child_collection_is_empty(field: Option<&str>) -> bool {
    matches!(
        field.map(str::trim),
        Some("{0}") | Some("0") | Some("{0,0}") | Some("{3,0,0}")
    )
}

fn parse_metadata_child_choice_form(
    field: Option<&str>,
    object_refs: &BTreeMap<String, String>,
) -> Option<MetadataChoiceForm> {
    let field = field?;
    if matches!(field.trim(), "0" | "00000000-0000-0000-0000-000000000000") {
        return Some(MetadataChoiceForm::Empty);
    }
    parse_design_time_reference(field, object_refs).map(MetadataChoiceForm::Reference)
}

fn metadata_choice_folders_and_items_xml(value: &str) -> Option<&'static str> {
    match value {
        "0" => Some("Items"),
        "1" => Some("Folders"),
        "2" => Some("FoldersAndItems"),
        _ => None,
    }
}

fn metadata_quick_choice_xml(value: &str) -> Option<&'static str> {
    match value {
        "0" => Some("Auto"),
        "1" => Some("Use"),
        "2" => Some("DontUse"),
        _ => None,
    }
}

fn metadata_create_on_input_xml(value: &str) -> Option<&'static str> {
    match value {
        "0" => Some("Auto"),
        "1" => Some("DontUse"),
        "2" => Some("Use"),
        _ => None,
    }
}

fn metadata_choice_history_on_input_xml(value: &str) -> Option<&'static str> {
    match value {
        "1" => Some("DontUse"),
        "0" => Some("Auto"),
        _ => None,
    }
}

fn metadata_data_history_xml(value: &str) -> Option<&'static str> {
    match value {
        "1" => Some("Use"),
        "0" => Some("DontUse"),
        _ => None,
    }
}

fn metadata_attribute_use_mode_xml(value: &str) -> Option<&'static str> {
    match value {
        "0" => Some("ForItem"),
        "1" => Some("ForFolder"),
        "2" => Some("ForFolderAndItem"),
        _ => None,
    }
}

fn metadata_attribute_indexing_xml(value: &str) -> Option<&'static str> {
    match value {
        "0" => Some("DontIndex"),
        "1" => Some("Index"),
        "2" => Some("IndexWithAdditionalOrder"),
        _ => None,
    }
}

fn metadata_attribute_full_text_search_xml(value: &str) -> Option<&'static str> {
    match value {
        "0" => Some("Use"),
        "1" => Some("DontUse"),
        _ => None,
    }
}

fn parse_metadata_type_pattern_from_child_field(
    field: &str,
    type_index: &BTreeMap<String, String>,
) -> Option<Vec<ConstantValueType>> {
    if let Some(value_types) = parse_metadata_type_pattern(field, type_index) {
        return Some(value_types);
    }
    let fields = split_1c_braced_fields(field, 0)?;
    if fields.first()?.trim() != "2" {
        return None;
    }
    fields
        .iter()
        .skip(1)
        .filter_map(|field| parse_metadata_type_pattern(field, type_index))
        .next()
}

fn attribute_tabular_section_child_object_tag(
    owner_kind: &str,
    owner_name: &str,
    owner_uuid: &str,
    text: &str,
    marker_start: usize,
    child: &MetadataHeader,
) -> Option<(&'static str, Option<MetadataHeader>)> {
    if is_offset_inside_metadata_object_code(text, marker_start, 11) {
        if let Some(tabular_section) = enclosing_metadata_header_for_code(text, marker_start, 11)
            && tabular_section.uuid != child.uuid
        {
            return Some(("Attribute", Some(tabular_section)));
        }
        return Some(("TabularSection", None));
    }
    if owner_kind == "DataProcessor"
        && is_offset_inside_metadata_object_code(text, marker_start, 27)
        && is_offset_inside_tabular_section_attribute_list(text, marker_start)
        && let Some((tabular_section, tabular_end)) =
            preceding_metadata_header_for_code_with_bounds(text, marker_start, 11)
        && tabular_section.uuid != child.uuid
        && !contains_metadata_header_uuid_between(text, tabular_end, marker_start, owner_uuid)
        && !contains_metadata_header_name_between(text, tabular_end, marker_start, owner_name)
    {
        return Some(("Attribute", Some(tabular_section)));
    }
    if owner_kind == "DataProcessor"
        && is_offset_inside_metadata_object_code(text, marker_start, 27)
    {
        return Some(("Attribute", None));
    }
    if is_offset_inside_metadata_object_code(text, marker_start, 5) {
        return Some(("Attribute", None));
    }
    if is_offset_inside_metadata_object_code(text, marker_start, 6) {
        if let Some(tabular_section) = enclosing_metadata_header_for_code(text, marker_start, 11)
            && tabular_section.uuid != child.uuid
        {
            return Some(("Attribute", Some(tabular_section)));
        }
        return Some(("Attribute", None));
    }
    if is_offset_inside_metadata_object_code(text, marker_start, 8)
        && let Some(tabular_section) = preceding_metadata_header_for_code(text, marker_start, 11)
    {
        return Some(("Attribute", Some(tabular_section)));
    }
    None
}

fn parse_catalog_properties_from_text(
    text: &str,
    uuid: &str,
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
    metadata_object_refs: &BTreeMap<String, String>,
    form_refs: &BTreeMap<String, FormSourceReference>,
    template_refs: &BTreeMap<String, TemplateSourceReference>,
) -> Option<CatalogProperties> {
    let header = parse_metadata_header_from_text(text, uuid)?;
    let fields = metadata_object_fields(text)?;
    if !matches!(fields.first().map(|value| value.trim()), Some("56" | "57")) {
        return None;
    }
    let mut generated_types = Vec::new();
    push_generated_type_entry(
        &mut generated_types,
        &fields,
        1,
        2,
        &format!("CatalogObject.{}", header.name),
        "Object",
    );
    push_generated_type_entry(
        &mut generated_types,
        &fields,
        3,
        4,
        &format!("CatalogRef.{}", header.name),
        "Ref",
    );
    push_generated_type_entry(
        &mut generated_types,
        &fields,
        5,
        6,
        &format!("CatalogSelection.{}", header.name),
        "Selection",
    );
    push_generated_type_entry(
        &mut generated_types,
        &fields,
        7,
        8,
        &format!("CatalogList.{}", header.name),
        "List",
    );
    push_generated_type_entry(
        &mut generated_types,
        &fields,
        34,
        35,
        &format!("CatalogManager.{}", header.name),
        "Manager",
    );
    let hierarchical = parse_catalog_hierarchical_flag(fields.get(9).copied()).unwrap_or(false);
    let level_count = parse_1c_u32_field(fields.get(10).copied()).unwrap_or(2);
    let folders_on_top = parse_1c_bool_field(fields.get(11).copied()).unwrap_or(true);
    let owners = parse_catalog_owners(fields.get(12).copied(), object_refs);
    let subordination_use =
        catalog_subordination_use_xml(parse_1c_u32_field(fields.get(13).copied()).unwrap_or(1));
    let check_unique = parse_1c_bool_field(fields.get(14).copied()).unwrap_or(false);
    let autonumbering = parse_1c_bool_field(fields.get(15).copied()).unwrap_or(false);
    let code_series =
        catalog_code_series_xml(parse_1c_u32_field(fields.get(16).copied()).unwrap_or(0));
    let code_length = parse_1c_u32_field(fields.get(17).copied()).unwrap_or(0);
    let code_type = catalog_code_type_xml(parse_1c_u32_field(fields.get(18).copied()).unwrap_or(1));
    let description_length = parse_1c_u32_field(fields.get(19).copied()).unwrap_or(0);
    let code_allowed_length =
        catalog_code_allowed_length_xml(parse_1c_u32_field(fields.get(20).copied()).unwrap_or(1));
    let use_standard_commands = parse_1c_bool_field(fields.get(33).copied()).unwrap_or(true);
    let include_help_in_contents = parse_1c_bool_field(fields.get(31).copied()).unwrap_or(false);

    Some(CatalogProperties {
        generated_types,
        hierarchical,
        level_count,
        folders_on_top,
        owners,
        subordination_use,
        use_standard_commands,
        code_length,
        description_length,
        code_type,
        code_allowed_length,
        code_series,
        check_unique,
        autonumbering,
        default_presentation: Some("AsDescription"),
        quick_choice: parse_1c_bool_field(fields.get(41).copied()).unwrap_or(true),
        choice_mode: enum_choice_mode_xml(parse_1c_u32_field(fields.get(40).copied()).unwrap_or(2)),
        input_by_string_fields: parse_catalog_input_by_string_fields(
            fields.get(42).copied(),
            &header.name,
        ),
        default_object_form: parse_catalog_form_ref(fields.get(21).copied(), form_refs),
        default_folder_form: parse_catalog_form_ref(fields.get(22).copied(), form_refs),
        default_list_form: parse_catalog_form_ref(fields.get(23).copied(), form_refs),
        default_choice_form: parse_catalog_form_ref(fields.get(24).copied(), form_refs),
        default_folder_choice_form: parse_catalog_form_ref(fields.get(25).copied(), form_refs),
        auxiliary_object_form: parse_catalog_form_ref(fields.get(26).copied(), form_refs),
        auxiliary_folder_form: parse_catalog_form_ref(fields.get(27).copied(), form_refs),
        auxiliary_list_form: parse_catalog_form_ref(fields.get(28).copied(), form_refs),
        auxiliary_choice_form: parse_catalog_form_ref(fields.get(29).copied(), form_refs),
        auxiliary_folder_choice_form: parse_catalog_form_ref(fields.get(30).copied(), form_refs),
        include_help_in_contents,
        object_presentation: parse_1c_synonyms(fields.get(46).copied().unwrap_or("{0}")),
        extended_object_presentation: parse_1c_synonyms(fields.get(47).copied().unwrap_or("{0}")),
        list_presentation: parse_1c_synonyms(fields.get(48).copied().unwrap_or("{0}")),
        extended_list_presentation: parse_1c_synonyms(fields.get(49).copied().unwrap_or("{0}")),
        explanation: parse_1c_synonyms(fields.get(50).copied().unwrap_or("{0}")),
        create_on_input: fields
            .get(51)
            .and_then(|field| metadata_create_on_input_xml(field.trim()))
            .unwrap_or("DontUse"),
        choice_history_on_input: fields
            .get(52)
            .and_then(|field| metadata_choice_history_on_input_xml(field.trim()))
            .unwrap_or("Auto"),
        data_history: fields
            .get(53)
            .and_then(|field| metadata_data_history_xml(field.trim()))
            .unwrap_or("DontUse"),
        update_data_history_immediately_after_write: fields
            .get(54)
            .and_then(|field| parse_1c_bool_flag(field.trim()))
            .unwrap_or(false),
        execute_after_write_data_history_version_processing: fields
            .get(55)
            .and_then(|field| parse_1c_bool_flag(field.trim()))
            .unwrap_or(false),
        standard_attribute_details: parse_catalog_standard_attribute_details(
            fields.get(45).copied(),
        ),
        child_metadata_objects: parse_attribute_tabular_section_child_objects(
            "Catalog",
            &header.name,
            text,
            uuid,
            Some(catalog_direct_attribute_wrapper_code(
                fields.first()?.trim(),
            )?),
            type_index,
            object_refs,
            metadata_object_refs,
            form_refs,
        ),
        child_forms: owned_catalog_form_names_in_text_order(text, &header.name, form_refs),
        child_templates: owned_catalog_template_names_in_text_order(
            text,
            &header.name,
            template_refs,
        ),
    })
}

fn parse_report_properties_from_text(
    text: &str,
    uuid: &str,
    type_index: &BTreeMap<String, String>,
    form_refs: &BTreeMap<String, FormSourceReference>,
    template_refs: &BTreeMap<String, TemplateSourceReference>,
    object_refs: &BTreeMap<String, String>,
) -> Option<ReportProperties> {
    let header = parse_metadata_header_from_text(text, uuid)?;
    let fields = metadata_object_fields(text)?;
    if !matches!(fields.first().map(|value| value.trim()), Some("19" | "20")) {
        return None;
    }

    let mut generated_types = Vec::new();
    push_generated_type_entry(
        &mut generated_types,
        &fields,
        1,
        2,
        &format!("ReportObject.{}", header.name),
        "Object",
    );
    push_generated_type_entry(
        &mut generated_types,
        &fields,
        12,
        13,
        &format!("ReportManager.{}", header.name),
        "Manager",
    );

    Some(ReportProperties {
        generated_types,
        use_standard_commands: parse_1c_bool_field(fields.get(7).copied()).unwrap_or(true),
        default_form: parse_catalog_form_ref(fields.get(4).copied(), form_refs),
        main_data_composition_schema: parse_metadata_template_ref(
            fields.get(5).copied(),
            template_refs,
        ),
        default_settings_form: parse_catalog_form_ref(fields.get(6).copied(), form_refs),
        default_variant_form: parse_catalog_form_ref(fields.get(10).copied(), form_refs),
        variants_storage: parse_metadata_object_ref(fields.get(8).copied(), object_refs),
        settings_storage: parse_metadata_object_ref(fields.get(9).copied(), object_refs),
        include_help_in_contents: parse_1c_bool_field(fields.get(11).copied()).unwrap_or(false),
        extended_presentation: parse_1c_synonyms(fields.get(15).copied().unwrap_or("{0}")),
        explanation: parse_1c_synonyms(fields.get(16).copied().unwrap_or("{0}")),
        child_metadata_objects: parse_attribute_tabular_section_child_objects(
            "Report",
            &header.name,
            text,
            uuid,
            None,
            type_index,
            object_refs,
            &BTreeMap::new(),
            form_refs,
        ),
        child_forms: owned_report_form_names_in_text_order(text, &header.name, form_refs),
        child_templates: parse_report_child_templates_from_text(text, template_refs),
        child_commands: nested_child_commands_from_text(text, uuid, type_index, object_refs),
    })
}

fn parse_document_properties_from_text(
    text: &str,
    uuid: &str,
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
    metadata_object_refs: &BTreeMap<String, String>,
    form_refs: &BTreeMap<String, FormSourceReference>,
    template_refs: &BTreeMap<String, TemplateSourceReference>,
) -> Option<DocumentProperties> {
    let header = parse_metadata_header_from_text(text, uuid)?;
    let fields = metadata_object_fields(text)?;
    if fields.first().map(|value| value.trim()) != Some("40") {
        return None;
    }

    let mut generated_types = Vec::new();
    push_generated_type_entry(
        &mut generated_types,
        &fields,
        1,
        2,
        &format!("DocumentObject.{}", header.name),
        "Object",
    );
    push_generated_type_entry(
        &mut generated_types,
        &fields,
        3,
        4,
        &format!("DocumentRef.{}", header.name),
        "Ref",
    );
    push_generated_type_entry(
        &mut generated_types,
        &fields,
        26,
        27,
        &format!("DocumentManager.{}", header.name),
        "Manager",
    );

    let header_index = metadata_header_field_index(&fields, uuid)?;
    Some(DocumentProperties {
        generated_types,
        use_standard_commands: parse_1c_bool_field(fields.get(header_index + 1).copied())
            .unwrap_or(true),
        numbering: parse_document_numbering_properties(&fields, header_index, object_refs),
        standard_attributes: parse_document_standard_attributes(&fields, uuid),
        default_object_form: parse_catalog_form_ref(
            fields.get(header_index + 14).copied(),
            form_refs,
        ),
        default_list_form: parse_default_list_form_ref(
            &fields,
            &[header_index + 15],
            form_refs,
            "Documents",
            &header.name,
        ),
        default_choice_form: parse_catalog_form_ref(
            fields.get(header_index + 16).copied(),
            form_refs,
        ),
        auxiliary_object_form: parse_catalog_form_ref(
            fields.get(header_index + 17).copied(),
            form_refs,
        ),
        auxiliary_list_form: parse_catalog_form_ref(
            fields.get(header_index + 18).copied(),
            form_refs,
        ),
        auxiliary_choice_form: parse_catalog_form_ref(
            fields.get(header_index + 19).copied(),
            form_refs,
        ),
        include_help_in_contents: parse_1c_bool_field(fields.get(header_index + 20).copied())
            .unwrap_or(false),
        child_metadata_objects: parse_attribute_tabular_section_child_objects(
            "Document",
            &header.name,
            text,
            uuid,
            None,
            type_index,
            object_refs,
            metadata_object_refs,
            form_refs,
        ),
        child_forms: owned_document_form_names_in_text_order(text, &header.name, form_refs),
        child_templates: owned_document_template_names_in_text_order(
            text,
            &header.name,
            template_refs,
        ),
    })
}

fn parse_business_process_properties_from_text(
    text: &str,
    uuid: &str,
    form_refs: &BTreeMap<String, FormSourceReference>,
) -> Option<BusinessProcessProperties> {
    let header = parse_metadata_header_from_text(text, uuid)?;
    let fields = metadata_object_fields(text)?;
    if fields.first().map(|value| value.trim()) != Some("30") {
        return None;
    }
    let header_index = metadata_header_field_index(&fields, uuid)?;

    let mut generated_types = Vec::new();
    push_generated_type_entry(
        &mut generated_types,
        &fields,
        3,
        4,
        &format!("BusinessProcessObject.{}", header.name),
        "Object",
    );
    push_generated_type_entry(
        &mut generated_types,
        &fields,
        5,
        6,
        &format!("BusinessProcessRef.{}", header.name),
        "Ref",
    );
    push_generated_type_entry(
        &mut generated_types,
        &fields,
        7,
        8,
        &format!("BusinessProcessSelection.{}", header.name),
        "Selection",
    );
    push_generated_type_entry(
        &mut generated_types,
        &fields,
        9,
        10,
        &format!("BusinessProcessList.{}", header.name),
        "List",
    );
    push_generated_type_entry(
        &mut generated_types,
        &fields,
        11,
        12,
        &format!("BusinessProcessManager.{}", header.name),
        "Manager",
    );
    push_generated_type_entry(
        &mut generated_types,
        &fields,
        13,
        14,
        &format!("BusinessProcessRoutePointRef.{}", header.name),
        "RoutePointRef",
    );

    Some(BusinessProcessProperties {
        generated_types,
        use_standard_commands: parse_1c_bool_field(fields.get(header_index + 1).copied())
            .unwrap_or(true),
        default_list_form: parse_default_list_form_ref(
            &fields,
            &[header_index + 9],
            form_refs,
            "BusinessProcesses",
            &header.name,
        ),
    })
}

fn parse_task_properties_from_text(
    text: &str,
    uuid: &str,
    form_refs: &BTreeMap<String, FormSourceReference>,
) -> Option<TaskProperties> {
    let header = parse_metadata_header_from_text(text, uuid)?;
    let fields = metadata_object_fields(text)?;
    if fields.first().map(|value| value.trim()) != Some("33")
        || metadata_header_field_index(&fields, uuid) != Some(1)
    {
        return None;
    }

    let mut generated_types = Vec::new();
    push_generated_type_entry(
        &mut generated_types,
        &fields,
        3,
        4,
        &format!("TaskObject.{}", header.name),
        "Object",
    );
    push_generated_type_entry(
        &mut generated_types,
        &fields,
        5,
        6,
        &format!("TaskRef.{}", header.name),
        "Ref",
    );
    push_generated_type_entry(
        &mut generated_types,
        &fields,
        7,
        8,
        &format!("TaskSelection.{}", header.name),
        "Selection",
    );
    push_generated_type_entry(
        &mut generated_types,
        &fields,
        9,
        10,
        &format!("TaskList.{}", header.name),
        "List",
    );
    push_generated_type_entry(
        &mut generated_types,
        &fields,
        11,
        12,
        &format!("TaskManager.{}", header.name),
        "Manager",
    );

    let header_index = metadata_header_field_index(&fields, uuid)?;
    Some(TaskProperties {
        generated_types,
        use_standard_commands: parse_1c_bool_field(fields.get(header_index + 1).copied())
            .unwrap_or(true),
        default_list_form: parse_default_list_form_ref(
            &fields,
            &[header_index + 9],
            form_refs,
            "Tasks",
            &header.name,
        ),
    })
}

fn parse_settings_storage_properties_from_text(
    text: &str,
    uuid: &str,
) -> Option<SettingsStorageProperties> {
    let header = parse_metadata_header_from_text(text, uuid)?;
    let fields = metadata_object_fields(text)?;
    if fields.first().map(|value| value.trim()) != Some("2")
        || metadata_header_field_index(&fields, uuid) != Some(1)
        || !field_starts_with(fields.get(1), "{0,")
    {
        return None;
    }

    let mut generated_types = Vec::new();
    push_generated_type_entry(
        &mut generated_types,
        &fields,
        2,
        3,
        &format!("SettingsStorageManager.{}", header.name),
        "Manager",
    );

    Some(SettingsStorageProperties { generated_types })
}

fn parse_data_processor_properties_from_text(
    text: &str,
    uuid: &str,
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
    form_refs: &BTreeMap<String, FormSourceReference>,
    template_refs: &BTreeMap<String, TemplateSourceReference>,
) -> Option<DataProcessorProperties> {
    let header = parse_metadata_header_from_text(text, uuid)?;
    let fields = metadata_object_fields(text)?;
    if fields.first().map(|value| value.trim()) != Some("17") {
        return None;
    }

    let mut generated_types = Vec::new();
    push_generated_type_entry(
        &mut generated_types,
        &fields,
        1,
        2,
        &format!("DataProcessorObject.{}", header.name),
        "Object",
    );
    push_generated_type_entry(
        &mut generated_types,
        &fields,
        7,
        8,
        &format!("DataProcessorManager.{}", header.name),
        "Manager",
    );

    Some(DataProcessorProperties {
        generated_types,
        use_standard_commands: parse_1c_bool_field(fields.get(5).copied()).unwrap_or(true),
        default_form: parse_catalog_form_ref(fields.get(4).copied(), form_refs),
        auxiliary_form: parse_catalog_form_ref(fields.get(9).copied(), form_refs),
        include_help_in_contents: parse_1c_bool_field(fields.get(6).copied()).unwrap_or(false),
        extended_presentation: parse_1c_synonyms(fields.get(10).copied().unwrap_or("{0}")),
        explanation: parse_1c_synonyms(fields.get(11).copied().unwrap_or("{0}")),
        child_metadata_objects: parse_attribute_tabular_section_child_objects(
            "DataProcessor",
            &header.name,
            text,
            uuid,
            None,
            type_index,
            object_refs,
            &BTreeMap::new(),
            form_refs,
        ),
        child_forms: owned_data_processor_form_names_in_text_order(text, &header.name, form_refs),
        child_templates: owned_data_processor_template_names_in_text_order(
            text,
            &header.name,
            template_refs,
        ),
        child_commands: nested_child_commands_from_text(text, uuid, type_index, object_refs),
    })
}

fn parse_enum_properties_from_text(
    text: &str,
    uuid: &str,
    form_refs: &BTreeMap<String, FormSourceReference>,
    template_refs: &BTreeMap<String, TemplateSourceReference>,
) -> Option<EnumProperties> {
    let header = parse_metadata_header_from_text(text, uuid)?;
    let fields = metadata_object_fields(text)?;
    if fields.first().map(|value| value.trim()) != Some("20") {
        return None;
    }
    let mut generated_types = Vec::new();
    push_generated_type_entry(
        &mut generated_types,
        &fields,
        1,
        2,
        &format!("EnumRef.{}", header.name),
        "Ref",
    );
    push_generated_type_entry(
        &mut generated_types,
        &fields,
        3,
        4,
        &format!("EnumManager.{}", header.name),
        "Manager",
    );
    push_generated_type_entry(
        &mut generated_types,
        &fields,
        7,
        8,
        &format!("EnumList.{}", header.name),
        "List",
    );

    let values = parse_enum_values_from_text(text);

    Some(EnumProperties {
        generated_types,
        use_standard_commands: parse_1c_bool_field(fields.get(6).copied()).unwrap_or(true),
        has_standard_attributes: braced_field_has_entries(fields.get(18).copied()),
        quick_choice: parse_1c_bool_field(fields.get(12).copied()).unwrap_or(true),
        choice_mode: enum_choice_mode_xml(parse_1c_u32_field(fields.get(11).copied()).unwrap_or(2)),
        choice_history_on_input: fields
            .last()
            .and_then(|field| metadata_choice_history_on_input_xml(field.trim()))
            .unwrap_or("Auto"),
        default_list_form: parse_catalog_form_ref(fields.get(9).copied(), form_refs),
        default_choice_form: parse_catalog_form_ref(fields.get(10).copied(), form_refs),
        auxiliary_list_form: parse_catalog_form_ref(fields.get(13).copied(), form_refs),
        auxiliary_choice_form: parse_catalog_form_ref(fields.get(14).copied(), form_refs),
        list_presentation: parse_1c_synonyms(fields.get(15).copied().unwrap_or("{0}")),
        extended_list_presentation: parse_1c_synonyms(fields.get(16).copied().unwrap_or("{0}")),
        explanation: parse_1c_synonyms(fields.get(17).copied().unwrap_or("{0}")),
        values,
        child_forms: owned_enum_form_names_in_text_order(text, &header.name, form_refs),
        child_templates: owned_enum_template_names_in_text_order(text, &header.name, template_refs),
    })
}

fn parse_enum_values_from_text(text: &str) -> Vec<MetadataHeader> {
    let Some(root_fields) = split_1c_braced_fields(text.trim_start_matches('\u{feff}'), 0) else {
        return Vec::new();
    };
    root_fields
        .iter()
        .rev()
        .find_map(|field| parse_enum_values(field))
        .unwrap_or_default()
}

fn parse_enum_values(text: &str) -> Option<Vec<MetadataHeader>> {
    let fields = split_1c_braced_fields(text, 0)?;
    let count = fields.get(1)?.trim().parse::<usize>().ok()?;
    if count == 0 || fields.len() < count + 2 {
        return None;
    }
    let values = fields
        .iter()
        .skip(2)
        .take(count)
        .filter_map(|field| parse_enum_value_header(field))
        .collect::<Vec<_>>();
    if values.len() == count {
        Some(values)
    } else {
        None
    }
}

fn parse_enum_value_header(text: &str) -> Option<MetadataHeader> {
    let marker = "{1,0,";
    let uuid_start = text.find(marker)? + marker.len();
    let uuid_end = uuid_start + 36;
    let uuid = text.get(uuid_start..uuid_end)?;
    if !is_uuid_text(uuid) {
        return None;
    }
    parse_metadata_header_from_text(text, uuid)
}

fn owned_enum_form_names_in_text_order(
    text: &str,
    enum_name: &str,
    form_refs: &BTreeMap<String, FormSourceReference>,
) -> Vec<String> {
    let mut names = Vec::new();
    let mut seen = BTreeSet::new();
    let known_form_uuids = form_refs.keys().cloned().collect::<BTreeSet<_>>();
    if let Some(form_uuids) = owned_form_uuid_values_matching_in_text_order(text, &known_form_uuids)
    {
        for uuid in form_uuids {
            let Some(form_ref) = form_refs.get(&uuid) else {
                continue;
            };
            if form_ref.kind != "Form"
                || !is_owned_metadata_child_path(
                    &form_ref.relative_path,
                    "Enums",
                    enum_name,
                    "Forms",
                )
            {
                continue;
            }
            let Some(name) = source_path_file_stem(&form_ref.relative_path) else {
                continue;
            };
            if seen.insert(name.clone()) {
                names.push(name);
            }
        }
    }

    let mut path_names = form_refs
        .values()
        .filter(|form_ref| {
            form_ref.kind == "Form"
                && is_owned_metadata_child_path(
                    &form_ref.relative_path,
                    "Enums",
                    enum_name,
                    "Forms",
                )
        })
        .filter_map(|form_ref| {
            source_path_file_stem(&form_ref.relative_path)
                .map(|name| (form_ref.relative_path.clone(), name))
        })
        .collect::<Vec<_>>();
    path_names.sort_by(|(left_path, _), (right_path, _)| left_path.cmp(right_path));
    for (_, name) in path_names {
        if seen.insert(name.clone()) {
            names.push(name);
        }
    }

    names
}

fn owned_enum_template_names_in_text_order(
    text: &str,
    enum_name: &str,
    template_refs: &BTreeMap<String, TemplateSourceReference>,
) -> Vec<String> {
    owned_metadata_template_names_in_text_order(text, "Enums", enum_name, template_refs)
}

fn is_owned_metadata_child_path(
    path: &Path,
    owner_folder: &str,
    owner_name: &str,
    child_folder: &str,
) -> bool {
    let parts = path
        .iter()
        .filter_map(|part| part.to_str())
        .collect::<Vec<_>>();
    parts.len() == 4
        && parts.first() == Some(&owner_folder)
        && parts.get(1) == Some(&owner_name)
        && parts.get(2) == Some(&child_folder)
}

fn source_path_file_stem(path: &Path) -> Option<String> {
    path.file_stem()?.to_str().map(ToString::to_string)
}

fn push_generated_type_entry(
    entries: &mut Vec<GeneratedTypeEntry>,
    fields: &[&str],
    type_index: usize,
    value_index: usize,
    name: &str,
    category: &'static str,
) {
    let Some(type_id) = fields.get(type_index).copied().and_then(parse_uuid_field) else {
        return;
    };
    let Some(value_id) = fields.get(value_index).copied().and_then(parse_uuid_field) else {
        return;
    };
    entries.push(GeneratedTypeEntry {
        name: name.to_string(),
        category,
        type_id,
        value_id,
    });
}

fn parse_catalog_hierarchical_flag(header_field: Option<&str>) -> Option<bool> {
    let outer = split_1c_braced_fields(header_field?, 0)?;
    let header = split_1c_braced_fields(outer.get(1)?, 0)?;
    parse_1c_bool_field(header.get(5).copied())
}

fn parse_catalog_owners(
    field: Option<&str>,
    object_refs: &BTreeMap<String, String>,
) -> Option<Vec<String>> {
    let field = field?;
    if field
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>()
        == "{0,0}"
    {
        return Some(Vec::new());
    }

    let mut owners = Vec::new();
    let mut seen = BTreeSet::new();
    collect_catalog_owner_refs(field, object_refs, &mut seen, &mut owners);
    (!owners.is_empty()).then_some(owners)
}

fn collect_catalog_owner_refs(
    field: &str,
    object_refs: &BTreeMap<String, String>,
    seen: &mut BTreeSet<String>,
    owners: &mut Vec<String>,
) {
    if let Some(uuid) = parse_non_zero_uuid(field.trim())
        && seen.insert(uuid.clone())
        && let Some(reference) = object_refs.get(&uuid)
    {
        owners.push(reference.clone());
    }

    let Some(fields) = split_1c_braced_fields(field, 0) else {
        return;
    };
    for nested in fields {
        collect_catalog_owner_refs(nested, object_refs, seen, owners);
    }
}

fn parse_catalog_form_ref(
    field: Option<&str>,
    form_refs: &BTreeMap<String, FormSourceReference>,
) -> Option<String> {
    let uuid = parse_non_zero_uuid(field?)?;
    form_refs.get(&uuid).and_then(form_source_reference_name)
}

fn parse_default_list_form_ref(
    fields: &[&str],
    candidate_indexes: &[usize],
    form_refs: &BTreeMap<String, FormSourceReference>,
    owner_folder: &str,
    owner_name: &str,
) -> Option<String> {
    candidate_indexes
        .iter()
        .filter_map(|index| fields.get(*index).copied())
        .find_map(|field| parse_catalog_form_ref(Some(field), form_refs))
        .or_else(|| owned_default_list_form_ref(form_refs, owner_folder, owner_name))
}

fn owned_default_list_form_ref(
    form_refs: &BTreeMap<String, FormSourceReference>,
    owner_folder: &str,
    owner_name: &str,
) -> Option<String> {
    const DEFAULT_LIST_FORM_NAMES: [&str; 3] = [
        "\u{424}\u{43e}\u{440}\u{43c}\u{430}\u{421}\u{43f}\u{438}\u{441}\u{43a}\u{430}",
        "ListForm",
        "List",
    ];

    for preferred_name in DEFAULT_LIST_FORM_NAMES {
        let mut matches = form_refs
            .values()
            .filter(|form_ref| {
                form_ref.kind == "Form"
                    && is_owned_metadata_child_path(
                        &form_ref.relative_path,
                        owner_folder,
                        owner_name,
                        "Forms",
                    )
            })
            .filter_map(|form_ref| {
                let name = source_path_file_stem(&form_ref.relative_path)?;
                (name == preferred_name).then_some(form_ref)
            })
            .collect::<Vec<_>>();
        matches.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        if let Some(form_ref) = matches.first() {
            return form_source_reference_name(form_ref);
        }
    }

    None
}

fn metadata_source_folder_for_kind(kind: &str) -> Option<&'static str> {
    match kind {
        "Document" => Some("Documents"),
        "InformationRegister" => Some("InformationRegisters"),
        "AccumulationRegister" => Some("AccumulationRegisters"),
        "AccountingRegister" => Some("AccountingRegisters"),
        "CalculationRegister" => Some("CalculationRegisters"),
        "BusinessProcess" => Some("BusinessProcesses"),
        "Task" => Some("Tasks"),
        "ExchangePlan" => Some("ExchangePlans"),
        "ChartOfAccounts" => Some("ChartsOfAccounts"),
        "ChartOfCalculationTypes" => Some("ChartsOfCalculationTypes"),
        "ChartOfCharacteristicTypes" => Some("ChartsOfCharacteristicTypes"),
        _ => None,
    }
}

fn parse_metadata_template_ref(
    field: Option<&str>,
    template_refs: &BTreeMap<String, TemplateSourceReference>,
) -> Option<String> {
    let uuid = parse_non_zero_uuid(field?)?;
    template_refs
        .get(&uuid)
        .and_then(template_source_reference_name)
}

fn parse_metadata_object_ref(
    field: Option<&str>,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    let uuid = parse_non_zero_uuid(field?)?;
    object_refs.get(&uuid).cloned()
}

fn parse_catalog_input_by_string_fields(field: Option<&str>, catalog_name: &str) -> Vec<String> {
    let Some(field) = field else {
        return Vec::new();
    };
    let mut values = Vec::new();
    for code in ["-3", "-2"] {
        let marker = format!("{{{code}}}");
        if field.contains(&marker) {
            if let Some(name) = catalog_standard_attribute_name(code) {
                values.push(format!("Catalog.{catalog_name}.StandardAttribute.{name}"));
            }
        }
    }
    values
}

const CATALOG_STANDARD_ATTRIBUTE_TOOLTIP_PROPERTY_UUID: &str =
    "4690ff70-e3fa-4914-9127-6a9acc5fc949";
const CATALOG_STANDARD_ATTRIBUTE_SYNONYM_PROPERTY_UUID: &str =
    "cf4abea3-37b2-11d4-940f-008048da11f9";

fn parse_catalog_standard_attribute_details(
    field: Option<&str>,
) -> BTreeMap<&'static str, CatalogStandardAttributeDetails> {
    parse_standard_attribute_details(field, catalog_standard_attribute_name)
}

fn parse_standard_attribute_details(
    field: Option<&str>,
    name_by_code: fn(&str) -> Option<&'static str>,
) -> BTreeMap<&'static str, CatalogStandardAttributeDetails> {
    let mut details = BTreeMap::new();
    if let Some(field) = field {
        parse_standard_attribute_details_from_field(field, &mut details, name_by_code);
    }
    details
}

fn parse_standard_attribute_details_from_field(
    field: &str,
    details: &mut BTreeMap<&'static str, CatalogStandardAttributeDetails>,
    name_by_code: fn(&str) -> Option<&'static str>,
) {
    let Some(fields) = split_1c_braced_fields(field, 0) else {
        return;
    };

    for index in 0..fields.len().saturating_sub(2) {
        let Some(code_fields) = split_1c_braced_fields(fields[index], 0) else {
            continue;
        };
        let Some(name) = code_fields
            .first()
            .and_then(|code| name_by_code(code.trim()))
        else {
            continue;
        };
        let detail = parse_standard_attribute_detail(fields[index + 2]);
        if !detail.tooltip.is_empty() || !detail.synonym.is_empty() {
            details.insert(name, detail);
        }
    }

    for nested in fields {
        if nested.starts_with('{') {
            parse_standard_attribute_details_from_field(nested, details, name_by_code);
        }
    }
}

fn parse_standard_attribute_detail(field: &str) -> CatalogStandardAttributeDetails {
    let mut detail = CatalogStandardAttributeDetails::default();
    let Some(fields) = split_1c_braced_fields(field, 0) else {
        return detail;
    };

    for pair in fields.get(2..).unwrap_or_default().chunks(2) {
        let [key, value] = pair else {
            continue;
        };
        match key.trim() {
            CATALOG_STANDARD_ATTRIBUTE_TOOLTIP_PROPERTY_UUID => {
                detail.tooltip = parse_wrapped_1c_synonyms(value);
            }
            CATALOG_STANDARD_ATTRIBUTE_SYNONYM_PROPERTY_UUID => {
                detail.synonym = parse_wrapped_1c_synonyms(value);
            }
            _ => {}
        }
    }

    detail
}

fn parse_wrapped_1c_synonyms(value: &str) -> Vec<(String, String)> {
    split_1c_braced_fields(value, 0)
        .and_then(|fields| fields.get(2).map(|field| parse_1c_synonyms(field)))
        .unwrap_or_else(|| parse_1c_synonyms(value))
}

fn parse_report_child_templates_from_text(
    text: &str,
    template_refs: &BTreeMap<String, TemplateSourceReference>,
) -> Vec<String> {
    let Some(root_fields) = split_1c_braced_fields(text, 0) else {
        return Vec::new();
    };
    let mut templates = Vec::new();
    let mut seen = BTreeSet::new();
    for field in root_fields.iter().skip(3) {
        let Some(child_fields) = split_1c_braced_fields(field, 0) else {
            continue;
        };
        let Some(uuid) = child_fields
            .get(2)
            .and_then(|value| parse_non_zero_uuid(value))
        else {
            continue;
        };
        let Some(template_ref) = template_refs.get(&uuid) else {
            continue;
        };
        let Some(name) = source_path_file_stem(&template_ref.relative_path) else {
            continue;
        };
        if seen.insert(name.clone()) {
            templates.push(name);
        }
    }
    templates
}

fn owned_report_form_names_in_text_order(
    text: &str,
    report_name: &str,
    form_refs: &BTreeMap<String, FormSourceReference>,
) -> Vec<String> {
    owned_metadata_form_names_in_text_order(text, "Reports", report_name, form_refs)
}

fn owned_document_form_names_in_text_order(
    text: &str,
    document_name: &str,
    form_refs: &BTreeMap<String, FormSourceReference>,
) -> Vec<String> {
    owned_metadata_form_names_in_text_order(text, "Documents", document_name, form_refs)
}

fn owned_document_template_names_in_text_order(
    text: &str,
    document_name: &str,
    template_refs: &BTreeMap<String, TemplateSourceReference>,
) -> Vec<String> {
    owned_metadata_template_names_in_text_order(text, "Documents", document_name, template_refs)
}

fn owned_catalog_form_names_in_text_order(
    text: &str,
    catalog_name: &str,
    form_refs: &BTreeMap<String, FormSourceReference>,
) -> Vec<String> {
    owned_metadata_form_names_in_text_order(text, "Catalogs", catalog_name, form_refs)
}

fn owned_catalog_template_names_in_text_order(
    text: &str,
    catalog_name: &str,
    template_refs: &BTreeMap<String, TemplateSourceReference>,
) -> Vec<String> {
    owned_metadata_template_names_in_text_order(text, "Catalogs", catalog_name, template_refs)
}

fn owned_data_processor_form_names_in_text_order(
    text: &str,
    data_processor_name: &str,
    form_refs: &BTreeMap<String, FormSourceReference>,
) -> Vec<String> {
    owned_metadata_form_names_in_text_order(text, "DataProcessors", data_processor_name, form_refs)
}

fn owned_data_processor_template_names_in_text_order(
    text: &str,
    data_processor_name: &str,
    template_refs: &BTreeMap<String, TemplateSourceReference>,
) -> Vec<String> {
    owned_metadata_template_names_in_text_order(
        text,
        "DataProcessors",
        data_processor_name,
        template_refs,
    )
}

fn owned_metadata_template_names_in_text_order(
    text: &str,
    owner_folder: &str,
    owner_name: &str,
    template_refs: &BTreeMap<String, TemplateSourceReference>,
) -> Vec<String> {
    let mut names = Vec::new();
    let mut seen = BTreeSet::new();
    for uuid in uuid_like_values_in_text_order(text) {
        let Some(template_ref) = template_refs.get(&uuid) else {
            continue;
        };
        if template_ref.kind != "Template"
            || !is_owned_metadata_child_path(
                &template_ref.relative_path,
                owner_folder,
                owner_name,
                "Templates",
            )
        {
            continue;
        }
        let Some(name) = source_path_file_stem(&template_ref.relative_path) else {
            continue;
        };
        if seen.insert(name.clone()) {
            names.push(name);
        }
    }

    let mut path_names = template_refs
        .values()
        .filter(|template_ref| {
            template_ref.kind == "Template"
                && is_owned_metadata_child_path(
                    &template_ref.relative_path,
                    owner_folder,
                    owner_name,
                    "Templates",
                )
        })
        .filter_map(|template_ref| {
            source_path_file_stem(&template_ref.relative_path)
                .map(|name| (template_ref.relative_path.clone(), name))
        })
        .collect::<Vec<_>>();
    path_names.sort_by(|(left_path, _), (right_path, _)| left_path.cmp(right_path));
    for (_, name) in path_names {
        if seen.insert(name.clone()) {
            names.push(name);
        }
    }

    names
}

fn catalog_standard_attribute_name(code: &str) -> Option<&'static str> {
    match code {
        "-13" => Some("PredefinedDataName"),
        "-10" => Some("Predefined"),
        "-8" => Some("Ref"),
        "-7" => Some("DeletionMark"),
        "-6" => Some("IsFolder"),
        "-5" => Some("Owner"),
        "-4" => Some("Parent"),
        "-3" => Some("Description"),
        "-2" => Some("Code"),
        _ => None,
    }
}

fn document_standard_attribute_name(code: &str) -> Option<&'static str> {
    match code {
        "-7" => Some("Posted"),
        "-5" => Some("Ref"),
        "-4" => Some("DeletionMark"),
        "-3" => Some("Date"),
        "-2" => Some("Number"),
        _ => None,
    }
}

fn parse_1c_bool_field(value: Option<&str>) -> Option<bool> {
    parse_1c_bool_flag(value?.trim())
}

fn braced_field_has_entries(field: Option<&str>) -> bool {
    split_1c_braced_fields(field.unwrap_or("{0}"), 0)
        .and_then(|fields| fields.first().map(|value| value.trim() != "0"))
        .unwrap_or(false)
}

fn parse_1c_u32_field(value: Option<&str>) -> Option<u32> {
    value?.trim().parse().ok()
}

fn catalog_subordination_use_xml(value: u32) -> Option<&'static str> {
    match value {
        0 => Some("ToFolders"),
        1 => Some("ToItems"),
        2 => Some("ToFoldersAndItems"),
        _ => None,
    }
}

fn catalog_code_type_xml(value: u32) -> Option<&'static str> {
    match value {
        0 => Some("Number"),
        1 => Some("String"),
        _ => None,
    }
}

fn catalog_code_allowed_length_xml(value: u32) -> Option<&'static str> {
    match value {
        0 => Some("Fixed"),
        1 => Some("Variable"),
        _ => None,
    }
}

fn catalog_code_series_xml(value: u32) -> Option<&'static str> {
    match value {
        0 => Some("WholeCatalog"),
        1 => Some("WithinOwner"),
        _ => None,
    }
}

fn enum_choice_mode_xml(value: u32) -> &'static str {
    match value {
        0 => "FromList",
        1 => "QuickChoice",
        2 => "BothWays",
        _ => "BothWays",
    }
}

fn parse_document_standard_attributes(
    fields: &[&str],
    uuid: &str,
) -> Option<DocumentStandardAttributes> {
    let header_index = metadata_header_field_index(fields, uuid)?;
    let number_type =
        document_number_type_xml(parse_1c_u32_field(fields.get(header_index + 3).copied())?);
    let details = parse_standard_attribute_details(
        fields.get(header_index + 23).copied(),
        document_standard_attribute_name,
    );
    Some(DocumentStandardAttributes {
        number_type,
        details,
    })
}

fn parse_document_numbering_properties(
    fields: &[&str],
    header_index: usize,
    object_refs: &BTreeMap<String, String>,
) -> Option<DocumentNumberingProperties> {
    Some(DocumentNumberingProperties {
        numerator: parse_metadata_object_ref(fields.get(header_index + 2).copied(), object_refs),
        number_type: document_number_type_xml(parse_1c_u32_field(
            fields.get(header_index + 3).copied(),
        )?),
        number_length: parse_1c_u32_field(fields.get(header_index + 4).copied())?,
        number_allowed_length: document_number_allowed_length_xml(parse_1c_u32_field(
            fields.get(header_index + 5).copied(),
        )?),
        number_periodicity: document_number_periodicity_xml(parse_1c_u32_field(
            fields.get(header_index + 6).copied(),
        )?),
        check_unique: parse_1c_bool_field(fields.get(header_index + 7).copied())?,
        autonumbering: parse_1c_bool_field(fields.get(header_index + 8).copied())?,
    })
}

fn document_number_type_xml(value: u32) -> &'static str {
    match value {
        0 => "Number",
        _ => "String",
    }
}

fn document_number_allowed_length_xml(value: u32) -> &'static str {
    match value {
        0 => "Fixed",
        _ => "Variable",
    }
}

fn document_number_periodicity_xml(value: u32) -> &'static str {
    match value {
        1 => "Month",
        2 => "Quarter",
        3 => "Day",
        4 => "None",
        _ => "Year",
    }
}

fn parse_1c_bool_flag(value: &str) -> Option<bool> {
    match value {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

fn parse_return_values_reuse_flag(value: &str) -> Option<ReturnValuesReuseValue> {
    match value {
        "0" => Some(ReturnValuesReuseValue::DontUse),
        "1" => Some(ReturnValuesReuseValue::DuringRequest),
        "2" => Some(ReturnValuesReuseValue::DuringSession),
        _ => None,
    }
}

fn parse_constant_properties_from_text(
    text: &str,
    uuid: &str,
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
    form_refs: &BTreeMap<String, FormSourceReference>,
) -> Option<ConstantProperties> {
    let marker = format!("{{1,0,{uuid}}}");
    let marker_start = text.find(&marker)?;
    let mut value_types = parse_typed_metadata_value_types_before(text, marker_start, type_index)?;
    if value_types.len() != 1 {
        return None;
    }
    let value_type = value_types.pop()?;

    let constant_object_start = text[..marker_start].rfind("{16,")?;
    let constant_fields = split_1c_braced_fields(text, constant_object_start)?;
    let constant_detail_fields = split_1c_braced_fields(constant_fields.get(1)?, 0)?;
    let tooltip = parse_1c_synonyms(constant_detail_fields.get(4).copied().unwrap_or("{0}"));
    let extended_presentation = parse_1c_synonyms(constant_fields.get(8).copied().unwrap_or("{0}"));
    let explanation = parse_1c_synonyms(constant_fields.get(9).copied().unwrap_or("{0}"));
    let use_standard_commands = parse_1c_bool_flag(constant_fields.get(7)?.trim())?;
    let default_form = parse_catalog_form_ref(constant_fields.get(10).copied(), form_refs);
    let password_mode = parse_1c_bool_flag(constant_detail_fields.get(2)?.trim())?;
    let format = parse_1c_synonyms(constant_detail_fields.get(3).copied().unwrap_or("{0}"));
    let edit_format = parse_1c_synonyms(constant_detail_fields.get(18).copied().unwrap_or("{0}"));
    let mask = constant_detail_fields
        .get(6)
        .and_then(|field| parse_1c_quoted_string(field.trim()))
        .unwrap_or_default();
    let min_value = parse_constant_bound_value(constant_detail_fields.get(8).copied());
    let max_value = parse_constant_bound_value(constant_detail_fields.get(9).copied());
    let fill_checking = match constant_detail_fields.get(13).map(|field| field.trim()) {
        Some("1") => "ShowError",
        _ => "DontCheck",
    };
    let choice_parameters =
        parse_constant_choice_parameters(constant_detail_fields.get(16).copied(), object_refs);
    let choice_history_on_input = match constant_detail_fields.get(22).map(|field| field.trim()) {
        Some("1") => "DontUse",
        _ => "Auto",
    };
    let data_lock_control_mode = match constant_fields.get(6).map(|field| field.trim()) {
        Some("0") => "Automatic",
        _ => "Managed",
    };
    let header = parse_metadata_header_from_text(text, uuid)?;
    let mut generated_types = Vec::new();
    push_generated_type_entry(
        &mut generated_types,
        &constant_fields,
        2,
        3,
        &format!("ConstantManager.{}", header.name),
        "Manager",
    );
    push_generated_type_entry(
        &mut generated_types,
        &constant_fields,
        4,
        5,
        &format!("ConstantValueManager.{}", header.name),
        "ValueManager",
    );
    push_generated_type_entry(
        &mut generated_types,
        &constant_fields,
        13,
        14,
        &format!("ConstantValueKey.{}", header.name),
        "ValueKey",
    );

    Some(ConstantProperties {
        generated_types,
        value_type,
        tooltip,
        extended_presentation,
        explanation,
        use_standard_commands,
        default_form,
        password_mode,
        format,
        edit_format,
        mask,
        min_value,
        max_value,
        fill_checking,
        choice_parameters,
        choice_history_on_input,
        data_lock_control_mode,
    })
}

fn parse_constant_choice_parameters(
    field: Option<&str>,
    object_refs: &BTreeMap<String, String>,
) -> Vec<ChoiceParameter> {
    let Some(fields) = field.and_then(|field| split_1c_braced_fields(field, 0)) else {
        return Vec::new();
    };
    let count = fields
        .get(1)
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(0);
    let mut parameters = Vec::new();
    let mut index = 2usize;
    for _ in 0..count {
        let (Some(name), Some(value)) = (fields.get(index), fields.get(index + 1)) else {
            break;
        };
        if let Some(name) = parse_1c_quoted_string(name.trim())
            && let Some(value_ref) = parse_design_time_reference(value, object_refs)
        {
            parameters.push(ChoiceParameter { name, value_ref });
        }
        index += 2;
    }
    parameters
}

fn parse_metadata_child_choice_parameters(
    field: Option<&str>,
    object_refs: &BTreeMap<String, String>,
) -> Option<Vec<MetadataChoiceParameter>> {
    let field = field?;
    if metadata_child_collection_is_empty(Some(field)) {
        return Some(Vec::new());
    }
    let fields = split_1c_braced_fields(field, 0)?;
    let count = fields
        .get(1)
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(0);
    if count == 0 {
        return Some(Vec::new());
    }

    let mut parameters = Vec::new();
    let mut index = 2usize;
    for _ in 0..count {
        let (Some(name), Some(value)) = (fields.get(index), fields.get(index + 1)) else {
            break;
        };
        if let Some(name) = parse_1c_quoted_string(name.trim())
            && let Some(value) = parse_metadata_choice_parameter_value(value, object_refs)
        {
            parameters.push(MetadataChoiceParameter { name, value });
        }
        index += 2;
    }

    if parameters.is_empty() {
        None
    } else {
        Some(parameters)
    }
}

fn parse_metadata_choice_parameter_value(
    field: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<MetadataChoiceParameterValue> {
    let value = field.trim();
    if let Some(boolean) = parse_1c_bool_flag(value) {
        return Some(MetadataChoiceParameterValue::Boolean(boolean));
    }
    if let Some(fields) = split_1c_braced_fields(value, 0)
        && fields.len() >= 2
        && parse_1c_quoted_string(fields[0].trim()).as_deref() == Some("B")
        && let Some(boolean) = parse_1c_bool_flag(fields[1].trim())
    {
        return Some(MetadataChoiceParameterValue::Boolean(boolean));
    }
    let value_refs = parse_design_time_references(value, object_refs);
    match value_refs.len() {
        0 => None,
        1 => Some(MetadataChoiceParameterValue::DesignTimeRef(
            value_refs.into_iter().next().unwrap(),
        )),
        _ => Some(MetadataChoiceParameterValue::FixedArray(
            value_refs
                .into_iter()
                .map(MetadataChoiceParameterValue::DesignTimeRef)
                .collect(),
        )),
    }
}

fn parse_metadata_child_choice_parameter_links(
    field: Option<&str>,
    object_refs: &BTreeMap<String, String>,
) -> Option<Vec<MetadataChoiceParameterLink>> {
    let field = field?;
    if metadata_child_collection_is_empty(Some(field)) {
        return Some(Vec::new());
    }
    let fields = split_1c_braced_fields(field, 0)?;
    if fields.first().map(|value| value.trim()) != Some("5006") {
        return None;
    }
    let count = fields
        .get(1)
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(0);
    if count == 0 {
        return Some(Vec::new());
    }

    let mut links = Vec::new();
    let mut index = 2usize;
    for _ in 0..count {
        let Some(name) = fields
            .get(index)
            .and_then(|field| parse_1c_quoted_string(field.trim()))
        else {
            break;
        };
        let value_change = metadata_choice_link_value_change_xml(
            fields
                .get(index + 1)
                .map(|field| field.trim())
                .unwrap_or("1"),
        );
        index += 2;

        let mut data_path = None;
        while let Some(field) = fields.get(index) {
            if let Some(reference) = parse_design_time_reference(field, object_refs) {
                data_path = Some(reference);
                index += 1;
                continue;
            }
            break;
        }
        let Some(data_path) = data_path else {
            break;
        };
        if fields
            .get(index)
            .is_some_and(|field| field.trim().chars().all(|ch| ch.is_ascii_digit()))
        {
            index += 1;
        }
        links.push(MetadataChoiceParameterLink {
            name,
            data_path,
            value_change,
        });
    }

    Some(links)
}

fn metadata_choice_link_value_change_xml(value: &str) -> &'static str {
    match value {
        "2" => "DontChange",
        _ => "Clear",
    }
}

fn parse_design_time_reference(
    text: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    uuid_like_values(text)
        .into_iter()
        .rev()
        .filter_map(|uuid| object_refs.get(&uuid).cloned())
        .next()
}

fn parse_design_time_references(text: &str, object_refs: &BTreeMap<String, String>) -> Vec<String> {
    uuid_like_values(text)
        .into_iter()
        .filter_map(|uuid| object_refs.get(&uuid).cloned())
        .collect()
}

fn parse_constant_bound_value(field: Option<&str>) -> Option<String> {
    let fields = split_1c_braced_fields(field?, 0)?;
    if fields.first()?.trim() != r#""S""# {
        return None;
    }
    fields
        .get(1)
        .and_then(|field| parse_1c_quoted_string(field.trim()))
}

fn parse_typed_metadata_properties_from_text(
    text: &str,
    uuid: &str,
    type_index: &BTreeMap<String, String>,
) -> Option<TypedMetadataProperties> {
    let marker = format!("{{1,0,{uuid}}}");
    let marker_start = text.find(&marker)?;
    let value_types = parse_typed_metadata_value_types_before(text, marker_start, type_index)?;
    if value_types.is_empty() {
        return None;
    }

    Some(TypedMetadataProperties { value_types })
}

fn parse_common_attribute_properties_from_text(
    text: &str,
    uuid: &str,
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
) -> Option<CommonAttributeProperties> {
    let typed = parse_typed_metadata_properties_from_text(text, uuid, type_index)?;
    let fields = metadata_object_fields(text)?;
    if fields.first().map(|field| field.trim()) != Some("5") {
        return None;
    }
    let use_fields = fields
        .get(2)
        .and_then(|field| split_1c_braced_fields(field, 0));
    let auto_use = use_fields
        .as_deref()
        .and_then(parse_common_attribute_auto_use)
        .unwrap_or("DontUse");
    let content = use_fields
        .as_deref()
        .map(|fields| parse_common_attribute_content(fields, object_refs))
        .unwrap_or_default();

    Some(CommonAttributeProperties {
        value_types: typed.value_types,
        property_details: parse_common_attribute_property_details(&fields),
        auto_use,
        content,
        separation: parse_common_attribute_separation_properties(&fields, object_refs),
    })
}

fn parse_common_attribute_property_details(
    fields: &[&str],
) -> Option<CommonAttributePropertyDetails> {
    let typed_fields = fields
        .get(1)
        .and_then(|field| split_1c_braced_fields(field, 0))
        .unwrap_or_default();
    let fill_value = typed_fields
        .get(19)
        .and_then(|field| parse_common_attribute_fill_value(field));
    let fill_checking = metadata_fill_checking_xml(typed_fields.get(20).copied());
    (fields.len() > 3 || fill_value.is_some()).then_some(CommonAttributePropertyDetails {
        fill_value,
        fill_checking,
    })
}

fn parse_common_attribute_fill_value(field: &str) -> Option<CommonAttributeFillValue> {
    let value = field.trim();
    if matches!(value, "{0}" | "00000000-0000-0000-0000-000000000000") {
        return Some(CommonAttributeFillValue::Nil);
    }
    let fields = split_1c_braced_fields(value, 0)?;
    match fields.first().map(|field| field.trim())? {
        r#""U""# => Some(CommonAttributeFillValue::Nil),
        r#""B""# => fields
            .get(1)
            .and_then(|field| parse_1c_bool_flag(field.trim()))
            .map(CommonAttributeFillValue::Boolean),
        r#""N""# => fields
            .get(1)
            .map(|field| CommonAttributeFillValue::Decimal(field.trim().to_string())),
        r#""S""# => fields
            .get(1)
            .and_then(|field| parse_1c_quoted_string(field.trim()))
            .map(CommonAttributeFillValue::String),
        _ => None,
    }
}

fn parse_common_attribute_separation_properties(
    fields: &[&str],
    object_refs: &BTreeMap<String, String>,
) -> Option<CommonAttributeSeparationProperties> {
    Some(CommonAttributeSeparationProperties {
        data_separation: common_attribute_use_xml(fields.get(3)?.trim())?,
        separated_data_use: common_attribute_separated_data_use_xml(fields.get(4)?.trim())?,
        users_separation: common_attribute_reversed_use_xml(fields.get(5)?.trim())?,
        authentication_separation: common_attribute_reversed_use_xml(fields.get(6)?.trim())?,
        data_separation_value: parse_common_attribute_optional_ref(fields.get(7)?, object_refs),
        data_separation_use: parse_common_attribute_optional_ref(fields.get(8)?, object_refs),
        conditional_separation: parse_common_attribute_optional_ref(fields.get(9)?, object_refs),
        configuration_extensions_separation: common_attribute_use_xml(fields.get(10)?.trim())?,
        indexing: common_attribute_indexing_xml(fields.get(11)?.trim())?,
        full_text_search: common_attribute_full_text_search_xml(fields.get(12)?.trim())?,
        data_history: common_attribute_use_xml(fields.get(14)?.trim())?,
    })
}

fn parse_common_attribute_optional_ref(
    field: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    let fields = split_1c_braced_fields(field, 0)?;
    let uuid = fields
        .get(1)
        .and_then(|field| parse_non_zero_uuid(field.trim()))?;
    object_refs.get(&uuid).cloned()
}

fn parse_common_attribute_auto_use(fields: &[&str]) -> Option<&'static str> {
    fields
        .first()
        .and_then(|field| common_attribute_auto_use_xml(field.trim()))
}

fn common_attribute_auto_use_xml(value: &str) -> Option<&'static str> {
    match value {
        "0" => Some("DontUse"),
        "1" => Some("Use"),
        "2" => Some("AutoUse"),
        _ => None,
    }
}

fn common_attribute_use_xml(value: &str) -> Option<&'static str> {
    match value {
        "0" => Some("DontUse"),
        "1" => Some("Use"),
        _ => None,
    }
}

fn common_attribute_reversed_use_xml(value: &str) -> Option<&'static str> {
    match value {
        "0" => Some("Use"),
        "1" => Some("DontUse"),
        _ => None,
    }
}

fn common_attribute_separated_data_use_xml(value: &str) -> Option<&'static str> {
    match value {
        "1" => Some("Independently"),
        _ => None,
    }
}

fn common_attribute_indexing_xml(value: &str) -> Option<&'static str> {
    match value {
        "0" => Some("DontIndex"),
        _ => None,
    }
}

fn common_attribute_full_text_search_xml(value: &str) -> Option<&'static str> {
    match value {
        "0" => Some("Use"),
        "1" => Some("DontUse"),
        _ => None,
    }
}

fn parse_common_attribute_content(
    fields: &[&str],
    object_refs: &BTreeMap<String, String>,
) -> Vec<CommonAttributeContentItem> {
    let Some(count) = fields
        .get(1)
        .and_then(|field| field.trim().parse::<usize>().ok())
    else {
        return Vec::new();
    };
    let mut content = Vec::new();
    let mut index = 2usize;
    while content.len() < count && index < fields.len() {
        let field = fields[index];
        if let Some(metadata) = parse_design_time_reference(field, object_refs) {
            let settings = fields
                .get(index + 1)
                .filter(|field| is_common_attribute_content_settings(field));
            content.push(CommonAttributeContentItem {
                metadata,
                use_mode: settings
                    .and_then(|field| parse_common_attribute_content_use(field))
                    .unwrap_or_else(|| parse_common_attribute_content_use(field).unwrap_or("Use")),
                conditional_separation: settings.and_then(|field| {
                    parse_common_attribute_content_conditional_separation(field, object_refs)
                }),
            });
            index += if settings.is_some() { 2 } else { 1 };
        } else {
            index += 1;
        }
    }
    content
}

fn is_common_attribute_content_settings(field: &str) -> bool {
    split_1c_braced_fields(field, 0)
        .and_then(|fields| fields.first().map(|field| field.trim() == "2"))
        .unwrap_or(false)
}

fn parse_common_attribute_content_use(field: &str) -> Option<&'static str> {
    let fields = split_1c_braced_fields(field, 0)?;
    fields
        .get(1)
        .and_then(|field| common_attribute_content_use_xml(field.trim()))
}

fn parse_common_attribute_content_conditional_separation(
    field: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    let fields = split_1c_braced_fields(field, 0)?;
    let uuid = fields
        .get(2)
        .and_then(|field| parse_non_zero_uuid(field.trim()))?;
    object_refs.get(&uuid).cloned()
}

fn common_attribute_content_use_xml(value: &str) -> Option<&'static str> {
    match value {
        "0" => Some("DontUse"),
        "1" => Some("Use"),
        "2" => Some("DontUse"),
        _ => None,
    }
}

fn parse_functional_options_parameter_properties_from_text(
    text: &str,
    uuid: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<FunctionalOptionsParameterProperties> {
    let fields = metadata_object_fields(text)?;
    if metadata_header_field_index(&fields, uuid) != Some(1) {
        return None;
    }
    let use_refs = fields
        .get(2)
        .and_then(|field| split_1c_braced_fields(field, 0))
        .map(|fields| parse_functional_options_parameter_use_refs(&fields, object_refs))
        .unwrap_or_default();
    Some(FunctionalOptionsParameterProperties { use_refs })
}

fn parse_functional_options_parameter_use_refs(
    fields: &[&str],
    object_refs: &BTreeMap<String, String>,
) -> Vec<String> {
    let Some(count) = fields
        .get(1)
        .and_then(|field| field.trim().parse::<usize>().ok())
    else {
        return Vec::new();
    };
    fields
        .iter()
        .skip(2)
        .take(count)
        .filter_map(|field| parse_design_time_reference(field, object_refs))
        .collect()
}

fn parse_language_properties_from_text(text: &str, uuid: &str) -> Option<LanguageProperties> {
    let fields = metadata_object_fields(text)?;
    if metadata_header_field_index(&fields, uuid) != Some(1) {
        return None;
    }
    let language_code = fields
        .get(2)
        .and_then(|field| parse_1c_quoted_string(field.trim()))?;
    Some(LanguageProperties { language_code })
}

fn parse_document_numerator_properties_from_text(
    text: &str,
    uuid: &str,
) -> Option<DocumentNumeratorProperties> {
    let fields = metadata_object_fields(text)?;
    if metadata_header_field_index(&fields, uuid) != Some(1) {
        return None;
    }
    Some(DocumentNumeratorProperties {
        number_type: document_numerator_number_type_xml(parse_1c_u32_field(
            fields.get(2).copied(),
        )?),
        number_length: parse_1c_u32_field(fields.get(3).copied())?,
        number_allowed_length: document_numerator_allowed_length_xml(parse_1c_u32_field(
            fields.get(4).copied(),
        )?),
        check_unique: parse_1c_bool_field(fields.get(5).copied())?,
        number_periodicity: document_numerator_periodicity_xml(parse_1c_u32_field(
            fields.get(6).copied(),
        )?),
    })
}

fn document_numerator_number_type_xml(value: u32) -> &'static str {
    match value {
        0 => "Number",
        _ => "String",
    }
}

fn document_numerator_allowed_length_xml(value: u32) -> &'static str {
    match value {
        1 => "Fixed",
        _ => "Variable",
    }
}

fn document_numerator_periodicity_xml(value: u32) -> &'static str {
    match value {
        1 => "Month",
        2 => "Quarter",
        3 => "Day",
        4 => "None",
        _ => "Year",
    }
}

fn parse_ws_reference_properties_from_text(
    text: &str,
    uuid: &str,
) -> Option<WSReferenceProperties> {
    let fields = metadata_object_fields(text)?;
    if fields.first().map(|field| field.trim()) != Some("2")
        || metadata_header_field_index(&fields, uuid) != Some(2)
    {
        return None;
    }
    let location_fields = split_1c_braced_fields(fields.get(1)?, 0)?;
    let location_url = location_fields
        .first()
        .and_then(|field| parse_1c_quoted_string(field.trim()))?;
    Some(WSReferenceProperties {
        location_url,
        manager_type_id: parse_uuid_field(fields.get(3)?.trim())?,
        manager_value_id: parse_uuid_field(fields.get(4)?.trim())?,
    })
}

fn parse_integration_service_properties_from_text(
    text: &str,
    uuid: &str,
) -> Option<IntegrationServiceProperties> {
    let root_fields = split_1c_braced_fields(text.trim_start_matches('\u{feff}'), 0)?;
    let fields = metadata_object_fields(text)?;
    if fields.first().map(|field| field.trim()) != Some("0")
        || metadata_header_field_index(&fields, uuid) != Some(1)
    {
        return None;
    }
    let mut channels = root_fields
        .get(3)
        .and_then(|field| parse_integration_service_channels(field))
        .unwrap_or_default();
    if channels.is_empty() {
        channels = parse_integration_service_channels_from_text(text);
    }
    Some(IntegrationServiceProperties {
        manager_type_id: parse_uuid_field(fields.get(2)?.trim())?,
        manager_value_id: parse_uuid_field(fields.get(3)?.trim())?,
        external_address: fields
            .get(4)
            .and_then(|field| parse_1c_quoted_string(field.trim()))
            .unwrap_or_default(),
        channels,
    })
}

fn parse_xdto_package_properties_from_text(
    text: &str,
    uuid: &str,
) -> Option<XdtoPackageProperties> {
    let fields = metadata_object_fields(text)?;
    if fields.first().map(|field| field.trim()) != Some("1")
        || metadata_header_field_index(&fields, uuid) != Some(1)
    {
        return None;
    }
    let namespace = fields
        .get(2)
        .and_then(|field| parse_1c_quoted_string(field.trim()))
        .unwrap_or_default();
    Some(XdtoPackageProperties { namespace })
}

fn parse_web_service_properties_from_text(
    text: &str,
    expected_header: &MetadataHeader,
    object_refs: &BTreeMap<String, String>,
) -> Option<WebServiceProperties> {
    let root = split_web_service_braced_fields(text.trim_start_matches('\u{feff}'))?;
    if root.len() != 4 || root.first()?.trim() != "1" || root.get(2)?.trim() != "1" {
        return None;
    }
    let fields = split_web_service_braced_fields(root.get(1)?)?;
    if fields.len() != 8 || fields.first()?.trim() != "4" {
        return None;
    }
    let header = parse_web_service_header(fields.get(2)?)?;
    if !header.uuid.eq_ignore_ascii_case(&expected_header.uuid)
        || header.name != expected_header.name
        || header.synonyms != expected_header.synonyms
        || header.comment != expected_header.comment
    {
        return None;
    }

    let mut seen_uuids = BTreeSet::from([header.uuid.to_ascii_lowercase()]);
    let mut xdto_packages =
        parse_web_service_metadata_packages(fields.get(3)?, object_refs, &mut seen_uuids)?;
    xdto_packages.extend(
        parse_web_service_namespace_packages(fields.get(5)?)?
            .into_iter()
            .map(WebServiceXdtoPackage::Namespace),
    );

    Some(WebServiceProperties {
        namespace: parse_web_service_nonempty_quoted_string(fields.get(1)?)?,
        xdto_packages,
        descriptor_file_name: parse_web_service_nonempty_quoted_string(fields.get(4)?)?,
        reuse_sessions: parse_web_service_reuse_sessions(fields.get(6)?)?,
        session_max_age: parse_web_service_u32(fields.get(7)?)?,
        operations: parse_web_service_operations(root.get(3)?, &mut seen_uuids)?,
    })
}

fn parse_web_service_metadata_packages(
    value: &str,
    object_refs: &BTreeMap<String, String>,
    seen_uuids: &mut BTreeSet<String>,
) -> Option<Vec<WebServiceXdtoPackage>> {
    let fields = split_web_service_braced_fields(value)?;
    if fields.first()?.trim() != "0" {
        return None;
    }
    let count = parse_web_service_usize(fields.get(1)?)?;
    if fields.len() != count.checked_add(2)? {
        return None;
    }
    let mut seen_references = BTreeSet::new();
    let mut packages = Vec::with_capacity(count);
    for field in fields.iter().skip(2) {
        let typed = split_web_service_braced_fields(field)?;
        if typed.len() != 3
            || parse_web_service_quoted_string(typed.first()?)? != "#"
            || !typed
                .get(1)?
                .trim()
                .eq_ignore_ascii_case(METADATA_OBJECT_REF_TYPE_UUID)
        {
            return None;
        }
        let target = split_web_service_braced_fields(typed.get(2)?)?;
        if target.len() != 2 || target.first()?.trim() != "1" {
            return None;
        }
        let uuid = parse_web_service_non_zero_uuid(target.get(1)?)?;
        let reference = object_refs.get(&uuid)?;
        let name = reference.strip_prefix("XDTOPackage.")?;
        if name.is_empty()
            || name.contains('.')
            || !reference.chars().all(is_xml_1_0_char)
            || !seen_uuids.insert(uuid.to_ascii_lowercase())
            || !seen_references.insert(reference.clone())
        {
            return None;
        }
        packages.push(WebServiceXdtoPackage::MetadataReference(reference.clone()));
    }
    Some(packages)
}

fn parse_web_service_namespace_packages(value: &str) -> Option<Vec<String>> {
    let fields = split_web_service_braced_fields(value)?;
    let count = parse_web_service_usize(fields.first()?)?;
    if fields.len() != count.checked_add(1)? {
        return None;
    }
    let mut seen = BTreeSet::new();
    let mut packages = Vec::with_capacity(count);
    for field in fields.iter().skip(1) {
        let namespace = parse_web_service_quoted_string(field)?;
        if namespace.is_empty() || !seen.insert(namespace.clone()) {
            return None;
        }
        packages.push(namespace);
    }
    Some(packages)
}

fn parse_web_service_operations(
    value: &str,
    seen_uuids: &mut BTreeSet<String>,
) -> Option<Vec<WebServiceOperationProperties>> {
    let fields = split_web_service_braced_fields(value)?;
    if !parse_uuid_field(fields.first()?.trim())
        .is_some_and(|marker| marker.eq_ignore_ascii_case(WEB_SERVICE_OPERATION_COLLECTION_UUID))
    {
        return None;
    }
    let count = parse_web_service_usize(fields.get(1)?)?;
    if fields.len() != count.checked_add(2)? {
        return None;
    }
    let mut seen_names = BTreeSet::new();
    let mut operations = Vec::with_capacity(count);
    for field in fields.iter().skip(2) {
        let entry = split_web_service_braced_fields(field)?;
        if entry.len() != 3 || entry.get(1)?.trim() != "1" {
            return None;
        }
        let operation = split_web_service_braced_fields(entry.first()?)?;
        if operation.len() != 7 || operation.first()?.trim() != "1" {
            return None;
        }
        let header = parse_web_service_header(operation.get(1)?)?;
        if header.name.is_empty()
            || !seen_names.insert(web_service_case_insensitive_name_key(&header.name))
            || !seen_uuids.insert(header.uuid.to_ascii_lowercase())
        {
            return None;
        }
        let data_lock_control_mode = match operation.get(6)?.trim() {
            "0" => "Automatic",
            "1" => "Managed",
            _ => return None,
        };
        operations.push(WebServiceOperationProperties {
            header,
            returning_value_type: parse_web_service_xdto_type(operation.get(2)?)?,
            nillable: information_register_bool(operation.get(3)?)?,
            transactioned: information_register_bool(operation.get(4)?)?,
            procedure_name: parse_web_service_nonempty_quoted_string(operation.get(5)?)?,
            data_lock_control_mode,
            parameters: parse_web_service_parameters(entry.get(2)?, seen_uuids)?,
        });
    }
    Some(operations)
}

fn parse_web_service_parameters(
    value: &str,
    seen_uuids: &mut BTreeSet<String>,
) -> Option<Vec<WebServiceParameterProperties>> {
    let fields = split_web_service_braced_fields(value)?;
    if !parse_uuid_field(fields.first()?.trim())
        .is_some_and(|marker| marker.eq_ignore_ascii_case(WEB_SERVICE_PARAMETER_COLLECTION_UUID))
    {
        return None;
    }
    let count = parse_web_service_usize(fields.get(1)?)?;
    if fields.len() != count.checked_add(2)? {
        return None;
    }
    let mut seen_names = BTreeSet::new();
    let mut parameters = Vec::with_capacity(count);
    for field in fields.iter().skip(2) {
        let entry = split_web_service_braced_fields(field)?;
        if entry.len() != 2 || entry.get(1)?.trim() != "0" {
            return None;
        }
        let parameter = split_web_service_braced_fields(entry.first()?)?;
        if parameter.len() != 5 || parameter.first()?.trim() != "0" {
            return None;
        }
        let header = parse_web_service_header(parameter.get(1)?)?;
        if header.name.is_empty()
            || !seen_names.insert(web_service_case_insensitive_name_key(&header.name))
            || !seen_uuids.insert(header.uuid.to_ascii_lowercase())
        {
            return None;
        }
        let transfer_direction = match parameter.get(4)?.trim() {
            "0" => "In",
            "1" => "Out",
            "2" => "InOut",
            _ => return None,
        };
        parameters.push(WebServiceParameterProperties {
            header,
            value_type: parse_web_service_xdto_type(parameter.get(2)?)?,
            nillable: information_register_bool(parameter.get(3)?)?,
            transfer_direction,
        });
    }
    Some(parameters)
}

fn parse_web_service_xdto_type(value: &str) -> Option<WebServiceXdtoType> {
    let fields = split_web_service_braced_fields(value)?;
    if fields.len() != 3 || fields.first()?.trim() != "0" {
        return None;
    }
    let namespace = parse_web_service_quoted_string(fields.get(1)?)?;
    let name = parse_web_service_quoted_string(fields.get(2)?)?;
    if namespace.is_empty()
        || namespace.chars().any(char::is_control)
        || matches!(namespace.as_str(), XML_NAMESPACE | XMLNS_NAMESPACE)
        || !is_xml_ncname(&name)
    {
        return None;
    }
    Some(WebServiceXdtoType { namespace, name })
}

fn parse_web_service_header(value: &str) -> Option<MetadataHeader> {
    let fields = split_web_service_braced_fields(value)?;
    if fields.len() != 9
        || fields.first()?.trim() != "3"
        || fields.get(5)?.trim() != "0"
        || fields.get(6)?.trim() != "0"
        || !parse_uuid_field(fields.get(7)?.trim())
            .is_some_and(|uuid| information_register_uuid_is_zero(&uuid))
        || fields.get(8)?.trim() != "0"
    {
        return None;
    }
    let identity = split_web_service_braced_fields(fields.get(1)?)?;
    if identity.len() != 3 || identity.first()?.trim() != "1" || identity.get(1)?.trim() != "0" {
        return None;
    }
    let name = parse_web_service_nonempty_quoted_string(fields.get(2)?)?;
    if name.contains('.') {
        return None;
    }
    Some(MetadataHeader {
        uuid: parse_web_service_non_zero_uuid(identity.get(2)?)?,
        name,
        synonyms: parse_web_service_localized_value(fields.get(3)?)?,
        comment: parse_web_service_quoted_string(fields.get(4)?)?,
        template_type_code: None,
    })
}

fn parse_web_service_quoted_string(value: &str) -> Option<String> {
    let value = value.trim();
    let (parsed, consumed) = parse_1c_quoted_string_with_len(value)?;
    (consumed == value.len() && parsed.chars().all(is_xml_1_0_char)).then_some(parsed)
}

fn parse_web_service_nonempty_quoted_string(value: &str) -> Option<String> {
    let value = parse_web_service_quoted_string(value)?;
    (!value.is_empty()).then_some(value)
}

fn parse_web_service_localized_value(value: &str) -> Option<Vec<(String, String)>> {
    let fields = split_web_service_braced_fields(value)?;
    let count = parse_web_service_usize(fields.first()?)?;
    if fields.len() != count.checked_mul(2)?.checked_add(1)? {
        return None;
    }
    let mut languages = BTreeSet::new();
    let mut values = Vec::with_capacity(count);
    for pair in fields[1..].chunks_exact(2) {
        let language = parse_web_service_quoted_string(pair[0])?;
        let content = parse_web_service_quoted_string(pair[1])?;
        if !languages.insert(language.clone()) {
            return None;
        }
        values.push((language, content));
    }
    Some(values)
}

fn split_web_service_braced_fields(value: &str) -> Option<Vec<&str>> {
    let value = value.trim();
    if scan_1c_braced_value(value, 0)? != value.len() {
        return None;
    }
    split_1c_braced_fields(value, 0)
}

fn parse_web_service_usize(value: &str) -> Option<usize> {
    let value = value.trim();
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    value.parse().ok()
}

fn parse_web_service_u32(value: &str) -> Option<u32> {
    let value = value.trim();
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    value.parse().ok()
}

fn parse_web_service_reuse_sessions(value: &str) -> Option<&'static str> {
    match value.trim() {
        "0" => Some("DontUse"),
        "1" => Some("Use"),
        "2" => Some("AutoUse"),
        _ => None,
    }
}

fn parse_web_service_non_zero_uuid(value: &str) -> Option<String> {
    let value = value.trim();
    if value.len() != 36
        || value.bytes().enumerate().any(|(index, byte)| match index {
            8 | 13 | 18 | 23 => byte != b'-',
            _ => !byte.is_ascii_hexdigit(),
        })
        || value == "00000000-0000-0000-0000-000000000000"
    {
        return None;
    }
    Some(value.to_string())
}

fn web_service_case_insensitive_name_key(value: &str) -> String {
    value.chars().flat_map(char::to_lowercase).collect()
}

fn is_xml_1_0_char(ch: char) -> bool {
    matches!(
        ch,
        '\u{0009}' | '\u{000a}' | '\u{000d}' | '\u{0020}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}'
    )
}

fn is_xml_ncname(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    is_xml_ncname_start_char(first) && chars.all(is_xml_ncname_char)
}

fn is_xml_ncname_start_char(ch: char) -> bool {
    matches!(
        ch,
        'A'..='Z'
            | '_'
            | 'a'..='z'
            | '\u{00c0}'..='\u{00d6}'
            | '\u{00d8}'..='\u{00f6}'
            | '\u{00f8}'..='\u{02ff}'
            | '\u{0370}'..='\u{037d}'
            | '\u{037f}'..='\u{1fff}'
            | '\u{200c}'..='\u{200d}'
            | '\u{2070}'..='\u{218f}'
            | '\u{2c00}'..='\u{2fef}'
            | '\u{3001}'..='\u{d7ff}'
            | '\u{f900}'..='\u{fdcf}'
            | '\u{fdf0}'..='\u{fffd}'
            | '\u{10000}'..='\u{effff}'
    )
}

fn is_xml_ncname_char(ch: char) -> bool {
    is_xml_ncname_start_char(ch)
        || matches!(
            ch,
            '-' | '.' | '0'..='9' | '\u{00b7}' | '\u{0300}'..='\u{036f}' | '\u{203f}'..='\u{2040}'
        )
}

fn parse_http_service_properties_from_text(
    text: &str,
    uuid: &str,
) -> Option<HttpServiceProperties> {
    let fields = metadata_object_fields(text)?;
    if fields.first().map(|field| field.trim()) != Some("2")
        || metadata_header_field_index(&fields, uuid) != Some(2)
    {
        return None;
    }
    let root_url = fields
        .get(1)
        .and_then(|field| parse_1c_quoted_string(field.trim()))
        .unwrap_or_default();
    let reuse_sessions = fields
        .get(3)
        .and_then(|field| http_service_reuse_sessions_from_code(field.trim()))
        .unwrap_or("DontUse");
    let session_max_age = fields
        .get(4)
        .and_then(|field| field.trim().parse::<u32>().ok())
        .unwrap_or(20);
    Some(HttpServiceProperties {
        root_url,
        reuse_sessions,
        session_max_age,
        url_templates: parse_http_service_url_templates_from_text(text, uuid),
    })
}

fn http_service_reuse_sessions_from_code(value: &str) -> Option<&'static str> {
    match value {
        "0" => Some("DontUse"),
        "1" => Some("Use"),
        "2" => Some("AutoUse"),
        _ => None,
    }
}

struct HttpServiceChildCandidate {
    start: usize,
    end: usize,
    header: MetadataHeader,
    strings: Vec<String>,
    is_method: bool,
}

fn parse_http_service_url_templates_from_text(
    text: &str,
    owner_uuid: &str,
) -> Vec<HttpServiceUrlTemplateProperties> {
    let candidates = http_service_child_candidates_from_text(text, owner_uuid);
    let method_candidates = candidates
        .iter()
        .filter_map(|candidate| {
            if !candidate.is_method {
                return None;
            }
            let http_method = candidate.strings.first()?;
            let handler = candidate.strings.get(1)?;
            Some(HttpServiceChildCandidate {
                start: candidate.start,
                end: candidate.end,
                header: candidate.header.clone(),
                strings: vec![http_method.clone(), handler.clone()],
                is_method: true,
            })
        })
        .collect::<Vec<_>>();

    let mut seen = BTreeSet::new();
    let template_candidates = candidates
        .into_iter()
        .filter_map(|candidate| {
            let template = candidate.strings.first()?.clone();
            if candidate.is_method
                || !is_http_service_url_template_text(&template)
                || !seen.insert(candidate.header.uuid.clone())
            {
                return None;
            }
            Some((candidate, template))
        })
        .collect::<Vec<_>>();

    template_candidates
        .iter()
        .enumerate()
        .map(|(index, (candidate, template))| {
            let next_template_start = template_candidates
                .get(index + 1)
                .map(|(next_candidate, _)| next_candidate.start)
                .unwrap_or(usize::MAX);
            let methods = method_candidates
                .iter()
                .filter(|method| {
                    method.start > candidate.start && method.start < next_template_start
                })
                .map(|method| HttpServiceMethodProperties {
                    header: method.header.clone(),
                    http_method: method.strings[0].clone(),
                    handler: method.strings[1].clone(),
                })
                .collect::<Vec<_>>();
            HttpServiceUrlTemplateProperties {
                header: candidate.header.clone(),
                template: template.clone(),
                methods,
            }
        })
        .collect()
}

fn http_service_child_candidates_from_text(
    text: &str,
    owner_uuid: &str,
) -> Vec<HttpServiceChildCandidate> {
    let mut candidates = Vec::new();
    let mut seen = BTreeSet::new();
    for (header, marker_start) in nested_headers_with_offsets_from_text(text, owner_uuid, |_| true)
    {
        let Some((start, end, fields)) =
            innermost_metadata_object_fields_around_header(text, marker_start, &header.uuid)
        else {
            continue;
        };
        if !seen.insert(header.uuid.clone()) {
            continue;
        }
        let Some(header_index) = metadata_header_field_index(&fields, &header.uuid) else {
            continue;
        };
        let (strings, is_method) = match header_index {
            2 => (
                fields
                    .get(1)
                    .and_then(|field| parse_1c_quoted_string(field.trim()))
                    .map(|template| vec![template])
                    .unwrap_or_default(),
                false,
            ),
            3 if fields.first().map(|field| field.trim()) == Some("0") => {
                let method = fields
                    .get(2)
                    .and_then(|field| http_service_method_from_code(field.trim()))
                    .or_else(|| canonical_http_service_method_name(&header.name));
                let handler = fields
                    .get(1)
                    .and_then(|field| parse_1c_quoted_string(field.trim()));
                match (method, handler) {
                    (Some(method), Some(handler)) => (vec![method.to_string(), handler], true),
                    _ => (Vec::new(), false),
                }
            }
            _ => {
                let strings = fields
                    .iter()
                    .skip(header_index + 1)
                    .filter_map(|field| parse_1c_quoted_string(field.trim()))
                    .collect::<Vec<_>>();
                match (
                    strings
                        .first()
                        .and_then(|method| canonical_http_service_method_name(method)),
                    strings.get(1),
                ) {
                    (Some(method), Some(handler)) => {
                        (vec![method.to_string(), handler.clone()], true)
                    }
                    _ => (strings, false),
                }
            }
        };
        if strings.is_empty() {
            continue;
        }
        candidates.push(HttpServiceChildCandidate {
            start,
            end,
            header,
            strings,
            is_method,
        });
    }
    candidates
}

fn innermost_metadata_object_fields_around_header<'a>(
    text: &'a str,
    marker_start: usize,
    uuid: &str,
) -> Option<(usize, usize, Vec<&'a str>)> {
    let mut search_end = marker_start;
    let mut best: Option<(usize, usize, Vec<&'a str>)> = None;
    while let Some(start) = text[..search_end].rfind('{') {
        search_end = start;
        let Some(end) = scan_1c_braced_value(text, start) else {
            continue;
        };
        if marker_start >= end {
            continue;
        }
        let Some(fields) = split_1c_braced_fields(text, start) else {
            continue;
        };
        if matches!(fields.first().map(|field| field.trim()), Some("1" | "3")) {
            continue;
        }
        if metadata_header_field_index(&fields, uuid).is_none() {
            continue;
        }
        if best.as_ref().map(|(_, best_end, _)| end < *best_end) != Some(false) {
            best = Some((start, end, fields));
        }
    }
    best
}

fn is_http_service_url_template_text(value: &str) -> bool {
    value.starts_with('/') || value.contains('{') || value == "*"
}

fn http_service_method_from_code(value: &str) -> Option<&'static str> {
    match value {
        "2" => Some("DELETE"),
        "3" => Some("GET"),
        "11" => Some("POST"),
        "14" => Some("PUT"),
        _ => None,
    }
}

fn canonical_http_service_method_name(value: &str) -> Option<&'static str> {
    match value {
        "DELETE" => Some("DELETE"),
        "GET" => Some("GET"),
        "HEAD" => Some("HEAD"),
        "OPTIONS" => Some("OPTIONS"),
        "PATCH" => Some("PATCH"),
        "POST" => Some("POST"),
        "PUT" => Some("PUT"),
        _ => None,
    }
}

fn parse_integration_service_channels_from_text(
    text: &str,
) -> Vec<IntegrationServiceChannelProperties> {
    let mut channels = Vec::new();
    let mut seen = BTreeSet::new();
    let mut offset = 0usize;
    while let Some(relative) = text[offset..].find("{1,") {
        let start = offset + relative;
        offset = start + 3;
        let Some(end) = scan_1c_braced_value(text, start) else {
            continue;
        };
        if let Some(channel) = parse_integration_service_channel(&text[start..end])
            && seen.insert(channel.header.uuid.clone())
        {
            channels.push(channel);
        }
    }
    channels
}

fn parse_integration_service_channels(
    text: &str,
) -> Option<Vec<IntegrationServiceChannelProperties>> {
    let fields = split_1c_braced_fields(text, 0)?;
    let count = fields.get(1)?.trim().parse::<usize>().ok()?;
    let channel_items = split_1c_braced_sequence(fields.get(2)?)?;
    Some(
        channel_items
            .into_iter()
            .take(count)
            .filter_map(|field| parse_integration_service_channel(field))
            .collect::<Vec<_>>(),
    )
}

fn parse_integration_service_channel(text: &str) -> Option<IntegrationServiceChannelProperties> {
    let object_start = text.find("{1,")?;
    let fields = split_1c_braced_fields(text, object_start)?;
    if fields.first().map(|field| field.trim()) != Some("1") {
        return None;
    }
    let header_text = *fields.get(1)?;
    let uuid = uuid_like_values_in_text_order(header_text)
        .into_iter()
        .next()?;
    let header = parse_metadata_header_from_text(header_text, &uuid)?;
    Some(IntegrationServiceChannelProperties {
        header,
        manager_type_id: parse_uuid_field(fields.get(2)?.trim())?,
        manager_value_id: parse_uuid_field(fields.get(3)?.trim())?,
        external_name: fields
            .get(4)
            .and_then(|field| parse_1c_quoted_string(field.trim()))
            .unwrap_or_default(),
        receive_message_processing: fields
            .get(5)
            .and_then(|field| parse_1c_quoted_string(field.trim()))
            .unwrap_or_default(),
        message_direction: integration_service_message_direction_xml(parse_1c_u32_field(
            fields.get(6).copied(),
        )?),
        transactioned: parse_1c_bool_field(fields.get(7).copied())?,
    })
}

fn integration_service_message_direction_xml(value: u32) -> &'static str {
    match value {
        1 => "Receive",
        _ => "Send",
    }
}

fn split_1c_braced_sequence(text: &str) -> Option<Vec<&str>> {
    let text = text.trim();
    if !text.starts_with('{') || !text.ends_with('}') {
        return None;
    }
    let mut fields = Vec::new();
    let mut offset = 1usize;
    let end = text.len().checked_sub(1)?;
    while offset < end {
        offset = skip_ascii_ws_at(text, offset);
        while text[offset..].starts_with(',') {
            offset += 1;
            offset = skip_ascii_ws_at(text, offset);
        }
        if offset >= end {
            break;
        }
        if !text[offset..].starts_with('{') {
            return None;
        }
        let item_end = scan_1c_braced_value(text, offset)?;
        fields.push(text[offset..item_end].trim());
        offset = item_end;
    }
    Some(fields)
}

fn parse_typed_metadata_value_types_before(
    text: &str,
    marker_start: usize,
    type_index: &BTreeMap<String, String>,
) -> Option<Vec<ConstantValueType>> {
    let typed_object_start = text[..marker_start].rfind("{2,")?;
    let typed_fields = split_1c_braced_fields(text, typed_object_start)?;
    parse_metadata_type_pattern(typed_fields.get(2)?, type_index)
}

fn parse_defined_type_properties_from_text(
    text: &str,
    uuid: &str,
    type_index: &BTreeMap<String, String>,
) -> Option<DefinedTypeProperties> {
    let marker = format!("{{1,0,{uuid}}}");
    let marker_start = text.find(&marker)?;
    let defined_type_start = text[..marker_start].rfind("{0,")?;
    let fields = split_1c_braced_fields(text, defined_type_start)?;
    let value_types = parse_metadata_type_pattern(fields.get(4)?, type_index)?;
    if value_types.is_empty() {
        return None;
    }
    let header = parse_metadata_header_from_text(text, uuid)?;
    let mut generated_types = Vec::new();
    push_generated_type_entry(
        &mut generated_types,
        &fields,
        1,
        2,
        &format!("DefinedType.{}", header.name),
        "DefinedType",
    );

    Some(DefinedTypeProperties {
        generated_types,
        value_types,
    })
}

fn parse_command_group_properties_from_text(
    text: &str,
    uuid: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<CommandGroupProperties> {
    let marker = format!("{{1,0,{uuid}}}");
    let marker_start = text.find(&marker)?;
    let inner_start = text[..marker_start].rfind("{3,")?;
    let group_start = text[..inner_start].rfind("{3,")?;
    let fields = split_1c_braced_fields(text, group_start)?;
    if fields.len() < 7 {
        return None;
    }
    let (picture_ref, picture_load_transparent) =
        parse_command_group_picture_value(fields.get(1)?, object_refs)?;
    let category = command_group_category_xml(fields.get(2)?.trim().parse().ok()?);
    let representation = command_group_representation_xml(fields.get(3)?.trim().parse().ok()?);
    let tooltip = parse_1c_synonyms(fields.get(4).copied().unwrap_or("{0}"));

    Some(CommandGroupProperties {
        representation,
        picture_ref,
        picture_load_transparent,
        tooltip,
        category,
    })
}

fn parse_common_command_properties_from_text(
    text: &str,
    uuid: &str,
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
) -> Option<CommonCommandProperties> {
    let marker = format!("{{1,0,{uuid}}}");
    let marker_start = text.find(&marker)?;
    let base_object_start = text[..marker_start].rfind("{3,")?;
    let command_start = text[..base_object_start].rfind("{9,")?;
    let fields = split_1c_braced_fields(text, command_start)?;
    if fields.len() < 13 {
        return None;
    }
    let (picture_ref, picture_load_transparent) =
        parse_common_command_picture_value(fields.get(1)?, object_refs)?;
    let representation = common_command_representation_xml(fields.get(2)?.trim().parse().ok()?);
    let tooltip = parse_1c_synonyms(fields.get(3).copied().unwrap_or("{0}"));
    let shortcut = fields
        .get(5)
        .and_then(|field| parse_common_command_shortcut_value(field));
    let include_help_in_contents = fields
        .get(6)
        .and_then(|field| parse_1c_bool_flag(field.trim()))
        .unwrap_or(false);
    let group = fields
        .get(7)
        .and_then(|field| parse_common_command_group_value(field, object_refs));
    let command_parameter_types = parse_common_command_parameter_types(fields.get(8)?, type_index);
    let parameter_use_mode =
        common_command_parameter_use_mode_xml(fields.get(11)?.trim().parse().ok()?);
    let modifies_data = fields
        .get(10)
        .and_then(|field| parse_1c_bool_flag(field.trim()))
        .unwrap_or(false);
    let on_main_server_unavailable_behavior =
        common_command_on_main_server_unavailable_behavior_xml(
            fields.get(12)?.trim().parse().ok()?,
        );

    Some(CommonCommandProperties {
        representation,
        picture_ref,
        picture_load_transparent,
        tooltip,
        shortcut,
        include_help_in_contents,
        group,
        command_parameter_types,
        parameter_use_mode,
        modifies_data,
        on_main_server_unavailable_behavior,
    })
}

fn parse_command_group_picture_value(
    value: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<(Option<String>, bool)> {
    let fields = split_1c_braced_fields(value, 0)?;
    if fields.first()?.trim() != "4" {
        return None;
    }
    let picture_kind = fields.get(1)?.trim().parse::<i32>().ok()?;
    let load_transparent = fields
        .get(6)
        .and_then(|field| parse_1c_bool_flag(field.trim()))
        .unwrap_or(false);
    if picture_kind == 0 {
        return Some((None, load_transparent));
    }
    if picture_kind == 1 {
        let ref_fields = split_1c_braced_fields(fields.get(2)?, 0)?;
        if ref_fields.first()?.trim() == "0" {
            let uuid = ref_fields.get(1)?.trim();
            if let Some(reference) = object_refs.get(uuid)
                && reference.starts_with("CommonPicture.")
            {
                return Some((Some(reference.clone()), load_transparent));
            }
            if let Some(reference) = common_command_standard_picture_name(uuid) {
                return Some((Some(reference.to_string()), load_transparent));
            }
        }
        if ref_fields.first()?.trim() == "-13" {
            return Some((Some("StdPicture.Print".to_string()), load_transparent));
        }
    }
    Some((None, load_transparent))
}

fn parse_common_command_picture_value(
    value: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<(Option<String>, bool)> {
    let fields = split_1c_braced_fields(value, 0)?;
    if fields.first()?.trim() != "4" {
        return None;
    }
    let picture_kind = fields.get(1)?.trim().parse::<i32>().ok()?;
    let load_transparent = fields
        .get(6)
        .and_then(|field| parse_1c_bool_flag(field.trim()))
        .unwrap_or(false);
    if picture_kind == 0 {
        return Some((None, load_transparent));
    }
    if picture_kind == 1 {
        let ref_fields = split_1c_braced_fields(fields.get(2)?, 0)?;
        if ref_fields.first()?.trim() == "-2" {
            return Some((
                Some("StdPicture.InputFieldClear".to_string()),
                load_transparent,
            ));
        }
        if ref_fields.first()?.trim() == "-3" {
            return Some((Some("StdPicture.MoveUp".to_string()), load_transparent));
        }
        if ref_fields.first()?.trim() == "-4" {
            return Some((Some("StdPicture.MoveDown".to_string()), load_transparent));
        }
        if ref_fields.first()?.trim() == "-9" {
            return Some((Some("StdPicture.MoveRight".to_string()), load_transparent));
        }
        if ref_fields.first()?.trim() == "-8" {
            return Some((Some("StdPicture.MoveLeft".to_string()), load_transparent));
        }
        if ref_fields.first()?.trim() == "-7" {
            return Some((
                Some("StdPicture.InputFieldOpen".to_string()),
                load_transparent,
            ));
        }
        if ref_fields.first()?.trim() == "-10" {
            return Some((Some("StdPicture.CheckAll".to_string()), load_transparent));
        }
        if ref_fields.first()?.trim() == "-11" {
            return Some((Some("StdPicture.UncheckAll".to_string()), load_transparent));
        }
        if ref_fields.first()?.trim() == "-13" {
            return Some((Some("StdPicture.Print".to_string()), load_transparent));
        }
        if ref_fields.first()?.trim() == "0" {
            let uuid = ref_fields.get(1)?.trim();
            if let Some(reference) = common_command_standard_picture_name(uuid) {
                return Some((Some(reference.to_string()), load_transparent));
            }
            if let Some(reference) = object_refs.get(uuid)
                && reference.starts_with("CommonPicture.")
            {
                return Some((Some(reference.clone()), load_transparent));
            }
        }
    }
    Some((None, load_transparent))
}

fn parse_common_command_shortcut_value(value: &str) -> Option<String> {
    let fields = split_1c_braced_fields(value, 0)?;
    if fields.first()?.trim() != "0" {
        return None;
    }
    let key_code = fields.get(1)?.trim().parse::<u16>().ok()?;
    let modifier_code = fields
        .get(2)
        .and_then(|field| field.trim().parse::<u16>().ok())
        .unwrap_or(0);
    if key_code == 0 || modifier_code != 0 {
        return None;
    }
    if (112..=123).contains(&key_code) {
        return Some(format!("F{}", key_code - 111));
    }
    None
}

fn common_command_standard_picture_name(uuid: &str) -> Option<&'static str> {
    match uuid.to_ascii_lowercase().as_str() {
        STD_PICTURE_INFORMATION_UUID => Some("StdPicture.Information"),
        STD_PICTURE_SAVE_FILE_UUID => Some("StdPicture.SaveFile"),
        STD_PICTURE_USER_UUID => Some("StdPicture.User"),
        STD_PICTURE_LOAD_REPORT_SETTINGS_UUID => Some("StdPicture.LoadReportSettings"),
        "942e0303-a3ec-4fe8-887c-5aea8516d424" => Some("StdPicture.ReportSettings"),
        STD_PICTURE_INFORMATION_REGISTER_UUID => Some("StdPicture.InformationRegister"),
        STD_PICTURE_SHOW_DATA_UUID => Some("StdPicture.ShowData"),
        STD_PICTURE_CUSTOMIZE_LIST_UUID => Some("StdPicture.CustomizeList"),
        "97b2cc97-d5c6-45fb-9824-9d6d73db21fe" => Some("StdPicture.Change"),
        "37cf7cc0-abad-4385-b597-6fd2d8dc085a" => Some("StdPicture.Task"),
        "2f130057-bb2a-4e22-bba5-e108fac26940" => Some("StdPicture.ChooseValue"),
        "47f01799-7968-4f44-9acc-fe1bdde8beb2" => Some("StdPicture.ActiveUsers"),
        "e8a49985-fef7-45a9-b6bb-ddd2b9028172" => Some("StdPicture.DataHistory"),
        "a24cff7f-a1a5-4403-af82-a7b31852cde9" => Some("StdPicture.BusinessProcessObject"),
        "f6532868-30b9-44ab-803c-78f0f0b06b02" => Some("StdPicture.CloneObject"),
        "448d6f55-d885-496c-870d-d1bd78374745" => Some("StdPicture.CloneListItem"),
        "977e831a-0e73-4d60-af51-091a6fa8612e" => Some("StdPicture.CreateListItem"),
        "723765ab-0b92-4745-a621-1ba0f77c92c9" => Some("StdPicture.EventLog"),
        "4fddea39-5129-4b4c-83fe-4e443cd61940" => Some("StdPicture.EventLogByUser"),
        "ffab30f1-da11-44b5-b34c-24da22badcf4" => Some("StdPicture.Find"),
        "785362cb-3756-48ed-87d2-292ded17054a" => Some("StdPicture.OpenFile"),
        "4d2570b5-205f-413c-b4cc-b2097f61684f" => Some("StdPicture.CreateInitialImage"),
        "0ce78048-0196-4f80-a781-9829cdb7f43e" => Some("StdPicture.GenerateReport"),
        "18492a87-2fe4-44af-b218-304897fed020" => Some("StdPicture.MarkToDelete"),
        "20ebc47b-f4d9-439c-acd3-fdc624fbac2a" => Some("StdPicture.Post"),
        "23f940bf-7381-4c2b-85a1-e541ed428042" => Some("StdPicture.SaveValues"),
        "a7707ed1-39b0-418f-974d-4d500d27a9c6" => Some("StdPicture.RestoreValues"),
        "8f29e0e2-d5e6-41e8-a34d-9a0288156322" => Some("StdPicture.Reread"),
        "db817ee1-fd28-4e7f-bb4a-53686b2b153c" => Some("StdPicture.Report"),
        "1970a480-9b38-405e-9d9e-8209f3fad5f1" => Some("StdPicture.ScheduledJob"),
        "58174855-39be-462e-8723-cb2d95182146" => Some("StdPicture.SetDateInterval"),
        "2ef82795-06fe-4365-bd0c-44b486264620" => Some("StdPicture.FilterCriterion"),
        "b1406535-6cc2-4410-95ea-753556e8460f" => Some("StdPicture.FilterByCurrentValue"),
        "479470e0-ea0f-4266-8549-e2b1e8c06534" => Some("StdPicture.ClearFilter"),
        "fb7e9fb5-110b-41cb-adc6-753969ae1c81" => Some("StdPicture.ExpandAll"),
        "27ee3053-952c-49e5-8261-9215098e0e9c" => Some("StdPicture.CollapseAll"),
        "5289d9a4-b012-4d54-9bce-50473fe29b57" => Some("StdPicture.DialogExclamation"),
        "55ef0776-5ee4-4daf-9a9b-70d63643ab8d" => Some("StdPicture.SetTime"),
        "fc4f29e0-d168-4fe0-8e64-e982fabf2595" => Some("StdPicture.Refresh"),
        "91022b99-b610-48ad-954e-a297848081ce" => Some("StdPicture.SortListAsc"),
        "1fa32fdb-a180-418f-a6eb-db7516b7a30b" => Some("StdPicture.SortListDesc"),
        "894afc03-9904-465d-b671-f555ffb9b21c" => Some("StdPicture.Document"),
        "1cd7b762-ec6a-4e92-ac9a-1832be228ec3" => Some("StdPicture.Stop"),
        "8ca4ea33-603d-4992-8a41-c7924b5bd40b" => Some("StdPicture.UndoPosting"),
        "894cf65b-4109-4533-a1d7-c87b1fcc80a3" => Some("StdPicture.Write"),
        "e6fc55a0-3d58-4b15-bdd3-717453929598" => Some("StdPicture.WriteAndClose"),
        "08a45a70-c221-4339-b3b1-9f11cb22147d" => Some("StdPicture.Delete"),
        "6e3687cf-a8d1-446a-833a-bfaf38516353" => Some("StdPicture.SwitchActivity"),
        "7a9cd2fd-6372-4342-9a9e-3ebbd754fd83" => Some("StdPicture.AppearanceCheckBox"),
        "0c1f7756-6143-4903-a94c-8f22c85e44de" => Some("StdPicture.Attribute"),
        "3c904ff7-1195-4a7c-9a38-7b1f6ca49cce" => Some("StdPicture.Back"),
        "509c4a7f-6406-4388-bb8c-bc81fb5131aa" => Some("StdPicture.BusinessProcess"),
        "97c5a6d5-47ed-43f9-8c8c-10e9903c23d2" => Some("StdPicture.Calendar"),
        "4ab0e87f-7d9b-4aa8-ac4b-680a78522da8" => Some("StdPicture.CreateFolder"),
        "ee7c4a5b-2d9b-4087-ae3e-947792085f09" => {
            Some("StdPicture.DataCompositionOutputParameters")
        }
        "544fdbe8-5956-4512-bc62-93b4c022d291" => Some("StdPicture.ExchangePlan"),
        "003024ed-fa25-42ac-9f53-f5014e383801" => Some("StdPicture.ExecuteTask"),
        "1a4342a5-fa06-4556-8a85-e8738fc25821" => Some("StdPicture.GetURL"),
        "3d4ad3b1-17de-4cf1-a2e4-0c2c83a5b5c2" => Some("StdPicture.ListViewModeHierarchicalList"),
        "64837726-d2a2-4682-a788-737423e80013" => Some("StdPicture.Picture"),
        "b4c7ab2c-bcda-4468-a28f-5fee93838c4e" => Some("StdPicture.Properties"),
        "b5a0aaba-3a83-4a71-b6f9-24aae1574681" => Some("StdPicture.SaveReportSettings"),
        "be23a908-fe1b-44df-be94-d0f6e8353abe" => Some("StdPicture.SendMessage"),
        "03665ff1-3a05-41d1-96d3-04bda2d8ede3" => Some("StdPicture.SpreadsheetInsertComment"),
        "2846af8d-af84-47e3-82b9-01b01f960426" => Some("StdPicture.SpreadsheetReadOnly"),
        "3bdc16c8-6a96-4467-9442-a8e4804b3fa2" => Some("StdPicture.SyncContents"),
        _ => None,
    }
}

fn parse_common_command_group_value(
    value: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    let fields = split_1c_braced_fields(value, 0)?;
    if fields.first()?.trim() != "1" {
        return None;
    }
    let uuid = fields.get(1)?.trim();
    if let Some(reference) = object_refs.get(uuid)
        && reference.starts_with("CommandGroup.")
    {
        return Some(reference.clone());
    }
    common_command_group_name(uuid).map(str::to_string)
}

fn parse_common_command_parameter_types(
    value: &str,
    type_index: &BTreeMap<String, String>,
) -> Vec<ConstantValueType> {
    parse_metadata_type_pattern(value, type_index)
        .unwrap_or_default()
        .into_iter()
        .map(|value_type| match value_type {
            ConstantValueType::Reference { reference }
                if reference.starts_with("cfg:DefinedType.") =>
            {
                ConstantValueType::ReferenceTypeSet { reference }
            }
            value_type => value_type,
        })
        .collect()
}

fn common_command_group_name(uuid: &str) -> Option<&'static str> {
    // Platform standard command group UUIDs; these are not configuration-specific ids.
    match uuid.to_ascii_lowercase().as_str() {
        "77ea1b8f-dd79-4717-9dba-5628e7f348cf" => Some("NavigationPanelOrdinary"),
        "bc80566a-86a5-4e87-acd4-872239385a2e" => Some("NavigationPanelSeeAlso"),
        "1af6d528-0b86-4fba-ab95-bd7475db03ba" => Some("NavigationPanelImportant"),
        "4f499c31-050b-47c5-aa84-d0366c0a0da8" => Some("ActionsPanelCreate"),
        "5b360bff-01a1-49b6-93d2-26e7e8e3a038" => Some("ActionsPanelReports"),
        "aabb34e1-98c1-4bd0-bf7f-243f95437b44" => Some("ActionsPanelTools"),
        "dc2ade0f-383e-4c78-85f2-c0dabc0e2dc0" => Some("FormCommandBarCreateBasedOn"),
        "cb50f5c0-8013-4262-93a2-f0db379d6b6b" => Some("FormCommandBarImportant"),
        "eacad741-96b9-4b3a-bf79-dde9ecead1a1" => Some("FormNavigationPanelGoTo"),
        "8ab1540c-0bfa-4fa6-a1e1-5d5069efc7d8" => Some("FormNavigationPanelSeeAlso"),
        "dc11a6be-de1f-4b64-a7a5-9b17bf4ec9f2" => Some("FormNavigationPanelImportant"),
        _ => None,
    }
}

fn command_group_representation_xml(value: u8) -> &'static str {
    match value {
        0 => "Text",
        1 => "Picture",
        2 => "PictureAndText",
        3 => "Auto",
        _ => "Auto",
    }
}

fn common_command_representation_xml(value: u8) -> &'static str {
    command_group_representation_xml(value)
}

fn common_command_parameter_use_mode_xml(value: u8) -> &'static str {
    match value {
        1 => "Multiple",
        _ => "Single",
    }
}

fn common_command_on_main_server_unavailable_behavior_xml(_value: u8) -> &'static str {
    "Auto"
}

fn command_group_category_xml(value: u8) -> &'static str {
    match value {
        1 => "NavigationPanel",
        2 => "FormNavigationPanel",
        4 => "ActionsPanel",
        8 => "FormCommandBar",
        _ => "FormCommandBar",
    }
}

fn parse_style_item_properties_from_text(text: &str, uuid: &str) -> Option<StyleItemProperties> {
    let fields = metadata_object_fields(text)?;
    if metadata_header_field_index(&fields, uuid) != Some(3) {
        return None;
    }
    let style_kind = fields.get(1)?.trim().parse::<u8>().ok()?;
    let value = fields.get(2)?;
    match style_kind {
        0 => Some(StyleItemProperties {
            item_type: "Color",
            value_xml: format!(
                "<Value xsi:type=\"v8ui:Color\">{}</Value>",
                escape_xml_text(&parse_style_color_value(value)?)
            ),
        }),
        1 => Some(StyleItemProperties {
            item_type: "Font",
            value_xml: parse_style_font_value_xml(value),
        }),
        2 => Some(StyleItemProperties {
            item_type: "Border",
            value_xml: parse_style_border_value_xml(value)?,
        }),
        _ => None,
    }
}

fn parse_scheduled_job_properties_from_text(
    text: &str,
    uuid: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<ScheduledJobProperties> {
    let fields = metadata_object_fields(text)?;
    if metadata_header_field_index(&fields, uuid) != Some(1) {
        return None;
    }
    let key = fields
        .get(2)
        .and_then(|field| parse_1c_quoted_string(field.trim()))
        .unwrap_or_default();
    let description = fields
        .get(3)
        .and_then(|field| parse_1c_quoted_string(field.trim()))
        .unwrap_or_default();
    let use_job = parse_1c_bool_field(fields.get(4).copied())?;
    let predefined = parse_1c_bool_field(fields.get(5).copied())?;
    let module_uuid = parse_uuid_field(fields.get(6)?.trim())?;
    let module_ref = object_refs.get(&module_uuid)?;
    let method = fields
        .get(7)
        .and_then(|field| parse_1c_quoted_string(field.trim()))?;
    let restart_count_on_failure = parse_1c_u32_field(fields.get(8).copied())?;
    let restart_interval_on_failure = parse_1c_u32_field(fields.get(9).copied())?;

    Some(ScheduledJobProperties {
        method_name: format!("{module_ref}.{method}"),
        description,
        key,
        use_job,
        predefined,
        restart_count_on_failure,
        restart_interval_on_failure,
    })
}

fn parse_event_subscription_properties_from_text(
    text: &str,
    uuid: &str,
    type_index: &BTreeMap<String, String>,
    object_refs: &BTreeMap<String, String>,
) -> Option<EventSubscriptionProperties> {
    let fields = metadata_object_fields(text)?;
    if metadata_header_field_index(&fields, uuid) != Some(1) {
        return None;
    }
    let raw_event = fields
        .get(3)
        .and_then(|field| parse_1c_quoted_string(field.trim()))?;
    let event = raw_event
        .split_once('_')
        .map(|(event, _)| event)
        .unwrap_or(raw_event.as_str())
        .to_string();
    let source_types = parse_event_subscription_type_pattern(fields.get(2)?, type_index, &event)?;
    let module_uuid = parse_uuid_field(fields.get(4)?.trim())?;
    let module_ref = object_refs.get(&module_uuid)?;
    let method = fields
        .get(5)
        .and_then(|field| parse_1c_quoted_string(field.trim()))?;

    Some(EventSubscriptionProperties {
        source_types,
        event,
        handler: format!("{module_ref}.{method}"),
    })
}

fn extract_style_body_xml(bytes: &[u8], object_refs: &BTreeMap<String, String>) -> Result<String> {
    let inflated = inflate_raw_deflate(bytes)?;
    let text = String::from_utf8(inflated)?;
    let mut items = parse_style_body_items(text.trim_start_matches('\u{feff}'), object_refs)
        .context("failed to parse style body")?;
    items.sort_by(|left, right| {
        left.standard_order
            .unwrap_or(usize::MAX)
            .cmp(&right.standard_order.unwrap_or(usize::MAX))
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(format_style_body_xml(&items))
}

fn parse_style_body_items(
    text: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<Vec<StyleBodyItem>> {
    let fields = split_1c_braced_fields(text, 0)?;
    if fields.first()?.trim() != "2" {
        return None;
    }
    let declared_count = fields.get(1)?.trim().parse::<usize>().ok()?;
    let mut items = Vec::new();
    for field in fields.iter().skip(2) {
        if field.trim() == "{0}" {
            continue;
        }
        let entry = split_1c_braced_fields(field, 0)?;
        let (name, standard_order) = style_body_item_name(entry.first()?, object_refs)?;
        let value = entry.get(2)?;
        let value_xml = match entry.get(1)?.trim() {
            "0" => format!(
                "<Color>{}</Color>",
                escape_xml_text(&parse_style_body_color_value(value, object_refs)?)
            ),
            "1" => parse_style_body_font_xml(value, object_refs)?,
            "2" => parse_style_body_border_xml(value, object_refs)?,
            _ => return None,
        };
        items.push(StyleBodyItem {
            name,
            standard_order,
            value_xml,
        });
    }
    if items.len() != declared_count {
        return None;
    }
    Some(items)
}

fn style_body_item_name(
    key: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<(String, Option<usize>)> {
    let fields = split_1c_braced_fields(key, 0)?;
    if fields.len() == 1 {
        let code = fields.first()?.trim().parse::<i32>().ok()?;
        let (order, name) = standard_style_item_for_code(code)?;
        return Some((name.to_string(), Some(order)));
    }
    if fields.first()?.trim() == "0" {
        let uuid = parse_uuid_field(fields.get(1)?.trim())?;
        let name = object_refs
            .get(&uuid)
            .cloned()
            .unwrap_or_else(|| format!("StyleItem.{uuid}"));
        return Some((name, None));
    }
    None
}

fn style_body_ref_name(ref_value: &str, object_refs: &BTreeMap<String, String>) -> Option<String> {
    let fields = split_1c_braced_fields(ref_value, 0)?;
    if fields.len() == 1 {
        let code = fields.first()?.trim().parse::<i32>().ok()?;
        return standard_style_item_for_code(code).map(|(_, name)| format!("style:{name}"));
    }
    if fields.first()?.trim() == "0" {
        let uuid = parse_uuid_field(fields.get(1)?.trim())?;
        return object_refs
            .get(&uuid)
            .and_then(|reference| reference.strip_prefix("StyleItem."))
            .map(|name| format!("style:{name}"));
    }
    None
}

fn standard_style_item_for_code(code: i32) -> Option<(usize, &'static str)> {
    let name = match code {
        -1 => "FormBackColor",
        -11 => "FormTextColor",
        -3 => "ButtonBackColor",
        -15 => "ButtonTextColor",
        -7 => "FieldBackColor",
        -13 => "FieldTextColor",
        -21 => "FieldSelectionBackColor",
        -10 => "FieldSelectedTextColor",
        -14 => "FieldAlternativeBackColor",
        -23 => "ToolTipBackColor",
        -24 => "ToolTipTextColor",
        -16 => "SpecialTextColor",
        -17 => "NegativeTextColor",
        -22 => "BorderColor",
        -25 => "ReportHeaderBackColor",
        -26 => "ReportGroup1BackColor",
        -27 => "ReportGroup2BackColor",
        -28 => "ReportLineColor",
        -18 => "ControlBorder",
        -20 => "TextFont",
        -30 => "SmallTextFont",
        -31 => "NormalTextFont",
        -32 => "LargeTextFont",
        -33 => "ExtraLargeTextFont",
        -34 => "ButtonBorderColor",
        -35 => "TableHeaderBackColor",
        -36 => "TableHeaderTextColor",
        -37 => "TableFooterBackColor",
        -38 => "TableFooterTextColor",
        -42 => "NavigationColor",
        -43 => "AuxiliaryNavigationColor",
        -44 => "ActivityColor",
        _ => return None,
    };
    let order = STANDARD_STYLE_ITEM_CODES
        .iter()
        .position(|item_code| *item_code == code)?;
    Some((order, name))
}

const STANDARD_STYLE_ITEM_CODES: &[i32] = &[
    -1, -11, -3, -15, -7, -13, -21, -10, -14, -23, -24, -16, -17, -22, -25, -26, -27, -28, -18,
    -20, -30, -31, -32, -33, -34, -35, -36, -37, -38, -42, -43, -44,
];

fn parse_style_body_color_value(
    value: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    let fields = split_1c_braced_fields(value, 0)?;
    if fields.first()?.trim() != "4" {
        return None;
    }
    let variant = fields.get(1)?.trim().parse::<i32>().ok()?;
    let code_fields = split_1c_braced_fields(fields.get(2)?, 0)?;
    let code = code_fields.first()?.trim().parse::<i32>().ok()?;
    match variant {
        0 => parse_moxel_direct_color(&code.to_string()),
        2 => style_web_color_name(code).map(ToOwned::to_owned),
        3 => style_body_ref_name(fields.get(2)?, object_refs),
        _ => None,
    }
}

fn parse_style_body_font_xml(
    value: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    let fields = split_1c_braced_fields(value, 0)?;
    if fields.first()?.trim() != "8" {
        return None;
    }
    let kind = fields.get(1).map(|field| field.trim()).unwrap_or("2");
    let mut attrs = Vec::<(&str, String)>::new();
    if kind == "2" {
        let reference = fields
            .get(3)
            .and_then(|field| style_body_ref_name(field, object_refs))
            .unwrap_or_else(|| "style:TextFont".to_string());
        attrs.push(("ref", reference));
        attrs.push(("kind", "StyleItem".to_string()));
    } else {
        attrs.push(("kind", "Absolute".to_string()));
    }

    let weight = fields
        .get(4)
        .and_then(|field| field.trim().parse::<i32>().ok())
        .unwrap_or(400);
    let bold = weight >= 700;
    let italic = fields
        .get(5)
        .and_then(|field| parse_1c_bool_flag(field.trim()))
        .unwrap_or(false);
    let underline = fields
        .get(6)
        .and_then(|field| parse_1c_bool_flag(field.trim()))
        .unwrap_or(false);
    let strikeout = fields
        .get(7)
        .and_then(|field| parse_1c_bool_flag(field.trim()))
        .unwrap_or(false);
    if bold || italic || underline || strikeout {
        attrs.push(("bold", xml_bool(bold).to_string()));
        attrs.push(("italic", xml_bool(italic).to_string()));
        attrs.push(("underline", xml_bool(underline).to_string()));
        attrs.push(("strikeout", xml_bool(strikeout).to_string()));
    }
    if let Some(scale) = fields.get(9).map(|field| field.trim())
        && scale != "100"
    {
        attrs.push(("scale", scale.to_string()));
    }

    Some(format_empty_style_body_value("Font", &attrs))
}

fn parse_style_body_border_xml(
    value: &str,
    object_refs: &BTreeMap<String, String>,
) -> Option<String> {
    let fields = split_1c_braced_fields(value, 0)?;
    if fields.first()?.trim() != "3" {
        return None;
    }
    let reference = fields
        .get(2)
        .and_then(|field| style_body_ref_name(field, object_refs))
        .unwrap_or_else(|| "style:ControlBorder".to_string());
    Some(format_empty_style_body_value(
        "Border",
        &[("ref", reference)],
    ))
}

fn format_empty_style_body_value(element: &str, attrs: &[(&str, String)]) -> String {
    let mut xml = format!("<{element}");
    for (name, value) in attrs {
        xml.push_str(&format!(" {name}=\"{}\"", escape_xml_text(value)));
    }
    xml.push_str("/>");
    xml
}

fn format_style_body_xml(items: &[StyleBodyItem]) -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<Style xmlns=\"http://v8.1c.ru/8.3/xcf/extrnprops\" xmlns:pal=\"http://v8.1c.ru/8.1/data/ui/colors/palette\" xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\" xmlns:sys=\"http://v8.1c.ru/8.1/data/ui/fonts/system\" xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" xmlns:web=\"http://v8.1c.ru/8.1/data/ui/colors/web\" xmlns:win=\"http://v8.1c.ru/8.1/data/ui/colors/windows\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"2.21\">\r\n",
    );
    for item in items {
        xml.push_str(&format!(
            "\t<Item name=\"{}\">\r\n\t\t{}\r\n\t</Item>\r\n",
            escape_xml_text(&item.name),
            item.value_xml
        ));
    }
    xml.push_str("</Style>\r\n");
    xml
}

fn parse_style_color_value(value: &str) -> Option<String> {
    let fields = split_1c_braced_fields(value, 0)?;
    if fields.first()?.trim() != r##""#""## {
        return None;
    }
    let color_fields = split_1c_braced_fields(fields.get(3)?, 0)?;
    if color_fields.first()?.trim() != "3" {
        return None;
    }
    let variant = color_fields.get(1)?.trim().parse::<i32>().ok()?;
    let code_fields = split_1c_braced_fields(color_fields.get(2)?, 0)?;
    let code = code_fields.first()?.trim().parse::<i32>().ok()?;
    match variant {
        0 => {
            let color = code.max(0) as u32 & 0x00ff_ffff;
            let red = color & 0xff;
            let green = (color >> 8) & 0xff;
            let blue = (color >> 16) & 0xff;
            Some(format!("#{red:02X}{green:02X}{blue:02X}"))
        }
        2 => style_web_color_name(code).map(ToOwned::to_owned),
        3 => style_system_color_name(code).map(ToOwned::to_owned),
        _ => None,
    }
}

fn parse_style_border_value_xml(value: &str) -> Option<String> {
    let fields = split_1c_braced_fields(value, 0)?;
    if fields.first()?.trim() != r##""#""## {
        return None;
    }
    let border_fields = split_1c_braced_fields(fields.get(3)?, 0)?;
    if border_fields.first()?.trim() != "3" {
        return None;
    }
    let style = match border_fields.get(3)?.trim() {
        "0" => "WithoutBorder",
        "1" => "Single",
        _ => return None,
    };
    Some(format!(
        "<Value xsi:type=\"v8ui:Border\" width=\"1\">\r\n\
\t\t\t\t<v8ui:style xsi:type=\"v8ui:ControlBorderType\">{style}</v8ui:style>\r\n\
\t\t\t</Value>"
    ))
}

fn style_web_color_name(code: i32) -> Option<&'static str> {
    match code {
        8 => Some("web:Black"),
        10 => Some("web:Blue"),
        20 => Some("web:Cream"),
        21 => Some("web:Crimson"),
        26 => Some("web:DarkGray"),
        27 => Some("web:DarkGreen"),
        31 => Some("web:DarkGreen"),
        23 => Some("web:DarkBlue"),
        33 => Some("web:DarkRed"),
        37 => Some("web:DarkSlateGray"),
        44 => Some("web:FireBrick"),
        45 => Some("web:FloralWhite"),
        46 => Some("web:ForestGreen"),
        50 => Some("web:Gold"),
        51 => Some("web:Goldenrod"),
        48 => Some("web:Gainsboro"),
        52 => Some("web:Gray"),
        53 => Some("web:Green"),
        55 => Some("web:HoneyDew"),
        64 => Some("web:LightCoral"),
        65 => Some("web:LightBlue"),
        66 => Some("web:LightCoral"),
        67 => Some("web:LightCyan"),
        68 => Some("web:LightGoldenRod"),
        69 => Some("web:LightGoldenRodYellow"),
        71 => Some("web:LightGray"),
        72 => Some("web:LightPink"),
        79 => Some("web:LightYellow"),
        84 => Some("web:Maroon"),
        86 => Some("web:MediumBlue"),
        87 => Some("web:MediumGray"),
        94 => Some("web:Orange"),
        105 => Some("web:Orange"),
        97 => Some("web:MintCream"),
        98 => Some("web:MistyRose"),
        96 => Some("web:Moccasin"),
        99 => Some("web:Moccasin"),
        119 => Some("web:Red"),
        120 => Some("web:RosyBrown"),
        121 => Some("web:RoyalBlue"),
        128 => Some("web:Silver"),
        130 => Some("web:SlateBlue"),
        134 => Some("web:SteelBlue"),
        140 => Some("web:Violet"),
        141 => Some("web:VioletRed"),
        144 => Some("web:WhiteSmoke"),
        145 => Some("web:Yellow"),
        _ => None,
    }
}

fn style_system_color_name(code: i32) -> Option<&'static str> {
    match code {
        -1 => Some("style:FormBackColor"),
        -3 => Some("style:FormTextColor"),
        -11 => Some("style:FieldTextColor"),
        -15 => Some("style:ButtonTextColor"),
        -7 => Some("style:FieldBackColor"),
        -13 => Some("style:FieldTextColor"),
        -21 => Some("style:FieldSelectionBackColor"),
        -10 => Some("style:FieldSelectedTextColor"),
        -14 => Some("style:FieldAlternativeBackColor"),
        -23 => Some("style:ToolTipBackColor"),
        -24 => Some("style:ToolTipTextColor"),
        -16 => Some("style:SpecialTextColor"),
        -17 => Some("style:NegativeTextColor"),
        -22 => Some("style:BorderColor"),
        -25 => Some("style:ReportHeaderBackColor"),
        -26 => Some("style:ReportGroup1BackColor"),
        -27 => Some("style:ReportGroup2BackColor"),
        -28 => Some("style:ReportLineColor"),
        -34 => Some("style:ButtonBorderColor"),
        -35 => Some("style:TableHeaderBackColor"),
        -36 => Some("style:TableHeaderTextColor"),
        -37 => Some("style:TableFooterBackColor"),
        -38 => Some("style:TableFooterTextColor"),
        -42 => Some("style:NavigationColor"),
        -43 => Some("style:AuxiliaryNavigationColor"),
        -44 => Some("style:ActivityColor"),
        _ => None,
    }
}

fn parse_style_font_value_xml(value: &str) -> String {
    let fields = split_1c_braced_fields(value, 0).unwrap_or_default();
    let font_fields = fields
        .get(3)
        .and_then(|field| split_1c_braced_fields(field, 0))
        .unwrap_or_default();
    let kind = font_fields.get(1).map(|field| field.trim()).unwrap_or("0");
    let mut attrs = Vec::<(&str, String)>::new();
    let (height, weight, italic, underline, strikeout, kind_xml, scale, include_false_flags) =
        match kind {
            "0" => {
                attrs.push((
                    "faceName",
                    font_fields
                        .get(16)
                        .and_then(|field| parse_1c_quoted_string(field.trim()))
                        .unwrap_or_else(|| "Arial".to_string()),
                ));
                (
                    font_height_xml(font_fields.get(3).map(|field| field.trim())),
                    font_fields
                        .get(7)
                        .and_then(|field| field.trim().parse::<i32>().ok())
                        .unwrap_or(400),
                    font_fields
                        .get(8)
                        .and_then(|field| parse_1c_bool_flag(field.trim()))
                        .unwrap_or(false),
                    font_fields
                        .get(9)
                        .and_then(|field| parse_1c_bool_flag(field.trim()))
                        .unwrap_or(false),
                    font_fields
                        .get(10)
                        .and_then(|field| parse_1c_bool_flag(field.trim()))
                        .unwrap_or(false),
                    "Absolute",
                    font_fields
                        .get(18)
                        .map(|field| field.trim())
                        .unwrap_or("100"),
                    true,
                )
            }
            "1" => {
                attrs.push(("ref", "sys:DefaultGUIFont".to_string()));
                if font_fields.len() >= 11 {
                    (
                        font_height_xml(font_fields.get(4).map(|field| field.trim())),
                        font_fields
                            .get(5)
                            .and_then(|field| field.trim().parse::<i32>().ok())
                            .unwrap_or(400),
                        font_fields
                            .get(6)
                            .and_then(|field| parse_1c_bool_flag(field.trim()))
                            .unwrap_or(false),
                        font_fields
                            .get(7)
                            .and_then(|field| parse_1c_bool_flag(field.trim()))
                            .unwrap_or(false),
                        font_fields
                            .get(8)
                            .and_then(|field| parse_1c_bool_flag(field.trim()))
                            .unwrap_or(false),
                        "WindowsFont",
                        font_fields
                            .get(10)
                            .map(|field| field.trim())
                            .unwrap_or("100"),
                        true,
                    )
                } else if font_fields.len() >= 10 {
                    (
                        None,
                        font_fields
                            .get(4)
                            .and_then(|field| field.trim().parse::<i32>().ok())
                            .unwrap_or(400),
                        font_fields
                            .get(5)
                            .and_then(|field| parse_1c_bool_flag(field.trim()))
                            .unwrap_or(false),
                        font_fields
                            .get(6)
                            .and_then(|field| parse_1c_bool_flag(field.trim()))
                            .unwrap_or(false),
                        font_fields
                            .get(7)
                            .and_then(|field| parse_1c_bool_flag(field.trim()))
                            .unwrap_or(false),
                        "WindowsFont",
                        font_fields
                            .get(9)
                            .map(|field| field.trim())
                            .unwrap_or("100"),
                        true,
                    )
                } else {
                    (
                        None,
                        400,
                        false,
                        false,
                        false,
                        "WindowsFont",
                        font_fields
                            .get(5)
                            .map(|field| field.trim())
                            .unwrap_or("100"),
                        false,
                    )
                }
            }
            _ => {
                let raw = font_fields.get(3).copied().unwrap_or("{0}");
                let raw_fields = split_1c_braced_fields(raw, 0).unwrap_or_default();
                if let Some(code) = raw_fields
                    .first()
                    .and_then(|field| field.trim().parse::<i32>().ok())
                    && let Some((_, name)) = standard_style_item_for_code(code)
                {
                    attrs.push(("ref", format!("style:{name}")));
                }
                if font_fields.len() >= 11 {
                    (
                        font_height_xml(font_fields.get(4).map(|field| field.trim())),
                        font_fields
                            .get(5)
                            .and_then(|field| field.trim().parse::<i32>().ok())
                            .unwrap_or(400),
                        font_fields
                            .get(6)
                            .and_then(|field| parse_1c_bool_flag(field.trim()))
                            .unwrap_or(false),
                        font_fields
                            .get(7)
                            .and_then(|field| parse_1c_bool_flag(field.trim()))
                            .unwrap_or(false),
                        font_fields
                            .get(8)
                            .and_then(|field| parse_1c_bool_flag(field.trim()))
                            .unwrap_or(false),
                        "StyleItem",
                        font_fields
                            .get(10)
                            .map(|field| field.trim())
                            .unwrap_or("100"),
                        true,
                    )
                } else if font_fields.len() >= 10 {
                    (
                        None,
                        font_fields
                            .get(4)
                            .and_then(|field| field.trim().parse::<i32>().ok())
                            .unwrap_or(400),
                        font_fields
                            .get(5)
                            .and_then(|field| parse_1c_bool_flag(field.trim()))
                            .unwrap_or(false),
                        font_fields
                            .get(6)
                            .and_then(|field| parse_1c_bool_flag(field.trim()))
                            .unwrap_or(false),
                        font_fields
                            .get(7)
                            .and_then(|field| parse_1c_bool_flag(field.trim()))
                            .unwrap_or(false),
                        "StyleItem",
                        font_fields
                            .get(9)
                            .map(|field| field.trim())
                            .unwrap_or("100"),
                        true,
                    )
                } else if font_fields.get(2).map(|field| field.trim()) == Some("2")
                    && font_fields.len() >= 7
                {
                    (
                        font_height_xml(font_fields.get(4).map(|field| field.trim())),
                        400,
                        false,
                        false,
                        false,
                        "StyleItem",
                        font_fields
                            .get(6)
                            .map(|field| field.trim())
                            .unwrap_or("100"),
                        false,
                    )
                } else {
                    (
                        None,
                        font_fields
                            .get(4)
                            .and_then(|field| field.trim().parse::<i32>().ok())
                            .unwrap_or(400),
                        false,
                        font_fields
                            .get(5)
                            .and_then(|field| parse_1c_bool_flag(field.trim()))
                            .unwrap_or(false),
                        false,
                        "StyleItem",
                        font_fields
                            .get(7)
                            .map(|field| field.trim())
                            .unwrap_or("100"),
                        false,
                    )
                }
            }
        };
    if let Some(height) = height {
        attrs.push(("height", height));
    }
    let bold = weight >= 700;
    if include_false_flags || bold {
        attrs.push(("bold", xml_bool(bold).to_string()));
    }
    if include_false_flags || italic {
        attrs.push(("italic", xml_bool(italic).to_string()));
    }
    if include_false_flags || underline {
        attrs.push(("underline", xml_bool(underline).to_string()));
    }
    if include_false_flags || strikeout {
        attrs.push(("strikeout", xml_bool(strikeout).to_string()));
    }
    attrs.push(("kind", kind_xml.to_string()));
    if scale != "100" || kind == "0" {
        attrs.push(("scale", scale.to_string()));
    }

    let mut xml = String::from("<Value xsi:type=\"v8ui:Font\"");
    for (name, value) in attrs {
        xml.push_str(&format!(" {name}=\"{}\"", escape_xml_text(&value)));
    }
    xml.push_str("/>");
    xml
}

fn font_height_xml(raw: Option<&str>) -> Option<String> {
    let value = raw?.parse::<i32>().ok()?;
    let height = value / 10;
    if height == 0 {
        return None;
    }
    Some(height.to_string())
}

fn parse_metadata_type_pattern(
    value: &str,
    type_index: &BTreeMap<String, String>,
) -> Option<Vec<ConstantValueType>> {
    let fields = split_1c_braced_fields(value, 0)?;
    if fields.first()?.trim() != r#""Pattern""# {
        return None;
    }
    fields
        .iter()
        .skip(1)
        .map(|field| parse_metadata_type_pattern_element(field, type_index))
        .collect()
}

fn parse_event_subscription_type_pattern(
    value: &str,
    type_index: &BTreeMap<String, String>,
    event: &str,
) -> Option<Vec<ConstantValueType>> {
    let fields = split_1c_braced_fields(value, 0)?;
    if fields.first()?.trim() != r#""Pattern""# {
        return None;
    }
    let type_ids = fields
        .iter()
        .skip(1)
        .map(|field| metadata_type_pattern_field_type_id(field))
        .collect::<Option<Vec<_>>>()?;
    fields
        .iter()
        .skip(1)
        .map(|field| {
            parse_event_subscription_type_pattern_element(field, type_index, event, &type_ids)
        })
        .collect()
}

fn parse_event_subscription_type_pattern_element(
    value: &str,
    type_index: &BTreeMap<String, String>,
    event: &str,
    pattern_type_ids: &[String],
) -> Option<ConstantValueType> {
    let element = split_1c_braced_fields(value, 0)?;
    if element.first()?.trim() == r##""#""## && element.len() >= 2 {
        let type_id = parse_uuid_field(element.get(1)?.trim())?;
        let reference = type_index
            .get(&type_id)
            .cloned()
            .or_else(|| {
                event_subscription_builtin_type_reference(event, pattern_type_ids, &type_id)
                    .map(ToOwned::to_owned)
            })
            .or_else(|| builtin_type_reference(&type_id).map(ToOwned::to_owned))?;
        return Some(ConstantValueType::Reference { reference });
    }
    parse_metadata_type_pattern_element(value, type_index)
}

fn parse_metadata_type_pattern_element(
    value: &str,
    type_index: &BTreeMap<String, String>,
) -> Option<ConstantValueType> {
    let element = split_1c_braced_fields(value, 0)?;
    match element.first()?.trim() {
        r#""B""# => Some(ConstantValueType::Boolean),
        r#""S""# if element.len() == 1 => Some(ConstantValueType::String {
            length: None,
            allowed_length_flag: 0,
        }),
        r#""S""# if element.len() == 3 => Some(ConstantValueType::String {
            length: Some(element.get(1)?.trim().parse().ok()?),
            allowed_length_flag: element.get(2)?.trim().parse().ok()?,
        }),
        r#""N""# if element.len() == 1 => Some(ConstantValueType::Number {
            digits: 0,
            fraction_digits: 0,
            allowed_sign_flag: 0,
        }),
        r#""N""# if element.len() == 4 => Some(ConstantValueType::Number {
            digits: element.get(1)?.trim().parse().ok()?,
            fraction_digits: element.get(2)?.trim().parse().ok()?,
            allowed_sign_flag: element.get(3)?.trim().parse().ok()?,
        }),
        r#""D""# => Some(ConstantValueType::DateTime {
            date_fractions: match element.get(1).map(|field| field.trim()) {
                Some(r#""D""#) => "Date",
                Some(r#""T""#) => "Time",
                _ => "DateTime",
            },
        }),
        r##""#""## if element.len() >= 2 => {
            let type_id = parse_uuid_field(element.get(1)?.trim())?;
            let reference = type_index
                .get(&type_id)
                .cloned()
                .or_else(|| builtin_type_reference(&type_id).map(ToOwned::to_owned))?;
            Some(ConstantValueType::Reference { reference })
        }
        _ => None,
    }
}

fn metadata_type_pattern_field_type_id(value: &str) -> Option<String> {
    let element = split_1c_braced_fields(value, 0)?;
    if element.first()?.trim() == r##""#""## && element.len() >= 2 {
        parse_uuid_field(element.get(1)?.trim())
    } else {
        Some(String::new())
    }
}

fn event_subscription_builtin_type_reference(
    event: &str,
    pattern_type_ids: &[String],
    type_id: &str,
) -> Option<&'static str> {
    if event == "FillCheckProcessing" {
        return match type_id {
            "3e63355c-1378-4953-be9b-1deb5fb6bec5" => Some("cfg:BusinessProcessObject"),
            _ => None,
        };
    }
    if pattern_type_ids.len() == 4
        && pattern_type_ids
            == [
                "238e7e88-3c5f-48b2-8a3b-81ebbecb20ed",
                "30b100d6-b29f-47ac-aec7-cb8ca8a54767",
                "82a1b659-b220-4d94-a9bd-14d757b95a48",
                "cf4abea6-37b2-11d4-940f-008048da11f9",
            ]
    {
        return match type_id {
            "238e7e88-3c5f-48b2-8a3b-81ebbecb20ed" => Some("cfg:ChartOfAccountsObject"),
            _ => None,
        };
    }
    if pattern_type_ids.len() == 1
        && pattern_type_ids.first().map(String::as_str)
            == Some("fcd3404e-1523-48ce-9bc0-ecdb822684a1")
        && matches!(event, "BeforeWrite" | "OnSetNewNumber")
    {
        return Some("cfg:BusinessProcessObject");
    }
    None
}

fn builtin_type_reference(type_id: &str) -> Option<&'static str> {
    match type_id {
        "acf6192e-81ca-46ef-93a6-5a6968b78663" => Some("v8:ValueTable"),
        "140b5ff4-37b1-4df5-b5ec-a0bfd2b94f8f" => Some("v8ui:FormattedString"),
        "9cd510c7-abfc-11d4-9434-004095e12fc7" => Some("v8ui:Color"),
        "9cd510c8-abfc-11d4-9434-004095e12fc7" => Some("v8ui:Font"),
        "e199ca70-93cf-46ce-a54b-6edc88c3a296" => Some("v8:ValueStorage"),
        "e603c0f2-92fb-4d47-8f38-a44a381cf235" => Some("v8:ValueTree"),
        "fc01b5df-97fe-449b-83d4-218a090e681e" => Some("v8:UUID"),
        "3ee983d7-ace7-40f9-bb7e-2e916fcddd56" => Some("v8:FixedStructure"),
        "4500381b-db30-4a10-9db4-990038032acf" => Some("v8:FixedArray"),
        "220455ea-6c85-4513-996f-bbe79ed07774" => Some("v8:FixedMap"),
        "2fdc88ec-7c9b-43cd-8ba5-873f043bdd88" => Some("v8:StandardPeriod"),
        "4772b3b4-f4a3-49c0-a1a5-8cb5961511a3" => Some("v8:ValueListType"),
        "e603103e-a318-4edc-a014-b1c6cf94d49f" => Some("mxl:SpreadsheetDocument"),
        "0a52f9de-73ea-4507-81e8-66217bead73a" => Some("cfg:ExchangePlanRef"),
        "280f5f0e-9c8a-49cc-bf6d-4d296cc17a63" => Some("cfg:AnyIBRef"),
        "0195e80c-b157-11d4-9435-004095e12fc7" => Some("cfg:ConstantValueManager"),
        "061d872a-5787-460e-95ac-ed74ea3a3e84" => Some("cfg:DocumentObject"),
        "238e7e88-3c5f-48b2-8a3b-81ebbecb20ed" => Some("cfg:BusinessProcessObject"),
        "30b100d6-b29f-47ac-aec7-cb8ca8a54767" => Some("cfg:ChartOfCalculationTypesObject"),
        "3e63355c-1378-4953-be9b-1deb5fb6bec5" => Some("cfg:ChartOfAccountsObject"),
        "82a1b659-b220-4d94-a9bd-14d757b95a48" => Some("cfg:ChartOfCharacteristicTypesObject"),
        "857c4a91-e5f4-4fac-86ec-787626f1c108" => Some("cfg:ExchangePlanObject"),
        "cf4abea6-37b2-11d4-940f-008048da11f9" => Some("cfg:CatalogObject"),
        "fcd3404e-1523-48ce-9bc0-ecdb822684a1" => Some("cfg:TaskObject"),
        "13134201-f60b-11d5-a3c7-0050bae0a776" => Some("cfg:InformationRegisterRecordSet"),
        "2deed9b8-0056-4ffe-a473-c20a6c32a0bc" => Some("cfg:AccountingRegisterRecordSet"),
        "b64d9a40-1642-11d6-a3c7-0050bae0a776" => Some("cfg:AccumulationRegisterRecordSet"),
        "f2de87a8-64e5-45eb-a22d-b3aedab050e7" => Some("cfg:CalculationRegisterRecordSet"),
        "274bf899-db0e-4df6-8ab5-67bf6371ec0b" => Some("cfg:SequenceRecordSet"),
        "bc587f20-35d9-11d6-a3c7-0050bae0a776" => Some("cfg:RecalculationRecordSet"),
        "0dee6ca3-50a1-4f94-8c34-e70eeb802d81" => Some("cfg:AccumulationRegisterManager"),
        "1aa09f48-f6d5-4999-a7f5-02a15794c795" => Some("cfg:InformationRegisterManager"),
        "2066866d-9d38-47fe-a272-3cd416eb9c85" => Some("cfg:ChartOfAccountsManager"),
        "26dd1dee-252a-4942-b4b5-62ea44ed8030" => Some("cfg:DocumentManager"),
        "2d0abc8e-dede-4184-afd7-7ae8da588d47" => Some("cfg:CalculationRegisterManager"),
        "38f1038d-8b0b-438b-bfbe-830a60a1153a" => Some("cfg:BusinessProcessManager"),
        "38bfd075-3e63-4aaa-a93e-94521380d579" => Some("cfg:DocumentRef"),
        "3ab47eda-6a5c-4590-9b08-0e633aa2f376" => Some("cfg:AccountingRegisterManager"),
        "3eab4ff4-f2d1-4c96-831c-04711b093999" => Some("cfg:ChartOfCalculationTypesManager"),
        "5e268c17-8035-458f-8041-daf9b15d05c9" => Some("cfg:TaskManager"),
        "7612de75-8b10-466a-b235-68572c605d92" => Some("cfg:ChartOfCharacteristicTypesManager"),
        "82faabf3-7f9b-4b2e-b499-98876415f270" => Some("cfg:CatalogManager"),
        "92e7f73f-bd66-4d9e-bc43-bae2acfadfd5" => Some("cfg:DocumentJournalManager"),
        "e61ef7b8-f3e1-4f4b-8ac7-676e90524997" => Some("cfg:CatalogRef"),
        _ => None,
    }
}

fn split_1c_braced_fields(text: &str, start: usize) -> Option<Vec<&str>> {
    let end = scan_1c_braced_value(text, start)?;
    let inner_start = start + text[start..].chars().next()?.len_utf8();
    let inner_end = end.checked_sub(1)?;
    let inner = &text[inner_start..inner_end];
    let mut fields = Vec::new();
    let mut field_start = 0usize;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut chars = inner.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        if in_string {
            if ch == '"' {
                if let Some((_, next)) = chars.peek()
                    && *next == '"'
                {
                    let _ = chars.next();
                    continue;
                }
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => depth = depth.checked_sub(1)?,
            ',' if depth == 0 => {
                fields.push(inner[field_start..index].trim());
                field_start = index + ch.len_utf8();
            }
            _ => {}
        }
    }
    fields.push(inner[field_start..].trim());
    Some(fields)
}

fn template_type_code_from_metadata_text(text: &str, uuid: &str) -> Option<u32> {
    let fields = metadata_object_fields(text)?;
    match parse_metadata_object_code(text)? {
        2 if metadata_header_field_index(&fields, uuid) == Some(2) => {
            fields.get(1)?.trim().parse().ok()
        }
        4 if is_common_template_metadata_fields(&fields, uuid) => {
            fields.get(2)?.trim().parse().ok()
        }
        _ => None,
    }
}

fn parse_1c_quoted_string_with_len(input: &str) -> Option<(String, usize)> {
    let mut chars = input.char_indices();
    if chars.next()?.1 != '"' {
        return None;
    }
    let mut output = String::new();
    while let Some((index, ch)) = chars.next() {
        if ch == '"' {
            if let Some((_, next)) = chars.clone().next()
                && next == '"'
            {
                output.push('"');
                let _ = chars.next();
                continue;
            }
            return Some((output, index + ch.len_utf8()));
        }
        output.push(ch);
    }
    None
}

fn parse_1c_quoted_string(input: &str) -> Option<String> {
    parse_1c_quoted_string_with_len(input).map(|(value, _)| value)
}

fn parse_1c_synonyms(input: &str) -> Vec<(String, String)> {
    let mut values = Vec::new();
    let mut offset = 0;
    while let Some(relative) = input[offset..].find('"') {
        offset += relative;
        let Some((value, consumed)) = parse_1c_quoted_string_with_len(&input[offset..]) else {
            break;
        };
        values.push(value);
        offset += consumed;
    }

    values
        .chunks(2)
        .filter_map(|chunk| match chunk {
            [lang, content] => Some((lang.clone(), content.clone())),
            _ => None,
        })
        .collect()
}

fn scan_1c_braced_value(text: &str, start: usize) -> Option<usize> {
    if text[start..].chars().next()? != '{' {
        return None;
    }
    let mut depth = 0usize;
    let mut in_string = false;
    let mut chars = text[start..].char_indices().peekable();
    while let Some((relative, ch)) = chars.next() {
        if in_string {
            if ch == '"' {
                if let Some((_, next)) = chars.peek()
                    && *next == '"'
                {
                    let _ = chars.next();
                    continue;
                }
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(start + relative + ch.len_utf8());
                }
            }
            _ => {}
        }
    }
    None
}

fn skip_ascii_ws_at(text: &str, mut offset: usize) -> usize {
    while let Some(byte) = text.as_bytes().get(offset)
        && byte.is_ascii_whitespace()
    {
        offset += 1;
    }
    offset
}

fn expect_comma_at(text: &str, offset: usize) -> Option<usize> {
    let offset = skip_ascii_ws_at(text, offset);
    if text.as_bytes().get(offset) == Some(&b',') {
        Some(offset + 1)
    } else {
        None
    }
}

fn format_common_picture_source_xml(
    header: &MetadataHeader,
    picture: &CommonPictureProperties,
    source_version: InfobaseConfigSourceVersion,
) -> String {
    let mut xml = format!(
        "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:app=\"http://v8.1c.ru/8.2/managed-application/core\" xmlns:cfg=\"http://v8.1c.ru/8.1/data/enterprise/current-config\" xmlns:cmi=\"http://v8.1c.ru/8.2/managed-application/cmi\" xmlns:ent=\"http://v8.1c.ru/8.1/data/enterprise\" xmlns:lf=\"http://v8.1c.ru/8.2/managed-application/logform\" xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\" xmlns:sys=\"http://v8.1c.ru/8.1/data/ui/fonts/system\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" xmlns:web=\"http://v8.1c.ru/8.1/data/ui/colors/web\" xmlns:win=\"http://v8.1c.ru/8.1/data/ui/colors/windows\" xmlns:xen=\"http://v8.1c.ru/8.3/xcf/enums\" xmlns:xpr=\"http://v8.1c.ru/8.3/xcf/predef\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"{}\">\r\n\
\t<CommonPicture uuid=\"{}\">\r\n\
\t\t<Properties>\r\n\
\t\t\t<Name>{}</Name>\r\n",
        source_version.as_str(),
        escape_xml_text(&header.uuid),
        escape_xml_text(&header.name)
    );
    if header.synonyms.is_empty() {
        xml.push_str("\t\t\t<Synonym/>\r\n");
    } else {
        xml.push_str("\t\t\t<Synonym>\r\n");
        for (lang, content) in &header.synonyms {
            xml.push_str("\t\t\t\t<v8:item>\r\n");
            xml.push_str(&format!(
                "\t\t\t\t\t<v8:lang>{}</v8:lang>\r\n",
                escape_xml_text(lang)
            ));
            xml.push_str(&format!(
                "\t\t\t\t\t<v8:content>{}</v8:content>\r\n",
                escape_xml_element_text(content)
            ));
            xml.push_str("\t\t\t\t</v8:item>\r\n");
        }
        xml.push_str("\t\t\t</Synonym>\r\n");
    }
    if header.comment.is_empty() {
        xml.push_str("\t\t\t<Comment/>\r\n");
    } else {
        xml.push_str(&format!(
            "\t\t\t<Comment>{}</Comment>\r\n",
            escape_xml_text(&header.comment)
        ));
    }
    xml.push_str(&format!(
        "\t\t\t<AvailabilityForChoice>{}</AvailabilityForChoice>\r\n\
\t\t\t<AvailabilityForAppearance>{}</AvailabilityForAppearance>\r\n\
\t\t</Properties>\r\n\
\t</CommonPicture>\r\n\
</MetaDataObject>",
        xml_bool(picture.availability_for_choice),
        xml_bool(picture.availability_for_appearance)
    ));
    xml
}

fn format_full_metadata_source_xml(
    kind: &str,
    header: &MetadataHeader,
    source_version: InfobaseConfigSourceVersion,
) -> String {
    let mut xml = format!(
        "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:app=\"http://v8.1c.ru/8.2/managed-application/core\" xmlns:cfg=\"http://v8.1c.ru/8.1/data/enterprise/current-config\" xmlns:cmi=\"http://v8.1c.ru/8.2/managed-application/cmi\" xmlns:ent=\"http://v8.1c.ru/8.1/data/enterprise\" xmlns:lf=\"http://v8.1c.ru/8.2/managed-application/logform\" xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\" xmlns:sys=\"http://v8.1c.ru/8.1/data/ui/fonts/system\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" xmlns:web=\"http://v8.1c.ru/8.1/data/ui/colors/web\" xmlns:win=\"http://v8.1c.ru/8.1/data/ui/colors/windows\" xmlns:xen=\"http://v8.1c.ru/8.3/xcf/enums\" xmlns:xpr=\"http://v8.1c.ru/8.3/xcf/predef\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"{}\">\r\n\
\t<{kind} uuid=\"{}\">\r\n\
\t\t<Properties>\r\n\
\t\t\t<Name>{}</Name>\r\n",
        source_version.as_str(),
        escape_xml_text(&header.uuid),
        escape_xml_element_text(&header.name)
    );
    if header.synonyms.is_empty() {
        xml.push_str("\t\t\t<Synonym/>\r\n");
    } else {
        xml.push_str("\t\t\t<Synonym>\r\n");
        for (lang, content) in &header.synonyms {
            xml.push_str("\t\t\t\t<v8:item>\r\n");
            xml.push_str(&format!(
                "\t\t\t\t\t<v8:lang>{}</v8:lang>\r\n",
                escape_xml_element_text(lang)
            ));
            xml.push_str(&format!(
                "\t\t\t\t\t<v8:content>{}</v8:content>\r\n",
                escape_xml_element_text(content)
            ));
            xml.push_str("\t\t\t\t</v8:item>\r\n");
        }
        xml.push_str("\t\t\t</Synonym>\r\n");
    }
    if header.comment.is_empty() {
        xml.push_str("\t\t\t<Comment/>\r\n");
    } else {
        xml.push_str(&format!(
            "\t\t\t<Comment>{}</Comment>\r\n",
            escape_xml_element_text(&header.comment)
        ));
    }
    xml.push_str(&format!(
        "\t\t</Properties>\r\n\
\t</{kind}>\r\n\
</MetaDataObject>"
    ));
    xml
}

fn format_default_list_form_metadata_source_xml(
    kind: &str,
    header: &MetadataHeader,
    properties: &DefaultListFormMetadataProperties,
    source_version: InfobaseConfigSourceVersion,
) -> String {
    let mut xml = format_full_metadata_source_xml(kind, header, source_version);
    if let Some(index) = xml.find("\t\t</Properties>") {
        let mut properties_xml = String::new();
        if let Some(use_standard_commands) = properties.use_standard_commands {
            properties_xml.push_str(&format!(
                "\t\t\t<UseStandardCommands>{}</UseStandardCommands>\r\n",
                xml_bool(use_standard_commands)
            ));
        }
        push_optional_text_element(
            &mut properties_xml,
            "\t\t\t",
            "DefaultListForm",
            properties.default_list_form.as_deref(),
        );
        xml.insert_str(index, &properties_xml);
    }
    xml
}

fn format_metadata_source_xml(
    kind: &str,
    header: &MetadataHeader,
    source_version: InfobaseConfigSourceVersion,
) -> String {
    let mut synonyms = String::new();
    if header.synonyms.is_empty() {
        synonyms.push_str("\t\t\t<Synonym/>\r\n");
    } else {
        synonyms.push_str("\t\t\t<Synonym>\r\n");
        for (lang, content) in &header.synonyms {
            synonyms.push_str("\t\t\t\t<v8:item>\r\n");
            synonyms.push_str(&format!(
                "\t\t\t\t\t<v8:lang>{}</v8:lang>\r\n",
                escape_xml_text(lang)
            ));
            synonyms.push_str(&format!(
                "\t\t\t\t\t<v8:content>{}</v8:content>\r\n",
                escape_xml_text(content)
            ));
            synonyms.push_str("\t\t\t\t</v8:item>\r\n");
        }
        synonyms.push_str("\t\t\t</Synonym>\r\n");
    }

    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" version=\"{source_version}\">\r\n\
\t<{kind} uuid=\"{uuid}\">\r\n\
\t\t<Properties>\r\n\
\t\t\t<Name>{name}</Name>\r\n\
{synonyms}\
\t\t\t<Comment>{comment}</Comment>\r\n\
\t\t</Properties>\r\n\
\t</{kind}>\r\n\
</MetaDataObject>\r\n",
        uuid = escape_xml_text(&header.uuid),
        name = escape_xml_text(&header.name),
        comment = escape_xml_text(&header.comment),
        source_version = source_version.as_str(),
    )
}

fn format_configuration_source_xml(
    header: &MetadataHeader,
    properties: &ConfigurationProperties,
    source_version: InfobaseConfigSourceVersion,
) -> String {
    let mut xml = format_full_metadata_source_xml("Configuration", header, source_version);
    let mut insert = String::new();
    push_optional_simple_property_xml(&mut insert, "NamePrefix", properties.name_prefix.as_deref());
    push_optional_simple_property_xml(
        &mut insert,
        "ConfigurationExtensionCompatibilityMode",
        properties
            .configuration_extension_compatibility_mode
            .as_deref(),
    );
    push_optional_simple_property_xml(&mut insert, "DefaultRunMode", properties.default_run_mode);
    if !properties.use_purposes.is_empty() {
        insert.push_str("\t\t\t<UsePurposes>\r\n");
        for purpose in &properties.use_purposes {
            insert.push_str(&format!(
                "\t\t\t\t<v8:Value xsi:type=\"app:ApplicationUsePurpose\">{}</v8:Value>\r\n",
                escape_xml_element_text(purpose)
            ));
        }
        insert.push_str("\t\t\t</UsePurposes>\r\n");
    }
    push_optional_simple_property_xml(
        &mut insert,
        "DefaultStyle",
        properties.default_style.as_deref(),
    );
    push_optional_simple_property_xml(
        &mut insert,
        "DefaultLanguage",
        properties.default_language.as_deref(),
    );
    push_optional_simple_property_xml(&mut insert, "ScriptVariant", properties.script_variant);
    if !properties.default_roles.is_empty() {
        insert.push_str("\t\t\t<DefaultRoles>\r\n");
        for role in &properties.default_roles {
            insert.push_str(&format!(
                "\t\t\t\t<xr:Item xsi:type=\"xr:MDObjectRef\">{}</xr:Item>\r\n",
                escape_xml_element_text(role)
            ));
        }
        insert.push_str("\t\t\t</DefaultRoles>\r\n");
    }
    push_optional_simple_property_xml(&mut insert, "Vendor", properties.vendor.as_deref());
    push_optional_simple_property_xml(&mut insert, "Version", properties.version.as_deref());
    push_optional_simple_property_xml(
        &mut insert,
        "UpdateCatalogAddress",
        properties.update_catalog_address.as_deref(),
    );
    push_optional_root_reference_xml(
        &mut insert,
        "CommonSettingsStorage",
        properties.common_settings_storage.as_ref(),
    );
    push_optional_root_reference_xml(
        &mut insert,
        "ReportsUserSettingsStorage",
        properties.reports_user_settings_storage.as_ref(),
    );
    push_optional_root_reference_xml(
        &mut insert,
        "ReportsVariantsStorage",
        properties.reports_variants_storage.as_ref(),
    );
    push_optional_root_reference_xml(
        &mut insert,
        "FormDataSettingsStorage",
        properties.form_data_settings_storage.as_ref(),
    );
    if !properties
        .used_mobile_application_functionalities
        .is_empty()
    {
        insert.push_str("\t\t\t<UsedMobileApplicationFunctionalities>\r\n");
        for functionality in &properties.used_mobile_application_functionalities {
            insert.push_str("\t\t\t\t<app:functionality>\r\n");
            insert.push_str(&format!(
                "\t\t\t\t\t<app:functionality>{}</app:functionality>\r\n",
                escape_xml_element_text(functionality.name)
            ));
            insert.push_str(&format!(
                "\t\t\t\t\t<app:use>{}</app:use>\r\n",
                xml_bool(functionality.use_functionality)
            ));
            insert.push_str("\t\t\t\t</app:functionality>\r\n");
        }
        insert.push_str("\t\t\t</UsedMobileApplicationFunctionalities>\r\n");
    }
    if let Some(localized) = &properties.localized_properties {
        push_localized_property(
            &mut insert,
            "\t\t\t",
            "BriefInformation",
            &localized.brief_information,
        );
        push_localized_property(
            &mut insert,
            "\t\t\t",
            "DetailedInformation",
            &localized.detailed_information,
        );
        push_localized_property(&mut insert, "\t\t\t", "Copyright", &localized.copyright);
        push_localized_property(
            &mut insert,
            "\t\t\t",
            "VendorInformationAddress",
            &localized.vendor_information_address,
        );
        push_localized_property(
            &mut insert,
            "\t\t\t",
            "ConfigurationInformationAddress",
            &localized.configuration_information_address,
        );
    } else {
        push_optional_localized_property_xml(
            &mut insert,
            "BriefInformation",
            &properties.brief_information,
        );
        push_optional_localized_property_xml(
            &mut insert,
            "DetailedInformation",
            &properties.detailed_information,
        );
        push_optional_localized_property_xml(&mut insert, "Copyright", &properties.copyright);
        push_optional_localized_property_xml(
            &mut insert,
            "VendorInformationAddress",
            &properties.vendor_information_address,
        );
        push_optional_localized_property_xml(
            &mut insert,
            "ConfigurationInformationAddress",
            &properties.configuration_information_address,
        );
    }
    push_optional_simple_property_xml(
        &mut insert,
        "CompatibilityMode",
        properties.compatibility_mode.as_deref(),
    );
    insert_metadata_properties_xml(&mut xml, &insert);
    xml
}

fn insert_configuration_internal_info_xml(
    xml: &mut String,
    contained_objects: &[ConfigurationContainedObject],
) {
    if contained_objects.is_empty() {
        return;
    }
    let Some(index) = xml.find("\t\t<Properties>\r\n") else {
        return;
    };
    let mut internal_info = "\t\t<InternalInfo>\r\n".to_string();
    for object in contained_objects {
        internal_info.push_str(&format!(
            "\t\t\t<xr:ContainedObject>\r\n\
\t\t\t\t<xr:ClassId>{}</xr:ClassId>\r\n\
\t\t\t\t<xr:ObjectId>{}</xr:ObjectId>\r\n\
\t\t\t</xr:ContainedObject>\r\n",
            escape_xml_element_text(&object.class_id),
            escape_xml_element_text(&object.object_id),
        ));
    }
    internal_info.push_str("\t\t</InternalInfo>\r\n");
    xml.insert_str(index, &internal_info);
}

fn insert_configuration_root_child_objects_xml(
    xml: &mut String,
    child_objects: &[ConfigurationRootChildObject],
) {
    let Some(index) = xml.find("\t</Configuration>") else {
        return;
    };
    if child_objects.is_empty() {
        xml.insert_str(index, "\t\t<ChildObjects/>\r\n");
        return;
    }
    let mut children = "\t\t<ChildObjects>\r\n".to_string();
    for child in child_objects {
        children.push_str(&format!(
            "\t\t\t<{}>{}</{}>\r\n",
            child.kind,
            escape_xml_element_text(&child.name),
            child.kind,
        ));
    }
    children.push_str("\t\t</ChildObjects>\r\n");
    xml.insert_str(index, &children);
}

fn push_optional_simple_property_xml(xml: &mut String, name: &str, value: Option<&str>) {
    let Some(value) = value else {
        return;
    };
    xml.push_str("\t\t\t");
    xml.push_str(&format_simple_property_xml(name, value));
    xml.push_str("\r\n");
}

fn push_optional_localized_property_xml(xml: &mut String, name: &str, values: &[(String, String)]) {
    if values.is_empty() {
        return;
    }
    xml.push_str("\t\t\t<");
    xml.push_str(name);
    xml.push_str(">\r\n");
    for (lang, content) in values {
        xml.push_str("\t\t\t\t<v8:item>\r\n");
        xml.push_str(&format!(
            "\t\t\t\t\t<v8:lang>{}</v8:lang>\r\n",
            escape_xml_element_text(lang)
        ));
        xml.push_str(&format!(
            "\t\t\t\t\t<v8:content>{}</v8:content>\r\n",
            escape_xml_element_text(content)
        ));
        xml.push_str("\t\t\t\t</v8:item>\r\n");
    }
    xml.push_str("\t\t\t</");
    xml.push_str(name);
    xml.push_str(">\r\n");
}

fn push_optional_root_reference_xml(
    xml: &mut String,
    name: &str,
    reference: Option<&ConfigurationRootReference>,
) {
    let Some(reference) = reference else {
        return;
    };
    xml.push_str("\t\t\t");
    xml.push_str(&format_simple_property_xml(
        name,
        reference.value.as_deref().unwrap_or_default(),
    ));
    xml.push_str("\r\n");
}

fn format_subsystem_source_xml(
    header: &MetadataHeader,
    subsystem: &SubsystemProperties,
    source_version: InfobaseConfigSourceVersion,
) -> String {
    let mut xml = format_full_metadata_source_xml("Subsystem", header, source_version);
    if let Some(offset) = xml.find("\t\t</Properties>") {
        let mut properties = format!(
            "\t\t\t<IncludeHelpInContents>{}</IncludeHelpInContents>\r\n\
\t\t\t<IncludeInCommandInterface>{}</IncludeInCommandInterface>\r\n\
\t\t\t<UseOneCommand>{}</UseOneCommand>\r\n",
            xml_bool(subsystem.include_help_in_contents),
            xml_bool(subsystem.include_in_command_interface),
            xml_bool(subsystem.use_one_command)
        );
        push_localized_property(
            &mut properties,
            "\t\t\t",
            "Explanation",
            &subsystem.explanation,
        );
        if let Some(reference) = &subsystem.picture_ref {
            properties.push_str(&format!(
                "\t\t\t<Picture>\r\n\
\t\t\t\t<xr:Ref>{}</xr:Ref>\r\n\
\t\t\t\t<xr:LoadTransparent>{}</xr:LoadTransparent>\r\n\
\t\t\t</Picture>\r\n",
                escape_xml_text(reference),
                xml_bool(subsystem.picture_load_transparent)
            ));
        } else {
            properties.push_str("\t\t\t<Picture/>\r\n");
        }
        if subsystem.content.is_empty() {
            properties.push_str("\t\t\t<Content/>\r\n");
        } else {
            properties.push_str("\t\t\t<Content>\r\n");
            for reference in &subsystem.content {
                properties.push_str(&format!(
                    "\t\t\t\t<xr:Item xsi:type=\"xr:MDObjectRef\">{}</xr:Item>\r\n",
                    escape_xml_text(reference)
                ));
            }
            properties.push_str("\t\t\t</Content>\r\n");
        }
        xml.insert_str(offset, &properties);
    }
    if let Some(offset) = xml.find("\t</Subsystem>") {
        if subsystem.child_subsystems.is_empty() {
            xml.insert_str(offset, "\t\t<ChildObjects/>\r\n");
            return xml;
        }
        let mut child_objects = "\t\t<ChildObjects>\r\n".to_string();
        for reference in &subsystem.child_subsystems {
            child_objects.push_str(&format!(
                "\t\t\t<Subsystem>{}</Subsystem>\r\n",
                escape_xml_text(reference)
            ));
        }
        child_objects.push_str("\t\t</ChildObjects>\r\n");
        xml.insert_str(offset, &child_objects);
    }
    xml
}

fn format_exchange_plan_source_xml(
    header: &MetadataHeader,
    exchange_plan: &ExchangePlanProperties,
    source_version: InfobaseConfigSourceVersion,
) -> String {
    let mut xml = format_full_metadata_source_xml("ExchangePlan", header, source_version);
    // V2.21 palette namespace placement follows the shared metadata-source formatter branch;
    // an ExchangePlan-specific native V2.21 byte gate is still pending.
    if source_version == InfobaseConfigSourceVersion::V2_21 {
        const STYLE_NAMESPACE_ATTRIBUTE: &str =
            " xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\"";
        const PALETTE_NAMESPACE_ATTRIBUTE: &str =
            " xmlns:pal=\"http://v8.1c.ru/8.1/data/ui/colors/palette\"";
        if let Some(style_index) = xml.find(STYLE_NAMESPACE_ATTRIBUTE) {
            xml.insert_str(style_index, PALETTE_NAMESPACE_ATTRIBUTE);
        }
    }
    let internal_info = format_exchange_plan_internal_info_xml(exchange_plan);
    if let Some(index) = xml.find("\t\t<Properties>\r\n") {
        xml.insert_str(index, &internal_info);
    }
    if let Some(index) = xml.find("\t\t</Properties>") {
        let mut properties = format!(
            "\t\t\t<UseStandardCommands>{}</UseStandardCommands>\r\n\
\t\t\t<CodeLength>{}</CodeLength>\r\n\
\t\t\t<CodeAllowedLength>{}</CodeAllowedLength>\r\n\
\t\t\t<DescriptionLength>{}</DescriptionLength>\r\n\
\t\t\t<DefaultPresentation>{}</DefaultPresentation>\r\n\
\t\t\t<EditType>{}</EditType>\r\n\
\t\t\t<QuickChoice>{}</QuickChoice>\r\n\
\t\t\t<ChoiceMode>{}</ChoiceMode>\r\n",
            xml_bool(exchange_plan.use_standard_commands),
            exchange_plan.code_length,
            exchange_plan.code_allowed_length,
            exchange_plan.description_length,
            exchange_plan.default_presentation,
            exchange_plan.edit_type,
            xml_bool(exchange_plan.quick_choice),
            exchange_plan.choice_mode,
        );
        push_catalog_input_by_string_xml(&mut properties, &exchange_plan.input_by_string);
        properties.push_str(&format!(
            "\t\t\t<SearchStringModeOnInputByString>{}</SearchStringModeOnInputByString>\r\n\
\t\t\t<FullTextSearchOnInputByString>{}</FullTextSearchOnInputByString>\r\n\
\t\t\t<ChoiceDataGetModeOnInputByString>{}</ChoiceDataGetModeOnInputByString>\r\n",
            exchange_plan.search_string_mode_on_input_by_string,
            exchange_plan.full_text_search_on_input_by_string,
            exchange_plan.choice_data_get_mode_on_input_by_string,
        ));
        push_exchange_plan_form_xml(
            &mut properties,
            "DefaultObjectForm",
            exchange_plan.default_object_form.as_deref(),
        );
        push_exchange_plan_form_xml(
            &mut properties,
            "DefaultListForm",
            exchange_plan.default_list_form.as_deref(),
        );
        push_exchange_plan_form_xml(
            &mut properties,
            "DefaultChoiceForm",
            exchange_plan.default_choice_form.as_deref(),
        );
        push_exchange_plan_form_xml(
            &mut properties,
            "AuxiliaryObjectForm",
            exchange_plan.auxiliary_object_form.as_deref(),
        );
        push_exchange_plan_form_xml(
            &mut properties,
            "AuxiliaryListForm",
            exchange_plan.auxiliary_list_form.as_deref(),
        );
        push_exchange_plan_form_xml(
            &mut properties,
            "AuxiliaryChoiceForm",
            exchange_plan.auxiliary_choice_form.as_deref(),
        );
        push_register_standard_attributes_xml(&mut properties, &exchange_plan.standard_attributes);
        properties.push_str("\t\t\t<Characteristics/>\r\n");
        push_exchange_plan_based_on_xml(&mut properties, exchange_plan.based_on.as_deref());
        properties.push_str(&format!(
            "\t\t\t<DistributedInfoBase>{}</DistributedInfoBase>\r\n\
\t\t\t<IncludeConfigurationExtensions>{}</IncludeConfigurationExtensions>\r\n\
\t\t\t<CreateOnInput>{}</CreateOnInput>\r\n\
\t\t\t<ChoiceHistoryOnInput>{}</ChoiceHistoryOnInput>\r\n\
\t\t\t<IncludeHelpInContents>{}</IncludeHelpInContents>\r\n",
            xml_bool(exchange_plan.distributed_infobase),
            xml_bool(exchange_plan.include_configuration_extensions),
            exchange_plan.create_on_input,
            exchange_plan.choice_history_on_input,
            xml_bool(exchange_plan.include_help_in_contents),
        ));
        push_exchange_plan_field_collection_xml(
            &mut properties,
            "DataLockFields",
            &exchange_plan.data_lock_fields,
        );
        properties.push_str(&format!(
            "\t\t\t<DataLockControlMode>{}</DataLockControlMode>\r\n\
\t\t\t<FullTextSearch>{}</FullTextSearch>\r\n",
            exchange_plan.data_lock_control_mode, exchange_plan.full_text_search,
        ));
        push_localized_property(
            &mut properties,
            "\t\t\t",
            "ObjectPresentation",
            &exchange_plan.object_presentation,
        );
        push_localized_property(
            &mut properties,
            "\t\t\t",
            "ExtendedObjectPresentation",
            &exchange_plan.extended_object_presentation,
        );
        push_localized_property(
            &mut properties,
            "\t\t\t",
            "ListPresentation",
            &exchange_plan.list_presentation,
        );
        push_localized_property(
            &mut properties,
            "\t\t\t",
            "ExtendedListPresentation",
            &exchange_plan.extended_list_presentation,
        );
        push_localized_property(
            &mut properties,
            "\t\t\t",
            "Explanation",
            &exchange_plan.explanation,
        );
        properties.push_str(&format!(
            "\t\t\t<DataHistory>{}</DataHistory>\r\n\
\t\t\t<UpdateDataHistoryImmediatelyAfterWrite>{}</UpdateDataHistoryImmediatelyAfterWrite>\r\n\
\t\t\t<ExecuteAfterWriteDataHistoryVersionProcessing>{}</ExecuteAfterWriteDataHistoryVersionProcessing>\r\n",
            exchange_plan.data_history,
            xml_bool(exchange_plan.update_data_history_immediately_after_write),
            xml_bool(exchange_plan.execute_after_write_data_history_version_processing),
        ));
        xml.insert_str(index, &properties);
    }
    if !exchange_plan.child_objects.is_empty() {
        let mut child_objects = String::new();
        for child in &exchange_plan.child_objects {
            push_metadata_child_object_xml(&mut child_objects, child);
        }
        insert_metadata_child_objects_xml(&mut xml, "ExchangePlan", &child_objects);
    }
    xml
}

fn push_exchange_plan_form_xml(xml: &mut String, name: &str, value: Option<&str>) {
    if let Some(value) = value {
        xml.push_str(&format!(
            "\t\t\t<{name}>{}</{name}>\r\n",
            escape_xml_element_text(value)
        ));
    } else {
        xml.push_str(&format!("\t\t\t<{name}/>\r\n"));
    }
}

fn push_exchange_plan_based_on_xml(xml: &mut String, value: Option<&str>) {
    if let Some(value) = value {
        xml.push_str("\t\t\t<BasedOn>\r\n");
        xml.push_str(&format!(
            "\t\t\t\t<xr:Item xsi:type=\"xr:MDObjectRef\">{}</xr:Item>\r\n",
            escape_xml_element_text(value)
        ));
        xml.push_str("\t\t\t</BasedOn>\r\n");
    } else {
        xml.push_str("\t\t\t<BasedOn/>\r\n");
    }
}

fn push_exchange_plan_field_collection_xml(xml: &mut String, name: &str, fields: &[String]) {
    if fields.is_empty() {
        xml.push_str(&format!("\t\t\t<{name}/>\r\n"));
        return;
    }
    xml.push_str(&format!("\t\t\t<{name}>\r\n"));
    for field in fields {
        xml.push_str(&format!(
            "\t\t\t\t<xr:Field>{}</xr:Field>\r\n",
            escape_xml_element_text(field)
        ));
    }
    xml.push_str(&format!("\t\t\t</{name}>\r\n"));
}

fn format_exchange_plan_internal_info_xml(exchange_plan: &ExchangePlanProperties) -> String {
    let mut xml = "\t\t<InternalInfo>\r\n".to_string();
    xml.push_str(&format!(
        "\t\t\t<xr:ThisNode>{}</xr:ThisNode>\r\n",
        escape_xml_element_text(&exchange_plan.this_node)
    ));
    for generated_type in &exchange_plan.generated_types {
        xml.push_str(&format!(
            "\t\t\t<xr:GeneratedType name=\"{}\" category=\"{}\">\r\n\
\t\t\t\t<xr:TypeId>{}</xr:TypeId>\r\n\
\t\t\t\t<xr:ValueId>{}</xr:ValueId>\r\n\
\t\t\t</xr:GeneratedType>\r\n",
            escape_xml_text(&generated_type.name),
            escape_xml_text(generated_type.category),
            escape_xml_text(&generated_type.type_id),
            escape_xml_text(&generated_type.value_id)
        ));
    }
    xml.push_str("\t\t</InternalInfo>\r\n");
    xml
}

fn format_recalculation_source_xml(
    header: &MetadataHeader,
    recalculation: &RecalculationProperties,
    source_version: InfobaseConfigSourceVersion,
) -> String {
    let mut xml = format_full_metadata_source_xml("Recalculation", header, source_version);
    let internal_info = format_generated_types_internal_info_xml(&recalculation.generated_types);
    if let Some(index) = xml.find("\t\t<Properties>\r\n") {
        xml.insert_str(index, &internal_info);
    }
    if let Some(index) = xml.find("\t\t</Properties>") {
        xml.insert_str(
            index,
            &format!(
                "\t\t\t<DataLockControlMode>{}</DataLockControlMode>\r\n",
                recalculation.data_lock_control_mode
            ),
        );
    }
    if let Some(index) = xml.find("\t</Recalculation>") {
        if recalculation.dimensions.is_empty() {
            xml.insert_str(index, "\t\t<ChildObjects/>\r\n");
        } else {
            let mut child_objects = "\t\t<ChildObjects>\r\n".to_string();
            for dimension in &recalculation.dimensions {
                push_recalculation_dimension_xml(&mut child_objects, dimension);
            }
            child_objects.push_str("\t\t</ChildObjects>\r\n");
            xml.insert_str(index, &child_objects);
        }
    }
    xml
}

fn push_recalculation_dimension_xml(xml: &mut String, dimension: &RecalculationDimension) {
    xml.push_str(&format!(
        "\t\t\t<Dimension uuid=\"{}\">\r\n\
\t\t\t\t<Properties>\r\n\
\t\t\t\t\t<Name>{}</Name>\r\n",
        escape_xml_text(&dimension.header.uuid),
        escape_xml_element_text(&dimension.header.name),
    ));
    push_header_synonym_xml(xml, "\t\t\t\t\t", &dimension.header.synonyms);
    if dimension.header.comment.is_empty() {
        xml.push_str("\t\t\t\t\t<Comment/>\r\n");
    } else {
        xml.push_str(&format!(
            "\t\t\t\t\t<Comment>{}</Comment>\r\n",
            escape_xml_element_text(&dimension.header.comment)
        ));
    }
    xml.push_str(&format!(
        "\t\t\t\t\t<RegisterDimension>{reference}</RegisterDimension>\r\n\
\t\t\t\t\t<LeadingRegisterData>\r\n\
\t\t\t\t\t\t<xr:Item xsi:type=\"xr:MDObjectRef\">{reference}</xr:Item>\r\n\
\t\t\t\t\t</LeadingRegisterData>\r\n\
\t\t\t\t</Properties>\r\n\
\t\t\t</Dimension>\r\n",
        reference = escape_xml_element_text(&dimension.register_dimension),
    ));
}

fn push_information_register_owner_properties_xml(
    xml: &mut String,
    register: &InformationRegisterOwnerProperties,
) {
    xml.push_str(&format!(
        "\t\t\t<UseStandardCommands>{}</UseStandardCommands>\r\n\
\t\t\t<EditType>{}</EditType>\r\n",
        xml_bool(register.use_standard_commands),
        register.edit_type,
    ));
    push_optional_text_element(
        xml,
        "\t\t\t",
        "DefaultRecordForm",
        register.default_record_form.as_deref(),
    );
    push_optional_text_element(
        xml,
        "\t\t\t",
        "DefaultListForm",
        register.default_list_form.as_deref(),
    );
    push_optional_text_element(
        xml,
        "\t\t\t",
        "AuxiliaryRecordForm",
        register.auxiliary_record_form.as_deref(),
    );
    push_optional_text_element(
        xml,
        "\t\t\t",
        "AuxiliaryListForm",
        register.auxiliary_list_form.as_deref(),
    );
    push_register_standard_attributes_xml(xml, &register.standard_attributes);
    xml.push_str(&format!(
        "\t\t\t<InformationRegisterPeriodicity>{}</InformationRegisterPeriodicity>\r\n\
\t\t\t<WriteMode>{}</WriteMode>\r\n\
\t\t\t<MainFilterOnPeriod>{}</MainFilterOnPeriod>\r\n\
\t\t\t<IncludeHelpInContents>{}</IncludeHelpInContents>\r\n\
\t\t\t<DataLockControlMode>{}</DataLockControlMode>\r\n\
\t\t\t<FullTextSearch>{}</FullTextSearch>\r\n\
\t\t\t<EnableTotalsSliceFirst>{}</EnableTotalsSliceFirst>\r\n\
\t\t\t<EnableTotalsSliceLast>{}</EnableTotalsSliceLast>\r\n",
        register.periodicity,
        register.write_mode,
        xml_bool(register.main_filter_on_period),
        xml_bool(register.include_help_in_contents),
        register.data_lock_control_mode,
        register.full_text_search,
        xml_bool(register.enable_totals_slice_first),
        xml_bool(register.enable_totals_slice_last),
    ));
    push_localized_property(
        xml,
        "\t\t\t",
        "RecordPresentation",
        &register.record_presentation,
    );
    push_localized_property(
        xml,
        "\t\t\t",
        "ExtendedRecordPresentation",
        &register.extended_record_presentation,
    );
    push_localized_property(
        xml,
        "\t\t\t",
        "ListPresentation",
        &register.list_presentation,
    );
    push_localized_property(
        xml,
        "\t\t\t",
        "ExtendedListPresentation",
        &register.extended_list_presentation,
    );
    push_localized_property(xml, "\t\t\t", "Explanation", &register.explanation);
    xml.push_str(&format!(
        "\t\t\t<DataHistory>{}</DataHistory>\r\n\
\t\t\t<UpdateDataHistoryImmediatelyAfterWrite>{}</UpdateDataHistoryImmediatelyAfterWrite>\r\n\
\t\t\t<ExecuteAfterWriteDataHistoryVersionProcessing>{}</ExecuteAfterWriteDataHistoryVersionProcessing>\r\n",
        register.data_history,
        xml_bool(register.update_data_history_immediately_after_write),
        xml_bool(register.execute_after_write_data_history_version_processing),
    ));
}

fn format_register_source_xml(
    kind: &str,
    header: &MetadataHeader,
    register: &RegisterProperties,
    source_version: InfobaseConfigSourceVersion,
) -> String {
    let mut xml = format_full_metadata_source_xml(kind, header, source_version);
    if kind == "InformationRegister" && source_version == InfobaseConfigSourceVersion::V2_21 {
        const STYLE_NAMESPACE_ATTRIBUTE: &str =
            " xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\"";
        const PALETTE_NAMESPACE_ATTRIBUTE: &str =
            " xmlns:pal=\"http://v8.1c.ru/8.1/data/ui/colors/palette\"";
        if let Some(style_index) = xml.find(STYLE_NAMESPACE_ATTRIBUTE) {
            xml.insert_str(style_index, PALETTE_NAMESPACE_ATTRIBUTE);
        }
    }
    let internal_info = format_generated_types_internal_info_xml(&register.generated_types);
    if let Some(index) = xml.find("\t\t<Properties>\r\n") {
        xml.insert_str(index, &internal_info);
    }
    if let Some(index) = xml.find("\t\t</Properties>") {
        let mut properties = String::new();
        if kind == "InformationRegister" {
            if let Some(information_register) = &register.information_register {
                push_information_register_owner_properties_xml(
                    &mut properties,
                    information_register,
                );
            }
            xml.insert_str(index, &properties);
        } else {
            properties.push_str(&format!(
                "\t\t\t<UseStandardCommands>{}</UseStandardCommands>\r\n",
                xml_bool(register.use_standard_commands)
            ));
            if kind == "AccountingRegister" {
                if let Some(include_help_in_contents) = register.include_help_in_contents {
                    properties.push_str(&format!(
                        "\t\t\t<IncludeHelpInContents>{}</IncludeHelpInContents>\r\n",
                        xml_bool(include_help_in_contents)
                    ));
                }
                push_optional_text_element(
                    &mut properties,
                    "\t\t\t",
                    "ChartOfAccounts",
                    register.chart_of_accounts.as_deref(),
                );
                if let Some(correspondence) = register.correspondence {
                    properties.push_str(&format!(
                        "\t\t\t<Correspondence>{}</Correspondence>\r\n",
                        xml_bool(correspondence)
                    ));
                }
                if let Some(period_adjustment_length) = register.period_adjustment_length {
                    properties.push_str(&format!(
                        "\t\t\t<PeriodAdjustmentLength>{period_adjustment_length}</PeriodAdjustmentLength>\r\n"
                    ));
                }
                push_optional_text_element(
                    &mut properties,
                    "\t\t\t",
                    "DefaultListForm",
                    register.default_list_form.as_deref(),
                );
                push_optional_text_element(
                    &mut properties,
                    "\t\t\t",
                    "AuxiliaryListForm",
                    register.auxiliary_list_form.as_deref(),
                );
                push_register_standard_attributes_xml(
                    &mut properties,
                    &register.standard_attributes,
                );
                push_optional_simple_property_xml(
                    &mut properties,
                    "DataLockControlMode",
                    register.data_lock_control_mode,
                );
                if let Some(enable_totals_splitting) = register.enable_totals_splitting {
                    properties.push_str(&format!(
                        "\t\t\t<EnableTotalsSplitting>{}</EnableTotalsSplitting>\r\n",
                        xml_bool(enable_totals_splitting)
                    ));
                }
                push_optional_simple_property_xml(
                    &mut properties,
                    "FullTextSearch",
                    register.full_text_search,
                );
                push_localized_property(
                    &mut properties,
                    "\t\t\t",
                    "ListPresentation",
                    &register.list_presentation,
                );
                push_localized_property(
                    &mut properties,
                    "\t\t\t",
                    "ExtendedListPresentation",
                    &register.extended_list_presentation,
                );
                push_localized_property(
                    &mut properties,
                    "\t\t\t",
                    "Explanation",
                    &register.explanation,
                );
                xml.insert_str(index, &properties);
                return if register.child_objects.is_empty() {
                    xml
                } else {
                    let mut child_objects = String::new();
                    for child in &register.child_objects {
                        push_metadata_child_object_xml(&mut child_objects, child);
                    }
                    insert_metadata_child_objects_xml(&mut xml, kind, &child_objects);
                    xml
                };
            }
            if kind == "AccumulationRegister" {
                push_optional_text_element(
                    &mut properties,
                    "\t\t\t",
                    "DefaultListForm",
                    register.default_list_form.as_deref(),
                );
                push_optional_text_element(
                    &mut properties,
                    "\t\t\t",
                    "AuxiliaryListForm",
                    register.auxiliary_list_form.as_deref(),
                );
            }
            if let Some(register_type) = register.register_type {
                properties.push_str(&format!(
                    "\t\t\t<RegisterType>{register_type}</RegisterType>\r\n"
                ));
            }
            if let Some(include_help_in_contents) = register.include_help_in_contents {
                properties.push_str(&format!(
                    "\t\t\t<IncludeHelpInContents>{}</IncludeHelpInContents>\r\n",
                    xml_bool(include_help_in_contents)
                ));
            }
            push_register_standard_attributes_xml(&mut properties, &register.standard_attributes);
            push_optional_simple_property_xml(
                &mut properties,
                "DataLockControlMode",
                register.data_lock_control_mode,
            );
            push_optional_simple_property_xml(
                &mut properties,
                "FullTextSearch",
                register.full_text_search,
            );
            xml.insert_str(index, &properties);
        }
    }
    if register.child_objects.is_empty() {
        return xml;
    }
    let mut child_objects = String::new();
    for child in &register.child_objects {
        push_metadata_child_object_xml(&mut child_objects, child);
    }
    insert_metadata_child_objects_xml(&mut xml, kind, &child_objects);
    xml
}

fn format_catalog_source_xml(header: &MetadataHeader, catalog: &CatalogProperties) -> String {
    let mut xml = format!(
        "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:app=\"http://v8.1c.ru/8.2/managed-application/core\" xmlns:cfg=\"http://v8.1c.ru/8.1/data/enterprise/current-config\" xmlns:cmi=\"http://v8.1c.ru/8.2/managed-application/cmi\" xmlns:ent=\"http://v8.1c.ru/8.1/data/enterprise\" xmlns:lf=\"http://v8.1c.ru/8.2/managed-application/logform\" xmlns:pal=\"http://v8.1c.ru/8.1/data/ui/colors/palette\" xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\" xmlns:sys=\"http://v8.1c.ru/8.1/data/ui/fonts/system\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" xmlns:web=\"http://v8.1c.ru/8.1/data/ui/colors/web\" xmlns:win=\"http://v8.1c.ru/8.1/data/ui/colors/windows\" xmlns:xen=\"http://v8.1c.ru/8.3/xcf/enums\" xmlns:xpr=\"http://v8.1c.ru/8.3/xcf/predef\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"2.21\">\r\n\
\t<Catalog uuid=\"{uuid}\">\r\n",
        uuid = escape_xml_text(&header.uuid),
    );

    xml.push_str(&format_generated_types_internal_info_xml(
        &catalog.generated_types,
    ));

    xml.push_str("\t\t<Properties>\r\n");
    xml.push_str(&format!(
        "\t\t\t<Name>{}</Name>\r\n",
        escape_xml_text(&header.name)
    ));
    if header.synonyms.is_empty() {
        xml.push_str("\t\t\t<Synonym/>\r\n");
    } else {
        xml.push_str("\t\t\t<Synonym>\r\n");
        for (lang, content) in &header.synonyms {
            xml.push_str("\t\t\t\t<v8:item>\r\n");
            xml.push_str(&format!(
                "\t\t\t\t\t<v8:lang>{}</v8:lang>\r\n",
                escape_xml_element_text(lang)
            ));
            xml.push_str(&format!(
                "\t\t\t\t\t<v8:content>{}</v8:content>\r\n",
                escape_xml_element_text(content)
            ));
            xml.push_str("\t\t\t\t</v8:item>\r\n");
        }
        xml.push_str("\t\t\t</Synonym>\r\n");
    }
    if header.comment.is_empty() {
        xml.push_str("\t\t\t<Comment/>\r\n");
    } else {
        xml.push_str(&format!(
            "\t\t\t<Comment>{}</Comment>\r\n",
            escape_xml_text(&header.comment)
        ));
    }
    xml.push_str(&format!(
        "\t\t\t<Hierarchical>{}</Hierarchical>\r\n\
\t\t\t<HierarchyType>HierarchyFoldersAndItems</HierarchyType>\r\n\
\t\t\t<LimitLevelCount>false</LimitLevelCount>\r\n\
\t\t\t<LevelCount>{}</LevelCount>\r\n\
\t\t\t<FoldersOnTop>{}</FoldersOnTop>\r\n\
\t\t\t<UseStandardCommands>{}</UseStandardCommands>\r\n",
        xml_bool(catalog.hierarchical),
        catalog.level_count,
        xml_bool(catalog.folders_on_top),
        xml_bool(catalog.use_standard_commands),
    ));
    if let Some(owners) = &catalog.owners {
        if owners.is_empty() {
            xml.push_str("\t\t\t<Owners/>\r\n");
        } else {
            xml.push_str("\t\t\t<Owners>\r\n");
            for owner in owners {
                xml.push_str(&format!(
                    "\t\t\t\t<xr:Item xsi:type=\"xr:MDObjectRef\">{}</xr:Item>\r\n",
                    escape_xml_element_text(owner)
                ));
            }
            xml.push_str("\t\t\t</Owners>\r\n");
        }
    }
    if let Some(value) = catalog.subordination_use {
        xml.push_str(&format!(
            "\t\t\t<SubordinationUse>{value}</SubordinationUse>\r\n"
        ));
    }
    xml.push_str(&format!(
        "\t\t\t<CodeLength>{}</CodeLength>\r\n\
\t\t\t<DescriptionLength>{}</DescriptionLength>\r\n",
        catalog.code_length, catalog.description_length
    ));
    if let Some(value) = catalog.code_type {
        xml.push_str(&format!("\t\t\t<CodeType>{value}</CodeType>\r\n"));
    }
    if let Some(value) = catalog.code_allowed_length {
        xml.push_str(&format!(
            "\t\t\t<CodeAllowedLength>{value}</CodeAllowedLength>\r\n"
        ));
    }
    if let Some(value) = catalog.code_series {
        xml.push_str(&format!("\t\t\t<CodeSeries>{value}</CodeSeries>\r\n"));
    }
    xml.push_str(&format!(
        "\t\t\t<CheckUnique>{}</CheckUnique>\r\n\
\t\t\t<Autonumbering>{}</Autonumbering>\r\n",
        xml_bool(catalog.check_unique),
        xml_bool(catalog.autonumbering),
    ));
    if let Some(value) = catalog.default_presentation {
        xml.push_str(&format!(
            "\t\t\t<DefaultPresentation>{value}</DefaultPresentation>\r\n"
        ));
    }
    push_catalog_standard_attributes_xml(&mut xml, catalog);
    xml.push_str(
        "\t\t\t<Characteristics/>\r\n\
\t\t\t<PredefinedDataUpdate>Auto</PredefinedDataUpdate>\r\n\
\t\t\t<EditType>InDialog</EditType>\r\n\
",
    );
    xml.push_str(&format!(
        "\t\t\t<QuickChoice>{}</QuickChoice>\r\n\
\t\t\t<ChoiceMode>{}</ChoiceMode>\r\n",
        xml_bool(catalog.quick_choice),
        catalog.choice_mode
    ));
    push_catalog_input_by_string_xml(&mut xml, &catalog.input_by_string_fields);
    xml.push_str(
        "\t\t\t<SearchStringModeOnInputByString>Begin</SearchStringModeOnInputByString>\r\n\
\t\t\t<FullTextSearchOnInputByString>DontUse</FullTextSearchOnInputByString>\r\n\
\t\t\t<ChoiceDataGetModeOnInputByString>Directly</ChoiceDataGetModeOnInputByString>\r\n",
    );
    push_optional_text_element(
        &mut xml,
        "\t\t\t",
        "DefaultObjectForm",
        catalog.default_object_form.as_deref(),
    );
    push_optional_text_element(
        &mut xml,
        "\t\t\t",
        "DefaultFolderForm",
        catalog.default_folder_form.as_deref(),
    );
    push_optional_text_element(
        &mut xml,
        "\t\t\t",
        "DefaultListForm",
        catalog.default_list_form.as_deref(),
    );
    push_optional_text_element(
        &mut xml,
        "\t\t\t",
        "DefaultChoiceForm",
        catalog.default_choice_form.as_deref(),
    );
    push_optional_text_element(
        &mut xml,
        "\t\t\t",
        "DefaultFolderChoiceForm",
        catalog.default_folder_choice_form.as_deref(),
    );
    push_optional_text_element(
        &mut xml,
        "\t\t\t",
        "AuxiliaryObjectForm",
        catalog.auxiliary_object_form.as_deref(),
    );
    push_optional_text_element(
        &mut xml,
        "\t\t\t",
        "AuxiliaryFolderForm",
        catalog.auxiliary_folder_form.as_deref(),
    );
    push_optional_text_element(
        &mut xml,
        "\t\t\t",
        "AuxiliaryListForm",
        catalog.auxiliary_list_form.as_deref(),
    );
    push_optional_text_element(
        &mut xml,
        "\t\t\t",
        "AuxiliaryChoiceForm",
        catalog.auxiliary_choice_form.as_deref(),
    );
    push_optional_text_element(
        &mut xml,
        "\t\t\t",
        "AuxiliaryFolderChoiceForm",
        catalog.auxiliary_folder_choice_form.as_deref(),
    );
    xml.push_str(&format!(
        "\t\t\t<IncludeHelpInContents>{}</IncludeHelpInContents>\r\n\
\t\t\t<BasedOn/>\r\n\
\t\t\t<DataLockFields/>\r\n\
\t\t\t<DataLockControlMode>Managed</DataLockControlMode>\r\n\
\t\t\t<FullTextSearch>Use</FullTextSearch>\r\n",
        xml_bool(catalog.include_help_in_contents),
    ));
    push_localized_property(
        &mut xml,
        "\t\t\t",
        "ObjectPresentation",
        &catalog.object_presentation,
    );
    push_localized_property(
        &mut xml,
        "\t\t\t",
        "ExtendedObjectPresentation",
        &catalog.extended_object_presentation,
    );
    push_localized_property(
        &mut xml,
        "\t\t\t",
        "ListPresentation",
        &catalog.list_presentation,
    );
    push_localized_property(
        &mut xml,
        "\t\t\t",
        "ExtendedListPresentation",
        &catalog.extended_list_presentation,
    );
    push_localized_property(&mut xml, "\t\t\t", "Explanation", &catalog.explanation);
    xml.push_str(&format!(
        "\t\t\t<CreateOnInput>{}</CreateOnInput>\r\n\
\t\t\t<ChoiceHistoryOnInput>{}</ChoiceHistoryOnInput>\r\n\
\t\t\t<DataHistory>{}</DataHistory>\r\n\
\t\t\t<UpdateDataHistoryImmediatelyAfterWrite>{}</UpdateDataHistoryImmediatelyAfterWrite>\r\n\
\t\t\t<ExecuteAfterWriteDataHistoryVersionProcessing>{}</ExecuteAfterWriteDataHistoryVersionProcessing>\r\n",
        catalog.create_on_input,
        catalog.choice_history_on_input,
        catalog.data_history,
        xml_bool(catalog.update_data_history_immediately_after_write),
        xml_bool(catalog.execute_after_write_data_history_version_processing)
    ));
    xml.push_str("\t\t</Properties>\r\n");
    if !catalog.child_metadata_objects.is_empty()
        || !catalog.child_forms.is_empty()
        || !catalog.child_templates.is_empty()
    {
        xml.push_str("\t\t<ChildObjects>\r\n");
        for child in &catalog.child_metadata_objects {
            push_metadata_child_object_xml(&mut xml, child);
        }
        for form in &catalog.child_forms {
            xml.push_str(&format!(
                "\t\t\t<Form>{}</Form>\r\n",
                escape_xml_element_text(form)
            ));
        }
        for template in &catalog.child_templates {
            xml.push_str(&format!(
                "\t\t\t<Template>{}</Template>\r\n",
                escape_xml_element_text(template)
            ));
        }
        xml.push_str("\t\t</ChildObjects>\r\n");
    }
    xml.push_str("\t</Catalog>\r\n</MetaDataObject>");
    xml
}

fn format_report_source_xml(
    header: &MetadataHeader,
    report: &ReportProperties,
    source_version: InfobaseConfigSourceVersion,
) -> String {
    let palette_namespace = if source_version == InfobaseConfigSourceVersion::V2_21 {
        " xmlns:pal=\"http://v8.1c.ru/8.1/data/ui/colors/palette\""
    } else {
        ""
    };
    let mut xml = format!(
        "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:app=\"http://v8.1c.ru/8.2/managed-application/core\" xmlns:cfg=\"http://v8.1c.ru/8.1/data/enterprise/current-config\" xmlns:cmi=\"http://v8.1c.ru/8.2/managed-application/cmi\" xmlns:ent=\"http://v8.1c.ru/8.1/data/enterprise\" xmlns:lf=\"http://v8.1c.ru/8.2/managed-application/logform\"{} xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\" xmlns:sys=\"http://v8.1c.ru/8.1/data/ui/fonts/system\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" xmlns:web=\"http://v8.1c.ru/8.1/data/ui/colors/web\" xmlns:win=\"http://v8.1c.ru/8.1/data/ui/colors/windows\" xmlns:xen=\"http://v8.1c.ru/8.3/xcf/enums\" xmlns:xpr=\"http://v8.1c.ru/8.3/xcf/predef\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"{}\">\r\n\
\t<Report uuid=\"{}\">\r\n",
        palette_namespace,
        source_version.as_str(),
        escape_xml_text(&header.uuid),
    );

    xml.push_str(&format_generated_types_internal_info_xml(
        &report.generated_types,
    ));

    xml.push_str("\t\t<Properties>\r\n");
    xml.push_str(&format!(
        "\t\t\t<Name>{}</Name>\r\n",
        escape_xml_element_text(&header.name)
    ));
    push_header_synonym_xml(&mut xml, "\t\t\t", &header.synonyms);
    if header.comment.is_empty() {
        xml.push_str("\t\t\t<Comment/>\r\n");
    } else {
        xml.push_str(&format!(
            "\t\t\t<Comment>{}</Comment>\r\n",
            escape_xml_element_text(&header.comment)
        ));
    }
    xml.push_str(&format!(
        "\t\t\t<UseStandardCommands>{}</UseStandardCommands>\r\n",
        xml_bool(report.use_standard_commands)
    ));
    push_optional_text_element(
        &mut xml,
        "\t\t\t",
        "DefaultForm",
        report.default_form.as_deref(),
    );
    xml.push_str("\t\t\t<AuxiliaryForm/>\r\n");
    push_optional_text_element(
        &mut xml,
        "\t\t\t",
        "MainDataCompositionSchema",
        report.main_data_composition_schema.as_deref(),
    );
    push_optional_text_element(
        &mut xml,
        "\t\t\t",
        "DefaultSettingsForm",
        report.default_settings_form.as_deref(),
    );
    xml.push_str("\t\t\t<AuxiliarySettingsForm/>\r\n");
    push_optional_text_element(
        &mut xml,
        "\t\t\t",
        "DefaultVariantForm",
        report.default_variant_form.as_deref(),
    );
    push_optional_text_element(
        &mut xml,
        "\t\t\t",
        "VariantsStorage",
        report.variants_storage.as_deref(),
    );
    push_optional_text_element(
        &mut xml,
        "\t\t\t",
        "SettingsStorage",
        report.settings_storage.as_deref(),
    );
    xml.push_str(&format!(
        "\t\t\t<IncludeHelpInContents>{}</IncludeHelpInContents>\r\n",
        xml_bool(report.include_help_in_contents)
    ));
    push_localized_property(
        &mut xml,
        "\t\t\t",
        "ExtendedPresentation",
        &report.extended_presentation,
    );
    push_localized_property(&mut xml, "\t\t\t", "Explanation", &report.explanation);
    xml.push_str("\t\t</Properties>\r\n");

    if !report.child_metadata_objects.is_empty()
        || !report.child_forms.is_empty()
        || !report.child_templates.is_empty()
        || !report.child_commands.is_empty()
    {
        xml.push_str("\t\t<ChildObjects>\r\n");
        for child in &report.child_metadata_objects {
            push_metadata_child_object_xml(&mut xml, child);
        }
        for form in &report.child_forms {
            xml.push_str(&format!(
                "\t\t\t<Form>{}</Form>\r\n",
                escape_xml_element_text(form)
            ));
        }
        for template in &report.child_templates {
            xml.push_str(&format!(
                "\t\t\t<Template>{}</Template>\r\n",
                escape_xml_element_text(template)
            ));
        }
        for command in &report.child_commands {
            push_metadata_child_command_xml(&mut xml, command);
        }
        xml.push_str("\t\t</ChildObjects>\r\n");
    }

    xml.push_str("\t</Report>\r\n</MetaDataObject>");
    xml
}

fn format_document_source_xml(
    header: &MetadataHeader,
    document: &DocumentProperties,
    source_version: InfobaseConfigSourceVersion,
) -> String {
    let mut xml = format_full_metadata_source_xml("Document", header, source_version);
    let internal_info = format_generated_types_internal_info_xml(&document.generated_types);
    if let Some(index) = xml.find("\t\t<Properties>\r\n") {
        xml.insert_str(index, &internal_info);
    }
    if let Some(index) = xml.find("\t\t</Properties>") {
        let mut properties = format!(
            "\t\t\t<UseStandardCommands>{}</UseStandardCommands>\r\n",
            xml_bool(document.use_standard_commands)
        );
        if let Some(numbering) = &document.numbering {
            push_document_numbering_xml(&mut properties, numbering);
        }
        xml.insert_str(index, &properties);
    }
    if let Some(standard_attributes) = &document.standard_attributes
        && let Some(index) = xml.find("\t\t</Properties>")
    {
        let mut properties = String::new();
        push_document_standard_attributes_xml(&mut properties, standard_attributes);
        xml.insert_str(index, &properties);
    }
    if let Some(index) = xml.find("\t\t</Properties>") {
        let mut properties = String::new();
        push_optional_text_element(
            &mut properties,
            "\t\t\t",
            "DefaultObjectForm",
            document.default_object_form.as_deref(),
        );
        push_optional_text_element(
            &mut properties,
            "\t\t\t",
            "DefaultListForm",
            document.default_list_form.as_deref(),
        );
        push_optional_text_element(
            &mut properties,
            "\t\t\t",
            "DefaultChoiceForm",
            document.default_choice_form.as_deref(),
        );
        push_optional_text_element(
            &mut properties,
            "\t\t\t",
            "AuxiliaryObjectForm",
            document.auxiliary_object_form.as_deref(),
        );
        push_optional_text_element(
            &mut properties,
            "\t\t\t",
            "AuxiliaryListForm",
            document.auxiliary_list_form.as_deref(),
        );
        push_optional_text_element(
            &mut properties,
            "\t\t\t",
            "AuxiliaryChoiceForm",
            document.auxiliary_choice_form.as_deref(),
        );
        properties.push_str(&format!(
            "\t\t\t<IncludeHelpInContents>{}</IncludeHelpInContents>\r\n",
            xml_bool(document.include_help_in_contents)
        ));
        xml.insert_str(index, &properties);
    }
    if !document.child_metadata_objects.is_empty()
        || !document.child_forms.is_empty()
        || !document.child_templates.is_empty()
    {
        let mut child_objects = "\t\t<ChildObjects>\r\n".to_string();
        for child in document
            .child_metadata_objects
            .iter()
            .filter(|child| child.tag == "Attribute")
        {
            push_metadata_child_object_xml(&mut child_objects, child);
        }
        for form in &document.child_forms {
            child_objects.push_str(&format!(
                "\t\t\t<Form>{}</Form>\r\n",
                escape_xml_element_text(form)
            ));
        }
        for child in document
            .child_metadata_objects
            .iter()
            .filter(|child| child.tag == "TabularSection")
        {
            push_metadata_child_object_xml(&mut child_objects, child);
        }
        for template in &document.child_templates {
            child_objects.push_str(&format!(
                "\t\t\t<Template>{}</Template>\r\n",
                escape_xml_element_text(template)
            ));
        }
        child_objects.push_str("\t\t</ChildObjects>\r\n");
        if let Some(index) = xml.find("\t</Document>") {
            xml.insert_str(index, &child_objects);
        }
    }
    xml
}

fn format_business_process_source_xml(
    header: &MetadataHeader,
    business_process: &BusinessProcessProperties,
    source_version: InfobaseConfigSourceVersion,
) -> String {
    let mut xml = format_full_metadata_source_xml("BusinessProcess", header, source_version);
    let internal_info = format_generated_types_internal_info_xml(&business_process.generated_types);
    if let Some(index) = xml.find("\t\t<Properties>\r\n") {
        xml.insert_str(index, &internal_info);
    }
    if let Some(index) = xml.find("\t\t</Properties>") {
        let mut properties = format!(
            "\t\t\t<UseStandardCommands>{}</UseStandardCommands>\r\n",
            xml_bool(business_process.use_standard_commands)
        );
        push_optional_text_element(
            &mut properties,
            "\t\t\t",
            "DefaultListForm",
            business_process.default_list_form.as_deref(),
        );
        xml.insert_str(index, &properties);
    }
    xml
}

fn format_task_source_xml(
    header: &MetadataHeader,
    task: &TaskProperties,
    source_version: InfobaseConfigSourceVersion,
) -> String {
    let mut xml = format_full_metadata_source_xml("Task", header, source_version);
    let internal_info = format_generated_types_internal_info_xml(&task.generated_types);
    if let Some(index) = xml.find("\t\t<Properties>\r\n") {
        xml.insert_str(index, &internal_info);
    }
    if let Some(index) = xml.find("\t\t</Properties>") {
        let mut properties = format!(
            "\t\t\t<UseStandardCommands>{}</UseStandardCommands>\r\n",
            xml_bool(task.use_standard_commands)
        );
        push_optional_text_element(
            &mut properties,
            "\t\t\t",
            "DefaultListForm",
            task.default_list_form.as_deref(),
        );
        xml.insert_str(index, &properties);
    }
    xml
}

fn format_settings_storage_source_xml(
    header: &MetadataHeader,
    settings_storage: &SettingsStorageProperties,
    source_version: InfobaseConfigSourceVersion,
) -> String {
    let mut xml = format_full_metadata_source_xml("SettingsStorage", header, source_version);
    let internal_info = format_generated_types_internal_info_xml(&settings_storage.generated_types);
    if let Some(index) = xml.find("\t\t<Properties>\r\n") {
        xml.insert_str(index, &internal_info);
    }
    xml
}

fn format_data_processor_source_xml(
    header: &MetadataHeader,
    data_processor: &DataProcessorProperties,
    source_version: InfobaseConfigSourceVersion,
) -> String {
    let mut xml = format_full_metadata_source_xml("DataProcessor", header, source_version);
    let internal_info = format_generated_types_internal_info_xml(&data_processor.generated_types);
    if let Some(index) = xml.find("\t\t<Properties>\r\n") {
        xml.insert_str(index, &internal_info);
    }
    if let Some(index) = xml.find("\t\t</Properties>") {
        let mut properties = String::new();
        properties.push_str(&format!(
            "\t\t\t<UseStandardCommands>{}</UseStandardCommands>\r\n",
            xml_bool(data_processor.use_standard_commands)
        ));
        push_optional_text_element(
            &mut properties,
            "\t\t\t",
            "DefaultForm",
            data_processor.default_form.as_deref(),
        );
        push_optional_text_element(
            &mut properties,
            "\t\t\t",
            "AuxiliaryForm",
            data_processor.auxiliary_form.as_deref(),
        );
        properties.push_str(&format!(
            "\t\t\t<IncludeHelpInContents>{}</IncludeHelpInContents>\r\n",
            xml_bool(data_processor.include_help_in_contents)
        ));
        push_localized_property(
            &mut properties,
            "\t\t\t",
            "ExtendedPresentation",
            &data_processor.extended_presentation,
        );
        push_localized_property(
            &mut properties,
            "\t\t\t",
            "Explanation",
            &data_processor.explanation,
        );
        xml.insert_str(index, &properties);
    }
    if data_processor.child_metadata_objects.is_empty()
        && data_processor.child_forms.is_empty()
        && data_processor.child_templates.is_empty()
        && data_processor.child_commands.is_empty()
    {
        if let Some(index) = xml.find("\t</DataProcessor>") {
            xml.insert_str(index, "\t\t<ChildObjects/>\r\n");
        }
    } else {
        let mut child_objects = "\t\t<ChildObjects>\r\n".to_string();
        for child in &data_processor.child_metadata_objects {
            push_metadata_child_object_xml(&mut child_objects, child);
        }
        for form in &data_processor.child_forms {
            child_objects.push_str(&format!(
                "\t\t\t<Form>{}</Form>\r\n",
                escape_xml_element_text(form)
            ));
        }
        for template in &data_processor.child_templates {
            child_objects.push_str(&format!(
                "\t\t\t<Template>{}</Template>\r\n",
                escape_xml_element_text(template)
            ));
        }
        for command in &data_processor.child_commands {
            push_metadata_child_command_xml(&mut child_objects, command);
        }
        child_objects.push_str("\t\t</ChildObjects>\r\n");
        if let Some(index) = xml.find("\t</DataProcessor>") {
            xml.insert_str(index, &child_objects);
        }
    }
    xml
}

fn format_enum_source_xml(
    header: &MetadataHeader,
    enumeration: &EnumProperties,
    source_version: InfobaseConfigSourceVersion,
) -> String {
    let palette_namespace = if source_version == InfobaseConfigSourceVersion::V2_21 {
        " xmlns:pal=\"http://v8.1c.ru/8.1/data/ui/colors/palette\""
    } else {
        ""
    };
    let mut xml = format!(
        "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:app=\"http://v8.1c.ru/8.2/managed-application/core\" xmlns:cfg=\"http://v8.1c.ru/8.1/data/enterprise/current-config\" xmlns:cmi=\"http://v8.1c.ru/8.2/managed-application/cmi\" xmlns:ent=\"http://v8.1c.ru/8.1/data/enterprise\" xmlns:lf=\"http://v8.1c.ru/8.2/managed-application/logform\"{} xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\" xmlns:sys=\"http://v8.1c.ru/8.1/data/ui/fonts/system\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" xmlns:web=\"http://v8.1c.ru/8.1/data/ui/colors/web\" xmlns:win=\"http://v8.1c.ru/8.1/data/ui/colors/windows\" xmlns:xen=\"http://v8.1c.ru/8.3/xcf/enums\" xmlns:xpr=\"http://v8.1c.ru/8.3/xcf/predef\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"{}\">\r\n\
\t<Enum uuid=\"{}\">\r\n",
        palette_namespace,
        source_version.as_str(),
        escape_xml_text(&header.uuid),
    );

    xml.push_str(&format_generated_types_internal_info_xml(
        &enumeration.generated_types,
    ));

    xml.push_str("\t\t<Properties>\r\n");
    xml.push_str(&format!(
        "\t\t\t<Name>{}</Name>\r\n",
        escape_xml_element_text(&header.name)
    ));
    push_header_synonym_xml(&mut xml, "\t\t\t", &header.synonyms);
    if header.comment.is_empty() {
        xml.push_str("\t\t\t<Comment/>\r\n");
    } else {
        xml.push_str(&format!(
            "\t\t\t<Comment>{}</Comment>\r\n",
            escape_xml_element_text(&header.comment)
        ));
    }
    xml.push_str(&format!(
        "\t\t\t<UseStandardCommands>{}</UseStandardCommands>\r\n",
        xml_bool(enumeration.use_standard_commands)
    ));
    if enumeration.has_standard_attributes {
        push_enum_standard_attributes_xml(&mut xml);
    }
    xml.push_str(&format!(
        "\t\t\t<Characteristics/>\r\n\
\t\t\t<QuickChoice>{}</QuickChoice>\r\n\
\t\t\t<ChoiceMode>{}</ChoiceMode>\r\n",
        xml_bool(enumeration.quick_choice),
        enumeration.choice_mode
    ));
    push_optional_text_element(
        &mut xml,
        "\t\t\t",
        "DefaultListForm",
        enumeration.default_list_form.as_deref(),
    );
    push_optional_text_element(
        &mut xml,
        "\t\t\t",
        "DefaultChoiceForm",
        enumeration.default_choice_form.as_deref(),
    );
    push_optional_text_element(
        &mut xml,
        "\t\t\t",
        "AuxiliaryListForm",
        enumeration.auxiliary_list_form.as_deref(),
    );
    push_optional_text_element(
        &mut xml,
        "\t\t\t",
        "AuxiliaryChoiceForm",
        enumeration.auxiliary_choice_form.as_deref(),
    );
    push_localized_property(
        &mut xml,
        "\t\t\t",
        "ListPresentation",
        &enumeration.list_presentation,
    );
    push_localized_property(
        &mut xml,
        "\t\t\t",
        "ExtendedListPresentation",
        &enumeration.extended_list_presentation,
    );
    push_localized_property(&mut xml, "\t\t\t", "Explanation", &enumeration.explanation);
    xml.push_str(&format!(
        "\t\t\t<ChoiceHistoryOnInput>{}</ChoiceHistoryOnInput>\r\n",
        enumeration.choice_history_on_input
    ));
    xml.push_str("\t\t</Properties>\r\n");

    if enumeration.values.is_empty()
        && enumeration.child_forms.is_empty()
        && enumeration.child_templates.is_empty()
    {
        xml.push_str("\t\t<ChildObjects/>\r\n");
    } else {
        xml.push_str("\t\t<ChildObjects>\r\n");
        for value in &enumeration.values {
            xml.push_str(&format!(
                "\t\t\t<EnumValue uuid=\"{}\">\r\n\
\t\t\t\t<Properties>\r\n\
\t\t\t\t\t<Name>{}</Name>\r\n",
                escape_xml_text(&value.uuid),
                escape_xml_element_text(&value.name)
            ));
            push_header_synonym_xml(&mut xml, "\t\t\t\t\t", &value.synonyms);
            if value.comment.is_empty() {
                xml.push_str("\t\t\t\t\t<Comment/>\r\n");
            } else {
                xml.push_str(&format!(
                    "\t\t\t\t\t<Comment>{}</Comment>\r\n",
                    escape_xml_element_text(&value.comment)
                ));
            }
            if source_version == InfobaseConfigSourceVersion::V2_21 {
                xml.push_str("\t\t\t\t\t<Color>auto</Color>\r\n");
            }
            xml.push_str("\t\t\t\t</Properties>\r\n\t\t\t</EnumValue>\r\n");
        }
        for form in &enumeration.child_forms {
            xml.push_str(&format!(
                "\t\t\t<Form>{}</Form>\r\n",
                escape_xml_element_text(form)
            ));
        }
        for template in &enumeration.child_templates {
            xml.push_str(&format!(
                "\t\t\t<Template>{}</Template>\r\n",
                escape_xml_element_text(template)
            ));
        }
        xml.push_str("\t\t</ChildObjects>\r\n");
    }

    xml.push_str("\t</Enum>\r\n</MetaDataObject>");
    xml
}

fn format_generated_types_internal_info_xml(generated_types: &[GeneratedTypeEntry]) -> String {
    format_generated_types_internal_info_xml_with_indent(generated_types, "\t\t")
}

fn format_generated_types_internal_info_xml_with_indent(
    generated_types: &[GeneratedTypeEntry],
    indent: &str,
) -> String {
    if generated_types.is_empty() {
        return String::new();
    }
    let nested = format!("{indent}\t");
    let mut xml = format!("{indent}<InternalInfo>\r\n");
    for generated_type in generated_types {
        xml.push_str(&format!(
            "{nested}<xr:GeneratedType name=\"{}\" category=\"{}\">\r\n\
{nested}\t<xr:TypeId>{}</xr:TypeId>\r\n\
{nested}\t<xr:ValueId>{}</xr:ValueId>\r\n\
{nested}</xr:GeneratedType>\r\n",
            escape_xml_text(&generated_type.name),
            escape_xml_text(generated_type.category),
            escape_xml_text(&generated_type.type_id),
            escape_xml_text(&generated_type.value_id)
        ));
    }
    xml.push_str(&format!("{indent}</InternalInfo>\r\n"));
    xml
}

fn push_header_synonym_xml(xml: &mut String, indent: &str, synonyms: &[(String, String)]) {
    if synonyms.is_empty() {
        xml.push_str(&format!("{indent}<Synonym/>\r\n"));
        return;
    }
    xml.push_str(&format!("{indent}<Synonym>\r\n"));
    for (lang, content) in synonyms {
        xml.push_str(&format!("{indent}\t<v8:item>\r\n"));
        xml.push_str(&format!(
            "{indent}\t\t<v8:lang>{}</v8:lang>\r\n",
            escape_xml_element_text(lang)
        ));
        xml.push_str(&format!(
            "{indent}\t\t<v8:content>{}</v8:content>\r\n",
            escape_xml_element_text(content)
        ));
        xml.push_str(&format!("{indent}\t</v8:item>\r\n"));
    }
    xml.push_str(&format!("{indent}</Synonym>\r\n"));
}

fn push_enum_standard_attributes_xml(xml: &mut String) {
    xml.push_str("\t\t\t<StandardAttributes>\r\n");
    for name in ["Order", "Ref"] {
        xml.push_str(&format!(
            "\t\t\t\t<xr:StandardAttribute name=\"{}\">\r\n\
\t\t\t\t\t<xr:LinkByType/>\r\n\
\t\t\t\t\t<xr:FillChecking>DontCheck</xr:FillChecking>\r\n\
\t\t\t\t\t<xr:MultiLine>false</xr:MultiLine>\r\n\
\t\t\t\t\t<xr:FillFromFillingValue>false</xr:FillFromFillingValue>\r\n\
\t\t\t\t\t<xr:CreateOnInput>Auto</xr:CreateOnInput>\r\n\
\t\t\t\t\t<xr:TypeReductionMode>TransformValues</xr:TypeReductionMode>\r\n\
\t\t\t\t\t<xr:MaxValue xsi:nil=\"true\"/>\r\n\
\t\t\t\t\t<xr:ToolTip/>\r\n\
\t\t\t\t\t<xr:ExtendedEdit>false</xr:ExtendedEdit>\r\n\
\t\t\t\t\t<xr:Format/>\r\n\
\t\t\t\t\t<xr:ChoiceForm/>\r\n\
\t\t\t\t\t<xr:QuickChoice>Auto</xr:QuickChoice>\r\n\
\t\t\t\t\t<xr:ChoiceHistoryOnInput>Auto</xr:ChoiceHistoryOnInput>\r\n\
\t\t\t\t\t<xr:EditFormat/>\r\n\
\t\t\t\t\t<xr:PasswordMode>false</xr:PasswordMode>\r\n\
\t\t\t\t\t<xr:DataHistory>Use</xr:DataHistory>\r\n\
\t\t\t\t\t<xr:MarkNegatives>false</xr:MarkNegatives>\r\n\
\t\t\t\t\t<xr:MinValue xsi:nil=\"true\"/>\r\n\
\t\t\t\t\t<xr:Synonym/>\r\n\
\t\t\t\t\t<xr:Comment/>\r\n\
\t\t\t\t\t<xr:FullTextSearch>Use</xr:FullTextSearch>\r\n\
\t\t\t\t\t<xr:ChoiceParameterLinks/>\r\n\
\t\t\t\t\t<xr:FillValue xsi:nil=\"true\"/>\r\n\
\t\t\t\t\t<xr:Mask/>\r\n\
\t\t\t\t\t<xr:ChoiceParameters/>\r\n\
\t\t\t\t</xr:StandardAttribute>\r\n",
            escape_xml_text(name)
        ));
    }
    xml.push_str("\t\t\t</StandardAttributes>\r\n");
}

fn push_register_standard_attributes_xml(
    xml: &mut String,
    attributes: &[RegisterStandardAttribute],
) {
    if attributes.is_empty() {
        return;
    }
    xml.push_str("\t\t\t<StandardAttributes>\r\n");
    for attribute in attributes {
        xml.push_str(&format!(
            "\t\t\t\t<xr:StandardAttribute name=\"{}\">\r\n",
            escape_xml_text(attribute.name),
        ));
        if let Some(link_by_type) = &attribute.link_by_type {
            xml.push_str(&format!(
                "\t\t\t\t\t<xr:LinkByType>\r\n\
\t\t\t\t\t\t<xr:DataPath>{}</xr:DataPath>\r\n\
\t\t\t\t\t\t<xr:LinkItem>{}</xr:LinkItem>\r\n\
\t\t\t\t\t</xr:LinkByType>\r\n",
                escape_xml_element_text(&link_by_type.data_path),
                link_by_type.link_item
            ));
        } else {
            xml.push_str("\t\t\t\t\t<xr:LinkByType/>\r\n");
        }
        xml.push_str(&format!(
            "\t\t\t\t\t<xr:FillChecking>{}</xr:FillChecking>\r\n\
\t\t\t\t\t<xr:MultiLine>false</xr:MultiLine>\r\n\
\t\t\t\t\t<xr:FillFromFillingValue>{}</xr:FillFromFillingValue>\r\n\
\t\t\t\t\t<xr:CreateOnInput>Auto</xr:CreateOnInput>\r\n\
\t\t\t\t\t<xr:TypeReductionMode>TransformValues</xr:TypeReductionMode>\r\n\
\t\t\t\t\t<xr:MaxValue xsi:nil=\"true\"/>\r\n",
            attribute.fill_checking,
            xml_bool(attribute.fill_from_filling_value),
        ));
        push_xr_localized_property_xml(xml, "\t\t\t\t\t", "ToolTip", &attribute.tooltip);
        xml.push_str("\t\t\t\t\t<xr:ExtendedEdit>false</xr:ExtendedEdit>\r\n");
        push_xr_localized_property_xml(xml, "\t\t\t\t\t", "Format", &attribute.format);
        xml.push_str(
            "\t\t\t\t\t<xr:ChoiceForm/>\r\n\
\t\t\t\t\t<xr:QuickChoice>Auto</xr:QuickChoice>\r\n\
\t\t\t\t\t<xr:ChoiceHistoryOnInput>Auto</xr:ChoiceHistoryOnInput>\r\n",
        );
        push_xr_localized_property_xml(xml, "\t\t\t\t\t", "EditFormat", &attribute.edit_format);
        xml.push_str(&format!(
            "\t\t\t\t\t<xr:PasswordMode>false</xr:PasswordMode>\r\n\
\t\t\t\t\t<xr:DataHistory>{}</xr:DataHistory>\r\n\
\t\t\t\t\t<xr:MarkNegatives>false</xr:MarkNegatives>\r\n\
\t\t\t\t\t<xr:MinValue xsi:nil=\"true\"/>\r\n",
            attribute.data_history,
        ));
        push_xr_localized_property_xml(xml, "\t\t\t\t\t", "Synonym", &attribute.synonym);
        xml.push_str(&format!(
            "\t\t\t\t\t<xr:Comment/>\r\n\
\t\t\t\t\t<xr:FullTextSearch>{}</xr:FullTextSearch>\r\n\
\t\t\t\t\t<xr:ChoiceParameterLinks/>\r\n\
",
            attribute.full_text_search,
        ));
        xml.push_str("\t\t\t\t\t");
        xml.push_str(&format_register_standard_attribute_fill_value_xml(
            &attribute.fill_value,
        ));
        xml.push_str(
            "\r\n\
\t\t\t\t\t<xr:Mask/>\r\n\
\t\t\t\t\t<xr:ChoiceParameters/>\r\n\
\t\t\t\t</xr:StandardAttribute>\r\n",
        );
    }
    xml.push_str("\t\t\t</StandardAttributes>\r\n");
}

fn format_register_standard_attribute_fill_value_xml(value: &MetadataChildFillValue) -> String {
    match value {
        MetadataChildFillValue::Nil => "<xr:FillValue xsi:nil=\"true\"/>".to_string(),
        MetadataChildFillValue::Boolean(value) => format!(
            "<xr:FillValue xsi:type=\"xs:boolean\">{}</xr:FillValue>",
            xml_bool(*value)
        ),
        MetadataChildFillValue::Decimal(value) => format!(
            "<xr:FillValue xsi:type=\"xs:decimal\">{}</xr:FillValue>",
            escape_xml_element_text(value)
        ),
        MetadataChildFillValue::DateTime(value) => format!(
            "<xr:FillValue xsi:type=\"xs:dateTime\">{}</xr:FillValue>",
            escape_xml_element_text(value)
        ),
        MetadataChildFillValue::DesignTimeRef(value) if value.is_empty() => {
            "<xr:FillValue xsi:type=\"xr:DesignTimeRef\"/>".to_string()
        }
        MetadataChildFillValue::DesignTimeRef(value) => format!(
            "<xr:FillValue xsi:type=\"xr:DesignTimeRef\">{}</xr:FillValue>",
            escape_xml_element_text(value)
        ),
        MetadataChildFillValue::String(value) if value.is_empty() => {
            "<xr:FillValue xsi:type=\"xs:string\"/>".to_string()
        }
        MetadataChildFillValue::String(value) => format!(
            "<xr:FillValue xsi:type=\"xs:string\">{}</xr:FillValue>",
            escape_xml_element_text(value)
        ),
    }
}

fn push_xr_localized_property_xml(
    xml: &mut String,
    indent: &str,
    name: &str,
    values: &[(String, String)],
) {
    if values.is_empty() {
        xml.push_str(&format!("{indent}<xr:{name}/>\r\n"));
        return;
    }
    xml.push_str(&format!("{indent}<xr:{name}>\r\n"));
    for (lang, content) in values {
        xml.push_str(&format!("{indent}\t<v8:item>\r\n"));
        xml.push_str(&format!(
            "{indent}\t\t<v8:lang>{}</v8:lang>\r\n",
            escape_xml_element_text(lang)
        ));
        xml.push_str(&format!(
            "{indent}\t\t<v8:content>{}</v8:content>\r\n",
            escape_xml_element_text(content)
        ));
        xml.push_str(&format!("{indent}\t</v8:item>\r\n"));
    }
    xml.push_str(&format!("{indent}</xr:{name}>\r\n"));
}

fn push_document_numbering_xml(xml: &mut String, numbering: &DocumentNumberingProperties) {
    push_optional_text_element(xml, "\t\t\t", "Numerator", numbering.numerator.as_deref());
    xml.push_str(&format!(
        "\t\t\t<NumberType>{}</NumberType>\r\n\
\t\t\t<NumberLength>{}</NumberLength>\r\n\
\t\t\t<NumberAllowedLength>{}</NumberAllowedLength>\r\n\
\t\t\t<NumberPeriodicity>{}</NumberPeriodicity>\r\n\
\t\t\t<CheckUnique>{}</CheckUnique>\r\n\
\t\t\t<Autonumbering>{}</Autonumbering>\r\n",
        numbering.number_type,
        numbering.number_length,
        numbering.number_allowed_length,
        numbering.number_periodicity,
        xml_bool(numbering.check_unique),
        xml_bool(numbering.autonumbering)
    ));
}

fn push_document_standard_attributes_xml(
    xml: &mut String,
    standard_attributes: &DocumentStandardAttributes,
) {
    xml.push_str("\t\t\t<StandardAttributes>\r\n");
    for attribute in document_standard_attributes() {
        push_document_standard_attribute_xml(xml, attribute, standard_attributes);
    }
    xml.push_str("\t\t\t</StandardAttributes>\r\n");
}

struct DocumentStandardAttribute {
    name: &'static str,
    fill_checking: &'static str,
    fill_value: DocumentStandardAttributeFillValue,
}

enum DocumentStandardAttributeFillValue {
    Nil,
    BooleanFalse,
    DateTimeZero,
    Number,
}

fn document_standard_attributes() -> &'static [DocumentStandardAttribute] {
    &[
        DocumentStandardAttribute {
            name: "Posted",
            fill_checking: "DontCheck",
            fill_value: DocumentStandardAttributeFillValue::Nil,
        },
        DocumentStandardAttribute {
            name: "Ref",
            fill_checking: "DontCheck",
            fill_value: DocumentStandardAttributeFillValue::Nil,
        },
        DocumentStandardAttribute {
            name: "DeletionMark",
            fill_checking: "DontCheck",
            fill_value: DocumentStandardAttributeFillValue::BooleanFalse,
        },
        DocumentStandardAttribute {
            name: "Date",
            fill_checking: "ShowError",
            fill_value: DocumentStandardAttributeFillValue::DateTimeZero,
        },
        DocumentStandardAttribute {
            name: "Number",
            fill_checking: "DontCheck",
            fill_value: DocumentStandardAttributeFillValue::Number,
        },
    ]
}

fn push_document_standard_attribute_xml(
    xml: &mut String,
    attribute: &DocumentStandardAttribute,
    standard_attributes: &DocumentStandardAttributes,
) {
    xml.push_str(&format!(
        "\t\t\t\t<xr:StandardAttribute name=\"{}\">\r\n\
\t\t\t\t\t<xr:LinkByType/>\r\n\
\t\t\t\t\t<xr:FillChecking>{}</xr:FillChecking>\r\n\
\t\t\t\t\t<xr:MultiLine>false</xr:MultiLine>\r\n\
\t\t\t\t\t<xr:FillFromFillingValue>false</xr:FillFromFillingValue>\r\n\
\t\t\t\t\t<xr:CreateOnInput>Auto</xr:CreateOnInput>\r\n\
\t\t\t\t\t<xr:TypeReductionMode>TransformValues</xr:TypeReductionMode>\r\n\
\t\t\t\t\t<xr:MaxValue xsi:nil=\"true\"/>\r\n",
        escape_xml_text(attribute.name),
        attribute.fill_checking,
    ));
    let details = standard_attributes.details.get(attribute.name);
    push_xr_localized_property_xml(
        xml,
        "\t\t\t\t\t",
        "ToolTip",
        details
            .map(|details| details.tooltip.as_slice())
            .unwrap_or_default(),
    );
    xml.push_str(
        "\t\t\t\t\t<xr:ExtendedEdit>false</xr:ExtendedEdit>\r\n\
\t\t\t\t\t<xr:Format/>\r\n\
\t\t\t\t\t<xr:ChoiceForm/>\r\n\
\t\t\t\t\t<xr:QuickChoice>Auto</xr:QuickChoice>\r\n\
\t\t\t\t\t<xr:ChoiceHistoryOnInput>Auto</xr:ChoiceHistoryOnInput>\r\n\
\t\t\t\t\t<xr:EditFormat/>\r\n\
\t\t\t\t\t<xr:PasswordMode>false</xr:PasswordMode>\r\n\
\t\t\t\t\t<xr:DataHistory>Use</xr:DataHistory>\r\n\
\t\t\t\t\t<xr:MarkNegatives>false</xr:MarkNegatives>\r\n\
\t\t\t\t\t<xr:MinValue xsi:nil=\"true\"/>\r\n",
    );
    push_xr_localized_property_xml(
        xml,
        "\t\t\t\t\t",
        "Synonym",
        details
            .map(|details| details.synonym.as_slice())
            .unwrap_or_default(),
    );
    xml.push_str(
        "\t\t\t\t\t<xr:Comment/>\r\n\
\t\t\t\t\t<xr:FullTextSearch>Use</xr:FullTextSearch>\r\n\
\t\t\t\t\t<xr:ChoiceParameterLinks/>\r\n",
    );
    push_document_standard_attribute_fill_value(xml, attribute, standard_attributes);
    xml.push_str(
        "\t\t\t\t\t<xr:Mask/>\r\n\
\t\t\t\t\t<xr:ChoiceParameters/>\r\n\
\t\t\t\t</xr:StandardAttribute>\r\n",
    );
}

fn push_document_standard_attribute_fill_value(
    xml: &mut String,
    attribute: &DocumentStandardAttribute,
    standard_attributes: &DocumentStandardAttributes,
) {
    match attribute.fill_value {
        DocumentStandardAttributeFillValue::Nil => {
            xml.push_str("\t\t\t\t\t<xr:FillValue xsi:nil=\"true\"/>\r\n");
        }
        DocumentStandardAttributeFillValue::BooleanFalse => {
            xml.push_str(
                "\t\t\t\t\t<xr:FillValue xsi:type=\"xs:boolean\">false</xr:FillValue>\r\n",
            );
        }
        DocumentStandardAttributeFillValue::DateTimeZero => {
            xml.push_str(
                "\t\t\t\t\t<xr:FillValue xsi:type=\"xs:dateTime\">0001-01-01T00:00:00</xr:FillValue>\r\n",
            );
        }
        DocumentStandardAttributeFillValue::Number => {
            if standard_attributes.number_type == "String" {
                xml.push_str("\t\t\t\t\t<xr:FillValue xsi:type=\"xs:string\"/>\r\n");
            } else {
                xml.push_str("\t\t\t\t\t<xr:FillValue xsi:nil=\"true\"/>\r\n");
            }
        }
    }
}

fn push_catalog_input_by_string_xml(xml: &mut String, fields: &[String]) {
    if fields.is_empty() {
        xml.push_str("\t\t\t<InputByString/>\r\n");
        return;
    }
    xml.push_str("\t\t\t<InputByString>\r\n");
    for field in fields {
        xml.push_str(&format!(
            "\t\t\t\t<xr:Field>{}</xr:Field>\r\n",
            escape_xml_text(field)
        ));
    }
    xml.push_str("\t\t\t</InputByString>\r\n");
}

fn push_catalog_standard_attributes_xml(xml: &mut String, catalog: &CatalogProperties) {
    xml.push_str("\t\t\t<StandardAttributes>\r\n");
    for attribute in catalog_standard_attributes() {
        push_catalog_standard_attribute_xml(xml, attribute, catalog);
    }
    xml.push_str("\t\t\t</StandardAttributes>\r\n");
}

struct CatalogStandardAttribute {
    name: &'static str,
    fill_checking: &'static str,
    fill_from_filling_value: bool,
    type_reduction_mode: &'static str,
    fill_value: CatalogStandardAttributeFillValue,
}

enum CatalogStandardAttributeFillValue {
    Nil,
    EmptyString,
    CodeString,
}

fn catalog_standard_attributes() -> &'static [CatalogStandardAttribute] {
    &[
        CatalogStandardAttribute {
            name: "PredefinedDataName",
            fill_checking: "DontCheck",
            fill_from_filling_value: false,
            type_reduction_mode: "TransformValues",
            fill_value: CatalogStandardAttributeFillValue::Nil,
        },
        CatalogStandardAttribute {
            name: "Predefined",
            fill_checking: "DontCheck",
            fill_from_filling_value: false,
            type_reduction_mode: "TransformValues",
            fill_value: CatalogStandardAttributeFillValue::Nil,
        },
        CatalogStandardAttribute {
            name: "Ref",
            fill_checking: "DontCheck",
            fill_from_filling_value: false,
            type_reduction_mode: "TransformValues",
            fill_value: CatalogStandardAttributeFillValue::Nil,
        },
        CatalogStandardAttribute {
            name: "DeletionMark",
            fill_checking: "DontCheck",
            fill_from_filling_value: false,
            type_reduction_mode: "TransformValues",
            fill_value: CatalogStandardAttributeFillValue::Nil,
        },
        CatalogStandardAttribute {
            name: "IsFolder",
            fill_checking: "DontCheck",
            fill_from_filling_value: false,
            type_reduction_mode: "TransformValues",
            fill_value: CatalogStandardAttributeFillValue::Nil,
        },
        CatalogStandardAttribute {
            name: "Owner",
            fill_checking: "ShowError",
            fill_from_filling_value: true,
            type_reduction_mode: "Deny",
            fill_value: CatalogStandardAttributeFillValue::Nil,
        },
        CatalogStandardAttribute {
            name: "Parent",
            fill_checking: "DontCheck",
            fill_from_filling_value: true,
            type_reduction_mode: "TransformValues",
            fill_value: CatalogStandardAttributeFillValue::Nil,
        },
        CatalogStandardAttribute {
            name: "Description",
            fill_checking: "ShowError",
            fill_from_filling_value: false,
            type_reduction_mode: "TransformValues",
            fill_value: CatalogStandardAttributeFillValue::EmptyString,
        },
        CatalogStandardAttribute {
            name: "Code",
            fill_checking: "ShowError",
            fill_from_filling_value: false,
            type_reduction_mode: "TransformValues",
            fill_value: CatalogStandardAttributeFillValue::CodeString,
        },
    ]
}

fn push_catalog_standard_attribute_xml(
    xml: &mut String,
    attribute: &CatalogStandardAttribute,
    catalog: &CatalogProperties,
) {
    let details = catalog.standard_attribute_details.get(attribute.name);
    xml.push_str(&format!(
        "\t\t\t\t<xr:StandardAttribute name=\"{}\">\r\n\
\t\t\t\t\t<xr:LinkByType/>\r\n\
\t\t\t\t\t<xr:FillChecking>{}</xr:FillChecking>\r\n\
\t\t\t\t\t<xr:MultiLine>false</xr:MultiLine>\r\n\
\t\t\t\t\t<xr:FillFromFillingValue>{}</xr:FillFromFillingValue>\r\n\
\t\t\t\t\t<xr:CreateOnInput>Auto</xr:CreateOnInput>\r\n\
\t\t\t\t\t<xr:TypeReductionMode>{}</xr:TypeReductionMode>\r\n\
\t\t\t\t\t<xr:MaxValue xsi:nil=\"true\"/>\r\n",
        escape_xml_text(attribute.name),
        attribute.fill_checking,
        xml_bool(attribute.fill_from_filling_value),
        attribute.type_reduction_mode
    ));
    push_localized_property(
        xml,
        "\t\t\t\t\t",
        "xr:ToolTip",
        details
            .map(|detail| detail.tooltip.as_slice())
            .unwrap_or(&[]),
    );
    xml.push_str(
        "\t\t\t\t\t<xr:ExtendedEdit>false</xr:ExtendedEdit>\r\n\
\t\t\t\t\t<xr:Format/>\r\n\
\t\t\t\t\t<xr:ChoiceForm/>\r\n\
\t\t\t\t\t<xr:QuickChoice>Auto</xr:QuickChoice>\r\n\
\t\t\t\t\t<xr:ChoiceHistoryOnInput>Auto</xr:ChoiceHistoryOnInput>\r\n\
\t\t\t\t\t<xr:EditFormat/>\r\n\
\t\t\t\t\t<xr:PasswordMode>false</xr:PasswordMode>\r\n\
\t\t\t\t\t<xr:DataHistory>Use</xr:DataHistory>\r\n\
\t\t\t\t\t<xr:MarkNegatives>false</xr:MarkNegatives>\r\n\
\t\t\t\t\t<xr:MinValue xsi:nil=\"true\"/>\r\n",
    );
    push_localized_property(
        xml,
        "\t\t\t\t\t",
        "xr:Synonym",
        details
            .map(|detail| detail.synonym.as_slice())
            .unwrap_or(&[]),
    );
    xml.push_str(
        "\t\t\t\t\t<xr:Comment/>\r\n\
\t\t\t\t\t<xr:FullTextSearch>Use</xr:FullTextSearch>\r\n\
\t\t\t\t\t<xr:ChoiceParameterLinks/>\r\n",
    );
    push_catalog_standard_attribute_fill_value(xml, attribute, catalog);
    xml.push_str(
        "\t\t\t\t\t<xr:Mask/>\r\n\
\t\t\t\t\t<xr:ChoiceParameters/>\r\n\
\t\t\t\t</xr:StandardAttribute>\r\n",
    );
}

fn push_catalog_standard_attribute_fill_value(
    xml: &mut String,
    attribute: &CatalogStandardAttribute,
    catalog: &CatalogProperties,
) {
    match attribute.fill_value {
        CatalogStandardAttributeFillValue::Nil => {
            xml.push_str("\t\t\t\t\t<xr:FillValue xsi:nil=\"true\"/>\r\n");
        }
        CatalogStandardAttributeFillValue::EmptyString => {
            xml.push_str("\t\t\t\t\t<xr:FillValue xsi:type=\"xs:string\"/>\r\n");
        }
        CatalogStandardAttributeFillValue::CodeString => {
            if catalog.code_type == Some("String") {
                xml.push_str(&format!(
                    "\t\t\t\t\t<xr:FillValue xsi:type=\"xs:string\">{}</xr:FillValue>\r\n",
                    " ".repeat(catalog.code_length as usize)
                ));
            } else {
                xml.push_str("\t\t\t\t\t<xr:FillValue xsi:nil=\"true\"/>\r\n");
            }
        }
    }
}

fn push_optional_text_element(xml: &mut String, indent: &str, name: &str, value: Option<&str>) {
    if let Some(value) = value {
        xml.push_str(&format!(
            "{indent}<{name}>{}</{name}>\r\n",
            escape_xml_text(value)
        ));
    } else {
        xml.push_str(&format!("{indent}<{name}/>\r\n"));
    }
}

fn push_localized_property(
    xml: &mut String,
    indent: &str,
    name: &str,
    values: &[(String, String)],
) {
    if values.is_empty() {
        xml.push_str(&format!("{indent}<{name}/>\r\n"));
        return;
    }
    xml.push_str(&format!("{indent}<{name}>\r\n"));
    for (lang, content) in values {
        xml.push_str(&format!("{indent}\t<v8:item>\r\n"));
        xml.push_str(&format!(
            "{indent}\t\t<v8:lang>{}</v8:lang>\r\n",
            escape_xml_element_text(lang)
        ));
        xml.push_str(&format!(
            "{indent}\t\t<v8:content>{}</v8:content>\r\n",
            escape_xml_element_text(content)
        ));
        xml.push_str(&format!("{indent}\t</v8:item>\r\n"));
    }
    xml.push_str(&format!("{indent}</{name}>\r\n"));
}

fn format_template_source_xml(
    kind: &str,
    header: &MetadataHeader,
    template_type: &str,
    source_version: InfobaseConfigSourceVersion,
) -> String {
    let mut xml = format_full_metadata_source_xml(kind, header, source_version);
    let insert = format!(
        "\t\t\t<TemplateType>{}</TemplateType>\r\n",
        escape_xml_text(template_type)
    );
    xml = xml.replace("\t\t</Properties>", &format!("{insert}\t\t</Properties>"));
    xml
}

fn format_functional_option_source_xml(
    header: &MetadataHeader,
    properties: &FunctionalOptionProperties,
    source_version: InfobaseConfigSourceVersion,
) -> String {
    let mut xml = format_full_metadata_source_xml("FunctionalOption", header, source_version);
    let mut insert = String::new();
    if let Some(location) = &properties.location {
        insert.push_str(&format!(
            "\t\t\t<Location>{}</Location>\r\n",
            escape_xml_text(location)
        ));
    }
    insert.push_str(&format!(
        "\t\t\t<PrivilegedGetMode>{}</PrivilegedGetMode>\r\n",
        xml_bool(properties.privileged_get_mode)
    ));
    if properties.content.is_empty() {
        insert.push_str("\t\t\t<Content/>\r\n");
    } else {
        insert.push_str("\t\t\t<Content>\r\n");
        for item in &properties.content {
            insert.push_str(&format!(
                "\t\t\t\t<xr:Object>{}</xr:Object>\r\n",
                escape_xml_text(item)
            ));
        }
        insert.push_str("\t\t\t</Content>\r\n");
    }
    xml = xml.replace("\t\t</Properties>", &format!("{insert}\t\t</Properties>"));
    xml
}

fn insert_metadata_child_commands_xml(
    xml: &mut String,
    owner_kind: &str,
    commands: &[MetadataHeader],
) {
    if commands.is_empty() {
        return;
    }
    let mut child_objects = String::new();
    for command in commands {
        push_metadata_header_child_object_xml(&mut child_objects, "Command", command);
    }
    insert_metadata_child_objects_xml(xml, owner_kind, &child_objects);
}

fn insert_metadata_child_command_objects_xml(
    xml: &mut String,
    owner_kind: &str,
    commands: &[MetadataChildCommand],
) {
    if commands.is_empty() {
        return;
    }
    let mut child_objects = String::new();
    for command in commands {
        push_metadata_child_command_xml(&mut child_objects, command);
    }
    insert_metadata_child_objects_xml(xml, owner_kind, &child_objects);
}

fn insert_metadata_child_objects_xml(xml: &mut String, owner_kind: &str, child_objects: &str) {
    if child_objects.is_empty() {
        return;
    }
    if let Some(index) = xml.rfind("\t\t</ChildObjects>") {
        xml.insert_str(index, child_objects);
        return;
    }
    let marker = format!("\t</{owner_kind}>");
    let Some(index) = xml.find(&marker) else {
        return;
    };
    xml.insert_str(
        index,
        &format!("\t\t<ChildObjects>\r\n{child_objects}\t\t</ChildObjects>\r\n"),
    );
}

fn push_metadata_header_child_object_xml(
    xml: &mut String,
    tag: &'static str,
    header: &MetadataHeader,
) {
    push_metadata_child_object_xml(
        xml,
        &MetadataChildObject {
            tag,
            header: header.clone(),
            generated_types: Vec::new(),
            value_types: Vec::new(),
            emit_empty_type: tag == "Attribute",
            properties: None,
            tabular_section_properties: None,
            child_objects: Vec::new(),
        },
    );
}

fn push_metadata_child_command_xml(xml: &mut String, command: &MetadataChildCommand) {
    xml.push_str(&format!(
        "\t\t\t<Command uuid=\"{}\">\r\n\
\t\t\t\t<Properties>\r\n\
\t\t\t\t\t<Name>{}</Name>\r\n",
        escape_xml_text(&command.header.uuid),
        escape_xml_element_text(&command.header.name),
    ));
    push_header_synonym_xml(xml, "\t\t\t\t\t", &command.header.synonyms);
    if command.header.comment.is_empty() {
        xml.push_str("\t\t\t\t\t<Comment/>\r\n");
    } else {
        xml.push_str(&format!(
            "\t\t\t\t\t<Comment>{}</Comment>\r\n",
            escape_xml_element_text(&command.header.comment)
        ));
    }
    if let Some(properties) = &command.properties {
        push_metadata_child_command_properties_xml(xml, properties);
    }
    xml.push_str("\t\t\t\t</Properties>\r\n\t\t\t</Command>\r\n");
}

fn push_metadata_child_command_properties_xml(
    xml: &mut String,
    properties: &CommonCommandProperties,
) {
    xml.push_str(&format!(
        "\t\t\t\t\t<Group>{}</Group>\r\n",
        escape_xml_text(properties.group.as_deref().unwrap_or("ActionsPanelTools"))
    ));
    format_common_command_parameter_type_xml_with_indent(
        xml,
        &properties.command_parameter_types,
        "\t\t\t\t\t",
    );
    xml.push_str(&format!(
        "\t\t\t\t\t<ParameterUseMode>{}</ParameterUseMode>\r\n\
\t\t\t\t\t<ModifiesData>{}</ModifiesData>\r\n\
\t\t\t\t\t<Representation>{}</Representation>\r\n",
        properties.parameter_use_mode,
        xml_bool(properties.modifies_data),
        properties.representation
    ));
    push_common_command_tooltip_xml_with_indent(xml, "\t\t\t\t\t", &properties.tooltip);
    format_common_command_picture_xml_with_indent(xml, "\t\t\t\t\t", properties);
    if let Some(shortcut) = properties.shortcut.as_deref() {
        xml.push_str(&format!(
            "\t\t\t\t\t<Shortcut>{}</Shortcut>\r\n",
            escape_xml_element_text(shortcut)
        ));
    } else {
        xml.push_str("\t\t\t\t\t<Shortcut/>\r\n");
    }
    xml.push_str(&format!(
        "\t\t\t\t\t<OnMainServerUnavalableBehavior>{}</OnMainServerUnavalableBehavior>\r\n",
        properties.on_main_server_unavailable_behavior
    ));
}

fn push_metadata_child_object_xml(xml: &mut String, child: &MetadataChildObject) {
    xml.push_str(&format!(
        "\t\t\t<{tag} uuid=\"{}\">\r\n",
        escape_xml_text(&child.header.uuid),
        tag = child.tag,
    ));
    if !child.generated_types.is_empty() {
        xml.push_str(&format_generated_types_internal_info_xml_with_indent(
            &child.generated_types,
            "\t\t\t\t",
        ));
    }
    xml.push_str(&format!(
        "\t\t\t\t<Properties>\r\n\
\t\t\t\t\t<Name>{}</Name>\r\n",
        escape_xml_element_text(&child.header.name),
    ));
    push_header_synonym_xml(xml, "\t\t\t\t\t", &child.header.synonyms);
    if child.header.comment.is_empty() {
        xml.push_str("\t\t\t\t\t<Comment/>\r\n");
    } else {
        xml.push_str(&format!(
            "\t\t\t\t\t<Comment>{}</Comment>\r\n",
            escape_xml_element_text(&child.header.comment)
        ));
    }
    if !child.value_types.is_empty() {
        xml.push_str(&format_metadata_types_xml_with_indent(
            &child.value_types,
            "\t\t\t\t\t",
        ));
    } else if child.emit_empty_type {
        xml.push_str("\t\t\t\t\t<Type/>\r\n");
    }
    if let Some(properties) = &child.properties {
        push_metadata_child_properties_xml(xml, "\t\t\t\t\t", properties);
    }
    if let Some(properties) = &child.tabular_section_properties {
        push_metadata_tabular_section_properties_xml(xml, "\t\t\t\t\t", properties);
    }
    xml.push_str("\t\t\t\t</Properties>\r\n");
    if !child.child_objects.is_empty() {
        xml.push_str("\t\t\t\t<ChildObjects>\r\n");
        for nested_child in &child.child_objects {
            push_nested_metadata_child_object_xml(xml, nested_child, 5);
        }
        xml.push_str("\t\t\t\t</ChildObjects>\r\n");
    }
    xml.push_str(&format!("\t\t\t</{}>\r\n", child.tag));
}

fn push_nested_metadata_child_object_xml(
    xml: &mut String,
    child: &MetadataChildObject,
    indent: usize,
) {
    let tab = "\t".repeat(indent);
    xml.push_str(&format!(
        "{tab}<{} uuid=\"{}\">\r\n",
        child.tag,
        escape_xml_text(&child.header.uuid)
    ));
    if !child.generated_types.is_empty() {
        xml.push_str(&format_generated_types_internal_info_xml_with_indent(
            &child.generated_types,
            &format!("{tab}\t"),
        ));
    }
    xml.push_str(&format!("{tab}\t<Properties>\r\n"));
    xml.push_str(&format!(
        "{tab}\t\t<Name>{}</Name>\r\n",
        escape_xml_element_text(&child.header.name)
    ));
    push_header_synonym_xml(xml, &format!("{tab}\t\t"), &child.header.synonyms);
    if child.header.comment.is_empty() {
        xml.push_str(&format!("{tab}\t\t<Comment/>\r\n"));
    } else {
        xml.push_str(&format!(
            "{tab}\t\t<Comment>{}</Comment>\r\n",
            escape_xml_element_text(&child.header.comment)
        ));
    }
    if !child.value_types.is_empty() {
        xml.push_str(&format_metadata_types_xml_with_indent(
            &child.value_types,
            &format!("{tab}\t\t"),
        ));
    } else if child.emit_empty_type {
        xml.push_str(&format!("{tab}\t\t<Type/>\r\n"));
    }
    if let Some(properties) = &child.properties {
        push_metadata_child_properties_xml(xml, &format!("{tab}\t\t"), properties);
    }
    if let Some(properties) = &child.tabular_section_properties {
        push_metadata_tabular_section_properties_xml(xml, &format!("{tab}\t\t"), properties);
    }
    xml.push_str(&format!("{tab}\t</Properties>\r\n"));
    if !child.child_objects.is_empty() {
        xml.push_str(&format!("{tab}\t<ChildObjects>\r\n"));
        for nested_child in &child.child_objects {
            push_nested_metadata_child_object_xml(xml, nested_child, indent + 2);
        }
        xml.push_str(&format!("{tab}\t</ChildObjects>\r\n"));
    }
    xml.push_str(&format!("{tab}</{}>\r\n", child.tag));
}

fn push_metadata_child_properties_xml(
    xml: &mut String,
    indent: &str,
    properties: &MetadataChildProperties,
) {
    xml.push_str(&format!(
        "{indent}<PasswordMode>{}</PasswordMode>\r\n",
        xml_bool(properties.password_mode)
    ));
    push_localized_property(xml, indent, "Format", &properties.format);
    push_localized_property(xml, indent, "EditFormat", &properties.edit_format);
    push_localized_property(xml, indent, "ToolTip", &properties.tooltip);
    xml.push_str(&format!(
        "{indent}<MarkNegatives>{}</MarkNegatives>\r\n\
{indent}{}\r\n\
{indent}<MultiLine>{}</MultiLine>\r\n\
{indent}<ExtendedEdit>{}</ExtendedEdit>\r\n\
{indent}{}\r\n\
{indent}{}\r\n\
",
        xml_bool(properties.mark_negatives),
        format_simple_property_xml("Mask", &properties.mask),
        xml_bool(properties.multi_line),
        xml_bool(properties.extended_edit),
        format_constant_bound_xml("MinValue", properties.min_value.as_deref()),
        format_constant_bound_xml("MaxValue", properties.max_value.as_deref()),
    ));
    if properties.emit_fill_from_filling_value {
        xml.push_str(&format!(
            "{indent}<FillFromFillingValue>{}</FillFromFillingValue>\r\n",
            xml_bool(properties.fill_from_filling_value),
        ));
    }
    if properties.emit_fill_value
        && let Some(fill_value) = &properties.fill_value
    {
        xml.push_str(&format!(
            "{indent}{}\r\n",
            format_metadata_child_fill_value_xml(fill_value)
        ));
    }
    xml.push_str(&format!(
        "{indent}<FillChecking>{}</FillChecking>\r\n",
        properties.fill_checking
    ));
    if let Some(choice_folders_and_items) = properties.choice_folders_and_items {
        xml.push_str(&format!(
            "{indent}<ChoiceFoldersAndItems>{choice_folders_and_items}</ChoiceFoldersAndItems>\r\n"
        ));
    }
    push_metadata_child_choice_parameter_links_xml(xml, indent, &properties.choice_parameter_links);
    if let Some(choice_parameters) = &properties.choice_parameters {
        push_metadata_child_choice_parameters_xml_with_style(
            xml,
            indent,
            choice_parameters,
            properties.self_close_empty_choice_parameter_refs,
        );
    }
    if let Some(quick_choice) = properties.quick_choice {
        xml.push_str(&format!(
            "{indent}<QuickChoice>{quick_choice}</QuickChoice>\r\n"
        ));
    }
    if let Some(create_on_input) = properties.create_on_input {
        xml.push_str(&format!(
            "{indent}<CreateOnInput>{create_on_input}</CreateOnInput>\r\n"
        ));
    }
    if let Some(choice_form) = &properties.choice_form {
        match choice_form {
            MetadataChoiceForm::Empty => {
                xml.push_str(&format!("{indent}<ChoiceForm/>\r\n"));
            }
            MetadataChoiceForm::Reference(reference) => {
                xml.push_str(&format!(
                    "{indent}<ChoiceForm>{}</ChoiceForm>\r\n",
                    escape_xml_element_text(reference)
                ));
            }
        }
    }
    if let Some(link_by_type) = &properties.link_by_type {
        xml.push_str(&format!(
            "{indent}<LinkByType>\r\n\
{indent}\t<xr:DataPath>{}</xr:DataPath>\r\n\
{indent}\t<xr:LinkItem>{}</xr:LinkItem>\r\n\
{indent}</LinkByType>\r\n",
            escape_xml_element_text(&link_by_type.data_path),
            link_by_type.link_item,
        ));
    } else if properties.link_by_type_empty {
        xml.push_str(&format!("{indent}<LinkByType/>\r\n"));
    }
    if let Some(choice_history_on_input) = properties.choice_history_on_input {
        xml.push_str(&format!(
            "{indent}<ChoiceHistoryOnInput>{choice_history_on_input}</ChoiceHistoryOnInput>\r\n"
        ));
    }
    if let Some(master) = properties.master {
        xml.push_str(&format!(
            "{indent}<Master>{}</Master>\r\n",
            xml_bool(master)
        ));
    }
    if let Some(main_filter) = properties.main_filter {
        xml.push_str(&format!(
            "{indent}<MainFilter>{}</MainFilter>\r\n",
            xml_bool(main_filter)
        ));
    }
    if let Some(balance) = properties.balance {
        xml.push_str(&format!(
            "{indent}<Balance>{}</Balance>\r\n",
            xml_bool(balance)
        ));
    }
    if let Some(accounting_flag) = &properties.accounting_flag {
        xml.push_str(&format!(
            "{indent}<AccountingFlag>{}</AccountingFlag>\r\n",
            escape_xml_element_text(accounting_flag)
        ));
    } else if properties.balance.is_some() || properties.ext_dimension_accounting_flag.is_some() {
        xml.push_str(&format!("{indent}<AccountingFlag/>\r\n"));
    }
    if let Some(ext_dimension_accounting_flag) = &properties.ext_dimension_accounting_flag {
        if ext_dimension_accounting_flag.is_empty() {
            xml.push_str(&format!("{indent}<ExtDimensionAccountingFlag/>\r\n"));
        } else {
            xml.push_str(&format!(
                "{indent}<ExtDimensionAccountingFlag>{}</ExtDimensionAccountingFlag>\r\n",
                escape_xml_element_text(ext_dimension_accounting_flag)
            ));
        }
    }
    if let Some(deny_incomplete_values) = properties.deny_incomplete_values {
        xml.push_str(&format!(
            "{indent}<DenyIncompleteValues>{}</DenyIncompleteValues>\r\n",
            xml_bool(deny_incomplete_values)
        ));
    }
    if let Some(use_mode) = properties.use_mode {
        xml.push_str(&format!("{indent}<Use>{use_mode}</Use>\r\n"));
    }
    if let Some(indexing) = properties.indexing {
        xml.push_str(&format!("{indent}<Indexing>{indexing}</Indexing>\r\n"));
    }
    if let Some(full_text_search) = properties.full_text_search {
        xml.push_str(&format!(
            "{indent}<FullTextSearch>{full_text_search}</FullTextSearch>\r\n"
        ));
    }
    if let Some(data_history) = properties.data_history {
        xml.push_str(&format!(
            "{indent}<DataHistory>{data_history}</DataHistory>\r\n"
        ));
    }
    if let Some(type_reduction_mode) = properties.type_reduction_mode {
        xml.push_str(&format!(
            "{indent}<TypeReductionMode>{type_reduction_mode}</TypeReductionMode>\r\n"
        ));
    }
    if let Some(update_data_history) = properties.update_data_history_immediately_after_write {
        xml.push_str(&format!(
            "{indent}<UpdateDataHistoryImmediatelyAfterWrite>{}</UpdateDataHistoryImmediatelyAfterWrite>\r\n",
            xml_bool(update_data_history)
        ));
    }
    if let Some(execute_after_write) =
        properties.execute_after_write_data_history_version_processing
    {
        xml.push_str(&format!(
            "{indent}<ExecuteAfterWriteDataHistoryVersionProcessing>{}</ExecuteAfterWriteDataHistoryVersionProcessing>\r\n",
            xml_bool(execute_after_write)
        ));
    }
}

fn push_metadata_child_choice_parameter_links_xml(
    xml: &mut String,
    indent: &str,
    links: &Option<Vec<MetadataChoiceParameterLink>>,
) {
    let Some(links) = links else {
        return;
    };
    if links.is_empty() {
        xml.push_str(&format!("{indent}<ChoiceParameterLinks/>\r\n"));
        return;
    }
    xml.push_str(&format!("{indent}<ChoiceParameterLinks>\r\n"));
    for link in links {
        xml.push_str(&format!(
            "{indent}\t<xr:Link>\r\n\
{indent}\t\t<xr:Name>{}</xr:Name>\r\n\
{indent}\t\t<xr:DataPath xsi:type=\"xs:string\">{}</xr:DataPath>\r\n\
{indent}\t\t<xr:ValueChange>{}</xr:ValueChange>\r\n\
{indent}\t</xr:Link>\r\n",
            escape_xml_element_text(&link.name),
            escape_xml_element_text(&link.data_path),
            link.value_change
        ));
    }
    xml.push_str(&format!("{indent}</ChoiceParameterLinks>\r\n"));
}

#[cfg(test)]
fn push_metadata_child_choice_parameters_xml(
    xml: &mut String,
    indent: &str,
    parameters: &[MetadataChoiceParameter],
) {
    push_metadata_child_choice_parameters_xml_with_style(xml, indent, parameters, false);
}

fn push_metadata_child_choice_parameters_xml_with_style(
    xml: &mut String,
    indent: &str,
    parameters: &[MetadataChoiceParameter],
    self_close_empty_refs: bool,
) {
    if parameters.is_empty() {
        xml.push_str(&format!("{indent}<ChoiceParameters/>\r\n"));
        return;
    }

    xml.push_str(&format!("{indent}<ChoiceParameters>\r\n"));
    for parameter in parameters {
        xml.push_str(&format!(
            "{indent}\t<app:item name=\"{}\">\r\n",
            escape_xml_text(&parameter.name)
        ));
        push_metadata_choice_parameter_value_xml(
            xml,
            &format!("{indent}\t\t"),
            "app:value",
            &parameter.value,
            self_close_empty_refs,
        );
        xml.push_str(&format!("{indent}\t</app:item>\r\n"));
    }
    xml.push_str(&format!("{indent}</ChoiceParameters>\r\n"));
}

fn push_metadata_choice_parameter_value_xml(
    xml: &mut String,
    indent: &str,
    tag: &str,
    value: &MetadataChoiceParameterValue,
    self_close_empty_refs: bool,
) {
    match value {
        MetadataChoiceParameterValue::Nil => {
            xml.push_str(&format!("{indent}<{tag} xsi:nil=\"true\"/>\r\n"));
        }
        MetadataChoiceParameterValue::Boolean(value) => {
            xml.push_str(&format!(
                "{indent}<{tag} xsi:type=\"xs:boolean\">{}</{tag}>\r\n",
                xml_bool(*value)
            ));
        }
        MetadataChoiceParameterValue::Decimal(value) => {
            xml.push_str(&format!(
                "{indent}<{tag} xsi:type=\"xs:decimal\">{}</{tag}>\r\n",
                escape_xml_element_text(value)
            ));
        }
        MetadataChoiceParameterValue::DateTime(value) => {
            xml.push_str(&format!(
                "{indent}<{tag} xsi:type=\"xs:dateTime\">{}</{tag}>\r\n",
                escape_xml_element_text(value)
            ));
        }
        MetadataChoiceParameterValue::String(value) => {
            if value.is_empty() {
                xml.push_str(&format!("{indent}<{tag} xsi:type=\"xs:string\"/>\r\n"));
            } else {
                xml.push_str(&format!(
                    "{indent}<{tag} xsi:type=\"xs:string\">{}</{tag}>\r\n",
                    escape_xml_element_text(value)
                ));
            }
        }
        MetadataChoiceParameterValue::DesignTimeRef(value_ref) => {
            if value_ref.is_empty() {
                if tag == "v8:Value" && !self_close_empty_refs {
                    xml.push_str(&format!(
                        "{indent}<{tag} xsi:type=\"xr:DesignTimeRef\"></{tag}>\r\n"
                    ));
                } else {
                    xml.push_str(&format!(
                        "{indent}<{tag} xsi:type=\"xr:DesignTimeRef\"/>\r\n"
                    ));
                }
            } else {
                xml.push_str(&format!(
                    "{indent}<{tag} xsi:type=\"xr:DesignTimeRef\">{}</{tag}>\r\n",
                    escape_xml_element_text(value_ref)
                ));
            }
        }
        MetadataChoiceParameterValue::FixedArray(values) => {
            if values.is_empty() {
                xml.push_str(&format!("{indent}<{tag} xsi:type=\"v8:FixedArray\"/>\r\n"));
            } else {
                xml.push_str(&format!("{indent}<{tag} xsi:type=\"v8:FixedArray\">\r\n"));
                let nested_indent = format!("{indent}\t");
                for value in values {
                    push_metadata_choice_parameter_value_xml(
                        xml,
                        &nested_indent,
                        "v8:Value",
                        value,
                        self_close_empty_refs,
                    );
                }
                xml.push_str(&format!("{indent}</{tag}>\r\n"));
            }
        }
    }
}

fn push_metadata_tabular_section_properties_xml(
    xml: &mut String,
    indent: &str,
    properties: &MetadataTabularSectionProperties,
) {
    push_localized_property(xml, indent, "ToolTip", &properties.tooltip);
    xml.push_str(&format!(
        "{indent}<FillChecking>{}</FillChecking>\r\n",
        properties.fill_checking
    ));
    push_metadata_line_number_standard_attribute_xml(
        xml,
        indent,
        properties.line_number_fill_checking,
    );
    if let Some(use_mode) = properties.use_mode {
        xml.push_str(&format!("{indent}<Use>{use_mode}</Use>\r\n"));
    }
    if let Some(line_number_length) = properties.line_number_length {
        xml.push_str(&format!(
            "{indent}<LineNumberLength>{line_number_length}</LineNumberLength>\r\n"
        ));
    }
}

fn push_metadata_line_number_standard_attribute_xml(
    xml: &mut String,
    indent: &str,
    fill_checking: &str,
) {
    xml.push_str(&format!(
        "{indent}<StandardAttributes>\r\n\
{indent}\t<xr:StandardAttribute name=\"LineNumber\">\r\n\
{indent}\t\t<xr:LinkByType/>\r\n\
{indent}\t\t<xr:FillChecking>{fill_checking}</xr:FillChecking>\r\n\
{indent}\t\t<xr:MultiLine>false</xr:MultiLine>\r\n\
{indent}\t\t<xr:FillFromFillingValue>false</xr:FillFromFillingValue>\r\n\
{indent}\t\t<xr:CreateOnInput>Auto</xr:CreateOnInput>\r\n\
{indent}\t\t<xr:TypeReductionMode>TransformValues</xr:TypeReductionMode>\r\n\
{indent}\t\t<xr:MaxValue xsi:nil=\"true\"/>\r\n\
{indent}\t\t<xr:ToolTip/>\r\n\
{indent}\t\t<xr:ExtendedEdit>false</xr:ExtendedEdit>\r\n\
{indent}\t\t<xr:Format/>\r\n\
{indent}\t\t<xr:ChoiceForm/>\r\n\
{indent}\t\t<xr:QuickChoice>Auto</xr:QuickChoice>\r\n\
{indent}\t\t<xr:ChoiceHistoryOnInput>Auto</xr:ChoiceHistoryOnInput>\r\n\
{indent}\t\t<xr:EditFormat/>\r\n\
{indent}\t\t<xr:PasswordMode>false</xr:PasswordMode>\r\n\
{indent}\t\t<xr:DataHistory>Use</xr:DataHistory>\r\n\
{indent}\t\t<xr:MarkNegatives>false</xr:MarkNegatives>\r\n\
{indent}\t\t<xr:MinValue xsi:nil=\"true\"/>\r\n\
{indent}\t\t<xr:Synonym/>\r\n\
{indent}\t\t<xr:Comment/>\r\n\
{indent}\t\t<xr:FullTextSearch>Use</xr:FullTextSearch>\r\n\
{indent}\t\t<xr:ChoiceParameterLinks/>\r\n\
{indent}\t\t<xr:FillValue xsi:nil=\"true\"/>\r\n\
{indent}\t\t<xr:Mask/>\r\n\
{indent}\t\t<xr:ChoiceParameters/>\r\n\
{indent}\t</xr:StandardAttribute>\r\n\
{indent}</StandardAttributes>\r\n"
    ));
}

fn format_metadata_child_fill_value_xml(value: &MetadataChildFillValue) -> String {
    match value {
        MetadataChildFillValue::Nil => "<FillValue xsi:nil=\"true\"/>".to_string(),
        MetadataChildFillValue::Boolean(value) => {
            format!(
                "<FillValue xsi:type=\"xs:boolean\">{}</FillValue>",
                xml_bool(*value)
            )
        }
        MetadataChildFillValue::Decimal(value) => format!(
            "<FillValue xsi:type=\"xs:decimal\">{}</FillValue>",
            escape_xml_element_text(value)
        ),
        MetadataChildFillValue::DateTime(value) => format!(
            "<FillValue xsi:type=\"xs:dateTime\">{}</FillValue>",
            escape_xml_element_text(value)
        ),
        MetadataChildFillValue::DesignTimeRef(value) if value.is_empty() => {
            "<FillValue xsi:type=\"xr:DesignTimeRef\"/>".to_string()
        }
        MetadataChildFillValue::DesignTimeRef(value) => format!(
            "<FillValue xsi:type=\"xr:DesignTimeRef\">{}</FillValue>",
            escape_xml_element_text(value)
        ),
        MetadataChildFillValue::String(value) if value.is_empty() => {
            "<FillValue xsi:type=\"xs:string\"/>".to_string()
        }
        MetadataChildFillValue::String(value) => format!(
            "<FillValue xsi:type=\"xs:string\">{}</FillValue>",
            escape_xml_element_text(value)
        ),
    }
}

fn format_common_command_source_xml_native(
    header: &MetadataHeader,
    properties: &CommonCommandProperties,
    source_version: InfobaseConfigSourceVersion,
) -> String {
    let mut xml = format_full_metadata_source_xml("CommonCommand", header, source_version);
    let mut insert = format!(
        "\t\t\t<Group>{}</Group>\r\n\
\t\t\t<Representation>{}</Representation>\r\n",
        escape_xml_text(properties.group.as_deref().unwrap_or("ActionsPanelTools")),
        properties.representation
    );
    push_common_command_tooltip_xml(&mut insert, &properties.tooltip);
    format_common_command_picture_xml(&mut insert, properties);
    if let Some(shortcut) = properties.shortcut.as_deref() {
        insert.push_str(&format!(
            "\t\t\t<Shortcut>{}</Shortcut>\r\n",
            escape_xml_element_text(shortcut)
        ));
    } else {
        insert.push_str("\t\t\t<Shortcut/>\r\n");
    }
    insert.push_str(&format!(
        "\t\t\t<IncludeHelpInContents>{}</IncludeHelpInContents>\r\n",
        xml_bool(properties.include_help_in_contents)
    ));
    format_common_command_parameter_type_xml(&mut insert, &properties.command_parameter_types);
    insert.push_str(&format!(
        "\t\t\t<ParameterUseMode>{}</ParameterUseMode>\r\n\
\t\t\t<ModifiesData>{}</ModifiesData>\r\n\
\t\t\t<OnMainServerUnavalableBehavior>{}</OnMainServerUnavalableBehavior>\r\n",
        properties.parameter_use_mode,
        xml_bool(properties.modifies_data),
        properties.on_main_server_unavailable_behavior,
    ));
    if let Some(index) = xml.find("\t\t</Properties>\r\n") {
        xml.insert_str(index, &insert);
    }
    xml
}

fn push_common_command_tooltip_xml(xml: &mut String, values: &[(String, String)]) {
    push_common_command_tooltip_xml_with_indent(xml, "\t\t\t", values);
}

fn push_common_command_tooltip_xml_with_indent(
    xml: &mut String,
    indent: &str,
    values: &[(String, String)],
) {
    if values.is_empty() {
        xml.push_str(&format!("{indent}<ToolTip/>\r\n"));
        return;
    }
    xml.push_str(&format!("{indent}<ToolTip>\r\n"));
    for (lang, content) in values {
        let content = content.replace("\r\n", "\n").replace('\r', "\n");
        xml.push_str(&format!("{indent}\t<v8:item>\r\n"));
        xml.push_str(&format!(
            "{indent}\t\t<v8:lang>{}</v8:lang>\r\n",
            escape_xml_text(lang)
        ));
        xml.push_str(&format!(
            "{indent}\t\t<v8:content>{}</v8:content>\r\n",
            escape_xml_element_text(&content)
        ));
        xml.push_str(&format!("{indent}\t</v8:item>\r\n"));
    }
    xml.push_str(&format!("{indent}</ToolTip>\r\n"));
}

fn format_common_command_picture_xml(xml: &mut String, properties: &CommonCommandProperties) {
    format_common_command_picture_xml_with_indent(xml, "\t\t\t", properties);
}

fn format_common_command_picture_xml_with_indent(
    xml: &mut String,
    indent: &str,
    properties: &CommonCommandProperties,
) {
    let Some(reference) = properties.picture_ref.as_deref() else {
        xml.push_str(&format!("{indent}<Picture/>\r\n"));
        return;
    };
    xml.push_str(&format!("{indent}<Picture>\r\n"));
    xml.push_str(&format!(
        "{indent}\t<xr:Ref>{}</xr:Ref>\r\n",
        escape_xml_text(reference)
    ));
    xml.push_str(&format!(
        "{indent}\t<xr:LoadTransparent>{}</xr:LoadTransparent>\r\n",
        xml_bool(properties.picture_load_transparent)
    ));
    xml.push_str(&format!("{indent}</Picture>\r\n"));
}

fn format_common_command_parameter_type_xml(xml: &mut String, types: &[ConstantValueType]) {
    format_common_command_parameter_type_xml_with_indent(xml, types, "\t\t\t");
}

fn format_common_command_parameter_type_xml_with_indent(
    xml: &mut String,
    types: &[ConstantValueType],
    indent: &str,
) {
    if types.is_empty() {
        xml.push_str(&format!("{indent}<CommandParameterType/>\r\n"));
        return;
    }
    xml.push_str(&format!("{indent}<CommandParameterType>\r\n"));
    for value_type in types {
        let tag_name = if matches!(value_type, ConstantValueType::ReferenceTypeSet { .. }) {
            "TypeSet"
        } else {
            "Type"
        };
        xml.push_str(&format!(
            "{indent}\t<v8:{tag_name}>{}</v8:{tag_name}>\r\n",
            escape_xml_text(&metadata_type_xml_name(value_type))
        ));
    }
    xml.push_str(&format!("{indent}</CommandParameterType>\r\n"));
}

fn format_command_group_source_xml(
    header: &MetadataHeader,
    properties: &CommandGroupProperties,
    source_version: InfobaseConfigSourceVersion,
) -> String {
    let mut xml = format_full_metadata_source_xml("CommandGroup", header, source_version);
    let mut insert = format!(
        "\t\t\t<Representation>{}</Representation>\r\n",
        properties.representation
    );
    if properties.tooltip.is_empty() {
        insert.push_str("\t\t\t<ToolTip/>\r\n");
    } else {
        insert.push_str("\t\t\t<ToolTip>\r\n");
        for (lang, content) in &properties.tooltip {
            insert.push_str("\t\t\t\t<v8:item>\r\n");
            insert.push_str(&format!(
                "\t\t\t\t\t<v8:lang>{}</v8:lang>\r\n",
                escape_xml_element_text(lang)
            ));
            insert.push_str(&format!(
                "\t\t\t\t\t<v8:content>{}</v8:content>\r\n",
                escape_xml_element_text(content)
            ));
            insert.push_str("\t\t\t\t</v8:item>\r\n");
        }
        insert.push_str("\t\t\t</ToolTip>\r\n");
    }
    match &properties.picture_ref {
        Some(reference) => {
            insert.push_str("\t\t\t<Picture>\r\n");
            insert.push_str(&format!(
                "\t\t\t\t<xr:Ref>{}</xr:Ref>\r\n",
                escape_xml_text(reference)
            ));
            insert.push_str(&format!(
                "\t\t\t\t<xr:LoadTransparent>{}</xr:LoadTransparent>\r\n\
\t\t\t</Picture>\r\n",
                xml_bool(properties.picture_load_transparent)
            ));
        }
        None => insert.push_str("\t\t\t<Picture/>\r\n"),
    }
    insert.push_str(&format!(
        "\t\t\t<Category>{}</Category>\r\n",
        properties.category
    ));
    xml = xml.replace("\t\t</Properties>", &format!("{insert}\t\t</Properties>"));
    xml
}

fn format_style_item_source_xml(
    header: &MetadataHeader,
    properties: &StyleItemProperties,
) -> String {
    let mut xml = format!(
        "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:app=\"http://v8.1c.ru/8.2/managed-application/core\" xmlns:cfg=\"http://v8.1c.ru/8.1/data/enterprise/current-config\" xmlns:cmi=\"http://v8.1c.ru/8.2/managed-application/cmi\" xmlns:ent=\"http://v8.1c.ru/8.1/data/enterprise\" xmlns:lf=\"http://v8.1c.ru/8.2/managed-application/logform\" xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\" xmlns:sys=\"http://v8.1c.ru/8.1/data/ui/fonts/system\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" xmlns:web=\"http://v8.1c.ru/8.1/data/ui/colors/web\" xmlns:win=\"http://v8.1c.ru/8.1/data/ui/colors/windows\" xmlns:xen=\"http://v8.1c.ru/8.3/xcf/enums\" xmlns:xpr=\"http://v8.1c.ru/8.3/xcf/predef\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"2.20\">\r\n\
\t<StyleItem uuid=\"{}\">\r\n\
\t\t<Properties>\r\n\
\t\t\t<Name>{}</Name>\r\n",
        escape_xml_text(&header.uuid),
        escape_xml_text(&header.name)
    );
    if header.synonyms.is_empty() {
        xml.push_str("\t\t\t<Synonym/>\r\n");
    } else {
        xml.push_str("\t\t\t<Synonym>\r\n");
        for (lang, content) in &header.synonyms {
            xml.push_str("\t\t\t\t<v8:item>\r\n");
            xml.push_str(&format!(
                "\t\t\t\t\t<v8:lang>{}</v8:lang>\r\n",
                escape_xml_element_text(lang)
            ));
            xml.push_str(&format!(
                "\t\t\t\t\t<v8:content>{}</v8:content>\r\n",
                escape_xml_element_text(content)
            ));
            xml.push_str("\t\t\t\t</v8:item>\r\n");
        }
        xml.push_str("\t\t\t</Synonym>\r\n");
    }
    if header.comment.is_empty() {
        xml.push_str("\t\t\t<Comment/>\r\n");
    } else {
        xml.push_str(&format!(
            "\t\t\t<Comment>{}</Comment>\r\n",
            escape_xml_element_text(&header.comment)
        ));
    }
    xml.push_str(&format!(
        "\t\t\t<Type>{}</Type>\r\n\
\t\t\t{}\r\n\
\t\t</Properties>\r\n\
\t</StyleItem>\r\n\
</MetaDataObject>",
        properties.item_type, properties.value_xml
    ));
    xml
}

fn format_common_module_source_xml(
    header: &MetadataHeader,
    flags: &CommonModuleFlags,
    source_version: InfobaseConfigSourceVersion,
) -> String {
    let mut xml = format!(
        "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:app=\"http://v8.1c.ru/8.2/managed-application/core\" xmlns:cfg=\"http://v8.1c.ru/8.1/data/enterprise/current-config\" xmlns:cmi=\"http://v8.1c.ru/8.2/managed-application/cmi\" xmlns:ent=\"http://v8.1c.ru/8.1/data/enterprise\" xmlns:lf=\"http://v8.1c.ru/8.2/managed-application/logform\" xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\" xmlns:sys=\"http://v8.1c.ru/8.1/data/ui/fonts/system\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" xmlns:web=\"http://v8.1c.ru/8.1/data/ui/colors/web\" xmlns:win=\"http://v8.1c.ru/8.1/data/ui/colors/windows\" xmlns:xen=\"http://v8.1c.ru/8.3/xcf/enums\" xmlns:xpr=\"http://v8.1c.ru/8.3/xcf/predef\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"{}\">\r\n\
\t<CommonModule uuid=\"{}\">\r\n\
\t\t<Properties>\r\n\
\t\t\t<Name>{}</Name>\r\n",
        source_version.as_str(),
        escape_xml_text(&header.uuid),
        escape_xml_element_text(&header.name)
    );
    if header.synonyms.is_empty() {
        xml.push_str("\t\t\t<Synonym/>\r\n");
    } else {
        xml.push_str("\t\t\t<Synonym>\r\n");
        for (lang, content) in &header.synonyms {
            xml.push_str("\t\t\t\t<v8:item>\r\n");
            xml.push_str(&format!(
                "\t\t\t\t\t<v8:lang>{}</v8:lang>\r\n",
                escape_xml_element_text(lang)
            ));
            xml.push_str(&format!(
                "\t\t\t\t\t<v8:content>{}</v8:content>\r\n",
                escape_xml_element_text(content)
            ));
            xml.push_str("\t\t\t\t</v8:item>\r\n");
        }
        xml.push_str("\t\t\t</Synonym>\r\n");
    }
    if header.comment.is_empty() {
        xml.push_str("\t\t\t<Comment/>\r\n");
    } else {
        xml.push_str(&format!(
            "\t\t\t<Comment>{}</Comment>\r\n",
            escape_xml_element_text(&header.comment)
        ));
    }
    xml.push_str(&format!(
        "\t\t\t<Global>{}</Global>\r\n\
\t\t\t<ClientManagedApplication>{}</ClientManagedApplication>\r\n\
\t\t\t<Server>{}</Server>\r\n\
\t\t\t<ExternalConnection>{}</ExternalConnection>\r\n\
\t\t\t<ClientOrdinaryApplication>{}</ClientOrdinaryApplication>\r\n\
\t\t\t<ServerCall>{}</ServerCall>\r\n\
\t\t\t<Privileged>{}</Privileged>\r\n\
\t\t\t<ReturnValuesReuse>{}</ReturnValuesReuse>\r\n\
\t\t</Properties>\r\n\
\t</CommonModule>\r\n\
</MetaDataObject>",
        xml_bool(flags.global),
        xml_bool(flags.client_managed_application),
        xml_bool(flags.server),
        xml_bool(flags.external_connection),
        xml_bool(flags.client_ordinary_application),
        xml_bool(flags.server_call),
        xml_bool(flags.privileged),
        return_values_reuse_xml(flags.return_values_reuse),
    ));
    xml
}

fn format_scheduled_job_source_xml(
    header: &MetadataHeader,
    scheduled_job: &ScheduledJobProperties,
    source_version: InfobaseConfigSourceVersion,
) -> String {
    let mut xml = format_full_metadata_source_xml("ScheduledJob", header, source_version);
    let insert = format!(
        "\t\t\t<MethodName>{}</MethodName>\r\n\
\t\t\t{}\r\n\
\t\t\t{}\r\n\
\t\t\t<Use>{}</Use>\r\n\
\t\t\t<Predefined>{}</Predefined>\r\n\
\t\t\t<RestartCountOnFailure>{}</RestartCountOnFailure>\r\n\
\t\t\t<RestartIntervalOnFailure>{}</RestartIntervalOnFailure>\r\n",
        escape_xml_element_text(&scheduled_job.method_name),
        format_simple_property_xml("Description", &scheduled_job.description),
        format_simple_property_xml("Key", &scheduled_job.key),
        xml_bool(scheduled_job.use_job),
        xml_bool(scheduled_job.predefined),
        scheduled_job.restart_count_on_failure,
        scheduled_job.restart_interval_on_failure
    );
    let marker = "\t\t</Properties>\r\n";
    if let Some(index) = xml.find(marker) {
        xml.insert_str(index, &insert);
    }
    xml
}

fn format_event_subscription_source_xml(
    header: &MetadataHeader,
    event_subscription: &EventSubscriptionProperties,
    source_version: InfobaseConfigSourceVersion,
) -> String {
    let mut xml = format_full_metadata_source_xml("EventSubscription", header, source_version);
    let mut insert = String::from("\t\t\t<Source>\r\n");
    let source_types = sorted_event_subscription_source_types(&event_subscription.source_types);
    for source_type in source_types {
        let reference = metadata_type_xml_name(source_type);
        let tag = event_subscription_source_type_tag(&reference);
        insert.push_str(&format!(
            "\t\t\t\t<v8:{tag}>{}</v8:{tag}>\r\n",
            escape_xml_element_text(&reference)
        ));
    }
    insert.push_str("\t\t\t</Source>\r\n");
    insert.push_str(&format!(
        "\t\t\t<Event>{}</Event>\r\n\
\t\t\t<Handler>{}</Handler>\r\n",
        escape_xml_element_text(&event_subscription.event),
        escape_xml_element_text(&event_subscription.handler)
    ));
    let marker = "\t\t</Properties>\r\n";
    if let Some(index) = xml.find(marker) {
        xml.insert_str(index, &insert);
    }
    xml
}

fn sorted_event_subscription_source_types(
    source_types: &[ConstantValueType],
) -> Vec<&ConstantValueType> {
    let mut indexed = source_types.iter().enumerate().collect::<Vec<_>>();
    let all_type_sets = indexed.iter().all(|(_, source_type)| {
        event_subscription_source_type_tag(&metadata_type_xml_name(source_type)) == "TypeSet"
    });
    indexed.sort_by_key(|(index, source_type)| {
        let reference = metadata_type_xml_name(source_type);
        let tag = event_subscription_source_type_tag(&reference);
        let tag_rank = if tag == "Type" { 0usize } else { 1usize };
        let type_set_rank = if all_type_sets {
            event_subscription_type_set_order(&reference).unwrap_or(usize::MAX)
        } else {
            usize::MAX
        };
        (tag_rank, type_set_rank, *index)
    });
    indexed
        .into_iter()
        .map(|(_, source_type)| source_type)
        .collect()
}

fn event_subscription_type_set_order(reference: &str) -> Option<usize> {
    match reference {
        "cfg:BusinessProcessObject" => Some(10),
        "cfg:ChartOfCalculationTypesObject" => Some(20),
        "cfg:ChartOfAccountsObject" => Some(30),
        "cfg:ChartOfCharacteristicTypesObject" => Some(40),
        "cfg:ConstantValueManager" => Some(50),
        "cfg:ExchangePlanObject" => Some(60),
        "cfg:CatalogObject" => Some(70),
        "cfg:TaskObject" => Some(80),
        "cfg:DocumentObject" => Some(90),
        "cfg:InformationRegisterRecordSet" => Some(200),
        "cfg:AccountingRegisterRecordSet" => Some(210),
        "cfg:AccumulationRegisterRecordSet" => Some(220),
        "cfg:SequenceRecordSet" => Some(230),
        "cfg:RecalculationRecordSet" => Some(240),
        "cfg:CalculationRegisterRecordSet" => Some(250),
        _ => None,
    }
}

fn event_subscription_source_type_tag(reference: &str) -> &'static str {
    if reference.starts_with("cfg:DefinedType.")
        || (reference.starts_with("cfg:")
            && !reference.contains('.')
            && !reference.ends_with("Manager"))
        || reference == "cfg:ConstantValueManager"
    {
        "TypeSet"
    } else {
        "Type"
    }
}

fn format_simple_property_xml(name: &str, value: &str) -> String {
    if value.is_empty() {
        format!("<{name}/>")
    } else {
        format!("<{name}>{}</{name}>", escape_xml_element_text(value))
    }
}

fn format_constant_bound_xml(name: &str, value: Option<&str>) -> String {
    match value {
        Some(value) => format!(
            "<{name} xsi:type=\"xs:string\">{}</{name}>",
            escape_xml_element_text(value)
        ),
        None => format!("<{name} xsi:nil=\"true\"/>"),
    }
}

fn format_choice_parameters_xml(parameters: &[ChoiceParameter]) -> String {
    if parameters.is_empty() {
        return "<ChoiceParameters/>".to_string();
    }
    let mut xml = "<ChoiceParameters>\r\n".to_string();
    for parameter in parameters {
        xml.push_str(&format!(
            "\t\t\t\t<app:item name=\"{}\">\r\n\
\t\t\t\t\t<app:value xsi:type=\"xr:DesignTimeRef\">{}</app:value>\r\n\
\t\t\t\t</app:item>\r\n",
            escape_xml_text(&parameter.name),
            escape_xml_element_text(&parameter.value_ref)
        ));
    }
    xml.push_str("\t\t\t</ChoiceParameters>");
    xml
}

fn format_constant_source_xml(
    header: &MetadataHeader,
    constant: &ConstantProperties,
    source_version: InfobaseConfigSourceVersion,
) -> String {
    let mut xml = format_full_metadata_source_xml("Constant", header, source_version);
    if !constant.generated_types.is_empty() {
        let mut internal = "\t\t<InternalInfo>\r\n".to_string();
        for generated_type in &constant.generated_types {
            internal.push_str(&format!(
                "\t\t\t<xr:GeneratedType name=\"{}\" category=\"{}\">\r\n\
\t\t\t\t<xr:TypeId>{}</xr:TypeId>\r\n\
\t\t\t\t<xr:ValueId>{}</xr:ValueId>\r\n\
\t\t\t</xr:GeneratedType>\r\n",
                escape_xml_text(&generated_type.name),
                escape_xml_text(generated_type.category),
                escape_xml_text(&generated_type.type_id),
                escape_xml_text(&generated_type.value_id)
            ));
        }
        internal.push_str("\t\t</InternalInfo>\r\n");
        if let Some(index) = xml.find("\t\t<Properties>\r\n") {
            xml.insert_str(index, &internal);
        }
    }
    let mut insert = format!(
        "{}\
\t\t\t<UseStandardCommands>{}</UseStandardCommands>\r\n",
        format_constant_type_xml(&constant.value_type),
        xml_bool(constant.use_standard_commands),
    );
    match &constant.default_form {
        Some(default_form) => insert.push_str(&format!(
            "\t\t\t<DefaultForm>{}</DefaultForm>\r\n",
            escape_xml_element_text(default_form)
        )),
        None => insert.push_str("\t\t\t<DefaultForm/>\r\n"),
    }
    push_localized_property(
        &mut insert,
        "\t\t\t",
        "ExtendedPresentation",
        &constant.extended_presentation,
    );
    push_localized_property(&mut insert, "\t\t\t", "Explanation", &constant.explanation);
    insert.push_str(&format!(
        "\t\t\t<PasswordMode>{}</PasswordMode>\r\n",
        xml_bool(constant.password_mode)
    ));
    push_localized_property(&mut insert, "\t\t\t", "Format", &constant.format);
    push_localized_property(&mut insert, "\t\t\t", "EditFormat", &constant.edit_format);
    push_localized_property(&mut insert, "\t\t\t", "ToolTip", &constant.tooltip);
    insert.push_str(&format!(
        "\t\t\t<MarkNegatives>false</MarkNegatives>\r\n\
\t\t\t{}\r\n\
\t\t\t<MultiLine>false</MultiLine>\r\n\
\t\t\t<ExtendedEdit>false</ExtendedEdit>\r\n\
\t\t\t{}\r\n\
\t\t\t{}\r\n\
\t\t\t<FillChecking>{}</FillChecking>\r\n\
\t\t\t<ChoiceFoldersAndItems>Items</ChoiceFoldersAndItems>\r\n\
\t\t\t<ChoiceParameterLinks/>\r\n\
\t\t\t{}\r\n\
\t\t\t<QuickChoice>Auto</QuickChoice>\r\n\
\t\t\t<ChoiceForm/>\r\n\
\t\t\t<LinkByType/>\r\n\
\t\t\t<ChoiceHistoryOnInput>{}</ChoiceHistoryOnInput>\r\n\
\t\t\t<DataLockControlMode>{}</DataLockControlMode>\r\n\
\t\t\t<DataHistory>DontUse</DataHistory>\r\n\
\t\t\t<UpdateDataHistoryImmediatelyAfterWrite>false</UpdateDataHistoryImmediatelyAfterWrite>\r\n\
\t\t\t<ExecuteAfterWriteDataHistoryVersionProcessing>false</ExecuteAfterWriteDataHistoryVersionProcessing>\r\n",
        format_simple_property_xml("Mask", &constant.mask),
        format_constant_bound_xml("MinValue", constant.min_value.as_deref()),
        format_constant_bound_xml("MaxValue", constant.max_value.as_deref()),
        constant.fill_checking,
        format_choice_parameters_xml(&constant.choice_parameters),
        constant.choice_history_on_input,
        constant.data_lock_control_mode
    ));
    let marker = "\t\t</Properties>\r\n";
    if let Some(index) = xml.find(marker) {
        xml.insert_str(index, &insert);
    }
    xml
}

fn format_defined_type_source_xml(
    header: &MetadataHeader,
    defined_type: &DefinedTypeProperties,
    source_version: InfobaseConfigSourceVersion,
) -> String {
    let mut xml = format_full_metadata_source_xml("DefinedType", header, source_version);
    if !defined_type.generated_types.is_empty() {
        let mut internal = "\t\t<InternalInfo>\r\n".to_string();
        for generated_type in &defined_type.generated_types {
            internal.push_str(&format!(
                "\t\t\t<xr:GeneratedType name=\"{}\" category=\"{}\">\r\n\
\t\t\t\t<xr:TypeId>{}</xr:TypeId>\r\n\
\t\t\t\t<xr:ValueId>{}</xr:ValueId>\r\n\
\t\t\t</xr:GeneratedType>\r\n",
                escape_xml_text(&generated_type.name),
                escape_xml_text(generated_type.category),
                escape_xml_text(&generated_type.type_id),
                escape_xml_text(&generated_type.value_id)
            ));
        }
        internal.push_str("\t\t</InternalInfo>\r\n");
        if let Some(index) = xml.find("\t\t<Properties>\r\n") {
            xml.insert_str(index, &internal);
        }
    }
    let insert = format_metadata_types_xml(&defined_type.value_types);
    let marker = "\t\t</Properties>\r\n";
    if let Some(index) = xml.find(marker) {
        xml.insert_str(index, &insert);
    }
    xml
}

fn format_typed_metadata_source_xml(
    kind: &str,
    header: &MetadataHeader,
    typed: &TypedMetadataProperties,
    source_version: InfobaseConfigSourceVersion,
) -> String {
    let mut xml = format_full_metadata_source_xml(kind, header, source_version);
    let insert = format_metadata_types_xml(&typed.value_types);
    let marker = "\t\t</Properties>\r\n";
    if let Some(index) = xml.find(marker) {
        xml.insert_str(index, &insert);
    }
    xml
}

fn format_common_attribute_source_xml(
    header: &MetadataHeader,
    common_attribute: &CommonAttributeProperties,
    source_version: InfobaseConfigSourceVersion,
) -> String {
    let typed = TypedMetadataProperties {
        value_types: common_attribute.value_types.clone(),
    };
    let mut xml =
        format_typed_metadata_source_xml("CommonAttribute", header, &typed, source_version);
    if let Some(details) = &common_attribute.property_details {
        insert_metadata_properties_xml(
            &mut xml,
            &format_common_attribute_property_details_xml(details),
        );
    }
    let mut insert = String::new();
    if common_attribute.content.is_empty() {
        insert.push_str("\t\t\t<Content/>\r\n");
    } else {
        insert.push_str("\t\t\t<Content>\r\n");
        for item in &common_attribute.content {
            insert.push_str(&format!(
                "\t\t\t\t<xr:Item>\r\n\
\t\t\t\t\t<xr:Metadata>{}</xr:Metadata>\r\n\
\t\t\t\t\t<xr:Use>{}</xr:Use>\r\n",
                escape_xml_element_text(&item.metadata),
                escape_xml_text(item.use_mode)
            ));
            if let Some(conditional_separation) = &item.conditional_separation {
                insert.push_str(&format!(
                    "\t\t\t\t\t<xr:ConditionalSeparation>{}</xr:ConditionalSeparation>\r\n",
                    escape_xml_element_text(conditional_separation)
                ));
            } else {
                insert.push_str("\t\t\t\t\t<xr:ConditionalSeparation/>\r\n");
            }
            insert.push_str("\t\t\t\t</xr:Item>\r\n");
        }
        insert.push_str("\t\t\t</Content>\r\n");
    }
    insert_metadata_properties_xml(&mut xml, &insert);
    insert_metadata_properties_xml(
        &mut xml,
        &format!(
            "\t\t\t<AutoUse>{}</AutoUse>\r\n",
            escape_xml_text(common_attribute.auto_use)
        ),
    );
    if let Some(separation) = &common_attribute.separation {
        insert_metadata_properties_xml(
            &mut xml,
            &format_common_attribute_separation_xml(separation),
        );
    }
    xml
}

fn format_common_attribute_property_details_xml(
    details: &CommonAttributePropertyDetails,
) -> String {
    let mut xml = "\t\t\t<PasswordMode>false</PasswordMode>\r\n\
\t\t\t<Format/>\r\n\
\t\t\t<EditFormat/>\r\n\
\t\t\t<ToolTip/>\r\n\
\t\t\t<MarkNegatives>false</MarkNegatives>\r\n\
\t\t\t<Mask/>\r\n\
\t\t\t<MultiLine>false</MultiLine>\r\n\
\t\t\t<ExtendedEdit>false</ExtendedEdit>\r\n\
\t\t\t<MinValue xsi:nil=\"true\"/>\r\n\
\t\t\t<MaxValue xsi:nil=\"true\"/>\r\n\
\t\t\t<FillFromFillingValue>false</FillFromFillingValue>\r\n\
\t\t\t"
        .to_string();
    if let Some(fill_value) = &details.fill_value {
        xml.push_str(&format_common_attribute_fill_value_xml(fill_value));
        xml.push_str("\r\n\t\t\t");
    }
    xml.push_str(&format!(
        "<FillChecking>{}</FillChecking>\r\n\
\t\t\t<ChoiceFoldersAndItems>Items</ChoiceFoldersAndItems>\r\n\
\t\t\t<ChoiceParameterLinks/>\r\n\
\t\t\t<ChoiceParameters/>\r\n\
\t\t\t<QuickChoice>Auto</QuickChoice>\r\n\
\t\t\t<CreateOnInput>Auto</CreateOnInput>\r\n\
\t\t\t<ChoiceForm/>\r\n\
\t\t\t<LinkByType/>\r\n\
\t\t\t<ChoiceHistoryOnInput>Auto</ChoiceHistoryOnInput>\r\n",
        details.fill_checking
    ));
    xml
}

fn format_common_attribute_fill_value_xml(value: &CommonAttributeFillValue) -> String {
    match value {
        CommonAttributeFillValue::Nil => "<FillValue xsi:nil=\"true\"/>".to_string(),
        CommonAttributeFillValue::Boolean(value) => format!(
            "<FillValue xsi:type=\"xs:boolean\">{}</FillValue>",
            xml_bool(*value)
        ),
        CommonAttributeFillValue::Decimal(value) => format!(
            "<FillValue xsi:type=\"xs:decimal\">{}</FillValue>",
            escape_xml_element_text(value)
        ),
        CommonAttributeFillValue::String(value) if value.is_empty() => {
            "<FillValue xsi:type=\"xs:string\"/>".to_string()
        }
        CommonAttributeFillValue::String(value) => format!(
            "<FillValue xsi:type=\"xs:string\">{}</FillValue>",
            escape_xml_element_text(value)
        ),
    }
}

fn format_common_attribute_separation_xml(
    properties: &CommonAttributeSeparationProperties,
) -> String {
    format!(
        "\t\t\t<DataSeparation>{}</DataSeparation>\r\n\
\t\t\t<SeparatedDataUse>{}</SeparatedDataUse>\r\n\
\t\t\t{}\r\n\
\t\t\t{}\r\n\
\t\t\t{}\r\n\
\t\t\t<UsersSeparation>{}</UsersSeparation>\r\n\
\t\t\t<AuthenticationSeparation>{}</AuthenticationSeparation>\r\n\
\t\t\t<ConfigurationExtensionsSeparation>{}</ConfigurationExtensionsSeparation>\r\n\
\t\t\t<Indexing>{}</Indexing>\r\n\
\t\t\t<FullTextSearch>{}</FullTextSearch>\r\n\
\t\t\t<DataHistory>{}</DataHistory>\r\n",
        escape_xml_text(properties.data_separation),
        escape_xml_text(properties.separated_data_use),
        format_common_attribute_optional_ref_xml(
            "DataSeparationValue",
            properties.data_separation_value.as_deref()
        ),
        format_common_attribute_optional_ref_xml(
            "DataSeparationUse",
            properties.data_separation_use.as_deref()
        ),
        format_common_attribute_optional_ref_xml(
            "ConditionalSeparation",
            properties.conditional_separation.as_deref()
        ),
        escape_xml_text(properties.users_separation),
        escape_xml_text(properties.authentication_separation),
        escape_xml_text(properties.configuration_extensions_separation),
        escape_xml_text(properties.indexing),
        escape_xml_text(properties.full_text_search),
        escape_xml_text(properties.data_history)
    )
}

fn format_common_attribute_optional_ref_xml(name: &str, value: Option<&str>) -> String {
    match value {
        Some(value) => format!(
            "<{name}>{}</{name}>",
            escape_xml_element_text(value),
            name = name
        ),
        None => format!("<{name}/>"),
    }
}

fn format_functional_options_parameter_source_xml(
    header: &MetadataHeader,
    properties: &FunctionalOptionsParameterProperties,
    source_version: InfobaseConfigSourceVersion,
) -> String {
    let mut xml =
        format_full_metadata_source_xml("FunctionalOptionsParameter", header, source_version);
    let mut insert = String::new();
    if properties.use_refs.is_empty() {
        insert.push_str("\t\t\t<Use/>\r\n");
    } else {
        insert.push_str("\t\t\t<Use>\r\n");
        for use_ref in &properties.use_refs {
            insert.push_str(&format!(
                "\t\t\t\t<xr:Item xsi:type=\"xr:MDObjectRef\">{}</xr:Item>\r\n",
                escape_xml_element_text(use_ref)
            ));
        }
        insert.push_str("\t\t\t</Use>\r\n");
    }
    let marker = "\t\t</Properties>\r\n";
    if let Some(index) = xml.find(marker) {
        xml.insert_str(index, &insert);
    }
    xml
}

fn format_xdto_package_source_xml(
    header: &MetadataHeader,
    package: &XdtoPackageProperties,
    source_version: InfobaseConfigSourceVersion,
) -> String {
    let mut xml = format_full_metadata_source_xml("XDTOPackage", header, source_version);
    insert_metadata_properties_xml(
        &mut xml,
        &format!(
            "\t\t\t<Namespace>{}</Namespace>\r\n",
            escape_xml_element_text(&package.namespace)
        ),
    );
    xml
}

fn format_filter_criterion_source_xml(
    header: &MetadataHeader,
    source_version: InfobaseConfigSourceVersion,
) -> String {
    let mut xml = format_full_metadata_source_xml("FilterCriterion", header, source_version);
    insert_metadata_properties_xml(
        &mut xml,
        "\t\t\t<UseStandardCommands>true</UseStandardCommands>\r\n",
    );
    xml
}

fn insert_metadata_properties_xml(xml: &mut String, insert: &str) {
    let marker = "\t\t</Properties>\r\n";
    if let Some(index) = xml.find(marker) {
        xml.insert_str(index, insert);
    }
}

fn format_web_service_source_xml(
    header: &MetadataHeader,
    service: &WebServiceProperties,
    source_version: InfobaseConfigSourceVersion,
) -> String {
    let mut xml = format_full_metadata_source_xml("WebService", header, source_version);
    if source_version == InfobaseConfigSourceVersion::V2_21 {
        const STYLE_NAMESPACE_ATTRIBUTE: &str =
            " xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\"";
        const PALETTE_NAMESPACE_ATTRIBUTE: &str =
            " xmlns:pal=\"http://v8.1c.ru/8.1/data/ui/colors/palette\"";
        if let Some(style_index) = xml.find(STYLE_NAMESPACE_ATTRIBUTE) {
            xml.insert_str(style_index, PALETTE_NAMESPACE_ATTRIBUTE);
        }
    }
    let mut properties = String::new();
    push_web_service_text_property(&mut properties, "\t\t\t", "Namespace", &service.namespace);
    push_web_service_xdto_packages_xml(&mut properties, &service.xdto_packages);
    push_web_service_text_property(
        &mut properties,
        "\t\t\t",
        "DescriptorFileName",
        &service.descriptor_file_name,
    );
    properties.push_str(&format!(
        "\t\t\t<ReuseSessions>{}</ReuseSessions>\r\n\
\t\t\t<SessionMaxAge>{}</SessionMaxAge>\r\n",
        service.reuse_sessions, service.session_max_age
    ));
    insert_metadata_properties_xml(&mut xml, &properties);

    let child_objects = if service.operations.is_empty() {
        "\t\t<ChildObjects/>\r\n".to_string()
    } else {
        let mut child_objects = "\t\t<ChildObjects>\r\n".to_string();
        for operation in &service.operations {
            push_web_service_operation_xml(&mut child_objects, operation);
        }
        child_objects.push_str("\t\t</ChildObjects>\r\n");
        child_objects
    };
    if let Some(index) = xml.find("\t</WebService>\r\n") {
        xml.insert_str(index, &child_objects);
    }
    xml
}

fn push_web_service_xdto_packages_xml(xml: &mut String, packages: &[WebServiceXdtoPackage]) {
    if packages.is_empty() {
        xml.push_str("\t\t\t<XDTOPackages/>\r\n");
        return;
    }
    xml.push_str("\t\t\t<XDTOPackages>\r\n");
    for package in packages {
        let (value_type, value) = match package {
            WebServiceXdtoPackage::MetadataReference(reference) => {
                ("xr:MDObjectRef", reference.as_str())
            }
            WebServiceXdtoPackage::Namespace(namespace) => ("xs:string", namespace.as_str()),
        };
        xml.push_str(&format!(
            "\t\t\t\t<xr:Item>\r\n\
\t\t\t\t\t<xr:Presentation/>\r\n\
\t\t\t\t\t<xr:CheckState>0</xr:CheckState>\r\n\
\t\t\t\t\t<xr:Value xsi:type=\"{}\">{}</xr:Value>\r\n\
\t\t\t\t</xr:Item>\r\n",
            value_type,
            escape_xml_element_text(value)
        ));
    }
    xml.push_str("\t\t\t</XDTOPackages>\r\n");
}

fn push_web_service_operation_xml(xml: &mut String, operation: &WebServiceOperationProperties) {
    xml.push_str(&format!(
        "\t\t\t<Operation uuid=\"{}\">\r\n\
\t\t\t\t<Properties>\r\n",
        escape_xml_text(&operation.header.uuid)
    ));
    push_web_service_header_properties_xml(xml, "\t\t\t\t\t", &operation.header);
    push_web_service_xdto_type_xml(
        xml,
        "\t\t\t\t\t",
        "XDTOReturningValueType",
        &operation.returning_value_type,
        "d6p1",
    );
    xml.push_str(&format!(
        "\t\t\t\t\t<Nillable>{}</Nillable>\r\n\
\t\t\t\t\t<Transactioned>{}</Transactioned>\r\n",
        xml_bool(operation.nillable),
        xml_bool(operation.transactioned)
    ));
    push_web_service_text_property(
        xml,
        "\t\t\t\t\t",
        "ProcedureName",
        &operation.procedure_name,
    );
    xml.push_str(&format!(
        "\t\t\t\t\t<DataLockControlMode>{}</DataLockControlMode>\r\n\
\t\t\t\t</Properties>\r\n",
        operation.data_lock_control_mode
    ));
    if operation.parameters.is_empty() {
        xml.push_str("\t\t\t\t<ChildObjects/>\r\n");
    } else {
        xml.push_str("\t\t\t\t<ChildObjects>\r\n");
        for parameter in &operation.parameters {
            push_web_service_parameter_xml(xml, parameter);
        }
        xml.push_str("\t\t\t\t</ChildObjects>\r\n");
    }
    xml.push_str("\t\t\t</Operation>\r\n");
}

fn push_web_service_parameter_xml(xml: &mut String, parameter: &WebServiceParameterProperties) {
    xml.push_str(&format!(
        "\t\t\t\t\t<Parameter uuid=\"{}\">\r\n\
\t\t\t\t\t\t<Properties>\r\n",
        escape_xml_text(&parameter.header.uuid)
    ));
    push_web_service_header_properties_xml(xml, "\t\t\t\t\t\t\t", &parameter.header);
    push_web_service_xdto_type_xml(
        xml,
        "\t\t\t\t\t\t\t",
        "XDTOValueType",
        &parameter.value_type,
        "d8p1",
    );
    xml.push_str(&format!(
        "\t\t\t\t\t\t\t<Nillable>{}</Nillable>\r\n\
\t\t\t\t\t\t\t<TransferDirection>{}</TransferDirection>\r\n\
\t\t\t\t\t\t</Properties>\r\n\
\t\t\t\t\t</Parameter>\r\n",
        xml_bool(parameter.nillable),
        parameter.transfer_direction
    ));
}

fn push_web_service_header_properties_xml(xml: &mut String, indent: &str, header: &MetadataHeader) {
    xml.push_str(&format!(
        "{indent}<Name>{}</Name>\r\n",
        escape_xml_element_text(&header.name)
    ));
    push_header_synonym_xml(xml, indent, &header.synonyms);
    push_web_service_text_property(xml, indent, "Comment", &header.comment);
}

fn push_web_service_xdto_type_xml(
    xml: &mut String,
    indent: &str,
    tag: &str,
    value_type: &WebServiceXdtoType,
    custom_prefix: &str,
) {
    let (prefix, namespace_attribute) = match value_type.namespace.as_str() {
        XDTO_XML_SCHEMA_NAMESPACE => ("xs", None),
        XDTO_CORE_NAMESPACE => ("v8", None),
        namespace => (custom_prefix, Some(namespace)),
    };
    let namespace_attribute = namespace_attribute
        .map(|namespace| {
            format!(
                " xmlns:{}=\"{}\"",
                custom_prefix,
                escape_xml_text(namespace)
            )
        })
        .unwrap_or_default();
    xml.push_str(&format!(
        "{indent}<{tag}{namespace_attribute}>{prefix}:{}</{tag}>\r\n",
        escape_xml_element_text(&value_type.name)
    ));
}

fn push_web_service_text_property(xml: &mut String, indent: &str, tag: &str, value: &str) {
    if value.is_empty() {
        xml.push_str(&format!("{indent}<{tag}/>\r\n"));
    } else {
        xml.push_str(&format!(
            "{indent}<{tag}>{}</{tag}>\r\n",
            escape_xml_element_text(value)
        ));
    }
}

fn format_http_service_source_xml(
    header: &MetadataHeader,
    service: &HttpServiceProperties,
    source_version: InfobaseConfigSourceVersion,
) -> String {
    let mut xml = format_full_metadata_source_xml("HTTPService", header, source_version);
    let insert = format!(
        "\t\t\t<RootURL>{}</RootURL>\r\n\
\t\t\t<ReuseSessions>{}</ReuseSessions>\r\n\
\t\t\t<SessionMaxAge>{}</SessionMaxAge>\r\n",
        escape_xml_element_text(&service.root_url),
        escape_xml_text(service.reuse_sessions),
        service.session_max_age
    );
    let marker = "\t\t</Properties>\r\n";
    if let Some(index) = xml.find(marker) {
        xml.insert_str(index, &insert);
    }
    if !service.url_templates.is_empty() {
        let mut child_xml = "\t\t<ChildObjects>\r\n".to_string();
        for template in &service.url_templates {
            push_http_service_url_template_xml(&mut child_xml, template);
        }
        child_xml.push_str("\t\t</ChildObjects>\r\n");
        let owner_end = "\t</HTTPService>\r\n";
        if let Some(index) = xml.find(owner_end) {
            xml.insert_str(index, &child_xml);
        }
    }
    xml
}

fn push_http_service_url_template_xml(
    xml: &mut String,
    template: &HttpServiceUrlTemplateProperties,
) {
    xml.push_str(&format!(
        "\t\t\t<URLTemplate uuid=\"{}\">\r\n\
\t\t\t\t<Properties>\r\n\
\t\t\t\t\t<Name>{}</Name>\r\n",
        escape_xml_text(&template.header.uuid),
        escape_xml_element_text(&template.header.name)
    ));
    push_header_synonym_xml(xml, "\t\t\t\t\t", &template.header.synonyms);
    if template.header.comment.is_empty() {
        xml.push_str("\t\t\t\t\t<Comment/>\r\n");
    } else {
        xml.push_str(&format!(
            "\t\t\t\t\t<Comment>{}</Comment>\r\n",
            escape_xml_element_text(&template.header.comment)
        ));
    }
    xml.push_str(&format!(
        "\t\t\t\t\t<Template>{}</Template>\r\n\
\t\t\t\t</Properties>\r\n",
        escape_xml_element_text(&template.template)
    ));
    if !template.methods.is_empty() {
        xml.push_str("\t\t\t\t<ChildObjects>\r\n");
        for method in &template.methods {
            push_http_service_method_xml(xml, method);
        }
        xml.push_str("\t\t\t\t</ChildObjects>\r\n");
    }
    xml.push_str("\t\t\t</URLTemplate>\r\n");
}

fn push_http_service_method_xml(xml: &mut String, method: &HttpServiceMethodProperties) {
    xml.push_str(&format!(
        "\t\t\t\t\t<Method uuid=\"{}\">\r\n\
\t\t\t\t\t\t<Properties>\r\n\
\t\t\t\t\t\t\t<Name>{}</Name>\r\n",
        escape_xml_text(&method.header.uuid),
        escape_xml_element_text(&method.header.name)
    ));
    push_header_synonym_xml(xml, "\t\t\t\t\t\t\t", &method.header.synonyms);
    if method.header.comment.is_empty() {
        xml.push_str("\t\t\t\t\t\t\t<Comment/>\r\n");
    } else {
        xml.push_str(&format!(
            "\t\t\t\t\t\t\t<Comment>{}</Comment>\r\n",
            escape_xml_element_text(&method.header.comment)
        ));
    }
    xml.push_str(&format!(
        "\t\t\t\t\t\t\t<HTTPMethod>{}</HTTPMethod>\r\n\
\t\t\t\t\t\t\t<Handler>{}</Handler>\r\n\
\t\t\t\t\t\t</Properties>\r\n\
\t\t\t\t\t</Method>\r\n",
        escape_xml_element_text(&method.http_method),
        escape_xml_element_text(&method.handler)
    ));
}

fn format_language_source_xml(
    header: &MetadataHeader,
    language: &LanguageProperties,
    source_version: InfobaseConfigSourceVersion,
) -> String {
    let mut xml = format_full_metadata_source_xml("Language", header, source_version);
    let insert = format!(
        "\t\t\t<LanguageCode>{}</LanguageCode>\r\n",
        escape_xml_element_text(&language.language_code)
    );
    let marker = "\t\t</Properties>\r\n";
    if let Some(index) = xml.find(marker) {
        xml.insert_str(index, &insert);
    }
    xml
}

fn format_document_numerator_source_xml(
    header: &MetadataHeader,
    properties: &DocumentNumeratorProperties,
    source_version: InfobaseConfigSourceVersion,
) -> String {
    let mut xml = format_full_metadata_source_xml("DocumentNumerator", header, source_version);
    let insert = format!(
        "\t\t\t<NumberType>{}</NumberType>\r\n\
\t\t\t<NumberLength>{}</NumberLength>\r\n\
\t\t\t<NumberAllowedLength>{}</NumberAllowedLength>\r\n\
\t\t\t<NumberPeriodicity>{}</NumberPeriodicity>\r\n\
\t\t\t<CheckUnique>{}</CheckUnique>\r\n",
        properties.number_type,
        properties.number_length,
        properties.number_allowed_length,
        properties.number_periodicity,
        xml_bool(properties.check_unique)
    );
    let marker = "\t\t</Properties>\r\n";
    if let Some(index) = xml.find(marker) {
        xml.insert_str(index, &insert);
    }
    xml
}

fn format_ws_reference_source_xml(
    header: &MetadataHeader,
    properties: &WSReferenceProperties,
    source_version: InfobaseConfigSourceVersion,
) -> String {
    let mut xml = format_full_metadata_source_xml("WSReference", header, source_version);
    let internal = format!(
        "\t\t<InternalInfo>\r\n\
\t\t\t<xr:GeneratedType name=\"WSReferenceManager.{}\" category=\"Manager\">\r\n\
\t\t\t\t<xr:TypeId>{}</xr:TypeId>\r\n\
\t\t\t\t<xr:ValueId>{}</xr:ValueId>\r\n\
\t\t\t</xr:GeneratedType>\r\n\
\t\t</InternalInfo>\r\n",
        escape_xml_text(&header.name),
        escape_xml_text(&properties.manager_type_id),
        escape_xml_text(&properties.manager_value_id)
    );
    if let Some(index) = xml.find("\t\t<Properties>\r\n") {
        xml.insert_str(index, &internal);
    }
    let insert = format!(
        "\t\t\t<LocationURL>{}</LocationURL>\r\n",
        escape_xml_element_text(&properties.location_url)
    );
    let marker = "\t\t</Properties>\r\n";
    if let Some(index) = xml.find(marker) {
        xml.insert_str(index, &insert);
    }
    xml
}

fn format_integration_service_source_xml(
    header: &MetadataHeader,
    properties: &IntegrationServiceProperties,
    source_version: InfobaseConfigSourceVersion,
) -> String {
    let mut xml = format_full_metadata_source_xml("IntegrationService", header, source_version);
    let internal = format!(
        "\t\t<InternalInfo>\r\n\
\t\t\t<xr:GeneratedType name=\"IntegrationServiceManager.{}\" category=\"Manager\">\r\n\
\t\t\t\t<xr:TypeId>{}</xr:TypeId>\r\n\
\t\t\t\t<xr:ValueId>{}</xr:ValueId>\r\n\
\t\t\t</xr:GeneratedType>\r\n\
\t\t</InternalInfo>\r\n",
        escape_xml_text(&header.name),
        escape_xml_text(&properties.manager_type_id),
        escape_xml_text(&properties.manager_value_id)
    );
    if let Some(index) = xml.find("\t\t<Properties>\r\n") {
        xml.insert_str(index, &internal);
    }
    let insert = if properties.external_address.is_empty() {
        "\t\t\t<ExternalIntegrationServiceAddress/>\r\n".to_string()
    } else {
        format!(
            "\t\t\t<ExternalIntegrationServiceAddress>{}</ExternalIntegrationServiceAddress>\r\n",
            escape_xml_element_text(&properties.external_address)
        )
    };
    if let Some(index) = xml.find("\t\t</Properties>\r\n") {
        xml.insert_str(index, &insert);
    }
    if properties.channels.is_empty() {
        return xml;
    }
    let mut child_objects = "\t\t<ChildObjects>\r\n".to_string();
    for channel in &properties.channels {
        push_integration_service_channel_xml(&mut child_objects, &header.name, channel);
    }
    child_objects.push_str("\t\t</ChildObjects>\r\n");
    let marker = "\t</IntegrationService>\r\n";
    if let Some(index) = xml.find(marker) {
        xml.insert_str(index, &child_objects);
    }
    xml
}

fn push_integration_service_channel_xml(
    xml: &mut String,
    service_name: &str,
    channel: &IntegrationServiceChannelProperties,
) {
    xml.push_str(&format!(
        "\t\t\t<IntegrationServiceChannel uuid=\"{}\">\r\n\
\t\t\t\t<InternalInfo>\r\n\
\t\t\t\t\t<xr:GeneratedType name=\"IntegrationServiceChannelManager.{}.{}\" category=\"Manager\">\r\n\
\t\t\t\t\t\t<xr:TypeId>{}</xr:TypeId>\r\n\
\t\t\t\t\t\t<xr:ValueId>{}</xr:ValueId>\r\n\
\t\t\t\t\t</xr:GeneratedType>\r\n\
\t\t\t\t</InternalInfo>\r\n\
\t\t\t\t<Properties>\r\n\
\t\t\t\t\t<Name>{}</Name>\r\n",
        escape_xml_text(&channel.header.uuid),
        escape_xml_text(service_name),
        escape_xml_text(&channel.header.name),
        escape_xml_text(&channel.manager_type_id),
        escape_xml_text(&channel.manager_value_id),
        escape_xml_element_text(&channel.header.name)
    ));
    push_header_synonym_xml(xml, "\t\t\t\t\t", &channel.header.synonyms);
    if channel.header.comment.is_empty() {
        xml.push_str("\t\t\t\t\t<Comment/>\r\n");
    } else {
        xml.push_str(&format!(
            "\t\t\t\t\t<Comment>{}</Comment>\r\n",
            escape_xml_element_text(&channel.header.comment)
        ));
    }
    xml.push_str(&format!(
        "\t\t\t\t\t<ExternalIntegrationServiceChannelName>{}</ExternalIntegrationServiceChannelName>\r\n\
\t\t\t\t\t<MessageDirection>{}</MessageDirection>\r\n",
        escape_xml_element_text(&channel.external_name),
        channel.message_direction
    ));
    if channel.receive_message_processing.is_empty() {
        xml.push_str("\t\t\t\t\t<ReceiveMessageProcessing/>\r\n");
    } else {
        xml.push_str(&format!(
            "\t\t\t\t\t<ReceiveMessageProcessing>{}</ReceiveMessageProcessing>\r\n",
            escape_xml_element_text(&channel.receive_message_processing)
        ));
    }
    xml.push_str(&format!(
        "\t\t\t\t\t<Transactioned>{}</Transactioned>\r\n\
\t\t\t\t</Properties>\r\n\
\t\t\t</IntegrationServiceChannel>\r\n",
        xml_bool(channel.transactioned)
    ));
}

fn format_metadata_types_xml(value_types: &[ConstantValueType]) -> String {
    format_metadata_types_xml_with_indent(value_types, "\t\t\t")
}

fn format_form_metadata_types_xml(value_types: &[ConstantValueType]) -> String {
    format_form_metadata_types_xml_with_indent(value_types, "\t\t\t")
}

fn format_form_metadata_types_xml_with_indent(
    value_types: &[ConstantValueType],
    indent: &str,
) -> String {
    let nested = format!("{indent}\t");
    let nested2 = format!("{nested}\t");
    let mut xml = format!("{indent}<Type>\r\n");
    for value_type in value_types {
        let tag_name = match value_type {
            ConstantValueType::Reference { reference } => form_reference_type_tag(reference),
            ConstantValueType::ReferenceTypeSet { .. } => "TypeSet",
            _ => "Type",
        };
        let namespace_attr = metadata_type_xml_namespace_attr(value_type);
        xml.push_str(&format!(
            "{nested}<v8:{tag_name}{namespace_attr}>{}</v8:{tag_name}>\r\n",
            metadata_type_xml_name(value_type)
        ));
    }

    if let Some(number) = value_types.iter().find_map(|value_type| match value_type {
        ConstantValueType::Number {
            digits,
            fraction_digits,
            allowed_sign_flag,
        } => Some((*digits, *fraction_digits, *allowed_sign_flag)),
        _ => None,
    }) {
        xml.push_str(&format!("{nested}<v8:NumberQualifiers>\r\n"));
        xml.push_str(&format!("{nested2}<v8:Digits>{}</v8:Digits>\r\n", number.0));
        xml.push_str(&format!(
            "{nested2}<v8:FractionDigits>{}</v8:FractionDigits>\r\n",
            number.1
        ));
        xml.push_str(&format!(
            "{nested2}<v8:AllowedSign>{}</v8:AllowedSign>\r\n",
            number_allowed_sign_xml(number.2)
        ));
        xml.push_str(&format!("{nested}</v8:NumberQualifiers>\r\n"));
    }

    if let Some(string) = value_types.iter().find_map(|value_type| match value_type {
        ConstantValueType::String {
            length,
            allowed_length_flag,
        } => Some((
            length.unwrap_or(0),
            length.map(|_| *allowed_length_flag).unwrap_or(1),
        )),
        _ => None,
    }) {
        xml.push_str(&format!("{nested}<v8:StringQualifiers>\r\n"));
        xml.push_str(&format!("{nested2}<v8:Length>{}</v8:Length>\r\n", string.0));
        xml.push_str(&format!(
            "{nested2}<v8:AllowedLength>{}</v8:AllowedLength>\r\n",
            string_allowed_length_xml(string.1)
        ));
        xml.push_str(&format!("{nested}</v8:StringQualifiers>\r\n"));
    }

    if let Some(date_fractions) = value_types.iter().find_map(|value_type| {
        if let ConstantValueType::DateTime { date_fractions } = value_type {
            Some(*date_fractions)
        } else {
            None
        }
    }) {
        xml.push_str(&format!(
            "{nested}<v8:DateQualifiers>\r\n\
{nested2}<v8:DateFractions>{date_fractions}</v8:DateFractions>\r\n\
{nested}</v8:DateQualifiers>\r\n"
        ));
    }

    xml.push_str(&format!("{indent}</Type>\r\n"));
    xml
}

fn format_type_description_value_types_xml(
    value_types: &[ConstantValueType],
    indent: &str,
) -> String {
    let nested = format!("{indent}\t");
    let mut xml = String::new();
    for value_type in value_types {
        let tag_name = metadata_type_xml_tag(value_type);
        let namespace_attr = metadata_type_xml_namespace_attr(value_type);
        xml.push_str(&format!(
            "{indent}<v8:{tag_name}{namespace_attr}>{}</v8:{tag_name}>\r\n",
            metadata_type_xml_name(value_type)
        ));
    }
    if let Some(number) = value_types.iter().find_map(|value_type| match value_type {
        ConstantValueType::Number {
            digits,
            fraction_digits,
            allowed_sign_flag,
        } => Some((*digits, *fraction_digits, *allowed_sign_flag)),
        _ => None,
    }) {
        xml.push_str(&format!("{indent}<v8:NumberQualifiers>\r\n"));
        xml.push_str(&format!("{nested}<v8:Digits>{}</v8:Digits>\r\n", number.0));
        xml.push_str(&format!(
            "{nested}<v8:FractionDigits>{}</v8:FractionDigits>\r\n",
            number.1
        ));
        xml.push_str(&format!(
            "{nested}<v8:AllowedSign>{}</v8:AllowedSign>\r\n",
            number_allowed_sign_xml(number.2)
        ));
        xml.push_str(&format!("{indent}</v8:NumberQualifiers>\r\n"));
    }
    if let Some(string) = value_types.iter().find_map(|value_type| match value_type {
        ConstantValueType::String {
            length,
            allowed_length_flag,
        } => Some((
            length.unwrap_or(0),
            length.map(|_| *allowed_length_flag).unwrap_or(1),
        )),
        _ => None,
    }) {
        xml.push_str(&format!("{indent}<v8:StringQualifiers>\r\n"));
        xml.push_str(&format!("{nested}<v8:Length>{}</v8:Length>\r\n", string.0));
        xml.push_str(&format!(
            "{nested}<v8:AllowedLength>{}</v8:AllowedLength>\r\n",
            string_allowed_length_xml(string.1)
        ));
        xml.push_str(&format!("{indent}</v8:StringQualifiers>\r\n"));
    }
    if let Some(date_fractions) = value_types.iter().find_map(|value_type| {
        if let ConstantValueType::DateTime { date_fractions } = value_type {
            Some(*date_fractions)
        } else {
            None
        }
    }) {
        xml.push_str(&format!(
            "{indent}<v8:DateQualifiers>\r\n\
{nested}<v8:DateFractions>{date_fractions}</v8:DateFractions>\r\n\
{indent}</v8:DateQualifiers>\r\n"
        ));
    }
    xml
}

fn format_metadata_types_xml_with_indent(
    value_types: &[ConstantValueType],
    indent: &str,
) -> String {
    let nested = format!("{indent}\t");
    let nested2 = format!("{nested}\t");
    let mut xml = format!("{indent}<Type>\r\n");
    for value_type in value_types {
        let tag_name = metadata_type_xml_tag(value_type);
        let namespace_attr = metadata_type_xml_namespace_attr(value_type);
        xml.push_str(&format!(
            "{nested}<v8:{tag_name}{namespace_attr}>{}</v8:{tag_name}>\r\n",
            metadata_type_xml_name(value_type)
        ));
    }

    if let Some(number) = value_types.iter().find_map(|value_type| match value_type {
        ConstantValueType::Number {
            digits,
            fraction_digits,
            allowed_sign_flag,
        } => Some((*digits, *fraction_digits, *allowed_sign_flag)),
        _ => None,
    }) {
        xml.push_str(&format!("{nested}<v8:NumberQualifiers>\r\n"));
        xml.push_str(&format!("{nested2}<v8:Digits>{}</v8:Digits>\r\n", number.0));
        xml.push_str(&format!(
            "{nested2}<v8:FractionDigits>{}</v8:FractionDigits>\r\n",
            number.1
        ));
        xml.push_str(&format!(
            "{nested2}<v8:AllowedSign>{}</v8:AllowedSign>\r\n",
            number_allowed_sign_xml(number.2)
        ));
        xml.push_str(&format!("{nested}</v8:NumberQualifiers>\r\n"));
    }

    if let Some(string) = value_types.iter().find_map(|value_type| match value_type {
        ConstantValueType::String {
            length,
            allowed_length_flag,
        } => Some((
            length.unwrap_or(0),
            length.map(|_| *allowed_length_flag).unwrap_or(1),
        )),
        _ => None,
    }) {
        xml.push_str(&format!("{nested}<v8:StringQualifiers>\r\n"));
        xml.push_str(&format!("{nested2}<v8:Length>{}</v8:Length>\r\n", string.0));
        xml.push_str(&format!(
            "{nested2}<v8:AllowedLength>{}</v8:AllowedLength>\r\n",
            string_allowed_length_xml(string.1)
        ));
        xml.push_str(&format!("{nested}</v8:StringQualifiers>\r\n"));
    }

    if let Some(date_fractions) = value_types.iter().find_map(|value_type| {
        if let ConstantValueType::DateTime { date_fractions } = value_type {
            Some(*date_fractions)
        } else {
            None
        }
    }) {
        xml.push_str(&format!(
            "{nested}<v8:DateQualifiers>\r\n\
{nested2}<v8:DateFractions>{date_fractions}</v8:DateFractions>\r\n\
{nested}</v8:DateQualifiers>\r\n"
        ));
    }

    xml.push_str(&format!("{indent}</Type>\r\n"));
    xml
}

fn metadata_type_xml_tag(value_type: &ConstantValueType) -> &'static str {
    match value_type {
        ConstantValueType::Reference { reference } => constant_reference_type_tag(reference),
        ConstantValueType::ReferenceTypeSet { .. } => "TypeSet",
        _ => "Type",
    }
}

fn metadata_type_xml_name(value_type: &ConstantValueType) -> String {
    match value_type {
        ConstantValueType::Boolean => "xs:boolean".to_string(),
        ConstantValueType::String { .. } => "xs:string".to_string(),
        ConstantValueType::Number { .. } => "xs:decimal".to_string(),
        ConstantValueType::DateTime { .. } => "xs:dateTime".to_string(),
        ConstantValueType::Reference { reference, .. }
        | ConstantValueType::ReferenceTypeSet { reference, .. } => reference.clone(),
    }
}

fn metadata_type_xml_namespace_attr(value_type: &ConstantValueType) -> &'static str {
    match value_type {
        ConstantValueType::Reference { reference }
        | ConstantValueType::ReferenceTypeSet { reference }
            if reference.starts_with("mxl:") =>
        {
            r#" xmlns:mxl="http://v8.1c.ru/8.2/data/spreadsheet""#
        }
        _ => "",
    }
}

fn format_constant_type_xml(value_type: &ConstantValueType) -> String {
    match value_type {
        ConstantValueType::Boolean => {
            "\t\t\t<Type>\r\n\t\t\t\t<v8:Type>xs:boolean</v8:Type>\r\n\t\t\t</Type>\r\n".to_string()
        }
        ConstantValueType::String {
            length,
            allowed_length_flag,
        } => {
            let mut xml = "\t\t\t<Type>\r\n\t\t\t\t<v8:Type>xs:string</v8:Type>\r\n".to_string();
            let allowed_length = if length.is_some() {
                string_allowed_length_xml(*allowed_length_flag)
            } else {
                "Variable"
            };
            let length = length.unwrap_or(0);
            xml.push_str("\t\t\t\t<v8:StringQualifiers>\r\n");
            xml.push_str(&format!("\t\t\t\t\t<v8:Length>{length}</v8:Length>\r\n"));
            xml.push_str(&format!(
                "\t\t\t\t\t<v8:AllowedLength>{}</v8:AllowedLength>\r\n",
                allowed_length
            ));
            xml.push_str("\t\t\t\t</v8:StringQualifiers>\r\n");
            xml.push_str("\t\t\t</Type>\r\n");
            xml
        }
        ConstantValueType::Number {
            digits,
            fraction_digits,
            allowed_sign_flag,
        } => format!(
            "\t\t\t<Type>\r\n\
\t\t\t\t<v8:Type>xs:decimal</v8:Type>\r\n\
\t\t\t\t<v8:NumberQualifiers>\r\n\
\t\t\t\t\t<v8:Digits>{digits}</v8:Digits>\r\n\
\t\t\t\t\t<v8:FractionDigits>{fraction_digits}</v8:FractionDigits>\r\n\
\t\t\t\t\t<v8:AllowedSign>{}</v8:AllowedSign>\r\n\
\t\t\t\t</v8:NumberQualifiers>\r\n\
\t\t\t</Type>\r\n",
            number_allowed_sign_xml(*allowed_sign_flag)
        ),
        ConstantValueType::DateTime { date_fractions } => format!(
            "\t\t\t<Type>\r\n\
\t\t\t\t<v8:Type>xs:dateTime</v8:Type>\r\n\
\t\t\t\t<v8:DateQualifiers>\r\n\
\t\t\t\t\t<v8:DateFractions>{date_fractions}</v8:DateFractions>\r\n\
\t\t\t\t</v8:DateQualifiers>\r\n\
\t\t\t</Type>\r\n"
        ),
        ConstantValueType::Reference { reference, .. } => {
            let tag = constant_reference_type_tag(reference);
            let namespace_attr = metadata_type_xml_namespace_attr(value_type);
            format!(
                "\t\t\t<Type>\r\n\t\t\t\t<v8:{tag}{namespace_attr}>{}</v8:{tag}>\r\n\t\t\t</Type>\r\n",
                escape_xml_text(reference)
            )
        }
        ConstantValueType::ReferenceTypeSet { reference, .. } => {
            let namespace_attr = metadata_type_xml_namespace_attr(value_type);
            format!(
                "\t\t\t<Type>\r\n\t\t\t\t<v8:TypeSet{namespace_attr}>{}</v8:TypeSet>\r\n\t\t\t</Type>\r\n",
                escape_xml_text(reference)
            )
        }
    }
}

fn constant_reference_type_tag(reference: &str) -> &'static str {
    if reference.starts_with("cfg:DefinedType.") || reference == "cfg:ExchangePlanRef" {
        "TypeSet"
    } else {
        "Type"
    }
}

fn form_reference_type_tag(reference: &str) -> &'static str {
    if reference.starts_with("cfg:DefinedType.")
        || reference == "cfg:AnyIBRef"
        || reference == "cfg:ExchangePlanRef"
        || (reference.starts_with("cfg:") && reference.ends_with("Ref") && !reference.contains('.'))
    {
        "TypeSet"
    } else {
        "Type"
    }
}

fn string_allowed_length_xml(value: u8) -> &'static str {
    match value {
        0 => "Fixed",
        _ => "Variable",
    }
}

fn number_allowed_sign_xml(value: u8) -> &'static str {
    match value {
        1 => "Nonnegative",
        _ => "Any",
    }
}

fn xml_bool(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

fn return_values_reuse_xml(value: ReturnValuesReuseValue) -> &'static str {
    match value {
        ReturnValuesReuseValue::DontUse => "DontUse",
        ReturnValuesReuseValue::DuringRequest => "DuringRequest",
        ReturnValuesReuseValue::DuringSession => "DuringSession",
    }
}

fn escape_xml_text(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' => output.push_str("&quot;"),
            _ => output.push(ch),
        }
    }
    output
}

fn escape_xml_element_text(value: &str) -> String {
    let value = value.replace("\r\n", "\n");
    let mut output = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            _ => output.push(ch),
        }
    }
    output
}

fn sanitize_source_path_segment(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
            output.push('_');
        } else {
            output.push(ch);
        }
    }
    if output.trim().is_empty() {
        "Unnamed".to_string()
    } else {
        output
    }
}

fn selected_file_names_from_args(
    file_names: &[String],
    file_name_lists: &[PathBuf],
) -> Result<BTreeSet<String>> {
    let mut combined = file_names.to_vec();
    for list_path in file_name_lists {
        let content = fs::read_to_string(list_path)
            .with_context(|| format!("failed to read {}", list_path.display()))?;
        combined.extend(
            content
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty() && !line.starts_with('#'))
                .map(str::to_string),
        );
    }
    Ok(expand_selected_file_names(&combined))
}

fn expand_selected_file_names(file_names: &[String]) -> BTreeSet<String> {
    let mut selected = BTreeSet::new();
    for file_name in file_names {
        let file_name = file_name.trim();
        if file_name.is_empty() {
            continue;
        }
        selected.insert(file_name.to_string());
        if let Some(metadata_id) = metadata_id_from_module_file_name(file_name) {
            selected.insert(metadata_id.to_string());
            continue;
        }
        for suffix in MODULE_BODY_SUFFIXES {
            selected.insert(format!("{file_name}.{suffix}"));
        }
    }
    selected
}

fn metadata_id_from_module_file_name(file_name: &str) -> Option<&str> {
    let (metadata_id, suffix) = file_name.rsplit_once('.')?;
    if metadata_id.is_empty() || !MODULE_BODY_SUFFIXES.contains(&suffix) {
        return None;
    }
    Some(metadata_id)
}

const MODULE_BODY_SUFFIXES: &[&str] = &["0", "1", "2", "3", "5", "6", "7", "8", "14", "15", "16"];

fn prepare_output_dir(path: &Path, overwrite: bool) -> Result<()> {
    if path.exists() {
        if !path.is_dir() {
            bail!(
                "output path exists and is not a directory: {}",
                path.display()
            );
        }
        if fs::read_dir(path)?.next().is_some() && !overwrite {
            bail!(
                "output directory is not empty: {}. Pass --overwrite to replace it",
                path.display()
            );
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

fn inflate_raw_deflate(input: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = DeflateDecoder::new(input);
    let mut output = Vec::new();
    decoder
        .read_to_end(&mut output)
        .context("failed to inflate raw deflate blob")?;
    Ok(output)
}

fn encode_hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn decode_hex(hex: &str) -> Result<Vec<u8>> {
    let hex = hex.trim().strip_prefix("0x").unwrap_or(hex.trim());
    if !hex.len().is_multiple_of(2) {
        return Err(anyhow!("hex string has odd length"));
    }
    (0..hex.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&hex[index..index + 2], 16)
                .with_context(|| format!("invalid hex byte at offset {index}"))
        })
        .collect()
}

#[cfg(test)]
fn extract_json_array(stdout: &str, context: &str) -> Result<String> {
    let start = stdout
        .find('[')
        .ok_or_else(|| anyhow!("{context}: sqlcmd output does not contain JSON array"))?;
    let end = stdout
        .rfind(']')
        .ok_or_else(|| anyhow!("{context}: sqlcmd output does not contain JSON array end"))?;
    if end < start {
        return Err(anyhow!("{context}: invalid JSON array boundaries"));
    }
    Ok(stdout[start..=end].to_string())
}

fn quote_ident(value: &str) -> String {
    format!("[{}]", value.replace(']', "]]"))
}

fn qualified_storage_table(database: &str, table: &str) -> String {
    format!("{}.dbo.{}", quote_ident(database), quote_ident(table))
}

fn quote_string(value: &str) -> String {
    value.replace('\'', "''")
}

fn safe_storage_file_name(file_name: &str, part_no: i32) -> String {
    let mut safe = String::with_capacity(file_name.len() + 16);
    for ch in file_name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
            safe.push(ch);
        } else {
            safe.push('_');
        }
    }
    if safe.is_empty() {
        safe.push_str("row");
    }
    format!("{safe}__part{part_no}")
}

#[cfg(test)]
mod tests;
